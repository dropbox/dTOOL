//! Pinecone vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the Pinecone vector store for `DashFlow`.
//! Pinecone is a managed, cloud-native vector database designed for high-performance
//! similarity search at scale.
//!
//! # Prerequisites
//!
//! You need a Pinecone account and API key. Sign up at <https://www.pinecone.io/>
//!
//! Set your API key as an environment variable:
//!
//! ```bash
//! export PINECONE_API_KEY="your-api-key-here"
//! ```
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_pinecone::PineconeVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create a Pinecone vector store
//! let mut store = PineconeVectorStore::new(
//!     "your-index-name",
//!     embeddings,
//!     None, // Uses PINECONE_API_KEY env var
//!     None, // Optional namespace
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
//! namespaces, and CRUD operations, run:
//!
//! ```bash
//! cargo run --package dashflow-pinecone --example pinecone_basic
//! ```
//!
//! See `examples/pinecone_basic.rs` for the full source code.
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-weaviate`](https://docs.rs/dashflow-weaviate) - Alternative: Weaviate cloud-native vector database
//! - [`dashflow-qdrant`](https://docs.rs/dashflow-qdrant) - Alternative: Qdrant self-hosted vector database
//! - [Pinecone Documentation](https://docs.pinecone.io/) - Official Pinecone docs

mod pinecone;

pub use pinecone::PineconeVectorStore;
