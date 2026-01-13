// Clippy allows for test helper crate
// All functions in this crate are test helpers designed to be used in #[cfg(test)]
// contexts. unwrap/expect/panic are acceptable for test assertions.
#![allow(clippy::unwrap_used)] // Test assertions need unwrap
#![allow(clippy::expect_used)] // Test assertions need expect
#![allow(clippy::panic)] // Test helpers may panic on setup failures
#![allow(clippy::unwrap_in_result)] // Test setup functions return Result
#![allow(clippy::clone_on_ref_ptr)] // Arc::clone() pattern is intentional
#![allow(clippy::redundant_clone)] // Clone for test isolation is acceptable
#![allow(clippy::needless_pass_by_value)] // Test helper APIs prioritize ergonomics
#![allow(clippy::float_cmp)] // Test assertions may compare floats directly

//! # `DashFlow` Standard Tests
//!
//! This crate provides standard conformance tests for `DashFlow` Rust providers.
//! It ensures that all `ChatModel`, Embeddings, and `VectorStore` implementations
//! behave consistently across different providers.
//!
//! ## Overview
//!
//! Python `DashFlow` has a `dashflow-standard-tests` package that defines
//! conformance tests all providers must pass. This crate ports that concept
//! to Rust using macros for zero-overhead test generation.
//!
//! ## Usage
//!
//! In your provider crate (e.g., `dashflow-openai`), add standard tests:
//!
//! ```rust,ignore
//! #[cfg(test)]
//! mod standard_tests {
//!     use dashflow_standard_tests::chat_model_standard_tests;
//!     use crate::ChatOpenAI;
//!
//!     chat_model_standard_tests!(ChatOpenAI, openai_standard);
//! }
//! ```
//!
//! This generates ~10 standard conformance tests that verify your
//! `ChatModel` implementation behaves correctly.

pub mod base_store_tests;
pub mod cache_tests;
pub mod chat_model_tests;
pub mod embeddings_tests;
pub mod indexer_tests;
pub mod retriever_advanced_tests;
pub mod retriever_tests;
pub mod tool_comprehensive_tests;
pub mod tool_tests;
pub mod vectorstore_tests;

// Re-export test functions
pub use base_store_tests::*;
pub use cache_tests::*;
pub use chat_model_tests::*;
pub use embeddings_tests::*;
pub use indexer_tests::*;
pub use retriever_advanced_tests::*;
pub use retriever_tests::*;
pub use tool_comprehensive_tests::*;
pub use tool_tests::*;
pub use vectorstore_tests::*;
