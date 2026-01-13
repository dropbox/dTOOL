//! Decision Explanation
//!
//! This module provides types for tracking and explaining routing decisions
//! made during graph execution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Decision Explanation
// ============================================================================

/// Decision log - tracks why a conditional edge was chosen
///
/// This struct enables AI agents to understand and explain their decision-making
/// process during graph execution. Each decision log captures the context around
/// a conditional edge choice, including the condition evaluated, which path was
/// chosen, and optional reasoning.
///
/// # Example
///
/// ```rust,ignore
/// // Log a routing decision
/// let decision = DecisionLog::new("router", "has_tool_calls()")
///     .with_chosen_path("tool_executor")
///     .with_reasoning("State has 3 pending tool calls that need execution")
///     .with_state_value("tool_calls_count", serde_json::json!(3))
///     .with_state_value("tool_calls", serde_json::json!(["search", "calculate", "write"]));
///
/// // Later, AI can explain: "I went to tool_executor because I had 3 pending tool calls"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionLog {
    /// Node where the decision was made
    pub node: String,
    /// The condition or expression that was evaluated
    pub condition: String,
    /// The path/node that was chosen
    pub chosen_path: String,
    /// Alternative paths that were not chosen
    pub alternative_paths: Vec<String>,
    /// Relevant state values at decision time
    pub state_values: HashMap<String, serde_json::Value>,
    /// Human-readable reasoning for the decision
    pub reasoning: Option<String>,
    /// Timestamp when decision was made (ISO 8601)
    pub timestamp: Option<String>,
    /// Execution index when this decision occurred
    pub execution_index: Option<usize>,
    /// Whether this was a default/fallback choice
    pub is_default: bool,
    /// Confidence score (0.0-1.0) if decision involved probabilistic choice
    pub confidence: Option<f64>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DecisionLog {
    /// Create a new decision log for a node and condition
    #[must_use]
    pub fn new(node: impl Into<String>, condition: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            condition: condition.into(),
            chosen_path: String::new(),
            alternative_paths: Vec::new(),
            state_values: HashMap::new(),
            reasoning: None,
            timestamp: None,
            execution_index: None,
            is_default: false,
            confidence: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a builder for decision logs
    #[must_use]
    pub fn builder() -> DecisionLogBuilder {
        DecisionLogBuilder::new()
    }

    /// Set the chosen path
    #[must_use]
    pub fn with_chosen_path(mut self, path: impl Into<String>) -> Self {
        self.chosen_path = path.into();
        self
    }

    /// Add an alternative path that was not chosen
    #[must_use]
    pub fn with_alternative(mut self, path: impl Into<String>) -> Self {
        self.alternative_paths.push(path.into());
        self
    }

    /// Set all alternative paths
    #[must_use]
    pub fn with_alternatives(mut self, paths: Vec<String>) -> Self {
        self.alternative_paths = paths;
        self
    }

    /// Add a relevant state value
    #[must_use]
    pub fn with_state_value(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.state_values.insert(key.into(), value);
        self
    }

    /// Set all state values
    #[must_use]
    pub fn with_state_values(mut self, values: HashMap<String, serde_json::Value>) -> Self {
        self.state_values = values;
        self
    }

    /// Set human-readable reasoning
    #[must_use]
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
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

    /// Mark as a default/fallback choice
    #[must_use]
    pub fn as_default(mut self) -> Self {
        self.is_default = true;
        self
    }

    /// Set confidence score
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Add custom metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Convert to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert to compact JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Check if this decision had alternatives
    #[must_use]
    pub fn had_alternatives(&self) -> bool {
        !self.alternative_paths.is_empty()
    }

    /// Get total number of possible paths (chosen + alternatives)
    #[must_use]
    pub fn total_paths(&self) -> usize {
        1 + self.alternative_paths.len()
    }

    /// Check if reasoning was provided
    #[must_use]
    pub fn has_reasoning(&self) -> bool {
        self.reasoning.is_some()
    }

    /// Get a state value by key
    #[must_use]
    pub fn get_state_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.state_values.get(key)
    }

    /// Generate a human-readable explanation of the decision
    #[must_use]
    pub fn explain(&self) -> String {
        let mut explanation = format!(
            "At node '{}', evaluated condition '{}'",
            self.node, self.condition
        );

        if !self.chosen_path.is_empty() {
            explanation.push_str(&format!(" and chose path '{}'", self.chosen_path));
        }

        if self.is_default {
            explanation.push_str(" (default)");
        }

        if !self.alternative_paths.is_empty() {
            explanation.push_str(&format!(
                " over alternatives: [{}]",
                self.alternative_paths.join(", ")
            ));
        }

        if let Some(ref reasoning) = self.reasoning {
            explanation.push_str(&format!(". Reason: {}", reasoning));
        }

        if let Some(confidence) = self.confidence {
            explanation.push_str(&format!(" (confidence: {:.1}%)", confidence * 100.0));
        }

        explanation
    }
}

/// Builder for creating decision logs
#[derive(Debug, Default)]
pub struct DecisionLogBuilder {
    node: Option<String>,
    condition: Option<String>,
    chosen_path: String,
    alternative_paths: Vec<String>,
    state_values: HashMap<String, serde_json::Value>,
    reasoning: Option<String>,
    timestamp: Option<String>,
    execution_index: Option<usize>,
    is_default: bool,
    confidence: Option<f64>,
    metadata: HashMap<String, serde_json::Value>,
}

impl DecisionLogBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the node where decision was made
    #[must_use]
    pub fn node(mut self, node: impl Into<String>) -> Self {
        self.node = Some(node.into());
        self
    }

    /// Set the condition evaluated
    #[must_use]
    pub fn condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }

    /// Set the chosen path
    #[must_use]
    pub fn chosen_path(mut self, path: impl Into<String>) -> Self {
        self.chosen_path = path.into();
        self
    }

    /// Add an alternative path
    #[must_use]
    pub fn add_alternative(mut self, path: impl Into<String>) -> Self {
        self.alternative_paths.push(path.into());
        self
    }

    /// Set all alternative paths
    #[must_use]
    pub fn alternatives(mut self, paths: Vec<String>) -> Self {
        self.alternative_paths = paths;
        self
    }

    /// Add a state value
    #[must_use]
    pub fn state_value(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.state_values.insert(key.into(), value);
        self
    }

    /// Set all state values
    #[must_use]
    pub fn state_values(mut self, values: HashMap<String, serde_json::Value>) -> Self {
        self.state_values = values;
        self
    }

    /// Set reasoning
    #[must_use]
    pub fn reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }

    /// Set timestamp
    #[must_use]
    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    /// Set execution index
    #[must_use]
    pub fn execution_index(mut self, index: usize) -> Self {
        self.execution_index = Some(index);
        self
    }

    /// Mark as default choice
    #[must_use]
    pub fn is_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    /// Set confidence score
    #[must_use]
    pub fn confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Add metadata
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Build the decision log
    ///
    /// # Errors
    ///
    /// Returns error if node or condition is not set
    pub fn build(self) -> Result<DecisionLog, &'static str> {
        Ok(DecisionLog {
            node: self.node.ok_or("node is required")?,
            condition: self.condition.ok_or("condition is required")?,
            chosen_path: self.chosen_path,
            alternative_paths: self.alternative_paths,
            state_values: self.state_values,
            reasoning: self.reasoning,
            timestamp: self.timestamp,
            execution_index: self.execution_index,
            is_default: self.is_default,
            confidence: self.confidence,
            metadata: self.metadata,
        })
    }
}

/// Decision history - collection of decision logs for analysis
///
/// This struct aggregates multiple decision logs from an execution,
/// enabling AI agents to analyze patterns in their decision-making.
///
/// # Example
///
/// ```rust,ignore
/// let mut history = DecisionHistory::new();
/// history.add(decision1);
/// history.add(decision2);
///
/// // Analyze decision patterns
/// let router_decisions = history.decisions_at_node("router");
/// let default_count = history.default_decision_count();
/// let avg_confidence = history.average_confidence();
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionHistory {
    /// All decision logs in chronological order
    pub decisions: Vec<DecisionLog>,
    /// Thread ID for this history
    pub thread_id: Option<String>,
    /// Execution ID for this history
    pub execution_id: Option<String>,
}

impl DecisionHistory {
    /// Create a new empty decision history
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a decision history with thread ID
    #[must_use]
    pub fn with_thread_id(thread_id: impl Into<String>) -> Self {
        Self {
            decisions: Vec::new(),
            thread_id: Some(thread_id.into()),
            execution_id: None,
        }
    }

    /// Set execution ID
    #[must_use]
    pub fn with_execution_id(mut self, execution_id: impl Into<String>) -> Self {
        self.execution_id = Some(execution_id.into());
        self
    }

    /// Add a decision to the history
    pub fn add(&mut self, decision: DecisionLog) {
        self.decisions.push(decision);
    }

    /// Add a decision and return self (for chaining)
    #[must_use]
    pub fn with_decision(mut self, decision: DecisionLog) -> Self {
        self.decisions.push(decision);
        self
    }

    /// Get all decisions as slice
    #[must_use]
    pub fn all(&self) -> &[DecisionLog] {
        &self.decisions
    }

    /// Get decision count
    #[must_use]
    pub fn len(&self) -> usize {
        self.decisions.len()
    }

    /// Check if history is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.decisions.is_empty()
    }

    /// Convert to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Get decisions made at a specific node
    #[must_use]
    pub fn decisions_at_node(&self, node: &str) -> Vec<&DecisionLog> {
        self.decisions.iter().filter(|d| d.node == node).collect()
    }

    /// Get decisions that chose a specific path
    #[must_use]
    pub fn decisions_choosing_path(&self, path: &str) -> Vec<&DecisionLog> {
        self.decisions
            .iter()
            .filter(|d| d.chosen_path == path)
            .collect()
    }

    /// Get decisions for a specific condition
    #[must_use]
    pub fn decisions_for_condition(&self, condition: &str) -> Vec<&DecisionLog> {
        self.decisions
            .iter()
            .filter(|d| d.condition == condition)
            .collect()
    }

    /// Get count of default decisions
    #[must_use]
    pub fn default_decision_count(&self) -> usize {
        self.decisions.iter().filter(|d| d.is_default).count()
    }

    /// Get percentage of default decisions
    #[must_use]
    pub fn default_decision_percentage(&self) -> f64 {
        if self.decisions.is_empty() {
            0.0
        } else {
            (self.default_decision_count() as f64 / self.decisions.len() as f64) * 100.0
        }
    }

    /// Get decisions with reasoning provided
    #[must_use]
    pub fn decisions_with_reasoning(&self) -> Vec<&DecisionLog> {
        self.decisions
            .iter()
            .filter(|d| d.reasoning.is_some())
            .collect()
    }

    /// Get average confidence across all decisions with confidence scores
    #[must_use]
    pub fn average_confidence(&self) -> Option<f64> {
        let confidences: Vec<f64> = self.decisions.iter().filter_map(|d| d.confidence).collect();

        if confidences.is_empty() {
            None
        } else {
            Some(confidences.iter().sum::<f64>() / confidences.len() as f64)
        }
    }

    /// Get minimum confidence across all decisions
    #[must_use]
    pub fn min_confidence(&self) -> Option<f64> {
        self.decisions
            .iter()
            .filter_map(|d| d.confidence)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get maximum confidence across all decisions
    #[must_use]
    pub fn max_confidence(&self) -> Option<f64> {
        self.decisions
            .iter()
            .filter_map(|d| d.confidence)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get unique nodes where decisions were made
    #[must_use]
    pub fn unique_decision_nodes(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        self.decisions
            .iter()
            .filter(|d| seen.insert(d.node.as_str()))
            .map(|d| d.node.as_str())
            .collect()
    }

    /// Get unique paths that were chosen
    #[must_use]
    pub fn unique_chosen_paths(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        self.decisions
            .iter()
            .filter(|d| !d.chosen_path.is_empty() && seen.insert(d.chosen_path.as_str()))
            .map(|d| d.chosen_path.as_str())
            .collect()
    }

    /// Get path choice frequency (how often each path was chosen)
    #[must_use]
    pub fn path_choice_frequency(&self) -> HashMap<String, usize> {
        let mut freq = HashMap::new();
        for decision in &self.decisions {
            if !decision.chosen_path.is_empty() {
                *freq.entry(decision.chosen_path.clone()).or_insert(0) += 1;
            }
        }
        freq
    }

    /// Get most frequently chosen path
    #[must_use]
    pub fn most_frequent_path(&self) -> Option<(String, usize)> {
        let freq = self.path_choice_frequency();
        freq.into_iter().max_by_key(|(_, count)| *count)
    }

    /// Get decisions in chronological order (by execution index)
    #[must_use]
    pub fn chronological(&self) -> Vec<&DecisionLog> {
        let mut sorted: Vec<_> = self.decisions.iter().collect();
        sorted.sort_by_key(|d| d.execution_index.unwrap_or(usize::MAX));
        sorted
    }

    /// Get the last decision made
    #[must_use]
    pub fn last_decision(&self) -> Option<&DecisionLog> {
        self.decisions.last()
    }

    /// Get decision by execution index
    #[must_use]
    pub fn decision_at_index(&self, index: usize) -> Option<&DecisionLog> {
        self.decisions
            .iter()
            .find(|d| d.execution_index == Some(index))
    }

    /// Generate a summary of all decisions
    #[must_use]
    pub fn summarize(&self) -> String {
        if self.decisions.is_empty() {
            return "No decisions recorded.".to_string();
        }

        let mut summary = format!("Decision History ({} decisions):\n", self.decisions.len());

        // Decision points summary
        let nodes = self.unique_decision_nodes();
        summary.push_str(&format!("- Decision points: {}\n", nodes.len()));

        // Default vs explicit
        let default_pct = self.default_decision_percentage();
        summary.push_str(&format!("- Default decisions: {:.1}%\n", default_pct));

        // Confidence summary
        if let Some(avg) = self.average_confidence() {
            summary.push_str(&format!("- Average confidence: {:.1}%\n", avg * 100.0));
        }

        // Most common path
        if let Some((ref path, count)) = self.most_frequent_path() {
            summary.push_str(&format!(
                "- Most chosen path: '{}' ({} times)\n",
                path, count
            ));
        }

        summary
    }
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // =========================================================================
    // DecisionLog Tests
    // =========================================================================

    #[test]
    fn test_decision_log_new() {
        let log = DecisionLog::new("router", "has_tool_calls()");
        assert_eq!(log.node, "router");
        assert_eq!(log.condition, "has_tool_calls()");
        assert!(log.chosen_path.is_empty());
        assert!(log.alternative_paths.is_empty());
        assert!(log.state_values.is_empty());
        assert!(log.reasoning.is_none());
        assert!(!log.is_default);
        assert!(log.confidence.is_none());
    }

    #[test]
    fn test_decision_log_with_chosen_path() {
        let log = DecisionLog::new("router", "condition").with_chosen_path("tool_executor");
        assert_eq!(log.chosen_path, "tool_executor");
    }

    #[test]
    fn test_decision_log_with_alternative() {
        let log = DecisionLog::new("router", "condition")
            .with_alternative("path_a")
            .with_alternative("path_b");
        assert_eq!(log.alternative_paths.len(), 2);
        assert!(log.alternative_paths.contains(&"path_a".to_string()));
        assert!(log.alternative_paths.contains(&"path_b".to_string()));
    }

    #[test]
    fn test_decision_log_with_alternatives() {
        let log = DecisionLog::new("router", "condition").with_alternatives(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
        ]);
        assert_eq!(log.alternative_paths.len(), 3);
    }

    #[test]
    fn test_decision_log_with_state_value() {
        let log = DecisionLog::new("router", "condition")
            .with_state_value("count", json!(5))
            .with_state_value("enabled", json!(true));
        assert_eq!(log.state_values.len(), 2);
        assert_eq!(log.get_state_value("count"), Some(&json!(5)));
        assert_eq!(log.get_state_value("enabled"), Some(&json!(true)));
        assert!(log.get_state_value("nonexistent").is_none());
    }

    #[test]
    fn test_decision_log_with_state_values() {
        let mut values = HashMap::new();
        values.insert("a".to_string(), json!(1));
        values.insert("b".to_string(), json!(2));

        let log = DecisionLog::new("router", "condition").with_state_values(values);
        assert_eq!(log.state_values.len(), 2);
    }

    #[test]
    fn test_decision_log_with_reasoning() {
        let log = DecisionLog::new("router", "condition")
            .with_reasoning("This path was chosen because...");
        assert!(log.has_reasoning());
        assert_eq!(
            log.reasoning,
            Some("This path was chosen because...".to_string())
        );
    }

    #[test]
    fn test_decision_log_with_timestamp() {
        let log = DecisionLog::new("router", "condition").with_timestamp("2024-01-15T10:30:00Z");
        assert_eq!(log.timestamp, Some("2024-01-15T10:30:00Z".to_string()));
    }

    #[test]
    fn test_decision_log_with_execution_index() {
        let log = DecisionLog::new("router", "condition").with_execution_index(42);
        assert_eq!(log.execution_index, Some(42));
    }

    #[test]
    fn test_decision_log_as_default() {
        let log = DecisionLog::new("router", "condition").as_default();
        assert!(log.is_default);
    }

    #[test]
    fn test_decision_log_with_confidence() {
        let log = DecisionLog::new("router", "condition").with_confidence(0.85);
        assert_eq!(log.confidence, Some(0.85));
    }

    #[test]
    fn test_decision_log_with_confidence_clamped() {
        // Test clamping to [0, 1]
        let log_over = DecisionLog::new("router", "condition").with_confidence(1.5);
        assert_eq!(log_over.confidence, Some(1.0));

        let log_under = DecisionLog::new("router", "condition").with_confidence(-0.5);
        assert_eq!(log_under.confidence, Some(0.0));
    }

    #[test]
    fn test_decision_log_with_metadata() {
        let log = DecisionLog::new("router", "condition")
            .with_metadata("custom_key", json!("custom_value"));
        assert_eq!(log.metadata.get("custom_key"), Some(&json!("custom_value")));
    }

    #[test]
    fn test_decision_log_to_json() {
        let log = DecisionLog::new("router", "has_tool_calls()")
            .with_chosen_path("executor")
            .with_reasoning("Had tool calls");

        let json = log.to_json().expect("to_json should succeed");
        assert!(json.contains("router"));
        assert!(json.contains("has_tool_calls()"));
        assert!(json.contains("executor"));
    }

    #[test]
    fn test_decision_log_to_json_compact() {
        let log = DecisionLog::new("router", "condition");
        let json = log
            .to_json_compact()
            .expect("to_json_compact should succeed");
        assert!(!json.contains('\n')); // Compact means no newlines
    }

    #[test]
    fn test_decision_log_from_json() {
        let json = r#"{"node":"router","condition":"check","chosen_path":"path_a","alternative_paths":[],"state_values":{},"reasoning":null,"timestamp":null,"execution_index":null,"is_default":false,"confidence":null,"metadata":{}}"#;
        let log = DecisionLog::from_json(json).expect("from_json should succeed");
        assert_eq!(log.node, "router");
        assert_eq!(log.condition, "check");
        assert_eq!(log.chosen_path, "path_a");
    }

    #[test]
    fn test_decision_log_json_roundtrip() {
        let original = DecisionLog::new("router", "condition")
            .with_chosen_path("selected")
            .with_alternative("other")
            .with_reasoning("Because")
            .with_confidence(0.9)
            .as_default();

        let json = original.to_json().expect("to_json should succeed");
        let parsed = DecisionLog::from_json(&json).expect("from_json should succeed");

        assert_eq!(parsed.node, original.node);
        assert_eq!(parsed.condition, original.condition);
        assert_eq!(parsed.chosen_path, original.chosen_path);
        assert_eq!(parsed.alternative_paths, original.alternative_paths);
        assert_eq!(parsed.reasoning, original.reasoning);
        assert_eq!(parsed.confidence, original.confidence);
        assert_eq!(parsed.is_default, original.is_default);
    }

    #[test]
    fn test_decision_log_had_alternatives() {
        let log_with = DecisionLog::new("router", "condition").with_alternative("other");
        assert!(log_with.had_alternatives());

        let log_without = DecisionLog::new("router", "condition");
        assert!(!log_without.had_alternatives());
    }

    #[test]
    fn test_decision_log_total_paths() {
        let log = DecisionLog::new("router", "condition")
            .with_chosen_path("main")
            .with_alternative("alt1")
            .with_alternative("alt2");
        assert_eq!(log.total_paths(), 3); // 1 chosen + 2 alternatives
    }

    #[test]
    fn test_decision_log_has_reasoning() {
        let log_with = DecisionLog::new("router", "condition").with_reasoning("Reason");
        assert!(log_with.has_reasoning());

        let log_without = DecisionLog::new("router", "condition");
        assert!(!log_without.has_reasoning());
    }

    #[test]
    fn test_decision_log_explain_basic() {
        let log = DecisionLog::new("router", "has_tool_calls()").with_chosen_path("tool_executor");

        let explanation = log.explain();
        assert!(explanation.contains("router"));
        assert!(explanation.contains("has_tool_calls()"));
        assert!(explanation.contains("tool_executor"));
    }

    #[test]
    fn test_decision_log_explain_with_default() {
        let log = DecisionLog::new("router", "condition")
            .with_chosen_path("fallback")
            .as_default();

        let explanation = log.explain();
        assert!(explanation.contains("(default)"));
    }

    #[test]
    fn test_decision_log_explain_with_alternatives() {
        let log = DecisionLog::new("router", "condition")
            .with_chosen_path("main")
            .with_alternative("alt1")
            .with_alternative("alt2");

        let explanation = log.explain();
        assert!(explanation.contains("alternatives"));
        assert!(explanation.contains("alt1"));
        assert!(explanation.contains("alt2"));
    }

    #[test]
    fn test_decision_log_explain_with_reasoning() {
        let log = DecisionLog::new("router", "condition")
            .with_chosen_path("path")
            .with_reasoning("This was the best option");

        let explanation = log.explain();
        assert!(explanation.contains("Reason:"));
        assert!(explanation.contains("This was the best option"));
    }

    #[test]
    fn test_decision_log_explain_with_confidence() {
        let log = DecisionLog::new("router", "condition")
            .with_chosen_path("path")
            .with_confidence(0.85);

        let explanation = log.explain();
        assert!(explanation.contains("confidence:"));
        assert!(explanation.contains("85"));
    }

    // =========================================================================
    // DecisionLogBuilder Tests
    // =========================================================================

    #[test]
    fn test_decision_log_builder_new() {
        let builder = DecisionLogBuilder::new();
        assert!(builder.build().is_err()); // Missing required fields
    }

    #[test]
    fn test_decision_log_builder_from_static_method() {
        let builder = DecisionLog::builder();
        assert!(builder.build().is_err()); // Missing required fields
    }

    #[test]
    fn test_decision_log_builder_minimal() {
        let result = DecisionLogBuilder::new()
            .node("router")
            .condition("check")
            .build();

        assert!(result.is_ok());
        let log = result.unwrap();
        assert_eq!(log.node, "router");
        assert_eq!(log.condition, "check");
    }

    #[test]
    fn test_decision_log_builder_missing_node() {
        let result = DecisionLogBuilder::new().condition("check").build();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "node is required");
    }

    #[test]
    fn test_decision_log_builder_missing_condition() {
        let result = DecisionLogBuilder::new().node("router").build();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "condition is required");
    }

    #[test]
    fn test_decision_log_builder_full() {
        let result = DecisionLogBuilder::new()
            .node("router")
            .condition("has_tool_calls()")
            .chosen_path("tool_executor")
            .add_alternative("end")
            .state_value("count", json!(3))
            .reasoning("Had tool calls")
            .timestamp("2024-01-15T10:30:00Z")
            .execution_index(5)
            .is_default(false)
            .confidence(0.95)
            .metadata("source", json!("test"))
            .build();

        assert!(result.is_ok());
        let log = result.unwrap();
        assert_eq!(log.node, "router");
        assert_eq!(log.condition, "has_tool_calls()");
        assert_eq!(log.chosen_path, "tool_executor");
        assert_eq!(log.alternative_paths, vec!["end"]);
        assert_eq!(log.state_values.get("count"), Some(&json!(3)));
        assert_eq!(log.reasoning, Some("Had tool calls".to_string()));
        assert_eq!(log.timestamp, Some("2024-01-15T10:30:00Z".to_string()));
        assert_eq!(log.execution_index, Some(5));
        assert!(!log.is_default);
        assert_eq!(log.confidence, Some(0.95));
        assert_eq!(log.metadata.get("source"), Some(&json!("test")));
    }

    #[test]
    fn test_decision_log_builder_alternatives() {
        let result = DecisionLogBuilder::new()
            .node("router")
            .condition("check")
            .alternatives(vec!["a".to_string(), "b".to_string()])
            .build();

        let log = result.unwrap();
        assert_eq!(log.alternative_paths.len(), 2);
    }

    #[test]
    fn test_decision_log_builder_state_values() {
        let mut values = HashMap::new();
        values.insert("key".to_string(), json!("value"));

        let result = DecisionLogBuilder::new()
            .node("router")
            .condition("check")
            .state_values(values)
            .build();

        let log = result.unwrap();
        assert_eq!(log.state_values.get("key"), Some(&json!("value")));
    }

    #[test]
    fn test_decision_log_builder_confidence_clamped() {
        let result = DecisionLogBuilder::new()
            .node("router")
            .condition("check")
            .confidence(2.0)
            .build();

        let log = result.unwrap();
        assert_eq!(log.confidence, Some(1.0));
    }

    // =========================================================================
    // DecisionHistory Tests
    // =========================================================================

    #[test]
    fn test_decision_history_new() {
        let history = DecisionHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.thread_id.is_none());
        assert!(history.execution_id.is_none());
    }

    #[test]
    fn test_decision_history_with_thread_id() {
        let history = DecisionHistory::with_thread_id("thread-123");
        assert_eq!(history.thread_id, Some("thread-123".to_string()));
        assert!(history.is_empty());
    }

    #[test]
    fn test_decision_history_with_execution_id() {
        let history = DecisionHistory::new().with_execution_id("exec-456");
        assert_eq!(history.execution_id, Some("exec-456".to_string()));
    }

    #[test]
    fn test_decision_history_add() {
        let mut history = DecisionHistory::new();
        history.add(DecisionLog::new("router1", "cond1"));
        history.add(DecisionLog::new("router2", "cond2"));

        assert_eq!(history.len(), 2);
        assert!(!history.is_empty());
    }

    #[test]
    fn test_decision_history_with_decision() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("router1", "cond1"))
            .with_decision(DecisionLog::new("router2", "cond2"));

        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_decision_history_all() {
        let mut history = DecisionHistory::new();
        history.add(DecisionLog::new("router", "cond"));

        let all = history.all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].node, "router");
    }

    #[test]
    fn test_decision_history_to_json() {
        let history = DecisionHistory::with_thread_id("thread-123")
            .with_decision(DecisionLog::new("router", "condition"));

        let json = history.to_json().expect("to_json should succeed");
        assert!(json.contains("thread-123"));
        assert!(json.contains("router"));
    }

    #[test]
    fn test_decision_history_from_json() {
        let json = r#"{"decisions":[{"node":"router","condition":"cond","chosen_path":"","alternative_paths":[],"state_values":{},"reasoning":null,"timestamp":null,"execution_index":null,"is_default":false,"confidence":null,"metadata":{}}],"thread_id":"t1","execution_id":null}"#;
        let history = DecisionHistory::from_json(json).expect("from_json should succeed");
        assert_eq!(history.thread_id, Some("t1".to_string()));
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_decision_history_json_roundtrip() {
        let original = DecisionHistory::with_thread_id("thread-1")
            .with_execution_id("exec-1")
            .with_decision(DecisionLog::new("router", "condition").with_chosen_path("path"));

        let json = original.to_json().expect("to_json should succeed");
        let parsed = DecisionHistory::from_json(&json).expect("from_json should succeed");

        assert_eq!(parsed.thread_id, original.thread_id);
        assert_eq!(parsed.execution_id, original.execution_id);
        assert_eq!(parsed.len(), original.len());
    }

    #[test]
    fn test_decision_history_decisions_at_node() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("router", "cond1"))
            .with_decision(DecisionLog::new("validator", "cond2"))
            .with_decision(DecisionLog::new("router", "cond3"));

        let router_decisions = history.decisions_at_node("router");
        assert_eq!(router_decisions.len(), 2);

        let validator_decisions = history.decisions_at_node("validator");
        assert_eq!(validator_decisions.len(), 1);

        let none_decisions = history.decisions_at_node("nonexistent");
        assert!(none_decisions.is_empty());
    }

    #[test]
    fn test_decision_history_decisions_choosing_path() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_chosen_path("path_a"))
            .with_decision(DecisionLog::new("r2", "c2").with_chosen_path("path_b"))
            .with_decision(DecisionLog::new("r3", "c3").with_chosen_path("path_a"));

        let path_a_decisions = history.decisions_choosing_path("path_a");
        assert_eq!(path_a_decisions.len(), 2);

        let path_b_decisions = history.decisions_choosing_path("path_b");
        assert_eq!(path_b_decisions.len(), 1);
    }

    #[test]
    fn test_decision_history_decisions_for_condition() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "has_tool_calls()"))
            .with_decision(DecisionLog::new("r2", "is_complete()"))
            .with_decision(DecisionLog::new("r3", "has_tool_calls()"));

        let tool_decisions = history.decisions_for_condition("has_tool_calls()");
        assert_eq!(tool_decisions.len(), 2);
    }

    #[test]
    fn test_decision_history_default_decision_count() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").as_default())
            .with_decision(DecisionLog::new("r2", "c2"))
            .with_decision(DecisionLog::new("r3", "c3").as_default());

        assert_eq!(history.default_decision_count(), 2);
    }

    #[test]
    fn test_decision_history_default_decision_percentage() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").as_default())
            .with_decision(DecisionLog::new("r2", "c2"))
            .with_decision(DecisionLog::new("r3", "c3").as_default())
            .with_decision(DecisionLog::new("r4", "c4"));

        assert!((history.default_decision_percentage() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_decision_history_default_decision_percentage_empty() {
        let history = DecisionHistory::new();
        assert!((history.default_decision_percentage() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_decision_history_decisions_with_reasoning() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_reasoning("Because 1"))
            .with_decision(DecisionLog::new("r2", "c2"))
            .with_decision(DecisionLog::new("r3", "c3").with_reasoning("Because 3"));

        let with_reasoning = history.decisions_with_reasoning();
        assert_eq!(with_reasoning.len(), 2);
    }

    #[test]
    fn test_decision_history_average_confidence() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_confidence(0.8))
            .with_decision(DecisionLog::new("r2", "c2").with_confidence(0.9))
            .with_decision(DecisionLog::new("r3", "c3")); // No confidence

        let avg = history.average_confidence().expect("Should have average");
        assert!((avg - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_decision_history_average_confidence_none() {
        let history = DecisionHistory::new().with_decision(DecisionLog::new("r1", "c1"));

        assert!(history.average_confidence().is_none());
    }

    #[test]
    fn test_decision_history_average_confidence_empty() {
        let history = DecisionHistory::new();
        assert!(history.average_confidence().is_none());
    }

    #[test]
    fn test_decision_history_min_confidence() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_confidence(0.9))
            .with_decision(DecisionLog::new("r2", "c2").with_confidence(0.5))
            .with_decision(DecisionLog::new("r3", "c3").with_confidence(0.7));

        assert_eq!(history.min_confidence(), Some(0.5));
    }

    #[test]
    fn test_decision_history_max_confidence() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_confidence(0.9))
            .with_decision(DecisionLog::new("r2", "c2").with_confidence(0.5))
            .with_decision(DecisionLog::new("r3", "c3").with_confidence(0.7));

        assert_eq!(history.max_confidence(), Some(0.9));
    }

    #[test]
    fn test_decision_history_unique_decision_nodes() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("router", "c1"))
            .with_decision(DecisionLog::new("validator", "c2"))
            .with_decision(DecisionLog::new("router", "c3"));

        let nodes = history.unique_decision_nodes();
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains(&"router"));
        assert!(nodes.contains(&"validator"));
    }

    #[test]
    fn test_decision_history_unique_chosen_paths() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_chosen_path("path_a"))
            .with_decision(DecisionLog::new("r2", "c2").with_chosen_path("path_b"))
            .with_decision(DecisionLog::new("r3", "c3").with_chosen_path("path_a"));

        let paths = history.unique_chosen_paths();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"path_a"));
        assert!(paths.contains(&"path_b"));
    }

    #[test]
    fn test_decision_history_unique_chosen_paths_excludes_empty() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_chosen_path("path_a"))
            .with_decision(DecisionLog::new("r2", "c2")); // Empty chosen_path

        let paths = history.unique_chosen_paths();
        assert_eq!(paths.len(), 1);
        assert!(!paths.contains(&""));
    }

    #[test]
    fn test_decision_history_path_choice_frequency() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_chosen_path("path_a"))
            .with_decision(DecisionLog::new("r2", "c2").with_chosen_path("path_b"))
            .with_decision(DecisionLog::new("r3", "c3").with_chosen_path("path_a"))
            .with_decision(DecisionLog::new("r4", "c4").with_chosen_path("path_a"));

        let freq = history.path_choice_frequency();
        assert_eq!(freq.get("path_a"), Some(&3));
        assert_eq!(freq.get("path_b"), Some(&1));
    }

    #[test]
    fn test_decision_history_most_frequent_path() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_chosen_path("path_a"))
            .with_decision(DecisionLog::new("r2", "c2").with_chosen_path("path_b"))
            .with_decision(DecisionLog::new("r3", "c3").with_chosen_path("path_a"));

        let (path, count) = history
            .most_frequent_path()
            .expect("Should have most frequent");
        assert_eq!(path, "path_a");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_decision_history_most_frequent_path_empty() {
        let history = DecisionHistory::new();
        assert!(history.most_frequent_path().is_none());
    }

    #[test]
    fn test_decision_history_chronological() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_execution_index(3))
            .with_decision(DecisionLog::new("r2", "c2").with_execution_index(1))
            .with_decision(DecisionLog::new("r3", "c3").with_execution_index(2));

        let sorted = history.chronological();
        assert_eq!(sorted[0].execution_index, Some(1));
        assert_eq!(sorted[1].execution_index, Some(2));
        assert_eq!(sorted[2].execution_index, Some(3));
    }

    #[test]
    fn test_decision_history_last_decision() {
        let mut history = DecisionHistory::new();
        assert!(history.last_decision().is_none());

        history.add(DecisionLog::new("r1", "c1"));
        history.add(DecisionLog::new("r2", "c2"));

        let last = history.last_decision().expect("Should have last");
        assert_eq!(last.node, "r2");
    }

    #[test]
    fn test_decision_history_decision_at_index() {
        let history = DecisionHistory::new()
            .with_decision(DecisionLog::new("r1", "c1").with_execution_index(0))
            .with_decision(DecisionLog::new("r2", "c2").with_execution_index(1))
            .with_decision(DecisionLog::new("r3", "c3").with_execution_index(2));

        let decision = history
            .decision_at_index(1)
            .expect("Should find at index 1");
        assert_eq!(decision.node, "r2");

        assert!(history.decision_at_index(99).is_none());
    }

    #[test]
    fn test_decision_history_summarize_empty() {
        let history = DecisionHistory::new();
        let summary = history.summarize();
        assert!(summary.contains("No decisions"));
    }

    #[test]
    fn test_decision_history_summarize_with_decisions() {
        let history = DecisionHistory::new()
            .with_decision(
                DecisionLog::new("router", "c1")
                    .with_chosen_path("path_a")
                    .with_confidence(0.9)
                    .as_default(),
            )
            .with_decision(
                DecisionLog::new("validator", "c2")
                    .with_chosen_path("path_a")
                    .with_confidence(0.8),
            );

        let summary = history.summarize();
        assert!(summary.contains("2 decisions"));
        assert!(summary.contains("Decision points:"));
        assert!(summary.contains("Default decisions:"));
        assert!(summary.contains("Average confidence:"));
        assert!(summary.contains("Most chosen path:"));
        assert!(summary.contains("path_a"));
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    #[test]
    fn test_full_decision_workflow() {
        // Simulate a typical agent workflow
        let mut history =
            DecisionHistory::with_thread_id("thread-001").with_execution_id("exec-001");

        // First decision: route to tool executor
        let decision1 = DecisionLog::new("router", "has_tool_calls()")
            .with_chosen_path("tool_executor")
            .with_alternative("end")
            .with_state_value("tool_calls_count", json!(3))
            .with_reasoning("State has pending tool calls")
            .with_execution_index(0)
            .with_confidence(0.95);
        history.add(decision1);

        // Second decision: continue after tools
        let decision2 = DecisionLog::new("router", "is_complete()")
            .with_chosen_path("summarize")
            .with_alternative("tool_executor")
            .with_state_value("is_complete", json!(false))
            .with_reasoning("More work needed")
            .with_execution_index(1)
            .with_confidence(0.85);
        history.add(decision2);

        // Third decision: end
        let decision3 = DecisionLog::new("router", "is_complete()")
            .with_chosen_path("end")
            .with_alternative("summarize")
            .with_state_value("is_complete", json!(true))
            .with_reasoning("Task complete")
            .with_execution_index(2)
            .with_confidence(0.99);
        history.add(decision3);

        // Analyze the history
        assert_eq!(history.len(), 3);
        assert_eq!(history.unique_decision_nodes().len(), 1);
        assert_eq!(history.decisions_at_node("router").len(), 3);
        assert_eq!(history.decisions_with_reasoning().len(), 3);

        let avg_confidence = history.average_confidence().expect("Should have average");
        assert!(avg_confidence > 0.9);

        // Summary should be informative
        let summary = history.summarize();
        assert!(summary.contains("3 decisions"));
    }
}
