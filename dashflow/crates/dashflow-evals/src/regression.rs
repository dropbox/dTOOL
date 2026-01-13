//! Regression detection for comparing evaluation results against baselines.
//!
//! This module provides tools to detect quality and performance regressions by comparing
//! current evaluation results to historical baselines. It includes statistical significance
//! testing to avoid false positives from random variation.

use crate::eval_runner::{EvalReport, ScenarioResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for regression detection.
#[derive(Debug, Clone)]
pub struct RegressionConfig {
    /// Fail if overall quality drops by more than this (e.g., 0.05 = 5%)
    pub quality_drop_threshold: f64,

    /// Fail if any scenario drops by more than this (e.g., 0.10 = 10%)
    pub scenario_drop_threshold: f64,

    /// Fail if performance regresses by more than this (e.g., 0.20 = 20%)
    pub latency_increase_threshold: f64,

    /// Require statistical significance for regression detection
    pub require_statistical_significance: bool,

    /// Significance level for statistical tests (e.g., 0.05 = 95% confidence)
    pub significance_level: f64,

    /// Minimum number of scenarios needed for statistical testing
    pub min_scenarios_for_stats: usize,
}

impl Default for RegressionConfig {
    fn default() -> Self {
        Self {
            quality_drop_threshold: 0.05,     // 5%
            scenario_drop_threshold: 0.10,    // 10%
            latency_increase_threshold: 0.20, // 20%
            require_statistical_significance: true,
            significance_level: 0.05, // 95% confidence
            min_scenarios_for_stats: 10,
        }
    }
}

/// Type of regression detected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegressionType {
    /// Overall quality dropped significantly
    QualityDrop,

    /// Individual scenario quality dropped
    ScenarioQualityDrop,

    /// Performance (latency) regressed
    PerformanceRegression,

    /// Previously passing scenario now fails
    NewFailure,

    /// Pass rate decreased significantly
    PassRateDecrease,
}

/// Severity level of a regression.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// Critical issue - blocks deployment
    Critical,

    /// Warning - should be investigated
    Warning,

    /// Informational - worth noting
    Info,
}

/// A detected regression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    /// Type of regression
    pub regression_type: RegressionType,

    /// Severity level
    pub severity: Severity,

    /// Human-readable description
    pub details: String,

    /// Scenario ID (if applicable)
    pub scenario_id: Option<String>,

    /// Baseline value
    pub baseline_value: Option<f64>,

    /// Current value
    pub current_value: Option<f64>,

    /// Absolute change (current - baseline)
    pub absolute_change: Option<f64>,

    /// Relative change as percentage (e.g., -0.15 = -15%)
    pub relative_change: Option<f64>,
}

/// Report of detected regressions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionReport {
    /// List of detected regressions
    pub regressions: Vec<Regression>,

    /// Git commit hash of baseline
    pub baseline_commit: Option<String>,

    /// Git commit hash of current results
    pub current_commit: Option<String>,

    /// Whether the difference is statistically significant
    pub statistically_significant: bool,

    /// P-value from statistical test (if applicable)
    pub p_value: Option<f64>,

    /// Overall summary statistics
    pub summary: RegressionSummary,
}

/// Summary statistics for regression report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionSummary {
    /// Number of critical regressions
    pub critical_count: usize,

    /// Number of warning regressions
    pub warning_count: usize,

    /// Number of info regressions
    pub info_count: usize,

    /// Baseline average quality
    pub baseline_avg_quality: f64,

    /// Current average quality
    pub current_avg_quality: f64,

    /// Quality change (current - baseline)
    pub quality_change: f64,

    /// Baseline average latency (ms)
    pub baseline_avg_latency: u64,

    /// Current average latency (ms)
    pub current_avg_latency: u64,

    /// Latency change percentage
    pub latency_change_percent: f64,
}

impl RegressionReport {
    /// Check if there are any critical regressions.
    #[must_use]
    pub fn has_critical_regressions(&self) -> bool {
        self.summary.critical_count > 0
    }

    /// Check if there are any regressions (critical or warning).
    #[must_use]
    pub fn has_regressions(&self) -> bool {
        self.summary.critical_count > 0 || self.summary.warning_count > 0
    }

    /// Get all critical regressions.
    #[must_use]
    pub fn critical_regressions(&self) -> Vec<&Regression> {
        self.regressions
            .iter()
            .filter(|r| r.severity == Severity::Critical)
            .collect()
    }

    /// Get all warning regressions.
    #[must_use]
    pub fn warning_regressions(&self) -> Vec<&Regression> {
        self.regressions
            .iter()
            .filter(|r| r.severity == Severity::Warning)
            .collect()
    }
}

/// Regression detector for comparing evaluation results.
pub struct RegressionDetector {
    config: RegressionConfig,
}

impl RegressionDetector {
    /// Create a new regression detector with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RegressionConfig::default(),
        }
    }

    /// Create a new regression detector with custom configuration.
    #[must_use]
    pub fn with_config(config: RegressionConfig) -> Self {
        Self { config }
    }

    /// Detect regressions by comparing current results to baseline.
    ///
    /// # Arguments
    ///
    /// * `baseline` - Historical evaluation results to compare against
    /// * `current` - Current evaluation results
    ///
    /// # Returns
    ///
    /// A report containing all detected regressions with severity levels.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_evals::{RegressionDetector, EvalReport, EvalMetadata};
    /// # use chrono::Utc;
    /// # let baseline = EvalReport {
    /// #     total: 50, passed: 48, failed: 2, results: vec![],
    /// #     metadata: EvalMetadata {
    /// #         started_at: Utc::now(), completed_at: Utc::now(),
    /// #         duration_secs: 120.5, config: "{}".to_string(),
    /// #     },
    /// # };
    /// # let current = baseline.clone();
    /// let detector = RegressionDetector::new();
    /// let report = detector.detect_regressions(&baseline, &current);
    ///
    /// if report.has_critical_regressions() {
    ///     eprintln!("Critical regressions detected!");
    ///     for regression in report.critical_regressions() {
    ///         eprintln!("  - {}", regression.details);
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn detect_regressions(
        &self,
        baseline: &EvalReport,
        current: &EvalReport,
    ) -> RegressionReport {
        let mut regressions = Vec::new();

        // Calculate summary statistics
        let baseline_quality = baseline.avg_quality();
        let current_quality = current.avg_quality();
        let quality_change = current_quality - baseline_quality;

        let baseline_latency = baseline.avg_latency_ms();
        let current_latency = current.avg_latency_ms();
        let latency_change_percent = if baseline_latency > 0 {
            (current_latency as f64 - baseline_latency as f64) / baseline_latency as f64
        } else {
            0.0
        };

        // Check overall quality regression
        if quality_change < -self.config.quality_drop_threshold {
            regressions.push(Regression {
                regression_type: RegressionType::QualityDrop,
                severity: Severity::Critical,
                details: format!(
                    "Overall quality dropped from {:.3} to {:.3} ({:.1}% decrease, threshold: {:.1}%)",
                    baseline_quality,
                    current_quality,
                    quality_change * 100.0,
                    self.config.quality_drop_threshold * 100.0
                ),
                scenario_id: None,
                baseline_value: Some(baseline_quality),
                current_value: Some(current_quality),
                absolute_change: Some(quality_change),
                relative_change: Some(quality_change / baseline_quality),
            });
        }

        // Check pass rate regression
        let baseline_pass_rate = baseline.pass_rate();
        let current_pass_rate = current.pass_rate();
        let pass_rate_change = current_pass_rate - baseline_pass_rate;

        if pass_rate_change < -self.config.quality_drop_threshold {
            regressions.push(Regression {
                regression_type: RegressionType::PassRateDecrease,
                severity: Severity::Critical,
                details: format!(
                    "Pass rate dropped from {:.1}% to {:.1}% ({:.1} percentage points)",
                    baseline_pass_rate * 100.0,
                    current_pass_rate * 100.0,
                    pass_rate_change * 100.0
                ),
                scenario_id: None,
                baseline_value: Some(baseline_pass_rate),
                current_value: Some(current_pass_rate),
                absolute_change: Some(pass_rate_change),
                relative_change: Some(pass_rate_change / baseline_pass_rate),
            });
        }

        // Check performance regression
        if latency_change_percent > self.config.latency_increase_threshold {
            regressions.push(Regression {
                regression_type: RegressionType::PerformanceRegression,
                severity: Severity::Warning,
                details: format!(
                    "Average latency increased from {}ms to {}ms ({:.1}% increase, threshold: {:.1}%)",
                    baseline_latency,
                    current_latency,
                    latency_change_percent * 100.0,
                    self.config.latency_increase_threshold * 100.0
                ),
                scenario_id: None,
                baseline_value: Some(baseline_latency as f64),
                current_value: Some(current_latency as f64),
                absolute_change: Some((current_latency as i64 - baseline_latency as i64) as f64),
                relative_change: Some(latency_change_percent),
            });
        }

        // Check per-scenario regressions
        let baseline_by_id = self.build_scenario_map(&baseline.results);
        let current_by_id = self.build_scenario_map(&current.results);

        for (scenario_id, baseline_result) in &baseline_by_id {
            if let Some(current_result) = current_by_id.get(scenario_id) {
                // Check for new failures
                if baseline_result.passed && !current_result.passed {
                    regressions.push(Regression {
                        regression_type: RegressionType::NewFailure,
                        severity: Severity::Critical,
                        details: format!(
                            "Scenario '{scenario_id}' now fails (was passing in baseline)"
                        ),
                        scenario_id: Some(scenario_id.clone()),
                        baseline_value: Some(1.0),
                        current_value: Some(0.0),
                        absolute_change: Some(-1.0),
                        relative_change: Some(-1.0),
                    });
                }

                // Check for quality drops
                let quality_drop =
                    baseline_result.quality_score.overall - current_result.quality_score.overall;

                if quality_drop > self.config.scenario_drop_threshold {
                    regressions.push(Regression {
                        regression_type: RegressionType::ScenarioQualityDrop,
                        severity: if quality_drop > self.config.scenario_drop_threshold * 2.0 {
                            Severity::Critical
                        } else {
                            Severity::Warning
                        },
                        details: format!(
                            "Scenario '{}' quality dropped from {:.3} to {:.3} ({:.1}% decrease)",
                            scenario_id,
                            baseline_result.quality_score.overall,
                            current_result.quality_score.overall,
                            quality_drop * 100.0
                        ),
                        scenario_id: Some(scenario_id.clone()),
                        baseline_value: Some(baseline_result.quality_score.overall),
                        current_value: Some(current_result.quality_score.overall),
                        absolute_change: Some(-quality_drop),
                        relative_change: Some(
                            -quality_drop / baseline_result.quality_score.overall,
                        ),
                    });
                }

                // Check for latency increases
                let latency_change = if baseline_result.latency_ms > 0 {
                    (current_result.latency_ms as f64 - baseline_result.latency_ms as f64)
                        / baseline_result.latency_ms as f64
                } else {
                    0.0
                };

                if latency_change > self.config.latency_increase_threshold * 2.0 {
                    // Only report significant per-scenario latency issues
                    regressions.push(Regression {
                        regression_type: RegressionType::PerformanceRegression,
                        severity: Severity::Info,
                        details: format!(
                            "Scenario '{}' latency increased from {}ms to {}ms ({:.1}% increase)",
                            scenario_id,
                            baseline_result.latency_ms,
                            current_result.latency_ms,
                            latency_change * 100.0
                        ),
                        scenario_id: Some(scenario_id.clone()),
                        baseline_value: Some(baseline_result.latency_ms as f64),
                        current_value: Some(current_result.latency_ms as f64),
                        absolute_change: Some(
                            (current_result.latency_ms as i64 - baseline_result.latency_ms as i64)
                                as f64,
                        ),
                        relative_change: Some(latency_change),
                    });
                }
            }
        }

        // Perform statistical significance test
        let (statistically_significant, p_value) = if self.config.require_statistical_significance
            && baseline.results.len() >= self.config.min_scenarios_for_stats
            && current.results.len() >= self.config.min_scenarios_for_stats
        {
            self.test_statistical_significance(baseline, current)
        } else {
            // Not enough data for statistical testing
            (true, None)
        };

        // Count regressions by severity
        let critical_count = regressions
            .iter()
            .filter(|r| r.severity == Severity::Critical)
            .count();
        let warning_count = regressions
            .iter()
            .filter(|r| r.severity == Severity::Warning)
            .count();
        let info_count = regressions
            .iter()
            .filter(|r| r.severity == Severity::Info)
            .count();

        RegressionReport {
            regressions,
            baseline_commit: None, // To be filled in by caller if available
            current_commit: None,  // To be filled in by caller if available
            statistically_significant,
            p_value,
            summary: RegressionSummary {
                critical_count,
                warning_count,
                info_count,
                baseline_avg_quality: baseline_quality,
                current_avg_quality: current_quality,
                quality_change,
                baseline_avg_latency: baseline_latency,
                current_avg_latency: current_latency,
                latency_change_percent,
            },
        }
    }

    /// Build a map of scenario ID to result for efficient lookup.
    fn build_scenario_map(&self, results: &[ScenarioResult]) -> HashMap<String, ScenarioResult> {
        results
            .iter()
            .map(|r| (r.scenario_id.clone(), r.clone()))
            .collect()
    }

    /// Test statistical significance using paired t-test.
    ///
    /// Returns (`is_significant`, `p_value`).
    fn test_statistical_significance(
        &self,
        baseline: &EvalReport,
        current: &EvalReport,
    ) -> (bool, Option<f64>) {
        // Extract quality scores for scenarios that appear in both reports
        let baseline_map = self.build_scenario_map(&baseline.results);
        let current_map = self.build_scenario_map(&current.results);

        let mut paired_diffs = Vec::new();

        for (scenario_id, baseline_result) in &baseline_map {
            if let Some(current_result) = current_map.get(scenario_id) {
                let diff =
                    current_result.quality_score.overall - baseline_result.quality_score.overall;
                paired_diffs.push(diff);
            }
        }

        if paired_diffs.len() < self.config.min_scenarios_for_stats {
            return (true, None); // Not enough paired data
        }

        // Perform paired t-test
        let (_t_statistic, p_value) = self.paired_t_test(&paired_diffs);

        // Significant if p-value is below threshold
        let is_significant = p_value < self.config.significance_level;

        (is_significant, Some(p_value))
    }

    /// Perform paired t-test on differences.
    ///
    /// Returns (t-statistic, two-tailed p-value).
    fn paired_t_test(&self, diffs: &[f64]) -> (f64, f64) {
        let n = diffs.len() as f64;

        if n < 2.0 {
            return (0.0, 1.0); // Cannot compute t-test
        }

        // Mean of differences
        let mean: f64 = diffs.iter().sum::<f64>() / n;

        // Standard deviation of differences
        let variance = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            // No variance - all differences are identical
            return (0.0, 1.0);
        }

        // T-statistic
        let t_statistic = mean / (std_dev / n.sqrt());

        // Degrees of freedom
        let df = n - 1.0;

        // Approximate two-tailed p-value using t-distribution
        // For simplicity, using a rough approximation
        // For production use, consider using a statistics library like `statrs`
        let p_value = self.t_distribution_p_value(t_statistic.abs(), df);

        (t_statistic, p_value * 2.0) // Two-tailed
    }

    /// Approximate p-value for t-distribution.
    ///
    /// This is a simplified approximation. For more accurate results,
    /// use a proper statistics library like `statrs`.
    fn t_distribution_p_value(&self, t: f64, df: f64) -> f64 {
        // For large df (>30), t-distribution approximates standard normal
        if df > 30.0 {
            // Use normal approximation
            return self.normal_cdf(-t);
        }

        // For small df, use rough approximation
        // This is not precise but gives reasonable results
        let x = df / (df + t * t);

        0.5 * self.beta_incomplete(df / 2.0, 0.5, x)
    }

    /// Approximate cumulative distribution function for standard normal.
    fn normal_cdf(&self, x: f64) -> f64 {
        // Approximation using error function
        0.5 * (1.0 + self.erf(x / 2.0_f64.sqrt()))
    }

    /// Approximate error function (erf).
    fn erf(&self, x: f64) -> f64 {
        // Abramowitz and Stegun approximation
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();

        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

        sign * y
    }

    /// Approximate incomplete beta function.
    ///
    /// This is a very rough approximation for illustration.
    /// For production use, use a proper statistics library.
    fn beta_incomplete(&self, _a: f64, _b: f64, x: f64) -> f64 {
        // Simplified - just return x as rough approximation
        x
    }
}

impl Default for RegressionDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_runner::{EvalMetadata, ValidationResult};
    use crate::quality_judge::QualityScore;
    use chrono::Utc;

    fn create_test_result(
        scenario_id: &str,
        passed: bool,
        quality: f64,
        latency_ms: u64,
    ) -> ScenarioResult {
        ScenarioResult {
            scenario_id: scenario_id.to_string(),
            passed,
            output: "test output".to_string(),
            quality_score: QualityScore {
                accuracy: quality,
                relevance: quality,
                completeness: quality,
                safety: quality,
                coherence: quality,
                conciseness: quality,
                overall: quality,
                reasoning: "test".to_string(),
                issues: vec![],
                suggestions: vec![],
            },
            latency_ms,
            validation: ValidationResult {
                passed,
                missing_contains: vec![],
                forbidden_found: vec![],
                failure_reason: None,
            },
            error: None,
            retry_attempts: 0,
            timestamp: Utc::now(),
            input: None,
            tokens_used: None,
            cost_usd: None,
        }
    }

    fn create_test_report(results: Vec<ScenarioResult>) -> EvalReport {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;

        EvalReport {
            total,
            passed,
            failed,
            results,
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 120.0,
                config: "{}".to_string(),
            },
        }
    }

    #[test]
    fn test_no_regression() {
        let detector = RegressionDetector::new();

        let baseline = create_test_report(vec![
            create_test_result("s1", true, 0.95, 100),
            create_test_result("s2", true, 0.90, 150),
        ]);

        let current = create_test_report(vec![
            create_test_result("s1", true, 0.94, 105),
            create_test_result("s2", true, 0.91, 145),
        ]);

        let report = detector.detect_regressions(&baseline, &current);
        assert_eq!(report.summary.critical_count, 0);
        assert_eq!(report.summary.warning_count, 0);
    }

    #[test]
    fn test_overall_quality_drop() {
        let detector = RegressionDetector::new();

        let baseline = create_test_report(vec![
            create_test_result("s1", true, 0.95, 100),
            create_test_result("s2", true, 0.95, 100),
        ]);

        let current = create_test_report(vec![
            create_test_result("s1", true, 0.80, 100), // Significant drop
            create_test_result("s2", true, 0.80, 100), // Significant drop
        ]);

        let report = detector.detect_regressions(&baseline, &current);
        assert!(report.summary.critical_count > 0);

        let quality_regressions: Vec<_> = report
            .regressions
            .iter()
            .filter(|r| r.regression_type == RegressionType::QualityDrop)
            .collect();
        assert!(!quality_regressions.is_empty());
    }

    #[test]
    fn test_new_failure_detected() {
        let detector = RegressionDetector::new();

        let baseline = create_test_report(vec![create_test_result("s1", true, 0.95, 100)]);

        let current = create_test_report(vec![
            create_test_result("s1", false, 0.50, 100), // Now failing
        ]);

        let report = detector.detect_regressions(&baseline, &current);
        assert!(report.summary.critical_count > 0);

        let new_failures: Vec<_> = report
            .regressions
            .iter()
            .filter(|r| r.regression_type == RegressionType::NewFailure)
            .collect();
        assert_eq!(new_failures.len(), 1);
    }

    #[test]
    fn test_performance_regression() {
        let config = RegressionConfig {
            latency_increase_threshold: 0.20, // 20%
            ..Default::default()
        };

        let detector = RegressionDetector::with_config(config);

        let baseline = create_test_report(vec![
            create_test_result("s1", true, 0.95, 100),
            create_test_result("s2", true, 0.95, 100),
        ]);

        let current = create_test_report(vec![
            create_test_result("s1", true, 0.95, 130), // 30% slower
            create_test_result("s2", true, 0.95, 130), // 30% slower
        ]);

        let report = detector.detect_regressions(&baseline, &current);

        let perf_regressions: Vec<_> = report
            .regressions
            .iter()
            .filter(|r| r.regression_type == RegressionType::PerformanceRegression)
            .collect();
        assert!(!perf_regressions.is_empty());
    }

    #[test]
    fn test_scenario_quality_drop() {
        let detector = RegressionDetector::new();

        let baseline = create_test_report(vec![
            create_test_result("s1", true, 0.95, 100),
            create_test_result("s2", true, 0.95, 100),
        ]);

        let current = create_test_report(vec![
            create_test_result("s1", true, 0.95, 100), // No change
            create_test_result("s2", true, 0.70, 100), // Significant drop (>10%)
        ]);

        let report = detector.detect_regressions(&baseline, &current);

        let scenario_drops: Vec<_> = report
            .regressions
            .iter()
            .filter(|r| r.regression_type == RegressionType::ScenarioQualityDrop)
            .collect();
        assert!(!scenario_drops.is_empty());
    }

    #[test]
    fn test_pass_rate_decrease() {
        let detector = RegressionDetector::new();

        let baseline = create_test_report(vec![
            create_test_result("s1", true, 0.95, 100),
            create_test_result("s2", true, 0.95, 100),
            create_test_result("s3", true, 0.95, 100),
            create_test_result("s4", true, 0.95, 100),
        ]);

        let current = create_test_report(vec![
            create_test_result("s1", false, 0.70, 100), // Failed
            create_test_result("s2", false, 0.70, 100), // Failed
            create_test_result("s3", true, 0.95, 100),
            create_test_result("s4", true, 0.95, 100),
        ]);

        let report = detector.detect_regressions(&baseline, &current);

        let pass_rate_drops: Vec<_> = report
            .regressions
            .iter()
            .filter(|r| r.regression_type == RegressionType::PassRateDecrease)
            .collect();
        assert!(!pass_rate_drops.is_empty());
    }

    #[test]
    fn test_regression_report_methods() {
        let detector = RegressionDetector::new();

        let baseline = create_test_report(vec![create_test_result("s1", true, 0.95, 100)]);

        let current = create_test_report(vec![create_test_result("s1", false, 0.50, 100)]);

        let report = detector.detect_regressions(&baseline, &current);

        assert!(report.has_critical_regressions());
        assert!(report.has_regressions());
        assert!(!report.critical_regressions().is_empty());
    }

    #[test]
    fn test_statistical_significance() {
        let config = RegressionConfig {
            require_statistical_significance: true,
            min_scenarios_for_stats: 3,
            ..Default::default()
        };

        let detector = RegressionDetector::with_config(config);

        // Create baseline with consistent quality
        let baseline = create_test_report(vec![
            create_test_result("s1", true, 0.95, 100),
            create_test_result("s2", true, 0.94, 100),
            create_test_result("s3", true, 0.96, 100),
        ]);

        // Current with slightly lower quality
        let current = create_test_report(vec![
            create_test_result("s1", true, 0.93, 100),
            create_test_result("s2", true, 0.92, 100),
            create_test_result("s3", true, 0.94, 100),
        ]);

        let report = detector.detect_regressions(&baseline, &current);

        // Statistical test should run
        assert!(report.p_value.is_some());
    }
}
