// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Execution metrics for graph performance tracking
//!
//! This module provides detailed metrics about graph execution including
//! node durations, total execution time, checkpoint operations, and state sizes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

// ============================================================================
// Local Metrics Batch - Reduces Mutex Lock Acquisitions
// ============================================================================

/// Local metrics batch for collecting updates without mutex contention.
///
/// Use this to accumulate metrics updates locally during hot path execution,
/// then apply them all at once with a single mutex lock. This reduces lock
/// acquisitions from O(n) to O(1) per execution loop iteration.
///
/// # Example
///
/// ```rust,ignore
/// // Instead of:
/// // metrics.lock().record_node_execution("node1", dur);
/// // metrics.lock().record_edge_traversal();
/// // metrics.lock().record_conditional_branch();
///
/// // Use batch:
/// let mut batch = LocalMetricsBatch::new();
/// batch.record_node_execution("node1", dur);
/// batch.record_edge_traversal();
/// batch.record_conditional_branch();
/// batch.apply_to(&mut metrics.lock().unwrap());
/// ```
#[derive(Debug, Default)]
pub struct LocalMetricsBatch {
    /// Node executions: (node_name, duration, started_at)
    node_executions: Vec<(String, Duration, Option<SystemTime>)>,
    /// Number of edges traversed
    edges_traversed: usize,
    /// Number of conditional branches
    conditional_branches: usize,
    /// Parallel execution records: (concurrency)
    parallel_executions: Vec<usize>,
    /// Total duration (if set)
    total_duration: Option<Duration>,
    /// Checkpoint saves count
    checkpoint_saves: usize,
    /// Token counts per node: (node_name, tokens)
    node_tokens: Vec<(String, u64)>,
}

impl LocalMetricsBatch {
    /// Create a new empty batch
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a node execution with optional start timestamp
    #[inline]
    pub fn record_node_execution(&mut self, node_name: impl Into<String>, duration: Duration) {
        self.node_executions
            .push((node_name.into(), duration, None));
    }

    /// Record a node execution with start timestamp for tracing
    #[inline]
    pub fn record_node_execution_with_timestamp(
        &mut self,
        node_name: impl Into<String>,
        duration: Duration,
        started_at: SystemTime,
    ) {
        self.node_executions
            .push((node_name.into(), duration, Some(started_at)));
    }

    /// Record edge traversal
    #[inline]
    pub fn record_edge_traversal(&mut self) {
        self.edges_traversed += 1;
    }

    /// Record conditional branch
    #[inline]
    pub fn record_conditional_branch(&mut self) {
        self.conditional_branches += 1;
    }

    /// Record parallel execution with concurrency level
    #[inline]
    pub fn record_parallel_execution(&mut self, concurrency: usize) {
        self.parallel_executions.push(concurrency);
    }

    /// Set total duration
    #[inline]
    pub fn set_total_duration(&mut self, duration: Duration) {
        self.total_duration = Some(duration);
    }

    /// Record checkpoint save
    #[inline]
    pub fn record_checkpoint_save(&mut self) {
        self.checkpoint_saves += 1;
    }

    /// Record token usage for a node
    ///
    /// This is used to track LLM token usage during node execution.
    /// The tokens will be aggregated when the batch is applied.
    #[inline]
    pub fn record_node_tokens(&mut self, node_name: impl Into<String>, tokens: u64) {
        self.node_tokens.push((node_name.into(), tokens));
    }

    /// Check if batch has any updates to apply
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.node_executions.is_empty()
            && self.edges_traversed == 0
            && self.conditional_branches == 0
            && self.parallel_executions.is_empty()
            && self.total_duration.is_none()
            && self.checkpoint_saves == 0
            && self.node_tokens.is_empty()
    }

    /// Apply all batched updates to ExecutionMetrics (single lock acquisition)
    pub fn apply_to(self, metrics: &mut ExecutionMetrics) {
        // Apply node executions with optional timestamps
        for (node_name, duration, started_at) in self.node_executions {
            if let Some(ts) = started_at {
                metrics.record_node_execution_with_timestamp(&node_name, duration, ts);
            } else {
                metrics.record_node_execution(&node_name, duration);
            }
        }

        // Apply edge traversals
        metrics.edges_traversed += self.edges_traversed;

        // Apply conditional branches
        metrics.conditional_branches += self.conditional_branches;

        // Apply parallel executions
        for concurrency in self.parallel_executions {
            metrics.record_parallel_execution(concurrency);
        }

        // Apply checkpoint saves
        metrics.checkpoint_count += self.checkpoint_saves;

        // Apply total duration if set
        if let Some(duration) = self.total_duration {
            metrics.set_total_duration(duration);
        }

        // Apply node token counts
        for (node_name, tokens) in self.node_tokens {
            metrics.record_node_tokens(&node_name, tokens);
        }
    }
}

/// Execution metrics captured during graph execution
///
/// Metrics are automatically collected when a graph executes and can be
/// accessed after completion to understand performance characteristics.
///
/// # Example
///
/// ```rust,ignore
/// let app = graph.compile()?;
/// let result = app.invoke(state).await?;
/// let metrics = app.metrics();
///
/// println!("Total time: {:?}", metrics.total_duration);
/// for (node, duration) in metrics.node_durations {
///     println!("  {}: {:?}", node, duration);
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Duration per node (`node_name` -> duration)
    pub node_durations: HashMap<String, Duration>,

    /// Number of times each node was executed
    pub node_execution_counts: HashMap<String, usize>,

    /// Start timestamps per node (`node_name` -> RFC3339 timestamp string)
    /// For nodes executed multiple times, stores the most recent start time.
    #[serde(default)]
    pub node_timestamps: HashMap<String, String>,

    /// Total execution duration (wall clock time)
    pub total_duration: Duration,

    /// Number of checkpoints saved
    pub checkpoint_count: usize,

    /// Number of checkpoints loaded
    pub checkpoint_loads: usize,

    /// State size in bytes (if serializable)
    pub state_size_bytes: Option<usize>,

    /// Number of edges traversed
    pub edges_traversed: usize,

    /// Number of conditional branches evaluated
    pub conditional_branches: usize,

    /// Number of parallel executions
    pub parallel_executions: usize,

    /// Peak number of concurrent nodes
    pub peak_concurrency: usize,

    /// Total number of events emitted
    pub events_emitted: usize,

    /// Token counts per node (`node_name` -> total tokens used)
    /// Populated when LLM providers report token usage via callbacks.
    #[serde(default)]
    pub node_tokens: HashMap<String, u64>,

    /// Total tokens used across all nodes
    #[serde(default)]
    pub total_tokens: u64,
}

impl ExecutionMetrics {
    /// Create new empty metrics
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record node execution
    pub(crate) fn record_node_execution(&mut self, node_name: &str, duration: Duration) {
        // Update duration (accumulate if node executed multiple times)
        *self
            .node_durations
            .entry(node_name.to_string())
            .or_insert(Duration::ZERO) += duration;

        // Increment execution count
        *self
            .node_execution_counts
            .entry(node_name.to_string())
            .or_insert(0) += 1;
    }

    /// Record node execution with start timestamp
    pub(crate) fn record_node_execution_with_timestamp(
        &mut self,
        node_name: &str,
        duration: Duration,
        started_at: SystemTime,
    ) {
        self.record_node_execution(node_name, duration);

        // Convert SystemTime to RFC3339 string
        use chrono::{DateTime, Utc};
        let timestamp = DateTime::<Utc>::from(started_at).to_rfc3339();
        self.node_timestamps
            .insert(node_name.to_string(), timestamp);
    }

    /// Record checkpoint save
    pub(crate) fn record_checkpoint_save(&mut self) {
        self.checkpoint_count += 1;
    }

    /// Record checkpoint load
    #[cfg(test)]
    pub(crate) fn record_checkpoint_load(&mut self) {
        self.checkpoint_loads += 1;
    }

    /// Record edge traversal
    /// Note: In production, use LocalMetricsBatch::record_edge_traversal for batching.
    #[cfg(test)]
    pub(crate) fn record_edge_traversal(&mut self) {
        self.edges_traversed += 1;
    }

    /// Record conditional branch
    /// Note: In production, use LocalMetricsBatch::record_conditional_branch for batching.
    #[cfg(test)]
    pub(crate) fn record_conditional_branch(&mut self) {
        self.conditional_branches += 1;
    }

    /// Record parallel execution
    pub(crate) fn record_parallel_execution(&mut self, concurrency: usize) {
        self.parallel_executions += 1;
        if concurrency > self.peak_concurrency {
            self.peak_concurrency = concurrency;
        }
    }

    /// Record event emitted
    #[cfg(test)]
    pub(crate) fn record_event(&mut self) {
        self.events_emitted += 1;
    }

    /// Set state size
    #[cfg(test)]
    pub(crate) fn set_state_size(&mut self, size: usize) {
        self.state_size_bytes = Some(size);
    }

    /// Set total duration
    pub(crate) fn set_total_duration(&mut self, duration: Duration) {
        self.total_duration = duration;
    }

    /// Record token usage for a node
    ///
    /// This accumulates token counts if the same node is called multiple times
    /// (e.g., in a loop). Total tokens across all nodes is also updated.
    pub fn record_node_tokens(&mut self, node_name: &str, tokens: u64) {
        *self.node_tokens.entry(node_name.to_string()).or_insert(0) += tokens;
        self.total_tokens += tokens;
    }

    /// Get average node execution time
    #[must_use]
    pub fn average_node_duration(&self) -> Duration {
        if self.node_durations.is_empty() {
            return Duration::ZERO;
        }

        let total: Duration = self.node_durations.values().sum();
        // Use checked conversion to handle potential u32 overflow for very large node counts
        let divisor = u32::try_from(self.node_durations.len()).unwrap_or(u32::MAX);
        total / divisor
    }

    /// Get slowest node
    #[must_use]
    pub fn slowest_node(&self) -> Option<(&str, Duration)> {
        self.node_durations
            .iter()
            .max_by_key(|(_, duration)| *duration)
            .map(|(name, duration)| (name.as_str(), *duration))
    }

    /// Get node execution percentage
    #[must_use]
    pub fn node_percentage(&self, node_name: &str) -> f64 {
        if self.total_duration.is_zero() {
            return 0.0;
        }

        self.node_durations.get(node_name).map_or(0.0, |d| {
            d.as_secs_f64() / self.total_duration.as_secs_f64() * 100.0
        })
    }

    /// Format metrics as a human-readable string
    #[must_use]
    pub fn to_string_pretty(&self) -> String {
        let mut output = String::from("Execution Metrics:\n");
        output.push_str(&format!("  Total Duration: {:?}\n", self.total_duration));
        output.push_str(&format!("  Edges Traversed: {}\n", self.edges_traversed));
        output.push_str(&format!(
            "  Checkpoints: {} saved, {} loaded\n",
            self.checkpoint_count, self.checkpoint_loads
        ));

        if let Some(size) = self.state_size_bytes {
            output.push_str(&format!("  State Size: {size} bytes\n"));
        }

        output.push_str("\n  Node Durations:\n");
        let mut sorted_nodes: Vec<_> = self.node_durations.iter().collect();
        sorted_nodes.sort_by_key(|(_, duration)| std::cmp::Reverse(*duration));

        for (node, duration) in sorted_nodes {
            let count = self.node_execution_counts.get(node).unwrap_or(&0);
            let pct = self.node_percentage(node);
            output.push_str(&format!(
                "    {node:<20} {duration:>10?} ({pct:>6.2}%) [{count} calls]\n"
            ));
        }

        if self.conditional_branches > 0 {
            output.push_str(&format!(
                "\n  Conditional Branches: {}\n",
                self.conditional_branches
            ));
        }

        if self.parallel_executions > 0 {
            output.push_str(&format!(
                "  Parallel Executions: {} (peak concurrency: {})\n",
                self.parallel_executions, self.peak_concurrency
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_basic() {
        let mut metrics = ExecutionMetrics::new();

        metrics.record_node_execution("node1", Duration::from_millis(100));
        metrics.record_node_execution("node2", Duration::from_millis(50));
        metrics.set_total_duration(Duration::from_millis(150));

        assert_eq!(metrics.node_durations.len(), 2);
        assert_eq!(metrics.average_node_duration(), Duration::from_millis(75));
        assert_eq!(metrics.slowest_node().unwrap().0, "node1");
    }

    #[test]
    fn test_metrics_node_percentage() {
        let mut metrics = ExecutionMetrics::new();

        metrics.record_node_execution("node1", Duration::from_millis(75));
        metrics.record_node_execution("node2", Duration::from_millis(25));
        metrics.set_total_duration(Duration::from_millis(100));

        assert!((metrics.node_percentage("node1") - 75.0).abs() < 0.1);
        assert!((metrics.node_percentage("node2") - 25.0).abs() < 0.1);
    }

    #[test]
    fn test_metrics_multiple_executions() {
        let mut metrics = ExecutionMetrics::new();

        // Node executed multiple times (e.g., in a loop)
        metrics.record_node_execution("loop_node", Duration::from_millis(10));
        metrics.record_node_execution("loop_node", Duration::from_millis(10));
        metrics.record_node_execution("loop_node", Duration::from_millis(10));

        assert_eq!(metrics.node_execution_counts.get("loop_node"), Some(&3));
        assert_eq!(
            metrics.node_durations.get("loop_node"),
            Some(&Duration::from_millis(30))
        );
    }

    #[test]
    fn test_record_checkpoint_save() {
        let mut metrics = ExecutionMetrics::new();
        assert_eq!(metrics.checkpoint_count, 0);

        metrics.record_checkpoint_save();
        assert_eq!(metrics.checkpoint_count, 1);

        metrics.record_checkpoint_save();
        assert_eq!(metrics.checkpoint_count, 2);
    }

    #[test]
    fn test_record_checkpoint_load() {
        let mut metrics = ExecutionMetrics::new();
        assert_eq!(metrics.checkpoint_loads, 0);

        metrics.record_checkpoint_load();
        assert_eq!(metrics.checkpoint_loads, 1);

        metrics.record_checkpoint_load();
        assert_eq!(metrics.checkpoint_loads, 2);
    }

    #[test]
    fn test_record_event() {
        let mut metrics = ExecutionMetrics::new();
        assert_eq!(metrics.events_emitted, 0);

        metrics.record_event();
        assert_eq!(metrics.events_emitted, 1);

        metrics.record_event();
        assert_eq!(metrics.events_emitted, 2);
    }

    #[test]
    fn test_set_state_size() {
        let mut metrics = ExecutionMetrics::new();
        assert_eq!(metrics.state_size_bytes, None);

        metrics.set_state_size(1024);
        assert_eq!(metrics.state_size_bytes, Some(1024));

        metrics.set_state_size(2048);
        assert_eq!(metrics.state_size_bytes, Some(2048));
    }

    #[test]
    fn test_record_edge_traversal() {
        let mut metrics = ExecutionMetrics::new();
        assert_eq!(metrics.edges_traversed, 0);

        metrics.record_edge_traversal();
        assert_eq!(metrics.edges_traversed, 1);

        metrics.record_edge_traversal();
        assert_eq!(metrics.edges_traversed, 2);
    }

    #[test]
    fn test_record_conditional_branch() {
        let mut metrics = ExecutionMetrics::new();
        assert_eq!(metrics.conditional_branches, 0);

        metrics.record_conditional_branch();
        assert_eq!(metrics.conditional_branches, 1);

        metrics.record_conditional_branch();
        assert_eq!(metrics.conditional_branches, 2);
    }

    #[test]
    fn test_record_parallel_execution() {
        let mut metrics = ExecutionMetrics::new();
        assert_eq!(metrics.parallel_executions, 0);
        assert_eq!(metrics.peak_concurrency, 0);

        metrics.record_parallel_execution(3);
        assert_eq!(metrics.parallel_executions, 1);
        assert_eq!(metrics.peak_concurrency, 3);

        metrics.record_parallel_execution(5);
        assert_eq!(metrics.parallel_executions, 2);
        assert_eq!(metrics.peak_concurrency, 5);

        // Lower concurrency doesn't reduce peak
        metrics.record_parallel_execution(2);
        assert_eq!(metrics.parallel_executions, 3);
        assert_eq!(metrics.peak_concurrency, 5);
    }

    #[test]
    fn test_average_node_duration_empty() {
        let metrics = ExecutionMetrics::new();
        assert_eq!(metrics.average_node_duration(), Duration::ZERO);
    }

    #[test]
    fn test_node_percentage_zero_total() {
        let mut metrics = ExecutionMetrics::new();
        metrics.record_node_execution("node1", Duration::from_millis(100));
        // total_duration is zero
        assert_eq!(metrics.node_percentage("node1"), 0.0);
    }

    #[test]
    fn test_node_percentage_nonexistent_node() {
        let mut metrics = ExecutionMetrics::new();
        metrics.record_node_execution("node1", Duration::from_millis(100));
        metrics.set_total_duration(Duration::from_millis(100));

        // Node that doesn't exist returns 0.0
        assert_eq!(metrics.node_percentage("nonexistent"), 0.0);
    }

    #[test]
    fn test_to_string_pretty_basic() {
        let mut metrics = ExecutionMetrics::new();
        metrics.record_node_execution("node1", Duration::from_millis(100));
        metrics.record_node_execution("node2", Duration::from_millis(50));
        metrics.set_total_duration(Duration::from_millis(150));
        metrics.record_checkpoint_save();
        metrics.record_checkpoint_load();
        metrics.record_edge_traversal();

        let output = metrics.to_string_pretty();
        assert!(output.contains("Execution Metrics"));
        assert!(output.contains("Total Duration"));
        assert!(output.contains("150ms"));
        assert!(output.contains("Edges Traversed: 1"));
        assert!(output.contains("Checkpoints: 1 saved, 1 loaded"));
        assert!(output.contains("node1"));
        assert!(output.contains("node2"));
    }

    #[test]
    fn test_to_string_pretty_with_state_size() {
        let mut metrics = ExecutionMetrics::new();
        metrics.record_node_execution("node1", Duration::from_millis(100));
        metrics.set_total_duration(Duration::from_millis(100));
        metrics.set_state_size(2048);

        let output = metrics.to_string_pretty();
        assert!(output.contains("State Size: 2048 bytes"));
    }

    #[test]
    fn test_to_string_pretty_with_conditional_branches() {
        let mut metrics = ExecutionMetrics::new();
        metrics.record_node_execution("node1", Duration::from_millis(100));
        metrics.set_total_duration(Duration::from_millis(100));
        metrics.record_conditional_branch();
        metrics.record_conditional_branch();

        let output = metrics.to_string_pretty();
        assert!(output.contains("Conditional Branches: 2"));
    }

    #[test]
    fn test_to_string_pretty_with_parallel_executions() {
        let mut metrics = ExecutionMetrics::new();
        metrics.record_node_execution("node1", Duration::from_millis(100));
        metrics.set_total_duration(Duration::from_millis(100));
        metrics.record_parallel_execution(3);
        metrics.record_parallel_execution(5);

        let output = metrics.to_string_pretty();
        assert!(output.contains("Parallel Executions: 2"));
        assert!(output.contains("peak concurrency: 5"));
    }

    #[test]
    fn test_metrics_default_trait() {
        let metrics = ExecutionMetrics::default();
        assert_eq!(metrics.node_durations.len(), 0);
        assert_eq!(metrics.checkpoint_count, 0);
        assert_eq!(metrics.total_duration, Duration::ZERO);
    }

    #[test]
    fn test_metrics_clone() {
        let mut metrics1 = ExecutionMetrics::new();
        metrics1.record_node_execution("node1", Duration::from_millis(100));
        metrics1.set_total_duration(Duration::from_millis(100));

        let metrics2 = metrics1.clone();
        assert_eq!(metrics1.total_duration, metrics2.total_duration);
        assert_eq!(metrics1.node_durations.len(), metrics2.node_durations.len());
    }

    #[test]
    fn test_slowest_node_multiple_nodes() {
        let mut metrics = ExecutionMetrics::new();
        metrics.record_node_execution("fast", Duration::from_millis(10));
        metrics.record_node_execution("medium", Duration::from_millis(50));
        metrics.record_node_execution("slow", Duration::from_millis(100));

        let (slowest, duration) = metrics.slowest_node().unwrap();
        assert_eq!(slowest, "slow");
        assert_eq!(duration, Duration::from_millis(100));
    }

    #[test]
    fn test_slowest_node_empty() {
        let metrics = ExecutionMetrics::new();
        assert!(metrics.slowest_node().is_none());
    }

    // ============================================================================
    // LocalMetricsBatch Tests
    // ============================================================================

    #[test]
    fn test_local_metrics_batch_new() {
        let batch = LocalMetricsBatch::new();
        assert!(batch.is_empty());
    }

    #[test]
    fn test_local_metrics_batch_record_node_execution() {
        let mut batch = LocalMetricsBatch::new();
        batch.record_node_execution("node1", Duration::from_millis(100));
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_local_metrics_batch_apply_to() {
        let mut batch = LocalMetricsBatch::new();
        batch.record_node_execution("node1", Duration::from_millis(100));
        batch.record_node_execution("node2", Duration::from_millis(50));
        batch.record_edge_traversal();
        batch.record_edge_traversal();
        batch.record_conditional_branch();
        batch.record_parallel_execution(3);
        batch.record_checkpoint_save();
        batch.set_total_duration(Duration::from_millis(200));

        let mut metrics = ExecutionMetrics::new();
        batch.apply_to(&mut metrics);

        assert_eq!(metrics.node_durations.len(), 2);
        assert_eq!(
            metrics.node_durations.get("node1"),
            Some(&Duration::from_millis(100))
        );
        assert_eq!(
            metrics.node_durations.get("node2"),
            Some(&Duration::from_millis(50))
        );
        assert_eq!(metrics.edges_traversed, 2);
        assert_eq!(metrics.conditional_branches, 1);
        assert_eq!(metrics.parallel_executions, 1);
        assert_eq!(metrics.peak_concurrency, 3);
        assert_eq!(metrics.checkpoint_count, 1);
        assert_eq!(metrics.total_duration, Duration::from_millis(200));
    }

    #[test]
    fn test_local_metrics_batch_multiple_apply() {
        // Simulate multiple batches being applied (like multiple loop iterations)
        let mut metrics = ExecutionMetrics::new();

        // First iteration batch
        let mut batch1 = LocalMetricsBatch::new();
        batch1.record_node_execution("node1", Duration::from_millis(100));
        batch1.record_edge_traversal();
        batch1.apply_to(&mut metrics);

        // Second iteration batch
        let mut batch2 = LocalMetricsBatch::new();
        batch2.record_node_execution("node2", Duration::from_millis(50));
        batch2.record_node_execution("node1", Duration::from_millis(30)); // node1 again
        batch2.record_edge_traversal();
        batch2.record_conditional_branch();
        batch2.apply_to(&mut metrics);

        assert_eq!(metrics.node_durations.len(), 2);
        // node1 should have accumulated duration
        assert_eq!(
            metrics.node_durations.get("node1"),
            Some(&Duration::from_millis(130))
        );
        assert_eq!(metrics.node_execution_counts.get("node1"), Some(&2));
        assert_eq!(metrics.edges_traversed, 2);
        assert_eq!(metrics.conditional_branches, 1);
    }

    #[test]
    fn test_local_metrics_batch_empty_apply() {
        let batch = LocalMetricsBatch::new();
        let mut metrics = ExecutionMetrics::new();

        // Pre-populate some data
        metrics.record_node_execution("existing", Duration::from_millis(50));
        metrics.edges_traversed = 5;

        // Apply empty batch - should not change anything
        batch.apply_to(&mut metrics);

        assert_eq!(metrics.node_durations.len(), 1);
        assert_eq!(metrics.edges_traversed, 5);
    }

    #[test]
    fn test_local_metrics_batch_parallel_executions() {
        let mut batch = LocalMetricsBatch::new();
        batch.record_parallel_execution(3);
        batch.record_parallel_execution(5);
        batch.record_parallel_execution(2);

        let mut metrics = ExecutionMetrics::new();
        batch.apply_to(&mut metrics);

        assert_eq!(metrics.parallel_executions, 3);
        assert_eq!(metrics.peak_concurrency, 5);
    }

    // ============================================================================
    // Token Tracking Tests (IMP-001)
    // ============================================================================

    #[test]
    fn test_record_node_tokens() {
        let mut metrics = ExecutionMetrics::new();
        assert_eq!(metrics.total_tokens, 0);
        assert!(metrics.node_tokens.is_empty());

        metrics.record_node_tokens("llm_node", 100);
        assert_eq!(metrics.total_tokens, 100);
        assert_eq!(metrics.node_tokens.get("llm_node"), Some(&100));

        // Accumulates on multiple calls
        metrics.record_node_tokens("llm_node", 50);
        assert_eq!(metrics.total_tokens, 150);
        assert_eq!(metrics.node_tokens.get("llm_node"), Some(&150));
    }

    #[test]
    fn test_record_tokens_multiple_nodes() {
        let mut metrics = ExecutionMetrics::new();

        metrics.record_node_tokens("classify", 50);
        metrics.record_node_tokens("generate", 200);
        metrics.record_node_tokens("summarize", 100);

        assert_eq!(metrics.total_tokens, 350);
        assert_eq!(metrics.node_tokens.get("classify"), Some(&50));
        assert_eq!(metrics.node_tokens.get("generate"), Some(&200));
        assert_eq!(metrics.node_tokens.get("summarize"), Some(&100));
    }

    #[test]
    fn test_local_metrics_batch_tokens() {
        let mut batch = LocalMetricsBatch::new();
        batch.record_node_tokens("node1", 100);
        batch.record_node_tokens("node2", 200);
        batch.record_node_tokens("node1", 50); // Same node again

        let mut metrics = ExecutionMetrics::new();
        batch.apply_to(&mut metrics);

        assert_eq!(metrics.total_tokens, 350);
        assert_eq!(metrics.node_tokens.get("node1"), Some(&150)); // 100 + 50
        assert_eq!(metrics.node_tokens.get("node2"), Some(&200));
    }

    #[test]
    fn test_local_metrics_batch_is_empty_with_tokens() {
        let mut batch = LocalMetricsBatch::new();
        assert!(batch.is_empty());

        batch.record_node_tokens("node", 100);
        assert!(!batch.is_empty());
    }
}
