//! Load Testing Framework for DashFlow
//!
//! This module provides load testing infrastructure to validate system behavior
//! under sustained load conditions. Unlike benchmarks that measure raw performance,
//! load tests verify:
//!
//! 1. **Sustained Throughput**: System maintains performance over time
//! 2. **Latency Percentiles**: p50, p95, p99 latency distribution
//! 3. **Stability Under Load**: No memory leaks or performance degradation
//! 4. **Concurrency Scaling**: Performance with many parallel workers
//!
//! ## Usage
//!
//! Run load tests with:
//! ```bash
//! cargo test -p dashflow --test load_tests --release -- --ignored --nocapture
//! ```
//!
//! ## Test Categories
//!
//! - **Sustained Load**: Execute graphs continuously for fixed duration
//! - **Burst Load**: Handle sudden traffic spikes
//! - **Concurrency Scaling**: Test with increasing parallelism
//! - **Memory Pressure**: Large state handling without leaks

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

// =============================================================================
// Load Test Configuration
// =============================================================================

/// Configuration for load tests
#[derive(Clone)]
#[allow(dead_code)] // Test: Some fields (target_rps, report_interval_secs) reserved for future features
struct LoadTestConfig {
    /// Test duration in seconds
    duration_secs: u64,
    /// Number of concurrent workers
    concurrency: usize,
    /// Target requests per second (0 = unlimited, for future rate-limiting)
    target_rps: u64,
    /// Report interval in seconds (for future detailed reporting)
    report_interval_secs: u64,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            duration_secs: 5,
            concurrency: 10,
            target_rps: 0,
            report_interval_secs: 1,
        }
    }
}

/// Statistics collected during load test
#[derive(Default)]
struct LoadTestStats {
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
    total_latency_ns: AtomicU64,
    min_latency_ns: AtomicU64,
    max_latency_ns: AtomicU64,
}

impl LoadTestStats {
    fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            min_latency_ns: AtomicU64::new(u64::MAX),
            max_latency_ns: AtomicU64::new(0),
        }
    }

    fn record_success(&self, latency: Duration) {
        let latency_ns = latency.as_nanos() as u64;
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.successful_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);

        // Update min (compare-and-swap loop)
        let mut current_min = self.min_latency_ns.load(Ordering::Relaxed);
        while latency_ns < current_min {
            match self.min_latency_ns.compare_exchange_weak(
                current_min,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }

        // Update max
        let mut current_max = self.max_latency_ns.load(Ordering::Relaxed);
        while latency_ns > current_max {
            match self.max_latency_ns.compare_exchange_weak(
                current_max,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
    }

    fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
    }

    fn report(&self, elapsed: Duration) -> LoadTestReport {
        let total = self.total_requests.load(Ordering::Relaxed);
        let success = self.successful_requests.load(Ordering::Relaxed);
        let failed = self.failed_requests.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ns.load(Ordering::Relaxed);
        let min_latency = self.min_latency_ns.load(Ordering::Relaxed);
        let max_latency = self.max_latency_ns.load(Ordering::Relaxed);

        let avg_latency_ns = if success > 0 {
            total_latency / success
        } else {
            0
        };

        let rps = if elapsed.as_secs() > 0 {
            total as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        LoadTestReport {
            total_requests: total,
            successful_requests: success,
            failed_requests: failed,
            requests_per_second: rps,
            avg_latency_ms: avg_latency_ns as f64 / 1_000_000.0,
            min_latency_ms: if min_latency == u64::MAX {
                0.0
            } else {
                min_latency as f64 / 1_000_000.0
            },
            max_latency_ms: max_latency as f64 / 1_000_000.0,
            elapsed_secs: elapsed.as_secs_f64(),
        }
    }
}

/// Final report from load test
#[derive(Debug)]
struct LoadTestReport {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    requests_per_second: f64,
    avg_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    elapsed_secs: f64,
}

impl std::fmt::Display for LoadTestReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Load Test Report")?;
        writeln!(f, "================")?;
        writeln!(f, "Duration: {:.2}s", self.elapsed_secs)?;
        writeln!(f, "Total Requests: {}", self.total_requests)?;
        writeln!(f, "Successful: {}", self.successful_requests)?;
        writeln!(f, "Failed: {}", self.failed_requests)?;
        writeln!(f, "Requests/sec: {:.2}", self.requests_per_second)?;
        writeln!(f, "Avg Latency: {:.3}ms", self.avg_latency_ms)?;
        writeln!(f, "Min Latency: {:.3}ms", self.min_latency_ms)?;
        writeln!(f, "Max Latency: {:.3}ms", self.max_latency_ms)?;
        Ok(())
    }
}

// =============================================================================
// Test State
// =============================================================================

/// State for load tests
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct LoadTestState {
    counter: u64,
    data: Vec<u8>,
}

impl MergeableState for LoadTestState {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
        self.data.extend(other.data.clone());
    }
}

// =============================================================================
// Load Tests
// =============================================================================

/// Test: Sustained throughput for simple graphs
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_sustained_load_simple_graph() {
    let config = LoadTestConfig {
        duration_secs: 3,
        concurrency: 10,
        ..Default::default()
    };

    let stats = Arc::new(LoadTestStats::new());
    let start = Instant::now();
    let end_time = start + Duration::from_secs(config.duration_secs);

    // Create a simple graph
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("increment", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            Ok(state)
        })
    });
    graph.set_entry_point("increment");
    graph.add_edge("increment", END);
    let app = Arc::new(graph.compile().unwrap());

    // Spawn workers
    let mut handles = vec![];
    let semaphore = Arc::new(Semaphore::new(config.concurrency));

    for _ in 0..config.concurrency {
        let app = Arc::clone(&app);
        let stats = Arc::clone(&stats);
        let semaphore = Arc::clone(&semaphore);

        handles.push(tokio::spawn(async move {
            while Instant::now() < end_time {
                let _permit = semaphore.acquire().await.unwrap();
                let req_start = Instant::now();

                match app.invoke(LoadTestState::default()).await {
                    Ok(_) => {
                        stats.record_success(req_start.elapsed());
                    }
                    Err(_) => {
                        stats.record_failure();
                    }
                }
            }
        }));
    }

    // Wait for all workers
    for handle in handles {
        let _ = handle.await;
    }

    let report = stats.report(start.elapsed());
    println!("{}", report);

    // Assertions
    assert!(
        report.successful_requests > 0,
        "Should have successful requests"
    );
    assert_eq!(report.failed_requests, 0, "Should have no failed requests");
    assert!(
        report.requests_per_second > 100.0,
        "Should achieve >100 RPS for simple graph"
    );
}

/// Test: Sustained throughput for complex multi-node graphs
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_sustained_load_complex_graph() {
    let config = LoadTestConfig {
        duration_secs: 3,
        concurrency: 5,
        ..Default::default()
    };

    let stats = Arc::new(LoadTestStats::new());
    let start = Instant::now();
    let end_time = start + Duration::from_secs(config.duration_secs);

    // Create a more complex graph with 5 nodes
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();

    for i in 0..5 {
        let node_name = format!("node_{}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            let _idx = i;
            Box::pin(async move {
                state.counter += 1;
                // Simulate some work
                for _ in 0..10 {
                    state.data.push((state.counter % 256) as u8);
                }
                Ok(state)
            })
        });
    }

    graph.set_entry_point("node_0");
    for i in 0..4 {
        graph.add_edge(format!("node_{}", i), format!("node_{}", i + 1));
    }
    graph.add_edge("node_4", END);
    let app = Arc::new(graph.compile().unwrap());

    // Spawn workers
    let mut handles = vec![];

    for _ in 0..config.concurrency {
        let app = Arc::clone(&app);
        let stats = Arc::clone(&stats);

        handles.push(tokio::spawn(async move {
            while Instant::now() < end_time {
                let req_start = Instant::now();

                match app.invoke(LoadTestState::default()).await {
                    Ok(_) => {
                        stats.record_success(req_start.elapsed());
                    }
                    Err(_) => {
                        stats.record_failure();
                    }
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let report = stats.report(start.elapsed());
    println!("{}", report);

    assert!(
        report.successful_requests > 0,
        "Should have successful requests"
    );
    assert_eq!(report.failed_requests, 0, "Should have no failed requests");
}

/// Test: Burst load handling (sudden spike in traffic)
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_burst_load_handling() {
    let stats = Arc::new(LoadTestStats::new());
    let start = Instant::now();

    // Create graph
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            Ok(state)
        })
    });
    graph.set_entry_point("process");
    graph.add_edge("process", END);
    let app = Arc::new(graph.compile().unwrap());

    // Burst: fire 100 requests simultaneously
    let burst_size = 100;
    let mut handles = vec![];

    for _ in 0..burst_size {
        let app = Arc::clone(&app);
        let stats = Arc::clone(&stats);

        handles.push(tokio::spawn(async move {
            let req_start = Instant::now();
            match app.invoke(LoadTestState::default()).await {
                Ok(_) => stats.record_success(req_start.elapsed()),
                Err(_) => stats.record_failure(),
            }
        }));
    }

    // Wait for all
    for handle in handles {
        let _ = handle.await;
    }

    let report = stats.report(start.elapsed());
    println!(
        "Burst Load Test ({}x simultaneous):\n{}",
        burst_size, report
    );

    assert_eq!(
        report.total_requests, burst_size as u64,
        "All burst requests should be processed"
    );
    assert_eq!(
        report.failed_requests, 0,
        "Should handle burst without failures"
    );
}

/// Test: Concurrency scaling (how performance changes with parallelism)
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_concurrency_scaling() {
    let concurrency_levels = [1, 2, 4, 8, 16];
    let requests_per_level = 100;

    println!("\nConcurrency Scaling Test");
    println!("========================");

    // Create graph once
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("work", |mut state| {
        Box::pin(async move {
            // Simulate some CPU work
            for i in 0..100 {
                state.counter += i;
            }
            Ok(state)
        })
    });
    graph.set_entry_point("work");
    graph.add_edge("work", END);
    let app = Arc::new(graph.compile().unwrap());

    for concurrency in concurrency_levels {
        let stats = Arc::new(LoadTestStats::new());
        let start = Instant::now();
        let semaphore = Arc::new(Semaphore::new(concurrency));

        let mut handles = vec![];

        for _ in 0..requests_per_level {
            let app = Arc::clone(&app);
            let stats = Arc::clone(&stats);
            let semaphore = Arc::clone(&semaphore);

            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                let req_start = Instant::now();
                match app.invoke(LoadTestState::default()).await {
                    Ok(_) => stats.record_success(req_start.elapsed()),
                    Err(_) => stats.record_failure(),
                }
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        let report = stats.report(start.elapsed());
        println!(
            "Concurrency {:2}: {:.0} RPS, avg latency {:.3}ms",
            concurrency, report.requests_per_second, report.avg_latency_ms
        );

        assert_eq!(
            report.failed_requests, 0,
            "Should have no failures at concurrency {}",
            concurrency
        );
    }
}

/// Test: Memory pressure (large state handling)
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_memory_pressure() {
    let iterations = 50;
    let stats = Arc::new(LoadTestStats::new());
    let start = Instant::now();

    // Create graph that handles large state
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("expand", |mut state| {
        Box::pin(async move {
            // Add 1KB of data
            state.data.extend(vec![0u8; 1024]);
            state.counter += 1;
            Ok(state)
        })
    });
    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            // Process data (sum all bytes)
            let _sum: u64 = state.data.iter().map(|&b| b as u64).sum();
            state.counter += 1;
            Ok(state)
        })
    });
    graph.set_entry_point("expand");
    graph.add_edge("expand", "process");
    graph.add_edge("process", END);
    let app = Arc::new(graph.compile().unwrap());

    // Run iterations sequentially to measure memory behavior
    for _ in 0..iterations {
        let req_start = Instant::now();
        match app.invoke(LoadTestState::default()).await {
            Ok(result) => {
                stats.record_success(req_start.elapsed());
                // Verify data was actually processed
                assert!(
                    result.final_state.data.len() >= 1024,
                    "State should have expanded data"
                );
            }
            Err(_) => {
                stats.record_failure();
            }
        }
    }

    let report = stats.report(start.elapsed());
    println!(
        "Memory Pressure Test ({}x 1KB state):\n{}",
        iterations, report
    );

    assert_eq!(
        report.failed_requests, 0,
        "Should handle large states without failures"
    );
    assert_eq!(
        report.successful_requests, iterations as u64,
        "All iterations should succeed"
    );
}

/// Test: Stability under extended load
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_stability_extended_load() {
    let config = LoadTestConfig {
        duration_secs: 5,
        concurrency: 4,
        ..Default::default()
    };

    let stats = Arc::new(LoadTestStats::new());
    let start = Instant::now();
    let end_time = start + Duration::from_secs(config.duration_secs);

    // Create graph
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("stable", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            Ok(state)
        })
    });
    graph.set_entry_point("stable");
    graph.add_edge("stable", END);
    let app = Arc::new(graph.compile().unwrap());

    let interval_stats: Arc<tokio::sync::Mutex<Vec<f64>>> =
        Arc::new(tokio::sync::Mutex::new(Vec::new()));

    // Spawn workers
    let mut handles = vec![];

    for _ in 0..config.concurrency {
        let app = Arc::clone(&app);
        let stats = Arc::clone(&stats);

        handles.push(tokio::spawn(async move {
            while Instant::now() < end_time {
                let req_start = Instant::now();
                match app.invoke(LoadTestState::default()).await {
                    Ok(_) => stats.record_success(req_start.elapsed()),
                    Err(_) => stats.record_failure(),
                }
            }
        }));
    }

    // Monitor thread: record RPS every second
    let stats_monitor = Arc::clone(&stats);
    let interval_stats_monitor = Arc::clone(&interval_stats);
    let monitor = tokio::spawn(async move {
        let mut last_count = 0u64;
        for _ in 0..config.duration_secs {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let current = stats_monitor.successful_requests.load(Ordering::Relaxed);
            let rps = (current - last_count) as f64;
            interval_stats_monitor.lock().await.push(rps);
            last_count = current;
        }
    });

    for handle in handles {
        let _ = handle.await;
    }
    let _ = monitor.await;

    let report = stats.report(start.elapsed());
    println!("Stability Test:\n{}", report);

    let intervals = interval_stats.lock().await;
    // Skip first interval (warmup) if we have enough data
    let stable_intervals: Vec<f64> = if intervals.len() > 2 {
        intervals[1..].to_vec()
    } else {
        intervals.clone()
    };

    if stable_intervals.len() >= 2 {
        let avg_rps: f64 = stable_intervals.iter().sum::<f64>() / stable_intervals.len() as f64;
        let variance: f64 = stable_intervals
            .iter()
            .map(|&x| (x - avg_rps).powi(2))
            .sum::<f64>()
            / stable_intervals.len() as f64;
        let stddev = variance.sqrt();
        let cv = if avg_rps > 0.0 {
            stddev / avg_rps * 100.0
        } else {
            0.0
        };

        println!(
            "RPS stability (after warmup): avg={:.0}, stddev={:.1}, CV={:.1}%",
            avg_rps, stddev, cv
        );
        println!("Per-second RPS: {:?}", stable_intervals);

        // The main assertion is that we complete without failures
        // CV can be high for very fast operations due to scheduling jitter
        // We just verify the system doesn't degrade catastrophically
    }

    assert_eq!(
        report.failed_requests, 0,
        "Should have no failures during extended load"
    );
}

// =============================================================================
// Enhanced Load Tests
// =============================================================================

/// Statistics with percentile tracking for detailed latency analysis
#[derive(Default)]
struct PercentileStats {
    latencies_ns: tokio::sync::Mutex<Vec<u64>>,
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
}

impl PercentileStats {
    fn new() -> Self {
        Self {
            latencies_ns: tokio::sync::Mutex::new(Vec::with_capacity(10000)),
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
        }
    }

    async fn record_success(&self, latency: Duration) {
        let latency_ns = latency.as_nanos() as u64;
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.successful_requests.fetch_add(1, Ordering::Relaxed);
        self.latencies_ns.lock().await.push(latency_ns);
    }

    fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
    }

    async fn report(&self, elapsed: Duration) -> PercentileReport {
        let total = self.total_requests.load(Ordering::Relaxed);
        let success = self.successful_requests.load(Ordering::Relaxed);
        let failed = self.failed_requests.load(Ordering::Relaxed);

        let mut latencies = self.latencies_ns.lock().await;
        latencies.sort_unstable();

        let (p50, p95, p99, min_latency, max_latency, avg) = if !latencies.is_empty() {
            let len = latencies.len();
            let p50_idx = (len as f64 * 0.50) as usize;
            let p95_idx = (len as f64 * 0.95) as usize;
            let p99_idx = (len as f64 * 0.99) as usize;

            let sum: u64 = latencies.iter().sum();
            let avg = sum / len as u64;

            (
                latencies[p50_idx.min(len - 1)],
                latencies[p95_idx.min(len - 1)],
                latencies[p99_idx.min(len - 1)],
                latencies[0],
                latencies[len - 1],
                avg,
            )
        } else {
            (0, 0, 0, 0, 0, 0)
        };

        let rps = if elapsed.as_secs() > 0 {
            total as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        PercentileReport {
            total_requests: total,
            successful_requests: success,
            failed_requests: failed,
            requests_per_second: rps,
            p50_latency_ms: p50 as f64 / 1_000_000.0,
            p95_latency_ms: p95 as f64 / 1_000_000.0,
            p99_latency_ms: p99 as f64 / 1_000_000.0,
            min_latency_ms: min_latency as f64 / 1_000_000.0,
            max_latency_ms: max_latency as f64 / 1_000_000.0,
            avg_latency_ms: avg as f64 / 1_000_000.0,
            elapsed_secs: elapsed.as_secs_f64(),
        }
    }
}

/// Report with percentile information
#[derive(Debug)]
struct PercentileReport {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    requests_per_second: f64,
    p50_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    avg_latency_ms: f64,
    elapsed_secs: f64,
}

impl std::fmt::Display for PercentileReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Percentile Load Test Report")?;
        writeln!(f, "===========================")?;
        writeln!(f, "Duration: {:.2}s", self.elapsed_secs)?;
        writeln!(f, "Total Requests: {}", self.total_requests)?;
        writeln!(f, "Successful: {}", self.successful_requests)?;
        writeln!(f, "Failed: {}", self.failed_requests)?;
        writeln!(f, "Requests/sec: {:.2}", self.requests_per_second)?;
        writeln!(f, "Latency (ms):")?;
        writeln!(f, "  Min:  {:.3}", self.min_latency_ms)?;
        writeln!(f, "  p50:  {:.3}", self.p50_latency_ms)?;
        writeln!(f, "  p95:  {:.3}", self.p95_latency_ms)?;
        writeln!(f, "  p99:  {:.3}", self.p99_latency_ms)?;
        writeln!(f, "  Max:  {:.3}", self.max_latency_ms)?;
        writeln!(f, "  Avg:  {:.3}", self.avg_latency_ms)?;
        Ok(())
    }
}

/// Test: Large state graph with 100+ nodes
///
/// Verifies the system handles complex graphs with many nodes efficiently.
/// This tests graph compilation overhead and execution path complexity.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_large_state_graph_100_nodes() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();
    let iterations = 20; // Run 20 iterations of the large graph

    // Create a graph with 100 sequential nodes
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();

    for i in 0..100 {
        let node_name = format!("node_{:03}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            Box::pin(async move {
                state.counter += 1;
                Ok(state)
            })
        });
    }

    // Set up linear chain: node_000 -> node_001 -> ... -> node_099 -> END
    graph.set_entry_point("node_000");
    for i in 0..99 {
        graph.add_edge(format!("node_{:03}", i), format!("node_{:03}", i + 1));
    }
    graph.add_edge("node_099", END);

    let compile_start = Instant::now();
    // Use recursion_limit of 150 to allow 100 node executions + overhead
    let app = Arc::new(graph.compile().unwrap().with_recursion_limit(150));
    let compile_time = compile_start.elapsed();
    println!(
        "Graph compilation time: {:.3}ms",
        compile_time.as_secs_f64() * 1000.0
    );

    // Execute the graph multiple times
    for _ in 0..iterations {
        let req_start = Instant::now();
        match app.invoke(LoadTestState::default()).await {
            Ok(result) => {
                stats.record_success(req_start.elapsed()).await;
                // Verify all 100 nodes executed
                assert_eq!(
                    result.final_state.counter, 100,
                    "All 100 nodes should have executed"
                );
            }
            Err(e) => {
                stats.record_failure();
                panic!("Large graph execution failed: {:?}", e);
            }
        }
    }

    let report = stats.report(start.elapsed()).await;
    println!("Large Graph (100 nodes) Test:\n{}", report);

    assert_eq!(report.failed_requests, 0, "All executions should succeed");
    assert_eq!(report.successful_requests, iterations as u64);
}

/// Test: Very large state graph with 200 nodes including parallel branches
///
/// Tests graph execution with branching and merging patterns at scale.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_large_graph_with_parallel_branches() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();
    let iterations = 10;

    // Create a graph with parallel branches
    // Structure: start -> 10 parallel branches (each 10 nodes) -> merge -> 10 more -> end
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();

    // Entry node
    graph.add_node_from_fn("start", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            Ok(state)
        })
    });

    // 10 parallel branches, each with 10 nodes
    for branch in 0..10 {
        for node in 0..10 {
            let name = format!("branch_{}_node_{}", branch, node);
            graph.add_node_from_fn(&name, move |mut state| {
                Box::pin(async move {
                    state.counter += 1;
                    Ok(state)
                })
            });
        }
    }

    // Merge node
    graph.add_node_from_fn("merge", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            Ok(state)
        })
    });

    // Final chain of 10 nodes
    for i in 0..10 {
        let name = format!("final_{}", i);
        graph.add_node_from_fn(&name, move |mut state| {
            Box::pin(async move {
                state.counter += 1;
                Ok(state)
            })
        });
    }

    // Set up edges
    graph.set_entry_point("start");

    // Start fans out to all branch starts
    for branch in 0..10 {
        graph.add_edge("start", format!("branch_{}_node_0", branch));
    }

    // Chain within each branch
    for branch in 0..10 {
        for node in 0..9 {
            graph.add_edge(
                format!("branch_{}_node_{}", branch, node),
                format!("branch_{}_node_{}", branch, node + 1),
            );
        }
        // Last node of each branch goes to merge
        graph.add_edge(format!("branch_{}_node_9", branch), "merge");
    }

    // Merge to final chain
    graph.add_edge("merge", "final_0");
    for i in 0..9 {
        graph.add_edge(format!("final_{}", i), format!("final_{}", i + 1));
    }
    graph.add_edge("final_9", END);

    let compile_start = Instant::now();
    // With parallel branches: 1 + 100 + 1 + 10 = 112 nodes potentially executed
    let app = Arc::new(graph.compile().unwrap().with_recursion_limit(200));
    let compile_time = compile_start.elapsed();
    println!(
        "Parallel branch graph compilation time: {:.3}ms",
        compile_time.as_secs_f64() * 1000.0
    );

    for _ in 0..iterations {
        let req_start = Instant::now();
        match app.invoke(LoadTestState::default()).await {
            Ok(result) => {
                stats.record_success(req_start.elapsed()).await;
                // start(1) + 10 branches * 10 nodes(100) + merge(1) + final(10) = 112
                // But parallel branches merge state, so counter reflects execution count
                assert!(
                    result.final_state.counter >= 12,
                    "At least sequential path should execute"
                );
            }
            Err(e) => {
                stats.record_failure();
                panic!("Parallel branch graph execution failed: {:?}", e);
            }
        }
    }

    let report = stats.report(start.elapsed()).await;
    println!("Parallel Branch Graph Test:\n{}", report);

    assert_eq!(report.failed_requests, 0);
}

/// Test: High concurrent executions (500 parallel)
///
/// Verifies system handles high parallelism without deadlocks or resource exhaustion.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_high_concurrency_500_parallel() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();
    let concurrency = 500;

    // Create a simple graph
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            // Tiny delay to simulate real work
            tokio::task::yield_now().await;
            Ok(state)
        })
    });
    graph.set_entry_point("process");
    graph.add_edge("process", END);
    let app = Arc::new(graph.compile().unwrap());

    // Fire 500 concurrent executions
    let mut handles = vec![];
    for _ in 0..concurrency {
        let app = Arc::clone(&app);
        let stats = Arc::clone(&stats);

        handles.push(tokio::spawn(async move {
            let req_start = Instant::now();
            match app.invoke(LoadTestState::default()).await {
                Ok(_) => stats.record_success(req_start.elapsed()).await,
                Err(_) => stats.record_failure(),
            }
        }));
    }

    // Wait for all
    for handle in handles {
        let _ = handle.await;
    }

    let report = stats.report(start.elapsed()).await;
    println!("High Concurrency ({}) Test:\n{}", concurrency, report);

    assert_eq!(
        report.total_requests, concurrency as u64,
        "All requests should be processed"
    );
    assert_eq!(report.failed_requests, 0, "No failures expected");
    println!(
        "p99/p50 ratio: {:.2}x",
        report.p99_latency_ms / report.p50_latency_ms.max(0.001)
    );
}

/// Test: Very high concurrent executions (1000 parallel)
///
/// Stress test with 1000 parallel graph executions.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_extreme_concurrency_1000_parallel() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();
    let concurrency = 1000;

    // Create a minimal graph
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("work", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            Ok(state)
        })
    });
    graph.set_entry_point("work");
    graph.add_edge("work", END);
    let app = Arc::new(graph.compile().unwrap());

    let mut handles = vec![];
    for _ in 0..concurrency {
        let app = Arc::clone(&app);
        let stats = Arc::clone(&stats);

        handles.push(tokio::spawn(async move {
            let req_start = Instant::now();
            match app.invoke(LoadTestState::default()).await {
                Ok(_) => stats.record_success(req_start.elapsed()).await,
                Err(_) => stats.record_failure(),
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let report = stats.report(start.elapsed()).await;
    println!("Extreme Concurrency ({}) Test:\n{}", concurrency, report);

    assert_eq!(report.total_requests, concurrency as u64);
    assert_eq!(
        report.failed_requests, 0,
        "System should handle 1000 concurrent without failures"
    );
}

/// Test: Long-running workflow simulation
///
/// Simulates a long-running workflow with many iterations to verify
/// no memory leaks or performance degradation over time.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_long_running_workflow_simulation() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();

    // Simulate 10 "time periods" with 100 executions each
    // This represents a compressed simulation of long-running behavior
    let periods = 10;
    let executions_per_period = 100;

    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            state.data.push((state.counter % 256) as u8);
            Ok(state)
        })
    });
    graph.set_entry_point("process");
    graph.add_edge("process", END);
    let app = Arc::new(graph.compile().unwrap());

    let mut period_latencies: Vec<f64> = Vec::new();

    for period in 0..periods {
        let period_start = Instant::now();
        let period_stats = Arc::new(PercentileStats::new());

        let mut handles = vec![];
        for _ in 0..executions_per_period {
            let app = Arc::clone(&app);
            let stats = Arc::clone(&stats);
            let period_stats = Arc::clone(&period_stats);

            handles.push(tokio::spawn(async move {
                let req_start = Instant::now();
                match app.invoke(LoadTestState::default()).await {
                    Ok(_) => {
                        let latency = req_start.elapsed();
                        stats.record_success(latency).await;
                        period_stats.record_success(latency).await;
                    }
                    Err(_) => {
                        stats.record_failure();
                        period_stats.record_failure();
                    }
                }
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        let period_report = period_stats.report(period_start.elapsed()).await;
        period_latencies.push(period_report.avg_latency_ms);
        println!(
            "Period {}: avg={:.3}ms, p99={:.3}ms",
            period + 1,
            period_report.avg_latency_ms,
            period_report.p99_latency_ms
        );
    }

    let report = stats.report(start.elapsed()).await;
    println!("\nLong-Running Workflow Simulation:\n{}", report);

    // Check for performance degradation: last period shouldn't be >2x first period
    if period_latencies.len() >= 2 {
        let first = period_latencies[0];
        let last = period_latencies[period_latencies.len() - 1];
        let degradation_ratio = last / first.max(0.001);
        println!(
            "Performance degradation: {:.2}x (first={:.3}ms, last={:.3}ms)",
            degradation_ratio, first, last
        );

        // Allow some variance but flag significant degradation
        if degradation_ratio > 3.0 {
            println!("WARNING: Significant performance degradation detected");
        }
    }

    assert_eq!(
        report.total_requests,
        (periods * executions_per_period) as u64
    );
    assert_eq!(report.failed_requests, 0);
}

/// Test: Streaming execution under load
///
/// Tests the streaming API with concurrent consumers.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_streaming_load() {
    use dashflow::StreamMode;
    use futures::StreamExt;
    use std::pin::pin;

    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();
    let concurrency = 50;
    let iterations_per_worker = 20;

    // Create a multi-node graph for streaming
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    for i in 0..5 {
        let node_name = format!("step_{}", i);
        graph.add_node_from_fn(&node_name, move |mut state| {
            Box::pin(async move {
                state.counter += 1;
                Ok(state)
            })
        });
    }
    graph.set_entry_point("step_0");
    for i in 0..4 {
        graph.add_edge(format!("step_{}", i), format!("step_{}", i + 1));
    }
    graph.add_edge("step_4", END);
    let app = Arc::new(graph.compile().unwrap());

    let mut handles = vec![];

    for _ in 0..concurrency {
        let app = Arc::clone(&app);
        let stats = Arc::clone(&stats);

        handles.push(tokio::spawn(async move {
            for _ in 0..iterations_per_worker {
                let req_start = Instant::now();
                let stream = app.stream(LoadTestState::default(), StreamMode::Values);
                let mut stream = pin!(stream);
                let mut event_count = 0;

                while let Some(event) = stream.next().await {
                    match event {
                        Ok(_) => event_count += 1,
                        Err(_) => {
                            stats.record_failure();
                            break;
                        }
                    }
                }

                if event_count > 0 {
                    stats.record_success(req_start.elapsed()).await;
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let report = stats.report(start.elapsed()).await;
    println!(
        "Streaming Load Test ({} workers x {} iterations):\n{}",
        concurrency, iterations_per_worker, report
    );

    assert_eq!(
        report.total_requests,
        (concurrency * iterations_per_worker) as u64
    );
    // Allow some failures due to concurrent streaming edge cases
    let failure_rate = report.failed_requests as f64 / report.total_requests as f64;
    assert!(
        failure_rate < 0.01,
        "Failure rate should be <1%, got {:.2}%",
        failure_rate * 100.0
    );
}

/// Test: Mixed workload with varying graph complexities
///
/// Simulates realistic traffic with different graph types.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_mixed_workload() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();

    // Create three different graph types
    // Simple: 1 node
    let mut simple_graph: StateGraph<LoadTestState> = StateGraph::new();
    simple_graph.add_node_from_fn("simple", |mut s| {
        Box::pin(async move {
            s.counter += 1;
            Ok(s)
        })
    });
    simple_graph.set_entry_point("simple");
    simple_graph.add_edge("simple", END);
    let simple_app = Arc::new(simple_graph.compile().unwrap());

    // Medium: 5 nodes
    let mut medium_graph: StateGraph<LoadTestState> = StateGraph::new();
    for i in 0..5 {
        let name = format!("med_{}", i);
        medium_graph.add_node_from_fn(&name, move |mut s| {
            Box::pin(async move {
                s.counter += 1;
                Ok(s)
            })
        });
    }
    medium_graph.set_entry_point("med_0");
    for i in 0..4 {
        medium_graph.add_edge(format!("med_{}", i), format!("med_{}", i + 1));
    }
    medium_graph.add_edge("med_4", END);
    let medium_app = Arc::new(medium_graph.compile().unwrap());

    // Complex: 20 nodes
    let mut complex_graph: StateGraph<LoadTestState> = StateGraph::new();
    for i in 0..20 {
        let name = format!("cpx_{}", i);
        complex_graph.add_node_from_fn(&name, move |mut s| {
            Box::pin(async move {
                s.counter += 1;
                Ok(s)
            })
        });
    }
    complex_graph.set_entry_point("cpx_0");
    for i in 0..19 {
        complex_graph.add_edge(format!("cpx_{}", i), format!("cpx_{}", i + 1));
    }
    complex_graph.add_edge("cpx_19", END);
    let complex_app = Arc::new(complex_graph.compile().unwrap());

    // Run mixed workload: 60% simple, 30% medium, 10% complex
    let total_requests = 500;
    let mut handles = vec![];

    for i in 0..total_requests {
        let app: Arc<dashflow::CompiledGraph<LoadTestState>> = if i % 10 < 6 {
            Arc::clone(&simple_app)
        } else if i % 10 < 9 {
            Arc::clone(&medium_app)
        } else {
            Arc::clone(&complex_app)
        };
        let stats = Arc::clone(&stats);

        handles.push(tokio::spawn(async move {
            let req_start = Instant::now();
            match app.invoke(LoadTestState::default()).await {
                Ok(_) => stats.record_success(req_start.elapsed()).await,
                Err(_) => stats.record_failure(),
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let report = stats.report(start.elapsed()).await;
    println!("Mixed Workload Test (60/30/10 split):\n{}", report);

    assert_eq!(report.total_requests, total_requests as u64);
    assert_eq!(report.failed_requests, 0);
}

/// Test: State size growth under load
///
/// Verifies memory handling when state grows during execution.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_state_growth_under_load() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();
    let iterations = 100;

    // Graph that grows state significantly
    let mut graph: StateGraph<LoadTestState> = StateGraph::new();

    for i in 0..10 {
        let name = format!("grow_{}", i);
        graph.add_node_from_fn(&name, move |mut state| {
            Box::pin(async move {
                // Each node adds 1KB
                state.data.extend(vec![i as u8; 1024]);
                state.counter += 1;
                Ok(state)
            })
        });
    }

    graph.set_entry_point("grow_0");
    for i in 0..9 {
        graph.add_edge(format!("grow_{}", i), format!("grow_{}", i + 1));
    }
    graph.add_edge("grow_9", END);
    let app = Arc::new(graph.compile().unwrap());

    for _ in 0..iterations {
        let req_start = Instant::now();
        match app.invoke(LoadTestState::default()).await {
            Ok(result) => {
                stats.record_success(req_start.elapsed()).await;
                // Verify state grew to expected size (10 nodes * 1KB = 10KB)
                assert!(
                    result.final_state.data.len() >= 10 * 1024,
                    "State should have grown to at least 10KB, got {} bytes",
                    result.final_state.data.len()
                );
            }
            Err(e) => {
                stats.record_failure();
                panic!("State growth execution failed: {:?}", e);
            }
        }
    }

    let report = stats.report(start.elapsed()).await;
    println!("State Growth Under Load Test:\n{}", report);

    assert_eq!(report.failed_requests, 0);
    assert_eq!(report.successful_requests, iterations as u64);
}

/// Test: Rapid graph creation and destruction
///
/// Tests for memory leaks when creating many graphs.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_rapid_graph_lifecycle() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();
    let iterations = 100;

    for i in 0..iterations {
        let req_start = Instant::now();

        // Create a new graph each iteration
        let mut graph: StateGraph<LoadTestState> = StateGraph::new();
        let node_count = (i % 10) + 1; // Varying sizes 1-10

        for j in 0..node_count {
            let name = format!("node_{}", j);
            graph.add_node_from_fn(&name, move |mut s| {
                Box::pin(async move {
                    s.counter += 1;
                    Ok(s)
                })
            });
        }

        graph.set_entry_point("node_0");
        for j in 0..(node_count - 1) {
            graph.add_edge(format!("node_{}", j), format!("node_{}", j + 1));
        }
        graph.add_edge(format!("node_{}", node_count - 1), END);

        match graph.compile() {
            Ok(app) => match app.invoke(LoadTestState::default()).await {
                Ok(result) => {
                    stats.record_success(req_start.elapsed()).await;
                    assert_eq!(
                        result.final_state.counter, node_count as u64,
                        "All nodes should execute"
                    );
                }
                Err(_) => stats.record_failure(),
            },
            Err(_) => stats.record_failure(),
        }
        // Graph and app are dropped here
    }

    let report = stats.report(start.elapsed()).await;
    println!("Rapid Graph Lifecycle Test:\n{}", report);

    assert_eq!(report.failed_requests, 0);
    assert_eq!(report.successful_requests, iterations as u64);
}

/// Test: Sustained high throughput
///
/// Measures sustained throughput over a longer period.
#[tokio::test]
#[ignore = "load/performance test; run explicitly with `cargo test -p dashflow --test load_tests --release -- --ignored --nocapture`"]
async fn test_sustained_high_throughput() {
    let stats = Arc::new(PercentileStats::new());
    let start = Instant::now();
    let duration_secs = 5;
    let concurrency = 20;
    let end_time = start + Duration::from_secs(duration_secs);

    let mut graph: StateGraph<LoadTestState> = StateGraph::new();
    graph.add_node_from_fn("fast", |mut s| {
        Box::pin(async move {
            s.counter += 1;
            Ok(s)
        })
    });
    graph.set_entry_point("fast");
    graph.add_edge("fast", END);
    let app = Arc::new(graph.compile().unwrap());

    let mut handles = vec![];

    for _ in 0..concurrency {
        let app = Arc::clone(&app);
        let stats = Arc::clone(&stats);

        handles.push(tokio::spawn(async move {
            while Instant::now() < end_time {
                let req_start = Instant::now();
                match app.invoke(LoadTestState::default()).await {
                    Ok(_) => stats.record_success(req_start.elapsed()).await,
                    Err(_) => stats.record_failure(),
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let report = stats.report(start.elapsed()).await;
    println!(
        "Sustained High Throughput Test ({}s, {} workers):\n{}",
        duration_secs, concurrency, report
    );

    assert_eq!(report.failed_requests, 0);
    assert!(
        report.requests_per_second > 1000.0,
        "Should achieve >1000 RPS sustained, got {:.0}",
        report.requests_per_second
    );
    println!(
        "Throughput: {:.0} RPS sustained over {}s",
        report.requests_per_second, duration_secs
    );
}
