//! Shared Trace Analysis Primitives
//!
//! This module provides common types and traits for analyzing execution traces
//! across the codebase. It consolidates duplicate analysis logic from:
//! - `introspection/trace.rs` - ExecutionTrace analysis methods
//! - `self_improvement/analyzers.rs` - Gap and pattern detection
//! - `adaptive_timeout.rs` - LatencyStats calculation
//! - `unified_introspection.rs` - Percentile calculations
//!
//! # Key Types
//!
//! - [`NodeMetrics`] - Aggregated metrics for a single node across multiple executions
//! - [`TraceStats`] - Summary statistics for a collection of traces
//! - [`percentile`] - Unified percentile calculation
//!
//! # Key Traits
//!
//! - [`TraceVisitor`] - Visitor pattern for trace traversal
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::trace_analysis::{TraceStats, percentile};
//!
//! let traces: Vec<ExecutionTrace> = /* ... */;
//! let stats = TraceStats::from_traces(&traces);
//!
//! println!("Total executions: {}", stats.total_executions);
//! println!("Success rate: {:.1}%", stats.success_rate() * 100.0);
//!
//! // Calculate p95 of a sample
//! let durations = vec![100.0, 200.0, 300.0, 400.0, 500.0];
//! let p95 = percentile(&durations, 0.95);
//! ```

use crate::introspection::{ErrorTrace, ExecutionTrace, NodeExecution};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Percentile Calculation - Shared Implementation
// =============================================================================

/// Calculate the percentile of a sorted slice of values.
///
/// This is the canonical percentile implementation for the codebase. Use this
/// instead of implementing percentile calculations inline.
///
/// # Arguments
///
/// * `sorted` - A sorted slice of f64 values (must be pre-sorted!)
/// * `p` - The percentile to calculate (0.0 to 1.0, e.g., 0.95 for p95)
///
/// # Returns
///
/// The percentile value, or 0.0 if the slice is empty.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::trace_analysis::percentile;
///
/// let mut values = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
/// values.sort_by(|a, b| a.partial_cmp(b).unwrap());
///
/// assert_eq!(percentile(&values, 0.50), 50.0); // median
/// assert_eq!(percentile(&values, 0.95), 95.0); // p95
/// ```
#[must_use]
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    let idx = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Calculate the percentile of u64 values.
///
/// Convenience wrapper for integer durations (e.g., milliseconds).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::trace_analysis::percentile_u64;
///
/// let mut durations: Vec<u64> = vec![100, 200, 300, 400, 500];
/// durations.sort();
///
/// assert_eq!(percentile_u64(&durations, 0.95), 500);
/// ```
#[must_use]
pub fn percentile_u64(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    let idx = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// =============================================================================
// NodeMetrics - Aggregated Node Statistics
// =============================================================================

/// Aggregated metrics for a single node across multiple executions.
///
/// This type consolidates metrics that were previously calculated inline
/// in multiple modules (introspection, self_improvement, adaptive_timeout).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::trace_analysis::NodeMetrics;
/// use dashflow::introspection::ExecutionTrace;
///
/// let traces: Vec<ExecutionTrace> = /* ... */;
/// let metrics = NodeMetrics::from_traces("my_node", &traces);
///
/// println!("Node: {}", metrics.node_name);
/// println!("Executions: {}", metrics.execution_count);
/// println!("Success rate: {:.1}%", metrics.success_rate() * 100.0);
/// println!("P95 latency: {}ms", metrics.p95_duration_ms);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    /// Node identifier
    pub node_name: String,
    /// Total number of executions
    pub execution_count: usize,
    /// Number of successful executions (no errors)
    pub successful_executions: usize,
    /// Number of failed executions
    pub failed_executions: usize,
    /// Number of retried executions
    pub retry_count: usize,
    /// Minimum duration in milliseconds
    pub min_duration_ms: u64,
    /// Maximum duration in milliseconds
    pub max_duration_ms: u64,
    /// Mean duration in milliseconds
    pub mean_duration_ms: f64,
    /// Standard deviation of duration in milliseconds
    pub std_dev_duration_ms: f64,
    /// Median (p50) duration in milliseconds
    pub p50_duration_ms: u64,
    /// 95th percentile duration in milliseconds
    pub p95_duration_ms: u64,
    /// 99th percentile duration in milliseconds
    pub p99_duration_ms: u64,
    /// Total tokens used across all executions
    pub total_tokens: u64,
    /// Mean tokens per execution
    pub mean_tokens: f64,
    /// Total duration across all executions
    pub total_duration_ms: u64,
}

impl NodeMetrics {
    /// Create metrics from traces for a specific node.
    #[must_use]
    pub fn from_traces(node_name: &str, traces: &[ExecutionTrace]) -> Self {
        let mut durations: Vec<u64> = Vec::new();
        let mut tokens: Vec<u64> = Vec::new();
        let mut error_count = 0;
        let mut retry_count = 0;

        for trace in traces {
            // Collect executions for this node
            for exec in &trace.nodes_executed {
                if exec.node == node_name {
                    durations.push(exec.duration_ms);
                    tokens.push(exec.tokens_used);
                }
            }

            // Count errors and retries for this node
            for error in &trace.errors {
                if error.node == node_name {
                    error_count += 1;
                    if error.retry_attempted {
                        retry_count += 1;
                    }
                }
            }
        }

        Self::from_raw_data(node_name, &durations, &tokens, error_count, retry_count)
    }

    /// Create metrics from node executions directly.
    #[must_use]
    pub fn from_executions(node_name: &str, executions: &[&NodeExecution]) -> Self {
        let durations: Vec<u64> = executions.iter().map(|e| e.duration_ms).collect();
        let tokens: Vec<u64> = executions.iter().map(|e| e.tokens_used).collect();

        Self::from_raw_data(node_name, &durations, &tokens, 0, 0)
    }

    /// Create metrics from raw duration and token data.
    #[must_use]
    fn from_raw_data(
        node_name: &str,
        durations: &[u64],
        tokens: &[u64],
        error_count: usize,
        retry_count: usize,
    ) -> Self {
        if durations.is_empty() {
            return Self {
                node_name: node_name.to_string(),
                execution_count: 0,
                successful_executions: 0,
                failed_executions: 0,
                retry_count: 0,
                min_duration_ms: 0,
                max_duration_ms: 0,
                mean_duration_ms: 0.0,
                std_dev_duration_ms: 0.0,
                p50_duration_ms: 0,
                p95_duration_ms: 0,
                p99_duration_ms: 0,
                total_tokens: 0,
                mean_tokens: 0.0,
                total_duration_ms: 0,
            };
        }

        // Sort durations for percentile calculation
        let mut sorted_durations = durations.to_vec();
        sorted_durations.sort_unstable();

        let execution_count = durations.len();
        let total_duration: u64 = durations.iter().sum();
        let mean_duration = total_duration as f64 / execution_count as f64;

        // Standard deviation
        let variance: f64 = durations
            .iter()
            .map(|d| (*d as f64 - mean_duration).powi(2))
            .sum::<f64>()
            / execution_count as f64;
        let std_dev = variance.sqrt();

        // Token stats
        let total_tokens: u64 = tokens.iter().sum();
        let mean_tokens = if tokens.is_empty() {
            0.0
        } else {
            total_tokens as f64 / tokens.len() as f64
        };

        Self {
            node_name: node_name.to_string(),
            execution_count,
            successful_executions: execution_count.saturating_sub(error_count),
            failed_executions: error_count,
            retry_count,
            min_duration_ms: sorted_durations[0],
            max_duration_ms: sorted_durations[execution_count - 1],
            mean_duration_ms: mean_duration,
            std_dev_duration_ms: std_dev,
            p50_duration_ms: percentile_u64(&sorted_durations, 0.50),
            p95_duration_ms: percentile_u64(&sorted_durations, 0.95),
            p99_duration_ms: percentile_u64(&sorted_durations, 0.99),
            total_tokens,
            mean_tokens,
            total_duration_ms: total_duration,
        }
    }

    /// Calculate success rate (0.0 to 1.0).
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.execution_count == 0 {
            0.0
        } else {
            self.successful_executions as f64 / self.execution_count as f64
        }
    }

    /// Calculate retry rate (0.0 to 1.0).
    #[must_use]
    pub fn retry_rate(&self) -> f64 {
        if self.execution_count == 0 {
            0.0
        } else {
            self.retry_count as f64 / self.execution_count as f64
        }
    }

    /// Calculate coefficient of variation (std_dev / mean).
    ///
    /// Low CV (<0.3) indicates stable latency, high CV indicates variable latency.
    #[must_use]
    pub fn coefficient_of_variation(&self) -> f64 {
        if self.mean_duration_ms > 0.0 {
            self.std_dev_duration_ms / self.mean_duration_ms
        } else {
            0.0
        }
    }

    /// Check if latency is stable (CV < 0.3).
    #[must_use]
    pub fn is_stable(&self) -> bool {
        self.coefficient_of_variation() < 0.3
    }
}

// =============================================================================
// TraceStats - Summary Statistics for Trace Collections
// =============================================================================

/// Summary statistics for a collection of execution traces.
///
/// Use this to get an overview of execution behavior across multiple traces.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::trace_analysis::TraceStats;
///
/// let traces: Vec<ExecutionTrace> = /* ... */;
/// let stats = TraceStats::from_traces(&traces);
///
/// println!("Total executions: {}", stats.total_executions);
/// println!("Unique nodes: {}", stats.unique_node_count);
/// println!("Error rate: {:.1}%", (1.0 - stats.success_rate()) * 100.0);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceStats {
    /// Total number of traces analyzed
    pub trace_count: usize,
    /// Total execution count across all traces
    pub total_executions: usize,
    /// Number of successful traces (completed without errors)
    pub successful_traces: usize,
    /// Number of failed traces
    pub failed_traces: usize,
    /// Total errors across all traces
    pub total_errors: usize,
    /// Total retries across all traces
    pub total_retries: usize,
    /// Number of unique nodes executed
    pub unique_node_count: usize,
    /// Total duration across all traces (ms)
    pub total_duration_ms: u64,
    /// Mean trace duration (ms)
    pub mean_duration_ms: f64,
    /// Total tokens used
    pub total_tokens: u64,
    /// Mean tokens per trace
    pub mean_tokens_per_trace: f64,
    /// Per-node metrics
    pub node_metrics: HashMap<String, NodeMetrics>,
}

impl TraceStats {
    /// Calculate statistics from a collection of traces.
    #[must_use]
    pub fn from_traces(traces: &[ExecutionTrace]) -> Self {
        if traces.is_empty() {
            return Self::default();
        }

        let mut total_executions = 0;
        let mut successful_traces = 0;
        let mut total_errors = 0;
        let mut total_retries = 0;
        let mut total_duration: u64 = 0;
        let mut total_tokens: u64 = 0;
        let mut node_durations: HashMap<String, Vec<u64>> = HashMap::new();
        let mut node_tokens: HashMap<String, Vec<u64>> = HashMap::new();
        let mut node_errors: HashMap<String, usize> = HashMap::new();
        let mut node_retries: HashMap<String, usize> = HashMap::new();

        for trace in traces {
            total_executions += trace.nodes_executed.len();
            total_duration += trace.total_duration_ms;
            total_tokens += trace.total_tokens;

            if trace.is_successful() {
                successful_traces += 1;
            }

            for node in &trace.nodes_executed {
                node_durations
                    .entry(node.node.clone())
                    .or_default()
                    .push(node.duration_ms);
                node_tokens
                    .entry(node.node.clone())
                    .or_default()
                    .push(node.tokens_used);
            }

            for error in &trace.errors {
                total_errors += 1;
                *node_errors.entry(error.node.clone()).or_insert(0) += 1;
                if error.retry_attempted {
                    total_retries += 1;
                    *node_retries.entry(error.node.clone()).or_insert(0) += 1;
                }
            }
        }

        // Build per-node metrics
        let empty_tokens: Vec<u64> = Vec::new();
        let mut node_metrics = HashMap::new();
        for (node_name, durations) in &node_durations {
            let tokens = node_tokens.get(node_name).unwrap_or(&empty_tokens);
            let errors = node_errors.get(node_name).copied().unwrap_or(0);
            let retries = node_retries.get(node_name).copied().unwrap_or(0);

            let metrics = NodeMetrics::from_raw_data(node_name, durations, tokens, errors, retries);
            node_metrics.insert(node_name.clone(), metrics);
        }

        let trace_count = traces.len();

        Self {
            trace_count,
            total_executions,
            successful_traces,
            failed_traces: trace_count - successful_traces,
            total_errors,
            total_retries,
            unique_node_count: node_durations.len(),
            total_duration_ms: total_duration,
            mean_duration_ms: total_duration as f64 / trace_count as f64,
            total_tokens,
            mean_tokens_per_trace: total_tokens as f64 / trace_count as f64,
            node_metrics,
        }
    }

    /// Calculate success rate (0.0 to 1.0).
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.trace_count == 0 {
            0.0
        } else {
            self.successful_traces as f64 / self.trace_count as f64
        }
    }

    /// Get metrics for a specific node.
    #[must_use]
    pub fn get_node_metrics(&self, node_name: &str) -> Option<&NodeMetrics> {
        self.node_metrics.get(node_name)
    }

    /// Get the slowest node by p95 latency.
    #[must_use]
    pub fn slowest_node(&self) -> Option<(&str, &NodeMetrics)> {
        self.node_metrics
            .iter()
            .max_by_key(|(_, m)| m.p95_duration_ms)
            .map(|(name, metrics)| (name.as_str(), metrics))
    }

    /// Get nodes with high retry rates (> threshold).
    #[must_use]
    pub fn high_retry_nodes(&self, threshold: f64) -> Vec<(&str, &NodeMetrics)> {
        self.node_metrics
            .iter()
            .filter(|(_, m)| m.retry_rate() > threshold)
            .map(|(name, metrics)| (name.as_str(), metrics))
            .collect()
    }

    /// Get nodes with low success rates (< threshold).
    #[must_use]
    pub fn low_success_nodes(&self, threshold: f64) -> Vec<(&str, &NodeMetrics)> {
        self.node_metrics
            .iter()
            .filter(|(_, m)| m.success_rate() < threshold)
            .map(|(name, metrics)| (name.as_str(), metrics))
            .collect()
    }
}

// =============================================================================
// TraceVisitor - Visitor Pattern for Trace Traversal
// =============================================================================

/// Trait for visiting execution trace elements.
///
/// Implement this trait to create custom trace analysis logic without
/// duplicating traversal code.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::trace_analysis::TraceVisitor;
/// use dashflow::introspection::{ExecutionTrace, NodeExecution, ErrorTrace};
///
/// struct TokenCounter {
///     total_tokens: u64,
/// }
///
/// impl TraceVisitor for TokenCounter {
///     fn visit_node(&mut self, trace: &ExecutionTrace, node: &NodeExecution) {
///         self.total_tokens += node.tokens_used;
///     }
/// }
///
/// let mut counter = TokenCounter { total_tokens: 0 };
/// counter.visit_traces(&traces);
/// println!("Total tokens: {}", counter.total_tokens);
/// ```
pub trait TraceVisitor {
    /// Called before processing a trace.
    fn visit_trace_start(&mut self, _trace: &ExecutionTrace) {}

    /// Called for each node execution in a trace.
    fn visit_node(&mut self, _trace: &ExecutionTrace, _node: &NodeExecution) {}

    /// Called for each error in a trace.
    fn visit_error(&mut self, _trace: &ExecutionTrace, _error: &ErrorTrace) {}

    /// Called after processing a trace.
    fn visit_trace_end(&mut self, _trace: &ExecutionTrace) {}

    /// Visit all traces in a collection.
    fn visit_traces(&mut self, traces: &[ExecutionTrace]) {
        for trace in traces {
            self.visit_trace_start(trace);
            for node in &trace.nodes_executed {
                self.visit_node(trace, node);
            }
            for error in &trace.errors {
                self.visit_error(trace, error);
            }
            self.visit_trace_end(trace);
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ErrorTrace, ExecutionTrace, NodeExecution};

    fn create_test_trace(
        nodes: Vec<(&str, u64, u64, bool)>, // (name, duration_ms, tokens, success)
        errors: Vec<(&str, &str, bool)>,    // (node, message, retry_attempted)
    ) -> ExecutionTrace {
        let mut trace = ExecutionTrace::new();
        trace.completed = errors.is_empty();
        trace.nodes_executed = nodes
            .iter()
            .map(|(name, duration, tokens, _)| {
                NodeExecution::new(*name, *duration).with_tokens(*tokens)
            })
            .collect();
        trace.total_duration_ms = nodes.iter().map(|(_, d, _, _)| d).sum();
        trace.total_tokens = nodes.iter().map(|(_, _, t, _)| t).sum();
        trace.errors = errors
            .iter()
            .map(|(node, msg, retry)| {
                let error = ErrorTrace::new(*node, *msg);
                if *retry {
                    error.with_retry_attempted()
                } else {
                    error
                }
            })
            .collect();
        trace
    }

    #[test]
    fn test_percentile_basic() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];

        assert_eq!(percentile(&values, 0.0), 10.0);
        // p50 on 10 elements: index = round(0.5 * 9) = round(4.5) = 5 -> value 60
        assert_eq!(percentile(&values, 0.5), 60.0);
        assert_eq!(percentile(&values, 1.0), 100.0);
    }

    #[test]
    fn test_percentile_empty() {
        let values: Vec<f64> = vec![];
        assert_eq!(percentile(&values, 0.5), 0.0);
    }

    #[test]
    fn test_percentile_single() {
        let values = vec![42.0];
        assert_eq!(percentile(&values, 0.5), 42.0);
        assert_eq!(percentile(&values, 0.95), 42.0);
    }

    #[test]
    fn test_percentile_u64_basic() {
        let values: Vec<u64> = vec![100, 200, 300, 400, 500];

        assert_eq!(percentile_u64(&values, 0.0), 100);
        assert_eq!(percentile_u64(&values, 0.5), 300);
        assert_eq!(percentile_u64(&values, 1.0), 500);
    }

    #[test]
    fn test_node_metrics_from_traces() {
        let traces = vec![
            create_test_trace(
                vec![("node1", 100, 50, true), ("node2", 200, 100, true)],
                vec![],
            ),
            create_test_trace(
                vec![("node1", 150, 60, true), ("node2", 250, 120, true)],
                vec![("node1", "Error", true)],
            ),
        ];

        let metrics = NodeMetrics::from_traces("node1", &traces);

        assert_eq!(metrics.node_name, "node1");
        assert_eq!(metrics.execution_count, 2);
        assert_eq!(metrics.min_duration_ms, 100);
        assert_eq!(metrics.max_duration_ms, 150);
        assert_eq!(metrics.total_tokens, 110);
        assert_eq!(metrics.retry_count, 1);
    }

    #[test]
    fn test_node_metrics_empty() {
        let traces: Vec<ExecutionTrace> = vec![];
        let metrics = NodeMetrics::from_traces("nonexistent", &traces);

        assert_eq!(metrics.execution_count, 0);
        assert_eq!(metrics.success_rate(), 0.0);
    }

    #[test]
    fn test_trace_stats_basic() {
        let traces = vec![
            create_test_trace(
                vec![("node1", 100, 50, true), ("node2", 200, 100, true)],
                vec![],
            ),
            create_test_trace(
                vec![("node1", 150, 60, true)],
                vec![("node1", "Error", false)],
            ),
        ];

        let stats = TraceStats::from_traces(&traces);

        assert_eq!(stats.trace_count, 2);
        assert_eq!(stats.successful_traces, 1);
        assert_eq!(stats.failed_traces, 1);
        assert_eq!(stats.total_executions, 3);
        assert_eq!(stats.unique_node_count, 2);
        assert_eq!(stats.total_errors, 1);
    }

    #[test]
    fn test_trace_stats_empty() {
        let traces: Vec<ExecutionTrace> = vec![];
        let stats = TraceStats::from_traces(&traces);

        assert_eq!(stats.trace_count, 0);
        assert_eq!(stats.success_rate(), 0.0);
    }

    #[test]
    fn test_trace_stats_slowest_node() {
        let traces = vec![
            create_test_trace(
                vec![("fast_node", 50, 10, true), ("slow_node", 500, 10, true)],
                vec![],
            ),
            create_test_trace(
                vec![("fast_node", 60, 10, true), ("slow_node", 600, 10, true)],
                vec![],
            ),
        ];

        let stats = TraceStats::from_traces(&traces);
        let (name, _) = stats.slowest_node().unwrap();
        assert_eq!(name, "slow_node");
    }

    #[test]
    fn test_coefficient_of_variation() {
        let traces = vec![
            create_test_trace(vec![("stable", 100, 10, true)], vec![]),
            create_test_trace(vec![("stable", 102, 10, true)], vec![]),
            create_test_trace(vec![("stable", 98, 10, true)], vec![]),
            create_test_trace(vec![("stable", 100, 10, true)], vec![]),
        ];

        let metrics = NodeMetrics::from_traces("stable", &traces);
        assert!(metrics.is_stable());
        assert!(metrics.coefficient_of_variation() < 0.1);
    }

    struct TestVisitor {
        node_count: usize,
        error_count: usize,
        trace_count: usize,
    }

    impl TraceVisitor for TestVisitor {
        fn visit_trace_start(&mut self, _trace: &ExecutionTrace) {
            self.trace_count += 1;
        }

        fn visit_node(&mut self, _trace: &ExecutionTrace, _node: &NodeExecution) {
            self.node_count += 1;
        }

        fn visit_error(&mut self, _trace: &ExecutionTrace, _error: &ErrorTrace) {
            self.error_count += 1;
        }
    }

    #[test]
    fn test_trace_visitor() {
        let traces = vec![
            create_test_trace(
                vec![("node1", 100, 10, true), ("node2", 200, 20, true)],
                vec![],
            ),
            create_test_trace(
                vec![("node1", 150, 15, true)],
                vec![("node1", "Error", false)],
            ),
        ];

        let mut visitor = TestVisitor {
            node_count: 0,
            error_count: 0,
            trace_count: 0,
        };

        visitor.visit_traces(&traces);

        assert_eq!(visitor.trace_count, 2);
        assert_eq!(visitor.node_count, 3);
        assert_eq!(visitor.error_count, 1);
    }

    // =============================================================================
    // M-304: Layer 4 Graph Viewer Validation Tests
    // =============================================================================

    #[test]
    fn test_node_metrics_percentiles() {
        // Create traces with known durations for percentile verification
        // Durations: 100, 200, 300, 400, 500, 600, 700, 800, 900, 1000 (10 samples)
        let traces: Vec<ExecutionTrace> = (1..=10)
            .map(|i| create_test_trace(vec![("node", i * 100, 10, true)], vec![]))
            .collect();

        let metrics = NodeMetrics::from_traces("node", &traces);

        assert_eq!(metrics.execution_count, 10);
        assert_eq!(metrics.min_duration_ms, 100);
        assert_eq!(metrics.max_duration_ms, 1000);

        // Verify percentiles (sorted: 100, 200, ..., 1000)
        // p50 (median): index = round(0.50 * 9) = 5 -> 600
        assert_eq!(metrics.p50_duration_ms, 600);
        // p95: index = round(0.95 * 9) = round(8.55) = 9 -> 1000
        assert_eq!(metrics.p95_duration_ms, 1000);
        // p99: index = round(0.99 * 9) = round(8.91) = 9 -> 1000
        assert_eq!(metrics.p99_duration_ms, 1000);
    }

    #[test]
    fn test_node_metrics_statistical_sanity() {
        // Create traces with varied durations to verify statistical ordering
        // Durations: 50, 100, 150, 200, 250, 300, 350, 400, 500, 1000 (outlier)
        let durations = vec![50, 100, 150, 200, 250, 300, 350, 400, 500, 1000];
        let traces: Vec<ExecutionTrace> = durations
            .iter()
            .map(|d| create_test_trace(vec![("node", *d, 10, true)], vec![]))
            .collect();

        let metrics = NodeMetrics::from_traces("node", &traces);

        // Statistical sanity: p99 >= p95 >= p50 >= mean (for this distribution)
        // Note: mean can be > p50 when there are outliers, so we only check p99 >= p95 >= p50
        assert!(
            metrics.p99_duration_ms >= metrics.p95_duration_ms,
            "p99 ({}) should be >= p95 ({})",
            metrics.p99_duration_ms,
            metrics.p95_duration_ms
        );
        assert!(
            metrics.p95_duration_ms >= metrics.p50_duration_ms,
            "p95 ({}) should be >= p50 ({})",
            metrics.p95_duration_ms,
            metrics.p50_duration_ms
        );

        // Verify mean calculation
        let expected_mean = durations.iter().sum::<u64>() as f64 / durations.len() as f64;
        assert!(
            (metrics.mean_duration_ms - expected_mean).abs() < 0.001,
            "mean should be {}, got {}",
            expected_mean,
            metrics.mean_duration_ms
        );
    }

    #[test]
    fn test_node_metrics_aggregation_large_sample() {
        // Test aggregation with 100 samples to verify statistical accuracy
        let traces: Vec<ExecutionTrace> = (1..=100)
            .map(|i| {
                // Create varied durations: base + some variance
                let duration = 100 + (i % 10) * 10; // 100-190ms range
                create_test_trace(vec![("node", duration, i, true)], vec![])
            })
            .collect();

        let metrics = NodeMetrics::from_traces("node", &traces);

        assert_eq!(metrics.execution_count, 100);

        // With 100 samples, percentiles should be well-defined
        // All durations are in 100-190 range
        assert!(metrics.p50_duration_ms >= 100 && metrics.p50_duration_ms <= 190);
        assert!(metrics.p95_duration_ms >= 100 && metrics.p95_duration_ms <= 190);
        assert!(metrics.p99_duration_ms >= 100 && metrics.p99_duration_ms <= 190);

        // Mean should be around 145 (average of 100-190 evenly distributed)
        assert!(
            metrics.mean_duration_ms >= 100.0 && metrics.mean_duration_ms <= 190.0,
            "mean {} should be in range [100, 190]",
            metrics.mean_duration_ms
        );

        // Verify total tokens aggregation
        // Sum of 1..=100 = 5050
        assert_eq!(metrics.total_tokens, 5050);
        assert!(
            (metrics.mean_tokens - 50.5).abs() < 0.001,
            "mean_tokens should be 50.5, got {}",
            metrics.mean_tokens
        );
    }

    #[test]
    fn test_node_metrics_mean_duration() {
        // Explicitly verify mean duration calculation
        let traces = vec![
            create_test_trace(vec![("node", 100, 10, true)], vec![]),
            create_test_trace(vec![("node", 200, 10, true)], vec![]),
            create_test_trace(vec![("node", 300, 10, true)], vec![]),
        ];

        let metrics = NodeMetrics::from_traces("node", &traces);

        // Mean of 100, 200, 300 = 200
        assert!(
            (metrics.mean_duration_ms - 200.0).abs() < 0.001,
            "mean should be 200.0, got {}",
            metrics.mean_duration_ms
        );

        // Verify standard deviation
        // Variance = ((100-200)^2 + (200-200)^2 + (300-200)^2) / 3 = 20000/3 = 6666.67
        // StdDev = sqrt(6666.67) ≈ 81.65
        assert!(
            (metrics.std_dev_duration_ms - 81.65).abs() < 0.1,
            "std_dev should be ~81.65, got {}",
            metrics.std_dev_duration_ms
        );
    }

    #[test]
    fn test_trace_stats_node_metrics_consistency() {
        // Verify that TraceStats and NodeMetrics are consistent when analyzing same data
        let traces = vec![
            create_test_trace(
                vec![("fast", 50, 10, true), ("slow", 500, 100, true)],
                vec![],
            ),
            create_test_trace(
                vec![("fast", 60, 10, true), ("slow", 600, 100, true)],
                vec![],
            ),
            create_test_trace(
                vec![("fast", 70, 10, true), ("slow", 700, 100, true)],
                vec![],
            ),
        ];

        let stats = TraceStats::from_traces(&traces);
        let fast_metrics = NodeMetrics::from_traces("fast", &traces);
        let slow_metrics = NodeMetrics::from_traces("slow", &traces);

        // Total executions from stats should equal sum of node execution counts
        assert_eq!(
            stats.total_executions,
            fast_metrics.execution_count + slow_metrics.execution_count
        );

        // Slowest node should be "slow"
        let (slowest_name, _) = stats.slowest_node().unwrap();
        assert_eq!(slowest_name, "slow");

        // Verify per-node metrics are correct
        assert_eq!(fast_metrics.execution_count, 3);
        assert_eq!(slow_metrics.execution_count, 3);
        assert!(fast_metrics.p95_duration_ms < slow_metrics.p95_duration_ms);
    }
}
