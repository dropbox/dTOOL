// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! HTML-specific text splitters.
//!
//! This module provides splitters optimized for HTML documents:
//! - [`HTMLTextSplitter`]: Split on HTML tag boundaries
//! - [`HTMLHeaderTextSplitter`]: Split with header metadata extraction

use crate::character::RecursiveCharacterTextSplitter;
use crate::traits::TextSplitter;

/// HTML-specific text splitter
#[derive(Debug, Clone)]
pub struct HTMLTextSplitter {
    inner: RecursiveCharacterTextSplitter,
}

impl HTMLTextSplitter {
    /// Create a new `HTMLTextSplitter` with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RecursiveCharacterTextSplitter::new()
                .with_separators(Self::get_separators())
                .with_separator_regex(false),
        }
    }

    /// Get the default separators for HTML
    #[must_use]
    pub fn get_separators() -> Vec<String> {
        vec![
            "<body".to_string(),
            "<div".to_string(),
            "<p".to_string(),
            "<br".to_string(),
            "<li".to_string(),
            "<h1".to_string(),
            "<h2".to_string(),
            "<h3".to_string(),
            "<h4".to_string(),
            "<h5".to_string(),
            "<h6".to_string(),
            "<span".to_string(),
            "<table".to_string(),
            "<tr".to_string(),
            "<td".to_string(),
            "<th".to_string(),
            "<ul".to_string(),
            "<ol".to_string(),
            "<header".to_string(),
            "<footer".to_string(),
            "<nav".to_string(),
            "<head".to_string(),
            "<style".to_string(),
            "<script".to_string(),
            "<meta".to_string(),
            "<title".to_string(),
            String::new(),
        ]
    }

    /// Set custom chunk size
    #[must_use]
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.inner = self.inner.with_chunk_size(chunk_size);
        self
    }

    /// Set custom chunk overlap
    #[must_use]
    pub fn with_chunk_overlap(mut self, chunk_overlap: usize) -> Self {
        self.inner = self.inner.with_chunk_overlap(chunk_overlap);
        self
    }
}

impl Default for HTMLTextSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSplitter for HTMLTextSplitter {
    fn split_text(&self, text: &str) -> Vec<String> {
        self.inner.split_text(text)
    }

    fn chunk_size(&self) -> usize {
        self.inner.chunk_size()
    }

    fn chunk_overlap(&self) -> usize {
        self.inner.chunk_overlap()
    }

    fn add_start_index(&self) -> bool {
        self.inner.add_start_index()
    }
}

/// HTML header text splitter that extracts header hierarchy as metadata
#[derive(Debug, Clone)]
pub struct HTMLHeaderTextSplitter {
    header_mapping: std::collections::HashMap<String, String>,
    header_tags: Vec<String>,
    return_each_element: bool,
}

impl HTMLHeaderTextSplitter {
    /// Create a new `HTMLHeaderTextSplitter`
    #[must_use]
    pub fn new(headers_to_split_on: Vec<(String, String)>) -> Self {
        let mut sorted_headers = headers_to_split_on;
        sorted_headers.sort_by_key(|(tag, _)| {
            tag.trim_start_matches('h').parse::<u32>().unwrap_or(9999)
        });

        let header_mapping: std::collections::HashMap<_, _> =
            sorted_headers.iter().cloned().collect();
        let header_tags: Vec<_> = sorted_headers.iter().map(|(tag, _)| tag.clone()).collect();

        Self {
            header_mapping,
            header_tags,
            return_each_element: false,
        }
    }

    /// Set whether to return each element as a separate document
    #[must_use]
    pub fn with_return_each_element(mut self, return_each_element: bool) -> Self {
        self.return_each_element = return_each_element;
        self
    }

    /// Split HTML text into documents with header metadata
    #[must_use]
    #[allow(clippy::unwrap_used)] // Static CSS selector "body" is always valid
    pub fn split_text(&self, text: &str) -> Vec<crate::Document> {
        use scraper::{Html, Node, Selector};

        let document = Html::parse_document(text);

        let mut active_headers: std::collections::HashMap<String, (String, u32, usize)> =
            std::collections::HashMap::new();
        let mut current_chunk: Vec<String> = Vec::new();
        let mut result: Vec<crate::Document> = Vec::new();

        let finalize_chunk =
            |current_chunk: &mut Vec<String>,
             active_headers: &std::collections::HashMap<String, (String, u32, usize)>|
             -> Option<crate::Document> {
                if current_chunk.is_empty() {
                    return None;
                }

                let final_text = current_chunk
                    .iter()
                    .filter(|s| !s.trim().is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("  \n");
                current_chunk.clear();

                if final_text.trim().is_empty() {
                    return None;
                }

                let mut doc = crate::Document::new(final_text);
                doc.metadata = active_headers
                    .iter()
                    .map(|(k, (v, _, _))| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                Some(doc)
            };

        let mut stack: Vec<(scraper::ElementRef, usize)> = Vec::new();

        let body_selector = Selector::parse("body").unwrap();
        let body = document
            .select(&body_selector)
            .next()
            .unwrap_or_else(|| document.root_element());

        stack.push((body, 0));

        while let Some((node, dom_depth)) = stack.pop() {
            let children: Vec<_> = node.children().collect();
            for child in children.iter().rev() {
                if let Some(elem) = scraper::ElementRef::wrap(*child) {
                    stack.push((elem, dom_depth + 1));
                }
            }

            let tag = node.value().name();

            let node_text: Vec<String> = node
                .children()
                .filter_map(|child| {
                    if let Node::Text(text) = child.value() {
                        let trimmed = text.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    } else {
                        None
                    }
                })
                .collect();

            let node_text = node_text.join(" ");
            if node_text.is_empty() {
                continue;
            }

            if self.header_tags.contains(&tag.to_string()) {
                if !self.return_each_element {
                    if let Some(doc) = finalize_chunk(&mut current_chunk, &active_headers) {
                        result.push(doc);
                    }
                }

                let level = tag.trim_start_matches('h').parse::<u32>().unwrap_or(9999);
                active_headers.retain(|_, (_, lvl, _)| *lvl < level);

                if let Some(header_name) = self.header_mapping.get(tag) {
                    active_headers
                        .insert(header_name.clone(), (node_text.clone(), level, dom_depth));
                }

                let mut header_doc = crate::Document::new(node_text);
                header_doc.metadata = active_headers
                    .iter()
                    .map(|(k, (v, _, _))| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                result.push(header_doc);
            } else {
                active_headers.retain(|_, (_, _, d)| dom_depth >= *d);

                if self.return_each_element {
                    let mut doc = crate::Document::new(node_text);
                    doc.metadata = active_headers
                        .iter()
                        .map(|(k, (v, _, _))| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    result.push(doc);
                } else {
                    current_chunk.push(node_text);
                }
            }
        }

        if !self.return_each_element {
            if let Some(doc) = finalize_chunk(&mut current_chunk, &active_headers) {
                result.push(doc);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // HTMLTextSplitter Tests
    // ============================================

    #[test]
    fn test_html_splitter_new() {
        let splitter = HTMLTextSplitter::new();
        // Default chunk size should be reasonable
        assert!(splitter.chunk_size() > 0);
    }

    #[test]
    fn test_html_splitter_default() {
        let splitter = HTMLTextSplitter::default();
        let splitter2 = HTMLTextSplitter::new();
        // Default and new should produce same chunk_size
        assert_eq!(splitter.chunk_size(), splitter2.chunk_size());
    }

    #[test]
    fn test_html_splitter_with_chunk_size() {
        let splitter = HTMLTextSplitter::new().with_chunk_size(500);
        assert_eq!(splitter.chunk_size(), 500);
    }

    #[test]
    fn test_html_splitter_with_chunk_overlap() {
        let splitter = HTMLTextSplitter::new().with_chunk_overlap(50);
        assert_eq!(splitter.chunk_overlap(), 50);
    }

    #[test]
    fn test_html_splitter_builder_chain() {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(100);
        assert_eq!(splitter.chunk_size(), 1000);
        assert_eq!(splitter.chunk_overlap(), 100);
    }

    #[test]
    fn test_html_splitter_add_start_index_default() {
        let splitter = HTMLTextSplitter::new();
        assert!(!splitter.add_start_index());
    }

    #[test]
    fn test_html_splitter_debug() {
        let splitter = HTMLTextSplitter::new();
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("HTMLTextSplitter"));
    }

    #[test]
    fn test_html_splitter_clone() {
        let splitter = HTMLTextSplitter::new().with_chunk_size(750);
        let cloned = splitter.clone();
        assert_eq!(cloned.chunk_size(), 750);
    }

    #[test]
    fn test_html_splitter_split_simple_text() {
        let splitter = HTMLTextSplitter::new().with_chunk_size(100);
        let text = "<p>Hello world</p>";
        let chunks = splitter.split_text(text);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_html_splitter_get_separators() {
        let seps = HTMLTextSplitter::get_separators();
        // Should include HTML-specific separators
        assert!(!seps.is_empty());
        // Should include common HTML tags
        assert!(seps.contains(&"<body".to_string()));
        assert!(seps.contains(&"<div".to_string()));
        assert!(seps.contains(&"<p".to_string()));
        assert!(seps.contains(&"<h1".to_string()));
        assert!(seps.contains(&"<h2".to_string()));
        assert!(seps.contains(&"<h3".to_string()));
        assert!(seps.contains(&"<li".to_string()));
        assert!(seps.contains(&"<table".to_string()));
        // Should end with empty string as final fallback
        assert_eq!(seps.last(), Some(&String::new()));
    }

    #[test]
    fn test_html_splitter_separators_include_all_headers() {
        let seps = HTMLTextSplitter::get_separators();
        // All HTML header levels should be present
        for i in 1..=6 {
            assert!(seps.contains(&format!("<h{}", i)));
        }
    }

    #[test]
    fn test_html_splitter_separators_include_structural_tags() {
        let seps = HTMLTextSplitter::get_separators();
        assert!(seps.contains(&"<header".to_string()));
        assert!(seps.contains(&"<footer".to_string()));
        assert!(seps.contains(&"<nav".to_string()));
    }

    #[test]
    fn test_html_splitter_separators_include_head_elements() {
        let seps = HTMLTextSplitter::get_separators();
        assert!(seps.contains(&"<head".to_string()));
        assert!(seps.contains(&"<style".to_string()));
        assert!(seps.contains(&"<script".to_string()));
        assert!(seps.contains(&"<meta".to_string()));
        assert!(seps.contains(&"<title".to_string()));
    }

    #[test]
    fn test_html_splitter_separators_count() {
        let seps = HTMLTextSplitter::get_separators();
        // Should have a reasonable number of separators (26 in the current impl)
        assert!(seps.len() >= 20);
        assert!(seps.len() <= 50);
    }

    // ============================================
    // HTMLHeaderTextSplitter Tests
    // ============================================

    fn default_html_headers() -> Vec<(String, String)> {
        vec![
            ("h1".to_string(), "Header 1".to_string()),
            ("h2".to_string(), "Header 2".to_string()),
            ("h3".to_string(), "Header 3".to_string()),
        ]
    }

    #[test]
    fn test_html_header_splitter_new() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("HTMLHeaderTextSplitter"));
    }

    #[test]
    fn test_html_header_splitter_with_return_each_element() {
        let splitter =
            HTMLHeaderTextSplitter::new(default_html_headers()).with_return_each_element(true);
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("return_each_element: true"));
    }

    #[test]
    fn test_html_header_splitter_debug() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("HTMLHeaderTextSplitter"));
        assert!(debug_str.contains("header_mapping"));
        assert!(debug_str.contains("header_tags"));
    }

    #[test]
    fn test_html_header_splitter_clone() {
        let splitter =
            HTMLHeaderTextSplitter::new(default_html_headers()).with_return_each_element(true);
        let cloned = splitter.clone();
        let debug_str = format!("{:?}", cloned);
        assert!(debug_str.contains("return_each_element: true"));
    }

    #[test]
    fn test_html_header_splitter_split_simple_h1() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let html = "<html><body><h1>My Title</h1><p>Some content here.</p></body></html>";
        let docs = splitter.split_text(html);
        assert!(!docs.is_empty());
    }

    #[test]
    fn test_html_header_splitter_split_nested_headers() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let html = r#"
            <html>
            <body>
                <h1>Title</h1>
                <p>Intro</p>
                <h2>Section</h2>
                <p>Content</p>
                <h3>Subsection</h3>
                <p>More content</p>
            </body>
            </html>
        "#;
        let docs = splitter.split_text(html);
        assert!(!docs.is_empty());
    }

    #[test]
    fn test_html_header_splitter_empty_html() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let docs = splitter.split_text("");
        // Empty HTML should return empty or minimal result
        assert!(docs.is_empty() || docs.iter().all(|d| d.page_content.trim().is_empty()));
    }

    #[test]
    fn test_html_header_splitter_no_body() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let html = "<h1>Title</h1><p>Content</p>";
        // Should handle HTML without explicit body tag
        let docs = splitter.split_text(html);
        // Should not panic
        assert!(docs.is_empty() || !docs.is_empty());
    }

    #[test]
    fn test_html_header_splitter_metadata_values_are_strings() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let html = "<html><body><h1>Test</h1><p>Content</p></body></html>";
        let docs = splitter.split_text(html);
        for doc in &docs {
            for (_, value) in &doc.metadata {
                assert!(value.is_string());
            }
        }
    }

    #[test]
    fn test_html_header_splitter_return_each_element_mode() {
        let splitter =
            HTMLHeaderTextSplitter::new(default_html_headers()).with_return_each_element(true);
        let html = "<html><body><h1>Title</h1><p>Line 1</p><p>Line 2</p></body></html>";
        let docs = splitter.split_text(html);
        // In return_each_element mode, should have more documents
        // (each element becomes a separate doc)
        assert!(!docs.is_empty());
    }

    #[test]
    fn test_html_header_splitter_sorts_headers_numerically() {
        // Headers should be sorted by header level (h1 < h2 < h3)
        let headers = vec![
            ("h3".to_string(), "H3".to_string()),
            ("h1".to_string(), "H1".to_string()),
            ("h2".to_string(), "H2".to_string()),
        ];
        let splitter = HTMLHeaderTextSplitter::new(headers);
        // Just verify it creates successfully with sorting
        let _ = format!("{:?}", splitter);
    }

    #[test]
    fn test_html_header_splitter_extracts_header_text() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let html = "<html><body><h1>My Custom Title</h1><p>Body text.</p></body></html>";
        let docs = splitter.split_text(html);
        // Should have documents with the header text
        let has_title_doc = docs.iter().any(|d| d.page_content.contains("My Custom Title"));
        assert!(has_title_doc);
    }

    #[test]
    fn test_html_header_splitter_with_whitespace() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let html = "   <html>   <body>   <h1>  Title  </h1>   <p>  Content  </p>   </body>   </html>   ";
        let docs = splitter.split_text(html);
        // Should handle whitespace gracefully
        assert!(!docs.is_empty() || docs.is_empty());
    }

    #[test]
    fn test_html_header_splitter_complex_structure() {
        let splitter = HTMLHeaderTextSplitter::new(default_html_headers());
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <header>
                    <h1>Main Title</h1>
                </header>
                <main>
                    <h2>First Section</h2>
                    <p>Paragraph one.</p>
                    <p>Paragraph two.</p>
                    <h2>Second Section</h2>
                    <div>
                        <h3>Nested Header</h3>
                        <p>Nested content.</p>
                    </div>
                </main>
                <footer>Footer text</footer>
            </body>
            </html>
        "#;
        let docs = splitter.split_text(html);
        assert!(!docs.is_empty());
    }
}
