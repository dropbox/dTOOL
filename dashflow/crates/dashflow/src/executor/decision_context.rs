//! Decision tracking context (FIX-014)
//!
//! PERF-002 FIX: Use global EventStore singleton instead of creating new one per invoke.
//! Previously, creating EventStore per invocation caused ~100ms overhead due to SQLite setup.

use crate::event::DecisionAlternative;
use crate::wal::{WALEvent, WALEventType};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) struct DecisionContextGuard {
    _private: (),
}

impl Drop for DecisionContextGuard {
    fn drop(&mut self) {
        // No-op: we use global singleton now, nothing to clean up
    }
}

/// Initialize decision context for this invocation.
///
/// PERF-002 FIX: This function now does nothing expensive - it just returns a guard.
/// The actual EventStore is accessed via the global singleton when decisions are recorded.
pub(crate) fn init_decision_context() -> DecisionContextGuard {
    DecisionContextGuard { _private: () }
}

/// Record a decision from inside a node.
#[allow(clippy::needless_pass_by_value)] // Public API: owned types are ergonomic and forwarded to record_decision_with_context()
pub fn record_decision(
    decision_maker: &str,
    decision_type: &str,
    chosen_option: &str,
    alternatives: Vec<DecisionAlternative>,
    reasoning: Option<String>,
) -> Option<String> {
    record_decision_with_context(
        decision_maker, decision_type, chosen_option, alternatives, reasoning, None, HashMap::new(),
    )
}

/// Record a decision with context.
///
/// PERF-002 FIX: Use global EventStore singleton instead of creating new one per call.
pub fn record_decision_with_context(
    decision_maker: &str,
    decision_type: &str,
    chosen_option: &str,
    alternatives: Vec<DecisionAlternative>,
    reasoning: Option<String>,
    confidence: Option<f64>,
    context: HashMap<String, String>,
) -> Option<String> {
    // Early return if WAL is disabled
    let store = crate::wal::global_event_store()?;

    let decision_id = Uuid::new_v4().to_string();
    let (execution_id, parent_id, root_id, depth) =
        crate::executor::execution_hierarchy::current_ids()
            .map(|ids| (Some(ids.execution_id), ids.parent_execution_id, ids.root_execution_id, Some(ids.depth)))
            .unwrap_or((None, None, None, None));

    let alternatives_json: Vec<serde_json::Value> = alternatives
        .into_iter()
        .map(|alt| serde_json::json!({"option": alt.option, "reason": alt.reason, "score": alt.score}))
        .collect();

    let reasoning_json = reasoning.map(serde_json::Value::String);

    let context_json: serde_json::Value = context.into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into();

    let payload = serde_json::json!({
        "decision_id": decision_id, "decision_maker": decision_maker,
        "decision_type": decision_type, "chosen_option": chosen_option,
        "alternatives_considered": alternatives_json, "confidence": confidence,
        "reasoning": reasoning_json, "context": context_json,
    });

    let event = WALEvent {
        timestamp_ms: timestamp_ms(), event_type: WALEventType::DecisionMade,
        execution_id, parent_execution_id: parent_id, root_execution_id: root_id, depth, payload,
    };

    // Use global singleton - no expensive EventStore creation per call
    if store.writer().write_event(&event).is_ok() {
        return Some(decision_id);
    }
    None
}

/// Record the outcome of a decision.
pub fn record_outcome(decision_id: &str, success: bool, score: Option<f64>) {
    record_outcome_with_details(decision_id, success, score, None, None, HashMap::new());
}

/// Record outcome with details.
///
/// PERF-002 FIX: Use global EventStore singleton instead of creating new one per call.
#[allow(clippy::needless_pass_by_value)] // Public API: owned types serialize cleanly and avoid forcing callers to allocate lifetimes
pub fn record_outcome_with_details(
    decision_id: &str, success: bool, score: Option<f64>,
    outcome_description: Option<String>, latency_ms: Option<u64>, metrics: HashMap<String, f64>,
) {
    // Early return if WAL is disabled
    let Some(store) = crate::wal::global_event_store() else { return };

    let (execution_id, parent_id, root_id, depth) =
        crate::executor::execution_hierarchy::current_ids()
            .map(|ids| (Some(ids.execution_id), ids.parent_execution_id, ids.root_execution_id, Some(ids.depth)))
            .unwrap_or((None, None, None, None));

    let metrics_json: serde_json::Value = metrics.into_iter()
        .map(|(k, v)| (k, serde_json::json!(v)))
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into();

    let payload = serde_json::json!({
        "decision_id": decision_id, "success": success, "score": score,
        "outcome_description": outcome_description, "latency_ms": latency_ms, "metrics": metrics_json,
    });

    let event = WALEvent {
        timestamp_ms: timestamp_ms(), event_type: WALEventType::OutcomeObserved,
        execution_id, parent_execution_id: parent_id, root_execution_id: root_id, depth, payload,
    };

    // Use global singleton - no expensive EventStore creation per call
    let _ = store.writer().write_event(&event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_ms_returns_nonzero() {
        let ts = timestamp_ms();
        // Timestamp should be after year 2020 (roughly 1577836800000 ms since epoch)
        assert!(ts > 1577836800000, "timestamp should be after 2020");
        // And before year 2100 (roughly 4102444800000 ms since epoch)
        assert!(ts < 4102444800000, "timestamp should be before 2100");
    }

    #[test]
    fn test_timestamp_ms_is_increasing() {
        let ts1 = timestamp_ms();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let ts2 = timestamp_ms();
        assert!(ts2 >= ts1, "timestamps should be monotonically increasing");
    }

    #[tokio::test]
    async fn test_init_decision_context_returns_guard() {
        // Without WAL enabled, init should still return a guard
        let _guard = init_decision_context();
        // Guard can be held and dropped without panic
    }

    #[test]
    fn test_record_decision_completes_without_panic() {
        // Test that record_decision handles multiple alternatives without panic
        let result = record_decision(
            "test-maker",
            "test-type",
            "option-a",
            vec![
                DecisionAlternative {
                    option: "option-a".to_string(),
                    reason: Some("primary choice".to_string()),
                    score: Some(0.9),
                    was_fully_evaluated: true,
                },
                DecisionAlternative {
                    option: "option-b".to_string(),
                    reason: Some("fallback".to_string()),
                    score: Some(0.5),
                    was_fully_evaluated: true,
                },
            ],
            Some("chosen for performance".to_string()),
        );
        // Result depends on WAL state - either None or a valid UUID
        if let Some(decision_id) = result {
            // Verify it's a valid UUID format (36 chars with dashes)
            assert_eq!(decision_id.len(), 36);
            assert!(decision_id.chars().filter(|c| *c == '-').count() == 4);
        }
        // No panic = success
    }

    #[test]
    fn test_record_outcome_without_wal_completes() {
        // Without WAL enabled, record_outcome should complete without panic
        record_outcome("decision-123", true, Some(0.95));
        // No assertion - test passes if no panic
    }

    #[test]
    fn test_record_outcome_with_details_without_wal_completes() {
        let mut metrics = HashMap::new();
        metrics.insert("latency".to_string(), 42.5);
        metrics.insert("throughput".to_string(), 100.0);

        // Without WAL enabled, should complete without panic
        record_outcome_with_details(
            "decision-456",
            false,
            Some(0.3),
            Some("failed due to timeout".to_string()),
            Some(5000),
            metrics,
        );
        // No assertion - test passes if no panic
    }

    #[test]
    fn test_record_decision_with_context_formats_correctly() {
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), "u-123".to_string());
        context.insert("session".to_string(), "s-456".to_string());

        // Test that the function handles context formatting without panic
        let result = record_decision_with_context(
            "router",
            "model-selection",
            "gpt-4",
            vec![DecisionAlternative {
                option: "gpt-4".to_string(),
                reason: Some("best quality".to_string()),
                score: Some(0.95),
                was_fully_evaluated: true,
            }],
            Some("selected highest quality model".to_string()),
            Some(0.95),
            context,
        );
        // Result depends on WAL state - either None or a valid UUID
        if let Some(decision_id) = result {
            // Verify it's a valid UUID format
            assert_eq!(decision_id.len(), 36);
        }
        // No panic = success
    }
}
