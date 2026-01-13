//! `PostgreSQL` pgvector integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the pgvector vector store for `DashFlow`.
//! pgvector is a `PostgreSQL` extension for vector similarity search, providing efficient
//! storage and retrieval of embeddings using `PostgreSQL`'s robust ACID properties.
//!
//! # Prerequisites
//!
//! You need `PostgreSQL` with the pgvector extension installed. The easiest way is with Docker:
//!
//! ```bash
//! docker run --name postgres-pgvector -e POSTGRES_PASSWORD=postgres \
//!   -p 5432:5432 -d pgvector/pgvector:pg16
//! ```
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_pgvector::PgVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Connect to PostgreSQL with pgvector
//! let mut store = PgVectorStore::new(
//!     "postgresql://postgres:postgres@localhost:5432/postgres",
//!     "my_collection",
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
//! cargo run --package dashflow-pgvector --example pgvector_basic
//! ```
//!
//! See `examples/pgvector_basic.rs` for the full source code.
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-supabase`](https://docs.rs/dashflow-supabase) - Alternative: Supabase (PostgreSQL-based with pgvector)
//! - [`dashflow-mongodb`](https://docs.rs/dashflow-mongodb) - Alternative: MongoDB Atlas Vector Search
//! - [pgvector Documentation](https://github.com/pgvector/pgvector) - Official pgvector docs

mod pgvector_store;

pub use pgvector_store::PgVectorStore;
