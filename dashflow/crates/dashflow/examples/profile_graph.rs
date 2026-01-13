//! Profiling example for graph execution
//!
//! Run with flamegraph:
//!   cargo flamegraph --example profile_graph --release
//!
//! This runs multiple graph iterations to get meaningful profiling data.

use dashflow::edge::END;
use dashflow::error::Result;
use dashflow::graph::StateGraph;
use dashflow::state::{AgentState, MergeableState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Large state to exercise state cloning paths
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProfileState {
    messages: Vec<String>,
    data: Vec<f64>,
    iteration: u32,
    metadata: HashMap<String, String>,
}

impl Default for ProfileState {
    fn default() -> Self {
        Self {
            messages: (0..50).map(|i| format!("Message {}", i)).collect(),
            data: vec![0.0; 128],
            iteration: 0,
            metadata: HashMap::new(),
        }
    }
}

impl MergeableState for ProfileState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.iter().cloned());
        self.iteration = self.iteration.max(other.iteration);
    }
}

fn build_sequential_graph() -> Result<dashflow::executor::CompiledGraph<ProfileState>> {
    let mut graph: StateGraph<ProfileState> = StateGraph::new();

    // 10-node sequential graph
    for i in 1..=10 {
        let node_name = format!("node{}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            Box::pin(async move {
                state.iteration += 1;
                state.messages.push(format!("node{} processed", state.iteration));
                state.metadata.insert(format!("step_{}", state.iteration), "done".to_string());
                Ok(state)
            })
        });
    }

    for i in 1..10 {
        graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
    }
    graph.add_edge("node10", END);
    graph.set_entry_point("node1");

    graph.compile()
}

fn build_conditional_graph() -> Result<dashflow::executor::CompiledGraph<ProfileState>> {
    let mut graph: StateGraph<ProfileState> = StateGraph::new();

    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            state.iteration += 1;
            state.messages.push(format!("iteration {}", state.iteration));
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "process".to_string());
    routes.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "process",
        |state: &ProfileState| {
            if state.iteration < 10 {
                "continue".to_string()
            } else {
                "end".to_string()
            }
        },
        routes,
    );

    graph.set_entry_point("process");
    graph.compile()
}

fn build_agent_state_graph() -> Result<dashflow::executor::CompiledGraph<AgentState>> {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            state.add_message(format!("iteration {}", state.iteration));
            state.iteration += 1;
            Ok(state)
        })
    });

    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "process".to_string());
    routes.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "process",
        |state: &AgentState| {
            if state.iteration < 10 {
                "continue".to_string()
            } else {
                "end".to_string()
            }
        },
        routes,
    );

    graph.set_entry_point("process");
    graph.compile()
}

#[tokio::main]
async fn main() -> Result<()> {
    let iterations = 20; // Reduced for faster profiling

    println!("Building graphs...");
    let sequential_graph = build_sequential_graph()?;
    let conditional_graph = build_conditional_graph()?;
    let agent_graph = build_agent_state_graph()?;

    println!("Running {} iterations of sequential graph (10 nodes)...", iterations);
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = sequential_graph.invoke(ProfileState::default()).await?;
    }
    let sequential_time = start.elapsed();
    println!("  Completed in {:?} ({:.2} ms/iteration)",
             sequential_time,
             sequential_time.as_secs_f64() * 1000.0 / iterations as f64);

    println!("Running {} iterations of conditional graph (10 iterations each)...", iterations);
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = conditional_graph.invoke(ProfileState::default()).await?;
    }
    let conditional_time = start.elapsed();
    println!("  Completed in {:?} ({:.2} ms/iteration)",
             conditional_time,
             conditional_time.as_secs_f64() * 1000.0 / iterations as f64);

    println!("Running {} iterations of AgentState graph (10 iterations each)...", iterations);
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = agent_graph.invoke(AgentState::new()).await?;
    }
    let agent_time = start.elapsed();
    println!("  Completed in {:?} ({:.2} ms/iteration)",
             agent_time,
             agent_time.as_secs_f64() * 1000.0 / iterations as f64);

    println!("\nSummary:");
    println!("  Sequential (10 nodes, large state): {:.2} ms/iteration",
             sequential_time.as_secs_f64() * 1000.0 / iterations as f64);
    println!("  Conditional (10 iterations, large state): {:.2} ms/iteration",
             conditional_time.as_secs_f64() * 1000.0 / iterations as f64);
    println!("  AgentState (10 iterations, small state): {:.2} ms/iteration",
             agent_time.as_secs_f64() * 1000.0 / iterations as f64);

    println!("\nProfiling complete. Use 'cargo flamegraph --example profile_graph --release' to generate flamegraph.");
    Ok(())
}
