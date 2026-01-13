//! Simple sequential workflow example
//!
//! This example demonstrates a basic linear workflow where nodes
//! execute one after another in a fixed sequence.
//!
//! Pattern: Input -> Process -> Validate -> Output

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkflowState {
    input: String,
    processed: String,
    validated: bool,
    output: String,
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
        if !other.processed.is_empty() {
            if self.processed.is_empty() {
                self.processed = other.processed.clone();
            } else {
                self.processed.push('\n');
                self.processed.push_str(&other.processed);
            }
        }
        self.validated = self.validated || other.validated;
        if !other.output.is_empty() {
            if self.output.is_empty() {
                self.output = other.output.clone();
            } else {
                self.output.push('\n');
                self.output.push_str(&other.output);
            }
        }
    }
}

impl WorkflowState {
    fn new(input: String) -> Self {
        Self {
            input,
            processed: String::new(),
            validated: false,
            output: String::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the graph
    let mut graph: StateGraph<WorkflowState> = StateGraph::new();

    // Node 1: Input processing
    graph.add_node_from_fn("input", |mut state| {
        Box::pin(async move {
            println!("📥 Input: Receiving '{}'", state.input);
            state.processed = format!("Processed: {}", state.input);
            Ok(state)
        })
    });

    // Node 2: Data processing
    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            println!("⚙️  Process: Transforming data...");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            state.processed = state.processed.to_uppercase();
            Ok(state)
        })
    });

    // Node 3: Validation
    graph.add_node_from_fn("validate", |mut state| {
        Box::pin(async move {
            println!("✔️  Validate: Checking data quality...");
            state.validated = !state.processed.is_empty();
            if !state.validated {
                eprintln!("❌ Validation failed!");
            }
            Ok(state)
        })
    });

    // Node 4: Output generation
    graph.add_node_from_fn("output", |mut state| {
        Box::pin(async move {
            println!("📤 Output: Generating final result...");
            state.output = format!(
                "Result: {} (validated: {})",
                state.processed, state.validated
            );
            Ok(state)
        })
    });

    // Build linear workflow: input -> process -> validate -> output -> END
    graph.set_entry_point("input");
    graph.add_edge("input", "process");
    graph.add_edge("process", "validate");
    graph.add_edge("validate", "output");
    graph.add_edge("output", END);

    // Compile and run
    let app = graph.compile()?;

    let initial_state = WorkflowState::new("Hello World".to_string());

    println!("🚀 Starting sequential workflow...\n");
    let result = app.invoke(initial_state).await?;

    println!("\n📊 Execution Summary:");
    println!("Nodes executed: {:?}", result.nodes_executed);
    println!("Final output: {}", result.final_state.output);
    println!("Validated: {}", result.final_state.validated);

    Ok(())
}
