//! Annoy vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the Annoy-inspired vector store for `DashFlow`.
//! It uses the `arroy` library, which is a Rust implementation inspired by Spotify's Annoy
//! (Approximate Nearest Neighbors Oh Yeah) library.
//!
//! # Features
//!
//! - **Approximate Nearest Neighbors**: Fast ANN search using tree-based indexing
//! - **Multiple Distance Metrics**: Euclidean, Manhattan, Cosine, and Dot Product
//! - **Persistent Storage**: LMDB-backed storage for durability
//! - **Memory Efficient**: Optimized for low memory usage
//! - **Cross-Process Sharing**: LMDB enables sharing indexes across processes
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_annoy::AnnoyVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create an Annoy vector store with 384 dimensions
//! let mut store = AnnoyVectorStore::new(embeddings, 384, None, None)?;
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
//! ## With Persistence
//!
//! ```ignore
//! use dashflow_annoy::AnnoyVectorStore;
//! use std::path::Path;
//! # use std::sync::Arc;
//! # use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create with custom path
//! let db_path = Path::new("/tmp/my_annoy_db");
//! let mut store = AnnoyVectorStore::new_with_path(
//!     embeddings.clone(),
//!     384,
//!     None,    // distance_metric (default: Euclidean)
//!     db_path,
//!     None,    // n_trees
//!     None,    // map_size_bytes (default: 10GB)
//! )?;
//!
//! // Add documents and they'll be persisted
//! // ... add documents ...
//! # Ok(())
//! # }
//! ```
//!
//! ## Complete Example
//!
//! For a comprehensive example demonstrating all features including metadata filtering,
//! persistence, and various search modes, run:
//!
//! ```bash
//! cargo run --package dashflow-annoy --example annoy_basic
//! ```

mod store;

pub use store::AnnoyVectorStore;
