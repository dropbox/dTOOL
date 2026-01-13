//! Profile DashFlow checkpoint operations for optimization
//!
//! Run with: cargo flamegraph --package dashflow --example profile_checkpointing --release
//! Or: cargo build --release --example profile_checkpointing && cargo flamegraph --example profile_checkpointing

use dashflow::{checkpoint::MemoryCheckpointer, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct State {
    counter: u32,
    messages: Vec<String>,
    metadata: HashMap<String, String>,
    next: String,
}

impl MergeableState for State {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
        self.messages.extend(other.messages.clone());
        for (key, value) in &other.metadata {
            self.metadata.insert(key.clone(), value.clone());
        }
        if !other.next.is_empty() {
            if self.next.is_empty() {
                self.next = other.next.clone();
            } else {
                self.next.push('\n');
                self.next.push_str(&other.next);
            }
        }
    }
}

impl State {
    fn new() -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("user".to_string(), "test".to_string());
        metadata.insert("session".to_string(), "profile".to_string());

        Self {
            counter: 0,
            messages: vec!["start".to_string()],
            metadata,
            next: String::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build a simple graph with checkpointing enabled
    let mut graph: StateGraph<State> = StateGraph::new();

    // Add nodes that modify state and trigger checkpoints
    for i in 1..=5 {
        let node_id = i;
        graph.add_node_from_fn(format!("node{}", i), move |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.messages.push(format!("Node {} completed", node_id));

                // Add some state complexity (similar to benchmark)
                for j in 0..10 {
                    state
                        .metadata
                        .insert(format!("node{}_item{}", node_id, j), "value".to_string());
                }

                Ok(state)
            })
        });
    }

    // Sequential edges
    for i in 1..=4 {
        graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
    }
    graph.add_edge("node5", END);
    graph.set_entry_point("node1");

    // Compile with memory checkpointer (fast, no I/O)
    let app = graph
        .compile()?
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("profile_thread");

    // Run 1000 iterations to get meaningful profiling data
    println!("Starting checkpoint profiling run (1000 iterations)...");
    for i in 0..1000 {
        let state = State::new();
        let _ = app.invoke(state).await?;
        if i % 100 == 0 {
            println!("Completed {} iterations", i);
        }
    }
    println!("Profiling complete!");

    Ok(())
}
