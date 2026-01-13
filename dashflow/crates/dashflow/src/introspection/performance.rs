//! Real-Time Performance Metrics
//!
//! This module provides types for monitoring and tracking performance metrics
//! during graph execution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Real-Time Performance Metrics
// ============================================================================

/// Performance metrics snapshot - tracks real-time performance indicators
///
/// This struct enables AI agents to monitor their own performance and make
/// adaptive decisions based on current system state. Agents can check latency,
/// throughput, error rates, and resource usage to optimize their behavior.
///
/// # Example
///
/// ```rust,ignore
/// let metrics = PerformanceMetrics::new()
///     .with_current_latency_ms(250.0)
///     .with_tokens_per_second(45.0)
///     .with_error_rate(0.02)
///     .with_memory_usage_mb(512.0);
///
/// // AI checks performance
/// if metrics.is_latency_high(1000.0) {
///     // Switch to faster model or reduce batch size
/// }
///
/// if metrics.is_error_rate_high(0.1) {
///     // Enable retry logic or alert operator
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Current operation latency in milliseconds
    pub current_latency_ms: f64,
    /// Average latency over recent operations (ms)
    pub average_latency_ms: f64,
    /// P95 latency over recent operations (ms)
    pub p95_latency_ms: f64,
    /// P99 latency over recent operations (ms)
    pub p99_latency_ms: f64,
    /// Token processing throughput (tokens/second)
    pub tokens_per_second: f64,
    /// Current error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Memory usage in megabytes
    pub memory_usage_mb: f64,
    /// CPU usage percentage (0.0 to 100.0)
    pub cpu_usage_percent: f64,
    /// Number of operations in the sample window
    pub sample_count: u64,
    /// Sample window duration in seconds
    pub sample_window_secs: f64,
    /// Timestamp of metrics snapshot (ISO 8601)
    pub timestamp: Option<String>,
    /// Thread ID associated with these metrics
    pub thread_id: Option<String>,
    /// Custom metrics
    pub custom: HashMap<String, f64>,
}

impl PerformanceMetrics {
    /// Create a new performance metrics snapshot
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for performance metrics
    #[must_use]
    pub fn builder() -> PerformanceMetricsBuilder {
        PerformanceMetricsBuilder::new()
    }

    /// Set current latency
    #[must_use]
    pub fn with_current_latency_ms(mut self, latency: f64) -> Self {
        self.current_latency_ms = latency.max(0.0);
        self
    }

    /// Set average latency
    #[must_use]
    pub fn with_average_latency_ms(mut self, latency: f64) -> Self {
        self.average_latency_ms = latency.max(0.0);
        self
    }

    /// Set P95 latency
    #[must_use]
    pub fn with_p95_latency_ms(mut self, latency: f64) -> Self {
        self.p95_latency_ms = latency.max(0.0);
        self
    }

    /// Set P99 latency
    #[must_use]
    pub fn with_p99_latency_ms(mut self, latency: f64) -> Self {
        self.p99_latency_ms = latency.max(0.0);
        self
    }

    /// Set tokens per second throughput
    #[must_use]
    pub fn with_tokens_per_second(mut self, tps: f64) -> Self {
        self.tokens_per_second = tps.max(0.0);
        self
    }

    /// Set error rate (0.0 to 1.0)
    #[must_use]
    pub fn with_error_rate(mut self, rate: f64) -> Self {
        self.error_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set memory usage in MB
    #[must_use]
    pub fn with_memory_usage_mb(mut self, usage: f64) -> Self {
        self.memory_usage_mb = usage.max(0.0);
        self
    }

    /// Set CPU usage percentage
    #[must_use]
    pub fn with_cpu_usage_percent(mut self, usage: f64) -> Self {
        self.cpu_usage_percent = usage.clamp(0.0, 100.0);
        self
    }

    /// Set sample count
    #[must_use]
    pub fn with_sample_count(mut self, count: u64) -> Self {
        self.sample_count = count;
        self
    }

    /// Set sample window duration
    #[must_use]
    pub fn with_sample_window_secs(mut self, secs: f64) -> Self {
        self.sample_window_secs = secs.max(0.0);
        self
    }

    /// Set timestamp
    #[must_use]
    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    /// Set thread ID
    #[must_use]
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Add a custom metric
    #[must_use]
    pub fn with_custom_metric(mut self, key: impl Into<String>, value: f64) -> Self {
        self.custom.insert(key.into(), value);
        self
    }

    /// Get a custom metric
    #[must_use]
    pub fn get_custom_metric(&self, key: &str) -> Option<f64> {
        self.custom.get(key).copied()
    }

    /// Check if current latency is high (above threshold)
    #[must_use]
    pub fn is_latency_high(&self, threshold_ms: f64) -> bool {
        self.current_latency_ms > threshold_ms
    }

    /// Check if average latency is high
    #[must_use]
    pub fn is_avg_latency_high(&self, threshold_ms: f64) -> bool {
        self.average_latency_ms > threshold_ms
    }

    /// Check if P95 latency is high
    #[must_use]
    pub fn is_p95_latency_high(&self, threshold_ms: f64) -> bool {
        self.p95_latency_ms > threshold_ms
    }

    /// Check if error rate is high (above threshold)
    #[must_use]
    pub fn is_error_rate_high(&self, threshold: f64) -> bool {
        self.error_rate > threshold
    }

    /// Check if memory usage is high (above threshold in MB)
    #[must_use]
    pub fn is_memory_high(&self, threshold_mb: f64) -> bool {
        self.memory_usage_mb > threshold_mb
    }

    /// Check if CPU usage is high (above threshold percentage)
    #[must_use]
    pub fn is_cpu_high(&self, threshold_percent: f64) -> bool {
        self.cpu_usage_percent > threshold_percent
    }

    /// Check if throughput is low (below threshold)
    #[must_use]
    pub fn is_throughput_low(&self, threshold_tps: f64) -> bool {
        self.tokens_per_second < threshold_tps
    }

    /// Get error rate as percentage
    #[must_use]
    pub fn error_rate_percent(&self) -> f64 {
        self.error_rate * 100.0
    }

    /// Check if system is healthy based on default thresholds
    ///
    /// Default healthy thresholds:
    /// - Latency < 5000ms
    /// - Error rate < 10%
    /// - Memory < 4096MB (4GB)
    /// - CPU < 90%
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        !self.is_latency_high(5000.0)
            && !self.is_error_rate_high(0.1)
            && !self.is_memory_high(4096.0)
            && !self.is_cpu_high(90.0)
    }

    /// Check if system is healthy with custom thresholds
    #[must_use]
    pub fn is_healthy_with_thresholds(&self, thresholds: &PerformanceThresholds) -> bool {
        !self.is_latency_high(thresholds.max_latency_ms)
            && !self.is_error_rate_high(thresholds.max_error_rate)
            && !self.is_memory_high(thresholds.max_memory_mb)
            && !self.is_cpu_high(thresholds.max_cpu_percent)
    }

    /// Get all alerts based on provided thresholds
    #[must_use]
    pub fn check_thresholds(&self, thresholds: &PerformanceThresholds) -> Vec<PerformanceAlert> {
        let mut alerts = Vec::new();

        if self.is_latency_high(thresholds.max_latency_ms) {
            alerts.push(PerformanceAlert {
                alert_type: AlertType::HighLatency,
                metric_name: "current_latency_ms".to_string(),
                current_value: self.current_latency_ms,
                threshold_value: thresholds.max_latency_ms,
                severity: self.latency_severity(thresholds.max_latency_ms),
                message: format!(
                    "Latency {:.1}ms exceeds threshold {:.1}ms",
                    self.current_latency_ms, thresholds.max_latency_ms
                ),
            });
        }

        if self.is_error_rate_high(thresholds.max_error_rate) {
            alerts.push(PerformanceAlert {
                alert_type: AlertType::HighErrorRate,
                metric_name: "error_rate".to_string(),
                current_value: self.error_rate,
                threshold_value: thresholds.max_error_rate,
                severity: self.error_rate_severity(thresholds.max_error_rate),
                message: format!(
                    "Error rate {:.1}% exceeds threshold {:.1}%",
                    self.error_rate * 100.0,
                    thresholds.max_error_rate * 100.0
                ),
            });
        }

        if self.is_memory_high(thresholds.max_memory_mb) {
            alerts.push(PerformanceAlert {
                alert_type: AlertType::HighMemory,
                metric_name: "memory_usage_mb".to_string(),
                current_value: self.memory_usage_mb,
                threshold_value: thresholds.max_memory_mb,
                severity: self.memory_severity(thresholds.max_memory_mb),
                message: format!(
                    "Memory {:.1}MB exceeds threshold {:.1}MB",
                    self.memory_usage_mb, thresholds.max_memory_mb
                ),
            });
        }

        if self.is_cpu_high(thresholds.max_cpu_percent) {
            alerts.push(PerformanceAlert {
                alert_type: AlertType::HighCpu,
                metric_name: "cpu_usage_percent".to_string(),
                current_value: self.cpu_usage_percent,
                threshold_value: thresholds.max_cpu_percent,
                severity: self.cpu_severity(thresholds.max_cpu_percent),
                message: format!(
                    "CPU {:.1}% exceeds threshold {:.1}%",
                    self.cpu_usage_percent, thresholds.max_cpu_percent
                ),
            });
        }

        if self.is_throughput_low(thresholds.min_tokens_per_second) {
            alerts.push(PerformanceAlert {
                alert_type: AlertType::LowThroughput,
                metric_name: "tokens_per_second".to_string(),
                current_value: self.tokens_per_second,
                threshold_value: thresholds.min_tokens_per_second,
                severity: AlertSeverity::Warning,
                message: format!(
                    "Throughput {:.1} tokens/s below threshold {:.1}",
                    self.tokens_per_second, thresholds.min_tokens_per_second
                ),
            });
        }

        alerts
    }

    /// Calculate severity based on how far latency exceeds threshold
    fn latency_severity(&self, threshold: f64) -> AlertSeverity {
        let ratio = self.current_latency_ms / threshold;
        if ratio > 3.0 {
            AlertSeverity::Critical
        } else if ratio > 2.0 {
            AlertSeverity::Error
        } else {
            AlertSeverity::Warning
        }
    }

    /// Calculate severity based on error rate
    fn error_rate_severity(&self, threshold: f64) -> AlertSeverity {
        let ratio = self.error_rate / threshold;
        if ratio > 3.0 {
            AlertSeverity::Critical
        } else if ratio > 2.0 {
            AlertSeverity::Error
        } else {
            AlertSeverity::Warning
        }
    }

    /// Calculate severity based on memory usage
    fn memory_severity(&self, threshold: f64) -> AlertSeverity {
        let ratio = self.memory_usage_mb / threshold;
        if ratio > 1.5 {
            AlertSeverity::Critical
        } else if ratio > 1.2 {
            AlertSeverity::Error
        } else {
            AlertSeverity::Warning
        }
    }

    /// Calculate severity based on CPU usage
    fn cpu_severity(&self, threshold: f64) -> AlertSeverity {
        let ratio = self.cpu_usage_percent / threshold;
        if ratio > 1.1 {
            AlertSeverity::Critical
        } else if ratio > 1.05 {
            AlertSeverity::Error
        } else {
            AlertSeverity::Warning
        }
    }

    /// Generate a summary of current performance
    #[must_use]
    pub fn summarize(&self) -> String {
        let mut summary = String::from("Performance Metrics:\n");

        summary.push_str(&format!(
            "- Current latency: {:.1}ms\n",
            self.current_latency_ms
        ));
        if self.average_latency_ms > 0.0 {
            summary.push_str(&format!(
                "- Average latency: {:.1}ms\n",
                self.average_latency_ms
            ));
        }
        if self.p95_latency_ms > 0.0 {
            summary.push_str(&format!("- P95 latency: {:.1}ms\n", self.p95_latency_ms));
        }
        summary.push_str(&format!(
            "- Throughput: {:.1} tokens/s\n",
            self.tokens_per_second
        ));
        summary.push_str(&format!("- Error rate: {:.2}%\n", self.error_rate * 100.0));
        summary.push_str(&format!("- Memory: {:.1}MB\n", self.memory_usage_mb));
        summary.push_str(&format!("- CPU: {:.1}%\n", self.cpu_usage_percent));

        let health_status = if self.is_healthy() {
            "HEALTHY"
        } else {
            "DEGRADED"
        };
        summary.push_str(&format!("- Status: {}\n", health_status));

        summary
    }

    /// Convert to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert to compact JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Builder for creating performance metrics
#[derive(Debug, Default)]
pub struct PerformanceMetricsBuilder {
    current_latency_ms: f64,
    average_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    tokens_per_second: f64,
    error_rate: f64,
    memory_usage_mb: f64,
    cpu_usage_percent: f64,
    sample_count: u64,
    sample_window_secs: f64,
    timestamp: Option<String>,
    thread_id: Option<String>,
    custom: HashMap<String, f64>,
}

impl PerformanceMetricsBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set current latency
    #[must_use]
    pub fn current_latency_ms(mut self, latency: f64) -> Self {
        self.current_latency_ms = latency.max(0.0);
        self
    }

    /// Set average latency
    #[must_use]
    pub fn average_latency_ms(mut self, latency: f64) -> Self {
        self.average_latency_ms = latency.max(0.0);
        self
    }

    /// Set P95 latency
    #[must_use]
    pub fn p95_latency_ms(mut self, latency: f64) -> Self {
        self.p95_latency_ms = latency.max(0.0);
        self
    }

    /// Set P99 latency
    #[must_use]
    pub fn p99_latency_ms(mut self, latency: f64) -> Self {
        self.p99_latency_ms = latency.max(0.0);
        self
    }

    /// Set tokens per second
    #[must_use]
    pub fn tokens_per_second(mut self, tps: f64) -> Self {
        self.tokens_per_second = tps.max(0.0);
        self
    }

    /// Set error rate
    #[must_use]
    pub fn error_rate(mut self, rate: f64) -> Self {
        self.error_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set memory usage
    #[must_use]
    pub fn memory_usage_mb(mut self, usage: f64) -> Self {
        self.memory_usage_mb = usage.max(0.0);
        self
    }

    /// Set CPU usage
    #[must_use]
    pub fn cpu_usage_percent(mut self, usage: f64) -> Self {
        self.cpu_usage_percent = usage.clamp(0.0, 100.0);
        self
    }

    /// Set sample count
    #[must_use]
    pub fn sample_count(mut self, count: u64) -> Self {
        self.sample_count = count;
        self
    }

    /// Set sample window
    #[must_use]
    pub fn sample_window_secs(mut self, secs: f64) -> Self {
        self.sample_window_secs = secs.max(0.0);
        self
    }

    /// Set timestamp
    #[must_use]
    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    /// Set thread ID
    #[must_use]
    pub fn thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Add custom metric
    #[must_use]
    pub fn custom_metric(mut self, key: impl Into<String>, value: f64) -> Self {
        self.custom.insert(key.into(), value);
        self
    }

    /// Build the performance metrics
    #[must_use]
    pub fn build(self) -> PerformanceMetrics {
        PerformanceMetrics {
            current_latency_ms: self.current_latency_ms,
            average_latency_ms: self.average_latency_ms,
            p95_latency_ms: self.p95_latency_ms,
            p99_latency_ms: self.p99_latency_ms,
            tokens_per_second: self.tokens_per_second,
            error_rate: self.error_rate,
            memory_usage_mb: self.memory_usage_mb,
            cpu_usage_percent: self.cpu_usage_percent,
            sample_count: self.sample_count,
            sample_window_secs: self.sample_window_secs,
            timestamp: self.timestamp,
            thread_id: self.thread_id,
            custom: self.custom,
        }
    }
}

/// Performance thresholds for alerting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Maximum acceptable latency in milliseconds
    pub max_latency_ms: f64,
    /// Maximum acceptable error rate (0.0 to 1.0)
    pub max_error_rate: f64,
    /// Maximum acceptable memory usage in MB
    pub max_memory_mb: f64,
    /// Maximum acceptable CPU usage percentage
    pub max_cpu_percent: f64,
    /// Minimum acceptable throughput (tokens/second)
    pub min_tokens_per_second: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_latency_ms: 5000.0,     // 5 seconds
            max_error_rate: 0.1,        // 10%
            max_memory_mb: 4096.0,      // 4 GB
            max_cpu_percent: 90.0,      // 90%
            min_tokens_per_second: 1.0, // At least 1 token/sec
        }
    }
}

impl PerformanceThresholds {
    /// Create new thresholds with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create strict thresholds (lower limits)
    #[must_use]
    pub fn strict() -> Self {
        Self {
            max_latency_ms: 1000.0, // 1 second
            max_error_rate: 0.01,   // 1%
            max_memory_mb: 1024.0,  // 1 GB
            max_cpu_percent: 70.0,  // 70%
            min_tokens_per_second: 10.0,
        }
    }

    /// Create lenient thresholds (higher limits)
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            max_latency_ms: 30000.0, // 30 seconds
            max_error_rate: 0.25,    // 25%
            max_memory_mb: 8192.0,   // 8 GB
            max_cpu_percent: 95.0,   // 95%
            min_tokens_per_second: 0.1,
        }
    }

    /// Set maximum latency
    #[must_use]
    pub fn with_max_latency_ms(mut self, latency: f64) -> Self {
        self.max_latency_ms = latency;
        self
    }

    /// Set maximum error rate
    #[must_use]
    pub fn with_max_error_rate(mut self, rate: f64) -> Self {
        self.max_error_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set maximum memory
    #[must_use]
    pub fn with_max_memory_mb(mut self, memory: f64) -> Self {
        self.max_memory_mb = memory;
        self
    }

    /// Set maximum CPU
    #[must_use]
    pub fn with_max_cpu_percent(mut self, cpu: f64) -> Self {
        self.max_cpu_percent = cpu.clamp(0.0, 100.0);
        self
    }

    /// Set minimum throughput
    #[must_use]
    pub fn with_min_tokens_per_second(mut self, tps: f64) -> Self {
        self.min_tokens_per_second = tps;
        self
    }
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational - metric is slightly outside normal range
    Info,
    /// Warning - metric exceeds threshold but system is operational
    Warning,
    /// Error - metric significantly exceeds threshold
    Error,
    /// Critical - system is at risk of failure
    Critical,
}

impl AlertSeverity {
    /// Check if severity is critical or error
    #[must_use]
    pub fn is_severe(&self) -> bool {
        matches!(self, AlertSeverity::Error | AlertSeverity::Critical)
    }
}

/// Type of performance alert
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertType {
    /// Latency exceeds threshold
    HighLatency,
    /// Error rate exceeds threshold
    HighErrorRate,
    /// Memory usage exceeds threshold
    HighMemory,
    /// CPU usage exceeds threshold
    HighCpu,
    /// Throughput below threshold
    LowThroughput,
}

/// Performance alert - represents a threshold violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    /// Type of alert
    pub alert_type: AlertType,
    /// Name of the metric that triggered the alert
    pub metric_name: String,
    /// Current value of the metric
    pub current_value: f64,
    /// Threshold value that was exceeded
    pub threshold_value: f64,
    /// Severity of the alert
    pub severity: AlertSeverity,
    /// Human-readable message
    pub message: String,
}

impl PerformanceAlert {
    /// Check if this alert is severe (Error or Critical)
    #[must_use]
    pub fn is_severe(&self) -> bool {
        self.severity.is_severe()
    }

    /// Get how much the threshold was exceeded by (as a ratio)
    #[must_use]
    pub fn excess_ratio(&self) -> f64 {
        if self.threshold_value == 0.0 {
            return 0.0;
        }
        match self.alert_type {
            AlertType::LowThroughput => {
                // For throughput, we're below threshold
                if self.current_value == 0.0 {
                    f64::INFINITY
                } else {
                    self.threshold_value / self.current_value
                }
            }
            _ => self.current_value / self.threshold_value,
        }
    }
}

/// Performance history - tracks metrics over time for trend analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceHistory {
    /// Historical metric snapshots
    pub snapshots: Vec<PerformanceMetrics>,
    /// Maximum number of snapshots to retain
    pub max_snapshots: usize,
    /// Thread ID for this history
    pub thread_id: Option<String>,
}

impl PerformanceHistory {
    /// Create a new performance history
    #[must_use]
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
            thread_id: None,
        }
    }

    /// Create a performance history with thread ID
    #[must_use]
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Add a metrics snapshot
    pub fn add(&mut self, metrics: PerformanceMetrics) {
        self.snapshots.push(metrics);
        // Trim to max size
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
    }

    /// Get number of snapshots
    #[must_use]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Check if history is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Get latest snapshot
    #[must_use]
    pub fn latest(&self) -> Option<&PerformanceMetrics> {
        self.snapshots.last()
    }

    /// Get average latency over all snapshots
    #[must_use]
    pub fn average_latency(&self) -> Option<f64> {
        if self.snapshots.is_empty() {
            return None;
        }
        let sum: f64 = self.snapshots.iter().map(|s| s.current_latency_ms).sum();
        Some(sum / self.snapshots.len() as f64)
    }

    /// Get average error rate over all snapshots
    #[must_use]
    pub fn average_error_rate(&self) -> Option<f64> {
        if self.snapshots.is_empty() {
            return None;
        }
        let sum: f64 = self.snapshots.iter().map(|s| s.error_rate).sum();
        Some(sum / self.snapshots.len() as f64)
    }

    /// Get average throughput over all snapshots
    #[must_use]
    pub fn average_throughput(&self) -> Option<f64> {
        if self.snapshots.is_empty() {
            return None;
        }
        let sum: f64 = self.snapshots.iter().map(|s| s.tokens_per_second).sum();
        Some(sum / self.snapshots.len() as f64)
    }

    /// Get max latency observed
    #[must_use]
    pub fn max_latency(&self) -> Option<f64> {
        self.snapshots
            .iter()
            .map(|s| s.current_latency_ms)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get min latency observed
    #[must_use]
    pub fn min_latency(&self) -> Option<f64> {
        self.snapshots
            .iter()
            .map(|s| s.current_latency_ms)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Check if latency is trending up (comparing first half to second half)
    #[must_use]
    pub fn is_latency_trending_up(&self) -> bool {
        if self.snapshots.len() < 4 {
            return false;
        }
        let mid = self.snapshots.len() / 2;
        let first_half_avg: f64 = self.snapshots[..mid]
            .iter()
            .map(|s| s.current_latency_ms)
            .sum::<f64>()
            / mid as f64;
        let second_half_avg: f64 = self.snapshots[mid..]
            .iter()
            .map(|s| s.current_latency_ms)
            .sum::<f64>()
            / (self.snapshots.len() - mid) as f64;
        second_half_avg > first_half_avg * 1.1 // 10% increase
    }

    /// Check if error rate is trending up
    #[must_use]
    pub fn is_error_rate_trending_up(&self) -> bool {
        if self.snapshots.len() < 4 {
            return false;
        }
        let mid = self.snapshots.len() / 2;
        let first_half_avg: f64 = self.snapshots[..mid]
            .iter()
            .map(|s| s.error_rate)
            .sum::<f64>()
            / mid as f64;
        let second_half_avg: f64 = self.snapshots[mid..]
            .iter()
            .map(|s| s.error_rate)
            .sum::<f64>()
            / (self.snapshots.len() - mid) as f64;
        second_half_avg > first_half_avg * 1.5 // 50% increase
    }

    /// Get health summary based on latest snapshot
    #[must_use]
    pub fn health_summary(&self) -> String {
        match self.latest() {
            Some(metrics) => {
                let health = if metrics.is_healthy() {
                    "HEALTHY"
                } else {
                    "DEGRADED"
                };

                let trend = if self.is_latency_trending_up() {
                    " (latency increasing)"
                } else {
                    ""
                };

                format!("Status: {}{}", health, trend)
            }
            None => "No data available".to_string(),
        }
    }

    /// Convert to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PerformanceMetrics Tests
    // =========================================================================

    #[test]
    fn test_metrics_new() {
        let metrics = PerformanceMetrics::new();
        assert!((metrics.current_latency_ms - 0.0).abs() < f64::EPSILON);
        assert!((metrics.average_latency_ms - 0.0).abs() < f64::EPSILON);
        assert!((metrics.error_rate - 0.0).abs() < f64::EPSILON);
        assert_eq!(metrics.sample_count, 0);
        assert!(metrics.timestamp.is_none());
        assert!(metrics.custom.is_empty());
    }

    #[test]
    fn test_metrics_default() {
        let metrics = PerformanceMetrics::default();
        assert!((metrics.current_latency_ms - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_current_latency() {
        let metrics = PerformanceMetrics::new().with_current_latency_ms(500.0);
        assert!((metrics.current_latency_ms - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_latency_clamping() {
        let metrics = PerformanceMetrics::new().with_current_latency_ms(-100.0);
        assert!((metrics.current_latency_ms - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_average_latency() {
        let metrics = PerformanceMetrics::new().with_average_latency_ms(250.5);
        assert!((metrics.average_latency_ms - 250.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_p95_latency() {
        let metrics = PerformanceMetrics::new().with_p95_latency_ms(1000.0);
        assert!((metrics.p95_latency_ms - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_p99_latency() {
        let metrics = PerformanceMetrics::new().with_p99_latency_ms(2000.0);
        assert!((metrics.p99_latency_ms - 2000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_tokens_per_second() {
        let metrics = PerformanceMetrics::new().with_tokens_per_second(45.5);
        assert!((metrics.tokens_per_second - 45.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_error_rate() {
        let metrics = PerformanceMetrics::new().with_error_rate(0.05);
        assert!((metrics.error_rate - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_error_rate_clamping() {
        let metrics_low = PerformanceMetrics::new().with_error_rate(-0.5);
        assert!((metrics_low.error_rate - 0.0).abs() < f64::EPSILON);

        let metrics_high = PerformanceMetrics::new().with_error_rate(1.5);
        assert!((metrics_high.error_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_memory_usage() {
        let metrics = PerformanceMetrics::new().with_memory_usage_mb(512.0);
        assert!((metrics.memory_usage_mb - 512.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_cpu_usage() {
        let metrics = PerformanceMetrics::new().with_cpu_usage_percent(75.0);
        assert!((metrics.cpu_usage_percent - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_cpu_usage_clamping() {
        let metrics_low = PerformanceMetrics::new().with_cpu_usage_percent(-10.0);
        assert!((metrics_low.cpu_usage_percent - 0.0).abs() < f64::EPSILON);

        let metrics_high = PerformanceMetrics::new().with_cpu_usage_percent(150.0);
        assert!((metrics_high.cpu_usage_percent - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_sample_count() {
        let metrics = PerformanceMetrics::new().with_sample_count(100);
        assert_eq!(metrics.sample_count, 100);
    }

    #[test]
    fn test_metrics_with_sample_window() {
        let metrics = PerformanceMetrics::new().with_sample_window_secs(60.0);
        assert!((metrics.sample_window_secs - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_with_timestamp() {
        let metrics = PerformanceMetrics::new().with_timestamp("2025-12-26T12:00:00Z");
        assert_eq!(metrics.timestamp, Some("2025-12-26T12:00:00Z".to_string()));
    }

    #[test]
    fn test_metrics_with_thread_id() {
        let metrics = PerformanceMetrics::new().with_thread_id("thread-42");
        assert_eq!(metrics.thread_id, Some("thread-42".to_string()));
    }

    #[test]
    fn test_metrics_custom_metric() {
        let metrics = PerformanceMetrics::new()
            .with_custom_metric("queue_depth", 15.0)
            .with_custom_metric("connections", 100.0);

        assert_eq!(metrics.get_custom_metric("queue_depth"), Some(15.0));
        assert_eq!(metrics.get_custom_metric("connections"), Some(100.0));
        assert_eq!(metrics.get_custom_metric("unknown"), None);
    }

    #[test]
    fn test_metrics_is_latency_high() {
        let metrics = PerformanceMetrics::new().with_current_latency_ms(1000.0);
        assert!(metrics.is_latency_high(500.0));
        assert!(!metrics.is_latency_high(2000.0));
    }

    #[test]
    fn test_metrics_is_avg_latency_high() {
        let metrics = PerformanceMetrics::new().with_average_latency_ms(750.0);
        assert!(metrics.is_avg_latency_high(500.0));
        assert!(!metrics.is_avg_latency_high(1000.0));
    }

    #[test]
    fn test_metrics_is_p95_latency_high() {
        let metrics = PerformanceMetrics::new().with_p95_latency_ms(3000.0);
        assert!(metrics.is_p95_latency_high(2000.0));
        assert!(!metrics.is_p95_latency_high(4000.0));
    }

    #[test]
    fn test_metrics_is_error_rate_high() {
        let metrics = PerformanceMetrics::new().with_error_rate(0.15);
        assert!(metrics.is_error_rate_high(0.1));
        assert!(!metrics.is_error_rate_high(0.2));
    }

    #[test]
    fn test_metrics_is_memory_high() {
        let metrics = PerformanceMetrics::new().with_memory_usage_mb(5000.0);
        assert!(metrics.is_memory_high(4096.0));
        assert!(!metrics.is_memory_high(8192.0));
    }

    #[test]
    fn test_metrics_is_cpu_high() {
        let metrics = PerformanceMetrics::new().with_cpu_usage_percent(95.0);
        assert!(metrics.is_cpu_high(90.0));
        assert!(!metrics.is_cpu_high(99.0));
    }

    #[test]
    fn test_metrics_is_throughput_low() {
        let metrics = PerformanceMetrics::new().with_tokens_per_second(0.5);
        assert!(metrics.is_throughput_low(1.0));
        assert!(!metrics.is_throughput_low(0.1));
    }

    #[test]
    fn test_metrics_error_rate_percent() {
        let metrics = PerformanceMetrics::new().with_error_rate(0.15);
        assert!((metrics.error_rate_percent() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_is_healthy_default_thresholds() {
        // Healthy metrics
        let healthy = PerformanceMetrics::new()
            .with_current_latency_ms(1000.0)
            .with_error_rate(0.05)
            .with_memory_usage_mb(2000.0)
            .with_cpu_usage_percent(50.0);
        assert!(healthy.is_healthy());

        // Unhealthy due to high latency
        let unhealthy_latency = PerformanceMetrics::new().with_current_latency_ms(10000.0);
        assert!(!unhealthy_latency.is_healthy());

        // Unhealthy due to high error rate
        let unhealthy_error = PerformanceMetrics::new().with_error_rate(0.15);
        assert!(!unhealthy_error.is_healthy());

        // Unhealthy due to high memory
        let unhealthy_memory = PerformanceMetrics::new().with_memory_usage_mb(8000.0);
        assert!(!unhealthy_memory.is_healthy());

        // Unhealthy due to high CPU
        let unhealthy_cpu = PerformanceMetrics::new().with_cpu_usage_percent(95.0);
        assert!(!unhealthy_cpu.is_healthy());
    }

    #[test]
    fn test_metrics_is_healthy_with_custom_thresholds() {
        // Strict threshold for latency is 1000ms
        let metrics = PerformanceMetrics::new()
            .with_current_latency_ms(1500.0) // > 1000ms strict threshold
            .with_error_rate(0.005);

        let strict = PerformanceThresholds::strict();
        assert!(!metrics.is_healthy_with_thresholds(&strict)); // 1500ms > 1000ms threshold

        let lenient = PerformanceThresholds::lenient();
        assert!(metrics.is_healthy_with_thresholds(&lenient)); // 1500ms < 30000ms lenient
    }

    #[test]
    fn test_metrics_check_thresholds_no_violations() {
        let metrics = PerformanceMetrics::new()
            .with_current_latency_ms(100.0)
            .with_error_rate(0.01)
            .with_memory_usage_mb(1000.0)
            .with_cpu_usage_percent(50.0)
            .with_tokens_per_second(50.0);

        let thresholds = PerformanceThresholds::default();
        let alerts = metrics.check_thresholds(&thresholds);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_metrics_check_thresholds_latency_violation() {
        // Set tokens_per_second > default min threshold (1.0) to avoid LowThroughput alert
        let metrics = PerformanceMetrics::new()
            .with_current_latency_ms(10000.0)
            .with_tokens_per_second(10.0);
        let thresholds = PerformanceThresholds::default();
        let alerts = metrics.check_thresholds(&thresholds);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, AlertType::HighLatency);
        assert!(alerts[0].message.contains("Latency"));
    }

    #[test]
    fn test_metrics_check_thresholds_multiple_violations() {
        let metrics = PerformanceMetrics::new()
            .with_current_latency_ms(20000.0) // > 5000
            .with_error_rate(0.3) // > 0.1
            .with_memory_usage_mb(10000.0) // > 4096
            .with_cpu_usage_percent(95.0) // > 90
            .with_tokens_per_second(0.1); // < 1.0

        let thresholds = PerformanceThresholds::default();
        let alerts = metrics.check_thresholds(&thresholds);

        assert_eq!(alerts.len(), 5);

        let types: Vec<_> = alerts.iter().map(|a| a.alert_type).collect();
        assert!(types.contains(&AlertType::HighLatency));
        assert!(types.contains(&AlertType::HighErrorRate));
        assert!(types.contains(&AlertType::HighMemory));
        assert!(types.contains(&AlertType::HighCpu));
        assert!(types.contains(&AlertType::LowThroughput));
    }

    #[test]
    fn test_metrics_severity_calculation() {
        // Critical latency (>3x threshold)
        let critical = PerformanceMetrics::new().with_current_latency_ms(20000.0);
        let alerts = critical.check_thresholds(&PerformanceThresholds::default());
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);

        // Warning latency (just above threshold)
        let warning = PerformanceMetrics::new().with_current_latency_ms(6000.0);
        let alerts = warning.check_thresholds(&PerformanceThresholds::default());
        assert_eq!(alerts[0].severity, AlertSeverity::Warning);
    }

    #[test]
    fn test_metrics_summarize() {
        let metrics = PerformanceMetrics::new()
            .with_current_latency_ms(250.0)
            .with_average_latency_ms(200.0)
            .with_p95_latency_ms(450.0)
            .with_tokens_per_second(30.0)
            .with_error_rate(0.02)
            .with_memory_usage_mb(1024.0)
            .with_cpu_usage_percent(45.0);

        let summary = metrics.summarize();
        assert!(summary.contains("250.0ms"));
        assert!(summary.contains("200.0ms"));
        assert!(summary.contains("450.0ms"));
        assert!(summary.contains("30.0 tokens/s"));
        assert!(summary.contains("2.00%"));
        assert!(summary.contains("1024.0MB"));
        assert!(summary.contains("45.0%"));
        assert!(summary.contains("HEALTHY"));
    }

    #[test]
    fn test_metrics_summarize_degraded() {
        let metrics = PerformanceMetrics::new()
            .with_current_latency_ms(10000.0) // Exceeds default threshold
            .with_error_rate(0.01);

        let summary = metrics.summarize();
        assert!(summary.contains("DEGRADED"));
    }

    #[test]
    fn test_metrics_json_roundtrip() {
        let metrics = PerformanceMetrics::new()
            .with_current_latency_ms(500.0)
            .with_error_rate(0.05)
            .with_memory_usage_mb(2048.0)
            .with_custom_metric("custom", 42.0)
            .with_timestamp("2025-12-26T12:00:00Z");

        let json = metrics.to_json().unwrap();
        let parsed = PerformanceMetrics::from_json(&json).unwrap();

        assert!((parsed.current_latency_ms - 500.0).abs() < f64::EPSILON);
        assert!((parsed.error_rate - 0.05).abs() < f64::EPSILON);
        assert_eq!(parsed.get_custom_metric("custom"), Some(42.0));
    }

    #[test]
    fn test_metrics_json_compact() {
        let metrics = PerformanceMetrics::new().with_current_latency_ms(100.0);
        let json = metrics.to_json_compact().unwrap();
        assert!(!json.contains('\n'));
        assert!(json.contains("100"));
    }

    // =========================================================================
    // PerformanceMetricsBuilder Tests
    // =========================================================================

    #[test]
    fn test_builder_new() {
        let builder = PerformanceMetricsBuilder::new();
        let metrics = builder.build();
        assert!((metrics.current_latency_ms - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder_all_fields() {
        let metrics = PerformanceMetrics::builder()
            .current_latency_ms(500.0)
            .average_latency_ms(400.0)
            .p95_latency_ms(800.0)
            .p99_latency_ms(1200.0)
            .tokens_per_second(25.0)
            .error_rate(0.03)
            .memory_usage_mb(2048.0)
            .cpu_usage_percent(60.0)
            .sample_count(500)
            .sample_window_secs(120.0)
            .timestamp("2025-12-26")
            .thread_id("main")
            .custom_metric("queue", 10.0)
            .build();

        assert!((metrics.current_latency_ms - 500.0).abs() < f64::EPSILON);
        assert!((metrics.average_latency_ms - 400.0).abs() < f64::EPSILON);
        assert!((metrics.p95_latency_ms - 800.0).abs() < f64::EPSILON);
        assert!((metrics.p99_latency_ms - 1200.0).abs() < f64::EPSILON);
        assert!((metrics.tokens_per_second - 25.0).abs() < f64::EPSILON);
        assert!((metrics.error_rate - 0.03).abs() < f64::EPSILON);
        assert!((metrics.memory_usage_mb - 2048.0).abs() < f64::EPSILON);
        assert!((metrics.cpu_usage_percent - 60.0).abs() < f64::EPSILON);
        assert_eq!(metrics.sample_count, 500);
        assert!((metrics.sample_window_secs - 120.0).abs() < f64::EPSILON);
        assert_eq!(metrics.timestamp, Some("2025-12-26".to_string()));
        assert_eq!(metrics.thread_id, Some("main".to_string()));
        assert_eq!(metrics.get_custom_metric("queue"), Some(10.0));
    }

    #[test]
    fn test_builder_clamping() {
        let metrics = PerformanceMetrics::builder()
            .current_latency_ms(-100.0) // Should clamp to 0
            .error_rate(2.0) // Should clamp to 1.0
            .cpu_usage_percent(-50.0) // Should clamp to 0
            .build();

        assert!((metrics.current_latency_ms - 0.0).abs() < f64::EPSILON);
        assert!((metrics.error_rate - 1.0).abs() < f64::EPSILON);
        assert!((metrics.cpu_usage_percent - 0.0).abs() < f64::EPSILON);
    }

    // =========================================================================
    // PerformanceThresholds Tests
    // =========================================================================

    #[test]
    fn test_thresholds_default() {
        let thresholds = PerformanceThresholds::default();
        assert!((thresholds.max_latency_ms - 5000.0).abs() < f64::EPSILON);
        assert!((thresholds.max_error_rate - 0.1).abs() < f64::EPSILON);
        assert!((thresholds.max_memory_mb - 4096.0).abs() < f64::EPSILON);
        assert!((thresholds.max_cpu_percent - 90.0).abs() < f64::EPSILON);
        assert!((thresholds.min_tokens_per_second - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_thresholds_new() {
        let thresholds = PerformanceThresholds::new();
        assert!((thresholds.max_latency_ms - 5000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_thresholds_strict() {
        let thresholds = PerformanceThresholds::strict();
        assert!((thresholds.max_latency_ms - 1000.0).abs() < f64::EPSILON);
        assert!((thresholds.max_error_rate - 0.01).abs() < f64::EPSILON);
        assert!((thresholds.max_memory_mb - 1024.0).abs() < f64::EPSILON);
        assert!((thresholds.max_cpu_percent - 70.0).abs() < f64::EPSILON);
        assert!((thresholds.min_tokens_per_second - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_thresholds_lenient() {
        let thresholds = PerformanceThresholds::lenient();
        assert!((thresholds.max_latency_ms - 30000.0).abs() < f64::EPSILON);
        assert!((thresholds.max_error_rate - 0.25).abs() < f64::EPSILON);
        assert!((thresholds.max_memory_mb - 8192.0).abs() < f64::EPSILON);
        assert!((thresholds.max_cpu_percent - 95.0).abs() < f64::EPSILON);
        assert!((thresholds.min_tokens_per_second - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_thresholds_with_methods() {
        let thresholds = PerformanceThresholds::new()
            .with_max_latency_ms(2000.0)
            .with_max_error_rate(0.05)
            .with_max_memory_mb(2048.0)
            .with_max_cpu_percent(80.0)
            .with_min_tokens_per_second(5.0);

        assert!((thresholds.max_latency_ms - 2000.0).abs() < f64::EPSILON);
        assert!((thresholds.max_error_rate - 0.05).abs() < f64::EPSILON);
        assert!((thresholds.max_memory_mb - 2048.0).abs() < f64::EPSILON);
        assert!((thresholds.max_cpu_percent - 80.0).abs() < f64::EPSILON);
        assert!((thresholds.min_tokens_per_second - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_thresholds_error_rate_clamping() {
        let thresholds = PerformanceThresholds::new().with_max_error_rate(2.0);
        assert!((thresholds.max_error_rate - 1.0).abs() < f64::EPSILON);

        let thresholds_neg = PerformanceThresholds::new().with_max_error_rate(-0.5);
        assert!((thresholds_neg.max_error_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_thresholds_cpu_clamping() {
        let thresholds = PerformanceThresholds::new().with_max_cpu_percent(150.0);
        assert!((thresholds.max_cpu_percent - 100.0).abs() < f64::EPSILON);
    }

    // =========================================================================
    // AlertSeverity Tests
    // =========================================================================

    #[test]
    fn test_alert_severity_is_severe() {
        assert!(!AlertSeverity::Info.is_severe());
        assert!(!AlertSeverity::Warning.is_severe());
        assert!(AlertSeverity::Error.is_severe());
        assert!(AlertSeverity::Critical.is_severe());
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_alert_severity_clone() {
        let severity = AlertSeverity::Critical;
        let cloned = severity.clone();
        assert_eq!(severity, cloned);
    }

    #[test]
    fn test_alert_severity_debug() {
        let debug = format!("{:?}", AlertSeverity::Warning);
        assert!(debug.contains("Warning"));
    }

    // =========================================================================
    // AlertType Tests
    // =========================================================================

    #[test]
    fn test_alert_type_equality() {
        assert_eq!(AlertType::HighLatency, AlertType::HighLatency);
        assert_ne!(AlertType::HighLatency, AlertType::HighErrorRate);
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_alert_type_clone() {
        let alert_type = AlertType::HighMemory;
        let cloned = alert_type.clone();
        assert_eq!(alert_type, cloned);
    }

    // =========================================================================
    // PerformanceAlert Tests
    // =========================================================================

    #[test]
    fn test_alert_is_severe() {
        let warning_alert = PerformanceAlert {
            alert_type: AlertType::HighLatency,
            metric_name: "latency".to_string(),
            current_value: 6000.0,
            threshold_value: 5000.0,
            severity: AlertSeverity::Warning,
            message: "Latency high".to_string(),
        };
        assert!(!warning_alert.is_severe());

        let critical_alert = PerformanceAlert {
            alert_type: AlertType::HighLatency,
            metric_name: "latency".to_string(),
            current_value: 20000.0,
            threshold_value: 5000.0,
            severity: AlertSeverity::Critical,
            message: "Latency critical".to_string(),
        };
        assert!(critical_alert.is_severe());
    }

    #[test]
    fn test_alert_excess_ratio() {
        let alert = PerformanceAlert {
            alert_type: AlertType::HighLatency,
            metric_name: "latency".to_string(),
            current_value: 10000.0,
            threshold_value: 5000.0,
            severity: AlertSeverity::Error,
            message: "Latency high".to_string(),
        };
        assert!((alert.excess_ratio() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_alert_excess_ratio_zero_threshold() {
        let alert = PerformanceAlert {
            alert_type: AlertType::HighLatency,
            metric_name: "latency".to_string(),
            current_value: 100.0,
            threshold_value: 0.0,
            severity: AlertSeverity::Warning,
            message: "Test".to_string(),
        };
        assert!((alert.excess_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_alert_excess_ratio_low_throughput() {
        // For low throughput, ratio is inverted
        let alert = PerformanceAlert {
            alert_type: AlertType::LowThroughput,
            metric_name: "tps".to_string(),
            current_value: 0.5,
            threshold_value: 1.0,
            severity: AlertSeverity::Warning,
            message: "Throughput low".to_string(),
        };
        assert!((alert.excess_ratio() - 2.0).abs() < f64::EPSILON); // 1.0 / 0.5 = 2.0
    }

    #[test]
    fn test_alert_excess_ratio_low_throughput_zero_current() {
        let alert = PerformanceAlert {
            alert_type: AlertType::LowThroughput,
            metric_name: "tps".to_string(),
            current_value: 0.0,
            threshold_value: 1.0,
            severity: AlertSeverity::Critical,
            message: "No throughput".to_string(),
        };
        assert!(alert.excess_ratio().is_infinite());
    }

    // =========================================================================
    // PerformanceHistory Tests
    // =========================================================================

    #[test]
    fn test_history_new() {
        let history = PerformanceHistory::new(100);
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert_eq!(history.max_snapshots, 100);
        assert!(history.thread_id.is_none());
    }

    #[test]
    fn test_history_with_thread_id() {
        let history = PerformanceHistory::new(50).with_thread_id("worker-1");
        assert_eq!(history.thread_id, Some("worker-1".to_string()));
    }

    #[test]
    fn test_history_add() {
        let mut history = PerformanceHistory::new(10);
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));

        assert_eq!(history.len(), 2);
        assert!(!history.is_empty());
    }

    #[test]
    fn test_history_trim_to_max() {
        let mut history = PerformanceHistory::new(3);
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(300.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(400.0));

        assert_eq!(history.len(), 3);
        // First snapshot should be trimmed
        assert!((history.snapshots[0].current_latency_ms - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_latest() {
        let mut history = PerformanceHistory::new(10);
        assert!(history.latest().is_none());

        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));

        let latest = history.latest().unwrap();
        assert!((latest.current_latency_ms - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_average_latency() {
        let mut history = PerformanceHistory::new(10);
        assert!(history.average_latency().is_none());

        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(300.0));

        let avg = history.average_latency().unwrap();
        assert!((avg - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_average_error_rate() {
        let mut history = PerformanceHistory::new(10);
        history.add(PerformanceMetrics::new().with_error_rate(0.1));
        history.add(PerformanceMetrics::new().with_error_rate(0.2));
        history.add(PerformanceMetrics::new().with_error_rate(0.3));

        let avg = history.average_error_rate().unwrap();
        assert!((avg - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_average_throughput() {
        let mut history = PerformanceHistory::new(10);
        history.add(PerformanceMetrics::new().with_tokens_per_second(10.0));
        history.add(PerformanceMetrics::new().with_tokens_per_second(20.0));
        history.add(PerformanceMetrics::new().with_tokens_per_second(30.0));

        let avg = history.average_throughput().unwrap();
        assert!((avg - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_max_min_latency() {
        let mut history = PerformanceHistory::new(10);
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(500.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(300.0));

        let max = history.max_latency().unwrap();
        assert!((max - 500.0).abs() < f64::EPSILON);

        let min = history.min_latency().unwrap();
        assert!((min - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_latency_trending_up() {
        let mut history = PerformanceHistory::new(10);
        // First half: lower latency
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(110.0));
        // Second half: higher latency (>10% increase)
        history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(220.0));

        assert!(history.is_latency_trending_up());
    }

    #[test]
    fn test_history_latency_not_trending_up() {
        let mut history = PerformanceHistory::new(10);
        // Stable latency
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(105.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(95.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));

        assert!(!history.is_latency_trending_up());
    }

    #[test]
    fn test_history_latency_trend_insufficient_data() {
        let mut history = PerformanceHistory::new(10);
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));
        // Only 2 snapshots, need at least 4

        assert!(!history.is_latency_trending_up());
    }

    #[test]
    fn test_history_error_rate_trending_up() {
        let mut history = PerformanceHistory::new(10);
        // First half: lower error rate
        history.add(PerformanceMetrics::new().with_error_rate(0.01));
        history.add(PerformanceMetrics::new().with_error_rate(0.02));
        // Second half: higher error rate (>50% increase)
        history.add(PerformanceMetrics::new().with_error_rate(0.05));
        history.add(PerformanceMetrics::new().with_error_rate(0.06));

        assert!(history.is_error_rate_trending_up());
    }

    #[test]
    fn test_history_health_summary_no_data() {
        let history = PerformanceHistory::new(10);
        let summary = history.health_summary();
        assert!(summary.contains("No data"));
    }

    #[test]
    fn test_history_health_summary_healthy() {
        let mut history = PerformanceHistory::new(10);
        history.add(
            PerformanceMetrics::new()
                .with_current_latency_ms(1000.0)
                .with_error_rate(0.01),
        );

        let summary = history.health_summary();
        assert!(summary.contains("HEALTHY"));
    }

    #[test]
    fn test_history_health_summary_degraded() {
        let mut history = PerformanceHistory::new(10);
        history.add(
            PerformanceMetrics::new()
                .with_current_latency_ms(10000.0) // Exceeds threshold
                .with_error_rate(0.01),
        );

        let summary = history.health_summary();
        assert!(summary.contains("DEGRADED"));
    }

    #[test]
    fn test_history_health_summary_with_trend() {
        let mut history = PerformanceHistory::new(10);
        // Create increasing latency trend
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(120.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(250.0));

        let summary = history.health_summary();
        assert!(summary.contains("latency increasing"));
    }

    #[test]
    fn test_history_json_roundtrip() {
        let mut history = PerformanceHistory::new(5);
        history.thread_id = Some("test-thread".to_string());
        history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
        history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));

        let json = history.to_json().unwrap();
        let parsed = PerformanceHistory::from_json(&json).unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.max_snapshots, 5);
        assert_eq!(parsed.thread_id, Some("test-thread".to_string()));
    }

    #[test]
    fn test_history_default() {
        let history = PerformanceHistory::default();
        assert!(history.is_empty());
        assert_eq!(history.max_snapshots, 0);
    }

    // =========================================================================
    // Chained Builder Pattern Tests
    // =========================================================================

    #[test]
    fn test_metrics_builder_chain() {
        let metrics = PerformanceMetrics::new()
            .with_current_latency_ms(100.0)
            .with_average_latency_ms(90.0)
            .with_p95_latency_ms(150.0)
            .with_p99_latency_ms(200.0)
            .with_tokens_per_second(50.0)
            .with_error_rate(0.01)
            .with_memory_usage_mb(512.0)
            .with_cpu_usage_percent(30.0)
            .with_sample_count(1000)
            .with_sample_window_secs(60.0)
            .with_timestamp("2025-12-26T12:00:00Z")
            .with_thread_id("main-thread")
            .with_custom_metric("queue_depth", 5.0)
            .with_custom_metric("connections", 100.0);

        assert!((metrics.current_latency_ms - 100.0).abs() < f64::EPSILON);
        assert!(metrics.is_healthy());
        assert_eq!(metrics.custom.len(), 2);
    }
}
