// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Automatic trace persistence for graph execution.
//!
//! This module provides automatic tracing of graph executions with PII hygiene.
//! Traces are saved to `.dashflow/traces/` by default.
//!
//! # Unified Tracing (FIX-008)
//!
//! When WAL is enabled (`DASHFLOW_WAL=true`, the default), traces are written to
//! BOTH locations:
//!
//! 1. **Project-local**: `.dashflow/traces/{execution_id}.json` - Simple JSON files
//!    for quick debugging within a project.
//!
//! 2. **User-global WAL**: `~/.dashflow/wal/` - Event store with SQLite index for
//!    cross-project querying via `LearningCorpus` and `dashflow executions` CLI.
//!
//! This unification enables:
//! - Self-improvement agents to learn from past executions across all projects
//! - The `dashflow executions list/show/events` CLI to query all executions
//! - `LearningCorpus::find_similar_executions()` to find relevant past runs
//!
//! # Configuration
//!
//! - `DASHFLOW_TRACE=false` - Disable trace persistence (default: enabled)
//! - `DASHFLOW_TRACE_REDACT=false` - Disable PII redaction (default: enabled)
//! - `DASHFLOW_LIVE_INTROSPECTION=false` - Disable live introspection (default: enabled)
//! - `DASHFLOW_WAL=false` - Disable WAL integration (default: enabled)

use std::time::SystemTime;

use crate::core::config_loader::env_vars::{
    env_bool, DASHFLOW_LIVE_INTROSPECTION, DASHFLOW_TRACE, DASHFLOW_TRACE_REDACT,
};
use crate::introspection::{ExecutionTrace, NodeExecution};
use crate::metrics::ExecutionMetrics;

use super::ExecutionResult;

/// Check if trace persistence is enabled.
///
/// Trace persistence is ON by default (per DESIGN_INVARIANTS.md Invariant 6).
/// Opt-out by setting `DASHFLOW_TRACE=false`.
pub(crate) fn is_trace_persistence_enabled() -> bool {
    env_bool(DASHFLOW_TRACE, true)
}

/// Check if trace redaction is enabled.
///
/// Trace redaction is ON by default for PII hygiene (M-222).
/// Redacts API keys, emails, phone numbers, SSNs, credit cards, JWTs, etc.
/// Opt-out by setting `DASHFLOW_TRACE_REDACT=false`.
pub(crate) fn is_trace_redaction_enabled() -> bool {
    env_bool(DASHFLOW_TRACE_REDACT, true)
}

/// Check if live introspection is enabled.
///
/// Live introspection is ON by default (per DESIGN_INVARIANTS.md Invariant 6).
/// Opt-out by setting `DASHFLOW_LIVE_INTROSPECTION=false`.
pub(crate) fn is_live_introspection_enabled() -> bool {
    env_bool(DASHFLOW_LIVE_INTROSPECTION, true)
}

/// Build an `ExecutionTrace` from execution result and metrics.
pub(crate) fn build_execution_trace<S: crate::state::GraphState + serde::Serialize>(
    result: &ExecutionResult<S>,
    metrics: &ExecutionMetrics,
    graph_name: Option<&str>,
    started_at: SystemTime,
    thread_id: Option<String>,
) -> ExecutionTrace {
    use chrono::{DateTime, Utc};

    let started_at_str = DateTime::<Utc>::from(started_at).to_rfc3339();
    let ended_at = SystemTime::now();
    let ended_at_str = DateTime::<Utc>::from(ended_at).to_rfc3339();

    // Build node executions from metrics
    let mut node_executions: Vec<NodeExecution> = result
        .nodes_executed
        .iter()
        .enumerate()
        .map(|(index, node_name)| {
            let duration_ms = metrics
                .node_durations
                .get(node_name)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            // Get timestamp from metrics (now populated during execution)
            let started_at = metrics.node_timestamps.get(node_name).cloned();

            // Get tokens from metrics (populated via callbacks from LLM providers)
            let tokens_used = metrics.node_tokens.get(node_name).copied().unwrap_or(0);

            NodeExecution {
                node: node_name.clone(),
                duration_ms,
                tokens_used,
                state_before: None,
                state_after: None,
                tools_called: Vec::new(),
                success: true,
                error_message: None,
                index,
                started_at,
                metadata: std::collections::HashMap::new(),
            }
        })
        .collect();

    // If we have node execution counts > 1, a node was executed multiple times
    // (e.g., in a loop). Add additional entries to match actual execution count.
    for (node_name, count) in &metrics.node_execution_counts {
        if *count > 1 {
            let base_index = node_executions.len();
            let duration_per = metrics
                .node_durations
                .get(node_name)
                .map(|d| d.as_millis() as u64 / *count as u64)
                .unwrap_or(0);

            // Get timestamp for this node (same as first execution - we only store latest)
            let started_at = metrics.node_timestamps.get(node_name).cloned();

            // Get tokens from metrics, divide evenly for multiple executions
            let total_node_tokens = metrics.node_tokens.get(node_name).copied().unwrap_or(0);
            let tokens_per = total_node_tokens / *count as u64;

            // We already have one entry from nodes_executed, add count-1 more
            for i in 1..*count {
                node_executions.push(NodeExecution {
                    node: node_name.clone(),
                    duration_ms: duration_per,
                    tokens_used: tokens_per,
                    state_before: None,
                    state_after: None,
                    tools_called: Vec::new(),
                    success: true,
                    error_message: None,
                    index: base_index + i,
                    started_at: started_at.clone(),
                    metadata: std::collections::HashMap::new(),
                });
            }
        }
    }

    // Serialize final state if possible
    let final_state_json = serde_json::to_value(&result.final_state).ok();

    let hierarchy = super::execution_hierarchy::current_ids();
    let (execution_id, parent_execution_id, root_execution_id, depth) = hierarchy
        .map(|ids| {
            (
                ids.execution_id,
                ids.parent_execution_id,
                ids.root_execution_id,
                ids.depth,
            )
        })
        .unwrap_or_else(|| (uuid::Uuid::new_v4().to_string(), None, None, 0));

    ExecutionTrace {
        thread_id,
        execution_id: Some(execution_id),
        // Hierarchical execution IDs (Observability Phase 3)
        parent_execution_id,
        root_execution_id,
        depth: Some(depth),
        nodes_executed: node_executions,
        total_duration_ms: metrics.total_duration.as_millis() as u64,
        total_tokens: metrics.total_tokens,
        errors: Vec::new(),
        completed: result.interrupted_at.is_none(),
        started_at: Some(started_at_str),
        ended_at: Some(ended_at_str),
        final_state: final_state_json,
        metadata: {
            let mut m = std::collections::HashMap::new();
            if let Some(name) = graph_name {
                m.insert(
                    "graph_name".to_string(),
                    serde_json::Value::String(name.to_string()),
                );
            }
            m.insert(
                "checkpoint_count".to_string(),
                serde_json::Value::Number(metrics.checkpoint_count.into()),
            );
            m.insert(
                "edges_traversed".to_string(),
                serde_json::Value::Number(metrics.edges_traversed.into()),
            );
            m.insert(
                "parallel_executions".to_string(),
                serde_json::Value::Number(metrics.parallel_executions.into()),
            );
            m
        },
        // Include rich execution metrics for self-improvement
        execution_metrics: Some(metrics.clone()),
        // Performance metrics are not automatically collected yet
        // This field enables future integration with PerformanceHistory
        performance_metrics: None,
    }
}

pub(crate) fn persist_trace_in_dir(trace: &ExecutionTrace, base_dir: &std::path::Path) {
    use crate::self_improvement::{RedactionConfig, SensitiveDataRedactor};
    use std::fs;
    use std::io::Write;

    let traces_dir = base_dir.join(".dashflow/traces");

    // Create directory if needed
    if let Err(e) = fs::create_dir_all(&traces_dir) {
        // M-656: Log at WARN so trace failures are visible in production
        tracing::warn!(error = %e, "Failed to create traces directory");
        return;
    }

    // Generate filename from execution ID or timestamp
    let filename = trace
        .execution_id
        .as_ref()
        .map(|id| format!("{id}.json"))
        .unwrap_or_else(|| {
            let now = chrono::Utc::now();
            format!("trace_{}.json", now.format("%Y%m%d_%H%M%S_%3f"))
        });

    let path = traces_dir.join(filename);

    // Apply redaction for PII hygiene (M-222)
    let trace_to_write = if is_trace_redaction_enabled() {
        let redactor = SensitiveDataRedactor::new(RedactionConfig::default());
        let mut redacted_trace = trace.clone();
        redactor.redact_execution_trace(&mut redacted_trace);
        redacted_trace
    } else {
        trace.clone()
    };

    // Serialize and write to project-local .dashflow/traces/
    match serde_json::to_string_pretty(&trace_to_write) {
        Ok(json) => {
            if let Err(e) = fs::File::create(&path).and_then(|mut f| f.write_all(json.as_bytes())) {
                // M-656: Log at WARN so trace failures are visible in production
                tracing::warn!(error = %e, path = %path.display(), "Failed to write trace");
            } else {
                tracing::debug!(
                    path = %path.display(),
                    redacted = is_trace_redaction_enabled(),
                    "Execution trace saved"
                );
            }
        }
        Err(e) => {
            // M-656: Log at WARN so trace failures are visible in production
            tracing::warn!(error = %e, "Failed to serialize trace");
        }
    }

    // FIX-008: Also write to user-global WAL for unified querying
    // This enables LearningCorpus to query traces across projects
    // PERF-002 FIX: Use global singleton instead of creating new EventStore per write.
    // Previously, creating EventStore per invocation caused ~100ms overhead due to SQLite setup.
    if let Some(store) = crate::wal::global_event_store() {
        if let Err(e) = store.write_trace(&trace_to_write) {
            // WAL write is best-effort - log but don't fail
            tracing::warn!(error = %e, "Failed to write trace to WAL");
        } else {
            tracing::debug!(
                execution_id = ?trace.execution_id,
                "Execution trace written to WAL"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

    // Mutex to serialize env-var-dependent tests (parallel execution causes races)
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // ============================================================================
    // is_trace_persistence_enabled Tests
    // ============================================================================

    #[test]
    fn trace_persistence_enabled_by_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Clear env var to test default behavior
        std::env::remove_var("DASHFLOW_TRACE");
        assert!(is_trace_persistence_enabled());
    }

    #[test]
    fn trace_persistence_disabled_with_false() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE", "false");
        let result = is_trace_persistence_enabled();
        std::env::remove_var("DASHFLOW_TRACE");
        assert!(!result);
    }

    #[test]
    fn trace_persistence_disabled_with_zero() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE", "0");
        let result = is_trace_persistence_enabled();
        std::env::remove_var("DASHFLOW_TRACE");
        assert!(!result);
    }

    #[test]
    fn trace_persistence_enabled_with_true() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE", "true");
        let result = is_trace_persistence_enabled();
        std::env::remove_var("DASHFLOW_TRACE");
        assert!(result);
    }

    #[test]
    fn trace_persistence_enabled_with_one() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE", "1");
        let result = is_trace_persistence_enabled();
        std::env::remove_var("DASHFLOW_TRACE");
        assert!(result);
    }

    #[test]
    fn trace_persistence_case_insensitive_false() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE", "FALSE");
        let result = is_trace_persistence_enabled();
        std::env::remove_var("DASHFLOW_TRACE");
        assert!(!result);
    }

    #[test]
    fn trace_persistence_case_insensitive_false_mixed() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE", "FaLsE");
        let result = is_trace_persistence_enabled();
        std::env::remove_var("DASHFLOW_TRACE");
        assert!(!result);
    }

    // ============================================================================
    // is_trace_redaction_enabled Tests
    // ============================================================================

    #[test]
    fn trace_redaction_enabled_by_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("DASHFLOW_TRACE_REDACT");
        assert!(is_trace_redaction_enabled());
    }

    #[test]
    fn trace_redaction_disabled_with_false() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE_REDACT", "false");
        let result = is_trace_redaction_enabled();
        std::env::remove_var("DASHFLOW_TRACE_REDACT");
        assert!(!result);
    }

    #[test]
    fn trace_redaction_disabled_with_zero() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE_REDACT", "0");
        let result = is_trace_redaction_enabled();
        std::env::remove_var("DASHFLOW_TRACE_REDACT");
        assert!(!result);
    }

    #[test]
    fn trace_redaction_enabled_with_true() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_TRACE_REDACT", "true");
        let result = is_trace_redaction_enabled();
        std::env::remove_var("DASHFLOW_TRACE_REDACT");
        assert!(result);
    }

    // ============================================================================
    // is_live_introspection_enabled Tests
    // ============================================================================

    #[test]
    fn live_introspection_enabled_by_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("DASHFLOW_LIVE_INTROSPECTION");
        assert!(is_live_introspection_enabled());
    }

    #[test]
    fn live_introspection_disabled_with_false() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_LIVE_INTROSPECTION", "false");
        let result = is_live_introspection_enabled();
        std::env::remove_var("DASHFLOW_LIVE_INTROSPECTION");
        assert!(!result);
    }

    #[test]
    fn live_introspection_disabled_with_zero() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_LIVE_INTROSPECTION", "0");
        let result = is_live_introspection_enabled();
        std::env::remove_var("DASHFLOW_LIVE_INTROSPECTION");
        assert!(!result);
    }

    #[test]
    fn live_introspection_enabled_with_true() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_LIVE_INTROSPECTION", "true");
        let result = is_live_introspection_enabled();
        std::env::remove_var("DASHFLOW_LIVE_INTROSPECTION");
        assert!(result);
    }

    // ============================================================================
    // build_execution_trace Tests
    // ============================================================================

    fn create_test_execution_result() -> ExecutionResult<serde_json::Value> {
        ExecutionResult {
            final_state: serde_json::json!({"result": "success"}),
            nodes_executed: vec!["node_a".to_string(), "node_b".to_string()],
            interrupted_at: None,
            next_nodes: vec![],
        }
    }

    fn create_test_metrics() -> ExecutionMetrics {
        let mut metrics = ExecutionMetrics::default();
        metrics.total_duration = Duration::from_millis(1000);
        metrics.total_tokens = 500;
        metrics.checkpoint_count = 2;
        metrics.edges_traversed = 1;
        metrics.parallel_executions = 0;
        metrics
            .node_durations
            .insert("node_a".to_string(), Duration::from_millis(400));
        metrics
            .node_durations
            .insert("node_b".to_string(), Duration::from_millis(600));
        metrics.node_tokens.insert("node_a".to_string(), 200);
        metrics.node_tokens.insert("node_b".to_string(), 300);
        metrics
    }

    #[test]
    fn build_execution_trace_basic_fields() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, Some("test_graph"), started_at, None);

        assert_eq!(trace.total_duration_ms, 1000);
        assert_eq!(trace.total_tokens, 500);
        assert!(trace.completed);
    }

    #[test]
    fn build_execution_trace_with_thread_id() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(
            &result,
            &metrics,
            Some("test_graph"),
            started_at,
            Some("thread-123".to_string()),
        );

        assert_eq!(trace.thread_id, Some("thread-123".to_string()));
    }

    #[test]
    fn build_execution_trace_execution_id_generated() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert!(trace.execution_id.is_some());
        let id = trace.execution_id.unwrap();
        assert!(!id.is_empty());
        // Should be a valid UUID format (36 chars with hyphens)
        assert_eq!(id.len(), 36);
    }

    #[test]
    fn build_execution_trace_nodes_executed() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert_eq!(trace.nodes_executed.len(), 2);
        assert_eq!(trace.nodes_executed[0].node, "node_a");
        assert_eq!(trace.nodes_executed[1].node, "node_b");
    }

    #[test]
    fn build_execution_trace_node_durations() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert_eq!(trace.nodes_executed[0].duration_ms, 400);
        assert_eq!(trace.nodes_executed[1].duration_ms, 600);
    }

    #[test]
    fn build_execution_trace_node_tokens() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert_eq!(trace.nodes_executed[0].tokens_used, 200);
        assert_eq!(trace.nodes_executed[1].tokens_used, 300);
    }

    #[test]
    fn build_execution_trace_node_indices() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert_eq!(trace.nodes_executed[0].index, 0);
        assert_eq!(trace.nodes_executed[1].index, 1);
    }

    #[test]
    fn build_execution_trace_timestamps() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert!(trace.started_at.is_some());
        assert!(trace.ended_at.is_some());
        // Should be RFC3339 format
        let started = trace.started_at.unwrap();
        assert!(started.contains("T")); // ISO 8601 format
    }

    #[test]
    fn build_execution_trace_metadata_graph_name() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace =
            build_execution_trace(&result, &metrics, Some("my_graph"), started_at, None);

        let graph_name = trace.metadata.get("graph_name").unwrap();
        assert_eq!(graph_name.as_str().unwrap(), "my_graph");
    }

    #[test]
    fn build_execution_trace_metadata_checkpoint_count() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        let checkpoint_count = trace.metadata.get("checkpoint_count").unwrap();
        assert_eq!(checkpoint_count.as_u64().unwrap(), 2);
    }

    #[test]
    fn build_execution_trace_metadata_edges_traversed() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        let edges = trace.metadata.get("edges_traversed").unwrap();
        assert_eq!(edges.as_u64().unwrap(), 1);
    }

    #[test]
    fn build_execution_trace_metadata_parallel_executions() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        let parallel = trace.metadata.get("parallel_executions").unwrap();
        assert_eq!(parallel.as_u64().unwrap(), 0);
    }

    #[test]
    fn build_execution_trace_final_state_serialized() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        let final_state = trace.final_state.unwrap();
        assert_eq!(final_state.get("result").unwrap(), "success");
    }

    #[test]
    fn build_execution_trace_interrupted() {
        let mut result = create_test_execution_result();
        result.interrupted_at = Some("node_b".to_string());
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert!(!trace.completed);
    }

    #[test]
    fn build_execution_trace_no_graph_name() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert!(!trace.metadata.contains_key("graph_name"));
    }

    #[test]
    fn build_execution_trace_execution_metrics_included() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert!(trace.execution_metrics.is_some());
    }

    #[test]
    fn build_execution_trace_performance_metrics_none() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        // Performance metrics are not automatically collected
        assert!(trace.performance_metrics.is_none());
    }

    #[test]
    fn build_execution_trace_empty_nodes() {
        let result = ExecutionResult {
            final_state: serde_json::json!({}),
            nodes_executed: vec![],
            interrupted_at: None,
            next_nodes: vec![],
        };
        let metrics = ExecutionMetrics::default();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert!(trace.nodes_executed.is_empty());
        assert!(trace.completed);
    }

    #[test]
    fn build_execution_trace_node_multiple_executions() {
        let result = ExecutionResult {
            final_state: serde_json::json!({}),
            nodes_executed: vec!["loop_node".to_string()],
            interrupted_at: None,
            next_nodes: vec![],
        };
        let mut metrics = ExecutionMetrics::default();
        metrics
            .node_execution_counts
            .insert("loop_node".to_string(), 3);
        metrics
            .node_durations
            .insert("loop_node".to_string(), Duration::from_millis(300));
        metrics.node_tokens.insert("loop_node".to_string(), 150);
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        // Should have 1 + (3-1) = 3 executions
        assert_eq!(trace.nodes_executed.len(), 3);
        // Duration should be split evenly
        assert_eq!(trace.nodes_executed[1].duration_ms, 100);
        // Tokens should be split evenly
        assert_eq!(trace.nodes_executed[1].tokens_used, 50);
    }

    #[test]
    fn build_execution_trace_missing_duration() {
        let result = ExecutionResult {
            final_state: serde_json::json!({}),
            nodes_executed: vec!["node_without_duration".to_string()],
            interrupted_at: None,
            next_nodes: vec![],
        };
        let metrics = ExecutionMetrics::default();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        // Should default to 0 when duration not found
        assert_eq!(trace.nodes_executed[0].duration_ms, 0);
    }

    #[test]
    fn build_execution_trace_missing_tokens() {
        let result = ExecutionResult {
            final_state: serde_json::json!({}),
            nodes_executed: vec!["node_without_tokens".to_string()],
            interrupted_at: None,
            next_nodes: vec![],
        };
        let metrics = ExecutionMetrics::default();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        // Should default to 0 when tokens not found
        assert_eq!(trace.nodes_executed[0].tokens_used, 0);
    }

    #[test]
    fn build_execution_trace_node_success_default() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        // All nodes should have success=true by default
        for node in &trace.nodes_executed {
            assert!(node.success);
            assert!(node.error_message.is_none());
        }
    }

    #[test]
    fn build_execution_trace_errors_empty() {
        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();

        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        assert!(trace.errors.is_empty());
    }

    // ============================================================================
    // persist_trace Tests (integration)
    // These tests change global state (env vars) and must not run in parallel
    // with other tests. They use temp directories with absolute paths to avoid
    // working directory race conditions.
    // ============================================================================

    #[test]
    #[ignore = "changes env vars - not safe for parallel execution"]
    fn persist_trace_creates_directory() {
        use tempfile::tempdir;

        // Lock ENV_MUTEX to prevent race conditions with other env-var tests
        let _guard = ENV_MUTEX.lock().unwrap();

        let temp = tempdir().unwrap();

        // Disable redaction for test
        std::env::set_var("DASHFLOW_TRACE_REDACT", "false");

        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();
        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        // Use temp.path() directly instead of changing cwd (avoids race conditions)
        persist_trace_in_dir(&trace, temp.path());

        // Check directory was created
        let traces_dir = temp.path().join(".dashflow/traces");
        assert!(traces_dir.exists());

        // Cleanup
        std::env::remove_var("DASHFLOW_TRACE_REDACT");
    }

    #[test]
    #[ignore = "changes env vars - not safe for parallel execution"]
    fn persist_trace_writes_json_file() {
        use std::fs;
        use tempfile::tempdir;

        // Lock ENV_MUTEX to prevent race conditions with other env-var tests
        let _guard = ENV_MUTEX.lock().unwrap();

        let temp = tempdir().unwrap();
        std::env::set_var("DASHFLOW_TRACE_REDACT", "false");

        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();
        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        // Use temp.path() directly instead of changing cwd (avoids race conditions)
        persist_trace_in_dir(&trace, temp.path());

        // Check a JSON file was written
        let traces_dir = temp.path().join(".dashflow/traces");
        let files: Vec<_> = fs::read_dir(&traces_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1);
        assert!(files[0].path().extension().unwrap() == "json");

        std::env::remove_var("DASHFLOW_TRACE_REDACT");
    }

    #[test]
    #[ignore = "changes env vars - not safe for parallel execution"]
    fn persist_trace_file_content_valid_json() {
        use std::fs;
        use tempfile::tempdir;

        // Lock ENV_MUTEX to prevent race conditions with other env-var tests
        let _guard = ENV_MUTEX.lock().unwrap();

        let temp = tempdir().unwrap();
        std::env::set_var("DASHFLOW_TRACE_REDACT", "false");

        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();
        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        // Use temp.path() directly instead of changing cwd (avoids race conditions)
        persist_trace_in_dir(&trace, temp.path());

        // Read and parse the file
        let traces_dir = temp.path().join(".dashflow/traces");
        let file_path = fs::read_dir(&traces_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let content = fs::read_to_string(&file_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert!(parsed.get("total_duration_ms").is_some());
        assert!(parsed.get("nodes_executed").is_some());

        std::env::remove_var("DASHFLOW_TRACE_REDACT");
    }

    #[test]
    #[ignore = "changes env vars - not safe for parallel execution"]
    fn persist_trace_uses_execution_id_as_filename() {
        use tempfile::tempdir;

        // Lock ENV_MUTEX to prevent race conditions with other env-var tests
        let _guard = ENV_MUTEX.lock().unwrap();

        let temp = tempdir().unwrap();
        std::env::set_var("DASHFLOW_TRACE_REDACT", "false");

        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();
        let trace = build_execution_trace(&result, &metrics, None, started_at, None);
        let execution_id = trace.execution_id.clone().unwrap();

        // Use temp.path() directly instead of changing cwd (avoids race conditions)
        persist_trace_in_dir(&trace, temp.path());

        let traces_dir = temp.path().join(".dashflow/traces");
        let expected_file = traces_dir.join(format!("{}.json", execution_id));
        assert!(expected_file.exists());

        std::env::remove_var("DASHFLOW_TRACE_REDACT");
    }

    // ============================================================================
    // FIX-008: WAL Integration Tests
    // ============================================================================

    #[test]
    #[ignore = "changes env vars and creates directories - not safe for parallel execution"]
    fn persist_trace_writes_to_wal_when_enabled() {
        use crate::wal::{EventStore, EventStoreConfig, WALWriterConfig};
        use std::fs;
        use tempfile::tempdir;

        // Lock ENV_MUTEX to prevent race conditions with other env-var tests
        let _guard = ENV_MUTEX.lock().unwrap();

        let temp = tempdir().unwrap();
        let traces_dir = temp.path().join(".dashflow/traces");
        let wal_dir = temp.path().join("wal");
        let index_path = temp.path().join("index.db");
        fs::create_dir_all(&wal_dir).unwrap();

        // Enable WAL and point to temp directory
        std::env::set_var("DASHFLOW_WAL", "true");
        std::env::set_var("DASHFLOW_WAL_DIR", wal_dir.to_string_lossy().to_string());
        std::env::set_var("DASHFLOW_INDEX_PATH", index_path.to_string_lossy().to_string());
        std::env::set_var("DASHFLOW_TRACE_REDACT", "false");
        std::env::set_var("DASHFLOW_WAL_AUTO_COMPACTION", "false"); // Faster tests

        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();
        let trace = build_execution_trace(&result, &metrics, None, started_at, None);
        let execution_id = trace.execution_id.clone().unwrap();

        // Persist trace to temp directory
        persist_trace_in_dir(&trace, temp.path());

        // Verify written to .dashflow/traces/
        let trace_file = traces_dir.join(format!("{}.json", execution_id));
        assert!(trace_file.exists(), "Trace file should exist in .dashflow/traces/");

        // Verify written to WAL
        let wal_files: Vec<_> = fs::read_dir(&wal_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();
        assert!(!wal_files.is_empty(), "WAL file should be created");

        // Verify the trace is queryable from EventStore
        let wal_config = WALWriterConfig {
            wal_dir: wal_dir.clone(),
            max_segment_bytes: 10 * 1024 * 1024,
            fsync_on_write: false,
            segment_extension: ".wal".to_string(),
        };
        let config = EventStoreConfig {
            wal: wal_config,
            index_path,
            auto_compaction: false,
        };
        let store = EventStore::new(config).unwrap();
        let recent = store.recent_executions(10).unwrap();
        assert!(
            recent.iter().any(|e| e.execution_id == execution_id),
            "Execution should be queryable from EventStore"
        );

        // Cleanup env vars
        std::env::remove_var("DASHFLOW_WAL");
        std::env::remove_var("DASHFLOW_WAL_DIR");
        std::env::remove_var("DASHFLOW_INDEX_PATH");
        std::env::remove_var("DASHFLOW_TRACE_REDACT");
        std::env::remove_var("DASHFLOW_WAL_AUTO_COMPACTION");
    }

    #[test]
    #[ignore = "changes env vars - not safe for parallel execution"]
    fn persist_trace_skips_wal_when_disabled() {
        use std::fs;
        use tempfile::tempdir;

        // Lock ENV_MUTEX to prevent race conditions with other env-var tests
        let _guard = ENV_MUTEX.lock().unwrap();

        let temp = tempdir().unwrap();
        let wal_dir = temp.path().join("wal");
        fs::create_dir_all(&wal_dir).unwrap();

        // Disable WAL
        std::env::set_var("DASHFLOW_WAL", "false");
        std::env::set_var("DASHFLOW_WAL_DIR", wal_dir.to_string_lossy().to_string());
        std::env::set_var("DASHFLOW_TRACE_REDACT", "false");

        let result = create_test_execution_result();
        let metrics = create_test_metrics();
        let started_at = SystemTime::now();
        let trace = build_execution_trace(&result, &metrics, None, started_at, None);

        persist_trace_in_dir(&trace, temp.path());

        // Verify NOT written to WAL
        let wal_files: Vec<_> = fs::read_dir(&wal_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();
        assert!(wal_files.is_empty(), "WAL file should NOT be created when WAL is disabled");

        // Cleanup env vars
        std::env::remove_var("DASHFLOW_WAL");
        std::env::remove_var("DASHFLOW_WAL_DIR");
        std::env::remove_var("DASHFLOW_TRACE_REDACT");
    }
}
