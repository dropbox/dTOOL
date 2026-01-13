//! Quality Monitor for Continuous Regression Detection
//!
//! This module provides continuous quality monitoring to detect regressions within 1 hour
//! (M-301). It orchestrates baseline management, regression detection, alerting, and
//! Prometheus metrics emission.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     QualityMonitor                               │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  1. Load latest eval results (from file or EventStore)         │
//! │  2. Compare against baseline using RegressionDetector          │
//! │  3. Generate alerts via AlertGenerator                          │
//! │  4. Send notifications (Slack, email, PagerDuty)               │
//! │  5. Emit Prometheus metrics for dashboards                      │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ## One-shot regression check
//!
//! ```rust,no_run
//! use dashflow_evals::monitor::{QualityMonitor, MonitorConfig};
//! use std::path::Path;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = MonitorConfig::default()
//!     .with_app_name("librarian")
//!     .with_baseline_dir(Path::new("baselines"));
//!
//! let monitor = QualityMonitor::new(config);
//!
//! // Check current results against baseline
//! let result = monitor.check_regression(
//!     "main",  // baseline name
//!     Path::new("target/eval_results/latest.json"),
//! ).await?;
//!
//! if result.has_regressions {
//!     eprintln!("Regressions detected!");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Continuous monitoring (checks every 15 minutes)
//!
//! ```rust,no_run
//! use dashflow_evals::monitor::{QualityMonitor, MonitorConfig};
//! use std::path::Path;
//! use std::time::Duration;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = MonitorConfig::default()
//!     .with_app_name("librarian")
//!     .with_check_interval(Duration::from_secs(15 * 60));
//!
//! let monitor = QualityMonitor::new(config);
//! monitor.start_continuous_monitoring().await?;
//! # Ok(())
//! # }
//! ```

use crate::{
    alerts::{Alert, AlertConfig, AlertGenerator, AlertSeverity},
    baseline::BaselineStore,
    eval_runner::EvalReport,
    notifications::{SlackConfig, SlackNotifier},
    regression::{RegressionConfig, RegressionDetector, RegressionReport},
};
use anyhow::{Context, Result};
use dashflow::core::config_loader::env_vars::{env_string, DASHFLOW_INSTANCE_ID};
use chrono::{DateTime, Utc};
use prometheus::{
    register_gauge_vec_with_registry, register_int_counter_vec_with_registry,
    register_int_gauge_vec_with_registry, GaugeVec, IntCounterVec, IntGaugeVec, Registry,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Configuration for the quality monitor.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Application name (e.g., "librarian")
    pub app_name: String,

    /// Directory for storing baselines
    pub baseline_dir: PathBuf,

    /// Directory to watch for new eval results
    pub results_dir: PathBuf,

    /// Interval between regression checks (default: 15 minutes)
    pub check_interval: Duration,

    /// Slack webhook URL for notifications
    pub slack_webhook_url: Option<String>,

    /// Slack channel for alerts
    pub slack_channel: Option<String>,

    /// Alert configuration
    pub alert_config: AlertConfig,

    /// Regression detection configuration
    pub regression_config: RegressionConfig,

    /// Enable Prometheus metrics
    pub enable_metrics: bool,

    /// Instance ID for metrics labeling
    pub instance_id: String,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            app_name: "default".to_string(),
            baseline_dir: PathBuf::from("baselines"),
            results_dir: PathBuf::from("target/eval_results"),
            check_interval: Duration::from_secs(15 * 60), // 15 minutes
            slack_webhook_url: None,
            slack_channel: None,
            alert_config: AlertConfig::default(),
            regression_config: RegressionConfig::default(),
            enable_metrics: true,
            instance_id: env_string(DASHFLOW_INSTANCE_ID)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        }
    }
}

impl MonitorConfig {
    /// Set the application name
    #[must_use]
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = name.into();
        self
    }

    /// Set the baseline directory
    #[must_use]
    pub fn with_baseline_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.baseline_dir = path.as_ref().to_path_buf();
        self
    }

    /// Set the results directory to watch
    #[must_use]
    pub fn with_results_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.results_dir = path.as_ref().to_path_buf();
        self
    }

    /// Set the check interval
    #[must_use]
    pub fn with_check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }

    /// Configure Slack notifications
    #[must_use]
    pub fn with_slack(mut self, webhook_url: impl Into<String>, channel: impl Into<String>) -> Self {
        self.slack_webhook_url = Some(webhook_url.into());
        self.slack_channel = Some(channel.into());
        self
    }

    /// Set the alert configuration
    #[must_use]
    pub fn with_alert_config(mut self, config: AlertConfig) -> Self {
        self.alert_config = config;
        self
    }

    /// Set the regression detection configuration
    #[must_use]
    pub fn with_regression_config(mut self, config: RegressionConfig) -> Self {
        self.regression_config = config;
        self
    }

    /// Set the instance ID
    #[must_use]
    pub fn with_instance_id(mut self, id: impl Into<String>) -> Self {
        self.instance_id = id.into();
        self
    }
}

/// Result of a regression check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionCheckResult {
    /// Whether any regressions were detected
    pub has_regressions: bool,

    /// Whether any critical regressions were detected
    pub has_critical_regressions: bool,

    /// Number of critical regressions
    pub critical_count: usize,

    /// Number of warning regressions
    pub warning_count: usize,

    /// Number of info-level regressions
    pub info_count: usize,

    /// Timestamp of the check
    pub checked_at: DateTime<Utc>,

    /// Baseline name used for comparison
    pub baseline_name: String,

    /// Current quality score
    pub current_quality: f64,

    /// Baseline quality score
    pub baseline_quality: f64,

    /// Quality change (current - baseline)
    pub quality_change: f64,

    /// Current pass rate
    pub current_pass_rate: f64,

    /// Baseline pass rate
    pub baseline_pass_rate: f64,

    /// Generated alerts
    pub alerts: Vec<Alert>,

    /// Full regression report (for detailed analysis)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regression_report: Option<RegressionReport>,
}

/// Prometheus metrics for quality monitoring.
pub struct MonitorMetrics {
    /// Quality score gauge (by app, baseline)
    pub quality_score: GaugeVec,

    /// Pass rate gauge (by app, baseline)
    pub pass_rate: GaugeVec,

    /// Regression count gauge (by app, baseline, severity)
    pub regression_count: IntGaugeVec,

    /// Total checks counter (by app)
    pub checks_total: IntCounterVec,

    /// Regressions detected counter (by app, severity)
    pub regressions_detected_total: IntCounterVec,

    /// Alerts sent counter (by app, channel)
    pub alerts_sent_total: IntCounterVec,

    /// Last check timestamp gauge (by app)
    pub last_check_timestamp: GaugeVec,

    /// Check duration gauge (by app)
    pub check_duration_seconds: GaugeVec,
}

impl MonitorMetrics {
    /// Create a new metrics set with the default Prometheus registry.
    pub fn new() -> Result<Self> {
        Self::with_registry(prometheus::default_registry())
    }

    /// Create a new metrics set with a custom registry.
    pub fn with_registry(registry: &Registry) -> Result<Self> {
        let quality_score = register_gauge_vec_with_registry!(
            "dashflow_quality_score",
            "Current quality score (0-1)",
            &["app", "baseline", "instance_id"],
            registry
        )?;

        let pass_rate = register_gauge_vec_with_registry!(
            "dashflow_pass_rate",
            "Current pass rate (0-1)",
            &["app", "baseline", "instance_id"],
            registry
        )?;

        let regression_count = register_int_gauge_vec_with_registry!(
            "dashflow_regression_count",
            "Number of active regressions",
            &["app", "baseline", "severity", "instance_id"],
            registry
        )?;

        let checks_total = register_int_counter_vec_with_registry!(
            "dashflow_quality_checks_total",
            "Total number of quality checks performed",
            &["app", "instance_id"],
            registry
        )?;

        let regressions_detected_total = register_int_counter_vec_with_registry!(
            "dashflow_regressions_detected_total",
            "Total number of regressions detected",
            &["app", "severity", "instance_id"],
            registry
        )?;

        let alerts_sent_total = register_int_counter_vec_with_registry!(
            "dashflow_alerts_sent_total",
            "Total number of alerts sent",
            &["app", "channel", "instance_id"],
            registry
        )?;

        let last_check_timestamp = register_gauge_vec_with_registry!(
            "dashflow_last_quality_check_timestamp",
            "Unix timestamp of last quality check",
            &["app", "instance_id"],
            registry
        )?;

        let check_duration_seconds = register_gauge_vec_with_registry!(
            "dashflow_quality_check_duration_seconds",
            "Duration of last quality check in seconds",
            &["app", "instance_id"],
            registry
        )?;

        Ok(Self {
            quality_score,
            pass_rate,
            regression_count,
            checks_total,
            regressions_detected_total,
            alerts_sent_total,
            last_check_timestamp,
            check_duration_seconds,
        })
    }
}

impl Default for MonitorMetrics {
    fn default() -> Self {
        Self::new().expect("Failed to create default monitor metrics")
    }
}

/// Quality monitor for continuous regression detection.
pub struct QualityMonitor {
    config: MonitorConfig,
    baseline_store: BaselineStore,
    regression_detector: RegressionDetector,
    alert_generator: AlertGenerator,
    slack_notifier: Option<SlackNotifier>,
    metrics: Option<MonitorMetrics>,
    last_check: Arc<RwLock<Option<RegressionCheckResult>>>,
}

impl QualityMonitor {
    /// Create a new quality monitor.
    pub fn new(config: MonitorConfig) -> Self {
        let baseline_store = BaselineStore::new(&config.baseline_dir);
        let regression_detector = RegressionDetector::with_config(config.regression_config.clone());
        let alert_generator = AlertGenerator::with_config(config.alert_config.clone());

        let slack_notifier = config.slack_webhook_url.as_ref().map(|url| {
            let slack_config = SlackConfig::new(url)
                .with_channel(config.slack_channel.clone().unwrap_or_else(|| "#evals".to_string()))
                .with_username("DashFlow Quality Monitor")
                .with_icon_emoji(":robot_face:");
            SlackNotifier::new(slack_config)
        });

        let metrics = if config.enable_metrics {
            MonitorMetrics::new().ok()
        } else {
            None
        };

        Self {
            config,
            baseline_store,
            regression_detector,
            alert_generator,
            slack_notifier,
            metrics,
            last_check: Arc::new(RwLock::new(None)),
        }
    }

    /// Save current evaluation results as a baseline.
    pub fn save_baseline(
        &self,
        name: &str,
        report: &EvalReport,
        description: Option<&str>,
    ) -> Result<()> {
        let git_commit = BaselineStore::current_git_commit();
        let git_author = BaselineStore::current_git_author();

        self.baseline_store.save_baseline(
            name,
            report,
            git_commit.as_deref(),
            git_author.as_deref(),
            description,
            &self.config.app_name,
        )?;

        info!(
            app = %self.config.app_name,
            baseline = name,
            scenarios = report.total,
            quality = %format!("{:.3}", report.avg_quality()),
            "Saved baseline"
        );

        Ok(())
    }

    /// List available baselines for this app.
    pub fn list_baselines(&self) -> Result<Vec<crate::baseline::BaselineMetadata>> {
        self.baseline_store.list_baselines(&self.config.app_name)
    }

    /// Load a baseline by name.
    pub fn load_baseline(&self, name: &str) -> Result<EvalReport> {
        self.baseline_store.load_baseline(name, &self.config.app_name)
    }

    /// Check for regressions against a baseline.
    ///
    /// This is the main entry point for one-shot regression checking.
    pub async fn check_regression(
        &self,
        baseline_name: &str,
        current_results_path: &Path,
    ) -> Result<RegressionCheckResult> {
        let start = std::time::Instant::now();

        // Load current results
        let current_content = tokio::fs::read_to_string(current_results_path)
            .await
            .with_context(|| format!("Failed to read results from {:?}", current_results_path))?;

        let current: EvalReport = serde_json::from_str(&current_content)
            .with_context(|| "Failed to parse evaluation results")?;

        // Load baseline
        let baseline = self.load_baseline(baseline_name)?;

        // Perform the check
        let result = self.check_regression_internal(baseline_name, &baseline, &current).await?;

        // Record duration
        let duration = start.elapsed();
        if let Some(ref metrics) = self.metrics {
            metrics
                .check_duration_seconds
                .with_label_values(&[&self.config.app_name, &self.config.instance_id])
                .set(duration.as_secs_f64());
        }

        Ok(result)
    }

    /// Check for regressions with pre-loaded reports.
    pub async fn check_regression_with_reports(
        &self,
        baseline_name: &str,
        baseline: &EvalReport,
        current: &EvalReport,
    ) -> Result<RegressionCheckResult> {
        self.check_regression_internal(baseline_name, baseline, current).await
    }

    /// Internal regression check implementation.
    async fn check_regression_internal(
        &self,
        baseline_name: &str,
        baseline: &EvalReport,
        current: &EvalReport,
    ) -> Result<RegressionCheckResult> {
        let checked_at = Utc::now();

        // Detect regressions
        let regression_report = self.regression_detector.detect_regressions(baseline, current);

        // Generate alerts
        let alerts = self.alert_generator.generate_alerts(&regression_report);

        // Build result
        let result = RegressionCheckResult {
            has_regressions: regression_report.has_regressions(),
            has_critical_regressions: regression_report.has_critical_regressions(),
            critical_count: regression_report.summary.critical_count,
            warning_count: regression_report.summary.warning_count,
            info_count: regression_report.summary.info_count,
            checked_at,
            baseline_name: baseline_name.to_string(),
            current_quality: current.avg_quality(),
            baseline_quality: baseline.avg_quality(),
            quality_change: regression_report.summary.quality_change,
            current_pass_rate: current.pass_rate(),
            baseline_pass_rate: baseline.pass_rate(),
            alerts: alerts.clone(),
            regression_report: Some(regression_report.clone()),
        };

        // Update metrics
        self.update_metrics(baseline_name, &result);

        // Send notifications if regressions detected
        if result.has_regressions {
            self.send_notifications(current, &regression_report, &alerts).await?;
        }

        // Store last check result
        *self.last_check.write().await = Some(result.clone());

        info!(
            app = %self.config.app_name,
            baseline = baseline_name,
            has_regressions = result.has_regressions,
            critical = result.critical_count,
            warning = result.warning_count,
            quality_change = %format!("{:.3}", result.quality_change),
            "Regression check completed"
        );

        Ok(result)
    }

    /// Update Prometheus metrics from check result.
    fn update_metrics(&self, baseline_name: &str, result: &RegressionCheckResult) {
        let Some(ref metrics) = self.metrics else {
            return;
        };

        let app = &self.config.app_name;
        let instance = &self.config.instance_id;

        // Quality and pass rate
        metrics
            .quality_score
            .with_label_values(&[app, baseline_name, instance])
            .set(result.current_quality);
        metrics
            .pass_rate
            .with_label_values(&[app, baseline_name, instance])
            .set(result.current_pass_rate);

        // Regression counts by severity
        metrics
            .regression_count
            .with_label_values(&[app, baseline_name, "critical", instance])
            .set(result.critical_count as i64);
        metrics
            .regression_count
            .with_label_values(&[app, baseline_name, "warning", instance])
            .set(result.warning_count as i64);
        metrics
            .regression_count
            .with_label_values(&[app, baseline_name, "info", instance])
            .set(result.info_count as i64);

        // Increment counters
        metrics
            .checks_total
            .with_label_values(&[app, instance])
            .inc();

        if result.critical_count > 0 {
            metrics
                .regressions_detected_total
                .with_label_values(&[app, "critical", instance])
                .inc_by(result.critical_count as u64);
        }
        if result.warning_count > 0 {
            metrics
                .regressions_detected_total
                .with_label_values(&[app, "warning", instance])
                .inc_by(result.warning_count as u64);
        }

        // Last check timestamp
        metrics
            .last_check_timestamp
            .with_label_values(&[app, instance])
            .set(result.checked_at.timestamp() as f64);
    }

    /// Send notifications for detected regressions.
    async fn send_notifications(
        &self,
        report: &EvalReport,
        regression: &RegressionReport,
        alerts: &[Alert],
    ) -> Result<()> {
        // Send Slack notification
        if let Some(ref notifier) = self.slack_notifier {
            if let Err(e) = notifier.notify_regression(report, regression).await {
                error!(error = %e, "Failed to send Slack notification");
            } else {
                info!(app = %self.config.app_name, "Sent Slack notification for regression");

                if let Some(ref metrics) = self.metrics {
                    metrics
                        .alerts_sent_total
                        .with_label_values(&[&self.config.app_name, "slack", &self.config.instance_id])
                        .inc();
                }
            }
        }

        // Log alerts at appropriate level
        for alert in alerts {
            match alert.severity {
                AlertSeverity::Critical => error!(
                    app = %self.config.app_name,
                    title = %alert.title,
                    "CRITICAL regression detected"
                ),
                AlertSeverity::Warning => warn!(
                    app = %self.config.app_name,
                    title = %alert.title,
                    "Warning: regression detected"
                ),
                AlertSeverity::Info => info!(
                    app = %self.config.app_name,
                    title = %alert.title,
                    "Info: regression detected"
                ),
            }
        }

        Ok(())
    }

    /// Start continuous monitoring.
    ///
    /// This runs indefinitely, checking for regressions at the configured interval.
    pub async fn start_continuous_monitoring(&self) -> Result<()> {
        info!(
            app = %self.config.app_name,
            interval_secs = self.config.check_interval.as_secs(),
            "Starting continuous quality monitoring"
        );

        loop {
            // Find latest results file
            if let Some(latest_path) = self.find_latest_results().await? {
                // Get the default baseline name (usually "main")
                let baseline_name = "main";

                match self.check_regression(baseline_name, &latest_path).await {
                    Ok(result) => {
                        if result.has_critical_regressions {
                            error!(
                                app = %self.config.app_name,
                                critical = result.critical_count,
                                "Critical regressions detected!"
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            app = %self.config.app_name,
                            error = %e,
                            "Regression check failed"
                        );
                    }
                }
            } else {
                info!(
                    app = %self.config.app_name,
                    dir = %self.config.results_dir.display(),
                    "No results files found"
                );
            }

            // Wait for next check
            tokio::time::sleep(self.config.check_interval).await;
        }
    }

    /// Find the most recent results file in the results directory.
    async fn find_latest_results(&self) -> Result<Option<PathBuf>> {
        if !self.config.results_dir.exists() {
            return Ok(None);
        }

        let mut entries = tokio::fs::read_dir(&self.config.results_dir).await?;
        let mut latest: Option<(PathBuf, std::time::SystemTime)> = None;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Only consider JSON files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Get modification time
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    match &latest {
                        None => latest = Some((path, modified)),
                        Some((_, latest_time)) if modified > *latest_time => {
                            latest = Some((path, modified));
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(latest.map(|(path, _)| path))
    }

    /// Get the last check result.
    pub async fn last_check_result(&self) -> Option<RegressionCheckResult> {
        self.last_check.read().await.clone()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_runner::{EvalMetadata, ScenarioResult, ValidationResult};
    use crate::quality_judge::QualityScore;
    use tempfile::TempDir;

    fn create_test_result(scenario_id: &str, passed: bool, quality: f64) -> ScenarioResult {
        ScenarioResult {
            scenario_id: scenario_id.to_string(),
            passed,
            output: "test".to_string(),
            quality_score: QualityScore {
                accuracy: quality,
                relevance: quality,
                completeness: quality,
                safety: 1.0,
                coherence: quality,
                conciseness: quality,
                overall: quality,
                reasoning: "test".to_string(),
                issues: vec![],
                suggestions: vec![],
            },
            latency_ms: 100,
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

    fn create_test_report(passed: usize, failed: usize, quality: f64) -> EvalReport {
        let mut results = Vec::new();
        for i in 0..passed {
            results.push(create_test_result(&format!("passed_{i}"), true, quality));
        }
        for i in 0..failed {
            results.push(create_test_result(&format!("failed_{i}"), false, quality * 0.5));
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

    #[tokio::test]
    async fn test_save_and_load_baseline() {
        let temp_dir = TempDir::new().unwrap();

        let config = MonitorConfig::default()
            .with_app_name("test_app")
            .with_baseline_dir(temp_dir.path());

        let monitor = QualityMonitor::new(config);

        let report = create_test_report(48, 2, 0.95);
        monitor.save_baseline("main", &report, Some("Test baseline")).unwrap();

        let loaded = monitor.load_baseline("main").unwrap();
        assert_eq!(loaded.total, 50);
        assert_eq!(loaded.passed, 48);
    }

    #[tokio::test]
    async fn test_no_regression_detected() {
        let temp_dir = TempDir::new().unwrap();

        let config = MonitorConfig::default()
            .with_app_name("test_app")
            .with_baseline_dir(temp_dir.path())
            .with_instance_id("test-instance");

        // Disable metrics to avoid prometheus registration conflicts in tests
        let mut config = config;
        config.enable_metrics = false;

        let monitor = QualityMonitor::new(config);

        // Create baseline and current with similar quality
        let baseline = create_test_report(48, 2, 0.95);
        let current = create_test_report(47, 3, 0.94);

        let result = monitor
            .check_regression_with_reports("main", &baseline, &current)
            .await
            .unwrap();

        assert!(!result.has_critical_regressions);
        assert_eq!(result.critical_count, 0);
    }

    #[tokio::test]
    async fn test_regression_detected() {
        let temp_dir = TempDir::new().unwrap();

        let config = MonitorConfig::default()
            .with_app_name("test_app")
            .with_baseline_dir(temp_dir.path())
            .with_instance_id("test-instance");

        // Disable metrics
        let mut config = config;
        config.enable_metrics = false;

        let monitor = QualityMonitor::new(config);

        // Create baseline with high quality and current with low quality
        let baseline = create_test_report(48, 2, 0.95);
        let current = create_test_report(35, 15, 0.70); // Significant drop

        let result = monitor
            .check_regression_with_reports("main", &baseline, &current)
            .await
            .unwrap();

        assert!(result.has_regressions);
        assert!(result.has_critical_regressions);
        assert!(result.critical_count > 0);
    }

    #[tokio::test]
    async fn test_list_baselines() {
        let temp_dir = TempDir::new().unwrap();

        let config = MonitorConfig::default()
            .with_app_name("test_app")
            .with_baseline_dir(temp_dir.path());

        let monitor = QualityMonitor::new(config);

        let report = create_test_report(48, 2, 0.95);
        monitor.save_baseline("main", &report, None).unwrap();
        monitor.save_baseline("v1.0.0", &report, None).unwrap();

        let baselines = monitor.list_baselines().unwrap();
        assert_eq!(baselines.len(), 2);
    }

    #[test]
    fn test_monitor_config_builder() {
        let config = MonitorConfig::default()
            .with_app_name("my_app")
            .with_baseline_dir("/path/to/baselines")
            .with_results_dir("/path/to/results")
            .with_check_interval(Duration::from_secs(300))
            .with_slack("https://hooks.slack.com/test", "#alerts")
            .with_instance_id("test-123");

        assert_eq!(config.app_name, "my_app");
        assert_eq!(config.baseline_dir, PathBuf::from("/path/to/baselines"));
        assert_eq!(config.results_dir, PathBuf::from("/path/to/results"));
        assert_eq!(config.check_interval, Duration::from_secs(300));
        assert_eq!(config.slack_webhook_url, Some("https://hooks.slack.com/test".to_string()));
        assert_eq!(config.slack_channel, Some("#alerts".to_string()));
        assert_eq!(config.instance_id, "test-123");
    }
}
