// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Evaluation Framework
//!
//! Provides evaluation metrics, baseline storage, and regression detection
//! for `DashFlow` applications.
//!
//! # Overview
//!
//! This module enables automated evaluation of `DashFlow` runs by:
//! 1. Defining evaluation metrics (quality, performance, cost)
//! 2. Storing baselines for comparison
//! 3. Detecting regressions automatically
//!
//! # Architecture
//!
//! ```text
//! parse_events → analyze_events → converter → EvalMetrics
//!                                              ↓
//!                                          compare
//!                                              ↓
//!                                          Baseline
//!                                              ↓
//!                                         Regression
//! ```
//!
//! # Example
//!
//! ```no_run
//! use dashflow_streaming::evals::{Baseline, EvalMetrics, AnalyticsConverter, detect_regressions, RegressionThresholds};
//!
//! // Load analytics JSON from analyze_events
//! let analytics_json = std::fs::read_to_string("analytics.json").unwrap();
//!
//! // Convert to metrics (with default pricing)
//! let metrics = AnalyticsConverter::from_json(&analytics_json, None).unwrap();
//!
//! // Load baseline
//! let baseline = Baseline::load("baselines/librarian_v1.0.0.json").unwrap();
//!
//! // Detect regressions
//! let thresholds = RegressionThresholds::default();
//! let regressions = detect_regressions(&baseline.metrics, &metrics, &thresholds);
//!
//! if regressions.is_empty() {
//!     println!("✓ No regressions detected");
//! } else {
//!     for reg in &regressions {
//!         println!("{}", reg.format_colored());
//!     }
//! }
//! ```

pub mod baseline;
pub mod benchmark;
pub mod converter;
pub mod dataset;
pub mod metrics;
pub mod regression;
pub mod test_harness;

pub use baseline::Baseline;
pub use benchmark::{
    detect_performance_regression, format_benchmark_report, format_comparison_report,
    BenchmarkConfig, BenchmarkResult, BenchmarkRunner,
};
pub use converter::AnalyticsConverter;
pub use dataset::{
    average_correctness, score_answer, score_suite, EvalCase, EvalSuite, ScoringMethod,
};
pub use metrics::{EvalMetrics, LlmPricing};
pub use regression::{
    count_by_severity, detect_regressions, has_critical_regressions, Regression,
    RegressionSeverity, RegressionThresholds,
};
pub use test_harness::{mock_baseline, mock_metrics, EvalTestRunner};
