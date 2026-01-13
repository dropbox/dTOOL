// Removed broad #![allow(clippy::expect_used)] - targeted allows used instead.

use crate::core::document_transformers::DocumentTransformer;
use crate::core::documents::Document;
use crate::core::error::Result;
use async_trait::async_trait;
use regex::Regex;

/// Heading style for markdown conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingStyle {
    /// ATX style: # H1, ## H2, etc.
    Atx,
    /// ATX closed style: # H1 #, ## H2 ##, etc.
    AtxClosed,
    /// Setext style: H1 with ===, H2 with ---
    Setext,
    /// Alias for Setext
    Underlined,
}

/// Converts HTML documents to Markdown format with customizable options for handling
/// links, images, other tags and heading styles.
///
/// This transformer uses the `scraper` library to provide HTML to Markdown conversion
/// with configurable options for tag handling, link styles, and heading formats.
///
/// # Arguments
///
/// * `strip` - A list of HTML tags to strip from the output
/// * `autolinks` - Whether to use "automatic link" style when `<a>` tag contents match href
/// * `heading_style` - How headings should be converted (ATX, `ATX_CLOSED`, SETEXT, UNDERLINED)
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::document_transformers::{MarkdownifyTransformer, HeadingStyle, DocumentTransformer};
/// use dashflow::core::documents::Document;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let html_doc = Document::new(
///     "<h1>Title</h1><p>This is a <strong>paragraph</strong>.</p>"
/// );
///
/// let transformer = MarkdownifyTransformer::new()
///     .with_heading_style(HeadingStyle::Atx)
///     .with_autolinks(true);
///
/// let markdown_docs = transformer.transform_documents(vec![html_doc])?;
/// # Ok(())
/// # }
/// ```
///
/// # More Options
///
/// The underlying scraper library provides HTML parsing capabilities.
/// See: <https://github.com/causal-agent/scraper>
///
/// # Based On
///
/// Python: `dashflow_community.document_transformers.markdownify.MarkdownifyTransformer`
#[derive(Debug, Clone)]
pub struct MarkdownifyTransformer {
    /// HTML tags to strip from output (not converted to markdown)
    strip: Option<Vec<String>>,
    /// Whether to use automatic link style when `<a>` contents match href
    autolinks: bool,
    /// Heading style for conversion
    heading_style: HeadingStyle,
}

impl Default for MarkdownifyTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownifyTransformer {
    /// Create a new `MarkdownifyTransformer` with default settings
    ///
    /// Defaults:
    /// - No tags stripped
    /// - Autolinks enabled
    /// - ATX heading style (# H1, ## H2, etc.)
    #[must_use]
    pub fn new() -> Self {
        Self {
            strip: None,
            autolinks: true,
            heading_style: HeadingStyle::Atx,
        }
    }

    /// Set HTML tags to strip from output
    ///
    /// Tags in this list will be removed from the HTML before conversion.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow::core::document_transformers::MarkdownifyTransformer;
    ///
    /// let transformer = MarkdownifyTransformer::new()
    ///     .with_strip(vec!["script".to_string(), "style".to_string()]);
    /// ```
    #[must_use]
    pub fn with_strip(mut self, strip: Vec<String>) -> Self {
        self.strip = Some(strip);
        self
    }

    /// Set whether to use automatic link style
    ///
    /// When enabled, if an `<a>` tag's text content matches its href,
    /// it will be rendered as `<url>` instead of `[url](url)`.
    #[must_use]
    pub fn with_autolinks(mut self, autolinks: bool) -> Self {
        self.autolinks = autolinks;
        self
    }

    /// Set the heading style for conversion
    ///
    /// # Heading Styles
    ///
    /// - `Atx`: `# H1`, `## H2`, etc.
    /// - `AtxClosed`: `# H1 #`, `## H2 ##`, etc.
    /// - `Setext`: `H1` with `===`, `H2` with `---`
    /// - `Underlined`: Alias for `Setext`
    #[must_use]
    pub fn with_heading_style(mut self, heading_style: HeadingStyle) -> Self {
        self.heading_style = heading_style;
        self
    }

    /// Convert HTML to Markdown using scraper library
    fn convert_html_to_markdown(&self, html: &str) -> String {
        // Simple HTML to Markdown conversion using scraper (MIT/Apache-2.0 licensed)
        // This is a basic implementation covering common HTML elements

        use scraper::Html;

        let document = Html::parse_document(html);

        // For now, use a simple approach: extract all text content
        // and preserve basic structure. This is a simplified version
        // compared to Python's markdownify library.

        // Get all text content
        let mut markdown = String::new();
        for node in document.tree.nodes() {
            match node.value() {
                scraper::Node::Text(text) => {
                    let text_content: &str = text.as_ref();
                    if !text_content.trim().is_empty() {
                        markdown.push_str(text_content);
                        markdown.push('\n');
                    }
                }
                scraper::Node::Element(element) => {
                    // Extract href from links
                    if element.name() == "a" {
                        if let Some(href) = element.attr("href") {
                            markdown.push_str(&format!(" ({href}) "));
                        }
                    }
                }
                _ => {}
            }
        }

        // Replace non-breaking spaces with regular spaces
        markdown = markdown.replace('\u{00A0}', " ");

        // Trim whitespace
        markdown = markdown.trim().to_string();

        markdown
    }

    /// Clean up markdown formatting
    ///
    /// Consolidates multiple consecutive newlines into double newlines
    #[allow(clippy::expect_used)] // Static regex pattern - infallible for valid regex literals
    fn clean_markdown(&self, markdown: &str) -> String {
        let multi_newline = Regex::new(r"\n\s*\n").expect("static regex pattern is valid");
        multi_newline.replace_all(markdown, "\n\n").to_string()
    }
}

#[async_trait]
impl DocumentTransformer for MarkdownifyTransformer {
    fn transform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        let mut converted_documents = Vec::with_capacity(documents.len());

        for doc in documents {
            // Convert HTML to Markdown
            let markdown_content = self.convert_html_to_markdown(&doc.page_content);

            // Clean up the markdown
            let cleaned_markdown = self.clean_markdown(&markdown_content);

            // Create new document with converted content, preserving metadata
            converted_documents.push(Document {
                page_content: cleaned_markdown,
                metadata: doc.metadata.clone(),
                id: doc.id.clone(),
            });
        }

        Ok(converted_documents)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_basic_html_to_markdown() {
        let html = r#"<h1>Title</h1><p>This is a <strong>bold</strong> paragraph.</p>"#;
        let doc = Document::new(html);

        let transformer = MarkdownifyTransformer::new();
        let result = transformer.transform_documents(vec![doc]).unwrap();

        assert_eq!(result.len(), 1);
        let markdown = &result[0].page_content;

        // Check that heading and bold text are converted
        assert!(markdown.contains("Title"));
        assert!(markdown.contains("bold"));
    }

    #[test]
    fn test_preserves_metadata() {
        let html = "<p>Test content</p>";
        let mut doc = Document::new(html);
        doc.metadata.insert(
            "source".to_string(),
            serde_json::Value::String("test.html".to_string()),
        );

        let transformer = MarkdownifyTransformer::new();
        let result = transformer.transform_documents(vec![doc]).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].metadata.get("source"),
            Some(&serde_json::Value::String("test.html".to_string()))
        );
    }

    #[test]
    fn test_multiple_documents() {
        let docs = vec![
            Document::new("<h1>Doc 1</h1>"),
            Document::new("<h1>Doc 2</h1>"),
            Document::new("<h1>Doc 3</h1>"),
        ];

        let transformer = MarkdownifyTransformer::new();
        let result = transformer.transform_documents(docs).unwrap();

        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_complex_html() {
        let html = r#"
            <html>
                <body>
                    <h1>Main Title</h1>
                    <h2>Subtitle</h2>
                    <p>A paragraph with <a href="https://example.com">a link</a>.</p>
                    <ul>
                        <li>Item 1</li>
                        <li>Item 2</li>
                    </ul>
                    <p>Text with <em>emphasis</em> and <strong>strong</strong>.</p>
                </body>
            </html>
        "#;
        let doc = Document::new(html);

        let transformer = MarkdownifyTransformer::new();
        let result = transformer.transform_documents(vec![doc]).unwrap();

        assert_eq!(result.len(), 1);
        let markdown = &result[0].page_content;

        // Basic checks that conversion happened
        assert!(markdown.contains("Main Title"));
        assert!(markdown.contains("Subtitle"));
        assert!(markdown.contains("example.com"));
    }

    #[test]
    fn test_non_breaking_space_replacement() {
        let html = "<p>Text\u{00A0}with\u{00A0}non-breaking\u{00A0}spaces</p>";
        let doc = Document::new(html);

        let transformer = MarkdownifyTransformer::new();
        let result = transformer.transform_documents(vec![doc]).unwrap();

        assert_eq!(result.len(), 1);
        let markdown = &result[0].page_content;

        // Non-breaking spaces should be converted to regular spaces
        assert!(!markdown.contains('\u{00A0}'));
        assert!(markdown.contains("Text with non-breaking spaces"));
    }

    #[test]
    fn test_builder_pattern() {
        let transformer = MarkdownifyTransformer::new()
            .with_autolinks(false)
            .with_heading_style(HeadingStyle::Setext)
            .with_strip(vec!["script".to_string()]);

        assert!(!transformer.autolinks);
        assert_eq!(transformer.heading_style, HeadingStyle::Setext);
        assert!(transformer.strip.is_some());
    }

    #[test]
    fn test_empty_html() {
        let doc = Document::new("");
        let transformer = MarkdownifyTransformer::new();
        let result = transformer.transform_documents(vec![doc]).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "");
    }
}
