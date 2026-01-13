//! Comprehensive reporting module for evaluation results.
//!
//! Provides multiple output formats for different use cases:
//! - **HTML**: Beautiful interactive reports for human review
//! - **JSON**: Machine-readable output for CI/CD pipelines
//! - **Markdown**: GitHub PR comments and documentation
//! - **Charts**: Visual analysis with SVG charts
//! - **Diff**: Compare expected vs actual outputs
//!
//! # Examples
//!
//! ## Generate HTML Report
//!
//! ```no_run
//! use dashflow_evals::report::html::HtmlReportGenerator;
//! # use dashflow_evals::eval_runner::EvalReport;
//! # fn example(report: EvalReport) -> anyhow::Result<()> {
//! HtmlReportGenerator::generate(&report, "my_dataset", "report.html")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Generate JSON for CI/CD
//!
//! ```no_run
//! use dashflow_evals::report::json::JsonReportGenerator;
//! # use dashflow_evals::eval_runner::EvalReport;
//! # fn example(report: EvalReport) -> anyhow::Result<()> {
//! JsonReportGenerator::generate(&report, Some("dataset"), None, "results.json")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Generate GitHub PR Comment
//!
//! ```no_run
//! use dashflow_evals::report::markdown::MarkdownReportGenerator;
//! # use dashflow_evals::eval_runner::EvalReport;
//! # fn example(report: EvalReport) -> anyhow::Result<()> {
//! let comment = MarkdownReportGenerator::generate_github_comment(&report, "dataset", None)?;
//! println!("{}", comment);
//! # Ok(())
//! # }
//! ```
//!
//! ## Generate All Charts
//!
//! ```no_run
//! use dashflow_evals::report::charts::ChartGenerator;
//! # use dashflow_evals::eval_runner::EvalReport;
//! # fn example(report: EvalReport) -> anyhow::Result<()> {
//! ChartGenerator::generate_all(&report, "charts/", "eval")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Compare Expected vs Actual
//!
//! ```
//! use dashflow_evals::report::diff::DiffGenerator;
//!
//! let expected = "Expected output";
//! let actual = "Actual output";
//! let html_diff = DiffGenerator::generate_html_diff(expected, actual)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod charts;
pub mod diff;
pub mod html;
pub mod json;
pub mod markdown;

use crate::eval_runner::EvalReport;

/// Convenience function to generate all report formats
///
/// Generates HTML, JSON, Markdown, and all charts in one call.
///
/// # Arguments
///
/// * `report` - Evaluation report
/// * `dataset_name` - Name of the dataset
/// * `output_dir` - Directory to save all outputs
/// * `prefix` - Filename prefix for all generated files
///
/// # Example
///
/// ```no_run
/// use dashflow_evals::report::generate_all_reports;
/// # use dashflow_evals::eval_runner::EvalReport;
/// # fn example(report: EvalReport) -> anyhow::Result<()> {
/// generate_all_reports(&report, "librarian", "reports/", "eval")?;
/// # Ok(())
/// # }
/// ```
///
/// This will create:
/// - `reports/eval.html` - Interactive HTML report
/// - `reports/eval.json` - JSON results for CI/CD
/// - `reports/eval.md` - Markdown for GitHub
/// - `reports/eval_quality.svg` - Quality histogram
/// - `reports/eval_latency.svg` - Latency chart
/// - `reports/eval_pass_fail.svg` - Pass/fail pie chart
pub fn generate_all_reports(
    report: &EvalReport,
    dataset_name: &str,
    output_dir: impl AsRef<std::path::Path>,
    prefix: &str,
) -> anyhow::Result<()> {
    let output_dir = output_dir.as_ref();

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)?;

    // Generate HTML report
    html::HtmlReportGenerator::generate(
        report,
        dataset_name,
        output_dir.join(format!("{prefix}.html")),
    )?;

    // Generate JSON report
    json::JsonReportGenerator::generate(
        report,
        Some(dataset_name),
        None,
        output_dir.join(format!("{prefix}.json")),
    )?;

    // Generate Markdown report
    markdown::MarkdownReportGenerator::generate_file(
        report,
        dataset_name,
        None,
        output_dir.join(format!("{prefix}.md")),
    )?;

    // Generate all charts
    charts::ChartGenerator::generate_all(report, output_dir, prefix)?;

    // Generate quality dimensions chart
    charts::ChartGenerator::quality_dimensions_chart(
        report,
        output_dir.join(format!("{prefix}_dimensions.svg")),
    )?;

    Ok(())
}

/// Generate exit code based on evaluation results
///
/// Returns 0 if all quality gates pass, non-zero otherwise.
/// Useful for CI/CD pipelines.
#[must_use]
pub fn exit_code(report: &EvalReport) -> i32 {
    if report.failed > 0 {
        return 1; // Some scenarios failed
    }

    if report.avg_quality() < 0.90 {
        return 2; // Quality below threshold
    }

    0 // Success
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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
            output: "Test output".to_string(),
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
    fn test_generate_all_reports() {
        let temp_dir = TempDir::new().unwrap();

        let report = EvalReport {
            total: 3,
            passed: 3,
            failed: 0,
            results: vec![
                create_mock_result("s1", true, 0.95),
                create_mock_result("s2", true, 0.92),
                create_mock_result("s3", true, 0.96),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 3.0,
                config: "{}".to_string(),
            },
        };

        generate_all_reports(&report, "test_dataset", temp_dir.path(), "test").unwrap();

        // Verify all files were created
        assert!(temp_dir.path().join("test.html").exists());
        assert!(temp_dir.path().join("test.json").exists());
        assert!(temp_dir.path().join("test.md").exists());
        assert!(temp_dir.path().join("test_quality.svg").exists());
        assert!(temp_dir.path().join("test_latency.svg").exists());
        assert!(temp_dir.path().join("test_pass_fail.svg").exists());
        assert!(temp_dir.path().join("test_dimensions.svg").exists());
    }

    #[test]
    fn test_exit_code_success() {
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

        assert_eq!(exit_code(&report), 0);
    }

    #[test]
    fn test_exit_code_failure() {
        let report = EvalReport {
            total: 2,
            passed: 1,
            failed: 1,
            results: vec![
                create_mock_result("s1", true, 0.95),
                create_mock_result("s2", false, 0.85),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 2.0,
                config: "{}".to_string(),
            },
        };

        assert_eq!(exit_code(&report), 1);
    }

    #[test]
    fn test_exit_code_low_quality() {
        let report = EvalReport {
            total: 2,
            passed: 2,
            failed: 0,
            results: vec![
                create_mock_result("s1", true, 0.88),
                create_mock_result("s2", true, 0.87),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 2.0,
                config: "{}".to_string(),
            },
        };

        assert_eq!(exit_code(&report), 2);
    }
}
