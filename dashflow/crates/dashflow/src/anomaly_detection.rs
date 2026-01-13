// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Anomaly Detection - AI Detects Unusual Behavior
//!
//! This module provides anomaly detection capabilities that allow AI agents to
//! identify unusual execution behavior compared to historical baselines.
//!
//! ## Overview
//!
//! Anomaly detection enables AI agents to:
//! - Detect when execution is significantly slower than usual
//! - Identify unusual token usage patterns
//! - Spot unexpected error rates or retry patterns
//! - Alert on unusual execution paths
//! - Detect resource exhaustion before it becomes critical
//!
//! ## Key Concepts
//!
//! - **Anomaly**: An unusual deviation from expected behavior
//! - **Severity**: How critical the anomaly is (Info, Warning, Critical)
//! - **AnomalyDetector**: Compares execution to historical statistics
//! - **ExecutionStats**: Historical baseline for comparison
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::anomaly_detection::AnomalyDetector;
//!
//! // Build detector from historical data
//! let detector = AnomalyDetector::train(&historical_traces);
//!
//! // Detect anomalies in current execution
//! let anomalies = detector.detect(&current_trace);
//!
//! if !anomalies.is_empty() {
//!     println!("Detected {} anomalies:", anomalies.len());
//!     for anomaly in anomalies {
//!         println!("  [{}] {}: {} (expected: {})",
//!             anomaly.severity,
//!             anomaly.metric,
//!             anomaly.actual_value,
//!             anomaly.expected_value
//!         );
//!     }
//! }
//! ```

use crate::constants::{MILLION, THOUSAND};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An anomaly detected in execution behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// The metric that deviated
    pub metric: AnomalyMetric,
    /// Expected value based on history
    pub expected_value: f64,
    /// Actual observed value
    pub actual_value: f64,
    /// How far from expected (in standard deviations)
    pub deviation: f64,
    /// Severity of this anomaly
    pub severity: AnomalySeverity,
    /// Human-readable explanation
    pub explanation: String,
    /// Suggested action to take
    pub suggestion: Option<String>,
    /// Node where anomaly was detected (if node-specific)
    pub node: Option<String>,
    /// Timestamp of detection
    pub detected_at: Option<String>,
    /// Additional context
    pub context: HashMap<String, serde_json::Value>,
}

impl Anomaly {
    /// Create a new anomaly
    #[must_use]
    pub fn new(
        metric: AnomalyMetric,
        expected: f64,
        actual: f64,
        severity: AnomalySeverity,
    ) -> Self {
        let deviation = if expected > 0.0 {
            (actual - expected).abs() / expected
        } else {
            0.0
        };

        Self {
            metric,
            expected_value: expected,
            actual_value: actual,
            deviation,
            severity,
            explanation: String::new(),
            suggestion: None,
            node: None,
            detected_at: None,
            context: HashMap::new(),
        }
    }

    /// Set explanation
    #[must_use]
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = explanation.into();
        self
    }

    /// Set suggestion
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Set node
    #[must_use]
    pub fn with_node(mut self, node: impl Into<String>) -> Self {
        self.node = Some(node.into());
        self
    }

    /// Add context
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.context.insert(key.into(), value);
        self
    }

    /// Get a formatted description
    #[must_use]
    pub fn description(&self) -> String {
        let mut desc = format!(
            "[{}] {}: {} (expected: {}, deviation: {:.1}%)",
            self.severity,
            self.metric,
            format_value(self.actual_value),
            format_value(self.expected_value),
            self.deviation * 100.0
        );

        if !self.explanation.is_empty() {
            desc.push_str(&format!(" - {}", self.explanation));
        }

        if let Some(ref node) = self.node {
            desc.push_str(&format!(" [node: {}]", node));
        }

        desc
    }

    /// Check if this is a critical anomaly
    #[must_use]
    pub fn is_critical(&self) -> bool {
        self.severity == AnomalySeverity::Critical
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

/// Metrics that can be anomalous
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnomalyMetric {
    /// Total execution duration
    TotalDuration,
    /// Node-specific duration
    NodeDuration,
    /// Total token usage
    TotalTokens,
    /// Node-specific token usage
    NodeTokens,
    /// Number of nodes executed
    NodeCount,
    /// Number of errors
    ErrorCount,
    /// Error rate
    ErrorRate,
    /// Number of retries
    RetryCount,
    /// Number of tool calls
    ToolCallCount,
    /// Execution path (unusual path taken)
    ExecutionPath,
    /// Loop iterations
    LoopIterations,
    /// Cost
    Cost,
}

impl std::fmt::Display for AnomalyMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnomalyMetric::TotalDuration => write!(f, "total_duration"),
            AnomalyMetric::NodeDuration => write!(f, "node_duration"),
            AnomalyMetric::TotalTokens => write!(f, "total_tokens"),
            AnomalyMetric::NodeTokens => write!(f, "node_tokens"),
            AnomalyMetric::NodeCount => write!(f, "node_count"),
            AnomalyMetric::ErrorCount => write!(f, "error_count"),
            AnomalyMetric::ErrorRate => write!(f, "error_rate"),
            AnomalyMetric::RetryCount => write!(f, "retry_count"),
            AnomalyMetric::ToolCallCount => write!(f, "tool_call_count"),
            AnomalyMetric::ExecutionPath => write!(f, "execution_path"),
            AnomalyMetric::LoopIterations => write!(f, "loop_iterations"),
            AnomalyMetric::Cost => write!(f, "cost"),
        }
    }
}

/// Severity levels for anomalies
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AnomalySeverity {
    /// Informational - notable but not concerning
    Info,
    /// Warning - should be investigated
    Warning,
    /// Critical - requires immediate attention
    Critical,
}

impl std::fmt::Display for AnomalySeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnomalySeverity::Info => write!(f, "INFO"),
            AnomalySeverity::Warning => write!(f, "WARN"),
            AnomalySeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Historical execution statistics for baseline comparison
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Average total duration (ms)
    pub avg_duration_ms: f64,
    /// Standard deviation of duration
    pub std_duration_ms: f64,
    /// Average token usage
    pub avg_tokens: f64,
    /// Standard deviation of tokens
    pub std_tokens: f64,
    /// Average node count
    pub avg_node_count: f64,
    /// Average error rate
    pub avg_error_rate: f64,
    /// Average retry count
    pub avg_retry_count: f64,
    /// Average tool call count
    pub avg_tool_calls: f64,
    /// Number of samples
    pub sample_count: usize,
    /// Per-node statistics
    pub node_stats: HashMap<String, NodeExecutionStats>,
    /// Common execution paths with frequencies
    pub common_paths: Vec<(Vec<String>, f64)>,
}

impl ExecutionStats {
    /// Create new empty stats
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if stats have enough samples for reliable detection
    #[must_use]
    pub fn is_reliable(&self) -> bool {
        self.sample_count >= 10
    }

    /// Get p95 duration estimate
    #[must_use]
    pub fn p95_duration_ms(&self) -> f64 {
        self.avg_duration_ms + 1.645 * self.std_duration_ms
    }

    /// Get p99 duration estimate
    #[must_use]
    pub fn p99_duration_ms(&self) -> f64 {
        self.avg_duration_ms + 2.326 * self.std_duration_ms
    }
}

/// Per-node execution statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeExecutionStats {
    /// Average duration for this node
    pub avg_duration_ms: f64,
    /// Standard deviation of duration
    pub std_duration_ms: f64,
    /// Average tokens for this node
    pub avg_tokens: f64,
    /// Standard deviation of tokens
    pub std_tokens: f64,
    /// Number of samples
    pub sample_count: usize,
    /// Average execution count per trace (for loops)
    pub avg_executions_per_trace: f64,
}

/// Configuration for anomaly detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectionConfig {
    /// Number of standard deviations for warning threshold
    pub warning_threshold_std: f64,
    /// Number of standard deviations for critical threshold
    pub critical_threshold_std: f64,
    /// Minimum samples needed for reliable detection
    pub min_samples: usize,
    /// Whether to detect duration anomalies
    pub detect_duration: bool,
    /// Whether to detect token anomalies
    pub detect_tokens: bool,
    /// Whether to detect error anomalies
    pub detect_errors: bool,
    /// Whether to detect path anomalies
    pub detect_paths: bool,
    /// Whether to detect loop anomalies
    pub detect_loops: bool,
    /// Minimum path frequency to be considered "normal"
    pub min_path_frequency: f64,
    /// Maximum loop iterations before warning
    pub max_loop_iterations: usize,
}

impl Default for AnomalyDetectionConfig {
    fn default() -> Self {
        Self {
            warning_threshold_std: 2.0,  // 2 standard deviations
            critical_threshold_std: 3.0, // 3 standard deviations
            min_samples: 10,
            detect_duration: true,
            detect_tokens: true,
            detect_errors: true,
            detect_paths: true,
            detect_loops: true,
            min_path_frequency: 0.05, // 5% of executions
            max_loop_iterations: 10,
        }
    }
}

impl AnomalyDetectionConfig {
    /// Create a sensitive configuration (lower thresholds)
    #[must_use]
    pub fn sensitive() -> Self {
        Self {
            warning_threshold_std: 1.5,
            critical_threshold_std: 2.0,
            min_samples: 5,
            ..Default::default()
        }
    }

    /// Create a relaxed configuration (higher thresholds)
    #[must_use]
    pub fn relaxed() -> Self {
        Self {
            warning_threshold_std: 3.0,
            critical_threshold_std: 4.0,
            min_samples: 20,
            ..Default::default()
        }
    }

    /// Create a new `AnomalyDetectionConfig` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the warning threshold in standard deviations.
    ///
    /// Values beyond this threshold trigger warnings.
    /// Default: 2.0.
    #[must_use]
    pub fn with_warning_threshold_std(mut self, threshold: f64) -> Self {
        self.warning_threshold_std = threshold;
        self
    }

    /// Set the critical threshold in standard deviations.
    ///
    /// Values beyond this threshold trigger critical alerts.
    /// Default: 3.0.
    #[must_use]
    pub fn with_critical_threshold_std(mut self, threshold: f64) -> Self {
        self.critical_threshold_std = threshold;
        self
    }

    /// Set the minimum samples needed for reliable detection.
    ///
    /// Detection is skipped if fewer samples are available.
    /// Default: 10.
    #[must_use]
    pub fn with_min_samples(mut self, min: usize) -> Self {
        self.min_samples = min;
        self
    }

    /// Enable or disable duration anomaly detection.
    ///
    /// Default: true.
    #[must_use]
    pub fn with_detect_duration(mut self, detect: bool) -> Self {
        self.detect_duration = detect;
        self
    }

    /// Enable or disable token anomaly detection.
    ///
    /// Default: true.
    #[must_use]
    pub fn with_detect_tokens(mut self, detect: bool) -> Self {
        self.detect_tokens = detect;
        self
    }

    /// Enable or disable error anomaly detection.
    ///
    /// Default: true.
    #[must_use]
    pub fn with_detect_errors(mut self, detect: bool) -> Self {
        self.detect_errors = detect;
        self
    }

    /// Enable or disable path anomaly detection.
    ///
    /// Default: true.
    #[must_use]
    pub fn with_detect_paths(mut self, detect: bool) -> Self {
        self.detect_paths = detect;
        self
    }

    /// Enable or disable loop anomaly detection.
    ///
    /// Default: true.
    #[must_use]
    pub fn with_detect_loops(mut self, detect: bool) -> Self {
        self.detect_loops = detect;
        self
    }

    /// Set the minimum path frequency to be considered "normal".
    ///
    /// Paths appearing less frequently than this are considered anomalous.
    /// Default: 0.05 (5% of executions).
    #[must_use]
    pub fn with_min_path_frequency(mut self, frequency: f64) -> Self {
        self.min_path_frequency = frequency;
        self
    }

    /// Set the maximum loop iterations before warning.
    ///
    /// Default: 10.
    #[must_use]
    pub fn with_max_loop_iterations(mut self, max: usize) -> Self {
        self.max_loop_iterations = max;
        self
    }
}

/// Anomaly detector trained on historical execution data
pub struct AnomalyDetector {
    /// Configuration
    config: AnomalyDetectionConfig,
    /// Baseline statistics
    stats: ExecutionStats,
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyDetector {
    /// Create a new detector with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AnomalyDetectionConfig::default(),
            stats: ExecutionStats::default(),
        }
    }

    /// Create detector with custom configuration
    #[must_use]
    pub fn with_config(config: AnomalyDetectionConfig) -> Self {
        Self {
            config,
            stats: ExecutionStats::default(),
        }
    }

    /// Train detector on historical traces
    #[must_use]
    pub fn train(traces: &[crate::introspection::ExecutionTrace]) -> Self {
        let mut detector = Self::new();
        detector.update(traces);
        detector
    }

    /// Train with custom config
    #[must_use]
    pub fn train_with_config(
        traces: &[crate::introspection::ExecutionTrace],
        config: AnomalyDetectionConfig,
    ) -> Self {
        let mut detector = Self::with_config(config);
        detector.update(traces);
        detector
    }

    /// Update detector with new traces
    pub fn update(&mut self, traces: &[crate::introspection::ExecutionTrace]) {
        if traces.is_empty() {
            return;
        }

        // Collect metrics
        let durations: Vec<f64> = traces.iter().map(|t| t.total_duration_ms as f64).collect();
        let tokens: Vec<f64> = traces.iter().map(|t| t.total_tokens as f64).collect();
        let node_counts: Vec<f64> = traces
            .iter()
            .map(|t| t.nodes_executed.len() as f64)
            .collect();
        let error_rates: Vec<f64> = traces
            .iter()
            .map(|t| {
                if t.nodes_executed.is_empty() {
                    0.0
                } else {
                    t.errors.len() as f64 / t.nodes_executed.len() as f64
                }
            })
            .collect();
        let tool_calls: Vec<f64> = traces
            .iter()
            .map(|t| {
                t.nodes_executed
                    .iter()
                    .map(|e| e.tools_called.len())
                    .sum::<usize>() as f64
            })
            .collect();

        // Calculate statistics
        self.stats.avg_duration_ms = mean(&durations);
        self.stats.std_duration_ms = std_dev(&durations);
        self.stats.avg_tokens = mean(&tokens);
        self.stats.std_tokens = std_dev(&tokens);
        self.stats.avg_node_count = mean(&node_counts);
        self.stats.avg_error_rate = mean(&error_rates);
        self.stats.avg_tool_calls = mean(&tool_calls);
        self.stats.sample_count = traces.len();

        // Calculate per-node statistics
        let mut node_durations: HashMap<String, Vec<f64>> = HashMap::new();
        let mut node_tokens: HashMap<String, Vec<f64>> = HashMap::new();
        let mut node_counts_per_trace: HashMap<String, Vec<usize>> = HashMap::new();

        for trace in traces {
            let mut trace_node_counts: HashMap<&str, usize> = HashMap::new();

            for exec in &trace.nodes_executed {
                node_durations
                    .entry(exec.node.clone())
                    .or_default()
                    .push(exec.duration_ms as f64);
                node_tokens
                    .entry(exec.node.clone())
                    .or_default()
                    .push(exec.tokens_used as f64);
                *trace_node_counts.entry(&exec.node).or_insert(0) += 1;
            }

            for (node, count) in trace_node_counts {
                node_counts_per_trace
                    .entry(node.to_string())
                    .or_default()
                    .push(count);
            }
        }

        for (node, durations) in &node_durations {
            let tokens = node_tokens.get(node).map(|v| v.as_slice()).unwrap_or(&[]);
            let counts = node_counts_per_trace
                .get(node)
                .map(|v| v.iter().map(|x| *x as f64).collect::<Vec<_>>())
                .unwrap_or_default();

            self.stats.node_stats.insert(
                node.clone(),
                NodeExecutionStats {
                    avg_duration_ms: mean(durations),
                    std_duration_ms: std_dev(durations),
                    avg_tokens: mean(tokens),
                    std_tokens: std_dev(tokens),
                    sample_count: durations.len(),
                    avg_executions_per_trace: mean(&counts),
                },
            );
        }

        // Calculate common paths
        let mut path_counts: HashMap<Vec<String>, usize> = HashMap::new();
        for trace in traces {
            let path: Vec<String> = trace.nodes_executed.iter().map(|e| e.node.clone()).fold(
                Vec::new(),
                |mut acc, node| {
                    if acc.last() != Some(&node) {
                        acc.push(node);
                    }
                    acc
                },
            );
            *path_counts.entry(path).or_insert(0) += 1;
        }

        // M-221: Guard against division by zero when traces is empty
        let total_traces = traces.len().max(1);
        self.stats.common_paths = path_counts
            .into_iter()
            .map(|(path, count)| (path, count as f64 / total_traces as f64))
            .collect();
        self.stats
            .common_paths
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// Detect anomalies in an execution trace
    #[must_use]
    pub fn detect(&self, trace: &crate::introspection::ExecutionTrace) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Check if we have enough data for reliable detection
        if self.stats.sample_count < self.config.min_samples {
            return anomalies; // Not enough data
        }

        // Check duration anomalies
        if self.config.detect_duration {
            anomalies.extend(self.check_duration_anomalies(trace));
        }

        // Check token anomalies
        if self.config.detect_tokens {
            anomalies.extend(self.check_token_anomalies(trace));
        }

        // Check error anomalies
        if self.config.detect_errors {
            anomalies.extend(self.check_error_anomalies(trace));
        }

        // Check path anomalies
        if self.config.detect_paths {
            anomalies.extend(self.check_path_anomalies(trace));
        }

        // Check loop anomalies
        if self.config.detect_loops {
            anomalies.extend(self.check_loop_anomalies(trace));
        }

        // Sort by severity (critical first)
        anomalies.sort_by(|a, b| b.severity.cmp(&a.severity));

        anomalies
    }

    /// Get the baseline statistics
    #[must_use]
    pub fn stats(&self) -> &ExecutionStats {
        &self.stats
    }

    /// Check if detector has enough data
    #[must_use]
    pub fn is_trained(&self) -> bool {
        self.stats.sample_count >= self.config.min_samples
    }

    /// Generate a summary report
    #[must_use]
    pub fn report(&self, anomalies: &[Anomaly]) -> String {
        let mut lines = vec![format!(
            "Anomaly Detection Report ({} anomalies detected)",
            anomalies.len()
        )];
        lines.push(String::new());

        let critical = anomalies.iter().filter(|a| a.is_critical()).count();
        let warnings = anomalies
            .iter()
            .filter(|a| a.severity == AnomalySeverity::Warning)
            .count();
        let info = anomalies
            .iter()
            .filter(|a| a.severity == AnomalySeverity::Info)
            .count();

        lines.push(format!(
            "Critical: {}, Warnings: {}, Info: {}",
            critical, warnings, info
        ));
        lines.push(String::new());

        if !anomalies.is_empty() {
            lines.push("Anomalies:".to_string());
            for (i, anomaly) in anomalies.iter().enumerate() {
                lines.push(format!("  {}. {}", i + 1, anomaly.description()));
                if let Some(ref suggestion) = anomaly.suggestion {
                    lines.push(format!("     Suggestion: {}", suggestion));
                }
            }
        }

        lines.push(String::new());
        lines.push(format!(
            "Baseline statistics (n={}):",
            self.stats.sample_count
        ));
        lines.push(format!(
            "  Duration: {:.0}ms (std: {:.0}ms)",
            self.stats.avg_duration_ms, self.stats.std_duration_ms
        ));
        lines.push(format!(
            "  Tokens: {:.0} (std: {:.0})",
            self.stats.avg_tokens, self.stats.std_tokens
        ));
        lines.push(format!(
            "  Error rate: {:.1}%",
            self.stats.avg_error_rate * 100.0
        ));

        lines.join("\n")
    }

    fn check_duration_anomalies(
        &self,
        trace: &crate::introspection::ExecutionTrace,
    ) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Total duration
        let actual_duration = trace.total_duration_ms as f64;
        if self.stats.std_duration_ms > 0.0 {
            let z_score =
                (actual_duration - self.stats.avg_duration_ms) / self.stats.std_duration_ms;

            if z_score > self.config.critical_threshold_std {
                anomalies.push(
                    Anomaly::new(
                        AnomalyMetric::TotalDuration,
                        self.stats.avg_duration_ms,
                        actual_duration,
                        AnomalySeverity::Critical,
                    )
                    .with_explanation(format!(
                        "Execution took {:.1}x longer than average ({:.0}ms vs {:.0}ms)",
                        actual_duration / self.stats.avg_duration_ms,
                        actual_duration,
                        self.stats.avg_duration_ms
                    ))
                    .with_suggestion("Investigate slow nodes or high token usage"),
                );
            } else if z_score > self.config.warning_threshold_std {
                anomalies.push(
                    Anomaly::new(
                        AnomalyMetric::TotalDuration,
                        self.stats.avg_duration_ms,
                        actual_duration,
                        AnomalySeverity::Warning,
                    )
                    .with_explanation(format!(
                        "Execution took {:.1}x longer than average",
                        actual_duration / self.stats.avg_duration_ms
                    )),
                );
            }
        }

        // Per-node duration
        for exec in &trace.nodes_executed {
            if let Some(node_stats) = self.stats.node_stats.get(&exec.node) {
                if node_stats.std_duration_ms > 0.0 {
                    let z_score = (exec.duration_ms as f64 - node_stats.avg_duration_ms)
                        / node_stats.std_duration_ms;

                    if z_score > self.config.critical_threshold_std {
                        anomalies.push(
                            Anomaly::new(
                                AnomalyMetric::NodeDuration,
                                node_stats.avg_duration_ms,
                                exec.duration_ms as f64,
                                AnomalySeverity::Critical,
                            )
                            .with_node(&exec.node)
                            .with_explanation(format!(
                                "Node '{}' took {:.1}x longer than average",
                                exec.node,
                                exec.duration_ms as f64 / node_stats.avg_duration_ms
                            )),
                        );
                    } else if z_score > self.config.warning_threshold_std {
                        anomalies.push(
                            Anomaly::new(
                                AnomalyMetric::NodeDuration,
                                node_stats.avg_duration_ms,
                                exec.duration_ms as f64,
                                AnomalySeverity::Warning,
                            )
                            .with_node(&exec.node)
                            .with_explanation(format!("Node '{}' slower than usual", exec.node)),
                        );
                    }
                }
            }
        }

        anomalies
    }

    fn check_token_anomalies(&self, trace: &crate::introspection::ExecutionTrace) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Total tokens
        let actual_tokens = trace.total_tokens as f64;
        if self.stats.std_tokens > 0.0 {
            let z_score = (actual_tokens - self.stats.avg_tokens) / self.stats.std_tokens;

            if z_score > self.config.critical_threshold_std {
                anomalies.push(
                    Anomaly::new(
                        AnomalyMetric::TotalTokens,
                        self.stats.avg_tokens,
                        actual_tokens,
                        AnomalySeverity::Critical,
                    )
                    .with_explanation(format!(
                        "Token usage {:.1}x higher than average ({:.0} vs {:.0})",
                        actual_tokens / self.stats.avg_tokens,
                        actual_tokens,
                        self.stats.avg_tokens
                    ))
                    .with_suggestion("Consider reducing context size or summarizing input"),
                );
            } else if z_score > self.config.warning_threshold_std {
                anomalies.push(
                    Anomaly::new(
                        AnomalyMetric::TotalTokens,
                        self.stats.avg_tokens,
                        actual_tokens,
                        AnomalySeverity::Warning,
                    )
                    .with_explanation(format!(
                        "Token usage {:.1}x higher than average",
                        actual_tokens / self.stats.avg_tokens
                    )),
                );
            }
        }

        anomalies
    }

    fn check_error_anomalies(&self, trace: &crate::introspection::ExecutionTrace) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Error count
        let error_count = trace.errors.len();
        if error_count > 0 {
            let actual_error_rate = if trace.nodes_executed.is_empty() {
                1.0
            } else {
                error_count as f64 / trace.nodes_executed.len() as f64
            };

            // Any errors when average is very low
            if self.stats.avg_error_rate < 0.05 && actual_error_rate > 0.1 {
                anomalies.push(
                    Anomaly::new(
                        AnomalyMetric::ErrorRate,
                        self.stats.avg_error_rate,
                        actual_error_rate,
                        AnomalySeverity::Critical,
                    )
                    .with_explanation(format!(
                        "{} errors detected (error rate: {:.1}% vs avg {:.1}%)",
                        error_count,
                        actual_error_rate * 100.0,
                        self.stats.avg_error_rate * 100.0
                    ))
                    .with_suggestion("Investigate error causes and add error handling"),
                );
            } else if actual_error_rate > self.stats.avg_error_rate * 2.0 {
                anomalies.push(
                    Anomaly::new(
                        AnomalyMetric::ErrorRate,
                        self.stats.avg_error_rate,
                        actual_error_rate,
                        AnomalySeverity::Warning,
                    )
                    .with_explanation(format!(
                        "Error rate {:.1}% is higher than average {:.1}%",
                        actual_error_rate * 100.0,
                        self.stats.avg_error_rate * 100.0
                    )),
                );
            }
        }

        anomalies
    }

    fn check_path_anomalies(&self, trace: &crate::introspection::ExecutionTrace) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Extract current path
        let current_path: Vec<String> = trace.nodes_executed.iter().map(|e| e.node.clone()).fold(
            Vec::new(),
            |mut acc, node| {
                if acc.last() != Some(&node) {
                    acc.push(node);
                }
                acc
            },
        );

        // Check if path is in common paths
        let path_frequency = self
            .stats
            .common_paths
            .iter()
            .find(|(path, _)| path == &current_path)
            .map(|(_, freq)| *freq)
            .unwrap_or(0.0);

        if path_frequency < self.config.min_path_frequency && !self.stats.common_paths.is_empty() {
            let severity = if path_frequency == 0.0 {
                AnomalySeverity::Warning
            } else {
                AnomalySeverity::Info
            };

            anomalies.push(
                Anomaly::new(
                    AnomalyMetric::ExecutionPath,
                    self.config.min_path_frequency * 100.0,
                    path_frequency * 100.0,
                    severity,
                )
                .with_explanation(format!(
                    "Unusual execution path: {} (seen in {:.1}% of executions)",
                    current_path.join(" -> "),
                    path_frequency * 100.0
                )),
            );
        }

        anomalies
    }

    fn check_loop_anomalies(&self, trace: &crate::introspection::ExecutionTrace) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Count executions per node
        let mut node_counts: HashMap<&str, usize> = HashMap::new();
        for exec in &trace.nodes_executed {
            *node_counts.entry(&exec.node).or_insert(0) += 1;
        }

        // Check for loop anomalies
        for (node, count) in node_counts {
            if count > self.config.max_loop_iterations {
                let severity = if count > self.config.max_loop_iterations * 2 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::Warning
                };

                anomalies.push(
                    Anomaly::new(
                        AnomalyMetric::LoopIterations,
                        self.config.max_loop_iterations as f64,
                        count as f64,
                        severity,
                    )
                    .with_node(node)
                    .with_explanation(format!(
                        "Node '{}' executed {} times (max expected: {})",
                        node, count, self.config.max_loop_iterations
                    ))
                    .with_suggestion("Add loop termination condition or iteration limit"),
                );
            } else if let Some(node_stats) = self.stats.node_stats.get(node) {
                if node_stats.avg_executions_per_trace > 0.0 {
                    let ratio = count as f64 / node_stats.avg_executions_per_trace;
                    if ratio > 3.0 {
                        anomalies.push(
                            Anomaly::new(
                                AnomalyMetric::LoopIterations,
                                node_stats.avg_executions_per_trace,
                                count as f64,
                                AnomalySeverity::Warning,
                            )
                            .with_node(node)
                            .with_explanation(format!(
                                "Node '{}' executed {:.0}x more than average",
                                node, ratio
                            )),
                        );
                    }
                }
            }
        }

        anomalies
    }
}

// Helper functions

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let avg = mean(values);
    let variance =
        values.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

fn format_value(value: f64) -> String {
    if value >= MILLION {
        format!("{:.1}M", value / MILLION)
    } else if value >= THOUSAND {
        format!("{:.1}K", value / THOUSAND)
    } else {
        format!("{:.1}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ErrorTrace, ExecutionTrace, ExecutionTraceBuilder, NodeExecution};

    fn create_normal_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("input", 100).with_tokens(500))
            .add_node_execution(NodeExecution::new("reasoning", 500).with_tokens(2000))
            .add_node_execution(NodeExecution::new("output", 100).with_tokens(500))
            .total_duration_ms(700)
            .total_tokens(3000)
            .completed(true)
            .build()
    }

    fn create_slow_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("input", 100).with_tokens(500))
            .add_node_execution(NodeExecution::new("reasoning", 5000).with_tokens(10000))
            .add_node_execution(NodeExecution::new("output", 100).with_tokens(500))
            .total_duration_ms(5200)
            .total_tokens(11000)
            .completed(true)
            .build()
    }

    fn create_error_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(
                NodeExecution::new("failing", 500)
                    .with_tokens(1000)
                    .with_error("Connection failed"),
            )
            .add_error(ErrorTrace::new("failing", "Connection failed"))
            .total_duration_ms(500)
            .total_tokens(1000)
            .completed(false)
            .build()
    }

    fn create_loop_trace() -> ExecutionTrace {
        let mut builder = ExecutionTraceBuilder::new();
        for i in 0..20 {
            builder = builder.add_node_execution(
                NodeExecution::new("loop_node", 50)
                    .with_index(i)
                    .with_tokens(100),
            );
        }
        builder
            .total_duration_ms(1000)
            .total_tokens(2000)
            .completed(true)
            .build()
    }

    fn create_training_set() -> Vec<ExecutionTrace> {
        // Create varied traces with some variance for std dev calculation
        (0..20)
            .map(|i| {
                let duration_variation = 600 + (i * 10) as u64; // 600-790ms range
                let token_variation = 2800 + (i * 20) as u64; // 2800-3180 token range
                ExecutionTraceBuilder::new()
                    .add_node_execution(
                        NodeExecution::new("input", 80 + (i * 2) as u64).with_tokens(500),
                    )
                    .add_node_execution(
                        NodeExecution::new("reasoning", 450 + (i * 5) as u64)
                            .with_tokens(1800 + (i * 15) as u64),
                    )
                    .add_node_execution(
                        NodeExecution::new("output", 70 + (i * 3) as u64).with_tokens(500),
                    )
                    .total_duration_ms(duration_variation)
                    .total_tokens(token_variation)
                    .completed(true)
                    .build()
            })
            .collect()
    }

    #[test]
    fn test_anomaly_creation() {
        let anomaly = Anomaly::new(
            AnomalyMetric::TotalDuration,
            1000.0,
            5000.0,
            AnomalySeverity::Critical,
        )
        .with_explanation("5x slower than expected")
        .with_suggestion("Optimize the slow node");

        assert_eq!(anomaly.expected_value, 1000.0);
        assert_eq!(anomaly.actual_value, 5000.0);
        assert!(anomaly.is_critical());
    }

    #[test]
    fn test_anomaly_description() {
        let anomaly = Anomaly::new(
            AnomalyMetric::TotalDuration,
            1000.0,
            2000.0,
            AnomalySeverity::Warning,
        )
        .with_explanation("Slower than usual")
        .with_node("reasoning");

        let desc = anomaly.description();
        assert!(desc.contains("total_duration"));
        assert!(desc.contains("reasoning"));
        assert!(desc.contains("WARN"));
    }

    #[test]
    fn test_anomaly_severity_ordering() {
        assert!(AnomalySeverity::Critical > AnomalySeverity::Warning);
        assert!(AnomalySeverity::Warning > AnomalySeverity::Info);
    }

    #[test]
    fn test_detector_train_empty() {
        let detector = AnomalyDetector::train(&[]);
        assert!(!detector.is_trained());
        assert_eq!(detector.stats().sample_count, 0);
    }

    #[test]
    fn test_detector_train_single() {
        let traces = vec![create_normal_trace()];
        let detector = AnomalyDetector::train(&traces);

        assert_eq!(detector.stats().sample_count, 1);
        // Not enough for reliable detection
        assert!(!detector.is_trained());
    }

    #[test]
    fn test_detector_train_sufficient() {
        let traces = create_training_set();
        let detector = AnomalyDetector::train(&traces);

        assert!(detector.is_trained());
        assert!(detector.stats().avg_duration_ms > 0.0);
        assert!(detector.stats().avg_tokens > 0.0);
    }

    #[test]
    fn test_detector_no_anomalies_normal() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        let test_trace = create_normal_trace();
        let anomalies = detector.detect(&test_trace);

        // Normal trace should have few or no anomalies
        let critical = anomalies.iter().filter(|a| a.is_critical()).count();
        assert_eq!(critical, 0);
    }

    #[test]
    fn test_detector_detects_slow_execution() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        let slow_trace = create_slow_trace();
        let anomalies = detector.detect(&slow_trace);

        // Should detect duration anomaly
        let has_duration_anomaly = anomalies
            .iter()
            .any(|a| a.metric == AnomalyMetric::TotalDuration);
        assert!(has_duration_anomaly);
    }

    #[test]
    fn test_detector_detects_high_tokens() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        let slow_trace = create_slow_trace(); // Also has high tokens
        let anomalies = detector.detect(&slow_trace);

        let has_token_anomaly = anomalies
            .iter()
            .any(|a| a.metric == AnomalyMetric::TotalTokens);
        assert!(has_token_anomaly);
    }

    #[test]
    fn test_detector_detects_errors() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        let error_trace = create_error_trace();
        let anomalies = detector.detect(&error_trace);

        let has_error_anomaly = anomalies
            .iter()
            .any(|a| a.metric == AnomalyMetric::ErrorRate || a.metric == AnomalyMetric::ErrorCount);
        assert!(has_error_anomaly);
    }

    #[test]
    fn test_detector_detects_loops() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        let loop_trace = create_loop_trace();
        let anomalies = detector.detect(&loop_trace);

        let has_loop_anomaly = anomalies
            .iter()
            .any(|a| a.metric == AnomalyMetric::LoopIterations);
        assert!(has_loop_anomaly);
    }

    #[test]
    fn test_detector_detects_unusual_path() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        // Create trace with different path
        let unusual = ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("special_node", 100).with_tokens(500))
            .add_node_execution(NodeExecution::new("another_node", 100).with_tokens(500))
            .total_duration_ms(200)
            .total_tokens(1000)
            .completed(true)
            .build();

        let anomalies = detector.detect(&unusual);

        let has_path_anomaly = anomalies
            .iter()
            .any(|a| a.metric == AnomalyMetric::ExecutionPath);
        assert!(has_path_anomaly);
    }

    #[test]
    fn test_execution_stats_p95() {
        let mut stats = ExecutionStats::new();
        stats.avg_duration_ms = 1000.0;
        stats.std_duration_ms = 200.0;

        // p95 should be avg + 1.645 * std
        let p95 = stats.p95_duration_ms();
        assert!((p95 - 1329.0).abs() < 1.0);
    }

    #[test]
    fn test_config_presets() {
        let sensitive = AnomalyDetectionConfig::sensitive();
        assert!(sensitive.warning_threshold_std < 2.0);

        let relaxed = AnomalyDetectionConfig::relaxed();
        assert!(relaxed.warning_threshold_std > 2.0);
    }

    #[test]
    fn test_anomaly_metric_display() {
        assert_eq!(AnomalyMetric::TotalDuration.to_string(), "total_duration");
        assert_eq!(AnomalyMetric::ErrorRate.to_string(), "error_rate");
    }

    #[test]
    fn test_anomaly_json_roundtrip() {
        let anomaly = Anomaly::new(
            AnomalyMetric::TotalDuration,
            1000.0,
            2000.0,
            AnomalySeverity::Warning,
        );

        let json = anomaly.to_json().unwrap();
        let parsed = Anomaly::from_json(&json).unwrap();

        assert_eq!(parsed.metric, anomaly.metric);
        assert_eq!(parsed.expected_value, anomaly.expected_value);
    }

    #[test]
    fn test_detector_report() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        let slow_trace = create_slow_trace();
        let anomalies = detector.detect(&slow_trace);

        let report = detector.report(&anomalies);
        assert!(report.contains("Anomaly Detection Report"));
        assert!(report.contains("Baseline statistics"));
    }

    #[test]
    fn test_format_value() {
        assert_eq!(format_value(500.0), "500.0");
        assert_eq!(format_value(1500.0), "1.5K");
        assert_eq!(format_value(1500000.0), "1.5M");
    }

    #[test]
    fn test_node_stats_collected() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        // Should have stats for nodes in training set
        assert!(detector.stats().node_stats.contains_key("reasoning"));
        assert!(detector.stats().node_stats.contains_key("input"));
        assert!(detector.stats().node_stats.contains_key("output"));
    }

    #[test]
    fn test_common_paths_collected() {
        let training = create_training_set();
        let detector = AnomalyDetector::train(&training);

        assert!(!detector.stats().common_paths.is_empty());
        // Most common path should have high frequency
        let (_, freq) = &detector.stats().common_paths[0];
        assert!(*freq > 0.5);
    }

    #[test]
    fn test_detector_update() {
        let mut detector = AnomalyDetector::new();
        assert_eq!(detector.stats().sample_count, 0);

        let traces = create_training_set();
        detector.update(&traces);
        assert_eq!(detector.stats().sample_count, 20);
    }

    #[test]
    fn test_anomaly_detection_config_builder_new() {
        let config = AnomalyDetectionConfig::new();
        let default_config = AnomalyDetectionConfig::default();
        assert_eq!(
            config.warning_threshold_std,
            default_config.warning_threshold_std
        );
        assert_eq!(
            config.critical_threshold_std,
            default_config.critical_threshold_std
        );
        assert_eq!(config.min_samples, default_config.min_samples);
    }

    #[test]
    fn test_anomaly_detection_config_builder_full_chain() {
        let config = AnomalyDetectionConfig::new()
            .with_warning_threshold_std(1.5)
            .with_critical_threshold_std(2.5)
            .with_min_samples(20)
            .with_detect_duration(false)
            .with_detect_tokens(false)
            .with_detect_errors(true)
            .with_detect_paths(false)
            .with_detect_loops(false)
            .with_min_path_frequency(0.10)
            .with_max_loop_iterations(5);

        assert_eq!(config.warning_threshold_std, 1.5);
        assert_eq!(config.critical_threshold_std, 2.5);
        assert_eq!(config.min_samples, 20);
        assert!(!config.detect_duration);
        assert!(!config.detect_tokens);
        assert!(config.detect_errors);
        assert!(!config.detect_paths);
        assert!(!config.detect_loops);
        assert_eq!(config.min_path_frequency, 0.10);
        assert_eq!(config.max_loop_iterations, 5);
    }

    #[test]
    fn test_anomaly_detection_config_builder_partial_chain() {
        // Test that partial builder chains preserve defaults
        let config = AnomalyDetectionConfig::new()
            .with_warning_threshold_std(1.0)
            .with_detect_errors(false);

        // Custom values
        assert_eq!(config.warning_threshold_std, 1.0);
        assert!(!config.detect_errors);

        // Default values preserved
        assert_eq!(config.critical_threshold_std, 3.0);
        assert_eq!(config.min_samples, 10);
        assert!(config.detect_duration);
        assert!(config.detect_tokens);
        assert!(config.detect_paths);
        assert!(config.detect_loops);
        assert_eq!(config.min_path_frequency, 0.05);
        assert_eq!(config.max_loop_iterations, 10);
    }
}
