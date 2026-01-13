//! Milvus vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the Milvus vector store for `DashFlow`.
//! Milvus is a cloud-native vector database designed for AI applications, supporting
//! various distance metrics and indexing algorithms for efficient similarity search.
//!
//! # Prerequisites
//!
//! You need a running Milvus instance. The easiest way is with Docker:
//!
//! ```bash
//! # Start Milvus Standalone
//! docker run -d --name milvus-standalone \
//!   -p 19530:19530 -p 9091:9091 \
//!   milvusdb/milvus:latest
//! ```
//!
//! # Features
//!
//! - Full `VectorStore` trait implementation
//! - Multiple index types: IVF_FLAT, IVF_SQ8, IVF_PQ, HNSW, and more
//! - Multiple distance metrics: L2, IP (Inner Product), Cosine
//! - Automatic collection and index management
//! - Metadata filtering support
//! - Horizontal scalability for large datasets
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_milvus::MilvusVectorStore;
//! use dashflow::core::embeddings::Embeddings;
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # struct MockEmbeddings;
//! # #[async_trait::async_trait]
//! # impl Embeddings for MockEmbeddings {
//! #     async fn embed_documents(&self, texts: &[String]) -> dashflow::core::Result<Vec<Vec<f32>>> {
//! #         Ok(vec![vec![0.0; 384]; texts.len()])
//! #     }
//! #     async fn embed_query(&self, text: &str) -> dashflow::core::Result<Vec<f32>> {
//! #         Ok(vec![0.0; 384])
//! #     }
//! # }
//! let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
//!
//! // Create a Milvus vector store
//! let mut store = MilvusVectorStore::new(
//!     "http://localhost:19530",  // Milvus endpoint
//!     "my_collection",           // Collection name
//!     embeddings,
//!     384,                       // Embedding dimension
//! ).await?;
//!
//! // Add documents
//! let texts = vec!["Hello world", "Milvus is fast"];
//! let ids = store.add_texts(&texts, None, None).await?;
//!
//! // Search for similar documents
//! let results = store._similarity_search("greeting", 2, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Index Types
//!
//! Milvus supports various index types:
//!
//! - **IVF_FLAT**: Inverted file index with flat storage (good accuracy)
//! - **IVF_SQ8**: IVF with scalar quantization (balanced)
//! - **IVF_PQ**: IVF with product quantization (memory efficient)
//! - **HNSW**: Hierarchical Navigable Small World (fast search)
//! - **ANNOY**: Approximate Nearest Neighbors Oh Yeah (Spotify's algorithm)
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-qdrant`](https://docs.rs/dashflow-qdrant) - Alternative: Qdrant vector database
//! - [`dashflow-weaviate`](https://docs.rs/dashflow-weaviate) - Alternative: Weaviate semantic search
//! - [Milvus Documentation](https://milvus.io/docs) - Official Milvus docs

mod milvus_store;

pub use milvus_store::MilvusVectorStore;
