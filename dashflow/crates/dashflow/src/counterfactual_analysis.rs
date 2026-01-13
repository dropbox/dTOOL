// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Counterfactual Analysis - AI Simulates "What If" Scenarios
//!
//! This module provides counterfactual analysis capabilities that allow AI agents to
//! simulate alternative decisions and estimate their impact without re-executing.
//!
//! ## Overview
//!
//! Counterfactual analysis enables AI agents to:
//! - Ask "What if I had used a different model?"
//! - Estimate impact of alternative timeout settings
//! - Predict outcomes of caching strategies
//! - Compare sequential vs parallel execution
//!
//! ## Key Concepts
//!
//! - **Decision**: A choice that could have been made differently
//! - **Alternative**: An alternative choice to compare against actual
//! - **CounterfactualResult**: Predicted outcome of the alternative
//! - **Improvement**: Estimated improvement in metrics (latency, tokens, cost)
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::counterfactual_analysis::{Decision, Alternative};
//!
//! // AI asks: "What if I had used GPT-3.5 instead of GPT-4?"
//! let result = trace.counterfactual_analysis(
//!     "reasoning",
//!     Alternative::UseModel("gpt-3.5-turbo".into())
//! );
//!
//! println!("If I had used gpt-3.5-turbo:");
//! println!("  Latency: {}ms faster", result.estimated_improvement.latency_ms);
//! println!("  Tokens: {} fewer", result.estimated_improvement.tokens);
//! println!("  Cost: ${:.4} cheaper", result.estimated_improvement.cost);
//! println!("  Quality: {:.1}% change", result.estimated_improvement.quality_delta * 100.0);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of alternative decisions that can be simulated
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Alternative {
    /// Use a different model
    UseModel(String),
    /// Use a different timeout (in ms)
    UseTimeout(u64),
    /// Skip this node entirely
    SkipNode,
    /// Cache the result of this node
    CacheResult,
    /// Run in parallel with another node
    ParallelWith(String),
    /// Use fewer tokens/shorter context
    ReduceTokens(u64),
    /// Use more retries
    IncreaseRetries(u32),
    /// Batch with other operations
    BatchOperations(Vec<String>),
    /// Use streaming instead of waiting
    UseStreaming,
    /// Custom alternative with description
    Custom(String),
}

impl std::fmt::Display for Alternative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Alternative::UseModel(model) => write!(f, "use model '{}'", model),
            Alternative::UseTimeout(ms) => write!(f, "use {}ms timeout", ms),
            Alternative::SkipNode => write!(f, "skip this node"),
            Alternative::CacheResult => write!(f, "cache the result"),
            Alternative::ParallelWith(node) => write!(f, "run parallel with '{}'", node),
            Alternative::ReduceTokens(tokens) => write!(f, "reduce to {} tokens", tokens),
            Alternative::IncreaseRetries(n) => write!(f, "use {} retries", n),
            Alternative::BatchOperations(ops) => write!(f, "batch with {} operations", ops.len()),
            Alternative::UseStreaming => write!(f, "use streaming"),
            Alternative::Custom(desc) => write!(f, "{}", desc),
        }
    }
}

/// Estimated improvement from a counterfactual alternative
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Improvement {
    /// Estimated latency change in milliseconds (positive = faster)
    pub latency_ms: i64,
    /// Estimated token usage change (positive = fewer tokens)
    pub tokens: i64,
    /// Estimated cost change in dollars (positive = cheaper)
    pub cost: f64,
    /// Estimated quality change (-1.0 to 1.0, positive = better)
    pub quality_delta: f64,
    /// Estimated error rate change (-1.0 to 1.0, positive = fewer errors)
    pub error_rate_delta: f64,
}

impl Improvement {
    /// Create a new improvement estimate
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set latency improvement
    #[must_use]
    pub fn with_latency(mut self, ms: i64) -> Self {
        self.latency_ms = ms;
        self
    }

    /// Set token improvement
    #[must_use]
    pub fn with_tokens(mut self, tokens: i64) -> Self {
        self.tokens = tokens;
        self
    }

    /// Set cost improvement
    #[must_use]
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost = cost;
        self
    }

    /// Set quality delta
    #[must_use]
    pub fn with_quality(mut self, delta: f64) -> Self {
        self.quality_delta = delta.clamp(-1.0, 1.0);
        self
    }

    /// Set error rate delta
    #[must_use]
    pub fn with_error_rate(mut self, delta: f64) -> Self {
        self.error_rate_delta = delta.clamp(-1.0, 1.0);
        self
    }

    /// Check if this is an overall improvement
    #[must_use]
    pub fn is_beneficial(&self) -> bool {
        // Weighted scoring: latency matters most, then tokens, then quality
        let score = (self.latency_ms as f64 * 0.01)  // 100ms = 1 point
            + (self.tokens as f64 * 0.001)           // 1000 tokens = 1 point
            + (self.cost * 10.0)                      // $0.10 = 1 point
            + (self.quality_delta * 5.0)              // 20% quality = 1 point
            + (self.error_rate_delta * 3.0); // 33% error reduction = 1 point

        score > 0.0
    }

    /// Get a human-readable summary
    #[must_use]
    pub fn summary(&self) -> String {
        // At most 5 metrics (latency, tokens, cost, quality, retries)
        let mut parts = Vec::with_capacity(5);

        if self.latency_ms != 0 {
            if self.latency_ms > 0 {
                parts.push(format!("{}ms faster", self.latency_ms));
            } else {
                parts.push(format!("{}ms slower", -self.latency_ms));
            }
        }

        if self.tokens != 0 {
            if self.tokens > 0 {
                parts.push(format!("{} fewer tokens", self.tokens));
            } else {
                parts.push(format!("{} more tokens", -self.tokens));
            }
        }

        if (self.cost).abs() > 0.0001 {
            if self.cost > 0.0 {
                parts.push(format!("${:.4} cheaper", self.cost));
            } else {
                parts.push(format!("${:.4} more expensive", -self.cost));
            }
        }

        if (self.quality_delta).abs() > 0.01 {
            if self.quality_delta > 0.0 {
                parts.push(format!("{:.1}% better quality", self.quality_delta * 100.0));
            } else {
                parts.push(format!("{:.1}% worse quality", -self.quality_delta * 100.0));
            }
        }

        if parts.is_empty() {
            "No significant change".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Result of counterfactual analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterfactualResult {
    /// The node being analyzed
    pub node: String,
    /// The alternative considered
    pub alternative: Alternative,
    /// Actual outcome metrics
    pub actual_outcome: OutcomeMetrics,
    /// Predicted outcome with alternative
    pub predicted_outcome: OutcomeMetrics,
    /// Estimated improvement
    pub estimated_improvement: Improvement,
    /// Confidence in this estimate (0.0-1.0)
    pub confidence: f64,
    /// Reasoning for this estimate
    pub reasoning: String,
    /// Recommendation based on analysis
    pub recommendation: Recommendation,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CounterfactualResult {
    /// Create a new counterfactual result
    #[must_use]
    pub fn new(node: impl Into<String>, alternative: Alternative) -> Self {
        Self {
            node: node.into(),
            alternative,
            actual_outcome: OutcomeMetrics::default(),
            predicted_outcome: OutcomeMetrics::default(),
            estimated_improvement: Improvement::default(),
            confidence: 0.5,
            reasoning: String::new(),
            recommendation: Recommendation::NoChange,
            metadata: HashMap::new(),
        }
    }

    /// Set actual outcome
    #[must_use]
    pub fn with_actual(mut self, outcome: OutcomeMetrics) -> Self {
        self.actual_outcome = outcome;
        self
    }

    /// Set predicted outcome
    #[must_use]
    pub fn with_predicted(mut self, outcome: OutcomeMetrics) -> Self {
        self.predicted_outcome = outcome;
        self
    }

    /// Set improvement
    #[must_use]
    pub fn with_improvement(mut self, improvement: Improvement) -> Self {
        self.estimated_improvement = improvement;
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set reasoning
    #[must_use]
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = reasoning.into();
        self
    }

    /// Set recommendation
    #[must_use]
    pub fn with_recommendation(mut self, recommendation: Recommendation) -> Self {
        self.recommendation = recommendation;
        self
    }

    /// Check if the alternative would be beneficial
    #[must_use]
    pub fn is_beneficial(&self) -> bool {
        self.estimated_improvement.is_beneficial()
    }

    /// Get a human-readable report
    #[must_use]
    pub fn report(&self) -> String {
        let mut lines = vec![
            format!(
                "Counterfactual Analysis: {} at '{}'",
                self.alternative, self.node
            ),
            String::new(),
        ];

        lines.push("Actual outcome:".to_string());
        lines.push(format!("  Latency: {}ms", self.actual_outcome.latency_ms));
        lines.push(format!("  Tokens: {}", self.actual_outcome.tokens));
        lines.push(format!("  Success: {}", self.actual_outcome.success));

        lines.push(String::new());
        lines.push(format!("Predicted outcome (with {}):", self.alternative));
        lines.push(format!(
            "  Latency: {}ms",
            self.predicted_outcome.latency_ms
        ));
        lines.push(format!("  Tokens: {}", self.predicted_outcome.tokens));
        lines.push(format!(
            "  Success probability: {:.1}%",
            self.predicted_outcome.success_probability * 100.0
        ));

        lines.push(String::new());
        lines.push(format!(
            "Estimated change: {}",
            self.estimated_improvement.summary()
        ));
        lines.push(format!("Confidence: {:.1}%", self.confidence * 100.0));
        lines.push(format!("Recommendation: {:?}", self.recommendation));

        if !self.reasoning.is_empty() {
            lines.push(String::new());
            lines.push(format!("Reasoning: {}", self.reasoning));
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

/// Outcome metrics for a node execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutcomeMetrics {
    /// Latency in milliseconds
    pub latency_ms: u64,
    /// Tokens used
    pub tokens: u64,
    /// Cost in dollars
    pub cost: f64,
    /// Whether execution succeeded
    pub success: bool,
    /// Success probability (for predictions)
    pub success_probability: f64,
    /// Number of retries
    pub retries: u32,
}

impl OutcomeMetrics {
    /// Create from actual execution
    #[must_use]
    pub fn from_execution(exec: &crate::introspection::NodeExecution) -> Self {
        Self {
            latency_ms: exec.duration_ms,
            tokens: exec.tokens_used,
            cost: 0.0, // Cost not tracked in basic execution
            success: exec.success,
            success_probability: if exec.success { 1.0 } else { 0.0 },
            retries: 0,
        }
    }

    /// Create a prediction
    #[must_use]
    pub fn predicted(latency_ms: u64, tokens: u64, success_probability: f64) -> Self {
        Self {
            latency_ms,
            tokens,
            cost: 0.0,
            success: success_probability > 0.5,
            success_probability,
            retries: 0,
        }
    }
}

/// Recommendation based on counterfactual analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recommendation {
    /// Keep current approach
    NoChange,
    /// Consider the alternative
    ConsiderAlternative,
    /// Strongly recommend the alternative
    StronglyRecommend,
    /// Avoid the alternative
    AvoidAlternative,
    /// Need more data to decide
    NeedMoreData,
}

/// Configuration for counterfactual analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterfactualConfig {
    /// Model performance estimates (model name -> relative speed factor)
    pub model_speed_factors: HashMap<String, f64>,
    /// Model cost estimates (model name -> cost per 1k tokens)
    pub model_costs: HashMap<String, f64>,
    /// Model quality estimates (model name -> relative quality factor)
    pub model_quality_factors: HashMap<String, f64>,
    /// Estimated cache hit improvement factor (0.0-1.0)
    pub cache_hit_factor: f64,
    /// Estimated parallel execution improvement factor
    pub parallel_factor: f64,
    /// Minimum confidence to recommend
    pub min_confidence_to_recommend: f64,
}

impl Default for CounterfactualConfig {
    fn default() -> Self {
        let mut model_speed_factors = HashMap::new();
        model_speed_factors.insert("gpt-4".to_string(), 1.0);
        model_speed_factors.insert("gpt-4-turbo".to_string(), 0.8);
        model_speed_factors.insert("gpt-3.5-turbo".to_string(), 0.3);
        model_speed_factors.insert("claude-3-opus".to_string(), 1.2);
        model_speed_factors.insert("claude-3-sonnet".to_string(), 0.6);
        model_speed_factors.insert("claude-3-haiku".to_string(), 0.2);

        let mut model_costs = HashMap::new();
        model_costs.insert("gpt-4".to_string(), 0.03); // $0.03/1k tokens
        model_costs.insert("gpt-4-turbo".to_string(), 0.01);
        model_costs.insert("gpt-3.5-turbo".to_string(), 0.0015);
        model_costs.insert("claude-3-opus".to_string(), 0.015);
        model_costs.insert("claude-3-sonnet".to_string(), 0.003);
        model_costs.insert("claude-3-haiku".to_string(), 0.00025);

        let mut model_quality_factors = HashMap::new();
        model_quality_factors.insert("gpt-4".to_string(), 1.0);
        model_quality_factors.insert("gpt-4-turbo".to_string(), 0.95);
        model_quality_factors.insert("gpt-3.5-turbo".to_string(), 0.7);
        model_quality_factors.insert("claude-3-opus".to_string(), 1.0);
        model_quality_factors.insert("claude-3-sonnet".to_string(), 0.85);
        model_quality_factors.insert("claude-3-haiku".to_string(), 0.6);

        Self {
            model_speed_factors,
            model_costs,
            model_quality_factors,
            cache_hit_factor: 0.95, // 95% latency reduction on cache hit
            parallel_factor: 0.5,   // 50% latency reduction for parallel
            min_confidence_to_recommend: 0.7, // 70% confidence minimum
        }
    }
}

impl CounterfactualConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the cache hit improvement factor (0.0-1.0).
    /// Higher values mean more latency reduction on cache hits.
    #[must_use]
    pub fn with_cache_hit_factor(mut self, factor: f64) -> Self {
        self.cache_hit_factor = factor;
        self
    }

    /// Set the parallel execution improvement factor (0.0-1.0).
    /// Higher values mean more latency reduction for parallel execution.
    #[must_use]
    pub fn with_parallel_factor(mut self, factor: f64) -> Self {
        self.parallel_factor = factor;
        self
    }

    /// Set the minimum confidence to make a recommendation (0.0-1.0).
    #[must_use]
    pub fn with_min_confidence_to_recommend(mut self, confidence: f64) -> Self {
        self.min_confidence_to_recommend = confidence;
        self
    }

    /// Add or update a model's speed factor.
    #[must_use]
    pub fn with_model_speed_factor(mut self, model: impl Into<String>, factor: f64) -> Self {
        self.model_speed_factors.insert(model.into(), factor);
        self
    }

    /// Add or update a model's cost per 1k tokens.
    #[must_use]
    pub fn with_model_cost(mut self, model: impl Into<String>, cost: f64) -> Self {
        self.model_costs.insert(model.into(), cost);
        self
    }

    /// Add or update a model's quality factor.
    #[must_use]
    pub fn with_model_quality_factor(mut self, model: impl Into<String>, factor: f64) -> Self {
        self.model_quality_factors.insert(model.into(), factor);
        self
    }
}

/// Counterfactual analyzer for execution traces
pub struct CounterfactualAnalyzer {
    config: CounterfactualConfig,
}

impl Default for CounterfactualAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CounterfactualAnalyzer {
    /// Create a new analyzer with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: CounterfactualConfig::default(),
        }
    }

    /// Create an analyzer with custom configuration
    #[must_use]
    pub fn with_config(config: CounterfactualConfig) -> Self {
        Self { config }
    }

    /// Analyze a counterfactual for a specific node
    #[must_use]
    pub fn analyze(
        &self,
        trace: &crate::introspection::ExecutionTrace,
        node_name: &str,
        alternative: Alternative,
    ) -> CounterfactualResult {
        // Find the node execution
        let executions: Vec<_> = trace
            .nodes_executed
            .iter()
            .filter(|e| e.node == node_name)
            .collect();

        if executions.is_empty() {
            return CounterfactualResult::new(node_name, alternative.clone())
                .with_confidence(0.0)
                .with_reasoning(format!("Node '{}' not found in trace", node_name))
                .with_recommendation(Recommendation::NeedMoreData);
        }

        // Calculate actual metrics
        let total_latency: u64 = executions.iter().map(|e| e.duration_ms).sum();
        let total_tokens: u64 = executions.iter().map(|e| e.tokens_used).sum();
        let success_rate =
            executions.iter().filter(|e| e.success).count() as f64 / executions.len() as f64;

        let actual = OutcomeMetrics {
            latency_ms: total_latency,
            tokens: total_tokens,
            cost: self.estimate_cost(total_tokens, None),
            success: success_rate > 0.5,
            success_probability: success_rate,
            retries: (executions.len() - 1) as u32,
        };

        // Calculate predicted metrics based on alternative
        let (predicted, improvement, confidence, reasoning) = match &alternative {
            Alternative::UseModel(model) => self.analyze_model_alternative(&actual, model),
            Alternative::UseTimeout(timeout) => {
                self.analyze_timeout_alternative(&actual, *timeout, &executions)
            }
            Alternative::SkipNode => self.analyze_skip_alternative(&actual),
            Alternative::CacheResult => self.analyze_cache_alternative(&actual, executions.len()),
            Alternative::ParallelWith(other) => {
                self.analyze_parallel_alternative(trace, &actual, other)
            }
            Alternative::ReduceTokens(tokens) => {
                self.analyze_reduce_tokens_alternative(&actual, *tokens)
            }
            Alternative::IncreaseRetries(retries) => {
                self.analyze_retries_alternative(&actual, *retries)
            }
            Alternative::BatchOperations(ops) => self.analyze_batch_alternative(&actual, ops.len()),
            Alternative::UseStreaming => self.analyze_streaming_alternative(&actual),
            Alternative::Custom(desc) => self.analyze_custom_alternative(&actual, desc),
        };

        let recommendation = self.determine_recommendation(&improvement, confidence);

        CounterfactualResult::new(node_name, alternative)
            .with_actual(actual)
            .with_predicted(predicted)
            .with_improvement(improvement)
            .with_confidence(confidence)
            .with_reasoning(reasoning)
            .with_recommendation(recommendation)
    }

    /// Analyze all reasonable alternatives for a node
    #[must_use]
    pub fn analyze_all_alternatives(
        &self,
        trace: &crate::introspection::ExecutionTrace,
        node_name: &str,
    ) -> Vec<CounterfactualResult> {
        // 3 models + cache + streaming + potentially token reduction = max 6
        let mut results = Vec::with_capacity(6);

        // Try different models
        for model in ["gpt-3.5-turbo", "claude-3-haiku", "claude-3-sonnet"] {
            results.push(self.analyze(trace, node_name, Alternative::UseModel(model.to_string())));
        }

        // Try caching
        results.push(self.analyze(trace, node_name, Alternative::CacheResult));

        // Try streaming
        results.push(self.analyze(trace, node_name, Alternative::UseStreaming));

        // Try token reduction
        if let Some(exec) = trace.nodes_executed.iter().find(|e| e.node == node_name) {
            if exec.tokens_used > 1000 {
                results.push(self.analyze(
                    trace,
                    node_name,
                    Alternative::ReduceTokens(exec.tokens_used / 2),
                ));
            }
        }

        // Sort by estimated improvement
        results.sort_by(|a, b| {
            let score_a = a.estimated_improvement.latency_ms as f64 * a.confidence;
            let score_b = b.estimated_improvement.latency_ms as f64 * b.confidence;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Get best recommendations across all nodes
    #[must_use]
    pub fn best_recommendations(
        &self,
        trace: &crate::introspection::ExecutionTrace,
        limit: usize,
    ) -> Vec<CounterfactualResult> {
        let mut all_results = Vec::new();

        // Get unique nodes
        let mut seen_nodes = std::collections::HashSet::new();
        for exec in &trace.nodes_executed {
            if seen_nodes.insert(&exec.node) {
                all_results.extend(self.analyze_all_alternatives(trace, &exec.node));
            }
        }

        // Filter to beneficial recommendations
        all_results.retain(|r| {
            r.is_beneficial() && r.confidence >= self.config.min_confidence_to_recommend
        });

        // Sort by improvement score
        all_results.sort_by(|a, b| {
            let score_a = a.estimated_improvement.latency_ms as f64 * a.confidence;
            let score_b = b.estimated_improvement.latency_ms as f64 * b.confidence;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        all_results.truncate(limit);
        all_results
    }

    fn estimate_cost(&self, tokens: u64, model: Option<&str>) -> f64 {
        let cost_per_1k = model
            .and_then(|m| self.config.model_costs.get(m))
            .copied()
            .unwrap_or(0.01);

        tokens as f64 * cost_per_1k / 1000.0
    }

    fn analyze_model_alternative(
        &self,
        actual: &OutcomeMetrics,
        model: &str,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        let speed_factor = self
            .config
            .model_speed_factors
            .get(model)
            .copied()
            .unwrap_or(1.0);

        let quality_factor = self
            .config
            .model_quality_factors
            .get(model)
            .copied()
            .unwrap_or(1.0);

        let predicted_latency = (actual.latency_ms as f64 * speed_factor) as u64;
        let predicted_cost = self.estimate_cost(actual.tokens, Some(model));

        let predicted = OutcomeMetrics {
            latency_ms: predicted_latency,
            tokens: actual.tokens, // Tokens roughly same for same task
            cost: predicted_cost,
            success: actual.success,
            success_probability: actual.success_probability * quality_factor.min(1.0),
            retries: actual.retries,
        };

        let improvement = Improvement::new()
            .with_latency((actual.latency_ms as i64) - (predicted_latency as i64))
            .with_cost(actual.cost - predicted_cost)
            .with_quality(quality_factor - 1.0);

        let confidence = if self.config.model_speed_factors.contains_key(model) {
            0.8
        } else {
            0.5
        };

        let reasoning = format!(
            "Model '{}' is estimated to be {:.1}x speed with {:.1}x quality",
            model, speed_factor, quality_factor
        );

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_timeout_alternative(
        &self,
        actual: &OutcomeMetrics,
        timeout: u64,
        executions: &[&crate::introspection::NodeExecution],
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        // Would a different timeout have helped?
        let would_timeout = actual.latency_ms > timeout;
        let max_latency = executions.iter().map(|e| e.duration_ms).max().unwrap_or(0);

        let predicted = if would_timeout {
            OutcomeMetrics {
                latency_ms: timeout,
                tokens: actual.tokens / 2, // Partial execution
                cost: actual.cost / 2.0,
                success: false,
                success_probability: 0.0,
                retries: actual.retries + 1,
            }
        } else {
            OutcomeMetrics {
                latency_ms: actual.latency_ms.min(timeout),
                tokens: actual.tokens,
                cost: actual.cost,
                success: actual.success,
                success_probability: actual.success_probability,
                retries: actual.retries,
            }
        };

        let improvement = if would_timeout {
            Improvement::new()
                .with_latency((actual.latency_ms as i64) - (timeout as i64))
                .with_quality(-0.5) // Failed execution = bad
        } else {
            Improvement::new() // No change if not hitting timeout
        };

        let confidence = 0.9; // High confidence for timeout analysis

        let reasoning = if would_timeout {
            format!(
                "Timeout of {}ms would have cut execution short (actual: {}ms)",
                timeout, actual.latency_ms
            )
        } else {
            format!(
                "Timeout of {}ms would not affect execution (max: {}ms)",
                timeout, max_latency
            )
        };

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_skip_alternative(
        &self,
        actual: &OutcomeMetrics,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        let predicted = OutcomeMetrics {
            latency_ms: 0,
            tokens: 0,
            cost: 0.0,
            success: true, // Skip always "succeeds"
            success_probability: 1.0,
            retries: 0,
        };

        let improvement = Improvement::new()
            .with_latency(actual.latency_ms as i64)
            .with_tokens(actual.tokens as i64)
            .with_cost(actual.cost)
            .with_quality(-0.5); // Skipping likely loses quality

        let confidence = 0.3; // Low confidence - we don't know if skip is safe

        let reasoning = "Skipping node saves all resources but may affect correctness".to_string();

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_cache_alternative(
        &self,
        actual: &OutcomeMetrics,
        execution_count: usize,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        // Caching helps more when there are repeated executions
        let cache_benefit = if execution_count > 1 {
            self.config.cache_hit_factor * (execution_count - 1) as f64 / execution_count as f64
        } else {
            0.0
        };

        let predicted_latency = (actual.latency_ms as f64 * (1.0 - cache_benefit)) as u64;

        let predicted = OutcomeMetrics {
            latency_ms: predicted_latency,
            tokens: (actual.tokens as f64 * (1.0 - cache_benefit * 0.5)) as u64,
            cost: actual.cost * (1.0 - cache_benefit * 0.5),
            success: actual.success,
            success_probability: actual.success_probability,
            retries: 0,
        };

        let improvement = Improvement::new()
            .with_latency((actual.latency_ms as i64) - (predicted_latency as i64))
            .with_tokens((actual.tokens as i64) - (predicted.tokens as i64))
            .with_cost(actual.cost - predicted.cost);

        let confidence = if execution_count > 1 { 0.85 } else { 0.4 };

        let reasoning = format!(
            "Caching with {} execution(s) would provide {:.1}% speedup",
            execution_count,
            cache_benefit * 100.0
        );

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_parallel_alternative(
        &self,
        trace: &crate::introspection::ExecutionTrace,
        actual: &OutcomeMetrics,
        other_node: &str,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        // Find the other node's execution time
        let other_latency: u64 = trace
            .nodes_executed
            .iter()
            .filter(|e| e.node == other_node)
            .map(|e| e.duration_ms)
            .sum();

        if other_latency == 0 {
            return (
                actual.clone(),
                Improvement::new(),
                0.3,
                format!("Node '{}' not found or has no latency", other_node),
            );
        }

        // Parallel execution takes max(a, b) instead of a + b
        let sequential_time = actual.latency_ms + other_latency;
        let parallel_time = actual.latency_ms.max(other_latency);
        let savings = sequential_time - parallel_time;

        let predicted = OutcomeMetrics {
            latency_ms: actual.latency_ms, // This node's time stays the same
            tokens: actual.tokens,
            cost: actual.cost,
            success: actual.success,
            success_probability: actual.success_probability,
            retries: actual.retries,
        };

        let improvement = Improvement::new().with_latency(savings as i64);

        let confidence = 0.7; // Medium confidence - depends on dependencies

        let reasoning = format!(
            "Parallel with '{}' saves {}ms ({}ms + {}ms → {}ms)",
            other_node, savings, actual.latency_ms, other_latency, parallel_time
        );

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_reduce_tokens_alternative(
        &self,
        actual: &OutcomeMetrics,
        target_tokens: u64,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        let reduction_ratio = if actual.tokens > 0 {
            target_tokens as f64 / actual.tokens as f64
        } else {
            1.0
        };

        // Latency scales roughly linearly with tokens
        let predicted_latency = (actual.latency_ms as f64 * reduction_ratio) as u64;

        let predicted = OutcomeMetrics {
            latency_ms: predicted_latency,
            tokens: target_tokens,
            cost: self.estimate_cost(target_tokens, None),
            success: actual.success,
            success_probability: actual.success_probability * (0.5 + 0.5 * reduction_ratio), // Quality may drop
            retries: actual.retries,
        };

        let improvement = Improvement::new()
            .with_latency((actual.latency_ms as i64) - (predicted_latency as i64))
            .with_tokens((actual.tokens as i64) - (target_tokens as i64))
            .with_cost(actual.cost - predicted.cost)
            .with_quality((reduction_ratio - 1.0) * 0.3); // Slight quality loss

        let confidence = 0.6;

        let reasoning =
            format!(
            "Reducing from {} to {} tokens ({:.0}% reduction) would proportionally reduce latency",
            actual.tokens, target_tokens, (1.0 - reduction_ratio) * 100.0
        );

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_retries_alternative(
        &self,
        actual: &OutcomeMetrics,
        additional_retries: u32,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        // More retries increase success probability but add latency
        let current_failure_rate = 1.0 - actual.success_probability;
        let new_failure_rate = current_failure_rate.powi((additional_retries + 1) as i32);
        let new_success_prob = 1.0 - new_failure_rate;

        let avg_retries = current_failure_rate * additional_retries as f64;
        let additional_latency = (actual.latency_ms as f64 * avg_retries) as u64;

        let predicted = OutcomeMetrics {
            latency_ms: actual.latency_ms + additional_latency,
            tokens: actual.tokens + (actual.tokens as f64 * avg_retries) as u64,
            cost: actual.cost * (1.0 + avg_retries),
            success: new_success_prob > 0.5,
            success_probability: new_success_prob,
            retries: actual.retries + additional_retries,
        };

        let improvement = Improvement::new()
            .with_latency(-(additional_latency as i64))
            .with_error_rate(new_success_prob - actual.success_probability);

        let confidence = 0.75;

        let reasoning = format!(
            "Adding {} retries increases success probability from {:.1}% to {:.1}%",
            additional_retries,
            actual.success_probability * 100.0,
            new_success_prob * 100.0
        );

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_batch_alternative(
        &self,
        actual: &OutcomeMetrics,
        batch_size: usize,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        // Batching reduces overhead but may increase per-item latency
        let overhead_per_call = 100; // Estimated 100ms overhead per separate call
        let overhead_savings = overhead_per_call * (batch_size.saturating_sub(1)) as u64;

        let predicted = OutcomeMetrics {
            latency_ms: actual.latency_ms.saturating_sub(overhead_savings),
            tokens: actual.tokens,
            cost: actual.cost,
            success: actual.success,
            success_probability: actual.success_probability,
            retries: actual.retries,
        };

        let improvement = Improvement::new().with_latency(overhead_savings as i64);

        let confidence = 0.5; // Medium confidence - depends on API support

        let reasoning = format!(
            "Batching {} operations could save ~{}ms in overhead",
            batch_size, overhead_savings
        );

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_streaming_alternative(
        &self,
        actual: &OutcomeMetrics,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        // Streaming doesn't reduce total time, but reduces time-to-first-token
        let time_to_first_token = actual.latency_ms / 4; // Rough estimate

        let predicted = OutcomeMetrics {
            latency_ms: actual.latency_ms, // Total time same
            tokens: actual.tokens,
            cost: actual.cost,
            success: actual.success,
            success_probability: actual.success_probability,
            retries: actual.retries,
        };

        let improvement = Improvement::new(); // No improvement in total metrics

        let confidence = 0.8;

        let reasoning = format!(
            "Streaming would provide first response ~{}ms earlier (of {}ms total)",
            time_to_first_token, actual.latency_ms
        );

        (predicted, improvement, confidence, reasoning)
    }

    fn analyze_custom_alternative(
        &self,
        actual: &OutcomeMetrics,
        description: &str,
    ) -> (OutcomeMetrics, Improvement, f64, String) {
        // Custom alternatives return no change - user must provide estimates
        let predicted = actual.clone();
        let improvement = Improvement::new();
        let confidence = 0.3;
        let reasoning = format!(
            "Custom alternative '{}' - no automatic estimation available",
            description
        );

        (predicted, improvement, confidence, reasoning)
    }

    fn determine_recommendation(
        &self,
        improvement: &Improvement,
        confidence: f64,
    ) -> Recommendation {
        if confidence < self.config.min_confidence_to_recommend {
            return Recommendation::NeedMoreData;
        }

        let is_beneficial = improvement.is_beneficial();
        let has_quality_loss = improvement.quality_delta < -0.1;

        if !is_beneficial {
            return Recommendation::NoChange;
        }

        if has_quality_loss {
            return Recommendation::ConsiderAlternative;
        }

        if improvement.latency_ms > 500 || improvement.cost > 0.01 {
            return Recommendation::StronglyRecommend;
        }

        Recommendation::ConsiderAlternative
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ExecutionTrace, ExecutionTraceBuilder, NodeExecution};

    fn create_test_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("reasoning", 1000).with_tokens(5000))
            .add_node_execution(NodeExecution::new("tool_call", 500).with_tokens(1000))
            .add_node_execution(NodeExecution::new("output", 200).with_tokens(500))
            .total_duration_ms(1700)
            .total_tokens(6500)
            .completed(true)
            .build()
    }

    fn create_repeated_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("loop_node", 500).with_tokens(1000))
            .add_node_execution(NodeExecution::new("loop_node", 500).with_tokens(1000))
            .add_node_execution(NodeExecution::new("loop_node", 500).with_tokens(1000))
            .total_duration_ms(1500)
            .total_tokens(3000)
            .completed(true)
            .build()
    }

    #[test]
    fn test_alternative_display() {
        assert_eq!(
            Alternative::UseModel("gpt-3.5".into()).to_string(),
            "use model 'gpt-3.5'"
        );
        assert_eq!(Alternative::CacheResult.to_string(), "cache the result");
        assert_eq!(Alternative::SkipNode.to_string(), "skip this node");
    }

    #[test]
    fn test_improvement_summary() {
        let improvement = Improvement::new()
            .with_latency(500)
            .with_tokens(1000)
            .with_cost(0.01);

        let summary = improvement.summary();
        assert!(summary.contains("500ms faster"));
        assert!(summary.contains("1000 fewer tokens"));
    }

    #[test]
    fn test_improvement_is_beneficial() {
        let beneficial = Improvement::new().with_latency(1000);
        assert!(beneficial.is_beneficial());

        let not_beneficial = Improvement::new().with_latency(-1000).with_quality(-0.5);
        assert!(!not_beneficial.is_beneficial());
    }

    #[test]
    fn test_counterfactual_result_creation() {
        let result = CounterfactualResult::new("test_node", Alternative::CacheResult)
            .with_confidence(0.8)
            .with_reasoning("Test reasoning");

        assert_eq!(result.node, "test_node");
        assert_eq!(result.confidence, 0.8);
    }

    #[test]
    fn test_analyzer_model_alternative() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_test_trace();

        let result = analyzer.analyze(
            &trace,
            "reasoning",
            Alternative::UseModel("gpt-3.5-turbo".into()),
        );

        assert_eq!(result.node, "reasoning");
        assert!(result.estimated_improvement.latency_ms > 0); // Should be faster
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_analyzer_cache_alternative() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_repeated_trace();

        let result = analyzer.analyze(&trace, "loop_node", Alternative::CacheResult);

        assert!(result.estimated_improvement.latency_ms > 0); // Caching should help with repeated executions
    }

    #[test]
    fn test_analyzer_skip_alternative() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_test_trace();

        let result = analyzer.analyze(&trace, "output", Alternative::SkipNode);

        assert!(result.estimated_improvement.latency_ms > 0); // Skipping saves time
        assert!(result.estimated_improvement.quality_delta < 0.0); // But loses quality
    }

    #[test]
    fn test_analyzer_timeout_alternative() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_test_trace();

        // Timeout shorter than actual execution
        let result = analyzer.analyze(&trace, "reasoning", Alternative::UseTimeout(500));
        assert!(result.reasoning.contains("cut execution short"));

        // Timeout longer than actual execution
        let result = analyzer.analyze(&trace, "reasoning", Alternative::UseTimeout(5000));
        assert!(result.reasoning.contains("would not affect"));
    }

    #[test]
    fn test_analyzer_node_not_found() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_test_trace();

        let result = analyzer.analyze(&trace, "nonexistent", Alternative::CacheResult);

        assert_eq!(result.confidence, 0.0);
        assert_eq!(result.recommendation, Recommendation::NeedMoreData);
    }

    #[test]
    fn test_analyzer_all_alternatives() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_test_trace();

        let results = analyzer.analyze_all_alternatives(&trace, "reasoning");

        assert!(!results.is_empty());
        // Should include model alternatives and caching
    }

    #[test]
    fn test_analyzer_best_recommendations() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_test_trace();

        let results = analyzer.best_recommendations(&trace, 3);

        // Should return up to 3 beneficial recommendations
        assert!(results.len() <= 3);
        for result in &results {
            assert!(result.is_beneficial());
        }
    }

    #[test]
    fn test_outcome_metrics_from_execution() {
        let exec = NodeExecution::new("test", 1000).with_tokens(5000);
        let metrics = OutcomeMetrics::from_execution(&exec);

        assert_eq!(metrics.latency_ms, 1000);
        assert_eq!(metrics.tokens, 5000);
        assert!(metrics.success);
    }

    #[test]
    fn test_counterfactual_json_roundtrip() {
        let result = CounterfactualResult::new("test", Alternative::CacheResult)
            .with_confidence(0.75)
            .with_reasoning("Test");

        let json = result.to_json().unwrap();
        let parsed = CounterfactualResult::from_json(&json).unwrap();

        assert_eq!(parsed.node, result.node);
        assert_eq!(parsed.confidence, result.confidence);
    }

    #[test]
    fn test_recommendation_determination() {
        let analyzer = CounterfactualAnalyzer::new();

        // High improvement, high confidence -> strongly recommend
        let trace = ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("slow", 10000).with_tokens(50000))
            .total_duration_ms(10000)
            .completed(true)
            .build();

        let result = analyzer.analyze(
            &trace,
            "slow",
            Alternative::UseModel("gpt-3.5-turbo".into()),
        );

        // Should have some recommendation
        assert_ne!(result.recommendation, Recommendation::AvoidAlternative);
    }

    #[test]
    fn test_parallel_alternative() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("node_a", 500).with_tokens(1000))
            .add_node_execution(NodeExecution::new("node_b", 300).with_tokens(500))
            .total_duration_ms(800)
            .completed(true)
            .build();

        let result = analyzer.analyze(&trace, "node_a", Alternative::ParallelWith("node_b".into()));

        assert!(result.reasoning.contains("Parallel"));
        // Parallel execution should save time
    }

    #[test]
    fn test_reduce_tokens_alternative() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_test_trace();

        let result = analyzer.analyze(&trace, "reasoning", Alternative::ReduceTokens(2500));

        assert!(result.estimated_improvement.tokens > 0);
        assert!(result.estimated_improvement.latency_ms > 0);
    }

    #[test]
    fn test_retries_alternative() {
        let analyzer = CounterfactualAnalyzer::new();
        let trace = create_test_trace();

        let result = analyzer.analyze(&trace, "reasoning", Alternative::IncreaseRetries(2));

        assert!(result.reasoning.contains("retries"));
        assert!(result.reasoning.contains("success probability"));
    }

    #[test]
    fn test_counterfactual_config_new() {
        let config = CounterfactualConfig::new();
        assert!((config.cache_hit_factor - 0.95).abs() < f64::EPSILON);
        assert!((config.parallel_factor - 0.5).abs() < f64::EPSILON);
        assert!((config.min_confidence_to_recommend - 0.7).abs() < f64::EPSILON);
        // Default models should be populated
        assert!(config.model_speed_factors.contains_key("gpt-4"));
        assert!(config.model_costs.contains_key("gpt-4"));
        assert!(config.model_quality_factors.contains_key("gpt-4"));
    }

    #[test]
    fn test_counterfactual_config_builder_pattern() {
        let config = CounterfactualConfig::new()
            .with_cache_hit_factor(0.8)
            .with_parallel_factor(0.6)
            .with_min_confidence_to_recommend(0.9)
            .with_model_speed_factor("custom-model", 0.5)
            .with_model_cost("custom-model", 0.02)
            .with_model_quality_factor("custom-model", 0.9);

        assert!((config.cache_hit_factor - 0.8).abs() < f64::EPSILON);
        assert!((config.parallel_factor - 0.6).abs() < f64::EPSILON);
        assert!((config.min_confidence_to_recommend - 0.9).abs() < f64::EPSILON);
        // Custom model should be added
        assert!((config.model_speed_factors.get("custom-model").unwrap() - 0.5).abs() < f64::EPSILON);
        assert!((config.model_costs.get("custom-model").unwrap() - 0.02).abs() < f64::EPSILON);
        assert!((config.model_quality_factors.get("custom-model").unwrap() - 0.9).abs() < f64::EPSILON);
        // Default models should still be present
        assert!(config.model_speed_factors.contains_key("gpt-4"));
    }

    #[test]
    fn test_counterfactual_config_override_existing_model() {
        let config = CounterfactualConfig::new()
            .with_model_cost("gpt-4", 0.05);

        // Overridden value
        assert!((config.model_costs.get("gpt-4").unwrap() - 0.05).abs() < f64::EPSILON);
        // Other values preserved
        assert!((config.model_speed_factors.get("gpt-4").unwrap() - 1.0).abs() < f64::EPSILON);
    }
}
