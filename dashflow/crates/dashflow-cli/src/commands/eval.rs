// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for eval command
// - float_cmp: Exact threshold comparisons for evaluation metrics
#![allow(clippy::float_cmp)]

//! eval - Evaluate graph performance on a test dataset
//!
//! This command evaluates predictions against ground truth using standard metrics
//! (exact match, F1, precision, recall). It works with JSONL files containing
//! JSON objects with expected and predicted fields.

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use colored::Colorize;
use dashflow::optimize::{exact_match, f1_score, precision_score, recall_score, JsonMetricConfig};
use dashflow::state::JsonState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Output format for evaluation results
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table
    Table,
    /// JSON output
    Json,
    /// CSV output
    Csv,
}

/// Evaluation metric to compute
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum EvalMetric {
    /// Exact string match
    ExactMatch,
    /// F1 score (token overlap)
    F1,
    /// Precision
    Precision,
    /// Recall
    Recall,
    /// BLEU score (not yet implemented in CLI - use library API)
    Bleu,
    /// ROUGE-L score (not yet implemented in CLI - use library API)
    RougeL,
    /// LLM-as-judge (not yet implemented in CLI - use library API)
    LlmJudge,
    /// All available metrics
    All,
}

#[derive(Args)]
pub struct EvalArgs {
    /// Path to graph definition or optimized state
    #[arg(short, long)]
    pub graph: PathBuf,

    /// Path to test dataset (JSONL)
    #[arg(short, long)]
    pub testset: PathBuf,

    /// Metrics to compute
    #[arg(short, long, value_enum, default_value_t = EvalMetric::All)]
    pub metric: EvalMetric,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Output path for results (stdout if not specified)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// LLM provider for execution
    #[arg(long, default_value = "openai")]
    pub provider: String,

    /// LLM model name
    #[arg(long, default_value = "gpt-4o-mini")]
    pub model: String,

    /// Judge model for LLM-as-judge metric
    #[arg(long, default_value = "gpt-4o")]
    pub judge_model: String,

    /// Number of parallel workers
    #[arg(long, default_value_t = 4)]
    pub workers: usize,

    /// Maximum examples to evaluate (all if not specified)
    #[arg(long)]
    pub limit: Option<usize>,

    /// Show individual example results
    #[arg(long)]
    pub show_examples: bool,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Field name for expected output in test data
    #[arg(long, default_value = "answer")]
    pub expected_field: String,

    /// Field name for actual/predicted output in test data
    #[arg(long, default_value = "output")]
    pub actual_field: String,
}

/// Per-example evaluation result
#[derive(Debug, Serialize, Deserialize)]
struct ExampleResult {
    index: usize,
    input: serde_json::Value,
    expected: serde_json::Value,
    actual: serde_json::Value,
    scores: std::collections::HashMap<String, f64>,
    passed: bool,
}

/// Overall evaluation summary
#[derive(Debug, Serialize, Deserialize)]
struct EvalSummary {
    total_examples: usize,
    passed: usize,
    failed: usize,
    metrics: std::collections::HashMap<String, f64>,
    duration_seconds: f64,
    model: String,
}

pub async fn run(args: EvalArgs) -> Result<()> {
    println!(
        "{} evaluation on {}",
        "Starting".bright_green(),
        args.testset.display()
    );
    println!("  Graph: {}", args.graph.display());
    println!("  Model: {}/{}", args.provider, args.model);
    println!("  Metric: {:?}", args.metric);

    // Warn about unimplemented metrics
    match args.metric {
        EvalMetric::Bleu | EvalMetric::RougeL | EvalMetric::LlmJudge => {
            println!(
                "  {} {:?} is not yet implemented in CLI. Computing ExactMatch/F1/Precision/Recall instead.",
                "Warning:".bright_yellow(),
                args.metric
            );
            println!(
                "  {}",
                "For BLEU/ROUGE-L/LlmJudge, use the library API: dashflow::quality::metrics"
                    .bright_black()
            );
        }
        _ => {}
    }

    // Validate files exist
    if !args.graph.exists() {
        anyhow::bail!("Graph file not found: {}", args.graph.display());
    }
    if !args.testset.exists() {
        anyhow::bail!("Test data not found: {}", args.testset.display());
    }

    let start = std::time::Instant::now();

    // Load test data (use tokio::fs to avoid blocking the async runtime)
    let testset_content = tokio::fs::read_to_string(&args.testset)
        .await
        .with_context(|| format!("Failed to read test data: {}", args.testset.display()))?;

    let mut examples: Vec<serde_json::Value> = testset_content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(i, line)| {
            serde_json::from_str(line)
                .with_context(|| format!("Failed to parse line {} of test data", i + 1))
        })
        .collect::<Result<Vec<_>>>()?;

    // Apply limit if specified
    if let Some(limit) = args.limit {
        examples.truncate(limit);
    }

    println!(
        "  {} {} test examples",
        "Loaded".bright_cyan(),
        examples.len()
    );

    // Configure metrics based on field names (for future use with compute_all_json_metrics)
    let _metric_config = JsonMetricConfig::new(&args.expected_field, &args.actual_field);

    if args.verbose {
        println!(
            "  Expected field: '{}', Actual field: '{}'",
            args.expected_field, args.actual_field
        );
    }

    // Evaluate each example using real metrics from dashflow
    let mut example_results: Vec<ExampleResult> = Vec::new();
    let mut metric_sums = std::collections::HashMap::new();
    metric_sums.insert("exact_match".to_string(), 0.0);
    metric_sums.insert("f1".to_string(), 0.0);
    metric_sums.insert("precision".to_string(), 0.0);
    metric_sums.insert("recall".to_string(), 0.0);

    let total = examples.len();
    let mut passed = 0;

    for (idx, example) in examples.iter().enumerate() {
        // Convert JSON to JsonState for metric evaluation
        let state = JsonState::from(example.clone());

        // For evaluation, we compare expected vs actual within the same record
        // (typical format: {"question": "...", "answer": "expected", "output": "predicted"})
        let expected_val = state.get_str(&args.expected_field).unwrap_or("");
        let actual_val = state.get_str(&args.actual_field).unwrap_or("");

        // Compute individual metrics
        let mut scores = std::collections::HashMap::new();
        scores.insert(
            "exact_match".to_string(),
            exact_match(expected_val, actual_val),
        );
        scores.insert("f1".to_string(), f1_score(actual_val, expected_val));
        scores.insert(
            "precision".to_string(),
            precision_score(actual_val, expected_val),
        );
        scores.insert("recall".to_string(), recall_score(actual_val, expected_val));

        // Accumulate for averages
        for (metric, score) in &scores {
            metric_sums
                .entry(metric.clone())
                .and_modify(|sum| *sum += score)
                .or_insert(*score);
        }

        let is_passed = scores.get("exact_match").copied().unwrap_or(0.0) == 1.0;
        if is_passed {
            passed += 1;
        }

        if args.show_examples || args.verbose {
            example_results.push(ExampleResult {
                index: idx,
                input: example.clone(),
                expected: serde_json::json!(expected_val),
                actual: serde_json::json!(actual_val),
                scores: scores.clone(),
                passed: is_passed,
            });
        }

        // Progress indicator for large datasets
        if args.verbose && (idx + 1) % 100 == 0 {
            println!("  Evaluated {}/{} examples...", idx + 1, total);
        }
    }

    // Compute average metrics
    let mut metrics = std::collections::HashMap::new();
    for (metric, sum) in metric_sums {
        metrics.insert(metric, sum / total as f64);
    }

    let failed = total - passed;
    let duration = start.elapsed();

    let summary = EvalSummary {
        total_examples: total,
        passed,
        failed,
        metrics: metrics.clone(),
        duration_seconds: duration.as_secs_f64(),
        model: format!("{}/{}", args.provider, args.model),
    };

    // Output results based on format
    match args.format {
        OutputFormat::Table => print_table_output(&summary, args.show_examples),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&summary)?;
            if let Some(output_path) = &args.output {
                // Use tokio::fs to avoid blocking the async runtime
                tokio::fs::write(output_path, &json).await?;
                println!("Results written to: {}", output_path.display());
            } else {
                println!("{}", json);
            }
        }
        OutputFormat::Csv => {
            let mut csv = String::new();
            csv.push_str("metric,value\n");
            for (metric, value) in &summary.metrics {
                csv.push_str(&format!("{},{:.4}\n", metric, value));
            }
            if let Some(output_path) = &args.output {
                // Use tokio::fs to avoid blocking the async runtime
                tokio::fs::write(output_path, &csv).await?;
                println!("Results written to: {}", output_path.display());
            } else {
                println!("{}", csv);
            }
        }
    }

    Ok(())
}

fn print_table_output(summary: &EvalSummary, _show_examples: bool) {
    println!();
    println!("{}", "=== Evaluation Results ===".bright_white().bold());
    println!();

    // Summary
    println!("  {} examples evaluated", summary.total_examples);
    println!(
        "  {} / {} ({:.1}%)",
        format!("{} passed", summary.passed).bright_green(),
        format!("{} failed", summary.failed).bright_red(),
        (summary.passed as f64 / summary.total_examples as f64) * 100.0
    );
    println!("  Duration: {:.1}s", summary.duration_seconds);
    println!();

    // Metrics table
    println!("{}", "Metrics:".bright_white());
    println!("  {:<15} {:>10}", "Metric", "Score");
    println!("  {}", "-".repeat(27));

    let mut metrics: Vec<_> = summary.metrics.iter().collect();
    metrics.sort_by(|a, b| a.0.cmp(b.0));

    for (metric, value) in metrics {
        let score_str = format!("{:.2}%", value * 100.0);
        let colored_score = if *value >= 0.9 {
            score_str.bright_green()
        } else if *value >= 0.7 {
            score_str.bright_yellow()
        } else {
            score_str.bright_red()
        };
        println!("  {:<15} {:>10}", metric, colored_score);
    }

    println!();
    println!("{} Evaluation complete", "✓".bright_green());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_eval_missing_testset() {
        let mut graph = NamedTempFile::new().unwrap();
        writeln!(graph, r#"{{"nodes": []}}"#).unwrap();

        let args = EvalArgs {
            graph: graph.path().to_path_buf(),
            testset: PathBuf::from("/nonexistent/testset.jsonl"),
            metric: EvalMetric::ExactMatch,
            format: OutputFormat::Table,
            output: None,
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            judge_model: "gpt-4o".to_string(),
            workers: 4,
            limit: None,
            show_examples: false,
            verbose: false,
            expected_field: "answer".to_string(),
            actual_field: "output".to_string(),
        };

        let result = run(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_output_format_values() {
        // Just verify enum parsing works
        let _table: OutputFormat = OutputFormat::Table;
        let _json: OutputFormat = OutputFormat::Json;
        let _csv: OutputFormat = OutputFormat::Csv;
    }

    #[tokio::test]
    async fn test_eval_with_real_metrics() {
        // Create test files
        let mut graph = NamedTempFile::new().unwrap();
        writeln!(graph, r#"{{"nodes": []}}"#).unwrap();

        let mut testset = NamedTempFile::new().unwrap();
        // Test data: 3 exact matches, 1 partial match
        writeln!(
            testset,
            r#"{{"question": "Q1", "answer": "Paris", "output": "paris"}}"#
        )
        .unwrap();
        writeln!(
            testset,
            r#"{{"question": "Q2", "answer": "London", "output": "London"}}"#
        )
        .unwrap();
        writeln!(
            testset,
            r#"{{"question": "Q3", "answer": "Tokyo", "output": "Tokyo"}}"#
        )
        .unwrap();
        writeln!(
            testset,
            r#"{{"question": "Q4", "answer": "Berlin", "output": "Berlin Germany"}}"#
        )
        .unwrap();

        let args = EvalArgs {
            graph: graph.path().to_path_buf(),
            testset: testset.path().to_path_buf(),
            metric: EvalMetric::All,
            format: OutputFormat::Table,
            output: None,
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            judge_model: "gpt-4o".to_string(),
            workers: 4,
            limit: None,
            show_examples: false,
            verbose: false,
            expected_field: "answer".to_string(),
            actual_field: "output".to_string(),
        };

        // Should succeed with real metric computation
        let result = run(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_eval_json_output() {
        let mut graph = NamedTempFile::new().unwrap();
        writeln!(graph, r#"{{"nodes": []}}"#).unwrap();

        let mut testset = NamedTempFile::new().unwrap();
        writeln!(testset, r#"{{"answer": "test", "output": "test"}}"#).unwrap();

        let output_file = NamedTempFile::new().unwrap();

        let args = EvalArgs {
            graph: graph.path().to_path_buf(),
            testset: testset.path().to_path_buf(),
            metric: EvalMetric::ExactMatch,
            format: OutputFormat::Json,
            output: Some(output_file.path().to_path_buf()),
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            judge_model: "gpt-4o".to_string(),
            workers: 4,
            limit: None,
            show_examples: false,
            verbose: false,
            expected_field: "answer".to_string(),
            actual_field: "output".to_string(),
        };

        let result = run(args).await;
        assert!(result.is_ok());

        // Verify output file was written
        let output_content = std::fs::read_to_string(output_file.path()).unwrap();
        let summary: serde_json::Value = serde_json::from_str(&output_content).unwrap();

        // Should have 100% exact match for identical inputs
        assert_eq!(summary["total_examples"], 1);
        assert_eq!(summary["passed"], 1);
        assert_eq!(summary["metrics"]["exact_match"], 1.0);
    }
}
