//! Qdrant vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the Qdrant vector store for `DashFlow`.
//! Qdrant is a high-performance vector search engine with support for dense vectors,
//! sparse vectors, and hybrid search combining both.
//!
//! # Prerequisites
//!
//! You need a running Qdrant server. The easiest way is with Docker:
//!
//! ```bash
//! docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant
//! ```
//!
//! Qdrant uses two ports:
//! - Port 6333: HTTP/REST API
//! - Port 6334: gRPC API (used by this client)
//!
//! # Features
//!
//! - **Dense vector search**: Traditional embedding-based similarity search
//! - **Sparse vector search**: BM25-style keyword-based retrieval (planned)
//! - **Hybrid search**: Combines dense and sparse vectors for best results (planned)
//! - **Advanced filtering**: Rich metadata filtering with nested conditions
//! - **Multiple distance metrics**: Cosine, Euclidean, Dot product, Manhattan
//! - **Maximum Marginal Relevance**: Diversity-aware search results
//!
//! # Current Status
//!
//! - ✅ Dense vector search (fully implemented)
//! - ⏳ Sparse vector search (planned)
//! - ⏳ Hybrid search (planned)
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create a Qdrant vector store
//! let mut store = QdrantVectorStore::new(
//!     "http://localhost:6334",
//!     "my_collection",
//!     embeddings,
//!     RetrievalMode::Dense,
//! ).await?;
//!
//! // Add documents
//! let texts = ["Hello world", "Goodbye world"];
//! let ids = store.add_texts(&texts, None, None).await?;
//!
//! // Search
//! let results = store._similarity_search("Hello", 2, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Complete Example
//!
//! For a comprehensive example demonstrating all features including metadata filtering,
//! MMR search, and CRUD operations, run:
//!
//! ```bash
//! cargo run --package dashflow-qdrant --example qdrant_basic
//! ```
//!
//! See `examples/qdrant_basic.rs` for the full source code.
//!
//! # Implementation Roadmap
//!
//! See `reports/make_everything_rust/qdrant_design_2025-10-28-09-23.md` for detailed
//! design documentation and implementation plan.
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-milvus`](https://docs.rs/dashflow-milvus) - Alternative: Milvus cloud-native vector database
//! - [`dashflow-chroma`](https://docs.rs/dashflow-chroma) - Alternative: Chroma open-source embedding database
//! - [Qdrant Documentation](https://qdrant.tech/documentation/) - Official Qdrant docs

// Public API
mod config_ext;
mod qdrant;
mod retrieval_mode;

pub use config_ext::{build_vector_store, build_vector_store_with_mode};
pub use qdrant::QdrantVectorStore;
pub use retrieval_mode::RetrievalMode;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qdrant_store_exists() {
        // Verify QdrantVectorStore is properly exported
        let type_name = std::any::type_name::<QdrantVectorStore>();
        assert!(type_name.contains("QdrantVectorStore"));
    }

    #[test]
    fn retrieval_mode_variants() {
        // Verify RetrievalMode enum is properly exported
        let _dense = RetrievalMode::Dense;
        let _sparse = RetrievalMode::Sparse;
        let _hybrid = RetrievalMode::Hybrid;
    }
}
