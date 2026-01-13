//! Database integration loaders.
//!
//! This module provides document loaders for various database systems.
//!
//! ## Placeholder Status
//!
//! **Note:** All database loaders are placeholders awaiting implementation.
//! They are not exported from the public API.
//! Real implementations require database client libraries.
//!
//! Previously listed databases (removed as unimplemented placeholders):
//! - SQL databases: PostgreSQL, MySQL
//! - NoSQL databases: MongoDB, Redis, Cassandra, Neo4j
//! - Data warehouses: BigQuery, Snowflake, Rockset
//! - Search engines: Elasticsearch
//!
//! To implement a database loader:
//! 1. Add required database client dependency to Cargo.toml
//! 2. Create struct with connection configuration
//! 3. Implement `DocumentLoader` trait with query execution
//! 4. Add integration tests with test containers or mocked responses
//! 5. Export from `mod.rs`

// No loaders implemented yet.
// This file exists to document the database integration category.
// Add implementations here as database clients are integrated.
//
// When adding implementations, import:
// - use crate::core::document_loaders::base::DocumentLoader;
// - use crate::core::documents::Document;
// - use crate::core::{Error, Result};
// - use async_trait::async_trait;
