//! `ClickHouse` vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the `ClickHouse` vector store for `DashFlow`.
//! `ClickHouse` is a fast open-source column-oriented database management system that supports
//! vector similarity search with HNSW (Hierarchical Navigable Small World) indexes.
//!
//! # Prerequisites
//!
//! You need `ClickHouse` server running. The easiest way is with Docker:
//!
//! ```bash
//! docker run -d --name clickhouse-server \
//!   -p 8123:8123 -p 9000:9000 \
//!   --ulimit nofile=262144:262144 \
//!   clickhouse/clickhouse-server
//! ```
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_clickhouse::ClickHouseVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Connect to ClickHouse
//! let mut store = ClickHouseVectorStore::new(
//!     "http://localhost:8123",
//!     "default",  // database
//!     "my_vectors",  // table name
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
//! ## Complete Example
//!
//! For a comprehensive example demonstrating all features including metadata filtering
//! and CRUD operations, run:
//!
//! ```bash
//! cargo run --package dashflow-clickhouse --example clickhouse_basic
//! ```
//!
//! See `examples/clickhouse_basic.rs` for the full source code.

mod clickhouse_store;

pub use clickhouse_store::ClickHouseVectorStore;
