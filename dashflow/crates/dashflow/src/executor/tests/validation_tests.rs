// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Tests for graph validation functionality.
//!
//! Extracted from executor/tests.rs by Worker #1695.

use super::*;

// ===== Graph Validation Tests =====

#[test]
fn test_validate_valid_graph() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", END);
    graph.set_entry_point("node1");

    let app = graph.compile().unwrap();
    let validation = app.validate();

    assert!(validation.is_valid(), "Valid graph should pass validation");
    assert_eq!(validation.warning_count(), 0);
}

#[test]
fn test_validate_unreachable_node() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("unreachable", |state| Box::pin(async move { Ok(state) }));
    graph.add_edge("entry", END);
    graph.set_entry_point("entry");

    // Use compile_without_validation() to test post-compile validation
    let app = graph.compile_without_validation().unwrap();
    let validation = app.validate();

    assert!(!validation.is_valid());
    assert!(validation.has_unreachable_nodes());
    assert!(validation.unreachable_nodes().contains(&"unreachable"));
}

#[test]
fn test_validate_no_path_to_end() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("node1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("node2", |state| Box::pin(async move { Ok(state) }));
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "node1"); // Infinite loop, no path to END
    graph.set_entry_point("node1");

    let app = graph.compile().unwrap();
    let validation = app.validate();

    assert!(!validation.is_valid());
    assert!(validation.has_no_path_to_end());
}

#[test]
fn test_validate_dead_end_node() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("entry", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("dead_end", |state| Box::pin(async move { Ok(state) }));
    graph.add_edge("entry", "dead_end");
    // No outgoing edge from dead_end
    graph.set_entry_point("entry");

    let app = graph.compile().unwrap();
    let validation = app.validate();

    assert!(!validation.is_valid());
    assert!(validation.has_dead_end_nodes());
    assert!(validation.dead_end_nodes().contains(&"dead_end"));
    // Also should have no path to END
    assert!(validation.has_no_path_to_end());
}

#[test]
fn test_validate_with_conditional_edges() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("path_a", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("path_b", |state| Box::pin(async move { Ok(state) }));

    let mut routes = HashMap::new();
    routes.insert("a".to_string(), "path_a".to_string());
    routes.insert("b".to_string(), "path_b".to_string());

    graph.add_conditional_edges("router", |_| "a".to_string(), routes);
    graph.add_edge("path_a", END);
    graph.add_edge("path_b", END);
    graph.set_entry_point("router");

    let app = graph.compile().unwrap();
    let validation = app.validate();

    assert!(
        validation.is_valid(),
        "Graph with conditional edges to END should be valid"
    );
}

#[test]
fn test_validate_with_parallel_edges() {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("splitter", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("worker1", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("worker2", |state| Box::pin(async move { Ok(state) }));
    graph.add_node_from_fn("merger", |state| Box::pin(async move { Ok(state) }));

    graph.add_parallel_edges(
        "splitter",
        vec!["worker1".to_string(), "worker2".to_string()],
    );
    graph.add_edge("worker1", "merger");
    graph.add_edge("worker2", "merger");
    graph.add_edge("merger", END);
    graph.set_entry_point("splitter");

    // Use compile_with_merge() for graphs with parallel edges
    let app = graph.compile_with_merge().unwrap();
    let validation = app.validate();

    assert!(
        validation.is_valid(),
        "Graph with parallel edges should be valid"
    );
}

#[test]
fn test_validation_result_helper_methods() {
    let mut result = GraphValidationResult::new();

    assert!(result.is_valid());
    assert_eq!(result.warning_count(), 0);

    result.add_warning(GraphValidationWarning::UnreachableNode {
        node: "orphan".to_string(),
    });
    result.add_warning(GraphValidationWarning::NoPathToEnd);
    result.add_warning(GraphValidationWarning::DeadEndNode {
        node: "stuck".to_string(),
    });

    assert!(!result.is_valid());
    assert_eq!(result.warning_count(), 3);
    assert!(result.has_unreachable_nodes());
    assert!(result.has_no_path_to_end());
    assert!(result.has_dead_end_nodes());
    assert!(result.unreachable_nodes().contains(&"orphan"));
    assert!(result.dead_end_nodes().contains(&"stuck"));
}

#[test]
fn test_validation_warning_display() {
    let warnings = [
        GraphValidationWarning::UnreachableNode {
            node: "orphan".to_string(),
        },
        GraphValidationWarning::NoPathToEnd,
        GraphValidationWarning::DeadEndNode {
            node: "stuck".to_string(),
        },
    ];

    assert_eq!(
        warnings[0].to_string(),
        "Node 'orphan' is unreachable from the entry point"
    );
    assert_eq!(
        warnings[1].to_string(),
        "Graph has no path to END - execution may never terminate"
    );
    assert_eq!(
        warnings[2].to_string(),
        "Node 'stuck' has no outgoing edges (dead end)"
    );
}
