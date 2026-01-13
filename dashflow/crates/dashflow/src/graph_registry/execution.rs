// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Execution Registry - Execution History Tracking
//!
//! This module provides tracking for graph execution history, enabling AI agents to:
//! - Monitor running vs completed executions
//! - Query execution statistics and success rates
//! - Track node execution and token usage

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

// ============================================================================
// ExecutionRegistry - Execution History
// ============================================================================

/// Status of a graph execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution is currently in progress
    Running,
    /// Execution completed successfully
    Completed,
    /// Execution failed with an error
    Failed,
    /// Execution was interrupted/cancelled
    Interrupted,
    /// Execution timed out
    TimedOut,
}

impl ExecutionStatus {
    /// Check if this is a terminal status
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ExecutionStatus::Completed
                | ExecutionStatus::Failed
                | ExecutionStatus::Interrupted
                | ExecutionStatus::TimedOut
        )
    }

    /// Check if this execution is still running
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, ExecutionStatus::Running)
    }

    /// Check if this execution succeeded
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionStatus::Completed)
    }
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Running => write!(f, "Running"),
            ExecutionStatus::Completed => write!(f, "Completed"),
            ExecutionStatus::Failed => write!(f, "Failed"),
            ExecutionStatus::Interrupted => write!(f, "Interrupted"),
            ExecutionStatus::TimedOut => write!(f, "Timed Out"),
        }
    }
}

/// Record of a single graph execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Thread ID for this execution
    pub thread_id: String,
    /// Graph ID that was executed
    pub graph_id: String,
    /// Graph version at time of execution
    pub graph_version: String,
    /// When execution started
    pub started_at: SystemTime,
    /// When execution completed (if finished)
    pub completed_at: Option<SystemTime>,
    /// Current status
    pub status: ExecutionStatus,
    /// Final state (if completed)
    pub final_state: Option<serde_json::Value>,
    /// List of nodes that were executed
    pub nodes_executed: Vec<String>,
    /// Total tokens used
    pub total_tokens: u64,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Custom metadata
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,
}

impl ExecutionRecord {
    /// Create a new execution record
    #[must_use]
    pub fn new(
        thread_id: impl Into<String>,
        graph_id: impl Into<String>,
        graph_version: impl Into<String>,
    ) -> Self {
        Self {
            thread_id: thread_id.into(),
            graph_id: graph_id.into(),
            graph_version: graph_version.into(),
            started_at: SystemTime::now(),
            completed_at: None,
            status: ExecutionStatus::Running,
            final_state: None,
            nodes_executed: Vec::new(),
            total_tokens: 0,
            error: None,
            custom: HashMap::new(),
        }
    }

    /// Mark this execution as completed
    pub fn complete(&mut self, final_state: Option<serde_json::Value>) {
        self.completed_at = Some(SystemTime::now());
        self.status = ExecutionStatus::Completed;
        self.final_state = final_state;
    }

    /// Mark this execution as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.completed_at = Some(SystemTime::now());
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.into());
    }

    /// Mark this execution as interrupted
    pub fn interrupt(&mut self) {
        self.completed_at = Some(SystemTime::now());
        self.status = ExecutionStatus::Interrupted;
    }

    /// Mark this execution as timed out
    pub fn timeout(&mut self) {
        self.completed_at = Some(SystemTime::now());
        self.status = ExecutionStatus::TimedOut;
    }

    /// Record a node execution
    pub fn record_node(&mut self, node_name: impl Into<String>) {
        self.nodes_executed.push(node_name.into());
    }

    /// Add tokens to the total
    pub fn add_tokens(&mut self, tokens: u64) {
        self.total_tokens += tokens;
    }

    /// Get execution duration
    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.completed_at.map(|end| {
            end.duration_since(self.started_at)
                .unwrap_or(Duration::ZERO)
        })
    }

    /// Get duration since start (even if not completed)
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.completed_at
            .unwrap_or_else(SystemTime::now)
            .duration_since(self.started_at)
            .unwrap_or(Duration::ZERO)
    }

    /// Add custom metadata
    pub fn set_custom(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.custom.insert(key.into(), value);
    }
}

/// Registry for tracking execution history
///
/// The ExecutionRegistry maintains a record of all graph executions,
/// enabling AI agents to query their execution history.
///
/// # Thread Safety
///
/// ExecutionRegistry uses internal RwLock for thread-safe access.
///
/// # Example
///
/// ```rust
/// use dashflow::graph_registry::{ExecutionRegistry, ExecutionStatus};
///
/// let registry = ExecutionRegistry::new();
///
/// // Record start
/// registry.record_start("thread_123", "agent_v1", "1.0.0");
///
/// // Record node execution
/// registry.record_node("thread_123", "reasoning");
///
/// // Record completion
/// registry.record_completion("thread_123", Some(serde_json::json!({"result": "success"})));
///
/// // Query executions
/// let running = registry.list_running();
/// let recent = registry.list_recent(10);
/// ```
#[derive(Debug)]
pub struct ExecutionRegistry {
    records: Arc<RwLock<HashMap<String, ExecutionRecord>>>,
    /// Maximum number of records to keep (0 = unlimited)
    max_records: usize,
}

impl Default for ExecutionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ExecutionRegistry {
    fn clone(&self) -> Self {
        Self {
            records: Arc::clone(&self.records),
            max_records: self.max_records,
        }
    }
}

impl ExecutionRegistry {
    /// Create a new execution registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            max_records: 0,
        }
    }

    /// Create a new execution registry with a maximum record limit
    #[must_use]
    pub fn with_max_records(max_records: usize) -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            max_records,
        }
    }

    /// Record the start of an execution
    pub fn record_start(
        &self,
        thread_id: impl Into<String>,
        graph_id: impl Into<String>,
        graph_version: impl Into<String>,
    ) {
        let tid = thread_id.into();
        let record = ExecutionRecord::new(&tid, graph_id, graph_version);

        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        records.insert(tid, record);

        // Prune old records if limit is set
        if self.max_records > 0 && records.len() > self.max_records {
            self.prune_oldest_internal(&mut records);
        }
    }

    /// Internal method to prune oldest completed records
    fn prune_oldest_internal(&self, records: &mut HashMap<String, ExecutionRecord>) {
        // Find oldest completed record
        let oldest = records
            .iter()
            .filter(|(_, r)| r.status.is_terminal())
            .min_by_key(|(_, r)| r.started_at)
            .map(|(id, _)| id.clone());

        if let Some(id) = oldest {
            records.remove(&id);
        }
    }

    /// Record a node being executed
    pub fn record_node(&self, thread_id: &str, node_name: impl Into<String>) {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        if let Some(record) = records.get_mut(thread_id) {
            record.record_node(node_name);
        }
    }

    /// Record token usage
    pub fn record_tokens(&self, thread_id: &str, tokens: u64) {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        if let Some(record) = records.get_mut(thread_id) {
            record.add_tokens(tokens);
        }
    }

    /// Record successful completion
    pub fn record_completion(&self, thread_id: &str, final_state: Option<serde_json::Value>) {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        if let Some(record) = records.get_mut(thread_id) {
            record.complete(final_state);
        }
    }

    /// Record failure
    pub fn record_failure(&self, thread_id: &str, error: impl Into<String>) {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        if let Some(record) = records.get_mut(thread_id) {
            record.fail(error);
        }
    }

    /// Record interruption
    pub fn record_interrupt(&self, thread_id: &str) {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        if let Some(record) = records.get_mut(thread_id) {
            record.interrupt();
        }
    }

    /// Record timeout
    pub fn record_timeout(&self, thread_id: &str) {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        if let Some(record) = records.get_mut(thread_id) {
            record.timeout();
        }
    }

    /// Get a specific execution record
    #[must_use]
    pub fn get(&self, thread_id: &str) -> Option<ExecutionRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records.get(thread_id).cloned()
    }

    /// List all running executions
    #[must_use]
    pub fn list_running(&self) -> Vec<ExecutionRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records
            .values()
            .filter(|r| r.status.is_running())
            .cloned()
            .collect()
    }

    /// List executions by status
    #[must_use]
    pub fn list_by_status(&self, status: ExecutionStatus) -> Vec<ExecutionRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records
            .values()
            .filter(|r| r.status == status)
            .cloned()
            .collect()
    }

    /// List executions for a specific graph
    #[must_use]
    pub fn list_by_graph(&self, graph_id: &str) -> Vec<ExecutionRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records
            .values()
            .filter(|r| r.graph_id == graph_id)
            .cloned()
            .collect()
    }

    /// List most recent executions
    #[must_use]
    pub fn list_recent(&self, limit: usize) -> Vec<ExecutionRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        let mut sorted: Vec<_> = records.values().cloned().collect();
        sorted.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        sorted.truncate(limit);
        sorted
    }

    /// List all executions
    #[must_use]
    pub fn list_all(&self) -> Vec<ExecutionRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records.values().cloned().collect()
    }

    /// Get total number of records
    #[must_use]
    pub fn count(&self) -> usize {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records.len()
    }

    /// Get count by status
    #[must_use]
    pub fn count_by_status(&self, status: ExecutionStatus) -> usize {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records.values().filter(|r| r.status == status).count()
    }

    /// Calculate success rate
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        let terminal: Vec<_> = records
            .values()
            .filter(|r| r.status.is_terminal())
            .collect();

        if terminal.is_empty() {
            return 1.0;
        }

        let successful = terminal.iter().filter(|r| r.status.is_success()).count();
        successful as f64 / terminal.len() as f64
    }

    /// Get average execution duration (for completed executions)
    #[must_use]
    pub fn average_duration(&self) -> Option<Duration> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        let durations: Vec<_> = records.values().filter_map(|r| r.duration()).collect();

        if durations.is_empty() {
            return None;
        }

        let total: Duration = durations.iter().sum();
        Some(total / durations.len() as u32)
    }

    /// Get total tokens used across all executions
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records.values().map(|r| r.total_tokens).sum()
    }

    /// Remove a specific record
    pub fn remove(&self, thread_id: &str) -> Option<ExecutionRecord> {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        records.remove(thread_id)
    }

    /// Clear all records
    pub fn clear(&self) {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        records.clear();
    }

    /// Clear only completed records
    pub fn clear_completed(&self) {
        let mut records = self.records.write().unwrap_or_else(|e| e.into_inner());
        records.retain(|_, r| !r.status.is_terminal());
    }

    /// Serialize to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        let records_vec: Vec<_> = records.values().collect();
        serde_json::to_string_pretty(&records_vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ============================================================================
    // ExecutionStatus Tests
    // ============================================================================

    mod execution_status {
        use super::*;

        #[test]
        fn running_is_not_terminal() {
            assert!(!ExecutionStatus::Running.is_terminal());
        }

        #[test]
        fn completed_is_terminal() {
            assert!(ExecutionStatus::Completed.is_terminal());
        }

        #[test]
        fn failed_is_terminal() {
            assert!(ExecutionStatus::Failed.is_terminal());
        }

        #[test]
        fn interrupted_is_terminal() {
            assert!(ExecutionStatus::Interrupted.is_terminal());
        }

        #[test]
        fn timed_out_is_terminal() {
            assert!(ExecutionStatus::TimedOut.is_terminal());
        }

        #[test]
        fn running_is_running() {
            assert!(ExecutionStatus::Running.is_running());
        }

        #[test]
        fn terminal_statuses_are_not_running() {
            assert!(!ExecutionStatus::Completed.is_running());
            assert!(!ExecutionStatus::Failed.is_running());
            assert!(!ExecutionStatus::Interrupted.is_running());
            assert!(!ExecutionStatus::TimedOut.is_running());
        }

        #[test]
        fn only_completed_is_success() {
            assert!(ExecutionStatus::Completed.is_success());
            assert!(!ExecutionStatus::Running.is_success());
            assert!(!ExecutionStatus::Failed.is_success());
            assert!(!ExecutionStatus::Interrupted.is_success());
            assert!(!ExecutionStatus::TimedOut.is_success());
        }

        #[test]
        fn display_formats_correctly() {
            assert_eq!(ExecutionStatus::Running.to_string(), "Running");
            assert_eq!(ExecutionStatus::Completed.to_string(), "Completed");
            assert_eq!(ExecutionStatus::Failed.to_string(), "Failed");
            assert_eq!(ExecutionStatus::Interrupted.to_string(), "Interrupted");
            assert_eq!(ExecutionStatus::TimedOut.to_string(), "Timed Out");
        }

        #[test]
        fn serialization_roundtrip() {
            for status in [
                ExecutionStatus::Running,
                ExecutionStatus::Completed,
                ExecutionStatus::Failed,
                ExecutionStatus::Interrupted,
                ExecutionStatus::TimedOut,
            ] {
                let json = serde_json::to_string(&status).expect("serialize");
                let restored: ExecutionStatus = serde_json::from_str(&json).expect("deserialize");
                assert_eq!(restored, status);
            }
        }

        #[test]
        fn equality() {
            assert_eq!(ExecutionStatus::Running, ExecutionStatus::Running);
            assert_ne!(ExecutionStatus::Running, ExecutionStatus::Completed);
        }

        #[test]
        fn copy_semantics() {
            let status = ExecutionStatus::Completed;
            let copied = status;
            assert_eq!(status, copied);
        }
    }

    // ============================================================================
    // ExecutionRecord Tests
    // ============================================================================

    mod execution_record {
        use super::*;

        #[test]
        fn new_creates_running_record() {
            let record = ExecutionRecord::new("thread_1", "graph_1", "1.0.0");

            assert_eq!(record.thread_id, "thread_1");
            assert_eq!(record.graph_id, "graph_1");
            assert_eq!(record.graph_version, "1.0.0");
            assert_eq!(record.status, ExecutionStatus::Running);
            assert!(record.completed_at.is_none());
            assert!(record.final_state.is_none());
            assert!(record.nodes_executed.is_empty());
            assert_eq!(record.total_tokens, 0);
            assert!(record.error.is_none());
            assert!(record.custom.is_empty());
        }

        #[test]
        fn complete_sets_correct_status_and_state() {
            let mut record = ExecutionRecord::new("t", "g", "v");
            let final_state = json!({"result": "success"});

            record.complete(Some(final_state.clone()));

            assert_eq!(record.status, ExecutionStatus::Completed);
            assert!(record.completed_at.is_some());
            assert_eq!(record.final_state, Some(final_state));
        }

        #[test]
        fn complete_with_none_state() {
            let mut record = ExecutionRecord::new("t", "g", "v");

            record.complete(None);

            assert_eq!(record.status, ExecutionStatus::Completed);
            assert!(record.final_state.is_none());
        }

        #[test]
        fn fail_sets_correct_status_and_error() {
            let mut record = ExecutionRecord::new("t", "g", "v");

            record.fail("Something went wrong");

            assert_eq!(record.status, ExecutionStatus::Failed);
            assert!(record.completed_at.is_some());
            assert_eq!(record.error, Some("Something went wrong".to_string()));
        }

        #[test]
        fn interrupt_sets_correct_status() {
            let mut record = ExecutionRecord::new("t", "g", "v");

            record.interrupt();

            assert_eq!(record.status, ExecutionStatus::Interrupted);
            assert!(record.completed_at.is_some());
        }

        #[test]
        fn timeout_sets_correct_status() {
            let mut record = ExecutionRecord::new("t", "g", "v");

            record.timeout();

            assert_eq!(record.status, ExecutionStatus::TimedOut);
            assert!(record.completed_at.is_some());
        }

        #[test]
        fn record_node_adds_to_list() {
            let mut record = ExecutionRecord::new("t", "g", "v");

            record.record_node("node_1");
            record.record_node("node_2");
            record.record_node("node_3");

            assert_eq!(record.nodes_executed, vec!["node_1", "node_2", "node_3"]);
        }

        #[test]
        fn add_tokens_accumulates() {
            let mut record = ExecutionRecord::new("t", "g", "v");

            record.add_tokens(100);
            record.add_tokens(50);
            record.add_tokens(25);

            assert_eq!(record.total_tokens, 175);
        }

        #[test]
        fn duration_returns_none_for_running() {
            let record = ExecutionRecord::new("t", "g", "v");
            assert!(record.duration().is_none());
        }

        #[test]
        fn duration_returns_some_for_completed() {
            let mut record = ExecutionRecord::new("t", "g", "v");
            std::thread::sleep(Duration::from_millis(10));
            record.complete(None);

            let duration = record.duration().expect("should have duration");
            assert!(duration >= Duration::from_millis(10));
        }

        #[test]
        fn elapsed_returns_duration_even_for_running() {
            let record = ExecutionRecord::new("t", "g", "v");
            std::thread::sleep(Duration::from_millis(5));

            let elapsed = record.elapsed();
            assert!(elapsed >= Duration::from_millis(4)); // Allow some tolerance
        }

        #[test]
        fn elapsed_returns_fixed_duration_for_completed() {
            let mut record = ExecutionRecord::new("t", "g", "v");
            std::thread::sleep(Duration::from_millis(10));
            record.complete(None);

            let elapsed1 = record.elapsed();
            std::thread::sleep(Duration::from_millis(10));
            let elapsed2 = record.elapsed();

            // Should be approximately the same since completed_at is set
            assert!((elapsed1.as_millis() as i64 - elapsed2.as_millis() as i64).abs() < 5);
        }

        #[test]
        fn set_custom_adds_metadata() {
            let mut record = ExecutionRecord::new("t", "g", "v");

            record.set_custom("key1", json!("value1"));
            record.set_custom("key2", json!(42));

            assert_eq!(record.custom.get("key1"), Some(&json!("value1")));
            assert_eq!(record.custom.get("key2"), Some(&json!(42)));
        }

        #[test]
        fn set_custom_overwrites_existing() {
            let mut record = ExecutionRecord::new("t", "g", "v");

            record.set_custom("key", json!("old"));
            record.set_custom("key", json!("new"));

            assert_eq!(record.custom.get("key"), Some(&json!("new")));
        }

        #[test]
        fn serialization_roundtrip() {
            let mut record = ExecutionRecord::new("thread_1", "my_graph", "2.0.0");
            record.record_node("start");
            record.record_node("process");
            record.add_tokens(500);
            record.set_custom("env", json!("production"));
            record.complete(Some(json!({"output": "done"})));

            let json = serde_json::to_string(&record).expect("serialize");
            let restored: ExecutionRecord = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(restored.thread_id, record.thread_id);
            assert_eq!(restored.graph_id, record.graph_id);
            assert_eq!(restored.graph_version, record.graph_version);
            assert_eq!(restored.status, record.status);
            assert_eq!(restored.nodes_executed, record.nodes_executed);
            assert_eq!(restored.total_tokens, record.total_tokens);
            assert_eq!(restored.final_state, record.final_state);
        }
    }

    // ============================================================================
    // ExecutionRegistry Tests
    // ============================================================================

    mod execution_registry {
        use super::*;

        #[test]
        fn new_creates_empty_registry() {
            let registry = ExecutionRegistry::new();
            assert_eq!(registry.count(), 0);
        }

        #[test]
        fn default_creates_empty_registry() {
            let registry = ExecutionRegistry::default();
            assert_eq!(registry.count(), 0);
        }

        #[test]
        fn with_max_records_limits_records() {
            let registry = ExecutionRegistry::with_max_records(3);

            // Add 5 records, complete them so they can be pruned
            for i in 0..5 {
                let tid = format!("thread_{i}");
                registry.record_start(&tid, "g", "v");
                registry.record_completion(&tid, None);
            }

            // Should have pruned to max_records
            assert!(registry.count() <= 3);
        }

        #[test]
        fn record_start_creates_running_record() {
            let registry = ExecutionRegistry::new();

            registry.record_start("thread_1", "graph_1", "1.0.0");

            let record = registry.get("thread_1").expect("should exist");
            assert_eq!(record.status, ExecutionStatus::Running);
            assert_eq!(record.graph_id, "graph_1");
            assert_eq!(record.graph_version, "1.0.0");
        }

        #[test]
        fn record_node_updates_record() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_node("t1", "node_a");
            registry.record_node("t1", "node_b");

            let record = registry.get("t1").expect("should exist");
            assert_eq!(record.nodes_executed, vec!["node_a", "node_b"]);
        }

        #[test]
        fn record_node_ignores_unknown_thread() {
            let registry = ExecutionRegistry::new();
            // Should not panic
            registry.record_node("unknown", "node");
            assert_eq!(registry.count(), 0);
        }

        #[test]
        fn record_tokens_accumulates() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_tokens("t1", 100);
            registry.record_tokens("t1", 200);

            let record = registry.get("t1").expect("should exist");
            assert_eq!(record.total_tokens, 300);
        }

        #[test]
        fn record_completion_updates_status() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_completion("t1", Some(json!({"done": true})));

            let record = registry.get("t1").expect("should exist");
            assert_eq!(record.status, ExecutionStatus::Completed);
            assert_eq!(record.final_state, Some(json!({"done": true})));
        }

        #[test]
        fn record_failure_updates_status() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_failure("t1", "Error message");

            let record = registry.get("t1").expect("should exist");
            assert_eq!(record.status, ExecutionStatus::Failed);
            assert_eq!(record.error, Some("Error message".to_string()));
        }

        #[test]
        fn record_interrupt_updates_status() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_interrupt("t1");

            let record = registry.get("t1").expect("should exist");
            assert_eq!(record.status, ExecutionStatus::Interrupted);
        }

        #[test]
        fn record_timeout_updates_status() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_timeout("t1");

            let record = registry.get("t1").expect("should exist");
            assert_eq!(record.status, ExecutionStatus::TimedOut);
        }

        #[test]
        fn get_returns_none_for_unknown() {
            let registry = ExecutionRegistry::new();
            assert!(registry.get("unknown").is_none());
        }

        #[test]
        fn list_running_filters_correctly() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_start("t2", "g", "v");
            registry.record_start("t3", "g", "v");
            registry.record_completion("t2", None);

            let running = registry.list_running();
            assert_eq!(running.len(), 2);
            assert!(running.iter().all(|r| r.status.is_running()));
        }

        #[test]
        fn list_by_status_filters_correctly() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_completion("t1", None);

            registry.record_start("t2", "g", "v");
            registry.record_failure("t2", "error");

            registry.record_start("t3", "g", "v");

            let completed = registry.list_by_status(ExecutionStatus::Completed);
            assert_eq!(completed.len(), 1);
            assert_eq!(completed[0].thread_id, "t1");

            let failed = registry.list_by_status(ExecutionStatus::Failed);
            assert_eq!(failed.len(), 1);
            assert_eq!(failed[0].thread_id, "t2");
        }

        #[test]
        fn list_by_graph_filters_correctly() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "graph_a", "v");
            registry.record_start("t2", "graph_b", "v");
            registry.record_start("t3", "graph_a", "v");

            let graph_a = registry.list_by_graph("graph_a");
            assert_eq!(graph_a.len(), 2);
            assert!(graph_a.iter().all(|r| r.graph_id == "graph_a"));
        }

        #[test]
        fn list_recent_returns_sorted() {
            let registry = ExecutionRegistry::new();

            // Add records with small delays to ensure different timestamps
            registry.record_start("oldest", "g", "v");
            std::thread::sleep(Duration::from_millis(2));
            registry.record_start("middle", "g", "v");
            std::thread::sleep(Duration::from_millis(2));
            registry.record_start("newest", "g", "v");

            let recent = registry.list_recent(3);
            assert_eq!(recent.len(), 3);
            // Most recent should be first
            assert!(recent[0].started_at >= recent[1].started_at);
            assert!(recent[1].started_at >= recent[2].started_at);
        }

        #[test]
        fn list_recent_respects_limit() {
            let registry = ExecutionRegistry::new();

            for i in 0..10 {
                registry.record_start(format!("t{i}"), "g", "v");
            }

            let recent = registry.list_recent(3);
            assert_eq!(recent.len(), 3);
        }

        #[test]
        fn list_all_returns_all_records() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_start("t2", "g", "v");
            registry.record_start("t3", "g", "v");

            let all = registry.list_all();
            assert_eq!(all.len(), 3);
        }

        #[test]
        fn count_returns_total() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_start("t2", "g", "v");

            assert_eq!(registry.count(), 2);
        }

        #[test]
        fn count_by_status_returns_correct_count() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_start("t2", "g", "v");
            registry.record_start("t3", "g", "v");
            registry.record_completion("t1", None);
            registry.record_completion("t2", None);

            assert_eq!(registry.count_by_status(ExecutionStatus::Completed), 2);
            assert_eq!(registry.count_by_status(ExecutionStatus::Running), 1);
        }

        #[test]
        fn success_rate_calculates_correctly() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_completion("t1", None);

            registry.record_start("t2", "g", "v");
            registry.record_completion("t2", None);

            registry.record_start("t3", "g", "v");
            registry.record_failure("t3", "error");

            registry.record_start("t4", "g", "v"); // Still running, not counted

            // 2 successes out of 3 terminal = 66.67%
            let rate = registry.success_rate();
            assert!((rate - 0.6666).abs() < 0.01);
        }

        #[test]
        fn success_rate_returns_one_for_no_terminal() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_start("t2", "g", "v");

            // All running, no terminal, should return 1.0
            assert_eq!(registry.success_rate(), 1.0);
        }

        #[test]
        fn success_rate_returns_one_for_empty() {
            let registry = ExecutionRegistry::new();
            assert_eq!(registry.success_rate(), 1.0);
        }

        #[test]
        fn average_duration_returns_none_for_no_completed() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");

            assert!(registry.average_duration().is_none());
        }

        #[test]
        fn average_duration_calculates_for_completed() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            std::thread::sleep(Duration::from_millis(10));
            registry.record_completion("t1", None);

            registry.record_start("t2", "g", "v");
            std::thread::sleep(Duration::from_millis(20));
            registry.record_completion("t2", None);

            let avg = registry.average_duration().expect("should have average");
            // Average should be between 10ms and 20ms (plus some overhead)
            assert!(avg >= Duration::from_millis(10));
            assert!(avg <= Duration::from_millis(50)); // Allow for test overhead
        }

        #[test]
        fn total_tokens_sums_all() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_tokens("t1", 100);

            registry.record_start("t2", "g", "v");
            registry.record_tokens("t2", 200);

            registry.record_start("t3", "g", "v");
            registry.record_tokens("t3", 150);

            assert_eq!(registry.total_tokens(), 450);
        }

        #[test]
        fn remove_deletes_record() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_start("t2", "g", "v");

            let removed = registry.remove("t1");
            assert!(removed.is_some());
            assert_eq!(removed.unwrap().thread_id, "t1");
            assert!(registry.get("t1").is_none());
            assert!(registry.get("t2").is_some());
        }

        #[test]
        fn remove_returns_none_for_unknown() {
            let registry = ExecutionRegistry::new();
            assert!(registry.remove("unknown").is_none());
        }

        #[test]
        fn clear_removes_all() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_start("t2", "g", "v");
            registry.record_start("t3", "g", "v");

            registry.clear();

            assert_eq!(registry.count(), 0);
        }

        #[test]
        fn clear_completed_keeps_running() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "g", "v");
            registry.record_completion("t1", None);

            registry.record_start("t2", "g", "v");
            registry.record_failure("t2", "error");

            registry.record_start("t3", "g", "v"); // Still running

            registry.clear_completed();

            assert_eq!(registry.count(), 1);
            let remaining = registry.get("t3").expect("should exist");
            assert!(remaining.status.is_running());
        }

        #[test]
        fn to_json_serializes() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "graph_a", "1.0");
            registry.record_start("t2", "graph_b", "2.0");

            let json_str = registry.to_json().expect("serialization should succeed");
            assert!(json_str.contains("t1") || json_str.contains("t2"));
            assert!(json_str.contains("graph_a") || json_str.contains("graph_b"));
        }

        #[test]
        fn clone_shares_underlying_data() {
            let registry = ExecutionRegistry::new();
            registry.record_start("t1", "g", "v");

            let cloned = registry.clone();
            cloned.record_start("t2", "g", "v");

            // Both should see both records
            assert_eq!(registry.count(), 2);
            assert_eq!(cloned.count(), 2);
        }

        #[test]
        fn prune_oldest_removes_completed_first() {
            let registry = ExecutionRegistry::with_max_records(2);

            registry.record_start("t1", "g", "v");
            registry.record_completion("t1", None);

            registry.record_start("t2", "g", "v");
            // t2 is still running

            registry.record_start("t3", "g", "v");
            registry.record_completion("t3", None);

            // Should prune t1 (oldest completed) rather than t2 (running)
            assert!(registry.count() <= 2);
            // Running record should be preserved
            assert!(registry.get("t2").is_some() || registry.count() < 2);
        }

        #[test]
        fn overwrite_thread_id() {
            let registry = ExecutionRegistry::new();

            registry.record_start("t1", "graph_old", "v1");
            registry.record_start("t1", "graph_new", "v2");

            let record = registry.get("t1").expect("should exist");
            // Second start overwrites the first
            assert_eq!(record.graph_id, "graph_new");
            assert_eq!(registry.count(), 1);
        }
    }
}
