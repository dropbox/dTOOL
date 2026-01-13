//! Performance benchmarks for DashFlow
//!
//! Run with: cargo bench --package dashflow
//! Run specific group: cargo bench --package dashflow compilation

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{criterion_group, criterion_main, Criterion};
use dashflow::{FileCheckpointer, MemoryCheckpointer, MergeableState, StateGraph, StreamMode, END};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// State Definitions
// ============================================================================

/// Simple state for basic benchmarks
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SimpleState {
    counter: u32,
    data: String,
}

impl SimpleState {
    fn new() -> Self {
        Self {
            counter: 0,
            data: String::new(),
        }
    }
}

impl MergeableState for SimpleState {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
        if !other.data.is_empty() {
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(&other.data);
        }
    }
}

/// More complex state for realistic benchmarks
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ComplexState {
    messages: Vec<String>,
    metadata: HashMap<String, String>,
    counter: u32,
    status: String,
    next: String,
}

impl ComplexState {
    fn new() -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("user".to_string(), "test_user".to_string());
        metadata.insert("session".to_string(), "test_session".to_string());

        Self {
            messages: vec!["Initial message".to_string()],
            metadata,
            counter: 0,
            status: "initialized".to_string(),
            next: String::new(),
        }
    }
}

impl MergeableState for ComplexState {
    fn merge(&mut self, other: &Self) {
        // Merge messages (append)
        self.messages.extend(other.messages.clone());
        // Merge metadata (combine keys, last value wins for conflicts)
        for (k, v) in other.metadata.iter() {
            self.metadata.insert(k.clone(), v.clone());
        }
        // Take max counter
        self.counter = self.counter.max(other.counter);
        // Take other's status (last value wins)
        self.status = other.status.clone();
    }
}

/// Small state for cloning benchmarks (< 1 KB)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SmallState {
    counter: u32,
    status: String,
    value: f64,
}

impl SmallState {
    fn new() -> Self {
        Self {
            counter: 0,
            status: "ready".to_string(),
            value: 0.0,
        }
    }
}

impl MergeableState for SmallState {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
        self.status = other.status.clone();
        self.value = self.value.max(other.value);
    }
}

/// Medium state for cloning benchmarks (1-10 KB)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct MediumState {
    data: Vec<String>,
    metadata: HashMap<String, String>,
    counters: Vec<u32>,
}

impl MediumState {
    fn new() -> Self {
        // ~5 KB state
        let data = (0..100).map(|i| format!("Item {}", i)).collect();
        let mut metadata = HashMap::new();
        for i in 0..50 {
            metadata.insert(format!("key{}", i), format!("value{}", i));
        }
        let counters = (0..100).collect();

        Self {
            data,
            metadata,
            counters,
        }
    }
}

impl MergeableState for MediumState {
    fn merge(&mut self, other: &Self) {
        self.data.extend(other.data.clone());
        for (k, v) in other.metadata.iter() {
            self.metadata.insert(k.clone(), v.clone());
        }
        self.counters.extend(other.counters.clone());
    }
}

/// Large state for cloning benchmarks (> 100 KB)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LargeState {
    messages: Vec<String>,
    metadata: HashMap<String, String>,
    data_blocks: Vec<Vec<u8>>,
}

impl LargeState {
    fn new() -> Self {
        // ~200 KB state
        let messages = (0..1000).map(|i| format!("Message {}", i)).collect();
        let mut metadata = HashMap::new();
        for i in 0..500 {
            metadata.insert(format!("key{}", i), format!("value{}", i));
        }
        // 10 blocks of 10KB each
        let data_blocks = (0..10).map(|_| vec![0u8; 10240]).collect();

        Self {
            messages,
            metadata,
            data_blocks,
        }
    }
}

impl MergeableState for LargeState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        for (k, v) in other.metadata.iter() {
            self.metadata.insert(k.clone(), v.clone());
        }
        self.data_blocks.extend(other.data_blocks.clone());
    }
}

// ============================================================================
// Graph Compilation Benchmarks
// ============================================================================

fn bench_graph_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation");

    // Simple graph compilation (3 nodes, linear)
    group.bench_function("simple_graph_3_nodes", |b| {
        b.iter(|| {
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

            graph.add_node_from_fn("node3", |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    Ok(state)
                })
            });

            graph.add_edge("node1", "node2");
            graph.add_edge("node2", "node3");
            graph.add_edge("node3", END);
            graph.set_entry_point("node1");

            graph.compile().unwrap()
        });
    });

    // Complex graph compilation (10 nodes, mixed edges)
    group.bench_function("complex_graph_10_nodes", |b| {
        b.iter(|| {
            let mut graph: StateGraph<ComplexState> = StateGraph::new();

            // Add 10 nodes
            for i in 0..10 {
                let node_name = format!("node{}", i);
                graph.add_node_from_fn(node_name.clone(), move |mut state| {
                    Box::pin(async move {
                        state.counter += 1;
                        state.messages.push(format!("Processed by node{}", i));
                        Ok(state)
                    })
                });
            }

            // Add linear edges
            for i in 0..9 {
                graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
            }
            graph.add_edge("node9", END);
            graph.set_entry_point("node0");

            graph.compile().unwrap()
        });
    });

    // Graph with conditional edges
    group.bench_function("graph_with_conditionals", |b| {
        b.iter(|| {
            let mut graph: StateGraph<ComplexState> = StateGraph::new();

            graph.add_node_from_fn("start", |mut state| {
                Box::pin(async move {
                    state.counter = 0;
                    state.next = "process".to_string();
                    Ok(state)
                })
            });

            graph.add_node_from_fn("process", |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.next = if state.counter >= 3 {
                        "end".to_string()
                    } else {
                        "continue".to_string()
                    };
                    Ok(state)
                })
            });

            graph.add_edge("start", "process");

            let mut routes = HashMap::new();
            routes.insert("continue".to_string(), "process".to_string());
            routes.insert("end".to_string(), END.to_string());

            graph.add_conditional_edges(
                "process",
                |state: &ComplexState| state.next.clone(),
                routes,
            );

            graph.set_entry_point("start");

            graph.compile().unwrap()
        });
    });

    group.finish();
}

// ============================================================================
// Sequential Execution Benchmarks
// ============================================================================

fn bench_sequential_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_execution");

    // 3-node sequential
    group.bench_function("3_nodes_simple", |b| {
        let mut graph: StateGraph<SimpleState> = StateGraph::new();

        graph.add_node_from_fn("node1", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.data.push_str("node1 ");
                Ok(state)
            })
        });

        graph.add_node_from_fn("node2", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.data.push_str("node2 ");
                Ok(state)
            })
        });

        graph.add_node_from_fn("node3", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.data.push_str("node3 ");
                Ok(state)
            })
        });

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = SimpleState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // 5-node sequential with complex state
    group.bench_function("5_nodes_complex", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 0..5 {
            let node_name = format!("node{}", i);
            graph.add_node_from_fn(node_name.clone(), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Processed by node{}", i));
                    state
                        .metadata
                        .insert(format!("step{}", i), "completed".to_string());
                    Ok(state)
                })
            });
        }

        for i in 0..4 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node4", END);
        graph.set_entry_point("node0");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // 10-node sequential (stress test)
    group.bench_function("10_nodes_stress", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 0..10 {
            let node_name = format!("node{}", i);
            graph.add_node_from_fn(node_name.clone(), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {} processing", i));
                    // Simulate some work
                    for j in 0..10 {
                        state
                            .metadata
                            .insert(format!("node{}_item{}", i, j), "value".to_string());
                    }
                    Ok(state)
                })
            });
        }

        for i in 0..9 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node9", END);
        graph.set_entry_point("node0");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    group.finish();
}

// ============================================================================
// Conditional Branching Benchmarks
// ============================================================================

fn bench_conditional_branching(c: &mut Criterion) {
    let mut group = c.benchmark_group("conditional_branching");

    // Simple binary conditional
    group.bench_function("binary_conditional", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("start", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.next = if state.counter % 2 == 0 {
                    "even".to_string()
                } else {
                    "odd".to_string()
                };
                Ok(state)
            })
        });

        graph.add_node_from_fn("even", |mut state| {
            Box::pin(async move {
                state.messages.push("Even path".to_string());
                Ok(state)
            })
        });

        graph.add_node_from_fn("odd", |mut state| {
            Box::pin(async move {
                state.messages.push("Odd path".to_string());
                Ok(state)
            })
        });

        let mut routes = HashMap::new();
        routes.insert("even".to_string(), "even".to_string());
        routes.insert("odd".to_string(), "odd".to_string());

        graph.add_conditional_edges("start", |state: &ComplexState| state.next.clone(), routes);

        graph.add_edge("even", END);
        graph.add_edge("odd", END);
        graph.set_entry_point("start");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Loop with conditional exit
    group.bench_function("loop_with_exit_condition", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("processor", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.messages.push(format!("Iteration {}", state.counter));
                state.next = if state.counter >= 5 {
                    "end".to_string()
                } else {
                    "continue".to_string()
                };
                Ok(state)
            })
        });

        let mut routes = HashMap::new();
        routes.insert("continue".to_string(), "processor".to_string());
        routes.insert("end".to_string(), END.to_string());

        graph.add_conditional_edges(
            "processor",
            |state: &ComplexState| state.next.clone(),
            routes,
        );

        graph.set_entry_point("processor");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Multi-branch conditional (4 branches)
    group.bench_function("multi_branch_4_routes", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("router", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                let route = match state.counter % 4 {
                    0 => "route_a",
                    1 => "route_b",
                    2 => "route_c",
                    _ => "route_d",
                };
                state.next = route.to_string();
                Ok(state)
            })
        });

        for route in ["route_a", "route_b", "route_c", "route_d"] {
            let route_name = route.to_string();
            graph.add_node_from_fn(route.to_string(), move |mut state| {
                let name = route_name.clone();
                Box::pin(async move {
                    state.messages.push(format!("Processed by {}", name));
                    Ok(state)
                })
            });
        }

        let mut routes = HashMap::new();
        for route in ["route_a", "route_b", "route_c", "route_d"] {
            routes.insert(route.to_string(), route.to_string());
        }

        graph.add_conditional_edges("router", |state: &ComplexState| state.next.clone(), routes);

        for route in ["route_a", "route_b", "route_c", "route_d"] {
            graph.add_edge(route, END);
        }

        graph.set_entry_point("router");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    group.finish();
}

// ============================================================================
// Parallel Execution Benchmarks
// ============================================================================

fn bench_parallel_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_execution");

    // Fan-out/fan-in: 3 parallel workers
    group.bench_function("fanout_3_workers", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("start", |mut state| {
            Box::pin(async move {
                state.counter = 0;
                state.status = "distributing".to_string();
                Ok(state)
            })
        });

        // Three parallel workers
        for i in 1..=3 {
            let worker_id = i;
            graph.add_node_from_fn(format!("worker{}", i), move |mut state| {
                Box::pin(async move {
                    state
                        .messages
                        .push(format!("Worker {} processing", worker_id));
                    state.metadata.insert(
                        format!("worker{}_result", worker_id),
                        "completed".to_string(),
                    );
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("collect", |mut state| {
            Box::pin(async move {
                state.status = "collected".to_string();
                state.counter = state.metadata.len() as u32;
                Ok(state)
            })
        });

        // Fan-out to 3 workers
        graph.add_parallel_edges(
            "start",
            vec![
                "worker1".to_string(),
                "worker2".to_string(),
                "worker3".to_string(),
            ],
        );

        // Fan-in from all workers to collector
        graph.add_edge("worker1", "collect");
        graph.add_edge("worker2", "collect");
        graph.add_edge("worker3", "collect");
        graph.add_edge("collect", END);

        graph.set_entry_point("start");

        let app = graph.compile_with_merge().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Fan-out/fan-in: 5 parallel workers with more work
    group.bench_function("fanout_5_workers_heavy", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("dispatcher", |mut state| {
            Box::pin(async move {
                state.counter = 0;
                state.status = "dispatching".to_string();
                Ok(state)
            })
        });

        // Five parallel workers with heavier workload
        for i in 1..=5 {
            let worker_id = i;
            graph.add_node_from_fn(format!("worker{}", i), move |mut state| {
                Box::pin(async move {
                    state.messages.push(format!("Worker {} started", worker_id));
                    // Simulate more work
                    for j in 0..20 {
                        state.metadata.insert(
                            format!("worker{}_item{}", worker_id, j),
                            format!("result_{}", j),
                        );
                    }
                    state
                        .messages
                        .push(format!("Worker {} completed", worker_id));
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("aggregator", |mut state| {
            Box::pin(async move {
                state.status = "aggregated".to_string();
                state.counter = state.messages.len() as u32;
                Ok(state)
            })
        });

        // Fan-out to 5 workers
        let workers: Vec<String> = (1..=5).map(|i| format!("worker{}", i)).collect();
        graph.add_parallel_edges("dispatcher", workers.clone());

        // Fan-in from all workers to aggregator
        for worker in &workers {
            graph.add_edge(worker.as_str(), "aggregator");
        }
        graph.add_edge("aggregator", END);

        graph.set_entry_point("dispatcher");

        let app = graph.compile_with_merge().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Fan-out with conditional fan-in (2 stages of parallel execution)
    group.bench_function("two_stage_parallel", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("stage1_start", |mut state| {
            Box::pin(async move {
                state.status = "stage1".to_string();
                Ok(state)
            })
        });

        // Stage 1: 3 parallel processors
        for i in 1..=3 {
            let proc_id = i;
            graph.add_node_from_fn(format!("stage1_proc{}", i), move |mut state| {
                Box::pin(async move {
                    state
                        .metadata
                        .insert(format!("stage1_{}", proc_id), "done".to_string());
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("stage2_start", |mut state| {
            Box::pin(async move {
                state.status = "stage2".to_string();
                Ok(state)
            })
        });

        // Stage 2: 3 parallel processors
        for i in 1..=3 {
            let proc_id = i;
            graph.add_node_from_fn(format!("stage2_proc{}", i), move |mut state| {
                Box::pin(async move {
                    state
                        .metadata
                        .insert(format!("stage2_{}", proc_id), "done".to_string());
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("final", |mut state| {
            Box::pin(async move {
                state.status = "complete".to_string();
                Ok(state)
            })
        });

        // Stage 1 fan-out
        graph.add_parallel_edges(
            "stage1_start",
            vec![
                "stage1_proc1".to_string(),
                "stage1_proc2".to_string(),
                "stage1_proc3".to_string(),
            ],
        );

        // Stage 1 fan-in to stage 2
        for i in 1..=3 {
            graph.add_edge(format!("stage1_proc{}", i), "stage2_start");
        }

        // Stage 2 fan-out
        graph.add_parallel_edges(
            "stage2_start",
            vec![
                "stage2_proc1".to_string(),
                "stage2_proc2".to_string(),
                "stage2_proc3".to_string(),
            ],
        );

        // Stage 2 fan-in to final
        for i in 1..=3 {
            graph.add_edge(format!("stage2_proc{}", i), "final");
        }
        graph.add_edge("final", END);

        graph.set_entry_point("stage1_start");

        let app = graph.compile_with_merge().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    group.finish();
}

// ============================================================================
// Checkpointing Benchmarks
// ============================================================================

fn bench_checkpointing(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpointing");

    // Checkpoint save/load: 3-node graph with memory checkpointer
    group.bench_function("memory_checkpoint_3_nodes", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=3 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {} completed", node_id));
                    Ok(state)
                })
            });
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let app = graph
            .compile()
            .unwrap()
            .with_checkpointer(MemoryCheckpointer::new())
            .with_thread_id("bench_thread_1");

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Checkpoint save/load: 5-node graph with memory checkpointer
    group.bench_function("memory_checkpoint_5_nodes", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=5 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {} completed", node_id));
                    // Add some state complexity
                    for j in 0..10 {
                        state
                            .metadata
                            .insert(format!("node{}_item{}", node_id, j), "value".to_string());
                    }
                    Ok(state)
                })
            });
        }

        for i in 1..=4 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        let app = graph
            .compile()
            .unwrap()
            .with_checkpointer(MemoryCheckpointer::new())
            .with_thread_id("bench_thread_2");

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Checkpoint with loop (multiple checkpoints)
    group.bench_function("memory_checkpoint_loop_5_iterations", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("processor", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.messages.push(format!("Iteration {}", state.counter));
                state.next = if state.counter >= 5 {
                    "end".to_string()
                } else {
                    "continue".to_string()
                };
                Ok(state)
            })
        });

        let mut routes = HashMap::new();
        routes.insert("continue".to_string(), "processor".to_string());
        routes.insert("end".to_string(), END.to_string());

        graph.add_conditional_edges(
            "processor",
            |state: &ComplexState| state.next.clone(),
            routes,
        );

        graph.set_entry_point("processor");

        let app = graph
            .compile()
            .unwrap()
            .with_checkpointer(MemoryCheckpointer::new())
            .with_thread_id("bench_thread_3");

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // File checkpoint save/load: 3-node graph (bincode + buffered I/O)
    group.bench_function("file_checkpoint_3_nodes", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=3 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {} completed", node_id));
                    Ok(state)
                })
            });
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        // Create unique temp directory for each iteration
        let unique_id = uuid::Uuid::new_v4().to_string();
        let temp_dir = std::env::temp_dir().join(format!("dashflow_bench_{}", unique_id));
        let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();

        let app = graph
            .compile()
            .unwrap()
            .with_checkpointer(checkpointer)
            .with_thread_id("bench_file_thread_1");

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                let result = app.invoke(state).await.unwrap();
                result
            });

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    });

    // File checkpoint save/load: 5-node graph with complex state
    group.bench_function("file_checkpoint_5_nodes", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=5 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {} completed", node_id));
                    // Add some state complexity
                    for j in 0..10 {
                        state
                            .metadata
                            .insert(format!("node{}_item{}", node_id, j), "value".to_string());
                    }
                    Ok(state)
                })
            });
        }

        for i in 1..=4 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        // Create unique temp directory
        let unique_id = uuid::Uuid::new_v4().to_string();
        let temp_dir = std::env::temp_dir().join(format!("dashflow_bench_{}", unique_id));
        let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();

        let app = graph
            .compile()
            .unwrap()
            .with_checkpointer(checkpointer)
            .with_thread_id("bench_file_thread_2");

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                let result = app.invoke(state).await.unwrap();
                result
            });

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    });

    group.finish();
}

// ============================================================================
// Event Streaming Benchmarks
// ============================================================================

fn bench_event_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_streaming");

    // Stream values: 5-node sequential
    group.bench_function("stream_values_5_nodes", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=5 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {}", node_id));
                    Ok(state)
                })
            });
        }

        for i in 1..=4 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                let mut stream = Box::pin(app.stream(state, StreamMode::Values));
                let mut count = 0;
                while let Some(event) = stream.next().await {
                    event.unwrap();
                    count += 1;
                }
                count
            });
    });

    // Stream events: 5-node sequential
    group.bench_function("stream_events_5_nodes", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=5 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {}", node_id));
                    Ok(state)
                })
            });
        }

        for i in 1..=4 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                let mut stream = Box::pin(app.stream(state, StreamMode::Events));
                let mut count = 0;
                while let Some(event) = stream.next().await {
                    event.unwrap();
                    count += 1;
                }
                count
            });
    });

    // Stream updates: 3-node with complex state
    group.bench_function("stream_updates_3_nodes_complex", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=3 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {}", node_id));
                    // Add complex state changes
                    for j in 0..10 {
                        state
                            .metadata
                            .insert(format!("node{}_item{}", node_id, j), "value".to_string());
                    }
                    Ok(state)
                })
            });
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                let mut stream = Box::pin(app.stream(state, StreamMode::Updates));
                let mut count = 0;
                while let Some(event) = stream.next().await {
                    event.unwrap();
                    count += 1;
                }
                count
            });
    });

    // Stream parallel execution
    group.bench_function("stream_parallel_3_workers", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("start", |mut state| {
            Box::pin(async move {
                state.status = "started".to_string();
                Ok(state)
            })
        });

        for i in 1..=3 {
            let worker_id = i;
            graph.add_node_from_fn(format!("worker{}", i), move |mut state| {
                Box::pin(async move {
                    state.messages.push(format!("Worker {}", worker_id));
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("collect", |mut state| {
            Box::pin(async move {
                state.status = "collected".to_string();
                Ok(state)
            })
        });

        graph.add_parallel_edges(
            "start",
            vec![
                "worker1".to_string(),
                "worker2".to_string(),
                "worker3".to_string(),
            ],
        );

        for i in 1..=3 {
            graph.add_edge(format!("worker{}", i), "collect");
        }
        graph.add_edge("collect", END);
        graph.set_entry_point("start");

        let app = graph.compile_with_merge().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                let mut stream = Box::pin(app.stream(state, StreamMode::Values));
                let mut count = 0;
                while let Some(event) = stream.next().await {
                    event.unwrap();
                    count += 1;
                }
                count
            });
    });

    group.finish();
}

// ============================================================================
// State Cloning Benchmarks
// ============================================================================

fn bench_state_cloning(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_cloning");

    // Small state cloning (< 1 KB)
    group.bench_function("small_state_clone", |b| {
        let state = SmallState::new();
        b.iter(|| {
            let _cloned = state.clone();
        });
    });

    // Small state execution (3 nodes) - measures actual cloning overhead in graph
    group.bench_function("small_state_3_nodes", |b| {
        let mut graph: StateGraph<SmallState> = StateGraph::new();

        for i in 1..=3 {
            graph.add_node_from_fn(format!("node{}", i), |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.value += 1.5;
                    Ok(state)
                })
            });
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = SmallState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Medium state cloning (1-10 KB)
    group.bench_function("medium_state_clone", |b| {
        let state = MediumState::new();
        b.iter(|| {
            let _cloned = state.clone();
        });
    });

    // Medium state execution (3 nodes)
    group.bench_function("medium_state_3_nodes", |b| {
        let mut graph: StateGraph<MediumState> = StateGraph::new();

        for i in 1..=3 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.data.push(format!("Node {} processed", node_id));
                    state.counters[node_id % 100] += 1;
                    Ok(state)
                })
            });
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = MediumState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Large state cloning (> 100 KB)
    group.bench_function("large_state_clone", |b| {
        let state = LargeState::new();
        b.iter(|| {
            let _cloned = state.clone();
        });
    });

    // Large state execution (3 nodes)
    group.bench_function("large_state_3_nodes", |b| {
        let mut graph: StateGraph<LargeState> = StateGraph::new();

        for i in 1..=3 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.messages.push(format!("Node {} processed", node_id));
                    if node_id < 10 {
                        state.data_blocks[node_id][0] = node_id as u8;
                    }
                    Ok(state)
                })
            });
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = LargeState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Parallel execution with medium state (measures cloning per worker)
    group.bench_function("medium_state_parallel_3_workers", |b| {
        let mut graph: StateGraph<MediumState> = StateGraph::new();

        graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

        for i in 1..=3 {
            graph.add_node_from_fn(format!("worker{}", i), move |mut state| {
                Box::pin(async move {
                    state.data.push(format!("Worker {} processed", i));
                    state.counters[i] += 1;
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("collect", |state| Box::pin(async move { Ok(state) }));

        graph.add_parallel_edges(
            "start",
            vec![
                "worker1".to_string(),
                "worker2".to_string(),
                "worker3".to_string(),
            ],
        );

        for i in 1..=3 {
            graph.add_edge(format!("worker{}", i), "collect");
        }
        graph.add_edge("collect", END);
        graph.set_entry_point("start");

        let app = graph.compile_with_merge().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = MediumState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Serialization benchmarks (for checkpoint optimization)
    group.bench_function("small_state_json_serialize", |b| {
        let state = SmallState::new();
        b.iter(|| serde_json::to_string(&state).unwrap());
    });

    group.bench_function("medium_state_json_serialize", |b| {
        let state = MediumState::new();
        b.iter(|| serde_json::to_string(&state).unwrap());
    });

    group.bench_function("large_state_json_serialize", |b| {
        let state = LargeState::new();
        b.iter(|| serde_json::to_string(&state).unwrap());
    });

    // Bincode serialization benchmarks (for checkpoint optimization comparison)
    group.bench_function("small_state_bincode_serialize", |b| {
        let state = SmallState::new();
        b.iter(|| bincode::serialize(&state).unwrap());
    });

    group.bench_function("medium_state_bincode_serialize", |b| {
        let state = MediumState::new();
        b.iter(|| bincode::serialize(&state).unwrap());
    });

    group.bench_function("large_state_bincode_serialize", |b| {
        let state = LargeState::new();
        b.iter(|| bincode::serialize(&state).unwrap());
    });

    group.finish();
}

// ============================================================================
// Stress Test Benchmarks
// ============================================================================

fn bench_stress_tests(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_tests");
    group.sample_size(10); // Reduce sample size for heavy benchmarks

    // Large graph: 100 nodes
    group.bench_function("large_graph_100_nodes", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        // Create 100 nodes
        for i in 0..100 {
            let node_name = format!("node{}", i);
            graph.add_node_from_fn(node_name.clone(), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {}", i));
                    Ok(state)
                })
            });
        }

        // Link nodes sequentially
        for i in 0..99 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node99", END);
        graph.set_entry_point("node0");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Deep nesting: 10 levels of conditional branching
    group.bench_function("deep_nesting_10_levels", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        // Create 10 levels of depth
        for level in 0..10 {
            let node_name = format!("level{}", level);
            graph.add_node_from_fn(node_name.clone(), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Level {}", level));
                    state.next = if level < 9 {
                        format!("level{}", level + 1)
                    } else {
                        "end".to_string()
                    };
                    Ok(state)
                })
            });

            if level == 0 {
                graph.set_entry_point(node_name.clone());
            }

            if level < 9 {
                let mut routes = HashMap::new();
                routes.insert(format!("level{}", level + 1), format!("level{}", level + 1));
                graph.add_conditional_edges(
                    node_name.as_str(),
                    |state: &ComplexState| state.next.clone(),
                    routes,
                );
            } else {
                let mut routes = HashMap::new();
                routes.insert("end".to_string(), END.to_string());
                graph.add_conditional_edges(
                    node_name.as_str(),
                    |state: &ComplexState| state.next.clone(),
                    routes,
                );
            }
        }

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Wide fanout: 50 parallel branches (reduced from 100 for practicality)
    group.bench_function("wide_fanout_50_parallel_branches", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("start", |mut state| {
            Box::pin(async move {
                state.status = "distributing".to_string();
                Ok(state)
            })
        });

        // Create 50 parallel workers
        for i in 0..50 {
            let worker_id = i;
            graph.add_node_from_fn(format!("worker{}", i), move |mut state| {
                Box::pin(async move {
                    state
                        .metadata
                        .insert(format!("worker{}", worker_id), "done".to_string());
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("collect", |mut state| {
            Box::pin(async move {
                state.status = "collected".to_string();
                state.counter = state.metadata.len() as u32;
                Ok(state)
            })
        });

        // Fan-out to all workers
        let workers: Vec<String> = (0..50).map(|i| format!("worker{}", i)).collect();
        graph.add_parallel_edges("start", workers.clone());

        // Fan-in from all workers to collector
        for worker in &workers {
            graph.add_edge(worker.as_str(), "collect");
        }
        graph.add_edge("collect", END);
        graph.set_entry_point("start");

        let app = graph.compile_with_merge().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Long-running workflow: Loop 100 iterations (reduced from 1000 for practicality)
    group.bench_function("long_running_100_iterations", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("processor", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state.messages.push(format!("Iteration {}", state.counter));
                state.next = if state.counter >= 100 {
                    "end".to_string()
                } else {
                    "continue".to_string()
                };
                Ok(state)
            })
        });

        let mut routes = HashMap::new();
        routes.insert("continue".to_string(), "processor".to_string());
        routes.insert("end".to_string(), END.to_string());

        graph.add_conditional_edges(
            "processor",
            |state: &ComplexState| state.next.clone(),
            routes,
        );

        graph.set_entry_point("processor");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    group.finish();
}

// ============================================================================
// Real-World Scenario Benchmarks
// ============================================================================

fn bench_real_world_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_scenarios");

    // Customer Service Router: Intent classification → Specialist routing → Escalation logic
    group.bench_function("customer_service_router", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        // Intent classifier
        graph.add_node_from_fn("classify_intent", |mut state| {
            Box::pin(async move {
                state.counter += 1;
                state
                    .messages
                    .push("Classifying customer intent".to_string());
                // Simulate classification: billing, tech, sales, other
                let intent = match state.counter % 4 {
                    0 => "billing",
                    1 => "tech",
                    2 => "sales",
                    _ => "other",
                };
                state.next = intent.to_string();
                state
                    .metadata
                    .insert("intent".to_string(), intent.to_string());
                Ok(state)
            })
        });

        // Billing specialist
        graph.add_node_from_fn("billing_specialist", |mut state| {
            Box::pin(async move {
                state
                    .messages
                    .push("Billing specialist handling request".to_string());
                state
                    .metadata
                    .insert("specialist".to_string(), "billing".to_string());
                // Check if needs escalation
                state.next = if state.counter % 5 == 0 {
                    "escalate".to_string()
                } else {
                    "resolve".to_string()
                };
                Ok(state)
            })
        });

        // Tech specialist
        graph.add_node_from_fn("tech_specialist", |mut state| {
            Box::pin(async move {
                state
                    .messages
                    .push("Tech specialist handling request".to_string());
                state
                    .metadata
                    .insert("specialist".to_string(), "tech".to_string());
                state.next = if state.counter % 5 == 0 {
                    "escalate".to_string()
                } else {
                    "resolve".to_string()
                };
                Ok(state)
            })
        });

        // Sales specialist
        graph.add_node_from_fn("sales_specialist", |mut state| {
            Box::pin(async move {
                state
                    .messages
                    .push("Sales specialist handling request".to_string());
                state
                    .metadata
                    .insert("specialist".to_string(), "sales".to_string());
                state.next = "resolve".to_string();
                Ok(state)
            })
        });

        // General agent
        graph.add_node_from_fn("general_agent", |mut state| {
            Box::pin(async move {
                state
                    .messages
                    .push("General agent handling request".to_string());
                state
                    .metadata
                    .insert("specialist".to_string(), "general".to_string());
                state.next = "resolve".to_string();
                Ok(state)
            })
        });

        // Escalation handler
        graph.add_node_from_fn("escalate", |mut state| {
            Box::pin(async move {
                state.messages.push("Escalating to supervisor".to_string());
                state
                    .metadata
                    .insert("escalated".to_string(), "true".to_string());
                state.status = "escalated".to_string();
                Ok(state)
            })
        });

        // Resolution handler
        graph.add_node_from_fn("resolve", |mut state| {
            Box::pin(async move {
                state.messages.push("Request resolved".to_string());
                state.status = "resolved".to_string();
                Ok(state)
            })
        });

        // Route from classifier to specialists
        let mut classifier_routes = HashMap::new();
        classifier_routes.insert("billing".to_string(), "billing_specialist".to_string());
        classifier_routes.insert("tech".to_string(), "tech_specialist".to_string());
        classifier_routes.insert("sales".to_string(), "sales_specialist".to_string());
        classifier_routes.insert("other".to_string(), "general_agent".to_string());

        graph.add_conditional_edges(
            "classify_intent",
            |state: &ComplexState| state.next.clone(),
            classifier_routes,
        );

        // Route from specialists to escalation or resolution
        let mut specialist_routes = HashMap::new();
        specialist_routes.insert("escalate".to_string(), "escalate".to_string());
        specialist_routes.insert("resolve".to_string(), "resolve".to_string());

        for specialist in [
            "billing_specialist",
            "tech_specialist",
            "sales_specialist",
            "general_agent",
        ] {
            graph.add_conditional_edges(
                specialist,
                |state: &ComplexState| state.next.clone(),
                specialist_routes.clone(),
            );
        }

        // Connect to END
        graph.add_edge("escalate", END);
        graph.add_edge("resolve", END);

        graph.set_entry_point("classify_intent");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Batch Processing Pipeline: Input → Parallel processing → Error handling → Aggregation
    group.bench_function("batch_processing_pipeline", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        // Batch input handler
        graph.add_node_from_fn("receive_batch", |mut state| {
            Box::pin(async move {
                state.counter = 10; // Simulate 10 items
                state
                    .messages
                    .push("Received batch of 10 items".to_string());
                state.status = "processing".to_string();
                Ok(state)
            })
        });

        // Parallel processors (simulate 5 workers)
        for i in 1..=5 {
            let worker_id = i;
            graph.add_node_from_fn(format!("processor{}", i), move |mut state| {
                Box::pin(async move {
                    // Simulate processing 2 items per worker
                    state
                        .messages
                        .push(format!("Processor {} handling items", worker_id));
                    for j in 0..2 {
                        state.metadata.insert(
                            format!("processor{}_item{}", worker_id, j),
                            "processed".to_string(),
                        );
                    }
                    // Simulate occasional errors
                    if worker_id == 5 {
                        state.next = "error".to_string();
                    } else {
                        state.next = "success".to_string();
                    }
                    Ok(state)
                })
            });
        }

        // Error handler
        graph.add_node_from_fn("handle_error", |mut state| {
            Box::pin(async move {
                state.messages.push("Handling processing error".to_string());
                state
                    .metadata
                    .insert("error_handled".to_string(), "true".to_string());
                Ok(state)
            })
        });

        // Aggregator
        graph.add_node_from_fn("aggregate_results", |mut state| {
            Box::pin(async move {
                state.messages.push("Aggregating results".to_string());
                state.status = "completed".to_string();
                state.counter = state.metadata.len() as u32;
                Ok(state)
            })
        });

        // Checkpointer for resume capability
        let app_no_checkpoint = {
            // Build graph edges
            graph.add_parallel_edges(
                "receive_batch",
                (1..=5).map(|i| format!("processor{}", i)).collect(),
            );

            // Route from processors
            let mut processor_routes = HashMap::new();
            processor_routes.insert("error".to_string(), "handle_error".to_string());
            processor_routes.insert("success".to_string(), "aggregate_results".to_string());

            for i in 1..=5 {
                graph.add_conditional_edges(
                    format!("processor{}", i).as_str(),
                    |state: &ComplexState| state.next.clone(),
                    processor_routes.clone(),
                );
            }

            graph.add_edge("handle_error", "aggregate_results");
            graph.add_edge("aggregate_results", END);
            graph.set_entry_point("receive_batch");

            graph.compile().unwrap()
        };

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app_no_checkpoint.invoke(state).await.unwrap()
            });
    });

    // Financial Analysis: Data gathering → Parallel analysis → Risk assessment → Report generation
    group.bench_function("financial_analysis_workflow", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        // Data gathering
        graph.add_node_from_fn("gather_data", |mut state| {
            Box::pin(async move {
                state.messages.push("Gathering financial data".to_string());
                state
                    .metadata
                    .insert("data_source".to_string(), "market_api".to_string());
                state.counter = 100; // Simulate 100 data points
                Ok(state)
            })
        });

        // Parallel analyzers
        for analyzer in ["fundamental", "technical", "sentiment"] {
            let analyzer_name = analyzer.to_string();
            graph.add_node_from_fn(analyzer.to_string(), move |mut state| {
                let name = analyzer_name.clone();
                Box::pin(async move {
                    state.messages.push(format!("{} analysis complete", name));
                    state.metadata.insert(
                        format!("{}_score", name),
                        format!("{:.2}", 0.75), // Simulated score
                    );
                    Ok(state)
                })
            });
        }

        // Risk assessment
        graph.add_node_from_fn("assess_risk", |mut state| {
            Box::pin(async move {
                state.messages.push("Assessing overall risk".to_string());
                state
                    .metadata
                    .insert("risk_level".to_string(), "medium".to_string());
                // Route based on risk
                state.next = if state.counter % 3 == 0 {
                    "high_risk_report".to_string()
                } else {
                    "standard_report".to_string()
                };
                Ok(state)
            })
        });

        // Report generators
        graph.add_node_from_fn("high_risk_report", |mut state| {
            Box::pin(async move {
                state
                    .messages
                    .push("Generating detailed risk report".to_string());
                state.status = "high_risk_reported".to_string();
                Ok(state)
            })
        });

        graph.add_node_from_fn("standard_report", |mut state| {
            Box::pin(async move {
                state
                    .messages
                    .push("Generating standard report".to_string());
                state.status = "reported".to_string();
                Ok(state)
            })
        });

        // Build graph
        graph.add_parallel_edges(
            "gather_data",
            vec![
                "fundamental".to_string(),
                "technical".to_string(),
                "sentiment".to_string(),
            ],
        );

        for analyzer in ["fundamental", "technical", "sentiment"] {
            graph.add_edge(analyzer, "assess_risk");
        }

        let mut risk_routes = HashMap::new();
        risk_routes.insert(
            "high_risk_report".to_string(),
            "high_risk_report".to_string(),
        );
        risk_routes.insert("standard_report".to_string(), "standard_report".to_string());

        graph.add_conditional_edges(
            "assess_risk",
            |state: &ComplexState| state.next.clone(),
            risk_routes,
        );

        graph.add_edge("high_risk_report", END);
        graph.add_edge("standard_report", END);
        graph.set_entry_point("gather_data");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    group.finish();
}

// ============================================================================
// Benchmark Groups
// ============================================================================

// ============================================================================
// Tracing Overhead Benchmarks
// ============================================================================

fn bench_tracing_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("tracing_overhead");

    // Compare 5-node sequential execution with and without tracing subscriber
    // This tests the overhead of tracing instrumentation when NO subscriber is active
    group.bench_function("5_nodes_no_subscriber", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=5 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {} completed", node_id));
                    for j in 0..10 {
                        state
                            .metadata
                            .insert(format!("node{}_item{}", node_id, j), "value".to_string());
                    }
                    Ok(state)
                })
            });
        }

        for i in 1..=4 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        let app = graph.compile().unwrap();

        // No tracing subscriber initialized - measures overhead of instrumentation code only
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Compare parallel execution (3 workers) with and without tracing subscriber
    group.bench_function("parallel_3_workers_no_subscriber", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        graph.add_node_from_fn("start", |mut state| {
            Box::pin(async move {
                state.status = "started".to_string();
                Ok(state)
            })
        });

        for i in 1..=3 {
            let worker_id = i;
            graph.add_node_from_fn(format!("worker{}", i), move |mut state| {
                Box::pin(async move {
                    state
                        .messages
                        .push(format!("Worker {} processing", worker_id));
                    for j in 0..20 {
                        state.metadata.insert(
                            format!("worker{}_item{}", worker_id, j),
                            format!("result_{}", j),
                        );
                    }
                    Ok(state)
                })
            });
        }

        graph.add_node_from_fn("collect", |mut state| {
            Box::pin(async move {
                state.status = "collected".to_string();
                Ok(state)
            })
        });

        graph.add_parallel_edges(
            "start",
            vec![
                "worker1".to_string(),
                "worker2".to_string(),
                "worker3".to_string(),
            ],
        );

        for i in 1..=3 {
            graph.add_edge(format!("worker{}", i), "collect");
        }
        graph.add_edge("collect", END);
        graph.set_entry_point("start");

        let app = graph.compile().unwrap();

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Checkpoint overhead with tracing
    group.bench_function("checkpoint_3_nodes_no_subscriber", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=3 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    state.messages.push(format!("Node {} completed", node_id));
                    Ok(state)
                })
            });
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let app = graph
            .compile()
            .unwrap()
            .with_checkpointer(MemoryCheckpointer::new())
            .with_thread_id("bench_tracing_overhead");

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    group.finish();
}

// ============================================================================
// Read-Only Node Optimization Benchmarks (M-245)
// ============================================================================

use async_trait::async_trait;
use dashflow::event::CollectingCallback;
use dashflow::node::Node;
use std::any::Any;

/// A read-only node for ComplexState - benchmarks M-245 optimization
struct ReadOnlyComplexNode;

#[async_trait]
impl Node<ComplexState> for ReadOnlyComplexNode {
    async fn execute(&self, state: ComplexState) -> dashflow::Result<ComplexState> {
        Ok(state)
    }

    fn is_read_only(&self) -> bool {
        true // M-245: This enables the optimization
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A read-only node for LargeState - benchmarks M-245 optimization
struct ReadOnlyLargeNode;

#[async_trait]
impl Node<LargeState> for ReadOnlyLargeNode {
    async fn execute(&self, state: LargeState) -> dashflow::Result<LargeState> {
        Ok(state)
    }

    fn is_read_only(&self) -> bool {
        true // M-245: This enables the optimization
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A mutating node that modifies state (for comparison)
struct MutatingCounterNode;

#[async_trait]
impl Node<ComplexState> for MutatingCounterNode {
    async fn execute(&self, mut state: ComplexState) -> dashflow::Result<ComplexState> {
        state.counter += 1;
        state.messages.push("Mutated".to_string());
        Ok(state)
    }

    fn is_read_only(&self) -> bool {
        false // Default behavior - state may be modified
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn bench_read_only_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_only_optimization");

    // Benchmark: 5 read-only nodes with callbacks (M-245 optimization applies)
    group.bench_function("5_read_only_nodes_with_callbacks", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=5 {
            graph.add_node(format!("node{}", i), ReadOnlyComplexNode);
        }

        for i in 1..=4 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        let callback = CollectingCallback::<ComplexState>::new();
        let app = graph.compile().unwrap().with_callback(callback);

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Benchmark: 5 mutating nodes with callbacks (no optimization - compute_state_changes runs)
    group.bench_function("5_mutating_nodes_with_callbacks", |b| {
        let mut graph: StateGraph<ComplexState> = StateGraph::new();

        for i in 1..=5 {
            graph.add_node(format!("node{}", i), MutatingCounterNode);
        }

        for i in 1..=4 {
            graph.add_edge(format!("node{}", i), format!("node{}", i + 1));
        }
        graph.add_edge("node5", END);
        graph.set_entry_point("node1");

        let callback = CollectingCallback::<ComplexState>::new();
        let app = graph.compile().unwrap().with_callback(callback);

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = ComplexState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Benchmark: Large state with read-only nodes (shows optimization benefit for large states)
    group.bench_function("3_read_only_nodes_large_state", |b| {
        let mut graph: StateGraph<LargeState> = StateGraph::new();

        for i in 1..=3 {
            graph.add_node(format!("node{}", i), ReadOnlyLargeNode);
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let callback = CollectingCallback::<LargeState>::new();
        let app = graph.compile().unwrap().with_callback(callback);

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = LargeState::new();
                app.invoke(state).await.unwrap()
            });
    });

    // Benchmark: Large state with mutating nodes (compute_state_changes runs - expensive)
    group.bench_function("3_mutating_nodes_large_state", |b| {
        let mut graph: StateGraph<LargeState> = StateGraph::new();

        for i in 1..=3 {
            let node_id = i;
            graph.add_node_from_fn(format!("node{}", i), move |mut state: LargeState| {
                Box::pin(async move {
                    state.messages.push(format!("Node {} processed", node_id));
                    Ok(state)
                })
            });
        }

        graph.add_edge("node1", "node2");
        graph.add_edge("node2", "node3");
        graph.add_edge("node3", END);
        graph.set_entry_point("node1");

        let callback = CollectingCallback::<LargeState>::new();
        let app = graph.compile().unwrap().with_callback(callback);

        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let state = LargeState::new();
                app.invoke(state).await.unwrap()
            });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_compilation,
    bench_sequential_execution,
    bench_conditional_branching,
    bench_parallel_execution,
    bench_checkpointing,
    bench_event_streaming,
    bench_state_cloning,
    bench_stress_tests,
    bench_real_world_scenarios,
    bench_tracing_overhead,
    bench_read_only_optimization,
);

criterion_main!(benches);
