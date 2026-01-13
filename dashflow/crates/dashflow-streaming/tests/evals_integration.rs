// Evals Integration Tests
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox

//! Integration tests for the evaluation framework test harness
//!
//! These tests demonstrate how to integrate the evals framework with
//! Rust's test harness using the provided helpers, assertion macros,
//! and mock fixtures.
//!
//! Run these tests with:
//! ```bash
//! cargo test --package dashflow-streaming --test evals_integration
//! ```

use dashflow_streaming::evals::{
    has_critical_regressions, mock_baseline, mock_metrics, Baseline, EvalMetrics, EvalTestRunner,
    RegressionSeverity, RegressionThresholds,
};
use dashflow_streaming::{
    assert_metric_within_threshold, assert_no_critical_regressions, assert_quality_maintained,
};
use anyhow::{Context, Result};
use std::io::Write;
use tempfile::NamedTempFile;

// Helper function to create a baseline file
fn create_baseline_file(baseline: &Baseline) -> Result<NamedTempFile> {
    let mut file = NamedTempFile::new().context("create temp baseline file")?;
    let json = serde_json::to_string_pretty(baseline).context("serialize baseline json")?;
    file.write_all(json.as_bytes())
        .context("write baseline json to temp file")?;
    file.flush().context("flush baseline file")?;
    Ok(file)
}

#[test]
fn test_eval_runner_no_regression() -> Result<()> {
    // Setup: Create baseline
    let baseline = mock_baseline("librarian", "1.0.0", 1850.0, 1047.5, 150);
    let baseline_file = create_baseline_file(&baseline)?;

    // Create test runner
    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Test: Run with metrics that don't regress
    let metrics = mock_metrics(1800.0, 1000.0, 145);

    // Assert: No regressions detected
    let regressions = runner.run(&metrics);
    assert!(regressions.is_empty(), "Expected no regressions");

    Ok(())
}

#[test]
fn test_eval_runner_performance_warning() -> Result<()> {
    // Setup: Create baseline
    let baseline = mock_baseline("librarian", "1.0.0", 1000.0, 500.0, 150);
    let baseline_file = create_baseline_file(&baseline)?;

    // Create test runner with default thresholds
    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Test: Run with slower performance (exceeds 20% threshold)
    // Baseline: 1000.0, threshold: 1.2, so 1200+ triggers warning
    let metrics = mock_metrics(1250.0, 625.0, 150);

    // Assert: Warning regression but not critical
    let regressions = runner.run(&metrics);
    assert!(!regressions.is_empty(), "Expected performance warning");
    assert!(
        !has_critical_regressions(&regressions),
        "Should not be critical"
    );

    Ok(())
}

#[test]
fn test_eval_runner_quality_regression() -> Result<()> {
    // Setup: Create baseline
    let baseline = mock_baseline("librarian", "1.0.0", 1850.0, 1047.5, 150);
    let baseline_file = create_baseline_file(&baseline)?;

    // Create test runner
    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Test: Run with quality degradation
    let mut metrics = mock_metrics(1800.0, 1000.0, 150);
    metrics.correctness = Some(0.85); // Drop from 0.95 to 0.85 (10% absolute)

    // Assert: Critical regression detected
    let regressions = runner.run(&metrics);
    assert!(
        has_critical_regressions(&regressions),
        "Expected critical regression"
    );

    // Check that it's a correctness regression
    let correctness_regression = regressions
        .iter()
        .find(|r| r.metric == "correctness")
        .context("Expected correctness regression")?;
    assert!(matches!(
        correctness_regression.severity,
        RegressionSeverity::Critical
    ));

    Ok(())
}

#[test]
fn test_eval_runner_with_strict_thresholds() -> Result<()> {
    // Setup: Create baseline
    let baseline = mock_baseline("librarian", "1.0.0", 1000.0, 500.0, 150);
    let baseline_file = create_baseline_file(&baseline)?;

    // Create test runner with strict thresholds
    let mut runner =
        EvalTestRunner::with_thresholds(baseline_file.path(), RegressionThresholds::strict());

    // Test: Run with slight performance degradation
    // Baseline: 1000.0, strict threshold: 1.1, so 1100+ triggers warning
    let metrics = mock_metrics(1150.0, 575.0, 150);

    // Assert: Strict thresholds catch smaller regressions
    let regressions = runner.run(&metrics);
    assert!(
        !regressions.is_empty(),
        "Strict thresholds should detect small regressions"
    );

    Ok(())
}

#[test]
fn test_eval_runner_with_lenient_thresholds() -> Result<()> {
    // Setup: Create baseline
    let baseline = mock_baseline("librarian", "1.0.0", 1000.0, 500.0, 150);
    let baseline_file = create_baseline_file(&baseline)?;

    // Create test runner with lenient thresholds
    let mut runner =
        EvalTestRunner::with_thresholds(baseline_file.path(), RegressionThresholds::lenient());

    // Test: Run with moderate performance degradation
    let metrics = mock_metrics(1200.0, 600.0, 180);

    // Assert: Lenient thresholds allow more variation
    let regressions = runner.run(&metrics);
    assert!(
        regressions.is_empty() || !has_critical_regressions(&regressions),
        "Lenient thresholds should be more permissive"
    );

    Ok(())
}

#[test]
fn test_assert_no_critical_regressions_macro_pass() {
    // Setup
    let baseline = mock_metrics(1850.0, 1000.0, 150);
    let current = mock_metrics(1800.0, 1000.0, 150);
    let thresholds = RegressionThresholds::default();

    // Test: Should not panic
    assert_no_critical_regressions!(current, baseline, thresholds);
}

// NOTE: The following tests use #[should_panic] INTENTIONALLY.
// These test assertion macros (assert_no_critical_regressions!, assert_metric_within_threshold!,
// assert_quality_maintained!) which are designed to panic when validation fails.
// Testing that assertions panic is the correct behavior here - these are regression guards.

#[test]
#[should_panic(expected = "Critical regressions detected")]
fn test_assert_no_critical_regressions_macro_fail() {
    // Setup
    let baseline = mock_metrics(1850.0, 1000.0, 150);
    let mut current = mock_metrics(1800.0, 1000.0, 150);
    current.correctness = Some(0.80); // Significant drop
    let thresholds = RegressionThresholds::default();

    // Test: Should panic with critical regression message
    assert_no_critical_regressions!(current, baseline, thresholds);
}

#[test]
fn test_assert_metric_within_threshold_macro_pass() {
    // Setup
    let current_latency = 1800.0;
    let baseline_latency = 1850.0;
    let threshold = 1.2; // 20% slower allowed

    // Test: Should not panic (improvement)
    assert_metric_within_threshold!(current_latency, baseline_latency, threshold, "P95 latency");
}

#[test]
#[should_panic(expected = "P95 latency regressed")]
fn test_assert_metric_within_threshold_macro_fail() {
    // Setup
    let current_latency = 2300.0;
    let baseline_latency = 1850.0;
    let threshold = 1.2; // 20% slower allowed

    // Test: Should panic (2300 > 1850 * 1.2 = 2220)
    assert_metric_within_threshold!(current_latency, baseline_latency, threshold, "P95 latency");
}

#[test]
fn test_assert_quality_maintained_macro_pass() {
    // Setup
    let current_correctness = 0.94;
    let baseline_correctness = 0.95;
    let threshold = 0.05; // 5% absolute drop allowed

    // Test: Should not panic (0.94 >= 0.95 - 0.05 = 0.90)
    assert_quality_maintained!(
        current_correctness,
        baseline_correctness,
        threshold,
        "Correctness"
    );
}

#[test]
#[should_panic(expected = "Correctness degraded")]
fn test_assert_quality_maintained_macro_fail() {
    // Setup
    let current_correctness = 0.85;
    let baseline_correctness = 0.95;
    let threshold = 0.05; // 5% absolute drop allowed

    // Test: Should panic (0.85 < 0.95 - 0.05 = 0.90)
    assert_quality_maintained!(
        current_correctness,
        baseline_correctness,
        threshold,
        "Correctness"
    );
}

#[test]
fn test_mock_metrics_helper() -> Result<()> {
    // Test: mock_metrics creates valid EvalMetrics
    let metrics = mock_metrics(1000.0, 500.0, 100);

    assert!((metrics.p95_latency - 1000.0).abs() < 1e-12);
    assert!((metrics.avg_latency - 500.0).abs() < 1e-12);
    assert_eq!(metrics.total_tokens, 100);
    assert!((metrics.success_rate - 100.0).abs() < 1e-12);
    assert!((metrics.error_rate - 0.0).abs() < 1e-12);

    let correctness = metrics.correctness.context("expected correctness")?;
    assert!((correctness - 0.95).abs() < 1e-12);

    let relevance = metrics.relevance.context("expected relevance")?;
    assert!((relevance - 0.90).abs() < 1e-12);

    let safety = metrics.safety.context("expected safety")?;
    assert!((safety - 1.0).abs() < 1e-12);

    let hallucination_rate = metrics
        .hallucination_rate
        .context("expected hallucination_rate")?;
    assert!((hallucination_rate - 0.05).abs() < 1e-12);

    Ok(())
}

#[test]
fn test_mock_baseline_helper() -> Result<()> {
    // Test: mock_baseline creates valid Baseline
    let baseline = mock_baseline("test_app", "1.0.0", 1000.0, 500.0, 100);

    assert_eq!(baseline.app_name, "test_app");
    assert_eq!(baseline.version, "1.0.0");
    assert!((baseline.metrics.p95_latency - 1000.0).abs() < 1e-12);
    assert!((baseline.metrics.avg_latency - 500.0).abs() < 1e-12);
    assert_eq!(baseline.metrics.total_tokens, 100);

    Ok(())
}

#[test]
fn test_cost_regression_detection() -> Result<()> {
    // Setup
    let baseline = mock_baseline("test_app", "1.0.0", 2000.0, 1200.0, 200);
    let baseline_file = create_baseline_file(&baseline)?;

    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Test: Token usage increased by 60% (exceeds 50% threshold)
    let metrics = mock_metrics(2000.0, 1200.0, 320);

    // Assert: Cost warning detected
    let regressions = runner.run(&metrics);
    assert!(!regressions.is_empty(), "Expected token usage warning");

    let token_regression = regressions
        .iter()
        .find(|r| r.metric == "total_tokens")
        .context("Expected token regression")?;
    assert!(matches!(
        token_regression.severity,
        RegressionSeverity::Warning
    ));

    Ok(())
}

#[test]
fn test_safety_regression_detection() -> Result<()> {
    // Setup
    let baseline = mock_baseline("chat_bot", "1.0.0", 1500.0, 800.0, 120);
    let baseline_file = create_baseline_file(&baseline)?;

    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Test: Safety score dropped from 1.0 to 0.90 (10% absolute)
    let mut metrics = mock_metrics(1500.0, 800.0, 120);
    metrics.safety = Some(0.90);

    // Assert: Critical safety regression
    let regressions = runner.run(&metrics);
    assert!(
        has_critical_regressions(&regressions),
        "Expected critical safety regression"
    );

    let safety_regression = regressions
        .iter()
        .find(|r| r.metric == "safety")
        .context("Expected safety regression")?;
    assert!(matches!(
        safety_regression.severity,
        RegressionSeverity::Critical
    ));

    Ok(())
}

#[test]
fn test_run_and_check_success() -> Result<()> {
    // Setup
    let baseline = mock_baseline("test", "1.0", 1850.0, 1000.0, 150);
    let baseline_file = create_baseline_file(&baseline)?;

    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Test: No regressions
    let metrics = mock_metrics(1800.0, 1000.0, 150);

    // Assert: run_and_check returns Ok
    let result = runner.run_and_check(&metrics);
    assert!(result.is_ok());

    Ok(())
}

#[test]
fn test_run_and_check_failure() -> Result<()> {
    // Setup
    let baseline = mock_baseline("test", "1.0", 1850.0, 1000.0, 150);
    let baseline_file = create_baseline_file(&baseline)?;

    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Test: Critical regression
    let mut metrics = mock_metrics(1800.0, 1000.0, 150);
    metrics.correctness = Some(0.80);

    // Assert: run_and_check returns Err with details
    let result = runner.run_and_check(&metrics);
    let Err(error) = result else {
        anyhow::bail!("expected run_and_check to fail");
    };
    assert!(error.contains("Critical regressions detected"));
    assert!(error.contains("correctness"));

    Ok(())
}

#[test]
fn test_multiple_metric_regressions() -> Result<()> {
    // Setup
    let baseline = mock_baseline("complex_app", "1.0.0", 1000.0, 500.0, 150);
    let baseline_file = create_baseline_file(&baseline)?;

    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Test: Multiple regressions (quality + performance + cost)
    let mut metrics = mock_metrics(1250.0, 625.0, 240);
    metrics.correctness = Some(0.88); // Drop from 0.95 to 0.88 (7% absolute)

    // Assert: Multiple regressions detected
    let regressions = runner.run(&metrics);
    assert!(regressions.len() >= 2, "Expected multiple regressions");

    // Check for quality regression (critical)
    assert!(
        has_critical_regressions(&regressions),
        "Expected critical regression"
    );

    // Check for performance/cost warnings
    let warnings: Vec<_> = regressions
        .iter()
        .filter(|r| matches!(r.severity, RegressionSeverity::Warning))
        .collect();
    assert!(!warnings.is_empty(), "Expected performance/cost warnings");

    Ok(())
}

/// Example test: Document search application evaluation
///
/// This demonstrates a realistic evaluation test for a document search app.
#[test]
fn example_librarian_eval() -> Result<()> {
    // Setup baseline (from previous release)
    let baseline = Baseline {
        app_name: "librarian".to_string(),
        version: "1.0.0".to_string(),
        date: "2025-11-10T12:00:00Z".to_string(),
        metrics: EvalMetrics {
            p95_latency: 1850.0,
            avg_latency: 1047.5,
            success_rate: 100.0,
            error_rate: 0.0,
            total_tokens: 150,
            cost_per_run: 0.00075,
            tool_calls: 2,
            correctness: Some(0.95),
            relevance: Some(0.92),
            safety: Some(1.0),
            hallucination_rate: Some(0.03),
        },
    };
    let baseline_file = create_baseline_file(&baseline)?;

    // Create test runner
    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Simulate current run metrics (after code changes)
    let current = EvalMetrics {
        p95_latency: 1620.0, // Improved! (12% faster)
        avg_latency: 980.0,  // Improved! (6% faster)
        success_rate: 100.0,
        error_rate: 0.0,
        total_tokens: 145,      // Slight improvement (3% fewer tokens)
        cost_per_run: 0.000725, // Slight improvement
        tool_calls: 2,
        correctness: Some(0.96), // Improved!
        relevance: Some(0.93),   // Improved!
        safety: Some(1.0),
        hallucination_rate: Some(0.02), // Improved!
    };

    // Run evaluation
    let regressions = runner.run(&current);

    // Assert: No regressions (all improvements!)
    assert!(
        regressions.is_empty(),
        "No regressions expected: {:?}",
        regressions
    );

    // Additional assertions using macros
    let thresholds = RegressionThresholds::default();
    assert_no_critical_regressions!(current, baseline.metrics, thresholds);

    assert_metric_within_threshold!(
        current.p95_latency,
        baseline.metrics.p95_latency,
        1.2,
        "P95 latency"
    );

    let current_correctness = current.correctness.context("expected current correctness")?;
    let baseline_correctness = baseline
        .metrics
        .correctness
        .context("expected baseline correctness")?;
    assert_quality_maintained!(
        current_correctness,
        baseline_correctness,
        0.05,
        "Correctness"
    );

    Ok(())
}

/// Example test: Application evaluation with regression
///
/// This demonstrates handling a detected regression.
#[test]
fn example_app_with_regression() -> Result<()> {
    // Setup baseline
    let baseline = mock_baseline("test_app", "2.0.0", 2200.0, 1350.0, 300);
    let baseline_file = create_baseline_file(&baseline)?;

    let mut runner = EvalTestRunner::new(baseline_file.path());

    // Simulate current run with latency regression
    let current = EvalMetrics {
        p95_latency: 2800.0, // Regressed! (27% slower)
        avg_latency: 1700.0, // Regressed! (26% slower)
        success_rate: 100.0,
        error_rate: 0.0,
        total_tokens: 300,
        cost_per_run: 0.0015,
        tool_calls: 3,
        correctness: Some(0.95),
        relevance: Some(0.90),
        safety: Some(1.0),
        hallucination_rate: Some(0.05),
    };

    // Run evaluation
    let regressions = runner.run(&current);

    // Assert: Performance regressions detected
    assert!(!regressions.is_empty(), "Expected performance regressions");

    // Check that regressions are warnings (not critical)
    assert!(
        !has_critical_regressions(&regressions),
        "Performance regressions should be warnings"
    );

    // Verify specific metrics regressed
    let has_latency_regression = regressions
        .iter()
        .any(|r| r.metric == "p95_latency" || r.metric == "avg_latency");
    assert!(has_latency_regression, "Expected latency regression");

    // run_and_check should still pass (warnings are not failures)
    let result = runner.run_and_check(&current);
    assert!(result.is_ok(), "Warnings should not fail the test");

    Ok(())
}
