//! Integration tests for DashFlow Streaming callback
//!
//! These tests require Kafka to be running. Start with:
//! ```bash
//! docker-compose -f docker-compose-kafka.yml up -d
//! ```

#![cfg(feature = "dashstream")]
#![allow(clippy::expect_used)]

use dashflow::{DashStreamCallback, DashStreamConfig, Error, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TestState {
    messages: Vec<String>,
    counter: i32,
}

impl MergeableState for TestState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.counter = self.counter.max(other.counter);
    }
}

async fn increment_node(mut state: TestState) -> Result<TestState, Error> {
    state.counter += 1;
    state
        .messages
        .push(format!("Incremented to {}", state.counter));
    Ok(state)
}

async fn double_node(mut state: TestState) -> Result<TestState, Error> {
    state.counter *= 2;
    state.messages.push(format!("Doubled to {}", state.counter));
    Ok(state)
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_dashstream_callback_basic() {
    // Create DashStream callback
    let callback = DashStreamCallback::<TestState>::new(
        "localhost:9092",
        "test-dashstream",
        "test-tenant",
        &format!("test-thread-{}", uuid::Uuid::new_v4()),
    )
    .await
    .expect("Failed to create DashStream callback");

    // Create a simple graph
    let mut graph = StateGraph::new();
    graph.add_node_from_fn("increment", |state: TestState| {
        Box::pin(increment_node(state))
    });
    graph.add_node_from_fn("double", |state: TestState| Box::pin(double_node(state)));
    graph.add_edge("increment", "double");
    graph.add_edge("double", END);
    graph.set_entry_point("increment");

    // Clone callback to keep a reference for flushing
    let callback_for_flush = callback.clone();

    let compiled = graph
        .compile()
        .expect("Failed to compile graph")
        .with_callback(callback);

    // Initial state
    let initial_state = TestState {
        messages: vec![],
        counter: 5,
    };

    // Invoke
    let result = compiled
        .invoke(initial_state)
        .await
        .expect("Failed to invoke graph")
        .final_state;

    // Verify result
    assert_eq!(result.counter, 12); // (5 + 1) * 2 = 12
    assert_eq!(result.messages.len(), 2);

    // Flush to ensure all messages are sent
    callback_for_flush.flush().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_dashstream_callback_with_config() {
    let config = DashStreamConfig {
        bootstrap_servers: "localhost:9092".to_string(),
        topic: "test-dashstream-custom".to_string(),
        tenant_id: "custom-tenant".to_string(),
        thread_id: format!("custom-thread-{}", uuid::Uuid::new_v4()),
        enable_state_diff: true,
        compression_threshold: 256,
        max_state_diff_size: dashflow::dashstream_callback::DEFAULT_MAX_STATE_DIFF_SIZE,
        ..Default::default()
    };

    let callback = DashStreamCallback::<TestState>::with_config(config)
        .await
        .expect("Failed to create DashStream callback");

    // Clone callback to keep a reference for flushing
    let callback_for_flush = callback.clone();

    // Create a simple graph
    let mut graph = StateGraph::new();
    graph.add_node_from_fn("increment", |state: TestState| {
        Box::pin(increment_node(state))
    });
    graph.add_edge("increment", END);
    graph.set_entry_point("increment");

    let compiled = graph
        .compile()
        .expect("Failed to compile graph")
        .with_callback(callback);

    let initial_state = TestState {
        messages: vec![],
        counter: 10,
    };

    let result = compiled
        .invoke(initial_state)
        .await
        .expect("Failed to invoke graph")
        .final_state;

    assert_eq!(result.counter, 11);

    callback_for_flush.flush().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_dashstream_callback_multiple_executions() {
    let callback = DashStreamCallback::<TestState>::new(
        "localhost:9092",
        "test-dashstream",
        "test-tenant",
        &format!("test-thread-{}", uuid::Uuid::new_v4()),
    )
    .await
    .expect("Failed to create DashStream callback");

    // Store callback in Arc to share across invocations
    let callback_arc = std::sync::Arc::new(callback);

    let mut graph = StateGraph::new();
    graph.add_node_from_fn("increment", |state: TestState| {
        Box::pin(increment_node(state))
    });
    graph.add_edge("increment", END);
    graph.set_entry_point("increment");

    let compiled = graph
        .compile()
        .expect("Failed to compile graph")
        .with_callback(callback_arc.as_ref().clone());

    // Execute multiple times with the same callback
    for i in 0..5 {
        let initial_state = TestState {
            messages: vec![],
            counter: i * 10,
        };

        let result = compiled
            .invoke(initial_state)
            .await
            .expect("Failed to invoke graph")
            .final_state;

        assert_eq!(result.counter, i * 10 + 1);
    }

    callback_arc.flush().await.expect("Failed to flush");
}
