// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Execution Trace Streaming Support
//!
//! This module provides helpers for streaming execution traces for distributed
//! self-reflection. It enables AI agents to share their execution history
//! across distributed systems for collaborative analysis.
//!
//! ## Usage
//!
//! ### Creating an ExecutionTrace message
//!
//! ```rust,ignore
//! use dashflow_streaming::trace::{create_execution_trace_message, TraceBuilder};
//!
//! // Build an execution trace message
//! let msg = TraceBuilder::new()
//!     .execution_id("exec-123")
//!     .thread_id("thread-456")
//!     .total_duration_ms(1500)
//!     .total_tokens(500)
//!     .completed(true)
//!     .add_node_execution(NodeExecutionRecord {
//!         node: "search".to_string(),
//!         duration_ms: 1000,
//!         total_tokens: 300,
//!         succeeded: true,
//!         ..Default::default()
//!     })
//!     .build();
//! ```
//!
//! ### Extracting from a DashStreamMessage
//!
//! ```rust,ignore
//! use dashflow_streaming::trace::extract_execution_trace;
//!
//! if let Some(trace) = extract_execution_trace(&message) {
//!     println!("Execution completed in {}ms", trace.total_duration_ms);
//!     for node in &trace.nodes_executed {
//!         println!("  Node {}: {}ms", node.node, node.duration_ms);
//!     }
//! }
//! ```

use crate::{
    dash_stream_message::Message, DashStreamMessage, ErrorRecord, ExecutionTrace, Header,
    MessageType, NodeExecutionRecord, CURRENT_SCHEMA_VERSION,
};
use chrono::Utc;
use uuid::Uuid;

/// Builder for constructing ExecutionTrace messages.
#[derive(Debug, Default)]
pub struct TraceBuilder {
    thread_id: String,
    execution_id: String,
    nodes_executed: Vec<NodeExecutionRecord>,
    total_duration_ms: u64,
    total_tokens: u64,
    errors: Vec<ErrorRecord>,
    completed: bool,
    started_at: String,
    ended_at: String,
    final_state: Vec<u8>,
    metadata: std::collections::HashMap<String, Vec<u8>>,
    tenant_id: String,
}

impl TraceBuilder {
    /// Create a new trace builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the thread ID.
    #[must_use]
    pub fn thread_id(mut self, id: impl Into<String>) -> Self {
        self.thread_id = id.into();
        self
    }

    /// Set the execution ID.
    #[must_use]
    pub fn execution_id(mut self, id: impl Into<String>) -> Self {
        self.execution_id = id.into();
        self
    }

    /// Set the total execution duration in milliseconds.
    #[must_use]
    pub fn total_duration_ms(mut self, ms: u64) -> Self {
        self.total_duration_ms = ms;
        self
    }

    /// Set the total tokens used.
    #[must_use]
    pub fn total_tokens(mut self, tokens: u64) -> Self {
        self.total_tokens = tokens;
        self
    }

    /// Set whether execution completed successfully.
    #[must_use]
    pub fn completed(mut self, completed: bool) -> Self {
        self.completed = completed;
        self
    }

    /// Set the start timestamp (ISO 8601 format).
    #[must_use]
    pub fn started_at(mut self, timestamp: impl Into<String>) -> Self {
        self.started_at = timestamp.into();
        self
    }

    /// Set the end timestamp (ISO 8601 format).
    #[must_use]
    pub fn ended_at(mut self, timestamp: impl Into<String>) -> Self {
        self.ended_at = timestamp.into();
        self
    }

    /// Set the final state (JSON encoded as bytes).
    #[must_use]
    pub fn final_state(mut self, state: Vec<u8>) -> Self {
        self.final_state = state;
        self
    }

    /// Set the tenant ID for the message header.
    #[must_use]
    pub fn tenant_id(mut self, id: impl Into<String>) -> Self {
        self.tenant_id = id.into();
        self
    }

    /// Add a node execution record.
    #[must_use]
    pub fn add_node_execution(mut self, node: NodeExecutionRecord) -> Self {
        self.nodes_executed.push(node);
        self
    }

    /// Add multiple node execution records.
    #[must_use]
    pub fn add_node_executions(
        mut self,
        nodes: impl IntoIterator<Item = NodeExecutionRecord>,
    ) -> Self {
        self.nodes_executed.extend(nodes);
        self
    }

    /// Add an error record.
    #[must_use]
    pub fn add_error(mut self, error: ErrorRecord) -> Self {
        self.errors.push(error);
        self
    }

    /// Add metadata entry (value is JSON encoded as bytes).
    #[must_use]
    pub fn add_metadata(mut self, key: impl Into<String>, value: Vec<u8>) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Build the DashStreamMessage containing the ExecutionTrace.
    #[must_use]
    pub fn build(self) -> DashStreamMessage {
        let header = Header {
            message_id: Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: Utc::now().timestamp_micros(),
            tenant_id: self.tenant_id,
            thread_id: self.thread_id.clone(),
            sequence: 0, // Intentionally 0: execution traces are summary messages, not sequenced events
            r#type: MessageType::ExecutionTrace as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: CURRENT_SCHEMA_VERSION,
        };

        let trace = ExecutionTrace {
            header: Some(header),
            thread_id: self.thread_id,
            execution_id: self.execution_id,
            nodes_executed: self.nodes_executed,
            total_duration_ms: self.total_duration_ms,
            total_tokens: self.total_tokens,
            errors: self.errors,
            completed: self.completed,
            started_at: self.started_at,
            ended_at: self.ended_at,
            final_state: self.final_state,
            metadata: self.metadata,
        };

        DashStreamMessage {
            message: Some(Message::ExecutionTrace(trace)),
        }
    }
}

/// Create an ExecutionTrace message with the given parameters.
///
/// This is a convenience function for simple cases. Use [`TraceBuilder`] for more control.
#[must_use]
pub fn create_execution_trace_message(
    execution_id: impl Into<String>,
    thread_id: impl Into<String>,
    total_duration_ms: u64,
    total_tokens: u64,
    completed: bool,
) -> DashStreamMessage {
    TraceBuilder::new()
        .execution_id(execution_id)
        .thread_id(thread_id)
        .total_duration_ms(total_duration_ms)
        .total_tokens(total_tokens)
        .completed(completed)
        .build()
}

/// Extract an ExecutionTrace from a DashStreamMessage if present.
///
/// Returns `None` if the message is not an ExecutionTrace.
#[must_use]
pub fn extract_execution_trace(message: &DashStreamMessage) -> Option<&ExecutionTrace> {
    match &message.message {
        Some(Message::ExecutionTrace(trace)) => Some(trace),
        _ => None,
    }
}

/// Check if a message is an ExecutionTrace.
#[must_use]
pub fn is_execution_trace(message: &DashStreamMessage) -> bool {
    matches!(&message.message, Some(Message::ExecutionTrace(_)))
}

/// Create a NodeExecutionRecord with the given parameters.
#[must_use]
pub fn create_node_record(
    node: impl Into<String>,
    duration_ms: u64,
    total_tokens: u64,
    succeeded: bool,
) -> NodeExecutionRecord {
    NodeExecutionRecord {
        node: node.into(),
        duration_ms,
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens,
        succeeded,
        started_at: String::new(),
        ended_at: String::new(),
        input: vec![],
        output: vec![],
        metadata: std::collections::HashMap::new(),
    }
}

/// Create an ErrorRecord with the given parameters.
#[must_use]
pub fn create_error_record(
    node: impl Into<String>,
    message: impl Into<String>,
    recovered: bool,
) -> ErrorRecord {
    ErrorRecord {
        node: node.into(),
        message: message.into(),
        error_code: String::new(),
        timestamp: Utc::now().to_rfc3339(),
        recovered,
        stack_trace: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_builder_basic() {
        let msg = TraceBuilder::new()
            .execution_id("exec-123")
            .thread_id("thread-456")
            .total_duration_ms(1500)
            .total_tokens(500)
            .completed(true)
            .build();

        let trace = extract_execution_trace(&msg).expect("Should be ExecutionTrace");
        assert_eq!(trace.execution_id, "exec-123");
        assert_eq!(trace.thread_id, "thread-456");
        assert_eq!(trace.total_duration_ms, 1500);
        assert_eq!(trace.total_tokens, 500);
        assert!(trace.completed);
    }

    #[test]
    fn test_trace_builder_with_nodes() {
        let msg = TraceBuilder::new()
            .execution_id("exec-123")
            .add_node_execution(create_node_record("search", 500, 100, true))
            .add_node_execution(create_node_record("analyze", 1000, 200, true))
            .build();

        let trace = extract_execution_trace(&msg).expect("Should be ExecutionTrace");
        assert_eq!(trace.nodes_executed.len(), 2);
        assert_eq!(trace.nodes_executed[0].node, "search");
        assert_eq!(trace.nodes_executed[1].node, "analyze");
    }

    #[test]
    fn test_trace_builder_with_errors() {
        let msg = TraceBuilder::new()
            .execution_id("exec-123")
            .completed(false)
            .add_error(create_error_record("fetch", "Connection timeout", false))
            .build();

        let trace = extract_execution_trace(&msg).expect("Should be ExecutionTrace");
        assert!(!trace.completed);
        assert_eq!(trace.errors.len(), 1);
        assert_eq!(trace.errors[0].node, "fetch");
        assert_eq!(trace.errors[0].message, "Connection timeout");
    }

    #[test]
    fn test_is_execution_trace() {
        let trace_msg = create_execution_trace_message("exec-1", "thread-1", 100, 50, true);
        assert!(is_execution_trace(&trace_msg));

        let empty_msg = DashStreamMessage { message: None };
        assert!(!is_execution_trace(&empty_msg));
    }

    #[test]
    fn test_extract_execution_trace_returns_none_for_other_types() {
        let msg = DashStreamMessage { message: None };
        assert!(extract_execution_trace(&msg).is_none());
    }

    #[test]
    fn test_create_node_record() {
        let node = create_node_record("my_node", 1000, 500, true);
        assert_eq!(node.node, "my_node");
        assert_eq!(node.duration_ms, 1000);
        assert_eq!(node.total_tokens, 500);
        assert!(node.succeeded);
    }

    #[test]
    fn test_create_error_record() {
        let error = create_error_record("failed_node", "Something went wrong", true);
        assert_eq!(error.node, "failed_node");
        assert_eq!(error.message, "Something went wrong");
        assert!(error.recovered);
    }
}
