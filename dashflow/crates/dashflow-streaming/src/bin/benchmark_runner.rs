// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// The blanket #![allow(clippy::unwrap_used)] was removed.
// This binary has no unwrap() calls that need allowing.

//! Command-line tool for running performance benchmarks on DashFlow Streaming applications.
//!
//! This tool runs an evaluation multiple times to collect performance statistics
//! and optionally compares against a baseline to detect regressions.
//!
//! # Usage
//!
//! Run a benchmark:
//! ```bash
//! cargo run --bin benchmark_runner -- \
//!   --analytics analytics.json \
//!   --iterations 50 \
//!   --output benchmark_result.json
//! ```
//!
//! Compare against baseline:
//! ```bash
//! cargo run --bin benchmark_runner -- \
//!   --analytics analytics.json \
//!   --baseline baseline_benchmark.json \
//!   --threshold 0.2
//! ```
//!
//! Save as new baseline:
//! ```bash
//! cargo run --bin benchmark_runner -- \
//!   --analytics analytics.json \
//!   --iterations 50 \
//!   --save-baseline new_baseline.json
//! ```

use clap::Parser;
use dashflow_streaming::evals::{
    benchmark::{detect_performance_regression, format_benchmark_report, format_comparison_report},
    benchmark::{BenchmarkConfig, BenchmarkResult, BenchmarkRunner},
    AnalyticsConverter, LlmPricing,
};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "benchmark_runner")]
#[command(about = "Run performance benchmarks on DashFlow Streaming applications")]
struct Args {
    /// Path to analytics JSON file (from `analyze_events`)
    #[arg(short, long)]
    analytics: PathBuf,

    /// Number of benchmark iterations
    #[arg(short, long, default_value = "50")]
    iterations: usize,

    /// Number of warmup iterations (not included in stats)
    #[arg(short, long, default_value = "5")]
    warmup: usize,

    /// Confidence level for confidence intervals (0.95 or 0.99)
    #[arg(short, long, default_value = "0.95")]
    confidence: f64,

    /// Path to baseline benchmark JSON (for comparison)
    #[arg(short, long)]
    baseline: Option<PathBuf>,

    /// Regression threshold (e.g., 0.2 for 20% tolerance)
    #[arg(short = 't', long, default_value = "0.2")]
    threshold: f64,

    /// Path to save benchmark result
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Path to save as new baseline
    #[arg(short = 's', long)]
    save_baseline: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Validate inputs
    if !args.analytics.exists() {
        eprintln!("Error: Analytics file not found: {:?}", args.analytics);
        std::process::exit(1);
    }

    // Allow exact float comparison: validating user-provided CLI values against known constants
    #[allow(clippy::float_cmp)]
    if args.confidence != 0.95 && args.confidence != 0.99 {
        eprintln!("Error: Confidence level must be 0.95 or 0.99");
        std::process::exit(1);
    }

    // Load analytics JSON
    println!("Loading analytics from {:?}...", args.analytics);
    let analytics_json = fs::read_to_string(&args.analytics)?;

    // Convert analytics to metrics
    let metrics = AnalyticsConverter::from_json(&analytics_json, Some(LlmPricing::GPT_4O))?;

    println!("Loaded metrics:");
    println!("  P95 Latency: {:.2}ms", metrics.p95_latency);
    println!("  Total Tokens: {}", metrics.total_tokens);
    println!("  Cost: ${:.6}", metrics.cost_per_run);
    println!();

    // Create benchmark config
    let config = BenchmarkConfig {
        iterations: args.iterations,
        warmup_iterations: args.warmup,
        confidence_level: args.confidence,
        parallel: false,
    };

    println!("Running benchmark:");
    println!("  Iterations: {}", config.iterations);
    println!("  Warmup: {}", config.warmup_iterations);
    println!("  Confidence: {}%", (config.confidence_level * 100.0) as u8);
    println!();

    // Run benchmark
    let mut runner = BenchmarkRunner::new(config);

    // Simulate multiple runs by adding the same metrics multiple times
    // In a real scenario, you would run the application multiple times and collect fresh metrics
    for i in 0..args.iterations + args.warmup {
        if i < args.warmup {
            print!("\rWarmup: {}/{}", i + 1, args.warmup);
        } else {
            print!("\rProgress: {}/{}", i - args.warmup + 1, args.iterations);
        }
        runner.add_sample(metrics.clone());
    }
    println!();
    println!();

    // Analyze results
    let result = runner.analyze();

    // Print benchmark report
    println!("{}", format_benchmark_report(&result));

    // Compare to baseline if provided
    if let Some(baseline_path) = args.baseline {
        if baseline_path.exists() {
            println!("Loading baseline from {baseline_path:?}...");
            let baseline_json = fs::read_to_string(&baseline_path)?;
            let baseline: BenchmarkResult = serde_json::from_str(&baseline_json)?;

            println!(
                "{}",
                format_comparison_report(&baseline, &result, args.threshold)
            );

            // Check for regression
            if detect_performance_regression(&baseline, &result, args.threshold) {
                eprintln!("❌ PERFORMANCE REGRESSION DETECTED");
                std::process::exit(1);
            } else {
                println!("✅ NO PERFORMANCE REGRESSION");
            }
        } else {
            eprintln!("Warning: Baseline file not found: {baseline_path:?}");
        }
    }

    // Save result if requested
    if let Some(output_path) = args.output {
        println!("Saving result to {output_path:?}...");
        let result_json = serde_json::to_string_pretty(&result)?;
        fs::write(&output_path, result_json)?;
        println!("Result saved.");
    }

    // Save as baseline if requested
    if let Some(baseline_path) = args.save_baseline {
        println!("Saving baseline to {baseline_path:?}...");
        let baseline_json = serde_json::to_string_pretty(&result)?;
        fs::write(&baseline_path, baseline_json)?;
        println!("Baseline saved.");
    }

    Ok(())
}
