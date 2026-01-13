//! Redis vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of Redis as a vector store using
//! Redis Stack (Redis + `RediSearch` module). Redis Stack enables efficient
//! vector similarity search with metadata filtering.
//!
//! # Features
//!
//! - **Vector Search**: Cosine, L2, and Inner Product distance metrics
//! - **Index Algorithms**: FLAT (exact KNN) and HNSW (approximate nearest neighbor)
//! - **Metadata Filtering**: Tag, numeric, and text filters with complex expressions
//! - **CRUD Operations**: Add, search, delete, and retrieve vectors
//! - **Batching**: Efficient batch operations for large datasets
//! - **Persistence**: All data persisted in Redis
//!
//! # Requirements
//!
//! This crate requires Redis Stack 6.2+ or Redis with `RediSearch` 2.6+ module installed.
//!
//! **Docker Setup:**
//! ```bash
//! docker run -d -p 6379:6379 redis/redis-stack:latest
//! ```
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```rust,ignore
//! use dashflow_redis::RedisVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create a Redis vector store
//! let mut store = RedisVectorStore::new(
//!     "redis://localhost:6379",
//!     "my_index",
//!     embeddings,
//!     None, // Use default schema
//!     None, // Use default vector config
//! ).await?;
//!
//! // Add documents
//! let texts = vec!["Hello world", "Goodbye world"];
//! let ids = store.add_texts(&texts, None, None).await?;
//!
//! // Search
//! let results = store._similarity_search("Hello", 2, None).await?;
//! for doc in results {
//!     println!("{}", doc.page_content);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## With Metadata and Filtering
//!
//! ```rust,ignore
//! use dashflow_redis::{RedisVectorStore, filters::*};
//! use std::collections::HashMap;
//! # use std::sync::Arc;
//! # use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! # use dashflow::core::vector_stores::VectorStore;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! # let mut store = RedisVectorStore::new("redis://localhost:6379", "idx", embeddings, None, None).await?;
//! // Add documents with metadata
//! let texts = vec!["Product A", "Product B"];
//! let mut meta1 = HashMap::new();
//! meta1.insert("category".to_string(), serde_json::json!("electronics"));
//! meta1.insert("price".to_string(), serde_json::json!(99.99));
//!
//! let mut meta2 = HashMap::new();
//! meta2.insert("category".to_string(), serde_json::json!("books"));
//! meta2.insert("price".to_string(), serde_json::json!(19.99));
//!
//! let metadatas = vec![meta1, meta2];
//! store.add_texts(&texts, Some(&metadatas), None).await?;
//!
//! // Search with filter
//! let filter = TagFilter::new("category").eq("electronics")
//!     .and(NumFilter::new("price").lt(150.0));
//!
//! let results = store.similarity_search_with_filter(
//!     "product",
//!     5,
//!     Some(&filter)
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## HNSW Index for Large Datasets
//!
//! ```rust,ignore
//! use dashflow_redis::{RedisVectorStore, schema::*};
//! # use std::sync::Arc;
//! # use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Configure HNSW algorithm for approximate nearest neighbor search
//! let vector_config = VectorFieldConfig::hnsw()
//!     .dims(384)
//!     .distance_metric(DistanceMetric::Cosine)
//!     .m(16)                    // Neighbors per layer
//!     .ef_construction(200)     // Construction time quality
//!     .ef_runtime(10)           // Search time quality
//!     .build();
//!
//! let mut store = RedisVectorStore::new(
//!     "redis://localhost:6379",
//!     "large_index",
//!     embeddings,
//!     None,
//!     Some(vector_config),
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-redis-checkpointer`](https://docs.rs/dashflow-redis-checkpointer) - Redis-based state checkpointing for graphs
//! - [`dashflow-pgvector`](https://docs.rs/dashflow-pgvector) - Alternative: PostgreSQL with pgvector extension
//! - [Redis Stack Documentation](https://redis.io/docs/stack/) - Official Redis Stack docs

pub mod constants;
pub mod filters;
pub mod schema;
pub mod utils;
pub mod vector_store;

pub use vector_store::RedisVectorStore;

// Re-export commonly used types

// Filter types
pub use filters::{FilterExpression, FilterOperator, NumFilter, TagFilter, TextFilter};

// Schema types
pub use schema::{
    DistanceMetric, FlatVectorField, HNSWVectorField, NumericFieldSchema, RedisIndexSchema,
    TagFieldSchema, TextFieldSchema, VectorDataType, VectorField,
};
