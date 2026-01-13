//! Pattern Learning
//!
//! This module provides types for identifying and learning execution patterns
//! that can be used to improve future runs.

use super::trace::{ExecutionTrace, NodeExecution};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Pattern Learning
// ============================================================================

/// Type of execution pattern recognized
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PatternType {
    /// Successful execution pattern
    #[default]
    Success,
    /// Failed execution pattern
    Failure,
    /// Slow execution pattern (above threshold)
    Slow,
    /// Efficient execution pattern (fast and low resources)
    Efficient,
    /// High token usage pattern
    HighTokenUsage,
    /// Low token usage pattern
    LowTokenUsage,
    /// Repeated execution pattern (same node multiple times)
    Repeated,
    /// Sequential execution pattern (A -> B -> C)
    Sequential,
    /// Error recovery pattern (retry succeeded after failure)
    ErrorRecovery,
    /// Timeout pattern (execution took too long)
    Timeout,
    /// Idle pattern (long gaps between executions)
    Idle,
    /// Burst pattern (many executions in short time)
    Burst,
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Failure => write!(f, "failure"),
            Self::Slow => write!(f, "slow"),
            Self::Efficient => write!(f, "efficient"),
            Self::HighTokenUsage => write!(f, "high_token_usage"),
            Self::LowTokenUsage => write!(f, "low_token_usage"),
            Self::Repeated => write!(f, "repeated"),
            Self::Sequential => write!(f, "sequential"),
            Self::ErrorRecovery => write!(f, "error_recovery"),
            Self::Timeout => write!(f, "timeout"),
            Self::Idle => write!(f, "idle"),
            Self::Burst => write!(f, "burst"),
        }
    }
}

/// Condition that defines when a pattern matches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCondition {
    /// Field being checked (e.g., "duration_ms", "tokens_used", "success")
    pub field: String,
    /// Operator for comparison
    pub operator: PatternOperator,
    /// Value to compare against
    pub value: PatternValue,
}

impl PatternCondition {
    /// Create a new pattern condition
    #[must_use]
    pub fn new(field: impl Into<String>, operator: PatternOperator, value: PatternValue) -> Self {
        Self {
            field: field.into(),
            operator,
            value,
        }
    }

    /// Create a condition for duration greater than threshold
    #[must_use]
    pub fn duration_gt(threshold_ms: u64) -> Self {
        Self::new(
            "duration_ms",
            PatternOperator::GreaterThan,
            PatternValue::Integer(threshold_ms as i64),
        )
    }

    /// Create a condition for duration less than threshold
    #[must_use]
    pub fn duration_lt(threshold_ms: u64) -> Self {
        Self::new(
            "duration_ms",
            PatternOperator::LessThan,
            PatternValue::Integer(threshold_ms as i64),
        )
    }

    /// Create a condition for tokens greater than threshold
    #[must_use]
    pub fn tokens_gt(threshold: u64) -> Self {
        Self::new(
            "tokens_used",
            PatternOperator::GreaterThan,
            PatternValue::Integer(threshold as i64),
        )
    }

    /// Create a condition for tokens less than threshold
    #[must_use]
    pub fn tokens_lt(threshold: u64) -> Self {
        Self::new(
            "tokens_used",
            PatternOperator::LessThan,
            PatternValue::Integer(threshold as i64),
        )
    }

    /// Create a condition for success
    #[must_use]
    pub fn is_success() -> Self {
        Self::new(
            "success",
            PatternOperator::Equals,
            PatternValue::Boolean(true),
        )
    }

    /// Create a condition for failure
    #[must_use]
    pub fn is_failure() -> Self {
        Self::new(
            "success",
            PatternOperator::Equals,
            PatternValue::Boolean(false),
        )
    }

    /// Create a condition for node name
    #[must_use]
    pub fn node_equals(name: impl Into<String>) -> Self {
        Self::new(
            "node",
            PatternOperator::Equals,
            PatternValue::String(name.into()),
        )
    }

    /// Create a condition for execution count
    #[must_use]
    pub fn count_gt(count: usize) -> Self {
        Self::new(
            "count",
            PatternOperator::GreaterThan,
            PatternValue::Integer(count as i64),
        )
    }

    /// Check if this condition matches a node execution
    #[must_use]
    pub fn matches(&self, exec: &NodeExecution) -> bool {
        match self.field.as_str() {
            "duration_ms" => {
                if let PatternValue::Integer(threshold) = &self.value {
                    self.operator
                        .compare_i64(exec.duration_ms as i64, *threshold)
                } else {
                    false
                }
            }
            "tokens_used" => {
                if let PatternValue::Integer(threshold) = &self.value {
                    self.operator
                        .compare_i64(exec.tokens_used as i64, *threshold)
                } else {
                    false
                }
            }
            "success" => {
                if let PatternValue::Boolean(expected) = &self.value {
                    match self.operator {
                        PatternOperator::Equals => exec.success == *expected,
                        PatternOperator::NotEquals => exec.success != *expected,
                        _ => false,
                    }
                } else {
                    false
                }
            }
            "node" => {
                if let PatternValue::String(expected) = &self.value {
                    match self.operator {
                        PatternOperator::Equals => exec.node == *expected,
                        PatternOperator::NotEquals => exec.node != *expected,
                        PatternOperator::Contains => exec.node.contains(expected.as_str()),
                        _ => false,
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Convert to JSON
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Operator for pattern condition comparisons
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternOperator {
    /// Equal to
    Equals,
    /// Not equal to
    NotEquals,
    /// Greater than
    GreaterThan,
    /// Greater than or equal to
    GreaterThanOrEqual,
    /// Less than
    LessThan,
    /// Less than or equal to
    LessThanOrEqual,
    /// Contains (for strings)
    Contains,
    /// Between (for ranges)
    Between,
}

impl PatternOperator {
    /// Compare two i64 values
    #[must_use]
    pub fn compare_i64(&self, left: i64, right: i64) -> bool {
        match self {
            Self::Equals => left == right,
            Self::NotEquals => left != right,
            Self::GreaterThan => left > right,
            Self::GreaterThanOrEqual => left >= right,
            Self::LessThan => left < right,
            Self::LessThanOrEqual => left <= right,
            Self::Contains | Self::Between => false, // Not applicable for numeric comparison
        }
    }

    /// Compare two f64 values
    #[must_use]
    pub fn compare_f64(&self, left: f64, right: f64) -> bool {
        match self {
            Self::Equals => (left - right).abs() < f64::EPSILON,
            Self::NotEquals => (left - right).abs() >= f64::EPSILON,
            Self::GreaterThan => left > right,
            Self::GreaterThanOrEqual => left >= right,
            Self::LessThan => left < right,
            Self::LessThanOrEqual => left <= right,
            Self::Contains | Self::Between => false,
        }
    }
}

impl std::fmt::Display for PatternOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Equals => write!(f, "=="),
            Self::NotEquals => write!(f, "!="),
            Self::GreaterThan => write!(f, ">"),
            Self::GreaterThanOrEqual => write!(f, ">="),
            Self::LessThan => write!(f, "<"),
            Self::LessThanOrEqual => write!(f, "<="),
            Self::Contains => write!(f, "contains"),
            Self::Between => write!(f, "between"),
        }
    }
}

/// Value type for pattern conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatternValue {
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// String value
    String(String),
    /// Range of integers (for Between operator)
    IntegerRange(i64, i64),
    /// Range of floats (for Between operator)
    FloatRange(f64, f64),
}

impl std::fmt::Display for PatternValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer(v) => write!(f, "{}", v),
            Self::Float(v) => write!(f, "{:.2}", v),
            Self::Boolean(v) => write!(f, "{}", v),
            Self::String(v) => write!(f, "\"{}\"", v),
            Self::IntegerRange(a, b) => write!(f, "[{}, {}]", a, b),
            Self::FloatRange(a, b) => write!(f, "[{:.2}, {:.2}]", a, b),
        }
    }
}

/// A learned execution pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// Unique identifier for this pattern
    pub id: String,
    /// Type of pattern
    pub pattern_type: PatternType,
    /// Conditions that define this pattern
    pub conditions: Vec<PatternCondition>,
    /// How many times this pattern was observed
    pub frequency: usize,
    /// Nodes where this pattern was observed
    pub affected_nodes: Vec<String>,
    /// Confidence level (0.0-1.0) in this pattern
    pub confidence: f64,
    /// Human-readable description
    pub description: String,
    /// Additional context or evidence
    pub evidence: Vec<String>,
    /// When this pattern was first observed (execution index)
    pub first_seen: usize,
    /// When this pattern was last observed (execution index)
    pub last_seen: usize,
}

impl Pattern {
    /// Create a new pattern
    #[must_use]
    pub fn new(id: impl Into<String>, pattern_type: PatternType) -> Self {
        Self {
            id: id.into(),
            pattern_type,
            conditions: Vec::new(),
            frequency: 1,
            affected_nodes: Vec::new(),
            confidence: 0.5,
            description: String::new(),
            evidence: Vec::new(),
            first_seen: 0,
            last_seen: 0,
        }
    }

    /// Create a pattern builder
    #[must_use]
    pub fn builder() -> PatternBuilder {
        PatternBuilder::new()
    }

    /// Add a condition to this pattern
    #[must_use]
    pub fn with_condition(mut self, condition: PatternCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set the frequency
    #[must_use]
    pub fn with_frequency(mut self, frequency: usize) -> Self {
        self.frequency = frequency;
        self
    }

    /// Add an affected node
    #[must_use]
    pub fn with_affected_node(mut self, node: impl Into<String>) -> Self {
        self.affected_nodes.push(node.into());
        self
    }

    /// Set the confidence level
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add evidence
    #[must_use]
    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }

    /// Set first seen index
    #[must_use]
    pub fn with_first_seen(mut self, index: usize) -> Self {
        self.first_seen = index;
        self
    }

    /// Set last seen index
    #[must_use]
    pub fn with_last_seen(mut self, index: usize) -> Self {
        self.last_seen = index;
        self
    }

    /// Check if this pattern matches a node execution
    #[must_use]
    pub fn matches(&self, exec: &NodeExecution) -> bool {
        // All conditions must match
        self.conditions.iter().all(|c| c.matches(exec))
    }

    /// Check if this pattern matches any execution in a trace
    #[must_use]
    pub fn matches_trace(&self, trace: &ExecutionTrace) -> bool {
        trace.nodes_executed.iter().any(|exec| self.matches(exec))
    }

    /// Count how many executions in a trace match this pattern
    #[must_use]
    pub fn count_matches(&self, trace: &ExecutionTrace) -> usize {
        trace
            .nodes_executed
            .iter()
            .filter(|exec| self.matches(exec))
            .count()
    }

    /// Check if this is a negative pattern (failure, slow, etc.)
    #[must_use]
    pub fn is_negative(&self) -> bool {
        matches!(
            self.pattern_type,
            PatternType::Failure
                | PatternType::Slow
                | PatternType::HighTokenUsage
                | PatternType::Timeout
        )
    }

    /// Check if this is a positive pattern (success, efficient, etc.)
    #[must_use]
    pub fn is_positive(&self) -> bool {
        matches!(
            self.pattern_type,
            PatternType::Success
                | PatternType::Efficient
                | PatternType::LowTokenUsage
                | PatternType::ErrorRecovery
        )
    }

    /// Get a summary of this pattern
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} (freq: {}, confidence: {:.0}%): {}",
            self.pattern_type,
            self.id,
            self.frequency,
            self.confidence * 100.0,
            self.description
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

/// Builder for Pattern
#[derive(Debug, Default)]
pub struct PatternBuilder {
    id: Option<String>,
    pattern_type: PatternType,
    conditions: Vec<PatternCondition>,
    frequency: usize,
    affected_nodes: Vec<String>,
    confidence: f64,
    description: Option<String>,
    evidence: Vec<String>,
    first_seen: usize,
    last_seen: usize,
}

impl PatternBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            frequency: 1,
            confidence: 0.5,
            ..Self::default()
        }
    }

    /// Set the pattern ID
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the pattern type
    #[must_use]
    pub fn pattern_type(mut self, pattern_type: PatternType) -> Self {
        self.pattern_type = pattern_type;
        self
    }

    /// Add a condition
    #[must_use]
    pub fn condition(mut self, condition: PatternCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set the frequency
    #[must_use]
    pub fn frequency(mut self, frequency: usize) -> Self {
        self.frequency = frequency;
        self
    }

    /// Add an affected node
    #[must_use]
    pub fn affected_node(mut self, node: impl Into<String>) -> Self {
        self.affected_nodes.push(node.into());
        self
    }

    /// Set the confidence
    #[must_use]
    pub fn confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the description
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add evidence
    #[must_use]
    pub fn evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }

    /// Set first seen index
    #[must_use]
    pub fn first_seen(mut self, index: usize) -> Self {
        self.first_seen = index;
        self
    }

    /// Set last seen index
    #[must_use]
    pub fn last_seen(mut self, index: usize) -> Self {
        self.last_seen = index;
        self
    }

    /// Build the pattern
    ///
    /// # Errors
    ///
    /// Returns error if required fields are missing
    pub fn build(self) -> Result<Pattern, &'static str> {
        let id = self.id.ok_or("id is required")?;
        let description = self.description.ok_or("description is required")?;

        Ok(Pattern {
            id,
            pattern_type: self.pattern_type,
            conditions: self.conditions,
            frequency: self.frequency,
            affected_nodes: self.affected_nodes,
            confidence: self.confidence,
            description,
            evidence: self.evidence,
            first_seen: self.first_seen,
            last_seen: self.last_seen,
        })
    }
}

/// Analysis result containing learned patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysis {
    /// All detected patterns
    pub patterns: Vec<Pattern>,
    /// Number of executions analyzed
    pub executions_analyzed: usize,
    /// Number of patterns learned
    pub patterns_learned: usize,
    /// Summary of findings
    pub summary: String,
}

impl PatternAnalysis {
    /// Create a new pattern analysis
    #[must_use]
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            executions_analyzed: 0,
            patterns_learned: 0,
            summary: String::new(),
        }
    }

    /// Check if any patterns were learned
    #[must_use]
    pub fn has_patterns(&self) -> bool {
        !self.patterns.is_empty()
    }

    /// Get the number of patterns
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Get patterns by type
    #[must_use]
    pub fn by_type(&self, pattern_type: &PatternType) -> Vec<&Pattern> {
        self.patterns
            .iter()
            .filter(|p| &p.pattern_type == pattern_type)
            .collect()
    }

    /// Get all negative patterns (failure, slow, etc.)
    #[must_use]
    pub fn negative_patterns(&self) -> Vec<&Pattern> {
        self.patterns.iter().filter(|p| p.is_negative()).collect()
    }

    /// Get all positive patterns (success, efficient, etc.)
    #[must_use]
    pub fn positive_patterns(&self) -> Vec<&Pattern> {
        self.patterns.iter().filter(|p| p.is_positive()).collect()
    }

    /// Get patterns for a specific node
    #[must_use]
    pub fn for_node(&self, node: &str) -> Vec<&Pattern> {
        self.patterns
            .iter()
            .filter(|p| p.affected_nodes.iter().any(|n| n == node))
            .collect()
    }

    /// Get most frequent patterns
    #[must_use]
    pub fn most_frequent(&self, limit: usize) -> Vec<&Pattern> {
        let mut patterns: Vec<_> = self.patterns.iter().collect();
        patterns.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        patterns.into_iter().take(limit).collect()
    }

    /// Get highest confidence patterns
    #[must_use]
    pub fn highest_confidence(&self, limit: usize) -> Vec<&Pattern> {
        let mut patterns: Vec<_> = self.patterns.iter().collect();
        patterns.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        patterns.into_iter().take(limit).collect()
    }

    /// Check if a trace matches any learned patterns
    #[must_use]
    pub fn match_trace(&self, trace: &ExecutionTrace) -> Vec<&Pattern> {
        self.patterns
            .iter()
            .filter(|p| p.matches_trace(trace))
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

    /// Generate a summary of the analysis
    fn generate_summary(&self) -> String {
        if self.patterns.is_empty() {
            return "No patterns learned from execution trace.".to_string();
        }

        let negative = self.negative_patterns().len();
        let positive = self.positive_patterns().len();

        let mut parts = Vec::new();
        if positive > 0 {
            parts.push(format!("{} positive", positive));
        }
        if negative > 0 {
            parts.push(format!("{} negative", negative));
        }

        format!(
            "Learned {} patterns from {} executions: {}",
            self.patterns.len(),
            self.executions_analyzed,
            parts.join(", ")
        )
    }
}

impl Default for PatternAnalysis {
    fn default() -> Self {
        Self::new()
    }
}

/// Thresholds for pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternThresholds {
    /// Duration threshold for "slow" pattern (ms)
    pub slow_duration_ms: u64,
    /// Duration threshold for "efficient" pattern (ms)
    pub efficient_duration_ms: u64,
    /// Token threshold for "high token usage" pattern
    pub high_token_threshold: u64,
    /// Token threshold for "low token usage" pattern
    pub low_token_threshold: u64,
    /// Minimum occurrences to form a pattern
    pub min_frequency: usize,
    /// Minimum confidence to report a pattern
    pub min_confidence: f64,
    /// Timeout threshold (ms)
    pub timeout_threshold_ms: u64,
}

impl Default for PatternThresholds {
    fn default() -> Self {
        Self {
            slow_duration_ms: 5000,
            efficient_duration_ms: 100,
            high_token_threshold: 2000,
            low_token_threshold: 100,
            min_frequency: 2,
            min_confidence: 0.3,
            timeout_threshold_ms: 30000,
        }
    }
}

impl PatternThresholds {
    /// Create new thresholds with defaults
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set slow duration threshold
    #[must_use]
    pub fn with_slow_duration(mut self, ms: u64) -> Self {
        self.slow_duration_ms = ms;
        self
    }

    /// Set efficient duration threshold
    #[must_use]
    pub fn with_efficient_duration(mut self, ms: u64) -> Self {
        self.efficient_duration_ms = ms;
        self
    }

    /// Set high token threshold
    #[must_use]
    pub fn with_high_token_threshold(mut self, threshold: u64) -> Self {
        self.high_token_threshold = threshold;
        self
    }

    /// Set low token threshold
    #[must_use]
    pub fn with_low_token_threshold(mut self, threshold: u64) -> Self {
        self.low_token_threshold = threshold;
        self
    }

    /// Set minimum frequency
    #[must_use]
    pub fn with_min_frequency(mut self, freq: usize) -> Self {
        self.min_frequency = freq;
        self
    }

    /// Set minimum confidence
    #[must_use]
    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set timeout threshold
    #[must_use]
    pub fn with_timeout_threshold(mut self, ms: u64) -> Self {
        self.timeout_threshold_ms = ms;
        self
    }
}

// Add learn_patterns methods to ExecutionTrace
impl ExecutionTrace {
    /// Learn patterns from execution trace
    #[must_use]
    pub fn learn_patterns(&self) -> PatternAnalysis {
        self.learn_patterns_with_thresholds(&PatternThresholds::default())
    }

    /// Learn patterns from execution trace with custom thresholds
    #[must_use]
    pub fn learn_patterns_with_thresholds(
        &self,
        thresholds: &PatternThresholds,
    ) -> PatternAnalysis {
        let mut analysis = PatternAnalysis::new();
        analysis.executions_analyzed = self.nodes_executed.len();

        if self.nodes_executed.is_empty() {
            analysis.summary = analysis.generate_summary();
            return analysis;
        }

        // Pattern 1: Success patterns
        self.learn_success_patterns(&mut analysis, thresholds);

        // Pattern 2: Failure patterns
        self.learn_failure_patterns(&mut analysis, thresholds);

        // Pattern 3: Slow execution patterns
        self.learn_slow_patterns(&mut analysis, thresholds);

        // Pattern 4: Efficient execution patterns
        self.learn_efficient_patterns(&mut analysis, thresholds);

        // Pattern 5: High token usage patterns
        self.learn_high_token_patterns(&mut analysis, thresholds);

        // Pattern 6: Low token usage patterns
        self.learn_low_token_patterns(&mut analysis, thresholds);

        // Pattern 7: Repeated execution patterns
        self.learn_repeated_patterns(&mut analysis, thresholds);

        // Pattern 8: Sequential patterns
        self.learn_sequential_patterns(&mut analysis, thresholds);

        // Pattern 9: Error recovery patterns
        self.learn_error_recovery_patterns(&mut analysis, thresholds);

        // Pattern 10: Timeout patterns
        self.learn_timeout_patterns(&mut analysis, thresholds);

        analysis.patterns_learned = analysis.patterns.len();
        analysis.summary = analysis.generate_summary();

        // Sort patterns by confidence (highest first)
        analysis.patterns.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        analysis
    }

    fn learn_success_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        thresholds: &PatternThresholds,
    ) {
        // Group successful executions by node
        let mut node_successes: HashMap<&str, Vec<(usize, &NodeExecution)>> = HashMap::new();
        for (idx, exec) in self.nodes_executed.iter().enumerate() {
            if exec.success {
                node_successes
                    .entry(&exec.node)
                    .or_default()
                    .push((idx, exec));
            }
        }

        for (node, successes) in node_successes {
            if successes.len() >= thresholds.min_frequency {
                let confidence = successes.len() as f64
                    / self
                        .nodes_executed
                        .iter()
                        .filter(|e| e.node == node)
                        .count()
                        .max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    let first_seen = successes.first().map(|(i, _)| *i).unwrap_or(0);
                    let last_seen = successes.last().map(|(i, _)| *i).unwrap_or(0);
                    let avg_duration = successes.iter().map(|(_, e)| e.duration_ms).sum::<u64>()
                        / successes.len() as u64;

                    analysis.patterns.push(
                        Pattern::new(format!("success_{}", node), PatternType::Success)
                            .with_condition(PatternCondition::node_equals(node))
                            .with_condition(PatternCondition::is_success())
                            .with_frequency(successes.len())
                            .with_affected_node(node)
                            .with_confidence(confidence)
                            .with_description(format!("Node '{}' succeeds consistently", node))
                            .with_evidence(format!(
                                "{} successful executions, avg duration: {} ms",
                                successes.len(),
                                avg_duration
                            ))
                            .with_first_seen(first_seen)
                            .with_last_seen(last_seen),
                    );
                }
            }
        }
    }

    fn learn_failure_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        thresholds: &PatternThresholds,
    ) {
        // Group failed executions by node
        let mut node_failures: HashMap<&str, Vec<(usize, &NodeExecution)>> = HashMap::new();
        for (idx, exec) in self.nodes_executed.iter().enumerate() {
            if !exec.success {
                node_failures
                    .entry(&exec.node)
                    .or_default()
                    .push((idx, exec));
            }
        }

        for (node, failures) in node_failures {
            if failures.len() >= thresholds.min_frequency {
                let total_for_node = self
                    .nodes_executed
                    .iter()
                    .filter(|e| e.node == node)
                    .count();
                let confidence = failures.len() as f64 / total_for_node.max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    let first_seen = failures.first().map(|(i, _)| *i).unwrap_or(0);
                    let last_seen = failures.last().map(|(i, _)| *i).unwrap_or(0);

                    // Collect error messages
                    let errors: Vec<_> = failures
                        .iter()
                        .filter_map(|(_, e)| e.error_message.as_ref())
                        .take(3)
                        .collect();

                    let error_evidence = if errors.is_empty() {
                        "No error messages captured".to_string()
                    } else {
                        format!(
                            "Errors: {}",
                            errors
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };

                    analysis.patterns.push(
                        Pattern::new(format!("failure_{}", node), PatternType::Failure)
                            .with_condition(PatternCondition::node_equals(node))
                            .with_condition(PatternCondition::is_failure())
                            .with_frequency(failures.len())
                            .with_affected_node(node)
                            .with_confidence(confidence)
                            .with_description(format!(
                                "Node '{}' fails frequently ({:.0}% failure rate)",
                                node,
                                confidence * 100.0
                            ))
                            .with_evidence(format!(
                                "{} failures out of {} executions",
                                failures.len(),
                                total_for_node
                            ))
                            .with_evidence(error_evidence)
                            .with_first_seen(first_seen)
                            .with_last_seen(last_seen),
                    );
                }
            }
        }
    }

    fn learn_slow_patterns(&self, analysis: &mut PatternAnalysis, thresholds: &PatternThresholds) {
        // Find slow executions
        let mut node_slow: HashMap<&str, Vec<(usize, &NodeExecution)>> = HashMap::new();
        for (idx, exec) in self.nodes_executed.iter().enumerate() {
            if exec.duration_ms >= thresholds.slow_duration_ms {
                node_slow.entry(&exec.node).or_default().push((idx, exec));
            }
        }

        for (node, slow_execs) in node_slow {
            if slow_execs.len() >= thresholds.min_frequency {
                let total_for_node = self
                    .nodes_executed
                    .iter()
                    .filter(|e| e.node == node)
                    .count();
                let confidence = slow_execs.len() as f64 / total_for_node.max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    let first_seen = slow_execs.first().map(|(i, _)| *i).unwrap_or(0);
                    let last_seen = slow_execs.last().map(|(i, _)| *i).unwrap_or(0);
                    let avg_duration = slow_execs.iter().map(|(_, e)| e.duration_ms).sum::<u64>()
                        / slow_execs.len() as u64;
                    let max_duration = slow_execs
                        .iter()
                        .map(|(_, e)| e.duration_ms)
                        .max()
                        .unwrap_or(0);

                    analysis.patterns.push(
                        Pattern::new(format!("slow_{}", node), PatternType::Slow)
                            .with_condition(PatternCondition::node_equals(node))
                            .with_condition(PatternCondition::duration_gt(
                                thresholds.slow_duration_ms,
                            ))
                            .with_frequency(slow_execs.len())
                            .with_affected_node(node)
                            .with_confidence(confidence)
                            .with_description(format!(
                                "Node '{}' often runs slowly (>{} ms)",
                                node, thresholds.slow_duration_ms
                            ))
                            .with_evidence(format!(
                                "Avg: {} ms, Max: {} ms",
                                avg_duration, max_duration
                            ))
                            .with_first_seen(first_seen)
                            .with_last_seen(last_seen),
                    );
                }
            }
        }
    }

    fn learn_efficient_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        thresholds: &PatternThresholds,
    ) {
        // Find efficient executions (fast AND successful)
        let mut node_efficient: HashMap<&str, Vec<(usize, &NodeExecution)>> = HashMap::new();
        for (idx, exec) in self.nodes_executed.iter().enumerate() {
            if exec.success && exec.duration_ms <= thresholds.efficient_duration_ms {
                node_efficient
                    .entry(&exec.node)
                    .or_default()
                    .push((idx, exec));
            }
        }

        for (node, efficient_execs) in node_efficient {
            if efficient_execs.len() >= thresholds.min_frequency {
                let total_for_node = self
                    .nodes_executed
                    .iter()
                    .filter(|e| e.node == node)
                    .count();
                let confidence = efficient_execs.len() as f64 / total_for_node.max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    let first_seen = efficient_execs.first().map(|(i, _)| *i).unwrap_or(0);
                    let last_seen = efficient_execs.last().map(|(i, _)| *i).unwrap_or(0);
                    let avg_duration = efficient_execs
                        .iter()
                        .map(|(_, e)| e.duration_ms)
                        .sum::<u64>()
                        / efficient_execs.len() as u64;

                    analysis.patterns.push(
                        Pattern::new(format!("efficient_{}", node), PatternType::Efficient)
                            .with_condition(PatternCondition::node_equals(node))
                            .with_condition(PatternCondition::is_success())
                            .with_condition(PatternCondition::duration_lt(
                                thresholds.efficient_duration_ms,
                            ))
                            .with_frequency(efficient_execs.len())
                            .with_affected_node(node)
                            .with_confidence(confidence)
                            .with_description(format!(
                                "Node '{}' executes efficiently (<{} ms)",
                                node, thresholds.efficient_duration_ms
                            ))
                            .with_evidence(format!("Avg duration: {} ms", avg_duration))
                            .with_first_seen(first_seen)
                            .with_last_seen(last_seen),
                    );
                }
            }
        }
    }

    fn learn_high_token_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        thresholds: &PatternThresholds,
    ) {
        // Find high token usage executions
        let mut node_high_tokens: HashMap<&str, Vec<(usize, &NodeExecution)>> = HashMap::new();
        for (idx, exec) in self.nodes_executed.iter().enumerate() {
            if exec.tokens_used >= thresholds.high_token_threshold {
                node_high_tokens
                    .entry(&exec.node)
                    .or_default()
                    .push((idx, exec));
            }
        }

        for (node, high_token_execs) in node_high_tokens {
            if high_token_execs.len() >= thresholds.min_frequency {
                let total_for_node = self
                    .nodes_executed
                    .iter()
                    .filter(|e| e.node == node)
                    .count();
                let confidence = high_token_execs.len() as f64 / total_for_node.max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    let first_seen = high_token_execs.first().map(|(i, _)| *i).unwrap_or(0);
                    let last_seen = high_token_execs.last().map(|(i, _)| *i).unwrap_or(0);
                    let avg_tokens = high_token_execs
                        .iter()
                        .map(|(_, e)| e.tokens_used)
                        .sum::<u64>()
                        / high_token_execs.len() as u64;
                    let max_tokens = high_token_execs
                        .iter()
                        .map(|(_, e)| e.tokens_used)
                        .max()
                        .unwrap_or(0);

                    analysis.patterns.push(
                        Pattern::new(format!("high_tokens_{}", node), PatternType::HighTokenUsage)
                            .with_condition(PatternCondition::node_equals(node))
                            .with_condition(PatternCondition::tokens_gt(
                                thresholds.high_token_threshold,
                            ))
                            .with_frequency(high_token_execs.len())
                            .with_affected_node(node)
                            .with_confidence(confidence)
                            .with_description(format!(
                                "Node '{}' uses many tokens (>{} tokens)",
                                node, thresholds.high_token_threshold
                            ))
                            .with_evidence(format!(
                                "Avg: {} tokens, Max: {} tokens",
                                avg_tokens, max_tokens
                            ))
                            .with_first_seen(first_seen)
                            .with_last_seen(last_seen),
                    );
                }
            }
        }
    }

    fn learn_low_token_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        thresholds: &PatternThresholds,
    ) {
        // Find low token usage executions
        let mut node_low_tokens: HashMap<&str, Vec<(usize, &NodeExecution)>> = HashMap::new();
        for (idx, exec) in self.nodes_executed.iter().enumerate() {
            if exec.tokens_used > 0 && exec.tokens_used <= thresholds.low_token_threshold {
                node_low_tokens
                    .entry(&exec.node)
                    .or_default()
                    .push((idx, exec));
            }
        }

        for (node, low_token_execs) in node_low_tokens {
            if low_token_execs.len() >= thresholds.min_frequency {
                let total_for_node = self
                    .nodes_executed
                    .iter()
                    .filter(|e| e.node == node)
                    .count();
                let confidence = low_token_execs.len() as f64 / total_for_node.max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    let first_seen = low_token_execs.first().map(|(i, _)| *i).unwrap_or(0);
                    let last_seen = low_token_execs.last().map(|(i, _)| *i).unwrap_or(0);
                    let avg_tokens = low_token_execs
                        .iter()
                        .map(|(_, e)| e.tokens_used)
                        .sum::<u64>()
                        / low_token_execs.len() as u64;

                    analysis.patterns.push(
                        Pattern::new(format!("low_tokens_{}", node), PatternType::LowTokenUsage)
                            .with_condition(PatternCondition::node_equals(node))
                            .with_condition(PatternCondition::tokens_lt(
                                thresholds.low_token_threshold,
                            ))
                            .with_frequency(low_token_execs.len())
                            .with_affected_node(node)
                            .with_confidence(confidence)
                            .with_description(format!(
                                "Node '{}' uses few tokens (<{} tokens)",
                                node, thresholds.low_token_threshold
                            ))
                            .with_evidence(format!("Avg: {} tokens", avg_tokens))
                            .with_first_seen(first_seen)
                            .with_last_seen(last_seen),
                    );
                }
            }
        }
    }

    fn learn_repeated_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        _thresholds: &PatternThresholds,
    ) {
        // Find nodes executed many times
        let mut node_counts: HashMap<&str, Vec<usize>> = HashMap::new();
        for (idx, exec) in self.nodes_executed.iter().enumerate() {
            node_counts.entry(&exec.node).or_default().push(idx);
        }

        let num_unique_nodes = node_counts.len();
        let avg_count = self.nodes_executed.len() as f64 / num_unique_nodes.max(1) as f64;

        for (node, indices) in node_counts {
            // Repeated means executed significantly more than average
            if indices.len() >= 5 && indices.len() as f64 > avg_count * 2.0 {
                let confidence = 0.7 + (indices.len() as f64 / 20.0).min(0.3);

                analysis.patterns.push(
                    Pattern::new(format!("repeated_{}", node), PatternType::Repeated)
                        .with_condition(PatternCondition::node_equals(node))
                        .with_condition(PatternCondition::count_gt(4))
                        .with_frequency(indices.len())
                        .with_affected_node(node)
                        .with_confidence(confidence)
                        .with_description(format!(
                            "Node '{}' is executed repeatedly ({} times)",
                            node,
                            indices.len()
                        ))
                        .with_evidence(format!("Average node execution count: {:.1}", avg_count))
                        .with_first_seen(*indices.first().unwrap_or(&0))
                        .with_last_seen(*indices.last().unwrap_or(&0)),
                );
            }
        }
    }

    fn learn_sequential_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        thresholds: &PatternThresholds,
    ) {
        // Find sequential patterns (A -> B)
        if self.nodes_executed.len() < 2 {
            return;
        }

        let mut sequence_counts: HashMap<(&str, &str), usize> = HashMap::new();
        for window in self.nodes_executed.windows(2) {
            let from = &window[0].node;
            let to = &window[1].node;
            if from != to {
                *sequence_counts.entry((from, to)).or_default() += 1;
            }
        }

        for ((from, to), count) in sequence_counts {
            if count >= thresholds.min_frequency {
                let total_from = self
                    .nodes_executed
                    .iter()
                    .filter(|e| e.node == from)
                    .count();
                let confidence = count as f64 / total_from.max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    analysis.patterns.push(
                        Pattern::new(format!("seq_{}_{}", from, to), PatternType::Sequential)
                            .with_frequency(count)
                            .with_affected_node(from)
                            .with_affected_node(to)
                            .with_confidence(confidence)
                            .with_description(format!("'{}' often followed by '{}'", from, to))
                            .with_evidence(format!("{} occurrences of {} -> {}", count, from, to)),
                    );
                }
            }
        }
    }

    fn learn_error_recovery_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        thresholds: &PatternThresholds,
    ) {
        // Find patterns where a failure is followed by success (same node)
        if self.nodes_executed.len() < 2 {
            return;
        }

        let mut recovery_counts: HashMap<&str, usize> = HashMap::new();
        for window in self.nodes_executed.windows(2) {
            let first = &window[0];
            let second = &window[1];
            if first.node == second.node && !first.success && second.success {
                *recovery_counts.entry(&first.node).or_default() += 1;
            }
        }

        for (node, count) in recovery_counts {
            if count >= thresholds.min_frequency {
                let failures = self
                    .nodes_executed
                    .iter()
                    .filter(|e| e.node == node && !e.success)
                    .count();
                let confidence = count as f64 / failures.max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    analysis.patterns.push(
                        Pattern::new(format!("recovery_{}", node), PatternType::ErrorRecovery)
                            .with_frequency(count)
                            .with_affected_node(node)
                            .with_confidence(confidence)
                            .with_description(format!(
                                "'{}' recovers from errors through retry",
                                node
                            ))
                            .with_evidence(format!(
                                "{} successful recoveries after failure",
                                count
                            )),
                    );
                }
            }
        }
    }

    fn learn_timeout_patterns(
        &self,
        analysis: &mut PatternAnalysis,
        thresholds: &PatternThresholds,
    ) {
        // Find executions that appear to have timed out
        let mut node_timeouts: HashMap<&str, Vec<(usize, &NodeExecution)>> = HashMap::new();
        for (idx, exec) in self.nodes_executed.iter().enumerate() {
            if exec.duration_ms >= thresholds.timeout_threshold_ms {
                node_timeouts
                    .entry(&exec.node)
                    .or_default()
                    .push((idx, exec));
            }
        }

        for (node, timeout_execs) in node_timeouts {
            if timeout_execs.len() >= thresholds.min_frequency {
                let total_for_node = self
                    .nodes_executed
                    .iter()
                    .filter(|e| e.node == node)
                    .count();
                let confidence = timeout_execs.len() as f64 / total_for_node.max(1) as f64;

                if confidence >= thresholds.min_confidence {
                    let first_seen = timeout_execs.first().map(|(i, _)| *i).unwrap_or(0);
                    let last_seen = timeout_execs.last().map(|(i, _)| *i).unwrap_or(0);
                    let max_duration = timeout_execs
                        .iter()
                        .map(|(_, e)| e.duration_ms)
                        .max()
                        .unwrap_or(0);

                    analysis.patterns.push(
                        Pattern::new(format!("timeout_{}", node), PatternType::Timeout)
                            .with_condition(PatternCondition::node_equals(node))
                            .with_condition(PatternCondition::duration_gt(
                                thresholds.timeout_threshold_ms,
                            ))
                            .with_frequency(timeout_execs.len())
                            .with_affected_node(node)
                            .with_confidence(confidence)
                            .with_description(format!(
                                "Node '{}' frequently times out (>{} ms)",
                                node, thresholds.timeout_threshold_ms
                            ))
                            .with_evidence(format!("Max duration: {} ms", max_duration))
                            .with_first_seen(first_seen)
                            .with_last_seen(last_seen),
                    );
                }
            }
        }
    }
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // PatternType Tests
    // ==========================================================================

    #[test]
    fn test_pattern_type_default() {
        let pt: PatternType = PatternType::default();
        assert_eq!(pt, PatternType::Success);
    }

    #[test]
    fn test_pattern_type_display() {
        assert_eq!(PatternType::Success.to_string(), "success");
        assert_eq!(PatternType::Failure.to_string(), "failure");
        assert_eq!(PatternType::Slow.to_string(), "slow");
        assert_eq!(PatternType::Efficient.to_string(), "efficient");
        assert_eq!(PatternType::HighTokenUsage.to_string(), "high_token_usage");
        assert_eq!(PatternType::LowTokenUsage.to_string(), "low_token_usage");
        assert_eq!(PatternType::Repeated.to_string(), "repeated");
        assert_eq!(PatternType::Sequential.to_string(), "sequential");
        assert_eq!(PatternType::ErrorRecovery.to_string(), "error_recovery");
        assert_eq!(PatternType::Timeout.to_string(), "timeout");
        assert_eq!(PatternType::Idle.to_string(), "idle");
        assert_eq!(PatternType::Burst.to_string(), "burst");
    }

    #[test]
    fn test_pattern_type_serialization() {
        let pt = PatternType::Efficient;
        let json = serde_json::to_string(&pt).unwrap();
        let deserialized: PatternType = serde_json::from_str(&json).unwrap();
        assert_eq!(pt, deserialized);
    }

    // ==========================================================================
    // PatternOperator Tests
    // ==========================================================================

    #[test]
    fn test_pattern_operator_compare_i64() {
        assert!(PatternOperator::Equals.compare_i64(10, 10));
        assert!(!PatternOperator::Equals.compare_i64(10, 11));

        assert!(PatternOperator::NotEquals.compare_i64(10, 11));
        assert!(!PatternOperator::NotEquals.compare_i64(10, 10));

        assert!(PatternOperator::GreaterThan.compare_i64(11, 10));
        assert!(!PatternOperator::GreaterThan.compare_i64(10, 10));

        assert!(PatternOperator::GreaterThanOrEqual.compare_i64(10, 10));
        assert!(PatternOperator::GreaterThanOrEqual.compare_i64(11, 10));
        assert!(!PatternOperator::GreaterThanOrEqual.compare_i64(9, 10));

        assert!(PatternOperator::LessThan.compare_i64(9, 10));
        assert!(!PatternOperator::LessThan.compare_i64(10, 10));

        assert!(PatternOperator::LessThanOrEqual.compare_i64(10, 10));
        assert!(PatternOperator::LessThanOrEqual.compare_i64(9, 10));
        assert!(!PatternOperator::LessThanOrEqual.compare_i64(11, 10));

        // Contains/Between not applicable for numeric
        assert!(!PatternOperator::Contains.compare_i64(10, 10));
        assert!(!PatternOperator::Between.compare_i64(10, 10));
    }

    #[test]
    fn test_pattern_operator_compare_f64() {
        assert!(PatternOperator::Equals.compare_f64(10.0, 10.0));
        assert!(!PatternOperator::Equals.compare_f64(10.0, 10.1));

        assert!(PatternOperator::NotEquals.compare_f64(10.0, 10.1));
        assert!(!PatternOperator::NotEquals.compare_f64(10.0, 10.0));

        assert!(PatternOperator::GreaterThan.compare_f64(10.1, 10.0));
        assert!(!PatternOperator::GreaterThan.compare_f64(10.0, 10.0));

        assert!(PatternOperator::LessThan.compare_f64(9.9, 10.0));
        assert!(!PatternOperator::LessThan.compare_f64(10.0, 10.0));

        // Contains/Between not applicable
        assert!(!PatternOperator::Contains.compare_f64(10.0, 10.0));
        assert!(!PatternOperator::Between.compare_f64(10.0, 10.0));
    }

    #[test]
    fn test_pattern_operator_display() {
        assert_eq!(PatternOperator::Equals.to_string(), "==");
        assert_eq!(PatternOperator::NotEquals.to_string(), "!=");
        assert_eq!(PatternOperator::GreaterThan.to_string(), ">");
        assert_eq!(PatternOperator::GreaterThanOrEqual.to_string(), ">=");
        assert_eq!(PatternOperator::LessThan.to_string(), "<");
        assert_eq!(PatternOperator::LessThanOrEqual.to_string(), "<=");
        assert_eq!(PatternOperator::Contains.to_string(), "contains");
        assert_eq!(PatternOperator::Between.to_string(), "between");
    }

    // ==========================================================================
    // PatternValue Tests
    // ==========================================================================

    #[test]
    fn test_pattern_value_display() {
        assert_eq!(PatternValue::Integer(42).to_string(), "42");
        assert_eq!(
            PatternValue::Float(314.0 / 100.0).to_string(),
            "3.14"
        );
        assert_eq!(PatternValue::Boolean(true).to_string(), "true");
        assert_eq!(PatternValue::Boolean(false).to_string(), "false");
        assert_eq!(PatternValue::String("test".into()).to_string(), "\"test\"");
        assert_eq!(PatternValue::IntegerRange(1, 10).to_string(), "[1, 10]");
        assert_eq!(
            PatternValue::FloatRange(1.0, 10.0).to_string(),
            "[1.00, 10.00]"
        );
    }

    #[test]
    fn test_pattern_value_serialization() {
        let values = vec![
            PatternValue::Integer(42),
            PatternValue::Float(314.0 / 100.0),
            PatternValue::Boolean(true),
            PatternValue::String("hello".into()),
            PatternValue::IntegerRange(1, 100),
            PatternValue::FloatRange(0.0, 1.0),
        ];

        for val in values {
            let json = serde_json::to_string(&val).unwrap();
            let deserialized: PatternValue = serde_json::from_str(&json).unwrap();
            assert_eq!(val, deserialized);
        }
    }

    // ==========================================================================
    // PatternCondition Tests
    // ==========================================================================

    #[test]
    fn test_pattern_condition_new() {
        let cond = PatternCondition::new(
            "test_field",
            PatternOperator::Equals,
            PatternValue::Integer(10),
        );
        assert_eq!(cond.field, "test_field");
        assert_eq!(cond.operator, PatternOperator::Equals);
        assert_eq!(cond.value, PatternValue::Integer(10));
    }

    #[test]
    fn test_pattern_condition_duration_gt() {
        let cond = PatternCondition::duration_gt(5000);
        assert_eq!(cond.field, "duration_ms");
        assert_eq!(cond.operator, PatternOperator::GreaterThan);
        assert_eq!(cond.value, PatternValue::Integer(5000));
    }

    #[test]
    fn test_pattern_condition_duration_lt() {
        let cond = PatternCondition::duration_lt(100);
        assert_eq!(cond.field, "duration_ms");
        assert_eq!(cond.operator, PatternOperator::LessThan);
        assert_eq!(cond.value, PatternValue::Integer(100));
    }

    #[test]
    fn test_pattern_condition_tokens_gt() {
        let cond = PatternCondition::tokens_gt(2000);
        assert_eq!(cond.field, "tokens_used");
        assert_eq!(cond.operator, PatternOperator::GreaterThan);
        assert_eq!(cond.value, PatternValue::Integer(2000));
    }

    #[test]
    fn test_pattern_condition_tokens_lt() {
        let cond = PatternCondition::tokens_lt(100);
        assert_eq!(cond.field, "tokens_used");
        assert_eq!(cond.operator, PatternOperator::LessThan);
        assert_eq!(cond.value, PatternValue::Integer(100));
    }

    #[test]
    fn test_pattern_condition_is_success() {
        let cond = PatternCondition::is_success();
        assert_eq!(cond.field, "success");
        assert_eq!(cond.operator, PatternOperator::Equals);
        assert_eq!(cond.value, PatternValue::Boolean(true));
    }

    #[test]
    fn test_pattern_condition_is_failure() {
        let cond = PatternCondition::is_failure();
        assert_eq!(cond.field, "success");
        assert_eq!(cond.operator, PatternOperator::Equals);
        assert_eq!(cond.value, PatternValue::Boolean(false));
    }

    #[test]
    fn test_pattern_condition_node_equals() {
        let cond = PatternCondition::node_equals("my_node");
        assert_eq!(cond.field, "node");
        assert_eq!(cond.operator, PatternOperator::Equals);
        assert_eq!(cond.value, PatternValue::String("my_node".into()));
    }

    #[test]
    fn test_pattern_condition_count_gt() {
        let cond = PatternCondition::count_gt(5);
        assert_eq!(cond.field, "count");
        assert_eq!(cond.operator, PatternOperator::GreaterThan);
        assert_eq!(cond.value, PatternValue::Integer(5));
    }

    #[test]
    fn test_pattern_condition_matches_duration() {
        let exec = NodeExecution::new("test_node", 1000);

        let cond_gt = PatternCondition::duration_gt(500);
        assert!(cond_gt.matches(&exec));

        let cond_lt = PatternCondition::duration_lt(500);
        assert!(!cond_lt.matches(&exec));

        let cond_lt2 = PatternCondition::duration_lt(2000);
        assert!(cond_lt2.matches(&exec));
    }

    #[test]
    fn test_pattern_condition_matches_tokens() {
        let exec = NodeExecution::new("test_node", 0).with_tokens(500);

        let cond_gt = PatternCondition::tokens_gt(200);
        assert!(cond_gt.matches(&exec));

        let cond_lt = PatternCondition::tokens_lt(200);
        assert!(!cond_lt.matches(&exec));
    }

    #[test]
    fn test_pattern_condition_matches_success() {
        let success_exec = NodeExecution::new("test", 0); // success defaults to true
        let failure_exec = NodeExecution::new("test", 0).with_error("failed");

        let cond_success = PatternCondition::is_success();
        assert!(cond_success.matches(&success_exec));
        assert!(!cond_success.matches(&failure_exec));

        let cond_failure = PatternCondition::is_failure();
        assert!(!cond_failure.matches(&success_exec));
        assert!(cond_failure.matches(&failure_exec));
    }

    #[test]
    fn test_pattern_condition_matches_node() {
        let exec = NodeExecution::new("llm_call", 0);

        let cond_eq = PatternCondition::node_equals("llm_call");
        assert!(cond_eq.matches(&exec));

        let cond_neq = PatternCondition::node_equals("other_node");
        assert!(!cond_neq.matches(&exec));

        // Contains operator
        let cond_contains = PatternCondition::new(
            "node",
            PatternOperator::Contains,
            PatternValue::String("llm".into()),
        );
        assert!(cond_contains.matches(&exec));
    }

    #[test]
    fn test_pattern_condition_matches_unknown_field() {
        let exec = NodeExecution::new("test", 0);
        let cond = PatternCondition::new(
            "unknown_field",
            PatternOperator::Equals,
            PatternValue::Integer(10),
        );
        assert!(!cond.matches(&exec));
    }

    #[test]
    fn test_pattern_condition_to_json() {
        let cond = PatternCondition::duration_gt(5000);
        let json = cond.to_json().unwrap();
        assert!(json.contains("duration_ms"));
        assert!(json.contains("GreaterThan"));
        assert!(json.contains("5000"));
    }

    // ==========================================================================
    // Pattern Tests
    // ==========================================================================

    #[test]
    fn test_pattern_new() {
        let pattern = Pattern::new("test_pattern", PatternType::Slow);
        assert_eq!(pattern.id, "test_pattern");
        assert_eq!(pattern.pattern_type, PatternType::Slow);
        assert_eq!(pattern.frequency, 1);
        assert_eq!(pattern.confidence, 0.5);
        assert!(pattern.conditions.is_empty());
        assert!(pattern.affected_nodes.is_empty());
    }

    #[test]
    fn test_pattern_with_condition() {
        let pattern = Pattern::new("test", PatternType::Success)
            .with_condition(PatternCondition::is_success())
            .with_condition(PatternCondition::duration_lt(100));
        assert_eq!(pattern.conditions.len(), 2);
    }

    #[test]
    fn test_pattern_with_frequency() {
        let pattern = Pattern::new("test", PatternType::Success).with_frequency(10);
        assert_eq!(pattern.frequency, 10);
    }

    #[test]
    fn test_pattern_with_affected_node() {
        let pattern = Pattern::new("test", PatternType::Success)
            .with_affected_node("node1")
            .with_affected_node("node2");
        assert_eq!(pattern.affected_nodes, vec!["node1", "node2"]);
    }

    #[test]
    fn test_pattern_with_confidence_clamped() {
        // Confidence should be clamped to [0.0, 1.0]
        let pattern_high = Pattern::new("test", PatternType::Success).with_confidence(1.5);
        assert!((pattern_high.confidence - 1.0).abs() < f64::EPSILON);

        let pattern_low = Pattern::new("test", PatternType::Success).with_confidence(-0.5);
        assert!(pattern_low.confidence.abs() < f64::EPSILON);

        let pattern_normal = Pattern::new("test", PatternType::Success).with_confidence(0.75);
        assert!((pattern_normal.confidence - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pattern_with_description() {
        let pattern =
            Pattern::new("test", PatternType::Success).with_description("A test description");
        assert_eq!(pattern.description, "A test description");
    }

    #[test]
    fn test_pattern_with_evidence() {
        let pattern = Pattern::new("test", PatternType::Success)
            .with_evidence("Evidence 1")
            .with_evidence("Evidence 2");
        assert_eq!(pattern.evidence, vec!["Evidence 1", "Evidence 2"]);
    }

    #[test]
    fn test_pattern_with_first_last_seen() {
        let pattern = Pattern::new("test", PatternType::Success)
            .with_first_seen(5)
            .with_last_seen(15);
        assert_eq!(pattern.first_seen, 5);
        assert_eq!(pattern.last_seen, 15);
    }

    #[test]
    fn test_pattern_matches() {
        let pattern = Pattern::new("test", PatternType::Efficient)
            .with_condition(PatternCondition::is_success())
            .with_condition(PatternCondition::duration_lt(100));

        let good_exec = NodeExecution::new("node", 50); // success defaults to true
        assert!(pattern.matches(&good_exec));

        let slow_exec = NodeExecution::new("node", 200); // success defaults to true
        assert!(!pattern.matches(&slow_exec));

        let failed_exec = NodeExecution::new("node", 50).with_error("failed");
        assert!(!pattern.matches(&failed_exec));
    }

    #[test]
    fn test_pattern_matches_empty_conditions() {
        let pattern = Pattern::new("test", PatternType::Success);
        let exec = NodeExecution::new("any_node", 0);
        // Empty conditions means all match (vacuous truth)
        assert!(pattern.matches(&exec));
    }

    #[test]
    fn test_pattern_matches_trace() {
        let pattern = Pattern::new("test", PatternType::Slow)
            .with_condition(PatternCondition::duration_gt(1000));

        let mut trace = ExecutionTrace::new();
        trace
            .nodes_executed
            .push(NodeExecution::new("fast_node", 100));
        trace
            .nodes_executed
            .push(NodeExecution::new("slow_node", 2000));

        assert!(pattern.matches_trace(&trace));

        // Trace with only fast nodes
        let mut fast_trace = ExecutionTrace::new();
        fast_trace
            .nodes_executed
            .push(NodeExecution::new("fast_node", 100));
        assert!(!pattern.matches_trace(&fast_trace));
    }

    #[test]
    fn test_pattern_count_matches() {
        let pattern = Pattern::new("test", PatternType::Success)
            .with_condition(PatternCondition::is_success());

        let mut trace = ExecutionTrace::new();
        trace.nodes_executed.push(NodeExecution::new("n1", 0)); // success=true
        trace
            .nodes_executed
            .push(NodeExecution::new("n2", 0).with_error("err")); // success=false
        trace.nodes_executed.push(NodeExecution::new("n3", 0)); // success=true
        trace.nodes_executed.push(NodeExecution::new("n4", 0)); // success=true

        assert_eq!(pattern.count_matches(&trace), 3);
    }

    #[test]
    fn test_pattern_is_negative() {
        assert!(Pattern::new("t", PatternType::Failure).is_negative());
        assert!(Pattern::new("t", PatternType::Slow).is_negative());
        assert!(Pattern::new("t", PatternType::HighTokenUsage).is_negative());
        assert!(Pattern::new("t", PatternType::Timeout).is_negative());

        assert!(!Pattern::new("t", PatternType::Success).is_negative());
        assert!(!Pattern::new("t", PatternType::Efficient).is_negative());
    }

    #[test]
    fn test_pattern_is_positive() {
        assert!(Pattern::new("t", PatternType::Success).is_positive());
        assert!(Pattern::new("t", PatternType::Efficient).is_positive());
        assert!(Pattern::new("t", PatternType::LowTokenUsage).is_positive());
        assert!(Pattern::new("t", PatternType::ErrorRecovery).is_positive());

        assert!(!Pattern::new("t", PatternType::Failure).is_positive());
        assert!(!Pattern::new("t", PatternType::Timeout).is_positive());
    }

    #[test]
    fn test_pattern_summary() {
        let pattern = Pattern::new("slow_llm", PatternType::Slow)
            .with_frequency(5)
            .with_confidence(0.8)
            .with_description("LLM is slow");

        let summary = pattern.summary();
        assert!(summary.contains("slow"));
        assert!(summary.contains("slow_llm"));
        assert!(summary.contains("5"));
        assert!(summary.contains("80%"));
        assert!(summary.contains("LLM is slow"));
    }

    #[test]
    fn test_pattern_json_roundtrip() {
        let pattern = Pattern::new("test_pattern", PatternType::Efficient)
            .with_condition(PatternCondition::is_success())
            .with_frequency(10)
            .with_affected_node("node1")
            .with_confidence(0.9)
            .with_description("Efficient pattern")
            .with_evidence("High speed observed")
            .with_first_seen(0)
            .with_last_seen(100);

        let json = pattern.to_json().unwrap();
        let restored = Pattern::from_json(&json).unwrap();

        assert_eq!(pattern.id, restored.id);
        assert_eq!(pattern.pattern_type, restored.pattern_type);
        assert_eq!(pattern.frequency, restored.frequency);
        assert_eq!(pattern.affected_nodes, restored.affected_nodes);
        assert!((pattern.confidence - restored.confidence).abs() < f64::EPSILON);
    }

    // ==========================================================================
    // PatternBuilder Tests
    // ==========================================================================

    #[test]
    fn test_pattern_builder_new() {
        let builder = PatternBuilder::new();
        // Default values
        assert_eq!(builder.frequency, 1);
        assert!((builder.confidence - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pattern_builder_fluent_api() {
        let result = Pattern::builder()
            .id("builder_test")
            .pattern_type(PatternType::Efficient)
            .condition(PatternCondition::is_success())
            .frequency(10)
            .affected_node("node1")
            .confidence(0.95)
            .description("Built pattern")
            .evidence("Evidence line")
            .first_seen(5)
            .last_seen(50)
            .build();

        assert!(result.is_ok());
        let pattern = result.unwrap();
        assert_eq!(pattern.id, "builder_test");
        assert_eq!(pattern.pattern_type, PatternType::Efficient);
        assert_eq!(pattern.conditions.len(), 1);
        assert_eq!(pattern.frequency, 10);
        assert_eq!(pattern.affected_nodes, vec!["node1"]);
        assert!((pattern.confidence - 0.95).abs() < f64::EPSILON);
        assert_eq!(pattern.description, "Built pattern");
        assert_eq!(pattern.evidence, vec!["Evidence line"]);
    }

    #[test]
    fn test_pattern_builder_missing_id() {
        let result = Pattern::builder()
            .pattern_type(PatternType::Success)
            .description("Test")
            .build();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "id is required");
    }

    #[test]
    fn test_pattern_builder_missing_description() {
        let result = Pattern::builder()
            .id("test")
            .pattern_type(PatternType::Success)
            .build();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "description is required");
    }

    #[test]
    fn test_pattern_builder_confidence_clamped() {
        let pattern = Pattern::builder()
            .id("test")
            .description("test")
            .confidence(2.0)
            .build()
            .unwrap();

        assert!((pattern.confidence - 1.0).abs() < f64::EPSILON);
    }

    // ==========================================================================
    // PatternAnalysis Tests
    // ==========================================================================

    #[test]
    fn test_pattern_analysis_new() {
        let analysis = PatternAnalysis::new();
        assert!(analysis.patterns.is_empty());
        assert_eq!(analysis.executions_analyzed, 0);
        assert_eq!(analysis.patterns_learned, 0);
        assert!(analysis.summary.is_empty());
    }

    #[test]
    fn test_pattern_analysis_default() {
        let analysis = PatternAnalysis::default();
        assert!(!analysis.has_patterns());
        assert_eq!(analysis.pattern_count(), 0);
    }

    #[test]
    fn test_pattern_analysis_has_patterns() {
        let mut analysis = PatternAnalysis::new();
        assert!(!analysis.has_patterns());

        analysis
            .patterns
            .push(Pattern::new("test", PatternType::Success));
        assert!(analysis.has_patterns());
    }

    #[test]
    fn test_pattern_analysis_by_type() {
        let mut analysis = PatternAnalysis::new();
        analysis
            .patterns
            .push(Pattern::new("s1", PatternType::Success));
        analysis
            .patterns
            .push(Pattern::new("s2", PatternType::Success));
        analysis
            .patterns
            .push(Pattern::new("f1", PatternType::Failure));

        let successes = analysis.by_type(&PatternType::Success);
        assert_eq!(successes.len(), 2);

        let failures = analysis.by_type(&PatternType::Failure);
        assert_eq!(failures.len(), 1);

        let slows = analysis.by_type(&PatternType::Slow);
        assert!(slows.is_empty());
    }

    #[test]
    fn test_pattern_analysis_negative_positive_patterns() {
        let mut analysis = PatternAnalysis::new();
        analysis
            .patterns
            .push(Pattern::new("s", PatternType::Success));
        analysis
            .patterns
            .push(Pattern::new("e", PatternType::Efficient));
        analysis
            .patterns
            .push(Pattern::new("f", PatternType::Failure));
        analysis
            .patterns
            .push(Pattern::new("t", PatternType::Timeout));

        let positive = analysis.positive_patterns();
        assert_eq!(positive.len(), 2);

        let negative = analysis.negative_patterns();
        assert_eq!(negative.len(), 2);
    }

    #[test]
    fn test_pattern_analysis_for_node() {
        let mut analysis = PatternAnalysis::new();
        analysis
            .patterns
            .push(Pattern::new("p1", PatternType::Success).with_affected_node("node_a"));
        analysis.patterns.push(
            Pattern::new("p2", PatternType::Slow)
                .with_affected_node("node_a")
                .with_affected_node("node_b"),
        );
        analysis
            .patterns
            .push(Pattern::new("p3", PatternType::Failure).with_affected_node("node_c"));

        let node_a = analysis.for_node("node_a");
        assert_eq!(node_a.len(), 2);

        let node_b = analysis.for_node("node_b");
        assert_eq!(node_b.len(), 1);

        let node_c = analysis.for_node("node_c");
        assert_eq!(node_c.len(), 1);

        let node_x = analysis.for_node("node_x");
        assert!(node_x.is_empty());
    }

    #[test]
    fn test_pattern_analysis_most_frequent() {
        let mut analysis = PatternAnalysis::new();
        analysis
            .patterns
            .push(Pattern::new("p1", PatternType::Success).with_frequency(5));
        analysis
            .patterns
            .push(Pattern::new("p2", PatternType::Success).with_frequency(10));
        analysis
            .patterns
            .push(Pattern::new("p3", PatternType::Success).with_frequency(3));

        let top2 = analysis.most_frequent(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].frequency, 10);
        assert_eq!(top2[1].frequency, 5);
    }

    #[test]
    fn test_pattern_analysis_highest_confidence() {
        let mut analysis = PatternAnalysis::new();
        analysis
            .patterns
            .push(Pattern::new("p1", PatternType::Success).with_confidence(0.5));
        analysis
            .patterns
            .push(Pattern::new("p2", PatternType::Success).with_confidence(0.9));
        analysis
            .patterns
            .push(Pattern::new("p3", PatternType::Success).with_confidence(0.7));

        let top2 = analysis.highest_confidence(2);
        assert_eq!(top2.len(), 2);
        assert!((top2[0].confidence - 0.9).abs() < f64::EPSILON);
        assert!((top2[1].confidence - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pattern_analysis_match_trace() {
        let mut analysis = PatternAnalysis::new();
        analysis.patterns.push(
            Pattern::new("slow_pattern", PatternType::Slow)
                .with_condition(PatternCondition::duration_gt(1000)),
        );
        analysis.patterns.push(
            Pattern::new("success_pattern", PatternType::Success)
                .with_condition(PatternCondition::is_success()),
        );

        let mut trace = ExecutionTrace::new();
        trace.nodes_executed.push(NodeExecution::new("node1", 500)); // success=true

        let matches = analysis.match_trace(&trace);
        // Should match success_pattern but not slow_pattern
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "success_pattern");
    }

    #[test]
    fn test_pattern_analysis_json_roundtrip() {
        let mut analysis = PatternAnalysis::new();
        analysis.executions_analyzed = 100;
        analysis.patterns_learned = 3;
        analysis.summary = "Test summary".to_string();
        analysis
            .patterns
            .push(Pattern::new("p1", PatternType::Success));

        let json = analysis.to_json().unwrap();
        let restored = PatternAnalysis::from_json(&json).unwrap();

        assert_eq!(analysis.executions_analyzed, restored.executions_analyzed);
        assert_eq!(analysis.patterns_learned, restored.patterns_learned);
        assert_eq!(analysis.patterns.len(), restored.patterns.len());
    }

    // ==========================================================================
    // PatternThresholds Tests
    // ==========================================================================

    #[test]
    fn test_pattern_thresholds_default() {
        let thresholds = PatternThresholds::default();
        assert_eq!(thresholds.slow_duration_ms, 5000);
        assert_eq!(thresholds.efficient_duration_ms, 100);
        assert_eq!(thresholds.high_token_threshold, 2000);
        assert_eq!(thresholds.low_token_threshold, 100);
        assert_eq!(thresholds.min_frequency, 2);
        assert!((thresholds.min_confidence - 0.3).abs() < f64::EPSILON);
        assert_eq!(thresholds.timeout_threshold_ms, 30000);
    }

    #[test]
    fn test_pattern_thresholds_new() {
        let thresholds = PatternThresholds::new();
        assert_eq!(thresholds.slow_duration_ms, 5000);
    }

    #[test]
    fn test_pattern_thresholds_builder_methods() {
        let thresholds = PatternThresholds::new()
            .with_slow_duration(10000)
            .with_efficient_duration(50)
            .with_high_token_threshold(5000)
            .with_low_token_threshold(50)
            .with_min_frequency(5)
            .with_min_confidence(0.5)
            .with_timeout_threshold(60000);

        assert_eq!(thresholds.slow_duration_ms, 10000);
        assert_eq!(thresholds.efficient_duration_ms, 50);
        assert_eq!(thresholds.high_token_threshold, 5000);
        assert_eq!(thresholds.low_token_threshold, 50);
        assert_eq!(thresholds.min_frequency, 5);
        assert!((thresholds.min_confidence - 0.5).abs() < f64::EPSILON);
        assert_eq!(thresholds.timeout_threshold_ms, 60000);
    }

    #[test]
    fn test_pattern_thresholds_confidence_clamped() {
        let thresholds = PatternThresholds::new().with_min_confidence(1.5);
        assert!((thresholds.min_confidence - 1.0).abs() < f64::EPSILON);

        let thresholds2 = PatternThresholds::new().with_min_confidence(-0.5);
        assert!(thresholds2.min_confidence.abs() < f64::EPSILON);
    }

    // ==========================================================================
    // ExecutionTrace.learn_patterns Tests
    // ==========================================================================

    #[test]
    fn test_learn_patterns_empty_trace() {
        let trace = ExecutionTrace::new();
        let analysis = trace.learn_patterns();

        assert!(!analysis.has_patterns());
        assert_eq!(analysis.executions_analyzed, 0);
        assert!(analysis.summary.contains("No patterns learned"));
    }

    #[test]
    fn test_learn_patterns_success_pattern() {
        let mut trace = ExecutionTrace::new();
        // Add multiple successful executions of the same node
        for _ in 0..5 {
            trace
                .nodes_executed
                .push(NodeExecution::new("reliable_node", 100)); // success defaults to true
        }

        let analysis = trace.learn_patterns();

        let success_patterns = analysis.by_type(&PatternType::Success);
        assert!(!success_patterns.is_empty());
        assert!(success_patterns
            .iter()
            .any(|p| p.id.contains("reliable_node")));
    }

    #[test]
    fn test_learn_patterns_failure_pattern() {
        let mut trace = ExecutionTrace::new();
        // Add multiple failed executions
        for _ in 0..5 {
            trace
                .nodes_executed
                .push(NodeExecution::new("flaky_node", 0).with_error("Connection timeout"));
        }

        let analysis = trace.learn_patterns();

        let failure_patterns = analysis.by_type(&PatternType::Failure);
        assert!(!failure_patterns.is_empty());
    }

    #[test]
    fn test_learn_patterns_slow_pattern() {
        let mut trace = ExecutionTrace::new();
        // Add slow executions exceeding default threshold (5000ms)
        for _ in 0..3 {
            trace
                .nodes_executed
                .push(NodeExecution::new("slow_node", 6000));
        }

        let analysis = trace.learn_patterns();

        let slow_patterns = analysis.by_type(&PatternType::Slow);
        assert!(!slow_patterns.is_empty());
        assert!(slow_patterns[0].description.contains("slow"));
    }

    #[test]
    fn test_learn_patterns_efficient_pattern() {
        let mut trace = ExecutionTrace::new();
        // Add efficient executions under threshold (100ms)
        for _ in 0..5 {
            trace
                .nodes_executed
                .push(NodeExecution::new("fast_node", 50));
        }

        let analysis = trace.learn_patterns();

        let efficient_patterns = analysis.by_type(&PatternType::Efficient);
        assert!(!efficient_patterns.is_empty());
    }

    #[test]
    fn test_learn_patterns_high_token_usage() {
        let mut trace = ExecutionTrace::new();
        // Add high token usage executions (>2000 default)
        for _ in 0..3 {
            trace
                .nodes_executed
                .push(NodeExecution::new("llm_node", 0).with_tokens(3000));
        }

        let analysis = trace.learn_patterns();

        let high_token_patterns = analysis.by_type(&PatternType::HighTokenUsage);
        assert!(!high_token_patterns.is_empty());
    }

    #[test]
    fn test_learn_patterns_low_token_usage() {
        let mut trace = ExecutionTrace::new();
        // Add low token usage executions (1-100)
        for _ in 0..5 {
            trace
                .nodes_executed
                .push(NodeExecution::new("simple_node", 0).with_tokens(50));
        }

        let analysis = trace.learn_patterns();

        let low_token_patterns = analysis.by_type(&PatternType::LowTokenUsage);
        assert!(!low_token_patterns.is_empty());
    }

    #[test]
    fn test_learn_patterns_repeated_pattern() {
        let mut trace = ExecutionTrace::new();
        // Add many executions of same node - needs to be > 2x average
        // With 15 repeated + 3 other nodes = 18 total, 4 unique nodes, avg = 4.5
        // 15 > 4.5 * 2 = 9, so 15 > 9 is true
        for _ in 0..15 {
            trace
                .nodes_executed
                .push(NodeExecution::new("repeated_node", 0));
        }
        // Add some other nodes to create variety
        trace
            .nodes_executed
            .push(NodeExecution::new("other_node_a", 0));
        trace
            .nodes_executed
            .push(NodeExecution::new("other_node_b", 0));
        trace
            .nodes_executed
            .push(NodeExecution::new("other_node_c", 0));

        let analysis = trace.learn_patterns();

        let repeated_patterns = analysis.by_type(&PatternType::Repeated);
        assert!(!repeated_patterns.is_empty());
        assert!(repeated_patterns
            .iter()
            .any(|p| p.id.contains("repeated_node")));
    }

    #[test]
    fn test_learn_patterns_sequential_pattern() {
        let mut trace = ExecutionTrace::new();
        // Add consistent sequence A -> B multiple times
        for _ in 0..5 {
            trace.nodes_executed.push(NodeExecution::new("node_a", 0));
            trace.nodes_executed.push(NodeExecution::new("node_b", 0));
        }

        let analysis = trace.learn_patterns();

        let sequential_patterns = analysis.by_type(&PatternType::Sequential);
        assert!(!sequential_patterns.is_empty());
        assert!(sequential_patterns.iter().any(|p| {
            p.affected_nodes.contains(&"node_a".to_string())
                && p.affected_nodes.contains(&"node_b".to_string())
        }));
    }

    #[test]
    fn test_learn_patterns_error_recovery() {
        let mut trace = ExecutionTrace::new();
        // Add fail-then-succeed pattern multiple times
        for _ in 0..3 {
            trace
                .nodes_executed
                .push(NodeExecution::new("retry_node", 0).with_error("err"));
            trace
                .nodes_executed
                .push(NodeExecution::new("retry_node", 0));
        }

        let analysis = trace.learn_patterns();

        let recovery_patterns = analysis.by_type(&PatternType::ErrorRecovery);
        assert!(!recovery_patterns.is_empty());
        assert!(recovery_patterns[0].description.contains("recover"));
    }

    #[test]
    fn test_learn_patterns_timeout_pattern() {
        let mut trace = ExecutionTrace::new();
        // Add timeout executions (>30000ms default)
        for _ in 0..3 {
            trace
                .nodes_executed
                .push(NodeExecution::new("hanging_node", 35000).with_error("timeout"));
        }

        let analysis = trace.learn_patterns();

        let timeout_patterns = analysis.by_type(&PatternType::Timeout);
        assert!(!timeout_patterns.is_empty());
    }

    #[test]
    fn test_learn_patterns_with_custom_thresholds() {
        let mut trace = ExecutionTrace::new();
        // Add executions that would be "slow" with custom threshold but not default
        for _ in 0..3 {
            trace
                .nodes_executed
                .push(NodeExecution::new("medium_node", 200));
        }

        // With default thresholds (5000ms), this shouldn't be slow
        let default_analysis = trace.learn_patterns();
        let default_slow = default_analysis.by_type(&PatternType::Slow);
        assert!(default_slow.is_empty());

        // With custom threshold (100ms), this should be slow
        let custom_thresholds = PatternThresholds::new().with_slow_duration(100);
        let custom_analysis = trace.learn_patterns_with_thresholds(&custom_thresholds);
        let custom_slow = custom_analysis.by_type(&PatternType::Slow);
        assert!(!custom_slow.is_empty());
    }

    #[test]
    fn test_learn_patterns_summary_generation() {
        let mut trace = ExecutionTrace::new();
        for _ in 0..5 {
            trace.nodes_executed.push(NodeExecution::new("node", 0));
        }
        for _ in 0..3 {
            trace.nodes_executed.push(NodeExecution::new("slow", 6000));
        }

        let analysis = trace.learn_patterns();

        assert!(!analysis.summary.is_empty());
        assert!(analysis.summary.contains("Learned"));
        assert!(analysis.summary.contains("patterns"));
    }

    #[test]
    fn test_learn_patterns_sorted_by_confidence() {
        let mut trace = ExecutionTrace::new();
        // Create scenarios with different confidence levels
        for _ in 0..10 {
            trace
                .nodes_executed
                .push(NodeExecution::new("consistent_node", 50));
        }
        // Add some variability for another node
        for _ in 0..3 {
            trace
                .nodes_executed
                .push(NodeExecution::new("variable_node", 6000));
        }
        trace
            .nodes_executed
            .push(NodeExecution::new("variable_node", 100));

        let analysis = trace.learn_patterns();

        // Patterns should be sorted by confidence (highest first)
        if analysis.patterns.len() >= 2 {
            assert!(analysis.patterns[0].confidence >= analysis.patterns[1].confidence);
        }
    }

    #[test]
    fn test_learn_patterns_below_frequency_threshold() {
        let mut trace = ExecutionTrace::new();
        // Add only one execution - below default min_frequency of 2
        trace
            .nodes_executed
            .push(NodeExecution::new("single_node", 6000));

        let analysis = trace.learn_patterns();

        // Should not learn patterns from single occurrence
        let slow_patterns = analysis.by_type(&PatternType::Slow);
        assert!(slow_patterns.is_empty());
    }

    #[test]
    fn test_learn_patterns_below_confidence_threshold() {
        let mut trace = ExecutionTrace::new();
        // Add 10 executions with only 1 slow (10% rate, below 30% default min_confidence)
        for _ in 0..9 {
            trace
                .nodes_executed
                .push(NodeExecution::new("mostly_fast", 50));
        }
        trace
            .nodes_executed
            .push(NodeExecution::new("mostly_fast", 6000));

        let thresholds = PatternThresholds::new()
            .with_min_frequency(1) // Allow single occurrence
            .with_min_confidence(0.3); // But require 30% confidence

        let analysis = trace.learn_patterns_with_thresholds(&thresholds);

        // 10% slow rate should not generate slow pattern with 30% min_confidence
        let slow_patterns = analysis.by_type(&PatternType::Slow);
        assert!(slow_patterns.is_empty());
    }
}
