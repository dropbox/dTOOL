//! Slack Notification Integration
//!
//! Send evaluation results to Slack channels via incoming webhooks.
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_evals::notifications::{SlackNotifier, SlackConfig};
//! use dashflow_evals::eval_runner::EvalReport;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = SlackConfig {
//!     webhook_url: "https://hooks.slack.com/services/YOUR/WEBHOOK/URL".to_string(),
//!     channel: Some("#evals".to_string()),
//!     username: Some("DashFlow Evals Bot".to_string()),
//!     icon_emoji: Some(":robot_face:".to_string()),
//! };
//!
//! let notifier = SlackNotifier::new(config);
//!
//! # let report = todo!();
//! notifier.notify_success(&report).await?;
//! # Ok(())
//! # }
//! ```

use crate::eval_runner::EvalReport;
use crate::regression::RegressionReport;
use anyhow::{Context, Result};
use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Slack notification configuration
#[derive(Debug, Clone)]
pub struct SlackConfig {
    /// Slack incoming webhook URL
    pub webhook_url: String,

    /// Optional channel override (e.g., "#evals")
    pub channel: Option<String>,

    /// Optional username override
    pub username: Option<String>,

    /// Optional icon emoji (e.g., ":`robot_face`:")
    pub icon_emoji: Option<String>,
}

impl SlackConfig {
    /// Create a new Slack config with just the webhook URL
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            channel: None,
            username: None,
            icon_emoji: None,
        }
    }

    /// Set the channel
    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    /// Set the username
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Set the icon emoji
    pub fn with_icon_emoji(mut self, icon: impl Into<String>) -> Self {
        self.icon_emoji = Some(icon.into());
        self
    }
}

/// Slack notifier
#[derive(Debug, Clone)]
pub struct SlackNotifier {
    config: SlackConfig,
    client: reqwest::Client,
}

impl SlackNotifier {
    /// Create a new Slack notifier
    #[must_use]
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            client: create_http_client(),
        }
    }

    /// Notify about successful evaluation
    pub async fn notify_success(&self, report: &EvalReport) -> Result<()> {
        let message = self.format_success_message(report);
        self.send_message(&message).await
    }

    /// Notify about failed evaluation
    pub async fn notify_failure(&self, report: &EvalReport) -> Result<()> {
        let message = self.format_failure_message(report);
        self.send_message(&message).await
    }

    /// Notify about regression detection
    pub async fn notify_regression(
        &self,
        report: &EvalReport,
        regression: &RegressionReport,
    ) -> Result<()> {
        let message = self.format_regression_message(report, regression);
        self.send_message(&message).await
    }

    /// Send a custom message
    pub async fn send_message(&self, message: &SlackMessage) -> Result<()> {
        let payload = self.build_payload(message);

        let response = self
            .client
            .post(&self.config.webhook_url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send Slack notification")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Slack API error: {status} - {body}");
        }

        Ok(())
    }

    /// Format success message
    fn format_success_message(&self, report: &EvalReport) -> SlackMessage {
        let pass_rate = report.pass_rate() * 100.0;
        let quality = report.avg_quality();

        SlackMessage {
            text: format!("✅ Evaluation Passed: {pass_rate:.1}% pass rate, {quality:.3} quality"),
            blocks: vec![
                SlackBlock::Section {
                    text: SlackText {
                        type_: "mrkdwn".to_string(),
                        text: format!("*✅ Evaluation Passed*\n\n*Summary:*\n• Pass Rate: {:.1}% ({}/{})\n• Quality: {:.3}\n• Latency: {}ms",
                            pass_rate, report.passed, report.total, quality, report.avg_latency_ms()
                        ),
                    },
                },
                SlackBlock::Divider,
                SlackBlock::Context {
                    elements: vec![
                        SlackText {
                            type_: "mrkdwn".to_string(),
                            text: format!("Duration: {:.1}s", report.metadata.duration_secs),
                        },
                    ],
                },
            ],
            attachments: vec![],
        }
    }

    /// Format failure message
    fn format_failure_message(&self, report: &EvalReport) -> SlackMessage {
        let pass_rate = report.pass_rate() * 100.0;
        let quality = report.avg_quality();

        // Get failed scenarios
        let failed_scenarios: Vec<String> = report
            .results
            .iter()
            .filter(|r| !r.passed)
            .take(5)
            .map(|r| {
                format!(
                    "• {}: {}",
                    r.scenario_id,
                    r.error.as_deref().unwrap_or("Unknown")
                )
            })
            .collect();

        let failures_text = if failed_scenarios.is_empty() {
            "No failures".to_string()
        } else {
            let mut text = failed_scenarios.join("\n");
            if report.failed > 5 {
                text.push_str(&format!("\n_...and {} more_", report.failed - 5));
            }
            text
        };

        SlackMessage {
            text: format!("❌ Evaluation Failed: {pass_rate:.1}% pass rate, {quality:.3} quality"),
            blocks: vec![
                SlackBlock::Section {
                    text: SlackText {
                        type_: "mrkdwn".to_string(),
                        text: format!("*❌ Evaluation Failed*\n\n*Summary:*\n• Pass Rate: {:.1}% ({}/{})\n• Quality: {:.3}\n• Latency: {}ms",
                            pass_rate, report.passed, report.total, quality, report.avg_latency_ms()
                        ),
                    },
                },
                SlackBlock::Section {
                    text: SlackText {
                        type_: "mrkdwn".to_string(),
                        text: format!("*Failed Scenarios:*\n{failures_text}"),
                    },
                },
                SlackBlock::Divider,
                SlackBlock::Context {
                    elements: vec![
                        SlackText {
                            type_: "mrkdwn".to_string(),
                            text: format!("Duration: {:.1}s", report.metadata.duration_secs),
                        },
                    ],
                },
            ],
            attachments: vec![
                SlackAttachment {
                    color: "danger".to_string(),
                    text: None,
                    fields: vec![],
                },
            ],
        }
    }

    /// Format regression message
    fn format_regression_message(
        &self,
        report: &EvalReport,
        regression: &RegressionReport,
    ) -> SlackMessage {
        let regressions_text: String = regression
            .regressions
            .iter()
            .take(5)
            .map(|r| format!("• {:?}: {}", r.regression_type, r.details))
            .collect::<Vec<_>>()
            .join("\n");

        SlackMessage {
            text: format!(
                "⚠️ Regression Detected: {} issue(s)",
                regression.regressions.len()
            ),
            blocks: vec![
                SlackBlock::Section {
                    text: SlackText {
                        type_: "mrkdwn".to_string(),
                        text: format!(
                            "*⚠️ Regression Detected*\n\n{} regression(s) found",
                            regression.regressions.len()
                        ),
                    },
                },
                SlackBlock::Section {
                    text: SlackText {
                        type_: "mrkdwn".to_string(),
                        text: format!("*Issues:*\n{regressions_text}"),
                    },
                },
                SlackBlock::Divider,
                SlackBlock::Context {
                    elements: vec![SlackText {
                        type_: "mrkdwn".to_string(),
                        text: format!(
                            "Pass Rate: {:.1}% | Quality: {:.3}",
                            report.pass_rate() * 100.0,
                            report.avg_quality()
                        ),
                    }],
                },
            ],
            attachments: vec![SlackAttachment {
                color: "warning".to_string(),
                text: None,
                fields: vec![],
            }],
        }
    }

    /// Build Slack webhook payload
    fn build_payload(&self, message: &SlackMessage) -> serde_json::Value {
        let mut payload = json!({
            "text": message.text,
            "blocks": message.blocks,
        });

        if !message.attachments.is_empty() {
            payload["attachments"] = json!(message.attachments);
        }

        if let Some(channel) = &self.config.channel {
            payload["channel"] = json!(channel);
        }

        if let Some(username) = &self.config.username {
            payload["username"] = json!(username);
        }

        if let Some(icon) = &self.config.icon_emoji {
            payload["icon_emoji"] = json!(icon);
        }

        payload
    }
}

/// Slack message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessage {
    /// Plain text fallback
    pub text: String,

    /// Rich message blocks
    pub blocks: Vec<SlackBlock>,

    /// Optional attachments
    pub attachments: Vec<SlackAttachment>,
}

/// Slack block
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlackBlock {
    Section { text: SlackText },
    Divider,
    Context { elements: Vec<SlackText> },
}

/// Slack text element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackText {
    #[serde(rename = "type")]
    pub type_: String, // "mrkdwn" or "plain_text"
    pub text: String,
}

/// Slack attachment (legacy format, used for color coding)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAttachment {
    pub color: String, // "good", "warning", "danger", or hex color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<SlackField>,
}

/// Slack attachment field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackField {
    pub title: String,
    pub value: String,
    pub short: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eval_runner::{EvalMetadata, ScenarioResult, ValidationResult},
        quality_judge::QualityScore,
    };
    use chrono::Utc;

    fn create_test_report(passed: usize, failed: usize) -> EvalReport {
        let mut results = Vec::new();

        for i in 0..passed {
            results.push(ScenarioResult {
                scenario_id: format!("passed_{}", i),
                passed: true,
                output: "success".to_string(),
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
        }

        for i in 0..failed {
            results.push(ScenarioResult {
                scenario_id: format!("failed_{}", i),
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
                    failure_reason: Some("Missing content".to_string()),
                },
                error: Some("Quality too low".to_string()),
                retry_attempts: 0,
                timestamp: Utc::now(),
                input: None,
                tokens_used: None,
                cost_usd: None,
            });
        }

        EvalReport {
            total: passed + failed,
            passed,
            failed,
            results,
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 10.0,
                config: "{}".to_string(),
            },
        }
    }

    #[test]
    fn test_format_success_message() {
        let config = SlackConfig::new("https://example.com/webhook");
        let notifier = SlackNotifier::new(config);
        let report = create_test_report(19, 1);

        let message = notifier.format_success_message(&report);

        assert!(message.text.contains("95.0%"));
        assert!(message.text.contains("✅"));
        assert!(!message.blocks.is_empty());
    }

    #[test]
    fn test_format_failure_message() {
        let config = SlackConfig::new("https://example.com/webhook");
        let notifier = SlackNotifier::new(config);
        let report = create_test_report(15, 5);

        let message = notifier.format_failure_message(&report);

        assert!(message.text.contains("75.0%"));
        assert!(message.text.contains("❌"));
        assert!(!message.blocks.is_empty());
        assert_eq!(message.attachments[0].color, "danger");
    }

    #[test]
    fn test_slack_config_builder() {
        let config = SlackConfig::new("https://example.com/webhook")
            .with_channel("#evals")
            .with_username("Eval Bot")
            .with_icon_emoji(":robot_face:");

        assert_eq!(config.webhook_url, "https://example.com/webhook");
        assert_eq!(config.channel.as_deref(), Some("#evals"));
        assert_eq!(config.username.as_deref(), Some("Eval Bot"));
        assert_eq!(config.icon_emoji.as_deref(), Some(":robot_face:"));
    }
}
