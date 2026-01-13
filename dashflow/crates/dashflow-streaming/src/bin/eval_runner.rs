// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// The blanket #![allow(clippy::unwrap_used)] was removed.
// Targeted allows are used where serialization of internal types is infallible.

//! Evaluation Runner
//!
//! CLI tool to run evaluations against baselines.
//!
//! # Usage
//!
//! ```bash
//! # Convert analytics to metrics and save as baseline
//! cargo run --bin analyze_events --format json | \
//!   cargo run --bin eval_runner -- --save-baseline baselines/librarian_v1.0.0.json
//!
//! # Compare current run to baseline
//! cargo run --bin analyze_events --format json | \
//!   cargo run --bin eval_runner -- --baseline baselines/librarian_v1.0.0.json
//!
//! # Specify LLM pricing
//! cargo run --bin analyze_events --format json | \
//!   cargo run --bin eval_runner -- --baseline baselines/librarian_v1.0.0.json --pricing gpt-3.5
//! ```

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use dashflow_streaming::evals::{
    average_correctness, count_by_severity, detect_regressions, has_critical_regressions,
    score_suite, AnalyticsConverter, Baseline, EvalMetrics, EvalSuite, LlmPricing,
    RegressionThresholds, ScoringMethod,
};
use std::io::{self, Read};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PricingModel {
    Gpt4o,
    Gpt35,
    Claude35,
}

impl PricingModel {
    fn to_pricing(self) -> LlmPricing {
        match self {
            Self::Gpt4o => LlmPricing::GPT_4O,
            Self::Gpt35 => LlmPricing::GPT_35_TURBO,
            Self::Claude35 => LlmPricing::CLAUDE_35_SONNET,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "eval_runner")]
#[command(about = "Evaluation runner for DashFlow applications", long_about = None)]
struct Args {
    /// Save metrics as baseline to this path
    #[arg(long)]
    save_baseline: Option<String>,

    /// Load baseline from this path for comparison
    #[arg(long)]
    baseline: Option<String>,

    /// Application name (for baseline metadata)
    #[arg(long, default_value = "app")]
    app_name: String,

    /// Version (for baseline metadata)
    #[arg(long, default_value = "1.0.0")]
    version: String,

    /// LLM pricing model
    #[arg(long, value_enum, default_value = "gpt4o")]
    pricing: PricingModel,

    /// Output format (text or json)
    #[arg(long, default_value = "text")]
    format: String,

    /// Regression threshold mode (default, strict, or lenient)
    #[arg(long, default_value = "default")]
    threshold_mode: String,

    /// Exit with non-zero code if critical regressions detected
    #[arg(long, default_value = "true")]
    fail_on_regression: bool,

    /// Run evaluations against golden dataset (eval suite JSON file)
    #[arg(long)]
    eval_suite: Option<String>,

    /// Scoring method for eval suite (exact, case-insensitive, fuzzy, contains)
    #[arg(long, default_value = "fuzzy")]
    scoring_method: String,

    /// Minimum correctness threshold (0.0-1.0) for eval suite to pass
    #[arg(long, default_value = "0.8")]
    correctness_threshold: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle eval suite mode (separate workflow)
    if let Some(eval_suite_path) = &args.eval_suite {
        return run_eval_suite(&args, eval_suite_path);
    }

    // Read analytics JSON from stdin
    let mut analytics_json = String::new();
    io::stdin()
        .read_to_string(&mut analytics_json)
        .context("Failed to read analytics JSON from stdin")?;

    // Convert to metrics
    let metrics = AnalyticsConverter::from_json(&analytics_json, Some(args.pricing.to_pricing()))
        .context("Failed to convert analytics to metrics")?;

    // Handle save baseline
    if let Some(save_path) = args.save_baseline {
        let baseline = Baseline::new(args.app_name.clone(), args.version.clone(), metrics);
        baseline
            .save(&save_path)
            .with_context(|| format!("Failed to save baseline to {save_path}"))?;

        println!("✓ Baseline saved to {save_path}");
        return Ok(());
    }

    // Handle comparison to baseline
    if let Some(baseline_path) = args.baseline {
        let baseline = Baseline::load(&baseline_path)
            .with_context(|| format!("Failed to load baseline from {baseline_path}"))?;

        // Get thresholds based on mode
        let thresholds = match args.threshold_mode.as_str() {
            "strict" => RegressionThresholds::strict(),
            "lenient" => RegressionThresholds::lenient(),
            _ => RegressionThresholds::default(),
        };

        // Detect regressions
        let regressions = detect_regressions(&baseline.metrics, &metrics, &thresholds);
        let has_critical = has_critical_regressions(&regressions);

        // Print results
        match args.format.as_str() {
            "json" => {
                print_json_comparison_with_regressions(&metrics, &baseline.metrics, &regressions);
            }
            _ => print_text_comparison_with_regressions(
                &metrics,
                &baseline.metrics,
                &regressions,
                &args.app_name,
                &baseline_path,
            ),
        }

        // Exit with error if critical regressions and fail_on_regression=true
        if has_critical && args.fail_on_regression {
            std::process::exit(1);
        }
    } else {
        // No baseline, just print metrics
        match args.format.as_str() {
            "json" => print_json_metrics(&metrics),
            _ => print_text_metrics(&metrics, &args.app_name),
        }
    }

    Ok(())
}

fn print_text_metrics(metrics: &EvalMetrics, app_name: &str) {
    println!("=== Evaluation Metrics ===\n");
    println!("App: {app_name}");
    println!();

    println!("QUALITY:");
    if let Some(c) = metrics.correctness {
        println!("  Correctness:        {:.2}%", c * 100.0);
    }
    if let Some(r) = metrics.relevance {
        println!("  Relevance:          {:.2}%", r * 100.0);
    }
    if let Some(s) = metrics.safety {
        println!("  Safety:             {:.2}%", s * 100.0);
    }
    if let Some(h) = metrics.hallucination_rate {
        println!("  Hallucination Rate: {:.2}%", h * 100.0);
    }
    if metrics.correctness.is_none()
        && metrics.relevance.is_none()
        && metrics.safety.is_none()
        && metrics.hallucination_rate.is_none()
    {
        println!("  (not evaluated)");
    }
    println!();

    println!("PERFORMANCE:");
    println!("  P95 Latency:  {:.2}ms", metrics.p95_latency);
    println!("  Avg Latency:  {:.2}ms", metrics.avg_latency);
    println!("  Success Rate: {:.1}%", metrics.success_rate * 100.0);
    println!("  Error Rate:   {:.1}%", metrics.error_rate * 100.0);
    println!();

    println!("COST:");
    println!("  Total Tokens: {}", metrics.total_tokens);
    println!("  Cost per Run: ${:.5}", metrics.cost_per_run);
    println!("  Tool Calls:   {}", metrics.tool_calls);
}

// SAFETY: EvalMetrics derives Serialize with standard serde types only. Serialization
// can only fail if the type definition is invalid, which would be caught at development time.
#[allow(clippy::unwrap_used)]
fn print_json_metrics(metrics: &EvalMetrics) {
    let json = serde_json::to_string_pretty(metrics).unwrap();
    println!("{json}");
}

fn print_text_comparison_with_regressions(
    metrics: &EvalMetrics,
    baseline: &EvalMetrics,
    regressions: &[dashflow_streaming::evals::Regression],
    app_name: &str,
    baseline_path: &str,
) {
    use dashflow_streaming::evals::Regression;

    println!("=== Evaluation Report ===\n");
    println!("App: {app_name}");
    println!("Baseline: {baseline_path}");
    println!();

    // Group regressions by metric for easy lookup
    let regression_map: std::collections::HashMap<String, &Regression> =
        regressions.iter().map(|r| (r.metric.clone(), r)).collect();

    println!("QUALITY:");
    let mut has_quality = false;
    if let (Some(c), Some(bc)) = (metrics.correctness, baseline.correctness) {
        has_quality = true;
        if let Some(reg) = regression_map.get("correctness") {
            println!("  {}", reg.format_colored());
        } else {
            println!(
                "  ✓ Correctness:        {:.2}% (baseline: {:.2}%)",
                c * 100.0,
                bc * 100.0
            );
        }
    }
    if let (Some(r), Some(br)) = (metrics.relevance, baseline.relevance) {
        has_quality = true;
        if let Some(reg) = regression_map.get("relevance") {
            println!("  {}", reg.format_colored());
        } else {
            println!(
                "  ✓ Relevance:          {:.2}% (baseline: {:.2}%)",
                r * 100.0,
                br * 100.0
            );
        }
    }
    if let (Some(s), Some(bs)) = (metrics.safety, baseline.safety) {
        has_quality = true;
        if let Some(reg) = regression_map.get("safety") {
            println!("  {}", reg.format_colored());
        } else {
            println!(
                "  ✓ Safety:             {:.2}% (baseline: {:.2}%)",
                s * 100.0,
                bs * 100.0
            );
        }
    }
    if let (Some(h), Some(bh)) = (metrics.hallucination_rate, baseline.hallucination_rate) {
        has_quality = true;
        if let Some(reg) = regression_map.get("hallucination_rate") {
            println!("  {}", reg.format_colored());
        } else {
            println!(
                "  ✓ Hallucination Rate: {:.2}% (baseline: {:.2}%)",
                h * 100.0,
                bh * 100.0
            );
        }
    }
    if !has_quality {
        println!("  (not evaluated)");
    }
    println!();

    println!("PERFORMANCE:");
    if let Some(reg) = regression_map.get("p95_latency") {
        println!("  {}", reg.format_colored());
    } else {
        println!(
            "  ✓ P95 Latency:  {:.2}ms (baseline: {:.2}ms)",
            metrics.p95_latency, baseline.p95_latency
        );
    }

    if let Some(reg) = regression_map.get("avg_latency") {
        println!("  {}", reg.format_colored());
    } else {
        println!(
            "  ✓ Avg Latency:  {:.2}ms (baseline: {:.2}ms)",
            metrics.avg_latency, baseline.avg_latency
        );
    }

    if let Some(reg) = regression_map.get("success_rate") {
        println!("  {}", reg.format_colored());
    } else {
        println!(
            "  ✓ Success Rate: {:.1}% (baseline: {:.1}%)",
            metrics.success_rate * 100.0,
            baseline.success_rate * 100.0
        );
    }

    if let Some(reg) = regression_map.get("error_rate") {
        println!("  {}", reg.format_colored());
    } else {
        println!(
            "  ✓ Error Rate:   {:.1}% (baseline: {:.1}%)",
            metrics.error_rate * 100.0,
            baseline.error_rate * 100.0
        );
    }
    println!();

    println!("COST:");
    if let Some(reg) = regression_map.get("total_tokens") {
        println!("  {}", reg.format_colored());
    } else {
        println!(
            "  ✓ Total Tokens: {} (baseline: {})",
            metrics.total_tokens, baseline.total_tokens
        );
    }

    if let Some(reg) = regression_map.get("cost_per_run") {
        println!("  {}", reg.format_colored());
    } else {
        println!(
            "  ✓ Cost per Run: ${:.5} (baseline: ${:.5})",
            metrics.cost_per_run, baseline.cost_per_run
        );
    }

    if let Some(reg) = regression_map.get("tool_calls") {
        println!("  {}", reg.format_colored());
    } else {
        println!(
            "  ✓ Tool Calls:   {} (baseline: {})",
            metrics.tool_calls, baseline.tool_calls
        );
    }
    println!();

    // Print summary
    let (critical, warning, _info) = count_by_severity(regressions);
    if regressions.is_empty() {
        println!("✓ PASSED: No regressions detected");
    } else if critical > 0 {
        println!("✗ FAILED: {critical} critical regression(s), {warning} warning(s)");
    } else {
        println!("⚠ PASSED (with warnings): {warning} warning(s)");
    }
}

// SAFETY: serde_json::json! macro produces values that always serialize successfully.
#[allow(clippy::unwrap_used)]
fn print_json_comparison_with_regressions(
    metrics: &EvalMetrics,
    baseline: &EvalMetrics,
    regressions: &[dashflow_streaming::evals::Regression],
) {
    let (critical, warning, info) = count_by_severity(regressions);
    let has_critical = has_critical_regressions(regressions);

    let comparison = serde_json::json!({
        "status": if has_critical { "FAILED" } else if warning > 0 { "PASSED_WITH_WARNINGS" } else { "PASSED" },
        "summary": {
            "critical_regressions": critical,
            "warnings": warning,
            "info": info,
        },
        "current": metrics,
        "baseline": baseline,
        "regressions": regressions,
    });

    let json = serde_json::to_string_pretty(&comparison).unwrap();
    println!("{json}");
}

/// Run evaluation suite mode
///
/// This mode loads an eval suite and scores actual answers (provided via stdin as JSON array).
/// The input should be a JSON array of strings with one answer per test case, in the same order
/// as the eval suite.
///
/// # Example Input
///
/// ```json
/// ["Paris", "William Shakespeare", "O(log n)", ...]
/// ```
fn run_eval_suite(args: &Args, eval_suite_path: &str) -> Result<()> {
    // Load eval suite
    let suite = EvalSuite::load(eval_suite_path)
        .with_context(|| format!("Failed to load eval suite from {eval_suite_path}"))?;

    // Parse scoring method
    let scoring_method = match args.scoring_method.as_str() {
        "exact" => ScoringMethod::ExactMatch,
        "case-insensitive" => ScoringMethod::CaseInsensitiveMatch,
        "fuzzy" => ScoringMethod::FuzzyMatch,
        "contains" => ScoringMethod::Contains,
        _ => {
            anyhow::bail!(
                "Invalid scoring method '{}'. Use: exact, case-insensitive, fuzzy, or contains",
                args.scoring_method
            );
        }
    };

    // Read actual answers from stdin (JSON array of strings)
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("Failed to read actual answers from stdin")?;

    let actual_answers: Vec<String> = serde_json::from_str(input.trim())
        .context("Failed to parse actual answers as JSON array of strings")?;

    // Score the suite
    let scores = score_suite(&suite, &actual_answers, scoring_method)
        .context("Failed to score eval suite")?;

    let avg_correctness = average_correctness(&scores);

    // Print results
    match args.format.as_str() {
        "json" => print_eval_suite_json(&suite, &actual_answers, &scores, avg_correctness),
        _ => print_eval_suite_text(
            &suite,
            &actual_answers,
            &scores,
            avg_correctness,
            args.correctness_threshold,
        ),
    }

    // Exit with error if below threshold
    if avg_correctness < args.correctness_threshold {
        eprintln!(
            "\n✗ FAILED: Correctness {:.1}% is below threshold {:.1}%",
            avg_correctness * 100.0,
            args.correctness_threshold * 100.0
        );
        std::process::exit(1);
    }

    Ok(())
}

/// Print eval suite results in text format
fn print_eval_suite_text(
    suite: &EvalSuite,
    actual_answers: &[String],
    scores: &[f64],
    avg_correctness: f64,
    threshold: f64,
) {
    println!("=== Eval Suite Results ===\n");
    println!("Suite: {} v{}", suite.name, suite.version);
    println!("Description: {}", suite.description);
    println!("Test Cases: {}", suite.len());
    println!();

    // Print individual results
    let mut passed = 0;
    let mut failed = 0;

    for (i, (case, (actual, score))) in suite
        .cases
        .iter()
        .zip(actual_answers.iter().zip(scores.iter()))
        .enumerate()
    {
        let pass = *score >= threshold;
        if pass {
            passed += 1;
        } else {
            failed += 1;
        }

        let icon = if pass { "✓" } else { "✗" };
        println!(
            "[{}] Case {}: {} ({:.1}%)",
            icon,
            i + 1,
            case.id,
            score * 100.0
        );
        println!("    Query:    {}", case.query);
        println!("    Expected: {}", case.expected_answer);
        println!("    Actual:   {actual}");
        println!();
    }

    // Print summary
    println!("=== Summary ===");
    println!("Average Correctness: {:.1}%", avg_correctness * 100.0);
    println!("Threshold: {:.1}%", threshold * 100.0);
    println!("Passed: {} / {}", passed, suite.len());
    println!("Failed: {} / {}", failed, suite.len());

    if avg_correctness >= threshold {
        println!("\n✓ PASSED");
    } else {
        println!("\n✗ FAILED");
    }
}

/// Print eval suite results in JSON format
// SAFETY: serde_json::json! macro produces values that always serialize successfully.
#[allow(clippy::unwrap_used)]
fn print_eval_suite_json(
    suite: &EvalSuite,
    actual_answers: &[String],
    scores: &[f64],
    avg_correctness: f64,
) {
    use serde_json::json;

    let results: Vec<_> = suite
        .cases
        .iter()
        .zip(actual_answers.iter().zip(scores.iter()))
        .map(|(case, (actual, score))| {
            json!({
                "id": case.id,
                "query": case.query,
                "expected_answer": case.expected_answer,
                "actual_answer": actual,
                "score": score,
                "metadata": case.metadata,
            })
        })
        .collect();

    let output = json!({
        "suite": {
            "name": suite.name,
            "version": suite.version,
            "description": suite.description,
        },
        "results": results,
        "summary": {
            "average_correctness": avg_correctness,
            "total_cases": suite.len(),
        },
    });

    let json = serde_json::to_string_pretty(&output).unwrap();
    println!("{json}");
}
