//! Performance benchmarks for graph execution
//!
//! Run with: cargo bench -p dashflow-benchmarks graph
//! Generate flamegraph: cargo flamegraph --bench graph_benchmarks -- --bench

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dashflow::edge::END;
use dashflow::graph::StateGraph;
use dashflow::state::{AgentState, MergeableState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Custom State Types for Benchmarking
// ============================================================================

/// Simple state for basic benchmarks
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct SimpleState {
    counter: u32,
    data: Vec<String>,
}

impl MergeableState for SimpleState {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
        self.data.extend(other.data.iter().cloned());
    }
}

/// Large state to test state cloning overhead
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LargeState {
    messages: Vec<String>,
    embeddings: Vec<f64>,
    metadata: HashMap<String, serde_json::Value>,
    iteration: u32,
}

impl Default for LargeState {
    fn default() -> Self {
        Self {
            messages: (0..100).map(|i| format!("Message {}", i)).collect(),
            embeddings: vec![0.0; 384], // Typical embedding size
            metadata: HashMap::new(),
            iteration: 0,
        }
    }
}

impl MergeableState for LargeState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.iter().cloned());
        self.iteration = self.iteration.max(other.iteration);
    }
}

// ============================================================================
// Sequential Graph Execution
// ============================================================================

fn bench_sequential_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_sequential");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // 2-node sequential graph
    group.bench_function("2_nodes", |b| {
        b.to_async(&runtime).iter(|| async {
            let mut graph: StateGraph<SimpleState> = StateGraph::new();

            graph.add_node_from_fn("node1", |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    Ok(state)
                })
            });

            graph.add_node_from_fn("node2", |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    Ok(state)
                })
            });

            graph.add_edge("node1", "node2");
            graph.add_edge("node2", END);
            graph.set_entry_point("node1");

            let app = graph.compile().unwrap();
            app.invoke(SimpleState::default()).await.unwrap()
        });
    });

    // 5-node sequential graph
    group.bench_function("5_nodes", |b| {
        b.to_async(&runtime).iter(|| async {
            let mut graph: StateGraph<SimpleState> = StateGraph::new();

            for i in 1..=5 {
                let node_name = format!("node{}", i);
                graph.add_node_from_fn(&node_name, |mut state| {
                    Box::pin(async move {
                        state.counter += 1;
                        Ok(state)
                    })
                });
            }

            for i in 1..5 {
                graph.add_edge(&format!("node{}", i), &format!("node{}", i + 1));
            }
            graph.add_edge("node5", END);
            graph.set_entry_point("node1");

            let app = graph.compile().unwrap();
            app.invoke(SimpleState::default()).await.unwrap()
        });
    });

    // 10-node sequential graph
    group.bench_function("10_nodes", |b| {
        b.to_async(&runtime).iter(|| async {
            let mut graph: StateGraph<SimpleState> = StateGraph::new();

            for i in 1..=10 {
                let node_name = format!("node{}", i);
                graph.add_node_from_fn(&node_name, |mut state| {
                    Box::pin(async move {
                        state.counter += 1;
                        Ok(state)
                    })
                });
            }

            for i in 1..10 {
                graph.add_edge(&format!("node{}", i), &format!("node{}", i + 1));
            }
            graph.add_edge("node10", END);
            graph.set_entry_point("node1");

            let app = graph.compile().unwrap();
            app.invoke(SimpleState::default()).await.unwrap()
        });
    });

    group.finish();
}

// ============================================================================
// Pre-compiled Graph Execution (measures pure execution, not compilation)
// ============================================================================

fn bench_precompiled_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_precompiled");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Build graph once, then benchmark just the invocation
    let app_5_nodes = {
        let mut graph: StateGraph<SimpleState> = StateGraph::new();

        for i in 1..=5 {
            let node_name = format!("node{}", i);
            graph.add_node_from_fn(&node_name, |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.data.push(format!("processed"));
                    Ok(state)
                })
            });
        }

        for i in 1..5 {
            graph.add_edge(&format!("node{}", i), &format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        graph.compile().unwrap()
    };

    group.bench_function("5_nodes_invoke_only", |b| {
        b.to_async(&runtime).iter(|| async {
            app_5_nodes.invoke(SimpleState::default()).await.unwrap()
        });
    });

    // 10 node pre-compiled
    let app_10_nodes = {
        let mut graph: StateGraph<SimpleState> = StateGraph::new();

        for i in 1..=10 {
            let node_name = format!("node{}", i);
            graph.add_node_from_fn(&node_name, |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    Ok(state)
                })
            });
        }

        for i in 1..10 {
            graph.add_edge(&format!("node{}", i), &format!("node{}", i + 1));
        }
        graph.add_edge("node10", END);
        graph.set_entry_point("node1");

        graph.compile().unwrap()
    };

    group.bench_function("10_nodes_invoke_only", |b| {
        b.to_async(&runtime).iter(|| async {
            app_10_nodes.invoke(SimpleState::default()).await.unwrap()
        });
    });

    group.finish();
}

// ============================================================================
// Conditional Edge Evaluation
// ============================================================================

fn bench_conditional_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_conditional");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Build pre-compiled conditional graph
    let conditional_graph = {
        let mut graph: StateGraph<SimpleState> = StateGraph::new();

        graph.add_node_from_fn("start", |mut state| {
            Box::pin(async move {
                state.counter = 1;
                Ok(state)
            })
        });

        graph.add_node_from_fn("branch_a", |mut state| {
            Box::pin(async move {
                state.data.push("branch_a".to_string());
                Ok(state)
            })
        });

        graph.add_node_from_fn("branch_b", |mut state| {
            Box::pin(async move {
                state.data.push("branch_b".to_string());
                Ok(state)
            })
        });

        let mut routes = HashMap::new();
        routes.insert("a".to_string(), "branch_a".to_string());
        routes.insert("b".to_string(), "branch_b".to_string());

        graph.add_conditional_edges(
            "start",
            |state: &SimpleState| {
                if state.counter % 2 == 1 {
                    "a".to_string()
                } else {
                    "b".to_string()
                }
            },
            routes,
        );

        graph.add_edge("branch_a", END);
        graph.add_edge("branch_b", END);
        graph.set_entry_point("start");

        graph.compile().unwrap()
    };

    group.bench_function("simple_branch", |b| {
        b.to_async(&runtime).iter(|| async {
            conditional_graph
                .invoke(SimpleState::default())
                .await
                .unwrap()
        });
    });

    // Loop with conditional exit (tests repeated edge evaluation)
    let loop_graph = {
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
                if state.counter < 5 {
                    "continue".to_string()
                } else {
                    "end".to_string()
                }
            },
            routes,
        );

        graph.set_entry_point("process");

        graph.compile().unwrap()
    };

    group.bench_function("loop_5_iterations", |b| {
        b.to_async(&runtime).iter(|| async {
            loop_graph.invoke(SimpleState::default()).await.unwrap()
        });
    });

    group.finish();
}

// ============================================================================
// State Size Impact
// ============================================================================

fn bench_state_size_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_state_size");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Small state graph
    let small_state_graph = {
        let mut graph: StateGraph<SimpleState> = StateGraph::new();

        for i in 1..=5 {
            let node_name = format!("node{}", i);
            graph.add_node_from_fn(&node_name, |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    Ok(state)
                })
            });
        }

        for i in 1..5 {
            graph.add_edge(&format!("node{}", i), &format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        graph.compile().unwrap()
    };

    group.bench_function("small_state_5_nodes", |b| {
        b.to_async(&runtime).iter(|| async {
            small_state_graph
                .invoke(SimpleState::default())
                .await
                .unwrap()
        });
    });

    // Large state graph (same structure, bigger state)
    let large_state_graph = {
        let mut graph: StateGraph<LargeState> = StateGraph::new();

        for i in 1..=5 {
            let node_name = format!("node{}", i);
            graph.add_node_from_fn(&node_name, |mut state| {
                Box::pin(async move {
                    state.iteration += 1;
                    state.messages.push(format!("processed node {}", state.iteration));
                    Ok(state)
                })
            });
        }

        for i in 1..5 {
            graph.add_edge(&format!("node{}", i), &format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        graph.compile().unwrap()
    };

    group.bench_function("large_state_5_nodes", |b| {
        b.to_async(&runtime).iter(|| async {
            large_state_graph
                .invoke(LargeState::default())
                .await
                .unwrap()
        });
    });

    group.finish();
}

// ============================================================================
// Graph Compilation
// ============================================================================

fn bench_graph_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_compilation");

    for node_count in [5, 10, 20] {
        group.throughput(Throughput::Elements(node_count));
        group.bench_with_input(
            BenchmarkId::new("nodes", node_count),
            &node_count,
            |b, &count| {
                b.iter(|| {
                    let mut graph: StateGraph<SimpleState> = StateGraph::new();

                    for i in 1..=count {
                        let node_name = format!("node{}", i);
                        graph.add_node_from_fn(&node_name, |mut state| {
                            Box::pin(async move {
                                state.counter += 1;
                                Ok(state)
                            })
                        });
                    }

                    for i in 1..count {
                        graph.add_edge(&format!("node{}", i), &format!("node{}", i + 1));
                    }
                    graph.add_edge(&format!("node{}", count), END);
                    graph.set_entry_point("node1");

                    graph.compile().unwrap()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// AgentState (standard state type used in most applications)
// ============================================================================

fn bench_agent_state_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_agent_state");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let agent_graph = {
        let mut graph: StateGraph<AgentState> = StateGraph::new();

        graph.add_node_from_fn("process", |mut state| {
            Box::pin(async move {
                state.add_message(&format!("iteration {}", state.iteration));
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
                if state.iteration < 5 {
                    "continue".to_string()
                } else {
                    "end".to_string()
                }
            },
            routes,
        );

        graph.set_entry_point("process");

        graph.compile().unwrap()
    };

    group.bench_function("agent_loop_5_iterations", |b| {
        b.to_async(&runtime).iter(|| async {
            agent_graph.invoke(AgentState::new()).await.unwrap()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sequential_execution,
    bench_precompiled_execution,
    bench_conditional_execution,
    bench_state_size_impact,
    bench_graph_compilation,
    bench_agent_state_execution,
);
criterion_main!(benches);
