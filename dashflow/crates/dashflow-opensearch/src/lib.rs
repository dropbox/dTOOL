//! # `OpenSearch` Vector Store for `DashFlow` Rust
//!
//! This crate provides an `OpenSearch` vector store implementation for `DashFlow` Rust.
//!
//! `OpenSearch` is an open-source, distributed search and analytics suite derived from
//! Elasticsearch 7.10. It provides powerful vector search capabilities through its k-NN
//! (k-Nearest Neighbors) plugin, supporting various ANN algorithms including HNSW, IVF,
//! and more.
//!
//! ## Features
//!
//! - **Vector Similarity Search**: Efficient k-NN search using `OpenSearch`'s k-NN plugin
//! - **Multiple Distance Metrics**: Cosine similarity, L2 (Euclidean), and inner product
//! - **Metadata Filtering**: Filter search results by document metadata
//! - **Bulk Operations**: Efficient batch document insertion
//! - **Automatic Index Management**: Index creation with proper k-NN mappings
//! - **HNSW Algorithm**: Uses Hierarchical Navigable Small World for fast ANN search
//!
//! ## Prerequisites
//!
//! You need an `OpenSearch` instance with the k-NN plugin enabled. The k-NN plugin is
//! typically included by default in `OpenSearch` distributions.
//!
//! ## Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! dashflow-opensearch = "0.1"
//! dashflow::core = "0.1"
//! ```
//!
//! ## Quick Start
//!
//! ```no_run
//! use std::sync::Arc;
//! use dashflow_opensearch::OpenSearchVectorStore;
//! use dashflow::core::embeddings::MockEmbeddings;
//! use dashflow::core::vector_stores::VectorStore;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create embeddings model
//!     let embeddings = Arc::new(MockEmbeddings::new(1536));
//!
//!     // Connect to OpenSearch and create vector store
//!     let mut store = OpenSearchVectorStore::new(
//!         "my_vectors",              // index name
//!         embeddings,                // embeddings model
//!         "https://localhost:9200",  // OpenSearch URL
//!     ).await?;
//!
//!     // Add documents
//!     let texts = vec![
//!         "OpenSearch is a distributed search engine",
//!         "It supports vector similarity search with k-NN",
//!         "HNSW provides fast approximate nearest neighbor search"
//!     ];
//!     let ids = store.add_texts(&texts, None, None).await?;
//!     println!("Added documents with IDs: {:?}", ids);
//!
//!     // Search for similar documents
//!     let results = store._similarity_search("vector search", 2, None).await?;
//!     for doc in results {
//!         println!("Found: {}", doc.page_content);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Authentication
//!
//! For AWS `OpenSearch` Service with IAM authentication, use the AWS SDK:
//!
//! ```text
//! Use opensearch crate's AWS auth features
//! See: https://docs.rs/opensearch/latest/opensearch/
//! ```
//!
//! For basic authentication:
//!
//! ```text
//! https://username:password@localhost:9200
//! ```
//!
//! ## Index Configuration
//!
//! The vector store automatically creates an index with these settings:
//!
//! - **k-NN enabled**: Index-level setting for vector search
//! - **HNSW algorithm**: Fast approximate nearest neighbor search
//! - **`ef_construction`**: 128 (build-time search quality)
//! - **m**: 24 (number of bi-directional links per element)
//! - **Vector dimension**: 1536 (default for `OpenAI` embeddings)
//!
//! ## Distance Metrics
//!
//! Supported distance metrics:
//!
//! - **Cosine Similarity** (default): Measures angle between vectors
//! - **L2 (Euclidean)**: Straight-line distance between vectors
//! - **Inner Product**: Dot product of vectors
//!
//! ## Performance Considerations
//!
//! - **Batch Inserts**: Use bulk operations for better throughput
//! - **HNSW Parameters**: Adjust `ef_construction` and `m` for speed vs accuracy tradeoff
//! - **Refresh Strategy**: Index refresh is called after bulk operations for immediate searchability
//! - **Vector Dimension**: Higher dimensions require more memory and slower search
//!
//! ## Examples
//!
//! See the `examples/` directory for complete examples:
//!
//! - `opensearch_basic.rs`: Basic usage with local `OpenSearch`
//! - `opensearch_aws.rs`: AWS `OpenSearch` Service with IAM authentication
//! - `opensearch_filtering.rs`: Metadata filtering examples
//!
//! ## Differences from Elasticsearch
//!
//! `OpenSearch` uses different terminology and configuration for vector search:
//!
//! | Elasticsearch | `OpenSearch` |
//! |---------------|------------|
//! | `dense_vector` type | `knn_vector` type |
//! | Native k-NN | k-NN plugin required |
//! | `similarity` parameter | `space_type` in method config |
//!
//! ## Resources
//!
//! - [OpenSearch Documentation](https://opensearch.org/docs/)
//! - [k-NN Plugin Guide](https://opensearch.org/docs/latest/search-plugins/knn/)
//! - [HNSW Algorithm](https://arxiv.org/abs/1603.09320)
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-elasticsearch`](https://docs.rs/dashflow-elasticsearch) - Alternative: Elasticsearch (OpenSearch fork origin)
//! - [`dashflow-typesense`](https://docs.rs/dashflow-typesense) - Alternative: Typesense full-text + vector search
//! - [OpenSearch k-NN Documentation](https://opensearch.org/docs/latest/search-plugins/knn/) - Official docs

pub mod bm25_retriever;
pub mod opensearch_store;

pub use bm25_retriever::OpenSearchBM25Retriever;
pub use opensearch_store::{OpenSearchVectorStore, VectorStoreRetriever};
