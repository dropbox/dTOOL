// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Parallel Analysis Utilities for Self-Improvement.
//!
//! Provides parallel processing utilities for analyzing multiple
//! execution traces concurrently using rayon.
//!
//! ## Status: Library-Only
//!
//! This module provides internal utilities for the self-improvement system.
//! It is NOT exposed via CLI commands - use programmatically only.
//!
//! ## Design
//!
//! This module provides utilities for parallelizing common trace analysis patterns:
//! - Parallel map-reduce over traces
//! - Parallel aggregation of metrics
//! - Parallel file loading
//!
//! ## Programmatic Usage
//!
//! ```rust,ignore
//! use dashflow::self_improvement::parallel_analysis::*;
//!
//! // Parallel aggregation of node durations
//! let durations = parallel_collect_node_durations(&traces);
//!
//! // Parallel loading of trace files
//! let traces = parallel_load_traces(&paths);
//! ```

use crate::introspection::ExecutionTrace;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// Parallel collection of node durations from traces.
///
/// Returns a HashMap mapping node names to vectors of durations.
/// Uses parallel iteration for efficient processing of large trace sets.
#[must_use]
pub fn parallel_collect_node_durations(traces: &[ExecutionTrace]) -> HashMap<String, Vec<u64>> {
    // Parallel map-reduce: each thread collects its own HashMap, then merge
    traces
        .par_iter()
        .fold(HashMap::new, |mut acc: HashMap<String, Vec<u64>>, trace| {
            for node in &trace.nodes_executed {
                acc.entry(node.node.clone())
                    .or_default()
                    .push(node.duration_ms);
            }
            acc
        })
        .reduce(HashMap::new, |mut a, b| {
            for (key, mut values) in b {
                a.entry(key).or_default().append(&mut values);
            }
            a
        })
}

/// Parallel collection of error counts per node.
///
/// Returns a HashMap mapping node names to error counts.
#[must_use]
pub fn parallel_collect_error_counts(traces: &[ExecutionTrace]) -> HashMap<String, usize> {
    traces
        .par_iter()
        .fold(HashMap::new, |mut acc: HashMap<String, usize>, trace| {
            for error in &trace.errors {
                *acc.entry(error.node.clone()).or_insert(0) += 1;
            }
            acc
        })
        .reduce(HashMap::new, |mut a, b| {
            for (key, count) in b {
                *a.entry(key).or_insert(0) += count;
            }
            a
        })
}

/// Parallel collection of node execution counts.
///
/// Returns a HashMap mapping node names to execution counts.
#[must_use]
pub fn parallel_collect_node_usage(traces: &[ExecutionTrace]) -> HashMap<String, usize> {
    traces
        .par_iter()
        .fold(HashMap::new, |mut acc: HashMap<String, usize>, trace| {
            for node in &trace.nodes_executed {
                *acc.entry(node.node.clone()).or_insert(0) += 1;
            }
            acc
        })
        .reduce(HashMap::new, |mut a, b| {
            for (key, count) in b {
                *a.entry(key).or_insert(0) += count;
            }
            a
        })
}

/// Parallel loading of trace files from paths.
///
/// Returns successfully loaded traces and skips files that fail to parse.
/// Logs warnings for failed loads.
#[must_use]
pub fn parallel_load_traces(paths: &[impl AsRef<Path> + Sync]) -> Vec<ExecutionTrace> {
    paths
        .par_iter()
        .filter_map(|path| {
            let path = path.as_ref();
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<ExecutionTrace>(&content) {
                    Ok(trace) => Some(trace),
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to parse trace file"
                        );
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to read trace file"
                    );
                    None
                }
            }
        })
        .collect()
}

/// Parallel collection of tool usage from traces.
///
/// Extracts tool names mentioned in trace metadata/errors and counts usage.
#[must_use]
pub fn parallel_collect_tool_usage(traces: &[ExecutionTrace]) -> HashMap<String, usize> {
    traces
        .par_iter()
        .fold(HashMap::new, |mut acc: HashMap<String, usize>, trace| {
            // Tools are typically recorded in node metadata or as node types
            for node in &trace.nodes_executed {
                // If node name looks like a tool invocation
                if node.node.contains("tool_") || node.node.ends_with("_tool") {
                    *acc.entry(node.node.clone()).or_insert(0) += 1;
                }
            }
            acc
        })
        .reduce(HashMap::new, |mut a, b| {
            for (key, count) in b {
                *a.entry(key).or_insert(0) += count;
            }
            a
        })
}

/// Compute parallel statistics over trace durations.
///
/// Returns (min, max, sum, count) for efficient parallel aggregation.
///
/// # Empty Traces
///
/// When `traces` is empty, returns `(u64::MAX, 0, 0, 0)`:
/// - `min = u64::MAX` indicates no valid minimum was computed
/// - `max = 0` indicates no valid maximum was computed
/// - `sum = 0` and `count = 0` correctly indicate empty input
///
/// Callers should check `count == 0` before using min/max values.
///
/// # Overflow Safety
///
/// The sum is computed using saturating arithmetic. For very large trace sets
/// with large durations, the sum may saturate at `u64::MAX` rather than overflow.
#[must_use]
pub fn parallel_duration_stats(traces: &[ExecutionTrace]) -> (u64, u64, u64, usize) {
    traces
        .par_iter()
        .map(|trace| {
            let duration = trace.total_duration_ms;
            (duration, duration, duration, 1usize)
        })
        .reduce(
            || (u64::MAX, 0u64, 0u64, 0usize),
            |(min1, max1, sum1, count1), (min2, max2, sum2, count2)| {
                (
                    min1.min(min2),
                    max1.max(max2),
                    sum1.saturating_add(sum2),
                    count1 + count2,
                )
            },
        )
}

/// Parallel analysis result merger.
///
/// Merges two HashMaps of vectors by appending values for matching keys.
pub fn merge_vec_maps<K, V>(mut a: HashMap<K, Vec<V>>, b: HashMap<K, Vec<V>>) -> HashMap<K, Vec<V>>
where
    K: std::hash::Hash + Eq,
{
    for (key, mut values) in b {
        a.entry(key).or_default().append(&mut values);
    }
    a
}

/// Parallel analysis result merger for counts.
///
/// Merges two HashMaps of counts by summing values for matching keys.
pub fn merge_count_maps<K>(mut a: HashMap<K, usize>, b: HashMap<K, usize>) -> HashMap<K, usize>
where
    K: std::hash::Hash + Eq,
{
    for (key, count) in b {
        *a.entry(key).or_insert(0) += count;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ErrorTrace, NodeExecution};
    use chrono::Utc;

    fn create_test_trace(node_names: &[&str], durations: &[u64]) -> ExecutionTrace {
        let nodes: Vec<NodeExecution> = node_names
            .iter()
            .zip(durations.iter())
            .enumerate()
            .map(|(i, (name, &duration))| NodeExecution {
                node: name.to_string(),
                duration_ms: duration,
                tokens_used: 30,
                state_before: None,
                state_after: None,
                tools_called: Vec::new(),
                success: true,
                error_message: None,
                index: i,
                started_at: None,
                metadata: std::collections::HashMap::new(),
            })
            .collect();

        ExecutionTrace {
            thread_id: Some("test-thread".to_string()),
            execution_id: Some(uuid::Uuid::new_v4().to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            nodes_executed: nodes,
            total_duration_ms: durations.iter().sum(),
            total_tokens: 30,
            errors: Vec::new(),
            completed: true,
            started_at: Some(Utc::now().to_rfc3339()),
            ended_at: Some(Utc::now().to_rfc3339()),
            final_state: None,
            metadata: std::collections::HashMap::new(),
            execution_metrics: None,
            performance_metrics: None,
        }
    }

    fn create_trace_with_errors(errors: &[(&str, &str)]) -> ExecutionTrace {
        let mut trace = create_test_trace(&["node1"], &[100]);
        trace.errors = errors
            .iter()
            .map(|(node, msg)| ErrorTrace {
                node: node.to_string(),
                message: msg.to_string(),
                error_type: Some("test".to_string()),
                state_at_error: None,
                timestamp: Some(Utc::now().to_rfc3339()),
                execution_index: None,
                recoverable: false,
                retry_attempted: false,
                context: None,
                metadata: std::collections::HashMap::new(),
            })
            .collect();
        trace
    }

    #[test]
    fn test_parallel_collect_node_durations() {
        let traces = vec![
            create_test_trace(&["node_a", "node_b"], &[100, 200]),
            create_test_trace(&["node_a", "node_c"], &[150, 300]),
            create_test_trace(&["node_b", "node_c"], &[250, 350]),
        ];

        let durations = parallel_collect_node_durations(&traces);

        assert_eq!(durations.get("node_a").unwrap().len(), 2);
        assert_eq!(durations.get("node_b").unwrap().len(), 2);
        assert_eq!(durations.get("node_c").unwrap().len(), 2);
    }

    #[test]
    fn test_parallel_collect_error_counts() {
        let traces = vec![
            create_trace_with_errors(&[("node_a", "error 1")]),
            create_trace_with_errors(&[("node_a", "error 2"), ("node_b", "error 3")]),
            create_trace_with_errors(&[("node_b", "error 4")]),
        ];

        let errors = parallel_collect_error_counts(&traces);

        assert_eq!(*errors.get("node_a").unwrap(), 2);
        assert_eq!(*errors.get("node_b").unwrap(), 2);
    }

    #[test]
    fn test_parallel_collect_node_usage() {
        let traces = vec![
            create_test_trace(&["node_a", "node_b"], &[100, 200]),
            create_test_trace(&["node_a"], &[150]),
            create_test_trace(&["node_a", "node_b", "node_c"], &[100, 200, 300]),
        ];

        let usage = parallel_collect_node_usage(&traces);

        assert_eq!(*usage.get("node_a").unwrap(), 3);
        assert_eq!(*usage.get("node_b").unwrap(), 2);
        assert_eq!(*usage.get("node_c").unwrap(), 1);
    }

    #[test]
    fn test_parallel_duration_stats() {
        let traces = vec![
            create_test_trace(&["node"], &[100]),
            create_test_trace(&["node"], &[200]),
            create_test_trace(&["node"], &[300]),
        ];

        let (min, max, sum, count) = parallel_duration_stats(&traces);

        assert_eq!(min, 100);
        assert_eq!(max, 300);
        assert_eq!(sum, 600);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_merge_vec_maps() {
        let mut a: HashMap<String, Vec<u64>> = HashMap::new();
        a.insert("key1".to_string(), vec![1, 2]);
        a.insert("key2".to_string(), vec![3]);

        let mut b: HashMap<String, Vec<u64>> = HashMap::new();
        b.insert("key1".to_string(), vec![4, 5]);
        b.insert("key3".to_string(), vec![6]);

        let merged = merge_vec_maps(a, b);

        assert_eq!(merged.get("key1").unwrap().len(), 4);
        assert_eq!(merged.get("key2").unwrap().len(), 1);
        assert_eq!(merged.get("key3").unwrap().len(), 1);
    }

    #[test]
    fn test_merge_count_maps() {
        let mut a: HashMap<String, usize> = HashMap::new();
        a.insert("key1".to_string(), 5);
        a.insert("key2".to_string(), 3);

        let mut b: HashMap<String, usize> = HashMap::new();
        b.insert("key1".to_string(), 2);
        b.insert("key3".to_string(), 7);

        let merged = merge_count_maps(a, b);

        assert_eq!(*merged.get("key1").unwrap(), 7);
        assert_eq!(*merged.get("key2").unwrap(), 3);
        assert_eq!(*merged.get("key3").unwrap(), 7);
    }
}
