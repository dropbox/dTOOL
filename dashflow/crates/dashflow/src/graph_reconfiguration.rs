// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Dynamic Graph Reconfiguration
//!
//! This module provides runtime modification capabilities for compiled graphs,
//! enabling AI agents to optimize and adapt their own execution structure.
//!
//! # Overview
//!
//! DashFlow graphs are normally immutable after compilation for thread safety.
//! This module provides controlled mutation capabilities through the
//! [`GraphMutation`] and [`MutationType`] system, allowing graphs to evolve
//! based on runtime performance data.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                 CompiledGraph                        │
//! │  ┌─────────────────┐    ┌──────────────────────┐   │
//! │  │ apply_mutation  │───→│ GraphMutation        │   │
//! │  └─────────────────┘    │  - mutation_type     │   │
//! │                         │  - target_node       │   │
//! │  ┌─────────────────┐    │  - config            │   │
//! │  │ self_optimize   │    └──────────────────────┘   │
//! │  └─────────────────┘                               │
//! │         │                                          │
//! │         ▼                                          │
//! │  ┌─────────────────────────────────────┐          │
//! │  │ BottleneckAnalysis (introspection)  │          │
//! │  └─────────────────────────────────────┘          │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::graph_reconfiguration::{GraphMutation, MutationType};
//!
//! // AI detects a bottleneck and creates a mutation
//! let mutation = GraphMutation::new(
//!     MutationType::AddRetry {
//!         node: "api_call".to_string(),
//!         max_retries: 3,
//!     },
//! );
//!
//! // Apply the mutation
//! let mut compiled = graph.compile()?;
//! compiled.apply_mutation(mutation)?;
//! ```
//!
//! # Safety
//!
//! All mutations are validated before application to ensure graph integrity.
//! Invalid mutations (e.g., referencing non-existent nodes) return errors
//! rather than corrupting the graph state.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::constants::DEFAULT_MAX_RETRIES;
use crate::edge::{Edge, END};
use crate::error::{Error, Result};
use crate::introspection::{Bottleneck, BottleneckAnalysis, BottleneckMetric};
use crate::node::BoxedNode;

// ============================================================================
// Mutation Types
// ============================================================================

/// Types of mutations that can be applied to a compiled graph
///
/// Each mutation type represents a specific transformation that can be
/// applied to the graph structure to improve performance or behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MutationType {
    /// Insert a caching node before the specified node
    ///
    /// Useful when a node is called repeatedly with the same inputs.
    /// The cache stores results keyed by input state hash.
    AddCache {
        /// Node to add caching before
        before_node: String,
        /// Cache key prefix for namespacing
        cache_key: String,
        /// Optional TTL for cache entries (default: no expiration)
        ttl: Option<Duration>,
    },

    /// Convert sequential nodes to parallel execution
    ///
    /// Nodes that have no data dependencies between them can be
    /// executed in parallel to reduce total execution time.
    ChangeToParallel {
        /// Nodes to execute in parallel
        nodes: Vec<String>,
        /// Node that triggers the parallel execution
        from_node: String,
    },

    /// Add retry logic to a node
    ///
    /// Useful for nodes that make external API calls that may fail transiently.
    AddRetry {
        /// Node to add retry logic to
        node: String,
        /// Maximum number of retry attempts
        max_retries: usize,
        /// Base delay between retries (exponential backoff applied)
        base_delay: Option<Duration>,
    },

    /// Adjust the timeout for a specific node
    ///
    /// Useful when a node consistently takes longer than the default timeout,
    /// or when the AI learns optimal timeouts from historical data.
    AdjustTimeout {
        /// Node to adjust timeout for
        node: String,
        /// New timeout duration
        timeout: Duration,
    },

    /// Add a new edge between existing nodes
    ///
    /// Creates a direct path between two nodes that were not previously connected.
    AddEdge {
        /// Source node
        from: String,
        /// Target node
        to: String,
    },

    /// Remove an edge between nodes
    ///
    /// Useful for eliminating unnecessary paths in the graph.
    RemoveEdge {
        /// Source node
        from: String,
        /// Target node
        to: String,
    },

    /// Set a node as an interrupt point (human-in-the-loop)
    ///
    /// Execution will pause before or after this node, allowing
    /// human review and approval.
    SetInterrupt {
        /// Node to set as interrupt point
        node: String,
        /// Whether to interrupt before (true) or after (false) the node
        before: bool,
    },

    /// Clear interrupt point from a node
    ClearInterrupt {
        /// Node to clear interrupt from
        node: String,
        /// Whether to clear before (true) or after (false) interrupt
        before: bool,
    },

    /// Adjust the graph recursion limit
    ///
    /// Useful when the AI determines that a higher or lower limit is appropriate
    /// based on graph complexity and execution patterns.
    AdjustRecursionLimit {
        /// New recursion limit
        limit: u32,
    },
}

impl MutationType {
    /// Get a human-readable description of the mutation
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::AddCache {
                before_node,
                cache_key,
                ttl,
            } => {
                let ttl_str = ttl.map(|d| format!(" (TTL: {:?})", d)).unwrap_or_default();
                format!(
                    "Add cache '{}' before node '{}'{}",
                    cache_key, before_node, ttl_str
                )
            }
            Self::ChangeToParallel { nodes, from_node } => {
                format!(
                    "Change nodes {:?} to parallel execution from '{}'",
                    nodes, from_node
                )
            }
            Self::AddRetry {
                node,
                max_retries,
                base_delay,
            } => {
                let delay_str = base_delay
                    .map(|d| format!(" (base delay: {:?})", d))
                    .unwrap_or_default();
                format!(
                    "Add {} retries to node '{}'{}",
                    max_retries, node, delay_str
                )
            }
            Self::AdjustTimeout { node, timeout } => {
                format!("Adjust timeout for node '{}' to {:?}", node, timeout)
            }
            Self::AddEdge { from, to } => {
                format!("Add edge from '{}' to '{}'", from, to)
            }
            Self::RemoveEdge { from, to } => {
                format!("Remove edge from '{}' to '{}'", from, to)
            }
            Self::SetInterrupt { node, before } => {
                let timing = if *before { "before" } else { "after" };
                format!("Set interrupt {} node '{}'", timing, node)
            }
            Self::ClearInterrupt { node, before } => {
                let timing = if *before { "before" } else { "after" };
                format!("Clear interrupt {} node '{}'", timing, node)
            }
            Self::AdjustRecursionLimit { limit } => {
                format!("Adjust recursion limit to {}", limit)
            }
        }
    }

    /// Get the target node(s) affected by this mutation
    #[must_use]
    pub fn target_nodes(&self) -> Vec<String> {
        match self {
            Self::AddCache { before_node, .. } => vec![before_node.clone()],
            Self::ChangeToParallel { nodes, from_node } => {
                let mut result = nodes.clone();
                result.push(from_node.clone());
                result
            }
            Self::AddRetry { node, .. } => vec![node.clone()],
            Self::AdjustTimeout { node, .. } => vec![node.clone()],
            Self::AddEdge { from, to } => vec![from.clone(), to.clone()],
            Self::RemoveEdge { from, to } => vec![from.clone(), to.clone()],
            Self::SetInterrupt { node, .. } => vec![node.clone()],
            Self::ClearInterrupt { node, .. } => vec![node.clone()],
            Self::AdjustRecursionLimit { .. } => vec![],
        }
    }
}

// ============================================================================
// Graph Mutation
// ============================================================================

/// A mutation to be applied to a compiled graph
///
/// Graph mutations encapsulate changes to graph structure, configuration,
/// or behavior. They can be created manually or generated automatically
/// by the `self_optimize` method based on execution analysis.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::graph_reconfiguration::{GraphMutation, MutationType};
///
/// let mutation = GraphMutation::new(MutationType::AddRetry {
///     node: "api_call".to_string(),
///     max_retries: 3,
///     base_delay: Some(Duration::from_millis(100)),
/// })
/// .with_reason("High retry rate detected (35%)")
/// .with_expected_improvement("Reduce failure rate from 35% to <5%");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMutation {
    /// The type of mutation to apply
    pub mutation_type: MutationType,
    /// Why this mutation was suggested (for explainability)
    pub reason: Option<String>,
    /// Expected improvement from applying this mutation
    pub expected_improvement: Option<String>,
    /// Confidence level (0.0 to 1.0) in the mutation's effectiveness
    pub confidence: Option<f64>,
    /// Additional configuration data
    pub config: Option<serde_json::Value>,
}

impl GraphMutation {
    /// Create a new graph mutation
    #[must_use]
    pub fn new(mutation_type: MutationType) -> Self {
        Self {
            mutation_type,
            reason: None,
            expected_improvement: None,
            confidence: None,
            config: None,
        }
    }

    /// Set the reason for this mutation
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the expected improvement
    #[must_use]
    pub fn with_expected_improvement(mut self, improvement: impl Into<String>) -> Self {
        self.expected_improvement = Some(improvement.into());
        self
    }

    /// Set the confidence level
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Set additional configuration
    #[must_use]
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = Some(config);
        self
    }

    /// Get a human-readable description of the mutation
    #[must_use]
    pub fn description(&self) -> String {
        let mut desc = self.mutation_type.description();

        if let Some(reason) = &self.reason {
            desc.push_str(&format!(" (reason: {})", reason));
        }

        if let Some(confidence) = self.confidence {
            desc.push_str(&format!(" [confidence: {:.0}%]", confidence * 100.0));
        }

        desc
    }
}

// ============================================================================
// Mutation Result
// ============================================================================

/// Result of applying a graph mutation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationResult {
    /// Whether the mutation was successfully applied
    pub success: bool,
    /// The mutation that was applied
    pub mutation: GraphMutation,
    /// Any warnings generated during application
    pub warnings: Vec<String>,
    /// Nodes that were affected by the mutation
    pub affected_nodes: Vec<String>,
}

impl MutationResult {
    /// Create a successful mutation result
    #[must_use]
    pub fn success(mutation: GraphMutation, affected_nodes: Vec<String>) -> Self {
        Self {
            success: true,
            mutation,
            warnings: Vec::new(),
            affected_nodes,
        }
    }

    /// Create a successful result with warnings
    #[must_use]
    pub fn success_with_warnings(
        mutation: GraphMutation,
        affected_nodes: Vec<String>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            success: true,
            mutation,
            warnings,
            affected_nodes,
        }
    }
}

// ============================================================================
// Optimization Suggestions
// ============================================================================

/// Suggestions generated by analyzing execution traces
///
/// The `self_optimize` method analyzes bottlenecks and generates
/// a list of suggested mutations along with confidence scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestions {
    /// Suggested mutations based on execution analysis
    pub mutations: Vec<GraphMutation>,
    /// Analysis that generated these suggestions
    pub analysis: BottleneckAnalysis,
    /// Overall health score of the graph (0.0 to 1.0)
    pub health_score: f64,
    /// Summary of analysis findings
    pub summary: String,
}

impl OptimizationSuggestions {
    /// Create new optimization suggestions
    #[must_use]
    pub fn new(analysis: BottleneckAnalysis) -> Self {
        let mutations = Self::generate_mutations_from_analysis(&analysis);
        let health_score = Self::calculate_health_score(&analysis);
        let summary = Self::generate_summary(&analysis, &mutations);

        Self {
            mutations,
            analysis,
            health_score,
            summary,
        }
    }

    /// Generate mutations based on bottleneck analysis
    fn generate_mutations_from_analysis(analysis: &BottleneckAnalysis) -> Vec<GraphMutation> {
        let mut mutations = Vec::with_capacity(analysis.bottlenecks.len());

        for bottleneck in &analysis.bottlenecks {
            if let Some(mutation) = Self::bottleneck_to_mutation(bottleneck) {
                mutations.push(mutation);
            }
        }

        mutations
    }

    /// Convert a bottleneck to an appropriate mutation
    fn bottleneck_to_mutation(bottleneck: &Bottleneck) -> Option<GraphMutation> {
        let mutation_type = match &bottleneck.metric {
            BottleneckMetric::Latency => Some(MutationType::AdjustTimeout {
                node: bottleneck.node.clone(),
                // Set timeout to 1.5x the observed latency to allow headroom
                timeout: Duration::from_secs_f64(bottleneck.value * 1.5),
            }),

            BottleneckMetric::ErrorRate => Some(MutationType::AddRetry {
                node: bottleneck.node.clone(),
                max_retries: DEFAULT_MAX_RETRIES as usize,
                base_delay: Some(Duration::from_millis(100)),
            }),

            BottleneckMetric::HighFrequency => {
                // High frequency suggests possible loop - consider adding interrupt
                Some(MutationType::SetInterrupt {
                    node: bottleneck.node.clone(),
                    before: true,
                })
            }

            BottleneckMetric::TokenUsage | BottleneckMetric::HighVariance => {
                // These require more complex optimizations not yet supported
                None
            }
        };

        mutation_type.map(|mt| {
            GraphMutation::new(mt)
                .with_reason(bottleneck.description.clone())
                .with_expected_improvement(bottleneck.suggestion.clone())
                .with_confidence(
                    bottleneck
                        .percentage_of_total
                        .map(|p| (p / 100.0).min(1.0))
                        .unwrap_or(0.5),
                )
        })
    }

    /// Calculate overall health score based on bottleneck analysis
    fn calculate_health_score(analysis: &BottleneckAnalysis) -> f64 {
        if analysis.bottlenecks.is_empty() {
            return 1.0; // Perfect health if no bottlenecks
        }

        // Score decreases with number and severity of bottlenecks
        let total_severity: f64 = analysis
            .bottlenecks
            .iter()
            .map(|b| match b.severity {
                crate::introspection::BottleneckSeverity::Minor => 0.1,
                crate::introspection::BottleneckSeverity::Moderate => 0.25,
                crate::introspection::BottleneckSeverity::Severe => 0.5,
                crate::introspection::BottleneckSeverity::Critical => 0.75,
            })
            .sum();

        (1.0 - total_severity).max(0.0)
    }

    /// Generate summary text
    fn generate_summary(analysis: &BottleneckAnalysis, mutations: &[GraphMutation]) -> String {
        if analysis.bottlenecks.is_empty() {
            return "No bottlenecks detected. Graph is performing optimally.".to_string();
        }

        format!(
            "Detected {} bottleneck(s). Generated {} optimization suggestion(s). \
             Primary issues: {}",
            analysis.bottlenecks.len(),
            mutations.len(),
            analysis
                .bottlenecks
                .iter()
                .map(|b| b.metric.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    /// Get high-confidence mutations (>= threshold)
    #[must_use]
    pub fn high_confidence_mutations(&self, threshold: f64) -> Vec<&GraphMutation> {
        self.mutations
            .iter()
            .filter(|m| m.confidence.unwrap_or(0.0) >= threshold)
            .collect()
    }

    /// Check if any optimizations are suggested
    #[must_use]
    pub fn has_suggestions(&self) -> bool {
        !self.mutations.is_empty()
    }
}

// ============================================================================
// Node Timeout Configuration
// ============================================================================

/// Per-node timeout configuration
///
/// Allows setting different timeouts for different nodes based on
/// their expected execution characteristics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeTimeouts {
    /// Map of node name to timeout duration
    pub timeouts: HashMap<String, Duration>,
}

impl NodeTimeouts {
    /// Create a new empty timeout configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set timeout for a specific node
    #[must_use]
    pub fn with_timeout(mut self, node: impl Into<String>, timeout: Duration) -> Self {
        self.timeouts.insert(node.into(), timeout);
        self
    }

    /// Get timeout for a node, or None if not configured
    #[must_use]
    pub fn get(&self, node: &str) -> Option<Duration> {
        self.timeouts.get(node).copied()
    }

    /// Check if any timeouts are configured
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.timeouts.is_empty()
    }
}

// ============================================================================
// Validation
// ============================================================================

/// Validate that a mutation can be applied to the graph
pub fn validate_mutation<S>(
    mutation: &GraphMutation,
    nodes: &HashMap<String, BoxedNode<S>>,
    edges: &[Edge],
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    // Validate target nodes exist
    for target in mutation.mutation_type.target_nodes() {
        // Allow END as a valid target
        if target != END && !nodes.contains_key(&target) {
            return Err(Error::Validation(format!(
                "Mutation references non-existent node: '{}'",
                target
            )));
        }
    }

    // Type-specific validation
    match &mutation.mutation_type {
        MutationType::RemoveEdge { from, to } => {
            // Check edge exists
            let edge_exists = edges
                .iter()
                .any(|e| e.from.as_str() == from && e.to.as_str() == to);
            if !edge_exists {
                return Err(Error::Validation(format!(
                    "Cannot remove non-existent edge: '{}' -> '{}'",
                    from, to
                )));
            }
        }

        MutationType::AddEdge { from, to } => {
            // Check edge doesn't already exist
            let edge_exists = edges
                .iter()
                .any(|e| e.from.as_str() == from && e.to.as_str() == to);
            if edge_exists {
                warnings.push(format!("Edge '{}' -> '{}' already exists", from, to));
            }
        }

        MutationType::AdjustRecursionLimit { limit } => {
            if *limit == 0 {
                return Err(Error::Validation("Recursion limit cannot be 0".to_string()));
            }
            if *limit > 1000 {
                warnings.push(format!(
                    "High recursion limit ({}) may cause stack overflow",
                    limit
                ));
            }
        }

        MutationType::AddRetry { max_retries, .. } => {
            if *max_retries == 0 {
                warnings.push("max_retries is 0, which disables retry functionality".to_string());
            }
            if *max_retries > 10 {
                warnings.push(format!(
                    "High retry count ({}) may cause excessive delays",
                    max_retries
                ));
            }
        }

        // Other mutation types don't need additional validation
        _ => {}
    }

    Ok(warnings)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutation_type_description() {
        let mt = MutationType::AddRetry {
            node: "api_call".to_string(),
            max_retries: DEFAULT_MAX_RETRIES as usize,
            base_delay: Some(Duration::from_millis(100)),
        };

        let desc = mt.description();
        assert!(desc.contains("api_call"));
        assert!(desc.contains(&DEFAULT_MAX_RETRIES.to_string()));
    }

    #[test]
    fn test_mutation_type_target_nodes() {
        let mt = MutationType::AddEdge {
            from: "node_a".to_string(),
            to: "node_b".to_string(),
        };

        let targets = mt.target_nodes();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&"node_a".to_string()));
        assert!(targets.contains(&"node_b".to_string()));
    }

    #[test]
    fn test_graph_mutation_builder() {
        let mutation = GraphMutation::new(MutationType::AdjustRecursionLimit { limit: 50 })
            .with_reason("Deep recursive graph detected")
            .with_expected_improvement("Allow complex workflows")
            .with_confidence(0.85);

        assert!(mutation.reason.is_some());
        assert!(mutation.expected_improvement.is_some());
        assert_eq!(mutation.confidence, Some(0.85));
    }

    #[test]
    fn test_confidence_clamping() {
        let mutation = GraphMutation::new(MutationType::AdjustRecursionLimit { limit: 50 })
            .with_confidence(1.5); // Over 1.0

        assert_eq!(mutation.confidence, Some(1.0)); // Should be clamped

        let mutation2 = GraphMutation::new(MutationType::AdjustRecursionLimit { limit: 50 })
            .with_confidence(-0.5); // Under 0.0

        assert_eq!(mutation2.confidence, Some(0.0)); // Should be clamped
    }

    #[test]
    fn test_mutation_result() {
        let mutation = GraphMutation::new(MutationType::AdjustRecursionLimit { limit: 50 });
        let result = MutationResult::success(mutation.clone(), vec!["node_a".to_string()]);

        assert!(result.success);
        assert!(result.warnings.is_empty());
        assert_eq!(result.affected_nodes, vec!["node_a".to_string()]);
    }

    #[test]
    fn test_node_timeouts() {
        let timeouts = NodeTimeouts::new()
            .with_timeout("slow_node", Duration::from_secs(60))
            .with_timeout("fast_node", Duration::from_millis(100));

        assert_eq!(timeouts.get("slow_node"), Some(Duration::from_secs(60)));
        assert_eq!(timeouts.get("fast_node"), Some(Duration::from_millis(100)));
        assert_eq!(timeouts.get("unknown"), None);
    }

    #[test]
    fn test_optimization_suggestions_health_score() {
        use crate::introspection::BottleneckThresholds;

        // Create an empty analysis (no bottlenecks)
        let analysis = BottleneckAnalysis {
            bottlenecks: Vec::new(),
            nodes_analyzed: 5,
            total_duration_ms: 1000,
            total_tokens: 500,
            thresholds: BottleneckThresholds::default(),
            summary: "No bottlenecks detected".to_string(),
        };

        let suggestions = OptimizationSuggestions::new(analysis);
        assert_eq!(suggestions.health_score, 1.0);
        assert!(!suggestions.has_suggestions());
    }

    #[test]
    fn test_change_to_parallel_description() {
        let mt = MutationType::ChangeToParallel {
            nodes: vec!["node_a".to_string(), "node_b".to_string()],
            from_node: "start".to_string(),
        };

        let desc = mt.description();
        assert!(desc.contains("parallel"));
        assert!(desc.contains("node_a"));
        assert!(desc.contains("node_b"));
        assert!(desc.contains("start"));
    }

    #[test]
    fn test_add_cache_description() {
        let mt = MutationType::AddCache {
            before_node: "expensive".to_string(),
            cache_key: "results".to_string(),
            ttl: Some(Duration::from_secs(300)),
        };

        let desc = mt.description();
        assert!(desc.contains("cache"));
        assert!(desc.contains("expensive"));
        assert!(desc.contains("results"));
        assert!(desc.contains("TTL"));
    }

    #[test]
    fn test_set_interrupt_description() {
        let mt_before = MutationType::SetInterrupt {
            node: "human_review".to_string(),
            before: true,
        };
        let mt_after = MutationType::SetInterrupt {
            node: "human_review".to_string(),
            before: false,
        };

        assert!(mt_before.description().contains("before"));
        assert!(mt_after.description().contains("after"));
    }

    #[test]
    fn test_high_confidence_mutations() {
        use crate::introspection::BottleneckThresholds;

        let analysis = BottleneckAnalysis {
            bottlenecks: Vec::new(),
            nodes_analyzed: 5,
            total_duration_ms: 1000,
            total_tokens: 500,
            thresholds: BottleneckThresholds::default(),
            summary: "No bottlenecks detected".to_string(),
        };

        let mut suggestions = OptimizationSuggestions::new(analysis);

        // Add some mutations with varying confidence
        suggestions.mutations.push(
            GraphMutation::new(MutationType::AdjustRecursionLimit { limit: 50 })
                .with_confidence(0.9),
        );
        suggestions.mutations.push(
            GraphMutation::new(MutationType::AdjustRecursionLimit { limit: 30 })
                .with_confidence(0.5),
        );
        suggestions.mutations.push(
            GraphMutation::new(MutationType::AdjustRecursionLimit { limit: 40 })
                .with_confidence(0.3),
        );

        let high_conf = suggestions.high_confidence_mutations(0.7);
        assert_eq!(high_conf.len(), 1);
        assert_eq!(high_conf[0].confidence, Some(0.9));
    }
}
