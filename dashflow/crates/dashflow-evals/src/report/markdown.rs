//! Markdown report generation for GitHub PR comments and documentation.
//!
//! Generates comprehensive markdown reports optimized for:
//! - GitHub PR comments with expandable sections
//! - Slack messages
//! - Confluence/wiki documentation
//! - Email notifications
//! - CLI output

use crate::eval_runner::{EvalReport, ScenarioResult};
use crate::quality_judge::IssueSeverity;
use crate::regression::RegressionReport;
use anyhow::{Context, Result};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::Path;

/// Markdown report generator
pub struct MarkdownReportGenerator;

impl MarkdownReportGenerator {
    /// Generate markdown report for GitHub PR comments
    ///
    /// Creates a comprehensive, well-formatted markdown report with:
    /// - Summary cards with emoji indicators
    /// - Quality distribution table
    /// - Regression alerts (if applicable)
    /// - Failed scenarios with details
    /// - Expandable section with all results
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_evals::report::markdown::MarkdownReportGenerator;
    /// # use dashflow_evals::eval_runner::EvalReport;
    /// # fn example(report: EvalReport) -> anyhow::Result<()> {
    /// let markdown = MarkdownReportGenerator::generate_github_comment(
    ///     &report,
    ///     "librarian",
    ///     None
    /// )?;
    /// println!("{}", markdown);
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_github_comment(
        report: &EvalReport,
        dataset_name: &str,
        regression_report: Option<&RegressionReport>,
    ) -> Result<String> {
        let mut md = String::new();

        // Header
        writeln!(md, "## Evaluation Report")?;
        writeln!(md)?;
        writeln!(md, "**Dataset:** `{dataset_name}`")?;
        writeln!(
            md,
            "**Timestamp:** {}",
            report.metadata.started_at.format("%Y-%m-%d %H:%M UTC")
        )?;
        writeln!(md)?;

        // Summary metrics
        let pass_rate = report.pass_rate() * 100.0;
        let pass_emoji = if pass_rate >= 95.0 {
            "✅"
        } else if pass_rate >= 80.0 {
            "⚠️"
        } else {
            "❌"
        };

        let avg_quality = report.avg_quality();
        let quality_emoji = if avg_quality >= 0.95 {
            "✅"
        } else if avg_quality >= 0.90 {
            "⚠️"
        } else {
            "❌"
        };

        writeln!(md, "### Summary")?;
        writeln!(md)?;
        writeln!(md, "| Metric | Value | Status |")?;
        writeln!(md, "|--------|-------|--------|")?;
        writeln!(
            md,
            "| **Pass Rate** | {}/{} ({:.1}%) | {} |",
            report.passed, report.total, pass_rate, pass_emoji
        )?;
        writeln!(
            md,
            "| **Avg Quality** | {avg_quality:.3} | {quality_emoji} |"
        )?;
        writeln!(
            md,
            "| **Avg Latency** | {}ms | ℹ️ |",
            report.avg_latency_ms()
        )?;
        writeln!(
            md,
            "| **Duration** | {:.1}s | ℹ️ |",
            report.metadata.duration_secs
        )?;
        writeln!(md)?;

        // Quality distribution
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

        writeln!(md, "### Quality Distribution")?;
        writeln!(md)?;
        writeln!(md, "| Category | Count | Percentage |")?;
        writeln!(md, "|----------|-------|------------|")?;
        writeln!(
            md,
            "| ✅ Excellent (≥0.95) | {} | {:.1}% |",
            excellent,
            (f64::from(excellent) / report.total as f64) * 100.0
        )?;
        writeln!(
            md,
            "| ✓ Good (0.90-0.95) | {} | {:.1}% |",
            good,
            (f64::from(good) / report.total as f64) * 100.0
        )?;
        writeln!(
            md,
            "| ⚠️ Fair (0.80-0.90) | {} | {:.1}% |",
            fair,
            (f64::from(fair) / report.total as f64) * 100.0
        )?;
        writeln!(
            md,
            "| ❌ Poor (<0.80) | {} | {:.1}% |",
            poor,
            (f64::from(poor) / report.total as f64) * 100.0
        )?;
        writeln!(md)?;

        // Regression alerts
        if let Some(rr) = regression_report {
            if rr.regressions.is_empty() {
                writeln!(md, "### ✅ No Regressions")?;
                writeln!(md)?;
                writeln!(
                    md,
                    "All quality metrics are stable or improved compared to baseline."
                )?;
                writeln!(md)?;
            } else {
                writeln!(md, "### 🚨 Regressions Detected")?;
                writeln!(md)?;
                writeln!(
                    md,
                    "**{}** regression(s) detected compared to baseline:",
                    rr.regressions.len()
                )?;
                writeln!(md)?;

                for regression in &rr.regressions {
                    let emoji = match regression.severity {
                        crate::regression::Severity::Critical => "🔴",
                        crate::regression::Severity::Warning => "🟠",
                        crate::regression::Severity::Info => "🟡",
                    };
                    writeln!(
                        md,
                        "- {} **{:?}**: {}",
                        emoji, regression.regression_type, regression.details
                    )?;
                }
                writeln!(md)?;

                if let Some(baseline) = &rr.baseline_commit {
                    writeln!(md, "**Baseline:** `{}`", &baseline[..8])?;
                }
                if let Some(current) = &rr.current_commit {
                    writeln!(md, "**Current:** `{}`", &current[..8])?;
                }
                writeln!(md)?;
            }
        }

        // Failed scenarios
        let failed_scenarios: Vec<&ScenarioResult> =
            report.results.iter().filter(|r| !r.passed).collect();

        if !failed_scenarios.is_empty() {
            writeln!(md, "### ❌ Failed Scenarios")?;
            writeln!(md)?;
            writeln!(
                md,
                "{} scenario(s) failed quality thresholds:",
                failed_scenarios.len()
            )?;
            writeln!(md)?;

            for result in failed_scenarios {
                writeln!(md, "#### `{}`", result.scenario_id)?;
                writeln!(md)?;
                writeln!(
                    md,
                    "**Quality:** {:.3} (threshold: ≥0.90)",
                    result.quality_score.overall
                )?;
                writeln!(md, "**Latency:** {}ms", result.latency_ms)?;
                writeln!(md)?;

                // Show critical/major issues
                let critical_issues: Vec<_> = result
                    .quality_score
                    .issues
                    .iter()
                    .filter(|i| {
                        matches!(i.severity, IssueSeverity::Critical | IssueSeverity::Major)
                    })
                    .collect();

                if !critical_issues.is_empty() {
                    writeln!(md, "**Issues:**")?;
                    for issue in critical_issues {
                        let emoji = match issue.severity {
                            IssueSeverity::Critical => "🔴",
                            IssueSeverity::Major => "🟠",
                            IssueSeverity::Minor => "🟡",
                        };
                        writeln!(
                            md,
                            "- {} **{}**: {}",
                            emoji, issue.dimension, issue.description
                        )?;
                    }
                    writeln!(md)?;
                }

                if let Some(error) = &result.error {
                    writeln!(md, "**Error:**")?;
                    writeln!(md, "```")?;
                    writeln!(md, "{error}")?;
                    writeln!(md, "```")?;
                    writeln!(md)?;
                }
            }
        }

        // Expandable details for all scenarios
        writeln!(md, "<details>")?;
        writeln!(
            md,
            "<summary><strong>View All Scenario Results</strong> (click to expand)</summary>"
        )?;
        writeln!(md)?;
        writeln!(md, "| Scenario | Status | Quality | Latency | Retries |")?;
        writeln!(md, "|----------|--------|---------|---------|---------|")?;

        for result in &report.results {
            let status = if result.passed {
                "✅ PASS"
            } else {
                "❌ FAIL"
            };
            let quality_indicator = if result.quality_score.overall >= 0.95 {
                "🟢"
            } else if result.quality_score.overall >= 0.90 {
                "🟡"
            } else {
                "🔴"
            };

            writeln!(
                md,
                "| `{}` | {} | {} {:.3} | {}ms | {} |",
                result.scenario_id,
                status,
                quality_indicator,
                result.quality_score.overall,
                result.latency_ms,
                result.retry_attempts
            )?;
        }

        writeln!(md)?;
        writeln!(md, "</details>")?;
        writeln!(md)?;

        // Footer
        writeln!(md, "---")?;
        writeln!(
            md,
            "*Generated by [DashFlow-Evals](https://github.com/dashflow-ai/dashflow)*"
        )?;

        Ok(md)
    }

    /// Generate simplified markdown for Slack messages
    ///
    /// Creates a shorter, more concise report suitable for Slack notifications.
    pub fn generate_slack_message(
        report: &EvalReport,
        dataset_name: &str,
        regression_report: Option<&RegressionReport>,
    ) -> Result<String> {
        let mut md = String::new();

        let pass_rate = report.pass_rate() * 100.0;
        let emoji = if pass_rate >= 95.0 && report.avg_quality() >= 0.90 {
            ":white_check_mark:"
        } else if pass_rate >= 80.0 {
            ":warning:"
        } else {
            ":x:"
        };

        writeln!(md, "{emoji} *Evaluation Report: {dataset_name}*")?;
        writeln!(md)?;
        writeln!(
            md,
            "*Pass Rate:* {}/{} ({:.1}%)",
            report.passed, report.total, pass_rate
        )?;
        writeln!(md, "*Avg Quality:* {:.3}", report.avg_quality())?;
        writeln!(md, "*Avg Latency:* {}ms", report.avg_latency_ms())?;
        writeln!(md)?;

        if let Some(rr) = regression_report {
            if !rr.regressions.is_empty() {
                writeln!(
                    md,
                    ":rotating_light: *{} regressions detected!*",
                    rr.regressions.len()
                )?;
                writeln!(md)?;
            }
        }

        let failed_count = report.results.iter().filter(|r| !r.passed).count();
        if failed_count > 0 {
            writeln!(md, ":x: {failed_count} scenario(s) failed")?;
        } else {
            writeln!(md, ":tada: All scenarios passed!")?;
        }

        Ok(md)
    }

    /// Generate markdown and save to file
    pub fn generate_file(
        report: &EvalReport,
        dataset_name: &str,
        regression_report: Option<&RegressionReport>,
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let markdown = Self::generate_github_comment(report, dataset_name, regression_report)?;

        fs::write(output_path.as_ref(), markdown).with_context(|| {
            format!(
                "Failed to write markdown report to {:?}",
                output_path.as_ref()
            )
        })?;

        Ok(())
    }

    /// Generate CLI-friendly output (plain text with ANSI colors would go here)
    pub fn generate_cli_output(report: &EvalReport) -> Result<String> {
        let mut output = String::new();

        writeln!(output, "EVALUATION REPORT")?;
        writeln!(output, "=================")?;
        writeln!(output)?;
        writeln!(
            output,
            "Pass Rate:   {}/{} ({:.1}%)",
            report.passed,
            report.total,
            report.pass_rate() * 100.0
        )?;
        writeln!(output, "Avg Quality: {:.3}", report.avg_quality())?;
        writeln!(output, "Avg Latency: {}ms", report.avg_latency_ms())?;
        writeln!(output, "Duration:    {:.1}s", report.metadata.duration_secs)?;
        writeln!(output)?;

        let failed: Vec<_> = report.results.iter().filter(|r| !r.passed).collect();
        if failed.is_empty() {
            writeln!(output, "All scenarios passed!")?;
        } else {
            writeln!(output, "FAILED SCENARIOS:")?;
            writeln!(output, "-----------------")?;
            for result in failed {
                writeln!(
                    output,
                    "  - {} (quality: {:.3})",
                    result.scenario_id, result.quality_score.overall
                )?;
            }
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_runner::{EvalMetadata, ValidationResult};
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
                reasoning: "Test reasoning".to_string(),
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
    fn test_github_comment_generation() {
        let report = EvalReport {
            total: 3,
            passed: 2,
            failed: 1,
            results: vec![
                create_mock_result("s1", true, 0.95),
                create_mock_result("s2", true, 0.92),
                create_mock_result("s3", false, 0.85),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 3.0,
                config: "{}".to_string(),
            },
        };

        let markdown =
            MarkdownReportGenerator::generate_github_comment(&report, "test", None).unwrap();

        assert!(markdown.contains("Evaluation Report"));
        assert!(markdown.contains("test"));
        assert!(markdown.contains("2/3"));
        assert!(markdown.contains("Quality Distribution"));
        assert!(markdown.contains("Failed Scenarios"));
        assert!(markdown.contains("s3"));
    }

    #[test]
    fn test_slack_message_generation() {
        let report = EvalReport {
            total: 2,
            passed: 2,
            failed: 0,
            results: vec![
                create_mock_result("s1", true, 0.96),
                create_mock_result("s2", true, 0.94),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 2.0,
                config: "{}".to_string(),
            },
        };

        let slack = MarkdownReportGenerator::generate_slack_message(&report, "test", None).unwrap();

        assert!(slack.contains("Evaluation Report: test"));
        assert!(slack.contains("2/2"));
        assert!(slack.contains("All scenarios passed"));
    }

    #[test]
    fn test_cli_output_generation() {
        let report = EvalReport {
            total: 2,
            passed: 1,
            failed: 1,
            results: vec![
                create_mock_result("pass", true, 0.95),
                create_mock_result("fail", false, 0.80),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 2.0,
                config: "{}".to_string(),
            },
        };

        let cli = MarkdownReportGenerator::generate_cli_output(&report).unwrap();

        assert!(cli.contains("EVALUATION REPORT"));
        assert!(cli.contains("1/2"));
        assert!(cli.contains("FAILED SCENARIOS"));
        assert!(cli.contains("fail"));
    }

    #[test]
    fn test_file_generation() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("report.md");

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

        MarkdownReportGenerator::generate_file(&report, "test", None, &output_path).unwrap();

        assert!(output_path.exists());
        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("Evaluation Report"));
    }
}
