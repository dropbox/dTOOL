// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! AI Self-Knowledge - Unified AI Self-Awareness API
//!
//! This module provides a unified API for AI agents to understand themselves
//! by combining graph registry, execution history, versioning, and state tracking.

use std::time::{Duration, SystemTime};

use super::{
    ExecutionRecord, ExecutionRegistry, ExecutionStatus, GraphDiff, GraphRegistry, GraphVersion,
    RegistryEntry, StateDiff, StateRegistry, StateSnapshot, VersionStore,
};

// ============================================================================
// AISelfKnowledge - Unified AI Self-Awareness API
// ============================================================================

/// Unified API for AI self-awareness
///
/// AISelfKnowledge combines graph registry, execution registry, version tracking,
/// and state registry capabilities into a single API that AI agents can use
/// to understand themselves.
///
/// # Example
///
/// ```rust,ignore
/// let knowledge = AISelfKnowledge::new()
///     .with_graph_registry(graph_registry)
///     .with_execution_registry(execution_registry)
///     .with_version_store(version_store)
///     .with_state_registry(state_registry);
///
/// // AI can now query:
/// let graph_info = knowledge.graph_info("my_graph");
/// let execution_history = knowledge.recent_executions(10);
/// let state_at_time = knowledge.state_at(thread_id, time);
/// ```
#[derive(Debug, Clone, Default)]
pub struct AISelfKnowledge {
    /// Graph registry for registered graphs
    pub graph_registry: GraphRegistry,

    /// Execution registry for execution history
    pub execution_registry: ExecutionRegistry,

    /// Version store for graph versioning
    pub version_store: VersionStore,

    /// State registry for state snapshots
    pub state_registry: StateRegistry,
}

impl AISelfKnowledge {
    /// Create a new AISelfKnowledge instance
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the graph registry
    #[must_use]
    pub fn with_graph_registry(mut self, registry: GraphRegistry) -> Self {
        self.graph_registry = registry;
        self
    }

    /// Set the execution registry
    #[must_use]
    pub fn with_execution_registry(mut self, registry: ExecutionRegistry) -> Self {
        self.execution_registry = registry;
        self
    }

    /// Set the version store
    #[must_use]
    pub fn with_version_store(mut self, store: VersionStore) -> Self {
        self.version_store = store;
        self
    }

    /// Set the state registry
    #[must_use]
    pub fn with_state_registry(mut self, registry: StateRegistry) -> Self {
        self.state_registry = registry;
        self
    }

    // ========== Graph Information ==========

    /// Get information about a specific graph
    #[must_use]
    pub fn graph_info(&self, graph_id: &str) -> Option<RegistryEntry> {
        self.graph_registry.get(graph_id)
    }

    /// List all registered graphs
    #[must_use]
    pub fn list_graphs(&self) -> Vec<RegistryEntry> {
        self.graph_registry.list_graphs()
    }

    /// List active graphs
    #[must_use]
    pub fn active_graphs(&self) -> Vec<RegistryEntry> {
        self.graph_registry.list_active()
    }

    /// Find graphs by tag
    #[must_use]
    pub fn find_graphs_by_tag(&self, tag: &str) -> Vec<RegistryEntry> {
        self.graph_registry.find_by_tag(tag)
    }

    // ========== Execution Information ==========

    /// Get recent executions across all graphs
    #[must_use]
    pub fn recent_executions(&self, limit: usize) -> Vec<ExecutionRecord> {
        self.execution_registry.list_recent(limit)
    }

    /// Get currently running executions
    #[must_use]
    pub fn running_executions(&self) -> Vec<ExecutionRecord> {
        self.execution_registry.list_running()
    }

    /// Get executions by status
    #[must_use]
    pub fn executions_by_status(&self, status: ExecutionStatus) -> Vec<ExecutionRecord> {
        self.execution_registry.list_by_status(status)
    }

    /// Get execution details
    #[must_use]
    pub fn execution(&self, thread_id: &str) -> Option<ExecutionRecord> {
        self.execution_registry.get(thread_id)
    }

    /// Get executions for a specific graph
    #[must_use]
    pub fn executions_for_graph(&self, graph_id: &str) -> Vec<ExecutionRecord> {
        self.execution_registry.list_by_graph(graph_id)
    }

    /// Get overall success rate
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        self.execution_registry.success_rate()
    }

    /// Get overall average execution duration
    #[must_use]
    pub fn average_duration(&self) -> Option<Duration> {
        self.execution_registry.average_duration()
    }

    // ========== Version Information ==========

    /// Get the latest version of a graph
    #[must_use]
    pub fn latest_version(&self, graph_id: &str) -> Option<GraphVersion> {
        self.version_store.get_latest(graph_id)
    }

    /// Get version history for a graph
    #[must_use]
    pub fn version_history(&self, graph_id: &str, limit: usize) -> Vec<GraphVersion> {
        self.version_store.version_history(graph_id, limit)
    }

    /// Check if a graph has changed
    #[must_use]
    pub fn has_graph_changed(&self, graph_id: &str, current_hash: &str) -> bool {
        self.version_store.has_changed(graph_id, current_hash)
    }

    /// Compare two versions
    #[must_use]
    pub fn compare_versions(&self, v1: &GraphVersion, v2: &GraphVersion) -> GraphDiff {
        v1.diff(v2)
    }

    // ========== State Information ==========

    /// Get state history for a thread
    #[must_use]
    pub fn state_history(&self, thread_id: &str) -> Vec<StateSnapshot> {
        self.state_registry.get_history(thread_id)
    }

    /// Get latest state for a thread
    #[must_use]
    pub fn latest_state(&self, thread_id: &str) -> Option<StateSnapshot> {
        self.state_registry.get_latest(thread_id)
    }

    /// Get state at a specific time
    #[must_use]
    pub fn state_at(&self, thread_id: &str, time: SystemTime) -> Option<StateSnapshot> {
        self.state_registry.get_at_time(thread_id, time)
    }

    /// Get state at a specific checkpoint
    #[must_use]
    pub fn state_at_checkpoint(
        &self,
        thread_id: &str,
        checkpoint_id: &str,
    ) -> Option<StateSnapshot> {
        self.state_registry
            .get_at_checkpoint(thread_id, checkpoint_id)
    }

    /// Get state changes for a thread
    #[must_use]
    pub fn state_changes(&self, thread_id: &str) -> Vec<StateDiff> {
        self.state_registry.get_changes(thread_id)
    }

    /// Get recent state snapshots (across threads)
    #[must_use]
    pub fn recent_states(&self, limit: usize) -> Vec<StateSnapshot> {
        self.state_registry.get_recent(limit)
    }

    // ========== High-Level Queries ==========

    /// Answer a question about self-knowledge
    ///
    /// Provides natural language answers to common AI self-awareness questions.
    #[must_use]
    pub fn query(&self, question: &str) -> String {
        let q = question.to_lowercase();

        // Graph questions
        if q.contains("what graphs") || q.contains("registered graphs") {
            let graphs = self.list_graphs();
            return format!(
                "{} graphs registered: {:?}",
                graphs.len(),
                graphs.iter().map(|g| &g.metadata.name).collect::<Vec<_>>()
            );
        }

        // Running questions
        if q.contains("running") || q.contains("active executions") {
            let running = self.running_executions();
            return format!(
                "{} executions running: {:?}",
                running.len(),
                running.iter().map(|r| &r.thread_id).collect::<Vec<_>>()
            );
        }

        // Version questions
        if q.contains("version") && q.contains("graph") {
            return "Use latest_version(graph_id) to get version info".to_string();
        }

        // State questions
        if q.contains("state") || q.contains("history") {
            return "Use state_history(thread_id) to get state snapshots".to_string();
        }

        // Performance questions
        if q.contains("performance") || q.contains("success rate") {
            return "Use success_rate(graph_id) to get success metrics".to_string();
        }

        "Unknown question. Try asking about: graphs, executions, versions, or state.".to_string()
    }

    /// Generate a summary of current self-knowledge
    #[must_use]
    pub fn summary(&self) -> String {
        let mut summary = String::new();

        // Graphs
        let graphs = self.list_graphs();
        summary.push_str(&format!("Registered graphs: {}\n", graphs.len()));

        // Running
        let running = self.running_executions();
        summary.push_str(&format!("Running executions: {}\n", running.len()));

        // States tracked
        let thread_ids = self.state_registry.thread_ids();
        summary.push_str(&format!(
            "Threads with state history: {}\n",
            thread_ids.len()
        ));

        summary
    }

    /// Export all knowledge as JSON for AI consumption
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "graphs": {
                "count": self.list_graphs().len(),
                "active": self.active_graphs().len()
            },
            "executions": {
                "running": self.running_executions().len(),
                "success_rate": self.success_rate()
            },
            "versions": {
                "tracked": self.version_store.version_count_total()
            },
            "states": {
                "threads_tracked": self.state_registry.thread_ids().len()
            }
        })
    }

    /// Get total version count across all graphs
    #[must_use]
    pub fn version_count_total(&self) -> usize {
        self.version_store.version_count_total()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ============================================================================
    // AISelfKnowledge Construction Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_new_creates_empty_instance() {
        let knowledge = AISelfKnowledge::new();
        assert!(knowledge.list_graphs().is_empty());
        assert!(knowledge.running_executions().is_empty());
        // success_rate returns 1.0 when no executions (no failures = success)
        assert_eq!(knowledge.success_rate(), 1.0);
    }

    #[test]
    fn ai_self_knowledge_default_creates_empty_instance() {
        let knowledge = AISelfKnowledge::default();
        assert!(knowledge.list_graphs().is_empty());
    }

    #[test]
    fn ai_self_knowledge_with_graph_registry() {
        let registry = GraphRegistry::default();
        let knowledge = AISelfKnowledge::new().with_graph_registry(registry);
        // Registry is set, but empty
        assert!(knowledge.list_graphs().is_empty());
    }

    #[test]
    fn ai_self_knowledge_with_execution_registry() {
        let registry = ExecutionRegistry::default();
        let knowledge = AISelfKnowledge::new().with_execution_registry(registry);
        assert!(knowledge.running_executions().is_empty());
    }

    #[test]
    fn ai_self_knowledge_with_version_store() {
        let store = VersionStore::default();
        let knowledge = AISelfKnowledge::new().with_version_store(store);
        assert_eq!(knowledge.version_count_total(), 0);
    }

    #[test]
    fn ai_self_knowledge_with_state_registry() {
        let registry = StateRegistry::default();
        let knowledge = AISelfKnowledge::new().with_state_registry(registry);
        assert!(knowledge.state_registry.thread_ids().is_empty());
    }

    #[test]
    fn ai_self_knowledge_builder_chain() {
        let knowledge = AISelfKnowledge::new()
            .with_graph_registry(GraphRegistry::default())
            .with_execution_registry(ExecutionRegistry::default())
            .with_version_store(VersionStore::default())
            .with_state_registry(StateRegistry::default());
        assert!(knowledge.list_graphs().is_empty());
    }

    // ============================================================================
    // Graph Information Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_graph_info_returns_none_for_missing() {
        let knowledge = AISelfKnowledge::new();
        assert!(knowledge.graph_info("nonexistent").is_none());
    }

    #[test]
    fn ai_self_knowledge_list_graphs_empty() {
        let knowledge = AISelfKnowledge::new();
        let graphs = knowledge.list_graphs();
        assert!(graphs.is_empty());
    }

    #[test]
    fn ai_self_knowledge_active_graphs_empty() {
        let knowledge = AISelfKnowledge::new();
        let active = knowledge.active_graphs();
        assert!(active.is_empty());
    }

    #[test]
    fn ai_self_knowledge_find_graphs_by_tag_empty() {
        let knowledge = AISelfKnowledge::new();
        let found = knowledge.find_graphs_by_tag("test-tag");
        assert!(found.is_empty());
    }

    // ============================================================================
    // Execution Information Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_recent_executions_empty() {
        let knowledge = AISelfKnowledge::new();
        let recent = knowledge.recent_executions(10);
        assert!(recent.is_empty());
    }

    #[test]
    fn ai_self_knowledge_running_executions_empty() {
        let knowledge = AISelfKnowledge::new();
        let running = knowledge.running_executions();
        assert!(running.is_empty());
    }

    #[test]
    fn ai_self_knowledge_executions_by_status_empty() {
        let knowledge = AISelfKnowledge::new();
        let running = knowledge.executions_by_status(ExecutionStatus::Running);
        assert!(running.is_empty());
    }

    #[test]
    fn ai_self_knowledge_execution_returns_none_for_missing() {
        let knowledge = AISelfKnowledge::new();
        assert!(knowledge.execution("thread-123").is_none());
    }

    #[test]
    fn ai_self_knowledge_executions_for_graph_empty() {
        let knowledge = AISelfKnowledge::new();
        let executions = knowledge.executions_for_graph("graph-1");
        assert!(executions.is_empty());
    }

    #[test]
    fn ai_self_knowledge_success_rate_one_when_empty() {
        let knowledge = AISelfKnowledge::new();
        // Returns 1.0 when no executions (no failures = success)
        assert_eq!(knowledge.success_rate(), 1.0);
    }

    #[test]
    fn ai_self_knowledge_average_duration_none_when_empty() {
        let knowledge = AISelfKnowledge::new();
        assert!(knowledge.average_duration().is_none());
    }

    // ============================================================================
    // Version Information Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_latest_version_returns_none_for_missing() {
        let knowledge = AISelfKnowledge::new();
        assert!(knowledge.latest_version("graph-1").is_none());
    }

    #[test]
    fn ai_self_knowledge_version_history_empty() {
        let knowledge = AISelfKnowledge::new();
        let history = knowledge.version_history("graph-1", 10);
        assert!(history.is_empty());
    }

    #[test]
    fn ai_self_knowledge_has_graph_changed_false_when_no_version() {
        let knowledge = AISelfKnowledge::new();
        // When no version exists, has_changed returns true (no baseline)
        let changed = knowledge.has_graph_changed("graph-1", "abc123");
        assert!(changed);
    }

    #[test]
    fn ai_self_knowledge_version_count_total_zero() {
        let knowledge = AISelfKnowledge::new();
        assert_eq!(knowledge.version_count_total(), 0);
    }

    // ============================================================================
    // State Information Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_state_history_empty() {
        let knowledge = AISelfKnowledge::new();
        let history = knowledge.state_history("thread-1");
        assert!(history.is_empty());
    }

    #[test]
    fn ai_self_knowledge_latest_state_returns_none_for_missing() {
        let knowledge = AISelfKnowledge::new();
        assert!(knowledge.latest_state("thread-1").is_none());
    }

    #[test]
    fn ai_self_knowledge_state_at_returns_none_for_missing() {
        let knowledge = AISelfKnowledge::new();
        let result = knowledge.state_at("thread-1", SystemTime::now());
        assert!(result.is_none());
    }

    #[test]
    fn ai_self_knowledge_state_at_checkpoint_returns_none_for_missing() {
        let knowledge = AISelfKnowledge::new();
        let result = knowledge.state_at_checkpoint("thread-1", "checkpoint-1");
        assert!(result.is_none());
    }

    #[test]
    fn ai_self_knowledge_state_changes_empty() {
        let knowledge = AISelfKnowledge::new();
        let changes = knowledge.state_changes("thread-1");
        assert!(changes.is_empty());
    }

    #[test]
    fn ai_self_knowledge_recent_states_empty() {
        let knowledge = AISelfKnowledge::new();
        let recent = knowledge.recent_states(10);
        assert!(recent.is_empty());
    }

    // ============================================================================
    // Query Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_query_graphs() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("what graphs are registered?");
        assert!(response.contains("0 graphs registered"));
    }

    #[test]
    fn ai_self_knowledge_query_running() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("what is running?");
        assert!(response.contains("0 executions running"));
    }

    #[test]
    fn ai_self_knowledge_query_active_executions() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("show active executions");
        assert!(response.contains("executions running"));
    }

    #[test]
    fn ai_self_knowledge_query_version() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("what version is the graph?");
        assert!(response.contains("latest_version"));
    }

    #[test]
    fn ai_self_knowledge_query_state() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("show state history");
        assert!(response.contains("state_history"));
    }

    #[test]
    fn ai_self_knowledge_query_performance() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("what is the performance?");
        assert!(response.contains("success_rate"));
    }

    #[test]
    fn ai_self_knowledge_query_success_rate() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("show success rate");
        assert!(response.contains("success_rate"));
    }

    #[test]
    fn ai_self_knowledge_query_unknown() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("something random unrelated");
        assert!(response.contains("Unknown question"));
    }

    #[test]
    fn ai_self_knowledge_query_case_insensitive() {
        let knowledge = AISelfKnowledge::new();
        let response1 = knowledge.query("WHAT GRAPHS");
        let response2 = knowledge.query("what graphs");
        assert!(response1.contains("graphs registered"));
        assert!(response2.contains("graphs registered"));
    }

    // ============================================================================
    // Summary Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_summary_contains_graphs() {
        let knowledge = AISelfKnowledge::new();
        let summary = knowledge.summary();
        assert!(summary.contains("Registered graphs: 0"));
    }

    #[test]
    fn ai_self_knowledge_summary_contains_executions() {
        let knowledge = AISelfKnowledge::new();
        let summary = knowledge.summary();
        assert!(summary.contains("Running executions: 0"));
    }

    #[test]
    fn ai_self_knowledge_summary_contains_threads() {
        let knowledge = AISelfKnowledge::new();
        let summary = knowledge.summary();
        assert!(summary.contains("Threads with state history: 0"));
    }

    #[test]
    fn ai_self_knowledge_summary_multiline() {
        let knowledge = AISelfKnowledge::new();
        let summary = knowledge.summary();
        let lines: Vec<&str> = summary.lines().collect();
        assert!(lines.len() >= 3);
    }

    // ============================================================================
    // to_json Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_to_json_structure() {
        let knowledge = AISelfKnowledge::new();
        let json = knowledge.to_json();
        assert!(json.is_object());
        assert!(json.get("graphs").is_some());
        assert!(json.get("executions").is_some());
        assert!(json.get("versions").is_some());
        assert!(json.get("states").is_some());
    }

    #[test]
    fn ai_self_knowledge_to_json_graphs_section() {
        let knowledge = AISelfKnowledge::new();
        let json = knowledge.to_json();
        let graphs = json.get("graphs").unwrap();
        assert_eq!(graphs.get("count").unwrap(), 0);
        assert_eq!(graphs.get("active").unwrap(), 0);
    }

    #[test]
    fn ai_self_knowledge_to_json_executions_section() {
        let knowledge = AISelfKnowledge::new();
        let json = knowledge.to_json();
        let executions = json.get("executions").unwrap();
        assert_eq!(executions.get("running").unwrap(), 0);
        // success_rate is 1.0 when empty (no failures)
        assert_eq!(executions.get("success_rate").unwrap(), 1.0);
    }

    #[test]
    fn ai_self_knowledge_to_json_versions_section() {
        let knowledge = AISelfKnowledge::new();
        let json = knowledge.to_json();
        let versions = json.get("versions").unwrap();
        assert_eq!(versions.get("tracked").unwrap(), 0);
    }

    #[test]
    fn ai_self_knowledge_to_json_states_section() {
        let knowledge = AISelfKnowledge::new();
        let json = knowledge.to_json();
        let states = json.get("states").unwrap();
        assert_eq!(states.get("threads_tracked").unwrap(), 0);
    }

    #[test]
    fn ai_self_knowledge_to_json_serializable() {
        let knowledge = AISelfKnowledge::new();
        let json = knowledge.to_json();
        let serialized = serde_json::to_string(&json).unwrap();
        assert!(serialized.contains("graphs"));
        assert!(serialized.contains("executions"));
    }

    // ============================================================================
    // Clone Tests
    // ============================================================================

    #[test]
    fn ai_self_knowledge_clone() {
        let knowledge = AISelfKnowledge::new();
        let cloned = knowledge.clone();
        assert!(cloned.list_graphs().is_empty());
    }

    #[test]
    fn ai_self_knowledge_debug_format() {
        let knowledge = AISelfKnowledge::new();
        let debug = format!("{:?}", knowledge);
        assert!(debug.contains("AISelfKnowledge"));
    }

    // ============================================================================
    // Integration Tests with Registries
    // ============================================================================

    #[test]
    fn ai_self_knowledge_with_state_registry_snapshot() {
        let registry = StateRegistry::default();
        let state = json!({"key": "value"});
        registry.snapshot("thread-1", "node-1", state);

        let knowledge = AISelfKnowledge::new().with_state_registry(registry);
        let history = knowledge.state_history("thread-1");
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn ai_self_knowledge_with_state_registry_latest() {
        let registry = StateRegistry::default();
        registry.snapshot("thread-1", "node-1", json!({"step": 1}));
        registry.snapshot("thread-1", "node-2", json!({"step": 2}));

        let knowledge = AISelfKnowledge::new().with_state_registry(registry);
        let latest = knowledge.latest_state("thread-1").unwrap();
        let json_str = latest.to_json().unwrap();
        assert!(json_str.contains("step"));
    }

    #[test]
    fn ai_self_knowledge_with_execution_registry_record() {
        let registry = ExecutionRegistry::default();
        registry.record_start("thread-1", "graph-1", "1.0.0");

        let knowledge = AISelfKnowledge::new().with_execution_registry(registry);
        let running = knowledge.running_executions();
        assert_eq!(running.len(), 1);
    }

    #[test]
    fn ai_self_knowledge_with_execution_registry_completion() {
        let registry = ExecutionRegistry::default();
        registry.record_start("thread-1", "graph-1", "1.0.0");
        registry.record_completion("thread-1", None);

        let knowledge = AISelfKnowledge::new().with_execution_registry(registry);
        let running = knowledge.running_executions();
        assert!(running.is_empty());
        assert!(knowledge.success_rate() > 0.0);
    }

    #[test]
    fn ai_self_knowledge_with_version_store_version() {
        use std::collections::HashMap;

        let store = VersionStore::default();
        let version = GraphVersion {
            graph_id: "graph-1".to_string(),
            version: "1.0.0".to_string(),
            content_hash: "hash123".to_string(),
            source_hash: None,
            created_at: SystemTime::now(),
            node_versions: HashMap::new(),
            node_count: 3,
            edge_count: 2,
        };
        store.save(version);

        let knowledge = AISelfKnowledge::new().with_version_store(store);
        let latest = knowledge.latest_version("graph-1");
        assert!(latest.is_some());
        assert_eq!(knowledge.version_count_total(), 1);
    }

    #[test]
    fn ai_self_knowledge_compare_versions() {
        use std::collections::HashMap;

        let v1 = GraphVersion {
            graph_id: "graph-1".to_string(),
            version: "1.0.0".to_string(),
            content_hash: "hash1".to_string(),
            source_hash: None,
            created_at: SystemTime::now(),
            node_versions: HashMap::new(),
            node_count: 1,
            edge_count: 0,
        };
        let v2 = GraphVersion {
            graph_id: "graph-1".to_string(),
            version: "2.0.0".to_string(),
            content_hash: "hash2".to_string(),
            source_hash: None,
            created_at: SystemTime::now(),
            node_versions: HashMap::new(),
            node_count: 2,
            edge_count: 1,
        };

        let knowledge = AISelfKnowledge::new();
        let diff = knowledge.compare_versions(&v1, &v2);
        // Different content hashes should show change
        assert!(diff.content_hash_changed || diff.edges_changed);
    }

    // ============================================================================
    // Edge Cases
    // ============================================================================

    #[test]
    fn ai_self_knowledge_query_empty_string() {
        let knowledge = AISelfKnowledge::new();
        let response = knowledge.query("");
        assert!(response.contains("Unknown question"));
    }

    #[test]
    fn ai_self_knowledge_recent_executions_with_zero_limit() {
        let knowledge = AISelfKnowledge::new();
        let recent = knowledge.recent_executions(0);
        assert!(recent.is_empty());
    }

    #[test]
    fn ai_self_knowledge_version_history_with_zero_limit() {
        let knowledge = AISelfKnowledge::new();
        let history = knowledge.version_history("graph-1", 0);
        assert!(history.is_empty());
    }

    #[test]
    fn ai_self_knowledge_recent_states_with_zero_limit() {
        let knowledge = AISelfKnowledge::new();
        let recent = knowledge.recent_states(0);
        assert!(recent.is_empty());
    }
}
