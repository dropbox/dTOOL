// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Test harness integration for evals framework
//!
//! This module provides helpers, macros, and utilities for integrating
//! the evals framework with Rust's test harness (`cargo test`).
//!
//! # Features
//!
//! - **Test helpers**: Functions to run evals as part of tests
//! - **Assertion macros**: Custom assertions for regression detection
//! - **Fixtures**: Mock data and baseline management for tests
//!
//! # Example
//!
//! ```no_run
//! use dashflow_streaming::evals::test_harness::EvalTestRunner;
//! use dashflow_streaming::assert_no_critical_regressions;
//! use dashflow_streaming::evals::{EvalMetrics, RegressionThresholds};
//!
//! #[tokio::test]
//! async fn test_librarian_quality() {
//!     let mut runner = EvalTestRunner::new("baselines/librarian_v1.json");
//!
//!     // Run eval with mock data
//!     let metrics = EvalMetrics {
//!         p95_latency: 1800.0,
//!         avg_latency: 1000.0,
//!         success_rate: 100.0,
//!         error_rate: 0.0,
//!         total_tokens: 150,
//!         cost_per_run: 0.00075,
//!         tool_calls: 2,
//!         correctness: Some(0.95),
//!         relevance: Some(0.90),
//!         safety: Some(1.0),
//!         hallucination_rate: Some(0.05),
//!     };
//!
//!     // Assert no critical regressions
//!     let baseline = runner.baseline();
//!     assert_no_critical_regressions!(metrics, baseline.metrics, RegressionThresholds::default());
//! }
//! ```

use crate::evals::{
    detect_regressions, has_critical_regressions, Baseline, EvalMetrics, Regression,
    RegressionSeverity, RegressionThresholds,
};
use std::path::{Path, PathBuf};

/// Test runner for evaluation tests
///
/// Provides utilities for loading baselines, running evals, and asserting
/// on regression detection within test contexts.
#[derive(Clone)]
pub struct EvalTestRunner {
    baseline_path: PathBuf,
    baseline: Option<Baseline>,
    thresholds: RegressionThresholds,
}

impl EvalTestRunner {
    /// Create a new test runner with a baseline path
    ///
    /// # Arguments
    ///
    /// * `baseline_path` - Path to baseline JSON file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_streaming::evals::test_harness::EvalTestRunner;
    ///
    /// let runner = EvalTestRunner::new("baselines/app_v1.json");
    /// ```
    pub fn new(baseline_path: impl AsRef<Path>) -> Self {
        Self {
            baseline_path: baseline_path.as_ref().to_path_buf(),
            baseline: None,
            thresholds: RegressionThresholds::default(),
        }
    }

    /// Create a test runner with custom thresholds
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_streaming::evals::test_harness::EvalTestRunner;
    /// use dashflow_streaming::evals::RegressionThresholds;
    ///
    /// let runner = EvalTestRunner::with_thresholds(
    ///     "baselines/app_v1.json",
    ///     RegressionThresholds::strict()
    /// );
    /// ```
    pub fn with_thresholds(
        baseline_path: impl AsRef<Path>,
        thresholds: RegressionThresholds,
    ) -> Self {
        Self {
            baseline_path: baseline_path.as_ref().to_path_buf(),
            baseline: None,
            thresholds,
        }
    }

    /// Load the baseline from disk
    ///
    /// Returns an error if the baseline file doesn't exist or can't be parsed.
    #[allow(clippy::unwrap_used)] // Unwrap is safe: baseline is Some after the if-block (either set here or was already Some)
    pub fn load_baseline(&mut self) -> Result<&Baseline, anyhow::Error> {
        if self.baseline.is_none() {
            self.baseline = Some(Baseline::load(&self.baseline_path)?);
        }
        Ok(self.baseline.as_ref().unwrap())
    }

    /// Get the loaded baseline (loads if not already loaded)
    #[allow(clippy::expect_used)] // Convenience API: panics on I/O error
    pub fn baseline(&mut self) -> &Baseline {
        self.load_baseline().expect("Failed to load baseline")
    }

    /// Run evaluation with provided metrics
    ///
    /// # Arguments
    ///
    /// * `metrics` - Current evaluation metrics
    ///
    /// # Returns
    ///
    /// Vector of detected regressions
    #[allow(clippy::expect_used)] // Baseline loading errors are fatal for evaluation
    pub fn run(&mut self, metrics: &EvalMetrics) -> Vec<Regression> {
        // Clone thresholds first to avoid borrowing issues
        let thresholds = self.thresholds.clone();
        // Load baseline (borrows self mutably)
        let baseline = self.load_baseline().expect("Failed to load baseline");
        // Now we can use baseline and thresholds
        detect_regressions(&baseline.metrics, metrics, &thresholds)
    }

    /// Run evaluation and check for critical regressions
    ///
    /// # Returns
    ///
    /// - `Ok(())` if no critical regressions detected
    /// - `Err(message)` if critical regressions found
    pub fn run_and_check(&mut self, metrics: &EvalMetrics) -> Result<(), String> {
        let regressions = self.run(metrics);

        if has_critical_regressions(&regressions) {
            let critical: Vec<_> = regressions
                .iter()
                .filter(|r| matches!(r.severity, RegressionSeverity::Critical))
                .collect();

            let mut error = String::from("Critical regressions detected:\n");
            for reg in critical {
                error.push_str(&format!("  - {}\n", reg.format_plain()));
            }
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Run evaluation with mock metrics (for testing)
    ///
    /// This is a convenience method that doesn't actually run an application,
    /// but allows testing the regression detection logic with mock metrics.
    pub fn run_with_mock_metrics(&mut self, metrics: &EvalMetrics) -> Vec<Regression> {
        self.run(metrics)
    }
}

/// Assert that no critical regressions are detected
///
/// Note: This macro is exported at the crate root due to `#[macro_export]`.
///
/// # Example
///
/// ```no_run
/// use dashflow_streaming::assert_no_critical_regressions;
/// use dashflow_streaming::evals::{EvalMetrics, Baseline, RegressionThresholds};
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let metrics = EvalMetrics {
///     p95_latency: 1800.0,
///     avg_latency: 1000.0,
///     success_rate: 100.0,
///     error_rate: 0.0,
///     total_tokens: 150,
///     cost_per_run: 0.00075,
///     tool_calls: 2,
///     correctness: Some(0.95),
///     relevance: Some(0.90),
///     safety: Some(1.0),
///     hallucination_rate: Some(0.05),
/// };
/// let baseline = Baseline::load("baselines/test.json")?;
/// let thresholds = RegressionThresholds::default();
///
/// assert_no_critical_regressions!(metrics, baseline.metrics, thresholds);
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! assert_no_critical_regressions {
    ($current:expr, $baseline:expr, $thresholds:expr) => {{
        let regressions = $crate::evals::detect_regressions(&$baseline, &$current, &$thresholds);
        let critical: Vec<_> = regressions
            .iter()
            .filter(|r| matches!(r.severity, $crate::evals::RegressionSeverity::Critical))
            .collect();

        if !critical.is_empty() {
            let mut msg = String::from("Critical regressions detected:\n");
            for reg in critical {
                msg.push_str(&format!("  - {}\n", reg.format_plain()));
            }
            panic!("{}", msg);
        }
    }};
}

/// Assert that a specific metric is within threshold
///
/// Note: This macro is exported at the crate root due to `#[macro_export]`.
///
/// # Example
///
/// ```no_run
/// use dashflow_streaming::assert_metric_within_threshold;
/// use dashflow_streaming::evals::EvalMetrics;
///
/// let metrics = EvalMetrics {
///     p95_latency: 1800.0,
///     avg_latency: 1000.0,
///     success_rate: 100.0,
///     error_rate: 0.0,
///     total_tokens: 150,
///     cost_per_run: 0.00075,
///     tool_calls: 2,
///     correctness: Some(0.95),
///     relevance: Some(0.90),
///     safety: Some(1.0),
///     hallucination_rate: Some(0.05),
/// };
/// let baseline_latency = 1850.0;
/// let threshold = 1.2; // 20% slower
///
/// assert_metric_within_threshold!(
///     metrics.p95_latency,
///     baseline_latency,
///     threshold,
///     "P95 latency"
/// );
/// ```
#[macro_export]
macro_rules! assert_metric_within_threshold {
    ($current:expr, $baseline:expr, $threshold:expr, $metric_name:expr) => {{
        let current = $current;
        let baseline = $baseline;
        let threshold = $threshold;
        let limit = baseline * threshold;

        assert!(
            current <= limit,
            "{} regressed: {} > {} (baseline {} × threshold {})",
            $metric_name,
            current,
            limit,
            baseline,
            threshold
        );
    }};
}

/// Assert that a quality metric (0.0-1.0) hasn't degraded
///
/// Note: This macro is exported at the crate root due to `#[macro_export]`.
///
/// # Example
///
/// ```no_run
/// use dashflow_streaming::assert_quality_maintained;
///
/// let current_correctness = 0.92;
/// let baseline_correctness = 0.95;
/// let threshold = 0.05; // Allow 5% absolute drop
///
/// assert_quality_maintained!(
///     current_correctness,
///     baseline_correctness,
///     threshold,
///     "Correctness"
/// );
/// ```
#[macro_export]
macro_rules! assert_quality_maintained {
    ($current:expr, $baseline:expr, $threshold:expr, $metric_name:expr) => {{
        let current = $current;
        let baseline = $baseline;
        let threshold = $threshold;
        let limit = baseline - threshold;

        assert!(
            current >= limit,
            "{} degraded: {} < {} (baseline {} - threshold {})",
            $metric_name,
            current,
            limit,
            baseline,
            threshold
        );
    }};
}

/// Create a mock `EvalMetrics` for testing
///
/// # Example
///
/// ```
/// use dashflow_streaming::evals::test_harness::mock_metrics;
///
/// let metrics = mock_metrics(
///     1800.0,  // p95_latency
///     1000.0,  // avg_latency
///     150      // total_tokens
/// );
/// ```
#[must_use]
pub fn mock_metrics(p95_latency: f64, avg_latency: f64, total_tokens: u64) -> EvalMetrics {
    EvalMetrics {
        p95_latency,
        avg_latency,
        success_rate: 100.0,
        error_rate: 0.0,
        total_tokens,
        cost_per_run: 0.00075,
        tool_calls: 2,
        correctness: Some(0.95),
        relevance: Some(0.90),
        safety: Some(1.0),
        hallucination_rate: Some(0.05),
    }
}

/// Create a mock baseline for testing
///
/// # Example
///
/// ```
/// use dashflow_streaming::evals::test_harness::mock_baseline;
///
/// let baseline = mock_baseline(
///     "test_app",
///     "1.0.0",
///     1850.0,  // p95_latency
///     1000.0,  // avg_latency
///     150      // total_tokens
/// );
/// ```
#[must_use]
pub fn mock_baseline(
    app_name: &str,
    version: &str,
    p95_latency: f64,
    avg_latency: f64,
    total_tokens: u64,
) -> Baseline {
    Baseline {
        app_name: app_name.to_string(),
        version: version.to_string(),
        date: chrono::Utc::now().to_rfc3339(),
        metrics: mock_metrics(p95_latency, avg_latency, total_tokens),
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_eval_test_runner_mock() {
        // Create a temporary baseline file
        let mut baseline_file = NamedTempFile::new().unwrap();
        let baseline = mock_baseline("test_app", "1.0.0", 1850.0, 1000.0, 150);
        let baseline_json = serde_json::to_string(&baseline).unwrap();
        baseline_file.write_all(baseline_json.as_bytes()).unwrap();
        baseline_file.flush().unwrap();

        // Create test runner
        let mut runner = EvalTestRunner::new(baseline_file.path());

        // Run with mock metrics (no regression)
        let metrics = mock_metrics(1800.0, 1000.0, 150);
        let regressions = runner.run_with_mock_metrics(&metrics);
        assert!(regressions.is_empty());
    }

    #[test]
    fn test_eval_test_runner_detects_regression() {
        // Create a temporary baseline file
        let mut baseline_file = NamedTempFile::new().unwrap();
        let baseline = mock_baseline("test_app", "1.0.0", 1850.0, 1000.0, 150);
        let baseline_json = serde_json::to_string(&baseline).unwrap();
        baseline_file.write_all(baseline_json.as_bytes()).unwrap();
        baseline_file.flush().unwrap();

        // Create test runner
        let mut runner = EvalTestRunner::new(baseline_file.path());

        // Run with mock metrics (quality degradation)
        let mut metrics = mock_metrics(1800.0, 1000.0, 150);
        metrics.correctness = Some(0.80); // Drop from 0.95 to 0.80 (15% absolute)
        let regressions = runner.run_with_mock_metrics(&metrics);

        assert!(!regressions.is_empty());
        assert!(has_critical_regressions(&regressions));
    }

    #[test]
    fn test_mock_metrics() {
        let metrics = mock_metrics(1000.0, 500.0, 100);
        assert_eq!(metrics.p95_latency, 1000.0);
        assert_eq!(metrics.avg_latency, 500.0);
        assert_eq!(metrics.total_tokens, 100);
    }

    #[test]
    fn test_mock_baseline() {
        let baseline = mock_baseline("test", "1.0", 1000.0, 500.0, 100);
        assert_eq!(baseline.app_name, "test");
        assert_eq!(baseline.version, "1.0");
        assert_eq!(baseline.metrics.p95_latency, 1000.0);
    }

    #[test]
    fn test_assert_no_critical_regressions_pass() {
        let baseline = mock_metrics(1850.0, 1000.0, 150);
        let current = mock_metrics(1800.0, 1000.0, 150);
        let thresholds = RegressionThresholds::default();

        // Should not panic
        assert_no_critical_regressions!(current, baseline, thresholds);
    }

    // NOTE: The following tests use #[should_panic] INTENTIONALLY.
    // These test assertion macros (assert_no_critical_regressions!, assert_metric_within_threshold!,
    // assert_quality_maintained!) which are designed to panic when validation fails.
    // Testing that assertions panic is the correct behavior here - these are regression guards.

    #[test]
    #[should_panic(expected = "Critical regressions detected")]
    fn test_assert_no_critical_regressions_fail() {
        let baseline = mock_metrics(1850.0, 1000.0, 150);
        let mut current = mock_metrics(1800.0, 1000.0, 150);
        current.correctness = Some(0.80); // Drop from 0.95 to 0.80
        let thresholds = RegressionThresholds::default();

        // Should panic
        assert_no_critical_regressions!(current, baseline, thresholds);
    }

    #[test]
    fn test_assert_metric_within_threshold_pass() {
        let current = 1800.0;
        let baseline = 1850.0;
        let threshold = 1.2;

        // Should not panic
        assert_metric_within_threshold!(current, baseline, threshold, "P95 latency");
    }

    #[test]
    #[should_panic(expected = "P95 latency regressed")]
    fn test_assert_metric_within_threshold_fail() {
        let current = 2300.0;
        let baseline = 1850.0;
        let threshold = 1.2;

        // Should panic
        assert_metric_within_threshold!(current, baseline, threshold, "P95 latency");
    }

    #[test]
    fn test_assert_quality_maintained_pass() {
        let current = 0.94;
        let baseline = 0.95;
        let threshold = 0.05;

        // Should not panic (0.94 >= 0.95 - 0.05 = 0.90)
        assert_quality_maintained!(current, baseline, threshold, "Correctness");
    }

    #[test]
    #[should_panic(expected = "Correctness degraded")]
    fn test_assert_quality_maintained_fail() {
        let current = 0.85;
        let baseline = 0.95;
        let threshold = 0.05;

        // Should panic (0.85 < 0.95 - 0.05 = 0.90)
        assert_quality_maintained!(current, baseline, threshold, "Correctness");
    }
}
