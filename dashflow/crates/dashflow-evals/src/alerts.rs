//! Alert generation for evaluation regressions.
//!
//! This module provides tools to generate human-readable alerts from regression
//! detection results, with formatting optimized for Slack, GitHub comments, and email.

use crate::regression::{Regression, RegressionReport, RegressionType, Severity};
use serde::{Deserialize, Serialize};

/// Configuration for alert generation.
#[derive(Debug, Clone)]
pub struct AlertConfig {
    /// Generate alerts for quality drops
    pub alert_on_quality_drop: bool,

    /// Quality drop threshold for alerts
    pub quality_drop_threshold: f64,

    /// Generate alerts for performance regressions
    pub alert_on_latency_increase: bool,

    /// Latency increase threshold for alerts
    pub latency_threshold: f64,

    /// Generate alerts for cost increases
    pub alert_on_cost_increase: bool,

    /// Cost increase threshold for alerts
    pub cost_threshold: f64,

    /// Generate alerts for new failures
    pub alert_on_new_failures: bool,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            alert_on_quality_drop: true,
            quality_drop_threshold: 0.05,
            alert_on_latency_increase: true,
            latency_threshold: 0.20,
            alert_on_cost_increase: false,
            cost_threshold: 0.15,
            alert_on_new_failures: true,
        }
    }
}

/// Severity level for alerts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Critical issue - requires immediate attention
    Critical,

    /// Warning - should be investigated
    Warning,

    /// Informational - for awareness
    Info,
}

impl From<&Severity> for AlertSeverity {
    fn from(severity: &Severity) -> Self {
        match severity {
            Severity::Critical => AlertSeverity::Critical,
            Severity::Warning => AlertSeverity::Warning,
            Severity::Info => AlertSeverity::Info,
        }
    }
}

/// An alert generated from regression detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Severity level
    pub severity: AlertSeverity,

    /// Alert title
    pub title: String,

    /// Detailed description
    pub description: String,

    /// Evidence supporting the alert
    pub evidence: Vec<String>,

    /// Suggested actions to address the issue
    pub suggested_actions: Vec<String>,

    /// Pre-formatted Slack message (markdown)
    pub slack_message: Option<String>,

    /// Pre-formatted GitHub comment (markdown)
    pub github_comment: Option<String>,
}

/// Alert generator for creating notifications from regression reports.
pub struct AlertGenerator {
    config: AlertConfig,
}

impl AlertGenerator {
    /// Create a new alert generator with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AlertConfig::default(),
        }
    }

    /// Create a new alert generator with custom configuration.
    #[must_use]
    pub fn with_config(config: AlertConfig) -> Self {
        Self { config }
    }

    /// Generate alerts from a regression report.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_evals::{AlertGenerator, RegressionDetector, EvalReport, EvalMetadata};
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
    /// let regression_report = detector.detect_regressions(&baseline, &current);
    ///
    /// let generator = AlertGenerator::new();
    /// let alerts = generator.generate_alerts(&regression_report);
    ///
    /// for alert in &alerts {
    ///     if alert.severity == dashflow_evals::AlertSeverity::Critical {
    ///         println!("CRITICAL: {}", alert.title);
    ///         println!("{}", alert.description);
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn generate_alerts(&self, report: &RegressionReport) -> Vec<Alert> {
        let mut alerts = Vec::new();

        // Filter regressions based on config
        let relevant_regressions: Vec<&Regression> = report
            .regressions
            .iter()
            .filter(|r| self.should_alert_for_regression(r))
            .collect();

        if relevant_regressions.is_empty() {
            return alerts;
        }

        // Generate alerts for each regression
        for regression in relevant_regressions {
            let alert = self.create_alert(regression, report);
            alerts.push(alert);
        }

        // Sort by severity (critical first)
        alerts.sort_by(|a, b| a.severity.cmp(&b.severity));

        alerts
    }

    /// Check if we should generate an alert for this regression based on config.
    fn should_alert_for_regression(&self, regression: &Regression) -> bool {
        match regression.regression_type {
            RegressionType::QualityDrop | RegressionType::ScenarioQualityDrop => {
                self.config.alert_on_quality_drop
            }
            RegressionType::PerformanceRegression => self.config.alert_on_latency_increase,
            RegressionType::NewFailure | RegressionType::PassRateDecrease => {
                self.config.alert_on_new_failures
            }
        }
    }

    /// Create an alert from a regression.
    fn create_alert(&self, regression: &Regression, report: &RegressionReport) -> Alert {
        let severity = AlertSeverity::from(&regression.severity);
        let title = self.generate_title(regression);
        let description = regression.details.clone();
        let evidence = self.generate_evidence(regression);
        let suggested_actions = self.generate_suggested_actions(regression);
        let slack_message = Some(self.format_slack_message(regression, report));
        let github_comment = Some(self.format_github_comment(regression, report));

        Alert {
            severity,
            title,
            description,
            evidence,
            suggested_actions,
            slack_message,
            github_comment,
        }
    }

    /// Generate a concise title for the alert.
    fn generate_title(&self, regression: &Regression) -> String {
        match regression.regression_type {
            RegressionType::QualityDrop => "Overall Quality Regression Detected".to_string(),
            RegressionType::ScenarioQualityDrop => {
                format!(
                    "Quality Drop in Scenario '{}'",
                    regression.scenario_id.as_deref().unwrap_or("unknown")
                )
            }
            RegressionType::PerformanceRegression => "Performance Regression Detected".to_string(),
            RegressionType::NewFailure => {
                format!(
                    "New Test Failure: '{}'",
                    regression.scenario_id.as_deref().unwrap_or("unknown")
                )
            }
            RegressionType::PassRateDecrease => "Pass Rate Decreased".to_string(),
        }
    }

    /// Generate evidence list for the alert.
    fn generate_evidence(&self, regression: &Regression) -> Vec<String> {
        let mut evidence = Vec::new();

        if let (Some(baseline), Some(current)) =
            (regression.baseline_value, regression.current_value)
        {
            evidence.push(format!("Baseline value: {baseline:.3}"));
            evidence.push(format!("Current value: {current:.3}"));

            if let Some(change) = regression.absolute_change {
                evidence.push(format!("Absolute change: {change:.3}"));
            }

            if let Some(pct_change) = regression.relative_change {
                evidence.push(format!("Relative change: {:.1}%", pct_change * 100.0));
            }
        }

        evidence
    }

    /// Generate suggested actions to address the regression.
    fn generate_suggested_actions(&self, regression: &Regression) -> Vec<String> {
        let mut actions = Vec::new();

        match regression.regression_type {
            RegressionType::QualityDrop | RegressionType::ScenarioQualityDrop => {
                actions.push("Review recent changes to prompts or system behavior".to_string());
                actions.push("Check if retrieval quality has degraded".to_string());
                actions.push(
                    "Verify that context is being properly provided to the agent".to_string(),
                );
                actions.push("Consider rolling back recent changes".to_string());
            }
            RegressionType::PerformanceRegression => {
                actions.push("Profile the agent execution to identify bottlenecks".to_string());
                actions.push("Check for increased network latency or API throttling".to_string());
                actions.push(
                    "Review recent changes that may have added processing overhead".to_string(),
                );
            }
            RegressionType::NewFailure => {
                actions.push("Investigate why this test is now failing".to_string());
                actions.push("Check if the expected behavior has changed".to_string());
                actions.push("Verify that required tools/resources are available".to_string());
                actions.push("Review error messages and stack traces".to_string());
            }
            RegressionType::PassRateDecrease => {
                actions.push("Identify which scenarios are newly failing".to_string());
                actions.push("Look for common patterns in the failures".to_string());
                actions.push("Check for infrastructure issues or API outages".to_string());
            }
        }

        actions
    }

    /// Format alert as a Slack message.
    fn format_slack_message(&self, regression: &Regression, report: &RegressionReport) -> String {
        let emoji = match regression.severity {
            Severity::Critical => "🚨",
            Severity::Warning => "⚠️",
            Severity::Info => "ℹ️",
        };

        let mut msg = format!("{} *{}*\n\n", emoji, self.generate_title(regression));
        msg.push_str(&format!("*Details:* {}\n\n", regression.details));

        if !report.regressions.is_empty() {
            msg.push_str(&format!(
                "*Summary:* {} critical, {} warning, {} info regressions\n",
                report.summary.critical_count,
                report.summary.warning_count,
                report.summary.info_count
            ));
        }

        if let Some(commit) = &report.current_commit {
            msg.push_str(&format!("*Commit:* `{commit}`\n"));
        }

        msg
    }

    /// Format alert as a GitHub comment.
    fn format_github_comment(&self, regression: &Regression, report: &RegressionReport) -> String {
        let emoji = match regression.severity {
            Severity::Critical => "❌",
            Severity::Warning => "⚠️",
            Severity::Info => "ℹ️",
        };

        let mut comment = format!("## {} {}\n\n", emoji, self.generate_title(regression));
        comment.push_str(&format!("{}\n\n", regression.details));

        if let (Some(baseline), Some(current)) =
            (regression.baseline_value, regression.current_value)
        {
            comment.push_str("### Metrics\n\n");
            comment.push_str("| Metric | Baseline | Current | Change |\n");
            comment.push_str("|--------|----------|---------|--------|\n");

            let change_str = if let Some(pct) = regression.relative_change {
                format!("{:.1}%", pct * 100.0)
            } else {
                "N/A".to_string()
            };

            comment.push_str(&format!(
                "| Value | {baseline:.3} | {current:.3} | {change_str} |\n\n"
            ));
        }

        if !report.regressions.is_empty() {
            comment.push_str("### Summary\n\n");
            comment.push_str(&format!(
                "- {} critical regressions\n",
                report.summary.critical_count
            ));
            comment.push_str(&format!(
                "- {} warning regressions\n",
                report.summary.warning_count
            ));
            comment.push_str(&format!("- {} info items\n\n", report.summary.info_count));
        }

        let actions = self.generate_suggested_actions(regression);
        if !actions.is_empty() {
            comment.push_str("### Suggested Actions\n\n");
            for action in actions {
                comment.push_str(&format!("- {action}\n"));
            }
        }

        comment
    }
}

impl Default for AlertGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::regression::RegressionSummary;

    fn create_test_regression(regression_type: RegressionType, severity: Severity) -> Regression {
        Regression {
            regression_type,
            severity,
            details: "Test regression details".to_string(),
            scenario_id: Some("test_scenario".to_string()),
            baseline_value: Some(0.95),
            current_value: Some(0.85),
            absolute_change: Some(-0.10),
            relative_change: Some(-0.105),
        }
    }

    fn create_test_report(regressions: Vec<Regression>) -> RegressionReport {
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
            baseline_commit: Some("abc123".to_string()),
            current_commit: Some("def456".to_string()),
            statistically_significant: true,
            p_value: Some(0.01),
            summary: RegressionSummary {
                critical_count,
                warning_count,
                info_count,
                baseline_avg_quality: 0.95,
                current_avg_quality: 0.85,
                quality_change: -0.10,
                baseline_avg_latency: 100,
                current_avg_latency: 120,
                latency_change_percent: 0.20,
            },
        }
    }

    #[test]
    fn test_generate_alerts_for_quality_drop() {
        let generator = AlertGenerator::new();

        let regression = create_test_regression(RegressionType::QualityDrop, Severity::Critical);
        let report = create_test_report(vec![regression]);

        let alerts = generator.generate_alerts(&report);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
        assert!(alerts[0].title.contains("Quality Regression"));
    }

    #[test]
    fn test_generate_alerts_for_new_failure() {
        let generator = AlertGenerator::new();

        let regression = create_test_regression(RegressionType::NewFailure, Severity::Critical);
        let report = create_test_report(vec![regression]);

        let alerts = generator.generate_alerts(&report);

        assert_eq!(alerts.len(), 1);
        assert!(alerts[0].title.contains("New Test Failure"));
        assert!(alerts[0].title.contains("test_scenario"));
    }

    #[test]
    fn test_alert_with_suggested_actions() {
        let generator = AlertGenerator::new();

        let regression = create_test_regression(RegressionType::QualityDrop, Severity::Critical);
        let report = create_test_report(vec![regression]);

        let alerts = generator.generate_alerts(&report);

        assert!(!alerts[0].suggested_actions.is_empty());
        assert!(alerts[0].suggested_actions[0].contains("prompt"));
    }

    #[test]
    fn test_slack_message_formatting() {
        let generator = AlertGenerator::new();

        let regression = create_test_regression(RegressionType::QualityDrop, Severity::Critical);
        let report = create_test_report(vec![regression]);

        let alerts = generator.generate_alerts(&report);

        let slack_msg = alerts[0].slack_message.as_ref().unwrap();
        assert!(slack_msg.contains("🚨")); // Critical emoji
        assert!(slack_msg.contains("*")); // Bold formatting
    }

    #[test]
    fn test_github_comment_formatting() {
        let generator = AlertGenerator::new();

        let regression = create_test_regression(RegressionType::QualityDrop, Severity::Critical);
        let report = create_test_report(vec![regression]);

        let alerts = generator.generate_alerts(&report);

        let github_comment = alerts[0].github_comment.as_ref().unwrap();
        assert!(github_comment.contains("##")); // Heading
        assert!(github_comment.contains("|")); // Table
    }

    #[test]
    fn test_alert_config_filtering() {
        let config = AlertConfig {
            alert_on_quality_drop: false, // Disable quality alerts
            ..Default::default()
        };

        let generator = AlertGenerator::with_config(config);

        let regression = create_test_regression(RegressionType::QualityDrop, Severity::Critical);
        let report = create_test_report(vec![regression]);

        let alerts = generator.generate_alerts(&report);

        // Should be filtered out by config
        assert_eq!(alerts.len(), 0);
    }

    #[test]
    fn test_multiple_alerts_sorted_by_severity() {
        let generator = AlertGenerator::new();

        let regressions = vec![
            create_test_regression(RegressionType::PerformanceRegression, Severity::Info),
            create_test_regression(RegressionType::QualityDrop, Severity::Critical),
            create_test_regression(RegressionType::ScenarioQualityDrop, Severity::Warning),
        ];
        let report = create_test_report(regressions);

        let alerts = generator.generate_alerts(&report);

        assert_eq!(alerts.len(), 3);
        // Critical should be first
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
        // Warning second
        assert_eq!(alerts[1].severity, AlertSeverity::Warning);
        // Info last
        assert_eq!(alerts[2].severity, AlertSeverity::Info);
    }
}
