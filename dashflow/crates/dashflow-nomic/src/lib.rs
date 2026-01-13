//! Nomic AI integration for `DashFlow` Rust
//!
//! This crate provides Nomic AI implementations for `DashFlow` Rust.
//!
//! Nomic provides high-quality text embeddings optimized for semantic search.
//!
//! # Features
//! - `NomicEmbeddings`: Embedding models using Nomic AI's API
//! - Task-specific embeddings (`search_document`, `search_query`, classification, clustering)
//! - Matryoshka embedding support (configurable dimensions)
//!
//! # Available Models
//! - nomic-embed-text-v1.5 (768 dimensions, default)
//! - nomic-embed-text-v1 (768 dimensions)
//!
//! # Embeddings Example
//! ```no_run
//! use dashflow_nomic::NomicEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! #[tokio::main]
//! async fn main() {
//!     let embedder = NomicEmbeddings::new()
//!         .with_model("nomic-embed-text-v1.5");
//!
//!     let embedding = embedder.embed_query("Hello, world!").await.unwrap();
//!     println!("Embedding dimension: {}", embedding.len());
//! }
//! ```

pub mod embeddings;

pub use embeddings::NomicEmbeddings;
