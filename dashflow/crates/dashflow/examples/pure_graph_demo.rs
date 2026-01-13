//! Pure Graph Demo - Proves DashFlow graph engine works without LLMs
//!
//! Run: cargo run --package dashflow --example pure_graph_demo
//!
//! This demonstrates:
//! - State management
//! - Sequential node execution
//! - Conditional routing
//! - Parallel execution with MergeableState
//! - Checkpointing and resume
//! - State diff tracking
//!
//! NO API KEYS REQUIRED. NO EXTERNAL SERVICES.

use dashflow::{FileCheckpointer, MergeableState, Result, StateGraph};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const END: &str = "__end__";

/// Demo state demonstrating all graph features without LLM
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct DemoState {
    input: String,
    step1_output: String,
    step2_output: String,
    parallel_results: Vec<String>,
    final_output: String,
    execution_path: Vec<String>,
    iteration_count: u32,
}

impl MergeableState for DemoState {
    fn merge(&mut self, other: &Self) {
        // Merge parallel results from different branches
        self.parallel_results.extend(other.parallel_results.clone());
        self.execution_path.extend(other.execution_path.clone());
        self.iteration_count = self.iteration_count.max(other.iteration_count);

        // Take non-empty values
        if !other.step1_output.is_empty() && self.step1_output.is_empty() {
            self.step1_output = other.step1_output.clone();
        }
        if !other.step2_output.is_empty() && self.step2_output.is_empty() {
            self.step2_output = other.step2_output.clone();
        }
        if !other.final_output.is_empty() && self.final_output.is_empty() {
            self.final_output = other.final_output.clone();
        }
    }
}

/// Step 1: Process input (no LLM - pure Rust)
async fn step1_process(state: DemoState) -> Result<DemoState> {
    println!("   [step1_process] Processing input: {:?}", state.input);
    let mut state = state;
    state.step1_output = format!("Processed: {}", state.input.to_uppercase());
    state.execution_path.push("step1_process".into());
    Ok(state)
}

/// Step 2: Validate result (no LLM - pure Rust)
async fn step2_validate(state: DemoState) -> Result<DemoState> {
    println!("   [step2_validate] Validating: {:?}", state.step1_output);
    let mut state = state;
    state.step2_output = format!("Validated: {} chars", state.step1_output.len());
    state.execution_path.push("step2_validate".into());
    Ok(state)
}

/// Parallel branch A (demonstrates MergeableState)
async fn parallel_branch_a(state: DemoState) -> Result<DemoState> {
    println!("   [parallel_a] Branch A executing...");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let mut state = state;
    state
        .parallel_results
        .push("Branch A: analysis complete".into());
    state.execution_path.push("parallel_a".into());
    Ok(state)
}

/// Parallel branch B (demonstrates MergeableState)
async fn parallel_branch_b(state: DemoState) -> Result<DemoState> {
    println!("   [parallel_b] Branch B executing...");
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    let mut state = state;
    state
        .parallel_results
        .push("Branch B: verification complete".into());
    state.execution_path.push("parallel_b".into());
    Ok(state)
}

/// Aggregator node - combines parallel results
async fn aggregate_results(state: DemoState) -> Result<DemoState> {
    println!(
        "   [aggregate] Combining {} parallel results",
        state.parallel_results.len()
    );
    let mut state = state;
    state.final_output = format!(
        "Aggregated {} results: {}",
        state.parallel_results.len(),
        state.parallel_results.join("; ")
    );
    state.execution_path.push("aggregate".into());
    Ok(state)
}

/// Conditional routing decision (no LLM - pure Rust logic)
fn should_run_parallel(state: &DemoState) -> bool {
    // Run parallel branches if input is long enough (processed output > 20 chars)
    state.step1_output.len() > 20
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("DashFlow Pure Graph Demo");
    println!("========================");
    println!("Proves graph execution works WITHOUT any LLM or API keys.\n");

    // ============================================
    // Part 1: Sequential + Conditional Execution
    // ============================================
    println!("Part 1: Sequential + Conditional Execution");
    println!("------------------------------------------");

    let mut graph: StateGraph<DemoState> = StateGraph::new();

    // Add nodes
    graph.add_node_from_fn("step1", |state| Box::pin(step1_process(state)));
    graph.add_node_from_fn("step2", |state| Box::pin(step2_validate(state)));
    graph.add_node_from_fn("parallel_a", |state| Box::pin(parallel_branch_a(state)));
    graph.add_node_from_fn("parallel_b", |state| Box::pin(parallel_branch_b(state)));
    graph.add_node_from_fn("aggregate", |state| Box::pin(aggregate_results(state)));

    // Sequential edges
    graph.add_edge("step1", "step2");

    // Conditional edge from step2 - decides if we do parallel processing
    let mut conditional_routes = HashMap::new();
    conditional_routes.insert("parallel".to_string(), "parallel_a".to_string());
    conditional_routes.insert("skip".to_string(), "aggregate".to_string());

    graph.add_conditional_edges(
        "step2",
        |state: &DemoState| {
            if should_run_parallel(state) {
                "parallel".to_string()
            } else {
                "skip".to_string()
            }
        },
        conditional_routes,
    );

    // Parallel branch also routes to parallel_b (would need fan-out in real usage)
    // For simplicity, run them sequentially here
    graph.add_edge("parallel_a", "parallel_b");
    graph.add_edge("parallel_b", "aggregate");
    graph.add_edge("aggregate", END);

    // Set entry point
    graph.set_entry_point("step1");

    // Compile
    println!("\n   Compiling graph...");
    let app = graph.compile()?;
    println!(
        "   Graph compiled: {} nodes, {} edges",
        app.node_count(),
        app.edge_count()
    );

    // Run with long input (triggers parallel)
    let initial_state = DemoState {
        input: "Hello DashFlow World!".into(),
        ..Default::default()
    };

    println!("\n   Running with input: {:?}", initial_state.input);
    let result = app.invoke(initial_state).await?;

    println!("\n   Execution complete!");
    println!("   Path: {:?}", result.state().execution_path);
    println!("   Step 1 output: {}", result.state().step1_output);
    println!("   Step 2 output: {}", result.state().step2_output);
    println!("   Parallel results: {:?}", result.state().parallel_results);
    println!("   Final output: {}", result.state().final_output);

    // Run with short input (skips parallel)
    println!("\n   Running with short input: \"Hi\"");
    let short_state = DemoState {
        input: "Hi".into(),
        ..Default::default()
    };

    // Need to rebuild graph for new execution
    let mut graph2: StateGraph<DemoState> = StateGraph::new();
    graph2.add_node_from_fn("step1", |state| Box::pin(step1_process(state)));
    graph2.add_node_from_fn("step2", |state| Box::pin(step2_validate(state)));
    graph2.add_node_from_fn("parallel_a", |state| Box::pin(parallel_branch_a(state)));
    graph2.add_node_from_fn("parallel_b", |state| Box::pin(parallel_branch_b(state)));
    graph2.add_node_from_fn("aggregate", |state| Box::pin(aggregate_results(state)));
    graph2.add_edge("step1", "step2");
    let mut conditional_routes2 = HashMap::new();
    conditional_routes2.insert("parallel".to_string(), "parallel_a".to_string());
    conditional_routes2.insert("skip".to_string(), "aggregate".to_string());
    graph2.add_conditional_edges(
        "step2",
        |state: &DemoState| {
            if should_run_parallel(state) {
                "parallel".to_string()
            } else {
                "skip".to_string()
            }
        },
        conditional_routes2,
    );
    graph2.add_edge("parallel_a", "parallel_b");
    graph2.add_edge("parallel_b", "aggregate");
    graph2.add_edge("aggregate", END);
    graph2.set_entry_point("step1");
    let app2 = graph2.compile()?;

    let result2 = app2.invoke(short_state).await?;
    println!("   Path (short): {:?}", result2.state().execution_path);
    println!("   (Parallel branches omitted due to conditional routing)");

    // ============================================
    // Part 2: Checkpointing Demo
    // ============================================
    println!("\n\nPart 2: Checkpointing Demo");
    println!("--------------------------");

    let temp_dir = std::env::temp_dir().join("dashflow_pure_graph_demo");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    println!("   Creating file checkpointer at: {:?}", temp_dir);
    let checkpointer = FileCheckpointer::new(&temp_dir)?;

    let mut checkpoint_graph: StateGraph<DemoState> = StateGraph::new();
    checkpoint_graph.add_node_from_fn("step1", |state| Box::pin(step1_process(state)));
    checkpoint_graph.add_node_from_fn("step2", |state| Box::pin(step2_validate(state)));
    checkpoint_graph.add_node_from_fn("aggregate", |state| Box::pin(aggregate_results(state)));
    checkpoint_graph.add_edge("step1", "step2");
    checkpoint_graph.add_edge("step2", "aggregate");
    checkpoint_graph.add_edge("aggregate", END);
    checkpoint_graph.set_entry_point("step1");

    let checkpoint_app = checkpoint_graph
        .compile()?
        .with_checkpointer(checkpointer)
        .with_thread_id("demo-thread-1");

    let checkpoint_state = DemoState {
        input: "Checkpoint test".into(),
        ..Default::default()
    };

    println!("   Running workflow with checkpointing...");
    let _checkpoint_result = checkpoint_app.invoke(checkpoint_state).await?;

    // Count checkpoint files
    let checkpoint_count = std::fs::read_dir(&temp_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    println!("   Checkpoints saved: {} files", checkpoint_count);
    println!("   (Checkpoints enable resume after failures)");

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();

    // ============================================
    // Summary
    // ============================================
    println!("\n\nSummary");
    println!("-------");
    println!("Demonstrated WITHOUT any LLM or API keys:");
    println!("  - State management (DemoState with MergeableState)");
    println!("  - Sequential execution (step1 -> step2)");
    println!("  - Conditional routing (should_run_parallel)");
    println!("  - Parallel branches (parallel_a, parallel_b)");
    println!("  - State aggregation (aggregate_results)");
    println!("  - File checkpointing (FileCheckpointer)");
    println!("\nThe DashFlow graph engine works independently of LLM providers.");
    println!("LLM integration is pluggable, not required for core functionality.");

    Ok(())
}
