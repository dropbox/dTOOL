//! `SQLite` VSS (Vector Similarity Search) integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of `SQLite` with vector similarity search
//! using the sqlite-vss extension. `SQLite` VSS enables efficient approximate nearest
//! neighbor search directly in `SQLite` databases.
//!
//! # Features
//!
//! - **Embedded Database**: No separate server required, perfect for local/edge applications
//! - **Persistent Storage**: Vectors stored directly in `SQLite` database files
//! - **Vector Search Extension**: Uses sqlite-vss for efficient similarity search
//! - **Multiple Distance Metrics**: Cosine, L2 (Euclidean), Inner Product
//! - **Metadata Support**: Store and filter by metadata alongside vectors
//! - **ACID Transactions**: Full `SQLite` transaction support
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_sqlitevss::SQLiteVSSStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create a SQLite VSS store with 384 dimensions
//! let mut store = SQLiteVSSStore::new(embeddings, ":memory:", 384, None)?;
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
//! ## With File-based Database
//!
//! ```ignore
//! use dashflow_sqlitevss::SQLiteVSSStore;
//! # use std::sync::Arc;
//! # use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create store with file-based database
//! let mut store = SQLiteVSSStore::new(
//!     embeddings,
//!     "vectors.db",
//!     384,
//!     None
//! )?;
//!
//! // Use the store...
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-lancedb`](https://docs.rs/dashflow-lancedb) - Alternative: LanceDB for embedded vector storage
//! - [`dashflow-faiss`](https://docs.rs/dashflow-faiss) - Alternative: FAISS for local high-performance search
//! - [sqlite-vss Documentation](https://github.com/asg017/sqlite-vss) - Official sqlite-vss docs

mod sqlitevss_store;

pub use sqlitevss_store::SQLiteVSSStore;
