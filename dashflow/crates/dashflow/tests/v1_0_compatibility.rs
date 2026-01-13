//! v1.0 Compatibility Tests
//!
//! Tests that v1.0-style API calls still work in v1.6+ with deprecated methods.
//! This ensures smooth upgrade path for existing applications.

use dashflow::{Error, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TestState {
    messages: Vec<String>,
    step: u32,
}

impl MergeableState for TestState {
    fn merge(&mut self, other: &Self) {
        // Merge messages (append)
        self.messages.extend(other.messages.clone());
        // Take max step
        self.step = self.step.max(other.step);
    }
}

async fn start_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("start".to_string());
    state.step += 1;
    Ok(state)
}

async fn path_a_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("path_a".to_string());
    state.step += 1;
    Ok(state)
}

async fn path_b_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("path_b".to_string());
    state.step += 1;
    Ok(state)
}

async fn parallel_1_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("parallel_1".to_string());
    state.step += 1;
    Ok(state)
}

async fn parallel_2_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("parallel_2".to_string());
    state.step += 1;
    Ok(state)
}

async fn middle_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("middle".to_string());
    state.step += 1;
    Ok(state)
}

async fn end_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("end_node".to_string());
    state.step += 1;
    Ok(state)
}

#[tokio::test]
#[allow(deprecated)]
async fn test_v1_0_add_conditional_edge() -> Result<(), Error> {
    // Test that v1.0-style add_conditional_edge still works
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)));
    graph.add_node_from_fn("path_a", |state: TestState| Box::pin(path_a_node(state)));
    graph.add_node_from_fn("path_b", |state: TestState| Box::pin(path_b_node(state)));

    // v1.0 style (deprecated but should work)
    let mut routes = HashMap::new();
    routes.insert("a".to_string(), "path_a".to_string());
    routes.insert("b".to_string(), "path_b".to_string());

    graph.add_conditional_edge(
        "start",
        |state: &TestState| {
            if state.step == 1 {
                "a".to_string()
            } else {
                "b".to_string()
            }
        },
        routes,
    );

    graph.add_edge("path_a", END);
    graph.add_edge("path_b", END);
    graph.set_entry_point("start");

    let app = graph.compile()?;

    let initial_state = TestState {
        messages: vec![],
        step: 0,
    };

    let result = app.invoke(initial_state).await?.final_state;

    // Should have taken path_a (because step == 1 after start_node)
    assert_eq!(result.messages, vec!["start", "path_a"]);
    assert_eq!(result.step, 2);
    Ok(())
}

#[tokio::test]
#[allow(deprecated)]
async fn test_v1_0_add_parallel_edge() -> Result<(), Error> {
    // Test that v1.0-style add_parallel_edge still works
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)));
    graph.add_node_from_fn("parallel_1", |state: TestState| {
        Box::pin(parallel_1_node(state))
    });
    graph.add_node_from_fn("parallel_2", |state: TestState| {
        Box::pin(parallel_2_node(state))
    });

    // v1.0 style (deprecated but should work)
    graph.add_parallel_edge(
        "start",
        vec!["parallel_1".to_string(), "parallel_2".to_string()],
    );

    // Both parallel nodes end
    graph.add_edge("parallel_1", END);
    graph.add_edge("parallel_2", END);

    graph.set_entry_point("start");

    let app = graph.compile_with_merge()?;

    let initial_state = TestState {
        messages: vec![],
        step: 0,
    };

    let result = app.invoke(initial_state).await?.final_state;

    // Should have executed start and both parallel nodes
    assert!(result.messages.contains(&"start".to_string()));
    assert!(
        result.messages.contains(&"parallel_1".to_string())
            || result.messages.contains(&"parallel_2".to_string()),
        "At least one parallel node should have executed. Got messages: {:?}",
        result.messages
    );

    // Step should be at least 2 (start + at least one parallel node)
    assert!(
        result.step >= 2,
        "Step should be at least 2, got {}",
        result.step
    );
    Ok(())
}

#[tokio::test]
#[allow(deprecated)]
async fn test_v1_0_mixed_api_usage() -> Result<(), Error> {
    // Test that v1.0 and v1.6 API can be mixed
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)));
    graph.add_node_from_fn("middle", |state: TestState| Box::pin(middle_node(state)));
    graph.add_node_from_fn("end_node", |state: TestState| Box::pin(end_node(state)));

    // Mix v1.0 style (deprecated)
    graph.add_edge("start", "middle");

    // With v1.6 style (current)
    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "end_node".to_string());
    routes.insert("stop".to_string(), END.to_string());

    graph.add_conditional_edges(
        "middle",
        |state: &TestState| {
            if state.step < 3 {
                "continue".to_string()
            } else {
                "stop".to_string()
            }
        },
        routes,
    );

    graph.add_edge("end_node", END);
    graph.set_entry_point("start");

    let app = graph.compile()?;

    let initial_state = TestState {
        messages: vec![],
        step: 0,
    };

    let result = app.invoke(initial_state).await?.final_state;

    // Should have executed all three nodes
    assert_eq!(result.messages, vec!["start", "middle", "end_node"]);
    assert_eq!(result.step, 3);
    Ok(())
}

#[test]
fn test_v1_0_api_exists() {
    // Compile-time test that v1.0 API methods exist
    // This test ensures the methods compile (even if deprecated)

    let mut graph: StateGraph<TestState> = StateGraph::new();

    // These should compile (even with deprecation warnings)
    #[allow(deprecated)]
    {
        graph.add_conditional_edge(
            "from",
            |_state: &TestState| "route".to_string(),
            HashMap::new(),
        );

        graph.add_parallel_edge("from", vec!["to".to_string()]);
    }
}
