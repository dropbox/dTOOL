// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! WAL Event Callback
//!
//! Provides an `EventCallback` implementation that persists GraphEvents to the WAL
//! for historical queries and Learning Corpus analysis.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use super::writer::{WALEvent, WALEventType, WALWriter, WALWriterError};
use crate::event::{EdgeType, EventCallback, GraphEvent};
use crate::state::GraphState;

/// EventCallback that persists GraphEvents to the WAL for historical queries.
///
/// This callback bridges the gap between in-memory graph execution events and
/// the persistent WAL storage, enabling the Learning Corpus to analyze execution
/// patterns over time.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::wal::WALEventCallback;
///
/// let app = graph.compile()?
///     .with_callback(WALEventCallback::from_env()?);
/// app.invoke(state).await?;  // Events automatically persisted to WAL
/// ```
///
/// # Execution ID Tracking
///
/// The callback generates a unique execution ID on each `GraphStart` event and
/// associates all subsequent events with that execution until `GraphEnd`.
pub struct WALEventCallback {
    writer: WALWriter,
    /// Fallback execution IDs for tests/manual event emission.
    ///
    /// In normal execution, IDs come from the executor's task-local hierarchy context.
    current_execution: Mutex<Option<ExecutionIds>>,
}

#[derive(Clone, Debug)]
struct ExecutionIds {
    execution_id: String,
    parent_execution_id: Option<String>,
    root_execution_id: Option<String>,
    depth: u32,
}

impl WALEventCallback {
    /// Create a new WAL event callback with the given writer.
    pub fn new(writer: WALWriter) -> Self {
        Self {
            writer,
            current_execution: Mutex::new(None),
        }
    }

    /// Create a WAL event callback from environment configuration.
    ///
    /// Uses `DASHFLOW_WAL_DIR` and other env vars for configuration.
    pub fn from_env() -> Result<Self, WALWriterError> {
        Ok(Self::new(WALWriter::from_env()?))
    }

    /// Get the current execution ID, if any.
    #[cfg(test)]
    fn get_execution_id(&self) -> Option<String> {
        if let Some((execution_id, _, _, _)) = crate::executor::current_execution_hierarchy_ids() {
            return Some(execution_id);
        }

        self.current_execution
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|ids| ids.execution_id.clone()))
    }

    fn set_execution_ids(&self, ids: Option<ExecutionIds>) {
        if let Ok(mut guard) = self.current_execution.lock() {
            *guard = ids;
        }
    }

    fn resolve_execution_ids(&self) -> Option<ExecutionIds> {
        if let Some((execution_id, parent_execution_id, root_execution_id, depth)) =
            crate::executor::current_execution_hierarchy_ids()
        {
            return Some(ExecutionIds {
                execution_id,
                parent_execution_id,
                root_execution_id,
                depth,
            });
        }

        self.current_execution
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Write a WAL event, logging but not failing on errors.
    fn write_event(&self, event: &WALEvent) {
        if let Err(e) = self.writer.write_event(event) {
            tracing::warn!("WAL write failed: {}", e);
        }
    }
}

impl<S> EventCallback<S> for WALEventCallback
where
    S: GraphState,
{
    fn on_event(&self, event: &GraphEvent<S>) {
        let wal_event = match event {
            GraphEvent::GraphStart { timestamp, .. } => {
                let ids = self.resolve_execution_ids().unwrap_or_else(|| ExecutionIds {
                    execution_id: uuid::Uuid::new_v4().to_string(),
                    parent_execution_id: None,
                    root_execution_id: None,
                    depth: 0,
                });
                self.set_execution_ids(Some(ids.clone()));

                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::ExecutionStart,
                    execution_id: Some(ids.execution_id),
                    parent_execution_id: ids.parent_execution_id,
                    root_execution_id: ids.root_execution_id,
                    depth: Some(ids.depth),
                    payload: json!({}),
                }
            }

            GraphEvent::GraphEnd {
                timestamp,
                duration,
                execution_path,
                ..
            } => {
                let ids = self.resolve_execution_ids();
                // Clear fallback IDs for next execution
                self.set_execution_ids(None);

                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::ExecutionEnd,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "duration_ms": duration.as_millis() as u64,
                        "execution_path": execution_path,
                    }),
                }
            }

            GraphEvent::NodeStart { timestamp, node, .. } => {
                let ids = self.resolve_execution_ids();
                WALEvent {
                    // Keep best-effort for manual emission; in normal execution, IDs come from context.
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::NodeStart,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "node": node,
                    }),
                }
            }

            GraphEvent::NodeEnd {
                timestamp,
                node,
                duration,
                ..
            } => {
                let ids = self.resolve_execution_ids();
                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::NodeEnd,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "node": node,
                        "duration_ms": duration.as_millis() as u64,
                    }),
                }
            }

            GraphEvent::NodeError {
                timestamp,
                node,
                error,
                ..
            } => {
                let ids = self.resolve_execution_ids();
                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::NodeError,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "node": node,
                        "error": error,
                    }),
                }
            }

            GraphEvent::EdgeTraversal {
                timestamp,
                from,
                to,
                edge_type,
            } => {
                let ids = self.resolve_execution_ids();
                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::EdgeTraversal,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "from": from,
                        "to": to,
                        "edge_type": edge_type_to_string(edge_type),
                    }),
                }
            }

            GraphEvent::EdgeEvaluated {
                timestamp,
                from_node,
                to_node,
                condition_expression,
                evaluation_result,
                alternatives,
            } => {
                let ids = self.resolve_execution_ids();
                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::EdgeEvaluated,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "from_node": from_node,
                        "to_node": to_node,
                        "condition_expression": condition_expression,
                        "evaluation_result": evaluation_result,
                        "alternatives_count": alternatives.len(),
                    }),
                }
            }

            GraphEvent::StateChanged {
                timestamp,
                node,
                summary,
                fields_added,
                fields_removed,
                fields_modified,
            } => {
                let ids = self.resolve_execution_ids();
                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::StateChanged,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "node": node,
                        "summary": summary,
                        "fields_added": fields_added,
                        "fields_removed": fields_removed,
                        "fields_modified": fields_modified,
                    }),
                }
            }

            GraphEvent::DecisionMade {
                timestamp,
                decision_id,
                decision_maker,
                decision_type,
                chosen_option,
                alternatives_considered,
                confidence,
                reasoning,
                context,
            } => {
                let ids = self.resolve_execution_ids();
                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::DecisionMade,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "decision_id": decision_id,
                        "decision_maker": decision_maker,
                        "decision_type": decision_type,
                        "chosen_option": chosen_option,
                        "alternatives_count": alternatives_considered.len(),
                        "confidence": confidence,
                        "reasoning": reasoning,
                        "context": context,
                    }),
                }
            }

            GraphEvent::OutcomeObserved {
                timestamp,
                decision_id,
                success,
                score,
                outcome_description,
                latency_ms,
                metrics,
            } => {
                let ids = self.resolve_execution_ids();
                WALEvent {
                    timestamp_ms: system_time_to_ms(timestamp),
                    event_type: WALEventType::OutcomeObserved,
                    execution_id: ids.as_ref().map(|ids| ids.execution_id.clone()),
                    parent_execution_id: ids.as_ref().and_then(|ids| ids.parent_execution_id.clone()),
                    root_execution_id: ids.as_ref().and_then(|ids| ids.root_execution_id.clone()),
                    depth: ids.as_ref().map(|ids| ids.depth),
                    payload: json!({
                        "decision_id": decision_id,
                        "success": success,
                        "score": score,
                        "outcome_description": outcome_description,
                        "latency_ms": latency_ms,
                        "metrics": metrics,
                    }),
                }
            }

            // Parallel events - not persisted to WAL (could add later if needed)
            GraphEvent::ParallelStart { .. } | GraphEvent::ParallelEnd { .. } => return,

            // Optimization events - not persisted to WAL (separate telemetry system)
            GraphEvent::OptimizationStart { .. } | GraphEvent::OptimizationEnd { .. } => return,
        };

        self.write_event(&wal_event);
    }
}

/// Convert SystemTime to milliseconds since epoch.
fn system_time_to_ms(time: &SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Convert EdgeType to string for JSON serialization.
fn edge_type_to_string(edge_type: &EdgeType) -> String {
    match edge_type {
        EdgeType::Simple => "simple".to_string(),
        EdgeType::Conditional { condition_result } => {
            format!("conditional:{}", condition_result)
        }
        EdgeType::Parallel => "parallel".to_string(),
    }
}

// ============================================================================
// WALTelemetrySink - TelemetrySink implementation for WAL
// ============================================================================

/// A [`crate::telemetry::TelemetrySink`] implementation that writes telemetry events to the WAL.
///
/// This sink enables LLM call events (and other telemetry events) to be persisted
/// to the Write-Ahead Log alongside graph execution events. This provides:
///
/// - **Durable storage**: Events survive process restarts
/// - **Self-improvement**: LLM calls can be replayed for learning/optimization
/// - **Unified telemetry**: All events flow through the same WAL infrastructure
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::wal::WALTelemetrySink;
/// use dashflow::telemetry::{TelemetrySink, TelemetryEvent};
///
/// let sink = WALTelemetrySink::from_env()?;
/// sink.record_event(TelemetryEvent::LlmCallCompleted {
///     model: "gpt-4o".to_string(),
///     provider: "openai".to_string(),
///     messages: None,
///     response: Some("Hello!".to_string()),
///     error: None,
///     duration_ms: 150,
///     input_tokens: Some(10),
///     output_tokens: Some(5),
/// });
/// ```
///
/// # Introspection
///
/// Use `dashflow introspect search WALTelemetrySink` to find this sink.
/// Use `dashflow introspect search TelemetrySink` to find all sink implementations.
pub struct WALTelemetrySink {
    writer: WALWriter,
}

impl WALTelemetrySink {
    /// Create a new WAL telemetry sink with the given writer.
    pub fn new(writer: WALWriter) -> Self {
        Self { writer }
    }

    /// Create a WAL telemetry sink from environment configuration.
    ///
    /// Uses `DASHFLOW_WAL_DIR` and other env vars for configuration.
    pub fn from_env() -> Result<Self, WALWriterError> {
        Ok(Self::new(WALWriter::from_env()?))
    }
}

impl crate::telemetry::TelemetrySink for WALTelemetrySink {
    fn record_event(&self, event: crate::telemetry::TelemetryEvent) {
        use crate::telemetry::TelemetryEvent;

        let wal_event = match event {
            TelemetryEvent::LlmCallCompleted {
                model,
                provider,
                messages,
                response,
                error,
                duration_ms,
                input_tokens,
                output_tokens,
            } => WALEvent {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                event_type: WALEventType::LlmCallCompleted,
                execution_id: None, // LLM calls aren't tied to a specific graph execution
                parent_execution_id: None,
                root_execution_id: None,
                depth: None,
                payload: json!({
                    "model": model,
                    "provider": provider,
                    "messages": messages,
                    "response": response,
                    "error": error,
                    "duration_ms": duration_ms,
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                }),
            },

            // For other event types, convert them to WAL format
            TelemetryEvent::ExecutionStarted { execution_id, graph_name } => WALEvent {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                event_type: WALEventType::ExecutionStart,
                execution_id: Some(execution_id),
                parent_execution_id: None,
                root_execution_id: None,
                depth: Some(0),
                payload: json!({ "graph_name": graph_name }),
            },

            TelemetryEvent::ExecutionCompleted { execution_id, duration_ms } => WALEvent {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                event_type: WALEventType::ExecutionEnd,
                execution_id: Some(execution_id),
                parent_execution_id: None,
                root_execution_id: None,
                depth: None,
                payload: json!({ "duration_ms": duration_ms }),
            },

            TelemetryEvent::ExecutionFailed { execution_id, error } => WALEvent {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                event_type: WALEventType::NodeError,
                execution_id: Some(execution_id),
                parent_execution_id: None,
                root_execution_id: None,
                depth: None,
                payload: json!({ "error": error }),
            },

            TelemetryEvent::NodeStarted { execution_id, node } => WALEvent {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                event_type: WALEventType::NodeStart,
                execution_id: Some(execution_id),
                parent_execution_id: None,
                root_execution_id: None,
                depth: None,
                payload: json!({ "node": node }),
            },

            TelemetryEvent::NodeCompleted { execution_id, node, duration_ms } => WALEvent {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                event_type: WALEventType::NodeEnd,
                execution_id: Some(execution_id),
                parent_execution_id: None,
                root_execution_id: None,
                depth: None,
                payload: json!({ "node": node, "duration_ms": duration_ms }),
            },

            TelemetryEvent::DecisionMade {
                execution_id,
                decision_maker,
                decision_type,
                chosen_option,
            } => WALEvent {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                event_type: WALEventType::DecisionMade,
                execution_id: Some(execution_id),
                parent_execution_id: None,
                root_execution_id: None,
                depth: None,
                payload: json!({
                    "decision_maker": decision_maker,
                    "decision_type": decision_type,
                    "chosen_option": chosen_option,
                }),
            },

            TelemetryEvent::Custom { name, payload } => WALEvent {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                event_type: WALEventType::ExecutionTrace,
                execution_id: None,
                parent_execution_id: None,
                root_execution_id: None,
                depth: None,
                payload: json!({ "custom_event": name, "data": payload }),
            },
        };

        // Write to WAL (best effort - don't fail on errors)
        if let Err(e) = self.writer.write_event(&wal_event) {
            tracing::warn!("WAL telemetry write failed: {}", e);
        }
    }

    fn flush(&self) {
        // WAL writer already fsyncs on each write
    }

    fn is_healthy(&self) -> bool {
        let dir = self.writer.wal_dir();
        // Check that WAL directory exists and is accessible
        match std::fs::metadata(dir) {
            Ok(meta) => meta.is_dir(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AgentState;
    use crate::telemetry::TelemetrySink;
    use crate::{subgraph::SubgraphNode, StateGraph, END};
    use std::collections::HashMap;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_callback() -> (WALEventCallback, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = super::super::writer::WALWriterConfig {
            wal_dir: temp_dir.path().to_path_buf(),
            max_segment_bytes: 10 * 1024 * 1024,
            fsync_on_write: false, // Faster tests
            segment_extension: ".wal".to_string(),
        };
        let writer = WALWriter::new(config).unwrap();
        (WALEventCallback::new(writer), temp_dir)
    }

    #[test]
    fn test_execution_id_tracking() {
        let (callback, _temp_dir) = create_test_callback();

        // Initially no execution ID
        assert!(callback.get_execution_id().is_none());

        // GraphStart sets execution ID
        callback.on_event(&GraphEvent::<AgentState>::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: AgentState::default(),
            manifest: None,
        });

        let exec_id = callback.get_execution_id();
        assert!(exec_id.is_some());

        // GraphEnd clears execution ID
        callback.on_event(&GraphEvent::<AgentState>::GraphEnd {
            timestamp: SystemTime::now(),
            final_state: AgentState::default(),
            duration: Duration::from_millis(100),
            execution_path: vec!["start".to_string(), "end".to_string()],
        });

        assert!(callback.get_execution_id().is_none());
    }

    #[test]
    fn test_node_events_use_current_execution_id() {
        let (callback, temp_dir) = create_test_callback();

        // Start execution
        callback.on_event(&GraphEvent::<AgentState>::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: AgentState::default(),
            manifest: None,
        });

        let exec_id = callback.get_execution_id().unwrap();
        assert!(!exec_id.is_empty(), "Execution ID should be non-empty");

        // Node events should use the same execution ID
        callback.on_event(&GraphEvent::<AgentState>::NodeStart {
            timestamp: SystemTime::now(),
            node: "test_node".to_string(),
            state: AgentState::default(),
            node_config: None,
        });

        callback.on_event(&GraphEvent::<AgentState>::NodeEnd {
            timestamp: SystemTime::now(),
            node: "test_node".to_string(),
            state: AgentState::default(),
            duration: Duration::from_millis(50),
            node_config: None,
        });

        // Verify events were written to WAL
        let wal_files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();

        assert!(!wal_files.is_empty(), "WAL file should be created");
    }

    #[test]
    fn test_edge_evaluated_persisted() {
        let (callback, temp_dir) = create_test_callback();

        callback.on_event(&GraphEvent::<AgentState>::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: AgentState::default(),
            manifest: None,
        });

        callback.on_event(&GraphEvent::<AgentState>::EdgeEvaluated {
            timestamp: SystemTime::now(),
            from_node: "router".to_string(),
            to_node: "handler_a".to_string(),
            condition_expression: Some("state.route == 'a'".to_string()),
            evaluation_result: true,
            alternatives: vec![],
        });

        // Verify WAL file exists
        let wal_files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();

        assert!(!wal_files.is_empty());
    }

    #[test]
    fn test_state_changed_persisted() {
        let (callback, temp_dir) = create_test_callback();

        callback.on_event(&GraphEvent::<AgentState>::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: AgentState::default(),
            manifest: None,
        });

        callback.on_event(&GraphEvent::<AgentState>::StateChanged {
            timestamp: SystemTime::now(),
            node: "processor".to_string(),
            summary: "2 modified".to_string(),
            fields_added: vec![],
            fields_removed: vec![],
            fields_modified: vec!["messages".to_string(), "context".to_string()],
        });

        let wal_files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();

        assert!(!wal_files.is_empty());
    }

    #[test]
    fn test_decision_made_persisted() {
        let (callback, temp_dir) = create_test_callback();

        callback.on_event(&GraphEvent::<AgentState>::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: AgentState::default(),
            manifest: None,
        });

        callback.on_event(&GraphEvent::<AgentState>::DecisionMade {
            timestamp: SystemTime::now(),
            decision_id: "dec-123".to_string(),
            decision_maker: "router_agent".to_string(),
            decision_type: "routing".to_string(),
            chosen_option: "path_a".to_string(),
            alternatives_considered: vec![],
            confidence: Some(0.95),
            reasoning: Some("High priority task".to_string()),
            context: HashMap::new(),
        });

        let wal_files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();

        assert!(!wal_files.is_empty());
    }

    #[test]
    fn test_outcome_observed_persisted() {
        let (callback, temp_dir) = create_test_callback();

        callback.on_event(&GraphEvent::<AgentState>::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: AgentState::default(),
            manifest: None,
        });

        callback.on_event(&GraphEvent::<AgentState>::OutcomeObserved {
            timestamp: SystemTime::now(),
            decision_id: "dec-123".to_string(),
            success: true,
            score: Some(0.92),
            outcome_description: Some("Task completed successfully".to_string()),
            latency_ms: Some(150),
            metrics: HashMap::new(),
        });

        let wal_files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();

        assert!(!wal_files.is_empty());
    }

    #[tokio::test]
    async fn test_wal_events_include_subgraph_hierarchical_ids() {
        let temp_dir = TempDir::new().unwrap();
        let wal_dir = temp_dir.path().join("wal");

        let parent_execution_id = {
            // Child graph writes to WAL.
            let config = super::super::writer::WALWriterConfig {
                wal_dir: wal_dir.clone(),
                max_segment_bytes: 10 * 1024 * 1024,
                fsync_on_write: true,
                segment_extension: ".wal".to_string(),
            };
            let writer = WALWriter::new(config).unwrap();

            let mut child_graph: StateGraph<AgentState> = StateGraph::new();
            child_graph.add_node_from_fn("child", |state| Box::pin(async move { Ok(state) }));
            child_graph.set_entry_point("child");
            child_graph.add_edge("child", END);
            let compiled_child = child_graph
                .compile()
                .unwrap()
                .with_trace_base_dir(temp_dir.path())
                .with_callback(WALEventCallback::new(writer));

            let subgraph_node = SubgraphNode::new(
                "child_graph",
                compiled_child,
                |parent: &AgentState| parent.clone(),
                |_parent: AgentState, child: AgentState| child,
            );

            let mut parent_graph: StateGraph<AgentState> = StateGraph::new();
            parent_graph.add_node("subgraph", subgraph_node);
            parent_graph.set_entry_point("subgraph");
            parent_graph.add_edge("subgraph", END);
            let compiled_parent = parent_graph
                .compile()
                .unwrap()
                .with_trace_base_dir(temp_dir.path());

            compiled_parent.invoke(AgentState::default()).await.unwrap();

            // Wait for async trace persistence (PERF-003 made this non-blocking)
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let traces_dir = temp_dir.path().join(".dashflow/traces");
            let mut traces = Vec::new();
            for entry in std::fs::read_dir(&traces_dir).unwrap().flatten() {
                let content = std::fs::read_to_string(entry.path()).unwrap();
                let trace: crate::introspection::ExecutionTrace =
                    serde_json::from_str(&content).unwrap();
                traces.push(trace);
            }
            traces
                .iter()
                .find(|t| t.depth == Some(0))
                .and_then(|t| t.execution_id.clone())
                .expect("parent execution trace should exist")
        };

        // Read WAL files after graphs/callbacks have been dropped/flushed.
        let wal_files: Vec<_> = std::fs::read_dir(&wal_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();
        assert!(!wal_files.is_empty(), "WAL file should be created");

        let mut start_events = Vec::new();
        for wal_file in wal_files {
            let content = std::fs::read_to_string(wal_file.path()).unwrap();
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let event: WALEvent = serde_json::from_str(line).unwrap();
                if event.event_type == WALEventType::ExecutionStart {
                    start_events.push(event);
                }
            }
        }

        assert!(!start_events.is_empty(), "should write an ExecutionStart event");
        assert!(
            start_events.iter().any(|e| {
                e.parent_execution_id.as_deref() == Some(parent_execution_id.as_str())
                    && e.root_execution_id.as_deref() == Some(parent_execution_id.as_str())
                    && e.depth == Some(1)
            }),
            "expected a subgraph ExecutionStart event with correct parent/root/depth"
        );

    }

    #[test]
    fn test_telemetry_sink_is_healthy_with_valid_directory() {
        let temp_dir = TempDir::new().unwrap();
        let config = super::super::writer::WALWriterConfig {
            wal_dir: temp_dir.path().to_path_buf(),
            max_segment_bytes: 10 * 1024 * 1024,
            fsync_on_write: false,
            segment_extension: ".wal".to_string(),
        };
        let writer = WALWriter::new(config).unwrap();
        let sink = WALTelemetrySink::new(writer);
        // Directory exists and is valid
        assert!(sink.is_healthy());
    }

    #[test]
    fn test_telemetry_sink_is_healthy_with_deleted_directory() {
        let temp_dir = TempDir::new().unwrap();
        let wal_dir = temp_dir.path().to_path_buf();
        let config = super::super::writer::WALWriterConfig {
            wal_dir: wal_dir.clone(),
            max_segment_bytes: 10 * 1024 * 1024,
            fsync_on_write: false,
            segment_extension: ".wal".to_string(),
        };
        let writer = WALWriter::new(config).unwrap();
        let sink = WALTelemetrySink::new(writer);

        // Initially healthy
        assert!(sink.is_healthy());

        // Delete the directory
        std::fs::remove_dir_all(&wal_dir).unwrap();

        // Now unhealthy since directory is gone
        assert!(!sink.is_healthy());
    }
}
