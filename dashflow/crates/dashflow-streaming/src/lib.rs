// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Streaming - Ultra-Efficient Streaming Telemetry

// M-2142: Unit tests assert on known float constants (ratios, thresholds).
#![cfg_attr(test, allow(clippy::float_cmp))]

//! # DashFlow Streaming
//!
//! Ultra-efficient streaming telemetry for DashFlow using Protocol Buffers.
//!
//! ## Features
//!
//! - **Protobuf Encoding**: Compact binary serialization (<100μs encoding target)
//! - **Compression**: Zstd/LZ4 support (5:1 compression ratio target)
//! - **Zero-Copy**: Efficient serialization with minimal allocations
//! - **Type-Safe**: Full Rust type system integration
//!
//! ## Message Types
//!
//! - **Event**: Lifecycle events (graph start/end, node execution, etc.)
//! - **`StateDiff`**: Incremental state updates using JSON Patch
//! - **`TokenChunk`**: LLM streaming tokens
//! - **`ToolExecution`**: Tool call tracking
//! - **Checkpoint**: Full state snapshots
//! - **Metrics**: Performance metrics
//! - **Error**: Error tracking and reporting
//! - **ExecutionTrace**: Complete execution traces for self-improvement
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_streaming::{DashStreamMessage, Event, Header, EventType, MessageType};
//! use dashflow_streaming::codec::encode_message;
//!
//! // Create an event
//! let event = Event {
//!     header: Some(Header {
//!         message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
//!         timestamp_us: chrono::Utc::now().timestamp_micros(),
//!         tenant_id: "my-tenant".to_string(),
//!         thread_id: "session-123".to_string(),
//!         sequence: 1,
//!         r#type: MessageType::Event as i32,
//!         parent_id: vec![],
//!         compression: 0,
//!         schema_version: 1,
//!     }),
//!     event_type: EventType::GraphStart as i32,
//!     node_id: "".to_string(),
//!     attributes: Default::default(),
//!     duration_us: 0,
//!     llm_request_id: "".to_string(),
//! };
//!
//! // Encode to bytes
//! let bytes = encode_message(&DashStreamMessage {
//!     message: Some(dashflow_streaming::dash_stream_message::Message::Event(event)),
//! }).unwrap();
//! ```

/// Current DashFlow Streaming protobuf schema version
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

// ============================================================================
// Kafka Configuration Constants (M-243: Replace magic numbers with constants)
// ============================================================================
//
// These constants provide sensible defaults for Kafka configuration.
// Each value is documented with its rationale and tradeoffs.
//
// To customize: Use the corresponding struct field instead of relying on Default.

/// Default DLQ (Dead Letter Queue) timeout in seconds.
/// 5 seconds balances fast failure detection with retry tolerance for transient issues.
/// Applies to both consumer DLQ sends and producer DLQ sends.
pub const DEFAULT_DLQ_TIMEOUT_SECS: u64 = 5;

/// Default DLQ topic name.
/// Convention: original topic name with "-dlq" suffix.
pub const DEFAULT_DLQ_TOPIC: &str = "dashstream-dlq";

/// Streaming backends and transport integrations.
pub mod backends;
/// Encode/decode utilities for DashStream messages.
pub mod codec;
/// Compression helpers (e.g., zstd) for message payloads.
pub mod compression;
/// Kafka consumer implementation and configuration.
pub mod consumer;
/// State diff utilities (JSON Patch) for incremental updates.
pub mod diff;
/// Dead-letter queue (DLQ) support for failed messages.
pub mod dlq;
/// Streaming-specific error types and conversions.
pub mod errors;
/// Centralized environment variables and typed accessors.
pub mod env_vars;
/// Evaluation scaffolding for quality and regressions.
pub mod evals;
/// Kafka utilities and provisioning helpers.
pub mod kafka;
/// Constants used by streaming metrics and monitoring.
pub mod metrics_constants;
/// Metrics monitoring and health sampling logic.
pub mod metrics_monitor;
/// Internal metrics helpers.
pub(crate) mod metrics_utils;
/// Kafka producer implementation and configuration.
pub mod producer;
/// Message quality types and scoring.
pub mod quality;
/// Quality gate enforcement for streaming messages.
pub mod quality_gate;
/// Rate limiting utilities for high-throughput pipelines.
pub mod rate_limiter;
/// Test utilities and fixtures for streaming components.
pub mod testing;
/// Execution trace types for introspection/self-improvement.
pub mod trace;

// Include the generated protobuf code.
// Note: This defines types including a protobuf `Error` message type.
// Our custom error types are in the `errors` module to avoid naming conflicts.
#[allow(missing_docs)]
mod proto {
    include!(concat!(env!("OUT_DIR"), "/dashstream.v1.rs"));
}

/// Re-export generated protobuf types at the crate root.
pub use proto::*;

/// Re-export the protobuf `error` module with a clearer name.
pub use proto::error as proto_error;

// Re-export commonly used Kafka types and functions for convenience (M-413, M-410, M-478, M-618)
pub use kafka::{
    // Types
    KafkaSecurityConfig, TopicConfig,
    // Constants
    METADATA_SESSION_TIMEOUT_MS, VALID_BROKER_ADDRESS_FAMILIES, VALID_SASL_MECHANISMS,
    VALID_SECURITY_PROTOCOLS,
    // Config functions (M-410: Topic Provisioning)
    dev_config, dlq_config, recommended_config,
    // Topic provisioning functions (M-410)
    ensure_topic_exists, ensure_topics_with_dlq,
    // Address family helper (M-478: IPv6 support)
    get_broker_address_family,
};

// Re-export consumer configuration constants (M-243)
pub use consumer::{
    DEFAULT_AUTO_COMMIT_INTERVAL_MS, DEFAULT_FETCH_BACKOFF_INITIAL_MS,
    DEFAULT_FETCH_BACKOFF_MAX_SECS, DEFAULT_IDLE_POLL_SLEEP_MS, DEFAULT_MAX_MESSAGE_SIZE,
    DEFAULT_SESSION_TIMEOUT_MS,
};

// Re-export producer configuration constants (M-243)
pub use producer::{DEFAULT_MAX_MESSAGE_SIZE as PRODUCER_DEFAULT_MAX_MESSAGE_SIZE, DEFAULT_PRODUCER_TIMEOUT_SECS};
