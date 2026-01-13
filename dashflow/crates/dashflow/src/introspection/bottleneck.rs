//! Bottleneck Detection
//!
//! This module provides types for detecting and analyzing performance bottlenecks
//! in graph execution.

use super::trace::{ExecutionTrace, NodeExecution};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Bottleneck Detection
// ============================================================================

/// Type of bottleneck detected in execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BottleneckMetric {
    /// Node has high latency
    Latency,
    /// Node uses excessive tokens
    TokenUsage,
    /// Node has high error rate
    ErrorRate,
    /// Node is called too frequently (potential infinite loop)
    HighFrequency,
    /// Node has high variance in execution time
    HighVariance,
}

impl std::fmt::Display for BottleneckMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Latency => write!(f, "latency"),
            Self::TokenUsage => write!(f, "token_usage"),
            Self::ErrorRate => write!(f, "error_rate"),
            Self::HighFrequency => write!(f, "high_frequency"),
            Self::HighVariance => write!(f, "high_variance"),
        }
    }
}

/// Severity level for bottleneck detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum BottleneckSeverity {
    /// Minor - noticeable but not urgent
    #[default]
    Minor,
    /// Moderate - should be addressed
    Moderate,
    /// Severe - significantly impacts performance
    Severe,
    /// Critical - requires immediate attention
    Critical,
}

impl std::fmt::Display for BottleneckSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Minor => write!(f, "minor"),
            Self::Moderate => write!(f, "moderate"),
            Self::Severe => write!(f, "severe"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// A detected bottleneck in execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    /// Node where the bottleneck was detected
    pub node: String,
    /// Type of metric indicating the bottleneck
    pub metric: BottleneckMetric,
    /// Measured value for the metric
    pub value: f64,
    /// Threshold that was exceeded
    pub threshold: f64,
    /// Severity of the bottleneck
    pub severity: BottleneckSeverity,
    /// Human-readable description of the problem
    pub description: String,
    /// Suggested action to address the bottleneck
    pub suggestion: String,
    /// Percentage of total (time, tokens, etc.) consumed by this node
    pub percentage_of_total: Option<f64>,
}

impl Bottleneck {
    /// Create a new bottleneck
    #[must_use]
    pub fn new(
        node: impl Into<String>,
        metric: BottleneckMetric,
        value: f64,
        threshold: f64,
        severity: BottleneckSeverity,
        description: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            node: node.into(),
            metric,
            value,
            threshold,
            severity,
            description: description.into(),
            suggestion: suggestion.into(),
            percentage_of_total: None,
        }
    }

    /// Create a builder for constructing bottlenecks
    #[must_use]
    pub fn builder() -> BottleneckBuilder {
        BottleneckBuilder::new()
    }

    /// Set the percentage of total
    #[must_use]
    pub fn with_percentage(mut self, percentage: f64) -> Self {
        self.percentage_of_total = Some(percentage);
        self
    }

    /// Check if this is a critical bottleneck
    #[must_use]
    pub fn is_critical(&self) -> bool {
        self.severity == BottleneckSeverity::Critical
    }

    /// Check if this is severe or critical
    #[must_use]
    pub fn is_severe_or_critical(&self) -> bool {
        self.severity >= BottleneckSeverity::Severe
    }

    /// Get a formatted summary of the bottleneck
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} in '{}': {} (value: {:.2}, threshold: {:.2})",
            self.severity, self.metric, self.node, self.description, self.value, self.threshold
        )
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

/// Builder for [`Bottleneck`].
///
/// # Required Fields
/// - `node` - The node name where bottleneck occurred
/// - `metric` - The type of metric that exceeded threshold
/// - `description` - Human-readable description of the bottleneck
/// - `suggestion` - Suggested remediation action
///
/// # Optional Fields (have defaults)
/// - `value` - Measured metric value (default: 0.0)
/// - `threshold` - Threshold that was exceeded (default: 0.0)
/// - `severity` - Bottleneck severity level (default: Medium)
/// - `percentage_of_total` - What percentage of total time this represents
///
/// # Example
/// ```rust,ignore
/// let bottleneck = BottleneckBuilder::new()
///     .node("llm_call")
///     .metric(BottleneckMetric::Latency)
///     .description("LLM call taking too long")
///     .suggestion("Consider caching or batching requests")
///     .build()?;
/// ```
#[derive(Debug, Default)]
pub struct BottleneckBuilder {
    node: Option<String>,
    metric: Option<BottleneckMetric>,
    value: f64,
    threshold: f64,
    severity: BottleneckSeverity,
    description: Option<String>,
    suggestion: Option<String>,
    percentage_of_total: Option<f64>,
}

impl BottleneckBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the node name
    #[must_use]
    pub fn node(mut self, node: impl Into<String>) -> Self {
        self.node = Some(node.into());
        self
    }

    /// Set the metric type
    #[must_use]
    pub fn metric(mut self, metric: BottleneckMetric) -> Self {
        self.metric = Some(metric);
        self
    }

    /// Set the measured value
    #[must_use]
    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
        self
    }

    /// Set the threshold
    #[must_use]
    pub fn threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set the severity
    #[must_use]
    pub fn severity(mut self, severity: BottleneckSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the description
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the suggestion
    #[must_use]
    pub fn suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Set the percentage of total
    #[must_use]
    pub fn percentage_of_total(mut self, percentage: f64) -> Self {
        self.percentage_of_total = Some(percentage);
        self
    }

    /// Build the bottleneck
    ///
    /// # Errors
    ///
    /// Returns error if required fields are missing
    pub fn build(self) -> Result<Bottleneck, &'static str> {
        let node = self.node.ok_or("node is required")?;
        let metric = self.metric.ok_or("metric is required")?;
        let description = self.description.ok_or("description is required")?;
        let suggestion = self.suggestion.ok_or("suggestion is required")?;

        Ok(Bottleneck {
            node,
            metric,
            value: self.value,
            threshold: self.threshold,
            severity: self.severity,
            description,
            suggestion,
            percentage_of_total: self.percentage_of_total,
        })
    }
}

/// Configuration for bottleneck detection thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckThresholds {
    /// Percentage of total time a single node can take before being flagged
    pub latency_percentage_minor: f64,
    /// Percentage for moderate severity
    pub latency_percentage_moderate: f64,
    /// Percentage for severe
    pub latency_percentage_severe: f64,
    /// Percentage for critical
    pub latency_percentage_critical: f64,

    /// Absolute latency in ms for a single execution to be flagged
    pub latency_absolute_minor_ms: u64,
    /// Absolute for moderate
    pub latency_absolute_moderate_ms: u64,
    /// Absolute for severe
    pub latency_absolute_severe_ms: u64,
    /// Absolute for critical
    pub latency_absolute_critical_ms: u64,

    /// Percentage of total tokens a single node can use before being flagged
    pub token_percentage_minor: f64,
    /// Token percentage for moderate
    pub token_percentage_moderate: f64,
    /// Token percentage for severe
    pub token_percentage_severe: f64,
    /// Token percentage for critical
    pub token_percentage_critical: f64,

    /// Error rate (0.0-1.0) thresholds
    pub error_rate_minor: f64,
    /// Error rate for moderate
    pub error_rate_moderate: f64,
    /// Error rate for severe
    pub error_rate_severe: f64,
    /// Error rate for critical
    pub error_rate_critical: f64,

    /// Number of executions of same node that triggers high frequency warning
    pub frequency_minor: usize,
    /// Frequency for moderate
    pub frequency_moderate: usize,
    /// Frequency for severe
    pub frequency_severe: usize,
    /// Frequency for critical
    pub frequency_critical: usize,

    /// Coefficient of variation (std dev / mean) for high variance detection
    pub variance_coefficient_minor: f64,
    /// Variance coefficient for moderate
    pub variance_coefficient_moderate: f64,
    /// Variance coefficient for severe
    pub variance_coefficient_severe: f64,
    /// Variance coefficient for critical
    pub variance_coefficient_critical: f64,
}

impl Default for BottleneckThresholds {
    fn default() -> Self {
        Self {
            // Latency percentage thresholds
            latency_percentage_minor: 30.0,
            latency_percentage_moderate: 50.0,
            latency_percentage_severe: 70.0,
            latency_percentage_critical: 85.0,

            // Absolute latency thresholds (ms)
            latency_absolute_minor_ms: 5000,     // 5 seconds
            latency_absolute_moderate_ms: 15000, // 15 seconds
            latency_absolute_severe_ms: 30000,   // 30 seconds
            latency_absolute_critical_ms: 60000, // 60 seconds

            // Token percentage thresholds
            token_percentage_minor: 40.0,
            token_percentage_moderate: 60.0,
            token_percentage_severe: 80.0,
            token_percentage_critical: 90.0,

            // Error rate thresholds
            error_rate_minor: 0.05,    // 5%
            error_rate_moderate: 0.15, // 15%
            error_rate_severe: 0.30,   // 30%
            error_rate_critical: 0.50, // 50%

            // Frequency thresholds (same node execution count)
            frequency_minor: 10,
            frequency_moderate: 25,
            frequency_severe: 50,
            frequency_critical: 100,

            // Variance coefficient thresholds
            variance_coefficient_minor: 0.5,
            variance_coefficient_moderate: 1.0,
            variance_coefficient_severe: 1.5,
            variance_coefficient_critical: 2.0,
        }
    }
}

impl BottleneckThresholds {
    /// Create new thresholds with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create strict thresholds for high-performance requirements
    #[must_use]
    pub fn strict() -> Self {
        Self {
            latency_percentage_minor: 20.0,
            latency_percentage_moderate: 35.0,
            latency_percentage_severe: 50.0,
            latency_percentage_critical: 70.0,

            latency_absolute_minor_ms: 2000,
            latency_absolute_moderate_ms: 5000,
            latency_absolute_severe_ms: 10000,
            latency_absolute_critical_ms: 30000,

            token_percentage_minor: 30.0,
            token_percentage_moderate: 50.0,
            token_percentage_severe: 70.0,
            token_percentage_critical: 85.0,

            error_rate_minor: 0.02,
            error_rate_moderate: 0.05,
            error_rate_severe: 0.10,
            error_rate_critical: 0.25,

            frequency_minor: 5,
            frequency_moderate: 15,
            frequency_severe: 30,
            frequency_critical: 50,

            variance_coefficient_minor: 0.3,
            variance_coefficient_moderate: 0.6,
            variance_coefficient_severe: 1.0,
            variance_coefficient_critical: 1.5,
        }
    }

    /// Create lenient thresholds for less critical systems
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            latency_percentage_minor: 50.0,
            latency_percentage_moderate: 70.0,
            latency_percentage_severe: 85.0,
            latency_percentage_critical: 95.0,

            latency_absolute_minor_ms: 10000,
            latency_absolute_moderate_ms: 30000,
            latency_absolute_severe_ms: 60000,
            latency_absolute_critical_ms: 120000,

            token_percentage_minor: 60.0,
            token_percentage_moderate: 75.0,
            token_percentage_severe: 90.0,
            token_percentage_critical: 95.0,

            error_rate_minor: 0.10,
            error_rate_moderate: 0.25,
            error_rate_severe: 0.50,
            error_rate_critical: 0.75,

            frequency_minor: 20,
            frequency_moderate: 50,
            frequency_severe: 100,
            frequency_critical: 200,

            variance_coefficient_minor: 1.0,
            variance_coefficient_moderate: 1.5,
            variance_coefficient_severe: 2.0,
            variance_coefficient_critical: 3.0,
        }
    }

    /// Get the severity level for a latency percentage
    #[must_use]
    pub fn latency_percentage_severity(&self, percentage: f64) -> Option<BottleneckSeverity> {
        if percentage >= self.latency_percentage_critical {
            Some(BottleneckSeverity::Critical)
        } else if percentage >= self.latency_percentage_severe {
            Some(BottleneckSeverity::Severe)
        } else if percentage >= self.latency_percentage_moderate {
            Some(BottleneckSeverity::Moderate)
        } else if percentage >= self.latency_percentage_minor {
            Some(BottleneckSeverity::Minor)
        } else {
            None
        }
    }

    /// Get the threshold for a latency percentage severity
    #[must_use]
    pub fn latency_percentage_threshold(&self, severity: BottleneckSeverity) -> f64 {
        match severity {
            BottleneckSeverity::Minor => self.latency_percentage_minor,
            BottleneckSeverity::Moderate => self.latency_percentage_moderate,
            BottleneckSeverity::Severe => self.latency_percentage_severe,
            BottleneckSeverity::Critical => self.latency_percentage_critical,
        }
    }

    /// Get the severity level for absolute latency
    #[must_use]
    pub fn latency_absolute_severity(&self, latency_ms: u64) -> Option<BottleneckSeverity> {
        if latency_ms >= self.latency_absolute_critical_ms {
            Some(BottleneckSeverity::Critical)
        } else if latency_ms >= self.latency_absolute_severe_ms {
            Some(BottleneckSeverity::Severe)
        } else if latency_ms >= self.latency_absolute_moderate_ms {
            Some(BottleneckSeverity::Moderate)
        } else if latency_ms >= self.latency_absolute_minor_ms {
            Some(BottleneckSeverity::Minor)
        } else {
            None
        }
    }

    /// Get the threshold for absolute latency severity
    #[must_use]
    pub fn latency_absolute_threshold(&self, severity: BottleneckSeverity) -> u64 {
        match severity {
            BottleneckSeverity::Minor => self.latency_absolute_minor_ms,
            BottleneckSeverity::Moderate => self.latency_absolute_moderate_ms,
            BottleneckSeverity::Severe => self.latency_absolute_severe_ms,
            BottleneckSeverity::Critical => self.latency_absolute_critical_ms,
        }
    }

    /// Get the severity level for a token percentage
    #[must_use]
    pub fn token_percentage_severity(&self, percentage: f64) -> Option<BottleneckSeverity> {
        if percentage >= self.token_percentage_critical {
            Some(BottleneckSeverity::Critical)
        } else if percentage >= self.token_percentage_severe {
            Some(BottleneckSeverity::Severe)
        } else if percentage >= self.token_percentage_moderate {
            Some(BottleneckSeverity::Moderate)
        } else if percentage >= self.token_percentage_minor {
            Some(BottleneckSeverity::Minor)
        } else {
            None
        }
    }

    /// Get the threshold for a token percentage severity
    #[must_use]
    pub fn token_percentage_threshold(&self, severity: BottleneckSeverity) -> f64 {
        match severity {
            BottleneckSeverity::Minor => self.token_percentage_minor,
            BottleneckSeverity::Moderate => self.token_percentage_moderate,
            BottleneckSeverity::Severe => self.token_percentage_severe,
            BottleneckSeverity::Critical => self.token_percentage_critical,
        }
    }

    /// Get the severity level for an error rate
    #[must_use]
    pub fn error_rate_severity(&self, error_rate: f64) -> Option<BottleneckSeverity> {
        if error_rate >= self.error_rate_critical {
            Some(BottleneckSeverity::Critical)
        } else if error_rate >= self.error_rate_severe {
            Some(BottleneckSeverity::Severe)
        } else if error_rate >= self.error_rate_moderate {
            Some(BottleneckSeverity::Moderate)
        } else if error_rate >= self.error_rate_minor {
            Some(BottleneckSeverity::Minor)
        } else {
            None
        }
    }

    /// Get the threshold for an error rate severity
    #[must_use]
    pub fn error_rate_threshold(&self, severity: BottleneckSeverity) -> f64 {
        match severity {
            BottleneckSeverity::Minor => self.error_rate_minor,
            BottleneckSeverity::Moderate => self.error_rate_moderate,
            BottleneckSeverity::Severe => self.error_rate_severe,
            BottleneckSeverity::Critical => self.error_rate_critical,
        }
    }

    /// Get the severity level for execution frequency
    #[must_use]
    pub fn frequency_severity(&self, count: usize) -> Option<BottleneckSeverity> {
        if count >= self.frequency_critical {
            Some(BottleneckSeverity::Critical)
        } else if count >= self.frequency_severe {
            Some(BottleneckSeverity::Severe)
        } else if count >= self.frequency_moderate {
            Some(BottleneckSeverity::Moderate)
        } else if count >= self.frequency_minor {
            Some(BottleneckSeverity::Minor)
        } else {
            None
        }
    }

    /// Get the threshold for frequency severity
    #[must_use]
    pub fn frequency_threshold(&self, severity: BottleneckSeverity) -> usize {
        match severity {
            BottleneckSeverity::Minor => self.frequency_minor,
            BottleneckSeverity::Moderate => self.frequency_moderate,
            BottleneckSeverity::Severe => self.frequency_severe,
            BottleneckSeverity::Critical => self.frequency_critical,
        }
    }

    /// Get the severity level for variance coefficient
    #[must_use]
    pub fn variance_severity(&self, coefficient: f64) -> Option<BottleneckSeverity> {
        if coefficient >= self.variance_coefficient_critical {
            Some(BottleneckSeverity::Critical)
        } else if coefficient >= self.variance_coefficient_severe {
            Some(BottleneckSeverity::Severe)
        } else if coefficient >= self.variance_coefficient_moderate {
            Some(BottleneckSeverity::Moderate)
        } else if coefficient >= self.variance_coefficient_minor {
            Some(BottleneckSeverity::Minor)
        } else {
            None
        }
    }

    /// Get the threshold for variance severity
    #[must_use]
    pub fn variance_threshold(&self, severity: BottleneckSeverity) -> f64 {
        match severity {
            BottleneckSeverity::Minor => self.variance_coefficient_minor,
            BottleneckSeverity::Moderate => self.variance_coefficient_moderate,
            BottleneckSeverity::Severe => self.variance_coefficient_severe,
            BottleneckSeverity::Critical => self.variance_coefficient_critical,
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

/// Result of bottleneck analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckAnalysis {
    /// All detected bottlenecks
    pub bottlenecks: Vec<Bottleneck>,
    /// Number of nodes analyzed
    pub nodes_analyzed: usize,
    /// Total execution time analyzed (ms)
    pub total_duration_ms: u64,
    /// Total tokens analyzed
    pub total_tokens: u64,
    /// Thresholds used for analysis
    pub thresholds: BottleneckThresholds,
    /// Summary of findings
    pub summary: String,
}

impl BottleneckAnalysis {
    /// Create a new bottleneck analysis
    #[must_use]
    pub fn new(thresholds: BottleneckThresholds) -> Self {
        Self {
            bottlenecks: Vec::new(),
            nodes_analyzed: 0,
            total_duration_ms: 0,
            total_tokens: 0,
            thresholds,
            summary: String::new(),
        }
    }

    /// Check if any bottlenecks were detected
    #[must_use]
    pub fn has_bottlenecks(&self) -> bool {
        !self.bottlenecks.is_empty()
    }

    /// Get the number of bottlenecks
    #[must_use]
    pub fn bottleneck_count(&self) -> usize {
        self.bottlenecks.len()
    }

    /// Check if any critical bottlenecks were detected
    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.bottlenecks.iter().any(|b| b.is_critical())
    }

    /// Get all critical bottlenecks
    #[must_use]
    pub fn critical_bottlenecks(&self) -> Vec<&Bottleneck> {
        self.bottlenecks
            .iter()
            .filter(|b| b.is_critical())
            .collect()
    }

    /// Get bottlenecks by severity
    #[must_use]
    pub fn by_severity(&self, severity: BottleneckSeverity) -> Vec<&Bottleneck> {
        self.bottlenecks
            .iter()
            .filter(|b| b.severity == severity)
            .collect()
    }

    /// Get bottlenecks by metric type
    #[must_use]
    pub fn by_metric(&self, metric: &BottleneckMetric) -> Vec<&Bottleneck> {
        self.bottlenecks
            .iter()
            .filter(|b| &b.metric == metric)
            .collect()
    }

    /// Get bottlenecks for a specific node
    #[must_use]
    pub fn for_node(&self, node: &str) -> Vec<&Bottleneck> {
        self.bottlenecks.iter().filter(|b| b.node == node).collect()
    }

    /// Get the most severe bottleneck
    #[must_use]
    pub fn most_severe(&self) -> Option<&Bottleneck> {
        self.bottlenecks.iter().max_by_key(|b| b.severity)
    }

    /// Get count by severity
    #[must_use]
    pub fn count_by_severity(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for bottleneck in &self.bottlenecks {
            let key = bottleneck.severity.to_string();
            *counts.entry(key).or_insert(0) += 1;
        }
        counts
    }

    /// Get count by metric
    #[must_use]
    pub fn count_by_metric(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for bottleneck in &self.bottlenecks {
            let key = bottleneck.metric.to_string();
            *counts.entry(key).or_insert(0) += 1;
        }
        counts
    }

    /// Generate a human-readable summary
    #[must_use]
    pub fn generate_summary(&self) -> String {
        if self.bottlenecks.is_empty() {
            return format!(
                "No bottlenecks detected in {} nodes ({} ms, {} tokens)",
                self.nodes_analyzed, self.total_duration_ms, self.total_tokens
            );
        }

        let severity_counts = self.count_by_severity();
        let metric_counts = self.count_by_metric();

        let mut summary = format!(
            "Found {} bottleneck(s) in {} nodes ({} ms, {} tokens):\n",
            self.bottlenecks.len(),
            self.nodes_analyzed,
            self.total_duration_ms,
            self.total_tokens
        );

        summary.push_str("\nBy severity:\n");
        for severity in [
            BottleneckSeverity::Critical,
            BottleneckSeverity::Severe,
            BottleneckSeverity::Moderate,
            BottleneckSeverity::Minor,
        ] {
            if let Some(&count) = severity_counts.get(&severity.to_string()) {
                summary.push_str(&format!("  - {}: {}\n", severity, count));
            }
        }

        summary.push_str("\nBy metric:\n");
        for (metric, count) in &metric_counts {
            summary.push_str(&format!("  - {}: {}\n", metric, count));
        }

        if self.has_critical() {
            summary.push_str("\n⚠️ CRITICAL issues require immediate attention!\n");
        }

        summary
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

impl ExecutionTrace {
    /// Detect bottlenecks in this execution trace using default thresholds
    #[must_use]
    pub fn detect_bottlenecks(&self) -> BottleneckAnalysis {
        self.detect_bottlenecks_with_thresholds(&BottleneckThresholds::default())
    }

    /// Detect bottlenecks using custom thresholds
    #[must_use]
    pub fn detect_bottlenecks_with_thresholds(
        &self,
        thresholds: &BottleneckThresholds,
    ) -> BottleneckAnalysis {
        let mut analysis = BottleneckAnalysis::new(thresholds.clone());
        analysis.total_duration_ms = self.total_duration_ms;
        analysis.total_tokens = self.total_tokens;

        // Group executions by node
        let mut node_executions: HashMap<&str, Vec<&NodeExecution>> = HashMap::new();
        for execution in &self.nodes_executed {
            node_executions
                .entry(&execution.node)
                .or_default()
                .push(execution);
        }

        analysis.nodes_analyzed = node_executions.len();

        // Analyze each unique node
        for (node_name, executions) in &node_executions {
            // Calculate aggregate metrics for this node
            let total_node_time: u64 = executions.iter().map(|e| e.duration_ms).sum();
            let total_node_tokens: u64 = executions.iter().map(|e| e.tokens_used).sum();
            let execution_count = executions.len();
            let error_count = executions.iter().filter(|e| !e.success).count();

            // 1. Latency percentage analysis
            if self.total_duration_ms > 0 {
                let latency_percentage =
                    (total_node_time as f64 / self.total_duration_ms as f64) * 100.0;
                if let Some(severity) = thresholds.latency_percentage_severity(latency_percentage) {
                    let threshold = thresholds.latency_percentage_threshold(severity);
                    analysis.bottlenecks.push(
                        Bottleneck::new(
                            *node_name,
                            BottleneckMetric::Latency,
                            latency_percentage,
                            threshold,
                            severity,
                            format!(
                                "Node '{}' consumes {:.1}% of total execution time",
                                node_name, latency_percentage
                            ),
                            Self::suggest_latency_fix(node_name, latency_percentage),
                        )
                        .with_percentage(latency_percentage),
                    );
                }
            }

            // 2. Absolute latency analysis (for single executions that are very slow)
            for execution in executions {
                if let Some(severity) = thresholds.latency_absolute_severity(execution.duration_ms)
                {
                    let threshold = thresholds.latency_absolute_threshold(severity);
                    // Only add if we don't already have a latency issue for this node or this is more severe
                    let existing = analysis
                        .bottlenecks
                        .iter()
                        .find(|b| b.node == *node_name && b.metric == BottleneckMetric::Latency);
                    if existing.is_none() || existing.map(|e| e.severity) < Some(severity) {
                        analysis.bottlenecks.push(Bottleneck::new(
                            *node_name,
                            BottleneckMetric::Latency,
                            execution.duration_ms as f64,
                            threshold as f64,
                            severity,
                            format!(
                                "Node '{}' took {} ms (execution #{})",
                                node_name, execution.duration_ms, execution.index
                            ),
                            Self::suggest_absolute_latency_fix(node_name, execution.duration_ms),
                        ));
                    }
                }
            }

            // 3. Token usage analysis
            if self.total_tokens > 0 {
                let token_percentage =
                    (total_node_tokens as f64 / self.total_tokens as f64) * 100.0;
                if let Some(severity) = thresholds.token_percentage_severity(token_percentage) {
                    let threshold = thresholds.token_percentage_threshold(severity);
                    analysis.bottlenecks.push(
                        Bottleneck::new(
                            *node_name,
                            BottleneckMetric::TokenUsage,
                            token_percentage,
                            threshold,
                            severity,
                            format!(
                                "Node '{}' uses {:.1}% of total tokens ({} tokens)",
                                node_name, token_percentage, total_node_tokens
                            ),
                            Self::suggest_token_fix(node_name, token_percentage),
                        )
                        .with_percentage(token_percentage),
                    );
                }
            }

            // 4. Error rate analysis
            if execution_count > 0 {
                let error_rate = error_count as f64 / execution_count as f64;
                if let Some(severity) = thresholds.error_rate_severity(error_rate) {
                    let threshold = thresholds.error_rate_threshold(severity);
                    analysis.bottlenecks.push(Bottleneck::new(
                        *node_name,
                        BottleneckMetric::ErrorRate,
                        error_rate * 100.0,
                        threshold * 100.0,
                        severity,
                        format!(
                            "Node '{}' has {:.1}% error rate ({}/{} executions failed)",
                            node_name,
                            error_rate * 100.0,
                            error_count,
                            execution_count
                        ),
                        Self::suggest_error_fix(node_name, error_rate),
                    ));
                }
            }

            // 5. High frequency analysis (potential infinite loops)
            if let Some(severity) = thresholds.frequency_severity(execution_count) {
                let threshold = thresholds.frequency_threshold(severity);
                analysis.bottlenecks.push(Bottleneck::new(
                    *node_name,
                    BottleneckMetric::HighFrequency,
                    execution_count as f64,
                    threshold as f64,
                    severity,
                    format!(
                        "Node '{}' executed {} times (potential loop)",
                        node_name, execution_count
                    ),
                    Self::suggest_frequency_fix(node_name, execution_count),
                ));
            }

            // 6. High variance analysis (if multiple executions)
            if execution_count > 1 {
                let durations: Vec<f64> = executions.iter().map(|e| e.duration_ms as f64).collect();
                let mean = durations.iter().sum::<f64>() / durations.len() as f64;
                if mean > 0.0 {
                    let variance = durations.iter().map(|d| (d - mean).powi(2)).sum::<f64>()
                        / durations.len() as f64;
                    let std_dev = variance.sqrt();
                    let coefficient_of_variation = std_dev / mean;

                    if let Some(severity) = thresholds.variance_severity(coefficient_of_variation) {
                        let threshold = thresholds.variance_threshold(severity);
                        analysis.bottlenecks.push(Bottleneck::new(
                            *node_name,
                            BottleneckMetric::HighVariance,
                            coefficient_of_variation,
                            threshold,
                            severity,
                            format!(
                                "Node '{}' has high execution time variance (CV: {:.2}, mean: {:.0} ms, std: {:.0} ms)",
                                node_name, coefficient_of_variation, mean, std_dev
                            ),
                            Self::suggest_variance_fix(node_name, coefficient_of_variation),
                        ));
                    }
                }
            }
        }

        // Sort bottlenecks by severity (highest first)
        analysis
            .bottlenecks
            .sort_by(|a, b| b.severity.cmp(&a.severity));

        // Generate summary
        analysis.summary = analysis.generate_summary();

        analysis
    }

    // Helper methods for generating suggestions

    fn suggest_latency_fix(node: &str, percentage: f64) -> String {
        if percentage >= 80.0 {
            format!(
                "Consider caching results from '{}', parallelizing its work, or using a faster model",
                node
            )
        } else if percentage >= 50.0 {
            format!(
                "Consider optimizing '{}' by reducing token usage or breaking into smaller steps",
                node
            )
        } else {
            format!(
                "Monitor '{}' for potential optimization opportunities",
                node
            )
        }
    }

    fn suggest_absolute_latency_fix(node: &str, duration_ms: u64) -> String {
        if duration_ms >= 60000 {
            format!(
                "Node '{}' is very slow. Consider: 1) Adding timeouts, 2) Using a faster model, 3) Caching results",
                node
            )
        } else if duration_ms >= 30000 {
            format!(
                "Node '{}' is slow. Consider reducing prompt size or using streaming for better UX",
                node
            )
        } else {
            format!(
                "Consider adding timeout handling for '{}' to prevent long waits",
                node
            )
        }
    }

    fn suggest_token_fix(node: &str, percentage: f64) -> String {
        if percentage >= 80.0 {
            format!(
                "Node '{}' dominates token usage. Consider: 1) Summarizing inputs, 2) Using a more efficient model, 3) Reducing context",
                node
            )
        } else if percentage >= 50.0 {
            format!(
                "Consider reducing prompt size or conversation history for '{}'",
                node
            )
        } else {
            format!(
                "Review prompt efficiency for '{}' to reduce token consumption",
                node
            )
        }
    }

    fn suggest_error_fix(node: &str, error_rate: f64) -> String {
        if error_rate >= 0.5 {
            format!(
                "Node '{}' is failing frequently. Check: 1) Input validation, 2) External service health, 3) Error handling",
                node
            )
        } else if error_rate >= 0.25 {
            format!(
                "Add retry logic with exponential backoff for '{}', or improve error handling",
                node
            )
        } else {
            format!(
                "Consider adding better error handling or retries for '{}'",
                node
            )
        }
    }

    fn suggest_frequency_fix(node: &str, count: usize) -> String {
        if count >= 100 {
            format!(
                "Node '{}' may be in an infinite loop. Add loop detection or maximum iteration limits",
                node
            )
        } else if count >= 50 {
            format!(
                "High execution count for '{}'. Verify this is expected behavior and add safeguards",
                node
            )
        } else {
            format!(
                "Monitor '{}' execution frequency. Consider adding circuit breakers if unexpected",
                node
            )
        }
    }

    fn suggest_variance_fix(node: &str, cv: f64) -> String {
        if cv >= 2.0 {
            format!(
                "Node '{}' has highly unpredictable execution times. Investigate input patterns or external dependencies",
                node
            )
        } else if cv >= 1.0 {
            format!(
                "Execution time for '{}' varies significantly. Consider adding timeouts and monitoring",
                node
            )
        } else {
            format!(
                "Some variance in '{}' execution time. Monitor for patterns",
                node
            )
        }
    }
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // BottleneckMetric
    // ========================================================================

    #[test]
    fn test_bottleneck_metric_display() {
        assert_eq!(format!("{}", BottleneckMetric::Latency), "latency");
        assert_eq!(format!("{}", BottleneckMetric::TokenUsage), "token_usage");
        assert_eq!(format!("{}", BottleneckMetric::ErrorRate), "error_rate");
        assert_eq!(
            format!("{}", BottleneckMetric::HighFrequency),
            "high_frequency"
        );
        assert_eq!(
            format!("{}", BottleneckMetric::HighVariance),
            "high_variance"
        );
    }

    #[test]
    fn test_bottleneck_metric_equality() {
        assert_eq!(BottleneckMetric::Latency, BottleneckMetric::Latency);
        assert_ne!(BottleneckMetric::Latency, BottleneckMetric::TokenUsage);
    }

    // ========================================================================
    // BottleneckSeverity
    // ========================================================================

    #[test]
    fn test_bottleneck_severity_display() {
        assert_eq!(format!("{}", BottleneckSeverity::Minor), "minor");
        assert_eq!(format!("{}", BottleneckSeverity::Moderate), "moderate");
        assert_eq!(format!("{}", BottleneckSeverity::Severe), "severe");
        assert_eq!(format!("{}", BottleneckSeverity::Critical), "critical");
    }

    #[test]
    fn test_bottleneck_severity_ordering() {
        assert!(BottleneckSeverity::Minor < BottleneckSeverity::Moderate);
        assert!(BottleneckSeverity::Moderate < BottleneckSeverity::Severe);
        assert!(BottleneckSeverity::Severe < BottleneckSeverity::Critical);
    }

    #[test]
    fn test_bottleneck_severity_default() {
        let severity: BottleneckSeverity = Default::default();
        assert_eq!(severity, BottleneckSeverity::Minor);
    }

    // ========================================================================
    // Bottleneck
    // ========================================================================

    #[test]
    fn test_bottleneck_new() {
        let bottleneck = Bottleneck::new(
            "test_node",
            BottleneckMetric::Latency,
            85.0,
            70.0,
            BottleneckSeverity::Severe,
            "High latency detected",
            "Consider caching",
        );

        assert_eq!(bottleneck.node, "test_node");
        assert_eq!(bottleneck.metric, BottleneckMetric::Latency);
        assert!((bottleneck.value - 85.0).abs() < f64::EPSILON);
        assert!((bottleneck.threshold - 70.0).abs() < f64::EPSILON);
        assert_eq!(bottleneck.severity, BottleneckSeverity::Severe);
        assert_eq!(bottleneck.description, "High latency detected");
        assert_eq!(bottleneck.suggestion, "Consider caching");
        assert!(bottleneck.percentage_of_total.is_none());
    }

    #[test]
    fn test_bottleneck_with_percentage() {
        let bottleneck = Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            75.0,
            50.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        )
        .with_percentage(75.0);

        assert_eq!(bottleneck.percentage_of_total, Some(75.0));
    }

    #[test]
    fn test_bottleneck_is_critical() {
        let critical = Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            95.0,
            85.0,
            BottleneckSeverity::Critical,
            "desc",
            "sugg",
        );
        let severe = Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            75.0,
            70.0,
            BottleneckSeverity::Severe,
            "desc",
            "sugg",
        );

        assert!(critical.is_critical());
        assert!(!severe.is_critical());
    }

    #[test]
    fn test_bottleneck_is_severe_or_critical() {
        let critical = Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            95.0,
            85.0,
            BottleneckSeverity::Critical,
            "desc",
            "sugg",
        );
        let severe = Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            75.0,
            70.0,
            BottleneckSeverity::Severe,
            "desc",
            "sugg",
        );
        let moderate = Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            55.0,
            50.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        );

        assert!(critical.is_severe_or_critical());
        assert!(severe.is_severe_or_critical());
        assert!(!moderate.is_severe_or_critical());
    }

    #[test]
    fn test_bottleneck_summary() {
        let bottleneck = Bottleneck::new(
            "llm_call",
            BottleneckMetric::Latency,
            75.5,
            50.0,
            BottleneckSeverity::Severe,
            "High latency",
            "Optimize",
        );

        let summary = bottleneck.summary();
        assert!(summary.contains("[severe]"));
        assert!(summary.contains("latency"));
        assert!(summary.contains("llm_call"));
        assert!(summary.contains("High latency"));
        assert!(summary.contains("75.50"));
        assert!(summary.contains("50.00"));
    }

    #[test]
    fn test_bottleneck_json_serialization() {
        let bottleneck = Bottleneck::new(
            "node",
            BottleneckMetric::TokenUsage,
            80.0,
            70.0,
            BottleneckSeverity::Severe,
            "High token usage",
            "Reduce tokens",
        );

        let json = bottleneck.to_json().expect("JSON serialization failed");
        let parsed = Bottleneck::from_json(&json).expect("JSON parsing failed");

        assert_eq!(parsed.node, "node");
        assert_eq!(parsed.metric, BottleneckMetric::TokenUsage);
        assert!((parsed.value - 80.0).abs() < f64::EPSILON);
    }

    // ========================================================================
    // BottleneckBuilder
    // ========================================================================

    #[test]
    fn test_bottleneck_builder_success() {
        let bottleneck = BottleneckBuilder::new()
            .node("test_node")
            .metric(BottleneckMetric::ErrorRate)
            .value(0.25)
            .threshold(0.15)
            .severity(BottleneckSeverity::Moderate)
            .description("25% error rate")
            .suggestion("Add retry logic")
            .percentage_of_total(25.0)
            .build()
            .expect("Build should succeed");

        assert_eq!(bottleneck.node, "test_node");
        assert_eq!(bottleneck.metric, BottleneckMetric::ErrorRate);
        assert!((bottleneck.value - 0.25).abs() < f64::EPSILON);
        assert!((bottleneck.threshold - 0.15).abs() < f64::EPSILON);
        assert_eq!(bottleneck.severity, BottleneckSeverity::Moderate);
        assert_eq!(bottleneck.description, "25% error rate");
        assert_eq!(bottleneck.suggestion, "Add retry logic");
        assert_eq!(bottleneck.percentage_of_total, Some(25.0));
    }

    #[test]
    fn test_bottleneck_builder_missing_node() {
        let result = BottleneckBuilder::new()
            .metric(BottleneckMetric::Latency)
            .description("desc")
            .suggestion("sugg")
            .build();

        assert_eq!(result.unwrap_err(), "node is required");
    }

    #[test]
    fn test_bottleneck_builder_missing_metric() {
        let result = BottleneckBuilder::new()
            .node("node")
            .description("desc")
            .suggestion("sugg")
            .build();

        assert_eq!(result.unwrap_err(), "metric is required");
    }

    #[test]
    fn test_bottleneck_builder_missing_description() {
        let result = BottleneckBuilder::new()
            .node("node")
            .metric(BottleneckMetric::Latency)
            .suggestion("sugg")
            .build();

        assert_eq!(result.unwrap_err(), "description is required");
    }

    #[test]
    fn test_bottleneck_builder_missing_suggestion() {
        let result = BottleneckBuilder::new()
            .node("node")
            .metric(BottleneckMetric::Latency)
            .description("desc")
            .build();

        assert_eq!(result.unwrap_err(), "suggestion is required");
    }

    #[test]
    fn test_bottleneck_builder_defaults() {
        let bottleneck = BottleneckBuilder::new()
            .node("node")
            .metric(BottleneckMetric::Latency)
            .description("desc")
            .suggestion("sugg")
            .build()
            .unwrap();

        assert!((bottleneck.value - 0.0).abs() < f64::EPSILON);
        assert!((bottleneck.threshold - 0.0).abs() < f64::EPSILON);
        assert_eq!(bottleneck.severity, BottleneckSeverity::Minor);
        assert!(bottleneck.percentage_of_total.is_none());
    }

    #[test]
    fn test_bottleneck_builder_method() {
        let bottleneck = Bottleneck::builder()
            .node("node")
            .metric(BottleneckMetric::Latency)
            .description("desc")
            .suggestion("sugg")
            .build()
            .unwrap();

        assert_eq!(bottleneck.node, "node");
    }

    // ========================================================================
    // BottleneckThresholds
    // ========================================================================

    #[test]
    fn test_thresholds_default() {
        let thresholds = BottleneckThresholds::default();

        assert!((thresholds.latency_percentage_minor - 30.0).abs() < f64::EPSILON);
        assert!((thresholds.latency_percentage_critical - 85.0).abs() < f64::EPSILON);
        assert_eq!(thresholds.latency_absolute_minor_ms, 5000);
        assert_eq!(thresholds.latency_absolute_critical_ms, 60000);
        assert!((thresholds.error_rate_minor - 0.05).abs() < f64::EPSILON);
        assert_eq!(thresholds.frequency_minor, 10);
    }

    #[test]
    fn test_thresholds_strict() {
        let thresholds = BottleneckThresholds::strict();

        assert!(
            thresholds.latency_percentage_minor
                < BottleneckThresholds::default().latency_percentage_minor
        );
        assert!(
            thresholds.error_rate_critical < BottleneckThresholds::default().error_rate_critical
        );
        assert!(thresholds.frequency_minor < BottleneckThresholds::default().frequency_minor);
    }

    #[test]
    fn test_thresholds_lenient() {
        let thresholds = BottleneckThresholds::lenient();

        assert!(
            thresholds.latency_percentage_minor
                > BottleneckThresholds::default().latency_percentage_minor
        );
        assert!(
            thresholds.error_rate_critical > BottleneckThresholds::default().error_rate_critical
        );
        assert!(thresholds.frequency_minor > BottleneckThresholds::default().frequency_minor);
    }

    #[test]
    fn test_thresholds_latency_percentage_severity() {
        let thresholds = BottleneckThresholds::default();

        assert_eq!(thresholds.latency_percentage_severity(20.0), None);
        assert_eq!(
            thresholds.latency_percentage_severity(35.0),
            Some(BottleneckSeverity::Minor)
        );
        assert_eq!(
            thresholds.latency_percentage_severity(55.0),
            Some(BottleneckSeverity::Moderate)
        );
        assert_eq!(
            thresholds.latency_percentage_severity(75.0),
            Some(BottleneckSeverity::Severe)
        );
        assert_eq!(
            thresholds.latency_percentage_severity(90.0),
            Some(BottleneckSeverity::Critical)
        );
    }

    #[test]
    fn test_thresholds_latency_percentage_threshold() {
        let thresholds = BottleneckThresholds::default();

        assert!(
            (thresholds.latency_percentage_threshold(BottleneckSeverity::Minor) - 30.0).abs()
                < f64::EPSILON
        );
        assert!(
            (thresholds.latency_percentage_threshold(BottleneckSeverity::Critical) - 85.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn test_thresholds_latency_absolute_severity() {
        let thresholds = BottleneckThresholds::default();

        assert_eq!(thresholds.latency_absolute_severity(1000), None);
        assert_eq!(
            thresholds.latency_absolute_severity(7000),
            Some(BottleneckSeverity::Minor)
        );
        assert_eq!(
            thresholds.latency_absolute_severity(20000),
            Some(BottleneckSeverity::Moderate)
        );
        assert_eq!(
            thresholds.latency_absolute_severity(40000),
            Some(BottleneckSeverity::Severe)
        );
        assert_eq!(
            thresholds.latency_absolute_severity(70000),
            Some(BottleneckSeverity::Critical)
        );
    }

    #[test]
    fn test_thresholds_latency_absolute_threshold() {
        let thresholds = BottleneckThresholds::default();

        assert_eq!(
            thresholds.latency_absolute_threshold(BottleneckSeverity::Minor),
            5000
        );
        assert_eq!(
            thresholds.latency_absolute_threshold(BottleneckSeverity::Critical),
            60000
        );
    }

    #[test]
    fn test_thresholds_token_percentage_severity() {
        let thresholds = BottleneckThresholds::default();

        assert_eq!(thresholds.token_percentage_severity(30.0), None);
        assert_eq!(
            thresholds.token_percentage_severity(45.0),
            Some(BottleneckSeverity::Minor)
        );
        assert_eq!(
            thresholds.token_percentage_severity(92.0),
            Some(BottleneckSeverity::Critical)
        );
    }

    #[test]
    fn test_thresholds_token_percentage_threshold() {
        let thresholds = BottleneckThresholds::default();

        assert!(
            (thresholds.token_percentage_threshold(BottleneckSeverity::Minor) - 40.0).abs()
                < f64::EPSILON
        );
        assert!(
            (thresholds.token_percentage_threshold(BottleneckSeverity::Critical) - 90.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn test_thresholds_error_rate_severity() {
        let thresholds = BottleneckThresholds::default();

        assert_eq!(thresholds.error_rate_severity(0.02), None);
        assert_eq!(
            thresholds.error_rate_severity(0.08),
            Some(BottleneckSeverity::Minor)
        );
        assert_eq!(
            thresholds.error_rate_severity(0.60),
            Some(BottleneckSeverity::Critical)
        );
    }

    #[test]
    fn test_thresholds_error_rate_threshold() {
        let thresholds = BottleneckThresholds::default();

        assert!(
            (thresholds.error_rate_threshold(BottleneckSeverity::Minor) - 0.05).abs()
                < f64::EPSILON
        );
        assert!(
            (thresholds.error_rate_threshold(BottleneckSeverity::Critical) - 0.50).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn test_thresholds_frequency_severity() {
        let thresholds = BottleneckThresholds::default();

        assert_eq!(thresholds.frequency_severity(5), None);
        assert_eq!(
            thresholds.frequency_severity(15),
            Some(BottleneckSeverity::Minor)
        );
        assert_eq!(
            thresholds.frequency_severity(150),
            Some(BottleneckSeverity::Critical)
        );
    }

    #[test]
    fn test_thresholds_frequency_threshold() {
        let thresholds = BottleneckThresholds::default();

        assert_eq!(
            thresholds.frequency_threshold(BottleneckSeverity::Minor),
            10
        );
        assert_eq!(
            thresholds.frequency_threshold(BottleneckSeverity::Critical),
            100
        );
    }

    #[test]
    fn test_thresholds_variance_severity() {
        let thresholds = BottleneckThresholds::default();

        assert_eq!(thresholds.variance_severity(0.3), None);
        assert_eq!(
            thresholds.variance_severity(0.7),
            Some(BottleneckSeverity::Minor)
        );
        assert_eq!(
            thresholds.variance_severity(2.5),
            Some(BottleneckSeverity::Critical)
        );
    }

    #[test]
    fn test_thresholds_variance_threshold() {
        let thresholds = BottleneckThresholds::default();

        assert!(
            (thresholds.variance_threshold(BottleneckSeverity::Minor) - 0.5).abs() < f64::EPSILON
        );
        assert!(
            (thresholds.variance_threshold(BottleneckSeverity::Critical) - 2.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn test_thresholds_json_serialization() {
        let thresholds = BottleneckThresholds::strict();
        let json = thresholds.to_json().expect("JSON serialization failed");
        let parsed = BottleneckThresholds::from_json(&json).expect("JSON parsing failed");

        assert!(
            (parsed.latency_percentage_minor - thresholds.latency_percentage_minor).abs()
                < f64::EPSILON
        );
        assert_eq!(parsed.frequency_critical, thresholds.frequency_critical);
    }

    // ========================================================================
    // BottleneckAnalysis
    // ========================================================================

    #[test]
    fn test_analysis_new() {
        let analysis = BottleneckAnalysis::new(BottleneckThresholds::default());

        assert!(analysis.bottlenecks.is_empty());
        assert_eq!(analysis.nodes_analyzed, 0);
        assert_eq!(analysis.total_duration_ms, 0);
        assert_eq!(analysis.total_tokens, 0);
        assert!(analysis.summary.is_empty());
    }

    #[test]
    fn test_analysis_has_bottlenecks() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        assert!(!analysis.has_bottlenecks());

        analysis.bottlenecks.push(Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        assert!(analysis.has_bottlenecks());
    }

    #[test]
    fn test_analysis_bottleneck_count() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        assert_eq!(analysis.bottleneck_count(), 0);

        analysis.bottlenecks.push(Bottleneck::new(
            "node1",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::TokenUsage,
            60.0,
            40.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        ));
        assert_eq!(analysis.bottleneck_count(), 2);
    }

    #[test]
    fn test_analysis_has_critical() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        assert!(!analysis.has_critical());

        analysis.bottlenecks.push(Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Severe,
            "desc",
            "sugg",
        ));
        assert!(!analysis.has_critical());

        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::ErrorRate,
            90.0,
            50.0,
            BottleneckSeverity::Critical,
            "desc",
            "sugg",
        ));
        assert!(analysis.has_critical());
    }

    #[test]
    fn test_analysis_critical_bottlenecks() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.bottlenecks.push(Bottleneck::new(
            "node1",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Severe,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::ErrorRate,
            90.0,
            50.0,
            BottleneckSeverity::Critical,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node3",
            BottleneckMetric::HighFrequency,
            200.0,
            100.0,
            BottleneckSeverity::Critical,
            "desc",
            "sugg",
        ));

        let critical = analysis.critical_bottlenecks();
        assert_eq!(critical.len(), 2);
    }

    #[test]
    fn test_analysis_by_severity() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.bottlenecks.push(Bottleneck::new(
            "node1",
            BottleneckMetric::Latency,
            35.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::Latency,
            55.0,
            50.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node3",
            BottleneckMetric::Latency,
            75.0,
            70.0,
            BottleneckSeverity::Severe,
            "desc",
            "sugg",
        ));

        assert_eq!(analysis.by_severity(BottleneckSeverity::Minor).len(), 1);
        assert_eq!(analysis.by_severity(BottleneckSeverity::Moderate).len(), 1);
        assert_eq!(analysis.by_severity(BottleneckSeverity::Severe).len(), 1);
        assert_eq!(analysis.by_severity(BottleneckSeverity::Critical).len(), 0);
    }

    #[test]
    fn test_analysis_by_metric() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.bottlenecks.push(Bottleneck::new(
            "node1",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::Latency,
            60.0,
            30.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node3",
            BottleneckMetric::TokenUsage,
            70.0,
            40.0,
            BottleneckSeverity::Severe,
            "desc",
            "sugg",
        ));

        assert_eq!(analysis.by_metric(&BottleneckMetric::Latency).len(), 2);
        assert_eq!(analysis.by_metric(&BottleneckMetric::TokenUsage).len(), 1);
        assert_eq!(analysis.by_metric(&BottleneckMetric::ErrorRate).len(), 0);
    }

    #[test]
    fn test_analysis_for_node() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.bottlenecks.push(Bottleneck::new(
            "slow_node",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "slow_node",
            BottleneckMetric::TokenUsage,
            60.0,
            40.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "other_node",
            BottleneckMetric::ErrorRate,
            25.0,
            15.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        ));

        assert_eq!(analysis.for_node("slow_node").len(), 2);
        assert_eq!(analysis.for_node("other_node").len(), 1);
        assert_eq!(analysis.for_node("nonexistent").len(), 0);
    }

    #[test]
    fn test_analysis_most_severe() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        assert!(analysis.most_severe().is_none());

        analysis.bottlenecks.push(Bottleneck::new(
            "node1",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::ErrorRate,
            90.0,
            50.0,
            BottleneckSeverity::Critical,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node3",
            BottleneckMetric::TokenUsage,
            60.0,
            40.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        ));

        let most_severe = analysis.most_severe().unwrap();
        assert_eq!(most_severe.severity, BottleneckSeverity::Critical);
    }

    #[test]
    fn test_analysis_count_by_severity() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.bottlenecks.push(Bottleneck::new(
            "node1",
            BottleneckMetric::Latency,
            35.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::Latency,
            36.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node3",
            BottleneckMetric::ErrorRate,
            90.0,
            50.0,
            BottleneckSeverity::Critical,
            "desc",
            "sugg",
        ));

        let counts = analysis.count_by_severity();
        assert_eq!(counts.get("minor"), Some(&2));
        assert_eq!(counts.get("critical"), Some(&1));
        assert_eq!(counts.get("severe"), None);
    }

    #[test]
    fn test_analysis_count_by_metric() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.bottlenecks.push(Bottleneck::new(
            "node1",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::Latency,
            60.0,
            30.0,
            BottleneckSeverity::Moderate,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node3",
            BottleneckMetric::TokenUsage,
            70.0,
            40.0,
            BottleneckSeverity::Severe,
            "desc",
            "sugg",
        ));

        let counts = analysis.count_by_metric();
        assert_eq!(counts.get("latency"), Some(&2));
        assert_eq!(counts.get("token_usage"), Some(&1));
        assert_eq!(counts.get("error_rate"), None);
    }

    #[test]
    fn test_analysis_generate_summary_no_bottlenecks() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.nodes_analyzed = 5;
        analysis.total_duration_ms = 1000;
        analysis.total_tokens = 500;

        let summary = analysis.generate_summary();
        assert!(summary.contains("No bottlenecks detected"));
        assert!(summary.contains("5 nodes"));
        assert!(summary.contains("1000 ms"));
        assert!(summary.contains("500 tokens"));
    }

    #[test]
    fn test_analysis_generate_summary_with_bottlenecks() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.nodes_analyzed = 5;
        analysis.total_duration_ms = 1000;
        analysis.total_tokens = 500;
        analysis.bottlenecks.push(Bottleneck::new(
            "node1",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.bottlenecks.push(Bottleneck::new(
            "node2",
            BottleneckMetric::ErrorRate,
            90.0,
            50.0,
            BottleneckSeverity::Critical,
            "desc",
            "sugg",
        ));

        let summary = analysis.generate_summary();
        assert!(summary.contains("Found 2 bottleneck"));
        assert!(summary.contains("5 nodes"));
        assert!(summary.contains("By severity"));
        assert!(summary.contains("By metric"));
        assert!(summary.contains("CRITICAL"));
    }

    #[test]
    fn test_analysis_json_serialization() {
        let mut analysis = BottleneckAnalysis::new(BottleneckThresholds::default());
        analysis.nodes_analyzed = 3;
        analysis.total_duration_ms = 500;
        analysis.total_tokens = 1000;
        analysis.bottlenecks.push(Bottleneck::new(
            "node",
            BottleneckMetric::Latency,
            50.0,
            30.0,
            BottleneckSeverity::Minor,
            "desc",
            "sugg",
        ));
        analysis.summary = "test summary".to_string();

        let json = analysis.to_json().expect("JSON serialization failed");
        let parsed = BottleneckAnalysis::from_json(&json).expect("JSON parsing failed");

        assert_eq!(parsed.nodes_analyzed, 3);
        assert_eq!(parsed.total_duration_ms, 500);
        assert_eq!(parsed.total_tokens, 1000);
        assert_eq!(parsed.bottleneck_count(), 1);
        assert_eq!(parsed.summary, "test summary");
    }

    // ========================================================================
    // ExecutionTrace::detect_bottlenecks (requires minimal trace data)
    // ========================================================================

    fn create_test_execution(
        node: &str,
        index: usize,
        duration_ms: u64,
        tokens_used: u64,
        success: bool,
    ) -> NodeExecution {
        NodeExecution {
            node: node.to_string(),
            index,
            duration_ms,
            tokens_used,
            success,
            state_before: None,
            state_after: None,
            tools_called: Vec::new(),
            error_message: None,
            started_at: None,
            metadata: HashMap::new(),
        }
    }

    fn create_test_trace(
        executions: Vec<NodeExecution>,
        total_duration_ms: u64,
        total_tokens: u64,
    ) -> ExecutionTrace {
        ExecutionTrace {
            thread_id: Some("test-thread".to_string()),
            execution_id: Some("test-exec".to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            total_duration_ms,
            total_tokens,
            nodes_executed: executions,
            errors: Vec::new(),
            completed: true,
            started_at: None,
            ended_at: None,
            final_state: None,
            metadata: HashMap::new(),
            execution_metrics: None,
            performance_metrics: None,
        }
    }

    #[test]
    fn test_detect_bottlenecks_empty_trace() {
        let trace = create_test_trace(vec![], 0, 0);
        let analysis = trace.detect_bottlenecks();

        assert!(!analysis.has_bottlenecks());
        assert_eq!(analysis.nodes_analyzed, 0);
    }

    #[test]
    fn test_detect_bottlenecks_latency_percentage() {
        // Node takes 85% of total time (should be critical)
        let executions = vec![
            create_test_execution("slow_node", 0, 8500, 100, true),
            create_test_execution("fast_node", 1, 1500, 100, true),
        ];
        let trace = create_test_trace(executions, 10000, 200);

        let analysis = trace.detect_bottlenecks();

        assert!(analysis.has_bottlenecks());
        let slow_node_issues = analysis.for_node("slow_node");
        assert!(!slow_node_issues.is_empty());

        let latency_issue = slow_node_issues
            .iter()
            .find(|b| b.metric == BottleneckMetric::Latency);
        assert!(latency_issue.is_some());
        assert_eq!(
            latency_issue.unwrap().severity,
            BottleneckSeverity::Critical
        );
    }

    #[test]
    fn test_detect_bottlenecks_token_usage() {
        // Node uses 92% of tokens (should be critical)
        let executions = vec![
            create_test_execution("token_heavy", 0, 1000, 920, true),
            create_test_execution("token_light", 1, 1000, 80, true),
        ];
        let trace = create_test_trace(executions, 2000, 1000);

        let analysis = trace.detect_bottlenecks();

        assert!(analysis.has_bottlenecks());
        let token_issues = analysis.by_metric(&BottleneckMetric::TokenUsage);
        assert!(!token_issues.is_empty());
    }

    #[test]
    fn test_detect_bottlenecks_error_rate() {
        // Node fails 60% of the time (should be critical)
        let executions = vec![
            create_test_execution("flaky_node", 0, 100, 10, false),
            create_test_execution("flaky_node", 1, 100, 10, false),
            create_test_execution("flaky_node", 2, 100, 10, false),
            create_test_execution("flaky_node", 3, 100, 10, true),
            create_test_execution("flaky_node", 4, 100, 10, true),
        ];
        let trace = create_test_trace(executions, 500, 50);

        let analysis = trace.detect_bottlenecks();

        let error_issues = analysis.by_metric(&BottleneckMetric::ErrorRate);
        assert!(!error_issues.is_empty());
        assert_eq!(error_issues[0].severity, BottleneckSeverity::Critical);
    }

    #[test]
    fn test_detect_bottlenecks_high_frequency() {
        // Node executes 150 times (should be critical)
        let executions: Vec<NodeExecution> = (0..150)
            .map(|i| create_test_execution("loop_node", i, 10, 1, true))
            .collect();
        let trace = create_test_trace(executions, 1500, 150);

        let analysis = trace.detect_bottlenecks();

        let freq_issues = analysis.by_metric(&BottleneckMetric::HighFrequency);
        assert!(!freq_issues.is_empty());
        assert_eq!(freq_issues[0].severity, BottleneckSeverity::Critical);
    }

    #[test]
    fn test_detect_bottlenecks_high_variance() {
        // Node has high variance in execution times
        let executions = vec![
            create_test_execution("variable_node", 0, 100, 10, true),
            create_test_execution("variable_node", 1, 1000, 10, true),
            create_test_execution("variable_node", 2, 50, 10, true),
            create_test_execution("variable_node", 3, 2000, 10, true),
        ];
        let trace = create_test_trace(executions, 3150, 40);

        let analysis = trace.detect_bottlenecks();

        let variance_issues = analysis.by_metric(&BottleneckMetric::HighVariance);
        assert!(!variance_issues.is_empty());
    }

    #[test]
    fn test_detect_bottlenecks_with_custom_thresholds() {
        // Use strict thresholds
        let executions = vec![create_test_execution("node", 0, 3000, 100, true)];
        let trace = create_test_trace(executions, 10000, 100);

        // With default thresholds, 30% latency might be minor
        let default_analysis = trace.detect_bottlenecks();

        // With strict thresholds, same 30% should be more severe
        let strict_analysis =
            trace.detect_bottlenecks_with_thresholds(&BottleneckThresholds::strict());

        // Strict should catch more or be more severe
        let default_latency = default_analysis.by_metric(&BottleneckMetric::Latency);
        let strict_latency = strict_analysis.by_metric(&BottleneckMetric::Latency);

        // If both found latency issues, strict should be same or more severe
        if !default_latency.is_empty() && !strict_latency.is_empty() {
            assert!(strict_latency[0].severity >= default_latency[0].severity);
        }
    }

    #[test]
    fn test_detect_bottlenecks_no_issues_healthy_trace() {
        // All nodes perform well
        let executions = vec![
            create_test_execution("node1", 0, 100, 50, true),
            create_test_execution("node2", 1, 100, 50, true),
            create_test_execution("node3", 2, 100, 50, true),
        ];
        let trace = create_test_trace(executions, 300, 150);

        // Use lenient thresholds
        let analysis = trace.detect_bottlenecks_with_thresholds(&BottleneckThresholds::lenient());

        // Might still have some minor issues, but no critical
        assert!(!analysis.has_critical());
    }

    #[test]
    fn test_detect_bottlenecks_sorted_by_severity() {
        let executions = vec![
            create_test_execution("critical_node", 0, 8500, 100, true),
            create_test_execution("minor_node", 1, 1500, 100, true),
        ];
        let trace = create_test_trace(executions, 10000, 200);

        let analysis = trace.detect_bottlenecks();

        if analysis.bottleneck_count() > 1 {
            // First should be most severe
            let first = &analysis.bottlenecks[0];
            let last = &analysis.bottlenecks[analysis.bottleneck_count() - 1];
            assert!(first.severity >= last.severity);
        }
    }
}
