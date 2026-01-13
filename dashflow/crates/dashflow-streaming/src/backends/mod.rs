// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Streaming - Alternative Streaming Backends

//! # Alternative Streaming Backends
//!
//! This module provides alternative backends to Kafka for streaming telemetry,
//! reducing deployment complexity for development and simple deployments.
//!
//! ## Available Backends
//!
//! - [`InMemoryBackend`](crate::backends::InMemoryBackend) - For testing and development
//! - [`FileBackend`](crate::backends::FileBackend) - JSONL files for local development and debugging
//! - [`SqliteBackend`](crate::backends::SqliteBackend) - Lightweight persistence for simple deployments
//!
//! ## Example
//!
//! ```rust
//! use dashflow_streaming::backends::{InMemoryBackend, StreamBackend, StreamConsumer, StreamProducer};
//! use dashflow_streaming::{Event, Header, EventType, MessageType, DashStreamMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create in-memory backend for testing
//!     let backend = InMemoryBackend::new();
//!
//!     // Create a producer
//!     let producer = backend.create_producer("test-topic").await?;
//!
//!     // Send a message
//!     let event = Event {
//!         header: Some(Header {
//!             message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
//!             timestamp_us: chrono::Utc::now().timestamp_micros(),
//!             tenant_id: "test".to_string(),
//!             thread_id: "thread-1".to_string(),
//!             sequence: 1,
//!             r#type: MessageType::Event as i32,
//!             parent_id: vec![],
//!             compression: 0,
//!             schema_version: 1,
//!         }),
//!         event_type: EventType::GraphStart as i32,
//!         node_id: "start".to_string(),
//!         attributes: Default::default(),
//!         duration_us: 0,
//!         llm_request_id: "".to_string(),
//!     };
//!
//!     producer.send_event(event).await?;
//!
//!     // Create a consumer
//!     let mut consumer = backend.create_consumer("test-topic", "group-1").await?;
//!
//!     // Receive the message
//!     if let Some(msg) = consumer.next().await {
//!         println!("Received: {:?}", msg);
//!     }
//!
//!     Ok(())
//! }
//! ```

mod file;
mod memory;
mod sqlite;
mod traits;

pub use file::FileBackend;
pub use memory::InMemoryBackend;
pub use sqlite::SqliteBackend;
pub use traits::{BackendError, BackendResult, StreamBackend, StreamConsumer, StreamProducer};
