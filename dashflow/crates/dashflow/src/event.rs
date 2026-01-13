// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for graph events
// - needless_pass_by_value: Event fields passed by value for ownership semantics
// - clone_on_ref_ptr: Event handlers clone Arc references for async operations
#![allow(clippy::needless_pass_by_value, clippy::clone_on_ref_ptr)]

//! Graph execution events
//!
//! Events provide visibility into graph execution for monitoring,
//! debugging, and custom callbacks.

use std::time::{Duration, SystemTime};

use crate::introspection::{GraphManifest, NodeConfig, OptimizationTrace};
use crate::state::GraphState;

/// Types of events emitted during graph execution
#[derive(Debug, Clone)]
pub enum GraphEvent<S>
where
    S: GraphState,
{
    /// Graph execution started
    GraphStart {
        /// Starting timestamp
        timestamp: SystemTime,
        /// Initial state
        initial_state: S,
        /// Optional graph manifest for telemetry (Manifest Telemetry)
        ///
        /// When present, this provides the complete graph structure to telemetry
        /// consumers, enabling AI agents and observability tools to understand
        /// the graph topology being executed.
        ///
        /// Boxed to reduce enum variant size difference (clippy::large_enum_variant).
        manifest: Option<Box<GraphManifest>>,
    },
    /// Graph execution completed
    GraphEnd {
        /// Completion timestamp
        timestamp: SystemTime,
        /// Final state
        final_state: S,
        /// Total execution duration
        duration: Duration,
        /// Path of nodes executed
        execution_path: Vec<String>,
    },
    /// Node execution started
    NodeStart {
        /// Event timestamp
        timestamp: SystemTime,
        /// Node name
        node: String,
        /// State before node execution
        state: S,
        /// Node configuration with version and hash (Config Versioning)
        ///
        /// When present, telemetry can track which config version was used for this execution,
        /// enabling A/B testing analysis and config change correlation.
        node_config: Option<NodeConfig>,
    },
    /// Node execution completed successfully
    NodeEnd {
        /// Event timestamp
        timestamp: SystemTime,
        /// Node name
        node: String,
        /// State after node execution
        state: S,
        /// Node execution duration
        duration: Duration,
        /// Node configuration with version and hash (Config Versioning)
        ///
        /// Included on NodeEnd as well for correlation with start event.
        node_config: Option<NodeConfig>,
    },
    /// Node execution failed
    NodeError {
        /// Event timestamp
        timestamp: SystemTime,
        /// Node name
        node: String,
        /// Error message
        error: String,
        /// State at time of error
        state: S,
    },
    /// Edge traversal
    EdgeTraversal {
        /// Event timestamp
        timestamp: SystemTime,
        /// Source node
        from: String,
        /// Target node(s)
        to: Vec<String>,
        /// Edge type (simple, conditional, parallel)
        edge_type: EdgeType,
    },
    /// Edge condition evaluated (Observability Phase 3)
    ///
    /// Emitted when a conditional edge is evaluated, providing visibility
    /// into why a particular path was chosen over alternatives.
    /// Answers "Why did the graph take this path?"
    EdgeEvaluated {
        /// Event timestamp
        timestamp: SystemTime,
        /// Source node name
        from_node: String,
        /// Selected target node name
        to_node: String,
        /// Human-readable condition expression (if available)
        condition_expression: Option<String>,
        /// Whether this edge was selected
        evaluation_result: bool,
        /// Alternative routes that were not taken
        alternatives: Vec<EdgeAlternative>,
    },
    /// Parallel execution started
    ParallelStart {
        /// Event timestamp
        timestamp: SystemTime,
        /// Nodes executing in parallel
        nodes: Vec<String>,
    },
    /// Parallel execution completed
    ParallelEnd {
        /// Event timestamp
        timestamp: SystemTime,
        /// Nodes that executed
        nodes: Vec<String>,
        /// Total parallel execution duration
        duration: Duration,
    },
    /// Optimization started (Optimization Telemetry)
    ///
    /// Emitted when an optimization run begins, enabling meta-learning
    /// analysis of optimization efficiency.
    OptimizationStart {
        /// Event timestamp
        timestamp: SystemTime,
        /// Unique optimization ID
        optimization_id: String,
        /// Target node being optimized
        target_node: String,
        /// Target parameter (e.g., "temperature", "system_prompt")
        target_param: String,
        /// Optimization strategy being used
        strategy: Option<String>,
    },
    /// Optimization completed (Optimization Telemetry)
    ///
    /// Emitted when an optimization run finishes, containing the full
    /// optimization trace for meta-learning analysis.
    OptimizationEnd {
        /// Event timestamp
        timestamp: SystemTime,
        /// Complete optimization trace with all variant results
        trace: Box<OptimizationTrace>,
    },
    /// State changed after node execution (Observability Phase 3)
    ///
    /// Provides semantic visibility into what changed in the graph state.
    /// Answers "What was the impact of this node execution?"
    StateChanged {
        /// Event timestamp
        timestamp: SystemTime,
        /// Node that caused the change
        node: String,
        /// Human-readable summary (e.g., "2 added, 1 modified")
        summary: String,
        /// Fields that were added
        fields_added: Vec<String>,
        /// Fields that were removed
        fields_removed: Vec<String>,
        /// Fields that were modified (field name only for privacy)
        fields_modified: Vec<String>,
    },
    /// Agent decision made (Observability Phase 4)
    ///
    /// Emitted when an agent makes a strategic decision, capturing the reasoning
    /// and alternatives considered. Enables Learning Corpus to analyze patterns.
    /// Answers "What decisions did the agent make and why?"
    DecisionMade {
        /// Event timestamp
        timestamp: SystemTime,
        /// Unique decision identifier for correlation with outcomes
        decision_id: String,
        /// Node or component that made the decision
        decision_maker: String,
        /// Category of decision (e.g., "routing", "tool_selection", "retry_strategy")
        decision_type: String,
        /// The decision that was chosen
        chosen_option: String,
        /// Alternatives that were considered but not chosen
        alternatives_considered: Vec<DecisionAlternative>,
        /// Confidence score (0.0 to 1.0) if available
        confidence: Option<f64>,
        /// Human-readable reasoning for the decision
        reasoning: Option<String>,
        /// Context that influenced the decision (key-value pairs, not full state)
        context: std::collections::HashMap<String, String>,
    },
    /// Outcome observed for a previous decision (Observability Phase 4)
    ///
    /// Emitted when the outcome of a decision becomes known, enabling
    /// the Learning Corpus to correlate decisions with results.
    /// Answers "Did the decision lead to success or failure?"
    OutcomeObserved {
        /// Event timestamp
        timestamp: SystemTime,
        /// Decision ID this outcome correlates to
        decision_id: String,
        /// Whether the outcome was successful
        success: bool,
        /// Quantitative score if applicable (e.g., quality metric, latency)
        score: Option<f64>,
        /// Human-readable outcome description
        outcome_description: Option<String>,
        /// Time elapsed since the decision was made
        latency_ms: Option<u64>,
        /// Metrics captured at outcome time
        metrics: std::collections::HashMap<String, f64>,
    },
}

/// Type of edge traversed
#[derive(Debug, Clone)]
pub enum EdgeType {
    /// Simple edge from one node to another
    Simple,
    /// Conditional edge (dynamic routing)
    Conditional {
        /// Condition evaluation result
        condition_result: String,
    },
    /// Parallel edge (multiple nodes)
    Parallel,
}

/// An alternative edge route that was not taken (Observability Phase 3)
///
/// Provides context about why a particular route was not selected
/// when a conditional edge is evaluated.
#[derive(Debug, Clone)]
pub struct EdgeAlternative {
    /// Target node name for this alternative
    pub to_node: String,
    /// Why this route was not selected (human-readable)
    pub reason: Option<String>,
    /// Whether this alternative's condition was evaluated
    pub was_evaluated: bool,
}

/// An alternative option that was considered but not chosen (Observability Phase 4)
///
/// Provides context about what alternatives were evaluated when making a decision.
#[derive(Debug, Clone)]
pub struct DecisionAlternative {
    /// The alternative option that was considered
    pub option: String,
    /// Why this option was not chosen (human-readable)
    pub reason: Option<String>,
    /// Score or weight assigned to this alternative (if applicable)
    pub score: Option<f64>,
    /// Whether this alternative was fully evaluated or filtered early
    pub was_fully_evaluated: bool,
}

/// Callback trait for handling graph events
///
/// Implement this trait to receive notifications about graph execution.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::event::{EventCallback, GraphEvent};
///
/// struct LoggingCallback;
///
/// impl<S: GraphState> EventCallback<S> for LoggingCallback {
///     fn on_event(&self, event: &GraphEvent<S>) {
///         match event {
///             GraphEvent::NodeStart { node, .. } => {
///                 println!("Starting node: {}", node);
///             }
///             GraphEvent::NodeEnd { node, duration, .. } => {
///                 println!("Completed node: {} in {:?}", node, duration);
///             }
///             _ => {}
///         }
///     }
/// }
/// ```
pub trait EventCallback<S>: Send + Sync
where
    S: GraphState,
{
    /// Called when an event occurs during graph execution
    ///
    /// # Arguments
    ///
    /// * `event` - The graph event that occurred
    fn on_event(&self, event: &GraphEvent<S>);

    /// Get producer for intra-node streaming (NEW)
    ///
    /// Return Some(producer) if this callback provides streaming capabilities.
    /// Used by executor to create NodeContext.
    #[cfg(feature = "dashstream")]
    fn get_producer(
        &self,
    ) -> Option<std::sync::Arc<dashflow_streaming::producer::DashStreamProducer>> {
        None
    }

    /// Get thread and tenant IDs (NEW)
    ///
    /// Return (thread_id, tenant_id) if available.
    /// Used for NodeContext message headers.
    #[cfg(feature = "dashstream")]
    fn get_ids(&self) -> Option<(String, String)> {
        None
    }
}

/// A simple event callback that prints events to stdout
pub struct PrintCallback;

impl<S> EventCallback<S> for PrintCallback
where
    S: GraphState,
{
    fn on_event(&self, event: &GraphEvent<S>) {
        match event {
            GraphEvent::GraphStart { .. } => {
                println!("🚀 Graph execution started");
            }
            GraphEvent::GraphEnd {
                duration,
                execution_path,
                ..
            } => {
                println!("✅ Graph execution completed in {duration:?}");
                println!("   Execution path: {}", execution_path.join(" -> "));
            }
            GraphEvent::NodeStart { node, .. } => {
                println!("  ▶️  Starting node: {node}");
            }
            GraphEvent::NodeEnd { node, duration, .. } => {
                println!("  ✔️  Completed node: {node} ({duration:?})");
            }
            GraphEvent::NodeError { node, error, .. } => {
                println!("  ❌ Node failed: {node} - {error}");
            }
            GraphEvent::EdgeTraversal {
                from,
                to,
                edge_type,
                ..
            } => {
                let edge_desc = match edge_type {
                    EdgeType::Simple => "→".to_string(),
                    EdgeType::Conditional { condition_result } => {
                        format!("→ [{condition_result}]")
                    }
                    EdgeType::Parallel => "⇉".to_string(),
                };
                println!("     {} {} {}", from, edge_desc, to.join(", "));
            }
            GraphEvent::ParallelStart { nodes, .. } => {
                println!("  ⚡ Parallel execution: {}", nodes.join(", "));
            }
            GraphEvent::ParallelEnd {
                nodes, duration, ..
            } => {
                println!(
                    "  ⚡ Parallel completed: {} ({:?})",
                    nodes.join(", "),
                    duration
                );
            }
            GraphEvent::OptimizationStart {
                optimization_id,
                target_node,
                target_param,
                ..
            } => {
                println!(
                    "  🔧 Optimization started: {} on {}.{}",
                    optimization_id, target_node, target_param
                );
            }
            GraphEvent::OptimizationEnd { trace, .. } => {
                println!(
                    "  🔧 Optimization completed: {} - {}",
                    trace.optimization_id,
                    if trace.found_improvement() {
                        format!("improved by {:.1}%", trace.improvement_delta * 100.0)
                    } else {
                        "no improvement".to_string()
                    }
                );
            }
            GraphEvent::EdgeEvaluated {
                from_node,
                to_node,
                condition_expression,
                evaluation_result,
                alternatives,
                ..
            } => {
                let expr = condition_expression
                    .as_deref()
                    .unwrap_or("(condition)");
                let result = if *evaluation_result { "matched" } else { "not matched" };
                println!("     {from_node} → {to_node} [{expr} = {result}]");
                if !alternatives.is_empty() {
                    for alt in alternatives {
                        let reason = alt.reason.as_deref().unwrap_or("not selected");
                        println!("        ↳ {}: {reason}", alt.to_node);
                    }
                }
            }
            GraphEvent::StateChanged {
                node,
                summary,
                fields_added,
                fields_removed,
                fields_modified,
                ..
            } => {
                println!("     📊 State changed after {node}: {summary}");
                if !fields_added.is_empty() {
                    println!("        + Added: {}", fields_added.join(", "));
                }
                if !fields_removed.is_empty() {
                    println!("        - Removed: {}", fields_removed.join(", "));
                }
                if !fields_modified.is_empty() {
                    println!("        ~ Modified: {}", fields_modified.join(", "));
                }
            }
            GraphEvent::DecisionMade {
                decision_id,
                decision_maker,
                decision_type,
                chosen_option,
                alternatives_considered,
                confidence,
                reasoning,
                ..
            } => {
                let conf_str = confidence
                    .map(|c| format!(" ({:.0}% confidence)", c * 100.0))
                    .unwrap_or_default();
                println!(
                    "  🎯 Decision [{decision_type}] by {decision_maker}: {chosen_option}{conf_str}"
                );
                println!("     ID: {decision_id}");
                if let Some(reason) = reasoning {
                    println!("     Reasoning: {reason}");
                }
                if !alternatives_considered.is_empty() {
                    println!(
                        "     Alternatives: {}",
                        alternatives_considered
                            .iter()
                            .map(|a| a.option.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
            GraphEvent::OutcomeObserved {
                decision_id,
                success,
                score,
                outcome_description,
                latency_ms,
                ..
            } => {
                let status = if *success { "✅ SUCCESS" } else { "❌ FAILURE" };
                let score_str = score
                    .map(|s| format!(" (score: {s:.2})"))
                    .unwrap_or_default();
                let latency_str = latency_ms
                    .map(|l| format!(" in {l}ms"))
                    .unwrap_or_default();
                println!("  📈 Outcome for {decision_id}: {status}{score_str}{latency_str}");
                if let Some(desc) = outcome_description {
                    println!("     {desc}");
                }
            }
        }
    }
}

/// A callback that collects events for later inspection
pub struct CollectingCallback<S>
where
    S: GraphState,
{
    events: std::sync::Arc<std::sync::Mutex<Vec<GraphEvent<S>>>>,
}

impl<S> CollectingCallback<S>
where
    S: GraphState,
{
    /// Create a new collecting callback
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Get all collected events
    #[must_use]
    pub fn events(&self) -> Vec<GraphEvent<S>> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Get the number of events collected
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Clear all collected events
    pub fn clear(&self) {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// Create a clone that shares the same event storage
    ///
    /// This is useful for passing to graph callbacks while retaining
    /// access to events for inspection after execution.
    #[must_use]
    pub fn shared_clone(&self) -> Self {
        Self {
            events: self.events.clone(),
        }
    }
}

impl<S> Default for CollectingCallback<S>
where
    S: GraphState,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> EventCallback<S> for CollectingCallback<S>
where
    S: GraphState,
{
    fn on_event(&self, event: &GraphEvent<S>) {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(event.clone());
    }
}

/// Tracer event passed to `FnTracer` callbacks
///
/// A simplified event for tracing node execution, designed for the common
/// use case of logging/debugging node transitions.
#[derive(Debug, Clone)]
pub enum TracerEvent<'a, S> {
    /// Node execution is about to start
    NodeStart {
        /// Name of the node
        node: &'a str,
        /// State before node execution
        state: &'a S,
    },
    /// Node execution completed successfully
    NodeEnd {
        /// Name of the node
        node: &'a str,
        /// State after node execution
        state: &'a S,
        /// Duration of node execution
        duration: Duration,
    },
    /// Node execution failed
    NodeError {
        /// Name of the node
        node: &'a str,
        /// Error message
        error: &'a str,
        /// State at time of error
        state: &'a S,
    },
}

/// A simple closure-based tracer for node execution
///
/// `FnTracer` provides a convenient way to add tracing without implementing
/// the full `EventCallback` trait. It only receives node-level events
/// (`NodeStart`, `NodeEnd`, `NodeError`) which covers most debugging use cases.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::FnTracer;
///
/// let app = graph.compile()?
///     .with_tracer(|event| {
///         match event {
///             TracerEvent::NodeStart { node, .. } => {
///                 println!("Starting: {}", node);
///             }
///             TracerEvent::NodeEnd { node, duration, .. } => {
///                 println!("Completed: {} in {:?}", node, duration);
///             }
///             TracerEvent::NodeError { node, error, .. } => {
///                 eprintln!("Failed: {} - {}", node, error);
///             }
///         }
///     });
/// ```
pub struct FnTracer<F> {
    callback: F,
}

impl<F> FnTracer<F> {
    /// Create a new function-based tracer
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<S, F> EventCallback<S> for FnTracer<F>
where
    S: GraphState,
    F: Fn(TracerEvent<'_, S>) + Send + Sync,
{
    fn on_event(&self, event: &GraphEvent<S>) {
        match event {
            GraphEvent::NodeStart { node, state, .. } => {
                (self.callback)(TracerEvent::NodeStart { node, state });
            }
            GraphEvent::NodeEnd {
                node,
                state,
                duration,
                ..
            } => {
                (self.callback)(TracerEvent::NodeEnd {
                    node,
                    state,
                    duration: *duration,
                });
            }
            GraphEvent::NodeError {
                node, error, state, ..
            } => {
                (self.callback)(TracerEvent::NodeError { node, error, state });
            }
            // Ignore graph-level and edge events for the simple tracer
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AgentState;

    #[test]
    fn test_print_callback() {
        let callback = PrintCallback;
        let state = AgentState::new();

        // Should not panic when printing events
        callback.on_event(&GraphEvent::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: state.clone(),
            manifest: None,
        });

        callback.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "test_node".to_string(),
            state: state.clone(),
            node_config: None,
        });

        callback.on_event(&GraphEvent::NodeEnd {
            timestamp: SystemTime::now(),
            node: "test_node".to_string(),
            state: state.clone(),
            duration: Duration::from_millis(100),
            node_config: None,
        });
    }

    #[test]
    fn test_collecting_callback() {
        let callback = CollectingCallback::<AgentState>::new();
        let state = AgentState::new();

        assert_eq!(callback.event_count(), 0);

        callback.on_event(&GraphEvent::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: state.clone(),
            manifest: None,
        });

        assert_eq!(callback.event_count(), 1);

        callback.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "test_node".to_string(),
            state: state.clone(),
            node_config: None,
        });

        assert_eq!(callback.event_count(), 2);

        let events = callback.events();
        assert_eq!(events.len(), 2);

        callback.clear();
        assert_eq!(callback.event_count(), 0);
    }

    // ===== GraphEvent Variant Tests =====

    #[test]
    fn test_graph_event_graph_start() {
        let state = AgentState::new();
        let timestamp = SystemTime::now();

        let event = GraphEvent::GraphStart {
            timestamp,
            initial_state: state.clone(),
            manifest: None,
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_graph_start_with_manifest() {
        use crate::introspection::{EdgeManifest, GraphManifest, NodeManifest, NodeType};

        let state = AgentState::new();
        let timestamp = SystemTime::now();

        // Create a simple manifest for testing
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .graph_name("test_graph")
            .add_node("start", NodeManifest::new("start", NodeType::Function))
            .add_node("end", NodeManifest::new("end", NodeType::Function))
            .add_edge("start", EdgeManifest::simple("start", "end"))
            .build()
            .unwrap();

        let event = GraphEvent::GraphStart {
            timestamp,
            initial_state: state.clone(),
            manifest: Some(Box::new(manifest)),
        };

        // Verify event can be created and cloned
        let cloned = event.clone();

        // Verify manifest is preserved in clone
        if let GraphEvent::GraphStart {
            manifest: Some(m), ..
        } = cloned
        {
            assert_eq!(m.entry_point, "start");
            assert_eq!(m.graph_name, Some("test_graph".to_string()));
            assert_eq!(m.nodes.len(), 2);
        } else {
            panic!("Expected GraphStart with manifest");
        }
    }

    #[test]
    fn test_graph_event_graph_end() {
        let state = AgentState::new();
        let timestamp = SystemTime::now();
        let duration = Duration::from_secs(5);
        let execution_path = vec!["node1".to_string(), "node2".to_string()];

        let event = GraphEvent::GraphEnd {
            timestamp,
            final_state: state.clone(),
            duration,
            execution_path: execution_path.clone(),
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_node_start() {
        let state = AgentState::new();
        let timestamp = SystemTime::now();

        let event = GraphEvent::NodeStart {
            timestamp,
            node: "test_node".to_string(),
            state: state.clone(),
            node_config: None,
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_node_end() {
        let state = AgentState::new();
        let timestamp = SystemTime::now();
        let duration = Duration::from_millis(250);

        let event = GraphEvent::NodeEnd {
            timestamp,
            node: "test_node".to_string(),
            state: state.clone(),
            duration,
            node_config: None,
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_node_start_with_config() {
        // Test config versioning in NodeStart events
        let state = AgentState::new();
        let timestamp = SystemTime::now();

        // Create a node config with version and hash
        let config = NodeConfig::new("llm_agent", "llm.chat")
            .with_config(serde_json::json!({"temperature": 0.7, "max_tokens": 1000}))
            .with_updated_by("human");

        let event = GraphEvent::NodeStart {
            timestamp,
            node: "llm_agent".to_string(),
            state: state.clone(),
            node_config: Some(config.clone()),
        };

        // Verify event can be created and cloned
        let cloned = event.clone();

        // Verify config is preserved
        if let GraphEvent::NodeStart {
            node_config: Some(cfg),
            ..
        } = cloned
        {
            assert_eq!(cfg.name, "llm_agent");
            assert_eq!(cfg.node_type, "llm.chat");
            assert_eq!(cfg.version, 1);
            assert!(cfg.config_hash.starts_with("sha256:"));
            assert_eq!(cfg.updated_by, Some("human".to_string()));
            assert_eq!(cfg.temperature(), Some(0.7));
        } else {
            panic!("Expected NodeStart with node_config");
        }
    }

    #[test]
    fn test_graph_event_node_end_with_config() {
        // Test config versioning in NodeEnd events
        let state = AgentState::new();
        let timestamp = SystemTime::now();
        let duration = Duration::from_millis(150);

        let config = NodeConfig::new("researcher", "tool.search")
            .with_config(serde_json::json!({"query_type": "semantic"}));

        let event = GraphEvent::NodeEnd {
            timestamp,
            node: "researcher".to_string(),
            state: state.clone(),
            duration,
            node_config: Some(config),
        };

        // Verify event can be created and cloned
        let cloned = event.clone();

        // Verify config is preserved with duration
        if let GraphEvent::NodeEnd {
            node_config: Some(cfg),
            duration: d,
            ..
        } = cloned
        {
            assert_eq!(cfg.name, "researcher");
            assert_eq!(cfg.version, 1);
            assert_eq!(d.as_millis(), 150);
        } else {
            panic!("Expected NodeEnd with node_config");
        }
    }

    #[test]
    fn test_graph_event_node_error() {
        let state = AgentState::new();
        let timestamp = SystemTime::now();

        let event = GraphEvent::NodeError {
            timestamp,
            node: "failing_node".to_string(),
            error: "Test error message".to_string(),
            state: state.clone(),
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_edge_traversal_simple() {
        let timestamp = SystemTime::now();

        let event = GraphEvent::<AgentState>::EdgeTraversal {
            timestamp,
            from: "node1".to_string(),
            to: vec!["node2".to_string()],
            edge_type: EdgeType::Simple,
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_edge_traversal_conditional() {
        let timestamp = SystemTime::now();

        let event = GraphEvent::<AgentState>::EdgeTraversal {
            timestamp,
            from: "decision_node".to_string(),
            to: vec!["branch_a".to_string()],
            edge_type: EdgeType::Conditional {
                condition_result: "route_a".to_string(),
            },
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_edge_traversal_parallel() {
        let timestamp = SystemTime::now();

        let event = GraphEvent::<AgentState>::EdgeTraversal {
            timestamp,
            from: "fanout_node".to_string(),
            to: vec![
                "worker1".to_string(),
                "worker2".to_string(),
                "worker3".to_string(),
            ],
            edge_type: EdgeType::Parallel,
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_parallel_start() {
        let timestamp = SystemTime::now();
        let nodes = vec![
            "worker1".to_string(),
            "worker2".to_string(),
            "worker3".to_string(),
        ];

        let event = GraphEvent::<AgentState>::ParallelStart {
            timestamp,
            nodes: nodes.clone(),
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    #[test]
    fn test_graph_event_parallel_end() {
        let timestamp = SystemTime::now();
        let nodes = vec!["worker1".to_string(), "worker2".to_string()];
        let duration = Duration::from_secs(2);

        let event = GraphEvent::<AgentState>::ParallelEnd {
            timestamp,
            nodes: nodes.clone(),
            duration,
        };

        // Verify event can be created and cloned
        let _cloned = event.clone();
    }

    // ===== EdgeType Tests =====

    #[test]
    fn test_edge_type_simple() {
        let edge_type = EdgeType::Simple;
        let _cloned = edge_type.clone();
    }

    #[test]
    fn test_edge_type_conditional() {
        let edge_type = EdgeType::Conditional {
            condition_result: "route_b".to_string(),
        };
        let _cloned = edge_type.clone();
    }

    #[test]
    fn test_edge_type_parallel() {
        let edge_type = EdgeType::Parallel;
        let _cloned = edge_type.clone();
    }

    // ===== PrintCallback Tests =====

    #[test]
    fn test_print_callback_graph_start() {
        let callback = PrintCallback;
        let state = AgentState::new();

        // Should not panic
        callback.on_event(&GraphEvent::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: state,
            manifest: None,
        });
    }

    #[test]
    fn test_print_callback_graph_end() {
        let callback = PrintCallback;
        let state = AgentState::new();

        // Should not panic
        callback.on_event(&GraphEvent::GraphEnd {
            timestamp: SystemTime::now(),
            final_state: state,
            duration: Duration::from_secs(1),
            execution_path: vec!["node1".to_string(), "node2".to_string()],
        });
    }

    #[test]
    fn test_print_callback_node_error() {
        let callback = PrintCallback;
        let state = AgentState::new();

        // Should not panic
        callback.on_event(&GraphEvent::NodeError {
            timestamp: SystemTime::now(),
            node: "error_node".to_string(),
            error: "Test error".to_string(),
            state,
        });
    }

    #[test]
    fn test_print_callback_edge_traversal_simple() {
        let callback = PrintCallback;

        // Should not panic
        callback.on_event(&GraphEvent::<AgentState>::EdgeTraversal {
            timestamp: SystemTime::now(),
            from: "node1".to_string(),
            to: vec!["node2".to_string()],
            edge_type: EdgeType::Simple,
        });
    }

    #[test]
    fn test_print_callback_edge_traversal_conditional() {
        let callback = PrintCallback;

        // Should not panic
        callback.on_event(&GraphEvent::<AgentState>::EdgeTraversal {
            timestamp: SystemTime::now(),
            from: "decision".to_string(),
            to: vec!["branch_a".to_string()],
            edge_type: EdgeType::Conditional {
                condition_result: "route_a".to_string(),
            },
        });
    }

    #[test]
    fn test_print_callback_edge_traversal_parallel() {
        let callback = PrintCallback;

        // Should not panic
        callback.on_event(&GraphEvent::<AgentState>::EdgeTraversal {
            timestamp: SystemTime::now(),
            from: "fanout".to_string(),
            to: vec!["worker1".to_string(), "worker2".to_string()],
            edge_type: EdgeType::Parallel,
        });
    }

    #[test]
    fn test_print_callback_parallel_start() {
        let callback = PrintCallback;

        // Should not panic
        callback.on_event(&GraphEvent::<AgentState>::ParallelStart {
            timestamp: SystemTime::now(),
            nodes: vec!["worker1".to_string(), "worker2".to_string()],
        });
    }

    #[test]
    fn test_print_callback_parallel_end() {
        let callback = PrintCallback;

        // Should not panic
        callback.on_event(&GraphEvent::<AgentState>::ParallelEnd {
            timestamp: SystemTime::now(),
            nodes: vec!["worker1".to_string(), "worker2".to_string()],
            duration: Duration::from_millis(500),
        });
    }

    // ===== CollectingCallback Tests =====

    #[test]
    fn test_collecting_callback_new() {
        let callback = CollectingCallback::<AgentState>::new();
        assert_eq!(callback.event_count(), 0);
    }

    #[test]
    fn test_collecting_callback_default() {
        let callback = CollectingCallback::<AgentState>::default();
        assert_eq!(callback.event_count(), 0);
    }

    #[test]
    fn test_collecting_callback_shared_clone() {
        let callback = CollectingCallback::<AgentState>::new();
        let state = AgentState::new();

        // Add event to original
        callback.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "test".to_string(),
            state: state.clone(),
            node_config: None,
        });

        assert_eq!(callback.event_count(), 1);

        // Create shared clone
        let cloned = callback.shared_clone();

        // Both should see the same events
        assert_eq!(cloned.event_count(), 1);

        // Add event to clone
        cloned.on_event(&GraphEvent::NodeEnd {
            timestamp: SystemTime::now(),
            node: "test".to_string(),
            state: state.clone(),
            duration: Duration::from_millis(100),
            node_config: None,
        });

        // Both should see both events (shared storage)
        assert_eq!(callback.event_count(), 2);
        assert_eq!(cloned.event_count(), 2);
    }

    #[test]
    fn test_collecting_callback_events_cloned() {
        let callback = CollectingCallback::<AgentState>::new();
        let state = AgentState::new();

        callback.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "test".to_string(),
            state: state.clone(),
            node_config: None,
        });

        let events1 = callback.events();
        let events2 = callback.events();

        // Each call returns a new clone
        assert_eq!(events1.len(), 1);
        assert_eq!(events2.len(), 1);
    }

    #[test]
    fn test_collecting_callback_clear() {
        let callback = CollectingCallback::<AgentState>::new();
        let state = AgentState::new();

        // Add multiple events
        for i in 0..5 {
            callback.on_event(&GraphEvent::NodeStart {
                timestamp: SystemTime::now(),
                node: format!("node{}", i),
                state: state.clone(),
                node_config: None,
            });
        }

        assert_eq!(callback.event_count(), 5);

        // Clear all events
        callback.clear();
        assert_eq!(callback.event_count(), 0);

        // Events should be empty
        let events = callback.events();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_collecting_callback_collects_all_event_types() {
        let callback = CollectingCallback::<AgentState>::new();
        let state = AgentState::new();

        // Collect one of each event type
        callback.on_event(&GraphEvent::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: state.clone(),
            manifest: None,
        });

        callback.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "node1".to_string(),
            state: state.clone(),
            node_config: None,
        });

        callback.on_event(&GraphEvent::NodeEnd {
            timestamp: SystemTime::now(),
            node: "node1".to_string(),
            state: state.clone(),
            duration: Duration::from_millis(100),
            node_config: None,
        });

        callback.on_event(&GraphEvent::NodeError {
            timestamp: SystemTime::now(),
            node: "node2".to_string(),
            error: "Test error".to_string(),
            state: state.clone(),
        });

        callback.on_event(&GraphEvent::EdgeTraversal {
            timestamp: SystemTime::now(),
            from: "node1".to_string(),
            to: vec!["node2".to_string()],
            edge_type: EdgeType::Simple,
        });

        callback.on_event(&GraphEvent::ParallelStart {
            timestamp: SystemTime::now(),
            nodes: vec!["worker1".to_string(), "worker2".to_string()],
        });

        callback.on_event(&GraphEvent::ParallelEnd {
            timestamp: SystemTime::now(),
            nodes: vec!["worker1".to_string(), "worker2".to_string()],
            duration: Duration::from_secs(1),
        });

        callback.on_event(&GraphEvent::GraphEnd {
            timestamp: SystemTime::now(),
            final_state: state.clone(),
            duration: Duration::from_secs(2),
            execution_path: vec!["node1".to_string(), "node2".to_string()],
        });

        assert_eq!(callback.event_count(), 8);

        let events = callback.events();
        assert_eq!(events.len(), 8);
    }

    #[test]
    fn test_collecting_callback_event_ordering() {
        let callback = CollectingCallback::<AgentState>::new();
        let state = AgentState::new();

        // Add events in specific order
        callback.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "first".to_string(),
            state: state.clone(),
            node_config: None,
        });

        callback.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "second".to_string(),
            state: state.clone(),
            node_config: None,
        });

        callback.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "third".to_string(),
            state: state.clone(),
            node_config: None,
        });

        let events = callback.events();
        assert_eq!(events.len(), 3);

        // Verify ordering is preserved
        assert!(
            matches!(&events[0], GraphEvent::NodeStart { node, .. } if node == "first"),
            "Expected NodeStart event for 'first', got {:?}",
            events[0]
        );

        assert!(
            matches!(&events[1], GraphEvent::NodeStart { node, .. } if node == "second"),
            "Expected NodeStart event for 'second', got {:?}",
            events[1]
        );

        assert!(
            matches!(&events[2], GraphEvent::NodeStart { node, .. } if node == "third"),
            "Expected NodeStart event for 'third', got {:?}",
            events[2]
        );
    }

    // ===== FnTracer Tests =====

    #[test]
    fn test_fn_tracer_node_start() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let tracer = FnTracer::new(move |event: TracerEvent<'_, AgentState>| {
            if matches!(event, TracerEvent::NodeStart { .. }) {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        let state = AgentState::new();
        tracer.on_event(&GraphEvent::NodeStart {
            timestamp: SystemTime::now(),
            node: "test_node".to_string(),
            state: state.clone(),
            node_config: None,
        });

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_fn_tracer_node_end() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let tracer = FnTracer::new(move |event: TracerEvent<'_, AgentState>| {
            if let TracerEvent::NodeEnd { duration, .. } = event {
                assert!(duration.as_millis() >= 100);
                call_count_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        let state = AgentState::new();
        tracer.on_event(&GraphEvent::NodeEnd {
            timestamp: SystemTime::now(),
            node: "test_node".to_string(),
            state: state.clone(),
            duration: Duration::from_millis(150),
            node_config: None,
        });

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_fn_tracer_node_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let tracer = FnTracer::new(move |event: TracerEvent<'_, AgentState>| {
            if let TracerEvent::NodeError { error, .. } = event {
                assert_eq!(error, "test error");
                call_count_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        let state = AgentState::new();
        tracer.on_event(&GraphEvent::NodeError {
            timestamp: SystemTime::now(),
            node: "failing_node".to_string(),
            error: "test error".to_string(),
            state: state.clone(),
        });

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_fn_tracer_ignores_non_node_events() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let tracer = FnTracer::new(move |_event: TracerEvent<'_, AgentState>| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let state = AgentState::new();

        // Graph-level events should be ignored
        tracer.on_event(&GraphEvent::GraphStart {
            timestamp: SystemTime::now(),
            initial_state: state.clone(),
            manifest: None,
        });

        tracer.on_event(&GraphEvent::GraphEnd {
            timestamp: SystemTime::now(),
            final_state: state.clone(),
            duration: Duration::from_secs(1),
            execution_path: vec!["node1".to_string()],
        });

        // Edge events should be ignored
        tracer.on_event(&GraphEvent::EdgeTraversal {
            timestamp: SystemTime::now(),
            from: "node1".to_string(),
            to: vec!["node2".to_string()],
            edge_type: EdgeType::Simple,
        });

        // Parallel events should be ignored
        tracer.on_event(&GraphEvent::ParallelStart {
            timestamp: SystemTime::now(),
            nodes: vec!["worker1".to_string()],
        });

        tracer.on_event(&GraphEvent::ParallelEnd {
            timestamp: SystemTime::now(),
            nodes: vec!["worker1".to_string()],
            duration: Duration::from_millis(100),
        });

        // Should not have been called for any of the above
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_fn_tracer_receives_all_node_events() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let start_count = std::sync::Arc::new(AtomicUsize::new(0));
        let end_count = std::sync::Arc::new(AtomicUsize::new(0));
        let error_count = std::sync::Arc::new(AtomicUsize::new(0));

        let start_clone = start_count.clone();
        let end_clone = end_count.clone();
        let error_clone = error_count.clone();

        let tracer = FnTracer::new(move |event: TracerEvent<'_, AgentState>| match event {
            TracerEvent::NodeStart { .. } => {
                start_clone.fetch_add(1, Ordering::SeqCst);
            }
            TracerEvent::NodeEnd { .. } => {
                end_clone.fetch_add(1, Ordering::SeqCst);
            }
            TracerEvent::NodeError { .. } => {
                error_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        let state = AgentState::new();

        // Send multiple events of each type
        for _ in 0..3 {
            tracer.on_event(&GraphEvent::NodeStart {
                timestamp: SystemTime::now(),
                node: "test".to_string(),
                state: state.clone(),
                node_config: None,
            });
        }

        for _ in 0..2 {
            tracer.on_event(&GraphEvent::NodeEnd {
                timestamp: SystemTime::now(),
                node: "test".to_string(),
                state: state.clone(),
                duration: Duration::from_millis(50),
                node_config: None,
            });
        }

        tracer.on_event(&GraphEvent::NodeError {
            timestamp: SystemTime::now(),
            node: "test".to_string(),
            error: "error".to_string(),
            state: state.clone(),
        });

        assert_eq!(start_count.load(Ordering::SeqCst), 3);
        assert_eq!(end_count.load(Ordering::SeqCst), 2);
        assert_eq!(error_count.load(Ordering::SeqCst), 1);
    }

    // ===== TracerEvent Tests =====

    #[test]
    fn test_tracer_event_node_start_fields() {
        let state = AgentState::new();
        let event = TracerEvent::NodeStart {
            node: "my_node",
            state: &state,
        };

        if let TracerEvent::NodeStart { node, .. } = event {
            assert_eq!(node, "my_node");
        } else {
            panic!("Expected NodeStart");
        }
    }

    #[test]
    fn test_tracer_event_node_end_fields() {
        let state = AgentState::new();
        let duration = Duration::from_millis(250);
        let event = TracerEvent::NodeEnd {
            node: "my_node",
            state: &state,
            duration,
        };

        if let TracerEvent::NodeEnd {
            node, duration: d, ..
        } = event
        {
            assert_eq!(node, "my_node");
            assert_eq!(d.as_millis(), 250);
        } else {
            panic!("Expected NodeEnd");
        }
    }

    #[test]
    fn test_tracer_event_node_error_fields() {
        let state = AgentState::new();
        let event = TracerEvent::NodeError {
            node: "failing_node",
            error: "something went wrong",
            state: &state,
        };

        if let TracerEvent::NodeError { node, error, .. } = event {
            assert_eq!(node, "failing_node");
            assert_eq!(error, "something went wrong");
        } else {
            panic!("Expected NodeError");
        }
    }

    #[test]
    fn test_tracer_event_debug() {
        let state = AgentState::new();
        let event = TracerEvent::NodeStart {
            node: "test",
            state: &state,
        };

        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("NodeStart"));
        assert!(debug_str.contains("test"));
    }

    // ===== DecisionMade and OutcomeObserved Tests (Phase 4) =====

    #[test]
    fn test_decision_alternative() {
        let alt = DecisionAlternative {
            option: "option_b".to_string(),
            reason: Some("lower confidence score".to_string()),
            score: Some(0.6),
            was_fully_evaluated: true,
        };

        assert_eq!(alt.option, "option_b");
        assert_eq!(alt.reason, Some("lower confidence score".to_string()));
        assert_eq!(alt.score, Some(0.6));
        assert!(alt.was_fully_evaluated);

        // Test clone
        let cloned = alt.clone();
        assert_eq!(cloned.option, "option_b");
    }

    #[test]
    fn test_graph_event_decision_made() {
        let _state = AgentState::new();
        let timestamp = SystemTime::now();
        let mut context = std::collections::HashMap::new();
        context.insert("task_type".to_string(), "code_generation".to_string());

        let alternatives = vec![
            DecisionAlternative {
                option: "template_approach".to_string(),
                reason: Some("too rigid for complex cases".to_string()),
                score: Some(0.4),
                was_fully_evaluated: true,
            },
            DecisionAlternative {
                option: "rule_based".to_string(),
                reason: Some("insufficient flexibility".to_string()),
                score: Some(0.3),
                was_fully_evaluated: false,
            },
        ];

        let event: GraphEvent<AgentState> = GraphEvent::DecisionMade {
            timestamp,
            decision_id: "dec-001".to_string(),
            decision_maker: "planner_node".to_string(),
            decision_type: "strategy_selection".to_string(),
            chosen_option: "llm_guided_generation".to_string(),
            alternatives_considered: alternatives,
            confidence: Some(0.85),
            reasoning: Some("LLM approach provides best flexibility for novel cases".to_string()),
            context,
        };

        // Verify event can be created and cloned
        let cloned = event.clone();

        if let GraphEvent::DecisionMade {
            decision_id,
            decision_maker,
            decision_type,
            chosen_option,
            alternatives_considered,
            confidence,
            reasoning,
            ..
        } = cloned
        {
            assert_eq!(decision_id, "dec-001");
            assert_eq!(decision_maker, "planner_node");
            assert_eq!(decision_type, "strategy_selection");
            assert_eq!(chosen_option, "llm_guided_generation");
            assert_eq!(alternatives_considered.len(), 2);
            assert_eq!(confidence, Some(0.85));
            assert!(reasoning.is_some());
        } else {
            panic!("Expected DecisionMade event");
        }
    }

    #[test]
    fn test_graph_event_outcome_observed() {
        let _state = AgentState::new();
        let timestamp = SystemTime::now();
        let mut metrics = std::collections::HashMap::new();
        metrics.insert("quality_score".to_string(), 0.92);
        metrics.insert("token_count".to_string(), 547.0);

        let event = GraphEvent::<AgentState>::OutcomeObserved {
            timestamp,
            decision_id: "dec-001".to_string(),
            success: true,
            score: Some(0.92),
            outcome_description: Some("Code generated successfully, all tests pass".to_string()),
            latency_ms: Some(2350),
            metrics,
        };

        // Verify event can be created and cloned
        let cloned = event.clone();

        if let GraphEvent::OutcomeObserved {
            decision_id,
            success,
            score,
            outcome_description,
            latency_ms,
            metrics,
            ..
        } = cloned
        {
            assert_eq!(decision_id, "dec-001");
            assert!(success);
            assert_eq!(score, Some(0.92));
            assert!(outcome_description.is_some());
            assert_eq!(latency_ms, Some(2350));
            assert!(metrics.contains_key("quality_score"));
        } else {
            panic!("Expected OutcomeObserved event");
        }
    }

    #[test]
    fn test_graph_event_outcome_observed_failure() {
        let _state = AgentState::new();
        let timestamp = SystemTime::now();

        let event = GraphEvent::<AgentState>::OutcomeObserved {
            timestamp,
            decision_id: "dec-002".to_string(),
            success: false,
            score: Some(0.15),
            outcome_description: Some("Generated code failed compilation".to_string()),
            latency_ms: Some(1500),
            metrics: std::collections::HashMap::new(),
        };

        if let GraphEvent::OutcomeObserved { success, score, .. } = event {
            assert!(!success);
            assert_eq!(score, Some(0.15));
        } else {
            panic!("Expected OutcomeObserved event");
        }
    }

    #[test]
    fn test_print_callback_decision_made() {
        let callback = PrintCallback;
        let mut context = std::collections::HashMap::new();
        context.insert("input_length".to_string(), "1500".to_string());

        // Should not panic
        callback.on_event(&GraphEvent::<AgentState>::DecisionMade {
            timestamp: SystemTime::now(),
            decision_id: "test-dec".to_string(),
            decision_maker: "agent".to_string(),
            decision_type: "tool_selection".to_string(),
            chosen_option: "search_tool".to_string(),
            alternatives_considered: vec![DecisionAlternative {
                option: "code_tool".to_string(),
                reason: Some("not relevant".to_string()),
                score: Some(0.2),
                was_fully_evaluated: true,
            }],
            confidence: Some(0.9),
            reasoning: Some("Query requires web search".to_string()),
            context,
        });
    }

    #[test]
    fn test_print_callback_outcome_observed() {
        let callback = PrintCallback;

        // Should not panic
        callback.on_event(&GraphEvent::<AgentState>::OutcomeObserved {
            timestamp: SystemTime::now(),
            decision_id: "test-dec".to_string(),
            success: true,
            score: Some(0.95),
            outcome_description: Some("Task completed successfully".to_string()),
            latency_ms: Some(500),
            metrics: std::collections::HashMap::new(),
        });
    }

    #[test]
    fn test_collecting_callback_collects_phase4_events() {
        let callback = CollectingCallback::<AgentState>::new();

        // Add Phase 4 events
        callback.on_event(&GraphEvent::DecisionMade {
            timestamp: SystemTime::now(),
            decision_id: "dec-1".to_string(),
            decision_maker: "agent".to_string(),
            decision_type: "routing".to_string(),
            chosen_option: "path_a".to_string(),
            alternatives_considered: vec![],
            confidence: None,
            reasoning: None,
            context: std::collections::HashMap::new(),
        });

        callback.on_event(&GraphEvent::OutcomeObserved {
            timestamp: SystemTime::now(),
            decision_id: "dec-1".to_string(),
            success: true,
            score: None,
            outcome_description: None,
            latency_ms: None,
            metrics: std::collections::HashMap::new(),
        });

        assert_eq!(callback.event_count(), 2);
    }
}
