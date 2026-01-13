//! HTTP API Gateway for DashFlow Package Registry
//!
//! This module provides the REST API for the package registry, including:
//! - Package operations (publish, get, resolve, yank)
//! - Search operations (semantic, keyword, capability)
//! - Contribution operations (bug reports, improvements, fixes, reviews)
//! - Trust operations (verify, keys, lineage)
//! - Batch operations (bulk resolve, bulk download)
//!
//! # Architecture
//!
//! The API is built on Axum with modular route handlers:
//! ```text
//! /api/v1/
//! ├── packages/      - Package CRUD operations
//! ├── search/        - Search endpoints
//! ├── contributions/ - Contribution submission and review
//! ├── trust/         - Signature verification and keys
//! ├── batch/         - Batch operations for AI optimization
//! └── colony/        - Colony P2P sync
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_registry::api::{ApiServer, ApiConfig};
//!
//! let config = ApiConfig::default();
//! let server = ApiServer::new(config).await?;
//! server.run().await?;
//! ```

#[cfg(feature = "server")]
pub mod middleware;
#[cfg(feature = "server")]
pub mod routes;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "server")]
pub mod state;
#[cfg(feature = "server")]
pub mod types;

#[cfg(feature = "server")]
pub use server::{ApiConfig, ApiServer};
#[cfg(feature = "server")]
pub use state::create_production_storage;
#[cfg(all(feature = "server", feature = "semantic-search"))]
pub use state::{create_production_search, SemanticSearchConfig};
#[cfg(feature = "server")]
pub use state::{AppState, SearchService, SearchServiceWrapper};
#[cfg(all(feature = "server", feature = "s3"))]
pub use state::{S3StorageConfig, S3StorageType};
#[cfg(feature = "server")]
pub use types::*;
