//! Benchmark for agent loop performance
//!
//! Run with: cargo bench -p codex-dashflow-core

use std::time::{Duration, Instant};

use codex_dashflow_core::{
    runner::{run_agent, RunnerConfig},
    state::{AgentState, Message},
};

/// Number of iterations for each benchmark
const ITERATIONS: usize = 10;

/// Benchmark result for a single test
struct BenchResult {
    name: &'static str,
    iterations: usize,
    total_time: Duration,
    avg_time: Duration,
    min_time: Duration,
    max_time: Duration,
}

impl BenchResult {
    fn print(&self) {
        println!("  {}", self.name);
        println!("    iterations: {}", self.iterations);
        println!("    total:      {:?}", self.total_time);
        println!("    avg:        {:?}", self.avg_time);
        println!("    min:        {:?}", self.min_time);
        println!("    max:        {:?}", self.max_time);
        println!();
    }
}

fn run_bench<F, Fut>(name: &'static str, iterations: usize, mut f: F) -> BenchResult
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();
        rt.block_on(f());
        times.push(start.elapsed());
    }

    let total_time: Duration = times.iter().sum();
    let avg_time = total_time / iterations as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();

    BenchResult {
        name,
        iterations,
        total_time,
        avg_time,
        min_time,
        max_time,
    }
}

/// Benchmark: Create AgentState
async fn bench_state_creation() {
    let _state = AgentState::new();
}

/// Benchmark: Create AgentState with message
async fn bench_state_with_message() {
    let mut state = AgentState::new();
    state.messages.push(Message::user("Hello, world!"));
}

/// Benchmark: Build and invoke agent graph with mock (no LLM call)
async fn bench_agent_loop_mock() {
    let mut state = AgentState::new();
    state.messages.push(Message::user("test"));

    // Use a config with max_turns to limit execution
    let config = RunnerConfig::default().with_max_turns(1);

    // This will use mock LLM responses in the reasoning node
    let _ = run_agent(state, &config).await;
}

/// Benchmark: Build agent graph only
async fn bench_graph_build() {
    let _ = codex_dashflow_core::graph::build_agent_graph();
}

fn main() {
    println!("\n=== Agent Loop Benchmarks ===\n");
    println!("N=50 Benchmark Iteration\n");

    // Warmup
    println!("Warming up...\n");
    let rt = tokio::runtime::Runtime::new().unwrap();
    for _ in 0..3 {
        rt.block_on(bench_agent_loop_mock());
    }

    // Run benchmarks
    let results = vec![
        run_bench("state_creation", ITERATIONS * 10, bench_state_creation),
        run_bench(
            "state_with_message",
            ITERATIONS * 10,
            bench_state_with_message,
        ),
        run_bench("graph_build", ITERATIONS, bench_graph_build),
        run_bench("agent_loop_mock", ITERATIONS, bench_agent_loop_mock),
    ];

    println!("Results:\n");
    for result in &results {
        result.print();
    }

    // Summary
    println!("Summary:");
    println!("  Total benchmarks: {}", results.len());
    println!(
        "  Total iterations: {}",
        results.iter().map(|r| r.iterations).sum::<usize>()
    );
    let total_time: Duration = results.iter().map(|r| r.total_time).sum();
    println!("  Total time: {:?}", total_time);
}
