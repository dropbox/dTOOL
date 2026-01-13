//! JSON output for CI/CD integration and programmatic consumption.
//!
//! Provides machine-readable evaluation results in JSON format optimized for:
//! - CI/CD pipelines (GitHub Actions, GitLab CI, etc.)
//! - Monitoring dashboards
//! - Automated quality gates
//! - Historical trend analysis
//! - Integration with other tools

use crate::eval_runner::EvalReport;
use crate::quality_judge::QualityIssue;
use crate::regression::RegressionReport;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Compact JSON summary for CI/CD consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReport {
    /// High-level summary metrics
    pub summary: JsonSummary,

    /// Per-scenario results
    pub scenarios: Vec<JsonScenarioResult>,

    /// Regression detection results (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regressions: Option<JsonRegressions>,

    /// Metadata
    pub metadata: JsonMetadata,
}

/// Summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSummary {
    /// Total number of scenarios
    pub total_scenarios: usize,

    /// Number of passed scenarios
    pub passed: usize,

    /// Number of failed scenarios
    pub failed: usize,

    /// Pass rate (0.0-1.0)
    pub pass_rate: f64,

    /// Average quality score (0.0-1.0)
    pub avg_quality: f64,

    /// Average latency in milliseconds
    pub avg_latency_ms: u64,

    /// Minimum latency
    pub min_latency_ms: u64,

    /// Maximum latency
    pub max_latency_ms: u64,

    /// Total execution time in seconds
    pub total_duration_secs: f64,

    /// Quality distribution
    pub quality_distribution: QualityDistribution,
}

/// Quality score distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityDistribution {
    /// Number of scenarios with quality >= 0.95
    pub excellent: usize,

    /// Number of scenarios with quality 0.90-0.95
    pub good: usize,

    /// Number of scenarios with quality 0.80-0.90
    pub fair: usize,

    /// Number of scenarios with quality < 0.80
    pub poor: usize,
}

/// Individual scenario result in compact format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonScenarioResult {
    /// Scenario identifier
    pub id: String,

    /// Pass/fail status
    pub passed: bool,

    /// Overall quality score
    pub quality: f64,

    /// Detailed quality dimensions
    pub quality_dimensions: QualityDimensions,

    /// Execution latency in milliseconds
    pub latency_ms: u64,

    /// Number of retry attempts
    pub retry_attempts: u32,

    /// Issues found (empty if none)
    pub issues: Vec<JsonIssue>,

    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Quality scores across all dimensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityDimensions {
    pub accuracy: f64,
    pub relevance: f64,
    pub completeness: f64,
    pub safety: f64,
    pub coherence: f64,
    pub conciseness: f64,
}

/// Compact issue representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonIssue {
    pub dimension: String,
    pub severity: String,
    pub description: String,
}

impl From<&QualityIssue> for JsonIssue {
    fn from(issue: &QualityIssue) -> Self {
        Self {
            dimension: issue.dimension.clone(),
            severity: format!("{:?}", issue.severity),
            description: issue.description.clone(),
        }
    }
}

/// Regression detection results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRegressions {
    /// Were any regressions detected?
    pub has_regressions: bool,

    /// Number of regressions
    pub regression_count: usize,

    /// List of regression descriptions
    pub regressions: Vec<String>,

    /// Baseline commit hash
    pub baseline_commit: String,

    /// Current commit hash
    pub current_commit: String,
}

/// Metadata about the evaluation run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonMetadata {
    /// When the evaluation started (ISO 8601)
    pub started_at: String,

    /// When the evaluation completed (ISO 8601)
    pub completed_at: String,

    /// Total duration in seconds
    pub duration_secs: f64,

    /// Dataset name or identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_name: Option<String>,

    /// Git commit hash (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,

    /// Git branch (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
}

/// JSON report generator
pub struct JsonReportGenerator;

impl JsonReportGenerator {
    /// Generate JSON report from evaluation results
    ///
    /// # Arguments
    ///
    /// * `report` - Evaluation report
    /// * `dataset_name` - Optional dataset name
    /// * `regression_report` - Optional regression detection results
    /// * `output_path` - Where to save the JSON file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_evals::report::json::JsonReportGenerator;
    /// # use dashflow_evals::eval_runner::EvalReport;
    /// # fn example(report: EvalReport) -> anyhow::Result<()> {
    /// JsonReportGenerator::generate(
    ///     &report,
    ///     Some("librarian"),
    ///     None,
    ///     "eval_results.json"
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate(
        report: &EvalReport,
        dataset_name: Option<&str>,
        regression_report: Option<&RegressionReport>,
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let json_report = Self::build_json_report(report, dataset_name, regression_report)?;

        // Pretty-print JSON for human readability
        let json = serde_json::to_string_pretty(&json_report)
            .context("Failed to serialize JSON report")?;

        fs::write(output_path.as_ref(), json).with_context(|| {
            format!("Failed to write JSON report to {:?}", output_path.as_ref())
        })?;

        Ok(())
    }

    /// Generate JSON string without writing to file
    pub fn generate_string(
        report: &EvalReport,
        dataset_name: Option<&str>,
        regression_report: Option<&RegressionReport>,
    ) -> Result<String> {
        let json_report = Self::build_json_report(report, dataset_name, regression_report)?;
        serde_json::to_string_pretty(&json_report).context("Failed to serialize JSON report")
    }

    /// Generate compact JSON (minified, no pretty-printing)
    pub fn generate_compact(
        report: &EvalReport,
        dataset_name: Option<&str>,
        regression_report: Option<&RegressionReport>,
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let json_report = Self::build_json_report(report, dataset_name, regression_report)?;

        let json =
            serde_json::to_string(&json_report).context("Failed to serialize JSON report")?;

        fs::write(output_path.as_ref(), json).with_context(|| {
            format!("Failed to write JSON report to {:?}", output_path.as_ref())
        })?;

        Ok(())
    }

    fn build_json_report(
        report: &EvalReport,
        dataset_name: Option<&str>,
        regression_report: Option<&RegressionReport>,
    ) -> Result<JsonReport> {
        // Calculate distribution
        let mut excellent = 0;
        let mut good = 0;
        let mut fair = 0;
        let mut poor = 0;

        for result in &report.results {
            let quality = result.quality_score.overall;
            if quality >= 0.95 {
                excellent += 1;
            } else if quality >= 0.90 {
                good += 1;
            } else if quality >= 0.80 {
                fair += 1;
            } else {
                poor += 1;
            }
        }

        let min_latency = report
            .results
            .iter()
            .map(|r| r.latency_ms)
            .min()
            .unwrap_or(0);
        let max_latency = report
            .results
            .iter()
            .map(|r| r.latency_ms)
            .max()
            .unwrap_or(0);

        // Build summary
        let summary = JsonSummary {
            total_scenarios: report.total,
            passed: report.passed,
            failed: report.failed,
            pass_rate: report.pass_rate(),
            avg_quality: report.avg_quality(),
            avg_latency_ms: report.avg_latency_ms(),
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
            total_duration_secs: report.metadata.duration_secs,
            quality_distribution: QualityDistribution {
                excellent,
                good,
                fair,
                poor,
            },
        };

        // Build scenario results
        let scenarios: Vec<JsonScenarioResult> = report
            .results
            .iter()
            .map(|r| JsonScenarioResult {
                id: r.scenario_id.clone(),
                passed: r.passed,
                quality: r.quality_score.overall,
                quality_dimensions: QualityDimensions {
                    accuracy: r.quality_score.accuracy,
                    relevance: r.quality_score.relevance,
                    completeness: r.quality_score.completeness,
                    safety: r.quality_score.safety,
                    coherence: r.quality_score.coherence,
                    conciseness: r.quality_score.conciseness,
                },
                latency_ms: r.latency_ms,
                retry_attempts: r.retry_attempts,
                issues: r.quality_score.issues.iter().map(JsonIssue::from).collect(),
                error: r.error.clone(),
            })
            .collect();

        // Build regressions (if available)
        let regressions = regression_report.map(|rr| JsonRegressions {
            has_regressions: !rr.regressions.is_empty(),
            regression_count: rr.regressions.len(),
            regressions: rr
                .regressions
                .iter()
                .map(|r| format!("{:?}: {}", r.regression_type, r.details))
                .collect(),
            baseline_commit: rr.baseline_commit.clone().unwrap_or_default(),
            current_commit: rr.current_commit.clone().unwrap_or_default(),
        });

        // Build metadata
        let metadata = JsonMetadata {
            started_at: report.metadata.started_at.to_rfc3339(),
            completed_at: report.metadata.completed_at.to_rfc3339(),
            duration_secs: report.metadata.duration_secs,
            dataset_name: dataset_name.map(String::from),
            git_commit: Self::get_git_commit().ok(),
            git_branch: Self::get_git_branch().ok(),
        };

        Ok(JsonReport {
            summary,
            scenarios,
            regressions,
            metadata,
        })
    }

    fn get_git_commit() -> Result<String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .context("Failed to get git commit")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            anyhow::bail!("Git command failed")
        }
    }

    fn get_git_branch() -> Result<String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .context("Failed to get git branch")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            anyhow::bail!("Git command failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_runner::{EvalMetadata, ScenarioResult, ValidationResult};
    use crate::quality_judge::QualityScore;
    use chrono::Utc;
    use tempfile::TempDir;

    fn create_mock_result(id: &str, passed: bool, quality: f64) -> ScenarioResult {
        ScenarioResult {
            scenario_id: id.to_string(),
            passed,
            output: format!("Output for {}", id),
            quality_score: QualityScore {
                accuracy: quality,
                relevance: quality,
                completeness: quality,
                safety: 1.0,
                coherence: quality,
                conciseness: quality,
                overall: quality,
                reasoning: "Test".to_string(),
                issues: vec![],
                suggestions: vec![],
            },
            latency_ms: 1000,
            validation: ValidationResult {
                passed: true,
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

    #[test]
    fn test_json_generation() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("report.json");

        let report = EvalReport {
            total: 2,
            passed: 2,
            failed: 0,
            results: vec![
                create_mock_result("s1", true, 0.95),
                create_mock_result("s2", true, 0.92),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 2.0,
                config: "{}".to_string(),
            },
        };

        JsonReportGenerator::generate(&report, Some("test"), None, &output_path).unwrap();

        // Verify file exists
        assert!(output_path.exists());

        // Parse and verify JSON structure
        let json_str = fs::read_to_string(&output_path).unwrap();
        let json_report: JsonReport = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json_report.summary.total_scenarios, 2);
        assert_eq!(json_report.summary.passed, 2);
        assert_eq!(json_report.summary.failed, 0);
        assert_eq!(json_report.scenarios.len(), 2);
        assert_eq!(json_report.metadata.dataset_name, Some("test".to_string()));
    }

    #[test]
    fn test_quality_distribution() {
        let report = EvalReport {
            total: 4,
            passed: 4,
            failed: 0,
            results: vec![
                create_mock_result("excellent", true, 0.97),
                create_mock_result("good", true, 0.92),
                create_mock_result("fair", true, 0.85),
                create_mock_result("poor", true, 0.70),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 4.0,
                config: "{}".to_string(),
            },
        };

        let json_str = JsonReportGenerator::generate_string(&report, None, None).unwrap();
        let json_report: JsonReport = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json_report.summary.quality_distribution.excellent, 1);
        assert_eq!(json_report.summary.quality_distribution.good, 1);
        assert_eq!(json_report.summary.quality_distribution.fair, 1);
        assert_eq!(json_report.summary.quality_distribution.poor, 1);
    }

    #[test]
    fn test_compact_generation() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("compact.json");

        let report = EvalReport {
            total: 1,
            passed: 1,
            failed: 0,
            results: vec![create_mock_result("s1", true, 0.95)],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 1.0,
                config: "{}".to_string(),
            },
        };

        JsonReportGenerator::generate_compact(&report, None, None, &output_path).unwrap();

        let json_str = fs::read_to_string(&output_path).unwrap();
        // Compact JSON should not have newlines (minified)
        assert!(!json_str.contains("\n  ")); // No indentation
    }
}
