//! Gutenberg book downloader with caching

use anyhow::{Context, Result};
use reqwest::Client;
use std::path::PathBuf;
use tokio::fs;
use tracing::{info, warn};

/// Downloads books from Project Gutenberg with local caching
pub struct GutenbergDownloader {
    client: Client,
    cache_dir: PathBuf,
}

impl GutenbergDownloader {
    /// Create a new downloader with the specified cache directory
    #[allow(clippy::expect_used)] // Client::builder().build() only fails on TLS init; acceptable in example
    pub fn new(cache_dir: PathBuf) -> Self {
        let client = Client::builder()
            .user_agent("DashFlow-BookSearch/1.0 (https://github.com/dropbox/dTOOL/dashflow)")
            .build()
            .expect("Failed to create HTTP client");

        Self { client, cache_dir }
    }

    /// Download a book by its Gutenberg ID
    pub async fn download(&self, id: u32) -> Result<String> {
        let cache_path = self.cache_dir.join(format!("pg{}.txt", id));

        // Check cache first
        if cache_path.exists() {
            info!("Using cached: pg{}.txt", id);
            return fs::read_to_string(&cache_path)
                .await
                .context("Failed to read cached book");
        }

        // Try multiple URL formats (Gutenberg has inconsistent URLs)
        let urls = vec![
            format!("https://www.gutenberg.org/cache/epub/{}/pg{}.txt", id, id),
            format!("https://www.gutenberg.org/files/{}/{}-0.txt", id, id),
            format!("https://www.gutenberg.org/files/{}/{}.txt", id, id),
        ];

        for url in urls {
            info!("Downloading: {}", url);
            match self.client.get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    let text = response.text().await?;

                    // Cache the book
                    fs::create_dir_all(&self.cache_dir).await?;
                    fs::write(&cache_path, &text).await?;

                    info!("Downloaded and cached: pg{}.txt ({} bytes)", id, text.len());
                    return Ok(text);
                }
                _ => continue,
            }
        }

        anyhow::bail!("Failed to download book {} from any URL", id)
    }

    /// Download multiple books concurrently
    pub async fn download_many(
        &self,
        ids: &[(u32, &str, &str)],
    ) -> Vec<Result<(u32, String, String, String)>> {
        let mut results = Vec::new();

        for (id, title, author) in ids {
            match self.download(*id).await {
                Ok(text) => {
                    results.push(Ok((*id, title.to_string(), author.to_string(), text)));
                }
                Err(e) => {
                    warn!("Failed to download book {}: {}", id, e);
                    results.push(Err(e));
                }
            }
        }

        results
    }
}

/// Strip Gutenberg header and footer boilerplate
pub fn strip_gutenberg_boilerplate(text: &str) -> &str {
    // Find start marker
    let start_markers = [
        "*** START OF THE PROJECT GUTENBERG EBOOK",
        "*** START OF THIS PROJECT GUTENBERG EBOOK",
        "*END*THE SMALL PRINT",
    ];

    let mut start = 0;
    for marker in &start_markers {
        if let Some(pos) = text.find(marker) {
            // Skip to end of line
            if let Some(newline) = text[pos..].find('\n') {
                start = pos + newline + 1;
                break;
            }
        }
    }

    // Find end marker
    let end_markers = [
        "*** END OF THE PROJECT GUTENBERG EBOOK",
        "*** END OF THIS PROJECT GUTENBERG EBOOK",
        "End of the Project Gutenberg EBook",
    ];

    let mut end = text.len();
    for marker in &end_markers {
        if let Some(pos) = text.find(marker) {
            end = pos;
            break;
        }
    }

    &text[start..end]
}

/// Split text into chunks with overlap, breaking at paragraph boundaries
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();

        // Try to break at paragraph boundary
        let chunk = if let Some(para_pos) = chunk.rfind("\n\n") {
            if para_pos > chunk_size / 2 {
                chunk[..para_pos].to_string()
            } else {
                chunk
            }
        } else {
            chunk
        };

        if !chunk.trim().is_empty() {
            chunks.push(chunk.trim().to_string());
        }

        // Move forward, accounting for overlap
        start = if start + chunk_size >= chars.len() {
            chars.len()
        } else {
            start + chunk_size - overlap
        };
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text() {
        let text = "Hello world. This is a test.\n\nNew paragraph here.";
        let chunks = chunk_text(text, 20, 5);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_strip_boilerplate() {
        let text = "Header stuff\n*** START OF THE PROJECT GUTENBERG EBOOK ***\nActual content\n*** END OF THE PROJECT GUTENBERG EBOOK ***\nFooter";
        let clean = strip_gutenberg_boilerplate(text);
        assert!(clean.contains("Actual content"));
        assert!(!clean.contains("Header stuff"));
    }
}
