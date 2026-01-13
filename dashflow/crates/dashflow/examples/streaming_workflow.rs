//! Streaming Workflow Example
//!
//! This example demonstrates DashFlow's streaming API for consuming
//! graph execution results in real-time as nodes complete.
//!
//! Streaming is essential for:
//! - Long-running workflows (show progress to users)
//! - Interactive applications (update UI as nodes finish)
//! - Resource monitoring (track execution as it happens)
//!
//! Run: cargo run --example streaming_workflow

use dashflow::{MergeableState, StateGraph, StreamEvent, StreamMode, END};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct ProcessingState {
    input: String,
    step1_result: String,
    step2_result: String,
    step3_result: String,
    final_output: String,
}

impl MergeableState for ProcessingState {
    fn merge(&mut self, other: &Self) {
        if !other.input.is_empty() {
            if self.input.is_empty() {
                self.input = other.input.clone();
            } else {
                self.input.push('\n');
                self.input.push_str(&other.input);
            }
        }
        if !other.step1_result.is_empty() {
            if self.step1_result.is_empty() {
                self.step1_result = other.step1_result.clone();
            } else {
                self.step1_result.push('\n');
                self.step1_result.push_str(&other.step1_result);
            }
        }
        if !other.step2_result.is_empty() {
            if self.step2_result.is_empty() {
                self.step2_result = other.step2_result.clone();
            } else {
                self.step2_result.push('\n');
                self.step2_result.push_str(&other.step2_result);
            }
        }
        if !other.step3_result.is_empty() {
            if self.step3_result.is_empty() {
                self.step3_result = other.step3_result.clone();
            } else {
                self.step3_result.push('\n');
                self.step3_result.push_str(&other.step3_result);
            }
        }
        if !other.final_output.is_empty() {
            if self.final_output.is_empty() {
                self.final_output = other.final_output.clone();
            } else {
                self.final_output.push('\n');
                self.final_output.push_str(&other.final_output);
            }
        }
    }
}

impl ProcessingState {
    fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            step1_result: String::new(),
            step2_result: String::new(),
            step3_result: String::new(),
            final_output: String::new(),
        }
    }
}

fn build_processing_graph() -> StateGraph<ProcessingState> {
    let mut graph = StateGraph::new();

    // Step 1: Data collection
    graph.add_node_from_fn("collect_data", |mut state: ProcessingState| {
        Box::pin(async move {
            println!("  [Step 1] Collecting data for: {}", state.input);
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            state.step1_result = format!("Collected data about {}", state.input);
            println!("  [Step 1] ✓ Complete");

            Ok(state)
        })
    });

    // Step 2: Data processing
    graph.add_node_from_fn("process_data", |mut state: ProcessingState| {
        Box::pin(async move {
            println!("  [Step 2] Processing collected data...");
            tokio::time::sleep(tokio::time::Duration::from_millis(700)).await;

            state.step2_result = format!("Processed: {}", state.step1_result);
            println!("  [Step 2] ✓ Complete");

            Ok(state)
        })
    });

    // Step 3: Data analysis
    graph.add_node_from_fn("analyze_data", |mut state: ProcessingState| {
        Box::pin(async move {
            println!("  [Step 3] Analyzing processed data...");
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

            state.step3_result = format!("Analysis: {}", state.step2_result);
            println!("  [Step 3] ✓ Complete");

            Ok(state)
        })
    });

    // Step 4: Generate report
    graph.add_node_from_fn("generate_report", |mut state: ProcessingState| {
        Box::pin(async move {
            println!("  [Step 4] Generating final report...");
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

            state.final_output = format!(
                "REPORT:\n  Input: {}\n  Step 1: {}\n  Step 2: {}\n  Step 3: {}",
                state.input, state.step1_result, state.step2_result, state.step3_result
            );
            println!("  [Step 4] ✓ Complete");

            Ok(state)
        })
    });

    graph.set_entry_point("collect_data");
    graph.add_edge("collect_data", "process_data");
    graph.add_edge("process_data", "analyze_data");
    graph.add_edge("analyze_data", "generate_report");
    graph.add_edge("generate_report", END);

    graph
}

#[tokio::main]
async fn main() -> dashflow::error::Result<()> {
    println!("=== DashFlow Streaming Example ===\n");

    // Part 1: Stream with Values mode
    println!("--- Part 1: Streaming with Values Mode ---");
    println!("(Full state after each node completes)\n");

    let graph = build_processing_graph();
    let app = graph.compile()?;

    let initial_state = ProcessingState::new("Customer Analytics");
    let mut stream = Box::pin(app.stream(initial_state, StreamMode::Values));

    println!("▶️  Starting workflow...\n");

    while let Some(event_result) = stream.next().await {
        let event = event_result?;

        match event {
            StreamEvent::Values { node, state } => {
                println!("📊 State after node '{}':", node);
                println!("   Step 1 Result: {}", state.step1_result);
                println!("   Step 2 Result: {}", state.step2_result);
                println!("   Step 3 Result: {}", state.step3_result);
                if !state.final_output.is_empty() {
                    println!(
                        "   Final Output:\n{}",
                        state.final_output.replace('\n', "\n   ")
                    );
                }
                println!();
            }
            StreamEvent::Done { execution_path, .. } => {
                println!("✅ Workflow complete!");
                println!("   Execution path: {}\n", execution_path.join(" → "));
            }
            _ => {}
        }
    }

    // Part 2: Stream with Events mode
    println!("\n--- Part 2: Streaming with Events Mode ---");
    println!("(Node start/end events)\n");

    let graph2 = build_processing_graph();
    let app2 = graph2.compile()?;

    let initial_state2 = ProcessingState::new("Sales Dashboard");
    let mut stream2 = Box::pin(app2.stream(initial_state2, StreamMode::Events));

    println!("▶️  Starting workflow...\n");

    while let Some(event_result) = stream2.next().await {
        let event = event_result?;

        match event {
            StreamEvent::NodeStart { node } => {
                println!("🔵 Starting: {}", node);
            }
            StreamEvent::NodeEnd { node, .. } => {
                println!("🟢 Finished: {}", node);
            }
            StreamEvent::Done { .. } => {
                println!("\n✅ Workflow complete!");
            }
            _ => {}
        }
    }

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("- Values mode: Full state after each node (best for monitoring state changes)");
    println!("- Events mode: Node lifecycle events (best for tracking execution flow)");
    println!("- Updates mode: Only state changes (most efficient for large states)");
    println!("\nStreaming enables real-time progress updates for long-running workflows!");
    Ok(())
}
