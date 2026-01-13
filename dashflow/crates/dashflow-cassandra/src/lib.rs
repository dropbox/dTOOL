//! # `DashFlow` Cassandra - Vector Store Implementation
//!
//! This crate provides an Apache Cassandra / `DataStax` Astra DB vector store implementation
//! for the `DashFlow` ecosystem.
//!
//! ## Features
//!
//! - **Native Vector Search**: Uses Cassandra 5.0+ / Astra DB vector search capabilities
//! - **VECTOR Type Support**: Stores embeddings using `VECTOR<FLOAT, N>` native type
//! - **ANN Search**: Approximate Nearest Neighbor search with `ORDER BY ANN OF`
//! - **Multiple Similarity Functions**: Cosine, Euclidean, Dot Product
//! - **Metadata Filtering**: Filter search results by document metadata
//! - **Distributed Architecture**: Leverages Cassandra's distributed database for horizontal scaling
//! - **Full `VectorStore` Trait**: Complete implementation of `DashFlow` `VectorStore` interface
//!
//! ## Requirements
//!
//! - Apache Cassandra 5.0+ (with vector search support) OR
//! - `DataStax` Astra DB (cloud-native Cassandra with built-in vector search)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_cassandra::CassandraVectorStore;
//! use dashflow::core::embeddings::Embeddings;
//! use dashflow::core::vector_stores::VectorStore;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create vector store instance
//! let store = CassandraVectorStore::builder()
//!     .contact_points(vec!["127.0.0.1:9042"])
//!     .keyspace("dashflow")
//!     .table("vector_store")
//!     .vector_dimension(1536)
//!     .build()
//!     .await?;
//!
//! // Add documents with embeddings
//! // store.add_texts(...).await?;
//!
//! // Perform similarity search
//! // let results = store._similarity_search("query", 5).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Cassandra Setup
//!
//! ### 1. Create Keyspace and Table
//!
//! ```cql
//! CREATE KEYSPACE IF NOT EXISTS dashflow
//! WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};
//!
//! CREATE TABLE IF NOT EXISTS dashflow.vector_store (
//!     id UUID PRIMARY KEY,
//!     content TEXT,
//!     metadata TEXT,
//!     vector VECTOR<FLOAT, 1536>
//! );
//! ```
//!
//! ### 2. Create Vector Index
//!
//! ```cql
//! CREATE INDEX IF NOT EXISTS idx_vector
//! ON dashflow.vector_store (vector)
//! WITH OPTIONS = {'similarity_function': 'cosine'};
//! ```
//!
//! ### 3. Similarity Functions
//!
//! - **cosine**: Cosine similarity (default, best for normalized vectors)
//! - **euclidean**: Euclidean distance (L2)
//! - **`dot_product`**: Dot product / Inner product
//!
//! ## Architecture Notes
//!
//! This implementation uses the `scylla` Rust driver, which is fully compatible with
//! Apache Cassandra. The driver provides:
//! - High-performance async I/O
//! - Automatic connection pooling
//! - Prepared statement caching
//! - Load balancing across cluster nodes

pub mod cassandra_store;

pub use cassandra_store::{CassandraVectorStore, CassandraVectorStoreBuilder, SimilarityFunction};
