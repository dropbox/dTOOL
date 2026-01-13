//! Execution Metrics Profiling Example
//!
//! This example demonstrates how to use execution metrics to profile
//! graph performance and identify bottlenecks.
//!
//! Run with:
//! ```bash
//! cargo run --example metrics_profiling
//! ```

use dashflow::MergeableState;
use dashflow::StateGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const END: &str = "__end__";

/// Simple agent state with messages
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentState {
    messages: Vec<String>,
    step_count: usize,
}

impl MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.step_count = self.step_count.max(other.step_count);
    }
}

impl AgentState {
    fn new(initial_message: String) -> Self {
        Self {
            messages: vec![initial_message],
            step_count: 0,
        }
    }

    fn add_message(&mut self, message: String) {
        self.messages.push(message);
        self.step_count += 1;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DashFlow Execution Metrics Demo ===\n");

    // Build a graph with varying node performance characteristics
    let mut graph = StateGraph::<AgentState>::new();

    // Add nodes using add_node_from_fn
    // Fast node - completes quickly
    graph.add_node_from_fn("fast", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            state.add_message("Fast node executed".to_string());
            Ok(state)
        })
    });

    // Medium node - takes moderate time
    graph.add_node_from_fn("medium", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            state.add_message("Medium node executed".to_string());
            Ok(state)
        })
    });

    // Slow node - simulates expensive computation
    graph.add_node_from_fn("slow", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            state.add_message("Slow node executed".to_string());
            Ok(state)
        })
    });

    // Set entry point
    graph.set_entry_point("fast");

    // Add edges
    graph.add_edge("fast", "medium");
    graph.add_edge("medium", "slow");

    // Add conditional edge with routing
    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "fast".to_string());
    routes.insert("finish".to_string(), END.to_string());

    graph.add_conditional_edges(
        "slow",
        |state: &AgentState| {
            if state.step_count < 3 {
                "continue".to_string()
            } else {
                "finish".to_string()
            }
        },
        routes,
    );

    // Compile the graph
    let app = graph.compile()?;

    println!("Graph structure:");
    println!("  fast -> medium -> slow -> [conditional]");
    println!("    If step_count < 3: slow -> fast (loop)");
    println!("    Otherwise: slow -> END\n");

    // Execute the graph
    println!("Executing graph...\n");
    let initial_state = AgentState::new("Starting execution".to_string());
    let result = app.invoke(initial_state).await?;

    // Get execution metrics
    let metrics = app.metrics();

    // Display results
    println!("=== Execution Results ===");
    println!("Final state:");
    println!("  Messages: {}", result.final_state.messages.len());
    println!("  Steps: {}", result.final_state.step_count);
    println!("  Nodes executed: {:?}", result.nodes_executed);
    println!();

    // Display metrics with pretty formatting
    println!("=== Execution Metrics ===\n{}", metrics.to_string_pretty());

    // Detailed analysis
    println!("\n=== Performance Analysis ===");

    // Average node duration
    println!(
        "Average node duration: {:?}",
        metrics.average_node_duration()
    );

    // Identify bottleneck
    if let Some((node, duration)) = metrics.slowest_node() {
        println!("Performance bottleneck: {} ({:?})", node, duration);
        let percentage = metrics.node_percentage(node);
        println!(
            "  This node consumed {:.1}% of total execution time",
            percentage
        );
    }

    // Per-node analysis
    println!("\nPer-node breakdown:");
    for (node, duration) in &metrics.node_durations {
        let count = metrics.node_execution_counts.get(node).unwrap_or(&0);
        let avg_duration = *duration / (*count as u32);
        println!(
            "  {}: {:?} total, {} executions, {:?} per execution",
            node, duration, count, avg_duration
        );
    }

    // Graph-level statistics
    println!("\nGraph statistics:");
    println!("  Total edges traversed: {}", metrics.edges_traversed);
    println!("  Conditional branches: {}", metrics.conditional_branches);
    println!("  Total execution time: {:?}", metrics.total_duration);

    // Efficiency calculation
    let node_time_sum: Duration = metrics.node_durations.values().sum();
    let overhead = metrics.total_duration.saturating_sub(node_time_sum);
    let efficiency = (node_time_sum.as_secs_f64() / metrics.total_duration.as_secs_f64()) * 100.0;

    println!("\nEfficiency:");
    println!("  Node execution time: {:?}", node_time_sum);
    println!("  Framework overhead: {:?}", overhead);
    println!("  Efficiency ratio: {:.1}%", efficiency);

    Ok(())
}
