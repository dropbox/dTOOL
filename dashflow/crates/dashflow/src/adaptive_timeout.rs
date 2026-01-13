// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Adaptive Timeout Adjustment
//!
//! This module enables AI agents to learn optimal timeout values from execution
//! history, automatically adjusting node timeouts based on observed performance.
//!
//! # Overview
//!
//! Fixed timeouts are often suboptimal - they may be too short (causing failures)
//! or too long (wasting resources during failures). This module provides
//! mechanisms to:
//!
//! 1. **Collect latency statistics** - Track execution times per node
//! 2. **Calculate optimal timeouts** - Use statistical analysis to determine
//!    timeouts that balance reliability and efficiency
//! 3. **Apply learned timeouts** - Update graph configuration automatically
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   ExecutionTrace                             │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │ collect_latency_stats()                               │  │
//! │  │   - Aggregates duration_ms per node                   │  │
//! │  │   - Calculates min, max, mean, p50, p95, p99          │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                            │                                 │
//! │                            ▼                                 │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │ calculate_optimal_timeouts()                          │  │
//! │  │   - Uses configurable percentile + buffer             │  │
//! │  │   - Respects min/max bounds                           │  │
//! │  │   - Generates TimeoutRecommendation per node          │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   CompiledGraph                              │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │ apply_learned_timeouts()                              │  │
//! │  │   - Updates node timeouts via GraphMutation           │  │
//! │  │   - Logs changes for traceability                     │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::adaptive_timeout::{TimeoutLearner, TimeoutConfig};
//!
//! // After multiple executions, learn optimal timeouts
//! let learner = TimeoutLearner::new();
//! let recommendations = learner.learn_from_traces(&traces, &TimeoutConfig::default());
//!
//! // Review recommendations
//! for rec in &recommendations.recommendations {
//!     println!("Node '{}': {} -> {} (confidence: {:.0}%)",
//!         rec.node,
//!         rec.current_timeout_ms.map(|t| format!("{}ms", t)).unwrap_or("default".to_string()),
//!         rec.recommended_timeout_ms,
//!         rec.confidence * 100.0
//!     );
//! }
//!
//! // Apply recommendations above confidence threshold
//! let mutations = recommendations.to_mutations(0.8);
//! for mutation in mutations {
//!     compiled.apply_mutation(mutation)?;
//! }
//! ```

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::graph_reconfiguration::{GraphMutation, MutationType};
use crate::introspection::{ExecutionTrace, NodeExecution};

// ============================================================================
// Latency Statistics
// ============================================================================

/// Statistics about node execution latency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Node name
    pub node: String,
    /// Number of samples (executions)
    pub sample_count: usize,
    /// Minimum latency in milliseconds
    pub min_ms: u64,
    /// Maximum latency in milliseconds
    pub max_ms: u64,
    /// Mean latency in milliseconds
    pub mean_ms: f64,
    /// Standard deviation in milliseconds
    pub std_dev_ms: f64,
    /// Median (p50) latency in milliseconds
    pub p50_ms: u64,
    /// 95th percentile latency in milliseconds
    pub p95_ms: u64,
    /// 99th percentile latency in milliseconds
    pub p99_ms: u64,
    /// Coefficient of variation (std_dev / mean)
    pub cv: f64,
    /// Whether the latency distribution is stable (low CV)
    pub is_stable: bool,
}

impl LatencyStats {
    /// Calculate latency statistics from execution durations
    #[must_use]
    pub fn from_executions(node: impl Into<String>, executions: &[&NodeExecution]) -> Self {
        let node = node.into();

        if executions.is_empty() {
            return Self {
                node,
                sample_count: 0,
                min_ms: 0,
                max_ms: 0,
                mean_ms: 0.0,
                std_dev_ms: 0.0,
                p50_ms: 0,
                p95_ms: 0,
                p99_ms: 0,
                cv: 0.0,
                is_stable: false,
            };
        }

        let mut durations: Vec<u64> = executions.iter().map(|e| e.duration_ms).collect();
        durations.sort_unstable();

        let sample_count = durations.len();
        let min_ms = durations[0];
        let max_ms = durations[sample_count - 1];

        let sum: u64 = durations.iter().sum();
        let mean_ms = sum as f64 / sample_count as f64;

        // Calculate standard deviation
        let variance: f64 = durations
            .iter()
            .map(|d| (*d as f64 - mean_ms).powi(2))
            .sum::<f64>()
            / sample_count as f64;
        let std_dev_ms = variance.sqrt();

        // Calculate percentiles
        let p50_idx = (sample_count as f64 * 0.50) as usize;
        let p95_idx = ((sample_count as f64 * 0.95) as usize).min(sample_count - 1);
        let p99_idx = ((sample_count as f64 * 0.99) as usize).min(sample_count - 1);

        let p50_ms = durations[p50_idx];
        let p95_ms = durations[p95_idx];
        let p99_ms = durations[p99_idx];

        // Coefficient of variation
        let cv = if mean_ms > 0.0 {
            std_dev_ms / mean_ms
        } else {
            0.0
        };

        // Consider stable if CV < 0.5 (std dev is less than half the mean)
        let is_stable = cv < 0.5;

        Self {
            node,
            sample_count,
            min_ms,
            max_ms,
            mean_ms,
            std_dev_ms,
            p50_ms,
            p95_ms,
            p99_ms,
            cv,
            is_stable,
        }
    }

    /// Calculate latency statistics directly from duration values.
    ///
    /// This is more efficient than `from_executions` when you already have
    /// raw duration values and don't need the full NodeExecution context.
    #[must_use]
    pub fn from_durations(node: impl Into<String>, durations: &[u64]) -> Self {
        let node = node.into();

        if durations.is_empty() {
            return Self {
                node,
                sample_count: 0,
                min_ms: 0,
                max_ms: 0,
                mean_ms: 0.0,
                std_dev_ms: 0.0,
                p50_ms: 0,
                p95_ms: 0,
                p99_ms: 0,
                cv: 0.0,
                is_stable: false,
            };
        }

        let mut sorted: Vec<u64> = durations.to_vec();
        sorted.sort_unstable();

        let sample_count = sorted.len();
        let min_ms = sorted[0];
        let max_ms = sorted[sample_count - 1];

        let sum: u64 = sorted.iter().sum();
        let mean_ms = sum as f64 / sample_count as f64;

        // Calculate standard deviation
        let variance: f64 = sorted
            .iter()
            .map(|d| (*d as f64 - mean_ms).powi(2))
            .sum::<f64>()
            / sample_count as f64;
        let std_dev_ms = variance.sqrt();

        // Calculate percentiles
        let p50_idx = (sample_count as f64 * 0.50) as usize;
        let p95_idx = ((sample_count as f64 * 0.95) as usize).min(sample_count - 1);
        let p99_idx = ((sample_count as f64 * 0.99) as usize).min(sample_count - 1);

        let p50_ms = sorted[p50_idx];
        let p95_ms = sorted[p95_idx];
        let p99_ms = sorted[p99_idx];

        // Coefficient of variation
        let cv = if mean_ms > 0.0 {
            std_dev_ms / mean_ms
        } else {
            0.0
        };

        // Consider stable if CV < 0.5 (std dev is less than half the mean)
        let is_stable = cv < 0.5;

        Self {
            node,
            sample_count,
            min_ms,
            max_ms,
            mean_ms,
            std_dev_ms,
            p50_ms,
            p95_ms,
            p99_ms,
            cv,
            is_stable,
        }
    }

    /// Get a human-readable summary
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "Node '{}': {} samples, mean={:.1}ms, p95={}ms, p99={}ms, CV={:.2} ({})",
            self.node,
            self.sample_count,
            self.mean_ms,
            self.p95_ms,
            self.p99_ms,
            self.cv,
            if self.is_stable { "stable" } else { "unstable" }
        )
    }
}

// ============================================================================
// Timeout Configuration
// ============================================================================

/// Configuration for timeout learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Which percentile to use as the base for timeout (default: p95)
    pub base_percentile: TimeoutPercentile,
    /// Multiplier to apply to the percentile (e.g., 1.5 = 50% buffer)
    pub buffer_multiplier: f64,
    /// Minimum timeout in milliseconds (never go below this)
    pub min_timeout_ms: u64,
    /// Maximum timeout in milliseconds (never go above this)
    pub max_timeout_ms: u64,
    /// Minimum samples required to make a recommendation
    pub min_samples: usize,
    /// Minimum confidence to include in recommendations
    pub min_confidence: f64,
    /// Whether to require stable latency distribution for recommendations
    pub require_stability: bool,
    /// Maximum coefficient of variation allowed for stable classification
    pub max_cv_for_stability: f64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            base_percentile: TimeoutPercentile::P95,
            buffer_multiplier: 1.5,
            min_timeout_ms: 100,     // 100ms minimum
            max_timeout_ms: 300_000, // 5 minutes maximum
            min_samples: 5,          // Need at least 5 samples
            min_confidence: 0.5,
            require_stability: false,
            max_cv_for_stability: 0.5,
        }
    }
}

impl TimeoutConfig {
    /// Create a conservative configuration (higher timeouts, more buffer)
    #[must_use]
    pub fn conservative() -> Self {
        Self {
            base_percentile: TimeoutPercentile::P99,
            buffer_multiplier: 2.0,
            min_timeout_ms: 500,
            max_timeout_ms: 600_000, // 10 minutes
            min_samples: 10,
            min_confidence: 0.7,
            require_stability: false,
            max_cv_for_stability: 0.5,
        }
    }

    /// Create an aggressive configuration (tighter timeouts)
    #[must_use]
    pub fn aggressive() -> Self {
        Self {
            base_percentile: TimeoutPercentile::P95,
            buffer_multiplier: 1.2,
            min_timeout_ms: 50,
            max_timeout_ms: 120_000, // 2 minutes
            min_samples: 3,
            min_confidence: 0.4,
            require_stability: false,
            max_cv_for_stability: 0.7,
        }
    }

    /// Create a strict configuration requiring stable latency
    #[must_use]
    pub fn stable_only() -> Self {
        Self {
            base_percentile: TimeoutPercentile::P95,
            buffer_multiplier: 1.5,
            min_timeout_ms: 100,
            max_timeout_ms: 300_000,
            min_samples: 10,
            min_confidence: 0.8,
            require_stability: true,
            max_cv_for_stability: 0.3,
        }
    }
}

/// Which percentile to use as the base for timeout calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeoutPercentile {
    /// Use 50th percentile (median)
    P50,
    /// Use 95th percentile
    P95,
    /// Use 99th percentile
    P99,
    /// Use maximum observed value
    Max,
    /// Use mean + N standard deviations
    MeanPlusStdDev(u8),
}

impl TimeoutPercentile {
    /// Get the timeout value for this percentile from stats
    #[must_use]
    pub fn value(&self, stats: &LatencyStats) -> u64 {
        match self {
            Self::P50 => stats.p50_ms,
            Self::P95 => stats.p95_ms,
            Self::P99 => stats.p99_ms,
            Self::Max => stats.max_ms,
            Self::MeanPlusStdDev(n) => (stats.mean_ms + (*n as f64 * stats.std_dev_ms)) as u64,
        }
    }
}

// ============================================================================
// Timeout Recommendation
// ============================================================================

/// A recommendation for a node's timeout value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutRecommendation {
    /// Node name
    pub node: String,
    /// Current timeout in milliseconds (if known)
    pub current_timeout_ms: Option<u64>,
    /// Recommended timeout in milliseconds
    pub recommended_timeout_ms: u64,
    /// Confidence in this recommendation (0.0 to 1.0)
    pub confidence: f64,
    /// Reason for this recommendation
    pub reason: String,
    /// Latency statistics this recommendation is based on
    pub stats: LatencyStats,
    /// Whether the recommendation differs significantly from current
    pub is_significant_change: bool,
    /// Expected improvement from applying this recommendation
    pub expected_improvement: Option<String>,
}

impl TimeoutRecommendation {
    /// Create a new timeout recommendation
    #[must_use]
    pub fn new(node: impl Into<String>, recommended_timeout_ms: u64, stats: LatencyStats) -> Self {
        Self {
            node: node.into(),
            current_timeout_ms: None,
            recommended_timeout_ms,
            confidence: 0.5,
            reason: String::new(),
            stats,
            is_significant_change: false,
            expected_improvement: None,
        }
    }

    /// Set the current timeout
    #[must_use]
    pub fn with_current_timeout(mut self, timeout_ms: u64) -> Self {
        self.current_timeout_ms = Some(timeout_ms);
        // Check if significant change (>20% difference)
        let ratio = self.recommended_timeout_ms as f64 / timeout_ms as f64;
        self.is_significant_change = !(0.8..=1.2).contains(&ratio);
        self
    }

    /// Set the confidence level
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the reason
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    /// Set the expected improvement
    #[must_use]
    pub fn with_expected_improvement(mut self, improvement: impl Into<String>) -> Self {
        self.expected_improvement = Some(improvement.into());
        self
    }

    /// Convert to a graph mutation
    #[must_use]
    pub fn to_mutation(&self) -> GraphMutation {
        GraphMutation::new(MutationType::AdjustTimeout {
            node: self.node.clone(),
            timeout: Duration::from_millis(self.recommended_timeout_ms),
        })
        .with_reason(self.reason.clone())
        .with_expected_improvement(
            self.expected_improvement.clone().unwrap_or_else(|| {
                "Optimal timeout based on historical execution data".to_string()
            }),
        )
        .with_confidence(self.confidence)
    }

    /// Get a human-readable description
    #[must_use]
    pub fn description(&self) -> String {
        let current_str = self
            .current_timeout_ms
            .map(|t| format!("{}ms -> ", t))
            .unwrap_or_default();

        format!(
            "Node '{}': {}{}ms (confidence: {:.0}%, {} samples)",
            self.node,
            current_str,
            self.recommended_timeout_ms,
            self.confidence * 100.0,
            self.stats.sample_count
        )
    }
}

// ============================================================================
// Timeout Recommendations (Collection)
// ============================================================================

/// Collection of timeout recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutRecommendations {
    /// Individual recommendations per node
    pub recommendations: Vec<TimeoutRecommendation>,
    /// Nodes that were analyzed but not enough data
    pub insufficient_data_nodes: Vec<String>,
    /// Nodes that were analyzed but too unstable
    pub unstable_nodes: Vec<String>,
    /// Summary of the analysis
    pub summary: String,
    /// Total nodes analyzed
    pub total_nodes_analyzed: usize,
    /// Configuration used for this analysis
    pub config: TimeoutConfig,
}

impl TimeoutRecommendations {
    /// Create new empty recommendations
    #[must_use]
    pub fn new(config: TimeoutConfig) -> Self {
        Self {
            recommendations: Vec::new(),
            insufficient_data_nodes: Vec::new(),
            unstable_nodes: Vec::new(),
            summary: String::new(),
            total_nodes_analyzed: 0,
            config,
        }
    }

    /// Get recommendations above a confidence threshold
    #[must_use]
    pub fn above_confidence(&self, threshold: f64) -> Vec<&TimeoutRecommendation> {
        self.recommendations
            .iter()
            .filter(|r| r.confidence >= threshold)
            .collect()
    }

    /// Get recommendations that represent significant changes
    #[must_use]
    pub fn significant_changes(&self) -> Vec<&TimeoutRecommendation> {
        self.recommendations
            .iter()
            .filter(|r| r.is_significant_change)
            .collect()
    }

    /// Convert recommendations above threshold to mutations
    #[must_use]
    pub fn to_mutations(&self, confidence_threshold: f64) -> Vec<GraphMutation> {
        self.recommendations
            .iter()
            .filter(|r| r.confidence >= confidence_threshold)
            .map(TimeoutRecommendation::to_mutation)
            .collect()
    }

    /// Generate summary text
    pub fn generate_summary(&mut self) {
        let high_conf = self.above_confidence(0.8).len();
        let med_conf = self
            .recommendations
            .iter()
            .filter(|r| r.confidence >= 0.5 && r.confidence < 0.8)
            .count();
        let low_conf = self
            .recommendations
            .iter()
            .filter(|r| r.confidence < 0.5)
            .count();

        self.summary = format!(
            "Analyzed {} node(s). Recommendations: {} high confidence, {} medium, {} low. \
             {} node(s) had insufficient data. {} node(s) had unstable latency.",
            self.total_nodes_analyzed,
            high_conf,
            med_conf,
            low_conf,
            self.insufficient_data_nodes.len(),
            self.unstable_nodes.len()
        );
    }

    /// Check if any recommendations are available
    #[must_use]
    pub fn has_recommendations(&self) -> bool {
        !self.recommendations.is_empty()
    }
}

// ============================================================================
// Timeout Learner
// ============================================================================

/// Learns optimal timeouts from execution traces
#[derive(Debug, Clone, Default)]
pub struct TimeoutLearner {
    /// Accumulated latency data per node
    latency_data: HashMap<String, Vec<u64>>,
}

impl TimeoutLearner {
    /// Create a new timeout learner
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add execution data from a trace
    pub fn add_trace(&mut self, trace: &ExecutionTrace) {
        for exec in &trace.nodes_executed {
            self.latency_data
                .entry(exec.node.clone())
                .or_default()
                .push(exec.duration_ms);
        }
    }

    /// Add multiple traces
    pub fn add_traces(&mut self, traces: &[ExecutionTrace]) {
        for trace in traces {
            self.add_trace(trace);
        }
    }

    /// Get the number of samples for a node
    #[must_use]
    pub fn sample_count(&self, node: &str) -> usize {
        self.latency_data.get(node).map_or(0, Vec::len)
    }

    /// Get all node names with data
    #[must_use]
    pub fn nodes(&self) -> Vec<&str> {
        self.latency_data.keys().map(String::as_str).collect()
    }

    /// Clear all accumulated data
    pub fn clear(&mut self) {
        self.latency_data.clear();
    }

    /// Calculate latency statistics for all nodes
    #[must_use]
    pub fn calculate_all_stats(&self) -> Vec<LatencyStats> {
        self.latency_data
            .iter()
            .map(|(node, durations)| LatencyStats::from_durations(node, durations))
            .collect()
    }

    /// Calculate latency statistics for a specific node
    #[must_use]
    pub fn calculate_stats(&self, node: &str) -> Option<LatencyStats> {
        let durations = self.latency_data.get(node)?;
        Some(LatencyStats::from_durations(node, durations))
    }

    /// Learn optimal timeouts from accumulated data
    #[must_use]
    pub fn learn(&self, config: &TimeoutConfig) -> TimeoutRecommendations {
        let mut recommendations = TimeoutRecommendations::new(config.clone());

        for (node, durations) in &self.latency_data {
            recommendations.total_nodes_analyzed += 1;

            // Check minimum samples
            if durations.len() < config.min_samples {
                recommendations.insufficient_data_nodes.push(node.clone());
                continue;
            }

            // Calculate statistics
            let stats = LatencyStats::from_durations(node, durations);

            // Check stability requirement
            if config.require_stability && stats.cv > config.max_cv_for_stability {
                recommendations.unstable_nodes.push(node.clone());
                continue;
            }

            // Calculate recommended timeout
            let base_value = config.base_percentile.value(&stats);
            let recommended = ((base_value as f64 * config.buffer_multiplier) as u64)
                .max(config.min_timeout_ms)
                .min(config.max_timeout_ms);

            // Calculate confidence based on sample size and stability
            let sample_confidence = (durations.len() as f64 / 20.0).min(1.0); // Max confidence at 20 samples
            let stability_confidence = if stats.is_stable { 1.0 } else { 0.7 };
            let confidence = sample_confidence * stability_confidence;

            // Skip if below minimum confidence
            if confidence < config.min_confidence {
                continue;
            }

            // Create recommendation
            let reason = format!(
                "Based on {} samples: p95={}ms, p99={}ms, CV={:.2}",
                stats.sample_count, stats.p95_ms, stats.p99_ms, stats.cv
            );

            let percentile_label = match config.base_percentile {
                TimeoutPercentile::P50 => "50".to_string(),
                TimeoutPercentile::P95 => "95".to_string(),
                TimeoutPercentile::P99 => "99".to_string(),
                TimeoutPercentile::Max => "max".to_string(),
                TimeoutPercentile::MeanPlusStdDev(n) => format!("mean+{}σ", n),
            };
            let expected_improvement = if stats.is_stable {
                format!(
                    "Timeout set to {:.1}x of p{} ({}ms base)",
                    config.buffer_multiplier,
                    percentile_label,
                    base_value
                )
            } else {
                "Timeout based on observed latency (distribution is variable)".to_string()
            };

            let rec = TimeoutRecommendation::new(node, recommended, stats)
                .with_confidence(confidence)
                .with_reason(reason)
                .with_expected_improvement(expected_improvement);

            recommendations.recommendations.push(rec);
        }

        recommendations.generate_summary();
        recommendations
    }

    /// Learn from a collection of traces (convenience method)
    #[must_use]
    pub fn learn_from_traces(
        traces: &[ExecutionTrace],
        config: &TimeoutConfig,
    ) -> TimeoutRecommendations {
        let mut learner = Self::new();
        learner.add_traces(traces);
        learner.learn(config)
    }
}

// ============================================================================
// Timeout History
// ============================================================================

/// Tracks timeout adjustments over time for learning from outcomes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimeoutHistory {
    /// All timeout adjustments made
    pub adjustments: Vec<TimeoutAdjustmentRecord>,
    /// Map of node name to number of adjustments
    pub adjustment_counts: HashMap<String, usize>,
}

/// Record of a timeout adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutAdjustmentRecord {
    /// Node that was adjusted
    pub node: String,
    /// Previous timeout in milliseconds
    pub previous_timeout_ms: Option<u64>,
    /// New timeout in milliseconds
    pub new_timeout_ms: u64,
    /// Reason for adjustment
    pub reason: String,
    /// Timestamp when adjustment was made
    pub adjusted_at: String,
    /// Outcome of the adjustment (if tracked)
    pub outcome: Option<TimeoutAdjustmentOutcome>,
}

/// Outcome of a timeout adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutAdjustmentOutcome {
    /// Whether the adjustment was beneficial
    pub beneficial: bool,
    /// Change in timeout-related errors
    pub error_rate_delta: f64,
    /// Change in average latency
    pub latency_delta_ms: f64,
    /// Notes about the outcome
    pub notes: Option<String>,
}

impl TimeoutHistory {
    /// Create a new empty history
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an adjustment record
    pub fn add_record(&mut self, record: TimeoutAdjustmentRecord) {
        *self
            .adjustment_counts
            .entry(record.node.clone())
            .or_insert(0) += 1;
        self.adjustments.push(record);
    }

    /// Get adjustments for a specific node
    #[must_use]
    pub fn adjustments_for_node(&self, node: &str) -> Vec<&TimeoutAdjustmentRecord> {
        self.adjustments.iter().filter(|r| r.node == node).collect()
    }

    /// Get beneficial adjustments
    #[must_use]
    pub fn beneficial_adjustments(&self) -> Vec<&TimeoutAdjustmentRecord> {
        self.adjustments
            .iter()
            .filter(|r| r.outcome.as_ref().is_some_and(|o| o.beneficial))
            .collect()
    }

    /// Get the success rate of adjustments
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let with_outcomes: Vec<_> = self
            .adjustments
            .iter()
            .filter(|r| r.outcome.is_some())
            .collect();

        if with_outcomes.is_empty() {
            return 0.0;
        }

        let successful = with_outcomes
            .iter()
            .filter(|r| r.outcome.as_ref().is_some_and(|o| o.beneficial))
            .count();

        successful as f64 / with_outcomes.len() as f64
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_executions(node: &str, durations: &[u64]) -> Vec<NodeExecution> {
        durations
            .iter()
            .enumerate()
            .map(|(i, d)| NodeExecution::new(node, *d).with_index(i))
            .collect()
    }

    #[test]
    fn test_latency_stats_from_executions() {
        let executions = create_test_executions("test_node", &[100, 120, 110, 130, 90]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();

        let stats = LatencyStats::from_executions("test_node", &refs);

        assert_eq!(stats.node, "test_node");
        assert_eq!(stats.sample_count, 5);
        assert_eq!(stats.min_ms, 90);
        assert_eq!(stats.max_ms, 130);
        assert!((stats.mean_ms - 110.0).abs() < 0.1);
    }

    #[test]
    fn test_latency_stats_empty() {
        let executions: Vec<NodeExecution> = Vec::new();
        let refs: Vec<&NodeExecution> = executions.iter().collect();

        let stats = LatencyStats::from_executions("empty", &refs);

        assert_eq!(stats.sample_count, 0);
        assert_eq!(stats.min_ms, 0);
        assert_eq!(stats.max_ms, 0);
        assert!(!stats.is_stable);
    }

    #[test]
    fn test_latency_stats_stability() {
        // Stable distribution (low variance)
        let stable_executions = create_test_executions("stable", &[100, 102, 98, 101, 99]);
        let refs: Vec<&NodeExecution> = stable_executions.iter().collect();
        let stable_stats = LatencyStats::from_executions("stable", &refs);
        assert!(stable_stats.is_stable);
        assert!(stable_stats.cv < 0.5);

        // Unstable distribution (high variance)
        let unstable_executions = create_test_executions("unstable", &[50, 200, 75, 300, 100]);
        let refs2: Vec<&NodeExecution> = unstable_executions.iter().collect();
        let unstable_stats = LatencyStats::from_executions("unstable", &refs2);
        assert!(!unstable_stats.is_stable);
        assert!(unstable_stats.cv >= 0.5);
    }

    #[test]
    fn test_timeout_config_default() {
        let config = TimeoutConfig::default();
        assert_eq!(config.base_percentile, TimeoutPercentile::P95);
        assert_eq!(config.buffer_multiplier, 1.5);
        assert_eq!(config.min_samples, 5);
    }

    #[test]
    fn test_timeout_config_conservative() {
        let config = TimeoutConfig::conservative();
        assert_eq!(config.base_percentile, TimeoutPercentile::P99);
        assert!(config.buffer_multiplier > TimeoutConfig::default().buffer_multiplier);
    }

    #[test]
    fn test_timeout_config_aggressive() {
        let config = TimeoutConfig::aggressive();
        assert!(config.buffer_multiplier < TimeoutConfig::default().buffer_multiplier);
        assert!(config.min_samples < TimeoutConfig::default().min_samples);
    }

    #[test]
    fn test_timeout_percentile_value() {
        let executions = create_test_executions("test", &[100, 200, 300, 400, 500]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();
        let stats = LatencyStats::from_executions("test", &refs);

        assert_eq!(TimeoutPercentile::P50.value(&stats), stats.p50_ms);
        assert_eq!(TimeoutPercentile::P95.value(&stats), stats.p95_ms);
        assert_eq!(TimeoutPercentile::P99.value(&stats), stats.p99_ms);
        assert_eq!(TimeoutPercentile::Max.value(&stats), stats.max_ms);
    }

    #[test]
    fn test_timeout_recommendation_builder() {
        let executions = create_test_executions("test", &[100, 110, 105]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();
        let stats = LatencyStats::from_executions("test", &refs);

        let rec = TimeoutRecommendation::new("test", 200, stats)
            .with_current_timeout(150)
            .with_confidence(0.85)
            .with_reason("Based on p95");

        assert_eq!(rec.node, "test");
        assert_eq!(rec.recommended_timeout_ms, 200);
        assert_eq!(rec.current_timeout_ms, Some(150));
        assert_eq!(rec.confidence, 0.85);
        assert!(rec.is_significant_change); // 200/150 = 1.33 > 1.2
    }

    #[test]
    fn test_timeout_recommendation_confidence_clamping() {
        let executions = create_test_executions("test", &[100]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();
        let stats = LatencyStats::from_executions("test", &refs);

        let rec = TimeoutRecommendation::new("test", 100, stats.clone()).with_confidence(1.5);
        assert_eq!(rec.confidence, 1.0);

        let rec2 = TimeoutRecommendation::new("test", 100, stats).with_confidence(-0.5);
        assert_eq!(rec2.confidence, 0.0);
    }

    #[test]
    fn test_timeout_recommendation_to_mutation() {
        let executions = create_test_executions("api_call", &[100, 110, 105]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();
        let stats = LatencyStats::from_executions("api_call", &refs);

        let rec = TimeoutRecommendation::new("api_call", 200, stats)
            .with_confidence(0.9)
            .with_reason("Based on p95");

        let mutation = rec.to_mutation();

        match &mutation.mutation_type {
            MutationType::AdjustTimeout { node, timeout } => {
                assert_eq!(node, "api_call");
                assert_eq!(*timeout, Duration::from_millis(200));
            }
            _ => panic!("Expected AdjustTimeout mutation"),
        }
        assert_eq!(mutation.confidence, Some(0.9));
    }

    #[test]
    fn test_timeout_learner_add_trace() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("node_a", 100))
            .add_node_execution(NodeExecution::new("node_b", 200))
            .add_node_execution(NodeExecution::new("node_a", 110))
            .build();

        let mut learner = TimeoutLearner::new();
        learner.add_trace(&trace);

        assert_eq!(learner.sample_count("node_a"), 2);
        assert_eq!(learner.sample_count("node_b"), 1);
        assert_eq!(learner.nodes().len(), 2);
    }

    #[test]
    fn test_timeout_learner_learn() {
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|i| {
                ExecutionTrace::builder()
                    .add_node_execution(NodeExecution::new("stable_node", 100 + i as u64 % 10))
                    .add_node_execution(NodeExecution::new("variable_node", 50 + i as u64 * 20))
                    .build()
            })
            .collect();

        let mut learner = TimeoutLearner::new();
        learner.add_traces(&traces);

        let config = TimeoutConfig::default();
        let recommendations = learner.learn(&config);

        assert!(recommendations.has_recommendations());
        assert!(!recommendations.recommendations.is_empty());
    }

    #[test]
    fn test_timeout_learner_insufficient_data() {
        let trace = ExecutionTrace::builder()
            .add_node_execution(NodeExecution::new("rare_node", 100))
            .build();

        let mut learner = TimeoutLearner::new();
        learner.add_trace(&trace);

        let config = TimeoutConfig::default(); // min_samples = 5
        let recommendations = learner.learn(&config);

        assert!(recommendations
            .insufficient_data_nodes
            .contains(&"rare_node".to_string()));
        assert!(recommendations
            .recommendations
            .iter()
            .all(|r| r.node != "rare_node"));
    }

    #[test]
    fn test_timeout_learner_clear() {
        let mut learner = TimeoutLearner::new();
        learner.add_trace(
            &ExecutionTrace::builder()
                .add_node_execution(NodeExecution::new("test", 100))
                .build(),
        );

        assert!(!learner.nodes().is_empty());
        learner.clear();
        assert!(learner.nodes().is_empty());
    }

    #[test]
    fn test_timeout_recommendations_above_confidence() {
        let executions = create_test_executions("test", &[100, 110, 105, 108, 102]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();
        let stats = LatencyStats::from_executions("test", &refs);

        let mut recommendations = TimeoutRecommendations::new(TimeoutConfig::default());
        recommendations
            .recommendations
            .push(TimeoutRecommendation::new("high_conf", 200, stats.clone()).with_confidence(0.9));
        recommendations
            .recommendations
            .push(TimeoutRecommendation::new("low_conf", 150, stats).with_confidence(0.3));

        let high = recommendations.above_confidence(0.8);
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].node, "high_conf");
    }

    #[test]
    fn test_timeout_recommendations_to_mutations() {
        let executions = create_test_executions("test", &[100, 110, 105, 108, 102]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();
        let stats = LatencyStats::from_executions("test", &refs);

        let mut recommendations = TimeoutRecommendations::new(TimeoutConfig::default());
        recommendations
            .recommendations
            .push(TimeoutRecommendation::new("node_a", 200, stats.clone()).with_confidence(0.9));
        recommendations
            .recommendations
            .push(TimeoutRecommendation::new("node_b", 150, stats).with_confidence(0.5));

        let mutations = recommendations.to_mutations(0.7);
        assert_eq!(mutations.len(), 1);
    }

    #[test]
    fn test_timeout_history() {
        let mut history = TimeoutHistory::new();

        let record = TimeoutAdjustmentRecord {
            node: "test_node".to_string(),
            previous_timeout_ms: Some(1000),
            new_timeout_ms: 500,
            reason: "Based on p95".to_string(),
            adjusted_at: "2025-01-01T00:00:00Z".to_string(),
            outcome: Some(TimeoutAdjustmentOutcome {
                beneficial: true,
                error_rate_delta: -0.1,
                latency_delta_ms: -50.0,
                notes: None,
            }),
        };

        history.add_record(record);

        assert_eq!(history.adjustments.len(), 1);
        assert_eq!(*history.adjustment_counts.get("test_node").unwrap(), 1);
        assert_eq!(history.beneficial_adjustments().len(), 1);
        assert_eq!(history.success_rate(), 1.0);
    }

    #[test]
    fn test_latency_stats_summary() {
        let executions = create_test_executions("api_node", &[100, 120, 110, 130, 90]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();

        let stats = LatencyStats::from_executions("api_node", &refs);
        let summary = stats.summary();

        assert!(summary.contains("api_node"));
        assert!(summary.contains("5 samples"));
    }

    #[test]
    fn test_timeout_recommendation_description() {
        let executions = create_test_executions("test", &[100, 110, 105]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();
        let stats = LatencyStats::from_executions("test", &refs);

        let rec = TimeoutRecommendation::new("test", 200, stats)
            .with_current_timeout(100)
            .with_confidence(0.85);

        let desc = rec.description();

        assert!(desc.contains("test"));
        assert!(desc.contains("200ms"));
        assert!(desc.contains("85%"));
    }

    #[test]
    fn test_learn_from_traces_static() {
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|i| {
                ExecutionTrace::builder()
                    .add_node_execution(NodeExecution::new("stable", 100 + i as u64 % 5))
                    .build()
            })
            .collect();

        let recommendations = TimeoutLearner::learn_from_traces(&traces, &TimeoutConfig::default());

        assert!(recommendations.has_recommendations());
    }

    #[test]
    fn test_timeout_config_stable_only() {
        let config = TimeoutConfig::stable_only();
        assert!(config.require_stability);
        assert!(config.max_cv_for_stability < TimeoutConfig::default().max_cv_for_stability);
        assert!(config.min_confidence > TimeoutConfig::default().min_confidence);
    }

    #[test]
    fn test_percentile_mean_plus_std_dev() {
        let executions = create_test_executions("test", &[100, 200, 150, 175, 125]);
        let refs: Vec<&NodeExecution> = executions.iter().collect();
        let stats = LatencyStats::from_executions("test", &refs);

        let value = TimeoutPercentile::MeanPlusStdDev(2).value(&stats);

        // Should be mean + 2*std_dev
        let expected = (stats.mean_ms + 2.0 * stats.std_dev_ms) as u64;
        assert_eq!(value, expected);
    }
}
