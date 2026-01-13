//! # DashStream-based Trace Collection
//!
//! This module provides trace collection capabilities by consuming DashStream events
//! from Kafka. Unlike runtime interception approaches, this design is:
//!
//! NOTE: This module defines deprecated `TraceCollector` type. Internal usage is
//! allowed for backward compatibility. New code should use `ExecutionTrace` and
//! `ExecutionTraceBuilder` from the introspection module.

#![allow(deprecated)]
//!
//! - **Decoupled**: Trace collection doesn't affect graph execution
//! - **Persistent**: Traces are stored in Kafka and can be replayed
//! - **Zero overhead**: Logging happens asynchronously, no execution impact
//! - **Scalable**: Kafka handles high-volume event streams
//!
//! ## Architecture
//!
//! ```text
//! DashFlow Execution → DashStreamCallback → Kafka
//!                                              ↓
//!                                     TraceCollector
//!                                              ↓
//!                          BootstrapFinetune / GRPO Optimizers
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::optimize::trace::TraceCollector;
//!
//! // Create trace collector
//! let mut collector = TraceCollector::new("localhost:9092", "dashstream-events").await?;
//!
//! // Execute graph with specific thread_id (events logged automatically)
//! let result = graph.invoke(initial_state, "session-123").await?;
//!
//! // Collect traces from Kafka
//! let traces = collector.collect_for_thread("session-123").await?;
//!
//! // Each trace entry has: node name, inputs, outputs
//! for entry in traces {
//!     println!("Node: {}", entry.predictor_name);
//!     println!("  Inputs: {:?}", entry.inputs);
//!     println!("  Outputs: {:?}", entry.outputs);
//! }
//! ```

#[allow(unused_imports)] // Example used in doc examples
use crate::optimize::example::Example;
#[allow(deprecated)] // TraceEntry is deprecated but still used for backwards compatibility
use crate::optimize::trace_types::{FailedPrediction, Prediction, PredictionOrFailed, TraceEntry};
#[allow(unused_imports)] // Serialize/Deserialize used by TraceData which may be removed
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use dashflow_streaming::consumer::DashStreamConsumer;
use dashflow_streaming::dash_stream_message::Message;
use dashflow_streaming::{diff_operation, Event, EventType, StateDiff};

const DASHFLOW_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS: &str =
    "DASHFLOW_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS";
const DEFAULT_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Error types for trace collection
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TraceError {
    /// Failed to create or interact with the Kafka consumer.
    ///
    /// This can occur if the Kafka broker is unreachable or the topic doesn't exist.
    #[error("Kafka consumer error: {0}")]
    Consumer(String),

    /// Failed to decode a message from Kafka.
    ///
    /// The message payload may be corrupted or use an incompatible schema version.
    #[error("Message decode error: {0}")]
    Decode(String),

    /// Failed to reconstruct the execution trace from events.
    ///
    /// This can occur if events are missing or out of order.
    #[error("Trace reconstruction failed: {0}")]
    Reconstruction(String),

    /// Timed out waiting for the GRAPH_END event.
    ///
    /// The graph execution may be stuck or the event was lost.
    #[error("Timeout waiting for GRAPH_END event (thread: {0})")]
    Timeout(String),

    /// The state diff format is invalid or unsupported.
    ///
    /// State diffs must be valid JSON Patch operations.
    #[error("Invalid state diff format: {0}")]
    InvalidDiff(String),

    /// Failed to parse JSON data.
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error from the DashStream library.
    #[error("DashStream error: {0}")]
    DashStream(#[from] dashflow_streaming::errors::Error),
}

/// Result type for trace operations.
pub type Result<T> = std::result::Result<T, TraceError>;

/// Collects execution traces from DashStream Kafka events
///
/// TraceCollector consumes DashStream messages for a specific thread_id (execution session)
/// and reconstructs the execution trace by pairing NODE_START/NODE_END events with
/// StateDiff messages.
///
/// **DEPRECATED:** Use `ExecutionTrace` and `ExecutionTraceBuilder` from the introspection module
/// directly for local trace collection. For DashStream integration, construct ExecutionTrace
/// from streaming events manually. Local trace collection no longer requires Kafka infrastructure.
#[deprecated(
    since = "1.11.3",
    note = "Use ExecutionTrace and ExecutionTraceBuilder from introspection module for local collection."
)]
pub struct TraceCollector {
    /// Kafka consumer for DashStream messages
    consumer: DashStreamConsumer,

    /// Cache of state diffs by thread_id
    state_cache: HashMap<String, Vec<StateDiff>>,

    /// Cache of events by thread_id
    event_cache: HashMap<String, Vec<Event>>,

    /// Timeout for waiting for GRAPH_END event (default: 60 seconds)
    timeout: Duration,
}

impl TraceCollector {
    /// Create a new trace collector
    ///
    /// # Arguments
    /// * `kafka_brokers` - Kafka broker address (e.g., "localhost:9092")
    /// * `topic` - Kafka topic name (e.g., "dashstream-events")
    ///
    /// # Example
    /// ```rust,ignore
    /// let collector = TraceCollector::new("localhost:9092", "dashstream-events").await?;
    /// ```
    pub async fn new(kafka_brokers: &str, topic: &str) -> Result<Self> {
        let connect_timeout = Duration::from_secs(
            crate::core::config_loader::env_vars::env_u64(
                DASHFLOW_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS,
                DEFAULT_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS,
            ),
        );
        let consumer = tokio::time::timeout(
            connect_timeout,
            DashStreamConsumer::new(
                kafka_brokers,
                topic,
                "trace-collector-group", // Consumer group
            ),
        )
        .await
        .map_err(|e| {
            TraceError::Consumer(format!(
                "Timed out creating Kafka consumer after {:?}: {} (set {} to increase)",
                connect_timeout, e, DASHFLOW_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS
            ))
        })?
        .map_err(|e| TraceError::Consumer(e.to_string()))?;

        Ok(Self {
            consumer,
            state_cache: HashMap::new(),
            event_cache: HashMap::new(),
            timeout: Duration::from_secs(60),
        })
    }

    /// Create a new trace collector for a specific Kafka partition.
    ///
    /// DashStreamConsumer is single-partition (rskafka PartitionClient), so for
    /// multi-partition topics you must select the partition that contains the
    /// target thread_id. If unsure, run one collector per partition.
    pub async fn new_for_partition(
        kafka_brokers: &str,
        topic: &str,
        partition: i32,
    ) -> Result<Self> {
        let connect_timeout = Duration::from_secs(
            crate::core::config_loader::env_vars::env_u64(
                DASHFLOW_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS,
                DEFAULT_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS,
            ),
        );
        let consumer = tokio::time::timeout(
            connect_timeout,
            DashStreamConsumer::new_for_partition(kafka_brokers, topic, partition),
        )
        .await
        .map_err(|e| {
            TraceError::Consumer(format!(
                "Timed out creating Kafka consumer for partition {} after {:?}: {} (set {} to increase)",
                partition, connect_timeout, e, DASHFLOW_TRACE_COLLECTOR_CONNECT_TIMEOUT_SECS
            ))
        })?
        .map_err(|e| TraceError::Consumer(e.to_string()))?;

        Ok(Self {
            consumer,
            state_cache: HashMap::new(),
            event_cache: HashMap::new(),
            timeout: Duration::from_secs(60),
        })
    }

    /// Set timeout for waiting for GRAPH_END event
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Collect traces for a specific thread_id (execution session)
    ///
    /// This method consumes Kafka messages until it sees a GRAPH_END event
    /// for the specified thread_id, then reconstructs the execution trace.
    ///
    /// # Arguments
    /// * `thread_id` - The execution session ID to collect traces for
    ///
    /// # Returns
    /// `Vec<TraceEntry>` - Ordered list of node executions with inputs/outputs
    ///
    /// # Example
    /// ```rust,ignore
    /// let traces = collector.collect_for_thread("session-123").await?;
    /// ```
    pub async fn collect_for_thread(&mut self, thread_id: &str) -> Result<Vec<TraceEntry>> {
        // Start timeout
        let start = std::time::Instant::now();

        // Consume messages until we see GRAPH_END for this thread_id
        loop {
            // Check timeout
            if start.elapsed() > self.timeout {
                return Err(TraceError::Timeout(thread_id.to_string()));
            }

            // Fetch next message with timeout
            let msg = match self.consumer.next().await {
                Some(Ok(msg)) => msg,
                Some(Err(e)) => return Err(TraceError::DashStream(e)),
                None => {
                    // No more messages, wait a bit and retry
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };

            // Process message based on type
            match msg.message {
                Some(Message::Event(event)) => {
                    // Check if this event is for our thread_id
                    if let Some(header) = &event.header {
                        if header.thread_id == thread_id {
                            // Cache the event
                            self.event_cache
                                .entry(thread_id.to_string())
                                .or_default()
                                .push(event.clone());

                            // Check if this is GRAPH_END
                            if event.event_type == EventType::GraphEnd as i32 {
                                break; // Done collecting for this thread
                            }
                        }
                    }
                }
                Some(Message::StateDiff(diff)) => {
                    // Check if this diff is for our thread_id
                    if let Some(header) = &diff.header {
                        if header.thread_id == thread_id {
                            self.state_cache
                                .entry(thread_id.to_string())
                                .or_default()
                                .push(diff);
                        }
                    }
                }
                _ => {
                    // Ignore other message types (TokenChunk, ToolExecution, etc.)
                }
            }
        }

        // Reconstruct trace from cached events and state diffs
        let trace = self.reconstruct_trace(thread_id)?;

        // Clean up cache to free memory
        self.event_cache.remove(thread_id);
        self.state_cache.remove(thread_id);

        Ok(trace)
    }

    /// Reconstruct execution trace from cached events and state diffs
    ///
    /// Algorithm:
    /// 1. Find all NODE_START/NODE_END pairs
    /// 2. For each node execution:
    ///    - Extract inputs from state BEFORE node execution
    ///    - Extract outputs from StateDiff AFTER node execution
    /// 3. Build TraceEntry with predictor_name, inputs, outputs
    fn reconstruct_trace(&self, thread_id: &str) -> Result<Vec<TraceEntry>> {
        // M-844: Include thread_id in error messages for easier debugging
        let events = self.event_cache.get(thread_id).ok_or_else(|| {
            TraceError::Reconstruction(format!("No events found for thread_id: {}", thread_id))
        })?;
        let state_diffs = self.state_cache.get(thread_id).ok_or_else(|| {
            TraceError::Reconstruction(format!("No state diffs found for thread_id: {}", thread_id))
        })?;

        let mut trace = Vec::new();
        let mut state_diff_index = 0;

        // Find NODE_START/NODE_END pairs
        let mut i = 0;
        while i < events.len() {
            let event = &events[i];

            if event.event_type == EventType::NodeStart as i32 {
                let node_name = event.node_id.clone();

                // Find matching NODE_END
                let mut j = i + 1;
                let mut found_end = false;
                while j < events.len() {
                    let end_event = &events[j];
                    if end_event.event_type == EventType::NodeEnd as i32
                        && end_event.node_id == node_name
                    {
                        // Found matching pair

                        // Extract inputs (state before node execution)
                        let inputs = if state_diff_index > 0 {
                            self.extract_inputs_from_diff(&state_diffs[state_diff_index - 1])?
                        } else {
                            HashMap::new() // First node, no prior state
                        };

                        // Extract outputs (state diff after node execution)
                        let outputs = if state_diff_index < state_diffs.len() {
                            self.extract_outputs_from_diff(&state_diffs[state_diff_index])?
                        } else {
                            // No state diff for this node (shouldn't happen in normal execution)
                            Prediction::new()
                        };

                        trace.push(TraceEntry {
                            predictor_name: node_name.clone(),
                            inputs,
                            outputs: PredictionOrFailed::Success(outputs),
                        });

                        state_diff_index += 1;
                        found_end = true;
                        i = j; // Continue after NODE_END
                        break;
                    }
                    j += 1;
                }

                if !found_end {
                    // NODE_START without matching NODE_END (error or crash)
                    trace.push(TraceEntry {
                        predictor_name: node_name.clone(),
                        inputs: HashMap::new(),
                        outputs: PredictionOrFailed::Failed(FailedPrediction {
                            error: "Node execution did not complete".to_string(),
                        }),
                    });
                }
            }
            i += 1;
        }

        Ok(trace)
    }

    /// Extract inputs from a StateDiff message
    ///
    /// Parses JSON Patch operations to extract state fields as inputs
    fn extract_inputs_from_diff(
        &self,
        diff: &StateDiff,
    ) -> Result<HashMap<String, serde_json::Value>> {
        Self::extract_fields_from_diff(diff)
    }

    /// Extract outputs from a StateDiff message
    ///
    /// Converts state diff operations to a Prediction object
    fn extract_outputs_from_diff(&self, diff: &StateDiff) -> Result<Prediction> {
        let fields = Self::extract_fields_from_diff(diff)?;

        let mut prediction = Prediction::new();
        for (key, value) in fields {
            prediction.fields.insert(key, value);
        }

        Ok(prediction)
    }

    /// Extract fields from a StateDiff (helper for testing)
    fn extract_fields_from_diff(diff: &StateDiff) -> Result<HashMap<String, serde_json::Value>> {
        let mut inputs = HashMap::new();

        for op in &diff.operations {
            // Only process ADD and REPLACE operations
            // M-842: Log warning when encountering an invalid/unknown op type
            let op_type = match diff_operation::OpType::try_from(op.op) {
                Ok(t) => t,
                Err(_) => {
                    tracing::warn!(
                        op_value = op.op,
                        path = %op.path,
                        "Unknown diff operation type encountered, defaulting to Add"
                    );
                    diff_operation::OpType::Add
                }
            };

            if matches!(
                op_type,
                diff_operation::OpType::Add | diff_operation::OpType::Replace
            ) {
                // Extract field name from path (e.g., "/query" -> "query")
                let field_name = op.path.trim_start_matches('/');
                if field_name.is_empty() {
                    continue; // Skip root operations
                }

                // Decode value (assumes JSON encoding)
                let value: serde_json::Value = serde_json::from_slice(&op.value)?;

                inputs.insert(field_name.to_string(), value);
            }
        }

        Ok(inputs)
    }

    /// Collect traces for multiple threads concurrently
    ///
    /// Useful for batch processing training data.
    ///
    /// # Example
    /// ```rust,ignore
    /// let thread_ids = vec!["session-1".to_string(), "session-2".to_string()];
    /// let all_traces = collector.collect_for_threads(thread_ids).await?;
    /// ```
    pub async fn collect_for_threads(
        &mut self,
        thread_ids: Vec<String>,
    ) -> Result<HashMap<String, Vec<TraceEntry>>> {
        let mut results = HashMap::new();

        for thread_id in thread_ids {
            let trace = self.collect_for_thread(&thread_id).await?;
            results.insert(thread_id, trace);
        }

        Ok(results)
    }

    /// Collect traces for multiple threads in a single pass (optimized)
    ///
    /// This method pre-fetches all messages and processes them in parallel,
    /// which is more efficient than calling `collect_for_thread` sequentially
    /// for large numbers of threads.
    ///
    /// # Arguments
    ///
    /// * `thread_ids` - Set of thread IDs to collect traces for
    ///
    /// # Returns
    ///
    /// HashMap mapping thread_id -> trace entries, collected in a single pass
    ///
    /// # Performance
    ///
    /// This is O(messages) instead of O(threads * messages), making it much
    /// faster for large trace collections in GRPO optimization.
    pub async fn collect_batch_parallel(
        &mut self,
        thread_ids: std::collections::HashSet<String>,
    ) -> Result<HashMap<String, Vec<TraceEntry>>> {
        use std::collections::HashSet;

        if thread_ids.is_empty() {
            return Ok(HashMap::new());
        }

        // Track which threads have completed (seen GRAPH_END)
        let mut completed: HashSet<String> = HashSet::new();

        // Start timeout
        let start = std::time::Instant::now();

        // Consume messages until we've completed all threads or timeout
        while completed.len() < thread_ids.len() {
            // Check timeout
            if start.elapsed() > self.timeout {
                // Return what we have so far rather than failing completely
                tracing::warn!(
                    "Batch trace collection timed out. Collected {}/{} threads.",
                    completed.len(),
                    thread_ids.len()
                );
                break;
            }

            // Fetch next message with short timeout
            let msg = match self.consumer.next().await {
                Some(Ok(msg)) => msg,
                Some(Err(e)) => return Err(TraceError::DashStream(e)),
                None => {
                    // No more messages, wait a bit and retry
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };

            // Process message based on type - for ALL relevant thread_ids
            match msg.message {
                Some(Message::Event(event)) => {
                    if let Some(header) = &event.header {
                        // Only process if this is a thread we care about
                        if thread_ids.contains(&header.thread_id) {
                            // Cache the event
                            self.event_cache
                                .entry(header.thread_id.clone())
                                .or_default()
                                .push(event.clone());

                            // Check if this is GRAPH_END
                            if event.event_type == EventType::GraphEnd as i32 {
                                completed.insert(header.thread_id.clone());
                            }
                        }
                    }
                }
                Some(Message::StateDiff(state_diff)) => {
                    if let Some(header) = &state_diff.header {
                        if thread_ids.contains(&header.thread_id) {
                            self.state_cache
                                .entry(header.thread_id.clone())
                                .or_default()
                                .push(state_diff.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // Now build trace entries for each completed thread in parallel
        let mut results = HashMap::new();
        for thread_id in &thread_ids {
            let events = self.event_cache.get(thread_id);
            let states = self.state_cache.get(thread_id);

            if let (Some(events), Some(states)) = (events, states) {
                let entries = self.build_trace_entries_from_cache(events, states);
                if !entries.is_empty() {
                    results.insert(thread_id.clone(), entries);
                }
            }
        }

        Ok(results)
    }

    /// Build trace entries from cached events and state diffs
    fn build_trace_entries_from_cache(
        &self,
        events: &[dashflow_streaming::Event],
        states: &[StateDiff],
    ) -> Vec<TraceEntry> {
        use std::collections::HashMap as StdHashMap;

        // StateDiff doesn't have node_id directly, so we match by sequence/timestamp
        // Build a vector of state diffs indexed by sequence number from header
        let state_by_seq: StdHashMap<u64, &StateDiff> = states
            .iter()
            .filter_map(|s| s.header.as_ref().map(|h| (h.sequence, s)))
            .collect();

        let mut entries = Vec::new();
        let mut node_starts: StdHashMap<&str, (&dashflow_streaming::Event, u64)> =
            StdHashMap::new();

        for event in events {
            let event_type = event.event_type;
            // M-843: Warn when header is missing (sequence defaults to 0, which may affect ordering)
            let seq = match event.header.as_ref() {
                Some(h) => h.sequence,
                None => {
                    tracing::warn!(
                        node_id = %event.node_id,
                        event_type = event_type,
                        "Event missing header, using sequence=0 which may affect trace ordering"
                    );
                    0
                }
            };

            if event_type == EventType::NodeStart as i32 {
                node_starts.insert(&event.node_id, (event, seq));
            } else if event_type == EventType::NodeEnd as i32 {
                // Try to build a trace entry
                if let Some((start_event, start_seq)) = node_starts.remove(event.node_id.as_str()) {
                    // Find state diff that occurred between start and end
                    // Look for state diffs with sequence between start_seq and current seq
                    let state_diff = state_by_seq
                        .iter()
                        .filter(|(&s, _)| s > start_seq && s <= seq)
                        .map(|(_, sd)| *sd)
                        .next();

                    if let Some(state_diff) = state_diff {
                        if let Some(entry) = self.build_trace_entry_from_events(
                            &event.node_id,
                            start_event,
                            event,
                            state_diff,
                        ) {
                            entries.push(entry);
                        }
                    } else {
                        // No state diff found, create entry from event attributes
                        entries.push(TraceEntry {
                            predictor_name: event.node_id.clone(),
                            inputs: HashMap::new(),
                            outputs: PredictionOrFailed::Success(Prediction::new()),
                        });
                    }
                }
            }
        }

        entries
    }

    /// Build a single trace entry from cached data
    fn build_trace_entry_from_events(
        &self,
        node_id: &str,
        _start_event: &dashflow_streaming::Event,
        _end_event: &dashflow_streaming::Event,
        state_diff: &StateDiff,
    ) -> Option<TraceEntry> {
        // Extract outputs from state_diff operations (JSON Patch format)
        let mut outputs = HashMap::new();

        // Parse operations to extract state changes
        for op in &state_diff.operations {
            // JSON Patch operations have path and value (value is bytes)
            // Extract the field name from the path (e.g., "/field" -> "field")
            let field_name = op.path.trim_start_matches('/').to_string();
            if !field_name.is_empty() {
                // Try to parse the value bytes as JSON
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&op.value) {
                    outputs.insert(field_name, value);
                } else if let Ok(str_val) = std::str::from_utf8(&op.value) {
                    // If not valid JSON, try as string
                    outputs.insert(field_name, serde_json::json!(str_val));
                }
            }
        }

        // If operations are empty, try to use full_state
        if outputs.is_empty() && !state_diff.full_state.is_empty() {
            if let Ok(full) = serde_json::from_slice::<serde_json::Value>(&state_diff.full_state) {
                if let Some(obj) = full.as_object() {
                    for (k, v) in obj {
                        outputs.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        Some(TraceEntry {
            predictor_name: node_id.to_string(),
            inputs: HashMap::new(),
            outputs: PredictionOrFailed::Success(Prediction { fields: outputs }),
        })
    }
}

// Note: TraceEntry, Prediction, FailedPrediction, PredictionOrFailed, and TraceData
// are now defined in trace_types.rs and re-exported from optimize/mod.rs
// This enables local optimization without requiring the dashstream feature.
// See DESIGN_INVARIANTS.md for the reasoning.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimize::TraceData;
    use dashflow_streaming::{diff_operation::OpType, DiffOperation, Header};

    #[test]
    fn test_extract_inputs_from_diff() {
        // Create a mock StateDiff with ADD operations
        let diff = StateDiff {
            header: Some(create_test_header("test-thread")),
            base_checkpoint_id: vec![],
            operations: vec![
                DiffOperation {
                    op: OpType::Add as i32,
                    path: "/query".to_string(),
                    value: serde_json::to_vec(&serde_json::json!("What is Rust?")).unwrap(),
                    from: String::new(),
                    encoding: 0, // JSON encoding
                },
                DiffOperation {
                    op: OpType::Add as i32,
                    path: "/context".to_string(),
                    value: serde_json::to_vec(&serde_json::json!("programming")).unwrap(),
                    from: String::new(),
                    encoding: 0,
                },
            ],
            state_hash: vec![],
            full_state: vec![],
        };

        let inputs =
            TraceCollector::extract_fields_from_diff(&diff).expect("Failed to extract inputs");

        assert_eq!(inputs.len(), 2);
        assert_eq!(
            inputs.get("query"),
            Some(&serde_json::json!("What is Rust?"))
        );
        assert_eq!(
            inputs.get("context"),
            Some(&serde_json::json!("programming"))
        );
    }

    #[test]
    fn test_extract_inputs_ignores_remove_operations() {
        // Create a StateDiff with REMOVE operations (should be ignored)
        let diff = StateDiff {
            header: Some(create_test_header("test-thread")),
            operations: vec![DiffOperation {
                op: OpType::Remove as i32,
                path: "/temp".to_string(),
                value: vec![],
                from: String::new(),
                encoding: 0,
            }],
            state_hash: vec![],
            base_checkpoint_id: vec![],
            full_state: vec![],
        };

        let inputs =
            TraceCollector::extract_fields_from_diff(&diff).expect("Failed to extract inputs");
        assert_eq!(inputs.len(), 0); // REMOVE operations should be ignored
    }

    #[test]
    fn test_extract_inputs_handles_replace() {
        // Create a StateDiff with REPLACE operation
        let diff = StateDiff {
            header: Some(create_test_header("test-thread")),
            operations: vec![DiffOperation {
                op: OpType::Replace as i32,
                path: "/answer".to_string(),
                value: serde_json::to_vec(&serde_json::json!("Updated answer")).unwrap(),
                from: String::new(),
                encoding: 0,
            }],
            state_hash: vec![],
            base_checkpoint_id: vec![],
            full_state: vec![],
        };

        let inputs =
            TraceCollector::extract_fields_from_diff(&diff).expect("Failed to extract inputs");
        assert_eq!(inputs.len(), 1);
        assert_eq!(
            inputs.get("answer"),
            Some(&serde_json::json!("Updated answer"))
        );
    }

    #[test]
    fn test_extract_outputs_from_diff() {
        // Create a StateDiff for outputs
        let diff = StateDiff {
            header: Some(create_test_header("test-thread")),
            operations: vec![DiffOperation {
                op: OpType::Add as i32,
                path: "/result".to_string(),
                value: serde_json::to_vec(&serde_json::json!("Success!")).unwrap(),
                from: String::new(),
                encoding: 0,
            }],
            state_hash: vec![],
            base_checkpoint_id: vec![],
            full_state: vec![],
        };

        let outputs =
            TraceCollector::extract_fields_from_diff(&diff).expect("Failed to extract outputs");
        assert_eq!(outputs.get("result"), Some(&serde_json::json!("Success!")));
    }

    #[test]
    fn test_extract_inputs_skips_empty_paths() {
        // Create a StateDiff with root path (should be skipped)
        let diff = StateDiff {
            header: Some(create_test_header("test-thread")),
            operations: vec![DiffOperation {
                op: OpType::Add as i32,
                path: "/".to_string(), // Root path
                value: serde_json::to_vec(&serde_json::json!({"nested": "data"})).unwrap(),
                from: String::new(),
                encoding: 0,
            }],
            state_hash: vec![],
            base_checkpoint_id: vec![],
            full_state: vec![],
        };

        let inputs =
            TraceCollector::extract_fields_from_diff(&diff).expect("Failed to extract inputs");
        assert_eq!(inputs.len(), 0); // Root paths should be skipped
    }

    #[test]
    fn test_trace_data_serialization() {
        let example = Example::new().with("input", "test input");

        let trace = vec![TraceEntry {
            predictor_name: "node1".to_string(),
            inputs: HashMap::new(),
            outputs: PredictionOrFailed::Success(Prediction::new()),
        }];

        let trace_data = TraceData {
            example_ind: 0,
            example,
            prediction: PredictionOrFailed::Success(Prediction::new()),
            trace,
            score: Some(0.95),
        };

        let json = serde_json::to_string(&trace_data).expect("Failed to serialize");
        assert!(json.contains("node1"));
        assert!(json.contains("0.95"));
    }

    // Helper functions for tests
    fn create_test_header(thread_id: &str) -> Header {
        Header {
            message_id: vec![1, 2, 3],
            timestamp_us: 0,
            tenant_id: String::new(),
            thread_id: thread_id.to_string(),
            sequence: 1,
            r#type: 0,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }
    }
}
