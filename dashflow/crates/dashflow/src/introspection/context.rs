//! Runtime Execution Context
//!
//! This module provides the [`ExecutionContext`] type for AI agents to understand
//! where they are during execution.

use serde::{Deserialize, Serialize};

// Runtime Execution Context
// ============================================================================

/// Execution context - current state of graph execution.
///
/// This struct provides AI agents with awareness of their current execution state,
/// enabling them to make informed decisions based on where they are in the graph
/// and what has happened so far.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::introspection::ExecutionContext;
///
/// async fn reasoning_node(state: State, context: ExecutionContext) -> Result<State> {
///     // AI can ask: "How many iterations have I done?"
///     if context.iteration > 10 {
///         return Err(Error::TooManyIterations);
///     }
///
///     // AI can ask: "What are my next options?"
///     let next_nodes = &context.available_next_nodes;
///
///     // AI can ask: "What have I already done?"
///     let history = &context.nodes_executed;
///
///     // AI can detect loops
///     if context.execution_count("tool_call") > 5 {
///         // Probably stuck in a loop
///     }
///
///     Ok(state)
/// }
/// ```
///
/// # Errors
///
/// - [`serde_json::Error`] - Returned by [`ExecutionContext::to_json`] and
///   [`ExecutionContext::from_json`] on
///   serialization/deserialization failure
///
/// # See Also
///
/// - [`crate::GraphManifest`] - Static graph structure information
/// - [`crate::ExecutionTrace`] - Complete execution history after completion
/// - [`CompiledGraph`](crate::CompiledGraph) - The compiled graph being executed
/// - [`ExecutionContextBuilder`] - Builder for creating contexts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Name of the node currently being executed
    pub current_node: String,
    /// Current iteration count (starts at 1, increments each execution step)
    pub iteration: u32,
    /// List of nodes already executed in this run (in execution order)
    pub nodes_executed: Vec<String>,
    /// Nodes that could be executed next (based on edges from current node)
    pub available_next_nodes: Vec<String>,
    /// Snapshot of the current state as JSON (for introspection)
    pub state_snapshot: Option<serde_json::Value>,
    /// Thread ID if using checkpointing
    pub thread_id: Option<String>,
    /// Whether execution is currently paused for human-in-the-loop
    pub is_interrupted: bool,
    /// Maximum iterations before recursion limit (0 = unlimited)
    pub recursion_limit: u32,
    /// Execution start timestamp (ISO 8601 string)
    pub started_at: Option<String>,
    /// Total execution duration so far in milliseconds
    pub elapsed_ms: Option<u64>,
}

impl ExecutionContext {
    /// Create a new execution context builder
    #[must_use]
    pub fn builder() -> ExecutionContextBuilder {
        ExecutionContextBuilder::new()
    }

    /// Create a basic execution context with required fields
    #[must_use]
    pub fn new(current_node: impl Into<String>, iteration: u32) -> Self {
        Self {
            current_node: current_node.into(),
            iteration,
            ..Default::default()
        }
    }

    /// Convert context to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert context to compact JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse context from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Check if this is the first iteration
    #[must_use]
    pub fn is_first_iteration(&self) -> bool {
        self.iteration == 1
    }

    /// Check if we're approaching the recursion limit
    ///
    /// Returns true if within 20% of the limit (or limit is 0 meaning unlimited)
    #[must_use]
    pub fn is_near_limit(&self) -> bool {
        if self.recursion_limit == 0 {
            return false;
        }
        let threshold = (self.recursion_limit as f32 * 0.8) as u32;
        self.iteration >= threshold
    }

    /// Get remaining iterations before recursion limit
    ///
    /// Returns None if recursion limit is 0 (unlimited)
    #[must_use]
    pub fn remaining_iterations(&self) -> Option<u32> {
        if self.recursion_limit == 0 {
            None
        } else {
            Some(self.recursion_limit.saturating_sub(self.iteration))
        }
    }

    /// Check if a specific node has been executed
    #[must_use]
    pub fn has_executed(&self, node_name: &str) -> bool {
        self.nodes_executed.iter().any(|n| n == node_name)
    }

    /// Count how many times a node has been executed
    #[must_use]
    pub fn execution_count(&self, node_name: &str) -> usize {
        self.nodes_executed
            .iter()
            .filter(|n| *n == node_name)
            .count()
    }

    /// Check if execution can proceed to a specific node
    #[must_use]
    pub fn can_go_to(&self, node_name: &str) -> bool {
        self.available_next_nodes.iter().any(|n| n == node_name)
    }

    /// Get the last N executed nodes
    #[must_use]
    pub fn recent_history(&self, n: usize) -> Vec<&str> {
        self.nodes_executed
            .iter()
            .rev()
            .take(n)
            .map(String::as_str)
            .collect()
    }

    /// Check if we're in a loop (same node executed multiple times recently)
    ///
    /// Returns the node name if a loop is detected within the last `window_size` executions
    #[must_use]
    pub fn detect_loop(&self, window_size: usize) -> Option<&str> {
        let recent: Vec<_> = self.nodes_executed.iter().rev().take(window_size).collect();
        if recent.len() < 2 {
            return None;
        }

        // Check if any node appears more than once in the window
        for (i, node) in recent.iter().enumerate() {
            if recent.iter().skip(i + 1).any(|n| n == node) {
                return Some(node);
            }
        }
        None
    }
}

/// Builder for creating execution contexts
#[derive(Debug, Default)]
pub struct ExecutionContextBuilder {
    current_node: Option<String>,
    iteration: u32,
    nodes_executed: Vec<String>,
    available_next_nodes: Vec<String>,
    state_snapshot: Option<serde_json::Value>,
    thread_id: Option<String>,
    is_interrupted: bool,
    recursion_limit: u32,
    started_at: Option<String>,
    elapsed_ms: Option<u64>,
}

impl ExecutionContextBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set current node name
    #[must_use]
    pub fn current_node(mut self, node: impl Into<String>) -> Self {
        self.current_node = Some(node.into());
        self
    }

    /// Set iteration count
    #[must_use]
    pub fn iteration(mut self, iteration: u32) -> Self {
        self.iteration = iteration;
        self
    }

    /// Set nodes already executed
    #[must_use]
    pub fn nodes_executed(mut self, nodes: Vec<String>) -> Self {
        self.nodes_executed = nodes;
        self
    }

    /// Add a single executed node
    #[must_use]
    pub fn add_executed_node(mut self, node: impl Into<String>) -> Self {
        self.nodes_executed.push(node.into());
        self
    }

    /// Set available next nodes
    #[must_use]
    pub fn available_next_nodes(mut self, nodes: Vec<String>) -> Self {
        self.available_next_nodes = nodes;
        self
    }

    /// Set state snapshot
    #[must_use]
    pub fn state_snapshot(mut self, snapshot: serde_json::Value) -> Self {
        self.state_snapshot = Some(snapshot);
        self
    }

    /// Set thread ID
    #[must_use]
    pub fn thread_id(mut self, id: impl Into<String>) -> Self {
        self.thread_id = Some(id.into());
        self
    }

    /// Set interrupted state
    #[must_use]
    pub fn is_interrupted(mut self, interrupted: bool) -> Self {
        self.is_interrupted = interrupted;
        self
    }

    /// Set recursion limit
    #[must_use]
    pub fn recursion_limit(mut self, limit: u32) -> Self {
        self.recursion_limit = limit;
        self
    }

    /// Set start timestamp
    #[must_use]
    pub fn started_at(mut self, timestamp: impl Into<String>) -> Self {
        self.started_at = Some(timestamp.into());
        self
    }

    /// Set elapsed milliseconds
    #[must_use]
    pub fn elapsed_ms(mut self, ms: u64) -> Self {
        self.elapsed_ms = Some(ms);
        self
    }

    /// Build the execution context
    ///
    /// # Errors
    ///
    /// Returns error if current_node is not set
    pub fn build(self) -> Result<ExecutionContext, &'static str> {
        Ok(ExecutionContext {
            current_node: self.current_node.ok_or("current_node is required")?,
            iteration: self.iteration,
            nodes_executed: self.nodes_executed,
            available_next_nodes: self.available_next_nodes,
            state_snapshot: self.state_snapshot,
            thread_id: self.thread_id,
            is_interrupted: self.is_interrupted,
            recursion_limit: self.recursion_limit,
            started_at: self.started_at,
            elapsed_ms: self.elapsed_ms,
        })
    }
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // ExecutionContext Tests
    // ========================================================================

    #[test]
    fn test_execution_context_new() {
        let ctx = ExecutionContext::new("node_a", 1);
        assert_eq!(ctx.current_node, "node_a");
        assert_eq!(ctx.iteration, 1);
        assert!(ctx.nodes_executed.is_empty());
        assert!(ctx.available_next_nodes.is_empty());
        assert!(ctx.state_snapshot.is_none());
        assert!(ctx.thread_id.is_none());
        assert!(!ctx.is_interrupted);
        assert_eq!(ctx.recursion_limit, 0);
    }

    #[test]
    fn test_execution_context_default() {
        let ctx = ExecutionContext::default();
        assert!(ctx.current_node.is_empty());
        assert_eq!(ctx.iteration, 0);
        assert!(!ctx.is_interrupted);
    }

    #[test]
    fn test_execution_context_builder_basic() {
        let ctx = ExecutionContext::builder()
            .current_node("start")
            .build()
            .unwrap();

        assert_eq!(ctx.current_node, "start");
    }

    #[test]
    fn test_execution_context_builder_missing_node() {
        let result = ExecutionContext::builder().build();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "current_node is required");
    }

    #[test]
    fn test_execution_context_builder_iteration() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(5)
            .build()
            .unwrap();

        assert_eq!(ctx.iteration, 5);
    }

    #[test]
    fn test_execution_context_builder_nodes_executed() {
        let ctx = ExecutionContext::builder()
            .current_node("node_c")
            .nodes_executed(vec!["node_a".to_string(), "node_b".to_string()])
            .build()
            .unwrap();

        assert_eq!(ctx.nodes_executed.len(), 2);
        assert_eq!(ctx.nodes_executed[0], "node_a");
        assert_eq!(ctx.nodes_executed[1], "node_b");
    }

    #[test]
    fn test_execution_context_builder_add_executed_node() {
        let ctx = ExecutionContext::builder()
            .current_node("node_c")
            .add_executed_node("node_a")
            .add_executed_node("node_b")
            .build()
            .unwrap();

        assert_eq!(ctx.nodes_executed.len(), 2);
    }

    #[test]
    fn test_execution_context_builder_available_next_nodes() {
        let ctx = ExecutionContext::builder()
            .current_node("node_a")
            .available_next_nodes(vec!["node_b".to_string(), "node_c".to_string()])
            .build()
            .unwrap();

        assert_eq!(ctx.available_next_nodes.len(), 2);
    }

    #[test]
    fn test_execution_context_builder_state_snapshot() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .state_snapshot(json!({"key": "value"}))
            .build()
            .unwrap();

        assert!(ctx.state_snapshot.is_some());
        assert_eq!(ctx.state_snapshot.unwrap(), json!({"key": "value"}));
    }

    #[test]
    fn test_execution_context_builder_thread_id() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .thread_id("thread-123")
            .build()
            .unwrap();

        assert_eq!(ctx.thread_id, Some("thread-123".to_string()));
    }

    #[test]
    fn test_execution_context_builder_is_interrupted() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .is_interrupted(true)
            .build()
            .unwrap();

        assert!(ctx.is_interrupted);
    }

    #[test]
    fn test_execution_context_builder_recursion_limit() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .recursion_limit(100)
            .build()
            .unwrap();

        assert_eq!(ctx.recursion_limit, 100);
    }

    #[test]
    fn test_execution_context_builder_started_at() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .started_at("2024-01-01T00:00:00Z")
            .build()
            .unwrap();

        assert_eq!(ctx.started_at, Some("2024-01-01T00:00:00Z".to_string()));
    }

    #[test]
    fn test_execution_context_builder_elapsed_ms() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .elapsed_ms(5000)
            .build()
            .unwrap();

        assert_eq!(ctx.elapsed_ms, Some(5000));
    }

    #[test]
    fn test_execution_context_to_json() {
        let ctx = ExecutionContext::new("node_a", 1);
        let json = ctx.to_json().unwrap();

        assert!(json.contains("current_node"));
        assert!(json.contains("node_a"));
        assert!(json.contains("iteration"));
    }

    #[test]
    fn test_execution_context_to_json_compact() {
        let ctx = ExecutionContext::new("node_a", 1);
        let json = ctx.to_json_compact().unwrap();

        assert!(!json.contains('\n'));
        assert!(json.contains("node_a"));
    }

    #[test]
    fn test_execution_context_from_json() {
        let json = r#"{"current_node":"test","iteration":3,"nodes_executed":[],"available_next_nodes":[],"state_snapshot":null,"thread_id":null,"is_interrupted":false,"recursion_limit":0,"started_at":null,"elapsed_ms":null}"#;
        let ctx = ExecutionContext::from_json(json).unwrap();

        assert_eq!(ctx.current_node, "test");
        assert_eq!(ctx.iteration, 3);
    }

    #[test]
    fn test_execution_context_json_roundtrip() {
        let ctx = ExecutionContext::builder()
            .current_node("processing")
            .iteration(5)
            .nodes_executed(vec!["start".to_string(), "validate".to_string()])
            .available_next_nodes(vec!["complete".to_string(), "retry".to_string()])
            .state_snapshot(json!({"count": 42}))
            .thread_id("thread-abc")
            .is_interrupted(false)
            .recursion_limit(50)
            .started_at("2024-01-01T12:00:00Z")
            .elapsed_ms(1500)
            .build()
            .unwrap();

        let json = ctx.to_json().unwrap();
        let restored = ExecutionContext::from_json(&json).unwrap();

        assert_eq!(ctx.current_node, restored.current_node);
        assert_eq!(ctx.iteration, restored.iteration);
        assert_eq!(ctx.nodes_executed.len(), restored.nodes_executed.len());
        assert_eq!(ctx.thread_id, restored.thread_id);
        assert_eq!(ctx.recursion_limit, restored.recursion_limit);
    }

    #[test]
    fn test_execution_context_is_first_iteration() {
        let first = ExecutionContext::new("node", 1);
        assert!(first.is_first_iteration());

        let second = ExecutionContext::new("node", 2);
        assert!(!second.is_first_iteration());

        let zero = ExecutionContext::new("node", 0);
        assert!(!zero.is_first_iteration());
    }

    #[test]
    fn test_execution_context_is_near_limit_unlimited() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(1000)
            .recursion_limit(0) // unlimited
            .build()
            .unwrap();

        assert!(!ctx.is_near_limit());
    }

    #[test]
    fn test_execution_context_is_near_limit_not_near() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(5)
            .recursion_limit(100)
            .build()
            .unwrap();

        assert!(!ctx.is_near_limit()); // 5 < 80 (80% of 100)
    }

    #[test]
    fn test_execution_context_is_near_limit_near() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(85)
            .recursion_limit(100)
            .build()
            .unwrap();

        assert!(ctx.is_near_limit()); // 85 >= 80 (80% of 100)
    }

    #[test]
    fn test_execution_context_is_near_limit_at_limit() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(100)
            .recursion_limit(100)
            .build()
            .unwrap();

        assert!(ctx.is_near_limit());
    }

    #[test]
    fn test_execution_context_remaining_iterations_unlimited() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(50)
            .recursion_limit(0)
            .build()
            .unwrap();

        assert!(ctx.remaining_iterations().is_none());
    }

    #[test]
    fn test_execution_context_remaining_iterations_with_limit() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(30)
            .recursion_limit(100)
            .build()
            .unwrap();

        assert_eq!(ctx.remaining_iterations(), Some(70));
    }

    #[test]
    fn test_execution_context_remaining_iterations_at_limit() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(100)
            .recursion_limit(100)
            .build()
            .unwrap();

        assert_eq!(ctx.remaining_iterations(), Some(0));
    }

    #[test]
    fn test_execution_context_remaining_iterations_over_limit() {
        let ctx = ExecutionContext::builder()
            .current_node("node")
            .iteration(150)
            .recursion_limit(100)
            .build()
            .unwrap();

        // saturating_sub should return 0
        assert_eq!(ctx.remaining_iterations(), Some(0));
    }

    #[test]
    fn test_execution_context_has_executed() {
        let ctx = ExecutionContext::builder()
            .current_node("node_c")
            .nodes_executed(vec!["node_a".to_string(), "node_b".to_string()])
            .build()
            .unwrap();

        assert!(ctx.has_executed("node_a"));
        assert!(ctx.has_executed("node_b"));
        assert!(!ctx.has_executed("node_c"));
        assert!(!ctx.has_executed("nonexistent"));
    }

    #[test]
    fn test_execution_context_execution_count() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .nodes_executed(vec![
                "tool_call".to_string(),
                "reasoning".to_string(),
                "tool_call".to_string(),
                "tool_call".to_string(),
                "validate".to_string(),
            ])
            .build()
            .unwrap();

        assert_eq!(ctx.execution_count("tool_call"), 3);
        assert_eq!(ctx.execution_count("reasoning"), 1);
        assert_eq!(ctx.execution_count("validate"), 1);
        assert_eq!(ctx.execution_count("nonexistent"), 0);
    }

    #[test]
    fn test_execution_context_can_go_to() {
        let ctx = ExecutionContext::builder()
            .current_node("decision")
            .available_next_nodes(vec![
                "approve".to_string(),
                "reject".to_string(),
                "escalate".to_string(),
            ])
            .build()
            .unwrap();

        assert!(ctx.can_go_to("approve"));
        assert!(ctx.can_go_to("reject"));
        assert!(ctx.can_go_to("escalate"));
        assert!(!ctx.can_go_to("decision"));
        assert!(!ctx.can_go_to("nonexistent"));
    }

    #[test]
    fn test_execution_context_recent_history() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .nodes_executed(vec![
                "node_a".to_string(),
                "node_b".to_string(),
                "node_c".to_string(),
                "node_d".to_string(),
                "node_e".to_string(),
            ])
            .build()
            .unwrap();

        let history = ctx.recent_history(3);
        assert_eq!(history.len(), 3);
        // Recent history is in reverse order (most recent first)
        assert_eq!(history[0], "node_e");
        assert_eq!(history[1], "node_d");
        assert_eq!(history[2], "node_c");
    }

    #[test]
    fn test_execution_context_recent_history_more_than_available() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .nodes_executed(vec!["node_a".to_string(), "node_b".to_string()])
            .build()
            .unwrap();

        let history = ctx.recent_history(10);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_execution_context_recent_history_empty() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .build()
            .unwrap();

        let history = ctx.recent_history(5);
        assert!(history.is_empty());
    }

    #[test]
    fn test_execution_context_detect_loop_no_loop() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .nodes_executed(vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
            ])
            .build()
            .unwrap();

        assert!(ctx.detect_loop(4).is_none());
    }

    #[test]
    fn test_execution_context_detect_loop_found() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .nodes_executed(vec![
                "a".to_string(),
                "tool_call".to_string(),
                "b".to_string(),
                "tool_call".to_string(),
            ])
            .build()
            .unwrap();

        let loop_node = ctx.detect_loop(4);
        assert!(loop_node.is_some());
        assert_eq!(loop_node.unwrap(), "tool_call");
    }

    #[test]
    fn test_execution_context_detect_loop_outside_window() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .nodes_executed(vec![
                "repeated".to_string(),
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "repeated".to_string(),
            ])
            .build()
            .unwrap();

        // Window of 3 should not detect the loop
        assert!(ctx.detect_loop(3).is_none());

        // Window of 6 should detect it
        assert!(ctx.detect_loop(6).is_some());
    }

    #[test]
    fn test_execution_context_detect_loop_empty() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .build()
            .unwrap();

        assert!(ctx.detect_loop(5).is_none());
    }

    #[test]
    fn test_execution_context_detect_loop_single_node() {
        let ctx = ExecutionContext::builder()
            .current_node("current")
            .nodes_executed(vec!["only_one".to_string()])
            .build()
            .unwrap();

        assert!(ctx.detect_loop(5).is_none());
    }

    // ========================================================================
    // Builder Chaining Tests
    // ========================================================================

    #[test]
    fn test_execution_context_builder_full_chain() {
        let ctx = ExecutionContext::builder()
            .current_node("process")
            .iteration(10)
            .nodes_executed(vec!["start".to_string()])
            .add_executed_node("validate")
            .add_executed_node("transform")
            .available_next_nodes(vec!["complete".to_string()])
            .state_snapshot(json!({"processed": true}))
            .thread_id("main-thread")
            .is_interrupted(false)
            .recursion_limit(25)
            .started_at("2024-06-15T08:30:00Z")
            .elapsed_ms(3500)
            .build()
            .unwrap();

        assert_eq!(ctx.current_node, "process");
        assert_eq!(ctx.iteration, 10);
        assert_eq!(ctx.nodes_executed.len(), 3);
        assert_eq!(ctx.available_next_nodes.len(), 1);
        assert!(ctx.state_snapshot.is_some());
        assert_eq!(ctx.thread_id, Some("main-thread".to_string()));
        assert!(!ctx.is_interrupted);
        assert_eq!(ctx.recursion_limit, 25);
        assert!(ctx.started_at.is_some());
        assert_eq!(ctx.elapsed_ms, Some(3500));
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_execution_context_empty_node_name() {
        let ctx = ExecutionContext::new("", 1);
        assert!(ctx.current_node.is_empty());
    }

    #[test]
    fn test_execution_context_unicode_node_name() {
        let ctx = ExecutionContext::new("节点_α_🚀", 1);
        assert_eq!(ctx.current_node, "节点_α_🚀");

        let json = ctx.to_json().unwrap();
        let restored = ExecutionContext::from_json(&json).unwrap();
        assert_eq!(restored.current_node, "节点_α_🚀");
    }

    #[test]
    fn test_execution_context_large_iteration() {
        let ctx = ExecutionContext::new("node", u32::MAX);
        assert_eq!(ctx.iteration, u32::MAX);
    }

    #[test]
    fn test_execution_context_complex_state_snapshot() {
        let complex_state = json!({
            "messages": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi there!"}
            ],
            "metadata": {
                "tokens_used": 150,
                "model": "gpt-4"
            },
            "nested": {
                "deep": {
                    "value": true
                }
            }
        });

        let ctx = ExecutionContext::builder()
            .current_node("chat")
            .state_snapshot(complex_state.clone())
            .build()
            .unwrap();

        assert_eq!(ctx.state_snapshot, Some(complex_state));
    }

    #[test]
    fn test_execution_context_serialization_preserves_all_fields() {
        let ctx = ExecutionContext {
            current_node: "test_node".to_string(),
            iteration: 42,
            nodes_executed: vec!["a".to_string(), "b".to_string()],
            available_next_nodes: vec!["c".to_string()],
            state_snapshot: Some(json!({"key": "value"})),
            thread_id: Some("thread-xyz".to_string()),
            is_interrupted: true,
            recursion_limit: 100,
            started_at: Some("2024-01-01T00:00:00Z".to_string()),
            elapsed_ms: Some(12345),
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let restored: ExecutionContext = serde_json::from_str(&json).unwrap();

        assert_eq!(ctx.current_node, restored.current_node);
        assert_eq!(ctx.iteration, restored.iteration);
        assert_eq!(ctx.nodes_executed, restored.nodes_executed);
        assert_eq!(ctx.available_next_nodes, restored.available_next_nodes);
        assert_eq!(ctx.state_snapshot, restored.state_snapshot);
        assert_eq!(ctx.thread_id, restored.thread_id);
        assert_eq!(ctx.is_interrupted, restored.is_interrupted);
        assert_eq!(ctx.recursion_limit, restored.recursion_limit);
        assert_eq!(ctx.started_at, restored.started_at);
        assert_eq!(ctx.elapsed_ms, restored.elapsed_ms);
    }
}
