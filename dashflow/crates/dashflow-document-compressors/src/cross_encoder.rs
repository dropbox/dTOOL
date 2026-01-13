//! Cross-encoder models for document reranking
//!
//! This module defines the trait for cross-encoder models that can score
//! pairs of text for relevance. Cross-encoders are typically used for reranking
//! documents in RAG pipelines.
//!
//! # Implementation Notes
//!
//! Concrete implementations require either:
//! - An ONNX runtime to run local cross-encoder models (e.g., ms-marco-MiniLM-L-6-v2)
//! - An API client to call a cross-encoder service
//! - Integration with sentence-transformers via a C++ binding
//!
//! # Example Implementation
//!
//! ```rust,ignore
//! use dashflow_document_compressors::CrossEncoder;
//! use dashflow::core::errors::Result;
//! use async_trait::async_trait;
//!
//! struct MyCrossEncoder {
//!     // ... fields for model/API client
//! }
//!
//! #[async_trait]
//! impl CrossEncoder for MyCrossEncoder {
//!     async fn score(&self, text_pairs: Vec<(String, String)>) -> Result<Vec<f32>> {
//!         // Score each (query, document) pair for relevance
//!         todo!("Implement scoring logic")
//!     }
//! }
//! ```

use async_trait::async_trait;
use dashflow::core::error::Result;

/// Interface for cross-encoder models.
///
/// Cross-encoders take pairs of text and score their relevance/similarity.
/// They are more accurate than bi-encoders but slower, as they cannot
/// precompute embeddings.
///
/// Typical use case: reranking documents retrieved by a bi-encoder (embeddings).
#[async_trait]
pub trait CrossEncoder: Send + Sync {
    /// Score pairs of texts for relevance/similarity.
    ///
    /// # Arguments
    ///
    /// * `text_pairs` - List of (query, document) pairs to score
    ///
    /// # Returns
    ///
    /// A vector of scores, one per input pair. Higher scores indicate
    /// greater relevance. Score range depends on the model.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pairs = vec![
    ///     ("What is Rust?".to_string(), "Rust is a programming language".to_string()),
    ///     ("What is Rust?".to_string(), "Iron oxide is rust".to_string()),
    /// ];
    /// let scores = encoder.score(pairs).await?;
    /// // scores might be [0.95, 0.3] indicating first doc is more relevant
    /// ```
    async fn score(&self, text_pairs: Vec<(String, String)>) -> Result<Vec<f32>>;
}
