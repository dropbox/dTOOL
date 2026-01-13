//! Voyage AI integration for DashFlow
//!
//! This crate provides integration with Voyage AI's text embedding and reranking models.
//! Voyage AI offers state-of-the-art models optimized for various use cases including
//! retrieval, code, finance, and legal domains.
//!
//! # Features
//!
//! - Text embeddings with voyage-3.5, voyage-3-large, and specialized models
//! - Document reranking with rerank-2.5 and rerank-2.5-lite
//! - Support for query vs document input types
//! - Configurable output dimensions
//! - Batch processing up to 1000 texts
//!
//! # Examples
//!
//! ## Embeddings
//!
//! ```rust,ignore
//! use dashflow_voyage::VoyageEmbeddings;
//! use dashflow::embed_query;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = Arc::new(VoyageEmbeddings::new()
//!     .with_api_key(std::env::var("VOYAGE_API_KEY")?));
//!
//! let vector = embed_query(embedder, "What is machine learning?").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Reranking
//!
//! ```no_run
//! use dashflow_voyage::VoyageRerank;
//! use dashflow::core::documents::{Document, DocumentCompressor};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let reranker = VoyageRerank::new()
//!     .with_api_key(std::env::var("VOYAGE_API_KEY")?)
//!     .with_top_k(Some(3));
//!
//! let documents = vec![
//!     Document::new("Paris is the capital of France."),
//!     Document::new("Berlin is the capital of Germany."),
//!     Document::new("The sky is blue."),
//! ];
//!
//! let reranked = reranker
//!     .compress_documents(documents, "What is the capital of France?", None)
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Authentication
//!
//! The Voyage AI API requires an API key for authentication. Get your API key from:
//! <https://dash.voyageai.com/>
//!
//! Set it via environment variable:
//! ```bash
//! export VOYAGE_API_KEY="your-api-key"
//! ```
//!
//! Or pass it directly:
//! ```no_run
//! # use dashflow_voyage::VoyageEmbeddings;
//! let embedder = VoyageEmbeddings::new().with_api_key("your-api-key");
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::embeddings::Embeddings`] - The trait implemented by embedding models
//! - [`dashflow::core::documents::DocumentCompressor`] - The trait for reranking
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI embeddings alternative
//! - [`dashflow_cohere`](https://docs.rs/dashflow-cohere) - Cohere embeddings/reranking
//! - [Voyage AI Dashboard](https://dash.voyageai.com/) - API key management

/// Voyage AI API base URL
pub const VOYAGE_API_BASE: &str = "https://api.voyageai.com/v1";

mod embeddings;
pub mod rerank;

pub use embeddings::{InputType, VoyageEmbeddings};
pub use rerank::{RerankResult, VoyageRerank};

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== VOYAGE_API_BASE constant ====================

    #[test]
    fn test_voyage_api_base_constant() {
        assert_eq!(VOYAGE_API_BASE, "https://api.voyageai.com/v1");
    }

    #[test]
    fn test_voyage_api_base_is_https() {
        assert!(VOYAGE_API_BASE.starts_with("https://"));
    }

    #[test]
    fn test_voyage_api_base_contains_version() {
        assert!(VOYAGE_API_BASE.contains("/v1"));
    }

    // ==================== Public re-exports ====================

    #[test]
    fn test_voyage_embeddings_reexport() {
        // Verify the type is accessible from crate root
        let _embedder = VoyageEmbeddings::new();
    }

    #[test]
    fn test_voyage_rerank_reexport() {
        // Verify the type is accessible from crate root
        let _reranker = VoyageRerank::new();
    }

    #[test]
    fn test_input_type_reexport() {
        // Verify all variants are accessible
        let _ = InputType::None;
        let _ = InputType::Query;
        let _ = InputType::Document;
    }

    #[test]
    fn test_rerank_result_reexport() {
        // Verify the type is accessible via deserialization
        let json = r#"{"index": 0, "relevance_score": 0.5}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 0);
    }

    // ==================== Module structure ====================

    #[test]
    fn test_embeddings_module_public() {
        // Embeddings module re-exports are accessible
        // Builder pattern works through re-export
        let _embedder = VoyageEmbeddings::new()
            .with_model("voyage-3.5")
            .with_input_type(InputType::Query);
    }

    #[test]
    fn test_rerank_module_public() {
        // Rerank module re-exports are accessible
        // Builder pattern works through re-export
        let _reranker = VoyageRerank::new()
            .with_model("rerank-2.5")
            .with_top_k(Some(5));
    }

    #[test]
    fn test_default_traits_available() {
        // Default trait is implemented for both main types
        let _embedder = VoyageEmbeddings::default();
        let _reranker = VoyageRerank::default();
    }
}
