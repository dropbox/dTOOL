//! Social media platform loaders.
//!
//! This module provides loaders for social media platforms including:
//! - Mastodon (`ActivityPub` export)
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;

use crate::core::documents::{Document, DocumentLoader};
use crate::core::error::Result;

/// Loader for Mastodon social media exports (`ActivityPub` format).
///
/// Parses Mastodon data export files that use `ActivityPub` JSON-LD format.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::MastodonLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = MastodonLoader::new("outbox.json");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct MastodonLoader {
    file_path: PathBuf,
}

impl MastodonLoader {
    /// Create a new Mastodon loader for the given export file.
    pub fn new(file_path: impl Into<PathBuf>) -> Self {
        Self {
            file_path: file_path.into(),
        }
    }
}

#[async_trait]
impl DocumentLoader for MastodonLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let content = fs::read_to_string(&self.file_path)?;

        // Mastodon exports use ActivityPub format
        let data: serde_json::Value = serde_json::from_str(&content)?;

        let mut documents = Vec::new();

        // Extract posts from orderedItems array
        if let Some(items) = data.get("orderedItems").and_then(|v| v.as_array()) {
            for item in items {
                // Only process Create activities with Note objects
                if item.get("type").and_then(|v| v.as_str()) == Some("Create") {
                    if let Some(object) = item.get("object") {
                        if object.get("type").and_then(|v| v.as_str()) == Some("Note") {
                            if let Some(content) = object.get("content").and_then(|v| v.as_str()) {
                                // Strip HTML tags from content
                                let text = html2text::from_read(content.as_bytes(), 80);

                                let published = object
                                    .get("published")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                let url = object.get("url").and_then(|v| v.as_str()).unwrap_or("");

                                let doc = Document::new(text)
                                    .with_metadata("source", url.to_string())
                                    .with_metadata("published", published.to_string())
                                    .with_metadata("format", "mastodon");

                                documents.push(doc);
                            }
                        }
                    }
                }
            }
        }

        Ok(documents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_mastodon_loader_new() {
        let loader = MastodonLoader::new("test.json");
        assert_eq!(loader.file_path, PathBuf::from("test.json"));
    }

    #[test]
    fn test_mastodon_loader_new_with_pathbuf() {
        let path = PathBuf::from("/some/path/outbox.json");
        let loader = MastodonLoader::new(path.clone());
        assert_eq!(loader.file_path, path);
    }

    #[tokio::test]
    async fn test_load_valid_activitypub() {
        let json = r#"{
            "orderedItems": [
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "content": "<p>Hello, Mastodon!</p>",
                        "published": "2024-01-15T10:30:00Z",
                        "url": "https://mastodon.social/@user/12345"
                    }
                }
            ]
        }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        assert_eq!(documents.len(), 1);
        assert!(documents[0].page_content.contains("Hello, Mastodon!"));
        assert_eq!(
            documents[0].metadata.get("source").unwrap(),
            "https://mastodon.social/@user/12345"
        );
        assert_eq!(
            documents[0].metadata.get("published").unwrap(),
            "2024-01-15T10:30:00Z"
        );
        assert_eq!(documents[0].metadata.get("format").unwrap(), "mastodon");
    }

    #[tokio::test]
    async fn test_load_multiple_notes() {
        let json = r#"{
            "orderedItems": [
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "content": "<p>First post</p>",
                        "published": "2024-01-01T00:00:00Z",
                        "url": "https://mastodon.social/@user/1"
                    }
                },
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "content": "<p>Second post</p>",
                        "published": "2024-01-02T00:00:00Z",
                        "url": "https://mastodon.social/@user/2"
                    }
                }
            ]
        }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        assert_eq!(documents.len(), 2);
        assert!(documents[0].page_content.contains("First post"));
        assert!(documents[1].page_content.contains("Second post"));
    }

    #[tokio::test]
    async fn test_load_skips_non_create_activities() {
        let json = r#"{
            "orderedItems": [
                {
                    "type": "Announce",
                    "object": {
                        "type": "Note",
                        "content": "<p>This is a boost, not a create</p>"
                    }
                },
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "content": "<p>This is an original post</p>",
                        "published": "2024-01-01T00:00:00Z",
                        "url": "https://mastodon.social/@user/1"
                    }
                }
            ]
        }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        assert_eq!(documents.len(), 1);
        assert!(documents[0].page_content.contains("original post"));
    }

    #[tokio::test]
    async fn test_load_skips_non_note_objects() {
        let json = r#"{
            "orderedItems": [
                {
                    "type": "Create",
                    "object": {
                        "type": "Article",
                        "content": "<p>This is an article, not a note</p>"
                    }
                },
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "content": "<p>This is a note</p>",
                        "published": "2024-01-01T00:00:00Z",
                        "url": "https://mastodon.social/@user/1"
                    }
                }
            ]
        }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        assert_eq!(documents.len(), 1);
        assert!(documents[0].page_content.contains("This is a note"));
    }

    #[tokio::test]
    async fn test_load_empty_ordered_items() {
        let json = r#"{ "orderedItems": [] }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        assert!(documents.is_empty());
    }

    #[tokio::test]
    async fn test_load_missing_ordered_items() {
        let json = r#"{ "type": "OrderedCollection" }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        assert!(documents.is_empty());
    }

    #[tokio::test]
    async fn test_load_html_stripping() {
        let json = r#"{
            "orderedItems": [
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "content": "<p><strong>Bold</strong> and <em>italic</em> text with <a href=\"https://example.com\">a link</a></p>",
                        "published": "2024-01-01T00:00:00Z",
                        "url": "https://mastodon.social/@user/1"
                    }
                }
            ]
        }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        assert_eq!(documents.len(), 1);
        // HTML should be stripped - verify no HTML tags remain
        assert!(!documents[0].page_content.contains("<p>"));
        assert!(!documents[0].page_content.contains("<strong>"));
        assert!(!documents[0].page_content.contains("<em>"));
        assert!(!documents[0].page_content.contains("<a"));
        // The text content should be present
        assert!(documents[0].page_content.contains("Bold"));
        assert!(documents[0].page_content.contains("italic"));
        assert!(documents[0].page_content.contains("link"));
    }

    #[tokio::test]
    async fn test_load_missing_optional_fields() {
        // Test with missing published and url fields
        let json = r#"{
            "orderedItems": [
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "content": "<p>Post without metadata</p>"
                    }
                }
            ]
        }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        assert_eq!(documents.len(), 1);
        assert!(documents[0].page_content.contains("Post without metadata"));
        // Missing fields should result in empty strings
        assert_eq!(documents[0].metadata.get("source").unwrap(), "");
        assert_eq!(documents[0].metadata.get("published").unwrap(), "");
    }

    #[tokio::test]
    async fn test_load_file_not_found() {
        let loader = MastodonLoader::new("/nonexistent/path/outbox.json");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_invalid_json() {
        let file = create_temp_file("not valid json {{{");
        let loader = MastodonLoader::new(file.path());
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_skips_notes_without_content() {
        let json = r#"{
            "orderedItems": [
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "published": "2024-01-01T00:00:00Z",
                        "url": "https://mastodon.social/@user/1"
                    }
                },
                {
                    "type": "Create",
                    "object": {
                        "type": "Note",
                        "content": "<p>Has content</p>",
                        "published": "2024-01-02T00:00:00Z",
                        "url": "https://mastodon.social/@user/2"
                    }
                }
            ]
        }"#;

        let file = create_temp_file(json);
        let loader = MastodonLoader::new(file.path());
        let documents = loader.load().await.unwrap();

        // Only the note with content should be loaded
        assert_eq!(documents.len(), 1);
        assert!(documents[0].page_content.contains("Has content"));
    }
}
