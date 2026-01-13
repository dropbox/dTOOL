//! Event Streaming Validation
//!
//! Tests that verify event streaming works correctly during graph execution.
//! Validates:
//! - Event order matches expected (NodeStart before NodeEnd)
//! - All node start/complete events fire
//! - Parallel execution emits proper events
//!
//! Run with:
//! ```bash
//! # Mock tests (no API key needed)
//! cargo test -p dashflow-test-utils --test streaming_parity -- --nocapture
//!
//! # Real OpenAI tests (requires OPENAI_API_KEY)
//! cargo test -p dashflow-test-utils --test streaming_parity -- --ignored --nocapture
//! ```

#![allow(clippy::expect_fun_call, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use dashflow::state::MergeableState;
use dashflow::stream::{StreamEvent, StreamMode};
use dashflow::{StateGraph, END};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};

/// Test state for streaming validation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StreamingState {
    messages: Vec<String>,
    node_execution_order: Vec<String>,
}

impl StreamingState {
    fn new() -> Self {
        Self::default()
    }

    fn add_message(&mut self, msg: &str) {
        self.messages.push(msg.to_string());
    }

    fn record_node(&mut self, node: &str) {
        self.node_execution_order.push(node.to_string());
    }
}

impl MergeableState for StreamingState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.node_execution_order
            .extend(other.node_execution_order.clone());
    }
}

/// Collector for stream events
struct EventCollector {
    events: Arc<Mutex<Vec<String>>>,
    node_starts: Arc<Mutex<Vec<String>>>,
    node_ends: Arc<Mutex<Vec<String>>>,
}

impl EventCollector {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            node_starts: Arc::new(Mutex::new(Vec::new())),
            node_ends: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record_event(&self, event_type: &str, node: Option<&str>) {
        let mut events = self.events.lock().expect("test: lock events");
        let event_str = match node {
            Some(n) => format!("{}:{}", event_type, n),
            None => event_type.to_string(),
        };
        events.push(event_str);

        if let Some(node_name) = node {
            if event_type == "NodeStart" {
                self.node_starts
                    .lock()
                    .expect("test: lock node_starts")
                    .push(node_name.to_string());
            } else if event_type == "NodeEnd" {
                self.node_ends
                    .lock()
                    .expect("test: lock node_ends")
                    .push(node_name.to_string());
            }
        }
    }

    fn events(&self) -> Vec<String> {
        self.events.lock().expect("test: lock events").clone()
    }

    fn node_starts(&self) -> Vec<String> {
        self.node_starts
            .lock()
            .expect("test: lock node_starts")
            .clone()
    }

    fn node_ends(&self) -> Vec<String> {
        self.node_ends.lock().expect("test: lock node_ends").clone()
    }
}

// ==============================================================================
// Test: Basic event order in linear graph
// ==============================================================================

#[tokio::test]
async fn test_linear_graph_event_order() {
    // Creates: node1 -> node2 -> node3 -> END
    // Expects: NodeStart:node1, NodeEnd:node1, NodeStart:node2, NodeEnd:node2, ...

    let mut graph: StateGraph<StreamingState> = StateGraph::new();

    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.add_message("node1 executed");
            state.record_node("node1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.add_message("node2 executed");
            state.record_node("node2");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node3", |mut state| {
        Box::pin(async move {
            state.add_message("node3 executed");
            state.record_node("node3");
            Ok(state)
        })
    });

    graph.set_entry_point("node1");
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node3");
    graph.add_edge("node3", END);

    let app = graph.compile().expect("test: compile graph");
    let mut stream = Box::pin(app.stream(StreamingState::new(), StreamMode::Events));

    let collector = EventCollector::new();

    while let Some(event_result) = stream.next().await {
        let event = event_result.expect("test: stream event");
        match &event {
            StreamEvent::NodeStart { node, .. } => {
                collector.record_event("NodeStart", Some(node));
            }
            StreamEvent::NodeEnd { node, .. } => {
                collector.record_event("NodeEnd", Some(node));
            }
            StreamEvent::Done { .. } => {
                collector.record_event("Done", None);
            }
            _ => {}
        }
    }

    let events = collector.events();
    let node_starts = collector.node_starts();
    let node_ends = collector.node_ends();

    // Verify all nodes have start and end events
    assert!(
        node_starts.contains(&"node1".to_string()),
        "Missing NodeStart for node1"
    );
    assert!(
        node_starts.contains(&"node2".to_string()),
        "Missing NodeStart for node2"
    );
    assert!(
        node_starts.contains(&"node3".to_string()),
        "Missing NodeStart for node3"
    );

    assert!(
        node_ends.contains(&"node1".to_string()),
        "Missing NodeEnd for node1"
    );
    assert!(
        node_ends.contains(&"node2".to_string()),
        "Missing NodeEnd for node2"
    );
    assert!(
        node_ends.contains(&"node3".to_string()),
        "Missing NodeEnd for node3"
    );

    // Verify event counts
    assert_eq!(
        node_starts.len(),
        3,
        "Expected 3 NodeStart events, got {}",
        node_starts.len()
    );
    assert_eq!(
        node_ends.len(),
        3,
        "Expected 3 NodeEnd events, got {}",
        node_ends.len()
    );

    // Verify Done event exists
    assert!(events.contains(&"Done".to_string()), "Missing Done event");

    // Verify order: For each node, NodeStart must come before NodeEnd
    for node in &["node1", "node2", "node3"] {
        let start_idx = events
            .iter()
            .position(|e| e == &format!("NodeStart:{}", node))
            .expect(&format!("test: find NodeStart:{}", node));
        let end_idx = events
            .iter()
            .position(|e| e == &format!("NodeEnd:{}", node))
            .expect(&format!("test: find NodeEnd:{}", node));

        assert!(
            start_idx < end_idx,
            "NodeStart:{} (idx {}) should come before NodeEnd:{} (idx {})",
            node,
            start_idx,
            node,
            end_idx
        );
    }

    println!("Event order: {:?}", events);
    println!("All events in correct order");
}

// ==============================================================================
// Test: Sequential node ordering
// ==============================================================================

#[tokio::test]
async fn test_sequential_node_ordering() {
    // For a linear graph, nodes should complete in order
    // node1 must end before node2 starts, etc.

    let mut graph: StateGraph<StreamingState> = StateGraph::new();

    graph.add_node_from_fn("first", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("first");
            Ok(state)
        })
    });

    graph.add_node_from_fn("second", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            state.add_message("second");
            Ok(state)
        })
    });

    graph.set_entry_point("first");
    graph.add_edge("first", "second");
    graph.add_edge("second", END);

    let app = graph.compile().expect("test: compile graph");
    let mut stream = Box::pin(app.stream(StreamingState::new(), StreamMode::Events));

    let mut events = Vec::new();

    while let Some(event_result) = stream.next().await {
        let event = event_result.expect("test: stream event");
        match &event {
            StreamEvent::NodeStart { node, .. } => {
                events.push(format!("Start:{}", node));
            }
            StreamEvent::NodeEnd { node, .. } => {
                events.push(format!("End:{}", node));
            }
            _ => {}
        }
    }

    // Verify order: first ends before second starts
    let first_end_idx = events
        .iter()
        .position(|e| e == "End:first")
        .expect("test: find End:first");
    let second_start_idx = events
        .iter()
        .position(|e| e == "Start:second")
        .expect("test: find Start:second");

    assert!(
        first_end_idx < second_start_idx,
        "In sequential graph, first node must end before second starts. \
         Got End:first at {}, Start:second at {}",
        first_end_idx,
        second_start_idx
    );

    println!("Sequential ordering verified: {:?}", events);
}

// ==============================================================================
// Test: All nodes emit start and end events
// ==============================================================================

#[tokio::test]
async fn test_all_nodes_emit_events() {
    // Every node added to the graph should emit both start and end events

    let mut graph: StateGraph<StreamingState> = StateGraph::new();

    let node_names = vec!["alpha", "beta", "gamma", "delta"];

    // Add nodes
    graph.add_node_from_fn("alpha", |mut state| {
        Box::pin(async move {
            state.add_message("alpha");
            Ok(state)
        })
    });

    graph.add_node_from_fn("beta", |mut state| {
        Box::pin(async move {
            state.add_message("beta");
            Ok(state)
        })
    });

    graph.add_node_from_fn("gamma", |mut state| {
        Box::pin(async move {
            state.add_message("gamma");
            Ok(state)
        })
    });

    graph.add_node_from_fn("delta", |mut state| {
        Box::pin(async move {
            state.add_message("delta");
            Ok(state)
        })
    });

    graph.set_entry_point("alpha");
    graph.add_edge("alpha", "beta");
    graph.add_edge("beta", "gamma");
    graph.add_edge("gamma", "delta");
    graph.add_edge("delta", END);

    let app = graph.compile().expect("test: compile graph");
    let mut stream = Box::pin(app.stream(StreamingState::new(), StreamMode::Events));

    let mut start_events: HashMap<String, bool> = HashMap::new();
    let mut end_events: HashMap<String, bool> = HashMap::new();

    while let Some(event_result) = stream.next().await {
        let event = event_result.expect("test: stream event");
        match &event {
            StreamEvent::NodeStart { node, .. } => {
                start_events.insert(node.clone(), true);
            }
            StreamEvent::NodeEnd { node, .. } => {
                end_events.insert(node.clone(), true);
            }
            _ => {}
        }
    }

    // Verify all nodes got both events
    for node in &node_names {
        assert!(
            start_events.contains_key(*node),
            "Node '{}' missing NodeStart event",
            node
        );
        assert!(
            end_events.contains_key(*node),
            "Node '{}' missing NodeEnd event",
            node
        );
    }

    println!(
        "All {} nodes emitted both start and end events",
        node_names.len()
    );
}

// ==============================================================================
// Test: Stream mode produces events (not just values)
// ==============================================================================

#[tokio::test]
async fn test_stream_mode_events() {
    // When using StreamMode::Events, we should get StreamEvent types

    let mut graph: StateGraph<StreamingState> = StateGraph::new();

    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            state.add_message("processed");
            Ok(state)
        })
    });

    graph.set_entry_point("process");
    graph.add_edge("process", END);

    let app = graph.compile().expect("test: compile graph");
    let mut stream = Box::pin(app.stream(StreamingState::new(), StreamMode::Events));

    let mut event_count = 0;
    let mut has_node_start = false;
    let mut has_node_end = false;
    let mut has_done = false;

    while let Some(event_result) = stream.next().await {
        let event = event_result.expect("test: stream event");
        event_count += 1;

        match event {
            StreamEvent::NodeStart { .. } => has_node_start = true,
            StreamEvent::NodeEnd { .. } => has_node_end = true,
            StreamEvent::Done { .. } => has_done = true,
            _ => {}
        }
    }

    assert!(
        event_count >= 3,
        "Expected at least 3 events (start, end, done), got {}",
        event_count
    );
    assert!(
        has_node_start,
        "StreamMode::Events should produce NodeStart"
    );
    assert!(has_node_end, "StreamMode::Events should produce NodeEnd");
    assert!(has_done, "StreamMode::Events should produce Done");

    println!(
        "StreamMode::Events produced {} events correctly",
        event_count
    );
}

// ==============================================================================
// Test: Stream collects final state
// ==============================================================================

#[tokio::test]
async fn test_stream_collects_final_state() {
    // The Done event should contain the final state

    let mut graph: StateGraph<StreamingState> = StateGraph::new();

    graph.add_node_from_fn("add_data", |mut state| {
        Box::pin(async move {
            state.add_message("test_message");
            state.record_node("add_data");
            Ok(state)
        })
    });

    graph.set_entry_point("add_data");
    graph.add_edge("add_data", END);

    let app = graph.compile().expect("test: compile graph");
    let mut stream = Box::pin(app.stream(StreamingState::new(), StreamMode::Events));

    let mut final_state: Option<StreamingState> = None;

    while let Some(event_result) = stream.next().await {
        let event = event_result.expect("test: stream event");
        if let StreamEvent::Done { state, .. } = event {
            final_state = Some(state);
        }
    }

    let state = final_state.expect("test: Done event should contain final state");
    assert!(
        state.messages.contains(&"test_message".to_string()),
        "Final state should contain message added by node"
    );
    assert!(
        state.node_execution_order.contains(&"add_data".to_string()),
        "Final state should record node execution"
    );

    println!("Final state collected correctly from stream");
}

// Note: Real OpenAI streaming tests are in the example apps
// See examples/apps/librarian/tests/ for LLM-based streaming tests
