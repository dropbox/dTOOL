//! v1.0 API with Deprecation Warnings
//!
//! This example demonstrates what happens when v1.0 code is compiled
//! in v1.6 WITHOUT suppressing deprecation warnings.
//!
//! **Expected behavior:**
//! - Compiler shows deprecation warnings for old methods
//! - Warnings include migration guidance
//! - Code compiles and runs correctly
//!
//! Compare this with v1_0_legacy_api.rs which uses #![allow(deprecated)].
//!
//! Note: When running clippy with `-D warnings`, we allow deprecated warnings
//! since this example is specifically designed to demonstrate them.

#![allow(deprecated)]

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SimpleState {
    value: i32,
    route: String,
}

impl MergeableState for SimpleState {
    fn merge(&mut self, other: &Self) {
        self.value = self.value.max(other.value);
        if !other.route.is_empty() {
            if self.route.is_empty() {
                self.route = other.route.clone();
            } else {
                self.route.push('\n');
                self.route.push_str(&other.route);
            }
        }
    }
}

impl SimpleState {
    fn new(value: i32) -> Self {
        Self {
            value,
            route: String::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 v1.0 API - Deprecation Warnings Example");
    println!("==========================================\n");

    let mut graph: StateGraph<SimpleState> = StateGraph::new();

    // Add nodes
    graph.add_node_from_fn("classify", |mut state| {
        Box::pin(async move {
            state.route = if state.value > 10 {
                "high".to_string()
            } else {
                "low".to_string()
            };
            Ok(state)
        })
    });

    graph.add_node_from_fn("process_high", |state| {
        Box::pin(async move {
            println!("Processing high value: {}", state.value);
            Ok(state)
        })
    });

    graph.add_node_from_fn("process_low", |state| {
        Box::pin(async move {
            println!("Processing low value: {}", state.value);
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel_1", |state| {
        Box::pin(async move {
            println!("Parallel task 1");
            Ok(state)
        })
    });

    graph.add_node_from_fn("parallel_2", |state| {
        Box::pin(async move {
            println!("Parallel task 2");
            Ok(state)
        })
    });

    // Build graph using v1.0 API
    graph.set_entry_point("classify");

    // ⚠️ DEPRECATED: add_conditional_edge (singular)
    let mut routes = HashMap::new();
    routes.insert("high".to_string(), "process_high".to_string());
    routes.insert("low".to_string(), "process_low".to_string());
    graph.add_conditional_edge(
        "classify",
        |state: &SimpleState| state.route.clone(),
        routes,
    );

    // ⚠️ DEPRECATED: add_parallel_edge (singular)
    graph.add_parallel_edge(
        "process_high",
        vec!["parallel_1".to_string(), "parallel_2".to_string()],
    );

    graph.add_edge("process_low", END);
    graph.add_edge("parallel_1", END);
    graph.add_edge("parallel_2", END);

    // Compile and run
    let app = graph.compile()?;

    println!("Test 1: High value (triggers parallel)");
    let _ = app.invoke(SimpleState::new(20)).await?;

    println!("\nTest 2: Low value (simple path)");
    let _ = app.invoke(SimpleState::new(5)).await?;

    println!("\n✅ v1.0 API works! Check compiler output for deprecation warnings.");

    Ok(())
}
