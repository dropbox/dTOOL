//! # `DashFlow` HNSW Vector Store
//!
//! This crate provides a [`HNSWVectorStore`] implementation for `DashFlow`,
//! using the [hnsw_rs](https://crates.io/crates/hnsw_rs) library for fast
//! approximate nearest neighbor search.
//!
//! ## Features
//!
//! - **High Performance**: HNSW (Hierarchical Navigable Small World) algorithm
//!   provides excellent speed/accuracy tradeoff for ANN search
//! - **In-Memory**: Fast local vector search without external dependencies
//! - **Multiple Distance Metrics**: Cosine, L2, L1, Dot Product, and more
//! - **Persistence**: Save and load indexes from disk
//! - **Multithreaded**: Parallel insertion and search operations
//! - **Configurable**: Tune performance with `ef_construction` and M parameters
//!
//! ## Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! dashflow-hnsw = "0.1.0"
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::core::embeddings::MockEmbeddings;
//! use dashflow::core::vector_stores::VectorStore;
//! use dashflow_hnsw::{HNSWVectorStore, HNSWConfig, DistanceMetric};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create embeddings model
//!     let embeddings = MockEmbeddings::new(384); // 384-dimensional embeddings
//!
//!     // Configure HNSW parameters
//!     let config = HNSWConfig {
//!         dimension: 384,
//!         max_elements: 10000,
//!         m: 16,                    // Number of connections per layer
//!         ef_construction: 200,     // Controls construction quality
//!         distance_metric: DistanceMetric::Cosine,
//!     };
//!
//!     // Create vector store
//!     let mut store = HNSWVectorStore::new(embeddings, config)?;
//!
//!     // Add documents
//!     let texts = vec![
//!         "Rust is a systems programming language",
//!         "Python is great for data science",
//!         "JavaScript runs in the browser",
//!     ];
//!     store.add_texts(texts, None).await?;
//!
//!     // Search for similar documents
//!     let results = store._similarity_search("programming languages", 2).await?;
//!     for doc in results {
//!         println!("{}", doc.page_content);
//!     }
//!
//!     // Save index to disk
//!     store.save("index.hnsw")?;
//!
//!     // Load index from disk
//!     let loaded_store = HNSWVectorStore::load("index.hnsw", embeddings, config)?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## HNSW Parameters
//!
//! ### M (`max_nb_connection`)
//! - Number of bidirectional links per element in the graph
//! - Higher M = better recall but more memory
//! - Typical range: 12-48
//! - Default: 16
//!
//! ### `ef_construction`
//! - Controls search quality during index construction
//! - Higher `ef_construction` = better quality but slower insertion
//! - Should be >= M and typically 100-200
//! - Default: 200
//!
//! ### `ef_search`
//! - Controls search quality during queries
//! - Higher `ef_search` = better recall but slower search
//! - Can be tuned per-query for speed/accuracy tradeoff
//! - Default: 200
//!
//! ## Distance Metrics
//!
//! Supported distance metrics:
//! - **Cosine**: Cosine similarity (default, good for normalized vectors)
//! - **L2**: Euclidean distance (good for geometric data)
//! - **L1**: Manhattan distance
//! - **`DotProduct`**: Inner product (good for semantic similarity)
//!
//! ## Performance Characteristics
//!
//! - **Insertion**: O(log N) with high constants
//! - **Search**: O(log N) approximate
//! - **Memory**: O(N * M * dimension)
//! - **Thread Safety**: Safe for concurrent reads, write operations require &mut
//!
//! ## Comparison with Other Vector Stores
//!
//! | Store | Speed | Accuracy | Memory | Persistence |
//! |-------|-------|----------|--------|-------------|
//! | HNSW  | Fast  | High     | Medium | Yes         |
//! | Annoy | Fast  | Medium   | Low    | Yes         |
//! | `USearch` | Fastest | High  | Medium | Yes         |
//! | Faiss | Medium | High    | Low    | Yes         |
//!
//! ## References
//!
//! - [Original HNSW Paper](https://arxiv.org/abs/1603.09320) by Malkov & Yashunin
//! - [hnsw_rs Documentation](https://docs.rs/hnsw_rs)

pub mod hnsw_store;

pub use hnsw_store::{DistanceMetric, HNSWConfig, HNSWVectorStore};
