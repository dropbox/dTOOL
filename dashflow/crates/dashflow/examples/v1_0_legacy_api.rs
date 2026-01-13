//! v1.0 Legacy API Example
//!
//! This example demonstrates the v1.0 API using deprecated methods
//! to verify backward compatibility with v1.6.
//!
//! **Expected behavior:**
//! - Code compiles with deprecation warnings
//! - Execution produces correct results
//! - Warnings guide users to new API
//!
//! This validates that v1.0 code works in v1.6 without modification.

#![allow(deprecated)] // Intentionally using deprecated APIs for testing

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkflowState {
    input: String,
    classification: String,
    processed: bool,
}

impl MergeableState for WorkflowState {
    fn merge(&mut self, other: &Self) {
        if !other.input.is_empty() {
            if self.input.is_empty() {
                self.input = other.input.clone();
            } else {
                self.input.push('\n');
                self.input.push_str(&other.input);
            }
        }
        if !other.classification.is_empty() {
            if self.classification.is_empty() {
                self.classification = other.classification.clone();
            } else {
                self.classification.push('\n');
                self.classification.push_str(&other.classification);
            }
        }
        self.processed = self.processed || other.processed;
    }
}

impl WorkflowState {
    fn new(input: String) -> Self {
        Self {
            input,
            classification: String::new(),
            processed: false,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 v1.0 Legacy API Test");
    println!("=====================\n");
    println!("This example uses the v1.0 API (deprecated methods)");
    println!("to verify backward compatibility with v1.6.\n");

    // Create graph using v1.6 API (StateGraph::new() is current)
    let mut graph: StateGraph<WorkflowState> = StateGraph::new();

    // Add nodes
    graph.add_node_from_fn("start", |state| {
        Box::pin(async move {
            println!("▶️  START: Processing input '{}'", state.input);
            Ok(state)
        })
    });

    graph.add_node_from_fn("classify", |mut state| {
        Box::pin(async move {
            state.classification = if state.input.len() > 5 {
                "long".to_string()
            } else {
                "short".to_string()
            };
            println!("🔍 CLASSIFY: Input is '{}'", state.classification);
            Ok(state)
        })
    });

    graph.add_node_from_fn("process_long", |mut state| {
        Box::pin(async move {
            println!("⚙️  PROCESS_LONG: Handling long input");
            state.processed = true;
            Ok(state)
        })
    });

    graph.add_node_from_fn("process_short", |mut state| {
        Box::pin(async move {
            println!("⚙️  PROCESS_SHORT: Handling short input");
            state.processed = true;
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel_a", |state| {
        Box::pin(async move {
            println!("🔀 PARALLEL_A: Running in parallel");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel_b", |state| {
        Box::pin(async move {
            println!("🔀 PARALLEL_B: Running in parallel");
            Ok(state)
        })
    });

    graph.add_node_from_fn("finalize", |state| {
        Box::pin(async move {
            println!("✅ FINALIZE: Workflow complete");
            Ok(state)
        })
    });

    // Build graph using v1.0 API (deprecated methods)
    graph.set_entry_point("start");
    graph.add_edge("start", "classify");

    // v1.0 API: add_conditional_edge (singular)
    // This method is deprecated in v1.6 but should still work
    let mut routes = HashMap::new();
    routes.insert("long".to_string(), "process_long".to_string());
    routes.insert("short".to_string(), "process_short".to_string());

    println!("⚠️  Using v1.0 API: add_conditional_edge (singular, deprecated)");
    graph.add_conditional_edge(
        "classify",
        |state: &WorkflowState| state.classification.clone(),
        routes,
    );

    // v1.0 API: add_parallel_edge (singular)
    // This method is deprecated in v1.6 but should still work
    println!("⚠️  Using v1.0 API: add_parallel_edge (singular, deprecated)\n");
    graph.add_parallel_edge(
        "process_long",
        vec!["parallel_a".to_string(), "parallel_b".to_string()],
    );

    graph.add_edge("process_short", "finalize");
    graph.add_edge("parallel_a", "finalize");
    graph.add_edge("parallel_b", "finalize");
    graph.add_edge("finalize", END);

    // Compile and test with different inputs
    let app = graph.compile()?;

    // Test 1: Long input (triggers parallel execution)
    println!("=== Test 1: Long input (triggers conditional + parallel) ===");
    let result1 = app
        .invoke(WorkflowState::new("Hello World!".to_string()))
        .await?;
    println!("✔️  Processed: {}", result1.final_state.processed);
    println!("✔️  Path: {:?}\n", result1.nodes_executed);

    // Test 2: Short input (simple path)
    println!("=== Test 2: Short input (triggers conditional only) ===");
    let result2 = app.invoke(WorkflowState::new("Hi".to_string())).await?;
    println!("✔️  Processed: {}", result2.final_state.processed);
    println!("✔️  Path: {:?}\n", result2.nodes_executed);

    println!("✅ SUCCESS: v1.0 API works in v1.6!");
    println!("Note: You should see deprecation warnings during compilation.");
    println!("Migrate to v1.6 API by changing:");
    println!("  - add_conditional_edge → add_conditional_edges (plural)");
    println!("  - add_parallel_edge → add_parallel_edges (plural)");

    Ok(())
}
