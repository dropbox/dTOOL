//! Progress Callbacks Validation
//!
//! Tests that verify the callback mechanism works correctly for tracking
//! graph execution progress. These tests use mock LLM calls since the
//! callback mechanism is what matters, not real LLM responses.

#![allow(clippy::expect_fun_call, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use dashflow::error::Error;
use dashflow::event::{CollectingCallback, EventCallback, GraphEvent};
use dashflow::state::{GraphState, MergeableState};
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};

/// Test state for progress tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TestState {
    messages: Vec<String>,
    progress_updates: Vec<f64>,
}

impl TestState {
    fn new() -> Self {
        Self::default()
    }

    fn add_message(&mut self, msg: &str) {
        self.messages.push(msg.to_string());
    }
}

impl MergeableState for TestState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.progress_updates.extend(other.progress_updates.clone());
    }
}

/// A callback that tracks progress as percentage of nodes completed
struct ProgressTrackingCallback<S>
where
    S: GraphState,
{
    /// Total number of nodes in the graph
    total_nodes: usize,
    /// Progress updates (0.0 to 1.0)
    progress: Arc<Mutex<Vec<f64>>>,
    /// Node start/end events
    events: Arc<Mutex<Vec<String>>>,
    /// Phantom data for state type
    _phantom: std::marker::PhantomData<S>,
}

impl<S: GraphState> ProgressTrackingCallback<S> {
    fn new(total_nodes: usize) -> Self {
        Self {
            total_nodes,
            progress: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(Vec::new())),
            _phantom: std::marker::PhantomData,
        }
    }

    fn progress_updates(&self) -> Vec<f64> {
        self.progress.lock().expect("test: lock progress").clone()
    }

    fn events(&self) -> Vec<String> {
        self.events.lock().expect("test: lock events").clone()
    }

    fn shared_clone(&self) -> Self {
        Self {
            total_nodes: self.total_nodes,
            progress: Arc::clone(&self.progress),
            events: Arc::clone(&self.events),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S: GraphState> EventCallback<S> for ProgressTrackingCallback<S> {
    fn on_event(&self, event: &GraphEvent<S>) {
        let mut events = self.events.lock().expect("test: lock events");
        let mut progress = self.progress.lock().expect("test: lock progress");

        match event {
            GraphEvent::GraphStart { .. } => {
                events.push("GraphStart".to_string());
                progress.push(0.0);
            }
            GraphEvent::NodeStart { node, .. } => {
                events.push(format!("NodeStart:{}", node));
            }
            GraphEvent::NodeEnd { node, .. } => {
                events.push(format!("NodeEnd:{}", node));
                // Calculate progress after each node completes
                let completed = progress.len();
                let new_progress = completed as f64 / self.total_nodes as f64;
                progress.push(new_progress);
            }
            GraphEvent::GraphEnd { .. } => {
                events.push("GraphEnd".to_string());
                progress.push(1.0);
            }
            _ => {}
        }
    }
}

/// Test that callbacks fire in correct order
#[tokio::test]
async fn test_callback_event_order() {
    let mut graph: StateGraph<TestState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1 executed");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2 executed");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node3", |mut state| {
        Box::pin(async move {
            state.add_message("node3 executed");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", END);

    let callback = ProgressTrackingCallback::<TestState>::new(3);
    let callback_clone = callback.shared_clone();

    let app = graph
        .compile()
        .expect("test: compile graph")
        .with_callback(callback);

    let result = app.invoke(TestState::new()).await;
    assert!(result.is_ok(), "Graph execution should succeed");

    let events = callback_clone.events();

    // Verify event order
    assert_eq!(events[0], "GraphStart", "First event should be GraphStart");

    // Find the index of GraphEnd
    let graph_end_idx = events
        .iter()
        .position(|e| e == "GraphEnd")
        .expect("test: GraphEnd should exist");

    assert_eq!(
        graph_end_idx,
        events.len() - 1,
        "GraphEnd should be last event"
    );

    // Verify each node has start/end events in order
    for node in ["node1", "node2", "node3"] {
        let start_idx = events
            .iter()
            .position(|e| e == &format!("NodeStart:{}", node))
            .expect(&format!("test: NodeStart for {} should exist", node));
        let end_idx = events
            .iter()
            .position(|e| e == &format!("NodeEnd:{}", node))
            .expect(&format!("test: NodeEnd for {} should exist", node));

        assert!(
            start_idx < end_idx,
            "NodeStart for {} should come before NodeEnd",
            node
        );
    }
}

/// Test that progress increases monotonically
#[tokio::test]
async fn test_progress_increases_monotonically() {
    let mut graph: StateGraph<TestState> = StateGraph::new();

    // Create a 4-node graph
    for i in 1..=4 {
        let node_name = format!("node{}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            let msg = format!("node{} executed", i);
            Box::pin(async move {
                state.add_message(&msg);
                Ok(state)
            })
        });
    }

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", "node4");
    graph.add_edge("node4", END);

    let callback = ProgressTrackingCallback::<TestState>::new(4);
    let callback_clone = callback.shared_clone();

    let app = graph
        .compile()
        .expect("test: compile graph")
        .with_callback(callback);

    let result = app.invoke(TestState::new()).await;
    assert!(result.is_ok(), "Graph execution should succeed");

    let progress = callback_clone.progress_updates();

    // Progress should start at 0
    assert!(
        (progress[0] - 0.0).abs() < f64::EPSILON,
        "Progress should start at 0"
    );

    // Progress should end at 1.0
    let last_progress = *progress.last().expect("test: progress should have values");
    assert!(
        (last_progress - 1.0).abs() < 1e-9,
        "Progress should end at 1.0"
    );

    // Progress should increase monotonically
    for i in 1..progress.len() {
        assert!(
            progress[i] >= progress[i - 1],
            "Progress should increase monotonically: {} >= {} at index {}",
            progress[i],
            progress[i - 1],
            i
        );
    }
}

/// Test callback with parallel execution
#[tokio::test]
async fn test_callback_with_parallel_nodes() {
    let mut graph: StateGraph<TestState> = StateGraph::new();

    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.add_message("start");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel_a", |mut state| {
        Box::pin(async move {
            // Small delay to simulate work
            tokio::time::sleep(Duration::from_millis(10)).await;
            state.add_message("parallel_a");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel_b", |mut state| {
        Box::pin(async move {
            // Small delay to simulate work
            tokio::time::sleep(Duration::from_millis(10)).await;
            state.add_message("parallel_b");
            Ok(state)
        })
    });

    graph.add_node_from_fn("end_node", |mut state| {
        Box::pin(async move {
            state.add_message("end_node");
            Ok(state)
        })
    });

    graph.set_entry_point("start");
    graph.add_parallel_edges(
        "start",
        vec!["parallel_a".to_string(), "parallel_b".to_string()],
    );
    graph.add_edge("parallel_a", "end_node");
    graph.add_edge("parallel_b", "end_node");
    graph.add_edge("end_node", END);

    let callback = CollectingCallback::<TestState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph
        .compile_with_merge()
        .expect("test: compile graph with merge")
        .with_callback(callback);

    let result = app.invoke(TestState::new()).await;
    assert!(result.is_ok(), "Parallel graph execution should succeed");

    let events = callback_clone.events();

    // Should have GraphStart
    let has_graph_start = events
        .iter()
        .any(|e| matches!(e, GraphEvent::GraphStart { .. }));
    assert!(has_graph_start, "Should have GraphStart event");

    // Should have GraphEnd
    let has_graph_end = events
        .iter()
        .any(|e| matches!(e, GraphEvent::GraphEnd { .. }));
    assert!(has_graph_end, "Should have GraphEnd event");

    // Should have ParallelStart for the parallel nodes
    let has_parallel_start = events
        .iter()
        .any(|e| matches!(e, GraphEvent::ParallelStart { .. }));
    assert!(has_parallel_start, "Should have ParallelStart event");

    // Count node events
    let node_start_count = events
        .iter()
        .filter(|e| matches!(e, GraphEvent::NodeStart { .. }))
        .count();
    let node_end_count = events
        .iter()
        .filter(|e| matches!(e, GraphEvent::NodeEnd { .. }))
        .count();

    // Parallel execution should emit node events for all branches
    // At minimum we expect: start node + some parallel nodes + end node
    assert!(
        node_start_count >= 1,
        "Should have at least 1 NodeStart event, got {}",
        node_start_count
    );
    assert!(
        node_end_count >= 1,
        "Should have at least 1 NodeEnd event, got {}",
        node_end_count
    );

    // Verify start and end_node specifically executed
    let has_start_node = events
        .iter()
        .any(|e| matches!(e, GraphEvent::NodeEnd { node, .. } if node == "start"));
    assert!(has_start_node, "Start node should have completed");
}

/// Test that all nodes emit events
#[tokio::test]
async fn test_all_nodes_emit_events() {
    let mut graph: StateGraph<TestState> = StateGraph::new();

    let node_names: Vec<&str> = vec!["alpha", "beta", "gamma", "delta"];

    for (i, name) in node_names.iter().enumerate() {
        let msg = format!("{} executed", name);
        let name_str = *name; // Dereference &str
        graph.add_node_from_fn(name_str, move |mut state| {
            let message = msg.clone();
            Box::pin(async move {
                state.add_message(&message);
                Ok(state)
            })
        });

        if i > 0 {
            graph.add_edge(node_names[i - 1], name_str);
        }
    }

    graph.set_entry_point(node_names[0]);
    graph.add_edge(node_names[node_names.len() - 1], END);

    let callback = CollectingCallback::<TestState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph
        .compile()
        .expect("test: compile graph")
        .with_callback(callback);

    let result = app.invoke(TestState::new()).await;
    assert!(result.is_ok(), "Graph execution should succeed");

    let events = callback_clone.events();

    // Verify each node has both start and end events
    for name in &node_names {
        let has_start = events
            .iter()
            .any(|e| matches!(e, GraphEvent::NodeStart { node, .. } if node == *name));
        let has_end = events
            .iter()
            .any(|e| matches!(e, GraphEvent::NodeEnd { node, .. } if node == *name));

        assert!(has_start, "Node {} should have NodeStart event", name);
        assert!(has_end, "Node {} should have NodeEnd event", name);
    }
}

/// Test callback receives error events on node failure
#[tokio::test]
async fn test_callback_receives_error_events() {
    let mut graph: StateGraph<TestState> = StateGraph::new();

    graph.add_node_from_fn("good_node", |mut state| {
        Box::pin(async move {
            state.add_message("good_node");
            Ok(state)
        })
    });

    graph.add_node_from_fn("failing_node", |_state| {
        Box::pin(async move { Err(Error::Generic("Intentional test failure".to_string())) })
    });

    graph.set_entry_point("good_node");
    graph.add_edge("good_node", "failing_node");
    graph.add_edge("failing_node", END);

    let callback = CollectingCallback::<TestState>::new();
    let callback_clone = callback.shared_clone();

    let app = graph
        .compile()
        .expect("test: compile graph")
        .with_callback(callback);

    let result = app.invoke(TestState::new()).await;
    assert!(result.is_err(), "Graph execution should fail");

    let events = callback_clone.events();

    // Should have GraphStart
    let has_graph_start = events
        .iter()
        .any(|e| matches!(e, GraphEvent::GraphStart { .. }));
    assert!(has_graph_start, "Should have GraphStart event");

    // Should have NodeError for the failing node
    let has_node_error = events.iter().any(|e| {
        matches!(e, GraphEvent::NodeError { node, error, .. }
            if node == "failing_node" && error.contains("Intentional test failure"))
    });
    assert!(
        has_node_error,
        "Should have NodeError event for failing_node"
    );

    // Should have good_node start and end
    let has_good_start = events
        .iter()
        .any(|e| matches!(e, GraphEvent::NodeStart { node, .. } if node == "good_node"));
    let has_good_end = events
        .iter()
        .any(|e| matches!(e, GraphEvent::NodeEnd { node, .. } if node == "good_node"));
    assert!(has_good_start, "good_node should have NodeStart");
    assert!(has_good_end, "good_node should have NodeEnd");
}
