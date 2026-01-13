//! Quality Gate Implementation
//!
//! Quality gates enforce minimum quality thresholds in CI/CD pipelines.
//! They can block PRs or deployments if evaluation results don't meet standards.
//!
//! # Example
//!
//! ```rust
//! use dashflow_evals::ci::{QualityGate, QualityGateConfig};
//! use dashflow_evals::eval_runner::EvalReport;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Configure quality gate
//! let gate_config = QualityGateConfig::default()
//!     .with_min_pass_rate(0.95)
//!     .with_min_quality(0.90)
//!     .with_max_latency_increase(0.20)
//!     .with_block_on_new_failures(true);
//!
//! let gate = QualityGate::new(gate_config);
//!
//! // Check if results meet quality gate
//! # let current_report = todo!();
//! # let baseline_report = None;
//! let result = gate.check(&current_report, baseline_report.as_ref())?;
//!
//! if result.passed {
//!     println!("✅ Quality gate passed!");
//! } else {
//!     println!("❌ Quality gate failed:");
//!     for violation in &result.violations {
//!         println!("  - {}", violation.description);
//!     }
//!     std::process::exit(1);
//! }
//! # Ok(())
//! # }
//! ```

use crate::eval_runner::EvalReport;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Quality gate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateConfig {
    /// Minimum pass rate (0-1). Default: 0.95 (95%)
    pub min_pass_rate: f64,

    /// Minimum quality score (0-1). Default: 0.90
    pub min_quality: f64,

    /// Maximum latency increase vs baseline (0-1). Default: 0.20 (20%)
    /// Set to None to disable latency checks.
    pub max_latency_increase: Option<f64>,

    /// Maximum cost increase vs baseline (0-1). Default: 0.15 (15%)
    /// Set to None to disable cost checks.
    pub max_cost_increase: Option<f64>,

    /// Block on new failures (scenarios that passed in baseline but fail now)?
    pub block_on_new_failures: bool,

    /// Block on quality degradation (any scenario with significant quality drop)?
    pub block_on_quality_degradation: bool,

    /// Quality degradation threshold (0-1). Default: 0.10 (10%)
    pub quality_degradation_threshold: f64,
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            min_pass_rate: 0.95,
            min_quality: 0.90,
            max_latency_increase: Some(0.20),
            max_cost_increase: Some(0.15),
            block_on_new_failures: true,
            block_on_quality_degradation: true,
            quality_degradation_threshold: 0.10,
        }
    }
}

impl QualityGateConfig {
    /// Set minimum pass rate
    #[must_use]
    pub fn with_min_pass_rate(mut self, rate: f64) -> Self {
        self.min_pass_rate = rate;
        self
    }

    /// Set minimum quality score
    #[must_use]
    pub fn with_min_quality(mut self, quality: f64) -> Self {
        self.min_quality = quality;
        self
    }

    /// Set maximum latency increase vs baseline
    #[must_use]
    pub fn with_max_latency_increase(mut self, increase: f64) -> Self {
        self.max_latency_increase = Some(increase);
        self
    }

    /// Disable latency checks
    #[must_use]
    pub fn without_latency_checks(mut self) -> Self {
        self.max_latency_increase = None;
        self
    }

    /// Set maximum cost increase vs baseline
    #[must_use]
    pub fn with_max_cost_increase(mut self, increase: f64) -> Self {
        self.max_cost_increase = Some(increase);
        self
    }

    /// Disable cost checks
    #[must_use]
    pub fn without_cost_checks(mut self) -> Self {
        self.max_cost_increase = None;
        self
    }

    /// Block on new failures
    #[must_use]
    pub fn with_block_on_new_failures(mut self, block: bool) -> Self {
        self.block_on_new_failures = block;
        self
    }

    /// Block on quality degradation
    #[must_use]
    pub fn with_block_on_quality_degradation(mut self, block: bool) -> Self {
        self.block_on_quality_degradation = block;
        self
    }

    /// Set quality degradation threshold
    #[must_use]
    pub fn with_quality_degradation_threshold(mut self, threshold: f64) -> Self {
        self.quality_degradation_threshold = threshold;
        self
    }
}

/// Result of quality gate check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Did the evaluation pass the quality gate?
    pub passed: bool,

    /// Violations found (if any)
    pub violations: Vec<GateViolation>,

    /// Summary message
    pub summary: String,

    /// Exit code for CI (0 = success, 1 = failure)
    pub exit_code: i32,
}

/// A quality gate violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateViolation {
    /// Severity level
    pub severity: ViolationSeverity,

    /// Type of violation
    pub violation_type: ViolationType,

    /// Human-readable description
    pub description: String,

    /// Measured value
    pub measured_value: String,

    /// Expected threshold
    pub threshold: String,

    /// Suggested action to fix
    pub suggested_action: Option<String>,
}

/// Severity of a violation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationSeverity {
    /// Critical - blocks PR/deployment
    Critical,
    /// Warning - informational only
    Warning,
}

/// Type of quality gate violation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationType {
    /// Pass rate below threshold
    PassRateTooLow,
    /// Quality score below threshold
    QualityTooLow,
    /// Latency increased too much
    LatencyRegression,
    /// Cost increased too much
    CostRegression,
    /// New scenario failures
    NewFailures,
    /// Significant quality drop in specific scenarios
    QualityDegradation,
}

/// Quality gate checker
#[derive(Debug, Clone)]
pub struct QualityGate {
    config: QualityGateConfig,
}

impl QualityGate {
    /// Create a new quality gate with the given configuration
    #[must_use]
    pub fn new(config: QualityGateConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    #[must_use]
    pub fn default_gate() -> Self {
        Self::new(QualityGateConfig::default())
    }

    /// Check if evaluation results meet quality gate criteria
    ///
    /// # Arguments
    ///
    /// * `current` - Current evaluation results
    /// * `baseline` - Optional baseline results for comparison
    ///
    /// # Returns
    ///
    /// `GateResult` indicating pass/fail and any violations found
    pub fn check(&self, current: &EvalReport, baseline: Option<&EvalReport>) -> Result<GateResult> {
        let mut violations = Vec::new();

        // Check 1: Pass rate
        let pass_rate = current.pass_rate();
        if pass_rate < self.config.min_pass_rate {
            violations.push(GateViolation {
                severity: ViolationSeverity::Critical,
                violation_type: ViolationType::PassRateTooLow,
                description: format!(
                    "Pass rate below threshold: {:.1}% < {:.1}%",
                    pass_rate * 100.0,
                    self.config.min_pass_rate * 100.0
                ),
                measured_value: format!("{:.1}%", pass_rate * 100.0),
                threshold: format!("≥{:.1}%", self.config.min_pass_rate * 100.0),
                suggested_action: Some(
                    "Review failed scenarios and fix underlying issues".to_string(),
                ),
            });
        }

        // Check 2: Quality score
        let avg_quality = current.avg_quality();
        if avg_quality < self.config.min_quality {
            violations.push(GateViolation {
                severity: ViolationSeverity::Critical,
                violation_type: ViolationType::QualityTooLow,
                description: format!(
                    "Average quality below threshold: {:.3} < {:.3}",
                    avg_quality, self.config.min_quality
                ),
                measured_value: format!("{avg_quality:.3}"),
                threshold: format!("≥{:.3}", self.config.min_quality),
                suggested_action: Some(
                    "Improve response quality by refining prompts or using better models"
                        .to_string(),
                ),
            });
        }

        // Compare to baseline if provided
        if let Some(baseline) = baseline {
            self.check_baseline_regressions(current, baseline, &mut violations)?;
        }

        // Generate summary
        let passed = violations.is_empty()
            || violations
                .iter()
                .all(|v| v.severity == ViolationSeverity::Warning);

        let summary = if passed {
            format!(
                "✅ Quality gate passed: {:.1}% pass rate, {:.3} quality",
                pass_rate * 100.0,
                avg_quality
            )
        } else {
            format!(
                "❌ Quality gate failed: {} critical violation(s)",
                violations
                    .iter()
                    .filter(|v| v.severity == ViolationSeverity::Critical)
                    .count()
            )
        };

        let exit_code = i32::from(!passed);

        Ok(GateResult {
            passed,
            violations,
            summary,
            exit_code,
        })
    }

    /// Check for regressions compared to baseline
    fn check_baseline_regressions(
        &self,
        current: &EvalReport,
        baseline: &EvalReport,
        violations: &mut Vec<GateViolation>,
    ) -> Result<()> {
        // Check 3: Latency regression
        if let Some(max_increase) = self.config.max_latency_increase {
            let current_latency = current.avg_latency_ms();
            let baseline_latency = baseline.avg_latency_ms();

            if baseline_latency > 0 {
                let increase_ratio =
                    (current_latency as f64 - baseline_latency as f64) / baseline_latency as f64;

                if increase_ratio > max_increase {
                    violations.push(GateViolation {
                        severity: ViolationSeverity::Critical,
                        violation_type: ViolationType::LatencyRegression,
                        description: format!(
                            "Latency increased by {:.1}% (from {}ms to {}ms)",
                            increase_ratio * 100.0,
                            baseline_latency,
                            current_latency
                        ),
                        measured_value: format!("{current_latency}ms"),
                        threshold: format!(
                            "≤{}ms",
                            (baseline_latency as f64 * (1.0 + max_increase)) as u64
                        ),
                        suggested_action: Some(
                            "Profile the application to identify performance bottlenecks"
                                .to_string(),
                        ),
                    });
                }
            }
        }

        // Check 4: New failures
        if self.config.block_on_new_failures {
            let new_failures = self.find_new_failures(current, baseline);
            if !new_failures.is_empty() {
                violations.push(GateViolation {
                    severity: ViolationSeverity::Critical,
                    violation_type: ViolationType::NewFailures,
                    description: format!(
                        "{} scenario(s) that passed in baseline now fail: {}",
                        new_failures.len(),
                        new_failures.join(", ")
                    ),
                    measured_value: new_failures.len().to_string(),
                    threshold: "0".to_string(),
                    suggested_action: Some(
                        "Review the failing scenarios and revert changes that broke them"
                            .to_string(),
                    ),
                });
            }
        }

        // Check 5: Quality degradation
        if self.config.block_on_quality_degradation {
            let degraded = self.find_quality_degradation(current, baseline);
            if !degraded.is_empty() {
                for (scenario_id, current_q, baseline_q) in &degraded {
                    violations.push(GateViolation {
                        severity: ViolationSeverity::Critical,
                        violation_type: ViolationType::QualityDegradation,
                        description: format!(
                            "Scenario '{}' quality dropped from {:.3} to {:.3} ({:.1}%)",
                            scenario_id,
                            baseline_q,
                            current_q,
                            ((baseline_q - current_q) / baseline_q) * 100.0
                        ),
                        measured_value: format!("{current_q:.3}"),
                        threshold: format!(
                            "≥{:.3}",
                            baseline_q * (1.0 - self.config.quality_degradation_threshold)
                        ),
                        suggested_action: Some(format!(
                            "Review scenario '{scenario_id}' for quality issues"
                        )),
                    });
                }
            }
        }

        Ok(())
    }

    /// Find scenarios that passed in baseline but fail now
    fn find_new_failures(&self, current: &EvalReport, baseline: &EvalReport) -> Vec<String> {
        let mut new_failures = Vec::new();

        for result in &current.results {
            if !result.passed {
                // Check if this scenario passed in baseline
                if let Some(baseline_result) = baseline
                    .results
                    .iter()
                    .find(|r| r.scenario_id == result.scenario_id)
                {
                    if baseline_result.passed {
                        new_failures.push(result.scenario_id.clone());
                    }
                }
            }
        }

        new_failures
    }

    /// Find scenarios with significant quality degradation
    fn find_quality_degradation(
        &self,
        current: &EvalReport,
        baseline: &EvalReport,
    ) -> Vec<(String, f64, f64)> {
        let mut degraded = Vec::new();

        for result in &current.results {
            if let Some(baseline_result) = baseline
                .results
                .iter()
                .find(|r| r.scenario_id == result.scenario_id)
            {
                let current_quality = result.quality_score.overall;
                let baseline_quality = baseline_result.quality_score.overall;

                // Check for significant drop
                if baseline_quality > 0.0 {
                    let drop_ratio = (baseline_quality - current_quality) / baseline_quality;
                    if drop_ratio > self.config.quality_degradation_threshold {
                        degraded.push((
                            result.scenario_id.clone(),
                            current_quality,
                            baseline_quality,
                        ));
                    }
                }
            }
        }

        degraded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eval_runner::{ScenarioResult, ValidationResult},
        quality_judge::QualityScore,
    };
    use chrono::Utc;

    fn create_test_report(
        num_passed: usize,
        num_failed: usize,
        avg_quality: f64,
        avg_latency: u64,
    ) -> EvalReport {
        let mut results = Vec::new();

        // Create passing scenarios
        for i in 0..num_passed {
            results.push(ScenarioResult {
                scenario_id: format!("scenario_{}", i),
                passed: true,
                output: "test output".to_string(),
                quality_score: QualityScore {
                    accuracy: avg_quality,
                    relevance: avg_quality,
                    completeness: avg_quality,
                    safety: 1.0,
                    coherence: avg_quality,
                    conciseness: avg_quality,
                    overall: avg_quality,
                    reasoning: "test".to_string(),
                    issues: Vec::new(),
                    suggestions: Vec::new(),
                },
                latency_ms: avg_latency,
                validation: ValidationResult {
                    passed: true,
                    missing_contains: Vec::new(),
                    forbidden_found: Vec::new(),
                    failure_reason: None,
                },
                error: None,
                retry_attempts: 0,
                timestamp: Utc::now(),
                input: None,
                tokens_used: None,
                cost_usd: None,
            });
        }

        // Create failing scenarios
        for i in 0..num_failed {
            results.push(ScenarioResult {
                scenario_id: format!("scenario_{}", num_passed + i),
                passed: false,
                output: "failed output".to_string(),
                quality_score: QualityScore {
                    accuracy: 0.5,
                    relevance: 0.5,
                    completeness: 0.5,
                    safety: 1.0,
                    coherence: 0.5,
                    conciseness: 0.5,
                    overall: 0.5,
                    reasoning: "test".to_string(),
                    issues: Vec::new(),
                    suggestions: Vec::new(),
                },
                latency_ms: avg_latency,
                validation: ValidationResult {
                    passed: false,
                    missing_contains: vec!["required content".to_string()],
                    forbidden_found: Vec::new(),
                    failure_reason: Some("Missing required content".to_string()),
                },
                error: Some("Quality threshold not met".to_string()),
                retry_attempts: 0,
                timestamp: Utc::now(),
                input: None,
                tokens_used: None,
                cost_usd: None,
            });
        }

        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;

        EvalReport {
            total,
            passed,
            failed,
            results,
            metadata: crate::eval_runner::EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 10.0,
                config: "{}".to_string(),
            },
        }
    }

    #[test]
    fn test_quality_gate_pass() {
        let gate = QualityGate::default_gate();
        let report = create_test_report(19, 1, 0.95, 1000);

        let result = gate.check(&report, None).unwrap();

        assert!(
            result.passed,
            "Gate should pass with 95% pass rate and 0.95 quality"
        );
        assert!(result.violations.is_empty());
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_quality_gate_fail_pass_rate() {
        let gate = QualityGate::default_gate();
        let report = create_test_report(17, 3, 0.95, 1000); // 85% pass rate

        let result = gate.check(&report, None).unwrap();

        assert!(!result.passed, "Gate should fail with 85% pass rate");
        // Should have 2 violations: pass rate (85% < 95%) and quality (0.88 < 0.90)
        assert_eq!(result.violations.len(), 2);
        assert!(result
            .violations
            .iter()
            .any(|v| v.violation_type == ViolationType::PassRateTooLow));
        assert!(result
            .violations
            .iter()
            .any(|v| v.violation_type == ViolationType::QualityTooLow));
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_quality_gate_fail_quality() {
        let gate = QualityGate::default_gate();
        let report = create_test_report(20, 0, 0.85, 1000); // 100% pass but low quality

        let result = gate.check(&report, None).unwrap();

        assert!(!result.passed, "Gate should fail with 0.85 quality");
        assert_eq!(result.violations.len(), 1);
        assert_eq!(
            result.violations[0].violation_type,
            ViolationType::QualityTooLow
        );
    }

    #[test]
    fn test_quality_gate_latency_regression() {
        let gate = QualityGate::default_gate();
        let baseline = create_test_report(20, 0, 0.95, 1000);
        let current = create_test_report(20, 0, 0.95, 1500); // 50% increase

        let result = gate.check(&current, Some(&baseline)).unwrap();

        assert!(!result.passed, "Gate should fail with 50% latency increase");
        let latency_violations: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.violation_type == ViolationType::LatencyRegression)
            .collect();
        assert_eq!(latency_violations.len(), 1);
    }

    #[test]
    fn test_quality_gate_new_failures() {
        let gate = QualityGate::default_gate();
        let mut baseline = create_test_report(20, 0, 0.95, 1000);
        let mut current = create_test_report(19, 0, 0.95, 1000);

        // Make scenario_5 fail in current but it passed in baseline
        current.results.push(ScenarioResult {
            scenario_id: "scenario_5_regression".to_string(),
            passed: false,
            output: "failed".to_string(),
            quality_score: QualityScore {
                accuracy: 0.5,
                relevance: 0.5,
                completeness: 0.5,
                safety: 1.0,
                coherence: 0.5,
                conciseness: 0.5,
                overall: 0.5,
                reasoning: "test".to_string(),
                issues: Vec::new(),
                suggestions: Vec::new(),
            },
            latency_ms: 1000,
            validation: ValidationResult {
                passed: false,
                missing_contains: vec!["required".to_string()],
                forbidden_found: Vec::new(),
                failure_reason: Some("Missing required content".to_string()),
            },
            error: Some("Failed".to_string()),
            retry_attempts: 0,
            timestamp: Utc::now(),
            input: None,
            tokens_used: None,
            cost_usd: None,
        });

        baseline.results.push(ScenarioResult {
            scenario_id: "scenario_5_regression".to_string(),
            passed: true,
            output: "passed".to_string(),
            quality_score: QualityScore {
                accuracy: 0.95,
                relevance: 0.95,
                completeness: 0.95,
                safety: 1.0,
                coherence: 0.95,
                conciseness: 0.95,
                overall: 0.95,
                reasoning: "test".to_string(),
                issues: Vec::new(),
                suggestions: Vec::new(),
            },
            latency_ms: 1000,
            validation: ValidationResult {
                passed: true,
                missing_contains: Vec::new(),
                forbidden_found: Vec::new(),
                failure_reason: None,
            },
            error: None,
            retry_attempts: 0,
            timestamp: Utc::now(),
            input: None,
            tokens_used: None,
            cost_usd: None,
        });

        let result = gate.check(&current, Some(&baseline)).unwrap();

        assert!(!result.passed, "Gate should fail with new failures");
        let failure_violations: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.violation_type == ViolationType::NewFailures)
            .collect();
        assert_eq!(failure_violations.len(), 1);
        assert!(failure_violations[0]
            .description
            .contains("scenario_5_regression"));
    }

    #[test]
    fn test_quality_gate_custom_thresholds() {
        let config = QualityGateConfig::default()
            .with_min_pass_rate(0.80)
            .with_min_quality(0.85);

        let gate = QualityGate::new(config);
        let report = create_test_report(20, 0, 0.87, 1000); // 100% pass, 0.87 quality

        let result = gate.check(&report, None).unwrap();

        assert!(
            result.passed,
            "Gate should pass with custom lower thresholds: pass_rate={}, quality={}",
            report.pass_rate(),
            report.avg_quality()
        );
    }
}
