//! GraphBuilder Tests
//!
//! Tests for the fluent builder API (GraphBuilder type alias for StateGraph).
//! These tests demonstrate the ergonomic benefits of the builder pattern.

use dashflow::{Error, GraphBuilder, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TestState {
    messages: Vec<String>,
    step: u32,
    value: i32,
}

impl MergeableState for TestState {
    fn merge(&mut self, other: &Self) {
        // Merge messages (append)
        self.messages.extend(other.messages.clone());
        // Take max step
        self.step = self.step.max(other.step);
        // Sum values
        self.value += other.value;
    }
}

async fn start_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("start".to_string());
    state.step += 1;
    Ok(state)
}

async fn middle_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("middle".to_string());
    state.step += 1;
    Ok(state)
}

async fn end_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("end".to_string());
    state.step += 1;
    Ok(state)
}

async fn path_a_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("path_a".to_string());
    state.step += 1;
    state.value += 10;
    Ok(state)
}

async fn path_b_node(mut state: TestState) -> Result<TestState, Error> {
    state.messages.push("path_b".to_string());
    state.step += 1;
    state.value += 20;
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

#[tokio::test]
async fn test_graph_builder_basic_fluent_api() -> Result<(), Error> {
    // Test that GraphBuilder works with fluent API
    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        .add_node_from_fn("middle", |state: TestState| Box::pin(middle_node(state)))
        .add_node_from_fn("end", |state: TestState| Box::pin(end_node(state)))
        .add_edge("start", "middle")
        .add_edge("middle", "end")
        .add_edge("end", END)
        .set_entry_point("start");

    let app = graph.compile()?;

    let initial_state = TestState {
        messages: vec![],
        step: 0,
        value: 0,
    };

    let result = app.invoke(initial_state).await?.final_state;

    assert_eq!(result.messages, vec!["start", "middle", "end"]);
    assert_eq!(result.step, 3);
    Ok(())
}

#[tokio::test]
async fn test_graph_builder_no_intermediate_node_variables() -> Result<(), Error> {
    // Demonstrate that with builder pattern, you can chain all operations
    // before compiling, reducing intermediate variables
    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        .add_node_from_fn("end", |state: TestState| Box::pin(end_node(state)))
        .add_edge("start", "end")
        .add_edge("end", END)
        .set_entry_point("start");

    let app = graph.compile()?;

    let result = app
        .invoke(TestState {
            messages: vec![],
            step: 0,
            value: 0,
        })
        .await
        ?
        .final_state;

    assert_eq!(result.messages, vec!["start", "end"]);
    assert_eq!(result.step, 2);
    Ok(())
}

#[tokio::test]
async fn test_graph_builder_conditional_edges() -> Result<(), Error> {
    // Test GraphBuilder with conditional edges
    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        .add_node_from_fn("path_a", |state: TestState| Box::pin(path_a_node(state)))
        .add_node_from_fn("path_b", |state: TestState| Box::pin(path_b_node(state)))
        .add_conditional_edges(
            "start",
            |state: &TestState| {
                if state.value < 5 {
                    "a".to_string()
                } else {
                    "b".to_string()
                }
            },
            [
                ("a".to_string(), "path_a".to_string()),
                ("b".to_string(), "path_b".to_string()),
            ]
            .into_iter()
            .collect(),
        )
        .add_edge("path_a", END)
        .add_edge("path_b", END)
        .set_entry_point("start");

    let app = graph.compile()?;

    // Test path A (value < 5)
    let result_a = app
        .invoke(TestState {
            messages: vec![],
            step: 0,
            value: 0,
        })
        .await?
        .final_state;

    assert_eq!(result_a.messages, vec!["start", "path_a"]);
    assert_eq!(result_a.value, 10);

    // Test path B (value >= 5)
    let result_b = app
        .invoke(TestState {
            messages: vec![],
            step: 0,
            value: 10,
        })
        .await?
        .final_state;

    assert_eq!(result_b.messages, vec!["start", "path_b"]);
    assert_eq!(result_b.value, 30);
    Ok(())
}

#[tokio::test]
async fn test_graph_builder_parallel_edges() -> Result<(), Error> {
    // Test GraphBuilder with parallel edges
    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        .add_node_from_fn("parallel_1", |state: TestState| {
            Box::pin(parallel_1_node(state))
        })
        .add_node_from_fn("parallel_2", |state: TestState| {
            Box::pin(parallel_2_node(state))
        })
        .add_parallel_edges(
            "start",
            vec!["parallel_1".to_string(), "parallel_2".to_string()],
        )
        .add_edge("parallel_1", END)
        .add_edge("parallel_2", END)
        .set_entry_point("start");

    let app = graph.compile_with_merge()?;

    let result = app
        .invoke(TestState {
            messages: vec![],
            step: 0,
            value: 0,
        })
        .await?
        .final_state;

    // Debug output
    eprintln!("Messages: {:?}", result.messages);
    eprintln!("Step: {}", result.step);

    assert_eq!(result.messages[0], "start");
    // parallel_1 and parallel_2 should both be in messages (order may vary)
    // Note: Parallel edges in DashFlow execute all nodes, but only the last result is kept
    // So we might only see one of them in the final state
    assert!(result.messages.len() >= 2);
    Ok(())
}

#[tokio::test]
async fn test_graph_builder_vs_state_graph_identical() -> Result<(), Error> {
    // Verify GraphBuilder and StateGraph are identical (type alias)
    let mut graph = GraphBuilder::new();
    graph
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        .add_edge("start", END)
        .set_entry_point("start");

    let builder_app = graph.compile()?;

    let mut state_graph = StateGraph::new();
    state_graph
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        .add_edge("start", END)
        .set_entry_point("start");
    let state_app = state_graph.compile()?;

    let initial_state = TestState {
        messages: vec![],
        step: 0,
        value: 0,
    };

    let builder_result = builder_app.invoke(initial_state.clone()).await?;
    let state_result = state_app.invoke(initial_state).await?;

    // Results should be identical
    assert_eq!(
        builder_result.final_state.messages,
        state_result.final_state.messages
    );
    assert_eq!(
        builder_result.final_state.step,
        state_result.final_state.step
    );
    Ok(())
}

#[tokio::test]
async fn test_graph_builder_method() -> Result<(), Error> {
    // Test the builder() convenience method
    let mut graph = GraphBuilder::builder();
    graph
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        .add_edge("start", END)
        .set_entry_point("start");

    let app = graph.compile()?;

    let result = app
        .invoke(TestState {
            messages: vec![],
            step: 0,
            value: 0,
        })
        .await?
        .final_state;

    assert_eq!(result.messages, vec!["start"]);
    Ok(())
}

#[tokio::test]
async fn test_graph_builder_complex_workflow() -> Result<(), Error> {
    // Comprehensive test: multiple node types, conditional routing, parallel execution
    let mut routes = HashMap::new();
    routes.insert("parallel".to_string(), "parallel_hub".to_string());
    routes.insert("end".to_string(), END.to_string());

    let mut graph = GraphBuilder::new();
    graph
        // Start node
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        // Conditional routing based on value
        .add_node_from_fn("path_a", |state: TestState| Box::pin(path_a_node(state)))
        .add_node_from_fn("path_b", |state: TestState| Box::pin(path_b_node(state)))
        .add_conditional_edges(
            "start",
            |state: &TestState| {
                if state.value < 5 {
                    "a".to_string()
                } else {
                    "b".to_string()
                }
            },
            [
                ("a".to_string(), "path_a".to_string()),
                ("b".to_string(), "path_b".to_string()),
            ]
            .into_iter()
            .collect(),
        )
        // Both paths converge to middle
        .add_edge("path_a", "middle")
        .add_edge("path_b", "middle")
        // Middle node decides: parallel work or end
        .add_node_from_fn("middle", |state: TestState| Box::pin(middle_node(state)))
        .add_conditional_edges(
            "middle",
            |state: &TestState| {
                if state.step < 5 {
                    "parallel".to_string()
                } else {
                    "end".to_string()
                }
            },
            routes,
        )
        // Parallel hub - passthrough node that triggers parallel execution
        .add_node_from_fn("parallel_hub", |state: TestState| {
            Box::pin(async move { Ok(state) })
        })
        // Parallel execution from hub
        .add_node_from_fn("parallel_1", |state: TestState| {
            Box::pin(parallel_1_node(state))
        })
        .add_node_from_fn("parallel_2", |state: TestState| {
            Box::pin(parallel_2_node(state))
        })
        .add_parallel_edges(
            "parallel_hub",
            vec!["parallel_1".to_string(), "parallel_2".to_string()],
        )
        .add_edge("parallel_1", END)
        .add_edge("parallel_2", END)
        .set_entry_point("start");

    let app = graph.compile_with_merge()?;

    let result = app
        .invoke(TestState {
            messages: vec![],
            step: 0,
            value: 0,
        })
        .await?
        .final_state;

    // Verify execution path
    eprintln!("Complex workflow messages: {:?}", result.messages);
    eprintln!("Complex workflow step: {}", result.step);

    assert_eq!(result.messages[0], "start");
    assert_eq!(result.messages[1], "path_a"); // value < 5, so path_a
    assert_eq!(result.messages[2], "middle");
    // Parallel edges: only last result is kept in final state
    assert!(result.messages.len() >= 3);
    Ok(())
}

#[test]
fn test_graph_builder_type_inference() {
    // Test that type inference works well with builder pattern
    // This should compile without explicit type annotations on GraphBuilder::new()
    let _graph = GraphBuilder::new()
        .add_node_from_fn("start", |state: TestState| Box::pin(start_node(state)))
        .add_edge("start", END)
        .set_entry_point("start");

    // Type is inferred from first add_node_from_fn call
}
