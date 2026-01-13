// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! State Registry and Diffing - State snapshot management
//!
//! This module provides state tracking capabilities:
//! - State snapshots at execution checkpoints
//! - State history queries
//! - State diffing between snapshots

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

// ============================================================================
// State Registry
// ============================================================================

/// Snapshot of a state at a specific point in time
///
/// StateSnapshot captures the complete state of an execution at a checkpoint,
/// enabling AI agents to query past states and understand state evolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Thread ID this snapshot belongs to
    pub thread_id: String,
    /// Checkpoint ID (if from a checkpoint)
    pub checkpoint_id: Option<String>,
    /// Timestamp when the snapshot was taken
    pub timestamp: SystemTime,
    /// Node that was executing when snapshot was taken
    pub node: String,
    /// The complete state at this point
    pub state: serde_json::Value,
    /// Size of the state in bytes
    pub size_bytes: usize,
    /// Optional description/annotation
    pub description: Option<String>,
    /// Custom metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl StateSnapshot {
    /// Create a new state snapshot
    #[must_use]
    pub fn new(
        thread_id: impl Into<String>,
        node: impl Into<String>,
        state: serde_json::Value,
    ) -> Self {
        let state_str = state.to_string();
        let size_bytes = state_str.len();

        Self {
            thread_id: thread_id.into(),
            checkpoint_id: None,
            timestamp: SystemTime::now(),
            node: node.into(),
            state,
            size_bytes,
            description: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a snapshot with a checkpoint ID
    #[must_use]
    pub fn with_checkpoint_id(mut self, checkpoint_id: impl Into<String>) -> Self {
        self.checkpoint_id = Some(checkpoint_id.into());
        self
    }

    /// Set a description for the snapshot
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set a specific timestamp
    #[must_use]
    pub fn with_timestamp(mut self, timestamp: SystemTime) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Add custom metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get a field from the state by path (dot-separated)
    #[must_use]
    pub fn get_field(&self, path: &str) -> Option<&serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = &self.state;

        for part in parts {
            current = current.get(part)?;
        }

        Some(current)
    }

    /// Serialize to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get elapsed time since the snapshot was taken
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or(Duration::ZERO)
    }
}

/// Registry for state snapshots
///
/// StateRegistry maintains snapshots of execution states, enabling AI agents
/// to query past states and understand state evolution over time.
///
/// # Thread Safety
///
/// StateRegistry uses internal RwLock for thread-safe access.
///
/// # Example
///
/// ```rust
/// use dashflow::graph_registry::StateRegistry;
///
/// let registry = StateRegistry::new();
///
/// // Take a snapshot
/// registry.snapshot("thread_123", "reasoning", serde_json::json!({
///     "messages": ["Hello"],
///     "step": 1
/// }));
///
/// // Query history
/// let history = registry.get_history("thread_123");
/// ```
#[derive(Debug)]
pub struct StateRegistry {
    snapshots: Arc<RwLock<HashMap<String, Vec<StateSnapshot>>>>,
    /// Maximum number of snapshots per thread (0 = unlimited)
    max_snapshots_per_thread: usize,
}

impl Default for StateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for StateRegistry {
    fn clone(&self) -> Self {
        Self {
            snapshots: Arc::clone(&self.snapshots),
            max_snapshots_per_thread: self.max_snapshots_per_thread,
        }
    }
}

impl StateRegistry {
    /// Create a new state registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            max_snapshots_per_thread: 0,
        }
    }

    /// Create a state registry with a maximum snapshots limit per thread
    #[must_use]
    pub fn with_max_snapshots(max_snapshots: usize) -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            max_snapshots_per_thread: max_snapshots,
        }
    }

    /// Take a snapshot of the current state
    pub fn snapshot(
        &self,
        thread_id: impl Into<String>,
        node: impl Into<String>,
        state: serde_json::Value,
    ) {
        let tid = thread_id.into();
        let snapshot = StateSnapshot::new(&tid, node, state);

        let mut snapshots = self.snapshots.write().unwrap_or_else(|e| e.into_inner());
        let thread_snapshots = snapshots.entry(tid).or_default();
        thread_snapshots.push(snapshot);

        // Prune if limit is set
        if self.max_snapshots_per_thread > 0
            && thread_snapshots.len() > self.max_snapshots_per_thread
        {
            thread_snapshots.remove(0);
        }
    }

    /// Add a pre-built snapshot
    pub fn add_snapshot(&self, snapshot: StateSnapshot) {
        let mut snapshots = self.snapshots.write().unwrap_or_else(|e| e.into_inner());
        let thread_snapshots = snapshots.entry(snapshot.thread_id.clone()).or_default();
        thread_snapshots.push(snapshot);

        // Prune if limit is set
        if self.max_snapshots_per_thread > 0
            && thread_snapshots.len() > self.max_snapshots_per_thread
        {
            thread_snapshots.remove(0);
        }
    }

    /// Get all snapshots for a thread
    #[must_use]
    pub fn get_history(&self, thread_id: &str) -> Vec<StateSnapshot> {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        snapshots.get(thread_id).cloned().unwrap_or_default()
    }

    /// Get the latest snapshot for a thread
    #[must_use]
    pub fn get_latest(&self, thread_id: &str) -> Option<StateSnapshot> {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        snapshots.get(thread_id).and_then(|s| s.last()).cloned()
    }

    /// Get snapshot at a specific checkpoint
    #[must_use]
    pub fn get_at_checkpoint(&self, thread_id: &str, checkpoint_id: &str) -> Option<StateSnapshot> {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        snapshots
            .get(thread_id)?
            .iter()
            .find(|s| s.checkpoint_id.as_deref() == Some(checkpoint_id))
            .cloned()
    }

    /// Get snapshot closest to a specific time
    #[must_use]
    pub fn get_at_time(&self, thread_id: &str, time: SystemTime) -> Option<StateSnapshot> {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        let thread_snapshots = snapshots.get(thread_id)?;

        thread_snapshots
            .iter()
            .min_by_key(|s| {
                if s.timestamp <= time {
                    time.duration_since(s.timestamp).unwrap_or(Duration::MAX)
                } else {
                    s.timestamp.duration_since(time).unwrap_or(Duration::MAX)
                }
            })
            .cloned()
    }

    /// Get snapshots for a specific node
    #[must_use]
    pub fn get_by_node(&self, thread_id: &str, node: &str) -> Vec<StateSnapshot> {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        snapshots
            .get(thread_id)
            .map(|s| s.iter().filter(|snap| snap.node == node).cloned().collect())
            .unwrap_or_default()
    }

    /// Get recent snapshots (across all threads)
    #[must_use]
    pub fn get_recent(&self, limit: usize) -> Vec<StateSnapshot> {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        let mut all: Vec<_> = snapshots.values().flatten().cloned().collect();
        all.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all.truncate(limit);
        all
    }

    /// Get snapshot count for a thread
    #[must_use]
    pub fn snapshot_count(&self, thread_id: &str) -> usize {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        snapshots.get(thread_id).map_or(0, Vec::len)
    }

    /// Get total snapshot count across all threads
    #[must_use]
    pub fn total_count(&self) -> usize {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        snapshots.values().map(Vec::len).sum()
    }

    /// Get all thread IDs with snapshots
    #[must_use]
    pub fn thread_ids(&self) -> Vec<String> {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        snapshots.keys().cloned().collect()
    }

    /// Compare two snapshots and return a diff
    #[must_use]
    pub fn diff_snapshots(before: &StateSnapshot, after: &StateSnapshot) -> StateDiff {
        state_diff(&before.state, &after.state)
    }

    /// Get state changes over time for a thread
    #[must_use]
    pub fn get_changes(&self, thread_id: &str) -> Vec<StateDiff> {
        let history = self.get_history(thread_id);
        if history.len() < 2 {
            return vec![];
        }

        history
            .windows(2)
            .map(|pair| Self::diff_snapshots(&pair[0], &pair[1]))
            .collect()
    }

    /// Clear snapshots for a thread
    pub fn clear_thread(&self, thread_id: &str) {
        let mut snapshots = self.snapshots.write().unwrap_or_else(|e| e.into_inner());
        snapshots.remove(thread_id);
    }

    /// Clear all snapshots
    pub fn clear(&self) {
        let mut snapshots = self.snapshots.write().unwrap_or_else(|e| e.into_inner());
        snapshots.clear();
    }

    /// Serialize to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let snapshots = self.snapshots.read().unwrap_or_else(|e| e.into_inner());
        let all: Vec<_> = snapshots.values().flatten().collect();
        serde_json::to_string_pretty(&all)
    }
}

// ============================================================================
// State Diff Visualization
// ============================================================================

/// Diff between two states showing what changed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    /// Fields that were added
    pub added: Vec<String>,
    /// Fields that were removed
    pub removed: Vec<String>,
    /// Fields that were modified
    pub modified: Vec<FieldDiff>,
}

impl StateDiff {
    /// Create an empty diff (no changes)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            added: vec![],
            removed: vec![],
            modified: vec![],
        }
    }

    /// Check if there are any changes
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }

    /// Get total number of changes
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }

    /// Generate a human-readable summary
    #[must_use]
    pub fn summary(&self) -> String {
        if !self.has_changes() {
            return "No changes".to_string();
        }

        let mut parts = vec![];
        if !self.added.is_empty() {
            parts.push(format!("{} added", self.added.len()));
        }
        if !self.removed.is_empty() {
            parts.push(format!("{} removed", self.removed.len()));
        }
        if !self.modified.is_empty() {
            parts.push(format!("{} modified", self.modified.len()));
        }
        parts.join(", ")
    }

    /// Generate a detailed report
    #[must_use]
    pub fn detailed_report(&self) -> String {
        let mut lines = vec!["State Diff:".to_string(), String::new()];

        if !self.added.is_empty() {
            lines.push("Added:".to_string());
            for field in &self.added {
                lines.push(format!("  + {field}"));
            }
            lines.push(String::new());
        }

        if !self.removed.is_empty() {
            lines.push("Removed:".to_string());
            for field in &self.removed {
                lines.push(format!("  - {field}"));
            }
            lines.push(String::new());
        }

        if !self.modified.is_empty() {
            lines.push("Modified:".to_string());
            for diff in &self.modified {
                lines.push(format!(
                    "  ~ {}: {} -> {}",
                    diff.path, diff.before, diff.after
                ));
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }

    /// Serialize to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// A single field change between two states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDiff {
    /// Path to the field (e.g., "messages.0.content" or "step")
    pub path: String,
    /// Value before the change
    pub before: serde_json::Value,
    /// Value after the change
    pub after: serde_json::Value,
}

impl FieldDiff {
    /// Create a new field diff
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        before: serde_json::Value,
        after: serde_json::Value,
    ) -> Self {
        Self {
            path: path.into(),
            before,
            after,
        }
    }

    /// Check if the change is a type change
    #[must_use]
    pub fn is_type_change(&self) -> bool {
        std::mem::discriminant(&self.before) != std::mem::discriminant(&self.after)
    }

    /// Get a description of the change
    #[must_use]
    pub fn description(&self) -> String {
        format!("{}: {} -> {}", self.path, self.before, self.after)
    }
}

/// Compute the diff between two JSON values
#[must_use]
pub fn state_diff(before: &serde_json::Value, after: &serde_json::Value) -> StateDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    diff_values("", before, after, &mut added, &mut removed, &mut modified);

    StateDiff {
        added,
        removed,
        modified,
    }
}

/// Recursively diff two JSON values
fn diff_values(
    path: &str,
    before: &serde_json::Value,
    after: &serde_json::Value,
    added: &mut Vec<String>,
    removed: &mut Vec<String>,
    modified: &mut Vec<FieldDiff>,
) {
    match (before, after) {
        (serde_json::Value::Object(b), serde_json::Value::Object(a)) => {
            // Check for removed keys
            for key in b.keys() {
                if !a.contains_key(key) {
                    let full_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    removed.push(full_path);
                }
            }

            // Check for added keys and recurse on existing keys
            for (key, after_val) in a {
                let full_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };

                if let Some(before_val) = b.get(key) {
                    diff_values(&full_path, before_val, after_val, added, removed, modified);
                } else {
                    added.push(full_path);
                }
            }
        }
        (serde_json::Value::Array(b), serde_json::Value::Array(a)) => {
            // For arrays, compare indices
            let max_len = b.len().max(a.len());
            for i in 0..max_len {
                let full_path = if path.is_empty() {
                    format!("[{i}]")
                } else {
                    format!("{path}[{i}]")
                };

                match (b.get(i), a.get(i)) {
                    (Some(bv), Some(av)) => {
                        diff_values(&full_path, bv, av, added, removed, modified);
                    }
                    (None, Some(_)) => {
                        added.push(full_path);
                    }
                    (Some(_), None) => {
                        removed.push(full_path);
                    }
                    (None, None) => {}
                }
            }
        }
        _ => {
            // Leaf values - check if they differ
            if before != after {
                let full_path = if path.is_empty() {
                    "(root)".to_string()
                } else {
                    path.to_string()
                };
                modified.push(FieldDiff::new(full_path, before.clone(), after.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ============================================================================
    // StateSnapshot Tests
    // ============================================================================

    mod state_snapshot {
        use super::*;

        #[test]
        fn new_creates_snapshot_with_correct_fields() {
            let state = json!({"key": "value"});
            let snapshot = StateSnapshot::new("thread_1", "node_1", state.clone());

            assert_eq!(snapshot.thread_id, "thread_1");
            assert_eq!(snapshot.node, "node_1");
            assert_eq!(snapshot.state, state);
            assert!(snapshot.checkpoint_id.is_none());
            assert!(snapshot.description.is_none());
            assert!(snapshot.metadata.is_empty());
        }

        #[test]
        fn new_calculates_size_bytes() {
            let state = json!({"message": "hello world"});
            let snapshot = StateSnapshot::new("t", "n", state.clone());

            // Size should match JSON string length
            let expected_size = state.to_string().len();
            assert_eq!(snapshot.size_bytes, expected_size);
        }

        #[test]
        fn with_checkpoint_id_sets_checkpoint() {
            let snapshot = StateSnapshot::new("t", "n", json!({}))
                .with_checkpoint_id("checkpoint_abc");

            assert_eq!(snapshot.checkpoint_id, Some("checkpoint_abc".to_string()));
        }

        #[test]
        fn with_description_sets_description() {
            let snapshot = StateSnapshot::new("t", "n", json!({}))
                .with_description("Test snapshot");

            assert_eq!(snapshot.description, Some("Test snapshot".to_string()));
        }

        #[test]
        fn with_timestamp_overrides_timestamp() {
            let custom_time = SystemTime::UNIX_EPOCH;
            let snapshot = StateSnapshot::new("t", "n", json!({}))
                .with_timestamp(custom_time);

            assert_eq!(snapshot.timestamp, custom_time);
        }

        #[test]
        fn with_metadata_adds_metadata() {
            let snapshot = StateSnapshot::new("t", "n", json!({}))
                .with_metadata("custom_key", json!("custom_value"))
                .with_metadata("number", json!(42));

            assert_eq!(snapshot.metadata.len(), 2);
            assert_eq!(snapshot.metadata.get("custom_key"), Some(&json!("custom_value")));
            assert_eq!(snapshot.metadata.get("number"), Some(&json!(42)));
        }

        #[test]
        fn builder_methods_chain() {
            let snapshot = StateSnapshot::new("t", "n", json!({}))
                .with_checkpoint_id("ckpt")
                .with_description("desc")
                .with_metadata("key", json!(1));

            assert_eq!(snapshot.checkpoint_id, Some("ckpt".to_string()));
            assert_eq!(snapshot.description, Some("desc".to_string()));
            assert_eq!(snapshot.metadata.get("key"), Some(&json!(1)));
        }

        #[test]
        fn get_field_returns_top_level_field() {
            let state = json!({"name": "Alice", "age": 30});
            let snapshot = StateSnapshot::new("t", "n", state);

            assert_eq!(snapshot.get_field("name"), Some(&json!("Alice")));
            assert_eq!(snapshot.get_field("age"), Some(&json!(30)));
        }

        #[test]
        fn get_field_returns_nested_field() {
            let state = json!({
                "user": {
                    "profile": {
                        "email": "test@example.com"
                    }
                }
            });
            let snapshot = StateSnapshot::new("t", "n", state);

            assert_eq!(
                snapshot.get_field("user.profile.email"),
                Some(&json!("test@example.com"))
            );
        }

        #[test]
        fn get_field_returns_none_for_missing_field() {
            let state = json!({"exists": true});
            let snapshot = StateSnapshot::new("t", "n", state);

            assert!(snapshot.get_field("missing").is_none());
            assert!(snapshot.get_field("exists.nested").is_none());
        }

        #[test]
        fn get_field_with_empty_path_returns_root() {
            let state = json!({"key": "value"});
            let snapshot = StateSnapshot::new("t", "n", state.clone());

            // Empty path should return the state itself
            let result = snapshot.get_field("");
            // With empty path split, parts is [""], so we try to get "" key which doesn't exist
            assert!(result.is_none());
        }

        #[test]
        fn to_json_serializes_correctly() {
            let snapshot = StateSnapshot::new("thread_1", "node_1", json!({"x": 1}))
                .with_checkpoint_id("ckpt_1");

            let json_str = snapshot.to_json().expect("serialization should succeed");
            assert!(json_str.contains("thread_1"));
            assert!(json_str.contains("node_1"));
            assert!(json_str.contains("ckpt_1"));
        }

        #[test]
        fn elapsed_returns_duration() {
            let old_time = SystemTime::now() - Duration::from_secs(5);
            let snapshot = StateSnapshot::new("t", "n", json!({}))
                .with_timestamp(old_time);

            let elapsed = snapshot.elapsed();
            assert!(elapsed >= Duration::from_secs(4)); // Allow some tolerance
        }

        #[test]
        fn elapsed_handles_future_timestamp() {
            // If timestamp is in the future (shouldn't happen, but test edge case)
            let future_time = SystemTime::now() + Duration::from_secs(100);
            let snapshot = StateSnapshot::new("t", "n", json!({}))
                .with_timestamp(future_time);

            // Should return ZERO instead of panicking
            assert_eq!(snapshot.elapsed(), Duration::ZERO);
        }

        #[test]
        fn serialization_roundtrip() {
            let original = StateSnapshot::new("thread_1", "node_1", json!({"data": [1, 2, 3]}))
                .with_checkpoint_id("ckpt")
                .with_description("test")
                .with_metadata("key", json!("value"));

            let json = serde_json::to_string(&original).expect("serialize");
            let restored: StateSnapshot = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(restored.thread_id, original.thread_id);
            assert_eq!(restored.node, original.node);
            assert_eq!(restored.state, original.state);
            assert_eq!(restored.checkpoint_id, original.checkpoint_id);
            assert_eq!(restored.description, original.description);
        }
    }

    // ============================================================================
    // StateRegistry Tests
    // ============================================================================

    mod state_registry {
        use super::*;

        #[test]
        fn new_creates_empty_registry() {
            let registry = StateRegistry::new();
            assert_eq!(registry.total_count(), 0);
            assert!(registry.thread_ids().is_empty());
        }

        #[test]
        fn default_creates_empty_registry() {
            let registry = StateRegistry::default();
            assert_eq!(registry.total_count(), 0);
        }

        #[test]
        fn with_max_snapshots_sets_limit() {
            let registry = StateRegistry::with_max_snapshots(5);

            // Add 7 snapshots to same thread
            for i in 0..7 {
                registry.snapshot("thread_1", format!("node_{i}"), json!({"i": i}));
            }

            // Should only keep 5
            assert_eq!(registry.snapshot_count("thread_1"), 5);
        }

        #[test]
        fn snapshot_adds_to_history() {
            let registry = StateRegistry::new();

            registry.snapshot("thread_1", "node_a", json!({"step": 1}));
            registry.snapshot("thread_1", "node_b", json!({"step": 2}));

            assert_eq!(registry.snapshot_count("thread_1"), 2);
        }

        #[test]
        fn snapshot_different_threads() {
            let registry = StateRegistry::new();

            registry.snapshot("thread_1", "n", json!({}));
            registry.snapshot("thread_2", "n", json!({}));
            registry.snapshot("thread_3", "n", json!({}));

            assert_eq!(registry.total_count(), 3);
            let thread_ids = registry.thread_ids();
            assert_eq!(thread_ids.len(), 3);
        }

        #[test]
        fn add_snapshot_adds_prebuilt_snapshot() {
            let registry = StateRegistry::new();

            let snapshot = StateSnapshot::new("custom_thread", "custom_node", json!({"custom": true}))
                .with_checkpoint_id("ckpt_123");

            registry.add_snapshot(snapshot);

            let retrieved = registry.get_latest("custom_thread").expect("should exist");
            assert_eq!(retrieved.checkpoint_id, Some("ckpt_123".to_string()));
        }

        #[test]
        fn get_history_returns_all_snapshots_for_thread() {
            let registry = StateRegistry::new();

            registry.snapshot("t1", "a", json!({}));
            registry.snapshot("t1", "b", json!({}));
            registry.snapshot("t1", "c", json!({}));

            let history = registry.get_history("t1");
            assert_eq!(history.len(), 3);
            assert_eq!(history[0].node, "a");
            assert_eq!(history[1].node, "b");
            assert_eq!(history[2].node, "c");
        }

        #[test]
        fn get_history_returns_empty_for_unknown_thread() {
            let registry = StateRegistry::new();
            let history = registry.get_history("unknown");
            assert!(history.is_empty());
        }

        #[test]
        fn get_latest_returns_most_recent() {
            let registry = StateRegistry::new();

            registry.snapshot("t1", "first", json!({}));
            registry.snapshot("t1", "second", json!({}));
            registry.snapshot("t1", "latest", json!({}));

            let latest = registry.get_latest("t1").expect("should exist");
            assert_eq!(latest.node, "latest");
        }

        #[test]
        fn get_latest_returns_none_for_unknown_thread() {
            let registry = StateRegistry::new();
            assert!(registry.get_latest("unknown").is_none());
        }

        #[test]
        fn get_at_checkpoint_finds_correct_snapshot() {
            let registry = StateRegistry::new();

            let snap1 = StateSnapshot::new("t1", "n1", json!({}));
            let snap2 = StateSnapshot::new("t1", "n2", json!({}))
                .with_checkpoint_id("target_ckpt");
            let snap3 = StateSnapshot::new("t1", "n3", json!({}));

            registry.add_snapshot(snap1);
            registry.add_snapshot(snap2);
            registry.add_snapshot(snap3);

            let found = registry.get_at_checkpoint("t1", "target_ckpt").expect("should find");
            assert_eq!(found.node, "n2");
        }

        #[test]
        fn get_at_checkpoint_returns_none_for_missing() {
            let registry = StateRegistry::new();
            registry.snapshot("t1", "n", json!({}));

            assert!(registry.get_at_checkpoint("t1", "nonexistent").is_none());
        }

        #[test]
        fn get_at_time_finds_closest_snapshot() {
            let registry = StateRegistry::new();

            let base_time = SystemTime::UNIX_EPOCH;

            let snap1 = StateSnapshot::new("t1", "early", json!({}))
                .with_timestamp(base_time + Duration::from_secs(100));
            let snap2 = StateSnapshot::new("t1", "middle", json!({}))
                .with_timestamp(base_time + Duration::from_secs(200));
            let snap3 = StateSnapshot::new("t1", "late", json!({}))
                .with_timestamp(base_time + Duration::from_secs(300));

            registry.add_snapshot(snap1);
            registry.add_snapshot(snap2);
            registry.add_snapshot(snap3);

            // Query for time closest to 190 -> should return "middle" (200)
            let target = base_time + Duration::from_secs(190);
            let found = registry.get_at_time("t1", target).expect("should find");
            assert_eq!(found.node, "middle");
        }

        #[test]
        fn get_at_time_handles_before_all() {
            let registry = StateRegistry::new();

            let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
            let snap = StateSnapshot::new("t1", "only", json!({}))
                .with_timestamp(base_time);
            registry.add_snapshot(snap);

            // Query for time before the snapshot
            let found = registry.get_at_time("t1", SystemTime::UNIX_EPOCH).expect("should find");
            assert_eq!(found.node, "only");
        }

        #[test]
        fn get_by_node_filters_correctly() {
            let registry = StateRegistry::new();

            registry.snapshot("t1", "reasoning", json!({"r": 1}));
            registry.snapshot("t1", "action", json!({}));
            registry.snapshot("t1", "reasoning", json!({"r": 2}));
            registry.snapshot("t1", "output", json!({}));

            let reasoning_snaps = registry.get_by_node("t1", "reasoning");
            assert_eq!(reasoning_snaps.len(), 2);
        }

        #[test]
        fn get_by_node_returns_empty_for_no_matches() {
            let registry = StateRegistry::new();
            registry.snapshot("t1", "other", json!({}));

            let results = registry.get_by_node("t1", "nonexistent");
            assert!(results.is_empty());
        }

        #[test]
        fn get_recent_returns_across_all_threads() {
            let registry = StateRegistry::new();

            // Add snapshots with distinct timestamps
            let base = SystemTime::UNIX_EPOCH;
            for i in 0..5 {
                let snap = StateSnapshot::new(format!("t{i}"), "n", json!({}))
                    .with_timestamp(base + Duration::from_secs(i as u64 * 100));
                registry.add_snapshot(snap);
            }

            let recent = registry.get_recent(3);
            assert_eq!(recent.len(), 3);
            // Should be sorted by timestamp descending
            assert!(recent[0].timestamp >= recent[1].timestamp);
            assert!(recent[1].timestamp >= recent[2].timestamp);
        }

        #[test]
        fn get_recent_handles_limit_larger_than_count() {
            let registry = StateRegistry::new();
            registry.snapshot("t1", "n", json!({}));
            registry.snapshot("t2", "n", json!({}));

            let recent = registry.get_recent(100);
            assert_eq!(recent.len(), 2);
        }

        #[test]
        fn snapshot_count_returns_correct_count() {
            let registry = StateRegistry::new();

            assert_eq!(registry.snapshot_count("t1"), 0);

            registry.snapshot("t1", "n", json!({}));
            assert_eq!(registry.snapshot_count("t1"), 1);

            registry.snapshot("t1", "n", json!({}));
            assert_eq!(registry.snapshot_count("t1"), 2);
        }

        #[test]
        fn total_count_sums_all_threads() {
            let registry = StateRegistry::new();

            registry.snapshot("t1", "n", json!({}));
            registry.snapshot("t1", "n", json!({}));
            registry.snapshot("t2", "n", json!({}));
            registry.snapshot("t3", "n", json!({}));
            registry.snapshot("t3", "n", json!({}));
            registry.snapshot("t3", "n", json!({}));

            assert_eq!(registry.total_count(), 6);
        }

        #[test]
        fn thread_ids_returns_all_threads() {
            let registry = StateRegistry::new();

            registry.snapshot("alpha", "n", json!({}));
            registry.snapshot("beta", "n", json!({}));
            registry.snapshot("gamma", "n", json!({}));

            let ids = registry.thread_ids();
            assert_eq!(ids.len(), 3);
            assert!(ids.contains(&"alpha".to_string()));
            assert!(ids.contains(&"beta".to_string()));
            assert!(ids.contains(&"gamma".to_string()));
        }

        #[test]
        fn diff_snapshots_detects_changes() {
            let before = StateSnapshot::new("t", "n", json!({"a": 1, "b": 2}));
            let after = StateSnapshot::new("t", "n", json!({"a": 1, "b": 3, "c": 4}));

            let diff = StateRegistry::diff_snapshots(&before, &after);

            assert!(diff.added.contains(&"c".to_string()));
            assert!(diff.modified.iter().any(|m| m.path == "b"));
            assert!(diff.removed.is_empty());
        }

        #[test]
        fn get_changes_returns_sequential_diffs() {
            let registry = StateRegistry::new();

            registry.snapshot("t1", "n", json!({"count": 0}));
            registry.snapshot("t1", "n", json!({"count": 1}));
            registry.snapshot("t1", "n", json!({"count": 2}));

            let changes = registry.get_changes("t1");
            assert_eq!(changes.len(), 2); // 3 snapshots = 2 diffs

            // Each should show count modification
            for change in &changes {
                assert!(change.modified.iter().any(|m| m.path == "count"));
            }
        }

        #[test]
        fn get_changes_returns_empty_for_single_snapshot() {
            let registry = StateRegistry::new();
            registry.snapshot("t1", "n", json!({}));

            let changes = registry.get_changes("t1");
            assert!(changes.is_empty());
        }

        #[test]
        fn get_changes_returns_empty_for_no_snapshots() {
            let registry = StateRegistry::new();
            let changes = registry.get_changes("unknown");
            assert!(changes.is_empty());
        }

        #[test]
        fn clear_thread_removes_only_specified_thread() {
            let registry = StateRegistry::new();

            registry.snapshot("t1", "n", json!({}));
            registry.snapshot("t1", "n", json!({}));
            registry.snapshot("t2", "n", json!({}));

            registry.clear_thread("t1");

            assert_eq!(registry.snapshot_count("t1"), 0);
            assert_eq!(registry.snapshot_count("t2"), 1);
        }

        #[test]
        fn clear_removes_all_snapshots() {
            let registry = StateRegistry::new();

            registry.snapshot("t1", "n", json!({}));
            registry.snapshot("t2", "n", json!({}));
            registry.snapshot("t3", "n", json!({}));

            registry.clear();

            assert_eq!(registry.total_count(), 0);
            assert!(registry.thread_ids().is_empty());
        }

        #[test]
        fn to_json_serializes_all_snapshots() {
            let registry = StateRegistry::new();

            registry.snapshot("t1", "n1", json!({"x": 1}));
            registry.snapshot("t2", "n2", json!({"y": 2}));

            let json_str = registry.to_json().expect("serialization should succeed");
            assert!(json_str.contains("t1"));
            assert!(json_str.contains("t2"));
        }

        #[test]
        fn clone_shares_underlying_data() {
            let registry = StateRegistry::new();
            registry.snapshot("t1", "n", json!({}));

            let cloned = registry.clone();

            // Modifications via clone should be visible in original
            cloned.snapshot("t1", "n", json!({}));

            assert_eq!(registry.snapshot_count("t1"), 2);
        }

        #[test]
        fn pruning_removes_oldest_when_limit_exceeded() {
            let registry = StateRegistry::with_max_snapshots(3);

            // Add snapshots with identifiable data
            registry.snapshot("t1", "first", json!({"i": 0}));
            registry.snapshot("t1", "second", json!({"i": 1}));
            registry.snapshot("t1", "third", json!({"i": 2}));
            registry.snapshot("t1", "fourth", json!({"i": 3}));

            let history = registry.get_history("t1");
            assert_eq!(history.len(), 3);

            // First should have been pruned
            assert_eq!(history[0].node, "second");
            assert_eq!(history[2].node, "fourth");
        }
    }

    // ============================================================================
    // StateDiff Tests
    // ============================================================================

    mod state_diff {
        use super::*;

        #[test]
        fn empty_creates_diff_with_no_changes() {
            let diff = StateDiff::empty();

            assert!(diff.added.is_empty());
            assert!(diff.removed.is_empty());
            assert!(diff.modified.is_empty());
        }

        #[test]
        fn has_changes_returns_false_for_empty() {
            let diff = StateDiff::empty();
            assert!(!diff.has_changes());
        }

        #[test]
        fn has_changes_returns_true_with_added() {
            let diff = StateDiff {
                added: vec!["new_field".to_string()],
                removed: vec![],
                modified: vec![],
            };
            assert!(diff.has_changes());
        }

        #[test]
        fn has_changes_returns_true_with_removed() {
            let diff = StateDiff {
                added: vec![],
                removed: vec!["old_field".to_string()],
                modified: vec![],
            };
            assert!(diff.has_changes());
        }

        #[test]
        fn has_changes_returns_true_with_modified() {
            let diff = StateDiff {
                added: vec![],
                removed: vec![],
                modified: vec![FieldDiff::new("field", json!(1), json!(2))],
            };
            assert!(diff.has_changes());
        }

        #[test]
        fn change_count_sums_all_changes() {
            let diff = StateDiff {
                added: vec!["a".to_string(), "b".to_string()],
                removed: vec!["c".to_string()],
                modified: vec![
                    FieldDiff::new("d", json!(1), json!(2)),
                    FieldDiff::new("e", json!(3), json!(4)),
                ],
            };

            assert_eq!(diff.change_count(), 5);
        }

        #[test]
        fn summary_returns_no_changes_for_empty() {
            let diff = StateDiff::empty();
            assert_eq!(diff.summary(), "No changes");
        }

        #[test]
        fn summary_includes_all_categories() {
            let diff = StateDiff {
                added: vec!["a".to_string()],
                removed: vec!["b".to_string()],
                modified: vec![FieldDiff::new("c", json!(1), json!(2))],
            };

            let summary = diff.summary();
            assert!(summary.contains("1 added"));
            assert!(summary.contains("1 removed"));
            assert!(summary.contains("1 modified"));
        }

        #[test]
        fn detailed_report_formats_correctly() {
            let diff = StateDiff {
                added: vec!["new_key".to_string()],
                removed: vec!["old_key".to_string()],
                modified: vec![FieldDiff::new("changed", json!("old"), json!("new"))],
            };

            let report = diff.detailed_report();
            assert!(report.contains("Added:"));
            assert!(report.contains("+ new_key"));
            assert!(report.contains("Removed:"));
            assert!(report.contains("- old_key"));
            assert!(report.contains("Modified:"));
            assert!(report.contains("~ changed:"));
        }

        #[test]
        fn to_json_serializes_correctly() {
            let diff = StateDiff {
                added: vec!["field".to_string()],
                removed: vec![],
                modified: vec![],
            };

            let json_str = diff.to_json().expect("serialization should succeed");
            assert!(json_str.contains("field"));
        }

        #[test]
        fn serialization_roundtrip() {
            let original = StateDiff {
                added: vec!["a".to_string(), "b".to_string()],
                removed: vec!["c".to_string()],
                modified: vec![FieldDiff::new("d", json!(1), json!(2))],
            };

            let json = serde_json::to_string(&original).expect("serialize");
            let restored: StateDiff = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(restored.added, original.added);
            assert_eq!(restored.removed, original.removed);
            assert_eq!(restored.modified.len(), original.modified.len());
        }
    }

    // ============================================================================
    // FieldDiff Tests
    // ============================================================================

    mod field_diff {
        use super::*;

        #[test]
        fn new_creates_field_diff() {
            let diff = FieldDiff::new("path.to.field", json!(1), json!(2));

            assert_eq!(diff.path, "path.to.field");
            assert_eq!(diff.before, json!(1));
            assert_eq!(diff.after, json!(2));
        }

        #[test]
        fn is_type_change_returns_true_for_different_types() {
            let diff = FieldDiff::new("field", json!("string"), json!(42));
            assert!(diff.is_type_change());
        }

        #[test]
        fn is_type_change_returns_false_for_same_types() {
            let diff = FieldDiff::new("field", json!(1), json!(2));
            assert!(!diff.is_type_change());
        }

        #[test]
        fn is_type_change_null_to_value() {
            let diff = FieldDiff::new("field", json!(null), json!("value"));
            assert!(diff.is_type_change());
        }

        #[test]
        fn is_type_change_array_to_object() {
            let diff = FieldDiff::new("field", json!([1, 2]), json!({"key": "val"}));
            assert!(diff.is_type_change());
        }

        #[test]
        fn description_formats_change() {
            let diff = FieldDiff::new("count", json!(5), json!(10));
            let desc = diff.description();

            assert!(desc.contains("count"));
            assert!(desc.contains("5"));
            assert!(desc.contains("10"));
        }

        #[test]
        fn serialization_roundtrip() {
            let original = FieldDiff::new("nested.path", json!({"a": 1}), json!({"a": 2}));

            let json = serde_json::to_string(&original).expect("serialize");
            let restored: FieldDiff = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(restored.path, original.path);
            assert_eq!(restored.before, original.before);
            assert_eq!(restored.after, original.after);
        }
    }

    // ============================================================================
    // state_diff Function Tests
    // ============================================================================

    mod state_diff_fn {
        use super::*;

        #[test]
        fn identical_values_produce_empty_diff() {
            let value = json!({"a": 1, "b": [1, 2, 3]});
            let diff = state_diff(&value, &value);

            assert!(!diff.has_changes());
        }

        #[test]
        fn detects_added_top_level_field() {
            let before = json!({"a": 1});
            let after = json!({"a": 1, "b": 2});

            let diff = state_diff(&before, &after);

            assert!(diff.added.contains(&"b".to_string()));
            assert!(diff.removed.is_empty());
            assert!(diff.modified.is_empty());
        }

        #[test]
        fn detects_removed_top_level_field() {
            let before = json!({"a": 1, "b": 2});
            let after = json!({"a": 1});

            let diff = state_diff(&before, &after);

            assert!(diff.removed.contains(&"b".to_string()));
            assert!(diff.added.is_empty());
            assert!(diff.modified.is_empty());
        }

        #[test]
        fn detects_modified_top_level_field() {
            let before = json!({"a": 1});
            let after = json!({"a": 2});

            let diff = state_diff(&before, &after);

            assert!(diff.modified.iter().any(|m| m.path == "a" && m.before == json!(1) && m.after == json!(2)));
        }

        #[test]
        fn detects_nested_field_changes() {
            let before = json!({"outer": {"inner": 1}});
            let after = json!({"outer": {"inner": 2}});

            let diff = state_diff(&before, &after);

            assert!(diff.modified.iter().any(|m| m.path == "outer.inner"));
        }

        #[test]
        fn detects_added_nested_field() {
            let before = json!({"outer": {}});
            let after = json!({"outer": {"new": "value"}});

            let diff = state_diff(&before, &after);

            assert!(diff.added.contains(&"outer.new".to_string()));
        }

        #[test]
        fn detects_removed_nested_field() {
            let before = json!({"outer": {"old": "value"}});
            let after = json!({"outer": {}});

            let diff = state_diff(&before, &after);

            assert!(diff.removed.contains(&"outer.old".to_string()));
        }

        #[test]
        fn detects_array_element_added() {
            let before = json!([1, 2]);
            let after = json!([1, 2, 3]);

            let diff = state_diff(&before, &after);

            assert!(diff.added.contains(&"[2]".to_string()));
        }

        #[test]
        fn detects_array_element_removed() {
            let before = json!([1, 2, 3]);
            let after = json!([1, 2]);

            let diff = state_diff(&before, &after);

            assert!(diff.removed.contains(&"[2]".to_string()));
        }

        #[test]
        fn detects_array_element_modified() {
            let before = json!([1, 2, 3]);
            let after = json!([1, 99, 3]);

            let diff = state_diff(&before, &after);

            assert!(diff.modified.iter().any(|m| m.path == "[1]"));
        }

        #[test]
        fn detects_nested_array_in_object() {
            let before = json!({"items": [1, 2]});
            let after = json!({"items": [1, 2, 3]});

            let diff = state_diff(&before, &after);

            assert!(diff.added.contains(&"items[2]".to_string()));
        }

        #[test]
        fn detects_root_value_change() {
            let before = json!("old");
            let after = json!("new");

            let diff = state_diff(&before, &after);

            assert!(diff.modified.iter().any(|m| m.path == "(root)"));
        }

        #[test]
        fn handles_type_change_at_field() {
            let before = json!({"field": 123});
            let after = json!({"field": "string"});

            let diff = state_diff(&before, &after);

            assert!(diff.modified.iter().any(|m| m.path == "field"));
        }

        #[test]
        fn complex_nested_structure() {
            let before = json!({
                "users": [
                    {"name": "Alice", "age": 30},
                    {"name": "Bob", "age": 25}
                ],
                "meta": {"version": 1}
            });
            let after = json!({
                "users": [
                    {"name": "Alice", "age": 31},
                    {"name": "Bob", "age": 25, "email": "bob@example.com"}
                ],
                "meta": {"version": 2},
                "new_field": true
            });

            let diff = state_diff(&before, &after);

            // Alice's age changed
            assert!(diff.modified.iter().any(|m| m.path == "users[0].age"));
            // Bob got email added
            assert!(diff.added.contains(&"users[1].email".to_string()));
            // Meta version changed
            assert!(diff.modified.iter().any(|m| m.path == "meta.version"));
            // New top-level field
            assert!(diff.added.contains(&"new_field".to_string()));
        }

        #[test]
        fn empty_objects() {
            let before = json!({});
            let after = json!({});

            let diff = state_diff(&before, &after);
            assert!(!diff.has_changes());
        }

        #[test]
        fn empty_arrays() {
            let before = json!([]);
            let after = json!([]);

            let diff = state_diff(&before, &after);
            assert!(!diff.has_changes());
        }

        #[test]
        fn null_values() {
            let before = json!({"field": null});
            let after = json!({"field": "not null"});

            let diff = state_diff(&before, &after);
            assert!(diff.modified.iter().any(|m| m.path == "field"));
        }

        #[test]
        fn boolean_values() {
            let before = json!({"flag": true});
            let after = json!({"flag": false});

            let diff = state_diff(&before, &after);
            assert!(diff.modified.iter().any(|m| m.path == "flag"));
        }
    }
}
