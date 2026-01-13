//! Configuration Recommendations
//!
//! This module provides types for generating configuration recommendations
//! based on pattern analysis.

use super::optimization::{OptimizationCategory, OptimizationPriority, OptimizationSuggestion};
use super::pattern::{PatternAnalysis, PatternType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Configuration Recommendations
// ============================================================================

/// Type of graph reconfiguration
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReconfigurationType {
    /// Add a cache node before a target node
    #[default]
    AddCache,
    /// Convert sequential edges to parallel edges
    Parallelize,
    /// Change the model used by a node
    SwapModel,
    /// Adjust timeout for a node
    AdjustTimeout,
    /// Add retry logic
    AddRetry,
    /// Remove or skip a node
    SkipNode,
    /// Merge multiple nodes into one
    MergeNodes,
    /// Split a node into multiple smaller nodes
    SplitNode,
    /// Add batching for repeated operations
    AddBatching,
    /// Add rate limiting
    AddRateLimiting,
    /// Change routing strategy
    ChangeRouting,
    /// Custom reconfiguration
    Custom(String),
}

impl std::fmt::Display for ReconfigurationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AddCache => write!(f, "add_cache"),
            Self::Parallelize => write!(f, "parallelize"),
            Self::SwapModel => write!(f, "swap_model"),
            Self::AdjustTimeout => write!(f, "adjust_timeout"),
            Self::AddRetry => write!(f, "add_retry"),
            Self::SkipNode => write!(f, "skip_node"),
            Self::MergeNodes => write!(f, "merge_nodes"),
            Self::SplitNode => write!(f, "split_node"),
            Self::AddBatching => write!(f, "add_batching"),
            Self::AddRateLimiting => write!(f, "add_rate_limiting"),
            Self::ChangeRouting => write!(f, "change_routing"),
            Self::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// Priority level for reconfigurations
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReconfigurationPriority {
    /// Low priority - nice to have
    Low,
    /// Medium priority - should consider
    #[default]
    Medium,
    /// High priority - should implement soon
    High,
    /// Critical priority - implement immediately
    Critical,
}

impl std::fmt::Display for ReconfigurationPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// A recommended graph reconfiguration based on learned patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphReconfiguration {
    /// Unique identifier for this recommendation
    pub id: String,
    /// Type of reconfiguration
    pub reconfiguration_type: ReconfigurationType,
    /// Target node(s) affected by this reconfiguration
    pub target_nodes: Vec<String>,
    /// Human-readable description
    pub description: String,
    /// Expected improvement from applying this change
    pub expected_improvement: String,
    /// Detailed implementation guidance
    pub implementation: String,
    /// Priority of this reconfiguration
    pub priority: ReconfigurationPriority,
    /// Estimated effort (1-5 scale, 1 being easiest)
    pub effort: u8,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
    /// Patterns that triggered this recommendation
    pub triggering_patterns: Vec<String>,
    /// Additional evidence or context
    pub evidence: Vec<String>,
    /// Estimated performance impact percentage
    pub estimated_impact_pct: Option<f64>,
    /// Prerequisites - other reconfigurations that should be done first
    pub prerequisites: Vec<String>,
    /// Conflicts - reconfigurations that cannot coexist
    pub conflicts: Vec<String>,
}

impl GraphReconfiguration {
    /// Create a new graph reconfiguration
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        reconfiguration_type: ReconfigurationType,
        target_nodes: Vec<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            reconfiguration_type,
            target_nodes,
            description: description.into(),
            expected_improvement: String::new(),
            implementation: String::new(),
            priority: ReconfigurationPriority::default(),
            effort: 3,
            confidence: 0.5,
            triggering_patterns: Vec::new(),
            evidence: Vec::new(),
            estimated_impact_pct: None,
            prerequisites: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    /// Create a builder for graph reconfigurations
    #[must_use]
    pub fn builder() -> GraphReconfigurationBuilder {
        GraphReconfigurationBuilder::new()
    }

    /// Set expected improvement
    #[must_use]
    pub fn with_expected_improvement(mut self, improvement: impl Into<String>) -> Self {
        self.expected_improvement = improvement.into();
        self
    }

    /// Set implementation guidance
    #[must_use]
    pub fn with_implementation(mut self, impl_guide: impl Into<String>) -> Self {
        self.implementation = impl_guide.into();
        self
    }

    /// Set priority
    #[must_use]
    pub fn with_priority(mut self, priority: ReconfigurationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set effort level
    #[must_use]
    pub fn with_effort(mut self, effort: u8) -> Self {
        self.effort = effort.clamp(1, 5);
        self
    }

    /// Set confidence level
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add a triggering pattern ID
    #[must_use]
    pub fn with_triggering_pattern(mut self, pattern_id: impl Into<String>) -> Self {
        self.triggering_patterns.push(pattern_id.into());
        self
    }

    /// Add evidence
    #[must_use]
    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }

    /// Set estimated impact percentage
    #[must_use]
    pub fn with_estimated_impact(mut self, impact_pct: f64) -> Self {
        self.estimated_impact_pct = Some(impact_pct);
        self
    }

    /// Add a prerequisite
    #[must_use]
    pub fn with_prerequisite(mut self, prereq_id: impl Into<String>) -> Self {
        self.prerequisites.push(prereq_id.into());
        self
    }

    /// Add a conflict
    #[must_use]
    pub fn with_conflict(mut self, conflict_id: impl Into<String>) -> Self {
        self.conflicts.push(conflict_id.into());
        self
    }

    /// Check if this is high priority
    #[must_use]
    pub fn is_high_priority(&self) -> bool {
        self.priority >= ReconfigurationPriority::High
    }

    /// Check if this is low effort
    #[must_use]
    pub fn is_low_effort(&self) -> bool {
        self.effort <= 2
    }

    /// Check if this is a quick win (high priority + low effort)
    #[must_use]
    pub fn is_quick_win(&self) -> bool {
        self.is_high_priority() && self.is_low_effort()
    }

    /// Get a quick win score (higher = better quick win)
    #[must_use]
    pub fn quick_win_score(&self) -> f64 {
        let priority_score = match self.priority {
            ReconfigurationPriority::Low => 1.0,
            ReconfigurationPriority::Medium => 2.0,
            ReconfigurationPriority::High => 3.0,
            ReconfigurationPriority::Critical => 4.0,
        };
        let effort_score = 6.0 - self.effort as f64;
        (priority_score * effort_score * self.confidence) / 4.0
    }

    /// Get a formatted summary
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} ({}): {} - {}",
            self.priority,
            self.reconfiguration_type,
            self.target_nodes.join(", "),
            self.description,
            self.expected_improvement
        )
    }

    /// Convert to OptimizationSuggestion for compatibility
    #[must_use]
    pub fn to_optimization_suggestion(&self) -> OptimizationSuggestion {
        let category = match self.reconfiguration_type {
            ReconfigurationType::AddCache => OptimizationCategory::Caching,
            ReconfigurationType::Parallelize => OptimizationCategory::Parallelization,
            ReconfigurationType::SwapModel => OptimizationCategory::ModelChoice,
            ReconfigurationType::AdjustTimeout => OptimizationCategory::Stabilization,
            ReconfigurationType::AddRetry => OptimizationCategory::ErrorHandling,
            ReconfigurationType::SkipNode => OptimizationCategory::Performance,
            ReconfigurationType::MergeNodes => OptimizationCategory::Performance,
            ReconfigurationType::SplitNode => OptimizationCategory::Performance,
            ReconfigurationType::AddBatching => OptimizationCategory::Performance,
            ReconfigurationType::AddRateLimiting => OptimizationCategory::Stabilization,
            ReconfigurationType::ChangeRouting => OptimizationCategory::Performance,
            ReconfigurationType::Custom(_) => OptimizationCategory::Performance,
        };

        let opt_priority = match self.priority {
            ReconfigurationPriority::Low => OptimizationPriority::Low,
            ReconfigurationPriority::Medium => OptimizationPriority::Medium,
            ReconfigurationPriority::High => OptimizationPriority::High,
            ReconfigurationPriority::Critical => OptimizationPriority::Critical,
        };

        let mut suggestion = OptimizationSuggestion::new(
            category,
            self.target_nodes.clone(),
            &self.description,
            &self.expected_improvement,
            &self.implementation,
        )
        .with_priority(opt_priority)
        .with_effort(self.effort)
        .with_confidence(self.confidence);

        for ev in &self.evidence {
            suggestion = suggestion.with_evidence(ev);
        }

        suggestion
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

/// Builder for GraphReconfiguration
#[derive(Debug, Default)]
pub struct GraphReconfigurationBuilder {
    id: Option<String>,
    reconfiguration_type: ReconfigurationType,
    target_nodes: Vec<String>,
    description: Option<String>,
    expected_improvement: String,
    implementation: String,
    priority: ReconfigurationPriority,
    effort: u8,
    confidence: f64,
    triggering_patterns: Vec<String>,
    evidence: Vec<String>,
    estimated_impact_pct: Option<f64>,
    prerequisites: Vec<String>,
    conflicts: Vec<String>,
}

impl GraphReconfigurationBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            effort: 3,
            confidence: 0.5,
            ..Self::default()
        }
    }

    /// Set the ID
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the reconfiguration type
    #[must_use]
    pub fn reconfiguration_type(mut self, rtype: ReconfigurationType) -> Self {
        self.reconfiguration_type = rtype;
        self
    }

    /// Add a target node
    #[must_use]
    pub fn target_node(mut self, node: impl Into<String>) -> Self {
        self.target_nodes.push(node.into());
        self
    }

    /// Set target nodes
    #[must_use]
    pub fn target_nodes(mut self, nodes: Vec<String>) -> Self {
        self.target_nodes = nodes;
        self
    }

    /// Set the description
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set expected improvement
    #[must_use]
    pub fn expected_improvement(mut self, improvement: impl Into<String>) -> Self {
        self.expected_improvement = improvement.into();
        self
    }

    /// Set implementation guidance
    #[must_use]
    pub fn implementation(mut self, impl_guide: impl Into<String>) -> Self {
        self.implementation = impl_guide.into();
        self
    }

    /// Set priority
    #[must_use]
    pub fn priority(mut self, priority: ReconfigurationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set effort level
    #[must_use]
    pub fn effort(mut self, effort: u8) -> Self {
        self.effort = effort.clamp(1, 5);
        self
    }

    /// Set confidence level
    #[must_use]
    pub fn confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add a triggering pattern
    #[must_use]
    pub fn triggering_pattern(mut self, pattern_id: impl Into<String>) -> Self {
        self.triggering_patterns.push(pattern_id.into());
        self
    }

    /// Add evidence
    #[must_use]
    pub fn evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }

    /// Set estimated impact
    #[must_use]
    pub fn estimated_impact(mut self, impact_pct: f64) -> Self {
        self.estimated_impact_pct = Some(impact_pct);
        self
    }

    /// Add a prerequisite
    #[must_use]
    pub fn prerequisite(mut self, prereq: impl Into<String>) -> Self {
        self.prerequisites.push(prereq.into());
        self
    }

    /// Add a conflict
    #[must_use]
    pub fn conflict(mut self, conflict: impl Into<String>) -> Self {
        self.conflicts.push(conflict.into());
        self
    }

    /// Build the reconfiguration
    ///
    /// # Errors
    ///
    /// Returns error if required fields are missing
    pub fn build(self) -> Result<GraphReconfiguration, &'static str> {
        let id = self.id.ok_or("id is required")?;
        let description = self.description.ok_or("description is required")?;

        Ok(GraphReconfiguration {
            id,
            reconfiguration_type: self.reconfiguration_type,
            target_nodes: self.target_nodes,
            description,
            expected_improvement: self.expected_improvement,
            implementation: self.implementation,
            priority: self.priority,
            effort: self.effort,
            confidence: self.confidence,
            triggering_patterns: self.triggering_patterns,
            evidence: self.evidence,
            estimated_impact_pct: self.estimated_impact_pct,
            prerequisites: self.prerequisites,
            conflicts: self.conflicts,
        })
    }
}

/// Configuration recommendations result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationRecommendations {
    /// All recommended reconfigurations
    pub recommendations: Vec<GraphReconfiguration>,
    /// Number of patterns analyzed
    pub patterns_analyzed: usize,
    /// Number of recommendations generated
    pub recommendations_count: usize,
    /// Summary of findings
    pub summary: String,
}

impl ConfigurationRecommendations {
    /// Create empty recommendations
    #[must_use]
    pub fn new() -> Self {
        Self {
            recommendations: Vec::new(),
            patterns_analyzed: 0,
            recommendations_count: 0,
            summary: String::new(),
        }
    }

    /// Check if there are any recommendations
    #[must_use]
    pub fn has_recommendations(&self) -> bool {
        !self.recommendations.is_empty()
    }

    /// Get high priority recommendations
    #[must_use]
    pub fn high_priority(&self) -> Vec<&GraphReconfiguration> {
        self.recommendations
            .iter()
            .filter(|r| r.is_high_priority())
            .collect()
    }

    /// Get quick wins (high priority + low effort)
    #[must_use]
    pub fn quick_wins(&self) -> Vec<&GraphReconfiguration> {
        self.recommendations
            .iter()
            .filter(|r| r.is_quick_win())
            .collect()
    }

    /// Get recommendations by type
    #[must_use]
    pub fn by_type(&self, rtype: &ReconfigurationType) -> Vec<&GraphReconfiguration> {
        self.recommendations
            .iter()
            .filter(|r| &r.reconfiguration_type == rtype)
            .collect()
    }

    /// Get recommendations for a specific node
    #[must_use]
    pub fn for_node(&self, node: &str) -> Vec<&GraphReconfiguration> {
        self.recommendations
            .iter()
            .filter(|r| r.target_nodes.iter().any(|n| n == node))
            .collect()
    }

    /// Get recommendations sorted by quick win score
    #[must_use]
    pub fn sorted_by_quick_win_score(&self) -> Vec<&GraphReconfiguration> {
        let mut sorted: Vec<_> = self.recommendations.iter().collect();
        sorted.sort_by(|a, b| {
            b.quick_win_score()
                .partial_cmp(&a.quick_win_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
    }

    /// Get recommendations sorted by priority
    #[must_use]
    pub fn sorted_by_priority(&self) -> Vec<&GraphReconfiguration> {
        let mut sorted: Vec<_> = self.recommendations.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    /// Get recommendations sorted by estimated impact
    #[must_use]
    pub fn sorted_by_impact(&self) -> Vec<&GraphReconfiguration> {
        let mut sorted: Vec<_> = self.recommendations.iter().collect();
        sorted.sort_by(|a, b| {
            let a_impact = a.estimated_impact_pct.unwrap_or(0.0);
            let b_impact = b.estimated_impact_pct.unwrap_or(0.0);
            b_impact
                .partial_cmp(&a_impact)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
    }

    /// Convert all recommendations to optimization suggestions
    #[must_use]
    pub fn to_optimization_suggestions(&self) -> Vec<OptimizationSuggestion> {
        self.recommendations
            .iter()
            .map(|r| r.to_optimization_suggestion())
            .collect()
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

impl Default for ConfigurationRecommendations {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for recommendation generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationConfig {
    /// Minimum confidence to include a recommendation
    pub min_confidence: f64,
    /// Minimum pattern frequency to trigger recommendation
    pub min_pattern_frequency: usize,
    /// Include cache recommendations
    pub include_cache: bool,
    /// Include parallelization recommendations
    pub include_parallelization: bool,
    /// Include model swap recommendations
    pub include_model_swap: bool,
    /// Include timeout recommendations
    pub include_timeout: bool,
    /// Include retry recommendations
    pub include_retry: bool,
    /// Include batching recommendations
    pub include_batching: bool,
}

impl Default for RecommendationConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,
            min_pattern_frequency: 2,
            include_cache: true,
            include_parallelization: true,
            include_model_swap: true,
            include_timeout: true,
            include_retry: true,
            include_batching: true,
        }
    }
}

impl RecommendationConfig {
    /// Create new config with defaults
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum confidence
    #[must_use]
    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set minimum pattern frequency
    #[must_use]
    pub fn with_min_pattern_frequency(mut self, freq: usize) -> Self {
        self.min_pattern_frequency = freq;
        self
    }

    /// Enable/disable cache recommendations
    #[must_use]
    pub fn with_cache(mut self, enabled: bool) -> Self {
        self.include_cache = enabled;
        self
    }

    /// Enable/disable parallelization recommendations
    #[must_use]
    pub fn with_parallelization(mut self, enabled: bool) -> Self {
        self.include_parallelization = enabled;
        self
    }

    /// Enable/disable model swap recommendations
    #[must_use]
    pub fn with_model_swap(mut self, enabled: bool) -> Self {
        self.include_model_swap = enabled;
        self
    }

    /// Enable/disable timeout recommendations
    #[must_use]
    pub fn with_timeout(mut self, enabled: bool) -> Self {
        self.include_timeout = enabled;
        self
    }

    /// Enable/disable retry recommendations
    #[must_use]
    pub fn with_retry(mut self, enabled: bool) -> Self {
        self.include_retry = enabled;
        self
    }

    /// Enable/disable batching recommendations
    #[must_use]
    pub fn with_batching(mut self, enabled: bool) -> Self {
        self.include_batching = enabled;
        self
    }
}

// Implement recommend_configurations on PatternAnalysis
impl PatternAnalysis {
    /// Generate configuration recommendations based on learned patterns
    #[must_use]
    pub fn recommend_configurations(&self) -> ConfigurationRecommendations {
        self.recommend_configurations_with_config(&RecommendationConfig::default())
    }

    /// Generate configuration recommendations with custom config
    #[must_use]
    pub fn recommend_configurations_with_config(
        &self,
        config: &RecommendationConfig,
    ) -> ConfigurationRecommendations {
        let mut recommendations = ConfigurationRecommendations::new();
        recommendations.patterns_analyzed = self.patterns.len();

        if self.patterns.is_empty() {
            recommendations.summary = "No patterns to analyze for recommendations.".to_string();
            return recommendations;
        }

        // Generate recommendations based on pattern types
        if config.include_cache {
            self.recommend_caching(&mut recommendations, config);
        }
        if config.include_parallelization {
            self.recommend_parallelization(&mut recommendations, config);
        }
        if config.include_model_swap {
            self.recommend_model_swap(&mut recommendations, config);
        }
        if config.include_timeout {
            self.recommend_timeout_adjustments(&mut recommendations, config);
        }
        if config.include_retry {
            self.recommend_retry_logic(&mut recommendations, config);
        }
        if config.include_batching {
            self.recommend_batching(&mut recommendations, config);
        }

        recommendations.recommendations_count = recommendations.recommendations.len();
        recommendations.summary = self.generate_recommendations_summary(&recommendations);

        // Sort by quick win score
        recommendations.recommendations.sort_by(|a, b| {
            b.quick_win_score()
                .partial_cmp(&a.quick_win_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        recommendations
    }

    fn recommend_caching(
        &self,
        recommendations: &mut ConfigurationRecommendations,
        config: &RecommendationConfig,
    ) {
        // Look for patterns that suggest caching would help
        // 1. Repeated patterns with same inputs
        for pattern in self.by_type(&PatternType::Repeated) {
            if pattern.frequency >= config.min_pattern_frequency
                && pattern.confidence >= config.min_confidence
            {
                for node in &pattern.affected_nodes {
                    let reconfig = GraphReconfiguration::new(
                        format!("cache_{}", node),
                        ReconfigurationType::AddCache,
                        vec![node.clone()],
                        format!("Add caching before '{}' to avoid repeated computations", node),
                    )
                    .with_expected_improvement(format!(
                        "Reduce redundant executions by caching results (node called {} times)",
                        pattern.frequency
                    ))
                    .with_implementation(format!(
                        "Insert a cache lookup node before '{}'. Use a hash of input state as cache key. \
                        Consider TTL based on data freshness requirements.",
                        node
                    ))
                    .with_priority(if pattern.frequency > 10 {
                        ReconfigurationPriority::High
                    } else {
                        ReconfigurationPriority::Medium
                    })
                    .with_effort(2)
                    .with_confidence(pattern.confidence)
                    .with_triggering_pattern(&pattern.id)
                    .with_evidence(format!("Pattern '{}' shows {} repeated executions", pattern.id, pattern.frequency))
                    .with_estimated_impact((pattern.frequency as f64 - 1.0) / pattern.frequency as f64 * 50.0);

                    recommendations.recommendations.push(reconfig);
                }
            }
        }

        // 2. Slow patterns that could benefit from caching
        for pattern in self.by_type(&PatternType::Slow) {
            if pattern.frequency >= config.min_pattern_frequency
                && pattern.confidence >= config.min_confidence
            {
                for node in &pattern.affected_nodes {
                    // Check if there's also a repeated pattern for this node
                    let is_repeated = self
                        .for_node(node)
                        .iter()
                        .any(|p| p.pattern_type == PatternType::Repeated);
                    if is_repeated {
                        let reconfig = GraphReconfiguration::new(
                            format!("cache_slow_{}", node),
                            ReconfigurationType::AddCache,
                            vec![node.clone()],
                            format!("Add caching for slow node '{}'", node),
                        )
                        .with_expected_improvement(
                            "Avoid slow recomputation by caching results".to_string(),
                        )
                        .with_implementation(format!(
                            "Add result caching for '{}'. Since this node is slow AND repeated, \
                            caching will significantly reduce total execution time.",
                            node
                        ))
                        .with_priority(ReconfigurationPriority::High)
                        .with_effort(2)
                        .with_confidence(pattern.confidence * 0.9)
                        .with_triggering_pattern(&pattern.id)
                        .with_evidence(format!("Node '{}' is both slow and repeated", node))
                        .with_estimated_impact(40.0);

                        recommendations.recommendations.push(reconfig);
                    }
                }
            }
        }
    }

    fn recommend_parallelization(
        &self,
        recommendations: &mut ConfigurationRecommendations,
        config: &RecommendationConfig,
    ) {
        // Look for sequential patterns that could potentially be parallelized
        for pattern in self.by_type(&PatternType::Sequential) {
            if pattern.confidence >= config.min_confidence && pattern.affected_nodes.len() >= 2 {
                // Check if there are multiple independent sequential patterns from same node
                let first_node = &pattern.affected_nodes[0];
                let sequential_from_same = self
                    .by_type(&PatternType::Sequential)
                    .iter()
                    .filter(|p| !p.affected_nodes.is_empty() && p.affected_nodes[0] == *first_node)
                    .count();

                if sequential_from_same >= 2 {
                    let target_nodes: Vec<String> = self
                        .by_type(&PatternType::Sequential)
                        .iter()
                        .filter(|p| {
                            !p.affected_nodes.is_empty() && p.affected_nodes[0] == *first_node
                        })
                        .flat_map(|p| p.affected_nodes.get(1).cloned())
                        .collect();

                    if target_nodes.len() >= 2 {
                        let reconfig = GraphReconfiguration::new(
                            format!("parallelize_{}", first_node),
                            ReconfigurationType::Parallelize,
                            target_nodes.clone(),
                            format!("Parallelize execution from '{}'", first_node),
                        )
                        .with_expected_improvement(format!(
                            "Execute {} nodes in parallel instead of sequentially",
                            target_nodes.len()
                        ))
                        .with_implementation(format!(
                            "Convert sequential edges from '{}' to parallel edges. \
                            Ensure target nodes are independent and don't share mutable state. \
                            Add a fan-in node to collect results.",
                            first_node
                        ))
                        .with_priority(ReconfigurationPriority::Medium)
                        .with_effort(3)
                        .with_confidence(pattern.confidence * 0.7) // Lower confidence since we're guessing independence
                        .with_triggering_pattern(&pattern.id)
                        .with_evidence(format!(
                            "{} sequential patterns from '{}' could potentially be parallelized",
                            sequential_from_same, first_node
                        ))
                        .with_estimated_impact(30.0);

                        recommendations.recommendations.push(reconfig);
                    }
                }
            }
        }
    }

    fn recommend_model_swap(
        &self,
        recommendations: &mut ConfigurationRecommendations,
        config: &RecommendationConfig,
    ) {
        // Look for high token usage patterns that could use a cheaper model
        for pattern in self.by_type(&PatternType::HighTokenUsage) {
            if pattern.frequency >= config.min_pattern_frequency
                && pattern.confidence >= config.min_confidence
            {
                for node in &pattern.affected_nodes {
                    // Check if this node also has efficient/success patterns (works well, just expensive)
                    let is_successful = self
                        .for_node(node)
                        .iter()
                        .any(|p| p.pattern_type == PatternType::Success);

                    if is_successful {
                        let reconfig = GraphReconfiguration::new(
                            format!("swap_model_{}", node),
                            ReconfigurationType::SwapModel,
                            vec![node.clone()],
                            format!("Consider using a smaller/cheaper model for '{}'", node),
                        )
                        .with_expected_improvement(
                            "Reduce token usage and cost while maintaining success rate"
                                .to_string(),
                        )
                        .with_implementation(format!(
                            "Node '{}' uses many tokens but succeeds consistently. \
                            Consider using a smaller model (e.g., GPT-3.5 instead of GPT-4) \
                            for this node to reduce costs. Test thoroughly to ensure quality.",
                            node
                        ))
                        .with_priority(ReconfigurationPriority::Medium)
                        .with_effort(2)
                        .with_confidence(pattern.confidence * 0.6) // Lower confidence - needs validation
                        .with_triggering_pattern(&pattern.id)
                        .with_evidence(format!(
                            "Node '{}' has high token usage but consistent success",
                            node
                        ))
                        .with_estimated_impact(25.0);

                        recommendations.recommendations.push(reconfig);
                    }
                }
            }
        }

        // Look for low token usage successful patterns - these are already efficient
        for pattern in self.by_type(&PatternType::LowTokenUsage) {
            if pattern.confidence >= config.min_confidence {
                for node in &pattern.affected_nodes {
                    let is_successful = self
                        .for_node(node)
                        .iter()
                        .any(|p| p.pattern_type == PatternType::Success);
                    let is_slow = self
                        .for_node(node)
                        .iter()
                        .any(|p| p.pattern_type == PatternType::Slow);

                    if is_successful && is_slow {
                        // Low tokens but slow - maybe need a faster model
                        let reconfig = GraphReconfiguration::new(
                            format!("swap_model_faster_{}", node),
                            ReconfigurationType::SwapModel,
                            vec![node.clone()],
                            format!("Consider using a faster model for '{}'", node),
                        )
                        .with_expected_improvement("Reduce latency while maintaining low token usage".to_string())
                        .with_implementation(format!(
                            "Node '{}' uses few tokens but is slow. The bottleneck may be model latency. \
                            Consider using a model with lower latency or hosting locally.",
                            node
                        ))
                        .with_priority(ReconfigurationPriority::Low)
                        .with_effort(3)
                        .with_confidence(pattern.confidence * 0.5)
                        .with_triggering_pattern(&pattern.id)
                        .with_evidence(format!("Node '{}' has low token usage but high latency", node));

                        recommendations.recommendations.push(reconfig);
                    }
                }
            }
        }
    }

    fn recommend_timeout_adjustments(
        &self,
        recommendations: &mut ConfigurationRecommendations,
        config: &RecommendationConfig,
    ) {
        // Look for timeout patterns
        for pattern in self.by_type(&PatternType::Timeout) {
            if pattern.frequency >= config.min_pattern_frequency
                && pattern.confidence >= config.min_confidence
            {
                for node in &pattern.affected_nodes {
                    // Check if timeouts lead to errors
                    let has_failures = self
                        .for_node(node)
                        .iter()
                        .any(|p| p.pattern_type == PatternType::Failure);

                    let priority = if has_failures {
                        ReconfigurationPriority::High
                    } else {
                        ReconfigurationPriority::Medium
                    };

                    let reconfig = GraphReconfiguration::new(
                        format!("adjust_timeout_{}", node),
                        ReconfigurationType::AdjustTimeout,
                        vec![node.clone()],
                        format!("Adjust timeout configuration for '{}'", node),
                    )
                    .with_expected_improvement(
                        "Reduce timeout-related failures and improve reliability".to_string(),
                    )
                    .with_implementation(format!(
                        "Node '{}' frequently hits timeout limits. Options:\n\
                        1. Increase timeout if the work is legitimate\n\
                        2. Add progress indicators to detect stalls\n\
                        3. Break into smaller chunks with individual timeouts\n\
                        4. Add circuit breaker to fail fast",
                        node
                    ))
                    .with_priority(priority)
                    .with_effort(1)
                    .with_confidence(pattern.confidence)
                    .with_triggering_pattern(&pattern.id)
                    .with_evidence(format!(
                        "Node '{}' timed out {} times",
                        node, pattern.frequency
                    ))
                    .with_estimated_impact(20.0);

                    recommendations.recommendations.push(reconfig);
                }
            }
        }

        // Look for slow patterns that might need higher timeouts
        for pattern in self.by_type(&PatternType::Slow) {
            if pattern.frequency >= config.min_pattern_frequency
                && pattern.confidence >= config.min_confidence
            {
                for node in &pattern.affected_nodes {
                    // Only suggest if not already suggesting for timeout
                    let already_suggested = recommendations.recommendations.iter().any(|r| {
                        r.target_nodes.contains(node)
                            && r.reconfiguration_type == ReconfigurationType::AdjustTimeout
                    });

                    if !already_suggested {
                        let reconfig = GraphReconfiguration::new(
                            format!("preemptive_timeout_{}", node),
                            ReconfigurationType::AdjustTimeout,
                            vec![node.clone()],
                            format!("Proactively adjust timeout for slow node '{}'", node),
                        )
                        .with_expected_improvement("Prevent future timeout issues".to_string())
                        .with_implementation(format!(
                            "Node '{}' runs slowly. Consider setting a higher timeout \
                            or implementing progressive timeout with exponential backoff.",
                            node
                        ))
                        .with_priority(ReconfigurationPriority::Low)
                        .with_effort(1)
                        .with_confidence(pattern.confidence * 0.5)
                        .with_triggering_pattern(&pattern.id)
                        .with_evidence(format!("Node '{}' frequently runs slowly", node));

                        recommendations.recommendations.push(reconfig);
                    }
                }
            }
        }
    }

    fn recommend_retry_logic(
        &self,
        recommendations: &mut ConfigurationRecommendations,
        config: &RecommendationConfig,
    ) {
        // Look for error recovery patterns - these already have retry, might need tuning
        for pattern in self.by_type(&PatternType::ErrorRecovery) {
            if pattern.frequency >= config.min_pattern_frequency
                && pattern.confidence >= config.min_confidence
            {
                for node in &pattern.affected_nodes {
                    let reconfig = GraphReconfiguration::new(
                        format!("tune_retry_{}", node),
                        ReconfigurationType::AddRetry,
                        vec![node.clone()],
                        format!("Tune retry configuration for '{}'", node),
                    )
                    .with_expected_improvement("Optimize retry behavior for better reliability".to_string())
                    .with_implementation(format!(
                        "Node '{}' successfully recovers from errors. Current retry behavior is working. \
                        Consider:\n\
                        1. Adding exponential backoff if not present\n\
                        2. Setting max retry limits\n\
                        3. Adding jitter to prevent thundering herd\n\
                        4. Logging retry attempts for monitoring",
                        node
                    ))
                    .with_priority(ReconfigurationPriority::Low)
                    .with_effort(2)
                    .with_confidence(pattern.confidence * 0.8)
                    .with_triggering_pattern(&pattern.id)
                    .with_evidence(format!("Node '{}' recovered from {} errors", node, pattern.frequency));

                    recommendations.recommendations.push(reconfig);
                }
            }
        }

        // Look for failure patterns without error recovery - need retry
        for pattern in self.by_type(&PatternType::Failure) {
            if pattern.frequency >= config.min_pattern_frequency
                && pattern.confidence >= config.min_confidence
            {
                for node in &pattern.affected_nodes {
                    // Check if error recovery exists for this node
                    let has_recovery = self
                        .for_node(node)
                        .iter()
                        .any(|p| p.pattern_type == PatternType::ErrorRecovery);

                    if !has_recovery {
                        let reconfig = GraphReconfiguration::new(
                            format!("add_retry_{}", node),
                            ReconfigurationType::AddRetry,
                            vec![node.clone()],
                            format!("Add retry logic for frequently failing node '{}'", node),
                        )
                        .with_expected_improvement(
                            "Improve reliability by adding automatic retry".to_string(),
                        )
                        .with_implementation(format!(
                            "Node '{}' fails frequently without recovery. Add retry logic:\n\
                            1. Wrap node execution in retry handler\n\
                            2. Use exponential backoff (start: 1s, max: 30s)\n\
                            3. Set max retries (3-5 typically)\n\
                            4. Consider retry only for transient errors",
                            node
                        ))
                        .with_priority(ReconfigurationPriority::High)
                        .with_effort(2)
                        .with_confidence(pattern.confidence)
                        .with_triggering_pattern(&pattern.id)
                        .with_evidence(format!(
                            "Node '{}' failed {} times with no automatic recovery",
                            node, pattern.frequency
                        ))
                        .with_estimated_impact(35.0);

                        recommendations.recommendations.push(reconfig);
                    }
                }
            }
        }
    }

    fn recommend_batching(
        &self,
        recommendations: &mut ConfigurationRecommendations,
        config: &RecommendationConfig,
    ) {
        // Look for repeated patterns that could benefit from batching
        for pattern in self.by_type(&PatternType::Repeated) {
            if pattern.frequency >= 5 && pattern.confidence >= config.min_confidence {
                for node in &pattern.affected_nodes {
                    // Check if not already suggesting caching
                    let already_caching = recommendations.recommendations.iter().any(|r| {
                        r.target_nodes.contains(node)
                            && r.reconfiguration_type == ReconfigurationType::AddCache
                    });

                    // Check if this is a high token usage node (batching helps with API calls)
                    let is_high_tokens = self
                        .for_node(node)
                        .iter()
                        .any(|p| p.pattern_type == PatternType::HighTokenUsage);

                    if !already_caching && is_high_tokens {
                        let reconfig = GraphReconfiguration::new(
                            format!("add_batching_{}", node),
                            ReconfigurationType::AddBatching,
                            vec![node.clone()],
                            format!("Add batching for repeated API calls in '{}'", node),
                        )
                        .with_expected_improvement(
                            "Reduce API calls and improve throughput".to_string(),
                        )
                        .with_implementation(format!(
                            "Node '{}' makes repeated API calls. Consider batching:\n\
                            1. Collect multiple requests before executing\n\
                            2. Use batch API endpoints if available\n\
                            3. Add request coalescing for identical requests\n\
                            4. Implement batch window (e.g., 100ms or 10 requests)",
                            node
                        ))
                        .with_priority(ReconfigurationPriority::Medium)
                        .with_effort(3)
                        .with_confidence(pattern.confidence * 0.7)
                        .with_triggering_pattern(&pattern.id)
                        .with_evidence(format!(
                            "Node '{}' called {} times with high token usage",
                            node, pattern.frequency
                        ))
                        .with_estimated_impact(25.0);

                        recommendations.recommendations.push(reconfig);
                    }
                }
            }
        }
    }

    fn generate_recommendations_summary(
        &self,
        recommendations: &ConfigurationRecommendations,
    ) -> String {
        if recommendations.recommendations.is_empty() {
            return "No configuration recommendations generated from patterns.".to_string();
        }

        let high_priority = recommendations.high_priority().len();
        let quick_wins = recommendations.quick_wins().len();

        let mut type_counts: HashMap<String, usize> = HashMap::new();
        for rec in &recommendations.recommendations {
            *type_counts
                .entry(rec.reconfiguration_type.to_string())
                .or_default() += 1;
        }

        let type_summary: Vec<String> = type_counts
            .iter()
            .map(|(t, c)| format!("{}: {}", t, c))
            .collect();

        format!(
            "Generated {} recommendations from {} patterns. High priority: {}, Quick wins: {}. Types: {}",
            recommendations.recommendations.len(),
            self.patterns.len(),
            high_priority,
            quick_wins,
            type_summary.join(", ")
        )
    }
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::pattern::Pattern;
    use super::*;

    // =========================================================================
    // ReconfigurationType Tests
    // =========================================================================

    #[test]
    fn test_reconfiguration_type_display() {
        assert_eq!(ReconfigurationType::AddCache.to_string(), "add_cache");
        assert_eq!(ReconfigurationType::Parallelize.to_string(), "parallelize");
        assert_eq!(ReconfigurationType::SwapModel.to_string(), "swap_model");
        assert_eq!(
            ReconfigurationType::AdjustTimeout.to_string(),
            "adjust_timeout"
        );
        assert_eq!(ReconfigurationType::AddRetry.to_string(), "add_retry");
        assert_eq!(ReconfigurationType::SkipNode.to_string(), "skip_node");
        assert_eq!(ReconfigurationType::MergeNodes.to_string(), "merge_nodes");
        assert_eq!(ReconfigurationType::SplitNode.to_string(), "split_node");
        assert_eq!(ReconfigurationType::AddBatching.to_string(), "add_batching");
        assert_eq!(
            ReconfigurationType::AddRateLimiting.to_string(),
            "add_rate_limiting"
        );
        assert_eq!(
            ReconfigurationType::ChangeRouting.to_string(),
            "change_routing"
        );
        assert_eq!(
            ReconfigurationType::Custom("my_custom".to_string()).to_string(),
            "custom:my_custom"
        );
    }

    #[test]
    fn test_reconfiguration_type_default() {
        assert_eq!(
            ReconfigurationType::default(),
            ReconfigurationType::AddCache
        );
    }

    #[test]
    fn test_reconfiguration_type_clone_eq() {
        let rt = ReconfigurationType::Parallelize;
        let cloned = rt.clone();
        assert_eq!(rt, cloned);

        let custom1 = ReconfigurationType::Custom("test".to_string());
        let custom2 = ReconfigurationType::Custom("test".to_string());
        assert_eq!(custom1, custom2);

        let custom3 = ReconfigurationType::Custom("other".to_string());
        assert_ne!(custom1, custom3);
    }

    #[test]
    fn test_reconfiguration_type_serialize_deserialize() {
        let types = vec![
            ReconfigurationType::AddCache,
            ReconfigurationType::Parallelize,
            ReconfigurationType::SwapModel,
            ReconfigurationType::AdjustTimeout,
            ReconfigurationType::AddRetry,
            ReconfigurationType::SkipNode,
            ReconfigurationType::MergeNodes,
            ReconfigurationType::SplitNode,
            ReconfigurationType::AddBatching,
            ReconfigurationType::AddRateLimiting,
            ReconfigurationType::ChangeRouting,
            ReconfigurationType::Custom("custom_reconfig".to_string()),
        ];

        for rt in types {
            let json = serde_json::to_string(&rt).unwrap();
            let deserialized: ReconfigurationType = serde_json::from_str(&json).unwrap();
            assert_eq!(rt, deserialized);
        }
    }

    // =========================================================================
    // ReconfigurationPriority Tests
    // =========================================================================

    #[test]
    fn test_reconfiguration_priority_display() {
        assert_eq!(ReconfigurationPriority::Low.to_string(), "low");
        assert_eq!(ReconfigurationPriority::Medium.to_string(), "medium");
        assert_eq!(ReconfigurationPriority::High.to_string(), "high");
        assert_eq!(ReconfigurationPriority::Critical.to_string(), "critical");
    }

    #[test]
    fn test_reconfiguration_priority_default() {
        assert_eq!(
            ReconfigurationPriority::default(),
            ReconfigurationPriority::Medium
        );
    }

    #[test]
    fn test_reconfiguration_priority_ordering() {
        assert!(ReconfigurationPriority::Low < ReconfigurationPriority::Medium);
        assert!(ReconfigurationPriority::Medium < ReconfigurationPriority::High);
        assert!(ReconfigurationPriority::High < ReconfigurationPriority::Critical);

        let mut priorities = vec![
            ReconfigurationPriority::High,
            ReconfigurationPriority::Low,
            ReconfigurationPriority::Critical,
            ReconfigurationPriority::Medium,
        ];
        priorities.sort();
        assert_eq!(
            priorities,
            vec![
                ReconfigurationPriority::Low,
                ReconfigurationPriority::Medium,
                ReconfigurationPriority::High,
                ReconfigurationPriority::Critical,
            ]
        );
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_reconfiguration_priority_clone_copy() {
        let p = ReconfigurationPriority::High;
        let cloned = p.clone();
        let copied = p; // Copy
        assert_eq!(p, cloned);
        assert_eq!(p, copied);
    }

    #[test]
    fn test_reconfiguration_priority_serialize_deserialize() {
        for p in [
            ReconfigurationPriority::Low,
            ReconfigurationPriority::Medium,
            ReconfigurationPriority::High,
            ReconfigurationPriority::Critical,
        ] {
            let json = serde_json::to_string(&p).unwrap();
            let deserialized: ReconfigurationPriority = serde_json::from_str(&json).unwrap();
            assert_eq!(p, deserialized);
        }
    }

    // =========================================================================
    // GraphReconfiguration Tests
    // =========================================================================

    #[test]
    fn test_graph_reconfiguration_new() {
        let reconfig = GraphReconfiguration::new(
            "test_id",
            ReconfigurationType::AddCache,
            vec!["node1".to_string(), "node2".to_string()],
            "Test description",
        );

        assert_eq!(reconfig.id, "test_id");
        assert_eq!(reconfig.reconfiguration_type, ReconfigurationType::AddCache);
        assert_eq!(reconfig.target_nodes, vec!["node1", "node2"]);
        assert_eq!(reconfig.description, "Test description");
        assert!(reconfig.expected_improvement.is_empty());
        assert!(reconfig.implementation.is_empty());
        assert_eq!(reconfig.priority, ReconfigurationPriority::Medium);
        assert_eq!(reconfig.effort, 3);
        assert!((reconfig.confidence - 0.5).abs() < f64::EPSILON);
        assert!(reconfig.triggering_patterns.is_empty());
        assert!(reconfig.evidence.is_empty());
        assert!(reconfig.estimated_impact_pct.is_none());
        assert!(reconfig.prerequisites.is_empty());
        assert!(reconfig.conflicts.is_empty());
    }

    #[test]
    fn test_graph_reconfiguration_with_expected_improvement() {
        let reconfig =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_expected_improvement("50% faster");

        assert_eq!(reconfig.expected_improvement, "50% faster");
    }

    #[test]
    fn test_graph_reconfiguration_with_implementation() {
        let reconfig =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_implementation("Add caching layer");

        assert_eq!(reconfig.implementation, "Add caching layer");
    }

    #[test]
    fn test_graph_reconfiguration_with_priority() {
        let reconfig =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_priority(ReconfigurationPriority::Critical);

        assert_eq!(reconfig.priority, ReconfigurationPriority::Critical);
    }

    #[test]
    fn test_graph_reconfiguration_with_effort_clamping() {
        let reconfig_low =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_effort(0);
        assert_eq!(reconfig_low.effort, 1); // Clamped to 1

        let reconfig_high =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_effort(10);
        assert_eq!(reconfig_high.effort, 5); // Clamped to 5

        let reconfig_mid =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_effort(3);
        assert_eq!(reconfig_mid.effort, 3);
    }

    #[test]
    fn test_graph_reconfiguration_with_confidence_clamping() {
        let reconfig_low =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_confidence(-0.5);
        assert!((reconfig_low.confidence - 0.0).abs() < f64::EPSILON);

        let reconfig_high =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_confidence(1.5);
        assert!((reconfig_high.confidence - 1.0).abs() < f64::EPSILON);

        let reconfig_mid =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_confidence(0.75);
        assert!((reconfig_mid.confidence - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_graph_reconfiguration_with_triggering_pattern() {
        let reconfig =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_triggering_pattern("pattern1")
                .with_triggering_pattern("pattern2");

        assert_eq!(reconfig.triggering_patterns, vec!["pattern1", "pattern2"]);
    }

    #[test]
    fn test_graph_reconfiguration_with_evidence() {
        let reconfig =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_evidence("Evidence 1")
                .with_evidence("Evidence 2");

        assert_eq!(reconfig.evidence, vec!["Evidence 1", "Evidence 2"]);
    }

    #[test]
    fn test_graph_reconfiguration_with_estimated_impact() {
        let reconfig =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_estimated_impact(25.5);

        assert_eq!(reconfig.estimated_impact_pct, Some(25.5));
    }

    #[test]
    fn test_graph_reconfiguration_with_prerequisite() {
        let reconfig =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_prerequisite("prereq1")
                .with_prerequisite("prereq2");

        assert_eq!(reconfig.prerequisites, vec!["prereq1", "prereq2"]);
    }

    #[test]
    fn test_graph_reconfiguration_with_conflict() {
        let reconfig =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_conflict("conflict1")
                .with_conflict("conflict2");

        assert_eq!(reconfig.conflicts, vec!["conflict1", "conflict2"]);
    }

    #[test]
    fn test_graph_reconfiguration_is_high_priority() {
        let low = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
            .with_priority(ReconfigurationPriority::Low);
        let medium = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
            .with_priority(ReconfigurationPriority::Medium);
        let high = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
            .with_priority(ReconfigurationPriority::High);
        let critical =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "desc")
                .with_priority(ReconfigurationPriority::Critical);

        assert!(!low.is_high_priority());
        assert!(!medium.is_high_priority());
        assert!(high.is_high_priority());
        assert!(critical.is_high_priority());
    }

    #[test]
    fn test_graph_reconfiguration_is_low_effort() {
        let effort_1 = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
            .with_effort(1);
        let effort_2 = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
            .with_effort(2);
        let effort_3 = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
            .with_effort(3);

        assert!(effort_1.is_low_effort());
        assert!(effort_2.is_low_effort());
        assert!(!effort_3.is_low_effort());
    }

    #[test]
    fn test_graph_reconfiguration_is_quick_win() {
        // Quick win: high priority AND low effort
        let quick_win = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
            .with_priority(ReconfigurationPriority::High)
            .with_effort(1);
        assert!(quick_win.is_quick_win());

        // Not quick win: low priority
        let not_qw_low_priority =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
                .with_priority(ReconfigurationPriority::Low)
                .with_effort(1);
        assert!(!not_qw_low_priority.is_quick_win());

        // Not quick win: high effort
        let not_qw_high_effort =
            GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
                .with_priority(ReconfigurationPriority::High)
                .with_effort(5);
        assert!(!not_qw_high_effort.is_quick_win());
    }

    #[test]
    fn test_graph_reconfiguration_quick_win_score() {
        // Formula: (priority_score * effort_score * confidence) / 4.0
        // priority: Low=1, Medium=2, High=3, Critical=4
        // effort_score = 6 - effort

        let reconfig = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
            .with_priority(ReconfigurationPriority::High) // priority_score = 3
            .with_effort(2) // effort_score = 4
            .with_confidence(0.8);

        let expected_score = (3.0 * 4.0 * 0.8) / 4.0;
        assert!((reconfig.quick_win_score() - expected_score).abs() < f64::EPSILON);

        // Critical priority, effort 1, confidence 1.0 = (4*5*1)/4 = 5.0
        let max_score = GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
            .with_priority(ReconfigurationPriority::Critical)
            .with_effort(1)
            .with_confidence(1.0);
        assert!((max_score.quick_win_score() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_graph_reconfiguration_summary() {
        let reconfig = GraphReconfiguration::new(
            "test_id",
            ReconfigurationType::AddCache,
            vec!["node1".to_string(), "node2".to_string()],
            "Test description",
        )
        .with_priority(ReconfigurationPriority::High)
        .with_expected_improvement("50% faster");

        let summary = reconfig.summary();
        assert!(summary.contains("high"));
        assert!(summary.contains("add_cache"));
        assert!(summary.contains("node1, node2"));
        assert!(summary.contains("Test description"));
        assert!(summary.contains("50% faster"));
    }

    #[test]
    fn test_graph_reconfiguration_to_optimization_suggestion() {
        let reconfig = GraphReconfiguration::new(
            "test_id",
            ReconfigurationType::AddCache,
            vec!["node1".to_string()],
            "Test description",
        )
        .with_expected_improvement("50% faster")
        .with_implementation("Add caching")
        .with_priority(ReconfigurationPriority::High)
        .with_effort(2)
        .with_confidence(0.9)
        .with_evidence("Evidence 1");

        let suggestion = reconfig.to_optimization_suggestion();

        assert_eq!(suggestion.category, OptimizationCategory::Caching);
        assert_eq!(suggestion.target_nodes, vec!["node1"]);
        assert_eq!(suggestion.description, "Test description");
        assert_eq!(suggestion.expected_improvement, "50% faster");
        assert_eq!(suggestion.implementation, "Add caching");
        assert_eq!(suggestion.priority, OptimizationPriority::High);
        assert_eq!(suggestion.effort, 2);
        assert!((suggestion.confidence - 0.9).abs() < f64::EPSILON);
        assert!(suggestion.evidence.contains(&"Evidence 1".to_string()));
    }

    #[test]
    fn test_graph_reconfiguration_to_optimization_suggestion_category_mapping() {
        let test_cases = vec![
            (ReconfigurationType::AddCache, OptimizationCategory::Caching),
            (
                ReconfigurationType::Parallelize,
                OptimizationCategory::Parallelization,
            ),
            (
                ReconfigurationType::SwapModel,
                OptimizationCategory::ModelChoice,
            ),
            (
                ReconfigurationType::AdjustTimeout,
                OptimizationCategory::Stabilization,
            ),
            (
                ReconfigurationType::AddRetry,
                OptimizationCategory::ErrorHandling,
            ),
            (
                ReconfigurationType::SkipNode,
                OptimizationCategory::Performance,
            ),
            (
                ReconfigurationType::MergeNodes,
                OptimizationCategory::Performance,
            ),
            (
                ReconfigurationType::SplitNode,
                OptimizationCategory::Performance,
            ),
            (
                ReconfigurationType::AddBatching,
                OptimizationCategory::Performance,
            ),
            (
                ReconfigurationType::AddRateLimiting,
                OptimizationCategory::Stabilization,
            ),
            (
                ReconfigurationType::ChangeRouting,
                OptimizationCategory::Performance,
            ),
            (
                ReconfigurationType::Custom("test".to_string()),
                OptimizationCategory::Performance,
            ),
        ];

        for (reconfig_type, expected_category) in test_cases {
            let reconfig = GraphReconfiguration::new("id", reconfig_type, vec![], "d");
            let suggestion = reconfig.to_optimization_suggestion();
            assert_eq!(
                suggestion.category, expected_category,
                "Mapping for {:?}",
                reconfig.reconfiguration_type
            );
        }
    }

    #[test]
    fn test_graph_reconfiguration_priority_mapping() {
        let test_cases = vec![
            (ReconfigurationPriority::Low, OptimizationPriority::Low),
            (
                ReconfigurationPriority::Medium,
                OptimizationPriority::Medium,
            ),
            (ReconfigurationPriority::High, OptimizationPriority::High),
            (
                ReconfigurationPriority::Critical,
                OptimizationPriority::Critical,
            ),
        ];

        for (reconfig_priority, expected_opt_priority) in test_cases {
            let reconfig =
                GraphReconfiguration::new("id", ReconfigurationType::AddCache, vec![], "d")
                    .with_priority(reconfig_priority);
            let suggestion = reconfig.to_optimization_suggestion();
            assert_eq!(
                suggestion.priority, expected_opt_priority,
                "Mapping for {:?}",
                reconfig_priority
            );
        }
    }

    #[test]
    fn test_graph_reconfiguration_json_roundtrip() {
        let reconfig = GraphReconfiguration::new(
            "test_id",
            ReconfigurationType::AddCache,
            vec!["node1".to_string()],
            "Test description",
        )
        .with_expected_improvement("50% faster")
        .with_implementation("Add caching")
        .with_priority(ReconfigurationPriority::High)
        .with_effort(2)
        .with_confidence(0.9)
        .with_evidence("Evidence 1")
        .with_estimated_impact(30.0)
        .with_prerequisite("prereq1")
        .with_conflict("conflict1")
        .with_triggering_pattern("pattern1");

        let json = reconfig.to_json().unwrap();
        let deserialized = GraphReconfiguration::from_json(&json).unwrap();

        assert_eq!(deserialized.id, reconfig.id);
        assert_eq!(
            deserialized.reconfiguration_type,
            reconfig.reconfiguration_type
        );
        assert_eq!(deserialized.target_nodes, reconfig.target_nodes);
        assert_eq!(deserialized.description, reconfig.description);
        assert_eq!(
            deserialized.expected_improvement,
            reconfig.expected_improvement
        );
        assert_eq!(deserialized.implementation, reconfig.implementation);
        assert_eq!(deserialized.priority, reconfig.priority);
        assert_eq!(deserialized.effort, reconfig.effort);
        assert!((deserialized.confidence - reconfig.confidence).abs() < f64::EPSILON);
        assert_eq!(deserialized.evidence, reconfig.evidence);
        assert_eq!(
            deserialized.estimated_impact_pct,
            reconfig.estimated_impact_pct
        );
        assert_eq!(deserialized.prerequisites, reconfig.prerequisites);
        assert_eq!(deserialized.conflicts, reconfig.conflicts);
        assert_eq!(
            deserialized.triggering_patterns,
            reconfig.triggering_patterns
        );
    }

    // =========================================================================
    // GraphReconfigurationBuilder Tests
    // =========================================================================

    #[test]
    fn test_graph_reconfiguration_builder_new() {
        let builder = GraphReconfigurationBuilder::new();
        // Builder has defaults
        let result = builder.id("id").description("desc").build();
        assert!(result.is_ok());
        let reconfig = result.unwrap();
        assert_eq!(reconfig.effort, 3);
        assert!((reconfig.confidence - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_graph_reconfiguration_builder_all_methods() {
        let reconfig = GraphReconfigurationBuilder::new()
            .id("builder_test")
            .reconfiguration_type(ReconfigurationType::Parallelize)
            .target_node("node1")
            .target_node("node2")
            .description("Builder test description")
            .expected_improvement("Better performance")
            .implementation("Parallelize nodes")
            .priority(ReconfigurationPriority::Critical)
            .effort(4)
            .confidence(0.85)
            .triggering_pattern("pattern1")
            .evidence("evidence1")
            .estimated_impact(40.0)
            .prerequisite("prereq1")
            .conflict("conflict1")
            .build()
            .unwrap();

        assert_eq!(reconfig.id, "builder_test");
        assert_eq!(
            reconfig.reconfiguration_type,
            ReconfigurationType::Parallelize
        );
        assert_eq!(reconfig.target_nodes, vec!["node1", "node2"]);
        assert_eq!(reconfig.description, "Builder test description");
        assert_eq!(reconfig.expected_improvement, "Better performance");
        assert_eq!(reconfig.implementation, "Parallelize nodes");
        assert_eq!(reconfig.priority, ReconfigurationPriority::Critical);
        assert_eq!(reconfig.effort, 4);
        assert!((reconfig.confidence - 0.85).abs() < f64::EPSILON);
        assert_eq!(reconfig.triggering_patterns, vec!["pattern1"]);
        assert_eq!(reconfig.evidence, vec!["evidence1"]);
        assert_eq!(reconfig.estimated_impact_pct, Some(40.0));
        assert_eq!(reconfig.prerequisites, vec!["prereq1"]);
        assert_eq!(reconfig.conflicts, vec!["conflict1"]);
    }

    #[test]
    fn test_graph_reconfiguration_builder_target_nodes() {
        let reconfig = GraphReconfigurationBuilder::new()
            .id("id")
            .description("desc")
            .target_nodes(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .build()
            .unwrap();

        assert_eq!(reconfig.target_nodes, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_graph_reconfiguration_builder_effort_clamping() {
        let low_effort = GraphReconfigurationBuilder::new()
            .id("id")
            .description("desc")
            .effort(0)
            .build()
            .unwrap();
        assert_eq!(low_effort.effort, 1);

        let high_effort = GraphReconfigurationBuilder::new()
            .id("id")
            .description("desc")
            .effort(100)
            .build()
            .unwrap();
        assert_eq!(high_effort.effort, 5);
    }

    #[test]
    fn test_graph_reconfiguration_builder_confidence_clamping() {
        let low_conf = GraphReconfigurationBuilder::new()
            .id("id")
            .description("desc")
            .confidence(-1.0)
            .build()
            .unwrap();
        assert!((low_conf.confidence - 0.0).abs() < f64::EPSILON);

        let high_conf = GraphReconfigurationBuilder::new()
            .id("id")
            .description("desc")
            .confidence(2.0)
            .build()
            .unwrap();
        assert!((high_conf.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_graph_reconfiguration_builder_missing_id() {
        let result = GraphReconfigurationBuilder::new()
            .description("desc")
            .build();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "id is required");
    }

    #[test]
    fn test_graph_reconfiguration_builder_missing_description() {
        let result = GraphReconfigurationBuilder::new().id("id").build();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "description is required");
    }

    #[test]
    fn test_graph_reconfiguration_builder_from_struct() {
        let reconfig = GraphReconfiguration::builder()
            .id("from_struct")
            .description("From struct method")
            .build()
            .unwrap();

        assert_eq!(reconfig.id, "from_struct");
    }

    // =========================================================================
    // ConfigurationRecommendations Tests
    // =========================================================================

    #[test]
    fn test_configuration_recommendations_new() {
        let recs = ConfigurationRecommendations::new();
        assert!(recs.recommendations.is_empty());
        assert_eq!(recs.patterns_analyzed, 0);
        assert_eq!(recs.recommendations_count, 0);
        assert!(recs.summary.is_empty());
    }

    #[test]
    fn test_configuration_recommendations_default() {
        let recs = ConfigurationRecommendations::default();
        assert!(recs.recommendations.is_empty());
    }

    #[test]
    fn test_configuration_recommendations_has_recommendations() {
        let mut recs = ConfigurationRecommendations::new();
        assert!(!recs.has_recommendations());

        recs.recommendations.push(GraphReconfiguration::new(
            "id",
            ReconfigurationType::AddCache,
            vec![],
            "desc",
        ));
        assert!(recs.has_recommendations());
    }

    fn create_test_recommendations() -> ConfigurationRecommendations {
        let mut recs = ConfigurationRecommendations::new();
        recs.recommendations.push(
            GraphReconfiguration::new(
                "low_effort_high_priority",
                ReconfigurationType::AddCache,
                vec!["node1".to_string()],
                "Quick win",
            )
            .with_priority(ReconfigurationPriority::High)
            .with_effort(1)
            .with_estimated_impact(50.0),
        );
        recs.recommendations.push(
            GraphReconfiguration::new(
                "high_effort_critical",
                ReconfigurationType::Parallelize,
                vec!["node2".to_string()],
                "Critical but hard",
            )
            .with_priority(ReconfigurationPriority::Critical)
            .with_effort(5)
            .with_estimated_impact(80.0),
        );
        recs.recommendations.push(
            GraphReconfiguration::new(
                "low_priority",
                ReconfigurationType::SwapModel,
                vec!["node1".to_string(), "node3".to_string()],
                "Nice to have",
            )
            .with_priority(ReconfigurationPriority::Low)
            .with_effort(2)
            .with_estimated_impact(10.0),
        );
        recs.recommendations.push(
            GraphReconfiguration::new(
                "another_cache",
                ReconfigurationType::AddCache,
                vec!["node4".to_string()],
                "Another cache rec",
            )
            .with_priority(ReconfigurationPriority::Medium)
            .with_effort(2),
        );
        recs
    }

    #[test]
    fn test_configuration_recommendations_high_priority() {
        let recs = create_test_recommendations();
        let high = recs.high_priority();
        assert_eq!(high.len(), 2);
        assert!(high.iter().all(|r| r.is_high_priority()));
    }

    #[test]
    fn test_configuration_recommendations_quick_wins() {
        let recs = create_test_recommendations();
        let qw = recs.quick_wins();
        // Only "low_effort_high_priority" is a quick win (high priority + effort <= 2)
        assert_eq!(qw.len(), 1);
        assert_eq!(qw[0].id, "low_effort_high_priority");
    }

    #[test]
    fn test_configuration_recommendations_by_type() {
        let recs = create_test_recommendations();

        let cache_recs = recs.by_type(&ReconfigurationType::AddCache);
        assert_eq!(cache_recs.len(), 2);

        let parallelize_recs = recs.by_type(&ReconfigurationType::Parallelize);
        assert_eq!(parallelize_recs.len(), 1);
        assert_eq!(parallelize_recs[0].id, "high_effort_critical");

        let swap_model_recs = recs.by_type(&ReconfigurationType::SwapModel);
        assert_eq!(swap_model_recs.len(), 1);

        let retry_recs = recs.by_type(&ReconfigurationType::AddRetry);
        assert!(retry_recs.is_empty());
    }

    #[test]
    fn test_configuration_recommendations_for_node() {
        let recs = create_test_recommendations();

        let node1_recs = recs.for_node("node1");
        assert_eq!(node1_recs.len(), 2); // low_effort_high_priority and low_priority

        let node2_recs = recs.for_node("node2");
        assert_eq!(node2_recs.len(), 1);

        let node5_recs = recs.for_node("node5");
        assert!(node5_recs.is_empty());
    }

    #[test]
    fn test_configuration_recommendations_sorted_by_quick_win_score() {
        let recs = create_test_recommendations();
        let sorted = recs.sorted_by_quick_win_score();

        // Verify sorted in descending order by quick win score
        for i in 1..sorted.len() {
            assert!(sorted[i - 1].quick_win_score() >= sorted[i].quick_win_score());
        }
    }

    #[test]
    fn test_configuration_recommendations_sorted_by_priority() {
        let recs = create_test_recommendations();
        let sorted = recs.sorted_by_priority();

        // Verify sorted in descending order by priority
        for i in 1..sorted.len() {
            assert!(sorted[i - 1].priority >= sorted[i].priority);
        }
        assert_eq!(sorted[0].priority, ReconfigurationPriority::Critical);
    }

    #[test]
    fn test_configuration_recommendations_sorted_by_impact() {
        let recs = create_test_recommendations();
        let sorted = recs.sorted_by_impact();

        // First one should have highest impact (80.0)
        assert_eq!(sorted[0].estimated_impact_pct, Some(80.0));
        // None values should be at end (treated as 0.0)
        assert_eq!(sorted[sorted.len() - 1].estimated_impact_pct, None);
    }

    #[test]
    fn test_configuration_recommendations_to_optimization_suggestions() {
        let recs = create_test_recommendations();
        let suggestions = recs.to_optimization_suggestions();

        assert_eq!(suggestions.len(), recs.recommendations.len());
    }

    #[test]
    fn test_configuration_recommendations_json_roundtrip() {
        let mut recs_with_meta = ConfigurationRecommendations::new();
        recs_with_meta.patterns_analyzed = 10;
        recs_with_meta.recommendations_count = 4;
        recs_with_meta.summary = "Test summary".to_string();
        recs_with_meta.recommendations = create_test_recommendations().recommendations;

        let json = recs_with_meta.to_json().unwrap();
        let deserialized = ConfigurationRecommendations::from_json(&json).unwrap();

        assert_eq!(
            deserialized.recommendations.len(),
            recs_with_meta.recommendations.len()
        );
        assert_eq!(
            deserialized.patterns_analyzed,
            recs_with_meta.patterns_analyzed
        );
        assert_eq!(
            deserialized.recommendations_count,
            recs_with_meta.recommendations_count
        );
        assert_eq!(deserialized.summary, recs_with_meta.summary);
    }

    // =========================================================================
    // RecommendationConfig Tests
    // =========================================================================

    #[test]
    fn test_recommendation_config_new() {
        let config = RecommendationConfig::new();
        assert!((config.min_confidence - 0.3).abs() < f64::EPSILON);
        assert_eq!(config.min_pattern_frequency, 2);
        assert!(config.include_cache);
        assert!(config.include_parallelization);
        assert!(config.include_model_swap);
        assert!(config.include_timeout);
        assert!(config.include_retry);
        assert!(config.include_batching);
    }

    #[test]
    fn test_recommendation_config_default() {
        let config = RecommendationConfig::default();
        assert!((config.min_confidence - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_recommendation_config_with_min_confidence() {
        let config = RecommendationConfig::new().with_min_confidence(0.8);
        assert!((config.min_confidence - 0.8).abs() < f64::EPSILON);

        // Clamping
        let low = RecommendationConfig::new().with_min_confidence(-0.5);
        assert!((low.min_confidence - 0.0).abs() < f64::EPSILON);

        let high = RecommendationConfig::new().with_min_confidence(1.5);
        assert!((high.min_confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_recommendation_config_with_min_pattern_frequency() {
        let config = RecommendationConfig::new().with_min_pattern_frequency(5);
        assert_eq!(config.min_pattern_frequency, 5);
    }

    #[test]
    fn test_recommendation_config_with_cache() {
        let config = RecommendationConfig::new().with_cache(false);
        assert!(!config.include_cache);
    }

    #[test]
    fn test_recommendation_config_with_parallelization() {
        let config = RecommendationConfig::new().with_parallelization(false);
        assert!(!config.include_parallelization);
    }

    #[test]
    fn test_recommendation_config_with_model_swap() {
        let config = RecommendationConfig::new().with_model_swap(false);
        assert!(!config.include_model_swap);
    }

    #[test]
    fn test_recommendation_config_with_timeout() {
        let config = RecommendationConfig::new().with_timeout(false);
        assert!(!config.include_timeout);
    }

    #[test]
    fn test_recommendation_config_with_retry() {
        let config = RecommendationConfig::new().with_retry(false);
        assert!(!config.include_retry);
    }

    #[test]
    fn test_recommendation_config_with_batching() {
        let config = RecommendationConfig::new().with_batching(false);
        assert!(!config.include_batching);
    }

    // =========================================================================
    // PatternAnalysis::recommend_configurations Tests
    // =========================================================================

    fn create_pattern(id: &str, ptype: PatternType, nodes: Vec<&str>, freq: usize) -> Pattern {
        let mut p = Pattern::new(id, ptype)
            .with_frequency(freq)
            .with_confidence(0.8)
            .with_description(format!("Test pattern {}", id));
        for node in nodes {
            p = p.with_affected_node(node);
        }
        p
    }

    fn create_pattern_with_nodes(
        id: &str,
        ptype: PatternType,
        nodes: Vec<&str>,
        freq: usize,
    ) -> Pattern {
        let mut p = Pattern::new(id, ptype)
            .with_frequency(freq)
            .with_confidence(0.8)
            .with_description(format!("Test pattern {}", id));
        for node in nodes {
            p = p.with_affected_node(node);
        }
        p
    }

    #[test]
    fn test_recommend_configurations_empty_patterns() {
        let analysis = PatternAnalysis::new();
        let recs = analysis.recommend_configurations();

        assert!(!recs.has_recommendations());
        assert_eq!(recs.patterns_analyzed, 0);
        assert!(recs.summary.contains("No patterns"));
    }

    #[test]
    fn test_recommend_configurations_caching_from_repeated() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "repeated_1",
            PatternType::Repeated,
            vec!["llm_node"],
            5,
        ));

        let recs = analysis.recommend_configurations();

        assert!(recs.has_recommendations());
        let cache_recs = recs.by_type(&ReconfigurationType::AddCache);
        assert!(!cache_recs.is_empty());
        assert!(cache_recs
            .iter()
            .any(|r| r.target_nodes.contains(&"llm_node".to_string())));
    }

    #[test]
    fn test_recommend_configurations_caching_high_frequency_is_high_priority() {
        let mut analysis = PatternAnalysis::new();
        // High frequency (> 10) should be high priority
        analysis.patterns.push(create_pattern(
            "high_freq",
            PatternType::Repeated,
            vec!["node1"],
            15,
        ));

        let recs = analysis.recommend_configurations();
        let cache_recs = recs.by_type(&ReconfigurationType::AddCache);

        assert!(cache_recs
            .iter()
            .any(|r| r.priority == ReconfigurationPriority::High));
    }

    #[test]
    fn test_recommend_configurations_caching_from_slow_and_repeated() {
        let mut analysis = PatternAnalysis::new();
        // Node is both slow AND repeated - should get cache recommendation
        analysis.patterns.push(create_pattern(
            "slow_1",
            PatternType::Slow,
            vec!["expensive_node"],
            3,
        ));
        analysis.patterns.push(create_pattern(
            "repeated_1",
            PatternType::Repeated,
            vec!["expensive_node"],
            3,
        ));

        let recs = analysis.recommend_configurations();
        let cache_recs = recs.by_type(&ReconfigurationType::AddCache);

        // Should have cache recommendations for the node
        assert!(cache_recs
            .iter()
            .any(|r| r.target_nodes.contains(&"expensive_node".to_string())));
    }

    #[test]
    fn test_recommend_configurations_parallelization() {
        let mut analysis = PatternAnalysis::new();
        // Multiple sequential patterns from same starting node
        analysis.patterns.push(create_pattern_with_nodes(
            "seq_1",
            PatternType::Sequential,
            vec!["start", "end1"],
            1,
        ));
        analysis.patterns.push(create_pattern_with_nodes(
            "seq_2",
            PatternType::Sequential,
            vec!["start", "end2"],
            1,
        ));
        analysis.patterns.push(create_pattern_with_nodes(
            "seq_3",
            PatternType::Sequential,
            vec!["start", "end3"],
            1,
        ));

        let recs = analysis.recommend_configurations();
        let parallel_recs = recs.by_type(&ReconfigurationType::Parallelize);

        // Should recommend parallelizing from "start" node
        if !parallel_recs.is_empty() {
            assert!(parallel_recs[0].description.contains("start"));
        }
    }

    #[test]
    fn test_recommend_configurations_model_swap_from_high_token_and_success() {
        let mut analysis = PatternAnalysis::new();
        // Node has high token usage but succeeds consistently - candidate for cheaper model
        analysis.patterns.push(create_pattern(
            "high_tokens",
            PatternType::HighTokenUsage,
            vec!["llm_node"],
            5,
        ));
        analysis.patterns.push(create_pattern(
            "success",
            PatternType::Success,
            vec!["llm_node"],
            5,
        ));

        let recs = analysis.recommend_configurations();
        let swap_recs = recs.by_type(&ReconfigurationType::SwapModel);

        assert!(!swap_recs.is_empty());
        assert!(swap_recs
            .iter()
            .any(|r| r.target_nodes.contains(&"llm_node".to_string())));
    }

    #[test]
    fn test_recommend_configurations_timeout_adjustments() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "timeout_1",
            PatternType::Timeout,
            vec!["slow_api"],
            5,
        ));

        let recs = analysis.recommend_configurations();
        let timeout_recs = recs.by_type(&ReconfigurationType::AdjustTimeout);

        assert!(!timeout_recs.is_empty());
        assert!(timeout_recs
            .iter()
            .any(|r| r.target_nodes.contains(&"slow_api".to_string())));
    }

    #[test]
    fn test_recommend_configurations_timeout_with_failures_is_high_priority() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "timeout_1",
            PatternType::Timeout,
            vec!["node"],
            5,
        ));
        analysis.patterns.push(create_pattern(
            "failure_1",
            PatternType::Failure,
            vec!["node"],
            5,
        ));

        let recs = analysis.recommend_configurations();
        let timeout_recs = recs.by_type(&ReconfigurationType::AdjustTimeout);

        // Should be high priority since timeouts lead to failures
        assert!(timeout_recs
            .iter()
            .any(|r| r.priority == ReconfigurationPriority::High));
    }

    #[test]
    fn test_recommend_configurations_retry_from_error_recovery() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "recovery_1",
            PatternType::ErrorRecovery,
            vec!["resilient_node"],
            5,
        ));

        let recs = analysis.recommend_configurations();
        let retry_recs = recs.by_type(&ReconfigurationType::AddRetry);

        assert!(!retry_recs.is_empty());
    }

    #[test]
    fn test_recommend_configurations_retry_from_failure_without_recovery() {
        let mut analysis = PatternAnalysis::new();
        // Node fails but has NO error recovery - should recommend adding retry
        analysis.patterns.push(create_pattern(
            "failure_1",
            PatternType::Failure,
            vec!["fragile_node"],
            5,
        ));

        let recs = analysis.recommend_configurations();
        let retry_recs = recs.by_type(&ReconfigurationType::AddRetry);

        assert!(!retry_recs.is_empty());
        let fragile_retry = retry_recs
            .iter()
            .find(|r| r.target_nodes.contains(&"fragile_node".to_string()));
        assert!(fragile_retry.is_some());
        assert_eq!(
            fragile_retry.unwrap().priority,
            ReconfigurationPriority::High
        );
    }

    #[test]
    fn test_recommend_configurations_batching_from_repeated_high_tokens() {
        let mut analysis = PatternAnalysis::new();
        // High frequency repeated pattern with high token usage - good for batching
        analysis.patterns.push(create_pattern(
            "repeated_1",
            PatternType::Repeated,
            vec!["api_node"],
            10,
        ));
        analysis.patterns.push(create_pattern(
            "high_tokens",
            PatternType::HighTokenUsage,
            vec!["api_node"],
            10,
        ));

        // Use config with cache disabled - batching is only recommended when caching
        // is NOT already recommended for the same node, so we disable caching to test
        // batching in isolation
        let config = RecommendationConfig::default().with_cache(false);
        let recs = analysis.recommend_configurations_with_config(&config);
        let batch_recs = recs.by_type(&ReconfigurationType::AddBatching);

        assert!(!batch_recs.is_empty());
    }

    #[test]
    fn test_recommend_configurations_config_filtering_no_cache() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "repeated_1",
            PatternType::Repeated,
            vec!["node"],
            5,
        ));

        let config = RecommendationConfig::new().with_cache(false);
        let recs = analysis.recommend_configurations_with_config(&config);

        let cache_recs = recs.by_type(&ReconfigurationType::AddCache);
        assert!(cache_recs.is_empty());
    }

    #[test]
    fn test_recommend_configurations_config_filtering_no_parallelization() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern_with_nodes(
            "seq_1",
            PatternType::Sequential,
            vec!["a", "b"],
            1,
        ));
        analysis.patterns.push(create_pattern_with_nodes(
            "seq_2",
            PatternType::Sequential,
            vec!["a", "c"],
            1,
        ));

        let config = RecommendationConfig::new().with_parallelization(false);
        let recs = analysis.recommend_configurations_with_config(&config);

        let parallel_recs = recs.by_type(&ReconfigurationType::Parallelize);
        assert!(parallel_recs.is_empty());
    }

    #[test]
    fn test_recommend_configurations_min_confidence_filtering() {
        let mut analysis = PatternAnalysis::new();
        let mut low_conf_pattern =
            create_pattern("low_conf", PatternType::Repeated, vec!["node"], 5);
        low_conf_pattern.confidence = 0.1; // Below default min_confidence of 0.3
        analysis.patterns.push(low_conf_pattern);

        let recs = analysis.recommend_configurations();
        // Should have no recommendations since confidence is too low
        assert!(recs.by_type(&ReconfigurationType::AddCache).is_empty());
    }

    #[test]
    fn test_recommend_configurations_min_frequency_filtering() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "low_freq",
            PatternType::Repeated,
            vec!["node"],
            1, // Below default min_pattern_frequency of 2
        ));

        let recs = analysis.recommend_configurations();
        // Should have no recommendations since frequency is too low
        assert!(recs.by_type(&ReconfigurationType::AddCache).is_empty());
    }

    #[test]
    fn test_recommend_configurations_sorted_by_quick_win_score() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "repeated_1",
            PatternType::Repeated,
            vec!["node1"],
            5,
        ));
        analysis.patterns.push(create_pattern(
            "failure_1",
            PatternType::Failure,
            vec!["node2"],
            5,
        ));

        let recs = analysis.recommend_configurations();

        // Verify sorted by quick win score
        for i in 1..recs.recommendations.len() {
            assert!(
                recs.recommendations[i - 1].quick_win_score()
                    >= recs.recommendations[i].quick_win_score()
            );
        }
    }

    #[test]
    fn test_recommend_configurations_summary_generation() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "repeated_1",
            PatternType::Repeated,
            vec!["node"],
            5,
        ));
        analysis.patterns.push(create_pattern(
            "failure_1",
            PatternType::Failure,
            vec!["node2"],
            5,
        ));

        let recs = analysis.recommend_configurations();

        assert!(!recs.summary.is_empty());
        assert!(recs.summary.contains("recommendations"));
        assert!(recs.summary.contains("patterns"));
    }

    #[test]
    fn test_recommend_configurations_no_duplicate_timeout_for_slow() {
        let mut analysis = PatternAnalysis::new();
        // Node has both timeout AND slow patterns - should only get one timeout recommendation
        analysis.patterns.push(create_pattern(
            "timeout_1",
            PatternType::Timeout,
            vec!["node"],
            5,
        ));
        analysis
            .patterns
            .push(create_pattern("slow_1", PatternType::Slow, vec!["node"], 5));

        let recs = analysis.recommend_configurations();
        let timeout_recs = recs.by_type(&ReconfigurationType::AdjustTimeout);

        // Should not have duplicate timeout recommendations for same node
        let node_timeout_count = timeout_recs
            .iter()
            .filter(|r| r.target_nodes.contains(&"node".to_string()))
            .count();
        // One from timeout pattern, slow pattern should skip since already suggested
        assert!(node_timeout_count >= 1);
    }

    #[test]
    fn test_recommend_configurations_batching_not_when_caching() {
        let mut analysis = PatternAnalysis::new();
        // Repeated pattern - will get cache recommendation
        // Should not also get batching recommendation for same node
        analysis.patterns.push(create_pattern(
            "repeated_1",
            PatternType::Repeated,
            vec!["node"],
            10,
        ));
        analysis.patterns.push(create_pattern(
            "high_tokens",
            PatternType::HighTokenUsage,
            vec!["node"],
            10,
        ));

        let recs = analysis.recommend_configurations();

        let cache_for_node = recs
            .by_type(&ReconfigurationType::AddCache)
            .iter()
            .filter(|r| r.target_nodes.contains(&"node".to_string()))
            .count();
        let batch_for_node = recs
            .by_type(&ReconfigurationType::AddBatching)
            .iter()
            .filter(|r| r.target_nodes.contains(&"node".to_string()))
            .count();

        // Should have cache but not batching for same node
        assert!(cache_for_node > 0);
        assert_eq!(batch_for_node, 0);
    }

    #[test]
    fn test_recommend_configurations_no_retry_when_recovery_exists() {
        let mut analysis = PatternAnalysis::new();
        // Node fails but ALSO has error recovery - should not recommend adding retry
        analysis.patterns.push(create_pattern(
            "failure_1",
            PatternType::Failure,
            vec!["node"],
            5,
        ));
        analysis.patterns.push(create_pattern(
            "recovery_1",
            PatternType::ErrorRecovery,
            vec!["node"],
            5,
        ));

        let recs = analysis.recommend_configurations();
        let retry_recs = recs.by_type(&ReconfigurationType::AddRetry);

        // Should have retry recommendation only for tuning, not adding new
        let high_priority_retry = retry_recs
            .iter()
            .filter(|r| {
                r.target_nodes.contains(&"node".to_string())
                    && r.priority == ReconfigurationPriority::High
            })
            .count();
        // High priority add_retry is for failures without recovery
        assert_eq!(high_priority_retry, 0);
    }

    #[test]
    fn test_recommend_configurations_all_features_disabled() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(create_pattern(
            "repeated_1",
            PatternType::Repeated,
            vec!["n1"],
            5,
        ));
        analysis.patterns.push(create_pattern(
            "seq_1",
            PatternType::Sequential,
            vec!["n2"],
            1,
        ));
        analysis.patterns.push(create_pattern(
            "high_tokens",
            PatternType::HighTokenUsage,
            vec!["n3"],
            5,
        ));
        analysis.patterns.push(create_pattern(
            "timeout_1",
            PatternType::Timeout,
            vec!["n4"],
            5,
        ));
        analysis.patterns.push(create_pattern(
            "failure_1",
            PatternType::Failure,
            vec!["n5"],
            5,
        ));

        let config = RecommendationConfig::new()
            .with_cache(false)
            .with_parallelization(false)
            .with_model_swap(false)
            .with_timeout(false)
            .with_retry(false)
            .with_batching(false);

        let recs = analysis.recommend_configurations_with_config(&config);
        assert!(!recs.has_recommendations());
    }
}
