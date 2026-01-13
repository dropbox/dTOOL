// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Execution Prediction - AI Predicts Its Own Execution Before Running
//!
//! This module provides execution prediction capabilities that allow AI agents to
//! predict execution outcomes (duration, tokens, cost, path) before running.
//!
//! ## Overview
//!
//! Execution prediction enables AI agents to:
//! - Predict execution duration based on input characteristics
//! - Estimate token usage before processing
//! - Calculate expected cost upfront
//! - Forecast which nodes will execute (predicted path)
//! - Warn about budget constraints before execution
//!
//! ## Key Concepts
//!
//! - **ExecutionPrediction**: The predicted outcome of an execution
//! - **InputFeatures**: Characteristics of the input that affect prediction
//! - **PredictionModel**: Statistical model trained on historical executions
//! - **PredictionConfidence**: How confident the model is in its prediction
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::execution_prediction::ExecutionPredictor;
//!
//! // Train predictor on historical executions
//! let predictor = ExecutionPredictor::train(&traces);
//!
//! // Predict before running
//! let prediction = predictor.predict(&input_features);
//!
//! println!("I predict this will:");
//! println!("  Take: {}ms", prediction.predicted_duration_ms);
//! println!("  Cost: ${:.4}", prediction.predicted_cost);
//! println!("  Path: {:?}", prediction.predicted_path);
//!
//! // AI can warn or adapt
//! if prediction.predicted_cost > budget {
//!     println!("Warning: Predicted cost exceeds budget");
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Features extracted from input that affect prediction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputFeatures {
    /// Number of input tokens
    pub input_tokens: u64,
    /// Complexity score (0.0-1.0)
    pub complexity: f64,
    /// Whether the input likely requires tool calls
    pub likely_needs_tools: bool,
    /// Number of expected tool calls
    pub expected_tool_calls: usize,
    /// Input type/category (if known)
    pub input_type: Option<String>,
    /// Additional features
    pub metadata: HashMap<String, serde_json::Value>,
}

impl InputFeatures {
    /// Create new input features
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set input token count
    #[must_use]
    pub fn with_tokens(mut self, tokens: u64) -> Self {
        self.input_tokens = tokens;
        self
    }

    /// Set complexity score
    #[must_use]
    pub fn with_complexity(mut self, complexity: f64) -> Self {
        self.complexity = complexity.clamp(0.0, 1.0);
        self
    }

    /// Set tool expectation
    #[must_use]
    pub fn with_tools(mut self, needs_tools: bool, expected_calls: usize) -> Self {
        self.likely_needs_tools = needs_tools;
        self.expected_tool_calls = expected_calls;
        self
    }

    /// Set input type
    #[must_use]
    pub fn with_type(mut self, input_type: impl Into<String>) -> Self {
        self.input_type = Some(input_type.into());
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Extract features from an execution trace (for training)
    #[must_use]
    pub fn from_trace(trace: &crate::introspection::ExecutionTrace) -> Self {
        // Count tool calls
        let total_tool_calls: usize = trace
            .nodes_executed
            .iter()
            .map(|e| e.tools_called.len())
            .sum();

        Self {
            input_tokens: trace.total_tokens / 2, // Rough estimate: half are input
            complexity: estimate_complexity(trace),
            likely_needs_tools: total_tool_calls > 0,
            expected_tool_calls: total_tool_calls,
            input_type: None,
            metadata: HashMap::new(),
        }
    }
}

/// Predicted execution outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPrediction {
    /// Predicted duration in milliseconds
    pub predicted_duration_ms: u64,
    /// Predicted token usage
    pub predicted_tokens: u64,
    /// Predicted cost in dollars
    pub predicted_cost: f64,
    /// Predicted execution path (node names)
    pub predicted_path: Vec<String>,
    /// Prediction confidence (0.0-1.0)
    pub confidence: f64,
    /// Confidence breakdown by metric
    pub confidence_breakdown: ConfidenceBreakdown,
    /// Prediction intervals (low, high estimates)
    pub intervals: PredictionIntervals,
    /// Warnings based on prediction
    pub warnings: Vec<PredictionWarning>,
    /// Model version used for prediction
    pub model_version: String,
}

impl ExecutionPrediction {
    /// Create a new prediction
    #[must_use]
    pub fn new() -> Self {
        Self {
            predicted_duration_ms: 0,
            predicted_tokens: 0,
            predicted_cost: 0.0,
            predicted_path: Vec::new(),
            confidence: 0.0,
            confidence_breakdown: ConfidenceBreakdown::default(),
            intervals: PredictionIntervals::default(),
            warnings: Vec::new(),
            model_version: "1.0".to_string(),
        }
    }

    /// Set predicted duration
    #[must_use]
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.predicted_duration_ms = ms;
        self
    }

    /// Set predicted tokens
    #[must_use]
    pub fn with_tokens(mut self, tokens: u64) -> Self {
        self.predicted_tokens = tokens;
        self
    }

    /// Set predicted cost
    #[must_use]
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.predicted_cost = cost;
        self
    }

    /// Set predicted path
    #[must_use]
    pub fn with_path(mut self, path: Vec<String>) -> Self {
        self.predicted_path = path;
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add a warning
    #[must_use]
    pub fn with_warning(mut self, warning: PredictionWarning) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Set intervals
    #[must_use]
    pub fn with_intervals(mut self, intervals: PredictionIntervals) -> Self {
        self.intervals = intervals;
        self
    }

    /// Check if prediction exceeds budget
    #[must_use]
    pub fn exceeds_budget(&self, budget_dollars: f64) -> bool {
        self.predicted_cost > budget_dollars
    }

    /// Check if prediction exceeds time limit
    #[must_use]
    pub fn exceeds_time_limit(&self, limit_ms: u64) -> bool {
        self.predicted_duration_ms > limit_ms
    }

    /// Get a human-readable summary
    #[must_use]
    pub fn summary(&self) -> String {
        let mut lines = vec![
            format!(
                "Execution Prediction (confidence: {:.1}%):",
                self.confidence * 100.0
            ),
            format!(
                "  Duration: {}ms ({}-{}ms)",
                self.predicted_duration_ms,
                self.intervals.duration_low_ms,
                self.intervals.duration_high_ms
            ),
            format!(
                "  Tokens: {} ({}-{})",
                self.predicted_tokens, self.intervals.tokens_low, self.intervals.tokens_high
            ),
            format!(
                "  Cost: ${:.4} (${:.4}-${:.4})",
                self.predicted_cost, self.intervals.cost_low, self.intervals.cost_high
            ),
        ];

        if !self.predicted_path.is_empty() {
            lines.push(format!("  Path: {}", self.predicted_path.join(" -> ")));
        }

        if !self.warnings.is_empty() {
            lines.push("  Warnings:".to_string());
            for warning in &self.warnings {
                lines.push(format!("    - [{}] {}", warning.severity, warning.message));
            }
        }

        lines.join("\n")
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

impl Default for ExecutionPrediction {
    fn default() -> Self {
        Self::new()
    }
}

/// Confidence breakdown by metric
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfidenceBreakdown {
    /// Confidence in duration prediction
    pub duration: f64,
    /// Confidence in token prediction
    pub tokens: f64,
    /// Confidence in cost prediction
    pub cost: f64,
    /// Confidence in path prediction
    pub path: f64,
}

/// Prediction intervals (uncertainty ranges)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PredictionIntervals {
    /// Low estimate for duration (p10)
    pub duration_low_ms: u64,
    /// High estimate for duration (p90)
    pub duration_high_ms: u64,
    /// Low estimate for tokens (p10)
    pub tokens_low: u64,
    /// High estimate for tokens (p90)
    pub tokens_high: u64,
    /// Low estimate for cost (p10)
    pub cost_low: f64,
    /// High estimate for cost (p90)
    pub cost_high: f64,
}

/// Warning generated during prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionWarning {
    /// Warning severity
    pub severity: WarningSeverity,
    /// Warning message
    pub message: String,
    /// Suggested action
    pub suggestion: Option<String>,
}

impl PredictionWarning {
    /// Create a new warning
    #[must_use]
    pub fn new(severity: WarningSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            suggestion: None,
        }
    }

    /// Add a suggestion
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Warning severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarningSeverity {
    /// Informational warning
    Info,
    /// Warning that might affect execution
    Warning,
    /// Critical warning that should be addressed
    Critical,
}

impl std::fmt::Display for WarningSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarningSeverity::Info => write!(f, "INFO"),
            WarningSeverity::Warning => write!(f, "WARN"),
            WarningSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Configuration for prediction model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionConfig {
    /// Cost per 1k tokens (for cost estimation)
    pub cost_per_1k_tokens: f64,
    /// Default latency per token (ms)
    pub latency_per_token_ms: f64,
    /// Base latency overhead (ms)
    pub base_latency_ms: u64,
    /// Minimum samples needed for confidence
    pub min_samples_for_confidence: usize,
    /// Confidence multiplier for more samples
    pub sample_confidence_factor: f64,
    /// Budget warning threshold (fraction of budget)
    pub budget_warning_threshold: f64,
    /// Time warning threshold (fraction of limit)
    pub time_warning_threshold: f64,
}

impl Default for PredictionConfig {
    fn default() -> Self {
        Self {
            cost_per_1k_tokens: 0.002,  // ~$0.002 per 1k tokens
            latency_per_token_ms: 0.05, // ~50ms per 1k tokens
            base_latency_ms: 500,       // 500ms overhead
            min_samples_for_confidence: 10,
            sample_confidence_factor: 0.1, // +10% confidence per 10 samples
            budget_warning_threshold: 0.8, // Warn at 80% of budget
            time_warning_threshold: 0.8,   // Warn at 80% of time limit
        }
    }
}

/// Historical statistics for a node
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeStats {
    /// Node name
    pub node: String,
    /// Average duration (ms)
    pub avg_duration_ms: f64,
    /// Standard deviation of duration
    pub std_duration_ms: f64,
    /// Average tokens used
    pub avg_tokens: f64,
    /// Standard deviation of tokens
    pub std_tokens: f64,
    /// Execution count
    pub sample_count: usize,
    /// Success rate
    pub success_rate: f64,
}

/// Execution predictor trained on historical data
pub struct ExecutionPredictor {
    /// Configuration
    config: PredictionConfig,
    /// Per-node statistics
    node_stats: HashMap<String, NodeStats>,
    /// Common execution paths with frequencies
    common_paths: Vec<(Vec<String>, f64)>,
    /// Total training samples
    total_samples: usize,
    /// Average complexity to tokens ratio
    complexity_token_ratio: f64,
}

impl Default for ExecutionPredictor {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionPredictor {
    /// Create a new predictor with default config
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: PredictionConfig::default(),
            node_stats: HashMap::new(),
            common_paths: Vec::new(),
            total_samples: 0,
            complexity_token_ratio: 1000.0, // Default: complexity 1.0 = 1000 tokens
        }
    }

    /// Create predictor with custom config
    #[must_use]
    pub fn with_config(config: PredictionConfig) -> Self {
        Self {
            config,
            node_stats: HashMap::new(),
            common_paths: Vec::new(),
            total_samples: 0,
            complexity_token_ratio: 1000.0,
        }
    }

    /// Train predictor on historical execution traces
    #[must_use]
    pub fn train(traces: &[crate::introspection::ExecutionTrace]) -> Self {
        let mut predictor = Self::new();
        predictor.update(traces);
        predictor
    }

    /// Update predictor with new traces
    pub fn update(&mut self, traces: &[crate::introspection::ExecutionTrace]) {
        if traces.is_empty() {
            return;
        }

        // Collect node statistics
        let mut node_durations: HashMap<String, Vec<u64>> = HashMap::new();
        let mut node_tokens: HashMap<String, Vec<u64>> = HashMap::new();
        let mut node_successes: HashMap<String, (usize, usize)> = HashMap::new();
        let mut paths: HashMap<Vec<String>, usize> = HashMap::new();

        for trace in traces {
            // Collect path
            let path: Vec<String> = trace
                .nodes_executed
                .iter()
                .map(|e| e.node.clone())
                .collect();

            // Deduplicate consecutive same nodes
            let deduped_path: Vec<String> = path.iter().fold(Vec::new(), |mut acc, node| {
                if acc.last() != Some(node) {
                    acc.push(node.clone());
                }
                acc
            });

            *paths.entry(deduped_path).or_insert(0) += 1;

            // Collect node stats
            for exec in &trace.nodes_executed {
                node_durations
                    .entry(exec.node.clone())
                    .or_default()
                    .push(exec.duration_ms);
                node_tokens
                    .entry(exec.node.clone())
                    .or_default()
                    .push(exec.tokens_used);

                let entry = node_successes.entry(exec.node.clone()).or_insert((0, 0));
                entry.1 += 1; // Total
                if exec.success {
                    entry.0 += 1; // Successes
                }
            }
        }

        // Calculate statistics
        for (node, durations) in &node_durations {
            let avg_duration = average_f64(durations);
            let std_duration = std_dev_f64(durations, avg_duration);

            let tokens = node_tokens.get(node).map(|t| t.as_slice()).unwrap_or(&[]);
            let avg_tokens = average_f64(tokens);
            let std_tokens = std_dev_f64(tokens, avg_tokens);

            let (successes, total) = node_successes.get(node).copied().unwrap_or((0, 0));
            let success_rate = if total > 0 {
                successes as f64 / total as f64
            } else {
                1.0
            };

            self.node_stats.insert(
                node.clone(),
                NodeStats {
                    node: node.clone(),
                    avg_duration_ms: avg_duration,
                    std_duration_ms: std_duration,
                    avg_tokens,
                    std_tokens,
                    sample_count: durations.len(),
                    success_rate,
                },
            );
        }

        // Store common paths
        // M-221: Guard against division by zero when paths is empty
        let total_paths: usize = paths.values().sum::<usize>().max(1);
        self.common_paths = paths
            .into_iter()
            .map(|(path, count)| (path, count as f64 / total_paths as f64))
            .collect();
        self.common_paths
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Learn complexity to token ratio
        let mut total_complexity = 0.0;
        let mut total_tokens = 0u64;
        for trace in traces {
            total_complexity += estimate_complexity(trace);
            total_tokens += trace.total_tokens;
        }
        if total_complexity > 0.0 {
            self.complexity_token_ratio = total_tokens as f64 / total_complexity;
        }

        self.total_samples += traces.len();
    }

    /// Predict execution outcome based on input features
    #[must_use]
    pub fn predict(&self, features: &InputFeatures) -> ExecutionPrediction {
        // Predict tokens based on input features
        let predicted_tokens = self.predict_tokens(features);

        // Predict duration based on tokens and complexity
        let predicted_duration_ms = self.predict_duration(features, predicted_tokens);

        // Predict cost
        let predicted_cost = self.predict_cost(predicted_tokens);

        // Predict path
        let predicted_path = self.predict_path(features);

        // Calculate confidence
        let confidence = self.calculate_confidence(features);

        // Calculate intervals
        let intervals =
            self.calculate_intervals(predicted_duration_ms, predicted_tokens, predicted_cost);

        // Generate warnings
        let warnings = self.generate_warnings(
            features,
            predicted_duration_ms,
            predicted_tokens,
            predicted_cost,
        );

        ExecutionPrediction {
            predicted_duration_ms,
            predicted_tokens,
            predicted_cost,
            predicted_path,
            confidence,
            confidence_breakdown: ConfidenceBreakdown {
                duration: confidence * 0.9, // Duration slightly less certain
                tokens: confidence,
                cost: confidence,       // Cost follows tokens
                path: confidence * 0.7, // Path is less predictable
            },
            intervals,
            warnings,
            model_version: "1.0".to_string(),
        }
    }

    /// Predict with budget constraints
    #[must_use]
    pub fn predict_with_budget(
        &self,
        features: &InputFeatures,
        budget: f64,
    ) -> ExecutionPrediction {
        let mut prediction = self.predict(features);

        if prediction.predicted_cost > budget {
            prediction.warnings.push(
                PredictionWarning::new(
                    WarningSeverity::Critical,
                    format!(
                        "Predicted cost ${:.4} exceeds budget ${:.4}",
                        prediction.predicted_cost, budget
                    ),
                )
                .with_suggestion("Consider using a cheaper model or reducing input size"),
            );
        } else if prediction.predicted_cost > budget * self.config.budget_warning_threshold {
            prediction.warnings.push(PredictionWarning::new(
                WarningSeverity::Warning,
                format!(
                    "Predicted cost ${:.4} is {:.0}% of budget ${:.4}",
                    prediction.predicted_cost,
                    (prediction.predicted_cost / budget) * 100.0,
                    budget
                ),
            ));
        }

        prediction
    }

    /// Predict with time limit constraints
    #[must_use]
    pub fn predict_with_time_limit(
        &self,
        features: &InputFeatures,
        limit_ms: u64,
    ) -> ExecutionPrediction {
        let mut prediction = self.predict(features);

        if prediction.predicted_duration_ms > limit_ms {
            prediction.warnings.push(
                PredictionWarning::new(
                    WarningSeverity::Critical,
                    format!(
                        "Predicted duration {}ms exceeds limit {}ms",
                        prediction.predicted_duration_ms, limit_ms
                    ),
                )
                .with_suggestion("Consider reducing input complexity or using a faster model"),
            );
        } else if prediction.predicted_duration_ms
            > (limit_ms as f64 * self.config.time_warning_threshold) as u64
        {
            prediction.warnings.push(PredictionWarning::new(
                WarningSeverity::Warning,
                format!(
                    "Predicted duration {}ms is {:.0}% of limit {}ms",
                    prediction.predicted_duration_ms,
                    (prediction.predicted_duration_ms as f64 / limit_ms as f64) * 100.0,
                    limit_ms
                ),
            ));
        }

        prediction
    }

    /// Compare prediction with actual execution
    #[must_use]
    pub fn evaluate_prediction(
        &self,
        prediction: &ExecutionPrediction,
        actual: &crate::introspection::ExecutionTrace,
    ) -> PredictionAccuracy {
        let duration_error =
            (prediction.predicted_duration_ms as f64 - actual.total_duration_ms as f64).abs()
                / actual.total_duration_ms.max(1) as f64;

        let token_error = (prediction.predicted_tokens as f64 - actual.total_tokens as f64).abs()
            / actual.total_tokens.max(1) as f64;

        // Check path accuracy
        let actual_path: Vec<String> = actual.nodes_executed.iter().map(|e| e.node.clone()).fold(
            Vec::new(),
            |mut acc, node| {
                if acc.last() != Some(&node) {
                    acc.push(node);
                }
                acc
            },
        );

        let path_match = prediction.predicted_path == actual_path;
        let path_similarity = calculate_path_similarity(&prediction.predicted_path, &actual_path);

        PredictionAccuracy {
            duration_error,
            token_error,
            path_match,
            path_similarity,
            overall_accuracy: 1.0 - (duration_error + token_error) / 2.0,
        }
    }

    /// Get statistics for a specific node
    #[must_use]
    pub fn get_node_stats(&self, node: &str) -> Option<&NodeStats> {
        self.node_stats.get(node)
    }

    /// Get most common execution paths
    #[must_use]
    pub fn get_common_paths(&self) -> &[(Vec<String>, f64)] {
        &self.common_paths
    }

    /// Get total training samples
    #[must_use]
    pub fn total_samples(&self) -> usize {
        self.total_samples
    }

    fn predict_tokens(&self, features: &InputFeatures) -> u64 {
        // Base: input tokens * 2 (for output)
        let base_tokens = features.input_tokens * 2;

        // Adjust for complexity
        let complexity_adjustment = (features.complexity * self.complexity_token_ratio) as u64;

        // Adjust for tools
        let tool_tokens = if features.likely_needs_tools {
            (features.expected_tool_calls as u64) * 500 // ~500 tokens per tool call
        } else {
            0
        };

        base_tokens + complexity_adjustment + tool_tokens
    }

    fn predict_duration(&self, features: &InputFeatures, predicted_tokens: u64) -> u64 {
        // Base latency
        let base = self.config.base_latency_ms;

        // Token-based latency
        let token_latency = (predicted_tokens as f64 * self.config.latency_per_token_ms) as u64;

        // Complexity adjustment
        let complexity_latency = (features.complexity * 1000.0) as u64; // Up to 1s for high complexity

        // Tool call overhead
        let tool_latency = if features.likely_needs_tools {
            (features.expected_tool_calls as u64) * 200 // ~200ms per tool call
        } else {
            0
        };

        // Use historical data if available
        let historical_adjustment = if !self.node_stats.is_empty() {
            let avg_duration: f64 = self
                .node_stats
                .values()
                .map(|s| s.avg_duration_ms)
                .sum::<f64>()
                / self.node_stats.len() as f64;
            (avg_duration * 0.3) as u64 // Blend 30% historical
        } else {
            0
        };

        base + token_latency + complexity_latency + tool_latency + historical_adjustment
    }

    fn predict_cost(&self, predicted_tokens: u64) -> f64 {
        (predicted_tokens as f64 / 1000.0) * self.config.cost_per_1k_tokens
    }

    fn predict_path(&self, features: &InputFeatures) -> Vec<String> {
        // Return most likely path based on input type
        if let Some(ref _input_type) = features.input_type {
            // Find path that best matches input type (if we had that metadata)
            // For now, return most common path
        }

        // Return most common path
        self.common_paths
            .first()
            .map(|(path, _)| path.clone())
            .unwrap_or_default()
    }

    fn calculate_confidence(&self, _features: &InputFeatures) -> f64 {
        // Base confidence starts at 50%
        let mut confidence = 0.5;

        // Increase confidence based on sample count
        let sample_factor =
            (self.total_samples as f64 / self.config.min_samples_for_confidence as f64).min(1.0);
        confidence += sample_factor * 0.3; // Up to +30% from samples

        // Increase confidence if we have node stats
        if !self.node_stats.is_empty() {
            confidence += 0.1;
        }

        // Increase confidence if we have path data
        if !self.common_paths.is_empty() {
            confidence += 0.1;
        }

        confidence.min(0.95) // Cap at 95%
    }

    fn calculate_intervals(&self, duration_ms: u64, tokens: u64, cost: f64) -> PredictionIntervals {
        // Use standard deviation if available, otherwise use 30% margins
        let duration_margin = if self.node_stats.is_empty() {
            (duration_ms as f64 * 0.3) as u64
        } else {
            let avg_std: f64 = self
                .node_stats
                .values()
                .map(|s| s.std_duration_ms)
                .sum::<f64>()
                / self.node_stats.len() as f64;
            (avg_std * 2.0) as u64 // 2 std devs for ~95% interval
        };

        let token_margin = if self.node_stats.is_empty() {
            (tokens as f64 * 0.3) as u64
        } else {
            let avg_std: f64 = self.node_stats.values().map(|s| s.std_tokens).sum::<f64>()
                / self.node_stats.len() as f64;
            (avg_std * 2.0) as u64
        };

        let cost_margin = cost * 0.3;

        PredictionIntervals {
            duration_low_ms: duration_ms.saturating_sub(duration_margin),
            duration_high_ms: duration_ms + duration_margin,
            tokens_low: tokens.saturating_sub(token_margin),
            tokens_high: tokens + token_margin,
            cost_low: (cost - cost_margin).max(0.0),
            cost_high: cost + cost_margin,
        }
    }

    fn generate_warnings(
        &self,
        features: &InputFeatures,
        duration_ms: u64,
        tokens: u64,
        _cost: f64,
    ) -> Vec<PredictionWarning> {
        let mut warnings = Vec::new();

        // Warn about high complexity
        if features.complexity > 0.8 {
            warnings.push(
                PredictionWarning::new(
                    WarningSeverity::Warning,
                    format!(
                        "High complexity input ({:.0}%) may increase variability",
                        features.complexity * 100.0
                    ),
                )
                .with_suggestion("Consider breaking down the task into smaller steps"),
            );
        }

        // Warn about many tool calls
        if features.expected_tool_calls > 5 {
            warnings.push(
                PredictionWarning::new(
                    WarningSeverity::Info,
                    format!(
                        "Expecting {} tool calls, which adds latency",
                        features.expected_tool_calls
                    ),
                )
                .with_suggestion("Consider batching tool calls if possible"),
            );
        }

        // Warn about high token usage
        if tokens > 10000 {
            warnings.push(
                PredictionWarning::new(
                    WarningSeverity::Info,
                    format!("High predicted token usage: {}", tokens),
                )
                .with_suggestion("Consider summarizing context to reduce tokens"),
            );
        }

        // Warn about long duration
        if duration_ms > 30000 {
            warnings.push(
                PredictionWarning::new(
                    WarningSeverity::Warning,
                    format!("Long predicted duration: {}ms", duration_ms),
                )
                .with_suggestion("Consider using streaming to get partial results earlier"),
            );
        }

        // Warn about low sample count
        if self.total_samples < self.config.min_samples_for_confidence {
            warnings.push(PredictionWarning::new(
                WarningSeverity::Info,
                format!(
                    "Low training samples ({}/{}), predictions may be less accurate",
                    self.total_samples, self.config.min_samples_for_confidence
                ),
            ));
        }

        warnings
    }
}

/// Accuracy metrics for prediction evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionAccuracy {
    /// Duration prediction error (0.0-1.0+)
    pub duration_error: f64,
    /// Token prediction error (0.0-1.0+)
    pub token_error: f64,
    /// Whether path exactly matched
    pub path_match: bool,
    /// Path similarity score (0.0-1.0)
    pub path_similarity: f64,
    /// Overall accuracy score (0.0-1.0)
    pub overall_accuracy: f64,
}

// Helper functions

fn estimate_complexity(trace: &crate::introspection::ExecutionTrace) -> f64 {
    // Estimate complexity based on execution characteristics
    let mut complexity = 0.0;

    // Token contribution (normalized to ~10k)
    complexity += (trace.total_tokens as f64 / 10000.0).min(0.5);

    // Node count contribution
    complexity += (trace.nodes_executed.len() as f64 / 10.0).min(0.3);

    // Tool call contribution
    let tool_calls: usize = trace
        .nodes_executed
        .iter()
        .map(|e| e.tools_called.len())
        .sum();
    complexity += (tool_calls as f64 / 5.0).min(0.2);

    complexity.min(1.0)
}

fn average_f64(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<u64>() as f64 / values.len() as f64
}

fn std_dev_f64(values: &[u64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let variance: f64 = values
        .iter()
        .map(|v| {
            let diff = *v as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;
    variance.sqrt()
}

fn calculate_path_similarity(predicted: &[String], actual: &[String]) -> f64 {
    if predicted.is_empty() && actual.is_empty() {
        return 1.0;
    }
    if predicted.is_empty() || actual.is_empty() {
        return 0.0;
    }

    // Simple Jaccard similarity
    let predicted_set: std::collections::HashSet<_> = predicted.iter().collect();
    let actual_set: std::collections::HashSet<_> = actual.iter().collect();

    let intersection = predicted_set.intersection(&actual_set).count();
    let union = predicted_set.union(&actual_set).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ExecutionTrace, ExecutionTraceBuilder, NodeExecution};

    fn create_simple_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("input", 100).with_tokens(500))
            .add_node_execution(NodeExecution::new("reasoning", 500).with_tokens(2000))
            .add_node_execution(NodeExecution::new("output", 100).with_tokens(500))
            .total_duration_ms(700)
            .total_tokens(3000)
            .completed(true)
            .build()
    }

    fn create_tool_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("input", 100).with_tokens(500))
            .add_node_execution(
                NodeExecution::new("tool_call", 1000)
                    .with_tokens(1500)
                    .with_tools(vec!["search".to_string(), "calculate".to_string()]),
            )
            .add_node_execution(NodeExecution::new("output", 200).with_tokens(800))
            .total_duration_ms(1300)
            .total_tokens(2800)
            .completed(true)
            .build()
    }

    fn create_complex_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("input", 200).with_tokens(2000))
            .add_node_execution(NodeExecution::new("reasoning", 2000).with_tokens(8000))
            .add_node_execution(
                NodeExecution::new("tool_call", 500)
                    .with_tokens(1000)
                    .with_tools(vec!["api".to_string()]),
            )
            .add_node_execution(NodeExecution::new("reasoning", 1500).with_tokens(5000))
            .add_node_execution(NodeExecution::new("output", 300).with_tokens(2000))
            .total_duration_ms(4500)
            .total_tokens(18000)
            .completed(true)
            .build()
    }

    #[test]
    fn test_input_features_creation() {
        let features = InputFeatures::new()
            .with_tokens(1000)
            .with_complexity(0.7)
            .with_tools(true, 3)
            .with_type("code_question");

        assert_eq!(features.input_tokens, 1000);
        assert_eq!(features.complexity, 0.7);
        assert!(features.likely_needs_tools);
        assert_eq!(features.expected_tool_calls, 3);
        assert_eq!(features.input_type, Some("code_question".to_string()));
    }

    #[test]
    fn test_input_features_from_trace() {
        let trace = create_tool_trace();
        let features = InputFeatures::from_trace(&trace);

        assert!(features.likely_needs_tools);
        assert_eq!(features.expected_tool_calls, 2);
    }

    #[test]
    fn test_execution_prediction_creation() {
        let prediction = ExecutionPrediction::new()
            .with_duration(1000)
            .with_tokens(5000)
            .with_cost(0.01)
            .with_path(vec!["input".to_string(), "output".to_string()])
            .with_confidence(0.8);

        assert_eq!(prediction.predicted_duration_ms, 1000);
        assert_eq!(prediction.predicted_tokens, 5000);
        assert_eq!(prediction.predicted_cost, 0.01);
        assert_eq!(prediction.predicted_path.len(), 2);
        assert_eq!(prediction.confidence, 0.8);
    }

    #[test]
    fn test_prediction_exceeds_budget() {
        let prediction = ExecutionPrediction::new().with_cost(0.05);
        assert!(prediction.exceeds_budget(0.03));
        assert!(!prediction.exceeds_budget(0.10));
    }

    #[test]
    fn test_prediction_exceeds_time_limit() {
        let prediction = ExecutionPrediction::new().with_duration(5000);
        assert!(prediction.exceeds_time_limit(3000));
        assert!(!prediction.exceeds_time_limit(10000));
    }

    #[test]
    fn test_prediction_summary() {
        let prediction = ExecutionPrediction::new()
            .with_duration(1000)
            .with_tokens(5000)
            .with_cost(0.01)
            .with_confidence(0.75);

        let summary = prediction.summary();
        assert!(summary.contains("1000ms"));
        assert!(summary.contains("5000"));
        assert!(summary.contains("75.0%"));
    }

    #[test]
    fn test_predictor_train_empty() {
        let predictor = ExecutionPredictor::train(&[]);
        assert_eq!(predictor.total_samples(), 0);
    }

    #[test]
    fn test_predictor_train_single() {
        let traces = vec![create_simple_trace()];
        let predictor = ExecutionPredictor::train(&traces);

        assert_eq!(predictor.total_samples(), 1);
        assert!(!predictor.node_stats.is_empty());
        assert!(predictor.get_node_stats("reasoning").is_some());
    }

    #[test]
    fn test_predictor_train_multiple() {
        let traces = vec![
            create_simple_trace(),
            create_tool_trace(),
            create_complex_trace(),
        ];
        let predictor = ExecutionPredictor::train(&traces);

        assert_eq!(predictor.total_samples(), 3);
        assert!(!predictor.common_paths.is_empty());
    }

    #[test]
    fn test_predictor_predict_basic() {
        let traces = vec![
            create_simple_trace(),
            create_simple_trace(),
            create_simple_trace(),
        ];
        let predictor = ExecutionPredictor::train(&traces);

        let features = InputFeatures::new().with_tokens(1000).with_complexity(0.5);

        let prediction = predictor.predict(&features);

        assert!(prediction.predicted_duration_ms > 0);
        assert!(prediction.predicted_tokens > 0);
        assert!(prediction.predicted_cost > 0.0);
        assert!(prediction.confidence > 0.0);
    }

    #[test]
    fn test_predictor_predict_with_tools() {
        let traces = vec![create_tool_trace()];
        let predictor = ExecutionPredictor::train(&traces);

        let features = InputFeatures::new().with_tokens(500).with_tools(true, 2);

        let prediction = predictor.predict(&features);

        // With tools, should predict more tokens/duration
        assert!(prediction.predicted_tokens > 1000);
    }

    #[test]
    fn test_predictor_predict_with_budget() {
        let traces = vec![create_simple_trace()];
        let predictor = ExecutionPredictor::train(&traces);

        let features = InputFeatures::new().with_tokens(10000).with_complexity(0.9);

        let prediction = predictor.predict_with_budget(&features, 0.001);

        // Should have budget warning
        let has_budget_warning = prediction
            .warnings
            .iter()
            .any(|w| w.message.contains("budget") || w.message.contains("cost"));
        assert!(has_budget_warning);
    }

    #[test]
    fn test_predictor_predict_with_time_limit() {
        let traces = vec![create_complex_trace()];
        let predictor = ExecutionPredictor::train(&traces);

        let features = InputFeatures::new()
            .with_tokens(5000)
            .with_complexity(0.9)
            .with_tools(true, 5);

        let prediction = predictor.predict_with_time_limit(&features, 100);

        // Should have time limit warning
        let has_time_warning = prediction
            .warnings
            .iter()
            .any(|w| w.message.contains("duration") || w.message.contains("limit"));
        assert!(has_time_warning);
    }

    #[test]
    fn test_predictor_evaluate_prediction() {
        let trace = create_simple_trace();
        let predictor = ExecutionPredictor::train(&[trace.clone()]);

        let prediction = ExecutionPrediction::new()
            .with_duration(800)
            .with_tokens(3500)
            .with_path(vec![
                "input".to_string(),
                "reasoning".to_string(),
                "output".to_string(),
            ]);

        let accuracy = predictor.evaluate_prediction(&prediction, &trace);

        // Duration error: |800 - 700| / 700 = 14%
        assert!(accuracy.duration_error < 0.2);
        // Token error: |3500 - 3000| / 3000 = 17%
        assert!(accuracy.token_error < 0.2);
        // Path should match
        assert!(accuracy.path_similarity > 0.9);
    }

    #[test]
    fn test_prediction_intervals() {
        let predictor = ExecutionPredictor::new();
        let features = InputFeatures::new().with_tokens(1000);
        let prediction = predictor.predict(&features);

        // Intervals should bracket the prediction
        assert!(prediction.intervals.duration_low_ms <= prediction.predicted_duration_ms);
        assert!(prediction.intervals.duration_high_ms >= prediction.predicted_duration_ms);
        assert!(prediction.intervals.tokens_low <= prediction.predicted_tokens);
        assert!(prediction.intervals.tokens_high >= prediction.predicted_tokens);
    }

    #[test]
    fn test_warning_severity_display() {
        assert_eq!(WarningSeverity::Info.to_string(), "INFO");
        assert_eq!(WarningSeverity::Warning.to_string(), "WARN");
        assert_eq!(WarningSeverity::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn test_prediction_warning_creation() {
        let warning = PredictionWarning::new(WarningSeverity::Warning, "Test warning")
            .with_suggestion("Fix it");

        assert_eq!(warning.severity, WarningSeverity::Warning);
        assert_eq!(warning.message, "Test warning");
        assert_eq!(warning.suggestion, Some("Fix it".to_string()));
    }

    #[test]
    fn test_node_stats() {
        let traces = vec![create_simple_trace(), create_simple_trace()];
        let predictor = ExecutionPredictor::train(&traces);

        let stats = predictor.get_node_stats("reasoning").unwrap();
        assert_eq!(stats.node, "reasoning");
        assert!(stats.avg_duration_ms > 0.0);
        assert!(stats.avg_tokens > 0.0);
        assert_eq!(stats.sample_count, 2);
        assert!(stats.success_rate > 0.0);
    }

    #[test]
    fn test_common_paths() {
        let traces = vec![
            create_simple_trace(),
            create_simple_trace(),
            create_tool_trace(),
        ];
        let predictor = ExecutionPredictor::train(&traces);

        let paths = predictor.get_common_paths();
        assert!(!paths.is_empty());

        // Most common should be first
        let (most_common_path, frequency) = &paths[0];
        assert!(!most_common_path.is_empty());
        assert!(*frequency > 0.0);
    }

    #[test]
    fn test_prediction_json_roundtrip() {
        let prediction = ExecutionPrediction::new()
            .with_duration(1000)
            .with_tokens(5000)
            .with_confidence(0.8);

        let json = prediction.to_json().unwrap();
        let parsed = ExecutionPrediction::from_json(&json).unwrap();

        assert_eq!(
            parsed.predicted_duration_ms,
            prediction.predicted_duration_ms
        );
        assert_eq!(parsed.predicted_tokens, prediction.predicted_tokens);
    }

    #[test]
    fn test_estimate_complexity() {
        let simple = create_simple_trace();
        let complex = create_complex_trace();

        let simple_complexity = estimate_complexity(&simple);
        let complex_complexity = estimate_complexity(&complex);

        assert!(complex_complexity > simple_complexity);
    }

    #[test]
    fn test_path_similarity() {
        let path1 = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let path2 = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(calculate_path_similarity(&path1, &path2), 1.0);

        let path3 = vec!["x".to_string(), "y".to_string(), "z".to_string()];
        assert_eq!(calculate_path_similarity(&path1, &path3), 0.0);

        let path4 = vec!["a".to_string(), "b".to_string(), "x".to_string()];
        let similarity = calculate_path_similarity(&path1, &path4);
        assert!(similarity > 0.0 && similarity < 1.0);
    }

    #[test]
    fn test_predictor_update() {
        let mut predictor = ExecutionPredictor::new();
        assert_eq!(predictor.total_samples(), 0);

        predictor.update(&[create_simple_trace()]);
        assert_eq!(predictor.total_samples(), 1);

        predictor.update(&[create_simple_trace(), create_tool_trace()]);
        assert_eq!(predictor.total_samples(), 3);
    }

    #[test]
    fn test_confidence_increases_with_samples() {
        let features = InputFeatures::new().with_tokens(1000);

        let predictor1 = ExecutionPredictor::train(&[create_simple_trace()]);
        let pred1 = predictor1.predict(&features);

        let many_traces: Vec<_> = (0..20).map(|_| create_simple_trace()).collect();
        let predictor2 = ExecutionPredictor::train(&many_traces);
        let pred2 = predictor2.predict(&features);

        // More samples should increase confidence
        assert!(pred2.confidence > pred1.confidence);
    }
}
