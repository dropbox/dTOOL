//! Weaviate vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the Weaviate vector store for `DashFlow`.
//! Weaviate is an open-source vector database that supports semantic search, hybrid search,
//! and multi-modal data.
//!
//! # Prerequisites
//!
//! You need a running Weaviate server. The easiest way is with Docker:
//!
//! ```bash
//! docker run -p 8080:8080 semitechnologies/weaviate:latest
//! ```
//!
//! # Features
//!
//! - **Dense vector search**: Traditional embedding-based similarity search
//! - **Hybrid search**: Combines dense vectors with BM25 keyword search
//! - **GraphQL API**: Weaviate's native query interface
//! - **Schema management**: Automatic class/collection creation
//! - **Advanced filtering**: Rich metadata filtering with nested conditions
//! - **Multi-tenancy support**: Isolated data spaces per tenant
//!
//! # Current Status
//!
//! Priority: Weaviate integration
//!
//! - ✅ Crate scaffold and dependencies
//! - ✅ `WeaviateVectorStore` struct
//! - ✅ `VectorStore` trait implementation
//! - ✅ `DocumentIndex` trait implementation
//! - ✅ Example application and unit tests
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_weaviate::WeaviateVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create a Weaviate vector store
//! let mut store = WeaviateVectorStore::new(
//!     "http://localhost:8080",
//!     "MyDocuments",
//!     embeddings,
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
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-pinecone`](https://docs.rs/dashflow-pinecone) - Alternative: Pinecone cloud-native vector database
//! - [`dashflow-milvus`](https://docs.rs/dashflow-milvus) - Alternative: Milvus distributed vector database
//! - [Weaviate Documentation](https://weaviate.io/developers/weaviate) - Official Weaviate docs

// Public API
mod weaviate;

pub use weaviate::WeaviateVectorStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weaviate_store_exists() {
        // Verify WeaviateVectorStore is properly exported
        let type_name = std::any::type_name::<WeaviateVectorStore>();
        assert!(type_name.contains("WeaviateVectorStore"));
    }
}
