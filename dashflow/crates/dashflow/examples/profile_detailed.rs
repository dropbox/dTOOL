//! Detailed profiling example for graph execution
//!
//! This example measures time spent in different parts of the execution.

use dashflow::edge::END;
use dashflow::error::Result;
use dashflow::graph::StateGraph;
use dashflow::state::MergeableState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Simple state for minimal overhead
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct SimpleState {
    counter: u32,
}

impl MergeableState for SimpleState {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
    }
}

/// Medium state with some data
#[derive(Clone, Debug, Serialize, Deserialize)]
struct MediumState {
    counter: u32,
    messages: Vec<String>,
}

impl Default for MediumState {
    fn default() -> Self {
        Self {
            counter: 0,
            messages: (0..10).map(|i| format!("msg{}", i)).collect(),
        }
    }
}

impl MergeableState for MediumState {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
        self.messages.extend(other.messages.iter().cloned());
    }
}

/// Large state
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LargeState {
    counter: u32,
    messages: Vec<String>,
    data: Vec<f64>,
    metadata: HashMap<String, String>,
}

impl Default for LargeState {
    fn default() -> Self {
        Self {
            counter: 0,
            messages: (0..50).map(|i| format!("message {}", i)).collect(),
            data: vec![0.0; 128],
            metadata: (0..20).map(|i| (format!("key{}", i), format!("value{}", i))).collect(),
        }
    }
}

impl MergeableState for LargeState {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
        self.messages.extend(other.messages.iter().cloned());
    }
}

fn measure<F: FnOnce() -> T, T>(name: &str, f: F) -> (T, Duration) {
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    println!("  {}: {:?}", name, elapsed);
    (result, elapsed)
}

async fn measure_async<F, Fut, T>(name: &str, f: F) -> (T, Duration)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let start = Instant::now();
    let result = f().await;
    let elapsed = start.elapsed();
    println!("  {}: {:?}", name, elapsed);
    (result, elapsed)
}

#[tokio::main]
async fn main() -> Result<()> {
    let iterations = 50;

    println!("=== Test 1: SimpleState (minimal) ===");
    {
        let (app_result, _compile_time) = measure("Graph compilation", || {
            let mut graph: StateGraph<SimpleState> = StateGraph::new();
            for i in 1..=5 {
                let name = format!("node{}", i);
                graph.add_node_from_fn(&name, |mut state| {
                    Box::pin(async move {
                        state.counter += 1;
                        Ok(state)
                    })
                });
            }
            for i in 1..5 {
                graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
            }
            graph.add_edge("node5", END);
            graph.set_entry_point("node1");
            graph.compile()
        });
        let app = app_result?;

        let (invoke_result, _invoke_time) =
            measure_async("Single invocation", || async { app.invoke(SimpleState::default()).await }).await;
        let _ = invoke_result?;

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = app.invoke(SimpleState::default()).await?;
        }
        let total = start.elapsed();
        println!("  {} iterations: {:?} ({:.2} ms/iter)",
                 iterations, total, total.as_secs_f64() * 1000.0 / iterations as f64);
    }

    println!("\n=== Test 2: MediumState (10 strings) ===");
    {
        let app = {
            let mut graph: StateGraph<MediumState> = StateGraph::new();
            for i in 1..=5 {
                let name = format!("node{}", i);
                graph.add_node_from_fn(&name, |mut state| {
                    Box::pin(async move {
                        state.counter += 1;
                        Ok(state)
                    })
                });
            }
            for i in 1..5 {
                graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
            }
            graph.add_edge("node5", END);
            graph.set_entry_point("node1");
            graph.compile()?
        };

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = app.invoke(MediumState::default()).await?;
        }
        let total = start.elapsed();
        println!("  {} iterations: {:?} ({:.2} ms/iter)",
                 iterations, total, total.as_secs_f64() * 1000.0 / iterations as f64);
    }

    println!("\n=== Test 3: LargeState (50 strings + 128 floats + 20 map entries) ===");
    {
        let app = {
            let mut graph: StateGraph<LargeState> = StateGraph::new();
            for i in 1..=5 {
                let name = format!("node{}", i);
                graph.add_node_from_fn(&name, |mut state| {
                    Box::pin(async move {
                        state.counter += 1;
                        Ok(state)
                    })
                });
            }
            for i in 1..5 {
                graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
            }
            graph.add_edge("node5", END);
            graph.set_entry_point("node1");
            graph.compile()?
        };

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = app.invoke(LargeState::default()).await?;
        }
        let total = start.elapsed();
        println!("  {} iterations: {:?} ({:.2} ms/iter)",
                 iterations, total, total.as_secs_f64() * 1000.0 / iterations as f64);
    }

    println!("\n=== Test 4: 10-node graph with SimpleState ===");
    {
        let app = {
            let mut graph: StateGraph<SimpleState> = StateGraph::new();
            for i in 1..=10 {
                let name = format!("node{}", i);
                graph.add_node_from_fn(&name, |mut state| {
                    Box::pin(async move {
                        state.counter += 1;
                        Ok(state)
                    })
                });
            }
            for i in 1..10 {
                graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
            }
            graph.add_edge("node10", END);
            graph.set_entry_point("node1");
            graph.compile()?
        };

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = app.invoke(SimpleState::default()).await?;
        }
        let total = start.elapsed();
        println!("  {} iterations: {:?} ({:.2} ms/iter)",
                 iterations, total, total.as_secs_f64() * 1000.0 / iterations as f64);
    }

    println!("\n=== Test 5: Conditional loop (10 iterations) with SimpleState ===");
    {
        let app = {
            let mut graph: StateGraph<SimpleState> = StateGraph::new();
            graph.add_node_from_fn("process", |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    Ok(state)
                })
            });

            let mut routes = HashMap::new();
            routes.insert("continue".to_string(), "process".to_string());
            routes.insert("end".to_string(), END.to_string());

            graph.add_conditional_edges(
                "process",
                |state: &SimpleState| {
                    if state.counter < 10 {
                        "continue".to_string()
                    } else {
                        "end".to_string()
                    }
                },
                routes,
            );

            graph.set_entry_point("process");
            graph.compile()?
        };

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = app.invoke(SimpleState::default()).await?;
        }
        let total = start.elapsed();
        println!("  {} iterations: {:?} ({:.2} ms/iter)",
                 iterations, total, total.as_secs_f64() * 1000.0 / iterations as f64);
    }

    println!("\n=== Test 6: Pure function call baseline (no graph) ===");
    {
        async fn process_state(mut state: SimpleState) -> SimpleState {
            state.counter += 1;
            state
        }

        let start = Instant::now();
        for _ in 0..iterations {
            let mut state = SimpleState::default();
            for _ in 0..5 {
                state = process_state(state).await;
            }
        }
        let total = start.elapsed();
        println!("  {} iterations (5 fn calls each): {:?} ({:.2} ms/iter)",
                 iterations, total, total.as_secs_f64() * 1000.0 / iterations as f64);
    }

    println!("\n=== Summary ===");
    println!("The baseline (Test 6) shows pure function overhead.");
    println!("The difference between Test 6 and Test 1 is graph framework overhead.");
    println!("The difference between Test 1 and Test 3 is state size overhead.");
    Ok(())
}
