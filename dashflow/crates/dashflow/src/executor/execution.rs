// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for execution module:
// - clone_on_ref_ptr: Concurrent execution uses Arc cloning extensively for parallel node execution
// - expect_used: Semaphore acquire uses expect() for synchronization (only panics if semaphore is closed)
#![allow(clippy::clone_on_ref_ptr, clippy::expect_used)]

//! Execution methods for CompiledGraph.
//!
//! This module contains the execution-related impl blocks for CompiledGraph
//! that require `MergeableState` bounds.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};

use async_stream::stream;
use futures::stream::Stream;
use tokio::sync::Semaphore;
use tracing::{info_span, Instrument, Span};
use uuid::Uuid;

// Import from parent module
use super::{CompiledGraph, ExecutionResult, DEFAULT_GRAPH_TIMEOUT, DEFAULT_NODE_TIMEOUT};

// Import from sibling trace module
use super::trace::{build_execution_trace, is_trace_persistence_enabled, persist_trace_in_dir};

use crate::edge::{ConditionalEdge, Edge, ParallelEdge, END};
use crate::error::{Error, Result};
use crate::event::{EdgeAlternative, EdgeType, GraphEvent};
use crate::metrics::{ExecutionMetrics, LocalMetricsBatch};
use crate::node::BoxedNode;
use crate::state::GraphState;
use crate::stream::{StreamEvent, StreamMode};

/// Static END node reference to avoid repeated allocations when reaching implicit end.
/// Used in `find_next_nodes` when no explicit edge is found.
static END_NODE: OnceLock<Arc<String>> = OnceLock::new();

/// Get a reference to the static END node (initializes on first call)
fn get_end_node() -> Arc<String> {
    Arc::clone(END_NODE.get_or_init(|| Arc::new(END.to_string())))
}

/// State change details for StateChanged event emission
struct StateChanges {
    fields_added: Vec<String>,
    fields_removed: Vec<String>,
    fields_modified: Vec<String>,
}

impl StateChanges {
    /// Returns true if there were any changes
    fn has_changes(&self) -> bool {
        !self.fields_added.is_empty()
            || !self.fields_removed.is_empty()
            || !self.fields_modified.is_empty()
    }

    /// Generate human-readable summary of changes
    fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.fields_added.is_empty() {
            parts.push(format!("{} added", self.fields_added.len()));
        }
        if !self.fields_removed.is_empty() {
            parts.push(format!("{} removed", self.fields_removed.len()));
        }
        if !self.fields_modified.is_empty() {
            parts.push(format!("{} modified", self.fields_modified.len()));
        }
        if parts.is_empty() {
            "no changes".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Compute state changes between old and new state (Observability Phase 3)
///
/// Compares top-level fields of JSON-serialized states to determine:
/// - Fields that were added (present in new_state but not old_state)
/// - Fields that were removed (present in old_state but not new_state)
/// - Fields that were modified (present in both but with different values)
fn compute_state_changes<S: GraphState>(old_state: &S, new_state: &S) -> Option<StateChanges> {
    // Serialize both states to JSON Value for comparison
    let old_json = match serde_json::to_value(old_state) {
        Ok(v) => v,
        Err(_) => return None,
    };
    let new_json = match serde_json::to_value(new_state) {
        Ok(v) => v,
        Err(_) => return None,
    };

    // Only compare if both are objects (most graph states are structs)
    let (old_obj, new_obj) = match (old_json.as_object(), new_json.as_object()) {
        (Some(old), Some(new)) => (old, new),
        _ => return None,
    };

    let mut fields_added = Vec::new();
    let mut fields_removed = Vec::new();
    let mut fields_modified = Vec::new();

    // Find added and modified fields
    for (key, new_val) in new_obj {
        match old_obj.get(key) {
            None => fields_added.push(key.clone()),
            Some(old_val) if old_val != new_val => fields_modified.push(key.clone()),
            _ => {} // Same value, no change
        }
    }

    // Find removed fields
    for key in old_obj.keys() {
        if !new_obj.contains_key(key) {
            fields_removed.push(key.clone());
        }
    }

    Some(StateChanges {
        fields_added,
        fields_removed,
        fields_modified,
    })
}

/// Next node(s) to execute
pub(super) enum NextNodes {
    /// Single node to execute
    Single(Arc<String>),
    /// Multiple nodes to execute in parallel
    Parallel(Arc<Vec<String>>),
}
/// Execution methods for `CompiledGraph` (requires `MergeableState`)
///
/// These methods are only available when the state type implements `MergeableState`,
/// which provides merge logic for parallel execution. For sequential-only graphs,
/// consider using `invoke_sequential()` which doesn't require merge support.
impl<S> CompiledGraph<S>
where
    S: crate::state::MergeableState,
{
    /// Invoke the graph with initial state
    ///
    /// Executes the graph from the entry point until reaching an END edge
    /// or encountering an error. This is the main execution method for graphs.
    ///
    /// # Arguments
    ///
    /// * `initial_state` - Starting state for the graph
    ///
    /// # Returns
    ///
    /// [`ExecutionResult`] containing the final state and execution metadata.
    /// Use `.state()` to access the final state.
    ///
    /// # Errors
    ///
    /// - [`crate::Error::Timeout`] - Execution exceeded `graph_timeout` (default: 5 minutes)
    /// - [`crate::Error::NodeNotFound`] - An edge references a non-existent node
    /// - [`crate::Error::RecursionLimit`] - Exceeded `recursion_limit` steps (default: 25)
    /// - [`crate::Error::NodeExecution`] - A node function returned an error
    /// - [`crate::Error::Checkpoint`] - Failed to save/restore checkpoint
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = app.invoke(AgentState::new()).await?;
    /// println!("Final state: {:?}", result.state());
    /// println!("Nodes executed: {:?}", result.nodes_executed());
    /// ```
    ///
    /// # See Also
    ///
    /// - [`CompiledGraph::stream`] - Stream events during execution
    /// - [`ExecutionResult`] - Return type with state and metadata
    pub async fn invoke(&self, initial_state: S) -> Result<ExecutionResult<S>> {
        super::execution_hierarchy::with_execution_stack(async {
            let _scope_guard = super::execution_hierarchy::enter_new_execution(
                Uuid::new_v4().to_string(),
            )
            .1;
            // FIX-014: Initialize decision tracking context
            let _decision_guard = super::init_decision_context();

            let started_at = SystemTime::now();

            // Always apply graph timeout (use default if not set)
            let timeout = self.graph_timeout.unwrap_or(DEFAULT_GRAPH_TIMEOUT);
            let result = match tokio::time::timeout(
                timeout,
                self.invoke_internal(initial_state, None, None),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(Error::Timeout(timeout)),
            };

            // Auto-persist trace on successful execution (PERF-003: non-blocking)
            // Opt-out via DASHFLOW_TRACE=false
            if let Ok(ref exec_result) = result {
                if is_trace_persistence_enabled() {
                    // Get metrics for trace (metrics_enabled check is implicit in the lock)
                    let metrics = self.metrics.lock().await.clone();
                    let trace = build_execution_trace(
                        exec_result,
                        &metrics,
                        self.name.as_deref(),
                        started_at,
                        self.thread_id.clone(),
                    );
                    let base_dir = self
                        .trace_base_dir
                        .clone()
                        .unwrap_or_else(|| std::path::PathBuf::from("."));

                    // PERF-003: Spawn trace persistence to background task so graph execution
                    // returns immediately. Previously, synchronous file I/O added ~8ms overhead.
                    // Traces are best-effort; errors are logged but don't block execution.
                    tokio::task::spawn_blocking(move || {
                        persist_trace_in_dir(&trace, &base_dir);
                    });
                }
            }

            result
        })
        .await
    }

    /// Resume execution from the last checkpoint
    ///
    /// This method loads the most recent checkpoint for the configured `thread_id` and
    /// continues execution from the interrupt point. Both a checkpointer and `thread_id`
    /// must be configured via `with_checkpointer()` and `with_thread_id()` for this to work.
    ///
    /// # Returns
    ///
    /// `ExecutionResult` which may include another interrupt or complete normally
    ///
    /// # Errors
    ///
    /// - Returns error if no checkpointer is configured
    /// - Returns error if no `thread_id` is configured
    /// - Returns error if no checkpoint exists for the `thread_id`
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // First invocation - graph interrupts
    /// let result = app.invoke(initial_state).await?;
    /// assert!(result.interrupted_at.is_some());
    ///
    /// // Human reviews and approves...
    ///
    /// // Resume from checkpoint
    /// let final_result = app.resume().await?;
    /// ```
    pub async fn resume(&self) -> Result<ExecutionResult<S>> {
        // Require checkpointer
        let checkpointer = self
            .checkpointer
            .as_ref()
            .ok_or_else(|| Error::ResumeWithoutCheckpointer)?;

        // Require thread_id
        let thread_id = self
            .thread_id
            .as_ref()
            .ok_or_else(|| Error::ResumeWithoutThreadId)?;

        // Load last checkpoint
        let checkpoint = checkpointer
            .get_latest(thread_id)
            .await?
            .ok_or_else(|| Error::NoCheckpointToResume(thread_id.clone()))?;

        // Resume execution from checkpoint state
        // Skip interrupt check for the node we're resuming from to avoid infinite loop
        // (interrupt_before would re-trigger if we don't skip it)
        let skip_node = checkpoint.node.clone();

        // Determine where to start based on whether this was interrupt_before or interrupt_after
        // For interrupt_before: node hasn't executed yet, so start from that node
        // For interrupt_after: node has executed, so start from next nodes based on routing
        let start_nodes = if self.interrupt_before.contains(&checkpoint.node) {
            // interrupt_before: start from the checkpoint node (hasn't executed yet)
            vec![checkpoint.node.clone()]
        } else if self.interrupt_after.contains(&checkpoint.node) {
            // interrupt_after: determine next nodes based on current state and routing
            let (next_nodes_enum, _) =
                self.find_next_nodes_with_type(&checkpoint.node, &checkpoint.state)?;
            match next_nodes_enum {
                NextNodes::Single(next) => {
                    if next.as_str() == END {
                        // No next nodes, execution should complete
                        return Ok(ExecutionResult {
                            final_state: checkpoint.state,
                            nodes_executed: vec![],
                            interrupted_at: None,
                            next_nodes: vec![],
                        });
                    }
                    vec![(*next).clone()]
                }
                NextNodes::Parallel(nexts) => (*nexts).clone(),
            }
        } else {
            // No interrupt configured for this node, default to starting from it
            vec![checkpoint.node.clone()]
        };

        super::execution_hierarchy::with_execution_stack(async {
            let _scope_guard = super::execution_hierarchy::enter_new_execution(
                Uuid::new_v4().to_string(),
            )
            .1;
            // FIX-014: Initialize decision tracking context
            let _decision_guard = super::init_decision_context();

            // Always apply graph timeout (use default if not set)
            let timeout = self.graph_timeout.unwrap_or(DEFAULT_GRAPH_TIMEOUT);
            match tokio::time::timeout(
                timeout,
                self.invoke_internal(checkpoint.state, Some(&skip_node), Some(start_nodes)),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(Error::Timeout(timeout)),
            }
        })
        .await
    }

    /// Get the current state from the latest checkpoint
    ///
    /// Retrieves the graph state saved at the most recent checkpoint for the
    /// configured `thread_id`. This is useful for inspecting state before resuming
    /// or for external systems that need to know the current graph state.
    ///
    /// # Returns
    ///
    /// The state from the latest checkpoint
    ///
    /// # Errors
    ///
    /// - Returns error if no checkpointer is configured
    /// - Returns error if no `thread_id` is configured
    /// - Returns error if no checkpoint exists for the `thread_id`
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // After interrupt
    /// let current_state = app.get_current_state().await?;
    /// println!("Review required: {:?}", current_state.needs_approval);
    /// ```
    pub async fn get_current_state(&self) -> Result<S> {
        // Require checkpointer
        let checkpointer =
            self.checkpointer
                .as_ref()
                .ok_or_else(|| Error::StateOperationWithoutCheckpointer {
                    operation: "get_current_state",
                })?;

        // Require thread_id
        let thread_id =
            self.thread_id
                .as_ref()
                .ok_or_else(|| Error::StateOperationWithoutThreadId {
                    operation: "get_current_state",
                })?;

        // Load last checkpoint
        let checkpoint = checkpointer
            .get_latest(thread_id)
            .await?
            .ok_or_else(|| Error::NoCheckpointToResume(thread_id.clone()))?;

        Ok(checkpoint.state)
    }

    /// Update the state at the current checkpoint
    ///
    /// Loads the latest checkpoint, applies the provided update function to modify
    /// the state, and saves the updated state back to the checkpoint. This allows
    /// external systems to modify graph state before resuming execution.
    ///
    /// # Arguments
    ///
    /// * `update_fn` - Function that takes the current state and returns updated state
    ///
    /// # Errors
    ///
    /// - Returns error if no checkpointer is configured
    /// - Returns error if no `thread_id` is configured
    /// - Returns error if no checkpoint exists for the `thread_id`
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Human approves the draft
    /// app.update_state(|mut state| {
    ///     state.approved = true;
    ///     state.reviewer_comments = Some("Looks good!".to_string());
    ///     state
    /// }).await?;
    ///
    /// // Now resume with updated state
    /// let result = app.resume().await?;
    /// ```
    pub async fn update_state<F>(&self, update_fn: F) -> Result<()>
    where
        F: FnOnce(S) -> S,
    {
        // Require checkpointer
        let checkpointer =
            self.checkpointer
                .as_ref()
                .ok_or_else(|| Error::StateOperationWithoutCheckpointer {
                    operation: "update_state",
                })?;

        // Require thread_id
        let thread_id =
            self.thread_id
                .as_ref()
                .ok_or_else(|| Error::StateOperationWithoutThreadId {
                    operation: "update_state",
                })?;

        // Load current checkpoint
        let mut checkpoint = checkpointer
            .get_latest(thread_id)
            .await?
            .ok_or_else(|| Error::NoCheckpointToResume(thread_id.clone()))?;

        // Apply update function
        checkpoint.state = update_fn(checkpoint.state);

        // Update timestamp to reflect modification
        checkpoint.timestamp = std::time::SystemTime::now();

        // Save updated checkpoint
        checkpointer.save(checkpoint).await?;

        Ok(())
    }

    /// Stream graph execution, yielding intermediate results
    ///
    /// Returns a stream that yields events as nodes complete execution.
    /// This enables real-time consumption of results for long-running workflows.
    ///
    /// # Arguments
    ///
    /// * `initial_state` - Starting state for the graph
    /// * `mode` - Controls what data is yielded:
    ///   - [`StreamMode::Values`] - Full state after each node
    ///   - [`StreamMode::Updates`] - Only changed fields
    ///   - [`StreamMode::Events`] - Low-level execution events
    ///
    /// # Returns
    ///
    /// A stream of [`StreamEvent`] items. Use `futures_util::StreamExt` to iterate.
    ///
    /// # Errors
    ///
    /// Stream items may contain errors:
    /// - [`crate::Error::NodeNotFound`] - An edge references a non-existent node
    /// - [`crate::Error::NodeExecution`] - A node function returned an error
    /// - [`crate::Error::Timeout`] - Node execution exceeded timeout
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use futures_util::StreamExt;
    /// use dashflow::StreamMode;
    ///
    /// let mut stream = app.stream(AgentState::new(), StreamMode::Values);
    ///
    /// while let Some(event) = stream.next().await {
    ///     match event? {
    ///         StreamEvent::Values { node, state } => {
    ///             println!("Node {} completed", node);
    ///         }
    ///         StreamEvent::Done { state, .. } => {
    ///             println!("Graph complete!");
    ///             break;
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    ///
    /// # See Also
    ///
    /// - [`CompiledGraph::invoke`] - Execute without streaming
    /// - [`StreamMode`] - Available streaming modes
    /// - [`StreamEvent`] - Event types yielded by the stream
    /// - [`CompiledGraph::stream_multi`] - Stream multiple modes simultaneously
    pub fn stream(
        &self,
        initial_state: S,
        mode: StreamMode,
    ) -> impl Stream<Item = Result<StreamEvent<S>>> + '_ {
        self.stream_multi(initial_state, vec![mode])
    }

    /// Stream graph execution with multiple concurrent modes
    ///
    /// This allows streaming multiple modes simultaneously, matching Python's
    /// `stream_mode=["values", "updates"]` capability.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Stream both values and updates
    /// let mut stream = app.stream_multi(
    ///     initial_state,
    ///     vec![StreamMode::Values, StreamMode::Updates]
    /// );
    ///
    /// while let Some(event) = stream.next().await {
    ///     match event? {
    ///         StreamEvent::Values { .. } => { /* ... */ },
    ///         StreamEvent::Update { .. } => { /* ... */ },
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub fn stream_multi(
        &self,
        initial_state: S,
        modes: Vec<StreamMode>,
    ) -> impl Stream<Item = Result<StreamEvent<S>>> + '_ {
        // Arc::clone is O(1) - just increments reference count
        let nodes = Arc::clone(&self.nodes);
        let edges = Arc::clone(&self.edges);
        let conditional_edges = Arc::clone(&self.conditional_edges);
        let parallel_edges = Arc::clone(&self.parallel_edges);
        let entry_point = Arc::clone(&self.entry_point);
        // Always apply node timeout (use default if not set)
        let node_timeout = self.node_timeout.unwrap_or(DEFAULT_NODE_TIMEOUT);
        // Use configured channel capacity or default
        let stream_capacity = self.stream_channel_capacity;
        let graph_name = self.name.clone().unwrap_or_else(|| "graph".to_string());
        // Generate request_id for this stream invocation (same as invoke())
        let request_id = Uuid::new_v4();
        let stream_modes = modes
            .iter()
            .map(|mode| format!("{mode:?}"))
            .collect::<Vec<_>>();
        let span = info_span!(
            "graph.stream",
            request_id = %request_id,
            graph.name = graph_name.as_str(),
            graph.entry_point = entry_point.as_str(),
            stream.modes = ?stream_modes
        );

        stream! {
            let mut state = initial_state;
            let mut current_nodes = vec![(*entry_point).clone()];
            // Pre-allocate with typical graph size
            let mut nodes_executed = Vec::with_capacity(16);

            loop {
                // Execute current node(s)
                if current_nodes.len() == 1 {
                    // Single node execution
                    let current_node = &current_nodes[0];
                    let node = if let Some(n) = nodes.get(current_node) { n } else {
                        yield Err(Error::NodeNotFound(current_node.clone()));
                        return;
                    };

                    nodes_executed.push(current_node.clone());

                    // Emit node start if Events mode is enabled
                    if modes.contains(&StreamMode::Events) {
                        yield Ok(StreamEvent::NodeStart {
                            node: current_node.clone(),
                        });
                    }

                    // Set up custom stream writer if Custom mode is enabled
                    let custom_rx = if modes.contains(&StreamMode::Custom) {
                        let (tx, rx) = match stream_capacity {
                            Some(cap) => crate::stream::create_stream_channel_with_capacity(cap),
                            None => crate::stream::create_stream_channel(),
                        };
                        crate::stream::set_stream_writer(Some(tx));
                        Some(rx)
                    } else {
                        crate::stream::set_stream_writer(None);
                        None
                    };

                    // Execute node with timeout
                    let execution = node.execute(state.clone());
                    let new_state = match tokio::time::timeout(node_timeout, execution)
                        .instrument(span.clone())
                        .await
                    {
                        Ok(result) => result.map_err(|e| Error::NodeExecution {
                            node: current_node.clone(),
                            source: Box::new(e),
                        }),
                        Err(_) => Err(Error::Timeout(node_timeout)),
                    };

                    // Clear stream writer
                    crate::stream::set_stream_writer(None);

                    match new_state {
                        Ok(new_state) => {
                            state = new_state;

                            // Yield custom events if in Custom mode
                            if let Some(mut rx) = custom_rx {
                                while let Ok(data) = rx.try_recv() {
                                    yield Ok(StreamEvent::Custom {
                                        node: current_node.clone(),
                                        data,
                                    });
                                }
                            }

                            // Emit events for each enabled mode
                            if modes.contains(&StreamMode::Values) {
                                yield Ok(StreamEvent::Values {
                                    node: current_node.clone(),
                                    state: state.clone(),
                                });
                            }
                            if modes.contains(&StreamMode::Updates) {
                                yield Ok(StreamEvent::Update {
                                    node: current_node.clone(),
                                    state: state.clone(),
                                });
                            }
                            if modes.contains(&StreamMode::Events) {
                                yield Ok(StreamEvent::NodeEnd {
                                    node: current_node.clone(),
                                    state: state.clone(),
                                });
                            }
                            // Custom events already yielded above
                        }
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    }

                    // Find next node(s)
                    let next_result = Self::find_next_nodes_static(
                        current_node,
                        &state,
                        &edges,
                        &conditional_edges,
                        &parallel_edges,
                    );

                    match next_result {
                        Ok((next_nodes, _edge_type)) => {
                            match &next_nodes {
                                NextNodes::Single(next) => {
                                    if next.as_str() == END {
                                        break;
                                    }
                                    current_nodes = vec![(**next).clone()];
                                }
                                NextNodes::Parallel(nexts) => {
                                    current_nodes = (**nexts).clone();
                                }
                            }
                        }
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    }
                } else {
                    // Parallel execution not fully supported in streaming yet
                    // Just execute nodes sequentially for now
                    for node_name in &current_nodes {
                        let node = if let Some(n) = nodes.get(node_name) { n } else {
                            yield Err(Error::NodeNotFound(node_name.clone()));
                            return;
                        };

                        nodes_executed.push(node_name.clone());

                        if modes.contains(&StreamMode::Events) {
                            yield Ok(StreamEvent::NodeStart {
                                node: node_name.clone(),
                            });
                        }

                        // Set up custom stream writer if Custom mode is enabled
                        let custom_rx = if modes.contains(&StreamMode::Custom) {
                            let (tx, rx) = match stream_capacity {
                            Some(cap) => crate::stream::create_stream_channel_with_capacity(cap),
                            None => crate::stream::create_stream_channel(),
                        };
                            crate::stream::set_stream_writer(Some(tx));
                            Some(rx)
                        } else {
                            crate::stream::set_stream_writer(None);
                            None
                        };

                        // Execute node with timeout
                        let execution = node.execute(state.clone());
                        let new_state = match tokio::time::timeout(node_timeout, execution)
                            .instrument(span.clone())
                            .await
                        {
                            Ok(result) => result.map_err(|e| Error::NodeExecution {
                                node: node_name.clone(),
                                source: Box::new(e),
                            }),
                            Err(_) => Err(Error::Timeout(node_timeout)),
                        };

                        // Clear stream writer
                        crate::stream::set_stream_writer(None);

                        match new_state {
                            Ok(new_state) => {
                                state = new_state;

                                // Yield custom events if in Custom mode
                                if let Some(mut rx) = custom_rx {
                                    while let Ok(data) = rx.try_recv() {
                                        yield Ok(StreamEvent::Custom {
                                            node: node_name.clone(),
                                            data,
                                        });
                                    }
                                }

                                // Emit events for each enabled mode
                                if modes.contains(&StreamMode::Values) {
                                    yield Ok(StreamEvent::Values {
                                        node: node_name.clone(),
                                        state: state.clone(),
                                    });
                                }
                                if modes.contains(&StreamMode::Updates) {
                                    yield Ok(StreamEvent::Update {
                                        node: node_name.clone(),
                                        state: state.clone(),
                                    });
                                }
                                if modes.contains(&StreamMode::Events) {
                                    yield Ok(StreamEvent::NodeEnd {
                                        node: node_name.clone(),
                                        state: state.clone(),
                                    });
                                }
                                // Custom events already yielded above
                            }
                            Err(e) => {
                                yield Err(e);
                                return;
                            }
                        }
                    }

                    // After parallel, continue with next node
                    let last_node = &current_nodes[current_nodes.len() - 1];
                    let next_result = Self::find_next_nodes_static(
                        last_node,
                        &state,
                        &edges,
                        &conditional_edges,
                        &parallel_edges,
                    );

                    match next_result {
                        Ok((next_nodes, _edge_type)) => {
                            match &next_nodes {
                                NextNodes::Single(next) => {
                                    if next.as_str() == END {
                                        break;
                                    }
                                    current_nodes = vec![(**next).clone()];
                                }
                                NextNodes::Parallel(nexts) => {
                                    current_nodes = (**nexts).clone();
                                }
                            }
                        }
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    }
                }
            }

            // Yield final event
            yield Ok(StreamEvent::Done {
                state,
                execution_path: nodes_executed,
            });
        }
    }

    /// Create NodeContext for a node execution
    #[cfg(feature = "dashstream")]
    fn create_node_context(&self, node_name: &str) -> crate::node::NodeContext {
        let producer = self.callbacks.iter().find_map(|cb| cb.get_producer());

        let (thread_id, tenant_id) = self
            .callbacks
            .iter()
            .find_map(|cb| cb.get_ids())
            .unwrap_or_else(|| ("default".to_string(), "default".to_string()));

        crate::node::NodeContext::new(node_name.to_string(), producer, thread_id, tenant_id)
    }

    /// Execute a node with optional timeout and retry logic
    ///
    /// By default, transient failures (timeout errors) are automatically retried
    /// using exponential backoff. Use `without_retries()` to disable this behavior.
    async fn execute_node(&self, node: &BoxedNode<S>, state: S, node_name: &str) -> Result<S> {
        // Calculate input state size for tracing (skip expensive serialization when metrics disabled)
        let input_size = if self.metrics_enabled {
            bincode::serialized_size(&state).unwrap_or(0)
        } else {
            0 // Skip serialization overhead when metrics disabled
        };

        // Create context if node supports streaming
        #[cfg(feature = "dashstream")]
        let ctx = if node.supports_streaming() {
            self.create_node_context(node_name)
        } else {
            crate::node::NodeContext::empty()
        };

        // Create a tracing span for this node execution
        #[cfg(feature = "dashstream")]
        let span = info_span!(
            "node.execute",
            node.name = node_name,
            input_size_bytes = input_size,
            output_size_bytes = tracing::field::Empty,
            streaming = node.supports_streaming(),
            retries_enabled = self.retry_policy.is_some()
        );

        #[cfg(not(feature = "dashstream"))]
        let span = info_span!(
            "node.execute",
            node.name = node_name,
            input_size_bytes = input_size,
            output_size_bytes = tracing::field::Empty,
            retries_enabled = self.retry_policy.is_some()
        );

        // Capture values before the async block
        let max_state_size = self.max_state_size;
        let metrics_enabled = self.metrics_enabled;
        let node_name_owned = node_name.to_string();
        let node_timeout = self.node_timeout.unwrap_or(DEFAULT_NODE_TIMEOUT);
        let retry_policy = self.retry_policy.clone();

        async move {
            // Helper macro to execute node once with timeout
            // This is inlined rather than a closure to avoid move issues with ctx
            macro_rules! execute_node_once {
                ($current_state:expr) => {{
                    // Call execute_with_context if dashstream feature enabled, otherwise execute
                    #[cfg(feature = "dashstream")]
                    let execution = node.execute_with_context($current_state, &ctx);
                    #[cfg(not(feature = "dashstream"))]
                    let execution = node.execute($current_state);

                    // Apply node timeout
                    let result = match tokio::time::timeout(node_timeout, execution).await {
                        Ok(result) => result.map_err(|e| Error::NodeExecution {
                            node: node_name_owned.clone(),
                            source: Box::new(e),
                        }),
                        Err(_) => Err(Error::Timeout(node_timeout)),
                    };

                    // Record output state size and check against limit if successful
                    // Only calculate size if needed for: (1) metrics/tracing OR (2) state size limit
                    if let Ok(ref output_state) = result {
                        let needs_size = metrics_enabled || max_state_size.is_some();
                        let output_size = if needs_size {
                            bincode::serialized_size(output_state).unwrap_or(0)
                        } else {
                            0 // Skip expensive serialization when not needed
                        };

                        if metrics_enabled {
                            tracing::Span::current().record("output_size_bytes", output_size);
                        }

                        // Check state size limit (if configured)
                        if let Some(max_bytes) = max_state_size {
                            if output_size > max_bytes {
                                return Err(Error::StateSizeExceeded {
                                    node: node_name_owned.clone(),
                                    actual_bytes: output_size,
                                    max_bytes,
                                });
                            }
                        }
                    }

                    result
                }};
            }

            // Apply retry logic if enabled
            match retry_policy {
                Some(ref policy) if policy.max_retries > 0 => {
                    let mut last_error = None;
                    let current_state = state;

                    for attempt in 0..=policy.max_retries {
                        if attempt > 0 {
                            // Calculate delay based on strategy
                            let delay = match &policy.strategy {
                                crate::core::retry::RetryStrategy::Exponential {
                                    initial_delay_ms,
                                    max_delay_ms,
                                    multiplier,
                                } => {
                                    let delay = initial_delay_ms
                                        * multiplier.saturating_pow((attempt - 1) as u32);
                                    Duration::from_millis(delay.min(*max_delay_ms))
                                }
                                crate::core::retry::RetryStrategy::ExponentialJitter {
                                    initial_delay_ms,
                                    max_delay_ms,
                                    exp_base,
                                    jitter_ms,
                                } => {
                                    use rand::Rng;
                                    let exp_delay = (*initial_delay_ms as f64)
                                        * exp_base.powi((attempt - 1) as i32);
                                    let base_delay = exp_delay.min(*max_delay_ms as f64) as u64;
                                    let jitter = rand::thread_rng().gen_range(0..=*jitter_ms);
                                    Duration::from_millis((base_delay + jitter).min(*max_delay_ms))
                                }
                                crate::core::retry::RetryStrategy::Fixed { delay_ms } => {
                                    Duration::from_millis(*delay_ms)
                                }
                            };
                            tracing::debug!(
                                "Retrying node {} (attempt {}/{}), delay {:?}",
                                node_name_owned,
                                attempt + 1,
                                policy.max_retries + 1,
                                delay
                            );
                            tokio::time::sleep(delay).await;
                        }

                        match execute_node_once!(current_state.clone()) {
                            Ok(result) => return Ok(result),
                            Err(err) => {
                                // Only retry on Timeout errors (transient failures)
                                // Other errors are not retried
                                if matches!(err, Error::Timeout(_)) {
                                    last_error = Some(err);
                                    // current_state already cloned above for next iteration
                                } else {
                                    // Non-retryable error, return immediately
                                    return Err(err);
                                }
                            }
                        }
                    }

                    // All retries exhausted
                    Err(last_error.unwrap_or_else(|| {
                        Error::InternalExecutionError(
                            "Retry loop completed without error".to_string(),
                        )
                    }))
                }
                _ => {
                    // No retry policy or max_retries == 0
                    execute_node_once!(state)
                }
            }
        }
        .instrument(span)
        .await
    }

    /// Merge multiple states from parallel execution using `MergeableState::merge()`
    ///
    /// **FIXED (v1.11.0):** Now automatically uses `MergeableState::merge()` to aggregate
    /// all parallel results. Zero data loss - all branches' changes are preserved.
    ///
    /// **Note (v1.12.0):** `CompiledGraph<S>` struct now accepts `S: GraphState`, but this method
    /// is only available when `S: MergeableState`. Sequential-only graphs that don't use
    /// parallel edges don't need to implement `MergeableState`.
    fn merge_parallel_results(&self, states: Vec<S>) -> Result<S> {
        // Use into_iter and fold to avoid unwrap entirely
        // If empty, return error instead of panicking
        let mut iter = states.into_iter();
        let first = match iter.next() {
            Some(state) => state,
            None => {
                tracing::error!("merge_parallel_results called with empty states vector");
                return Err(Error::InternalExecutionError(
                    "Cannot merge empty state vector".to_string(),
                ));
            }
        };

        // If only one state, return it directly (no merge needed)
        let mut result = first;
        let mut count = 1;
        for state in iter {
            result.merge(&state);
            count += 1;
        }

        if count > 1 {
            tracing::info!(
                "Merged {} parallel states using MergeableState::merge() - zero data loss",
                count
            );
        }

        Ok(result)
    }

    /// # Parameters
    ///
    /// * `initial_state` - The initial state to start execution from
    /// * `skip_interrupt_for` - Optional node name to skip interrupt checks for (used by resume)
    /// * `start_nodes` - Optional list of nodes to start from (defaults to `entry_point` if None)
    async fn invoke_internal(
        &self,
        initial_state: S,
        skip_interrupt_for: Option<&str>,
        start_nodes: Option<Vec<String>>,
    ) -> Result<ExecutionResult<S>> {
        // Create a tracing span for the entire graph execution
        let graph_name = self.name.as_deref().unwrap_or("graph");
        // Generate a unique request_id for this invocation.
        // This propagates to all child spans (node.execute, scheduler.*, etc.)
        // via tracing's parent-child context, enabling end-to-end correlation.
        let request_id = Uuid::new_v4();
        let span = info_span!(
            "graph.invoke",
            request_id = %request_id,
            graph.name = graph_name,
            graph.entry_point = %self.entry_point,
            graph.duration_ms = tracing::field::Empty,
            graph.nodes_executed = tracing::field::Empty
        );

        async move {
            let start_time = SystemTime::now();

            // Reset metrics for this execution (if enabled)
            if self.metrics_enabled {
                let mut metrics = self.metrics.lock().await;
                *metrics = ExecutionMetrics::new();
            }

            // Record Prometheus metrics if observability feature is enabled
            #[cfg(feature = "observability")]
            {
                if let Some(recorder) = dashflow_observability::metrics::MetricsRecorder::global() {
                    recorder.inc_active_graphs(graph_name);
                }
            }

            // Emit graph start event (only clone state if we have callbacks)
            if !self.callbacks.is_empty() {
                // Broadcast graph manifest with telemetry for AI introspection
                let manifest = Some(Box::new(self.manifest()));
                self.emit_event(GraphEvent::GraphStart {
                    timestamp: start_time,
                    initial_state: initial_state.clone(),
                    manifest,
                });
            }

            let mut state = initial_state;
            let mut current_nodes =
                start_nodes.unwrap_or_else(|| vec![(*self.entry_point).clone()]);
            // Pre-allocate with typical graph size (reduces allocations in hot path)
            let mut nodes_executed = Vec::with_capacity(16);
            let mut last_checkpoint_id: Option<String> = None;
            let mut iteration_count: u32 = 0;
            // Track if we've already applied the skip_interrupt_for (only applies to first matching node)
            let mut skip_interrupt_for_remaining = skip_interrupt_for;

            // Use LocalMetricsBatch to reduce mutex lock acquisitions
            // Instead of locking per-operation, we batch metrics locally and apply once at end
            let mut metrics_batch = LocalMetricsBatch::new();

            loop {
                // Check recursion limit
                iteration_count += 1;
                if iteration_count > self.recursion_limit {
                    return Err(Error::RecursionLimit {
                        limit: self.recursion_limit,
                    });
                }

                // Execute current node(s)
                if current_nodes.len() == 1 {
                    // Single node execution
                    let current_node = &current_nodes[0];
                    let node = self
                        .nodes
                        .get(current_node)
                        .ok_or_else(|| Error::NodeNotFound(current_node.clone()))?;

                    nodes_executed.push(current_node.clone());

                    // CHECK: Should interrupt BEFORE this node?
                    // Skip interrupt check if this is the node we're resuming from (only first occurrence)
                    let should_skip = skip_interrupt_for_remaining == Some(current_node.as_str());
                    let should_interrupt_before =
                        self.interrupt_before.contains(current_node) && !should_skip;

                    // Clear the skip flag after using it once
                    if should_skip {
                        skip_interrupt_for_remaining = None;
                    }

                    if should_interrupt_before {
                        // Verify checkpointer is configured
                        if self.checkpointer.is_none() {
                            return Err(Error::InterruptWithoutCheckpointer(current_node.clone()));
                        }
                        // Verify thread_id is configured
                        if self.thread_id.is_none() {
                            return Err(Error::InterruptWithoutThreadId(current_node.clone()));
                        }

                        // Save checkpoint BEFORE node execution (always, regardless of policy)
                        // Use take() to avoid clone - checkpoint returns new ID which we discard for interrupts
                        // Interrupts bypass policy because resumability is critical for human-in-the-loop
                        self.save_checkpoint_unconditional(
                            &state,
                            current_node,
                            last_checkpoint_id.take(),
                        )
                        .await?;

                        // Return with interrupt metadata
                        let total_duration = start_time.elapsed().unwrap_or(Duration::ZERO);

                        // Apply batched metrics before returning (interrupt path)
                        if self.metrics_enabled {
                            let mut metrics = self.metrics.lock().await;
                            metrics_batch.apply_to(&mut metrics);
                            metrics.set_total_duration(total_duration);
                        }

                        // Emit graph end event with interrupt
                        if !self.callbacks.is_empty() {
                            self.emit_event(GraphEvent::GraphEnd {
                                timestamp: SystemTime::now(),
                                final_state: state.clone(),
                                duration: total_duration,
                                execution_path: nodes_executed.clone(),
                            });
                        }

                        return Ok(ExecutionResult {
                            final_state: state,
                            nodes_executed,
                            interrupted_at: Some(current_node.clone()),
                            next_nodes: vec![current_node.clone()],
                        });
                    }

                    // Emit node start event (only clone state if we have callbacks)
                    let node_start_time = SystemTime::now();
                    // Get node config for telemetry (Config Versioning)
                    let node_config = self.node_configs.get(current_node).cloned();
                    if !self.callbacks.is_empty() {
                        self.emit_event(GraphEvent::NodeStart {
                            timestamp: node_start_time,
                            node: current_node.clone(),
                            state: state.clone(),
                            node_config: node_config.clone(),
                        });
                    }

                    // M-245 optimization: check if node is read-only to skip state change computation
                    // Read-only nodes don't modify state, so we can skip the expensive
                    // compute_state_changes() call which involves JSON serialization
                    let is_read_only = node.is_read_only();

                    // Execute node
                    match self.execute_node(node, state.clone(), current_node).await {
                        Ok(new_state) => {
                            let node_duration = node_start_time.elapsed().unwrap_or(Duration::ZERO);

                            // Record node execution with timestamp in batch (no lock)
                            metrics_batch.record_node_execution_with_timestamp(
                                current_node,
                                node_duration,
                                node_start_time,
                            );

                            // Record Prometheus metrics for node execution
                            #[cfg(feature = "observability")]
                            {
                                if let Some(recorder) =
                                    dashflow_observability::metrics::MetricsRecorder::global()
                                {
                                    recorder.record_node_execution(
                                        graph_name,
                                        current_node,
                                        "success",
                                    );
                                    recorder.record_node_duration(
                                        graph_name,
                                        current_node,
                                        node_duration.as_secs_f64(),
                                    );
                                }
                            }

                            // Emit node end event (only clone state if we have callbacks)
                            if !self.callbacks.is_empty() {
                                self.emit_event(GraphEvent::NodeEnd {
                                    timestamp: SystemTime::now(),
                                    node: current_node.clone(),
                                    state: new_state.clone(),
                                    duration: node_duration,
                                    node_config,
                                });

                                // Emit StateChanged event (Observability Phase 3)
                                // Computes diff between old and new state to provide visibility
                                // into what changed. M-245: Skip for read-only nodes (no changes possible).
                                if !is_read_only {
                                    if let Some(changes) = compute_state_changes(&state, &new_state) {
                                        if changes.has_changes() {
                                            self.emit_event(GraphEvent::StateChanged {
                                                timestamp: SystemTime::now(),
                                                node: current_node.clone(),
                                                summary: changes.summary(),
                                                fields_added: changes.fields_added,
                                                fields_removed: changes.fields_removed,
                                                fields_modified: changes.fields_modified,
                                            });
                                        }
                                    }
                                }
                            }

                            state = new_state;

                            // Save checkpoint after node execution
                            // Use take() to avoid clone - we're replacing the value anyway
                            last_checkpoint_id = self
                                .save_checkpoint(&state, current_node, last_checkpoint_id.take())
                                .await?;

                            // CHECK: Should interrupt AFTER this node?
                            // Skip interrupt check if this is the node we're resuming from (only first occurrence)
                            let should_skip =
                                skip_interrupt_for_remaining == Some(current_node.as_str());
                            let should_interrupt_after =
                                self.interrupt_after.contains(current_node) && !should_skip;

                            // Clear the skip flag after using it once
                            if should_skip {
                                skip_interrupt_for_remaining = None;
                            }

                            if should_interrupt_after {
                                // Verify checkpointer is configured
                                if self.checkpointer.is_none() {
                                    return Err(Error::InterruptWithoutCheckpointer(
                                        current_node.clone(),
                                    ));
                                }
                                // Verify thread_id is configured
                                if self.thread_id.is_none() {
                                    return Err(Error::InterruptWithoutThreadId(
                                        current_node.clone(),
                                    ));
                                }

                                // Force save checkpoint for interrupt_after (bypass policy)
                                // This ensures resumability for human-in-the-loop workflows
                                // We save unconditionally because even if policy saved earlier,
                                // we need the checkpoint to be at this exact node/state for resume
                                let _checkpoint_id = self
                                    .save_checkpoint_unconditional(
                                        &state,
                                        current_node,
                                        last_checkpoint_id.take(),
                                    )
                                    .await?;

                                // Determine next nodes for resume
                                let (next_nodes_enum, _) =
                                    self.find_next_nodes_with_type(current_node, &state)?;

                                let next_nodes_list = match &next_nodes_enum {
                                    NextNodes::Single(next) => {
                                        if next.as_str() == END {
                                            vec![]
                                        } else {
                                            vec![(**next).clone()]
                                        }
                                    }
                                    NextNodes::Parallel(nexts) => (**nexts).clone(),
                                };

                                // Return with interrupt metadata
                                let total_duration = start_time.elapsed().unwrap_or(Duration::ZERO);

                                // Apply batched metrics before returning (interrupt_after path)
                                if self.metrics_enabled {
                                    let mut metrics = self.metrics.lock().await;
                                    metrics_batch.apply_to(&mut metrics);
                                    metrics.set_total_duration(total_duration);
                                }

                                // Emit graph end event with interrupt
                                if !self.callbacks.is_empty() {
                                    self.emit_event(GraphEvent::GraphEnd {
                                        timestamp: SystemTime::now(),
                                        final_state: state.clone(),
                                        duration: total_duration,
                                        execution_path: nodes_executed.clone(),
                                    });
                                }

                                return Ok(ExecutionResult {
                                    final_state: state,
                                    nodes_executed,
                                    interrupted_at: Some(current_node.clone()),
                                    next_nodes: next_nodes_list,
                                });
                            }
                        }
                        Err(e) => {
                            // Emit node error event (only clone state if we have callbacks)
                            if !self.callbacks.is_empty() {
                                self.emit_event(GraphEvent::NodeError {
                                    timestamp: SystemTime::now(),
                                    node: current_node.clone(),
                                    error: e.to_string(),
                                    state: state.clone(),
                                });
                            }

                            // Apply batched metrics before returning error
                            if self.metrics_enabled {
                                let mut metrics = self.metrics.lock().await;
                                metrics_batch.apply_to(&mut metrics);
                            }

                            // Record Prometheus metrics for error
                            #[cfg(feature = "observability")]
                            {
                                if let Some(recorder) =
                                    dashflow_observability::metrics::MetricsRecorder::global()
                                {
                                    recorder.dec_active_graphs(graph_name);
                                    recorder.record_graph_invocation(graph_name, "error");
                                    recorder.record_node_execution(
                                        graph_name,
                                        current_node,
                                        "error",
                                    );
                                }
                            }

                            return Err(e);
                        }
                    }

                    // Find next node(s)
                    let (next_nodes, edge_type) =
                        self.find_next_nodes_with_type(current_node, &state)?;

                    // Record metrics for edge traversal in batch (no lock)
                    metrics_batch.record_edge_traversal();
                    if let EdgeType::Conditional { ref condition_result } = edge_type {
                        metrics_batch.record_conditional_branch();

                        // Emit EdgeEvaluated event for observability (FIX-001)
                        // This provides visibility into why a particular path was chosen
                        if !self.callbacks.is_empty() {
                            // Find the conditional edge that was evaluated
                            if let Some(cond_edge) = self
                                .conditional_edges
                                .iter()
                                .find(|e| e.from.as_str() == current_node)
                            {
                                let selected_target = match &next_nodes {
                                    NextNodes::Single(t) => (**t).clone(),
                                    NextNodes::Parallel(_) => {
                                        // Conditional edges always return single node
                                        current_node.clone()
                                    }
                                };

                                // Build alternatives from other routes
                                let alternatives: Vec<EdgeAlternative> = cond_edge
                                    .routes
                                    .iter()
                                    .filter(|(result, _)| *result != condition_result)
                                    .map(|(result, target)| EdgeAlternative {
                                        to_node: (**target).clone(),
                                        reason: Some(format!(
                                            "condition returned '{}', not '{}'",
                                            condition_result, result
                                        )),
                                        was_evaluated: false,
                                    })
                                    .collect();

                                self.emit_event(GraphEvent::EdgeEvaluated {
                                    timestamp: SystemTime::now(),
                                    from_node: current_node.clone(),
                                    to_node: selected_target,
                                    condition_expression: None,
                                    evaluation_result: true,
                                    alternatives,
                                });
                            }
                        }
                    }

                    match &next_nodes {
                        NextNodes::Single(next) => {
                            // Emit edge traversal event (including END edges for observability parity with metrics)
                            self.emit_event(GraphEvent::EdgeTraversal {
                                timestamp: SystemTime::now(),
                                from: current_node.clone(),
                                to: vec![(**next).clone()],
                                edge_type,
                            });

                            if next.as_str() == END {
                                break;
                            }
                            current_nodes = vec![(**next).clone()];
                        }
                        NextNodes::Parallel(nexts) => {
                            // Emit edge traversal event
                            self.emit_event(GraphEvent::EdgeTraversal {
                                timestamp: SystemTime::now(),
                                from: current_node.clone(),
                                to: (**nexts).clone(),
                                edge_type,
                            });

                            current_nodes = (**nexts).clone();
                        }
                    }
                } else {
                    // Parallel execution - all nodes get the same state
                    let parallel_start_time = SystemTime::now();
                    let concurrency = current_nodes.len();

                    // Record parallel execution in batch (no lock)
                    metrics_batch.record_parallel_execution(concurrency);

                    // Emit parallel start event
                    self.emit_event(GraphEvent::ParallelStart {
                        timestamp: parallel_start_time,
                        nodes: current_nodes.clone(),
                    });

                    // Check if scheduler is configured for distributed execution
                    if let Some(scheduler) = &self.scheduler {
                        // Use work-stealing scheduler for distributed execution
                        let node_results = scheduler
                            .execute_parallel(&current_nodes, &state, &self.nodes)
                            .await?;

                        // Record all executed nodes
                        nodes_executed.extend(current_nodes.iter().cloned());

                        // Merge parallel results using MergeableState::merge()
                        // Fixed (M-818): Previously used "last state wins" which caused data loss
                        // for distributed execution. Now consistent with local parallel path.
                        state = self.merge_parallel_results(node_results)?;
	                    } else {
	                        // Local parallel execution using tokio::spawn with concurrency limit
	                        let mut tasks = Vec::with_capacity(current_nodes.len());
	                        // Always apply node timeout (use default if not set)
	                        let node_timeout = self.node_timeout.unwrap_or(DEFAULT_NODE_TIMEOUT);
	                        let execution_stack = super::execution_hierarchy::capture_stack();

                        // Create semaphore for limiting concurrent parallel tasks
                        // If max_parallel_tasks is None (via without_limits()), allow unlimited
                        let semaphore = self
                            .max_parallel_tasks
                            .map(|limit| Arc::new(Semaphore::new(limit)));

	                        for node_name in &current_nodes {
	                            let node = self
	                                .nodes
	                                .get(node_name)
	                                .ok_or_else(|| Error::NodeNotFound(node_name.clone()))?
	                                .clone();

	                            let node_name_clone = node_name.clone();
	                            let state_clone = state.clone();
	                            let sem_clone = semaphore.clone();
	                            let inherited_stack = execution_stack.clone();

	                            tasks.push(tokio::spawn(async move {
	                                let task_body = async move {
	                                    // Acquire semaphore permit before executing (if limit is set)
	                                    // The permit is automatically released when _permit is dropped
	                                    let _permit = if let Some(ref sem) = sem_clone {
	                                        Some(
	                                            sem.acquire()
	                                                .await
	                                                .expect("semaphore closed unexpectedly"),
	                                        )
	                                    } else {
	                                        None
	                                    };

	                                    let execution = node.execute(state_clone);
	                                    // Execute with timeout
	                                    let result = match tokio::time::timeout(node_timeout, execution).await
	                                    {
	                                        Ok(r) => r,
	                                        Err(_) => Err(Error::Timeout(node_timeout)),
	                                    };
	                                    (node_name_clone, result)
	                                };

	                                if let Some(stack) = inherited_stack {
	                                    super::execution_hierarchy::scope_stack(stack, task_body).await
	                                } else {
	                                    task_body.await
	                                }
	                            }));
	                        }

                        // Wait for all tasks to complete
                        let mut results = Vec::with_capacity(tasks.len());
                        for task in tasks {
                            let (node_name, result) = task.await.map_err(|e| {
                                Error::InternalExecutionError(format!("Task join error: {e}"))
                            })?;
                            results.push((node_name, result));
                        }

                        // Process results - collect all successful states
                        let mut successful_states = Vec::new();
                        for (node_name, result) in results {
                            match result {
                                Ok(new_state) => {
                                    // Move node_name into nodes_executed (avoid clone)
                                    nodes_executed.push(node_name);
                                    successful_states.push(new_state);
                                }
                                Err(e) => {
                                    return Err(Error::NodeExecution {
                                        node: node_name,
                                        source: Box::new(e),
                                    });
                                }
                            }
                        }

                        if successful_states.is_empty() {
                            return Err(Error::ParallelExecutionFailed);
                        }

                        // Merge parallel results
                        // Check if S implements MergeableState and use proper merging if so
                        state = self.merge_parallel_results(successful_states)?;
                    }

                    // Emit parallel end event
                    let parallel_duration = parallel_start_time.elapsed().unwrap_or(Duration::ZERO);
                    self.emit_event(GraphEvent::ParallelEnd {
                        timestamp: SystemTime::now(),
                        nodes: current_nodes.clone(),
                        duration: parallel_duration,
                    });

                    // Save checkpoint after parallel execution
                    // Use the last parallel node as the checkpoint node name
                    let checkpoint_node = current_nodes.last().ok_or_else(|| {
                        Error::InternalExecutionError(
                            "parallel execution with empty node list".to_string(),
                        )
                    })?;
                    // Use take() to avoid clone - we're replacing the value anyway
                    last_checkpoint_id = self
                        .save_checkpoint(&state, checkpoint_node, last_checkpoint_id.take())
                        .await?;

                    // After parallel execution, continue with single path
                    // Find next node from last executed parallel node
                    let last_node = current_nodes.last().ok_or_else(|| {
                        Error::InternalExecutionError(
                            "parallel execution with empty node list".to_string(),
                        )
                    })?;
                    match self.find_next_nodes(last_node, &state)? {
                        NextNodes::Single(next) => {
                            if next.as_str() == END {
                                break;
                            }
                            current_nodes = vec![(*next).clone()];
                        }
                        NextNodes::Parallel(nexts) => {
                            current_nodes = (*nexts).clone();
                        }
                    }
                }
            }

            let total_duration = start_time.elapsed().unwrap_or(Duration::ZERO);

            // Apply all batched metrics in single lock acquisition (if enabled)
            if self.metrics_enabled {
                let mut metrics = self.metrics.lock().await;
                metrics_batch.apply_to(&mut metrics);
                metrics.set_total_duration(total_duration);
            }

            // Record Prometheus metrics if observability feature is enabled
            #[cfg(feature = "observability")]
            {
                if let Some(recorder) = dashflow_observability::metrics::MetricsRecorder::global() {
                    recorder.dec_active_graphs(graph_name);
                    recorder.record_graph_invocation(graph_name, "success");
                    recorder.record_graph_duration(graph_name, total_duration.as_secs_f64());
                }
            }

            // Record span attributes for the graph execution
            // Safety: as_millis() returns u128, but practical graph durations fit in i64.
            // Saturate to i64::MAX on overflow (would require ~292 million years execution).
            Span::current().record(
                "graph.duration_ms",
                i64::try_from(total_duration.as_millis()).unwrap_or(i64::MAX),
            );
            // Safety: Vec length on 64-bit systems fits in i64 (max usize < i64::MAX).
            Span::current().record("graph.nodes_executed", nodes_executed.len() as i64);

            // Emit graph end event (only clone state if we have callbacks)
            if !self.callbacks.is_empty() {
                self.emit_event(GraphEvent::GraphEnd {
                    timestamp: SystemTime::now(),
                    final_state: state.clone(),
                    duration: total_duration,
                    execution_path: nodes_executed.clone(),
                });
            }

            Ok(ExecutionResult {
                final_state: state,
                nodes_executed,
                interrupted_at: None,
                next_nodes: Vec::new(),
            })
        }
        .instrument(span)
        .await
    }

    /// Find the next node(s) to execute with edge type information
    ///
    /// Returns both the next nodes and the type of edge traversed
    fn find_next_nodes_with_type(&self, current: &str, state: &S) -> Result<(NextNodes, EdgeType)> {
        // Check conditional edges first (highest priority)
        for cond_edge in self.conditional_edges.iter() {
            if cond_edge.from.as_str() == current {
                let next = cond_edge.evaluate(state);
                // Map the condition result to actual node name
                if let Some(target) = cond_edge.routes.get(&next) {
                    // Use Arc::clone() - routes values are pre-wrapped in Arc at edge construction
                    return Ok((
                        NextNodes::Single(Arc::clone(target)),
                        EdgeType::Conditional {
                            condition_result: next,
                        },
                    ));
                }
                return Err(Error::InvalidEdge(format!(
                    "Conditional edge from '{current}' returned '{next}' but no route exists for it"
                )));
            }
        }

        // Check parallel edges (second priority)
        for edge in self.parallel_edges.iter() {
            if edge.from.as_str() == current {
                return Ok((NextNodes::Parallel(edge.to.clone()), EdgeType::Parallel));
            }
        }

        // Check simple edges (third priority)
        for edge in self.edges.iter() {
            if edge.from.as_str() == current {
                return Ok((NextNodes::Single(edge.to.clone()), EdgeType::Simple));
            }
        }

        // No edge found - implicit end (use static END_NODE to avoid allocation)
        Ok((NextNodes::Single(get_end_node()), EdgeType::Simple))
    }

    /// Find the next node(s) to execute
    ///
    /// Returns either a single node name or a list of nodes to execute in parallel
    fn find_next_nodes(&self, current: &str, state: &S) -> Result<NextNodes> {
        let (next_nodes, _) = self.find_next_nodes_with_type(current, state)?;
        Ok(next_nodes)
    }

    /// Static version of `find_next_nodes_with_type` for use in `stream()`
    fn find_next_nodes_static(
        current: &str,
        state: &S,
        edges: &[Edge],
        conditional_edges: &[Arc<ConditionalEdge<S>>],
        parallel_edges: &[ParallelEdge],
    ) -> Result<(NextNodes, EdgeType)> {
        // Check conditional edges first (highest priority)
        for cond_edge in conditional_edges {
            if cond_edge.from.as_str() == current {
                let next = cond_edge.evaluate(state);
                // Map the condition result to actual node name
                if let Some(target) = cond_edge.routes.get(&next) {
                    // Use Arc::clone() - routes values are pre-wrapped in Arc at edge construction
                    return Ok((
                        NextNodes::Single(Arc::clone(target)),
                        EdgeType::Conditional {
                            condition_result: next,
                        },
                    ));
                }
                return Err(Error::InvalidEdge(format!(
                    "Conditional edge from '{current}' returned '{next}' but no route exists for it"
                )));
            }
        }

        // Check parallel edges (second priority)
        for edge in parallel_edges {
            if edge.from.as_str() == current {
                return Ok((NextNodes::Parallel(edge.to.clone()), EdgeType::Parallel));
            }
        }

        // Check simple edges (third priority)
        for edge in edges {
            if edge.from.as_str() == current {
                return Ok((NextNodes::Single(edge.to.clone()), EdgeType::Simple));
            }
        }

        // No edge found - implicit end (use static END_NODE to avoid allocation)
        Ok((NextNodes::Single(get_end_node()), EdgeType::Simple))
    }

    /// Get the entry point node name
    #[must_use]
    pub fn entry_point(&self) -> &str {
        &self.entry_point
    }

    /// Get the number of nodes in the graph
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges in the graph
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len() + self.conditional_edges.len() + self.parallel_edges.len()
    }

    /// Merge parallel states using custom strategy
    ///
    /// This method is available for ALL state types and allows explicit control
    /// over how parallel results are aggregated.
    ///
    /// # Arguments
    /// * `states` - Vector of states from parallel execution
    /// * `merge_fn` - Function that merges `other` into `base`
    ///
    /// # Example
    /// ```rust,no_run
    /// # use dashflow::executor::CompiledGraph;
    /// # use serde::{Deserialize, Serialize};
    /// # use dashflow::MergeableState;
    /// # #[derive(Clone, Serialize, Deserialize)]
    /// # struct MyState {
    /// #     findings: Vec<String>,
    /// #     insights: Vec<String>,
    /// # }
    /// # impl MergeableState for MyState {
    /// #     fn merge(&mut self, other: &Self) {
    /// #         self.findings.extend(other.findings.clone());
    /// #         self.insights.extend(other.insights.clone());
    /// #     }
    /// # }
    /// # let graph: CompiledGraph<MyState> = unimplemented!();
    /// # let states: Vec<MyState> = vec![];
    /// // Manual merge
    /// let merged = graph.merge_parallel_custom(states, |base, other| {
    ///     base.findings.extend(other.findings.clone());
    ///     base.insights.extend(other.insights.clone());
    /// });
    /// ```
    ///
    /// # Errors
    /// Returns an error if `states` is empty.
    pub fn merge_parallel_custom<F>(&self, states: Vec<S>, merge_fn: F) -> Result<S>
    where
        F: Fn(&mut S, &S),
    {
        // Use into_iter to avoid unwrap
        let mut iter = states.into_iter();
        let first = match iter.next() {
            Some(state) => state,
            None => {
                tracing::error!("merge_parallel_custom called with empty states vector");
                return Err(Error::InternalExecutionError(
                    "Cannot merge empty state vector".to_string(),
                ));
            }
        };

        let mut result = first;
        let mut count = 1;
        for state in iter {
            merge_fn(&mut result, &state);
            count += 1;
        }

        if count > 1 {
            tracing::debug!("Merged {} parallel states using custom strategy", count);
        }

        Ok(result)
    }
}

/// Additional methods for `CompiledGraph` (merge utilities)
///
/// **Note (v1.12.0):** These methods are available only when `S: MergeableState`.
/// Sequential-only graphs using `S: GraphState` don't have access to these methods.
impl<S> CompiledGraph<S>
where
    S: crate::state::MergeableState,
{
    /// Merge parallel states using `MergeableState::merge()`
    ///
    /// **Note:** As of v1.11.0, this is automatically used during parallel execution
    /// via `merge_parallel_results()`. This public method remains available for
    /// explicit merge scenarios outside of graph execution.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use dashflow::executor::CompiledGraph;
    /// # use dashflow::MergeableState;
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Clone, Serialize, Deserialize)]
    /// # struct MyState {
    /// #     findings: Vec<String>,
    /// #     insights: Vec<String>,
    /// # }
    /// # impl MergeableState for MyState {
    /// #     fn merge(&mut self, other: &Self) {
    /// #         self.findings.extend(other.findings.clone());
    /// #         self.insights.extend(other.insights.clone());
    /// #     }
    /// # }
    /// # let graph: CompiledGraph<MyState> = unimplemented!();
    /// # let states: Vec<MyState> = vec![];
    ///
    /// // Then use automatic merging
    /// let merged = graph.merge_with_mergeable(states);
    /// ```
    ///
    /// # Errors
    /// Returns an error if `states` is empty.
    pub fn merge_with_mergeable(&self, states: Vec<S>) -> Result<S> {
        // Use into_iter to avoid unwrap
        let mut iter = states.into_iter();
        let first = match iter.next() {
            Some(state) => state,
            None => {
                tracing::error!("merge_with_mergeable called with empty states vector");
                return Err(Error::InternalExecutionError(
                    "Cannot merge empty state vector".to_string(),
                ));
            }
        };

        let mut result = first;
        let mut count = 1;
        for state in iter {
            result.merge(&state);
            count += 1;
        }

        if count > 1 {
            tracing::debug!(
                "Merged {} parallel states using MergeableState::merge()",
                count
            );
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::{Edge, ParallelEdge, END};
    use crate::event::EdgeType;
    use crate::graph::StateGraph;
    use crate::state::MergeableState;
    use serde::{Deserialize, Serialize};

    // =========================================================================
    // Test State Type
    // =========================================================================

    #[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
    struct ExecTestState {
        values: Vec<i32>,
        counter: i32,
    }

    // GraphState is implemented via blanket impl for Clone + Send + Sync + Serialize + Deserialize

    impl MergeableState for ExecTestState {
        fn merge(&mut self, other: &Self) {
            self.values.extend(other.values.clone());
            self.counter += other.counter;
        }
    }

    // =========================================================================
    // get_end_node Tests
    // =========================================================================

    #[test]
    fn test_get_end_node_returns_end() {
        let node = get_end_node();
        assert_eq!(node.as_str(), END);
    }

    #[test]
    fn test_get_end_node_returns_same_arc() {
        // Multiple calls should return the same Arc (cached)
        let node1 = get_end_node();
        let node2 = get_end_node();
        assert!(Arc::ptr_eq(&node1, &node2));
    }

    #[test]
    fn test_get_end_node_is_arc_string() {
        let node = get_end_node();
        // Verify it's actually an Arc<String> containing END
        let string_ref: &String = &node;
        assert_eq!(string_ref, END);
    }

    // =========================================================================
    // NextNodes Enum Tests
    // =========================================================================

    #[test]
    fn test_next_nodes_single_variant() {
        let node_name = Arc::new("test_node".to_string());
        let next = NextNodes::Single(node_name.clone());

        match next {
            NextNodes::Single(n) => assert_eq!(n.as_str(), "test_node"),
            NextNodes::Parallel(_) => panic!("Expected Single variant"),
        }
    }

    #[test]
    fn test_next_nodes_parallel_variant() {
        let nodes = Arc::new(vec!["node1".to_string(), "node2".to_string()]);
        let next = NextNodes::Parallel(nodes.clone());

        match next {
            NextNodes::Single(_) => panic!("Expected Parallel variant"),
            NextNodes::Parallel(n) => {
                assert_eq!(n.len(), 2);
                assert_eq!(n[0], "node1");
                assert_eq!(n[1], "node2");
            }
        }
    }

    #[test]
    fn test_next_nodes_parallel_empty() {
        let nodes = Arc::new(vec![]);
        let next = NextNodes::Parallel(nodes);

        match next {
            NextNodes::Parallel(n) => assert!(n.is_empty()),
            _ => panic!("Expected Parallel variant"),
        }
    }

    // =========================================================================
    // find_next_nodes_static Tests
    // =========================================================================

    #[test]
    fn test_find_next_nodes_static_simple_edge() {
        let state = ExecTestState::default();
        let edges = vec![Edge {
            from: Arc::new("node1".to_string()),
            to: Arc::new("node2".to_string()),
        }];
        let conditional_edges: Vec<Arc<crate::edge::ConditionalEdge<ExecTestState>>> = vec![];
        let parallel_edges: Vec<ParallelEdge> = vec![];

        let result =
            CompiledGraph::find_next_nodes_static("node1", &state, &edges, &conditional_edges, &parallel_edges);

        assert!(result.is_ok());
        let (next_nodes, edge_type) = result.unwrap();
        match next_nodes {
            NextNodes::Single(n) => assert_eq!(n.as_str(), "node2"),
            _ => panic!("Expected Single variant"),
        }
        assert!(matches!(edge_type, EdgeType::Simple));
    }

    #[test]
    fn test_find_next_nodes_static_parallel_edge() {
        let state = ExecTestState::default();
        let edges: Vec<Edge> = vec![];
        let conditional_edges: Vec<Arc<crate::edge::ConditionalEdge<ExecTestState>>> = vec![];
        let parallel_edges = vec![ParallelEdge {
            from: Arc::new("start".to_string()),
            to: Arc::new(vec!["branch1".to_string(), "branch2".to_string()]),
        }];

        let result =
            CompiledGraph::find_next_nodes_static("start", &state, &edges, &conditional_edges, &parallel_edges);

        assert!(result.is_ok());
        let (next_nodes, edge_type) = result.unwrap();
        match next_nodes {
            NextNodes::Parallel(n) => {
                assert_eq!(n.len(), 2);
                assert!(n.contains(&"branch1".to_string()));
                assert!(n.contains(&"branch2".to_string()));
            }
            _ => panic!("Expected Parallel variant"),
        }
        assert!(matches!(edge_type, EdgeType::Parallel));
    }

    #[test]
    fn test_find_next_nodes_static_no_edge_implicit_end() {
        let state = ExecTestState::default();
        let edges: Vec<Edge> = vec![];
        let conditional_edges: Vec<Arc<crate::edge::ConditionalEdge<ExecTestState>>> = vec![];
        let parallel_edges: Vec<ParallelEdge> = vec![];

        let result =
            CompiledGraph::find_next_nodes_static("orphan", &state, &edges, &conditional_edges, &parallel_edges);

        assert!(result.is_ok());
        let (next_nodes, edge_type) = result.unwrap();
        match next_nodes {
            NextNodes::Single(n) => assert_eq!(n.as_str(), END),
            _ => panic!("Expected Single variant"),
        }
        assert!(matches!(edge_type, EdgeType::Simple));
    }

    #[test]
    fn test_find_next_nodes_static_parallel_priority_over_simple() {
        // Parallel edges should take priority over simple edges
        let state = ExecTestState::default();
        let edges = vec![Edge {
            from: Arc::new("node1".to_string()),
            to: Arc::new("simple_target".to_string()),
        }];
        let conditional_edges: Vec<Arc<crate::edge::ConditionalEdge<ExecTestState>>> = vec![];
        let parallel_edges = vec![ParallelEdge {
            from: Arc::new("node1".to_string()),
            to: Arc::new(vec!["parallel1".to_string(), "parallel2".to_string()]),
        }];

        let result =
            CompiledGraph::find_next_nodes_static("node1", &state, &edges, &conditional_edges, &parallel_edges);

        assert!(result.is_ok());
        let (next_nodes, edge_type) = result.unwrap();
        // Parallel should win over simple
        match next_nodes {
            NextNodes::Parallel(n) => assert_eq!(n.len(), 2),
            _ => panic!("Expected Parallel variant (parallel priority over simple)"),
        }
        assert!(matches!(edge_type, EdgeType::Parallel));
    }

    // =========================================================================
    // merge_parallel_custom Tests
    // =========================================================================

    #[tokio::test]
    async fn test_merge_parallel_custom_single_state() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("test");
        graph.add_edge("test", END);
        let compiled = graph.compile().unwrap();

        let states = vec![ExecTestState {
            values: vec![1, 2],
            counter: 5,
        }];

        let result = compiled.merge_parallel_custom(states, |base, other| {
            base.values.extend(other.values.clone());
            base.counter += other.counter;
        });

        assert!(result.is_ok());
        let merged = result.unwrap();
        assert_eq!(merged.values, vec![1, 2]);
        assert_eq!(merged.counter, 5);
    }

    #[tokio::test]
    async fn test_merge_parallel_custom_multiple_states() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("test");
        graph.add_edge("test", END);
        let compiled = graph.compile().unwrap();

        let states = vec![
            ExecTestState {
                values: vec![1, 2],
                counter: 10,
            },
            ExecTestState {
                values: vec![3, 4],
                counter: 20,
            },
            ExecTestState {
                values: vec![5],
                counter: 30,
            },
        ];

        let result = compiled.merge_parallel_custom(states, |base, other| {
            base.values.extend(other.values.clone());
            base.counter += other.counter;
        });

        assert!(result.is_ok());
        let merged = result.unwrap();
        assert_eq!(merged.values, vec![1, 2, 3, 4, 5]);
        assert_eq!(merged.counter, 60);
    }

    #[tokio::test]
    async fn test_merge_parallel_custom_empty_states_error() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("test");
        graph.add_edge("test", END);
        let compiled = graph.compile().unwrap();

        let states: Vec<ExecTestState> = vec![];

        let result = compiled.merge_parallel_custom(states, |base, other| {
            base.values.extend(other.values.clone());
        });

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InternalExecutionError(_)));
    }

    // =========================================================================
    // merge_with_mergeable Tests
    // =========================================================================

    #[tokio::test]
    async fn test_merge_with_mergeable_single_state() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("test");
        graph.add_edge("test", END);
        let compiled = graph.compile().unwrap();

        let states = vec![ExecTestState {
            values: vec![1, 2, 3],
            counter: 100,
        }];

        let result = compiled.merge_with_mergeable(states);

        assert!(result.is_ok());
        let merged = result.unwrap();
        assert_eq!(merged.values, vec![1, 2, 3]);
        assert_eq!(merged.counter, 100);
    }

    #[tokio::test]
    async fn test_merge_with_mergeable_multiple_states() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("test");
        graph.add_edge("test", END);
        let compiled = graph.compile().unwrap();

        let states = vec![
            ExecTestState {
                values: vec![1],
                counter: 10,
            },
            ExecTestState {
                values: vec![2],
                counter: 20,
            },
        ];

        let result = compiled.merge_with_mergeable(states);

        assert!(result.is_ok());
        let merged = result.unwrap();
        // Uses MergeableState::merge which extends values and adds counters
        assert_eq!(merged.values, vec![1, 2]);
        assert_eq!(merged.counter, 30);
    }

    #[tokio::test]
    async fn test_merge_with_mergeable_empty_states_error() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("test");
        graph.add_edge("test", END);
        let compiled = graph.compile().unwrap();

        let states: Vec<ExecTestState> = vec![];

        let result = compiled.merge_with_mergeable(states);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InternalExecutionError(_)));
    }

    // =========================================================================
    // node_count and edge_count Tests
    // =========================================================================

    #[tokio::test]
    async fn test_node_count_empty_graph() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        // Need at least one node to compile
        graph.add_node_from_fn("single", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("single");
        graph.add_edge("single", END);
        let compiled = graph.compile().unwrap();

        assert_eq!(compiled.node_count(), 1);
    }

    #[tokio::test]
    async fn test_node_count_multiple_nodes() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node3", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node1");
        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        let compiled = graph.compile().unwrap();

        assert_eq!(compiled.node_count(), 3);
    }

    #[tokio::test]
    async fn test_edge_count_simple_edges() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node1");
        graph.add_edge("node1", "node2");
        graph.add_edge("node2", END);
        let compiled = graph.compile().unwrap();

        assert_eq!(compiled.edge_count(), 2);
    }

    #[tokio::test]
    async fn test_edge_count_with_parallel() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("branch1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("branch2", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");
        graph.add_parallel_edges("start", vec!["branch1".to_string(), "branch2".to_string()]);
        graph.add_edge("branch1", END);
        graph.add_edge("branch2", END);
        // Must use compile_with_merge() for graphs with parallel edges
        let compiled = graph.compile_with_merge().unwrap();

        // 1 parallel edge + 2 simple edges = 3
        assert_eq!(compiled.edge_count(), 3);
    }

    #[tokio::test]
    async fn test_edge_count_with_conditional() {
        use std::collections::HashMap;

        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("option1", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("option2", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("start");

        let mut routes = HashMap::new();
        routes.insert("a".to_string(), "option1".to_string());
        routes.insert("b".to_string(), "option2".to_string());

        graph.add_conditional_edges("start", |_state: &ExecTestState| "a".to_string(), routes);
        graph.add_edge("option1", END);
        graph.add_edge("option2", END);
        let compiled = graph.compile().unwrap();

        // 1 conditional edge + 2 simple edges = 3
        assert_eq!(compiled.edge_count(), 3);
    }

    // =========================================================================
    // entry_point Tests
    // =========================================================================

    #[tokio::test]
    async fn test_entry_point_returns_correct_node() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("my_entry", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("my_entry");
        graph.add_edge("my_entry", END);
        let compiled = graph.compile().unwrap();

        assert_eq!(compiled.entry_point(), "my_entry");
    }

    #[tokio::test]
    async fn test_entry_point_different_node() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();
        graph.add_node_from_fn("node_a", |state| Box::pin(async move { Ok(state) }));
        graph.add_node_from_fn("node_b", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node_b");
        graph.add_edge("node_b", "node_a");
        graph.add_edge("node_a", END);
        let compiled = graph.compile().unwrap();

        assert_eq!(compiled.entry_point(), "node_b");
    }

    // =========================================================================
    // DEFAULT_GRAPH_TIMEOUT and DEFAULT_NODE_TIMEOUT Tests
    // =========================================================================

    #[test]
    fn test_default_graph_timeout_is_1_hour() {
        // Default graph timeout is 1 hour (3600 seconds)
        assert_eq!(DEFAULT_GRAPH_TIMEOUT, Duration::from_secs(3600));
    }

    #[test]
    fn test_default_node_timeout_is_5_minutes() {
        // Default node timeout is 5 minutes (300 seconds)
        assert_eq!(DEFAULT_NODE_TIMEOUT, Duration::from_secs(300));
    }

    // =========================================================================
    // ExecutionResult Tests
    // =========================================================================

    #[test]
    fn test_execution_result_state_method() {
        let result = ExecutionResult {
            final_state: ExecTestState {
                values: vec![1, 2, 3],
                counter: 42,
            },
            nodes_executed: vec!["node1".to_string(), "node2".to_string()],
            interrupted_at: None,
            next_nodes: vec![],
        };

        let state = result.state();
        assert_eq!(state.values, vec![1, 2, 3]);
        assert_eq!(state.counter, 42);
    }

    #[test]
    fn test_execution_result_nodes_executed() {
        let result = ExecutionResult {
            final_state: ExecTestState::default(),
            nodes_executed: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            interrupted_at: None,
            next_nodes: vec![],
        };

        assert_eq!(result.nodes_executed.len(), 3);
        assert_eq!(result.nodes_executed[0], "a");
        assert_eq!(result.nodes_executed[1], "b");
        assert_eq!(result.nodes_executed[2], "c");
    }

    #[test]
    fn test_execution_result_interrupted_at() {
        let result = ExecutionResult {
            final_state: ExecTestState::default(),
            nodes_executed: vec!["node1".to_string()],
            interrupted_at: Some("node1".to_string()),
            next_nodes: vec!["node2".to_string()],
        };

        assert_eq!(result.interrupted_at, Some("node1".to_string()));
        assert!(!result.next_nodes.is_empty());
    }

    #[test]
    fn test_execution_result_not_interrupted() {
        let result = ExecutionResult {
            final_state: ExecTestState::default(),
            nodes_executed: vec!["node1".to_string()],
            interrupted_at: None,
            next_nodes: vec![],
        };

        assert!(result.interrupted_at.is_none());
        assert!(result.next_nodes.is_empty());
    }

    // =========================================================================
    // Integration Tests (simple async execution)
    // =========================================================================

    #[tokio::test]
    async fn test_invoke_simple_graph() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();

        graph.add_node_from_fn("increment", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.values.push(state.counter);
                Ok(state)
            })
        });

        graph.set_entry_point("increment");
        graph.add_edge("increment", END);

        let compiled = graph.compile().unwrap();
        let result = compiled.invoke(ExecTestState::default()).await.unwrap();

        assert_eq!(result.nodes_executed.len(), 1);
        assert_eq!(result.nodes_executed[0], "increment");
        assert_eq!(result.final_state.counter, 1);
        assert_eq!(result.final_state.values, vec![1]);
    }

    #[tokio::test]
    async fn test_invoke_two_node_chain() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();

        graph.add_node_from_fn("first", |mut state| {
            Box::pin(async move {
                state.counter = 10;
                Ok(state)
            })
        });

        graph.add_node_from_fn("second", |mut state| {
            Box::pin(async move {
                state.counter *= 2;
                Ok(state)
            })
        });

        graph.set_entry_point("first");
        graph.add_edge("first", "second");
        graph.add_edge("second", END);

        let compiled = graph.compile().unwrap();
        let result = compiled.invoke(ExecTestState::default()).await.unwrap();

        assert_eq!(result.nodes_executed.len(), 2);
        assert_eq!(result.final_state.counter, 20);
    }

    #[tokio::test]
    async fn test_invoke_preserves_initial_state_values() {
        let mut graph: StateGraph<ExecTestState> = StateGraph::new();

        graph.add_node_from_fn("append", |mut state| {
            Box::pin(async move {
                state.values.push(999);
                Ok(state)
            })
        });

        graph.set_entry_point("append");
        graph.add_edge("append", END);

        let compiled = graph.compile().unwrap();
        let initial = ExecTestState {
            values: vec![1, 2, 3],
            counter: 0,
        };
        let result = compiled.invoke(initial).await.unwrap();

        assert_eq!(result.final_state.values, vec![1, 2, 3, 999]);
    }
}
