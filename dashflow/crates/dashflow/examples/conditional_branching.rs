//! Conditional branching workflow
//!
//! This example demonstrates conditional routing where the workflow
//! branches based on state conditions.
//!
//! Pattern: Input -> Classifier -> (Route A | Route B) -> Merge -> Output

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BranchState {
    input: i32,
    route: String,
    result: String,
}

impl MergeableState for BranchState {
    fn merge(&mut self, other: &Self) {
        self.input = self.input.max(other.input);
        if !other.route.is_empty() {
            if self.route.is_empty() {
                self.route = other.route.clone();
            } else {
                self.route.push('\n');
                self.route.push_str(&other.route);
            }
        }
        if !other.result.is_empty() {
            if self.result.is_empty() {
                self.result = other.result.clone();
            } else {
                self.result.push('\n');
                self.result.push_str(&other.result);
            }
        }
    }
}

impl BranchState {
    fn new(input: i32) -> Self {
        Self {
            input,
            route: String::new(),
            result: String::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the graph
    let mut graph: StateGraph<BranchState> = StateGraph::new();

    // Input node - receives and logs input
    graph.add_node_from_fn("input", |state| {
        Box::pin(async move {
            println!("📥 Input: Received number {}", state.input);
            Ok(state)
        })
    });

    // Classifier node - determines which route to take
    graph.add_node_from_fn("classifier", |mut state| {
        Box::pin(async move {
            if state.input % 2 == 0 {
                state.route = "even".to_string();
                println!("🔀 Classifier: Number is EVEN, routing to even handler");
            } else {
                state.route = "odd".to_string();
                println!("🔀 Classifier: Number is ODD, routing to odd handler");
            }
            Ok(state)
        })
    });

    // Even number handler
    graph.add_node_from_fn("handle_even", |mut state| {
        Box::pin(async move {
            println!("⚡ Even Handler: Processing even number...");
            state.result = format!("{} is even (divisible by 2)", state.input);
            Ok(state)
        })
    });

    // Odd number handler
    graph.add_node_from_fn("handle_odd", |mut state| {
        Box::pin(async move {
            println!("⚡ Odd Handler: Processing odd number...");
            state.result = format!("{} is odd (not divisible by 2)", state.input);
            Ok(state)
        })
    });

    // Merge node - combines results from either branch
    graph.add_node_from_fn("merge", |mut state| {
        Box::pin(async move {
            println!("🔗 Merge: Finalizing result...");
            state.result = format!("Analysis: {}", state.result);
            Ok(state)
        })
    });

    // Build the graph with conditional routing
    graph.set_entry_point("input");
    graph.add_edge("input", "classifier");

    // Conditional routing based on classifier decision
    let mut routes = HashMap::new();
    routes.insert("even".to_string(), "handle_even".to_string());
    routes.insert("odd".to_string(), "handle_odd".to_string());

    graph.add_conditional_edges(
        "classifier",
        |state: &BranchState| state.route.clone(),
        routes,
    );

    // Both branches converge to merge
    graph.add_edge("handle_even", "merge");
    graph.add_edge("handle_odd", "merge");
    graph.add_edge("merge", END);

    // Compile and run with different inputs
    let app = graph.compile()?;

    println!("🚀 Starting conditional branching workflow...\n");
    println!("=== Test 1: Even number ===");
    let result1 = app.invoke(BranchState::new(42)).await?;
    println!("Result: {}", result1.final_state.result);
    println!("Path: {:?}\n", result1.nodes_executed);

    println!("=== Test 2: Odd number ===");
    let result2 = app.invoke(BranchState::new(17)).await?;
    println!("Result: {}", result2.final_state.result);
    println!("Path: {:?}", result2.nodes_executed);

    Ok(())
}
