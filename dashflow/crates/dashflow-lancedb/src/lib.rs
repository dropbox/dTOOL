//! # `LanceDB` Vector Store for `DashFlow` Rust
//!
//! This crate provides a `LanceDB` integration for `DashFlow` Rust, enabling efficient
//! vector storage and similarity search using `LanceDB`'s columnar format.
//!
//! ## Features
//!
//! - Fast vector similarity search using `LanceDB`
//! - 100x faster random access than Parquet
//! - Zero-copy operations
//! - Automatic versioning
//! - Support for local storage and cloud (S3, GCS)
//! - Multi-modal data support
//!
//! ## Example
//!
//! **Note**: The example uses a mock embeddings implementation for demonstration.
//! For production use, replace with a real provider like `dashflow_openai::OpenAIEmbeddings`.
//!
//! ```rust,no_run
//! use dashflow_lancedb::LanceDBVectorStore;
//! use dashflow::core::embeddings::Embeddings;
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # // MockEmbeddings is for demonstration only - not for production use!
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
//! // Create vector store
//! let mut store = LanceDBVectorStore::new(
//!     "data/lancedb",  // Local path
//!     "documents",      // Table name
//!     embeddings,
//! ).await?;
//!
//! // Add documents
//! let texts = vec!["Hello world", "LanceDB is fast"];
//! let ids = store.add_texts(&texts, None, None).await?;
//!
//! // Search
//! let results = store._similarity_search("greeting", 1, None).await?;
//! println!("Found: {}", results[0].page_content);
//! # Ok(())
//! # }
//! ```
//!
//! ## Cloud Storage
//!
//! `LanceDB` supports cloud storage out of the box:
//!
//! ```rust,no_run
//! # use dashflow_lancedb::LanceDBVectorStore;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # use std::sync::Arc;
//! # use dashflow::core::embeddings::MockEmbeddings;
//! # let embeddings: Arc<dyn dashflow::core::embeddings::Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // S3
//! let store = LanceDBVectorStore::new(
//!     "s3://my-bucket/lancedb",
//!     "documents",
//!     embeddings.clone(),
//! ).await?;
//!
//! // GCS
//! let store = LanceDBVectorStore::new(
//!     "gs://my-bucket/lancedb",
//!     "documents",
//!     embeddings,
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-faiss`](https://docs.rs/dashflow-faiss) - Alternative: FAISS for local high-performance search
//! - [`dashflow-sqlitevss`](https://docs.rs/dashflow-sqlitevss) - Alternative: SQLite-based local vector storage
//! - [LanceDB Documentation](https://lancedb.github.io/lancedb/) - Official LanceDB docs

mod lancedb_store;

pub use lancedb_store::LanceDBVectorStore;
