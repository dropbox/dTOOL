//! Typesense search engine integration for `DashFlow` Rust.
//!
//! This crate provides a `TypesenseVectorStore` implementation that integrates
//! with Typesense, an open-source search engine optimized for developer experience
//! and fast searches.
//!
//! # Features
//!
//! - Full `VectorStore` trait implementation
//! - Support for vector similarity search (cosine similarity)
//! - Metadata filtering capabilities
//! - Hybrid search (combining keyword and vector search)
//! - Async/await API
//! - Auto-embedding with Typesense ML models (optional)
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_typesense::TypesenseVectorStore;
//! use dashflow::core::embeddings::Embeddings;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # struct MockEmbeddings;
//! # #[async_trait::async_trait]
//! # impl Embeddings for MockEmbeddings {
//! #     async fn _embed_documents(&self, texts: &[String]) -> dashflow::core::Result<Vec<Vec<f32>>> {
//! #         Ok(vec![vec![0.0; 384]; texts.len()])
//! #     }
//! #     async fn _embed_query(&self, text: &str) -> dashflow::core::Result<Vec<f32>> {
//! #         Ok(vec![0.0; 384])
//! #     }
//! # }
//! let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
//!
//! let mut store = TypesenseVectorStore::new(
//!     "http://localhost:8108",
//!     "my_api_key",
//!     "documents",
//!     embeddings,
//!     384, // embedding dimension
//!     "text", // text field name
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Typesense Setup
//!
//! Before using this crate, you need to have Typesense running. The easiest way is with Docker:
//!
//! ```bash
//! docker run -d \
//!   -p 8108:8108 \
//!   -v /tmp/typesense-data:/data \
//!   -e TYPESENSE_DATA_DIR=/data \
//!   -e TYPESENSE_API_KEY=xyz \
//!   typesense/typesense:27.0
//! ```
//!
//! # Collection Schema
//!
//! Typesense requires a collection schema to be defined before storing data. This implementation
//! will automatically create a collection if it doesn't exist, with the following fields:
//!
//! - `id`: The document ID (string)
//! - `text`: The document content (string)
//! - `embedding`: The vector embedding (float array with `num_dim`)
//! - `metadata`: The document metadata (object)
//!
//! The collection uses HNSW indexing for fast approximate nearest neighbor search.
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-elasticsearch`](https://docs.rs/dashflow-elasticsearch) - Alternative: Elasticsearch for full-text + vector search
//! - [`dashflow-opensearch`](https://docs.rs/dashflow-opensearch) - Alternative: OpenSearch with k-NN plugin
//! - [Typesense Documentation](https://typesense.org/docs/) - Official Typesense docs

pub mod typesense_store;

pub use typesense_store::TypesenseVectorStore;
