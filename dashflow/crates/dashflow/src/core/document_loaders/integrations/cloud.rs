//! Cloud storage integration loaders.
//!
//! This module provides document loaders for various cloud storage services.
//!
//! ## Placeholder Status
//!
//! **Note:** All cloud storage loaders are placeholders awaiting implementation.
//! They are not exported from the public API.
//! Real implementations require cloud provider SDK integrations.
//!
//! Previously listed services (removed as unimplemented placeholders):
//! - AWS: S3
//! - Azure: Blob Storage
//! - Google Cloud: Cloud Storage (GCS)
//! - Personal cloud: Dropbox, Google Drive, OneDrive
//!
//! To implement a cloud storage loader:
//! 1. Add required cloud SDK dependency to Cargo.toml
//! 2. Create struct with auth and path configuration
//! 3. Implement `DocumentLoader` trait with file download
//! 4. Add integration tests with mocked SDK responses
//! 5. Export from `mod.rs`

// No loaders implemented yet.
// This file exists to document the cloud storage integration category.
// Add implementations here as cloud SDKs are integrated.
//
// When adding implementations, import:
// - use crate::core::document_loaders::base::DocumentLoader;
// - use crate::core::documents::Document;
// - use crate::core::{Error, Result};
// - use async_trait::async_trait;
