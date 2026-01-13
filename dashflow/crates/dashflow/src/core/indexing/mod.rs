//! @dashflow-module
//! @name indexing
//! @category core
//! @status stable
//!
//! Document indexing system for managing vector store data
//!
//! This module provides infrastructure for intelligent document indexing into vector stores.
//! It tracks which documents have been indexed, detects changes, and handles deduplication
//! to prevent redundant API calls and storage waste.
//!
//! # Key Features
//!
//! - **Change Detection**: Only re-index documents that have changed
//! - **Deduplication**: Automatically detect and skip duplicate documents
//! - **Incremental Updates**: Efficiently update large document sets
//! - **Cleanup Modes**: Remove outdated documents with configurable strategies
//! - **Multiple Hash Algorithms**: SHA-1, SHA-256, SHA-512, `BLAKE2b` support
//!
//! # Architecture
//!
//! The indexing system consists of three main components:
//!
//! 1. **`RecordManager`**: Tracks document metadata (hash, timestamp, `source_id`)
//! 2. **`DocumentIndex`**: Abstraction for vector stores supporting upsert/delete
//! 3. **`index()` function**: Orchestrates the indexing workflow
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use dashflow::core::indexing::{InMemoryRecordManager, index, CleanupMode, HashAlgorithm};
//! use dashflow::core::documents::Document;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a record manager to track indexed documents
//! let record_manager = InMemoryRecordManager::new("my_namespace");
//!
//! // Documents to index
//! let docs = vec![
//!     Document::new("First document content"),
//!     Document::new("Second document content"),
//! ];
//!
//! // Index with incremental cleanup
//! // let result = index(
//! //     docs,
//! //     record_manager,
//! //     vector_store,
//! //     CleanupMode::Incremental,
//! //     HashAlgorithm::Sha256,
//! // ).await?;
//! //
//! // println!("Added: {}, Updated: {}, Unchanged: {}, Deleted: {}",
//! //     result.num_added, result.num_updated, result.num_skipped, result.num_deleted);
//! # Ok(())
//! # }
//! ```
//!
//! # Cleanup Modes
//!
//! - **None**: No cleanup, documents are only added/updated
//! - **Incremental**: Delete outdated docs with same `source_id` during indexing
//! - **Full**: Delete all docs not returned by loader (use carefully!)
//! - **`ScopedFull`**: Like Full, but only for `source_ids` seen in this run
//!
//! # Hash Algorithms
//!
//! Document hashes uniquely identify content for change detection:
//!
//! - **SHA-1**: Fast but collision-vulnerable (legacy, not recommended)
//! - **SHA-256**: Good balance of security and performance (recommended)
//! - **SHA-512**: Maximum security, slower
//! - **`BLAKE2b`**: Fast and secure, modern choice

pub mod api;
pub mod document_index;
pub mod hashing;
pub mod record_manager;

pub use api::{index, CleanupMode, IndexingError, IndexingResult};
pub use document_index::{DeleteResponse, DocumentIndex, UpsertResponse};
pub use hashing::{deduplicate_documents, hash_document, HashAlgorithm};
pub use record_manager::{InMemoryRecordManager, Record, RecordManager};
