//! Supabase vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the Supabase vector store for `DashFlow`.
//! Supabase uses `PostgreSQL` with the pgvector extension, so this implementation wraps
//! the pgvector store with Supabase-specific connection logic.
//!
//! # Prerequisites
//!
//! You need a Supabase project with the pgvector extension enabled. Visit
//! [supabase.com](https://supabase.com) to create a project.
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_supabase::SupabaseVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Connect to Supabase
//! let mut store = SupabaseVectorStore::new(
//!     "postgresql://postgres.[PROJECT_ID].supabase.co:5432/postgres",
//!     "[YOUR_PASSWORD]",
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
//! cargo run --package dashflow-supabase --example supabase_basic
//! ```
//!
//! See `examples/supabase_basic.rs` for the full source code.
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-pgvector`](https://docs.rs/dashflow-pgvector) - Alternative: Direct PostgreSQL pgvector integration
//! - [`dashflow-mongodb`](https://docs.rs/dashflow-mongodb) - Alternative: MongoDB Atlas Vector Search
//! - [Supabase Vector Documentation](https://supabase.com/docs/guides/ai/vector-columns) - Official docs

mod supabase_store;

pub use supabase_store::SupabaseVectorStore;
