//! Elasticsearch integration for `DashFlow` Rust.
//!
//! This crate provides Rust implementations of Elasticsearch integrations for `DashFlow`:
//!
//! - **[`ElasticsearchVectorStore`]**: Dense vector search using kNN
//! - **[`ElasticsearchBM25Retriever`]**: Full-text search using BM25 scoring
//! - **[`ElasticsearchDatabaseChain`]**: SQL-like queries over Elasticsearch
//!
//! # Prerequisites
//!
//! You need a running Elasticsearch instance (version 8.0+). The easiest way is with Docker:
//!
//! ```bash
//! docker run -p 9200:9200 -e "discovery.type=single-node" \
//!   -e "xpack.security.enabled=false" docker.elastic.co/elasticsearch/elasticsearch:8.11.0
//! ```
//!
//! # Vector Search vs BM25
//!
//! | Feature | `ElasticsearchVectorStore` | `ElasticsearchBM25Retriever` |
//! |---------|---------------------------|------------------------------|
//! | Algorithm | kNN (dense vectors) | BM25 (term frequency) |
//! | Requires Embeddings | Yes | No |
//! | Best For | Semantic similarity | Keyword matching |
//! | Index Type | `dense_vector` | `text` |
//!
//! # Examples
//!
//! ## Vector Search (Semantic)
//!
//! ```ignore
//! use dashflow_elasticsearch::ElasticsearchVectorStore;
//! use dashflow::core::embeddings::Embeddings;
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = todo!();
//! let mut store = ElasticsearchVectorStore::new(
//!     "my_index",
//!     embeddings,
//!     "http://localhost:9200",
//! ).await?;
//!
//! store.add_texts(&["Hello world", "Goodbye world"], None, None).await?;
//! let results = store._similarity_search("Hello", 2, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## BM25 Search (Keyword)
//!
//! ```ignore
//! use dashflow_elasticsearch::ElasticsearchBM25Retriever;
//! use dashflow::core::retrievers::Retriever;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut retriever = ElasticsearchBM25Retriever::new(
//!     "documents",
//!     "http://localhost:9200",
//! ).await?;
//!
//! retriever.add_texts(&["The quick brown fox", "A lazy dog"]).await?;
//! let results = retriever._get_relevant_documents("quick fox", None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - Vector store trait
//! - [`Retriever`](dashflow::core::retrievers::Retriever) - Retriever trait
//! - [`dashflow-opensearch`](https://docs.rs/dashflow-opensearch) - Alternative: OpenSearch (Elasticsearch fork)
//! - [`dashflow-typesense`](https://docs.rs/dashflow-typesense) - Alternative: Typesense full-text + vector search
//! - [Elasticsearch Vector Search](https://www.elastic.co/guide/en/elasticsearch/reference/current/dense-vector.html) - Official docs

mod bm25_retriever;
mod database_chain;
mod elasticsearch;

pub use bm25_retriever::ElasticsearchBM25Retriever;
pub use database_chain::{
    ElasticsearchDatabaseChain, ElasticsearchDatabaseChainConfig, ElasticsearchDatabaseChainOutput,
};
pub use elasticsearch::ElasticsearchVectorStore;
