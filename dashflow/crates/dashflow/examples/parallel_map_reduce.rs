//! Map-Reduce pattern with parallel execution
//!
//! This example demonstrates how to use parallel edges to execute
//! multiple nodes concurrently.
//!
//! Pattern:
//! - Input node prepares data
//! - Multiple mapper nodes process in parallel (all execute concurrently)
//! - Reducer node processes results
//!
//! Note: Currently, parallel nodes all receive the same input state and execute
//! concurrently, but only the last node's state modifications are kept. For
//! full map-reduce with state merging, you would need to implement a custom
//! merge strategy or use shared state (e.g., Arc<Mutex<Vec>>).

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MapReduceState {
    input: String,
    results: Vec<String>,
    final_result: String,
}

impl MergeableState for MapReduceState {
    fn merge(&mut self, other: &Self) {
        if !other.input.is_empty() {
            if self.input.is_empty() {
                self.input = other.input.clone();
            } else {
                self.input.push('\n');
                self.input.push_str(&other.input);
            }
        }
        self.results.extend(other.results.clone());
        if !other.final_result.is_empty() {
            if self.final_result.is_empty() {
                self.final_result = other.final_result.clone();
            } else {
                self.final_result.push('\n');
                self.final_result.push_str(&other.final_result);
            }
        }
    }
}

impl MapReduceState {
    fn new(input: String) -> Self {
        Self {
            input,
            results: Vec::new(),
            final_result: String::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the graph
    let mut graph: StateGraph<MapReduceState> = StateGraph::new();

    // Input node - prepares data for mapping
    graph.add_node_from_fn("input", |state| {
        Box::pin(async move {
            println!("📥 Input: Processing '{}'", state.input);
            Ok(state)
        })
    });

    // Mapper 1 - Analyzes word count
    graph.add_node_from_fn("mapper1_word_count", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let word_count = state.input.split_whitespace().count();
            let result = format!("Words: {}", word_count);
            println!("🔄 Mapper 1: {}", result);
            state.results.push(result);
            Ok(state)
        })
    });

    // Mapper 2 - Analyzes character count
    graph.add_node_from_fn("mapper2_char_count", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let char_count = state.input.len();
            let result = format!("Chars: {}", char_count);
            println!("🔄 Mapper 2: {}", result);
            state.results.push(result);
            Ok(state)
        })
    });

    // Mapper 3 - Analyzes uppercase letters
    graph.add_node_from_fn("mapper3_uppercase", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let uppercase_count = state.input.chars().filter(|c| c.is_uppercase()).count();
            let result = format!("Uppercase: {}", uppercase_count);
            println!("🔄 Mapper 3: {}", result);
            state.results.push(result);
            Ok(state)
        })
    });

    // Reducer node - Combines results
    graph.add_node_from_fn("reduce", |mut state| {
        Box::pin(async move {
            println!("🔀 Reducer: Combining results...");
            state.final_result = state.results.join(", ");
            println!("✅ Final result: {}", state.final_result);
            Ok(state)
        })
    });

    // Build the graph
    graph.set_entry_point("input");

    // Fan out to parallel mappers
    graph.add_parallel_edges(
        "input",
        vec![
            "mapper1_word_count".to_string(),
            "mapper2_char_count".to_string(),
            "mapper3_uppercase".to_string(),
        ],
    );

    // All mappers converge to reducer
    graph.add_edge("mapper3_uppercase", "reduce");
    graph.add_edge("reduce", END);

    // Compile and run
    let app = graph.compile()?;

    let initial_state = MapReduceState::new("Hello World! This is DashFlow.".to_string());

    println!("🚀 Starting Map-Reduce workflow...\n");
    let result = app.invoke(initial_state).await?;

    println!("\n📊 Execution Summary:");
    println!("Nodes executed: {:?}", result.nodes_executed);
    println!("Final result: {}", result.final_state.final_result);

    Ok(())
}
