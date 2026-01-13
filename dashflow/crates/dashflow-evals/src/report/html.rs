//! HTML report generation with beautiful, interactive visualization.
//!
//! Generates a comprehensive HTML report with:
//! - Summary cards showing pass rate, quality, latency
//! - Interactive table with expandable scenario details
//! - Color-coded quality indicators
//! - Client-side filtering and search
//! - Quality distribution statistics
//! - Detailed per-dimension scoring

use crate::eval_runner::EvalReport;
use crate::quality_judge::{IssueSeverity, QualityIssue};
use anyhow::{Context, Result};
use askama::Template;
use chrono::{Datelike, Utc};
use std::fs;
use std::path::Path;

/// Template context for rendering HTML reports
#[derive(Template)]
#[template(path = "report.html")]
struct ReportTemplate {
    // Header info
    dataset_name: String,
    timestamp: String,
    year: i32,

    // Summary metrics
    total: usize,
    passed: usize,
    pass_rate: String,
    pass_rate_num: f64, // Numeric version for template comparisons
    pass_class: String,

    avg_quality: String,
    quality_class: String,

    avg_latency: String,
    min_latency: u64,
    max_latency: u64,
    p50_latency: u64,
    p90_latency: u64,
    p95_latency: u64,
    p99_latency: u64,

    // Distribution
    excellent_count: usize,
    good_count: usize,
    below_count: usize,

    // Statistical rigor
    pass_rate_ci_lower: String,
    pass_rate_ci_upper: String,
    quality_ci_lower: String,
    quality_ci_upper: String,
    quality_threshold_met: usize,

    // Individual results
    results: Vec<ResultRow>,

    // Executive summary and recommendations
    executive_summary: ExecutiveSummary,
    recommendations: Vec<Recommendation>,
}

/// Executive summary with key insights
#[derive(Debug)]
struct ExecutiveSummary {
    pass_trend: String,
    quality_trend: String,
    latency_trend: String,
    key_insights: Vec<String>,
}

/// Actionable recommendation based on data
#[derive(Debug)]
struct Recommendation {
    priority: String, // "High" | "Medium" | "Low"
    category: String, // "Quality" | "Performance" | "Coverage"
    description: String,
    action: String,
    data_link: Option<String>, // Anchor link to relevant data section (e.g., "#quality-distribution")
    data_section: String,      // Human-readable section name (e.g., "Quality Distribution")
}

/// Individual scenario result for template rendering
#[derive(Debug)]
struct ResultRow {
    id: String,
    status: String,
    status_label: String,
    quality_score: String,
    quality_percent: u32,
    quality_class: String,
    latency_ms: u64,
    retry_attempts: u32,
    is_critical: bool, // Failed or very low quality

    // Detailed info (shown when expanded)
    output: String,
    accuracy: String,
    relevance: String,
    completeness: String,
    safety: String,
    coherence: String,
    conciseness: String,
    reasoning: String,
    issues: Vec<IssueRow>,
    suggestions: Vec<String>,
    error: Option<String>,
}

#[derive(Debug)]
struct IssueRow {
    dimension: String,
    description: String,
    severity_class: String,
    example: Option<String>,
}

impl From<&QualityIssue> for IssueRow {
    fn from(issue: &QualityIssue) -> Self {
        let severity_class = match issue.severity {
            IssueSeverity::Critical => "critical",
            IssueSeverity::Major => "major",
            IssueSeverity::Minor => "minor",
        }
        .to_string();

        Self {
            dimension: issue.dimension.clone(),
            description: issue.description.clone(),
            severity_class,
            example: issue.example.clone(),
        }
    }
}

/// HTML report generator
pub struct HtmlReportGenerator;

impl HtmlReportGenerator {
    /// Calculate confidence interval for a proportion (Wilson score interval)
    fn confidence_interval_95(successes: usize, total: usize) -> (f64, f64) {
        if total == 0 {
            return (0.0, 0.0);
        }

        let p = successes as f64 / total as f64;
        let n = total as f64;
        let z = 1.96; // 95% confidence

        let denominator = 1.0 + z * z / n;
        let center = p + z * z / (2.0 * n);
        let margin = z * ((p * (1.0 - p) / n) + (z * z / (4.0 * n * n))).sqrt();

        let lower = ((center - margin) / denominator).max(0.0);
        let upper = ((center + margin) / denominator).min(1.0);

        (lower, upper)
    }

    /// Calculate percentile from sorted values
    fn percentile(sorted_values: &[u64], p: f64) -> u64 {
        if sorted_values.is_empty() {
            return 0;
        }

        let index = (p * (sorted_values.len() - 1) as f64).round() as usize;
        sorted_values[index.min(sorted_values.len() - 1)]
    }

    /// Generate executive summary from report data
    fn generate_executive_summary(report: &EvalReport) -> ExecutiveSummary {
        let pass_rate = report.pass_rate() * 100.0;
        let avg_quality = report.avg_quality();
        let avg_latency = report.avg_latency_ms();

        let pass_trend = if pass_rate >= 95.0 {
            "Excellent - Exceeds production target (≥95%)".to_string()
        } else if pass_rate >= 80.0 {
            format!("Good - Close to production target ({pass_rate:.1}% vs ≥95% target)")
        } else {
            format!("Needs Improvement - Below production target ({pass_rate:.1}% vs ≥95% target)")
        };

        let quality_trend = if avg_quality >= 0.95 {
            format!(
                "Excellent - High quality responses ({avg_quality:.3} vs ≥0.95 excellent threshold)"
            )
        } else if avg_quality >= 0.90 {
            format!("Good - Meets quality threshold ({avg_quality:.3} vs ≥0.90 target)")
        } else {
            format!("Below Target - Quality improvements needed ({avg_quality:.3} vs ≥0.90 target)")
        };

        let latency_trend = if avg_latency < 1000 {
            format!("Excellent - Sub-second latency ({avg_latency}ms)")
        } else if avg_latency < 3000 {
            format!("Good - Acceptable response time ({avg_latency}ms)")
        } else {
            format!("Slow - Performance optimization needed ({avg_latency}ms vs <3000ms target)")
        };

        // Generate comprehensive key insights with specific data points
        let mut key_insights = Vec::new();

        // Overview insight
        key_insights.push(format!(
            "OVERALL: {:.1}% pass rate ({}/{}), {:.3} avg quality, {}ms avg latency across {} evaluation runs",
            pass_rate, report.passed, report.total, avg_quality, avg_latency, report.total
        ));

        // Quality distribution insight with detailed breakdown
        let excellent = report
            .results
            .iter()
            .filter(|r| r.quality_score.overall >= 0.95)
            .count();
        let good = report
            .results
            .iter()
            .filter(|r| r.quality_score.overall >= 0.90 && r.quality_score.overall < 0.95)
            .count();
        let below = report
            .results
            .iter()
            .filter(|r| r.quality_score.overall < 0.90)
            .count();

        key_insights.push(format!(
            "QUALITY DISTRIBUTION: {} excellent (≥0.95, {:.0}%), {} good (0.90-0.95, {:.0}%), {} below threshold (<0.90, {:.0}%)",
            excellent, (excellent as f64 / report.total as f64) * 100.0,
            good, (good as f64 / report.total as f64) * 100.0,
            below, (below as f64 / report.total as f64) * 100.0
        ));

        // Dimension-specific insights
        let avg_accuracy = report
            .results
            .iter()
            .map(|r| r.quality_score.accuracy)
            .sum::<f64>()
            / report.total as f64;
        let avg_relevance = report
            .results
            .iter()
            .map(|r| r.quality_score.relevance)
            .sum::<f64>()
            / report.total as f64;
        let avg_safety = report
            .results
            .iter()
            .map(|r| r.quality_score.safety)
            .sum::<f64>()
            / report.total as f64;

        let dimensions = [
            ("accuracy", avg_accuracy),
            ("relevance", avg_relevance),
            ("safety", avg_safety),
        ];
        let min_dimension = dimensions
            .iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap();

        key_insights.push(format!(
            "DIMENSION ANALYSIS: Accuracy {:.3}, Relevance {:.3}, Safety {:.3} - Lowest: {} ({:.3})",
            avg_accuracy, avg_relevance, avg_safety, min_dimension.0, min_dimension.1
        ));

        // Failure analysis
        if report.failed > 0 {
            let failed_low_quality = report
                .results
                .iter()
                .filter(|r| !r.passed && r.quality_score.overall < 0.80)
                .count();
            let failed_safety = report
                .results
                .iter()
                .filter(|r| !r.passed && r.quality_score.safety < 0.95)
                .count();

            key_insights.push(format!(
                "FAILURE ANALYSIS: {} failed scenarios, {} with low quality (<0.80), {} with safety concerns (<0.95)",
                report.failed, failed_low_quality, failed_safety
            ));
        }

        // Performance analysis with percentiles
        let mut latencies: Vec<u64> = report.results.iter().map(|r| r.latency_ms).collect();
        latencies.sort_unstable();
        let p50 = Self::percentile(&latencies, 0.50);
        let p90 = Self::percentile(&latencies, 0.90);
        let p99 = Self::percentile(&latencies, 0.99);

        key_insights.push(format!(
            "PERFORMANCE DISTRIBUTION: P50={}ms, P90={}ms, P99={}ms - {}% of requests under 3 seconds",
            p50, p90, p99,
            (latencies.iter().filter(|&&l| l < 3000).count() as f64 / report.total as f64 * 100.0) as u32
        ));

        // Anomaly detection
        let high_retries = report
            .results
            .iter()
            .filter(|r| r.retry_attempts > 0)
            .count();
        if high_retries > 0 {
            key_insights.push(format!(
                "RELIABILITY: {} scenarios ({:.0}%) required retries - investigate retry patterns",
                high_retries,
                (high_retries as f64 / report.total as f64) * 100.0
            ));
        }

        ExecutiveSummary {
            pass_trend,
            quality_trend,
            latency_trend,
            key_insights,
        }
    }

    /// Generate actionable recommendations from report data
    fn generate_recommendations(report: &EvalReport) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        let pass_rate = report.pass_rate() * 100.0;
        let avg_quality = report.avg_quality();
        let avg_latency = report.avg_latency_ms();

        // Quality recommendations
        if avg_quality < 0.90 {
            recommendations.push(Recommendation {
                priority: "High".to_string(),
                category: "Quality".to_string(),
                description: format!(
                    "Average quality score ({avg_quality:.3}) is below target (0.90)"
                ),
                action: "Review failed scenarios and improve prompt engineering or model selection"
                    .to_string(),
                data_link: Some("#quality-distribution".to_string()),
                data_section: "Quality Distribution".to_string(),
            });
        }

        let below_threshold = report
            .results
            .iter()
            .filter(|r| r.quality_score.overall < 0.90)
            .count();
        if below_threshold > 0 && avg_quality >= 0.90 {
            recommendations.push(Recommendation {
                priority: "Medium".to_string(),
                category: "Quality".to_string(),
                description: format!(
                    "{} scenarios ({:.0}%) scored below quality threshold",
                    below_threshold,
                    (below_threshold as f64 / report.total as f64) * 100.0
                ),
                action: "Investigate low-scoring scenarios for common patterns or failure modes"
                    .to_string(),
                data_link: Some("#quality-distribution".to_string()),
                data_section: "Quality Distribution".to_string(),
            });
        }

        // Pass rate recommendations
        if pass_rate < 95.0 {
            let failed_scenarios: Vec<&str> = report
                .results
                .iter()
                .filter(|r| !r.passed)
                .take(3)
                .map(|r| r.scenario_id.as_str())
                .collect();

            recommendations.push(Recommendation {
                priority: if pass_rate < 80.0 { "High" } else { "Medium" }.to_string(),
                category: "Coverage".to_string(),
                description: format!(
                    "Pass rate ({pass_rate:.1}%) is below production target (≥95%)"
                ),
                action: if failed_scenarios.is_empty() {
                    "Review validation criteria and improve scenario coverage".to_string()
                } else {
                    format!(
                        "Focus on failed scenarios: {}{}",
                        failed_scenarios.join(", "),
                        if report.failed > 3 {
                            format!(" and {} more", report.failed - 3)
                        } else {
                            String::new()
                        }
                    )
                },
                data_link: Some("#scenario-results".to_string()),
                data_section: "Scenario Results".to_string(),
            });
        }

        // Performance recommendations
        if avg_latency > 3000 {
            recommendations.push(Recommendation {
                priority: "High".to_string(),
                category: "Performance".to_string(),
                description: format!(
                    "Average latency ({avg_latency}ms) exceeds acceptable threshold"
                ),
                action: "Consider caching, parallel processing, or faster model selection"
                    .to_string(),
                data_link: Some("#statistical-rigor".to_string()),
                data_section: "Statistical Rigor (Latency Percentiles)".to_string(),
            });
        } else if avg_latency > 2000 {
            recommendations.push(Recommendation {
                priority: "Medium".to_string(),
                category: "Performance".to_string(),
                description: format!("Latency ({avg_latency}ms) could be optimized"),
                action: "Profile slow scenarios and optimize retrieval or generation steps"
                    .to_string(),
                data_link: Some("#statistical-rigor".to_string()),
                data_section: "Statistical Rigor (Latency Percentiles)".to_string(),
            });
        }

        // Safety recommendations
        let safety_issues = report
            .results
            .iter()
            .filter(|r| r.quality_score.safety < 0.95)
            .count();
        if safety_issues > 0 {
            recommendations.push(Recommendation {
                priority: "High".to_string(),
                category: "Quality".to_string(),
                description: format!(
                    "{safety_issues} scenarios have safety concerns (safety score < 0.95)"
                ),
                action: "Review safety issues immediately and add guardrails or content filtering"
                    .to_string(),
                data_link: Some("#scenario-results".to_string()),
                data_section: "Scenario Results".to_string(),
            });
        }

        // If everything is great, add positive recommendation
        if recommendations.is_empty() {
            recommendations.push(Recommendation {
                priority: "Low".to_string(),
                category: "Quality".to_string(),
                description: "All metrics meet or exceed production targets".to_string(),
                action: "Continue monitoring and maintain current quality standards".to_string(),
                data_link: Some("#executive-summary".to_string()),
                data_section: "Executive Summary".to_string(),
            });
        }

        recommendations
    }
    /// Generate beautiful HTML report from evaluation results
    ///
    /// # Arguments
    ///
    /// * `report` - Evaluation report with all scenario results
    /// * `dataset_name` - Name of the evaluated dataset
    /// * `output_path` - Where to save the HTML file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_evals::report::html::HtmlReportGenerator;
    /// # use dashflow_evals::eval_runner::EvalReport;
    /// # fn example(report: EvalReport) -> anyhow::Result<()> {
    /// HtmlReportGenerator::generate(
    ///     &report,
    ///     "librarian",
    ///     "eval_report.html"
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate(
        report: &EvalReport,
        dataset_name: &str,
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let output_path = output_path.as_ref();

        // Calculate summary statistics
        let pass_rate = report.pass_rate() * 100.0;
        let pass_class = if pass_rate >= 95.0 {
            "pass"
        } else if pass_rate >= 80.0 {
            "warning"
        } else {
            "fail"
        }
        .to_string();

        let avg_quality = report.avg_quality();
        let quality_class = if avg_quality >= 0.95 {
            "pass"
        } else if avg_quality >= 0.90 {
            "warning"
        } else {
            "fail"
        }
        .to_string();

        let avg_latency = report.avg_latency_ms();

        // Calculate latency statistics
        let mut latencies: Vec<u64> = report.results.iter().map(|r| r.latency_ms).collect();
        latencies.sort_unstable();

        let min_latency = *latencies.first().unwrap_or(&0);
        let max_latency = *latencies.last().unwrap_or(&0);
        let p50_latency = Self::percentile(&latencies, 0.50);
        let p90_latency = Self::percentile(&latencies, 0.90);
        let p95_latency = Self::percentile(&latencies, 0.95);
        let p99_latency = Self::percentile(&latencies, 0.99);

        // Calculate confidence intervals
        let (pass_ci_lower, pass_ci_upper) =
            Self::confidence_interval_95(report.passed, report.total);

        // For quality CI, we need to calculate based on proportion of scenarios meeting threshold
        let quality_threshold_count = report
            .results
            .iter()
            .filter(|r| r.quality_score.overall >= 0.90)
            .count();
        let (quality_ci_lower, quality_ci_upper) =
            Self::confidence_interval_95(quality_threshold_count, report.total);

        // Calculate quality distribution
        let mut excellent_count = 0;
        let mut good_count = 0;
        let mut below_count = 0;

        for result in &report.results {
            let quality = result.quality_score.overall;
            if quality >= 0.95 {
                excellent_count += 1;
            } else if quality >= 0.90 {
                good_count += 1;
            } else {
                below_count += 1;
            }
        }

        // Build result rows
        let results: Vec<ResultRow> = report
            .results
            .iter()
            .map(|r| {
                let quality = r.quality_score.overall;
                let quality_class = if quality >= 0.95 {
                    "quality-excellent"
                } else if quality >= 0.90 {
                    "quality-good"
                } else if quality >= 0.80 {
                    "quality-fair"
                } else {
                    "quality-poor"
                }
                .to_string();

                ResultRow {
                    id: r.scenario_id.clone(),
                    status: if r.passed { "passed" } else { "failed" }.to_string(),
                    status_label: if r.passed { "PASS" } else { "FAIL" }.to_string(),
                    quality_score: format!("{quality:.3}"),
                    quality_percent: (quality * 100.0) as u32,
                    quality_class,
                    latency_ms: r.latency_ms,
                    retry_attempts: r.retry_attempts,
                    is_critical: !r.passed || quality < 0.80 || r.quality_score.safety < 0.95,
                    output: r.output.clone(),
                    accuracy: format!("{:.3}", r.quality_score.accuracy),
                    relevance: format!("{:.3}", r.quality_score.relevance),
                    completeness: format!("{:.3}", r.quality_score.completeness),
                    safety: format!("{:.3}", r.quality_score.safety),
                    coherence: format!("{:.3}", r.quality_score.coherence),
                    conciseness: format!("{:.3}", r.quality_score.conciseness),
                    reasoning: r.quality_score.reasoning.clone(),
                    issues: r.quality_score.issues.iter().map(IssueRow::from).collect(),
                    suggestions: r.quality_score.suggestions.clone(),
                    error: r.error.clone(),
                }
            })
            .collect();

        // Generate executive summary and recommendations
        let executive_summary = Self::generate_executive_summary(report);
        let recommendations = Self::generate_recommendations(report);

        // Create template context
        let template = ReportTemplate {
            dataset_name: dataset_name.to_string(),
            timestamp: report
                .metadata
                .started_at
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
            year: Utc::now().year(),
            total: report.total,
            passed: report.passed,
            pass_rate: format!("{pass_rate:.1}"),
            pass_rate_num: pass_rate,
            pass_class,
            avg_quality: format!("{avg_quality:.3}"),
            quality_class,
            avg_latency: format!("{avg_latency:.0}"),
            min_latency,
            max_latency,
            p50_latency,
            p90_latency,
            p95_latency,
            p99_latency,
            excellent_count,
            good_count,
            below_count,
            pass_rate_ci_lower: format!("{:.1}", pass_ci_lower * 100.0),
            pass_rate_ci_upper: format!("{:.1}", pass_ci_upper * 100.0),
            quality_ci_lower: format!("{:.1}", quality_ci_lower * 100.0),
            quality_ci_upper: format!("{:.1}", quality_ci_upper * 100.0),
            quality_threshold_met: quality_threshold_count,
            results,
            executive_summary,
            recommendations,
        };

        // Render template
        let html = template
            .render()
            .context("Failed to render HTML template")?;

        // Write to file
        fs::write(output_path, html)
            .with_context(|| format!("Failed to write HTML report to {output_path:?}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_runner::{EvalMetadata, ScenarioResult, ValidationResult};
    use crate::quality_judge::{QualityIssue, QualityScore};
    use chrono::Utc;
    use tempfile::TempDir;

    fn create_mock_result(id: &str, passed: bool, quality: f64) -> ScenarioResult {
        ScenarioResult {
            scenario_id: id.to_string(),
            passed,
            output: format!("Test output for {}", id),
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
                suggestions: vec!["Improve accuracy".to_string()],
            },
            latency_ms: 1500,
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
    fn test_html_generation() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("report.html");

        let report = EvalReport {
            total: 3,
            passed: 2,
            failed: 1,
            results: vec![
                create_mock_result("scenario_1", true, 0.95),
                create_mock_result("scenario_2", true, 0.92),
                create_mock_result("scenario_3", false, 0.85),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 5.0,
                config: "{}".to_string(),
            },
        };

        HtmlReportGenerator::generate(&report, "test_dataset", &output_path).unwrap();

        // Verify file was created
        assert!(output_path.exists());

        // Verify content
        let html = fs::read_to_string(&output_path).unwrap();
        assert!(html.contains("Evaluation Report"));
        assert!(html.contains("test_dataset"));
        assert!(html.contains("scenario_1"));
        assert!(html.contains("scenario_2"));
        assert!(html.contains("scenario_3"));
        assert!(html.contains("0.950")); // Quality score
    }

    #[test]
    fn test_quality_classification() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("report.html");

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

        HtmlReportGenerator::generate(&report, "quality_test", &output_path).unwrap();

        let html = fs::read_to_string(&output_path).unwrap();
        assert!(html.contains("quality-excellent"));
        assert!(html.contains("quality-good"));
        assert!(html.contains("quality-fair"));
        assert!(html.contains("quality-poor"));
    }

    #[test]
    fn test_issue_severity_display() {
        let issue = QualityIssue {
            dimension: "accuracy".to_string(),
            severity: IssueSeverity::Critical,
            description: "Critical accuracy issue".to_string(),
            example: Some("Example text".to_string()),
        };

        let row: IssueRow = (&issue).into();
        assert_eq!(row.severity_class, "critical");
        assert_eq!(row.dimension, "accuracy");
        assert_eq!(row.example, Some("Example text".to_string()));
    }
}
