//! `USearch` vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the `USearch` vector store for `DashFlow`.
//! `USearch` is a high-performance SIMD-accelerated library for Approximate Nearest Neighbor (ANN)
//! search in high-dimensional spaces, developed by Unum Cloud.
//!
//! # Features
//!
//! - **High Performance**: SIMD-accelerated distance calculations
//! - **Multiple Distance Metrics**: Cosine, L2, Inner Product, and more
//! - **In-Memory**: Fast, zero-latency vector search
//! - **Persistence**: Save and load indexes from disk
//! - **Flexible Quantization**: Support for f32, f16, i8, and binary vectors
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_usearch::USearchVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create a USearch vector store with 384 dimensions
//! let mut store = USearchVectorStore::new(embeddings, 384, None)?;
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
//! use dashflow_usearch::USearchVectorStore;
//! # use std::sync::Arc;
//! # use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create and populate store
//! let mut store = USearchVectorStore::new(embeddings.clone(), 384, None)?;
//! // ... add documents ...
//!
//! // Save to disk
//! store.save("my_index.usearch")?;
//!
//! // Load from disk later
//! let loaded_store = USearchVectorStore::load("my_index.usearch", embeddings)?;
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
//! cargo run --package dashflow-usearch --example usearch_basic
//! ```
//!
//! See `examples/usearch_basic.rs` for the full source code.
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-faiss`](https://docs.rs/dashflow-faiss) - Alternative: FAISS for local high-performance search
//! - [`dashflow-lancedb`](https://docs.rs/dashflow-lancedb) - Alternative: LanceDB for embedded vector storage
//! - [USearch GitHub](https://github.com/unum-cloud/usearch) - Official USearch repository

mod usearch_store;

pub use usearch_store::USearchVectorStore;
