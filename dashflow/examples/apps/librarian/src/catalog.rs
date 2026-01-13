//! Gutenberg catalog integration for dynamic book discovery
//!
//! This module provides access to Project Gutenberg's catalog for discovering
//! and loading books dynamically, rather than using hardcoded book lists.
//!
//! ## Data Sources
//!
//! 1. **Book Index**: CSV list of all book IDs from Gutenberg
//! 2. **Metadata API**: JSON API for individual book metadata
//! 3. **Local Cache**: Cached metadata to avoid repeated API calls
//!
//! ## Usage
//!
//! ```rust,ignore
//! let catalog = GutenbergCatalog::new("data/catalog").await?;
//!
//! // Get all English text books
//! let books = catalog.list_english_books().await?;
//!
//! // Get metadata for a specific book
//! let meta = catalog.get_metadata(1342).await?;
//! ```

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info, warn};

/// Metadata for a Gutenberg book from the catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// Gutenberg book ID
    pub id: u32,
    /// Book title
    pub title: String,
    /// Author(s)
    pub authors: Vec<String>,
    /// Language code (e.g., "en", "fr", "de")
    pub language: String,
    /// Subjects/genres
    pub subjects: Vec<String>,
    /// Available formats
    pub formats: Vec<String>,
    /// Download count (popularity metric)
    pub download_count: Option<u32>,
}

impl CatalogEntry {
    /// Get primary author name
    pub fn primary_author(&self) -> &str {
        self.authors
            .first()
            .map(|s| s.as_str())
            .unwrap_or("Unknown")
    }

    /// Check if this is a text book (not audio, image, etc.)
    pub fn is_text(&self) -> bool {
        self.formats.iter().any(|f| f.contains("text/plain"))
    }

    /// Check if this is an English book
    pub fn is_english(&self) -> bool {
        self.language == "en"
    }
}

/// Gutenberg catalog for dynamic book discovery
pub struct GutenbergCatalog {
    client: Client,
    cache_dir: PathBuf,
    /// In-memory cache of book metadata
    metadata_cache: HashMap<u32, CatalogEntry>,
    /// Cached list of all book IDs
    book_ids: Vec<u32>,
}

impl GutenbergCatalog {
    /// Create a new catalog instance with the specified cache directory
    pub async fn new(cache_dir: impl Into<PathBuf>) -> Result<Self> {
        let cache_dir = cache_dir.into();
        fs::create_dir_all(&cache_dir).await?;

        let client = Client::builder()
            .user_agent("DashFlow-Librarian/1.0 (https://github.com/dropbox/dTOOL/dashflow)")
            .build()
            .context("Failed to create HTTP client")?;

        let mut catalog = Self {
            client,
            cache_dir,
            metadata_cache: HashMap::new(),
            book_ids: Vec::new(),
        };

        // Load cached book IDs if available
        catalog.load_book_ids_cache().await?;

        Ok(catalog)
    }

    /// Load book IDs from local cache
    async fn load_book_ids_cache(&mut self) -> Result<()> {
        let cache_path = self.cache_dir.join("book_ids.json");
        if cache_path.exists() {
            let content = fs::read_to_string(&cache_path).await?;
            self.book_ids = serde_json::from_str(&content)?;
            info!("Loaded {} book IDs from cache", self.book_ids.len());
        }
        Ok(())
    }

    /// Save book IDs to local cache
    async fn save_book_ids_cache(&self) -> Result<()> {
        let cache_path = self.cache_dir.join("book_ids.json");
        let content = serde_json::to_string(&self.book_ids)?;
        fs::write(&cache_path, content).await?;
        Ok(())
    }

    /// Fetch the list of all book IDs from Gutenberg
    ///
    /// This uses Gutenberg's robot-friendly book list endpoint with pagination.
    /// The list is cached locally to avoid repeated downloads.
    /// URLs are in format: `https://aleph.gutenberg.org/path/to/BOOKID/BOOKID.zip`
    pub async fn refresh_book_list(&mut self) -> Result<usize> {
        self.refresh_book_list_limited(None).await
    }

    /// Fetch book IDs with optional limit for testing
    pub async fn refresh_book_list_limited(&mut self, max_books: Option<usize>) -> Result<usize> {
        info!("Fetching book list from Project Gutenberg...");

        let mut ids: Vec<u32> = Vec::new();
        let mut offset: usize = 0;
        let base_url = "https://www.gutenberg.org/robot/harvest";

        loop {
            let url = format!("{}?offset={}&filetypes[]=txt&langs[]=en", base_url, offset);
            debug!("Fetching page at offset {}", offset);

            let response = self
                .client
                .get(&url)
                .send()
                .await
                .context("Failed to fetch book list")?;

            if !response.status().is_success() {
                anyhow::bail!("Failed to fetch book list: HTTP {}", response.status());
            }

            let html = response.text().await?;

            // Parse book IDs from the harvest page
            // URLs look like: https://aleph.gutenberg.org/1/0/0/8/10084/10084.zip
            for line in html.lines() {
                if let Some(start) = line.find("aleph.gutenberg.org/") {
                    let rest = &line[start + 20..];
                    if let Some(zip_pos) = rest.find(".zip") {
                        let before_zip = &rest[..zip_pos];
                        if let Some(last_slash) = before_zip.rfind('/') {
                            let filename = &before_zip[last_slash + 1..];
                            let id_str = filename.split('-').next().unwrap_or(filename);
                            if let Ok(id) = id_str.parse::<u32>() {
                                ids.push(id);
                            }
                        }
                    }
                }
            }

            // Check for next page
            let next_offset = if let Some(next_pos) = html.find("harvest?offset=") {
                let rest = &html[next_pos + 15..];
                if let Some(amp_pos) = rest.find('&') {
                    rest[..amp_pos].parse::<usize>().ok()
                } else {
                    None
                }
            } else {
                None
            };

            // Check user-specified limit
            if let Some(max) = max_books {
                if ids.len() >= max {
                    info!("Reached requested limit of {} books", max);
                    break;
                }
            }

            match next_offset {
                Some(next) if next > offset => {
                    offset = next;
                    // Small delay to be polite to the server
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                _ => break,
            }
        }

        ids.sort_unstable();
        ids.dedup();

        self.book_ids = ids;
        self.save_book_ids_cache().await?;

        info!("Found {} English text books", self.book_ids.len());
        Ok(self.book_ids.len())
    }

    /// Get the list of all known book IDs
    pub fn book_ids(&self) -> &[u32] {
        &self.book_ids
    }

    /// Get metadata for a specific book
    ///
    /// Fetches from API if not cached, caches result locally.
    pub async fn get_metadata(&mut self, id: u32) -> Result<CatalogEntry> {
        // Check in-memory cache
        if let Some(entry) = self.metadata_cache.get(&id) {
            return Ok(entry.clone());
        }

        // Check file cache
        let cache_path = self.cache_dir.join(format!("meta_{}.json", id));
        if cache_path.exists() {
            let content = fs::read_to_string(&cache_path).await?;
            let entry: CatalogEntry = serde_json::from_str(&content)?;
            self.metadata_cache.insert(id, entry.clone());
            return Ok(entry);
        }

        // Fetch from API
        let entry = self.fetch_metadata(id).await?;

        // Cache to file
        let content = serde_json::to_string_pretty(&entry)?;
        fs::write(&cache_path, content).await?;

        // Cache in memory
        self.metadata_cache.insert(id, entry.clone());

        Ok(entry)
    }

    /// Fetch metadata from Gutenberg's JSON API
    async fn fetch_metadata(&self, id: u32) -> Result<CatalogEntry> {
        let url = format!("https://gutendex.com/books/{}", id);
        debug!("Fetching metadata for book {}", id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch book metadata")?;

        if !response.status().is_success() {
            // Try alternative API
            return self.fetch_metadata_alternative(id).await;
        }

        let json: GutendexBook = response.json().await?;

        Ok(CatalogEntry {
            id,
            title: json.title,
            authors: json.authors.into_iter().map(|a| a.name).collect(),
            language: json
                .languages
                .first()
                .cloned()
                .unwrap_or_else(|| "en".to_string()),
            subjects: json.subjects,
            formats: json.formats.keys().cloned().collect(),
            download_count: Some(json.download_count),
        })
    }

    /// Alternative metadata fetch using Gutenberg's direct HTML
    async fn fetch_metadata_alternative(&self, id: u32) -> Result<CatalogEntry> {
        let url = format!("https://www.gutenberg.org/ebooks/{}", id);
        debug!("Fetching metadata (alternative) for book {}", id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch book page")?;

        if !response.status().is_success() {
            anyhow::bail!("Book {} not found", id);
        }

        let html = response.text().await?;

        // Basic HTML parsing for title and author
        let title = extract_meta_content(&html, "og:title")
            .or_else(|| extract_between(&html, "<title>", "</title>"))
            .map(|s| s.replace(" - Project Gutenberg", "").trim().to_string())
            .unwrap_or_else(|| format!("Book {}", id));

        let author = extract_meta_content(&html, "og:author")
            .or_else(|| extract_between(&html, "by ", "</h1>"))
            .unwrap_or_else(|| "Unknown".to_string());

        Ok(CatalogEntry {
            id,
            title,
            authors: vec![author],
            language: "en".to_string(),
            subjects: vec![],
            formats: vec!["text/plain".to_string()],
            download_count: None,
        })
    }

    /// Get metadata for multiple books, returning successfully fetched entries
    pub async fn get_metadata_batch(&mut self, ids: &[u32]) -> Vec<CatalogEntry> {
        let mut entries = Vec::new();

        for &id in ids {
            match self.get_metadata(id).await {
                Ok(entry) => entries.push(entry),
                Err(e) => warn!("Failed to get metadata for book {}: {}", id, e),
            }
        }

        entries
    }

    /// List all English text books from the catalog
    pub async fn list_english_books(&mut self) -> Result<Vec<CatalogEntry>> {
        if self.book_ids.is_empty() {
            self.refresh_book_list().await?;
        }

        let ids = self.book_ids.clone();
        Ok(self.get_metadata_batch(&ids).await)
    }

    /// Get a sample of books for testing (first N books)
    pub fn sample_book_ids(&self, n: usize) -> Vec<u32> {
        self.book_ids.iter().take(n).copied().collect()
    }

    /// Get popular books (sorted by download count)
    pub async fn popular_books(&mut self, limit: usize) -> Result<Vec<CatalogEntry>> {
        let ids = self.sample_book_ids(limit.min(1000));
        let mut entries = self.get_metadata_batch(&ids).await;

        // Sort by download count (highest first)
        entries.sort_by(|a, b| {
            b.download_count
                .unwrap_or(0)
                .cmp(&a.download_count.unwrap_or(0))
        });

        Ok(entries.into_iter().take(limit).collect())
    }
}

/// Gutendex API response structure
#[derive(Debug, Deserialize)]
struct GutendexBook {
    title: String,
    authors: Vec<GutendexAuthor>,
    languages: Vec<String>,
    subjects: Vec<String>,
    formats: HashMap<String, String>,
    download_count: u32,
}

#[derive(Debug, Deserialize)]
struct GutendexAuthor {
    name: String,
}

/// Helper to extract meta content from HTML
fn extract_meta_content(html: &str, property: &str) -> Option<String> {
    let pattern = format!("property=\"{}\" content=\"", property);
    if let Some(start) = html.find(&pattern) {
        let rest = &html[start + pattern.len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Helper to extract text between markers
fn extract_between(html: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    if let Some(start) = html.find(start_marker) {
        let rest = &html[start + start_marker.len()..];
        if let Some(end) = rest.find(end_marker) {
            return Some(rest[..end].trim().to_string());
        }
    }
    None
}

/// Book deduplication utilities
pub mod dedup {
    use super::CatalogEntry;
    use std::collections::HashMap;

    /// Normalize a title for deduplication
    ///
    /// - Converts to lowercase
    /// - Removes articles (the, a, an)
    /// - Removes punctuation
    /// - Removes edition markers (vol., volume, part, etc.)
    pub fn normalize_title(title: &str) -> String {
        let title = title.to_lowercase();

        // Remove common articles and prefixes
        let title = title
            .trim_start_matches("the ")
            .trim_start_matches("a ")
            .trim_start_matches("an ");

        // Remove punctuation and normalize whitespace
        let normalized: String = title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' {
                    c
                } else {
                    ' '
                }
            })
            .collect();

        // Remove edition markers and trailing volume numbers
        let words: Vec<&str> = normalized.split_whitespace().collect();
        let filtered: Vec<&str> = words
            .into_iter()
            .filter(|w| {
                // Remove edition markers
                if matches!(*w, "vol" | "volume" | "part" | "book" | "edition" | "ed") {
                    return false;
                }
                // Remove standalone numbers (e.g., "1" in "Vol. 1")
                if w.chars().all(|c| c.is_ascii_digit()) {
                    return false;
                }
                true
            })
            .collect();

        filtered.join(" ")
    }

    /// Normalize an author name for deduplication
    ///
    /// - Converts to lowercase
    /// - Removes titles (sir, dr, etc.)
    /// - Handles "Last, First" format
    pub fn normalize_author(author: &str) -> String {
        let author = author.to_lowercase();

        // Remove common titles
        let author = author
            .replace("sir ", "")
            .replace("dr. ", "")
            .replace("dr ", "")
            .replace("lord ", "")
            .replace("lady ", "");

        // Handle "Last, First" format
        if let Some(comma_pos) = author.find(',') {
            let last = author[..comma_pos].trim();
            let first = author[comma_pos + 1..].trim();
            return format!("{} {}", first, last);
        }

        author.trim().to_string()
    }

    /// Deduplicate a list of catalog entries
    ///
    /// Keeps the entry with the highest download count for each unique work.
    pub fn deduplicate(entries: Vec<CatalogEntry>) -> Vec<CatalogEntry> {
        let mut unique: HashMap<String, CatalogEntry> = HashMap::new();

        for entry in entries {
            let key = format!(
                "{}:{}",
                normalize_title(&entry.title),
                normalize_author(entry.primary_author())
            );

            // Keep entry with higher download count
            if let Some(existing) = unique.get(&key) {
                if entry.download_count.unwrap_or(0) > existing.download_count.unwrap_or(0) {
                    unique.insert(key, entry);
                }
            } else {
                unique.insert(key, entry);
            }
        }

        let mut result: Vec<CatalogEntry> = unique.into_values().collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_title() {
        assert_eq!(dedup::normalize_title("The Great Gatsby"), "great gatsby");
        assert_eq!(
            dedup::normalize_title("A Tale of Two Cities"),
            "tale of two cities"
        );
        assert_eq!(
            dedup::normalize_title("Pride and Prejudice, Vol. 1"),
            "pride and prejudice"
        );
    }

    #[test]
    fn test_normalize_author() {
        assert_eq!(dedup::normalize_author("Austen, Jane"), "jane austen");
        assert_eq!(
            dedup::normalize_author("Sir Arthur Conan Doyle"),
            "arthur conan doyle"
        );
        assert_eq!(
            dedup::normalize_author("Dr. Samuel Johnson"),
            "samuel johnson"
        );
    }

    #[test]
    fn test_catalog_entry_is_text() {
        let entry = CatalogEntry {
            id: 1,
            title: "Test".to_string(),
            authors: vec!["Author".to_string()],
            language: "en".to_string(),
            subjects: vec![],
            formats: vec!["text/plain".to_string(), "application/epub".to_string()],
            download_count: None,
        };
        assert!(entry.is_text());

        let non_text = CatalogEntry {
            formats: vec!["audio/mp3".to_string()],
            ..entry
        };
        assert!(!non_text.is_text());
    }

    #[test]
    fn test_catalog_entry_is_english() {
        let entry = CatalogEntry {
            id: 1,
            title: "Test".to_string(),
            authors: vec!["Author".to_string()],
            language: "en".to_string(),
            subjects: vec![],
            formats: vec![],
            download_count: None,
        };
        assert!(entry.is_english());

        let french = CatalogEntry {
            language: "fr".to_string(),
            ..entry
        };
        assert!(!french.is_english());
    }

    #[test]
    fn test_dedup() {
        let entries = vec![
            CatalogEntry {
                id: 1,
                title: "Pride and Prejudice".to_string(),
                authors: vec!["Austen, Jane".to_string()],
                language: "en".to_string(),
                subjects: vec![],
                formats: vec![],
                download_count: Some(100),
            },
            CatalogEntry {
                id: 2,
                title: "Pride and Prejudice, Vol. 1".to_string(),
                authors: vec!["Jane Austen".to_string()],
                language: "en".to_string(),
                subjects: vec![],
                formats: vec![],
                download_count: Some(50),
            },
        ];

        let deduped = dedup::deduplicate(entries);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].id, 1); // Higher download count
    }
}
