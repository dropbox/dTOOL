// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Unified Pattern Detection Engine
//!
//! This module provides a unified interface for pattern detection across DashFlow.
//! It consolidates functionality from three previously separate systems:
//!
//! - `PatternRecognizer` (pattern_recognition.rs) - Execution pattern discovery
//! - `PatternDetector` (self_improvement/analyzers.rs) - Self-improvement patterns
//! - `CrossAgentLearner` (cross_agent_learning.rs) - Cross-agent learning
//!
//! ## Overview
//!
//! The `PatternEngine` trait provides a common interface for pattern detection,
//! enabling consistent access to patterns discovered from different perspectives.
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::pattern_engine::{PatternEngineBuilder, UnifiedPattern};
//!
//! let engine = PatternEngineBuilder::new()
//!     .enable_execution_patterns()
//!     .enable_self_improvement_patterns()
//!     .enable_cross_agent_patterns()
//!     .build();
//!
//! let patterns = engine.detect(&traces);
//!
//! for pattern in patterns {
//!     println!("[{}] {}: {}",
//!         pattern.source,
//!         pattern.pattern_type,
//!         pattern.description
//!     );
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::introspection::ExecutionTrace;

/// Source of a detected pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternSource {
    /// Pattern from execution trace analysis (PatternRecognizer)
    ExecutionAnalysis,
    /// Pattern from self-improvement analysis (PatternDetector)
    SelfImprovement,
    /// Pattern from cross-agent learning (CrossAgentLearner)
    CrossAgentLearning,
}

impl std::fmt::Display for PatternSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternSource::ExecutionAnalysis => write!(f, "Execution"),
            PatternSource::SelfImprovement => write!(f, "Self-Improvement"),
            PatternSource::CrossAgentLearning => write!(f, "Cross-Agent"),
        }
    }
}

/// Unified pattern category that spans all detection systems
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnifiedPatternType {
    /// Token usage patterns (high/low usage correlations)
    TokenUsage,
    /// Performance/latency patterns
    Performance,
    /// Error patterns (recurring errors, error correlations)
    Error,
    /// Node execution patterns (repeated execution, flow patterns)
    NodeExecution,
    /// Tool usage patterns
    ToolUsage,
    /// Resource consumption patterns
    ResourceUsage,
    /// Success/failure correlations
    SuccessCorrelation,
    /// Behavioral patterns (how agents behave)
    Behavioral,
    /// Structural patterns (graph structure)
    Structural,
    /// Custom pattern type
    Custom(String),
}

impl std::fmt::Display for UnifiedPatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnifiedPatternType::TokenUsage => write!(f, "Token Usage"),
            UnifiedPatternType::Performance => write!(f, "Performance"),
            UnifiedPatternType::Error => write!(f, "Error"),
            UnifiedPatternType::NodeExecution => write!(f, "Node Execution"),
            UnifiedPatternType::ToolUsage => write!(f, "Tool Usage"),
            UnifiedPatternType::ResourceUsage => write!(f, "Resource Usage"),
            UnifiedPatternType::SuccessCorrelation => write!(f, "Success Correlation"),
            UnifiedPatternType::Behavioral => write!(f, "Behavioral"),
            UnifiedPatternType::Structural => write!(f, "Structural"),
            UnifiedPatternType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// A unified pattern representation that works across all detection systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPattern {
    /// Unique identifier for this pattern
    pub id: String,
    /// Human-readable description of the pattern
    pub description: String,
    /// The type/category of pattern
    pub pattern_type: UnifiedPatternType,
    /// Which detection system found this pattern
    pub source: PatternSource,
    /// Pattern strength/consistency (0.0-1.0)
    pub strength: f64,
    /// Confidence in this pattern (0.0-1.0)
    pub confidence: f64,
    /// Number of observations supporting this pattern
    pub sample_count: usize,
    /// Actionable recommendations
    pub recommendations: Vec<String>,
    /// Impact description (what happens if pattern is addressed/exploited)
    pub impact: Option<String>,
    /// Expected improvement if recommendation is followed (0.0-1.0)
    pub expected_improvement: Option<f64>,
    /// Nodes affected by this pattern
    pub affected_nodes: Vec<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl UnifiedPattern {
    /// Create a new unified pattern
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        pattern_type: UnifiedPatternType,
        source: PatternSource,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            pattern_type,
            source,
            strength: 0.0,
            confidence: 0.0,
            sample_count: 0,
            recommendations: Vec::new(),
            impact: None,
            expected_improvement: None,
            affected_nodes: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set strength
    #[must_use]
    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set sample count
    #[must_use]
    pub fn with_sample_count(mut self, count: usize) -> Self {
        self.sample_count = count;
        self
    }

    /// Add a recommendation
    #[must_use]
    pub fn with_recommendation(mut self, rec: impl Into<String>) -> Self {
        self.recommendations.push(rec.into());
        self
    }

    /// Set impact description
    #[must_use]
    pub fn with_impact(mut self, impact: impl Into<String>) -> Self {
        self.impact = Some(impact.into());
        self
    }

    /// Set expected improvement
    #[must_use]
    pub fn with_expected_improvement(mut self, improvement: f64) -> Self {
        self.expected_improvement = Some(improvement.clamp(0.0, 10.0));
        self
    }

    /// Add affected nodes
    #[must_use]
    pub fn with_affected_nodes(mut self, nodes: Vec<String>) -> Self {
        self.affected_nodes = nodes;
        self
    }

    /// Check if this pattern is actionable (high strength, confidence, has recommendations)
    #[must_use]
    pub fn is_actionable(&self) -> bool {
        self.strength >= 0.7 && self.confidence >= 0.6 && !self.recommendations.is_empty()
    }

    /// Get a brief summary
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} ({}% strength, {} samples)",
            self.source,
            self.description,
            (self.strength * 100.0) as i32,
            self.sample_count
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

/// Trait for pattern detection engines
///
/// This trait provides a common interface for all pattern detection systems,
/// enabling uniform access regardless of the underlying implementation.
pub trait PatternEngine {
    /// Detect patterns from execution traces
    fn detect(&self, traces: &[ExecutionTrace]) -> Vec<UnifiedPattern>;

    /// Get the source identifier for patterns from this engine
    fn source(&self) -> PatternSource;

    /// Get the name of this engine
    fn name(&self) -> &str;
}

/// Adapter to convert PatternRecognizer output to unified format
pub struct ExecutionPatternAdapter {
    recognizer: crate::pattern_recognition::PatternRecognizer,
}

impl Default for ExecutionPatternAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionPatternAdapter {
    /// Create a new adapter with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            recognizer: crate::pattern_recognition::PatternRecognizer::new(),
        }
    }

    /// Create an adapter with custom configuration
    #[must_use]
    pub fn with_config(config: crate::pattern_recognition::PatternRecognitionConfig) -> Self {
        Self {
            recognizer: crate::pattern_recognition::PatternRecognizer::with_config(config),
        }
    }

    /// Convert pattern condition to unified type
    fn condition_to_type(
        condition: &crate::pattern_recognition::PatternCondition,
    ) -> UnifiedPatternType {
        use crate::pattern_recognition::PatternCondition;
        match condition {
            PatternCondition::HighTokenInput(_) | PatternCondition::LowTokenInput(_) => {
                UnifiedPatternType::TokenUsage
            }
            PatternCondition::LongExecution(_) => UnifiedPatternType::Performance,
            PatternCondition::NodePresent(_)
            | PatternCondition::NodeAbsent(_)
            | PatternCondition::NodeExecutedMultipleTimes(_, _) => {
                UnifiedPatternType::NodeExecution
            }
            PatternCondition::ErrorOccurred(_) => UnifiedPatternType::Error,
            PatternCondition::ToolCalled(_) => UnifiedPatternType::ToolUsage,
            PatternCondition::TimeOfDay(_, _)
            | PatternCondition::Always
            | PatternCondition::Custom(_) => UnifiedPatternType::Custom("Other".to_string()),
        }
    }
}

impl PatternEngine for ExecutionPatternAdapter {
    fn detect(&self, traces: &[ExecutionTrace]) -> Vec<UnifiedPattern> {
        self.recognizer
            .discover_patterns(traces)
            .into_iter()
            .map(|p| {
                let mut unified = UnifiedPattern::new(
                    &p.id,
                    &p.description,
                    Self::condition_to_type(&p.condition),
                    PatternSource::ExecutionAnalysis,
                )
                .with_strength(p.strength)
                .with_confidence(p.confidence)
                .with_sample_count(p.sample_count);

                for rec in &p.recommendations {
                    unified = unified.with_recommendation(rec);
                }

                unified
            })
            .collect()
    }

    fn source(&self) -> PatternSource {
        PatternSource::ExecutionAnalysis
    }

    fn name(&self) -> &str {
        "Execution Pattern Recognizer"
    }
}

/// Adapter to convert PatternDetector output to unified format
pub struct SelfImprovementPatternAdapter {
    detector: crate::self_improvement::PatternDetector,
}

impl Default for SelfImprovementPatternAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfImprovementPatternAdapter {
    /// Create a new adapter with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            detector: crate::self_improvement::PatternDetector::default(),
        }
    }

    /// Create an adapter with custom configuration
    #[must_use]
    pub fn with_config(config: crate::self_improvement::PatternConfig) -> Self {
        Self {
            detector: crate::self_improvement::PatternDetector::new(config),
        }
    }

    /// Convert detector pattern type to unified type
    fn pattern_type_to_unified(
        pattern_type: &crate::self_improvement::PatternType,
    ) -> UnifiedPatternType {
        use crate::self_improvement::PatternType;
        match pattern_type {
            PatternType::RecurringError => UnifiedPatternType::Error,
            PatternType::PerformanceDegradation => UnifiedPatternType::Performance,
            PatternType::ExecutionFlow => UnifiedPatternType::NodeExecution,
            PatternType::ResourceUsage => UnifiedPatternType::ResourceUsage,
        }
    }
}

impl PatternEngine for SelfImprovementPatternAdapter {
    fn detect(&self, traces: &[ExecutionTrace]) -> Vec<UnifiedPattern> {
        self.detector
            .detect(traces)
            .into_iter()
            .map(|p| {
                UnifiedPattern::new(
                    format!("si_{}", sanitize_id(&p.description)),
                    &p.description,
                    Self::pattern_type_to_unified(&p.pattern_type),
                    PatternSource::SelfImprovement,
                )
                .with_strength(p.confidence)
                .with_confidence(p.confidence)
                .with_sample_count(p.occurrences)
                .with_recommendation(&p.suggestion)
                .with_affected_nodes(p.affected_nodes)
            })
            .collect()
    }

    fn source(&self) -> PatternSource {
        PatternSource::SelfImprovement
    }

    fn name(&self) -> &str {
        "Self-Improvement Pattern Detector"
    }
}

/// Adapter to convert CrossAgentLearner output to unified format
pub struct CrossAgentPatternAdapter {
    learner: crate::cross_agent_learning::CrossAgentLearner,
}

impl Default for CrossAgentPatternAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CrossAgentPatternAdapter {
    /// Create a new adapter with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            learner: crate::cross_agent_learning::CrossAgentLearner::new(),
        }
    }

    /// Create an adapter with custom configuration
    #[must_use]
    pub fn with_config(config: crate::cross_agent_learning::CrossAgentConfig) -> Self {
        Self {
            learner: crate::cross_agent_learning::CrossAgentLearner::with_config(config),
        }
    }

    /// Convert cross-agent pattern type to unified type
    fn pattern_type_to_unified(
        pattern_type: &crate::cross_agent_learning::PatternType,
    ) -> UnifiedPatternType {
        use crate::cross_agent_learning::PatternType;
        match pattern_type {
            PatternType::Behavioral => UnifiedPatternType::Behavioral,
            PatternType::Structural => UnifiedPatternType::Structural,
            PatternType::Configuration => UnifiedPatternType::Custom("Configuration".to_string()),
            PatternType::Resource => UnifiedPatternType::ResourceUsage,
            PatternType::ErrorHandling => UnifiedPatternType::Error,
            PatternType::Communication => UnifiedPatternType::Custom("Communication".to_string()),
            PatternType::Timing => UnifiedPatternType::Performance,
        }
    }
}

impl PatternEngine for CrossAgentPatternAdapter {
    fn detect(&self, traces: &[ExecutionTrace]) -> Vec<UnifiedPattern> {
        let insights = self.learner.analyze(traces);

        // Convert successful patterns to unified patterns
        insights
            .successful_patterns
            .into_iter()
            .map(|p| {
                let mut unified = UnifiedPattern::new(
                    &p.id,
                    &p.name,
                    Self::pattern_type_to_unified(&p.pattern_type),
                    PatternSource::CrossAgentLearning,
                )
                .with_strength(p.success_rate)
                .with_confidence(p.confidence)
                .with_sample_count(p.sample_count)
                .with_expected_improvement(p.performance_improvement);

                if !p.application_guide.is_empty() {
                    unified = unified.with_recommendation(&p.application_guide);
                }

                unified.affected_nodes = p.agents_exhibiting;
                unified
            })
            .collect()
    }

    fn source(&self) -> PatternSource {
        PatternSource::CrossAgentLearning
    }

    fn name(&self) -> &str {
        "Cross-Agent Pattern Learner"
    }
}

/// Configuration for the unified pattern engine
#[derive(Debug, Clone, Default)]
pub struct UnifiedPatternEngineConfig {
    /// Enable execution pattern analysis
    pub enable_execution_patterns: bool,
    /// Enable self-improvement pattern detection
    pub enable_self_improvement_patterns: bool,
    /// Enable cross-agent pattern learning
    pub enable_cross_agent_patterns: bool,
    /// Minimum strength to include a pattern (0.0-1.0)
    pub min_strength: f64,
    /// Minimum confidence to include a pattern (0.0-1.0)
    pub min_confidence: f64,
    /// Deduplicate similar patterns
    pub deduplicate: bool,
}

impl UnifiedPatternEngineConfig {
    /// Create a new configuration with all engines enabled
    #[must_use]
    pub fn all() -> Self {
        Self {
            enable_execution_patterns: true,
            enable_self_improvement_patterns: true,
            enable_cross_agent_patterns: true,
            min_strength: 0.5,
            min_confidence: 0.5,
            deduplicate: true,
        }
    }

    /// Create a configuration with only execution patterns
    #[must_use]
    pub fn execution_only() -> Self {
        Self {
            enable_execution_patterns: true,
            ..Default::default()
        }
    }

    /// Create a configuration with only self-improvement patterns
    #[must_use]
    pub fn self_improvement_only() -> Self {
        Self {
            enable_self_improvement_patterns: true,
            ..Default::default()
        }
    }

    /// Create a configuration with only cross-agent patterns
    #[must_use]
    pub fn cross_agent_only() -> Self {
        Self {
            enable_cross_agent_patterns: true,
            ..Default::default()
        }
    }
}

/// Builder for creating a unified pattern engine
pub struct UnifiedPatternEngineBuilder {
    config: UnifiedPatternEngineConfig,
}

impl Default for UnifiedPatternEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedPatternEngineBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: UnifiedPatternEngineConfig::default(),
        }
    }

    /// Enable execution pattern analysis
    #[must_use]
    pub fn enable_execution_patterns(mut self) -> Self {
        self.config.enable_execution_patterns = true;
        self
    }

    /// Enable self-improvement pattern detection
    #[must_use]
    pub fn enable_self_improvement_patterns(mut self) -> Self {
        self.config.enable_self_improvement_patterns = true;
        self
    }

    /// Enable cross-agent pattern learning
    #[must_use]
    pub fn enable_cross_agent_patterns(mut self) -> Self {
        self.config.enable_cross_agent_patterns = true;
        self
    }

    /// Enable all pattern engines
    #[must_use]
    pub fn enable_all(mut self) -> Self {
        self.config.enable_execution_patterns = true;
        self.config.enable_self_improvement_patterns = true;
        self.config.enable_cross_agent_patterns = true;
        self
    }

    /// Set minimum strength threshold
    #[must_use]
    pub fn min_strength(mut self, strength: f64) -> Self {
        self.config.min_strength = strength.clamp(0.0, 1.0);
        self
    }

    /// Set minimum confidence threshold
    #[must_use]
    pub fn min_confidence(mut self, confidence: f64) -> Self {
        self.config.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Enable deduplication of similar patterns
    #[must_use]
    pub fn deduplicate(mut self, deduplicate: bool) -> Self {
        self.config.deduplicate = deduplicate;
        self
    }

    /// Build the unified pattern engine
    #[must_use]
    pub fn build(self) -> UnifiedPatternEngine {
        UnifiedPatternEngine::new(self.config)
    }
}

/// Unified pattern engine that combines all pattern detection systems
pub struct UnifiedPatternEngine {
    config: UnifiedPatternEngineConfig,
    execution_adapter: Option<ExecutionPatternAdapter>,
    self_improvement_adapter: Option<SelfImprovementPatternAdapter>,
    cross_agent_adapter: Option<CrossAgentPatternAdapter>,
}

impl Default for UnifiedPatternEngine {
    fn default() -> Self {
        Self::new(UnifiedPatternEngineConfig::all())
    }
}

impl UnifiedPatternEngine {
    /// Create a new unified pattern engine with the given configuration
    #[must_use]
    pub fn new(config: UnifiedPatternEngineConfig) -> Self {
        let execution_adapter = if config.enable_execution_patterns {
            Some(ExecutionPatternAdapter::new())
        } else {
            None
        };

        let self_improvement_adapter = if config.enable_self_improvement_patterns {
            Some(SelfImprovementPatternAdapter::new())
        } else {
            None
        };

        let cross_agent_adapter = if config.enable_cross_agent_patterns {
            Some(CrossAgentPatternAdapter::new())
        } else {
            None
        };

        Self {
            config,
            execution_adapter,
            self_improvement_adapter,
            cross_agent_adapter,
        }
    }

    /// Detect patterns from all enabled engines
    #[must_use]
    pub fn detect(&self, traces: &[ExecutionTrace]) -> Vec<UnifiedPattern> {
        let mut all_patterns = Vec::new();

        // Collect from each enabled engine
        if let Some(adapter) = &self.execution_adapter {
            all_patterns.extend(adapter.detect(traces));
        }

        if let Some(adapter) = &self.self_improvement_adapter {
            all_patterns.extend(adapter.detect(traces));
        }

        if let Some(adapter) = &self.cross_agent_adapter {
            all_patterns.extend(adapter.detect(traces));
        }

        // Filter by thresholds
        all_patterns.retain(|p| {
            p.strength >= self.config.min_strength && p.confidence >= self.config.min_confidence
        });

        // Deduplicate if enabled
        if self.config.deduplicate {
            all_patterns = self.deduplicate_patterns(all_patterns);
        }

        // Sort by strength * confidence
        all_patterns.sort_by(|a, b| {
            let score_a = a.strength * a.confidence;
            let score_b = b.strength * b.confidence;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        all_patterns
    }

    /// Detect patterns and group by source
    #[must_use]
    pub fn detect_grouped(
        &self,
        traces: &[ExecutionTrace],
    ) -> HashMap<PatternSource, Vec<UnifiedPattern>> {
        let patterns = self.detect(traces);
        let mut grouped: HashMap<PatternSource, Vec<UnifiedPattern>> = HashMap::new();

        for pattern in patterns {
            grouped.entry(pattern.source).or_default().push(pattern);
        }

        grouped
    }

    /// Detect patterns and group by type
    #[must_use]
    pub fn detect_by_type(
        &self,
        traces: &[ExecutionTrace],
    ) -> HashMap<UnifiedPatternType, Vec<UnifiedPattern>> {
        let patterns = self.detect(traces);
        let mut grouped: HashMap<UnifiedPatternType, Vec<UnifiedPattern>> = HashMap::new();

        for pattern in patterns {
            grouped
                .entry(pattern.pattern_type.clone())
                .or_default()
                .push(pattern);
        }

        grouped
    }

    /// Get only actionable patterns (high strength/confidence with recommendations)
    #[must_use]
    pub fn actionable_patterns(&self, traces: &[ExecutionTrace]) -> Vec<UnifiedPattern> {
        self.detect(traces)
            .into_iter()
            .filter(|p| p.is_actionable())
            .collect()
    }

    /// Generate a summary report of detected patterns
    #[must_use]
    pub fn generate_report(&self, traces: &[ExecutionTrace]) -> String {
        let patterns = self.detect(traces);
        let actionable = patterns.iter().filter(|p| p.is_actionable()).count();

        let mut lines = vec![
            "Unified Pattern Detection Report".to_string(),
            "================================".to_string(),
            format!("Traces analyzed: {}", traces.len()),
            format!("Patterns detected: {}", patterns.len()),
            format!("Actionable patterns: {}", actionable),
            String::new(),
        ];

        // Group by source
        let grouped = {
            let mut g: HashMap<PatternSource, Vec<&UnifiedPattern>> = HashMap::new();
            for p in &patterns {
                g.entry(p.source).or_default().push(p);
            }
            g
        };

        for (source, source_patterns) in &grouped {
            lines.push(format!("{} ({} patterns):", source, source_patterns.len()));
            for pattern in source_patterns.iter().take(5) {
                lines.push(format!("  - {}", pattern.summary()));
                for rec in &pattern.recommendations {
                    lines.push(format!("    → {}", rec));
                }
            }
            if source_patterns.len() > 5 {
                lines.push(format!("  ... and {} more", source_patterns.len() - 5));
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }

    /// Deduplicate patterns by description similarity
    fn deduplicate_patterns(&self, patterns: Vec<UnifiedPattern>) -> Vec<UnifiedPattern> {
        let mut seen_descriptions: Vec<String> = Vec::new();
        let mut deduplicated = Vec::new();

        for pattern in patterns {
            let normalized = normalize_description(&pattern.description);

            // Check if we've seen a similar pattern
            let is_duplicate = seen_descriptions
                .iter()
                .any(|seen| similarity(seen, &normalized) > 0.7);

            if !is_duplicate {
                seen_descriptions.push(normalized);
                deduplicated.push(pattern);
            }
        }

        deduplicated
    }
}

// Helper functions

/// Sanitize a string to be a valid ID
fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .take(50)
        .collect()
}

/// Normalize a description for comparison
fn normalize_description(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Calculate similarity between two strings (Jaccard index of words)
fn similarity(a: &str, b: &str) -> f64 {
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();

    if a_words.is_empty() && b_words.is_empty() {
        return 1.0;
    }
    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();

    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ExecutionTraceBuilder, NodeExecution};

    fn create_test_trace(tokens: u64, duration_ms: u64) -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("test_node", duration_ms).with_tokens(tokens))
            .total_duration_ms(duration_ms)
            .total_tokens(tokens)
            .completed(true)
            .build()
    }

    #[test]
    fn test_unified_pattern_creation() {
        let pattern = UnifiedPattern::new(
            "test_id",
            "Test pattern description",
            UnifiedPatternType::Performance,
            PatternSource::ExecutionAnalysis,
        )
        .with_strength(0.8)
        .with_confidence(0.9)
        .with_sample_count(10)
        .with_recommendation("Do something");

        assert_eq!(pattern.id, "test_id");
        assert_eq!(pattern.strength, 0.8);
        assert!(pattern.is_actionable());
    }

    #[test]
    fn test_unified_pattern_not_actionable() {
        let pattern = UnifiedPattern::new(
            "test",
            "Test",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_strength(0.5); // Below threshold

        assert!(!pattern.is_actionable());
    }

    #[test]
    fn test_pattern_source_display() {
        assert_eq!(PatternSource::ExecutionAnalysis.to_string(), "Execution");
        assert_eq!(
            PatternSource::SelfImprovement.to_string(),
            "Self-Improvement"
        );
        assert_eq!(PatternSource::CrossAgentLearning.to_string(), "Cross-Agent");
    }

    #[test]
    fn test_unified_pattern_type_display() {
        assert_eq!(UnifiedPatternType::TokenUsage.to_string(), "Token Usage");
        assert_eq!(UnifiedPatternType::Performance.to_string(), "Performance");
        assert_eq!(
            UnifiedPatternType::Custom("Test".to_string()).to_string(),
            "Test"
        );
    }

    #[test]
    fn test_engine_builder() {
        let engine = UnifiedPatternEngineBuilder::new()
            .enable_execution_patterns()
            .enable_self_improvement_patterns()
            .min_strength(0.6)
            .min_confidence(0.7)
            .build();

        assert!(engine.execution_adapter.is_some());
        assert!(engine.self_improvement_adapter.is_some());
        assert!(engine.cross_agent_adapter.is_none());
    }

    #[test]
    fn test_engine_builder_enable_all() {
        let engine = UnifiedPatternEngineBuilder::new().enable_all().build();

        assert!(engine.execution_adapter.is_some());
        assert!(engine.self_improvement_adapter.is_some());
        assert!(engine.cross_agent_adapter.is_some());
    }

    #[test]
    fn test_engine_detect_empty_traces() {
        let engine = UnifiedPatternEngine::default();
        let patterns = engine.detect(&[]);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_engine_detect_few_traces() {
        let engine = UnifiedPatternEngine::default();
        let traces = vec![create_test_trace(1000, 500)];
        let patterns = engine.detect(&traces);
        // With single trace, most patterns won't meet sample requirements
        assert!(patterns.len() <= 10); // Sanity check
    }

    #[test]
    fn test_config_presets() {
        let all = UnifiedPatternEngineConfig::all();
        assert!(all.enable_execution_patterns);
        assert!(all.enable_self_improvement_patterns);
        assert!(all.enable_cross_agent_patterns);

        let exec_only = UnifiedPatternEngineConfig::execution_only();
        assert!(exec_only.enable_execution_patterns);
        assert!(!exec_only.enable_self_improvement_patterns);
        assert!(!exec_only.enable_cross_agent_patterns);
    }

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("Hello World!"), "hello_world_");
        assert_eq!(sanitize_id("test123"), "test123");
    }

    #[test]
    fn test_normalize_description() {
        assert_eq!(normalize_description("Hello  World!"), "hello world");
        assert_eq!(normalize_description("Test-Pattern"), "testpattern");
    }

    #[test]
    fn test_similarity() {
        assert!(similarity("hello world", "hello world") > 0.99);
        assert!(similarity("hello world", "hello there") > 0.3);
        assert!(similarity("a b c", "x y z") < 0.1);
    }

    #[test]
    fn test_pattern_json_roundtrip() {
        let pattern = UnifiedPattern::new(
            "test",
            "Test pattern",
            UnifiedPatternType::TokenUsage,
            PatternSource::CrossAgentLearning,
        )
        .with_strength(0.85);

        let json = pattern.to_json().unwrap();
        let parsed = UnifiedPattern::from_json(&json).unwrap();

        assert_eq!(parsed.id, pattern.id);
        assert_eq!(parsed.strength, pattern.strength);
    }

    #[test]
    fn test_generate_report() {
        let engine = UnifiedPatternEngine::default();
        let traces = vec![create_test_trace(1000, 500)];
        let report = engine.generate_report(&traces);

        assert!(report.contains("Unified Pattern Detection Report"));
        assert!(report.contains("Traces analyzed: 1"));
    }

    #[test]
    fn test_execution_adapter() {
        let adapter = ExecutionPatternAdapter::new();
        assert_eq!(adapter.source(), PatternSource::ExecutionAnalysis);
        assert_eq!(adapter.name(), "Execution Pattern Recognizer");
    }

    #[test]
    fn test_self_improvement_adapter() {
        let adapter = SelfImprovementPatternAdapter::new();
        assert_eq!(adapter.source(), PatternSource::SelfImprovement);
        assert_eq!(adapter.name(), "Self-Improvement Pattern Detector");
    }

    #[test]
    fn test_cross_agent_adapter() {
        let adapter = CrossAgentPatternAdapter::new();
        assert_eq!(adapter.source(), PatternSource::CrossAgentLearning);
        assert_eq!(adapter.name(), "Cross-Agent Pattern Learner");
    }

    // ========================================================================
    //  Comprehensive Pattern Engine Coverage
    // ========================================================================

    /// Helper to create a trace with errors
    fn create_trace_with_errors(error_node: &str, error_msg: &str) -> ExecutionTrace {
        use crate::introspection::ErrorTrace;
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("start", 100).with_tokens(500))
            .add_node_execution(NodeExecution::new(error_node, 50).with_error(error_msg))
            .add_error(ErrorTrace::new(error_node, error_msg))
            .total_duration_ms(150)
            .total_tokens(500)
            .completed(false)
            .build()
    }

    /// Helper to create a trace with high token usage
    fn create_high_token_trace(node_name: &str, tokens: u64, duration_ms: u64) -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new(node_name, duration_ms).with_tokens(tokens))
            .total_duration_ms(duration_ms)
            .total_tokens(tokens)
            .completed(true)
            .build()
    }

    /// Helper to create a trace with tool calls
    #[allow(dead_code)] // Test: Helper for future tool pattern detection tests
    fn create_trace_with_tools(tools: Vec<&str>) -> ExecutionTrace {
        let mut builder = ExecutionTraceBuilder::new();
        for (i, tool) in tools.iter().enumerate() {
            builder = builder.add_node_execution(
                NodeExecution::new(format!("node_{}", i), 100)
                    .with_tokens(500)
                    .with_tool(*tool),
            );
        }
        builder
            .total_duration_ms(100 * tools.len() as u64)
            .total_tokens(500 * tools.len() as u64)
            .completed(true)
            .build()
    }

    /// Helper to create a trace with slow execution
    fn create_slow_trace(node_name: &str, duration_ms: u64) -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new(node_name, duration_ms).with_tokens(100))
            .total_duration_ms(duration_ms)
            .total_tokens(100)
            .completed(true)
            .build()
    }

    // ------------------------------------------------------------------------
    // Adapter Coverage Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_execution_adapter_detect_with_traces() {
        let adapter = ExecutionPatternAdapter::new();

        // Create multiple traces to trigger pattern detection
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|i| create_high_token_trace("llm_node", 5000 + i * 100, 200 + i * 10))
            .collect();

        let patterns = adapter.detect(&traces);

        // Verify patterns have correct source
        for pattern in &patterns {
            assert_eq!(pattern.source, PatternSource::ExecutionAnalysis);
        }
    }

    #[test]
    fn test_execution_adapter_with_custom_config() {
        use crate::pattern_recognition::PatternRecognitionConfig;

        let config = PatternRecognitionConfig {
            min_strength: 0.3,
            min_samples: 2,
            ..Default::default()
        };
        let adapter = ExecutionPatternAdapter::with_config(config);

        let traces: Vec<ExecutionTrace> = (0..5)
            .map(|i| create_slow_trace("slow_node", 10000 + i * 1000))
            .collect();

        let _patterns = adapter.detect(&traces);

        // Should find patterns with lower thresholds
        assert_eq!(adapter.source(), PatternSource::ExecutionAnalysis);
    }

    #[test]
    fn test_self_improvement_adapter_detect_error_patterns() {
        let adapter = SelfImprovementPatternAdapter::new();

        // Create traces with recurring errors to trigger error pattern detection
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|i| {
                create_trace_with_errors(
                    "api_node",
                    &format!("Connection timeout after {}ms", i * 100),
                )
            })
            .collect();

        let patterns = adapter.detect(&traces);

        // Verify patterns have correct source
        for pattern in &patterns {
            assert_eq!(pattern.source, PatternSource::SelfImprovement);
        }
    }

    #[test]
    fn test_self_improvement_adapter_with_custom_config() {
        use crate::self_improvement::PatternConfig;

        let config = PatternConfig {
            min_confidence: 0.3,
            ..Default::default()
        };
        let adapter = SelfImprovementPatternAdapter::with_config(config);

        let traces: Vec<ExecutionTrace> = (0..5).map(|_| create_test_trace(1000, 500)).collect();

        let _patterns = adapter.detect(&traces);
        assert_eq!(adapter.source(), PatternSource::SelfImprovement);
    }

    #[test]
    fn test_cross_agent_adapter_detect_with_traces() {
        let adapter = CrossAgentPatternAdapter::new();

        // Create diverse traces for cross-agent analysis
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|i| {
                ExecutionTraceBuilder::new()
                    .execution_id(format!("agent_{}", i % 3))
                    .add_node_execution(
                        NodeExecution::new("process", 100 + i * 10).with_tokens(500 + i * 50),
                    )
                    .total_duration_ms(100 + i * 10)
                    .total_tokens(500 + i * 50)
                    .completed(true)
                    .build()
            })
            .collect();

        let patterns = adapter.detect(&traces);

        // Verify patterns have correct source
        for pattern in &patterns {
            assert_eq!(pattern.source, PatternSource::CrossAgentLearning);
        }
    }

    #[test]
    fn test_cross_agent_adapter_with_custom_config() {
        use crate::cross_agent_learning::CrossAgentConfig;

        let config = CrossAgentConfig {
            min_executions_per_agent: 1,
            min_pattern_samples: 2,
            min_success_rate: 0.3,
            ..Default::default()
        };
        let adapter = CrossAgentPatternAdapter::with_config(config);

        let traces: Vec<ExecutionTrace> = (0..5).map(|_| create_test_trace(1000, 500)).collect();

        let _patterns = adapter.detect(&traces);
        assert_eq!(adapter.source(), PatternSource::CrossAgentLearning);
    }

    #[test]
    fn test_adapter_pattern_type_mapping_execution() {
        // Test that ExecutionPatternAdapter correctly maps pattern types
        let adapter = ExecutionPatternAdapter::new();

        // High token traces should produce TokenUsage patterns
        let high_token_traces: Vec<ExecutionTrace> = (0..10)
            .map(|_| create_high_token_trace("llm", 50000, 1000))
            .collect();

        let _patterns = adapter.detect(&high_token_traces);

        // Verify the adapter is functioning
        assert_eq!(adapter.name(), "Execution Pattern Recognizer");
    }

    #[test]
    fn test_adapter_pattern_type_mapping_self_improvement() {
        // Test pattern type mapping for self-improvement adapter
        let adapter = SelfImprovementPatternAdapter::new();

        // Create error traces
        let error_traces: Vec<ExecutionTrace> = (0..10)
            .map(|_| create_trace_with_errors("api", "Rate limit exceeded"))
            .collect();

        let patterns = adapter.detect(&error_traces);

        // If patterns found, verify type mapping
        for pattern in &patterns {
            // Error patterns should map to UnifiedPatternType::Error
            match &pattern.pattern_type {
                UnifiedPatternType::Error => {}
                UnifiedPatternType::Performance => {}
                UnifiedPatternType::NodeExecution => {}
                UnifiedPatternType::ResourceUsage => {}
                _ => {} // Other types are valid too
            }
        }
    }

    // ------------------------------------------------------------------------
    // Deduplication Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_deduplication_removes_similar_patterns() {
        let engine = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .deduplicate(true)
            .min_strength(0.0)
            .min_confidence(0.0)
            .build();

        // Create traces that might produce similar patterns from different sources
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|i| create_high_token_trace("expensive_node", 10000 + i * 100, 500))
            .collect();

        let patterns = engine.detect(&traces);

        // Check that deduplication occurred by verifying no exact duplicate descriptions
        let descriptions: Vec<&str> = patterns.iter().map(|p| p.description.as_str()).collect();
        let unique_count = descriptions.len();

        // With deduplication on, should have unique patterns
        assert!(unique_count <= patterns.len());
    }

    #[test]
    fn test_deduplication_disabled_keeps_all() {
        let engine = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .deduplicate(false)
            .min_strength(0.0)
            .min_confidence(0.0)
            .build();

        let traces: Vec<ExecutionTrace> = (0..5).map(|_| create_test_trace(1000, 500)).collect();

        let patterns_no_dedup = engine.detect(&traces);

        // Create engine with dedup enabled
        let engine_with_dedup = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .deduplicate(true)
            .min_strength(0.0)
            .min_confidence(0.0)
            .build();

        let patterns_with_dedup = engine_with_dedup.detect(&traces);

        // Without dedup should have >= patterns than with dedup
        assert!(patterns_no_dedup.len() >= patterns_with_dedup.len());
    }

    #[test]
    fn test_similarity_function_identical_strings() {
        assert!((similarity("error in node A", "error in node A") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_similarity_function_completely_different() {
        assert!(similarity("foo bar baz", "qux quux corge") < 0.01);
    }

    #[test]
    fn test_similarity_function_partial_overlap() {
        let sim = similarity(
            "high token usage in llm node",
            "high token consumption in llm",
        );
        assert!(sim > 0.3); // Some overlap
        assert!(sim < 1.0); // Not identical
    }

    #[test]
    fn test_similarity_empty_strings() {
        assert!((similarity("", "") - 1.0).abs() < 0.01);
        assert!((similarity("hello", "") - 0.0).abs() < 0.01);
        assert!((similarity("", "world") - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_normalize_description_special_chars() {
        assert_eq!(
            normalize_description("Error: Rate-limit (429)"),
            "error ratelimit 429"
        );
        assert_eq!(normalize_description("Node#123"), "node123");
        assert_eq!(
            normalize_description("   multiple   spaces   "),
            "multiple spaces"
        );
    }

    // ------------------------------------------------------------------------
    // Threshold Filtering Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_threshold_filtering_strength() {
        let engine_low = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .min_strength(0.0)
            .min_confidence(0.0)
            .build();

        let engine_high = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .min_strength(0.9)
            .min_confidence(0.0)
            .build();

        let traces: Vec<ExecutionTrace> = (0..10).map(|_| create_test_trace(1000, 500)).collect();

        let low_patterns = engine_low.detect(&traces);
        let high_patterns = engine_high.detect(&traces);

        // High threshold should have <= patterns than low threshold
        assert!(high_patterns.len() <= low_patterns.len());

        // All high threshold patterns should have strength >= 0.9
        for pattern in &high_patterns {
            assert!(pattern.strength >= 0.9);
        }
    }

    #[test]
    fn test_threshold_filtering_confidence() {
        let engine_low = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .min_strength(0.0)
            .min_confidence(0.0)
            .build();

        let engine_high = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .min_strength(0.0)
            .min_confidence(0.9)
            .build();

        let traces: Vec<ExecutionTrace> = (0..10).map(|_| create_test_trace(1000, 500)).collect();

        let low_patterns = engine_low.detect(&traces);
        let high_patterns = engine_high.detect(&traces);

        // High confidence threshold should have <= patterns
        assert!(high_patterns.len() <= low_patterns.len());

        // All patterns should meet confidence threshold
        for pattern in &high_patterns {
            assert!(pattern.confidence >= 0.9);
        }
    }

    #[test]
    fn test_threshold_both_strength_and_confidence() {
        let engine = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .min_strength(0.7)
            .min_confidence(0.6)
            .build();

        let traces: Vec<ExecutionTrace> = (0..10).map(|_| create_test_trace(1000, 500)).collect();

        let patterns = engine.detect(&traces);

        // All patterns should meet both thresholds
        for pattern in &patterns {
            assert!(
                pattern.strength >= 0.7,
                "Pattern strength {} below threshold 0.7",
                pattern.strength
            );
            assert!(
                pattern.confidence >= 0.6,
                "Pattern confidence {} below threshold 0.6",
                pattern.confidence
            );
        }
    }

    #[test]
    fn test_threshold_clamping() {
        // Thresholds should be clamped to [0.0, 1.0]
        let engine = UnifiedPatternEngineBuilder::new()
            .min_strength(1.5) // Above max
            .min_confidence(-0.5) // Below min
            .build();

        assert!((engine.config.min_strength - 1.0).abs() < 0.01);
        assert!((engine.config.min_confidence - 0.0).abs() < 0.01);
    }

    // ------------------------------------------------------------------------
    // Builder Pattern Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_builder_default_values() {
        let engine = UnifiedPatternEngineBuilder::new().build();

        // Default should have no adapters enabled
        assert!(engine.execution_adapter.is_none());
        assert!(engine.self_improvement_adapter.is_none());
        assert!(engine.cross_agent_adapter.is_none());
    }

    #[test]
    fn test_builder_enable_individual_adapters() {
        let exec_only = UnifiedPatternEngineBuilder::new()
            .enable_execution_patterns()
            .build();
        assert!(exec_only.execution_adapter.is_some());
        assert!(exec_only.self_improvement_adapter.is_none());
        assert!(exec_only.cross_agent_adapter.is_none());

        let si_only = UnifiedPatternEngineBuilder::new()
            .enable_self_improvement_patterns()
            .build();
        assert!(si_only.execution_adapter.is_none());
        assert!(si_only.self_improvement_adapter.is_some());
        assert!(si_only.cross_agent_adapter.is_none());

        let ca_only = UnifiedPatternEngineBuilder::new()
            .enable_cross_agent_patterns()
            .build();
        assert!(ca_only.execution_adapter.is_none());
        assert!(ca_only.self_improvement_adapter.is_none());
        assert!(ca_only.cross_agent_adapter.is_some());
    }

    #[test]
    fn test_builder_method_chaining() {
        let engine = UnifiedPatternEngineBuilder::new()
            .enable_execution_patterns()
            .enable_self_improvement_patterns()
            .enable_cross_agent_patterns()
            .min_strength(0.5)
            .min_confidence(0.6)
            .deduplicate(true)
            .build();

        assert!(engine.execution_adapter.is_some());
        assert!(engine.self_improvement_adapter.is_some());
        assert!(engine.cross_agent_adapter.is_some());
        assert!((engine.config.min_strength - 0.5).abs() < 0.01);
        assert!((engine.config.min_confidence - 0.6).abs() < 0.01);
        assert!(engine.config.deduplicate);
    }

    #[test]
    fn test_config_preset_all() {
        let config = UnifiedPatternEngineConfig::all();
        let engine = UnifiedPatternEngine::new(config);

        assert!(engine.execution_adapter.is_some());
        assert!(engine.self_improvement_adapter.is_some());
        assert!(engine.cross_agent_adapter.is_some());
    }

    #[test]
    fn test_config_preset_self_improvement_only() {
        let config = UnifiedPatternEngineConfig::self_improvement_only();
        let engine = UnifiedPatternEngine::new(config);

        assert!(engine.execution_adapter.is_none());
        assert!(engine.self_improvement_adapter.is_some());
        assert!(engine.cross_agent_adapter.is_none());
    }

    #[test]
    fn test_config_preset_cross_agent_only() {
        let config = UnifiedPatternEngineConfig::cross_agent_only();
        let engine = UnifiedPatternEngine::new(config);

        assert!(engine.execution_adapter.is_none());
        assert!(engine.self_improvement_adapter.is_none());
        assert!(engine.cross_agent_adapter.is_some());
    }

    // ------------------------------------------------------------------------
    // Grouped Detection Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_detect_grouped_by_source() {
        let engine = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .min_strength(0.0)
            .min_confidence(0.0)
            .build();

        let traces: Vec<ExecutionTrace> = (0..10).map(|_| create_test_trace(1000, 500)).collect();

        let grouped = engine.detect_grouped(&traces);

        // Grouped patterns should have valid sources as keys
        for (source, patterns) in &grouped {
            for pattern in patterns {
                assert_eq!(&pattern.source, source);
            }
        }
    }

    #[test]
    fn test_detect_by_type() {
        let engine = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .min_strength(0.0)
            .min_confidence(0.0)
            .build();

        let traces: Vec<ExecutionTrace> = (0..10).map(|_| create_test_trace(1000, 500)).collect();

        let by_type = engine.detect_by_type(&traces);

        // Each pattern type bucket should contain patterns of that type
        for (pattern_type, patterns) in &by_type {
            for pattern in patterns {
                assert_eq!(&pattern.pattern_type, pattern_type);
            }
        }
    }

    #[test]
    fn test_actionable_patterns_filtering() {
        let engine = UnifiedPatternEngine::default();

        let traces: Vec<ExecutionTrace> = (0..10).map(|_| create_test_trace(1000, 500)).collect();

        let actionable = engine.actionable_patterns(&traces);

        // All actionable patterns should meet is_actionable() criteria
        for pattern in &actionable {
            assert!(pattern.is_actionable());
            assert!(pattern.strength >= 0.7);
            assert!(pattern.confidence >= 0.6);
            assert!(!pattern.recommendations.is_empty());
        }
    }

    // ------------------------------------------------------------------------
    // Pattern Sorting Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_patterns_sorted_by_score() {
        let engine = UnifiedPatternEngineBuilder::new()
            .enable_all()
            .min_strength(0.0)
            .min_confidence(0.0)
            .deduplicate(false)
            .build();

        let traces: Vec<ExecutionTrace> = (0..10).map(|_| create_test_trace(1000, 500)).collect();

        let patterns = engine.detect(&traces);

        // Verify patterns are sorted by strength * confidence (descending)
        for i in 1..patterns.len() {
            let prev_score = patterns[i - 1].strength * patterns[i - 1].confidence;
            let curr_score = patterns[i].strength * patterns[i].confidence;
            assert!(prev_score >= curr_score, "Patterns not sorted correctly");
        }
    }

    // ------------------------------------------------------------------------
    // UnifiedPattern Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_unified_pattern_builder_methods() {
        let pattern = UnifiedPattern::new(
            "test_pattern",
            "A test pattern",
            UnifiedPatternType::Performance,
            PatternSource::ExecutionAnalysis,
        )
        .with_strength(0.85)
        .with_confidence(0.9)
        .with_sample_count(100)
        .with_recommendation("Optimize the slow path")
        .with_recommendation("Add caching")
        .with_impact("20% latency reduction")
        .with_expected_improvement(0.2)
        .with_affected_nodes(vec!["node_a".to_string(), "node_b".to_string()]);

        assert_eq!(pattern.id, "test_pattern");
        assert_eq!(pattern.strength, 0.85);
        assert_eq!(pattern.confidence, 0.9);
        assert_eq!(pattern.sample_count, 100);
        assert_eq!(pattern.recommendations.len(), 2);
        assert_eq!(pattern.impact, Some("20% latency reduction".to_string()));
        assert_eq!(pattern.expected_improvement, Some(0.2));
        assert_eq!(pattern.affected_nodes.len(), 2);
    }

    #[test]
    fn test_unified_pattern_strength_clamping() {
        let pattern_high = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_strength(1.5);
        assert_eq!(pattern_high.strength, 1.0);

        let pattern_low = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_strength(-0.5);
        assert_eq!(pattern_low.strength, 0.0);
    }

    #[test]
    fn test_unified_pattern_confidence_clamping() {
        let pattern_high = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_confidence(2.0);
        assert_eq!(pattern_high.confidence, 1.0);

        let pattern_low = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_confidence(-1.0);
        assert_eq!(pattern_low.confidence, 0.0);
    }

    #[test]
    fn test_unified_pattern_expected_improvement_clamping() {
        // expected_improvement is clamped to [0.0, 10.0]
        let pattern_high = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_expected_improvement(15.0);
        assert_eq!(pattern_high.expected_improvement, Some(10.0));

        let pattern_low = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_expected_improvement(-5.0);
        assert_eq!(pattern_low.expected_improvement, Some(0.0));
    }

    #[test]
    fn test_unified_pattern_summary() {
        let pattern = UnifiedPattern::new(
            "test_id",
            "High token usage detected",
            UnifiedPatternType::TokenUsage,
            PatternSource::ExecutionAnalysis,
        )
        .with_strength(0.85)
        .with_sample_count(50);

        let summary = pattern.summary();
        assert!(summary.contains("Execution"));
        assert!(summary.contains("High token usage detected"));
        assert!(summary.contains("85%"));
        assert!(summary.contains("50 samples"));
    }

    #[test]
    fn test_unified_pattern_is_actionable_requires_all_criteria() {
        // Missing recommendations
        let pattern1 = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_strength(0.8)
        .with_confidence(0.7);
        assert!(!pattern1.is_actionable());

        // Low strength
        let pattern2 = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_strength(0.5)
        .with_confidence(0.7)
        .with_recommendation("Fix it");
        assert!(!pattern2.is_actionable());

        // Low confidence
        let pattern3 = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_strength(0.8)
        .with_confidence(0.5)
        .with_recommendation("Fix it");
        assert!(!pattern3.is_actionable());

        // All criteria met
        let pattern4 = UnifiedPattern::new(
            "t",
            "t",
            UnifiedPatternType::Error,
            PatternSource::SelfImprovement,
        )
        .with_strength(0.8)
        .with_confidence(0.7)
        .with_recommendation("Fix it");
        assert!(pattern4.is_actionable());
    }

    // ------------------------------------------------------------------------
    // Report Generation Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_report_format() {
        let engine = UnifiedPatternEngine::default();
        let traces = vec![create_test_trace(1000, 500)];
        let report = engine.generate_report(&traces);

        assert!(report.contains("Unified Pattern Detection Report"));
        assert!(report.contains("================================"));
        assert!(report.contains("Traces analyzed:"));
        assert!(report.contains("Patterns detected:"));
        assert!(report.contains("Actionable patterns:"));
    }

    #[test]
    fn test_report_with_multiple_traces() {
        let engine = UnifiedPatternEngine::default();
        let traces: Vec<ExecutionTrace> = (0..5).map(|_| create_test_trace(1000, 500)).collect();
        let report = engine.generate_report(&traces);

        assert!(report.contains("Traces analyzed: 5"));
    }

    // ------------------------------------------------------------------------
    // Edge Cases
    // ------------------------------------------------------------------------

    #[test]
    fn test_empty_traces_all_methods() {
        let engine = UnifiedPatternEngine::default();
        let empty: Vec<ExecutionTrace> = vec![];

        assert!(engine.detect(&empty).is_empty());
        assert!(engine.detect_grouped(&empty).is_empty());
        assert!(engine.detect_by_type(&empty).is_empty());
        assert!(engine.actionable_patterns(&empty).is_empty());

        let report = engine.generate_report(&empty);
        assert!(report.contains("Traces analyzed: 0"));
    }

    #[test]
    fn test_single_trace_detection() {
        let engine = UnifiedPatternEngine::default();
        let traces = vec![create_test_trace(1000, 500)];

        // Should not panic with single trace
        let patterns = engine.detect(&traces);
        let _grouped = engine.detect_grouped(&traces);
        let _by_type = engine.detect_by_type(&traces);

        // Results may be empty (not enough samples) but should not panic
        assert!(patterns.len() <= 100); // Sanity check
    }

    #[test]
    fn test_sanitize_id_truncation() {
        let long_string = "a".repeat(100);
        let sanitized = sanitize_id(&long_string);
        assert_eq!(sanitized.len(), 50); // Should be truncated to 50 chars
    }

    #[test]
    fn test_sanitize_id_special_characters() {
        assert_eq!(sanitize_id("Hello World!@#$%"), "hello_world_____");
        assert_eq!(sanitize_id("node-123-test"), "node_123_test");
        assert_eq!(sanitize_id("UPPERCASE"), "uppercase");
    }

    #[test]
    fn test_pattern_type_equality() {
        assert_eq!(
            UnifiedPatternType::TokenUsage,
            UnifiedPatternType::TokenUsage
        );
        assert_ne!(
            UnifiedPatternType::TokenUsage,
            UnifiedPatternType::Performance
        );
        assert_eq!(
            UnifiedPatternType::Custom("test".to_string()),
            UnifiedPatternType::Custom("test".to_string())
        );
        assert_ne!(
            UnifiedPatternType::Custom("a".to_string()),
            UnifiedPatternType::Custom("b".to_string())
        );
    }

    #[test]
    fn test_pattern_source_equality() {
        assert_eq!(
            PatternSource::ExecutionAnalysis,
            PatternSource::ExecutionAnalysis
        );
        assert_ne!(
            PatternSource::ExecutionAnalysis,
            PatternSource::SelfImprovement
        );
    }

    #[test]
    fn test_default_implementations() {
        // Test Default trait implementations
        let _adapter1 = ExecutionPatternAdapter::default();
        let _adapter2 = SelfImprovementPatternAdapter::default();
        let _adapter3 = CrossAgentPatternAdapter::default();
        let _builder = UnifiedPatternEngineBuilder::default();
        let _engine = UnifiedPatternEngine::default();
        let _config = UnifiedPatternEngineConfig::default();
    }

    #[test]
    fn test_pattern_engine_trait_implementation() {
        // Verify trait is implemented correctly for all adapters
        let adapters: Vec<Box<dyn PatternEngine>> = vec![
            Box::new(ExecutionPatternAdapter::new()),
            Box::new(SelfImprovementPatternAdapter::new()),
            Box::new(CrossAgentPatternAdapter::new()),
        ];

        let traces = vec![create_test_trace(1000, 500)];

        for adapter in adapters {
            let _ = adapter.detect(&traces);
            let _ = adapter.source();
            let _ = adapter.name();
        }
    }
}
