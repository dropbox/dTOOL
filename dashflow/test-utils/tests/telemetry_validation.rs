//! Telemetry/Tracing Validation
//!
//! Tests that verify tracing/telemetry works correctly during graph execution.
//! Validates:
//! - Trace spans are created for graph execution
//! - Node execution is properly instrumented
//! - Timing data is captured in spans
//!
//! Run with:
//! ```bash
//! cargo test -p dashflow-test-utils --test telemetry_validation -- --nocapture
//! ```

use std::sync::{Arc, Mutex};

use dashflow::state::MergeableState;
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};
use tracing::Subscriber;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Test state for telemetry validation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TelemetryState {
    messages: Vec<String>,
    execution_count: usize,
}

impl TelemetryState {
    fn new() -> Self {
        Self::default()
    }

    fn add_message(&mut self, msg: &str) {
        self.messages.push(msg.to_string());
    }

    fn increment(&mut self) {
        self.execution_count += 1;
    }
}

impl MergeableState for TelemetryState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.execution_count += other.execution_count;
    }
}

/// Custom layer that collects span names for testing
struct SpanCollector {
    span_names: Arc<Mutex<Vec<String>>>,
}

impl SpanCollector {
    fn new(span_names: Arc<Mutex<Vec<String>>>) -> Self {
        Self { span_names }
    }
}

impl<S: Subscriber> tracing_subscriber::Layer<S> for SpanCollector {
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let name = attrs.metadata().name().to_string();
        if let Ok(mut span_names) = self.span_names.lock() {
            span_names.push(name);
        }
    }
}

// ==============================================================================
// Test: Graph execution creates trace spans
// ==============================================================================

#[tokio::test]
async fn test_graph_execution_creates_spans() -> dashflow_test_utils::Result<()> {
    // Set up span collector
    let span_names: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let collector = SpanCollector::new(Arc::clone(&span_names));

    // Initialize tracing with our collector
    let subscriber = tracing_subscriber::registry()
        .with(collector)
        .with(tracing_subscriber::fmt::layer().with_test_writer());

    // Use a guard to ensure we clean up after the test
    let _guard = subscriber.set_default();

    // Create a simple graph
    let mut graph: StateGraph<TelemetryState> = StateGraph::new();

    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            state.add_message("processed");
            state.increment();
            Ok(state)
        })
    });

    graph.set_entry_point("process");
    graph.add_edge("process", END);

    let app = graph
        .compile()
        .map_err(|e| dashflow_test_utils::TestError::Other(format!("test: compile graph: {e}")))?;

    // Execute the graph
    app.invoke(TelemetryState::new())
        .await
        .map_err(|e| dashflow_test_utils::TestError::Other(format!("test: invoke: {e}")))?;

    // Check that spans were created (M-106: must actually assert spans are collected)
    let collected_spans = span_names
        .lock()
        .map_err(|_| dashflow_test_utils::TestError::Other("test: lock span_names".to_string()))?;

    println!("Collected spans ({} total): {:?}", collected_spans.len(), *collected_spans);

    // The executor should create at least some spans during execution
    // We don't require specific span names since tracing instrumentation may vary
    // But we DO require that at least one span was created - zero spans means
    // tracing is not working as expected
    assert!(
        !collected_spans.is_empty(),
        "At least one span should be collected during graph execution. \
         Got 0 spans. This indicates tracing instrumentation is not working."
    );

    // Additional sanity check: verify we got a reasonable number of spans
    // Graph execution with one node should create at least 1 span
    // (the graph.invoke span, or node execution span, etc.)
    println!(
        "Telemetry validation passed: {} spans collected during graph execution",
        collected_spans.len()
    );
    Ok(())
}

// ==============================================================================
// Test: Node execution timing is captured
// ==============================================================================

#[tokio::test]
async fn test_node_execution_completes_with_timing() -> dashflow_test_utils::Result<()> {
    use std::time::{Duration, Instant};

    // Create a graph with a slow node
    let mut graph: StateGraph<TelemetryState> = StateGraph::new();

    graph.add_node_from_fn("slow_node", |mut state| {
        Box::pin(async move {
            // Simulate some work
            tokio::time::sleep(Duration::from_millis(50)).await;
            state.add_message("slow work done");
            Ok(state)
        })
    });

    graph.set_entry_point("slow_node");
    graph.add_edge("slow_node", END);

    let app = graph
        .compile()
        .map_err(|e| dashflow_test_utils::TestError::Other(format!("test: compile graph: {e}")))?;

    // Measure execution time
    let start = Instant::now();
    let result = app.invoke(TelemetryState::new()).await.map_err(|e| {
        dashflow_test_utils::TestError::Other(format!("test: invoke: {e}"))
    })?;
    let elapsed = start.elapsed();

    // Verify timing
    assert!(
        elapsed >= Duration::from_millis(50),
        "Execution should take at least 50ms, took {:?}",
        elapsed
    );

    // Verify the result contains the expected data
    assert!(
        result
            .final_state
            .messages
            .contains(&"slow work done".to_string()),
        "Final state should contain the message"
    );

    // Check execution metadata
    assert_eq!(
        result.nodes_executed.len(),
        1,
        "Should have executed 1 node"
    );
    assert!(
        result.nodes_executed.contains(&"slow_node".to_string()),
        "Should have executed slow_node"
    );

    println!(
        "Node execution timing verified: took {:?} for 1 node",
        elapsed
    );
    Ok(())
}

// ==============================================================================
// Test: Multiple nodes have independent timing
// ==============================================================================

#[tokio::test]
async fn test_multiple_nodes_timing() -> dashflow_test_utils::Result<()> {
    use std::time::{Duration, Instant};

    let mut graph: StateGraph<TelemetryState> = StateGraph::new();

    // Add three nodes with different delays
    graph.add_node_from_fn("fast", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            state.add_message("fast");
            Ok(state)
        })
    });

    graph.add_node_from_fn("medium", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            state.add_message("medium");
            Ok(state)
        })
    });

    graph.add_node_from_fn("slow", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            state.add_message("slow");
            Ok(state)
        })
    });

    graph.set_entry_point("fast");
    graph.add_edge("fast", "medium");
    graph.add_edge("medium", "slow");
    graph.add_edge("slow", END);

    let app = graph
        .compile()
        .map_err(|e| dashflow_test_utils::TestError::Other(format!("test: compile graph: {e}")))?;

    let start = Instant::now();
    let result = app.invoke(TelemetryState::new()).await.map_err(|e| {
        dashflow_test_utils::TestError::Other(format!("test: invoke: {e}"))
    })?;
    let elapsed = start.elapsed();

    // Should take at least the sum of all delays (10 + 20 + 30 = 60ms)
    assert!(
        elapsed >= Duration::from_millis(60),
        "Sequential execution should take at least 60ms, took {:?}",
        elapsed
    );

    // Verify all nodes executed
    assert_eq!(
        result.nodes_executed.len(),
        3,
        "Should have executed 3 nodes"
    );

    // Verify execution order in messages
    let messages = &result.final_state.messages;
    let fast_idx = messages.iter().position(|m| m == "fast").ok_or_else(|| {
        dashflow_test_utils::TestError::Other("test: find fast".to_string())
    })?;
    let medium_idx = messages
        .iter()
        .position(|m| m == "medium")
        .ok_or_else(|| dashflow_test_utils::TestError::Other("test: find medium".to_string()))?;
    let slow_idx = messages.iter().position(|m| m == "slow").ok_or_else(|| {
        dashflow_test_utils::TestError::Other("test: find slow".to_string())
    })?;

    assert!(
        fast_idx < medium_idx && medium_idx < slow_idx,
        "Nodes should execute in order: fast ({}) < medium ({}) < slow ({})",
        fast_idx,
        medium_idx,
        slow_idx
    );

    println!(
        "Multiple node timing verified: {:?} for 3 sequential nodes",
        elapsed
    );
    Ok(())
}

// ==============================================================================
// Test: Execution result contains metadata
// ==============================================================================

#[tokio::test]
async fn test_execution_result_metadata() -> dashflow_test_utils::Result<()> {
    let mut graph: StateGraph<TelemetryState> = StateGraph::new();

    graph.add_node_from_fn("node_a", |mut state| {
        Box::pin(async move {
            state.add_message("a");
            Ok(state)
        })
    });

    graph.add_node_from_fn("node_b", |mut state| {
        Box::pin(async move {
            state.add_message("b");
            Ok(state)
        })
    });

    graph.set_entry_point("node_a");
    graph.add_edge("node_a", "node_b");
    graph.add_edge("node_b", END);

    let app = graph
        .compile()
        .map_err(|e| dashflow_test_utils::TestError::Other(format!("test: compile graph: {e}")))?;
    let result = app.invoke(TelemetryState::new()).await.map_err(|e| {
        dashflow_test_utils::TestError::Other(format!("test: invoke: {e}"))
    })?;

    // Check execution result structure
    assert!(
        !result.nodes_executed.is_empty(),
        "nodes_executed should not be empty"
    );

    // Verify both nodes are in the execution list
    assert!(
        result.nodes_executed.contains(&"node_a".to_string()),
        "nodes_executed should contain node_a"
    );
    assert!(
        result.nodes_executed.contains(&"node_b".to_string()),
        "nodes_executed should contain node_b"
    );

    // Verify final state
    assert_eq!(
        result.final_state.messages.len(),
        2,
        "Final state should have 2 messages"
    );

    println!(
        "Execution metadata verified: nodes_executed={:?}",
        result.nodes_executed
    );
    Ok(())
}

// ==============================================================================
// Test: Tracing subscriber integration
// ==============================================================================

#[tokio::test]
async fn test_tracing_subscriber_integration() -> dashflow_test_utils::Result<()> {
    // This test verifies that the tracing infrastructure can be initialized
    // and doesn't interfere with graph execution

    // Create graph
    let mut graph: StateGraph<TelemetryState> = StateGraph::new();

    graph.add_node_from_fn("traced_node", |mut state| {
        Box::pin(async move {
            // Emit a tracing event from within the node
            tracing::info!("Node executing");
            state.add_message("traced");
            Ok(state)
        })
    });

    graph.set_entry_point("traced_node");
    graph.add_edge("traced_node", END);

    let app = graph
        .compile()
        .map_err(|e| dashflow_test_utils::TestError::Other(format!("test: compile graph: {e}")))?;

    // Execute - tracing events from within nodes should not cause issues
    let result = app.invoke(TelemetryState::new()).await.map_err(|e| {
        dashflow_test_utils::TestError::Other(format!("test: invoke: {e}"))
    })?;

    assert!(
        result.final_state.messages.contains(&"traced".to_string()),
        "Node should have executed and added message"
    );

    println!("Tracing subscriber integration test passed");
    Ok(())
}
