// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Document type for citation-enabled content in RAG workflows

use super::citation::Citation;
use serde::{Deserialize, Serialize};

/// Document with optional citation metadata for RAG workflows
///
/// Represents a retrieved document that can be used as context for LLM queries,
/// with built-in support for source attribution.
///
/// # Example
///
/// ```rust
/// use dashflow::optimize::types::{Document, Citation};
///
/// let doc = Document::new("This is the document content...")
///     .with_title("Research Paper")
///     .with_source("https://example.com/paper.pdf")
///     .with_score(0.95);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Document {
    /// Document content/text
    pub content: String,

    /// Optional document title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Optional source URL or identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Optional document ID (for retrieval systems)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Optional relevance/similarity score (0.0 to 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,

    /// Optional page number (for paginated documents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,

    /// Optional chunk index (for chunked documents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_index: Option<usize>,

    /// Optional metadata as key-value pairs
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, String>,
}

impl Document {
    /// Create a new document with content
    ///
    /// # Arguments
    /// * `content` - Document text content
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            title: None,
            source: None,
            id: None,
            score: None,
            page: None,
            chunk_index: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set document title
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set source URL or identifier
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set document ID
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set relevance score
    #[must_use]
    pub fn with_score(mut self, score: f64) -> Self {
        self.score = Some(score.clamp(0.0, 1.0));
        self
    }

    /// Set page number
    #[must_use]
    pub fn with_page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }

    /// Set chunk index
    #[must_use]
    pub fn with_chunk_index(mut self, index: usize) -> Self {
        self.chunk_index = Some(index);
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Get content length
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if document is empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Convert to a Citation
    ///
    /// Creates a Citation from this document's metadata.
    pub fn to_citation(&self) -> Option<Citation> {
        let source = self.source.clone()?;
        let title = self
            .title
            .clone()
            .unwrap_or_else(|| "Untitled Document".to_string());

        let mut citation = Citation::new(title, source);

        if let Some(page) = self.page {
            citation = citation.with_page(page);
        }

        if let Some(score) = self.score {
            citation = citation.with_confidence(score);
        }

        Some(citation)
    }

    /// Truncate content to max length with ellipsis
    pub fn truncate(&self, max_len: usize) -> String {
        if self.content.len() <= max_len {
            self.content.clone()
        } else if max_len > 3 {
            format!("{}...", &self.content[..max_len - 3])
        } else {
            self.content[..max_len].to_string()
        }
    }

    /// Format document for use in LLM prompt
    pub fn format_for_prompt(&self) -> String {
        let mut result = String::new();

        if let Some(title) = &self.title {
            result.push_str(&format!("[{}]\n", title));
        }

        if let Some(source) = &self.source {
            result.push_str(&format!("Source: {}\n", source));
        }

        if let Some(page) = self.page {
            result.push_str(&format!("Page: {}\n", page));
        }

        if !result.is_empty() {
            result.push('\n');
        }

        result.push_str(&self.content);
        result
    }
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.truncate(100))
    }
}

/// Collection of documents (e.g., from retrieval)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Documents(Vec<Document>);

impl Documents {
    /// Create empty documents collection
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create from a vector of documents
    pub fn from_vec(docs: Vec<Document>) -> Self {
        Self(docs)
    }

    /// Add a document
    pub fn add(&mut self, doc: Document) {
        self.0.push(doc);
    }

    /// Get number of documents
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over documents
    pub fn iter(&self) -> impl Iterator<Item = &Document> {
        self.0.iter()
    }

    /// Get document by index
    pub fn get(&self, index: usize) -> Option<&Document> {
        self.0.get(index)
    }

    /// Sort by score (highest first)
    pub fn sort_by_score(&mut self) {
        self.0.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Filter by minimum score
    pub fn filter_by_score(&self, min_score: f64) -> Self {
        Self(
            self.0
                .iter()
                .filter(|d| d.score.unwrap_or(0.0) >= min_score)
                .cloned()
                .collect(),
        )
    }

    /// Get top N documents by score
    pub fn top_n(&self, n: usize) -> Self {
        let mut sorted = self.clone();
        sorted.sort_by_score();
        Self(sorted.0.into_iter().take(n).collect())
    }

    /// Combine all document contents with separator
    pub fn combine_content(&self, separator: &str) -> String {
        self.0
            .iter()
            .map(|d| d.content.as_str())
            .collect::<Vec<_>>()
            .join(separator)
    }

    /// Format all documents for LLM prompt
    pub fn format_for_prompt(&self, separator: &str) -> String {
        self.0
            .iter()
            .enumerate()
            .map(|(i, d)| format!("[Document {}]\n{}", i + 1, d.format_for_prompt()))
            .collect::<Vec<_>>()
            .join(separator)
    }

    /// Convert all documents to citations
    pub fn to_citations(&self) -> Vec<Citation> {
        self.0.iter().filter_map(|d| d.to_citation()).collect()
    }
}

impl IntoIterator for Documents {
    type Item = Document;
    type IntoIter = std::vec::IntoIter<Document>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Documents {
    type Item = &'a Document;
    type IntoIter = std::slice::Iter<'a, Document>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<Document> for Documents {
    fn from_iter<I: IntoIterator<Item = Document>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_new() {
        let doc = Document::new("Test content");
        assert_eq!(doc.content, "Test content");
        assert_eq!(doc.title, None);
    }

    #[test]
    fn test_document_builder() {
        let doc = Document::new("Content")
            .with_title("Title")
            .with_source("https://example.com")
            .with_id("doc-123")
            .with_score(0.95)
            .with_page(5)
            .with_chunk_index(2)
            .with_metadata("author", "Smith");

        assert_eq!(doc.title, Some("Title".to_string()));
        assert_eq!(doc.source, Some("https://example.com".to_string()));
        assert_eq!(doc.id, Some("doc-123".to_string()));
        assert_eq!(doc.score, Some(0.95));
        assert_eq!(doc.page, Some(5));
        assert_eq!(doc.chunk_index, Some(2));
        assert_eq!(doc.get_metadata("author"), Some("Smith"));
    }

    #[test]
    fn test_document_score_clamping() {
        let d1 = Document::new("Test").with_score(1.5);
        assert_eq!(d1.score, Some(1.0));

        let d2 = Document::new("Test").with_score(-0.5);
        assert_eq!(d2.score, Some(0.0));
    }

    #[test]
    fn test_document_to_citation() {
        let doc = Document::new("Content")
            .with_title("Paper")
            .with_source("https://example.com")
            .with_page(42)
            .with_score(0.9);

        let citation = doc.to_citation().unwrap();
        assert_eq!(citation.title, "Paper");
        assert_eq!(citation.url, "https://example.com");
        assert_eq!(citation.page, Some(42));
        assert_eq!(citation.confidence, Some(0.9));
    }

    #[test]
    fn test_document_truncate() {
        let doc = Document::new("This is a long document content");
        assert_eq!(doc.truncate(15), "This is a lo...");
        assert_eq!(doc.truncate(100), "This is a long document content");
    }

    #[test]
    fn test_document_format_for_prompt() {
        let doc = Document::new("Content here")
            .with_title("Title")
            .with_source("https://example.com");

        let formatted = doc.format_for_prompt();
        assert!(formatted.contains("[Title]"));
        assert!(formatted.contains("Source: https://example.com"));
        assert!(formatted.contains("Content here"));
    }

    #[test]
    fn test_documents_collection() {
        let mut docs = Documents::new();
        docs.add(Document::new("Doc 1"));
        docs.add(Document::new("Doc 2"));

        assert_eq!(docs.len(), 2);
        assert!(!docs.is_empty());
        assert_eq!(docs.get(0).unwrap().content, "Doc 1");
    }

    #[test]
    fn test_documents_sort_by_score() {
        let mut docs = Documents::from_vec(vec![
            Document::new("Low").with_score(0.3),
            Document::new("High").with_score(0.9),
            Document::new("Medium").with_score(0.6),
        ]);

        docs.sort_by_score();
        assert_eq!(docs.get(0).unwrap().content, "High");
        assert_eq!(docs.get(1).unwrap().content, "Medium");
        assert_eq!(docs.get(2).unwrap().content, "Low");
    }

    #[test]
    fn test_documents_filter_by_score() {
        let docs = Documents::from_vec(vec![
            Document::new("High").with_score(0.9),
            Document::new("Low").with_score(0.3),
            Document::new("Medium").with_score(0.6),
        ]);

        let filtered = docs.filter_by_score(0.5);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_documents_top_n() {
        let docs = Documents::from_vec(vec![
            Document::new("D1").with_score(0.3),
            Document::new("D2").with_score(0.9),
            Document::new("D3").with_score(0.6),
        ]);

        let top2 = docs.top_n(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2.get(0).unwrap().content, "D2");
        assert_eq!(top2.get(1).unwrap().content, "D3");
    }

    #[test]
    fn test_documents_combine_content() {
        let docs = Documents::from_vec(vec![Document::new("First"), Document::new("Second")]);

        assert_eq!(docs.combine_content("\n\n"), "First\n\nSecond");
    }

    #[test]
    fn test_serialization() {
        let doc = Document::new("Content").with_title("Title").with_score(0.8);

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("Content"));
        assert!(json.contains("Title"));

        let deserialized: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content, "Content");
        assert_eq!(deserialized.title, Some("Title".to_string()));
    }
}
