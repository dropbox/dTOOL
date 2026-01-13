// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Citation type for source tracking in RAG (Retrieval Augmented Generation)
//!
//! Supports Anthropic's Citations API and general source attribution.

use serde::{Deserialize, Serialize};

/// Citation for tracking source references in RAG
///
/// Used to attribute information to specific sources, enabling
/// verification and transparency in AI responses.
///
/// # Example
///
/// ```rust
/// use dashflow::optimize::types::Citation;
///
/// let citation = Citation::new("Research Paper", "https://example.com/paper.pdf")
///     .with_page(42)
///     .with_quote("The key finding was...");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Citation {
    /// Source title or name
    pub title: String,

    /// Source URL or identifier
    pub url: String,

    /// Optional page number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,

    /// Optional section or heading
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,

    /// Optional quoted text from the source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,

    /// Optional publication date (ISO 8601 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,

    /// Optional author(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Optional confidence score (0.0 to 1.0) for relevance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,

    /// Optional character start position in source document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_start: Option<usize>,

    /// Optional character end position in source document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_end: Option<usize>,
}

impl Citation {
    /// Create a new citation
    ///
    /// # Arguments
    /// * `title` - Source title or name
    /// * `url` - Source URL or identifier
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            page: None,
            section: None,
            quote: None,
            date: None,
            author: None,
            confidence: None,
            char_start: None,
            char_end: None,
        }
    }

    /// Set page number
    #[must_use]
    pub fn with_page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }

    /// Set section or heading
    #[must_use]
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.section = Some(section.into());
        self
    }

    /// Set quoted text
    #[must_use]
    pub fn with_quote(mut self, quote: impl Into<String>) -> Self {
        self.quote = Some(quote.into());
        self
    }

    /// Set publication date
    #[must_use]
    pub fn with_date(mut self, date: impl Into<String>) -> Self {
        self.date = Some(date.into());
        self
    }

    /// Set author(s)
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Set confidence score
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Set character range in source document
    #[must_use]
    pub fn with_char_range(mut self, start: usize, end: usize) -> Self {
        self.char_start = Some(start);
        self.char_end = Some(end);
        self
    }

    /// Get a human-readable citation string
    pub fn to_string_short(&self) -> String {
        if let Some(page) = self.page {
            format!("{} (p. {})", self.title, page)
        } else {
            self.title.clone()
        }
    }

    /// Get a full citation string
    pub fn to_string_full(&self) -> String {
        let mut parts = vec![self.title.clone()];

        if let Some(author) = &self.author {
            parts.insert(0, format!("{}.", author));
        }

        if let Some(date) = &self.date {
            parts.push(format!("({})", date));
        }

        if let Some(page) = self.page {
            parts.push(format!("p. {}", page));
        }

        if let Some(section) = &self.section {
            parts.push(format!("Section: {}", section));
        }

        parts.push(self.url.clone());

        parts.join(". ")
    }

    /// Convert to Anthropic Citations API format
    pub fn to_anthropic_format(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "type": "citation",
            "title": self.title,
            "url": self.url
        });

        if let Some(quote) = &self.quote {
            obj["quoted_text"] = serde_json::Value::String(quote.clone());
        }

        if let (Some(start), Some(end)) = (self.char_start, self.char_end) {
            obj["char_location"] = serde_json::json!({
                "start": start,
                "end": end
            });
        }

        obj
    }
}

impl std::fmt::Display for Citation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_short())
    }
}

/// Collection of citations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Citations(Vec<Citation>);

impl Citations {
    /// Create empty citations collection
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create from a vector of citations
    pub fn from_vec(citations: Vec<Citation>) -> Self {
        Self(citations)
    }

    /// Add a citation
    pub fn add(&mut self, citation: Citation) {
        self.0.push(citation);
    }

    /// Get number of citations
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over citations
    pub fn iter(&self) -> impl Iterator<Item = &Citation> {
        self.0.iter()
    }

    /// Get citation by index
    pub fn get(&self, index: usize) -> Option<&Citation> {
        self.0.get(index)
    }

    /// Filter citations by confidence threshold
    pub fn filter_by_confidence(&self, min_confidence: f64) -> Self {
        Self(
            self.0
                .iter()
                .filter(|c| c.confidence.unwrap_or(1.0) >= min_confidence)
                .cloned()
                .collect(),
        )
    }

    /// Convert to Anthropic format
    pub fn to_anthropic_format(&self) -> Vec<serde_json::Value> {
        self.0.iter().map(|c| c.to_anthropic_format()).collect()
    }
}

impl IntoIterator for Citations {
    type Item = Citation;
    type IntoIter = std::vec::IntoIter<Citation>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Citations {
    type Item = &'a Citation;
    type IntoIter = std::slice::Iter<'a, Citation>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<Citation> for Citations {
    fn from_iter<I: IntoIterator<Item = Citation>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_citation_new() {
        let citation = Citation::new("Test Document", "https://example.com/doc.pdf");
        assert_eq!(citation.title, "Test Document");
        assert_eq!(citation.url, "https://example.com/doc.pdf");
        assert_eq!(citation.page, None);
    }

    #[test]
    fn test_citation_with_page() {
        let citation = Citation::new("Paper", "https://example.com").with_page(42);
        assert_eq!(citation.page, Some(42));
    }

    #[test]
    fn test_citation_with_all_fields() {
        let citation = Citation::new("Research Paper", "https://example.com/paper.pdf")
            .with_page(42)
            .with_section("Introduction")
            .with_quote("Key finding...")
            .with_date("2024-01-15")
            .with_author("Smith et al.")
            .with_confidence(0.95)
            .with_char_range(100, 200);

        assert_eq!(citation.page, Some(42));
        assert_eq!(citation.section, Some("Introduction".to_string()));
        assert_eq!(citation.quote, Some("Key finding...".to_string()));
        assert_eq!(citation.date, Some("2024-01-15".to_string()));
        assert_eq!(citation.author, Some("Smith et al.".to_string()));
        assert_eq!(citation.confidence, Some(0.95));
        assert_eq!(citation.char_start, Some(100));
        assert_eq!(citation.char_end, Some(200));
    }

    #[test]
    fn test_citation_confidence_clamping() {
        let c1 = Citation::new("Test", "url").with_confidence(1.5);
        assert_eq!(c1.confidence, Some(1.0));

        let c2 = Citation::new("Test", "url").with_confidence(-0.5);
        assert_eq!(c2.confidence, Some(0.0));
    }

    #[test]
    fn test_citation_to_string() {
        let citation = Citation::new("Paper", "https://example.com").with_page(42);
        assert_eq!(citation.to_string_short(), "Paper (p. 42)");

        let citation2 = Citation::new("Paper", "https://example.com");
        assert_eq!(citation2.to_string_short(), "Paper");
    }

    #[test]
    fn test_citation_display() {
        let citation = Citation::new("Paper", "https://example.com").with_page(42);
        assert_eq!(format!("{}", citation), "Paper (p. 42)");
    }

    #[test]
    fn test_citations_collection() {
        let mut citations = Citations::new();
        citations.add(Citation::new("Doc 1", "url1"));
        citations.add(Citation::new("Doc 2", "url2"));

        assert_eq!(citations.len(), 2);
        assert!(!citations.is_empty());
        assert_eq!(citations.get(0).unwrap().title, "Doc 1");
    }

    #[test]
    fn test_citations_filter_by_confidence() {
        let citations = Citations::from_vec(vec![
            Citation::new("High", "url1").with_confidence(0.9),
            Citation::new("Low", "url2").with_confidence(0.3),
            Citation::new("Medium", "url3").with_confidence(0.7),
        ]);

        let filtered = citations.filter_by_confidence(0.5);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_anthropic_format() {
        let citation = Citation::new("Paper", "https://example.com")
            .with_quote("Important text")
            .with_char_range(100, 200);

        let json = citation.to_anthropic_format();
        assert_eq!(json["type"], "citation");
        assert_eq!(json["title"], "Paper");
        assert_eq!(json["url"], "https://example.com");
        assert_eq!(json["quoted_text"], "Important text");
        assert_eq!(json["char_location"]["start"], 100);
        assert_eq!(json["char_location"]["end"], 200);
    }

    #[test]
    fn test_serialization() {
        let citation = Citation::new("Test", "url").with_page(10);
        let json = serde_json::to_string(&citation).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("10"));

        let deserialized: Citation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, "Test");
        assert_eq!(deserialized.page, Some(10));
    }

    #[test]
    fn test_citations_iteration() {
        let citations =
            Citations::from_vec(vec![Citation::new("A", "url1"), Citation::new("B", "url2")]);

        let titles: Vec<_> = citations.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["A", "B"]);
    }
}
