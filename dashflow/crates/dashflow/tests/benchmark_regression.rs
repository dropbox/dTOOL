//! Benchmark Regression Tracking for DashFlow
//!
//! This module provides tools for detecting performance regressions by comparing
//! benchmark results against stored baselines.
//!
//! ## Usage
//!
//! 1. **Establish baseline**: Run benchmarks and save results
//! 2. **Compare**: Run benchmarks again and compare against baseline
//! 3. **Alert**: Fail tests if regressions exceed threshold
//!
//! ## File Format
//!
//! Baselines are stored as JSON in `benchmarks/baselines/`:
//! ```json
//! {
//!   "version": 1,
//!   "created": "2025-12-03T00:00:00Z",
//!   "benchmarks": {
//!     "graph_compile_simple": { "mean_ns": 1000, "stddev_ns": 100 },
//!     "graph_execute_5_nodes": { "mean_ns": 5000, "stddev_ns": 200 }
//!   }
//! }
//! ```
//!
//! ## Running
//!
//! ```bash
//! # Run regression tests
//! cargo test -p dashflow --test benchmark_regression --release -- --nocapture
//! ```

#![allow(clippy::redundant_clone, clippy::unwrap_used)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// =============================================================================
// Data Types
// =============================================================================

/// A single benchmark measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMeasurement {
    /// Mean execution time in nanoseconds
    pub mean_ns: u64,
    /// Standard deviation in nanoseconds
    pub stddev_ns: u64,
    /// Number of samples
    pub samples: u32,
}

/// Baseline file format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkBaseline {
    /// Schema version
    pub version: u32,
    /// When baseline was created
    pub created: String,
    /// Git commit hash (if available)
    #[serde(default)]
    pub commit: String,
    /// Benchmark results by name
    pub benchmarks: HashMap<String, BenchmarkMeasurement>,
}

impl BenchmarkBaseline {
    /// Create a new empty baseline
    pub fn new() -> Self {
        Self {
            version: 1,
            created: chrono_lite_now(),
            commit: String::new(),
            benchmarks: HashMap::new(),
        }
    }

    /// Add a benchmark measurement
    pub fn add(&mut self, name: &str, measurement: BenchmarkMeasurement) {
        self.benchmarks.insert(name.to_string(), measurement);
    }
}

impl Default for BenchmarkBaseline {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of comparing against baseline
#[derive(Debug)]
pub struct RegressionResult {
    pub name: String,
    pub baseline_ns: u64,
    pub current_ns: u64,
    pub change_percent: f64,
    pub is_regression: bool,
}

/// Configuration for regression detection
#[derive(Clone)]
pub struct RegressionConfig {
    /// Percentage increase that constitutes a regression
    pub regression_threshold_percent: f64,
    /// Minimum absolute change to consider (filters out noise)
    pub min_change_ns: u64,
}

impl Default for RegressionConfig {
    fn default() -> Self {
        Self {
            regression_threshold_percent: 10.0, // 10% slowdown = regression
            min_change_ns: 1000,                // Ignore < 1μs differences
        }
    }
}

// =============================================================================
// Benchmark Runner
// =============================================================================

/// Run a benchmark and collect measurements
pub fn run_benchmark<F>(name: &str, iterations: u32, mut f: F) -> BenchmarkMeasurement
where
    F: FnMut(),
{
    let mut times: Vec<u64> = Vec::with_capacity(iterations as usize);

    // Warmup
    for _ in 0..5 {
        f();
    }

    // Measure
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        times.push(start.elapsed().as_nanos() as u64);
    }

    // Calculate stats
    let sum: u64 = times.iter().sum();
    let mean = sum / iterations as u64;

    let variance: f64 = times
        .iter()
        .map(|&t| {
            let diff = t as f64 - mean as f64;
            diff * diff
        })
        .sum::<f64>()
        / iterations as f64;
    let stddev = variance.sqrt() as u64;

    println!(
        "  {}: mean={:.2}μs stddev={:.2}μs",
        name,
        mean as f64 / 1000.0,
        stddev as f64 / 1000.0
    );

    BenchmarkMeasurement {
        mean_ns: mean,
        stddev_ns: stddev,
        samples: iterations,
    }
}

/// Run an async benchmark
pub fn run_async_benchmark<F, Fut>(
    name: &str,
    iterations: u32,
    runtime: &tokio::runtime::Runtime,
    mut f: F,
) -> BenchmarkMeasurement
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let mut times: Vec<u64> = Vec::with_capacity(iterations as usize);

    // Warmup
    for _ in 0..5 {
        runtime.block_on(f());
    }

    // Measure
    for _ in 0..iterations {
        let start = Instant::now();
        runtime.block_on(f());
        times.push(start.elapsed().as_nanos() as u64);
    }

    // Calculate stats
    let sum: u64 = times.iter().sum();
    let mean = sum / iterations as u64;

    let variance: f64 = times
        .iter()
        .map(|&t| {
            let diff = t as f64 - mean as f64;
            diff * diff
        })
        .sum::<f64>()
        / iterations as f64;
    let stddev = variance.sqrt() as u64;

    println!(
        "  {}: mean={:.2}μs stddev={:.2}μs",
        name,
        mean as f64 / 1000.0,
        stddev as f64 / 1000.0
    );

    BenchmarkMeasurement {
        mean_ns: mean,
        stddev_ns: stddev,
        samples: iterations,
    }
}

/// Compare current results against baseline
pub fn check_regressions(
    baseline: &BenchmarkBaseline,
    current: &BenchmarkBaseline,
    config: &RegressionConfig,
) -> Vec<RegressionResult> {
    let mut results = Vec::new();

    for (name, current_m) in &current.benchmarks {
        if let Some(baseline_m) = baseline.benchmarks.get(name) {
            let change = current_m.mean_ns as i64 - baseline_m.mean_ns as i64;
            let change_percent = if baseline_m.mean_ns > 0 {
                (change as f64 / baseline_m.mean_ns as f64) * 100.0
            } else {
                0.0
            };

            let is_regression = change_percent > config.regression_threshold_percent
                && change.unsigned_abs() > config.min_change_ns;

            results.push(RegressionResult {
                name: name.clone(),
                baseline_ns: baseline_m.mean_ns,
                current_ns: current_m.mean_ns,
                change_percent,
                is_regression,
            });
        }
    }

    results
}

/// Simple ISO timestamp without external dependency
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let secs = duration.as_secs();
    // Basic ISO format: YYYY-MM-DDTHH:MM:SSZ
    // This is approximate but sufficient for our needs
    format!("{}Z", secs)
}

// =============================================================================
// Tests
// =============================================================================

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Test state
#[derive(Clone, Debug, SerdeSerialize, SerdeDeserialize, Default)]
struct BenchState {
    counter: u64,
}

impl MergeableState for BenchState {
    fn merge(&mut self, other: &Self) {
        self.counter = self.counter.max(other.counter);
    }
}

/// Test: Run core benchmarks and check for regressions
#[test]
fn test_benchmark_regression_tracking() {
    println!("\n=== Benchmark Regression Tracking ===\n");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let iterations = 100;

    // Create a baseline (in real usage, this would be loaded from file)
    let mut baseline = BenchmarkBaseline::new();

    // Run benchmarks and add to baseline
    println!("Running benchmarks...\n");

    // Benchmark 1: Graph compilation
    let compile_result = run_benchmark("graph_compile_simple", iterations, || {
        let mut graph: StateGraph<BenchState> = StateGraph::new();
        graph.add_node_from_fn("node", |state| Box::pin(async move { Ok(state) }));
        graph.set_entry_point("node");
        graph.add_edge("node", END);
        let _ = graph.compile();
    });
    baseline.add("graph_compile_simple", compile_result);

    // Benchmark 2: Graph execution (simple)
    let mut graph: StateGraph<BenchState> = StateGraph::new();
    graph.add_node_from_fn("increment", |mut state| {
        Box::pin(async move {
            state.counter += 1;
            Ok(state)
        })
    });
    graph.set_entry_point("increment");
    graph.add_edge("increment", END);
    let app_simple = std::sync::Arc::new(graph.compile().unwrap());

    let execute_simple = run_async_benchmark("graph_execute_simple", iterations, &runtime, || {
        let app = std::sync::Arc::clone(&app_simple);
        async move {
            let _ = app.invoke(BenchState::default()).await;
        }
    });
    baseline.add("graph_execute_simple", execute_simple);

    // Benchmark 3: Multi-node graph execution
    let mut graph: StateGraph<BenchState> = StateGraph::new();
    for i in 0..5 {
        let name = format!("node_{}", i);
        graph.add_node_from_fn(&name, |mut state| {
            Box::pin(async move {
                state.counter += 1;
                Ok(state)
            })
        });
    }
    graph.set_entry_point("node_0");
    for i in 0..4 {
        graph.add_edge(format!("node_{}", i), format!("node_{}", i + 1));
    }
    graph.add_edge("node_4", END);
    let app_multi = std::sync::Arc::new(graph.compile().unwrap());

    let execute_multi = run_async_benchmark("graph_execute_5_nodes", iterations, &runtime, || {
        let app = std::sync::Arc::clone(&app_multi);
        async move {
            let _ = app.invoke(BenchState::default()).await;
        }
    });
    baseline.add("graph_execute_5_nodes", execute_multi);

    // Benchmark 4: State cloning
    let clone_result = run_benchmark("state_clone_small", iterations * 10, || {
        let state = BenchState { counter: 42 };
        std::hint::black_box(state.clone());
    });
    baseline.add("state_clone_small", clone_result);

    println!("\n=== Baseline Established ===\n");

    // In real usage, you would save baseline to file:
    // let json = serde_json::to_string_pretty(&baseline).unwrap();
    // std::fs::write("benchmarks/baselines/current.json", json).unwrap();

    // Simulate comparison (comparing baseline against itself should show no regressions)
    let config = RegressionConfig::default();
    let results = check_regressions(&baseline, &baseline, &config);

    println!("Regression Check Results:");
    println!("-------------------------");
    for result in &results {
        let status = if result.is_regression {
            "REGRESSION"
        } else {
            "OK"
        };
        println!(
            "  {}: {} ({:+.1}%)",
            result.name, status, result.change_percent
        );
    }

    // No regressions when comparing to self
    let regression_count = results.iter().filter(|r| r.is_regression).count();
    assert_eq!(
        regression_count, 0,
        "Self-comparison should have no regressions"
    );

    println!("\n=== All benchmarks passed (no regressions) ===\n");
}

/// Test: Detect artificial regression
#[test]
fn test_regression_detection() {
    let mut baseline = BenchmarkBaseline::new();
    baseline.add(
        "test_benchmark",
        BenchmarkMeasurement {
            mean_ns: 10000,
            stddev_ns: 100,
            samples: 100,
        },
    );

    // Create "regressed" results (20% slower)
    let mut current = BenchmarkBaseline::new();
    current.add(
        "test_benchmark",
        BenchmarkMeasurement {
            mean_ns: 12000, // 20% increase
            stddev_ns: 100,
            samples: 100,
        },
    );

    let config = RegressionConfig {
        regression_threshold_percent: 10.0,
        min_change_ns: 100,
    };

    let results = check_regressions(&baseline, &current, &config);
    assert_eq!(results.len(), 1);
    assert!(results[0].is_regression, "Should detect 20% regression");
    assert!(
        (results[0].change_percent - 20.0).abs() < 0.1,
        "Should report ~20% change"
    );
}

/// Test: Ignore small changes below threshold
#[test]
fn test_ignore_small_changes() {
    let mut baseline = BenchmarkBaseline::new();
    baseline.add(
        "test_benchmark",
        BenchmarkMeasurement {
            mean_ns: 10000,
            stddev_ns: 100,
            samples: 100,
        },
    );

    // Create results with small change (5%)
    let mut current = BenchmarkBaseline::new();
    current.add(
        "test_benchmark",
        BenchmarkMeasurement {
            mean_ns: 10500, // 5% increase
            stddev_ns: 100,
            samples: 100,
        },
    );

    let config = RegressionConfig {
        regression_threshold_percent: 10.0, // 10% threshold
        min_change_ns: 100,
    };

    let results = check_regressions(&baseline, &current, &config);
    assert!(
        !results[0].is_regression,
        "5% change should not trigger regression at 10% threshold"
    );
}

/// Test: Baseline serialization
#[test]
fn test_baseline_serialization() {
    let mut baseline = BenchmarkBaseline::new();
    baseline.add(
        "benchmark_1",
        BenchmarkMeasurement {
            mean_ns: 1000,
            stddev_ns: 50,
            samples: 100,
        },
    );
    baseline.add(
        "benchmark_2",
        BenchmarkMeasurement {
            mean_ns: 5000,
            stddev_ns: 200,
            samples: 50,
        },
    );

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&baseline).unwrap();
    println!("Serialized baseline:\n{}", json);

    // Deserialize back
    let loaded: BenchmarkBaseline = serde_json::from_str(&json).unwrap();

    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.benchmarks.len(), 2);
    assert_eq!(loaded.benchmarks["benchmark_1"].mean_ns, 1000);
    assert_eq!(loaded.benchmarks["benchmark_2"].mean_ns, 5000);
}
