//! Graph execution engine
//!
//! Executes computation graphs with real-time status updates.

use crate::{ComputationGraph, GraphError, GraphState, NodeId, NodeStatus, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Events emitted during graph execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionEvent {
    /// Execution started
    Started {
        graph_name: String,
    },
    /// Node execution started
    NodeStarted {
        node_id: NodeId,
        input_state: GraphState,
    },
    /// Node execution completed
    NodeCompleted {
        node_id: NodeId,
        output_state: GraphState,
        duration_ms: u64,
    },
    /// Node execution failed
    NodeFailed {
        node_id: NodeId,
        error: String,
    },
    /// State updated
    StateUpdated {
        state: GraphState,
    },
    /// Execution completed
    Completed {
        final_state: GraphState,
        total_duration_ms: u64,
    },
    /// Execution failed
    Failed {
        error: String,
    },
}

/// Configuration for graph execution
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Maximum number of parallel executions
    pub max_parallel: usize,
    /// Timeout per node (ms)
    pub node_timeout_ms: u64,
    /// Total execution timeout (ms)
    pub total_timeout_ms: u64,
    /// Whether to continue on node failure
    pub continue_on_failure: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_parallel: 4,
            node_timeout_ms: 30_000,
            total_timeout_ms: 300_000,
            continue_on_failure: false,
        }
    }
}

/// Graph execution engine
pub struct GraphExecutor {
    config: ExecutionConfig,
}

impl GraphExecutor {
    pub fn new(config: ExecutionConfig) -> Self {
        Self { config }
    }

    /// Execute a graph with the given initial state
    /// Returns a channel for receiving execution events
    pub async fn execute(
        &self,
        graph: &mut ComputationGraph,
        initial_state: GraphState,
    ) -> Result<mpsc::Receiver<ExecutionEvent>> {
        let (tx, rx) = mpsc::channel(100);

        // Get topological order
        let order = graph.topological_order()?;

        // Send start event
        let _ = tx.send(ExecutionEvent::Started {
            graph_name: graph.name.clone(),
        }).await;

        let mut state = initial_state;
        let start_time = std::time::Instant::now();

        for node_id in order {
            // Update node status
            if let Some(node) = graph.get_node_mut(&node_id) {
                node.status = NodeStatus::Running;
                node.timing = Some(crate::node::NodeTiming::start_now());
            }

            // Send node started event
            let _ = tx.send(ExecutionEvent::NodeStarted {
                node_id: node_id.clone(),
                input_state: state.clone(),
            }).await;

            // Execute node (placeholder - actual execution would call node handlers)
            let node_start = std::time::Instant::now();

            // Simulate execution - in real implementation, this would invoke
            // the node's handler function
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            let duration_ms = node_start.elapsed().as_millis() as u64;

            // Update node status
            if let Some(node) = graph.get_node_mut(&node_id) {
                node.status = NodeStatus::Success;
                if let Some(ref mut timing) = node.timing {
                    timing.complete();
                }
            }

            // Send node completed event
            let _ = tx.send(ExecutionEvent::NodeCompleted {
                node_id: node_id.clone(),
                output_state: state.clone(),
                duration_ms,
            }).await;
        }

        // Send completion event
        let _ = tx.send(ExecutionEvent::Completed {
            final_state: state,
            total_duration_ms: start_time.elapsed().as_millis() as u64,
        }).await;

        Ok(rx)
    }

    /// Execute a graph synchronously (blocking)
    pub fn execute_sync(
        &self,
        graph: &mut ComputationGraph,
        initial_state: GraphState,
    ) -> Result<GraphState> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| GraphError::ExecutionError {
                node: "runtime".to_string(),
                message: e.to_string(),
            })?;

        rt.block_on(async {
            let mut rx = self.execute(graph, initial_state).await?;
            let mut final_state = GraphState::new();

            while let Some(event) = rx.recv().await {
                match event {
                    ExecutionEvent::Completed { final_state: state, .. } => {
                        final_state = state;
                    }
                    ExecutionEvent::Failed { error } => {
                        return Err(GraphError::ExecutionError {
                            node: "unknown".to_string(),
                            message: error,
                        });
                    }
                    _ => {}
                }
            }

            Ok(final_state)
        })
    }
}

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new(ExecutionConfig::default())
    }
}
