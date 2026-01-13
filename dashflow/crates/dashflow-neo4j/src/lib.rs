//! # `DashFlow` Neo4j Vector Store
//!
//! Neo4j vector store integration for `DashFlow`, enabling vector similarity search
//! with graph database capabilities.
//!
//! ## Features
//!
//! - **Graph Database + Vector Search**: Combines Neo4j's graph capabilities with vector similarity search
//! - **Native Vector Indexes**: Uses Neo4j's built-in vector index support (Neo4j 5.11+)
//! - **Multiple Distance Metrics**: Cosine similarity and Euclidean distance
//! - **Flexible Search**: Vector similarity search, hybrid search capabilities
//! - **Metadata Support**: Store and filter by document metadata as graph properties
//! - **Transaction Support**: Full ACID transaction support via Neo4j
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_neo4j::Neo4jVector;
//! use dashflow::core::embeddings::MockEmbeddings;
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create embeddings model
//! let embeddings = Arc::new(MockEmbeddings::new(384));
//!
//! // Connect to Neo4j
//! let mut store = Neo4jVector::new(
//!     "bolt://localhost:7687",
//!     "neo4j",
//!     "password",
//!     embeddings,
//! ).await?;
//!
//! // Add texts
//! let ids = store.add_texts(&["First document", "Second document"], None, None).await?;
//!
//! // Search
//! let results = store._similarity_search("query", 5, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Prerequisites
//!
//! - Neo4j 5.11+ (for vector index support)
//! - Database with vector index capability enabled
//!
//! ## Distance Metrics
//!
//! - **Cosine Similarity**: Default, measures angle between vectors (0-1, higher is more similar)
//! - **Euclidean Distance**: Measures L2 distance between vectors (0-∞, lower is more similar)

mod graph_store;
mod neo4j_graph;
mod neo4j_vector;

pub use graph_store::{
    format_structured_schema, GraphStore, PropertyDefinition, SchemaRelationship, StructuredSchema,
};
pub use neo4j_graph::Neo4jGraph;
pub use neo4j_vector::{DistanceStrategy, Neo4jVector, Neo4jVectorConfig};
