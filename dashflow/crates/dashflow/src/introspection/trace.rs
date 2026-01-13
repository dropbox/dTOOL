//! Execution Tracing API
//!
//! This module provides the [`ExecutionTrace`] type for recording and analyzing
//! the execution history of graph runs.

use super::PerformanceMetrics;
use crate::metrics::ExecutionMetrics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Execution Tracing API
// ============================================================================

/// Execution trace - complete history of a graph execution.
///
/// This struct provides AI agents with the ability to review their own
/// execution history, including which nodes were executed, how long they took,
/// token usage, and any errors encountered. Essential for debugging, optimization,
/// and self-improvement workflows.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::introspection::ExecutionTrace;
///
/// // AI reviews its previous execution
/// let trace = graph.get_execution_trace("session_123").await?;
///
/// // "Which node took longest?"
/// if let Some(slowest) = trace.slowest_node() {
///     println!("Slowest: {} took {}ms", slowest.node_name, slowest.duration_ms);
/// }
///
/// // "Where did I use most tokens?"
/// if let Some(expensive) = trace.most_expensive_node() {
///     println!("Most tokens: {} used {}", expensive.node_name, expensive.tokens_used);
/// }
///
/// // "Did any errors occur?"
/// if trace.has_errors() {
///     for error in &trace.errors {
///         println!("Error in {}: {}", error.node, error.message);
///     }
/// }
///
/// // Export for analysis
/// let json = trace.to_json()?;
/// ```
///
/// # Errors
///
/// - [`serde_json::Error`] - Returned by [`ExecutionTrace::to_json`] and
///   [`ExecutionTrace::from_json`] on
///   serialization/deserialization failure
///
/// # See Also
///
/// - [`NodeExecution`] - Details about individual node executions
/// - [`ErrorTrace`] - Error information captured during execution
/// - [`crate::ExecutionContext`] - Live context during execution
/// - [`crate::GraphManifest`] - Static graph structure
/// - [`PerformanceMetrics`] - Real-time performance monitoring
/// - [`ExecutionTraceBuilder`] - Builder for creating traces programmatically
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Thread ID for this execution (if using checkpointing)
    pub thread_id: Option<String>,
    /// Unique execution ID for this trace
    pub execution_id: Option<String>,
    /// Parent execution ID (Observability Phase 3)
    ///
    /// For subgraph executions, this is the execution_id of the parent graph
    /// that invoked this subgraph. Enables hierarchical execution tracing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_execution_id: Option<String>,
    /// Root execution ID (Observability Phase 3)
    ///
    /// For nested subgraph executions, this is the execution_id of the
    /// top-level graph. Always present for subgraphs, allows correlating
    /// all executions in a graph hierarchy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_execution_id: Option<String>,
    /// Subgraph depth (Observability Phase 3)
    ///
    /// 0 for top-level graphs, 1 for direct subgraphs, 2+ for nested.
    /// Enables depth-limited queries and visualization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// All nodes executed in order
    pub nodes_executed: Vec<NodeExecution>,
    /// Total execution duration
    pub total_duration_ms: u64,
    /// Total tokens used across all nodes
    pub total_tokens: u64,
    /// All errors encountered during execution
    pub errors: Vec<ErrorTrace>,
    /// Whether execution completed successfully
    pub completed: bool,
    /// Execution start timestamp (ISO 8601)
    pub started_at: Option<String>,
    /// Execution end timestamp (ISO 8601)
    pub ended_at: Option<String>,
    /// Final state snapshot (if captured)
    pub final_state: Option<serde_json::Value>,
    /// Custom metadata about the execution
    pub metadata: HashMap<String, serde_json::Value>,
    /// Rich execution metrics from LocalMetricsBatch
    ///
    /// This field connects the efficient metrics collection system (LocalMetricsBatch)
    /// to the introspection/self-improvement systems. When populated, provides:
    /// - Node durations and execution counts
    /// - Edge traversals and conditional branches
    /// - Parallel execution stats
    /// - Checkpoint operations
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_metrics: Option<ExecutionMetrics>,
    /// Real-time performance metrics snapshot
    ///
    /// This field connects the real-time performance monitoring system
    /// to self-improvement analysis. When populated, provides:
    /// - Current, average, P95, P99 latencies
    /// - Token throughput
    /// - Error rate
    /// - Resource usage (memory, CPU)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_metrics: Option<PerformanceMetrics>,
}

impl ExecutionTrace {
    /// Create a new empty execution trace
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for execution traces
    #[must_use]
    pub fn builder() -> ExecutionTraceBuilder {
        ExecutionTraceBuilder::new()
    }

    /// Convert trace to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert trace to compact JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse trace from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Get the number of nodes executed
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes_executed.len()
    }

    /// Get the number of errors
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Check if the execution had any errors
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if execution completed without errors
    #[must_use]
    pub fn is_successful(&self) -> bool {
        self.completed && self.errors.is_empty()
    }

    /// Get the slowest node execution
    #[must_use]
    pub fn slowest_node(&self) -> Option<&NodeExecution> {
        self.nodes_executed.iter().max_by_key(|n| n.duration_ms)
    }

    /// Get the node that used the most tokens
    #[must_use]
    pub fn most_expensive_node(&self) -> Option<&NodeExecution> {
        self.nodes_executed.iter().max_by_key(|n| n.tokens_used)
    }

    /// Get execution by node name (first occurrence)
    #[must_use]
    pub fn get_node_execution(&self, node_name: &str) -> Option<&NodeExecution> {
        self.nodes_executed.iter().find(|n| n.node == node_name)
    }

    /// Get all executions for a specific node (for loops)
    #[must_use]
    pub fn get_all_node_executions(&self, node_name: &str) -> Vec<&NodeExecution> {
        self.nodes_executed
            .iter()
            .filter(|n| n.node == node_name)
            .collect()
    }

    /// Get execution count for a specific node
    #[must_use]
    pub fn node_execution_count(&self, node_name: &str) -> usize {
        self.nodes_executed
            .iter()
            .filter(|n| n.node == node_name)
            .count()
    }

    /// Get total time spent in a specific node (sum of all executions)
    #[must_use]
    pub fn total_time_in_node(&self, node_name: &str) -> u64 {
        self.nodes_executed
            .iter()
            .filter(|n| n.node == node_name)
            .map(|n| n.duration_ms)
            .sum()
    }

    /// Get total tokens used by a specific node
    #[must_use]
    pub fn total_tokens_in_node(&self, node_name: &str) -> u64 {
        self.nodes_executed
            .iter()
            .filter(|n| n.node == node_name)
            .map(|n| n.tokens_used)
            .sum()
    }

    /// Get errors for a specific node
    #[must_use]
    pub fn errors_for_node(&self, node_name: &str) -> Vec<&ErrorTrace> {
        self.errors.iter().filter(|e| e.node == node_name).collect()
    }

    /// Get unique node names in execution order
    #[must_use]
    pub fn unique_nodes(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        self.nodes_executed
            .iter()
            .filter(|n| seen.insert(n.node.as_str()))
            .map(|n| n.node.as_str())
            .collect()
    }

    /// Get the average node execution time in milliseconds
    #[must_use]
    pub fn average_node_duration_ms(&self) -> f64 {
        if self.nodes_executed.is_empty() {
            0.0
        } else {
            self.nodes_executed
                .iter()
                .map(|n| n.duration_ms as f64)
                .sum::<f64>()
                / self.nodes_executed.len() as f64
        }
    }

    /// Get execution time breakdown by node (percentage)
    #[must_use]
    pub fn time_breakdown(&self) -> HashMap<String, f64> {
        let mut result = HashMap::new();
        if self.total_duration_ms == 0 {
            return result;
        }

        for node in &self.nodes_executed {
            let entry = result.entry(node.node.clone()).or_insert(0.0);
            *entry += (node.duration_ms as f64 / self.total_duration_ms as f64) * 100.0;
        }
        result
    }

    /// Get token usage breakdown by node (percentage)
    #[must_use]
    pub fn token_breakdown(&self) -> HashMap<String, f64> {
        let mut result = HashMap::new();
        if self.total_tokens == 0 {
            return result;
        }

        for node in &self.nodes_executed {
            let entry = result.entry(node.node.clone()).or_insert(0.0);
            *entry += (node.tokens_used as f64 / self.total_tokens as f64) * 100.0;
        }
        result
    }

    /// Detect patterns in this single trace using the unified pattern engine.
    ///
    /// For pattern detection across multiple traces (recommended), collect traces
    /// and use `UnifiedPatternEngine::detect(&traces)` directly.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = execution_result.trace();
    /// let patterns = trace.detect_patterns();
    /// for pattern in patterns.iter().filter(|p| p.is_actionable()) {
    ///     println!("Action: {} - {}", pattern.description, pattern.recommendations.join(", "));
    /// }
    /// ```
    #[must_use]
    pub fn detect_patterns(&self) -> Vec<crate::pattern_engine::UnifiedPattern> {
        let engine = crate::pattern_engine::UnifiedPatternEngine::default();
        engine.detect(std::slice::from_ref(self))
    }

    /// Detect patterns with custom configuration.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::pattern_engine::UnifiedPatternEngineConfig;
    ///
    /// let config = UnifiedPatternEngineConfig::execution_only();
    /// let patterns = trace.detect_patterns_with_config(config);
    /// ```
    #[must_use]
    pub fn detect_patterns_with_config(
        &self,
        config: crate::pattern_engine::UnifiedPatternEngineConfig,
    ) -> Vec<crate::pattern_engine::UnifiedPattern> {
        let engine = crate::pattern_engine::UnifiedPatternEngine::new(config);
        engine.detect(std::slice::from_ref(self))
    }

    /// Get actionable patterns from this trace.
    ///
    /// Actionable patterns have high strength (≥70%), high confidence (≥60%),
    /// and include specific recommendations.
    #[must_use]
    pub fn actionable_patterns(&self) -> Vec<crate::pattern_engine::UnifiedPattern> {
        self.detect_patterns()
            .into_iter()
            .filter(|p| p.is_actionable())
            .collect()
    }

    /// Generate a pattern analysis report for this trace.
    #[must_use]
    pub fn pattern_report(&self) -> String {
        let engine = crate::pattern_engine::UnifiedPatternEngine::default();
        engine.generate_report(std::slice::from_ref(self))
    }
}

/// Builder for creating execution traces
#[derive(Debug, Default)]
pub struct ExecutionTraceBuilder {
    thread_id: Option<String>,
    execution_id: Option<String>,
    parent_execution_id: Option<String>,
    root_execution_id: Option<String>,
    depth: Option<u32>,
    nodes_executed: Vec<NodeExecution>,
    total_duration_ms: u64,
    total_tokens: u64,
    errors: Vec<ErrorTrace>,
    completed: bool,
    started_at: Option<String>,
    ended_at: Option<String>,
    final_state: Option<serde_json::Value>,
    metadata: HashMap<String, serde_json::Value>,
}

impl ExecutionTraceBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set thread ID
    #[must_use]
    pub fn thread_id(mut self, id: impl Into<String>) -> Self {
        self.thread_id = Some(id.into());
        self
    }

    /// Set execution ID
    #[must_use]
    pub fn execution_id(mut self, id: impl Into<String>) -> Self {
        self.execution_id = Some(id.into());
        self
    }

    /// Set parent execution ID (Observability Phase 3)
    ///
    /// Used for subgraph executions to link back to the parent graph.
    #[must_use]
    pub fn parent_execution_id(mut self, id: impl Into<String>) -> Self {
        self.parent_execution_id = Some(id.into());
        self
    }

    /// Set root execution ID (Observability Phase 3)
    ///
    /// Used for nested subgraph executions to link back to the top-level graph.
    #[must_use]
    pub fn root_execution_id(mut self, id: impl Into<String>) -> Self {
        self.root_execution_id = Some(id.into());
        self
    }

    /// Set subgraph depth (Observability Phase 3)
    ///
    /// 0 for top-level graphs, 1 for direct subgraphs, 2+ for nested.
    #[must_use]
    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = Some(depth);
        self
    }

    /// Add a node execution
    #[must_use]
    pub fn add_node_execution(mut self, execution: NodeExecution) -> Self {
        self.nodes_executed.push(execution);
        self
    }

    /// Set all node executions
    #[must_use]
    pub fn nodes_executed(mut self, nodes: Vec<NodeExecution>) -> Self {
        self.nodes_executed = nodes;
        self
    }

    /// Set total duration in milliseconds
    #[must_use]
    pub fn total_duration_ms(mut self, ms: u64) -> Self {
        self.total_duration_ms = ms;
        self
    }

    /// Set total tokens used
    #[must_use]
    pub fn total_tokens(mut self, tokens: u64) -> Self {
        self.total_tokens = tokens;
        self
    }

    /// Add an error
    #[must_use]
    pub fn add_error(mut self, error: ErrorTrace) -> Self {
        self.errors.push(error);
        self
    }

    /// Set all errors
    #[must_use]
    pub fn errors(mut self, errors: Vec<ErrorTrace>) -> Self {
        self.errors = errors;
        self
    }

    /// Set completion status
    #[must_use]
    pub fn completed(mut self, completed: bool) -> Self {
        self.completed = completed;
        self
    }

    /// Set start timestamp
    #[must_use]
    pub fn started_at(mut self, timestamp: impl Into<String>) -> Self {
        self.started_at = Some(timestamp.into());
        self
    }

    /// Set end timestamp
    #[must_use]
    pub fn ended_at(mut self, timestamp: impl Into<String>) -> Self {
        self.ended_at = Some(timestamp.into());
        self
    }

    /// Set final state
    #[must_use]
    pub fn final_state(mut self, state: serde_json::Value) -> Self {
        self.final_state = Some(state);
        self
    }

    /// Add metadata
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Build the execution trace
    #[must_use]
    pub fn build(self) -> ExecutionTrace {
        ExecutionTrace {
            thread_id: self.thread_id,
            execution_id: self.execution_id,
            parent_execution_id: self.parent_execution_id,
            root_execution_id: self.root_execution_id,
            depth: self.depth,
            nodes_executed: self.nodes_executed,
            total_duration_ms: self.total_duration_ms,
            total_tokens: self.total_tokens,
            errors: self.errors,
            completed: self.completed,
            started_at: self.started_at,
            ended_at: self.ended_at,
            final_state: self.final_state,
            metadata: self.metadata,
            // Builder-constructed traces don't include metrics by default
            execution_metrics: None,
            performance_metrics: None,
        }
    }
}

/// Node execution record - details about a single node execution
///
/// Captures timing, token usage, state changes, and tools called for
/// a single execution of a graph node.
///
/// # Example
///
/// ```rust,ignore
/// let node_exec = NodeExecution::new("tool_executor", 150)
///     .with_tokens(500)
///     .with_state_before(state_before)
///     .with_state_after(state_after)
///     .with_tool("search")
///     .with_tool("calculate");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeExecution {
    /// Node name
    pub node: String,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Tokens used during this execution
    pub tokens_used: u64,
    /// State before execution (if captured)
    pub state_before: Option<serde_json::Value>,
    /// State after execution (if captured)
    pub state_after: Option<serde_json::Value>,
    /// Tools called during this execution
    pub tools_called: Vec<String>,
    /// Whether this execution was successful
    pub success: bool,
    /// Error message if execution failed
    pub error_message: Option<String>,
    /// Execution index (order in the trace)
    pub index: usize,
    /// Timestamp when execution started (ISO 8601)
    pub started_at: Option<String>,
    /// Custom metadata about this execution
    pub metadata: HashMap<String, serde_json::Value>,
}

impl NodeExecution {
    /// Create a new node execution record
    #[must_use]
    pub fn new(node: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            node: node.into(),
            duration_ms,
            tokens_used: 0,
            state_before: None,
            state_after: None,
            tools_called: Vec::new(),
            success: true,
            error_message: None,
            index: 0,
            started_at: None,
            metadata: HashMap::new(),
        }
    }

    /// Set tokens used
    #[must_use]
    pub fn with_tokens(mut self, tokens: u64) -> Self {
        self.tokens_used = tokens;
        self
    }

    /// Set state before execution
    #[must_use]
    pub fn with_state_before(mut self, state: serde_json::Value) -> Self {
        self.state_before = Some(state);
        self
    }

    /// Set state after execution
    #[must_use]
    pub fn with_state_after(mut self, state: serde_json::Value) -> Self {
        self.state_after = Some(state);
        self
    }

    /// Add a tool that was called
    #[must_use]
    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tools_called.push(tool.into());
        self
    }

    /// Set tools called
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools_called = tools;
        self
    }

    /// Mark as failed with error message
    #[must_use]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.success = false;
        self.error_message = Some(error.into());
        self
    }

    /// Set execution index
    #[must_use]
    pub fn with_index(mut self, index: usize) -> Self {
        self.index = index;
        self
    }

    /// Set start timestamp
    #[must_use]
    pub fn with_started_at(mut self, timestamp: impl Into<String>) -> Self {
        self.started_at = Some(timestamp.into());
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Check if this node called any tools
    #[must_use]
    pub fn called_tools(&self) -> bool {
        !self.tools_called.is_empty()
    }

    /// Get the number of tools called
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.tools_called.len()
    }

    /// Check if state changed during execution
    #[must_use]
    pub fn state_changed(&self) -> bool {
        match (&self.state_before, &self.state_after) {
            (Some(before), Some(after)) => before != after,
            _ => false, // Can't determine if state changed without both snapshots
        }
    }

    /// Get keys that changed between state_before and state_after
    #[must_use]
    pub fn changed_keys(&self) -> Vec<String> {
        match (&self.state_before, &self.state_after) {
            (Some(serde_json::Value::Object(before)), Some(serde_json::Value::Object(after))) => {
                let mut changed = Vec::new();
                // Check for new or modified keys
                for (key, after_val) in after {
                    match before.get(key) {
                        Some(before_val) if before_val != after_val => changed.push(key.clone()),
                        None => changed.push(key.clone()),
                        _ => {}
                    }
                }
                // Check for removed keys
                for key in before.keys() {
                    if !after.contains_key(key) {
                        changed.push(key.clone());
                    }
                }
                changed
            }
            _ => Vec::new(),
        }
    }
}

/// Error trace - details about an error during execution
///
/// Captures information about errors that occurred during graph execution,
/// including which node failed, the error message, and context.
///
/// # Example
///
/// ```rust,ignore
/// let error = ErrorTrace::new("tool_executor", "Connection timeout")
///     .with_error_type("Timeout")
///     .with_state_at_error(current_state);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorTrace {
    /// Node where the error occurred
    pub node: String,
    /// Error message
    pub message: String,
    /// Error type/category (e.g., "Timeout", "ValidationError", "LLMError")
    pub error_type: Option<String>,
    /// State at the time of error (if captured)
    pub state_at_error: Option<serde_json::Value>,
    /// Timestamp when error occurred (ISO 8601)
    pub timestamp: Option<String>,
    /// Execution index when error occurred
    pub execution_index: Option<usize>,
    /// Whether the error was recoverable
    pub recoverable: bool,
    /// Whether a retry was attempted
    pub retry_attempted: bool,
    /// Stack trace or additional context
    pub context: Option<String>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ErrorTrace {
    /// Create a new error trace
    #[must_use]
    pub fn new(node: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            message: message.into(),
            error_type: None,
            state_at_error: None,
            timestamp: None,
            execution_index: None,
            recoverable: false,
            retry_attempted: false,
            context: None,
            metadata: HashMap::new(),
        }
    }

    /// Set error type
    #[must_use]
    pub fn with_error_type(mut self, error_type: impl Into<String>) -> Self {
        self.error_type = Some(error_type.into());
        self
    }

    /// Set state at error
    #[must_use]
    pub fn with_state_at_error(mut self, state: serde_json::Value) -> Self {
        self.state_at_error = Some(state);
        self
    }

    /// Set timestamp
    #[must_use]
    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    /// Set execution index
    #[must_use]
    pub fn with_execution_index(mut self, index: usize) -> Self {
        self.execution_index = Some(index);
        self
    }

    /// Mark as recoverable
    #[must_use]
    pub fn recoverable(mut self) -> Self {
        self.recoverable = true;
        self
    }

    /// Mark that retry was attempted
    #[must_use]
    pub fn with_retry_attempted(mut self) -> Self {
        self.retry_attempted = true;
        self
    }

    /// Set context/stack trace
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

// ============================================================================
// Streaming Integration
// ============================================================================

/// Convert ExecutionTrace to streaming DashStreamMessage.
///
/// This function is only available when the `dashstream` feature is enabled.
/// It allows execution traces to be streamed over Kafka for distributed self-reflection.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::introspection::ExecutionTrace;
/// use dashflow::introspection::trace::to_streaming_message;
///
/// let trace = ExecutionTrace::builder()
///     .execution_id("exec-123")
///     .completed(true)
///     .build();
///
/// let msg = to_streaming_message(&trace);
/// // Send msg over Kafka...
/// ```
#[cfg(feature = "dashstream")]
pub fn to_streaming_message(trace: &ExecutionTrace) -> dashflow_streaming::DashStreamMessage {
    use dashflow_streaming::trace::TraceBuilder;

    let mut builder = TraceBuilder::new()
        .execution_id(trace.execution_id.clone().unwrap_or_default())
        .thread_id(trace.thread_id.clone().unwrap_or_default())
        .total_duration_ms(trace.total_duration_ms)
        .total_tokens(trace.total_tokens)
        .completed(trace.completed)
        .started_at(trace.started_at.clone().unwrap_or_default())
        .ended_at(trace.ended_at.clone().unwrap_or_default());

    // Convert final_state to bytes if present
    if let Some(ref state) = trace.final_state {
        if let Ok(bytes) = serde_json::to_vec(state) {
            builder = builder.final_state(bytes);
        }
    }

    // Convert node executions
    for node in &trace.nodes_executed {
        let node_record = dashflow_streaming::NodeExecutionRecord {
            node: node.node.clone(),
            duration_ms: node.duration_ms,
            prompt_tokens: 0, // Not tracked separately in NodeExecution
            completion_tokens: 0,
            total_tokens: node.tokens_used,
            succeeded: node.success,
            started_at: node.started_at.clone().unwrap_or_default(),
            ended_at: String::new(), // Not tracked in NodeExecution
            input: node
                .state_before
                .as_ref()
                .and_then(|v| serde_json::to_vec(v).ok())
                .unwrap_or_default(),
            output: node
                .state_after
                .as_ref()
                .and_then(|v| serde_json::to_vec(v).ok())
                .unwrap_or_default(),
            metadata: node
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect(),
        };
        builder = builder.add_node_execution(node_record);
    }

    // Convert errors
    for error in &trace.errors {
        let error_record = dashflow_streaming::ErrorRecord {
            node: error.node.clone(),
            message: error.message.clone(),
            error_code: error.error_type.clone().unwrap_or_default(), // Map error_type to error_code
            timestamp: error.timestamp.clone().unwrap_or_default(),
            recovered: error.recoverable,
            stack_trace: error.context.clone().unwrap_or_default(),
        };
        builder = builder.add_error(error_record);
    }

    // Convert metadata
    for (key, value) in &trace.metadata {
        if let Ok(bytes) = serde_json::to_vec(value) {
            builder = builder.add_metadata(key.clone(), bytes);
        }
    }

    builder.build()
}

/// Convert a streaming ExecutionTrace message back to a Rust ExecutionTrace.
///
/// This function is only available when the `dashstream` feature is enabled.
/// It allows execution traces received from Kafka to be analyzed locally.
#[cfg(feature = "dashstream")]
pub fn from_streaming_message(
    msg: &dashflow_streaming::DashStreamMessage,
) -> Option<ExecutionTrace> {
    use dashflow_streaming::dash_stream_message::Message;

    let proto_trace = match &msg.message {
        Some(Message::ExecutionTrace(trace)) => trace,
        _ => return None,
    };

    let mut trace = ExecutionTrace {
        thread_id: if proto_trace.thread_id.is_empty() {
            None
        } else {
            Some(proto_trace.thread_id.clone())
        },
        execution_id: if proto_trace.execution_id.is_empty() {
            None
        } else {
            Some(proto_trace.execution_id.clone())
        },
        // Phase 3 hierarchical execution IDs - default to None for backward compatibility
        parent_execution_id: None,
        root_execution_id: None,
        depth: None,
        nodes_executed: vec![],
        total_duration_ms: proto_trace.total_duration_ms,
        total_tokens: proto_trace.total_tokens,
        errors: vec![],
        completed: proto_trace.completed,
        started_at: if proto_trace.started_at.is_empty() {
            None
        } else {
            Some(proto_trace.started_at.clone())
        },
        ended_at: if proto_trace.ended_at.is_empty() {
            None
        } else {
            Some(proto_trace.ended_at.clone())
        },
        final_state: if proto_trace.final_state.is_empty() {
            None
        } else {
            serde_json::from_slice(&proto_trace.final_state).ok()
        },
        metadata: proto_trace
            .metadata
            .iter()
            .filter_map(|(k, v)| serde_json::from_slice(v).ok().map(|val| (k.clone(), val)))
            .collect(),
        execution_metrics: None,
        performance_metrics: None,
    };

    // Convert node executions
    for (index, node) in proto_trace.nodes_executed.iter().enumerate() {
        let node_exec = NodeExecution {
            node: node.node.clone(),
            duration_ms: node.duration_ms,
            tokens_used: node.total_tokens,
            state_before: if node.input.is_empty() {
                None
            } else {
                serde_json::from_slice(&node.input).ok()
            },
            state_after: if node.output.is_empty() {
                None
            } else {
                serde_json::from_slice(&node.output).ok()
            },
            tools_called: vec![],
            success: node.succeeded,
            error_message: None,
            index,
            started_at: if node.started_at.is_empty() {
                None
            } else {
                Some(node.started_at.clone())
            },
            metadata: node
                .metadata
                .iter()
                .filter_map(|(k, v)| serde_json::from_str(v).ok().map(|val| (k.clone(), val)))
                .collect(),
        };
        trace.nodes_executed.push(node_exec);
    }

    // Convert errors
    for error in &proto_trace.errors {
        let error_trace = ErrorTrace {
            node: error.node.clone(),
            message: error.message.clone(),
            error_type: if error.error_code.is_empty() {
                None
            } else {
                Some(error.error_code.clone()) // Map error_code back to error_type
            },
            state_at_error: None,
            timestamp: if error.timestamp.is_empty() {
                None
            } else {
                Some(error.timestamp.clone())
            },
            execution_index: None,
            recoverable: error.recovered,
            retry_attempted: false,
            context: if error.stack_trace.is_empty() {
                None
            } else {
                Some(error.stack_trace.clone())
            },
            metadata: HashMap::new(),
        };
        trace.errors.push(error_trace);
    }

    Some(trace)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // ExecutionTrace tests
    // ============================================================================

    #[test]
    fn test_execution_trace_new() {
        let trace = ExecutionTrace::new();
        assert!(trace.nodes_executed.is_empty());
        assert!(trace.errors.is_empty());
        assert_eq!(trace.total_duration_ms, 0);
        assert_eq!(trace.total_tokens, 0);
        assert!(!trace.completed);
    }

    #[test]
    fn test_execution_trace_builder() {
        let trace = ExecutionTrace::builder()
            .thread_id("thread-123")
            .execution_id("exec-456")
            .total_duration_ms(1500)
            .total_tokens(500)
            .completed(true)
            .started_at("2025-01-01T00:00:00Z")
            .ended_at("2025-01-01T00:01:00Z")
            .metadata("key1", serde_json::json!("value1"))
            .build();

        assert_eq!(trace.thread_id, Some("thread-123".to_string()));
        assert_eq!(trace.execution_id, Some("exec-456".to_string()));
        assert_eq!(trace.total_duration_ms, 1500);
        assert_eq!(trace.total_tokens, 500);
        assert!(trace.completed);
        assert_eq!(trace.started_at, Some("2025-01-01T00:00:00Z".to_string()));
        assert_eq!(trace.ended_at, Some("2025-01-01T00:01:00Z".to_string()));
        assert_eq!(
            trace.metadata.get("key1"),
            Some(&serde_json::json!("value1"))
        );
    }

    #[test]
    fn test_execution_trace_node_count() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("node1", 100))
            .add_node_execution(NodeExecution::new("node2", 200))
            .build();

        assert_eq!(trace.node_count(), 2);
    }

    #[test]
    fn test_execution_trace_error_count() {
        let trace = ExecutionTrace::builder()
            .add_error(ErrorTrace::new("node1", "Error 1"))
            .add_error(ErrorTrace::new("node2", "Error 2"))
            .build();

        assert_eq!(trace.error_count(), 2);
        assert!(trace.has_errors());
    }

    #[test]
    fn test_execution_trace_is_successful() {
        // Completed without errors = successful
        let successful = ExecutionTrace::builder().completed(true).build();
        assert!(successful.is_successful());

        // Completed with errors = not successful
        let with_errors = ExecutionTrace::builder()
            .completed(true)
            .add_error(ErrorTrace::new("node1", "Error"))
            .build();
        assert!(!with_errors.is_successful());

        // Not completed = not successful
        let incomplete = ExecutionTrace::builder().completed(false).build();
        assert!(!incomplete.is_successful());
    }

    #[test]
    fn test_execution_trace_slowest_node() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("fast", 100))
            .add_node_execution(NodeExecution::new("slow", 500))
            .add_node_execution(NodeExecution::new("medium", 300))
            .build();

        let slowest = trace.slowest_node();
        assert!(slowest.is_some());
        assert_eq!(slowest.unwrap().node, "slow");
        assert_eq!(slowest.unwrap().duration_ms, 500);
    }

    #[test]
    fn test_execution_trace_slowest_node_empty() {
        let trace = ExecutionTrace::new();
        assert!(trace.slowest_node().is_none());
    }

    #[test]
    fn test_execution_trace_most_expensive_node() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("cheap", 100).with_tokens(50))
            .add_node_execution(NodeExecution::new("expensive", 100).with_tokens(1000))
            .add_node_execution(NodeExecution::new("medium", 100).with_tokens(300))
            .build();

        let most_expensive = trace.most_expensive_node();
        assert!(most_expensive.is_some());
        assert_eq!(most_expensive.unwrap().node, "expensive");
        assert_eq!(most_expensive.unwrap().tokens_used, 1000);
    }

    #[test]
    fn test_execution_trace_get_node_execution() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("node1", 100))
            .add_node_execution(NodeExecution::new("node2", 200))
            .build();

        assert!(trace.get_node_execution("node1").is_some());
        assert!(trace.get_node_execution("node2").is_some());
        assert!(trace.get_node_execution("nonexistent").is_none());
    }

    #[test]
    fn test_execution_trace_get_all_node_executions() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("loop_node", 100))
            .add_node_execution(NodeExecution::new("other_node", 150))
            .add_node_execution(NodeExecution::new("loop_node", 200))
            .add_node_execution(NodeExecution::new("loop_node", 300))
            .build();

        let loop_executions = trace.get_all_node_executions("loop_node");
        assert_eq!(loop_executions.len(), 3);
    }

    #[test]
    fn test_execution_trace_node_execution_count() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("loop_node", 100))
            .add_node_execution(NodeExecution::new("loop_node", 200))
            .build();

        assert_eq!(trace.node_execution_count("loop_node"), 2);
        assert_eq!(trace.node_execution_count("nonexistent"), 0);
    }

    #[test]
    fn test_execution_trace_total_time_in_node() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("loop_node", 100))
            .add_node_execution(NodeExecution::new("loop_node", 200))
            .add_node_execution(NodeExecution::new("loop_node", 300))
            .build();

        assert_eq!(trace.total_time_in_node("loop_node"), 600);
        assert_eq!(trace.total_time_in_node("nonexistent"), 0);
    }

    #[test]
    fn test_execution_trace_total_tokens_in_node() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("llm_node", 100).with_tokens(500))
            .add_node_execution(NodeExecution::new("llm_node", 200).with_tokens(300))
            .build();

        assert_eq!(trace.total_tokens_in_node("llm_node"), 800);
        assert_eq!(trace.total_tokens_in_node("nonexistent"), 0);
    }

    #[test]
    fn test_execution_trace_errors_for_node() {
        let trace = ExecutionTrace::builder()
            .add_error(ErrorTrace::new("node1", "Error 1"))
            .add_error(ErrorTrace::new("node2", "Error 2"))
            .add_error(ErrorTrace::new("node1", "Error 3"))
            .build();

        let node1_errors = trace.errors_for_node("node1");
        assert_eq!(node1_errors.len(), 2);
        assert_eq!(trace.errors_for_node("node2").len(), 1);
        assert_eq!(trace.errors_for_node("nonexistent").len(), 0);
    }

    #[test]
    fn test_execution_trace_unique_nodes() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("node1", 100))
            .add_node_execution(NodeExecution::new("node2", 100))
            .add_node_execution(NodeExecution::new("node1", 100))
            .add_node_execution(NodeExecution::new("node3", 100))
            .build();

        let unique = trace.unique_nodes();
        assert_eq!(unique, vec!["node1", "node2", "node3"]);
    }

    #[test]
    fn test_execution_trace_average_node_duration_ms() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("node1", 100))
            .add_node_execution(NodeExecution::new("node2", 200))
            .add_node_execution(NodeExecution::new("node3", 300))
            .build();

        assert!((trace.average_node_duration_ms() - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_execution_trace_average_node_duration_ms_empty() {
        let trace = ExecutionTrace::new();
        assert_eq!(trace.average_node_duration_ms(), 0.0);
    }

    #[test]
    fn test_execution_trace_time_breakdown() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("node1", 100))
            .add_node_execution(NodeExecution::new("node2", 300))
            .total_duration_ms(400)
            .build();

        let breakdown = trace.time_breakdown();
        assert!((breakdown.get("node1").unwrap() - 25.0).abs() < 0.001);
        assert!((breakdown.get("node2").unwrap() - 75.0).abs() < 0.001);
    }

    #[test]
    fn test_execution_trace_time_breakdown_zero_duration() {
        let trace = ExecutionTrace::builder()
            .total_duration_ms(0)
            .add_node_execution(NodeExecution::new("node1", 100))
            .build();

        let breakdown = trace.time_breakdown();
        assert!(breakdown.is_empty());
    }

    #[test]
    fn test_execution_trace_token_breakdown() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("llm1", 100).with_tokens(200))
            .add_node_execution(NodeExecution::new("llm2", 100).with_tokens(800))
            .total_tokens(1000)
            .build();

        let breakdown = trace.token_breakdown();
        assert!((breakdown.get("llm1").unwrap() - 20.0).abs() < 0.001);
        assert!((breakdown.get("llm2").unwrap() - 80.0).abs() < 0.001);
    }

    #[test]
    fn test_execution_trace_json_round_trip() {
        let original = ExecutionTrace::builder()
            .thread_id("thread-123")
            .execution_id("exec-456")
            .total_duration_ms(1500)
            .completed(true)
            .add_node_execution(NodeExecution::new("node1", 100).with_tokens(50))
            .add_error(ErrorTrace::new("node2", "Test error"))
            .build();

        let json = original.to_json().expect("Serialization should succeed");
        let restored = ExecutionTrace::from_json(&json).expect("Deserialization should succeed");

        assert_eq!(restored.thread_id, original.thread_id);
        assert_eq!(restored.execution_id, original.execution_id);
        assert_eq!(restored.total_duration_ms, original.total_duration_ms);
        assert_eq!(restored.completed, original.completed);
        assert_eq!(restored.nodes_executed.len(), 1);
        assert_eq!(restored.errors.len(), 1);
    }

    #[test]
    fn test_execution_trace_json_compact() {
        let trace = ExecutionTrace::builder().execution_id("exec-123").build();

        let compact = trace
            .to_json_compact()
            .expect("Compact serialization should succeed");
        assert!(!compact.contains('\n'));
    }

    // ============================================================================
    // NodeExecution tests
    // ============================================================================

    #[test]
    fn test_node_execution_new() {
        let node = NodeExecution::new("test_node", 150);
        assert_eq!(node.node, "test_node");
        assert_eq!(node.duration_ms, 150);
        assert_eq!(node.tokens_used, 0);
        assert!(node.success);
        assert!(node.tools_called.is_empty());
    }

    #[test]
    fn test_node_execution_builder_pattern() {
        let node = NodeExecution::new("test_node", 150)
            .with_tokens(500)
            .with_tool("search")
            .with_tool("calculate")
            .with_index(5)
            .with_started_at("2025-01-01T00:00:00Z")
            .with_metadata("model", serde_json::json!("gpt-4"));

        assert_eq!(node.tokens_used, 500);
        assert_eq!(node.tools_called, vec!["search", "calculate"]);
        assert_eq!(node.index, 5);
        assert_eq!(node.started_at, Some("2025-01-01T00:00:00Z".to_string()));
        assert_eq!(
            node.metadata.get("model"),
            Some(&serde_json::json!("gpt-4"))
        );
    }

    #[test]
    fn test_node_execution_with_error() {
        let node = NodeExecution::new("failing_node", 100).with_error("Connection timeout");

        assert!(!node.success);
        assert_eq!(node.error_message, Some("Connection timeout".to_string()));
    }

    #[test]
    fn test_node_execution_with_tools() {
        let node = NodeExecution::new("tool_node", 100)
            .with_tools(vec!["tool1".to_string(), "tool2".to_string()]);

        assert_eq!(node.tools_called.len(), 2);
        assert!(node.called_tools());
        assert_eq!(node.tool_count(), 2);
    }

    #[test]
    fn test_node_execution_called_tools() {
        let with_tools = NodeExecution::new("node1", 100).with_tool("search");
        let without_tools = NodeExecution::new("node2", 100);

        assert!(with_tools.called_tools());
        assert!(!without_tools.called_tools());
    }

    #[test]
    fn test_node_execution_state_changed() {
        let state_before = serde_json::json!({"counter": 0});
        let state_after = serde_json::json!({"counter": 1});

        let changed = NodeExecution::new("node1", 100)
            .with_state_before(state_before.clone())
            .with_state_after(state_after);
        assert!(changed.state_changed());

        let unchanged = NodeExecution::new("node2", 100)
            .with_state_before(state_before.clone())
            .with_state_after(state_before);
        assert!(!unchanged.state_changed());

        // Without both states, can't determine if changed
        let incomplete =
            NodeExecution::new("node3", 100).with_state_before(serde_json::json!({"counter": 0}));
        assert!(!incomplete.state_changed());
    }

    #[test]
    fn test_node_execution_changed_keys() {
        let state_before = serde_json::json!({"a": 1, "b": 2, "c": 3});
        let state_after = serde_json::json!({"a": 1, "b": 999, "d": 4}); // b changed, c removed, d added

        let node = NodeExecution::new("node1", 100)
            .with_state_before(state_before)
            .with_state_after(state_after);

        let changed_keys = node.changed_keys();
        assert!(changed_keys.contains(&"b".to_string())); // modified
        assert!(changed_keys.contains(&"c".to_string())); // removed
        assert!(changed_keys.contains(&"d".to_string())); // added
        assert!(!changed_keys.contains(&"a".to_string())); // unchanged
    }

    #[test]
    fn test_node_execution_changed_keys_non_objects() {
        let node = NodeExecution::new("node1", 100)
            .with_state_before(serde_json::json!([1, 2, 3]))
            .with_state_after(serde_json::json!([4, 5, 6]));

        // Non-object states return empty changed_keys
        assert!(node.changed_keys().is_empty());
    }

    // ============================================================================
    // ErrorTrace tests
    // ============================================================================

    #[test]
    fn test_error_trace_new() {
        let error = ErrorTrace::new("test_node", "Something went wrong");
        assert_eq!(error.node, "test_node");
        assert_eq!(error.message, "Something went wrong");
        assert!(!error.recoverable);
        assert!(!error.retry_attempted);
    }

    #[test]
    fn test_error_trace_builder_pattern() {
        let error = ErrorTrace::new("test_node", "Connection timeout")
            .with_error_type("NetworkError")
            .with_timestamp("2025-01-01T00:00:00Z")
            .with_execution_index(5)
            .recoverable()
            .with_retry_attempted()
            .with_context("at line 123: socket.connect()")
            .with_metadata("attempt", serde_json::json!(3));

        assert_eq!(error.error_type, Some("NetworkError".to_string()));
        assert_eq!(error.timestamp, Some("2025-01-01T00:00:00Z".to_string()));
        assert_eq!(error.execution_index, Some(5));
        assert!(error.recoverable);
        assert!(error.retry_attempted);
        assert_eq!(
            error.context,
            Some("at line 123: socket.connect()".to_string())
        );
        assert_eq!(error.metadata.get("attempt"), Some(&serde_json::json!(3)));
    }

    #[test]
    fn test_error_trace_with_state() {
        let state = serde_json::json!({"step": "processing", "items": 42});
        let error = ErrorTrace::new("process_node", "Failed").with_state_at_error(state.clone());

        assert_eq!(error.state_at_error, Some(state));
    }

    // ============================================================================
    // ExecutionTraceBuilder tests
    // ============================================================================

    #[test]
    fn test_execution_trace_builder_final_state() {
        let final_state = serde_json::json!({"result": "success", "count": 42});
        let trace = ExecutionTrace::builder()
            .final_state(final_state.clone())
            .build();

        assert_eq!(trace.final_state, Some(final_state));
    }

    #[test]
    fn test_execution_trace_builder_nodes_executed() {
        let nodes = vec![
            NodeExecution::new("node1", 100),
            NodeExecution::new("node2", 200),
        ];
        let trace = ExecutionTrace::builder().nodes_executed(nodes).build();

        assert_eq!(trace.nodes_executed.len(), 2);
    }

    #[test]
    fn test_execution_trace_builder_errors() {
        let errors = vec![
            ErrorTrace::new("node1", "Error 1"),
            ErrorTrace::new("node2", "Error 2"),
        ];
        let trace = ExecutionTrace::builder().errors(errors).build();

        assert_eq!(trace.errors.len(), 2);
    }
}

#[cfg(all(test, feature = "dashstream"))]
mod streaming_tests {
    use super::*;

    #[test]
    fn test_round_trip_conversion() {
        let original = ExecutionTrace {
            thread_id: Some("thread-123".to_string()),
            execution_id: Some("exec-456".to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: None,
            nodes_executed: vec![NodeExecution {
                node: "search".to_string(),
                duration_ms: 1000,
                tokens_used: 500,
                success: true,
                ..Default::default()
            }],
            total_duration_ms: 1500,
            total_tokens: 500,
            errors: vec![],
            completed: true,
            started_at: Some("2025-01-01T00:00:00Z".to_string()),
            ended_at: Some("2025-01-01T00:00:01Z".to_string()),
            final_state: None,
            metadata: HashMap::new(),
            execution_metrics: None,
            performance_metrics: None,
        };

        let msg = to_streaming_message(&original);
        let restored = from_streaming_message(&msg).expect("Should convert back");

        assert_eq!(restored.thread_id, original.thread_id);
        assert_eq!(restored.execution_id, original.execution_id);
        assert_eq!(restored.total_duration_ms, original.total_duration_ms);
        assert_eq!(restored.total_tokens, original.total_tokens);
        assert_eq!(restored.completed, original.completed);
        assert_eq!(restored.nodes_executed.len(), 1);
        assert_eq!(restored.nodes_executed[0].node, "search");
    }

    #[test]
    fn test_conversion_with_errors() {
        let original = ExecutionTrace {
            execution_id: Some("exec-789".to_string()),
            errors: vec![ErrorTrace {
                node: "fetch".to_string(),
                message: "Connection timeout".to_string(),
                recoverable: true,
                ..Default::default()
            }],
            completed: false,
            ..Default::default()
        };

        let msg = to_streaming_message(&original);
        let restored = from_streaming_message(&msg).expect("Should convert back");

        assert!(!restored.completed);
        assert_eq!(restored.errors.len(), 1);
        assert_eq!(restored.errors[0].node, "fetch");
        assert_eq!(restored.errors[0].message, "Connection timeout");
        assert!(restored.errors[0].recoverable);
    }
}
