//! `TimescaleVector` (pgvectorscale) integration for `DashFlow` Rust.
//!
//! This crate provides a vector store implementation backed by `TimescaleDB`'s
//! [pgvectorscale](https://github.com/timescale/pgvectorscale) extension, which
//! enhances `PostgreSQL`'s pgvector with:
//!
//! - **`StreamingDiskANN`**: High-performance approximate nearest neighbor search
//! - **Statistical Binary Quantization (SBQ)**: Cost-efficient vector storage
//! - **Label-based Filtering**: Combine vector similarity with label filtering
//!
//! ## Performance Benefits
//!
//! Compared to standard pgvector:
//! - 28x lower p95 latency
//! - 16x higher query throughput
//! - 75% lower cost (vs cloud providers like Pinecone)
//!
//! ## Prerequisites
//!
//! You need `PostgreSQL` with both pgvector and pgvectorscale extensions installed:
//!
//! ```sql
//! CREATE EXTENSION IF NOT EXISTS vector;
//! CREATE EXTENSION IF NOT EXISTS vectorscale CASCADE;
//! ```
//!
//! ## Installation
//!
//! For installation instructions, see:
//! - [pgvectorscale GitHub](https://github.com/timescale/pgvectorscale)
//! - [Timescale Cloud](https://www.timescale.com/) (managed service with extensions pre-installed)
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use dashflow_timescale::TimescaleVectorStore;
//! use dashflow::core::embeddings::Embeddings;
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create embeddings (use your actual embeddings implementation)
//! # struct MockEmbeddings;
//! # #[async_trait::async_trait]
//! # impl Embeddings for MockEmbeddings {
//! #     async fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, dashflow::core::Error> {
//! #         Ok(texts.iter().map(|_| vec![0.0; 1536]).collect())
//! #     }
//! #     async fn embed_query(&self, text: &str) -> Result<Vec<f32>, dashflow::core::Error> {
//! #         Ok(vec![0.0; 1536])
//! #     }
//! # }
//! let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
//!
//! // Connect to TimescaleDB
//! let mut store = TimescaleVectorStore::new(
//!     "postgresql://user:pass@localhost:5432/vectordb",
//!     "my_documents",
//!     embeddings,
//! ).await?;
//!
//! // Add documents
//! store.add_texts(&["Hello world", "Rust is great"], None, None).await?;
//!
//! // Search with DiskANN
//! let results = store._similarity_search("programming", 5, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Index Types
//!
//! pgvectorscale supports multiple index types:
//! - **diskann** (default): `StreamingDiskANN` for high throughput
//! - **hnsw**: Hierarchical Navigable Small World (from pgvector)
//! - **ivfflat**: Inverted file index (from pgvector)
//!
//! The default implementation uses `diskann` for optimal performance.

mod timescale_store;

pub use timescale_store::TimescaleVectorStore;
