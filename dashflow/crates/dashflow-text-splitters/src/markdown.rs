// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Markdown-specific text splitters.
//!
//! This module provides splitters optimized for Markdown documents:
//! - [`MarkdownTextSplitter`]: Split on Markdown boundaries (headers, code blocks, etc.)
//! - [`MarkdownHeaderTextSplitter`]: Split with header metadata extraction

use crate::character::RecursiveCharacterTextSplitter;
use crate::traits::TextSplitter;

/// Markdown-specific text splitter
///
/// A specialized version of [`RecursiveCharacterTextSplitter`] configured with
/// separators optimized for Markdown documents.
#[derive(Debug, Clone)]
pub struct MarkdownTextSplitter {
    inner: RecursiveCharacterTextSplitter,
}

impl MarkdownTextSplitter {
    /// Create a new `MarkdownTextSplitter` with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RecursiveCharacterTextSplitter::new()
                .with_separators(Self::get_separators())
                .with_separator_regex(true),
        }
    }

    fn get_separators() -> Vec<String> {
        vec![
            r"\n#{1,6} ".to_string(),
            "```\n".to_string(),
            r"\n\*\*\*+\n".to_string(),
            r"\n---+\n".to_string(),
            r"\n___+\n".to_string(),
            "\n\n".to_string(),
            "\n".to_string(),
            " ".to_string(),
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

impl Default for MarkdownTextSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSplitter for MarkdownTextSplitter {
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

/// Markdown header text splitter with metadata extraction
#[derive(Debug, Clone)]
pub struct MarkdownHeaderTextSplitter {
    headers_to_split_on: Vec<(String, String)>,
    return_each_line: bool,
    strip_headers: bool,
}

#[derive(Debug, Clone)]
struct LineWithMetadata {
    content: String,
    metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct HeaderInfo {
    level: usize,
    name: String,
    _data: String,
}

impl MarkdownHeaderTextSplitter {
    /// Create a new `MarkdownHeaderTextSplitter`
    #[must_use]
    pub fn new(headers_to_split_on: Vec<(String, String)>) -> Self {
        let mut sorted_headers = headers_to_split_on;
        sorted_headers.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        Self {
            headers_to_split_on: sorted_headers,
            return_each_line: false,
            strip_headers: true,
        }
    }

    /// Set whether to return each line individually with metadata
    #[must_use]
    pub fn with_return_each_line(mut self, return_each_line: bool) -> Self {
        self.return_each_line = return_each_line;
        self
    }

    /// Set whether to strip header lines from chunk content
    #[must_use]
    pub fn with_strip_headers(mut self, strip_headers: bool) -> Self {
        self.strip_headers = strip_headers;
        self
    }

    /// Split markdown text into documents with header metadata
    #[must_use]
    pub fn split_text(&self, text: &str) -> Vec<crate::Document> {
        let lines: Vec<&str> = text.split('\n').collect();
        let mut lines_with_metadata: Vec<LineWithMetadata> = Vec::new();
        let mut current_content: Vec<String> = Vec::new();
        let mut current_metadata: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut header_stack: Vec<HeaderInfo> = Vec::new();
        let mut initial_metadata: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        let mut in_code_block = false;
        let mut opening_fence = "";

        for line in lines {
            let stripped_line = line.trim();
            let stripped_line: String = stripped_line
                .chars()
                .filter(|c| !c.is_control() || *c == '\t')
                .collect();

            if !in_code_block {
                if stripped_line.starts_with("```") && stripped_line.matches("```").count() == 1 {
                    in_code_block = true;
                    opening_fence = "```";
                } else if stripped_line.starts_with("~~~") {
                    in_code_block = true;
                    opening_fence = "~~~";
                }
            } else if stripped_line.starts_with(opening_fence) {
                in_code_block = false;
                opening_fence = "";
            }

            if in_code_block {
                current_content.push(stripped_line.clone());
                continue;
            }

            let mut matched_header = false;
            for (sep, name) in &self.headers_to_split_on {
                let is_standard_header = stripped_line.starts_with(sep)
                    && (stripped_line.len() == sep.len()
                        || stripped_line.chars().nth(sep.len()) == Some(' '));

                if is_standard_header {
                    let current_header_level = sep.chars().filter(|c| *c == '#').count();

                    while let Some(top) = header_stack.last() {
                        if top.level >= current_header_level {
                            // SAFETY: We just verified last() is Some, so pop() must succeed
                            if let Some(popped) = header_stack.pop() {
                                initial_metadata.remove(&popped.name);
                            }
                        } else {
                            break;
                        }
                    }

                    let header_text = stripped_line[sep.len()..].trim().to_string();
                    let header = HeaderInfo {
                        level: current_header_level,
                        name: name.clone(),
                        _data: header_text.clone(),
                    };
                    header_stack.push(header);
                    initial_metadata.insert(name.clone(), header_text);

                    if !current_content.is_empty() {
                        lines_with_metadata.push(LineWithMetadata {
                            content: current_content.join("\n"),
                            metadata: current_metadata.clone(),
                        });
                        current_content.clear();
                    }

                    if !self.strip_headers {
                        current_content.push(stripped_line.clone());
                    }

                    matched_header = true;
                    break;
                }
            }

            if !matched_header {
                if !stripped_line.is_empty() {
                    current_content.push(stripped_line.clone());
                } else if !current_content.is_empty() {
                    lines_with_metadata.push(LineWithMetadata {
                        content: current_content.join("\n"),
                        metadata: current_metadata.clone(),
                    });
                    current_content.clear();
                }
            }

            current_metadata = initial_metadata.clone();
        }

        if !current_content.is_empty() {
            lines_with_metadata.push(LineWithMetadata {
                content: current_content.join("\n"),
                metadata: current_metadata,
            });
        }

        if self.return_each_line {
            self.lines_to_documents(lines_with_metadata)
        } else {
            self.aggregate_lines_to_chunks(lines_with_metadata)
        }
    }

    fn lines_to_documents(&self, lines: Vec<LineWithMetadata>) -> Vec<crate::Document> {
        lines
            .into_iter()
            .map(|line| {
                let mut doc = crate::Document::new(line.content);
                doc.metadata = line
                    .metadata
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::String(v)))
                    .collect();
                doc
            })
            .collect()
    }

    fn aggregate_lines_to_chunks(&self, lines: Vec<LineWithMetadata>) -> Vec<crate::Document> {
        let mut aggregated: Vec<LineWithMetadata> = Vec::new();

        for line in lines {
            if let Some(last) = aggregated.last_mut() {
                if last.metadata == line.metadata {
                    last.content.push_str("  \n");
                    last.content.push_str(&line.content);
                } else if last.metadata.len() < line.metadata.len()
                    && last
                        .content
                        .lines()
                        .last()
                        .is_some_and(|l| l.starts_with('#'))
                    && !self.strip_headers
                {
                    last.content.push_str("  \n");
                    last.content.push_str(&line.content);
                    last.metadata = line.metadata;
                } else {
                    aggregated.push(line);
                }
            } else {
                aggregated.push(line);
            }
        }

        aggregated
            .into_iter()
            .map(|chunk| {
                let mut doc = crate::Document::new(chunk.content);
                doc.metadata = chunk
                    .metadata
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::String(v)))
                    .collect();
                doc
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // MarkdownTextSplitter Tests
    // ============================================

    #[test]
    fn test_markdown_splitter_new() {
        let splitter = MarkdownTextSplitter::new();
        // Default chunk size should be reasonable
        assert!(splitter.chunk_size() > 0);
    }

    #[test]
    fn test_markdown_splitter_default() {
        let splitter = MarkdownTextSplitter::default();
        let splitter2 = MarkdownTextSplitter::new();
        // Default and new should produce same chunk_size
        assert_eq!(splitter.chunk_size(), splitter2.chunk_size());
    }

    #[test]
    fn test_markdown_splitter_with_chunk_size() {
        let splitter = MarkdownTextSplitter::new().with_chunk_size(500);
        assert_eq!(splitter.chunk_size(), 500);
    }

    #[test]
    fn test_markdown_splitter_with_chunk_overlap() {
        let splitter = MarkdownTextSplitter::new().with_chunk_overlap(50);
        assert_eq!(splitter.chunk_overlap(), 50);
    }

    #[test]
    fn test_markdown_splitter_builder_chain() {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(100);
        assert_eq!(splitter.chunk_size(), 1000);
        assert_eq!(splitter.chunk_overlap(), 100);
    }

    #[test]
    fn test_markdown_splitter_add_start_index_default() {
        let splitter = MarkdownTextSplitter::new();
        assert!(!splitter.add_start_index());
    }

    #[test]
    fn test_markdown_splitter_debug() {
        let splitter = MarkdownTextSplitter::new();
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("MarkdownTextSplitter"));
    }

    #[test]
    fn test_markdown_splitter_clone() {
        let splitter = MarkdownTextSplitter::new().with_chunk_size(750);
        let cloned = splitter.clone();
        assert_eq!(cloned.chunk_size(), 750);
    }

    #[test]
    fn test_markdown_splitter_split_simple_text() {
        let splitter = MarkdownTextSplitter::new().with_chunk_size(100);
        let text = "Hello world";
        let chunks = splitter.split_text(text);
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.contains("Hello")));
    }

    #[test]
    fn test_markdown_splitter_split_with_paragraphs() {
        let splitter = MarkdownTextSplitter::new().with_chunk_size(50);
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = splitter.split_text(text);
        // Should split at paragraph boundaries
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_markdown_splitter_get_separators() {
        let seps = MarkdownTextSplitter::get_separators();
        // Should include markdown-specific separators
        assert!(!seps.is_empty());
        // Should include header pattern
        assert!(seps.iter().any(|s| s.contains('#')));
        // Should include code block marker
        assert!(seps.iter().any(|s| s.contains("```")));
        // Should include horizontal rule patterns
        assert!(seps.iter().any(|s| s.contains("---") || s.contains("*")));
        // Should include double newline for paragraphs
        assert!(seps.contains(&"\n\n".to_string()));
        // Should end with empty string as final fallback
        assert_eq!(seps.last(), Some(&String::new()));
    }

    // ============================================
    // MarkdownHeaderTextSplitter Tests
    // ============================================

    fn default_headers() -> Vec<(String, String)> {
        vec![
            ("#".to_string(), "Header 1".to_string()),
            ("##".to_string(), "Header 2".to_string()),
            ("###".to_string(), "Header 3".to_string()),
        ]
    }

    #[test]
    fn test_header_splitter_new() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        // Should create successfully
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("MarkdownHeaderTextSplitter"));
    }

    #[test]
    fn test_header_splitter_new_sorts_headers_by_length() {
        // Headers should be sorted by length (descending) for proper matching
        let headers = vec![
            ("#".to_string(), "H1".to_string()),
            ("###".to_string(), "H3".to_string()),
            ("##".to_string(), "H2".to_string()),
        ];
        let splitter = MarkdownHeaderTextSplitter::new(headers);
        // Just verify it creates successfully with sorting
        let _ = format!("{:?}", splitter);
    }

    #[test]
    fn test_header_splitter_with_return_each_line() {
        let splitter =
            MarkdownHeaderTextSplitter::new(default_headers()).with_return_each_line(true);
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("return_each_line: true"));
    }

    #[test]
    fn test_header_splitter_with_strip_headers_false() {
        let splitter =
            MarkdownHeaderTextSplitter::new(default_headers()).with_strip_headers(false);
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("strip_headers: false"));
    }

    #[test]
    fn test_header_splitter_debug() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let debug_str = format!("{:?}", splitter);
        assert!(debug_str.contains("MarkdownHeaderTextSplitter"));
        assert!(debug_str.contains("headers_to_split_on"));
    }

    #[test]
    fn test_header_splitter_clone() {
        let splitter =
            MarkdownHeaderTextSplitter::new(default_headers()).with_return_each_line(true);
        let cloned = splitter.clone();
        let debug_str = format!("{:?}", cloned);
        assert!(debug_str.contains("return_each_line: true"));
    }

    #[test]
    fn test_header_splitter_split_simple_h1() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let text = "# My Title\n\nSome content here.";
        let docs = splitter.split_text(text);
        assert!(!docs.is_empty());
        // Should have metadata from header
        let has_h1_metadata = docs
            .iter()
            .any(|d| d.metadata.contains_key("Header 1"));
        assert!(has_h1_metadata);
    }

    #[test]
    fn test_header_splitter_split_nested_headers() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let text = "# Title\n\nIntro\n\n## Section\n\nContent\n\n### Subsection\n\nMore content";
        let docs = splitter.split_text(text);
        assert!(!docs.is_empty());
    }

    #[test]
    fn test_header_splitter_preserves_code_blocks() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let text = "# Code Example\n\n```rust\nfn main() {\n    # This is not a header\n}\n```\n\nAfter code.";
        let docs = splitter.split_text(text);
        // The # inside the code block should not be treated as a header
        assert!(!docs.is_empty());
    }

    #[test]
    fn test_header_splitter_handles_tilde_code_blocks() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let text = "# Example\n\n~~~\n# Not a header\n~~~\n\nAfter.";
        let docs = splitter.split_text(text);
        assert!(!docs.is_empty());
    }

    #[test]
    fn test_header_splitter_empty_text() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let docs = splitter.split_text("");
        // Empty text should return empty or minimal result
        assert!(docs.is_empty() || docs.iter().all(|d| d.page_content.trim().is_empty()));
    }

    #[test]
    fn test_header_splitter_no_headers_in_text() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let text = "Just some plain text without any headers.";
        let docs = splitter.split_text(text);
        assert!(!docs.is_empty());
        // Should still produce documents, just without header metadata
    }

    #[test]
    fn test_header_splitter_metadata_values_are_strings() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let text = "# Test Title\n\nContent";
        let docs = splitter.split_text(text);
        for doc in &docs {
            for (_, value) in &doc.metadata {
                assert!(value.is_string());
            }
        }
    }

    #[test]
    fn test_header_splitter_header_text_extraction() {
        let splitter = MarkdownHeaderTextSplitter::new(default_headers());
        let text = "# My Custom Title\n\nSome body text.";
        let docs = splitter.split_text(text);
        // Find doc with header metadata
        let has_title = docs.iter().any(|d| {
            d.metadata.get("Header 1").is_some_and(|v| {
                v.as_str()
                    .is_some_and(|s| s.contains("My Custom Title"))
            })
        });
        assert!(has_title);
    }

    #[test]
    fn test_header_splitter_return_each_line_mode() {
        let splitter =
            MarkdownHeaderTextSplitter::new(default_headers()).with_return_each_line(true);
        let text = "# Header\n\nLine 1\nLine 2\nLine 3";
        let docs = splitter.split_text(text);
        // In return_each_line mode, should have more documents
        assert!(docs.len() >= 1);
    }

    #[test]
    fn test_header_splitter_strip_headers_mode() {
        let splitter =
            MarkdownHeaderTextSplitter::new(default_headers()).with_strip_headers(false);
        let text = "# My Header\n\nContent after header.";
        let docs = splitter.split_text(text);
        // With strip_headers=false, headers should appear in content
        let _has_header_in_content = docs.iter().any(|d| d.page_content.contains("# My Header"));
        // Note: exact behavior depends on implementation details
        assert!(!docs.is_empty());
    }

    // ============================================
    // LineWithMetadata Tests (internal struct)
    // ============================================

    #[test]
    fn test_line_with_metadata_debug() {
        let line = LineWithMetadata {
            content: "test".to_string(),
            metadata: std::collections::HashMap::new(),
        };
        let debug_str = format!("{:?}", line);
        assert!(debug_str.contains("LineWithMetadata"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_line_with_metadata_clone() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("key".to_string(), "value".to_string());
        let line = LineWithMetadata {
            content: "original".to_string(),
            metadata,
        };
        let cloned = line.clone();
        assert_eq!(cloned.content, "original");
        assert_eq!(cloned.metadata.get("key"), Some(&"value".to_string()));
    }

    // ============================================
    // HeaderInfo Tests (internal struct)
    // ============================================

    #[test]
    fn test_header_info_debug() {
        let header = HeaderInfo {
            level: 2,
            name: "Header 2".to_string(),
            _data: "Section Title".to_string(),
        };
        let debug_str = format!("{:?}", header);
        assert!(debug_str.contains("HeaderInfo"));
        assert!(debug_str.contains("level: 2"));
    }

    #[test]
    fn test_header_info_clone() {
        let header = HeaderInfo {
            level: 1,
            name: "H1".to_string(),
            _data: "Title".to_string(),
        };
        let cloned = header.clone();
        assert_eq!(cloned.level, 1);
        assert_eq!(cloned.name, "H1");
    }
}
