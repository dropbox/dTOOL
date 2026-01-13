//! Profile DashFlow execution for optimization
//!
//! Run with: cargo flamegraph --package dashflow --example profile_execution --release
//! Or: cargo build --release --example profile_execution && cargo flamegraph --example profile_execution

use dashflow::{MergeableState, StateGraph, END};
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
    // Build a representative graph (sequential + conditional + parallel)
    let mut graph: StateGraph<State> = StateGraph::new();

    // Sequential nodes
    graph.add_node_from_fn("node1", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            state.messages.push("node1".to_string());
            for i in 0..10 {
                state
                    .metadata
                    .insert(format!("node1_key{}", i), format!("value{}", i));
            }
            Ok(state)
        })
    });

    graph.add_node_from_fn("node2", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            state.messages.push("node2".to_string());
            for i in 0..10 {
                state
                    .metadata
                    .insert(format!("node2_key{}", i), format!("value{}", i));
            }
            Ok(state)
        })
    });

    // Conditional router
    graph.add_node_from_fn("router", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            state.next = if state.counter % 2 == 0 {
                "branch_a".to_string()
            } else {
                "branch_b".to_string()
            };
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_a", |mut state| {
        Box::pin(async move {
            state.messages.push("branch_a".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("branch_b", |mut state| {
        Box::pin(async move {
            state.messages.push("branch_b".to_string());
            Ok(state)
        })
    });

    // Parallel workers
    graph.add_node_from_fn("dispatcher", |mut state| {
        Box::pin(async move {
            state.messages.push("dispatching".to_string());
            Ok(state)
        })
    });

    for i in 1..=3 {
        let worker_id = i;
        graph.add_node_from_fn(format!("worker{}", i), move |mut state| {
            Box::pin(async move {
                state.messages.push(format!("worker{}", worker_id));
                for j in 0..10 {
                    state.metadata.insert(
                        format!("worker{}_item{}", worker_id, j),
                        format!("result{}", j),
                    );
                }
                Ok(state)
            })
        });
    }

    graph.add_node_from_fn("collector", |mut state| {
        Box::pin(async move {
            state.messages.push("collected".to_string());
            Ok(state)
        })
    });

    // Build graph
    graph.add_edge("node1", "node2");
    graph.add_edge("node2", "router");

    let mut routes = HashMap::new();
    routes.insert("branch_a".to_string(), "branch_a".to_string());
    routes.insert("branch_b".to_string(), "branch_b".to_string());
    graph.add_conditional_edges("router", |state: &State| state.next.clone(), routes);

    graph.add_edge("branch_a", "dispatcher");
    graph.add_edge("branch_b", "dispatcher");

    graph.add_parallel_edges(
        "dispatcher",
        vec![
            "worker1".to_string(),
            "worker2".to_string(),
            "worker3".to_string(),
        ],
    );

    for i in 1..=3 {
        graph.add_edge(format!("worker{}", i), "collector");
    }
    graph.add_edge("collector", END);

    graph.set_entry_point("node1");

    let app = graph.compile()?;

    // Run 1000 iterations to get meaningful profiling data
    println!("Starting profiling run (1000 iterations)...");
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
