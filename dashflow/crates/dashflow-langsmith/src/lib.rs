//! # `LangSmith` Rust SDK
//!
//! This crate provides a Rust client for `LangSmith`, enabling tracing, debugging,
//! and monitoring of language models and AI agents.
//!
//! ## Features
//!
//! - **Tracing**: Track execution of LLM chains and agents
//! - **Run Management**: Create, update, and query runs
//! - **Batch Ingestion**: Efficiently submit multiple runs
//! - **Authentication**: Support for API key authentication
//!
//! ## Example
//!
//! ```no_run
//! use dashflow_langsmith::Client;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::builder()
//!         .api_key("your-api-key")
//!         .build()?;
//!
//!     // Use the client to create runs and traces
//!     Ok(())
//! }
//! ```

mod batch_queue;
mod client;
mod error;
mod run;

pub use batch_queue::{BatchQueue, DEFAULT_BATCH_SIZE, DEFAULT_FLUSH_INTERVAL};
pub use client::{Client, ClientBuilder};
pub use error::{Error, Result};
pub use run::{Run, RunCreate, RunType, RunUpdate};
