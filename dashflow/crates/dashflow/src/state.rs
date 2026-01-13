// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph state management
//!
//! State is the data that flows through the graph, being transformed
//! by each node. State must be Clone + Send + Sync to enable graph execution.

use serde::{Deserialize, Serialize};

/// Trait for graph state
///
/// State types must implement this trait to be used in a `StateGraph`.
/// The trait is automatically implemented for types that implement
/// Clone + Send + Sync + Serialize + Deserialize.
///
/// # Example
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use dashflow::GraphState;
///
/// #[derive(Clone, Serialize, Deserialize)]
/// struct MyState {
///     messages: Vec<String>,
///     iteration: u32,
/// }
///
/// // GraphState is automatically implemented
/// ```
pub trait GraphState:
    Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static
{
}

// Blanket implementation for any type that meets the requirements
impl<T> GraphState for T where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static
{
}

/// Trait for mergeable graph state (optional)
///
/// This trait enables custom state aggregation when multiple nodes execute
/// in parallel. By default, parallel execution uses "last-write-wins" strategy,
/// which can cause data loss when parallel branches modify different parts of state.
///
/// Implement this trait to define custom merge logic.
///
/// # Quick Start: Use Derive Macro (Recommended)
///
/// For simple cases, use `#[derive(MergeableState)]` to auto-generate merge logic:
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use dashflow::{DeriveMergeableState, MergeableState};
///
/// #[derive(Clone, Serialize, Deserialize, DeriveMergeableState)]
/// struct SimpleState {
///     findings: Vec<String>,  // Auto-extends
///     max_score: i32,         // Auto-takes max
///     summary: String,        // Auto-concatenates with newline
/// }
/// ```
///
/// The derive macro automatically generates merge logic based on field types:
/// - `Vec<T>`: Extends with other's elements
/// - `Option<T>`: Takes other if self is None
/// - Numeric types (`i32`, `u32`, `i64`, `u64`, `f32`, `f64`, `usize`): Takes max value
/// - `String`: Concatenates with newline separator
/// - Other types: Keeps self's value (safe default)
///
/// # When to Use Manual Implementation
///
/// Use manual implementation when you need:
/// - Custom merge logic (e.g., `HashMap` merging, weighted averages)
/// - Business logic in merge (e.g., validation, transformation)
/// - Complex field interactions
///
/// ```rust
/// use std::collections::HashMap;
/// use serde::{Deserialize, Serialize};
/// use dashflow::MergeableState;
///
/// #[derive(Clone, Serialize, Deserialize)]
/// struct ComplexState {
///     metadata: HashMap<String, String>,
///     scores: Vec<f64>,
/// }
///
/// impl MergeableState for ComplexState {
///     fn merge(&mut self, other: &Self) {
///         // Custom HashMap merge logic
///         for (key, value) in &other.metadata {
///             self.metadata.insert(key.clone(), value.clone());
///         }
///         // Custom score aggregation
///         self.scores.extend(other.scores.iter());
///     }
/// }
/// ```
///
/// # Default Behavior (No `MergeableState`)
///
/// Without implementing `MergeableState`, parallel execution will use the LAST
/// node's state, discarding changes from earlier parallel nodes:
///
/// ```text
/// Parallel execution of nodes A and B:
/// - Node A returns: { findings: [F1, F2], insights: [] }
/// - Node B returns: { findings: [], insights: [I1, I2] }
/// Final state: { findings: [], insights: [I1, I2] }  ← A's findings LOST
/// ```
///
/// # With `MergeableState`
///
/// Implementing `MergeableState` allows aggregating changes from all parallel branches:
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use dashflow::state::{GraphState, MergeableState};
///
/// #[derive(Clone, Serialize, Deserialize, Default)]
/// struct ResearchState {
///     findings: Vec<String>,
///     insights: Vec<String>,
/// }
///
/// impl MergeableState for ResearchState {
///     fn merge(&mut self, other: &Self) {
///         // Aggregate findings and insights from parallel branches
///         self.findings.extend(other.findings.clone());
///         self.insights.extend(other.insights.clone());
///     }
/// }
/// ```
///
/// # Usage
///
/// **Framework Integration:** The framework provides two methods for merging parallel results:
///
/// 1. **Automatic merging (recommended):**
/// ```text
/// graph.merge_with_mergeable(states) - Only available when S: MergeableState
/// Automatically calls your merge() implementation
/// ```
///
/// 2. **Custom merging (for any state type):**
/// ```text
/// graph.merge_parallel_custom(states, |base, other| {
///     base.findings.extend(other.findings.clone());
/// })
/// ```
///
/// **Note:** The default parallel execution still uses last-write-wins for backward compatibility.
/// To use proper state merging, explicitly call one of the merge methods after parallel execution,
/// or use an aggregator node that calls them
///
/// # Example: Parallel Agent Coordination
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use dashflow::state::{GraphState, MergeableState};
///
/// #[derive(Clone, Serialize, Deserialize, Default)]
/// struct MultiAgentState {
///     // Each field can be contributed by different agents
///     research_findings: Vec<String>,
///     analysis_insights: Vec<String>,
///     errors: Vec<String>,
/// }
///
/// impl MergeableState for MultiAgentState {
///     fn merge(&mut self, other: &Self) {
///         // Combine results from all parallel agents
///         self.research_findings.extend(other.research_findings.clone());
///         self.analysis_insights.extend(other.analysis_insights.clone());
///         self.errors.extend(other.errors.clone());
///     }
/// }
/// ```
///
/// Now when researcher and analyst run in parallel, their outputs are combined
/// rather than one overwriting the other.
///
/// # Complete Graph Example
///
/// ```rust,ignore
/// use dashflow::graph::StateGraph;
///
/// let mut graph = StateGraph::new();
/// graph.add_node("researcher", researcher_node);
/// graph.add_node("analyst", analyst_node);
///
/// // Parallel execution
/// graph.add_parallel_edges("start", vec!["researcher", "analyst"]);
///
/// // Option 1: Automatic merge (recommended for MergeableState types)
/// graph.add_node("aggregate", |state: MultiAgentState| {
///     Box::pin(async move {
///         // State is automatically merged when control flow reconverges
///         // Or explicitly call: graph.merge_with_mergeable(states)
///         Ok(state)
///     })
/// });
///
/// // Option 2: Custom merge strategy (any state type)
/// graph.add_node("aggregate", |state: MyState| {
///     Box::pin(async move {
///         // graph.merge_parallel_custom(states, |base, other| {
///         //     base.custom_field = custom_merge_logic(base, other);
///         // })
///         Ok(state)
///     })
/// });
/// ```
///
/// # See Also
///
/// - [`GraphState`] - Base trait required for all graph states
/// - [`DeriveMergeableState`](crate::DeriveMergeableState) - Derive macro for automatic merge logic
/// - [`StateGraph::add_parallel_edges`](crate::StateGraph::add_parallel_edges) - Create parallel execution paths
/// - [`ParallelEdge`](crate::edge::ParallelEdge) - Edge type for fan-out execution
pub trait MergeableState: GraphState {
    /// Merge another state into this one
    ///
    /// Called during parallel execution to aggregate state from multiple branches.
    /// Implement this to define how states should be combined.
    ///
    /// # Arguments
    /// * `other` - The state from another parallel branch to merge into self
    ///
    /// # Example: Appending Lists
    /// ```rust
    /// # use serde::{Deserialize, Serialize};
    /// # use dashflow::state::{GraphState, MergeableState};
    /// # #[derive(Clone, Serialize, Deserialize, Default)]
    /// # struct State { items: Vec<String> }
    /// impl MergeableState for State {
    ///     fn merge(&mut self, other: &Self) {
    ///         self.items.extend(other.items.clone());
    ///     }
    /// }
    /// ```
    ///
    /// # Example: Taking Maximum Value
    /// ```rust
    /// # use serde::{Deserialize, Serialize};
    /// # use dashflow::state::{GraphState, MergeableState};
    /// # #[derive(Clone, Serialize, Deserialize, Default)]
    /// # struct State { score: i32 }
    /// impl MergeableState for State {
    ///     fn merge(&mut self, other: &Self) {
    ///         self.score = self.score.max(other.score);
    ///     }
    /// }
    /// ```
    fn merge(&mut self, other: &Self);
}

/// Example state type for multi-agent workflows
///
/// This demonstrates a typical state structure for agent graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Messages exchanged between agents
    pub messages: Vec<String>,
    /// Current iteration count
    pub iteration: u32,
    /// Next node to execute (for conditional routing)
    pub next: Option<String>,
    /// Arbitrary metadata
    pub metadata: serde_json::Value,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            iteration: 0,
            next: None,
            metadata: serde_json::Value::Null,
        }
    }
}

impl MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        // Append messages from parallel branches
        self.messages.extend(other.messages.clone());
        // Take maximum iteration
        self.iteration = self.iteration.max(other.iteration);
        // Note: `next` and `metadata` keep self's values (not merged from other).
        // This is intentional - routing decisions and metadata are branch-specific.
        // For custom merge behavior, implement MergeableState manually.
    }
}

impl AgentState {
    /// Create a new agent state
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a message to the state
    pub fn add_message(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
    }

    /// Increment the iteration counter
    pub fn increment_iteration(&mut self) {
        self.iteration += 1;
    }

    /// Set the next node to execute
    pub fn set_next(&mut self, next: impl Into<String>) {
        self.next = Some(next.into());
    }
}

// ============================================================================
// Dynamic JSON State
// ============================================================================

/// Dynamic state type for CLI tools and rapid prototyping
///
/// `JsonState` wraps a `serde_json::Value` and implements `GraphState`, enabling
/// dynamic graph execution without compile-time type definitions. This is useful for:
///
/// - CLI tools that work with arbitrary JSON data
/// - Rapid prototyping before defining typed state structs
/// - Testing and experimentation
/// - Loading training data from JSONL files
///
/// # Example
///
/// ```rust
/// use dashflow::state::JsonState;
/// use serde_json::json;
///
/// // Create from JSON value
/// let state = JsonState::from(json!({
///     "question": "What is the capital of France?",
///     "answer": "Paris"
/// }));
///
/// // Access fields
/// assert_eq!(state.get("question").and_then(|v| v.as_str()), Some("What is the capital of France?"));
///
/// // Create from JSONL line
/// let state: JsonState = r#"{"input": "test", "output": "result"}"#.parse().unwrap();
/// ```
///
/// # Field Access
///
/// `JsonState` provides convenient field access methods:
///
/// ```rust
/// use dashflow::state::JsonState;
/// use serde_json::json;
///
/// let state = JsonState::from(json!({
///     "name": "Alice",
///     "age": 30,
///     "active": true
/// }));
///
/// // Type-safe field access
/// assert_eq!(state.get_str("name"), Some("Alice"));
/// assert_eq!(state.get_i64("age"), Some(30));
/// assert_eq!(state.get_bool("active"), Some(true));
/// ```
///
/// # Optimization Compatibility
///
/// `JsonState` works with optimization commands for simple evaluation tasks.
/// For production optimization, define typed state structs for better performance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JsonState {
    /// The underlying JSON data
    #[serde(flatten)]
    data: serde_json::Value,
}

impl JsonState {
    /// Create an empty JSON state
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Create from a JSON object
    #[must_use]
    pub fn from_object(obj: serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            data: serde_json::Value::Object(obj),
        }
    }

    /// Get the underlying JSON value
    #[must_use]
    pub fn as_value(&self) -> &serde_json::Value {
        &self.data
    }

    /// Get a mutable reference to the underlying JSON value
    pub fn as_value_mut(&mut self) -> &mut serde_json::Value {
        &mut self.data
    }

    /// Consume and return the underlying JSON value
    #[must_use]
    pub fn into_value(self) -> serde_json::Value {
        self.data
    }

    /// Get a field from the state
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Get a mutable reference to a field
    pub fn get_mut(&mut self, key: &str) -> Option<&mut serde_json::Value> {
        self.data.get_mut(key)
    }

    /// Set a field in the state
    pub fn set(&mut self, key: impl Into<String>, value: serde_json::Value) {
        if let serde_json::Value::Object(ref mut map) = self.data {
            map.insert(key.into(), value);
        }
    }

    /// Remove a field from the state
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        if let serde_json::Value::Object(ref mut map) = self.data {
            map.remove(key)
        } else {
            None
        }
    }

    /// Check if a field exists
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        if let serde_json::Value::Object(map) = &self.data {
            map.contains_key(key)
        } else {
            false
        }
    }

    /// Get a string field
    #[must_use]
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    /// Get an i64 field
    #[must_use]
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    /// Get an f64 field
    #[must_use]
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_f64())
    }

    /// Get a bool field
    #[must_use]
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    /// Get an array field
    #[must_use]
    pub fn get_array(&self, key: &str) -> Option<&Vec<serde_json::Value>> {
        self.get(key).and_then(|v| v.as_array())
    }

    /// Get an object field
    #[must_use]
    pub fn get_object(&self, key: &str) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.get(key).and_then(|v| v.as_object())
    }

    /// Iterate over fields (returns empty iterator if not an object)
    pub fn iter(&self) -> JsonStateIter<'_> {
        JsonStateIter {
            inner: match &self.data {
                serde_json::Value::Object(map) => Some(map.iter()),
                _ => None,
            },
        }
    }

    /// Get the number of fields
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.data {
            serde_json::Value::Object(map) => map.len(),
            _ => 0,
        }
    }

    /// Check if the state is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match &self.data {
            serde_json::Value::Object(map) => map.is_empty(),
            _ => true,
        }
    }
}

/// Iterator over JsonState fields
pub struct JsonStateIter<'a> {
    inner: Option<serde_json::map::Iter<'a>>,
}

impl<'a> Iterator for JsonStateIter<'a> {
    type Item = (&'a String, &'a serde_json::Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.as_mut()?.next()
    }
}

impl From<serde_json::Value> for JsonState {
    fn from(value: serde_json::Value) -> Self {
        Self { data: value }
    }
}

impl From<serde_json::Map<String, serde_json::Value>> for JsonState {
    fn from(map: serde_json::Map<String, serde_json::Value>) -> Self {
        Self::from_object(map)
    }
}

impl std::str::FromStr for JsonState {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let value: serde_json::Value = serde_json::from_str(s)?;
        Ok(Self::from(value))
    }
}

impl std::fmt::Display for JsonState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(&self.data).unwrap_or_default()
        )
    }
}

impl MergeableState for JsonState {
    fn merge(&mut self, other: &Self) {
        // Deep merge objects: combine fields from both states
        if let (serde_json::Value::Object(self_map), serde_json::Value::Object(other_map)) =
            (&mut self.data, &other.data)
        {
            for (key, value) in other_map {
                match (self_map.get(key), value) {
                    // Both are arrays: extend
                    (
                        Some(serde_json::Value::Array(self_arr)),
                        serde_json::Value::Array(other_arr),
                    ) => {
                        let mut combined = self_arr.clone();
                        combined.extend(other_arr.clone());
                        self_map.insert(key.clone(), serde_json::Value::Array(combined));
                    }
                    // Both are objects: recursive merge
                    (
                        Some(serde_json::Value::Object(self_obj)),
                        serde_json::Value::Object(other_obj),
                    ) => {
                        let mut self_state = JsonState::from_object(self_obj.clone());
                        let other_state = JsonState::from_object(other_obj.clone());
                        self_state.merge(&other_state);
                        self_map.insert(key.clone(), self_state.into_value());
                    }
                    // Otherwise: other wins (last-write-wins for scalars)
                    _ => {
                        self_map.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state() {
        let mut state = AgentState::new();
        assert_eq!(state.messages.len(), 0);
        assert_eq!(state.iteration, 0);

        state.add_message("Hello");
        state.increment_iteration();
        state.set_next("next_node");

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.iteration, 1);
        assert_eq!(state.next, Some("next_node".to_string()));
    }

    #[test]
    fn test_custom_state() {
        #[derive(Clone, Serialize, Deserialize)]
        struct CustomState {
            _value: i32,
        }

        // Verify GraphState is implemented automatically
        fn _assert_graph_state<T: GraphState>() {}
        _assert_graph_state::<CustomState>();
    }

    #[test]
    fn test_agent_state_default() {
        let state = AgentState::default();
        assert_eq!(state.messages.len(), 0);
        assert_eq!(state.iteration, 0);
        assert_eq!(state.next, None);
        assert_eq!(state.metadata, serde_json::Value::Null);
    }

    #[test]
    fn test_agent_state_new_equals_default() {
        let state1 = AgentState::new();
        let state2 = AgentState::default();
        assert_eq!(state1.messages, state2.messages);
        assert_eq!(state1.iteration, state2.iteration);
        assert_eq!(state1.next, state2.next);
    }

    #[test]
    fn test_add_message_multiple() {
        let mut state = AgentState::new();
        state.add_message("Message 1");
        state.add_message("Message 2");
        state.add_message("Message 3");

        assert_eq!(state.messages.len(), 3);
        assert_eq!(state.messages[0], "Message 1");
        assert_eq!(state.messages[1], "Message 2");
        assert_eq!(state.messages[2], "Message 3");
    }

    #[test]
    fn test_add_message_string() {
        let mut state = AgentState::new();
        state.add_message(String::from("String message"));
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0], "String message");
    }

    #[test]
    fn test_add_message_str_slice() {
        let mut state = AgentState::new();
        let message = "Borrowed str";
        state.add_message(message);
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0], "Borrowed str");
    }

    #[test]
    fn test_increment_iteration_multiple() {
        let mut state = AgentState::new();
        assert_eq!(state.iteration, 0);

        state.increment_iteration();
        assert_eq!(state.iteration, 1);

        state.increment_iteration();
        assert_eq!(state.iteration, 2);

        state.increment_iteration();
        assert_eq!(state.iteration, 3);
    }

    #[test]
    fn test_set_next_string() {
        let mut state = AgentState::new();
        state.set_next(String::from("node_a"));
        assert_eq!(state.next, Some("node_a".to_string()));
    }

    #[test]
    fn test_set_next_str_slice() {
        let mut state = AgentState::new();
        state.set_next("node_b");
        assert_eq!(state.next, Some("node_b".to_string()));
    }

    #[test]
    fn test_set_next_multiple_times() {
        let mut state = AgentState::new();
        state.set_next("node_1");
        assert_eq!(state.next, Some("node_1".to_string()));

        state.set_next("node_2");
        assert_eq!(state.next, Some("node_2".to_string()));

        state.set_next("node_3");
        assert_eq!(state.next, Some("node_3".to_string()));
    }

    #[test]
    fn test_agent_state_clone() {
        let mut state1 = AgentState::new();
        state1.add_message("Test message");
        state1.increment_iteration();
        state1.set_next("node");

        let state2 = state1.clone();
        assert_eq!(state1.messages, state2.messages);
        assert_eq!(state1.iteration, state2.iteration);
        assert_eq!(state1.next, state2.next);
    }

    #[test]
    fn test_agent_state_serialization() {
        let mut state = AgentState::new();
        state.add_message("Message 1");
        state.increment_iteration();
        state.set_next("next_node");

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AgentState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.messages, deserialized.messages);
        assert_eq!(state.iteration, deserialized.iteration);
        assert_eq!(state.next, deserialized.next);
    }

    #[test]
    fn test_agent_state_metadata() {
        let mut state = AgentState::new();
        state.metadata = serde_json::json!({
            "user_id": "user123",
            "session": "session456",
            "tags": ["tag1", "tag2"]
        });

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AgentState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.metadata, deserialized.metadata);
        assert_eq!(state.metadata["user_id"].as_str().unwrap(), "user123");
    }

    #[test]
    fn test_agent_state_debug_format() {
        let mut state = AgentState::new();
        state.add_message("Test");
        state.increment_iteration();

        let debug_output = format!("{:?}", state);
        assert!(debug_output.contains("AgentState"));
        assert!(debug_output.contains("messages"));
        assert!(debug_output.contains("iteration"));
    }

    #[test]
    fn test_graph_state_trait_on_simple_types() {
        #[derive(Clone, Serialize, Deserialize)]
        struct SimpleState {
            value: String,
        }

        fn _assert_simple_state<T: GraphState>() {}
        _assert_simple_state::<SimpleState>();
    }

    #[test]
    fn test_graph_state_trait_on_complex_types() {
        #[derive(Clone, Serialize, Deserialize)]
        struct ComplexState {
            data: Vec<String>,
            nested: std::collections::HashMap<String, i32>,
            optional: Option<String>,
        }

        fn _assert_complex_state<T: GraphState>() {}
        _assert_complex_state::<ComplexState>();
    }

    #[test]
    fn test_agent_state_empty_messages() {
        let state = AgentState::new();
        assert!(state.messages.is_empty());
        assert_eq!(state.messages.capacity(), 0);
    }

    #[test]
    fn test_agent_state_with_metadata_null() {
        let state = AgentState::new();
        assert!(state.metadata.is_null());
    }

    #[test]
    fn test_agent_state_next_none_by_default() {
        let state = AgentState::new();
        assert_eq!(state.next, None);
    }

    #[test]
    fn test_agent_state_combined_operations() {
        let mut state = AgentState::new();

        // Perform multiple operations
        state.add_message("Message 1");
        state.increment_iteration();
        state.add_message("Message 2");
        state.increment_iteration();
        state.set_next("final_node");
        state.add_message("Message 3");

        // Verify final state
        assert_eq!(state.messages.len(), 3);
        assert_eq!(state.iteration, 2);
        assert_eq!(state.next, Some("final_node".to_string()));
    }

    #[test]
    fn test_agent_state_serialization_with_special_characters() {
        let mut state = AgentState::new();
        state.add_message("Message with \"quotes\" and \n newlines");
        state.set_next("node/with/slashes");

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AgentState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.messages, deserialized.messages);
        assert_eq!(state.next, deserialized.next);
    }

    #[test]
    fn test_agent_state_large_iteration_count() {
        let mut state = AgentState::new();
        for _ in 0..1000 {
            state.increment_iteration();
        }
        assert_eq!(state.iteration, 1000);
    }

    #[test]
    fn test_agent_state_many_messages() {
        let mut state = AgentState::new();
        for i in 0..100 {
            state.add_message(format!("Message {}", i));
        }
        assert_eq!(state.messages.len(), 100);
        assert_eq!(state.messages[0], "Message 0");
        assert_eq!(state.messages[99], "Message 99");
    }

    #[test]
    fn test_custom_state_with_different_types() {
        #[derive(Clone, Serialize, Deserialize)]
        struct MultiTypeState {
            int_field: i32,
            float_field: f64,
            bool_field: bool,
            string_field: String,
            vec_field: Vec<u8>,
        }

        fn _assert_multi_type_state<T: GraphState>() {}
        _assert_multi_type_state::<MultiTypeState>();
    }

    #[test]
    fn test_agent_state_serialization_roundtrip_empty() {
        let state = AgentState::new();
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AgentState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.messages, deserialized.messages);
        assert_eq!(state.iteration, deserialized.iteration);
        assert_eq!(state.next, deserialized.next);
    }

    #[test]
    fn test_agent_state_metadata_nested_json() {
        let mut state = AgentState::new();
        state.metadata = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": "deep_value"
                }
            }
        });

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AgentState = serde_json::from_str(&json).unwrap();

        assert_eq!(
            deserialized.metadata["level1"]["level2"]["level3"]
                .as_str()
                .unwrap(),
            "deep_value"
        );
    }

    #[test]
    fn test_graph_state_trait_send_sync() {
        // Verify that GraphState requires Send + Sync
        fn _assert_send_sync<T: GraphState>() {
            fn _is_send<T: Send>() {}
            fn _is_sync<T: Sync>() {}
            _is_send::<T>();
            _is_sync::<T>();
        }
        _assert_send_sync::<AgentState>();
    }

    #[test]
    fn test_agent_state_equality_after_operations() {
        let mut state1 = AgentState::new();
        let mut state2 = AgentState::new();

        state1.add_message("Same message");
        state2.add_message("Same message");

        state1.increment_iteration();
        state2.increment_iteration();

        state1.set_next("same_node");
        state2.set_next("same_node");

        // Both states should be structurally equal
        assert_eq!(state1.messages, state2.messages);
        assert_eq!(state1.iteration, state2.iteration);
        assert_eq!(state1.next, state2.next);
    }

    // ========================================================================
    // JsonState tests
    // ========================================================================

    #[test]
    fn test_json_state_new() {
        let state = JsonState::new();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn test_json_state_from_value() {
        let state = JsonState::from(serde_json::json!({
            "question": "What is the capital of France?",
            "answer": "Paris"
        }));
        assert_eq!(
            state.get_str("question"),
            Some("What is the capital of France?")
        );
        assert_eq!(state.get_str("answer"), Some("Paris"));
        assert_eq!(state.len(), 2);
    }

    #[test]
    fn test_json_state_set_and_get() {
        let mut state = JsonState::new();
        state.set("name", serde_json::json!("Alice"));
        state.set("age", serde_json::json!(30));
        state.set("active", serde_json::json!(true));

        assert_eq!(state.get_str("name"), Some("Alice"));
        assert_eq!(state.get_i64("age"), Some(30));
        assert_eq!(state.get_bool("active"), Some(true));
    }

    #[test]
    fn test_json_state_from_str() {
        let state: JsonState = r#"{"input": "test", "output": "result"}"#.parse().unwrap();
        assert_eq!(state.get_str("input"), Some("test"));
        assert_eq!(state.get_str("output"), Some("result"));
    }

    #[test]
    fn test_json_state_remove() {
        let mut state = JsonState::from(serde_json::json!({
            "a": 1,
            "b": 2
        }));
        let removed = state.remove("a");
        assert_eq!(removed, Some(serde_json::json!(1)));
        assert!(!state.contains("a"));
        assert!(state.contains("b"));
    }

    #[test]
    fn test_json_state_contains() {
        let state = JsonState::from(serde_json::json!({"key": "value"}));
        assert!(state.contains("key"));
        assert!(!state.contains("missing"));
    }

    #[test]
    fn test_json_state_iter() {
        let state = JsonState::from(serde_json::json!({
            "a": 1,
            "b": 2
        }));
        let keys: Vec<&String> = state.iter().map(|(k, _)| k).collect();
        assert!(keys.contains(&&"a".to_string()));
        assert!(keys.contains(&&"b".to_string()));
    }

    #[test]
    fn test_json_state_array_field() {
        let state = JsonState::from(serde_json::json!({
            "items": [1, 2, 3]
        }));
        let arr = state.get_array("items").unwrap();
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_json_state_object_field() {
        let state = JsonState::from(serde_json::json!({
            "nested": {"key": "value"}
        }));
        let obj = state.get_object("nested").unwrap();
        assert_eq!(obj.get("key").and_then(|v| v.as_str()), Some("value"));
    }

    #[test]
    fn test_json_state_serialization() {
        let state = JsonState::from(serde_json::json!({
            "question": "test",
            "answer": "result"
        }));
        let json = serde_json::to_string(&state).unwrap();
        let restored: JsonState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.get_str("question"), restored.get_str("question"));
        assert_eq!(state.get_str("answer"), restored.get_str("answer"));
    }

    #[test]
    fn test_json_state_merge_scalars() {
        let mut state1 = JsonState::from(serde_json::json!({
            "a": 1,
            "b": 2
        }));
        let state2 = JsonState::from(serde_json::json!({
            "b": 3,
            "c": 4
        }));
        state1.merge(&state2);

        assert_eq!(state1.get_i64("a"), Some(1)); // unchanged
        assert_eq!(state1.get_i64("b"), Some(3)); // overwritten
        assert_eq!(state1.get_i64("c"), Some(4)); // added
    }

    #[test]
    fn test_json_state_merge_arrays() {
        let mut state1 = JsonState::from(serde_json::json!({
            "items": [1, 2]
        }));
        let state2 = JsonState::from(serde_json::json!({
            "items": [3, 4]
        }));
        state1.merge(&state2);

        let items = state1.get_array("items").unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0], serde_json::json!(1));
        assert_eq!(items[3], serde_json::json!(4));
    }

    #[test]
    fn test_json_state_merge_nested_objects() {
        let mut state1 = JsonState::from(serde_json::json!({
            "outer": {
                "a": 1
            }
        }));
        let state2 = JsonState::from(serde_json::json!({
            "outer": {
                "b": 2
            }
        }));
        state1.merge(&state2);

        let outer = state1.get_object("outer").unwrap();
        assert_eq!(outer.get("a").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(outer.get("b").and_then(|v| v.as_i64()), Some(2));
    }

    #[test]
    fn test_json_state_graph_state_trait() {
        // Verify JsonState implements GraphState
        fn _assert_graph_state<T: GraphState>() {}
        _assert_graph_state::<JsonState>();
    }

    #[test]
    fn test_json_state_mergeable_state_trait() {
        // Verify JsonState implements MergeableState
        fn _assert_mergeable_state<T: MergeableState>() {}
        _assert_mergeable_state::<JsonState>();
    }

    #[test]
    fn test_json_state_display() {
        let state = JsonState::from(serde_json::json!({"key": "value"}));
        let display = format!("{}", state);
        assert!(display.contains("key"));
        assert!(display.contains("value"));
    }

    #[test]
    fn test_json_state_into_value() {
        let state = JsonState::from(serde_json::json!({"test": true}));
        let value = state.into_value();
        assert_eq!(value["test"], serde_json::json!(true));
    }
}
