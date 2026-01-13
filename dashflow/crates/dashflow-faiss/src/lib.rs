//! DashFlow.
//!
//! This crate provides a `FaissVectorStore` implementation that integrates
//! with Facebook AI Similarity Search (FAISS), a library for efficient
//! similarity search and clustering of dense vectors.
//!
//! # Features
//!
//! - Full VectorStore trait implementation
//! - Support for multiple distance metrics (Cosine, Euclidean, Inner Product)
//! - Flexible index types (Flat, IVF, HNSW, etc.)
//! - Metadata filtering capabilities
//! - Maximum Marginal Relevance (MMR) search
//! - Async/await API
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_faiss::FaissVectorStore;
//! use dashflow::core::embeddings::Embeddings;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
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
//! // Create a FAISS vector store with a flat index
//! let mut store = FaissVectorStore::new(
//!     embeddings,
//!     384, // embedding dimension
//!     "Flat", // exact search
//! ).await?;
//!
//! // Add documents
//! let texts = vec!["Hello world", "FAISS is fast"];
//! store.add_texts(&texts, None, None).await?;
//!
//! // Search
//! let results = store._similarity_search("greeting", 1, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # FAISS Setup
//!
//! Before using this crate, you need to have the FAISS C library installed:
//!
//! ## macOS
//! ```bash
//! brew install faiss
//! ```
//!
//! ## Ubuntu/Debian
//! ```bash
//! apt-get install libfaiss-dev
//! ```
//!
//! ## From Source
//! Follow the instructions at: https://github.com/facebookresearch/faiss/blob/main/INSTALL.md
//!
//! # Index Types
//!
//! FAISS supports various index types for different use cases:
//!
//! - **"Flat"**: Exact search (exhaustive). Best for small datasets (<10K vectors).
//! - **"IVFx,Flat"**: Inverted file index. Good balance of speed and accuracy.
//!   - Example: "IVF100,Flat" creates 100 clusters
//! - **"HNSWx"**: Hierarchical Navigable Small World graph. Very fast approximate search.
//!   - Example: "HNSW32" creates graph with 32 neighbors
//! - **"IVFPQ"**: Product quantization for compressed storage.
//!
//! See [FAISS documentation](https://github.com/facebookresearch/faiss/wiki) for more details.
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-usearch`](https://docs.rs/dashflow-usearch) - Alternative: USearch for fast local vector search
//! - [`dashflow-lancedb`](https://docs.rs/dashflow-lancedb) - Alternative: LanceDB for embedded vector storage
//! - [FAISS GitHub](https://github.com/facebookresearch/faiss) - Official FAISS repository

pub mod faiss_store;

pub use faiss_store::FaissVectorStore;
