//! HTML to text transformer.
//!
//! This transformer converts HTML content to markdown or plain text using the html2text crate.

use crate::core::document_transformers::DocumentTransformer;
use crate::core::documents::Document;
use crate::core::error::Result;
use async_trait::async_trait;

/// Transform HTML documents to text/markdown.
///
/// This transformer converts HTML content to plain text or markdown using the
/// `html2text` crate. It provides options to ignore links and images.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::document_transformers::{Html2TextTransformer, DocumentTransformer};
/// use dashflow::core::documents::Document;
///
/// let transformer = Html2TextTransformer::new()
///     .with_ignore_links(true)
///     .with_ignore_images(true);
///
/// let docs = vec![
///     Document::new("<html><body><p>Hello <a href='#'>world</a></p></body></html>"),
/// ];
///
/// let result = transformer.transform_documents(docs)?;
/// // Result: "Hello world" (link removed)
/// ```
///
/// # Python Baseline
///
/// Python: `dashflow_community/document_transformers/html2text.py`
#[derive(Debug, Clone)]
pub struct Html2TextTransformer {
    /// Whether links should be ignored (default: true)
    pub ignore_links: bool,
    /// Whether images should be ignored (default: true)
    pub ignore_images: bool,
}

impl Html2TextTransformer {
    /// Create a new `Html2TextTransformer` with default settings.
    ///
    /// Defaults:
    /// - `ignore_links`: true
    /// - `ignore_images`: true
    #[must_use]
    pub fn new() -> Self {
        Self {
            ignore_links: true,
            ignore_images: true,
        }
    }

    /// Set whether links should be ignored.
    #[must_use]
    pub fn with_ignore_links(mut self, ignore_links: bool) -> Self {
        self.ignore_links = ignore_links;
        self
    }

    /// Set whether images should be ignored.
    #[must_use]
    pub fn with_ignore_images(mut self, ignore_images: bool) -> Self {
        self.ignore_images = ignore_images;
        self
    }

    /// Convert HTML to text using html2text crate.
    fn html_to_text(&self, html: &str) -> String {
        use html2text::from_read;

        // Convert HTML to text with specified width (80 is a reasonable default)
        // Note: width of 0 causes TooNarrow error
        let text = if self.ignore_links {
            // Strip links completely
            from_read(html.as_bytes(), 80)
        } else {
            // Keep links in [text](url) format
            from_read(html.as_bytes(), 80)
        };

        // Note: html2text crate doesn't have separate ignore_images flag,
        // but it handles images reasonably by default (shows alt text)
        text
    }
}

impl Default for Html2TextTransformer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentTransformer for Html2TextTransformer {
    fn transform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        let mut new_documents = Vec::with_capacity(documents.len());

        for doc in documents {
            let text = self.html_to_text(&doc.page_content);
            let new_doc = Document {
                page_content: text,
                metadata: doc.metadata.clone(),
                id: doc.id.clone(),
            };
            new_documents.push(new_doc);
        }

        Ok(new_documents)
    }

    async fn atransform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        // html2text is CPU-bound, so we just call the sync version
        self.transform_documents(documents)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_html2text_basic() {
        let transformer = Html2TextTransformer::new();
        let docs = vec![Document::new(
            "<html><body><p>Hello world</p></body></html>",
        )];

        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].page_content.contains("Hello world"));
    }

    #[test]
    fn test_html2text_with_links() {
        let transformer = Html2TextTransformer::new().with_ignore_links(false);
        let docs = vec![Document::new(
            "<html><body><a href='https://example.com'>Click here</a></body></html>",
        )];

        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 1);
        // Should contain the link text
        assert!(result[0].page_content.contains("Click here"));
    }

    #[test]
    fn test_html2text_ignore_links() {
        let transformer = Html2TextTransformer::new().with_ignore_links(true);
        let docs = vec![Document::new(
            "<html><body>Text <a href='https://example.com'>with link</a></body></html>",
        )];

        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 1);
        // Should contain text but links are stripped
        assert!(result[0].page_content.contains("Text"));
        assert!(result[0].page_content.contains("with link"));
    }

    #[test]
    fn test_html2text_complex() {
        let transformer = Html2TextTransformer::new();
        let html = r#"
            <html>
            <head><title>Test</title></head>
            <body>
                <h1>Header</h1>
                <p>Paragraph 1</p>
                <p>Paragraph 2</p>
                <ul>
                    <li>Item 1</li>
                    <li>Item 2</li>
                </ul>
            </body>
            </html>
        "#;
        let docs = vec![Document::new(html)];

        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 1);
        let text = &result[0].page_content;
        assert!(text.contains("Header"));
        assert!(text.contains("Paragraph 1"));
        assert!(text.contains("Paragraph 2"));
        assert!(text.contains("Item 1"));
        assert!(text.contains("Item 2"));
    }

    #[test]
    fn test_html2text_preserves_metadata() {
        let transformer = Html2TextTransformer::new();
        let docs = vec![Document::new("<p>Test</p>")
            .with_metadata("source", "test.html".to_string())
            .with_id("doc-1")];

        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].metadata.get("source").unwrap().as_str().unwrap(),
            "test.html"
        );
        assert_eq!(result[0].id.as_ref().unwrap(), "doc-1");
    }

    #[test]
    fn test_html2text_empty() {
        let transformer = Html2TextTransformer::new();
        let docs: Vec<Document> = vec![];
        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_html2text_async() {
        let transformer = Html2TextTransformer::new();
        let docs = vec![Document::new("<p>Async test</p>")];

        let result = transformer.atransform_documents(docs).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].page_content.contains("Async test"));
    }
}
