// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Performance benchmarking for DashFlow Streaming applications.
//!
//! This module provides tools for running performance benchmarks with statistical analysis.
//! Benchmarks run an evaluation multiple times to measure latency, throughput, and cost metrics
//! with confidence intervals and regression detection.
//!
//! # Example
//!
//! ```rust
//! use dashflow_streaming::evals::benchmark::{BenchmarkConfig, BenchmarkRunner, BenchmarkResult};
//! use dashflow_streaming::evals::metrics::EvalMetrics;
//!
//! // Run a benchmark with 50 iterations
//! let config = BenchmarkConfig {
//!     iterations: 50,
//!     warmup_iterations: 5,
//!     confidence_level: 0.95,
//!     parallel: false,
//! };
//!
//! let mut runner = BenchmarkRunner::new(config);
//!
//! // Simulate collecting metrics from 50 runs
//! for _ in 0..50 {
//!     let metrics = EvalMetrics::default();
//!     runner.add_sample(metrics);
//! }
//!
//! let result = runner.analyze();
//!
//! println!("P50 latency: {:.2}ms", result.p50_latency);
//! println!("P95 latency: {:.2}ms", result.p95_latency);
//! println!("P99 latency: {:.2}ms", result.p99_latency);
//! println!("Throughput: {:.2} req/s", result.throughput);
//! ```

use crate::evals::metrics::EvalMetrics;
use serde::{Deserialize, Serialize};

/// Configuration for benchmark runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Number of iterations to run (after warmup)
    pub iterations: usize,

    /// Number of warmup iterations (not included in statistics)
    pub warmup_iterations: usize,

    /// Confidence level for confidence intervals (e.g., 0.95 for 95%)
    pub confidence_level: f64,

    /// Whether to run iterations in parallel (default: false for accurate timing)
    pub parallel: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 50,
            warmup_iterations: 5,
            confidence_level: 0.95,
            parallel: false,
        }
    }
}

/// Statistical analysis results from a benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Number of samples analyzed
    pub sample_count: usize,

    // Latency statistics (milliseconds)
    /// Minimum latency observed (milliseconds).
    pub min_latency: f64,
    /// Maximum latency observed (milliseconds).
    pub max_latency: f64,
    /// Mean latency observed (milliseconds).
    pub mean_latency: f64,
    /// Median latency observed (milliseconds).
    pub median_latency: f64,
    /// 50th percentile latency (milliseconds).
    pub p50_latency: f64,
    /// 95th percentile latency (milliseconds).
    pub p95_latency: f64,
    /// 99th percentile latency (milliseconds).
    pub p99_latency: f64,
    /// Standard deviation of latency (milliseconds).
    pub stddev_latency: f64,
    /// Lower bound of the latency confidence interval (milliseconds).
    pub latency_ci_lower: f64, // confidence interval lower bound
    /// Upper bound of the latency confidence interval (milliseconds).
    pub latency_ci_upper: f64, // confidence interval upper bound

    // Throughput (requests per second)
    /// Throughput measured in requests per second.
    pub throughput: f64,

    // Token usage statistics
    /// Minimum tokens observed per request.
    pub min_tokens: u64,
    /// Maximum tokens observed per request.
    pub max_tokens: u64,
    /// Mean tokens observed per request.
    pub mean_tokens: f64,
    /// Median tokens observed per request.
    pub median_tokens: u64,
    /// Standard deviation of tokens observed per request.
    pub stddev_tokens: f64,

    // Cost statistics (USD)
    /// Minimum cost observed per request (USD).
    pub min_cost: f64,
    /// Maximum cost observed per request (USD).
    pub max_cost: f64,
    /// Mean cost observed per request (USD).
    pub mean_cost: f64,
    /// Median cost observed per request (USD).
    pub median_cost: f64,
    /// Standard deviation of cost observed per request (USD).
    pub stddev_cost: f64,

    // Quality metrics (if available)
    /// Mean correctness score, if correctness is evaluated.
    pub mean_correctness: Option<f64>,
    /// Mean relevance score, if relevance is evaluated.
    pub mean_relevance: Option<f64>,
    /// Mean safety score, if safety is evaluated.
    pub mean_safety: Option<f64>,
}

/// Runner for executing benchmarks and collecting statistics.
pub struct BenchmarkRunner {
    config: BenchmarkConfig,
    samples: Vec<EvalMetrics>,
    in_warmup: bool,
    warmup_count: usize,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner with the given configuration.
    #[must_use]
    pub fn new(config: BenchmarkConfig) -> Self {
        let in_warmup = config.warmup_iterations > 0;
        Self {
            config,
            samples: Vec::new(),
            in_warmup,
            warmup_count: 0,
        }
    }

    /// Add a sample to the benchmark.
    ///
    /// During warmup phase, samples are discarded. After warmup, samples are stored for analysis.
    pub fn add_sample(&mut self, metrics: EvalMetrics) {
        if self.in_warmup {
            self.warmup_count += 1;
            if self.warmup_count >= self.config.warmup_iterations {
                self.in_warmup = false;
            }
        } else {
            self.samples.push(metrics);
        }
    }

    /// Check if enough samples have been collected.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.in_warmup && self.samples.len() >= self.config.iterations
    }

    /// Get the number of samples collected (excluding warmup).
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Analyze collected samples and return benchmark results.
    ///
    /// # Panics
    ///
    /// Panics if no samples have been collected.
    #[must_use]
    pub fn analyze(&self) -> BenchmarkResult {
        assert!(!self.samples.is_empty(), "No samples to analyze");

        // Collect latency values (p95_latency from each sample)
        let mut latencies: Vec<f64> = self.samples.iter().map(|m| m.p95_latency).collect();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Collect token values
        let mut tokens: Vec<u64> = self.samples.iter().map(|m| m.total_tokens).collect();
        tokens.sort_unstable();

        // Collect cost values
        let mut costs: Vec<f64> = self.samples.iter().map(|m| m.cost_per_run).collect();
        costs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Calculate latency statistics
        let min_latency = latencies[0];
        let max_latency = latencies[latencies.len() - 1];
        let mean_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let median_latency = percentile(&latencies, 0.5);
        let p50_latency = percentile(&latencies, 0.5);
        let p95_latency = percentile(&latencies, 0.95);
        let p99_latency = percentile(&latencies, 0.99);
        let stddev_latency = standard_deviation(&latencies, mean_latency);

        // Calculate confidence interval for latency
        let (latency_ci_lower, latency_ci_upper) = confidence_interval(
            mean_latency,
            stddev_latency,
            latencies.len(),
            self.config.confidence_level,
        );

        // Calculate throughput (requests per second)
        // Throughput = 1000 / mean_latency (convert ms to seconds)
        let throughput = 1000.0 / mean_latency;

        // Calculate token statistics
        let min_tokens = tokens[0];
        let max_tokens = tokens[tokens.len() - 1];
        let mean_tokens = tokens.iter().sum::<u64>() as f64 / tokens.len() as f64;
        let median_tokens = tokens[tokens.len() / 2];
        let stddev_tokens = standard_deviation(
            &tokens.iter().map(|&t| t as f64).collect::<Vec<_>>(),
            mean_tokens,
        );

        // Calculate cost statistics
        let min_cost = costs[0];
        let max_cost = costs[costs.len() - 1];
        let mean_cost = costs.iter().sum::<f64>() / costs.len() as f64;
        let median_cost = costs[costs.len() / 2];
        let stddev_cost = standard_deviation(&costs, mean_cost);

        // Calculate quality metrics (if available)
        let mean_correctness = calculate_mean_option(
            &self
                .samples
                .iter()
                .map(|m| m.correctness)
                .collect::<Vec<_>>(),
        );
        let mean_relevance =
            calculate_mean_option(&self.samples.iter().map(|m| m.relevance).collect::<Vec<_>>());
        let mean_safety =
            calculate_mean_option(&self.samples.iter().map(|m| m.safety).collect::<Vec<_>>());

        BenchmarkResult {
            sample_count: self.samples.len(),
            min_latency,
            max_latency,
            mean_latency,
            median_latency,
            p50_latency,
            p95_latency,
            p99_latency,
            stddev_latency,
            latency_ci_lower,
            latency_ci_upper,
            throughput,
            min_tokens,
            max_tokens,
            mean_tokens,
            median_tokens,
            stddev_tokens,
            min_cost,
            max_cost,
            mean_cost,
            median_cost,
            stddev_cost,
            mean_correctness,
            mean_relevance,
            mean_safety,
        }
    }
}

/// Compare benchmark results to detect performance regressions.
///
/// Returns `true` if the current benchmark shows a statistically significant regression
/// compared to the baseline.
#[must_use]
pub fn detect_performance_regression(
    baseline: &BenchmarkResult,
    current: &BenchmarkResult,
    threshold: f64,
) -> bool {
    // Use p95 latency for comparison (industry standard)
    let current_p95 = current.p95_latency;

    // Check if current p95 is outside baseline confidence interval
    current_p95 > baseline.latency_ci_upper * (1.0 + threshold)
}

/// Format benchmark results as a human-readable report.
#[must_use]
pub fn format_benchmark_report(result: &BenchmarkResult) -> String {
    let mut report = String::new();

    report.push_str("=== Benchmark Report ===\n\n");
    report.push_str(&format!("Samples: {}\n\n", result.sample_count));

    report.push_str("Latency (ms):\n");
    report.push_str(&format!("  Min:     {:.2}\n", result.min_latency));
    report.push_str(&format!("  P50:     {:.2}\n", result.p50_latency));
    report.push_str(&format!("  Mean:    {:.2}\n", result.mean_latency));
    report.push_str(&format!("  P95:     {:.2}\n", result.p95_latency));
    report.push_str(&format!("  P99:     {:.2}\n", result.p99_latency));
    report.push_str(&format!("  Max:     {:.2}\n", result.max_latency));
    report.push_str(&format!("  StdDev:  {:.2}\n", result.stddev_latency));
    report.push_str(&format!(
        "  95% CI:  [{:.2}, {:.2}]\n\n",
        result.latency_ci_lower, result.latency_ci_upper
    ));

    report.push_str(&format!("Throughput: {:.2} req/s\n\n", result.throughput));

    report.push_str("Tokens:\n");
    report.push_str(&format!("  Min:     {}\n", result.min_tokens));
    report.push_str(&format!("  Median:  {}\n", result.median_tokens));
    report.push_str(&format!("  Mean:    {:.0}\n", result.mean_tokens));
    report.push_str(&format!("  Max:     {}\n", result.max_tokens));
    report.push_str(&format!("  StdDev:  {:.2}\n\n", result.stddev_tokens));

    report.push_str("Cost (USD):\n");
    report.push_str(&format!("  Min:     ${:.6}\n", result.min_cost));
    report.push_str(&format!("  Median:  ${:.6}\n", result.median_cost));
    report.push_str(&format!("  Mean:    ${:.6}\n", result.mean_cost));
    report.push_str(&format!("  Max:     ${:.6}\n", result.max_cost));
    report.push_str(&format!("  StdDev:  ${:.6}\n\n", result.stddev_cost));

    if let Some(correctness) = result.mean_correctness {
        report.push_str("Quality:\n");
        report.push_str(&format!("  Correctness: {correctness:.2}\n"));
        if let Some(relevance) = result.mean_relevance {
            report.push_str(&format!("  Relevance:   {relevance:.2}\n"));
        }
        if let Some(safety) = result.mean_safety {
            report.push_str(&format!("  Safety:      {safety:.2}\n"));
        }
    }

    report
}

/// Format a comparison report between baseline and current benchmarks.
#[must_use]
pub fn format_comparison_report(
    baseline: &BenchmarkResult,
    current: &BenchmarkResult,
    threshold: f64,
) -> String {
    let mut report = String::new();

    report.push_str("=== Benchmark Comparison ===\n\n");

    // Latency comparison
    let latency_change =
        ((current.p95_latency - baseline.p95_latency) / baseline.p95_latency) * 100.0;
    let latency_regressed = detect_performance_regression(baseline, current, threshold);

    report.push_str("P95 Latency:\n");
    report.push_str(&format!(
        "  Baseline: {:.2}ms (95% CI: [{:.2}, {:.2}])\n",
        baseline.p95_latency, baseline.latency_ci_lower, baseline.latency_ci_upper
    ));
    report.push_str(&format!(
        "  Current:  {:.2}ms (95% CI: [{:.2}, {:.2}])\n",
        current.p95_latency, current.latency_ci_lower, current.latency_ci_upper
    ));
    report.push_str(&format!("  Change:   {latency_change:+.2}% "));

    if latency_regressed {
        report.push_str("⚠️  REGRESSION DETECTED\n\n");
    } else if latency_change < 0.0 {
        report.push_str("✅ IMPROVED\n\n");
    } else {
        report.push_str("✅ ACCEPTABLE\n\n");
    }

    // Throughput comparison
    let throughput_change =
        ((current.throughput - baseline.throughput) / baseline.throughput) * 100.0;
    report.push_str("Throughput:\n");
    report.push_str(&format!("  Baseline: {:.2} req/s\n", baseline.throughput));
    report.push_str(&format!("  Current:  {:.2} req/s\n", current.throughput));
    report.push_str(&format!("  Change:   {throughput_change:+.2}%\n\n"));

    // Token comparison
    let token_change =
        ((current.mean_tokens - baseline.mean_tokens) / baseline.mean_tokens) * 100.0;
    report.push_str("Tokens:\n");
    report.push_str(&format!("  Baseline: {:.0}\n", baseline.mean_tokens));
    report.push_str(&format!("  Current:  {:.0}\n", current.mean_tokens));
    report.push_str(&format!("  Change:   {token_change:+.2}%\n\n"));

    // Cost comparison
    let cost_change = ((current.mean_cost - baseline.mean_cost) / baseline.mean_cost) * 100.0;
    report.push_str("Cost:\n");
    report.push_str(&format!("  Baseline: ${:.6}\n", baseline.mean_cost));
    report.push_str(&format!("  Current:  ${:.6}\n", current.mean_cost));
    report.push_str(&format!("  Change:   {cost_change:+.2}%\n\n"));

    report
}

// Statistical helper functions

/// Calculate percentile from sorted data.
fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    assert!(!sorted_data.is_empty(), "Data must not be empty");
    assert!(
        (0.0..=1.0).contains(&p),
        "Percentile must be between 0 and 1"
    );

    let index = (p * (sorted_data.len() - 1) as f64).round() as usize;
    sorted_data[index]
}

/// Calculate standard deviation.
fn standard_deviation(data: &[f64], mean: f64) -> f64 {
    let variance = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64;
    variance.sqrt()
}

/// Calculate confidence interval using t-distribution approximation.
///
/// For large samples (n > 30), t-distribution ≈ normal distribution.
/// For 95% confidence: z = 1.96
/// For 99% confidence: z = 2.576
fn confidence_interval(mean: f64, stddev: f64, n: usize, confidence: f64) -> (f64, f64) {
    // Z-scores for common confidence levels
    let z = if (confidence - 0.95).abs() < 0.01 {
        1.96
    } else if (confidence - 0.99).abs() < 0.01 {
        2.576
    } else {
        // Default to 95%
        1.96
    };

    let margin = z * stddev / (n as f64).sqrt();
    (mean - margin, mean + margin)
}

/// Calculate mean of optional values (ignoring None).
fn calculate_mean_option(values: &[Option<f64>]) -> Option<f64> {
    let valid_values: Vec<f64> = values.iter().filter_map(|&x| x).collect();
    if valid_values.is_empty() {
        None
    } else {
        Some(valid_values.iter().sum::<f64>() / valid_values.len() as f64)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_config_default() {
        let config = BenchmarkConfig::default();
        assert_eq!(config.iterations, 50);
        assert_eq!(config.warmup_iterations, 5);
        assert_eq!(config.confidence_level, 0.95);
        assert!(!config.parallel);
    }

    #[test]
    fn test_benchmark_runner_warmup() {
        let config = BenchmarkConfig {
            iterations: 3,
            warmup_iterations: 2,
            confidence_level: 0.95,
            parallel: false,
        };

        let mut runner = BenchmarkRunner::new(config);

        // Add warmup samples
        runner.add_sample(EvalMetrics::default());
        assert_eq!(runner.sample_count(), 0); // Still in warmup
        assert!(!runner.is_complete());

        runner.add_sample(EvalMetrics::default());
        assert_eq!(runner.sample_count(), 0); // Warmup complete, no real samples yet

        // Add real samples
        runner.add_sample(EvalMetrics::default());
        assert_eq!(runner.sample_count(), 1);

        runner.add_sample(EvalMetrics::default());
        assert_eq!(runner.sample_count(), 2);

        runner.add_sample(EvalMetrics::default());
        assert_eq!(runner.sample_count(), 3);
        assert!(runner.is_complete());
    }

    #[test]
    fn test_benchmark_runner_analyze() {
        let config = BenchmarkConfig {
            iterations: 10,
            warmup_iterations: 0,
            confidence_level: 0.95,
            parallel: false,
        };

        let mut runner = BenchmarkRunner::new(config);

        // Add samples with varying latencies
        for i in 0..10 {
            let metrics = EvalMetrics {
                p95_latency: 1000.0 + (i as f64 * 100.0),
                total_tokens: 100 + (i * 10),
                cost_per_run: 0.001 + (i as f64 * 0.0001),
                ..Default::default()
            };
            runner.add_sample(metrics);
        }

        let result = runner.analyze();

        assert_eq!(result.sample_count, 10);
        assert_eq!(result.min_latency, 1000.0);
        assert_eq!(result.max_latency, 1900.0);
        assert!((result.mean_latency - 1450.0).abs() < 0.01);
        assert!(result.p95_latency > result.p50_latency);
        assert!(result.throughput > 0.0);
    }

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(percentile(&data, 0.0), 1.0);
        assert_eq!(percentile(&data, 0.5), 3.0);
        assert_eq!(percentile(&data, 1.0), 5.0);
    }

    #[test]
    fn test_standard_deviation() {
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let mean = 5.0;
        let stddev = standard_deviation(&data, mean);

        // Expected stddev ≈ 2.0
        assert!((stddev - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_confidence_interval() {
        let (lower, upper) = confidence_interval(100.0, 10.0, 100, 0.95);

        // For n=100, stddev=10, mean=100, 95% CI should be ≈ [98.04, 101.96]
        assert!((lower - 98.04).abs() < 0.1);
        assert!((upper - 101.96).abs() < 0.1);
    }

    #[test]
    fn test_detect_performance_regression_no_regression() {
        let baseline = BenchmarkResult {
            sample_count: 50,
            p95_latency: 1000.0,
            latency_ci_lower: 950.0,
            latency_ci_upper: 1050.0,
            ..mock_benchmark_result()
        };

        let current = BenchmarkResult {
            sample_count: 50,
            p95_latency: 1020.0, // 2% increase, within threshold
            ..mock_benchmark_result()
        };

        assert!(!detect_performance_regression(&baseline, &current, 0.2)); // 20% threshold
    }

    #[test]
    fn test_detect_performance_regression_with_regression() {
        let baseline = BenchmarkResult {
            sample_count: 50,
            p95_latency: 1000.0,
            latency_ci_lower: 950.0,
            latency_ci_upper: 1050.0,
            ..mock_benchmark_result()
        };

        let current = BenchmarkResult {
            sample_count: 50,
            p95_latency: 1300.0, // 30% increase, exceeds 20% threshold
            ..mock_benchmark_result()
        };

        assert!(detect_performance_regression(&baseline, &current, 0.2)); // 20% threshold
    }

    #[test]
    fn test_format_benchmark_report() {
        let result = mock_benchmark_result();
        let report = format_benchmark_report(&result);

        assert!(report.contains("=== Benchmark Report ==="));
        assert!(report.contains("Samples: 50"));
        assert!(report.contains("Latency (ms)"));
        assert!(report.contains("Throughput:"));
        assert!(report.contains("Tokens:"));
        assert!(report.contains("Cost (USD):"));
    }

    #[test]
    fn test_format_comparison_report() {
        let baseline = mock_benchmark_result();
        let current = BenchmarkResult {
            p95_latency: 1100.0, // 10% slower
            mean_tokens: 110.0,  // 10% more tokens
            mean_cost: 0.0011,   // 10% more expensive
            ..mock_benchmark_result()
        };

        let report = format_comparison_report(&baseline, &current, 0.2);

        assert!(report.contains("=== Benchmark Comparison ==="));
        assert!(report.contains("P95 Latency:"));
        assert!(report.contains("Throughput:"));
        assert!(report.contains("Tokens:"));
        assert!(report.contains("Cost:"));
    }

    #[test]
    fn test_calculate_mean_option() {
        let values = vec![Some(0.9), Some(0.95), None, Some(0.92)];
        let mean = calculate_mean_option(&values);

        assert!(mean.is_some());
        assert!((mean.unwrap() - 0.9233).abs() < 0.01);
    }

    #[test]
    fn test_calculate_mean_option_all_none() {
        let values = vec![None, None, None];
        let mean = calculate_mean_option(&values);

        assert!(mean.is_none());
    }

    // Helper function for tests
    fn mock_benchmark_result() -> BenchmarkResult {
        BenchmarkResult {
            sample_count: 50,
            min_latency: 900.0,
            max_latency: 1200.0,
            mean_latency: 1000.0,
            median_latency: 1000.0,
            p50_latency: 1000.0,
            p95_latency: 1000.0,
            p99_latency: 1150.0,
            stddev_latency: 50.0,
            latency_ci_lower: 986.0,
            latency_ci_upper: 1014.0,
            throughput: 1.0,
            min_tokens: 80,
            max_tokens: 120,
            mean_tokens: 100.0,
            median_tokens: 100,
            stddev_tokens: 10.0,
            min_cost: 0.0008,
            max_cost: 0.0012,
            mean_cost: 0.001,
            median_cost: 0.001,
            stddev_cost: 0.0001,
            mean_correctness: Some(0.95),
            mean_relevance: Some(0.90),
            mean_safety: Some(1.0),
        }
    }
}
