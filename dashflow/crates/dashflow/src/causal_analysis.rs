// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for causal analysis
// - panic: panic!() to catch invalid states in hypothesis testing
// - needless_pass_by_value: Analysis data passed by value for ownership semantics
#![allow(clippy::panic, clippy::needless_pass_by_value)]

//! # Causal Analysis - AI Understands WHY Things Happened
//!
//! This module provides causal analysis capabilities that allow AI agents to understand
//! the root causes of execution outcomes, performance issues, and errors.
//!
//! ## Overview
//!
//! Understanding causality enables AI agents to:
//! - Identify WHY execution was slow, not just THAT it was slow
//! - Trace errors back to their root causes
//! - Understand the contribution of each factor to outcomes
//! - Make informed decisions about optimization strategies
//!
//! ## Key Concepts
//!
//! - **Effect**: An observable outcome (e.g., "high latency", "token budget exceeded")
//! - **Cause**: A factor that contributed to the effect with evidence
//! - **CausalChain**: Links an effect to its contributing causes with contribution weights
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::causal_analysis::{CausalChain, Effect};
//!
//! // AI asks: "Why was I slow?"
//! let chain = trace.analyze_causality(Effect::HighLatency);
//!
//! println!("Root causes of {}:", chain.effect);
//! for cause in chain.causes {
//!     println!("  - {} ({}%): {}",
//!         cause.factor,
//!         cause.contribution * 100.0,
//!         cause.evidence
//!     );
//! }
//!
//! // AI now knows exactly what to fix
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of effects that can be analyzed for causality
///
/// These represent observable outcomes that an AI agent might want to understand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Effect {
    /// Total execution took longer than expected
    HighLatency,
    /// A specific node was slow
    SlowNode(String),
    /// Token usage exceeded expectations
    HighTokenUsage,
    /// Execution failed with errors
    ExecutionFailure,
    /// Multiple retries were needed
    HighRetryRate,
    /// A specific node failed
    NodeFailure(String),
    /// Execution was stuck in a loop
    InfiniteLoop,
    /// Resource limits were approached or exceeded
    ResourceExhaustion,
    /// Custom effect defined by the user
    Custom(String),
}

impl std::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::HighLatency => write!(f, "high total latency"),
            Effect::SlowNode(node) => write!(f, "slow execution in '{}'", node),
            Effect::HighTokenUsage => write!(f, "high token usage"),
            Effect::ExecutionFailure => write!(f, "execution failure"),
            Effect::HighRetryRate => write!(f, "high retry rate"),
            Effect::NodeFailure(node) => write!(f, "failure in '{}'", node),
            Effect::InfiniteLoop => write!(f, "infinite loop detected"),
            Effect::ResourceExhaustion => write!(f, "resource exhaustion"),
            Effect::Custom(desc) => write!(f, "{}", desc),
        }
    }
}

/// Types of causal factors that can contribute to effects
///
/// These represent the underlying reasons why something happened.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CausalFactor {
    /// Large input/context size in tokens
    LargeContext,
    /// Complex computation or reasoning
    ComplexComputation,
    /// Many tool calls in the execution
    ManyToolCalls,
    /// Slow external API calls
    SlowExternalApi,
    /// Network latency
    NetworkLatency,
    /// Model inference time
    ModelInference,
    /// A specific node was the bottleneck
    NodeBottleneck(String),
    /// Multiple executions of the same node (loops)
    RepeatedExecution(String),
    /// Error handling and retries
    ErrorRetries,
    /// State serialization/deserialization overhead
    StateSerialization,
    /// Memory pressure
    MemoryPressure,
    /// Input validation overhead
    InputValidation,
    /// Output processing overhead
    OutputProcessing,
    /// Upstream dependency failure
    UpstreamFailure(String),
    /// Configuration issue
    ConfigurationIssue(String),
    /// Custom factor defined by analysis
    Custom(String),
}

impl std::fmt::Display for CausalFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CausalFactor::LargeContext => write!(f, "large context size"),
            CausalFactor::ComplexComputation => write!(f, "complex computation"),
            CausalFactor::ManyToolCalls => write!(f, "many tool calls"),
            CausalFactor::SlowExternalApi => write!(f, "slow external API"),
            CausalFactor::NetworkLatency => write!(f, "network latency"),
            CausalFactor::ModelInference => write!(f, "model inference time"),
            CausalFactor::NodeBottleneck(node) => write!(f, "bottleneck in '{}'", node),
            CausalFactor::RepeatedExecution(node) => write!(f, "repeated execution of '{}'", node),
            CausalFactor::ErrorRetries => write!(f, "error retries"),
            CausalFactor::StateSerialization => write!(f, "state serialization overhead"),
            CausalFactor::MemoryPressure => write!(f, "memory pressure"),
            CausalFactor::InputValidation => write!(f, "input validation overhead"),
            CausalFactor::OutputProcessing => write!(f, "output processing overhead"),
            CausalFactor::UpstreamFailure(node) => write!(f, "upstream failure from '{}'", node),
            CausalFactor::ConfigurationIssue(issue) => write!(f, "configuration issue: {}", issue),
            CausalFactor::Custom(desc) => write!(f, "{}", desc),
        }
    }
}

/// An individual cause contributing to an effect
///
/// Represents one factor that contributed to an observable outcome,
/// with a quantified contribution and supporting evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cause {
    /// The causal factor
    pub factor: CausalFactor,
    /// How much this factor contributed to the effect (0.0-1.0)
    /// Sum of all contributions in a CausalChain should be 1.0
    pub contribution: f64,
    /// Supporting evidence for this cause
    pub evidence: String,
    /// Confidence in this causal attribution (0.0-1.0)
    pub confidence: f64,
    /// Suggested remediation for this cause
    pub remediation: Option<String>,
    /// Additional details about this cause
    pub details: HashMap<String, serde_json::Value>,
}

impl Cause {
    /// Create a new cause
    #[must_use]
    pub fn new(factor: CausalFactor, contribution: f64, evidence: impl Into<String>) -> Self {
        Self {
            factor,
            contribution: contribution.clamp(0.0, 1.0),
            evidence: evidence.into(),
            confidence: 1.0,
            remediation: None,
            details: HashMap::new(),
        }
    }

    /// Set the confidence level
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set a suggested remediation
    #[must_use]
    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    /// Add a detail key-value pair
    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }

    /// Get a human-readable description of this cause
    #[must_use]
    pub fn description(&self) -> String {
        let mut desc = format!(
            "{} ({:.1}% contribution): {}",
            self.factor,
            self.contribution * 100.0,
            self.evidence
        );
        if let Some(ref rem) = self.remediation {
            desc.push_str(&format!(" [Remediation: {}]", rem));
        }
        desc
    }
}

/// A chain linking an effect to its causes
///
/// Represents the complete causal analysis of an observable outcome,
/// with all contributing factors and their relative weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalChain {
    /// The effect being analyzed
    pub effect: Effect,
    /// All causes contributing to this effect
    pub causes: Vec<Cause>,
    /// Summary description of the causal chain
    pub summary: String,
    /// Overall confidence in this analysis (0.0-1.0)
    pub confidence: f64,
    /// Additional metadata about the analysis
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CausalChain {
    /// Create a new causal chain
    #[must_use]
    pub fn new(effect: Effect) -> Self {
        Self {
            effect,
            causes: Vec::new(),
            summary: String::new(),
            confidence: 1.0,
            metadata: HashMap::new(),
        }
    }

    /// Add a cause to the chain
    #[must_use]
    pub fn with_cause(mut self, cause: Cause) -> Self {
        self.causes.push(cause);
        self
    }

    /// Add multiple causes at once
    #[must_use]
    pub fn with_causes(mut self, causes: impl IntoIterator<Item = Cause>) -> Self {
        self.causes.extend(causes);
        self
    }

    /// Set the summary
    #[must_use]
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// Set the confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get the primary cause (highest contribution)
    #[must_use]
    pub fn primary_cause(&self) -> Option<&Cause> {
        self.causes.iter().max_by(|a, b| {
            a.contribution
                .partial_cmp(&b.contribution)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Get causes above a contribution threshold
    #[must_use]
    pub fn significant_causes(&self, threshold: f64) -> Vec<&Cause> {
        self.causes
            .iter()
            .filter(|c| c.contribution >= threshold)
            .collect()
    }

    /// Get causes sorted by contribution (highest first)
    #[must_use]
    pub fn causes_by_contribution(&self) -> Vec<&Cause> {
        let mut causes: Vec<_> = self.causes.iter().collect();
        causes.sort_by(|a, b| {
            b.contribution
                .partial_cmp(&a.contribution)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        causes
    }

    /// Get total contribution (should be close to 1.0 if well-formed)
    #[must_use]
    pub fn total_contribution(&self) -> f64 {
        self.causes.iter().map(|c| c.contribution).sum()
    }

    /// Check if the chain is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.causes.is_empty()
    }

    /// Get the number of causes
    #[must_use]
    pub fn len(&self) -> usize {
        self.causes.len()
    }

    /// Normalize contributions to sum to 1.0
    pub fn normalize(&mut self) {
        let total = self.total_contribution();
        if total > 0.0 && (total - 1.0).abs() > 0.001 {
            for cause in &mut self.causes {
                cause.contribution /= total;
            }
        }
    }

    /// Get a human-readable report of this causal chain
    #[must_use]
    pub fn report(&self) -> String {
        let mut lines = vec![format!("Causal Analysis: {}", self.effect)];

        if !self.summary.is_empty() {
            lines.push(format!("Summary: {}", self.summary));
        }

        lines.push(format!("Confidence: {:.1}%", self.confidence * 100.0));
        lines.push(String::new());
        lines.push("Causes:".to_string());

        for (i, cause) in self.causes_by_contribution().iter().enumerate() {
            lines.push(format!("  {}. {}", i + 1, cause.description()));
        }

        lines.join("\n")
    }

    /// Convert to JSON for AI consumption
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

/// Configuration for causal analysis
///
/// Controls thresholds and behavior of the causal analysis algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalAnalysisConfig {
    /// Minimum contribution to report a cause (0.0-1.0)
    pub min_contribution: f64,
    /// Minimum confidence to report a cause (0.0-1.0)
    pub min_confidence: f64,
    /// Maximum number of causes to report
    pub max_causes: usize,
    /// Thresholds for detecting issues
    pub thresholds: CausalThresholds,
}

impl Default for CausalAnalysisConfig {
    fn default() -> Self {
        Self {
            min_contribution: 0.05, // 5% minimum
            min_confidence: 0.5,    // 50% confidence minimum
            max_causes: 10,         // At most 10 causes
            thresholds: CausalThresholds::default(),
        }
    }
}

impl CausalAnalysisConfig {
    /// Create a detailed configuration (lower thresholds, more causes)
    #[must_use]
    pub fn detailed() -> Self {
        Self {
            min_contribution: 0.01,
            min_confidence: 0.3,
            max_causes: 20,
            thresholds: CausalThresholds::default(),
        }
    }

    /// Create a summary configuration (higher thresholds, fewer causes)
    #[must_use]
    pub fn summary() -> Self {
        Self {
            min_contribution: 0.15,
            min_confidence: 0.7,
            max_causes: 5,
            thresholds: CausalThresholds::default(),
        }
    }
}

/// Thresholds for detecting causal factors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalThresholds {
    /// Token count considered "large context" (for latency)
    pub large_context_tokens: u64,
    /// Number of tool calls considered "many"
    pub many_tool_calls: usize,
    /// Node duration percentile considered "slow" (relative to average)
    pub slow_node_ratio: f64,
    /// Node execution count considered "repeated" (potential loop)
    pub repeated_execution_count: usize,
    /// Error rate considered "high"
    pub high_error_rate: f64,
    /// Latency considered "high" (ms)
    pub high_latency_ms: u64,
}

impl Default for CausalThresholds {
    fn default() -> Self {
        Self {
            large_context_tokens: 8000,
            many_tool_calls: 5,
            slow_node_ratio: 2.0,        // 2x average is slow
            repeated_execution_count: 5, // 5+ executions is repeated
            high_error_rate: 0.1,        // 10% error rate
            high_latency_ms: 10_000,     // 10 seconds
        }
    }
}

/// Causal analyzer for execution traces
///
/// Analyzes execution traces to determine root causes of various effects.
pub struct CausalAnalyzer {
    config: CausalAnalysisConfig,
}

impl Default for CausalAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CausalAnalyzer {
    /// Create a new analyzer with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: CausalAnalysisConfig::default(),
        }
    }

    /// Create an analyzer with custom configuration
    #[must_use]
    pub fn with_config(config: CausalAnalysisConfig) -> Self {
        Self { config }
    }

    /// Analyze a trace for a specific effect
    #[must_use]
    pub fn analyze(
        &self,
        trace: &crate::introspection::ExecutionTrace,
        effect: Effect,
    ) -> CausalChain {
        match effect {
            Effect::HighLatency => self.analyze_high_latency(trace),
            Effect::SlowNode(ref node) => self.analyze_slow_node(trace, node),
            Effect::HighTokenUsage => self.analyze_high_token_usage(trace),
            Effect::ExecutionFailure => self.analyze_execution_failure(trace),
            Effect::HighRetryRate => self.analyze_high_retry_rate(trace),
            Effect::NodeFailure(ref node) => self.analyze_node_failure(trace, node),
            Effect::InfiniteLoop => self.analyze_infinite_loop(trace),
            Effect::ResourceExhaustion => self.analyze_resource_exhaustion(trace),
            Effect::Custom(ref desc) => self.analyze_custom(trace, desc),
        }
    }

    /// Automatically detect effects and analyze them all
    #[must_use]
    pub fn auto_analyze(&self, trace: &crate::introspection::ExecutionTrace) -> Vec<CausalChain> {
        let mut chains = Vec::new();

        // Check for high latency
        if trace.total_duration_ms > self.config.thresholds.high_latency_ms {
            chains.push(self.analyze_high_latency(trace));
        }

        // Check for execution failure
        if !trace.completed || !trace.errors.is_empty() {
            chains.push(self.analyze_execution_failure(trace));
        }

        // Check for potential loops
        let mut node_counts: HashMap<&str, usize> = HashMap::new();
        for exec in &trace.nodes_executed {
            *node_counts.entry(&exec.node).or_insert(0) += 1;
        }
        for (node, count) in &node_counts {
            if *count >= self.config.thresholds.repeated_execution_count {
                chains.push(self.analyze(trace, Effect::InfiniteLoop));
                break;
            }
            // Also detect slow nodes
            let node_total_time: u64 = trace
                .nodes_executed
                .iter()
                .filter(|e| &e.node == node)
                .map(|e| e.duration_ms)
                .sum();
            let avg_time = if trace.nodes_executed.is_empty() {
                0.0
            } else {
                trace.total_duration_ms as f64 / trace.nodes_executed.len() as f64
            };
            if node_total_time as f64
                > avg_time * self.config.thresholds.slow_node_ratio * (*count as f64)
            {
                chains.push(self.analyze_slow_node(trace, node));
            }
        }

        chains
    }

    fn analyze_high_latency(&self, trace: &crate::introspection::ExecutionTrace) -> CausalChain {
        let chain = CausalChain::new(Effect::HighLatency);
        let mut causes = Vec::new();
        let total_ms = trace.total_duration_ms as f64;

        if total_ms == 0.0 {
            return chain.with_summary("No execution data to analyze");
        }

        // Analyze node contributions
        let mut node_times: HashMap<&str, (u64, usize)> = HashMap::new();
        for exec in &trace.nodes_executed {
            let entry = node_times.entry(&exec.node).or_insert((0, 0));
            entry.0 += exec.duration_ms;
            entry.1 += 1;
        }

        // Find bottleneck nodes
        let avg_time = total_ms / node_times.len().max(1) as f64;
        for (node, (time_ms, count)) in &node_times {
            let contribution = *time_ms as f64 / total_ms;
            if contribution >= self.config.min_contribution {
                let factor = if *count > 1 {
                    CausalFactor::RepeatedExecution((*node).to_string())
                } else if (*time_ms as f64) > avg_time * self.config.thresholds.slow_node_ratio {
                    CausalFactor::NodeBottleneck((*node).to_string())
                } else {
                    continue;
                };

                let evidence = format!(
                    "Node '{}' took {}ms ({} execution{}), {:.1}% of total time",
                    node,
                    time_ms,
                    count,
                    if *count > 1 { "s" } else { "" },
                    contribution * 100.0
                );

                let mut cause = Cause::new(factor, contribution, evidence);
                if *count > 1 {
                    cause = cause.with_remediation(format!(
                        "Consider caching results for '{}' or reducing loop iterations",
                        node
                    ));
                } else {
                    cause = cause.with_remediation(format!(
                        "Consider optimizing '{}' or using a faster model",
                        node
                    ));
                }
                causes.push(cause);
            }
        }

        // Analyze token-related latency
        if trace.total_tokens > self.config.thresholds.large_context_tokens {
            let token_factor = (trace.total_tokens as f64
                / self.config.thresholds.large_context_tokens as f64)
                .min(1.0);
            let contribution = token_factor * 0.3; // Token overhead typically contributes ~30% max
            if contribution >= self.config.min_contribution {
                causes.push(
                    Cause::new(
                        CausalFactor::LargeContext,
                        contribution,
                        format!(
                            "{} tokens used (threshold: {})",
                            trace.total_tokens, self.config.thresholds.large_context_tokens
                        ),
                    )
                    .with_remediation("Consider summarizing context or using shorter prompts"),
                );
            }
        }

        // Analyze tool calls
        let total_tool_calls: usize = trace
            .nodes_executed
            .iter()
            .map(|e| e.tools_called.len())
            .sum();
        if total_tool_calls >= self.config.thresholds.many_tool_calls {
            let contribution = (total_tool_calls as f64 / 20.0).min(0.4); // Cap at 40%
            if contribution >= self.config.min_contribution {
                causes.push(
                    Cause::new(
                        CausalFactor::ManyToolCalls,
                        contribution,
                        format!(
                            "{} tool calls made (threshold: {})",
                            total_tool_calls, self.config.thresholds.many_tool_calls
                        ),
                    )
                    .with_remediation("Consider batching tool calls or reducing unnecessary calls"),
                );
            }
        }

        // Sort and limit causes
        causes.sort_by(|a, b| {
            b.contribution
                .partial_cmp(&a.contribution)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        causes.truncate(self.config.max_causes);

        // Normalize contributions
        let total_contribution: f64 = causes.iter().map(|c| c.contribution).sum();
        if total_contribution > 0.0 {
            for cause in &mut causes {
                cause.contribution /= total_contribution;
            }
        }

        let cause_count = causes.len();
        let summary = if cause_count == 0 {
            "No significant causes identified for high latency".to_string()
        } else {
            format!(
                "Execution took {}ms. {} significant cause(s) identified.",
                trace.total_duration_ms, cause_count
            )
        };

        chain
            .with_causes(causes)
            .with_summary(summary)
            .with_metadata(
                "total_duration_ms".to_string(),
                trace.total_duration_ms.into(),
            )
    }

    fn analyze_slow_node(
        &self,
        trace: &crate::introspection::ExecutionTrace,
        node_name: &str,
    ) -> CausalChain {
        let chain = CausalChain::new(Effect::SlowNode(node_name.to_string()));
        let mut causes = Vec::new();

        let node_executions: Vec<_> = trace
            .nodes_executed
            .iter()
            .filter(|e| e.node == node_name)
            .collect();

        if node_executions.is_empty() {
            return chain.with_summary(format!("Node '{}' not found in trace", node_name));
        }

        let total_node_time: u64 = node_executions.iter().map(|e| e.duration_ms).sum();
        let total_node_tokens: u64 = node_executions.iter().map(|e| e.tokens_used).sum();
        let execution_count = node_executions.len();

        // Multiple executions?
        if execution_count > 1 {
            let contribution = 0.5; // Repeated execution is typically a major factor
            causes.push(
                Cause::new(
                    CausalFactor::RepeatedExecution(node_name.to_string()),
                    contribution,
                    format!(
                        "Node executed {} times, total {}ms",
                        execution_count, total_node_time
                    ),
                )
                .with_remediation("Add caching or break the loop earlier"),
            );
        }

        // High token usage in this node?
        if total_node_tokens > self.config.thresholds.large_context_tokens / 2 {
            let contribution = 0.3;
            causes.push(
                Cause::new(
                    CausalFactor::LargeContext,
                    contribution,
                    format!(
                        "Node used {} tokens (high for single node)",
                        total_node_tokens
                    ),
                )
                .with_remediation("Reduce prompt/context size for this node"),
            );
        }

        // Tool calls in this node?
        let tool_calls: usize = node_executions.iter().map(|e| e.tools_called.len()).sum();
        if tool_calls > 0 {
            let contribution = (tool_calls as f64 / 10.0).min(0.3);
            if contribution >= self.config.min_contribution {
                causes.push(
                    Cause::new(
                        CausalFactor::ManyToolCalls,
                        contribution,
                        format!("Node made {} tool call(s)", tool_calls),
                    )
                    .with_remediation("Optimize or batch tool calls"),
                );
            }
        }

        // Default cause if nothing specific found
        if causes.is_empty() {
            causes.push(
                Cause::new(
                    CausalFactor::ModelInference,
                    1.0,
                    format!("Model inference took {}ms", total_node_time),
                )
                .with_remediation("Consider using a faster model for this task"),
            );
        }

        // Normalize
        let total: f64 = causes.iter().map(|c| c.contribution).sum();
        if total > 0.0 {
            for cause in &mut causes {
                cause.contribution /= total;
            }
        }

        chain.with_causes(causes).with_summary(format!(
            "Node '{}' took {}ms across {} execution(s)",
            node_name, total_node_time, execution_count
        ))
    }

    fn analyze_high_token_usage(
        &self,
        trace: &crate::introspection::ExecutionTrace,
    ) -> CausalChain {
        let chain = CausalChain::new(Effect::HighTokenUsage);
        let mut causes = Vec::new();

        if trace.total_tokens == 0 {
            return chain.with_summary("No token data available");
        }

        // Find nodes with highest token usage
        let mut node_tokens: HashMap<&str, u64> = HashMap::new();
        for exec in &trace.nodes_executed {
            *node_tokens.entry(&exec.node).or_insert(0) += exec.tokens_used;
        }

        let total_tokens = trace.total_tokens as f64;
        for (node, tokens) in &node_tokens {
            let contribution = *tokens as f64 / total_tokens;
            if contribution >= self.config.min_contribution {
                causes.push(
                    Cause::new(
                        CausalFactor::NodeBottleneck((*node).to_string()),
                        contribution,
                        format!(
                            "Node '{}' used {} tokens ({:.1}% of total)",
                            node,
                            tokens,
                            contribution * 100.0
                        ),
                    )
                    .with_remediation(format!(
                        "Optimize prompts in '{}' or use a more efficient model",
                        node
                    )),
                );
            }
        }

        causes.sort_by(|a, b| {
            b.contribution
                .partial_cmp(&a.contribution)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        causes.truncate(self.config.max_causes);

        let cause_count = causes.len();
        chain.with_causes(causes).with_summary(format!(
            "Total token usage: {}. {} significant consumer(s) identified.",
            trace.total_tokens, cause_count
        ))
    }

    fn analyze_execution_failure(
        &self,
        trace: &crate::introspection::ExecutionTrace,
    ) -> CausalChain {
        let chain = CausalChain::new(Effect::ExecutionFailure);
        let mut causes = Vec::new();

        if trace.errors.is_empty() && trace.completed {
            return chain.with_summary("No failures detected");
        }

        // Analyze errors
        let error_count = trace.errors.len();
        if error_count > 0 {
            let mut error_nodes: HashMap<&str, Vec<&str>> = HashMap::new();
            for error in &trace.errors {
                error_nodes
                    .entry(&error.node)
                    .or_default()
                    .push(&error.message);
            }

            for (node, messages) in &error_nodes {
                let contribution = messages.len() as f64 / error_count as f64;
                causes.push(
                    Cause::new(
                        CausalFactor::NodeBottleneck((*node).to_string()),
                        contribution,
                        format!(
                            "Node '{}' had {} error(s): {}",
                            node,
                            messages.len(),
                            messages.first().unwrap_or(&"unknown")
                        ),
                    )
                    .with_remediation(format!(
                        "Add error handling in '{}' or fix the root cause",
                        node
                    )),
                );
            }
        }

        // Check for incomplete executions
        let failed_nodes: Vec<_> = trace.nodes_executed.iter().filter(|e| !e.success).collect();

        for exec in failed_nodes {
            let already_counted = causes
                .iter()
                .any(|c| matches!(&c.factor, CausalFactor::NodeBottleneck(n) if n == &exec.node));
            if !already_counted {
                causes.push(
                    Cause::new(
                        CausalFactor::NodeBottleneck(exec.node.clone()),
                        0.5,
                        exec.error_message
                            .clone()
                            .unwrap_or_else(|| "Execution failed".to_string()),
                    )
                    .with_remediation(format!("Investigate failure in '{}'", exec.node)),
                );
            }
        }

        // Normalize
        let total: f64 = causes.iter().map(|c| c.contribution).sum();
        if total > 0.0 {
            for cause in &mut causes {
                cause.contribution /= total;
            }
        }

        let cause_count = causes.len();
        chain.with_causes(causes).with_summary(format!(
            "Execution failed with {} error(s). {} cause(s) identified.",
            error_count, cause_count
        ))
    }

    fn analyze_high_retry_rate(&self, trace: &crate::introspection::ExecutionTrace) -> CausalChain {
        let chain = CausalChain::new(Effect::HighRetryRate);
        let mut causes = Vec::new();

        // Count executions per node to detect retries
        let mut node_counts: HashMap<&str, usize> = HashMap::new();
        for exec in &trace.nodes_executed {
            *node_counts.entry(&exec.node).or_insert(0) += 1;
        }

        // Nodes executed more than once might be retries
        for (node, count) in &node_counts {
            if *count > 1 {
                let contribution = (*count as f64 - 1.0) / trace.nodes_executed.len().max(1) as f64;
                if contribution >= self.config.min_contribution {
                    causes.push(
                        Cause::new(
                            CausalFactor::RepeatedExecution((*node).to_string()),
                            contribution,
                            format!(
                                "Node '{}' executed {} times (possibly retried)",
                                node, count
                            ),
                        )
                        .with_remediation(format!("Investigate why '{}' needs retries", node)),
                    );
                }
            }
        }

        // Errors might indicate need for retries
        if !trace.errors.is_empty() {
            let error_contribution =
                trace.errors.len() as f64 / trace.nodes_executed.len().max(1) as f64;
            causes.push(
                Cause::new(
                    CausalFactor::ErrorRetries,
                    error_contribution.min(0.5),
                    format!("{} errors encountered during execution", trace.errors.len()),
                )
                .with_remediation("Address root causes of errors to reduce retry needs"),
            );
        }

        // Normalize
        let total: f64 = causes.iter().map(|c| c.contribution).sum();
        if total > 0.0 {
            for cause in &mut causes {
                cause.contribution /= total;
            }
        }

        let cause_count = causes.len();
        chain.with_causes(causes).with_summary(format!(
            "{} node execution(s) total, {} cause(s) of retry behavior identified",
            trace.nodes_executed.len(),
            cause_count
        ))
    }

    fn analyze_node_failure(
        &self,
        trace: &crate::introspection::ExecutionTrace,
        node_name: &str,
    ) -> CausalChain {
        let chain = CausalChain::new(Effect::NodeFailure(node_name.to_string()));
        let mut causes = Vec::new();

        // Find errors for this node
        let node_errors: Vec<_> = trace
            .errors
            .iter()
            .filter(|e| e.node == node_name)
            .collect();

        // Find failed executions for this node
        let failed_executions: Vec<_> = trace
            .nodes_executed
            .iter()
            .filter(|e| e.node == node_name && !e.success)
            .collect();

        if node_errors.is_empty() && failed_executions.is_empty() {
            return chain.with_summary(format!("No failures found for node '{}'", node_name));
        }

        // Analyze error messages for patterns
        for error in &node_errors {
            let factor = if error.message.contains("timeout") || error.message.contains("Timeout") {
                CausalFactor::NetworkLatency
            } else if error.message.contains("token") || error.message.contains("context") {
                CausalFactor::LargeContext
            } else if error.message.contains("upstream") || error.message.contains("dependency") {
                CausalFactor::UpstreamFailure(node_name.to_string())
            } else {
                CausalFactor::Custom(error.message.clone())
            };

            causes.push(
                Cause::new(factor, 1.0 / node_errors.len() as f64, &error.message)
                    .with_remediation("Fix the error condition"),
            );
        }

        // Add failed execution info
        for exec in &failed_executions {
            if let Some(ref msg) = exec.error_message {
                let already_counted = causes.iter().any(|c| c.evidence.contains(msg));
                if !already_counted {
                    causes.push(
                        Cause::new(CausalFactor::Custom(msg.clone()), 0.5, msg)
                            .with_remediation("Investigate and fix the failure"),
                    );
                }
            }
        }

        // Normalize
        let total: f64 = causes.iter().map(|c| c.contribution).sum();
        if total > 0.0 {
            for cause in &mut causes {
                cause.contribution /= total;
            }
        }

        chain.with_causes(causes).with_summary(format!(
            "Node '{}' failed with {} error(s)",
            node_name,
            node_errors.len() + failed_executions.len()
        ))
    }

    fn analyze_infinite_loop(&self, trace: &crate::introspection::ExecutionTrace) -> CausalChain {
        let chain = CausalChain::new(Effect::InfiniteLoop);
        let mut causes = Vec::new();

        // Count executions per node
        let mut node_counts: HashMap<&str, usize> = HashMap::new();
        for exec in &trace.nodes_executed {
            *node_counts.entry(&exec.node).or_insert(0) += 1;
        }

        // Find nodes with high execution counts
        let total_executions = trace.nodes_executed.len();
        for (node, count) in &node_counts {
            if *count >= self.config.thresholds.repeated_execution_count {
                let contribution = *count as f64 / total_executions as f64;
                causes.push(
                    Cause::new(
                        CausalFactor::RepeatedExecution((*node).to_string()),
                        contribution,
                        format!("Node '{}' executed {} times", node, count),
                    )
                    .with_remediation(format!(
                        "Add loop termination condition or iteration limit for '{}'",
                        node
                    )),
                );
            }
        }

        // Normalize
        let total: f64 = causes.iter().map(|c| c.contribution).sum();
        if total > 0.0 {
            for cause in &mut causes {
                cause.contribution /= total;
            }
        }

        let cause_count = causes.len();
        chain.with_causes(causes).with_summary(format!(
            "Potential loop detected: {} total executions, {} repeating node(s)",
            total_executions, cause_count
        ))
    }

    fn analyze_resource_exhaustion(
        &self,
        trace: &crate::introspection::ExecutionTrace,
    ) -> CausalChain {
        let chain = CausalChain::new(Effect::ResourceExhaustion);
        let mut causes = Vec::new();

        // Token exhaustion
        if trace.total_tokens > self.config.thresholds.large_context_tokens * 2 {
            causes.push(
                Cause::new(
                    CausalFactor::LargeContext,
                    0.5,
                    format!(
                        "Total tokens {} exceeds 2x threshold ({})",
                        trace.total_tokens, self.config.thresholds.large_context_tokens
                    ),
                )
                .with_remediation("Implement context summarization or pagination"),
            );
        }

        // Time exhaustion
        if trace.total_duration_ms > self.config.thresholds.high_latency_ms * 2 {
            causes.push(
                Cause::new(
                    CausalFactor::ComplexComputation,
                    0.5,
                    format!(
                        "Execution time {}ms exceeds 2x threshold ({}ms)",
                        trace.total_duration_ms, self.config.thresholds.high_latency_ms
                    ),
                )
                .with_remediation("Optimize computation or increase timeouts"),
            );
        }

        // Normalize
        let total: f64 = causes.iter().map(|c| c.contribution).sum();
        if total > 0.0 {
            for cause in &mut causes {
                cause.contribution /= total;
            }
        }

        chain.with_causes(causes).with_summary(format!(
            "Resource exhaustion analysis: {}ms duration, {} tokens",
            trace.total_duration_ms, trace.total_tokens
        ))
    }

    fn analyze_custom(
        &self,
        trace: &crate::introspection::ExecutionTrace,
        description: &str,
    ) -> CausalChain {
        // For custom effects, do a general analysis
        let chain = CausalChain::new(Effect::Custom(description.to_string()));

        // Include any errors as causes
        let mut causes = Vec::new();
        if !trace.errors.is_empty() {
            for error in &trace.errors {
                causes.push(Cause::new(
                    CausalFactor::Custom(error.message.clone()),
                    1.0 / trace.errors.len() as f64,
                    format!("Error in '{}': {}", error.node, error.message),
                ));
            }
        }

        let cause_count = causes.len();
        chain.with_causes(causes).with_summary(format!(
            "Custom analysis for '{}': {} potential cause(s)",
            description, cause_count
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ErrorTrace, ExecutionTrace, ExecutionTraceBuilder, NodeExecution};

    fn create_test_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .thread_id("test-thread")
            .execution_id("test-exec")
            .add_node_execution(NodeExecution::new("reasoning", 500).with_tokens(3000))
            .add_node_execution(
                NodeExecution::new("tool_call", 1500)
                    .with_tokens(1000)
                    .with_tools(vec!["search".to_string(), "calculate".to_string()]),
            )
            .add_node_execution(NodeExecution::new("output", 200).with_tokens(500))
            .total_duration_ms(2200)
            .total_tokens(4500)
            .completed(true)
            .build()
    }

    fn create_slow_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("slow_node", 15000).with_tokens(10000))
            .add_node_execution(NodeExecution::new("fast_node", 100).with_tokens(100))
            .total_duration_ms(15100)
            .total_tokens(10100)
            .completed(true)
            .build()
    }

    fn create_loop_trace() -> ExecutionTrace {
        let mut builder = ExecutionTraceBuilder::new();
        for i in 0..10 {
            builder =
                builder.add_node_execution(NodeExecution::new("loop_node", 100).with_index(i));
        }
        builder
            .total_duration_ms(1000)
            .total_tokens(1000)
            .completed(true)
            .build()
    }

    fn create_error_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(
                NodeExecution::new("failing_node", 500).with_error("Connection timeout"),
            )
            .add_error(ErrorTrace::new("failing_node", "Connection timeout"))
            .total_duration_ms(500)
            .completed(false)
            .build()
    }

    #[test]
    fn test_effect_display() {
        assert_eq!(Effect::HighLatency.to_string(), "high total latency");
        assert_eq!(
            Effect::SlowNode("test".into()).to_string(),
            "slow execution in 'test'"
        );
        assert_eq!(Effect::ExecutionFailure.to_string(), "execution failure");
    }

    #[test]
    fn test_causal_factor_display() {
        assert_eq!(CausalFactor::LargeContext.to_string(), "large context size");
        assert_eq!(
            CausalFactor::NodeBottleneck("test".into()).to_string(),
            "bottleneck in 'test'"
        );
    }

    #[test]
    fn test_cause_creation() {
        let cause = Cause::new(CausalFactor::LargeContext, 0.5, "Test evidence")
            .with_confidence(0.8)
            .with_remediation("Use smaller context");

        assert_eq!(cause.contribution, 0.5);
        assert_eq!(cause.confidence, 0.8);
        assert_eq!(cause.remediation.as_deref(), Some("Use smaller context"));
    }

    #[test]
    fn test_cause_contribution_clamping() {
        let cause = Cause::new(CausalFactor::LargeContext, 1.5, "Test");
        assert_eq!(cause.contribution, 1.0);

        let cause = Cause::new(CausalFactor::LargeContext, -0.5, "Test");
        assert_eq!(cause.contribution, 0.0);
    }

    #[test]
    fn test_causal_chain_creation() {
        let chain = CausalChain::new(Effect::HighLatency)
            .with_cause(Cause::new(CausalFactor::LargeContext, 0.6, "Large context"))
            .with_cause(Cause::new(CausalFactor::ManyToolCalls, 0.4, "Many tools"))
            .with_summary("Test summary");

        assert_eq!(chain.len(), 2);
        assert_eq!(chain.total_contribution(), 1.0);
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_causal_chain_primary_cause() {
        let chain = CausalChain::new(Effect::HighLatency)
            .with_cause(Cause::new(CausalFactor::LargeContext, 0.3, ""))
            .with_cause(Cause::new(CausalFactor::ManyToolCalls, 0.7, ""));

        let primary = chain.primary_cause().unwrap();
        assert_eq!(primary.factor, CausalFactor::ManyToolCalls);
    }

    #[test]
    fn test_causal_chain_significant_causes() {
        let chain = CausalChain::new(Effect::HighLatency)
            .with_cause(Cause::new(CausalFactor::LargeContext, 0.05, ""))
            .with_cause(Cause::new(CausalFactor::ManyToolCalls, 0.3, ""))
            .with_cause(Cause::new(CausalFactor::NetworkLatency, 0.65, ""));

        let significant = chain.significant_causes(0.2);
        assert_eq!(significant.len(), 2);
    }

    #[test]
    fn test_causal_chain_normalize() {
        let mut chain = CausalChain::new(Effect::HighLatency)
            .with_cause(Cause::new(CausalFactor::LargeContext, 0.5, ""))
            .with_cause(Cause::new(CausalFactor::ManyToolCalls, 0.3, ""));

        // Total is 0.8, should normalize to 1.0
        chain.normalize();
        let total = chain.total_contribution();
        assert!((total - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_causal_chain_report() {
        let chain = CausalChain::new(Effect::HighLatency)
            .with_cause(Cause::new(
                CausalFactor::LargeContext,
                0.6,
                "Large context used",
            ))
            .with_summary("Test analysis");

        let report = chain.report();
        assert!(report.contains("high total latency"));
        assert!(report.contains("Test analysis"));
        assert!(report.contains("Large context used"));
    }

    #[test]
    fn test_causal_chain_json_roundtrip() {
        let chain = CausalChain::new(Effect::HighLatency)
            .with_cause(Cause::new(CausalFactor::LargeContext, 0.5, "Test"))
            .with_summary("JSON test");

        let json = chain.to_json().unwrap();
        let parsed = CausalChain::from_json(&json).unwrap();

        assert_eq!(parsed.len(), chain.len());
        assert_eq!(parsed.summary, chain.summary);
    }

    #[test]
    fn test_analyzer_high_latency() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_slow_trace();

        let chain = analyzer.analyze(&trace, Effect::HighLatency);

        assert!(!chain.is_empty());
        assert!(chain.summary.contains("15100ms"));
    }

    #[test]
    fn test_analyzer_slow_node() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_slow_trace();

        let chain = analyzer.analyze(&trace, Effect::SlowNode("slow_node".into()));

        assert!(!chain.is_empty());
        assert!(chain.summary.contains("slow_node"));
    }

    #[test]
    fn test_analyzer_execution_failure() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_error_trace();

        let chain = analyzer.analyze(&trace, Effect::ExecutionFailure);

        assert!(!chain.is_empty());
        assert!(chain.summary.contains("failed"));
    }

    #[test]
    fn test_analyzer_infinite_loop() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_loop_trace();

        let chain = analyzer.analyze(&trace, Effect::InfiniteLoop);

        assert!(!chain.is_empty());
        // Should identify loop_node as repeated
        let has_loop_cause = chain
            .causes
            .iter()
            .any(|c| matches!(&c.factor, CausalFactor::RepeatedExecution(n) if n == "loop_node"));
        assert!(has_loop_cause);
    }

    #[test]
    fn test_analyzer_high_token_usage() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_slow_trace();

        let chain = analyzer.analyze(&trace, Effect::HighTokenUsage);

        assert!(!chain.is_empty());
        // slow_node uses 10000 tokens, should be identified
        let has_token_cause = chain
            .causes
            .iter()
            .any(|c| matches!(&c.factor, CausalFactor::NodeBottleneck(n) if n == "slow_node"));
        assert!(has_token_cause);
    }

    #[test]
    fn test_analyzer_auto_analyze() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_slow_trace();

        let chains = analyzer.auto_analyze(&trace);

        // Should detect high latency (15100ms > 10000ms threshold)
        assert!(!chains.is_empty());
    }

    #[test]
    fn test_analyzer_with_config() {
        let config = CausalAnalysisConfig {
            min_contribution: 0.01,
            min_confidence: 0.1,
            max_causes: 5,
            thresholds: CausalThresholds {
                high_latency_ms: 1000,
                ..Default::default()
            },
        };
        let analyzer = CausalAnalyzer::with_config(config);
        let trace = create_test_trace();

        let chains = analyzer.auto_analyze(&trace);

        // With 1000ms threshold, 2200ms trace should trigger high latency
        assert!(!chains.is_empty());
    }

    #[test]
    fn test_analyzer_empty_trace() {
        let analyzer = CausalAnalyzer::new();
        let trace = ExecutionTrace::new();

        let chain = analyzer.analyze(&trace, Effect::HighLatency);

        // Should handle empty trace gracefully
        assert!(chain.is_empty() || chain.summary.contains("No"));
    }

    #[test]
    fn test_analyzer_node_failure() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_error_trace();

        let chain = analyzer.analyze(&trace, Effect::NodeFailure("failing_node".into()));

        assert!(!chain.is_empty());
        assert!(chain.summary.contains("failing_node"));
    }

    #[test]
    fn test_analyzer_high_retry_rate() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_loop_trace();

        let chain = analyzer.analyze(&trace, Effect::HighRetryRate);

        // loop_node executed 10 times
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_analyzer_resource_exhaustion() {
        let analyzer = CausalAnalyzer::new();
        let trace = create_slow_trace();

        let chain = analyzer.analyze(&trace, Effect::ResourceExhaustion);

        // 10100 tokens > 8000 * 2 = 16000? No
        // 15100ms > 10000 * 2 = 20000? No
        // So this might be empty or have partial causes
        // That's fine, the analysis should still work
        assert!(chain.summary.contains("Resource exhaustion"));
    }

    #[test]
    fn test_cause_description() {
        let cause = Cause::new(CausalFactor::LargeContext, 0.75, "Used 12000 tokens")
            .with_remediation("Reduce context size");

        let desc = cause.description();
        assert!(desc.contains("75.0%"));
        assert!(desc.contains("12000 tokens"));
        assert!(desc.contains("Reduce context size"));
    }

    #[test]
    fn test_config_presets() {
        let detailed = CausalAnalysisConfig::detailed();
        assert_eq!(detailed.min_contribution, 0.01);
        assert_eq!(detailed.max_causes, 20);

        let summary = CausalAnalysisConfig::summary();
        assert_eq!(summary.min_contribution, 0.15);
        assert_eq!(summary.max_causes, 5);
    }

    #[test]
    fn test_thresholds_default() {
        let thresholds = CausalThresholds::default();
        assert_eq!(thresholds.large_context_tokens, 8000);
        assert_eq!(thresholds.many_tool_calls, 5);
        assert_eq!(thresholds.high_latency_ms, 10_000);
    }
}
