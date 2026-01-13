//! Personal knowledge management and information aggregation loaders.
//!
//! This module provides loaders for personal knowledge management systems and information sources:
//! - Obsidian (.md) - Obsidian markdown with wikilinks and tags
//! - Roam Research (.md) - Roam markdown with block references
//! - RSS/Atom feeds - Feed readers and aggregators
//! - Sitemap (.xml) - XML sitemap URL lists
//! - Org-mode (.org) - Emacs org-mode hierarchical notes

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Obsidian markdown loader
///
/// Loads Obsidian markdown files, preserving wikilinks, tags, and frontmatter.
/// Obsidian is a popular knowledge base that uses markdown files with special syntax.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::ObsidianLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ObsidianLoader::new("vault/notes/document.md");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct ObsidianLoader {
    file_path: PathBuf,
}

impl ObsidianLoader {
    /// Create a new Obsidian loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for ObsidianLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Obsidian files are markdown with special features:
        // - [[wikilinks]]
        // - #tags
        // - YAML frontmatter
        // We preserve all of this as it provides context

        Ok(vec![Document::new(&content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "obsidian")
            .with_metadata("type", "markdown")])
    }
}

/// Roam Research markdown loader
///
/// Loads Roam Research markdown files, preserving block references and page links.
/// Roam uses an outliner-based markdown format with bidirectional linking.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::RoamLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = RoamLoader::new("roam/notes/document.md");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct RoamLoader {
    file_path: PathBuf,
}

impl RoamLoader {
    /// Create a new Roam loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for RoamLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Roam files use markdown with special features:
        // - [[Page Links]]
        // - ((block references))
        // - #tags and #[[tagged pages]]
        // - Indented bullet points for hierarchy
        // We preserve all of this as it provides context

        Ok(vec![Document::new(&content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "roam")
            .with_metadata("type", "markdown")])
    }
}

/// Loads RSS/Atom feeds.
///
/// The `RSSLoader` fetches and parses RSS or Atom feeds, extracting feed items.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::RSSLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = RSSLoader::new("https://example.com/feed.xml");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct RSSLoader {
    /// URL of the RSS/Atom feed
    pub url: String,
    /// Create separate documents per item (default: true)
    pub separate_items: bool,
}

impl RSSLoader {
    /// Create a new `RSSLoader` for the given feed URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            separate_items: true,
        }
    }

    /// Concatenate all feed items into a single document.
    #[must_use]
    pub fn with_concatenated_items(mut self) -> Self {
        self.separate_items = false;
        self
    }

    fn extract_xml_elements(content: &str, tag: &str) -> Vec<String> {
        let mut elements = Vec::new();
        let start_tag = format!("<{tag}");
        let end_tag = format!("</{tag}>");

        let mut pos = 0;
        while let Some(start) = content[pos..].find(&start_tag) {
            let start = pos + start;
            if let Some(end) = content[start..].find(&end_tag) {
                let end = start + end + end_tag.len();
                elements.push(content[start..end].to_string());
                pos = end;
            } else {
                break;
            }
        }

        elements
    }

    fn extract_xml_text(content: &str, tag: &str) -> String {
        let start_tag = format!("<{tag}");
        let end_tag = format!("</{tag}>");

        if let Some(start) = content.find(&start_tag) {
            // Find the end of the opening tag
            if let Some(tag_end) = content[start..].find('>') {
                let content_start = start + tag_end + 1;
                if let Some(end) = content[content_start..].find(&end_tag) {
                    let text = &content[content_start..content_start + end];
                    return Self::strip_html_tags(text).trim().to_string();
                }
            }
        }

        String::new()
    }

    fn extract_xml_attribute_or_text(content: &str, tag: &str) -> String {
        let start_tag = format!("<{tag}");

        if let Some(start) = content.find(&start_tag) {
            if let Some(tag_end) = content[start..].find('>') {
                let tag_content = &content[start..start + tag_end];

                // Check for href attribute (Atom feeds)
                if let Some(href_pos) = tag_content.find("href=\"") {
                    let href_start = href_pos + 6;
                    if let Some(href_end) = tag_content[href_start..].find('"') {
                        return tag_content[href_start..href_start + href_end].to_string();
                    }
                }

                // Otherwise, extract text content
                return Self::extract_xml_text(content, tag);
            }
        }

        String::new()
    }

    fn strip_html_tags(content: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;

        for ch in content.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(ch),
                _ => {}
            }
        }

        result
    }
}

#[async_trait]
impl DocumentLoader for RSSLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // M-551: SSRF protection - validate URL before fetching
        crate::core::http_client::validate_url_for_ssrf(&self.url)?;

        // Fetch the feed content
        let response = reqwest::get(&self.url)
            .await
            .map_err(|e| crate::core::error::Error::Network(e.to_string()))?;
        let content = response
            .text()
            .await
            .map_err(|e| crate::core::error::Error::Network(e.to_string()))?;

        // Parse as XML and extract items
        let mut documents = Vec::new();
        let mut all_content = String::new();
        let mut item_count = 0;

        // Simple RSS/Atom parser using basic XML tag extraction
        // Look for <item> or <entry> tags
        let is_atom = content.contains("<feed") && content.contains("</feed>");
        let item_tag = if is_atom { "entry" } else { "item" };
        let title_tag = "title";
        let description_tag = if is_atom { "content" } else { "description" };
        let link_tag = "link";

        // Extract items/entries
        for item_content in Self::extract_xml_elements(&content, item_tag) {
            item_count += 1;

            let title = Self::extract_xml_text(&item_content, title_tag);
            let description = Self::extract_xml_text(&item_content, description_tag);
            let link = Self::extract_xml_attribute_or_text(&item_content, link_tag);

            let mut item_text = String::new();
            if !title.is_empty() {
                item_text.push_str("Title: ");
                item_text.push_str(&title);
                item_text.push('\n');
            }
            if !link.is_empty() {
                item_text.push_str("Link: ");
                item_text.push_str(&link);
                item_text.push('\n');
            }
            if !description.is_empty() {
                item_text.push('\n');
                item_text.push_str(&description);
            }

            if self.separate_items {
                let mut doc = Document::new(item_text)
                    .with_metadata("source", self.url.clone())
                    .with_metadata("item_index", item_count - 1);

                if !title.is_empty() {
                    doc = doc.with_metadata("title", title);
                }
                if !link.is_empty() {
                    doc = doc.with_metadata("link", link);
                }

                documents.push(doc);
            } else {
                all_content.push_str(&item_text);
                all_content.push_str("\n\n");
            }
        }

        if !self.separate_items {
            let doc = Document::new(all_content)
                .with_metadata("source", self.url.clone())
                .with_metadata("format", if is_atom { "atom" } else { "rss" })
                .with_metadata("item_count", item_count);

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loads XML sitemap files.
///
/// The `SitemapLoader` reads sitemap.xml files and extracts URLs.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::SitemapLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SitemapLoader::new("https://example.com/sitemap.xml");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SitemapLoader {
    /// URL or path to the sitemap file
    pub source: String,
    /// Create separate documents per URL (default: false, list all URLs)
    pub separate_urls: bool,
}

impl SitemapLoader {
    /// Create a new `SitemapLoader` for the given sitemap URL or file path.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            separate_urls: false,
        }
    }

    /// Create separate documents per URL in the sitemap.
    #[must_use]
    pub fn with_separate_urls(mut self, separate: bool) -> Self {
        self.separate_urls = separate;
        self
    }

    fn extract_sitemap_urls(content: &str) -> Vec<String> {
        let mut urls = Vec::new();

        // Extract <loc> tags from sitemap XML
        let mut pos = 0;
        while let Some(start) = content[pos..].find("<loc>") {
            let start = pos + start + 5; // Skip "<loc>"
            if let Some(end) = content[start..].find("</loc>") {
                let url = content[start..start + end].trim().to_string();
                if !url.is_empty() {
                    urls.push(url);
                }
                pos = start + end + 6; // Skip "</loc>"
            } else {
                break;
            }
        }

        urls
    }
}

#[async_trait]
impl DocumentLoader for SitemapLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Determine if source is URL or file path
        let content = if self.source.starts_with("http://") || self.source.starts_with("https://") {
            // M-551: SSRF protection - validate URL before fetching
            crate::core::http_client::validate_url_for_ssrf(&self.source)?;

            // Fetch from URL
            let response = reqwest::get(&self.source)
                .await
                .map_err(|e| crate::core::error::Error::Network(e.to_string()))?;
            response
                .text()
                .await
                .map_err(|e| crate::core::error::Error::Network(e.to_string()))?
        } else {
            // Read from file
            let blob = Blob::from_path(&self.source);
            blob.as_string()?
        };

        // Extract URLs from sitemap
        let urls = Self::extract_sitemap_urls(&content);

        let mut documents = Vec::new();

        if self.separate_urls {
            // Create separate document per URL
            for (idx, url) in urls.iter().enumerate() {
                let doc = Document::new(url.clone())
                    .with_metadata("source", self.source.clone())
                    .with_metadata("url_index", idx as i64)
                    .with_metadata("url", url.clone());

                documents.push(doc);
            }
        } else {
            // Create single document listing all URLs
            let all_urls = urls.join("\n");
            let doc = Document::new(all_urls)
                .with_metadata("source", self.source.clone())
                .with_metadata("format", "sitemap")
                .with_metadata("url_count", urls.len() as i64);

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loads Emacs org-mode files (.org).
///
/// The `OrgModeLoader` reads org-mode files, which are hierarchical plain-text notes
/// and TODO lists. Can optionally separate by top-level headings.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::OrgModeLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = OrgModeLoader::new("notes.org");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OrgModeLoader {
    /// Path to the org-mode file
    pub file_path: PathBuf,
    /// Separate documents per top-level heading (default: false)
    pub separate_headings: bool,
}

impl OrgModeLoader {
    /// Create a new `OrgModeLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_headings: false,
        }
    }

    /// Create separate documents per top-level heading (lines starting with `* `).
    #[must_use]
    pub fn with_separate_headings(mut self, separate: bool) -> Self {
        self.separate_headings = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for OrgModeLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_headings {
            // Split by top-level headings (lines starting with "* ")
            let mut documents = Vec::new();
            let mut current_section = String::new();
            let mut heading_title = String::new();
            let mut heading_index = 0;

            for line in content.lines() {
                if line.starts_with("* ") {
                    // Save previous section
                    if !current_section.is_empty() {
                        let doc = Document::new(current_section.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("heading_index", heading_index)
                            .with_metadata("heading_title", heading_title.clone())
                            .with_metadata("format", "org");

                        documents.push(doc);
                        current_section.clear();
                        heading_index += 1;
                    }

                    // Start new section
                    heading_title = line.trim_start_matches("* ").to_string();
                }

                current_section.push_str(line);
                current_section.push('\n');
            }

            // Add last section
            if !current_section.is_empty() {
                let doc = Document::new(current_section)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("heading_index", heading_index)
                    .with_metadata("heading_title", heading_title)
                    .with_metadata("format", "org");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "org");

            Ok(vec![doc])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ==========================================================================
    // ObsidianLoader Tests
    // ==========================================================================

    #[test]
    fn test_obsidian_loader_new() {
        let loader = ObsidianLoader::new("vault/notes/test.md");
        assert_eq!(loader.file_path, PathBuf::from("vault/notes/test.md"));
    }

    #[test]
    fn test_obsidian_loader_new_from_pathbuf() {
        let path = PathBuf::from("/home/user/vault/note.md");
        let loader = ObsidianLoader::new(&path);
        assert_eq!(loader.file_path, path);
    }

    #[tokio::test]
    async fn test_obsidian_loader_load_basic() {
        let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
        let content = "# My Note\n\nSome content with [[wikilink]] and #tag";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = ObsidianLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("[[wikilink]]"));
        assert!(docs[0].page_content.contains("#tag"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("obsidian")
        );
        assert_eq!(
            docs[0].metadata.get("type").and_then(|v| v.as_str()),
            Some("markdown")
        );
    }

    #[tokio::test]
    async fn test_obsidian_loader_preserves_frontmatter() {
        let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
        let content = "---\ntitle: Test Note\ntags: [tag1, tag2]\n---\n\n# Content";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = ObsidianLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("---"));
        assert!(docs[0].page_content.contains("title: Test Note"));
    }

    #[tokio::test]
    async fn test_obsidian_loader_source_metadata() {
        let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
        temp_file.write_all(b"test content").unwrap();

        let loader = ObsidianLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.get("source").is_some());
    }

    // ==========================================================================
    // RoamLoader Tests
    // ==========================================================================

    #[test]
    fn test_roam_loader_new() {
        let loader = RoamLoader::new("roam/notes/page.md");
        assert_eq!(loader.file_path, PathBuf::from("roam/notes/page.md"));
    }

    #[test]
    fn test_roam_loader_new_from_pathbuf() {
        let path = PathBuf::from("/home/user/roam/notes.md");
        let loader = RoamLoader::new(&path);
        assert_eq!(loader.file_path, path);
    }

    #[tokio::test]
    async fn test_roam_loader_load_basic() {
        let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
        let content = "- [[Page Link]]\n  - ((block-ref))\n  - #tag and #[[tagged page]]";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = RoamLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("[[Page Link]]"));
        assert!(docs[0].page_content.contains("((block-ref))"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("roam")
        );
        assert_eq!(
            docs[0].metadata.get("type").and_then(|v| v.as_str()),
            Some("markdown")
        );
    }

    #[tokio::test]
    async fn test_roam_loader_preserves_hierarchy() {
        let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
        let content = "- Level 1\n  - Level 2\n    - Level 3";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = RoamLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].page_content.contains("  - Level 2"));
        assert!(docs[0].page_content.contains("    - Level 3"));
    }

    // ==========================================================================
    // RSSLoader Tests
    // ==========================================================================

    #[test]
    fn test_rss_loader_new() {
        let loader = RSSLoader::new("https://example.com/feed.xml");
        assert_eq!(loader.url, "https://example.com/feed.xml");
        assert!(loader.separate_items);
    }

    #[test]
    fn test_rss_loader_new_from_string() {
        let url = String::from("https://example.com/rss");
        let loader = RSSLoader::new(url);
        assert_eq!(loader.url, "https://example.com/rss");
    }

    #[test]
    fn test_rss_loader_with_concatenated_items() {
        let loader = RSSLoader::new("https://example.com/feed.xml").with_concatenated_items();
        assert!(!loader.separate_items);
    }

    #[test]
    fn test_rss_loader_debug_clone() {
        let loader = RSSLoader::new("https://test.com/feed.xml");
        let cloned = loader.clone();
        assert_eq!(loader.url, cloned.url);
        assert_eq!(loader.separate_items, cloned.separate_items);

        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("RSSLoader"));
        assert!(debug_str.contains("test.com"));
    }

    #[test]
    fn test_extract_xml_elements_single() {
        let xml = "<feed><item><title>Test</title></item></feed>";
        let items = RSSLoader::extract_xml_elements(xml, "item");
        assert_eq!(items.len(), 1);
        assert!(items[0].contains("<title>Test</title>"));
    }

    #[test]
    fn test_extract_xml_elements_multiple() {
        let xml = "<feed><item><title>One</title></item><item><title>Two</title></item></feed>";
        let items = RSSLoader::extract_xml_elements(xml, "item");
        assert_eq!(items.len(), 2);
        assert!(items[0].contains("One"));
        assert!(items[1].contains("Two"));
    }

    #[test]
    fn test_extract_xml_elements_empty() {
        let xml = "<feed></feed>";
        let items = RSSLoader::extract_xml_elements(xml, "item");
        assert!(items.is_empty());
    }

    #[test]
    fn test_extract_xml_elements_nested() {
        let xml = "<feed><entry><title>Entry</title><link>http://example.com</link></entry></feed>";
        let entries = RSSLoader::extract_xml_elements(xml, "entry");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].contains("<title>Entry</title>"));
        assert!(entries[0].contains("<link>"));
    }

    #[test]
    fn test_extract_xml_text_simple() {
        let xml = "<item><title>Hello World</title></item>";
        let text = RSSLoader::extract_xml_text(xml, "title");
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_extract_xml_text_with_html() {
        let xml = "<item><description><p>Some <b>HTML</b> content</p></description></item>";
        let text = RSSLoader::extract_xml_text(xml, "description");
        // HTML tags should be stripped
        assert!(!text.contains("<p>"));
        assert!(!text.contains("<b>"));
        assert!(text.contains("Some"));
        assert!(text.contains("HTML"));
        assert!(text.contains("content"));
    }

    #[test]
    fn test_extract_xml_text_missing() {
        let xml = "<item><title>Test</title></item>";
        let text = RSSLoader::extract_xml_text(xml, "description");
        assert!(text.is_empty());
    }

    #[test]
    fn test_extract_xml_text_empty() {
        let xml = "<item><title></title></item>";
        let text = RSSLoader::extract_xml_text(xml, "title");
        assert!(text.is_empty());
    }

    #[test]
    fn test_extract_xml_text_with_whitespace() {
        let xml = "<item><title>  Trimmed Title  </title></item>";
        let text = RSSLoader::extract_xml_text(xml, "title");
        assert_eq!(text, "Trimmed Title");
    }

    #[test]
    fn test_extract_xml_attribute_or_text_href() {
        let xml = r#"<entry><link href="https://example.com/page" rel="alternate" /></entry>"#;
        let link = RSSLoader::extract_xml_attribute_or_text(xml, "link");
        assert_eq!(link, "https://example.com/page");
    }

    #[test]
    fn test_extract_xml_attribute_or_text_content() {
        let xml = "<item><link>https://example.com/rss</link></item>";
        let link = RSSLoader::extract_xml_attribute_or_text(xml, "link");
        assert_eq!(link, "https://example.com/rss");
    }

    #[test]
    fn test_extract_xml_attribute_or_text_missing() {
        let xml = "<item><title>Test</title></item>";
        let link = RSSLoader::extract_xml_attribute_or_text(xml, "link");
        assert!(link.is_empty());
    }

    #[test]
    fn test_strip_html_tags_basic() {
        let html = "<p>Hello <b>World</b></p>";
        let text = RSSLoader::strip_html_tags(html);
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_strip_html_tags_nested() {
        let html = "<div><p>Some <em>emphasized</em> text</p></div>";
        let text = RSSLoader::strip_html_tags(html);
        assert_eq!(text, "Some emphasized text");
    }

    #[test]
    fn test_strip_html_tags_no_tags() {
        let html = "Plain text content";
        let text = RSSLoader::strip_html_tags(html);
        assert_eq!(text, "Plain text content");
    }

    #[test]
    fn test_strip_html_tags_empty() {
        let html = "";
        let text = RSSLoader::strip_html_tags(html);
        assert!(text.is_empty());
    }

    #[test]
    fn test_strip_html_tags_only_tags() {
        let html = "<br/><hr>";
        let text = RSSLoader::strip_html_tags(html);
        assert!(text.is_empty());
    }

    #[test]
    fn test_strip_html_tags_preserves_special_chars() {
        let html = "<p>Hello &amp; Goodbye</p>";
        let text = RSSLoader::strip_html_tags(html);
        assert_eq!(text, "Hello &amp; Goodbye");
    }

    // ==========================================================================
    // SitemapLoader Tests
    // ==========================================================================

    #[test]
    fn test_sitemap_loader_new() {
        let loader = SitemapLoader::new("https://example.com/sitemap.xml");
        assert_eq!(loader.source, "https://example.com/sitemap.xml");
        assert!(!loader.separate_urls);
    }

    #[test]
    fn test_sitemap_loader_new_file_path() {
        let loader = SitemapLoader::new("/path/to/sitemap.xml");
        assert_eq!(loader.source, "/path/to/sitemap.xml");
    }

    #[test]
    fn test_sitemap_loader_with_separate_urls_true() {
        let loader = SitemapLoader::new("https://example.com/sitemap.xml").with_separate_urls(true);
        assert!(loader.separate_urls);
    }

    #[test]
    fn test_sitemap_loader_with_separate_urls_false() {
        let loader =
            SitemapLoader::new("https://example.com/sitemap.xml").with_separate_urls(false);
        assert!(!loader.separate_urls);
    }

    #[test]
    fn test_sitemap_loader_debug_clone() {
        let loader = SitemapLoader::new("https://test.com/sitemap.xml");
        let cloned = loader.clone();
        assert_eq!(loader.source, cloned.source);
        assert_eq!(loader.separate_urls, cloned.separate_urls);

        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("SitemapLoader"));
        assert!(debug_str.contains("test.com"));
    }

    #[test]
    fn test_extract_sitemap_urls_single() {
        let xml = r#"<?xml version="1.0"?><urlset><url><loc>https://example.com/page1</loc></url></urlset>"#;
        let urls = SitemapLoader::extract_sitemap_urls(xml);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com/page1");
    }

    #[test]
    fn test_extract_sitemap_urls_multiple() {
        let xml = r#"<urlset>
            <url><loc>https://example.com/page1</loc></url>
            <url><loc>https://example.com/page2</loc></url>
            <url><loc>https://example.com/page3</loc></url>
        </urlset>"#;
        let urls = SitemapLoader::extract_sitemap_urls(xml);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://example.com/page1");
        assert_eq!(urls[1], "https://example.com/page2");
        assert_eq!(urls[2], "https://example.com/page3");
    }

    #[test]
    fn test_extract_sitemap_urls_empty() {
        let xml = r#"<urlset></urlset>"#;
        let urls = SitemapLoader::extract_sitemap_urls(xml);
        assert!(urls.is_empty());
    }

    #[test]
    fn test_extract_sitemap_urls_with_metadata() {
        let xml = r#"<urlset>
            <url>
                <loc>https://example.com/page</loc>
                <lastmod>2024-01-01</lastmod>
                <changefreq>daily</changefreq>
                <priority>0.8</priority>
            </url>
        </urlset>"#;
        let urls = SitemapLoader::extract_sitemap_urls(xml);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com/page");
    }

    #[test]
    fn test_extract_sitemap_urls_trims_whitespace() {
        let xml = r#"<urlset><url><loc>  https://example.com/page  </loc></url></urlset>"#;
        let urls = SitemapLoader::extract_sitemap_urls(xml);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com/page");
    }

    #[test]
    fn test_extract_sitemap_urls_skips_empty() {
        let xml = r#"<urlset><url><loc></loc></url><url><loc>https://example.com</loc></url></urlset>"#;
        let urls = SitemapLoader::extract_sitemap_urls(xml);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com");
    }

    #[tokio::test]
    async fn test_sitemap_loader_load_from_file() {
        let mut temp_file = NamedTempFile::with_suffix(".xml").unwrap();
        let xml = r#"<?xml version="1.0"?><urlset>
            <url><loc>https://example.com/page1</loc></url>
            <url><loc>https://example.com/page2</loc></url>
        </urlset>"#;
        temp_file.write_all(xml.as_bytes()).unwrap();

        let loader = SitemapLoader::new(temp_file.path().to_str().unwrap());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("https://example.com/page1"));
        assert!(docs[0].page_content.contains("https://example.com/page2"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("sitemap")
        );
        assert_eq!(
            docs[0].metadata.get("url_count").and_then(|v| v.as_i64()),
            Some(2)
        );
    }

    #[tokio::test]
    async fn test_sitemap_loader_load_separate_urls() {
        let mut temp_file = NamedTempFile::with_suffix(".xml").unwrap();
        let xml = r#"<?xml version="1.0"?><urlset>
            <url><loc>https://example.com/page1</loc></url>
            <url><loc>https://example.com/page2</loc></url>
        </urlset>"#;
        temp_file.write_all(xml.as_bytes()).unwrap();

        let loader =
            SitemapLoader::new(temp_file.path().to_str().unwrap()).with_separate_urls(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "https://example.com/page1");
        assert_eq!(docs[1].page_content, "https://example.com/page2");
        assert_eq!(
            docs[0].metadata.get("url_index").and_then(|v| v.as_i64()),
            Some(0)
        );
        assert_eq!(
            docs[1].metadata.get("url_index").and_then(|v| v.as_i64()),
            Some(1)
        );
    }

    // ==========================================================================
    // OrgModeLoader Tests
    // ==========================================================================

    #[test]
    fn test_org_mode_loader_new() {
        let loader = OrgModeLoader::new("notes.org");
        assert_eq!(loader.file_path, PathBuf::from("notes.org"));
        assert!(!loader.separate_headings);
    }

    #[test]
    fn test_org_mode_loader_new_from_pathbuf() {
        let path = PathBuf::from("/home/user/org/notes.org");
        let loader = OrgModeLoader::new(&path);
        assert_eq!(loader.file_path, path);
    }

    #[test]
    fn test_org_mode_loader_with_separate_headings_true() {
        let loader = OrgModeLoader::new("notes.org").with_separate_headings(true);
        assert!(loader.separate_headings);
    }

    #[test]
    fn test_org_mode_loader_with_separate_headings_false() {
        let loader = OrgModeLoader::new("notes.org").with_separate_headings(false);
        assert!(!loader.separate_headings);
    }

    #[test]
    fn test_org_mode_loader_debug_clone() {
        let loader = OrgModeLoader::new("test.org");
        let cloned = loader.clone();
        assert_eq!(loader.file_path, cloned.file_path);
        assert_eq!(loader.separate_headings, cloned.separate_headings);

        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("OrgModeLoader"));
        assert!(debug_str.contains("test.org"));
    }

    #[tokio::test]
    async fn test_org_mode_loader_load_basic() {
        let mut temp_file = NamedTempFile::with_suffix(".org").unwrap();
        let content = "#+TITLE: My Notes\n\n* Heading 1\nContent 1\n\n* Heading 2\nContent 2";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = OrgModeLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("#+TITLE:"));
        assert!(docs[0].page_content.contains("Heading 1"));
        assert!(docs[0].page_content.contains("Heading 2"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("org")
        );
    }

    #[tokio::test]
    async fn test_org_mode_loader_load_separate_headings() {
        let mut temp_file = NamedTempFile::with_suffix(".org").unwrap();
        let content = "* First Heading\nFirst content\n\n* Second Heading\nSecond content";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = OrgModeLoader::new(temp_file.path()).with_separate_headings(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("First Heading"));
        assert!(docs[0].page_content.contains("First content"));
        assert!(docs[1].page_content.contains("Second Heading"));
        assert!(docs[1].page_content.contains("Second content"));
    }

    #[tokio::test]
    async fn test_org_mode_loader_heading_metadata() {
        let mut temp_file = NamedTempFile::with_suffix(".org").unwrap();
        let content = "* My Heading\nSome content";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = OrgModeLoader::new(temp_file.path()).with_separate_headings(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0]
                .metadata
                .get("heading_title")
                .and_then(|v| v.as_str()),
            Some("My Heading")
        );
        assert_eq!(
            docs[0]
                .metadata
                .get("heading_index")
                .and_then(|v| v.as_i64()),
            Some(0)
        );
    }

    #[tokio::test]
    async fn test_org_mode_loader_nested_headings() {
        let mut temp_file = NamedTempFile::with_suffix(".org").unwrap();
        let content = "* Top Level\n** Sub Level\nContent\n*** Sub-sub Level";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = OrgModeLoader::new(temp_file.path()).with_separate_headings(true);
        let docs = loader.load().await.unwrap();

        // Only splits on top-level headings (single *)
        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("** Sub Level"));
        assert!(docs[0].page_content.contains("*** Sub-sub Level"));
    }

    #[tokio::test]
    async fn test_org_mode_loader_no_headings() {
        let mut temp_file = NamedTempFile::with_suffix(".org").unwrap();
        let content = "Just some content without headings";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = OrgModeLoader::new(temp_file.path()).with_separate_headings(true);
        let docs = loader.load().await.unwrap();

        // With no headings, should still produce one document
        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Just some content"));
    }

    #[tokio::test]
    async fn test_org_mode_loader_preserves_todo_items() {
        let mut temp_file = NamedTempFile::with_suffix(".org").unwrap();
        let content = "* TODO Task 1\n* DONE Task 2\n* IN-PROGRESS Task 3";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = OrgModeLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].page_content.contains("TODO"));
        assert!(docs[0].page_content.contains("DONE"));
        assert!(docs[0].page_content.contains("IN-PROGRESS"));
    }

    #[tokio::test]
    async fn test_org_mode_loader_source_metadata() {
        let mut temp_file = NamedTempFile::with_suffix(".org").unwrap();
        temp_file.write_all(b"test content").unwrap();

        let loader = OrgModeLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.get("source").is_some());
    }

    // ==========================================================================
    // Integration Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_obsidian_loader_empty_file() {
        let temp_file = NamedTempFile::with_suffix(".md").unwrap();
        // File exists but is empty

        let loader = ObsidianLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_roam_loader_empty_file() {
        let temp_file = NamedTempFile::with_suffix(".md").unwrap();

        let loader = RoamLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_org_mode_loader_empty_file() {
        let temp_file = NamedTempFile::with_suffix(".org").unwrap();

        let loader = OrgModeLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_sitemap_loader_empty_file() {
        let mut temp_file = NamedTempFile::with_suffix(".xml").unwrap();
        temp_file.write_all(b"<urlset></urlset>").unwrap();

        let loader = SitemapLoader::new(temp_file.path().to_str().unwrap());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
        assert_eq!(
            docs[0].metadata.get("url_count").and_then(|v| v.as_i64()),
            Some(0)
        );
    }

    #[test]
    fn test_rss_extract_real_rss_format() {
        let rss_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <rss version="2.0">
            <channel>
                <title>Example Feed</title>
                <link>https://example.com</link>
                <description>An example RSS feed</description>
                <item>
                    <title>First Article</title>
                    <link>https://example.com/article1</link>
                    <description>This is the first article.</description>
                </item>
                <item>
                    <title>Second Article</title>
                    <link>https://example.com/article2</link>
                    <description>This is the second article.</description>
                </item>
            </channel>
        </rss>"#;

        let items = RSSLoader::extract_xml_elements(rss_xml, "item");
        assert_eq!(items.len(), 2);

        let title = RSSLoader::extract_xml_text(&items[0], "title");
        assert_eq!(title, "First Article");

        let link = RSSLoader::extract_xml_attribute_or_text(&items[0], "link");
        assert_eq!(link, "https://example.com/article1");

        let description = RSSLoader::extract_xml_text(&items[0], "description");
        assert_eq!(description, "This is the first article.");
    }

    #[test]
    fn test_rss_extract_atom_format() {
        let atom_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <title>Example Feed</title>
            <entry>
                <title>First Entry</title>
                <link href="https://example.com/entry1" rel="alternate"/>
                <content>This is the first entry content.</content>
            </entry>
            <entry>
                <title>Second Entry</title>
                <link href="https://example.com/entry2" rel="alternate"/>
                <content>This is the second entry content.</content>
            </entry>
        </feed>"#;

        let entries = RSSLoader::extract_xml_elements(atom_xml, "entry");
        assert_eq!(entries.len(), 2);

        let title = RSSLoader::extract_xml_text(&entries[0], "title");
        assert_eq!(title, "First Entry");

        // Atom uses href attribute for links
        let link = RSSLoader::extract_xml_attribute_or_text(&entries[0], "link");
        assert_eq!(link, "https://example.com/entry1");

        let content = RSSLoader::extract_xml_text(&entries[0], "content");
        assert_eq!(content, "This is the first entry content.");
    }
}
