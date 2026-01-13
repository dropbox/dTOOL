//! Document compressor using cross-encoder models for reranking
//!
//! This module provides a document compressor that uses cross-encoder models
//! to rerank documents based on their relevance to a query.
//!
//! # Overview
//!
//! Cross-encoders are neural models that directly score the relevance of
//! (query, document) pairs. They are more accurate than bi-encoder approaches
//! (like embeddings) but require scoring each pair individually, making them
//! slower for large document sets.
//!
//! Typical workflow:
//! 1. Use fast bi-encoder (embeddings) to retrieve top-k candidates (e.g., k=100)
//! 2. Use cross-encoder to rerank top-n results (e.g., n=10)
//!
//! # Implementation Requirements
//!
//! This compressor requires a `CrossEncoder` implementation. Options include:
//! - ONNX runtime with exported cross-encoder models
//! - API services providing cross-encoder scoring
//! - C++ bindings to sentence-transformers
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_document_compressors::{CrossEncoderRerank, CrossEncoder};
//! use dashflow::core::documents::Document;
//!
//! // Assume we have a CrossEncoder implementation
//! let model: Box<dyn CrossEncoder> = get_cross_encoder_model();
//!
//! let reranker = CrossEncoderRerank::new(model)
//!     .with_top_n(3);
//!
//! let documents = vec![
//!     Document::new("Rust is a systems programming language"),
//!     Document::new("Python is great for data science"),
//!     Document::new("Rust has memory safety without garbage collection"),
//! ];
//!
//! let reranked = reranker
//!     .compress_documents(documents, "What is Rust?", None)
//!     .await?;
//!
//! // reranked will contain the top 3 documents sorted by relevance score
//! assert_eq!(reranked.len(), 3);
//! ```

use async_trait::async_trait;
use dashflow::core::documents::{Document, DocumentCompressor};
use dashflow::core::error::Result;

use crate::cross_encoder::CrossEncoder;

/// Document compressor that uses cross-encoder models for reranking.
///
/// Cross-encoders provide more accurate relevance scoring than bi-encoders
/// (embeddings) but are slower as they must score each (query, document) pair.
///
/// # Usage Pattern
///
/// 1. Retrieve candidates with fast bi-encoder (e.g., top-100)
/// 2. Rerank with cross-encoder (e.g., top-10)
/// 3. Use reranked results for downstream tasks
pub struct CrossEncoderRerank {
    /// Cross-encoder model for scoring query-document pairs
    model: Box<dyn CrossEncoder>,
    /// Number of top documents to return after reranking
    top_n: usize,
}

impl CrossEncoderRerank {
    /// Create a new `CrossEncoderRerank` compressor.
    ///
    /// # Arguments
    ///
    /// * `model` - A cross-encoder implementation for scoring documents
    ///
    /// # Default Values
    ///
    /// * `top_n` = 3 - Returns top 3 documents by default
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let model: Box<dyn CrossEncoder> = get_model();
    /// let reranker = CrossEncoderRerank::new(model);
    /// ```
    #[must_use]
    pub fn new(model: Box<dyn CrossEncoder>) -> Self {
        Self { model, top_n: 3 }
    }

    /// Set the number of documents to return after reranking.
    ///
    /// # Arguments
    ///
    /// * `top_n` - Number of documents to return
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let reranker = CrossEncoderRerank::new(model).with_top_n(5);
    /// ```
    #[must_use]
    pub fn with_top_n(mut self, top_n: usize) -> Self {
        self.top_n = top_n;
        self
    }

    /// Get the current `top_n` setting.
    #[must_use]
    pub fn top_n(&self) -> usize {
        self.top_n
    }
}

#[async_trait]
impl DocumentCompressor for CrossEncoderRerank {
    /// Rerank documents using cross-encoder model.
    ///
    /// Scores each (query, `document.page_content`) pair and returns the
    /// top-n documents sorted by relevance score (highest first).
    ///
    /// # Arguments
    ///
    /// * `documents` - Documents to rerank
    /// * `query` - Query to compare documents against
    /// * `_callbacks` - Optional callbacks (currently unused)
    ///
    /// # Returns
    ///
    /// Top-n documents sorted by relevance score (highest to lowest).
    /// The `relevance_score` is added to each document's metadata.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let reranked = reranker
    ///     .compress_documents(docs, "machine learning", None)
    ///     .await?;
    ///
    /// for doc in reranked {
    ///     if let Some(score) = doc.metadata.get("relevance_score") {
    ///         println!("Score: {}", score);
    ///     }
    /// }
    /// ```
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        _config: Option<&dashflow::core::config::RunnableConfig>,
    ) -> Result<Vec<Document>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        // Create (query, document) pairs for scoring
        let text_pairs: Vec<(String, String)> = documents
            .iter()
            .map(|doc| (query.to_string(), doc.page_content.clone()))
            .collect();

        // Score all pairs
        let scores = self.model.score(text_pairs).await?;

        // Zip documents with their scores
        let mut docs_with_scores: Vec<(Document, f32)> =
            documents.into_iter().zip(scores).collect();

        // Sort by score descending (highest relevance first)
        docs_with_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-n and add relevance scores to metadata
        let result: Vec<Document> = docs_with_scores
            .into_iter()
            .take(self.top_n)
            .map(|(mut doc, score)| {
                doc.metadata
                    .insert("relevance_score".to_string(), serde_json::json!(score));
                doc
            })
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::documents::Document;

    // ============================================================
    // MOCK CROSS ENCODER IMPLEMENTATIONS
    // ============================================================

    /// Mock CrossEncoder that returns scores based on document position
    struct MockCrossEncoder {
        /// If true, return decreasing scores; if false, return increasing scores
        reverse: bool,
    }

    #[async_trait]
    impl CrossEncoder for MockCrossEncoder {
        async fn score(&self, text_pairs: Vec<(String, String)>) -> Result<Vec<f32>> {
            if self.reverse {
                // Return decreasing scores for testing
                Ok((0..text_pairs.len())
                    .map(|i| 1.0 - (i as f32 * 0.1))
                    .collect())
            } else {
                // Return increasing scores
                Ok((0..text_pairs.len()).map(|i| i as f32 * 0.1).collect())
            }
        }
    }

    /// Mock CrossEncoder that returns fixed scores
    struct FixedScoreEncoder {
        scores: Vec<f32>,
    }

    #[async_trait]
    impl CrossEncoder for FixedScoreEncoder {
        async fn score(&self, _text_pairs: Vec<(String, String)>) -> Result<Vec<f32>> {
            Ok(self.scores.clone())
        }
    }

    /// Mock CrossEncoder that returns scores based on query matching
    struct QueryMatchEncoder;

    #[async_trait]
    impl CrossEncoder for QueryMatchEncoder {
        async fn score(&self, text_pairs: Vec<(String, String)>) -> Result<Vec<f32>> {
            // Score based on how much the document contains words from the query
            Ok(text_pairs
                .iter()
                .map(|(query, doc)| {
                    let query_lower = query.to_lowercase();
                    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
                    let doc_lower = doc.to_lowercase();
                    let matches = query_words
                        .iter()
                        .filter(|w| doc_lower.contains(*w))
                        .count();
                    matches as f32 / query_words.len().max(1) as f32
                })
                .collect())
        }
    }

    // ============================================================
    // BASIC FUNCTIONALITY TESTS
    // ============================================================

    #[tokio::test]
    async fn test_cross_encoder_rerank_basic() {
        let model = Box::new(MockCrossEncoder { reverse: false });
        let reranker = CrossEncoderRerank::new(model).with_top_n(2);

        let documents = vec![
            Document::new("First document"),
            Document::new("Second document"),
            Document::new("Third document"),
        ];

        let result = reranker
            .compress_documents(documents, "test query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        // Should return last document first (highest score)
        assert_eq!(result[0].page_content, "Third document");
        assert_eq!(result[1].page_content, "Second document");
    }

    #[tokio::test]
    async fn test_cross_encoder_rerank_with_metadata() {
        let model = Box::new(MockCrossEncoder { reverse: true });
        let reranker = CrossEncoderRerank::new(model);

        let documents = vec![
            Document::new("First document"),
            Document::new("Second document"),
        ];

        let result = reranker
            .compress_documents(documents, "test query", None)
            .await
            .unwrap();

        // Check that relevance scores were added
        assert!(result[0].metadata.contains_key("relevance_score"));
        assert!(result[1].metadata.contains_key("relevance_score"));

        // First document should have highest score (1.0)
        let score1 = result[0].metadata.get("relevance_score").unwrap();
        assert!((score1.as_f64().unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_cross_encoder_rerank_empty_input() {
        let model = Box::new(MockCrossEncoder { reverse: false });
        let reranker = CrossEncoderRerank::new(model);

        let documents: Vec<Document> = vec![];
        let result = reranker
            .compress_documents(documents, "test query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_cross_encoder_rerank_top_n_larger_than_docs() {
        let model = Box::new(MockCrossEncoder { reverse: false });
        let reranker = CrossEncoderRerank::new(model).with_top_n(10);

        let documents = vec![
            Document::new("First document"),
            Document::new("Second document"),
        ];

        let result = reranker
            .compress_documents(documents, "test query", None)
            .await
            .unwrap();

        // Should return all documents when top_n > len(documents)
        assert_eq!(result.len(), 2);
    }

    // ============================================================
    // BUILDER PATTERN TESTS
    // ============================================================

    #[test]
    fn test_builder_pattern() {
        let model = Box::new(MockCrossEncoder { reverse: false });
        let reranker = CrossEncoderRerank::new(model).with_top_n(5);

        assert_eq!(reranker.top_n(), 5);
    }

    #[test]
    fn test_default_top_n() {
        let model = Box::new(MockCrossEncoder { reverse: false });
        let reranker = CrossEncoderRerank::new(model);

        // Default top_n should be 3
        assert_eq!(reranker.top_n(), 3);
    }

    #[test]
    fn test_with_top_n_zero() {
        let model = Box::new(MockCrossEncoder { reverse: false });
        let reranker = CrossEncoderRerank::new(model).with_top_n(0);

        assert_eq!(reranker.top_n(), 0);
    }

    #[test]
    fn test_with_top_n_large() {
        let model = Box::new(MockCrossEncoder { reverse: false });
        let reranker = CrossEncoderRerank::new(model).with_top_n(1000);

        assert_eq!(reranker.top_n(), 1000);
    }

    // ============================================================
    // SINGLE DOCUMENT TESTS
    // ============================================================

    #[tokio::test]
    async fn test_single_document() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.95],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(5);

        let documents = vec![Document::new("Only document")];
        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "Only document");
        let score = result[0].metadata.get("relevance_score").unwrap();
        assert!((score.as_f64().unwrap() - 0.95).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_single_document_with_top_n_one() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.5],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(1);

        let documents = vec![Document::new("Only document")];
        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
    }

    // ============================================================
    // SCORE HANDLING TESTS
    // ============================================================

    #[tokio::test]
    async fn test_equal_scores() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.5, 0.5, 0.5],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(3);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 3);
        // All should have same score
        for doc in &result {
            let score = doc.metadata.get("relevance_score").unwrap();
            assert!((score.as_f64().unwrap() - 0.5).abs() < 1e-6);
        }
    }

    #[tokio::test]
    async fn test_negative_scores() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![-0.5, -0.3, -0.8],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(3);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 3);
        // Should be sorted by score descending: B (-0.3), A (-0.5), C (-0.8)
        assert_eq!(result[0].page_content, "Doc B");
        assert_eq!(result[1].page_content, "Doc A");
        assert_eq!(result[2].page_content, "Doc C");
    }

    #[tokio::test]
    async fn test_very_large_scores() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![1e10, 1e9, 1e8],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(3);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].page_content, "Doc A"); // Highest score
    }

    #[tokio::test]
    async fn test_very_small_scores() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![1e-10, 1e-9, 1e-8],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(3);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].page_content, "Doc C"); // Highest (1e-8)
    }

    #[tokio::test]
    async fn test_zero_scores() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.0, 0.0, 0.0],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(3);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 3);
        // All scores should be 0.0
        for doc in &result {
            let score = doc.metadata.get("relevance_score").unwrap();
            assert!((score.as_f64().unwrap() - 0.0).abs() < 1e-6);
        }
    }

    // ============================================================
    // METADATA PRESERVATION TESTS
    // ============================================================

    #[tokio::test]
    async fn test_preserves_existing_metadata() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.9, 0.1],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(2);

        let mut doc1 = Document::new("Doc A");
        doc1.metadata
            .insert("source".to_string(), serde_json::json!("web"));
        doc1.metadata
            .insert("page".to_string(), serde_json::json!(42));

        let mut doc2 = Document::new("Doc B");
        doc2.metadata
            .insert("source".to_string(), serde_json::json!("book"));

        let documents = vec![doc1, doc2];
        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        // Doc A should be first (higher score)
        assert_eq!(result[0].page_content, "Doc A");
        assert_eq!(
            result[0].metadata.get("source"),
            Some(&serde_json::json!("web"))
        );
        assert_eq!(
            result[0].metadata.get("page"),
            Some(&serde_json::json!(42))
        );
        assert!(result[0].metadata.contains_key("relevance_score"));

        // Doc B should be second
        assert_eq!(result[1].page_content, "Doc B");
        assert_eq!(
            result[1].metadata.get("source"),
            Some(&serde_json::json!("book"))
        );
    }

    #[tokio::test]
    async fn test_overwrites_existing_relevance_score() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.75],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(1);

        let mut doc = Document::new("Doc A");
        doc.metadata
            .insert("relevance_score".to_string(), serde_json::json!(0.25));

        let documents = vec![doc];
        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        // Should have the new score, not the old one
        let score = result[0].metadata.get("relevance_score").unwrap();
        assert!((score.as_f64().unwrap() - 0.75).abs() < 1e-6);
    }

    // ============================================================
    // TOP_N EDGE CASES
    // ============================================================

    #[tokio::test]
    async fn test_top_n_zero_returns_empty() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.9, 0.8, 0.7],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(0);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_top_n_one() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.3, 0.9, 0.5],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(1);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "Doc B"); // Highest score 0.9
    }

    // ============================================================
    // QUERY-BASED SCORING TESTS
    // ============================================================

    #[tokio::test]
    async fn test_query_match_scoring() {
        let model = Box::new(QueryMatchEncoder);
        let reranker = CrossEncoderRerank::new(model).with_top_n(3);

        let documents = vec![
            Document::new("This document is about cats and dogs"),
            Document::new("This document mentions rust programming"),
            Document::new("Rust is a systems programming language about safety"),
        ];

        let result = reranker
            .compress_documents(documents, "rust programming", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 3);
        // Documents with "rust" and "programming" should score higher
        // Doc 3 has both words, Doc 2 has both, Doc 1 has neither
        assert!(result[0].page_content.contains("rust") || result[0].page_content.contains("Rust"));
    }

    // ============================================================
    // MANY DOCUMENTS TESTS
    // ============================================================

    #[tokio::test]
    async fn test_many_documents() {
        let scores: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let model = Box::new(FixedScoreEncoder {
            scores: scores.clone(),
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(10);

        let documents: Vec<Document> = (0..100)
            .map(|i| Document::new(format!("Document {i}")))
            .collect();

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 10);
        // Highest score is 0.99 (index 99)
        assert_eq!(result[0].page_content, "Document 99");
        assert_eq!(result[9].page_content, "Document 90");
    }

    #[tokio::test]
    async fn test_documents_with_ids() {
        let model = Box::new(FixedScoreEncoder {
            scores: vec![0.3, 0.9, 0.1],
        });
        let reranker = CrossEncoderRerank::new(model).with_top_n(3);

        let documents = vec![
            Document::new("Doc A").with_id("id-a"),
            Document::new("Doc B").with_id("id-b"),
            Document::new("Doc C").with_id("id-c"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 3);
        // IDs should be preserved and ordered by score
        assert_eq!(result[0].id, Some("id-b".to_string()));
        assert_eq!(result[1].id, Some("id-a".to_string()));
        assert_eq!(result[2].id, Some("id-c".to_string()));
    }
}
