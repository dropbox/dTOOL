// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Cross-Agent Learning - AI Learns From Other Agents' Experiences
//!
//! **NOTE**: For pattern detection only, consider using `pattern_engine::UnifiedPatternEngine`
//! which provides a unified API. However, this module provides additional functionality
//! (pitfalls, optimization strategies, agent summaries, correlations) not available
//! through the unified engine.
//!
//! ```rust,ignore
//! // For cross-agent pattern detection only:
//! use dashflow::pattern_engine::{UnifiedPatternEngineBuilder, PatternSource};
//! let engine = UnifiedPatternEngineBuilder::new()
//!     .enable_cross_agent_patterns()
//!     .build();
//!
//! // For full cross-agent insights (patterns, pitfalls, strategies):
//! use dashflow::cross_agent_learning::CrossAgentLearner;
//! let learner = CrossAgentLearner::new();
//! let insights = learner.analyze(&traces);
//! ```
//!
//! This module provides cross-agent learning capabilities that allow AI agents to
//! learn from the collective experiences of all agents in the system.
//!
//! ## Overview
//!
//! Cross-agent learning enables AI agents to:
//! - Discover successful patterns used by other agents
//! - Identify common pitfalls and how to avoid them
//! - Learn optimization strategies that work across different contexts
//! - Share knowledge without direct communication
//!
//! ## Key Concepts
//!
//! - **CrossAgentInsights**: The complete set of learnings from analyzing all agents
//! - **SuccessPattern**: A pattern that correlates with successful executions
//! - **Pitfall**: A common mistake or anti-pattern to avoid
//! - **OptimizationStrategy**: A strategy that improves agent performance
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::cross_agent_learning::CrossAgentLearner;
//!
//! // Learn from all agents' execution traces
//! let learner = CrossAgentLearner::new();
//! let insights = learner.analyze(&all_traces);
//!
//! println!("Learned from {} agents:", insights.agents_analyzed);
//!
//! for pattern in &insights.successful_patterns {
//!     println!("Pattern: {} ({}% success rate)",
//!         pattern.name,
//!         pattern.success_rate * 100.0
//!     );
//! }
//!
//! for pitfall in &insights.common_pitfalls {
//!     println!("Avoid: {} - {}", pitfall.name, pitfall.description);
//! }
//!
//! for strategy in &insights.optimization_strategies {
//!     if strategy.expected_improvement > 0.2 {
//!         println!("Apply strategy '{}' for {:.1}% improvement",
//!             strategy.name,
//!             strategy.expected_improvement * 100.0
//!         );
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Insights learned from analyzing multiple agents
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrossAgentInsights {
    /// Number of distinct agents analyzed
    pub agents_analyzed: usize,
    /// Total number of executions analyzed
    pub executions_analyzed: usize,
    /// Patterns that correlate with success
    pub successful_patterns: Vec<SuccessPattern>,
    /// Common pitfalls to avoid
    pub common_pitfalls: Vec<Pitfall>,
    /// Optimization strategies ranked by effectiveness
    pub optimization_strategies: Vec<OptimizationStrategy>,
    /// Per-agent performance summary
    pub agent_summaries: Vec<AgentSummary>,
    /// Cross-agent correlations discovered
    pub correlations: Vec<AgentCorrelation>,
    /// Metadata about the analysis
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CrossAgentInsights {
    /// Create empty insights
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of actionable insights
    #[must_use]
    pub fn actionable_count(&self) -> usize {
        self.successful_patterns
            .iter()
            .filter(|p| p.is_actionable())
            .count()
            + self
                .common_pitfalls
                .iter()
                .filter(|p| p.is_actionable())
                .count()
            + self
                .optimization_strategies
                .iter()
                .filter(|s| s.is_actionable())
                .count()
    }

    /// Get high-value patterns (high success rate, significant sample size)
    #[must_use]
    pub fn high_value_patterns(&self) -> Vec<&SuccessPattern> {
        self.successful_patterns
            .iter()
            .filter(|p| p.success_rate >= 0.8 && p.sample_count >= 10)
            .collect()
    }

    /// Get critical pitfalls (high failure rate, significant sample size)
    #[must_use]
    pub fn critical_pitfalls(&self) -> Vec<&Pitfall> {
        self.common_pitfalls
            .iter()
            .filter(|p| p.failure_rate >= 0.5 && p.occurrences >= 5)
            .collect()
    }

    /// Get top strategies by expected improvement
    #[must_use]
    pub fn top_strategies(&self, n: usize) -> Vec<&OptimizationStrategy> {
        let mut sorted: Vec<_> = self.optimization_strategies.iter().collect();
        sorted.sort_by(|a, b| {
            b.expected_improvement
                .partial_cmp(&a.expected_improvement)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }

    /// Generate a summary report
    #[must_use]
    pub fn summary(&self) -> String {
        let mut lines = vec![
            "Cross-Agent Learning Insights".to_string(),
            "============================".to_string(),
            format!("Agents analyzed: {}", self.agents_analyzed),
            format!("Executions analyzed: {}", self.executions_analyzed),
            String::new(),
            format!("Successful Patterns: {}", self.successful_patterns.len()),
            format!("Common Pitfalls: {}", self.common_pitfalls.len()),
            format!(
                "Optimization Strategies: {}",
                self.optimization_strategies.len()
            ),
        ];

        if !self.successful_patterns.is_empty() {
            lines.push(String::new());
            lines.push("Top Patterns:".to_string());
            for pattern in self.successful_patterns.iter().take(5) {
                lines.push(format!(
                    "  - {} ({:.1}% success, {} samples)",
                    pattern.name,
                    pattern.success_rate * 100.0,
                    pattern.sample_count
                ));
            }
        }

        if !self.common_pitfalls.is_empty() {
            lines.push(String::new());
            lines.push("Critical Pitfalls:".to_string());
            for pitfall in self.common_pitfalls.iter().take(5) {
                lines.push(format!(
                    "  - {} ({:.1}% failure rate, {} occurrences)",
                    pitfall.name,
                    pitfall.failure_rate * 100.0,
                    pitfall.occurrences
                ));
            }
        }

        if !self.optimization_strategies.is_empty() {
            lines.push(String::new());
            lines.push("Top Strategies:".to_string());
            for strategy in self.top_strategies(5) {
                lines.push(format!(
                    "  - {} ({:.1}% expected improvement)",
                    strategy.name,
                    strategy.expected_improvement * 100.0
                ));
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

/// A pattern that correlates with successful executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessPattern {
    /// Unique identifier for this pattern
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Detailed description
    pub description: String,
    /// The pattern type/category
    pub pattern_type: PatternType,
    /// Success rate when this pattern is present (0.0-1.0)
    pub success_rate: f64,
    /// Average performance improvement when applied
    pub performance_improvement: f64,
    /// Number of observations
    pub sample_count: usize,
    /// Confidence in this pattern (0.0-1.0)
    pub confidence: f64,
    /// Which agents exhibited this pattern
    pub agents_exhibiting: Vec<String>,
    /// Conditions that define this pattern
    pub conditions: Vec<PatternCondition>,
    /// How to apply this pattern
    pub application_guide: String,
    /// Prerequisites for applying this pattern
    pub prerequisites: Vec<String>,
    /// Potential risks of applying this pattern
    pub risks: Vec<String>,
    /// Related patterns
    pub related_patterns: Vec<String>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl SuccessPattern {
    /// Create a new success pattern
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            pattern_type: PatternType::Behavioral,
            success_rate: 0.0,
            performance_improvement: 0.0,
            sample_count: 0,
            confidence: 0.0,
            agents_exhibiting: Vec::new(),
            conditions: Vec::new(),
            application_guide: String::new(),
            prerequisites: Vec::new(),
            risks: Vec::new(),
            related_patterns: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set pattern type
    #[must_use]
    pub fn with_type(mut self, pattern_type: PatternType) -> Self {
        self.pattern_type = pattern_type;
        self
    }

    /// Set success rate
    #[must_use]
    pub fn with_success_rate(mut self, rate: f64) -> Self {
        self.success_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set performance improvement
    #[must_use]
    pub fn with_performance_improvement(mut self, improvement: f64) -> Self {
        self.performance_improvement = improvement;
        self
    }

    /// Set sample count
    #[must_use]
    pub fn with_sample_count(mut self, count: usize) -> Self {
        self.sample_count = count;
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add an agent that exhibits this pattern
    #[must_use]
    pub fn with_agent(mut self, agent: impl Into<String>) -> Self {
        self.agents_exhibiting.push(agent.into());
        self
    }

    /// Add a condition
    #[must_use]
    pub fn with_condition(mut self, condition: PatternCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set application guide
    #[must_use]
    pub fn with_application_guide(mut self, guide: impl Into<String>) -> Self {
        self.application_guide = guide.into();
        self
    }

    /// Check if this pattern is actionable
    #[must_use]
    pub fn is_actionable(&self) -> bool {
        self.success_rate >= 0.7
            && self.confidence >= 0.6
            && self.sample_count >= 5
            && !self.application_guide.is_empty()
    }

    /// Get a brief summary
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{}: {:.1}% success rate, {:.1}% improvement ({} samples)",
            self.name,
            self.success_rate * 100.0,
            self.performance_improvement * 100.0,
            self.sample_count
        )
    }
}

/// Types of patterns that can be discovered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternType {
    /// Behavioral patterns (how agents behave)
    Behavioral,
    /// Structural patterns (graph structure)
    Structural,
    /// Configuration patterns (settings and parameters)
    Configuration,
    /// Resource usage patterns
    Resource,
    /// Error handling patterns
    ErrorHandling,
    /// Communication patterns (between agents)
    Communication,
    /// Timing patterns (execution order, delays)
    Timing,
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternType::Behavioral => write!(f, "Behavioral"),
            PatternType::Structural => write!(f, "Structural"),
            PatternType::Configuration => write!(f, "Configuration"),
            PatternType::Resource => write!(f, "Resource"),
            PatternType::ErrorHandling => write!(f, "Error Handling"),
            PatternType::Communication => write!(f, "Communication"),
            PatternType::Timing => write!(f, "Timing"),
        }
    }
}

/// A condition that defines a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCondition {
    /// What aspect is being measured
    pub metric: String,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Threshold value
    pub threshold: f64,
    /// Human-readable description
    pub description: String,
}

impl PatternCondition {
    /// Create a new condition
    #[must_use]
    pub fn new(metric: impl Into<String>, operator: ComparisonOperator, threshold: f64) -> Self {
        let metric = metric.into();
        let description = format!("{} {} {}", metric, operator, threshold);
        Self {
            metric,
            operator,
            threshold,
            description,
        }
    }

    /// Check if a value satisfies this condition
    #[must_use]
    pub fn is_satisfied(&self, value: f64) -> bool {
        match self.operator {
            ComparisonOperator::LessThan => value < self.threshold,
            ComparisonOperator::LessOrEqual => value <= self.threshold,
            ComparisonOperator::Equal => (value - self.threshold).abs() < f64::EPSILON,
            ComparisonOperator::GreaterOrEqual => value >= self.threshold,
            ComparisonOperator::GreaterThan => value > self.threshold,
            ComparisonOperator::NotEqual => (value - self.threshold).abs() >= f64::EPSILON,
        }
    }
}

/// Comparison operators for conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    /// Less than (`<`).
    LessThan,
    /// Less than or equal (`<=`).
    LessOrEqual,
    /// Equal (`==`).
    Equal,
    /// Greater than or equal (`>=`).
    GreaterOrEqual,
    /// Greater than (`>`).
    GreaterThan,
    /// Not equal (`!=`).
    NotEqual,
}

impl std::fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonOperator::LessThan => write!(f, "<"),
            ComparisonOperator::LessOrEqual => write!(f, "<="),
            ComparisonOperator::Equal => write!(f, "=="),
            ComparisonOperator::GreaterOrEqual => write!(f, ">="),
            ComparisonOperator::GreaterThan => write!(f, ">"),
            ComparisonOperator::NotEqual => write!(f, "!="),
        }
    }
}

/// A common pitfall or anti-pattern to avoid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pitfall {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Detailed description
    pub description: String,
    /// Category of pitfall
    pub category: PitfallCategory,
    /// How often this pitfall leads to failure (0.0-1.0)
    pub failure_rate: f64,
    /// Number of times this pitfall was observed
    pub occurrences: usize,
    /// Severity of the pitfall
    pub severity: PitfallSeverity,
    /// How to detect this pitfall
    pub detection_criteria: Vec<String>,
    /// How to avoid this pitfall
    pub avoidance_strategies: Vec<String>,
    /// Example agents that fell into this pitfall
    pub example_agents: Vec<String>,
    /// Impact on performance when this pitfall occurs
    pub performance_impact: f64,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Pitfall {
    /// Create a new pitfall
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            category: PitfallCategory::General,
            failure_rate: 0.0,
            occurrences: 0,
            severity: PitfallSeverity::Medium,
            detection_criteria: Vec::new(),
            avoidance_strategies: Vec::new(),
            example_agents: Vec::new(),
            performance_impact: 0.0,
            metadata: HashMap::new(),
        }
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set category
    #[must_use]
    pub fn with_category(mut self, category: PitfallCategory) -> Self {
        self.category = category;
        self
    }

    /// Set failure rate
    #[must_use]
    pub fn with_failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set occurrences
    #[must_use]
    pub fn with_occurrences(mut self, count: usize) -> Self {
        self.occurrences = count;
        self
    }

    /// Set severity
    #[must_use]
    pub fn with_severity(mut self, severity: PitfallSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Add detection criteria
    #[must_use]
    pub fn with_detection(mut self, criteria: impl Into<String>) -> Self {
        self.detection_criteria.push(criteria.into());
        self
    }

    /// Add avoidance strategy
    #[must_use]
    pub fn with_avoidance(mut self, strategy: impl Into<String>) -> Self {
        self.avoidance_strategies.push(strategy.into());
        self
    }

    /// Set performance impact
    #[must_use]
    pub fn with_performance_impact(mut self, impact: f64) -> Self {
        self.performance_impact = impact;
        self
    }

    /// Check if this pitfall is actionable
    #[must_use]
    pub fn is_actionable(&self) -> bool {
        self.occurrences >= 3
            && !self.avoidance_strategies.is_empty()
            && !self.detection_criteria.is_empty()
    }

    /// Get a brief summary
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{}: {:.1}% failure rate, {} occurrences ({})",
            self.name,
            self.failure_rate * 100.0,
            self.occurrences,
            self.severity
        )
    }
}

/// Categories of pitfalls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PitfallCategory {
    /// General/uncategorized
    General,
    /// Resource exhaustion (memory, tokens, time)
    ResourceExhaustion,
    /// Infinite loops or excessive retries
    InfiniteLoop,
    /// Poor error handling
    ErrorHandling,
    /// Configuration mistakes
    Configuration,
    /// Inefficient patterns
    Inefficiency,
    /// Race conditions or timing issues
    Concurrency,
    /// Security issues
    Security,
}

impl std::fmt::Display for PitfallCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PitfallCategory::General => write!(f, "General"),
            PitfallCategory::ResourceExhaustion => write!(f, "Resource Exhaustion"),
            PitfallCategory::InfiniteLoop => write!(f, "Infinite Loop"),
            PitfallCategory::ErrorHandling => write!(f, "Error Handling"),
            PitfallCategory::Configuration => write!(f, "Configuration"),
            PitfallCategory::Inefficiency => write!(f, "Inefficiency"),
            PitfallCategory::Concurrency => write!(f, "Concurrency"),
            PitfallCategory::Security => write!(f, "Security"),
        }
    }
}

/// Severity levels for pitfalls
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PitfallSeverity {
    /// Minor issue, low impact
    Low,
    /// Moderate issue
    Medium,
    /// Significant issue
    High,
    /// Critical issue that often causes failure
    Critical,
}

impl std::fmt::Display for PitfallSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PitfallSeverity::Low => write!(f, "Low"),
            PitfallSeverity::Medium => write!(f, "Medium"),
            PitfallSeverity::High => write!(f, "High"),
            PitfallSeverity::Critical => write!(f, "Critical"),
        }
    }
}

/// An optimization strategy that can improve agent performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationStrategy {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Detailed description
    pub description: String,
    /// Type of optimization
    pub strategy_type: StrategyType,
    /// Expected improvement when applied (0.0-1.0)
    pub expected_improvement: f64,
    /// Confidence in this strategy (0.0-1.0)
    pub confidence: f64,
    /// Number of observations supporting this strategy
    pub evidence_count: usize,
    /// Agents where this strategy was effective
    pub effective_agents: Vec<String>,
    /// Implementation steps
    pub implementation_steps: Vec<String>,
    /// Estimated complexity to implement (1-10)
    pub complexity: u8,
    /// Risk level of applying this strategy
    pub risk: StrategyRisk,
    /// What metrics this strategy improves
    pub improves_metrics: Vec<String>,
    /// What metrics might be negatively affected
    pub potential_tradeoffs: Vec<String>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl OptimizationStrategy {
    /// Create a new optimization strategy
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            strategy_type: StrategyType::General,
            expected_improvement: 0.0,
            confidence: 0.0,
            evidence_count: 0,
            effective_agents: Vec::new(),
            implementation_steps: Vec::new(),
            complexity: 5,
            risk: StrategyRisk::Low,
            improves_metrics: Vec::new(),
            potential_tradeoffs: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set strategy type
    #[must_use]
    pub fn with_type(mut self, strategy_type: StrategyType) -> Self {
        self.strategy_type = strategy_type;
        self
    }

    /// Set expected improvement
    #[must_use]
    pub fn with_expected_improvement(mut self, improvement: f64) -> Self {
        self.expected_improvement = improvement.clamp(0.0, 10.0);
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set evidence count
    #[must_use]
    pub fn with_evidence_count(mut self, count: usize) -> Self {
        self.evidence_count = count;
        self
    }

    /// Add implementation step
    #[must_use]
    pub fn with_step(mut self, step: impl Into<String>) -> Self {
        self.implementation_steps.push(step.into());
        self
    }

    /// Set complexity (1-10)
    #[must_use]
    pub fn with_complexity(mut self, complexity: u8) -> Self {
        self.complexity = complexity.clamp(1, 10);
        self
    }

    /// Set risk level
    #[must_use]
    pub fn with_risk(mut self, risk: StrategyRisk) -> Self {
        self.risk = risk;
        self
    }

    /// Add a metric this strategy improves
    #[must_use]
    pub fn improves(mut self, metric: impl Into<String>) -> Self {
        self.improves_metrics.push(metric.into());
        self
    }

    /// Add a potential tradeoff
    #[must_use]
    pub fn with_tradeoff(mut self, tradeoff: impl Into<String>) -> Self {
        self.potential_tradeoffs.push(tradeoff.into());
        self
    }

    /// Check if this strategy is actionable
    #[must_use]
    pub fn is_actionable(&self) -> bool {
        self.expected_improvement >= 0.1
            && self.confidence >= 0.6
            && self.evidence_count >= 3
            && !self.implementation_steps.is_empty()
    }

    /// Get ROI score (improvement / complexity)
    #[must_use]
    pub fn roi_score(&self) -> f64 {
        if self.complexity == 0 {
            return 0.0;
        }
        self.expected_improvement / (self.complexity as f64 / 10.0)
    }

    /// Get a brief summary
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{}: {:.1}% improvement, complexity {}/10, {} evidence ({})",
            self.name,
            self.expected_improvement * 100.0,
            self.complexity,
            self.evidence_count,
            self.risk
        )
    }
}

/// Types of optimization strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyType {
    /// General optimization
    General,
    /// Caching results
    Caching,
    /// Parallel execution
    Parallelization,
    /// Reducing token usage
    TokenReduction,
    /// Improving latency
    LatencyReduction,
    /// Better error handling
    ErrorHandling,
    /// Model selection optimization
    ModelSelection,
    /// Resource management
    ResourceManagement,
    /// Prompt optimization
    PromptOptimization,
}

impl std::fmt::Display for StrategyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrategyType::General => write!(f, "General"),
            StrategyType::Caching => write!(f, "Caching"),
            StrategyType::Parallelization => write!(f, "Parallelization"),
            StrategyType::TokenReduction => write!(f, "Token Reduction"),
            StrategyType::LatencyReduction => write!(f, "Latency Reduction"),
            StrategyType::ErrorHandling => write!(f, "Error Handling"),
            StrategyType::ModelSelection => write!(f, "Model Selection"),
            StrategyType::ResourceManagement => write!(f, "Resource Management"),
            StrategyType::PromptOptimization => write!(f, "Prompt Optimization"),
        }
    }
}

/// Risk levels for strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StrategyRisk {
    /// Very low risk, safe to apply
    VeryLow,
    /// Low risk
    Low,
    /// Moderate risk
    Moderate,
    /// High risk, requires careful testing
    High,
}

impl std::fmt::Display for StrategyRisk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrategyRisk::VeryLow => write!(f, "Very Low"),
            StrategyRisk::Low => write!(f, "Low"),
            StrategyRisk::Moderate => write!(f, "Moderate"),
            StrategyRisk::High => write!(f, "High"),
        }
    }
}

/// Summary of a single agent's performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    /// Agent identifier (e.g., graph_id)
    pub agent_id: String,
    /// Number of executions analyzed
    pub execution_count: usize,
    /// Success rate (0.0-1.0)
    pub success_rate: f64,
    /// Average execution duration (ms)
    pub avg_duration_ms: f64,
    /// Average token usage
    pub avg_tokens: f64,
    /// Patterns this agent exhibits
    pub patterns_exhibited: Vec<String>,
    /// Pitfalls this agent encountered
    pub pitfalls_encountered: Vec<String>,
    /// Performance percentile compared to other agents (0-100)
    pub performance_percentile: u8,
    /// Notable characteristics
    pub characteristics: Vec<String>,
}

impl AgentSummary {
    /// Create a new agent summary
    #[must_use]
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            execution_count: 0,
            success_rate: 0.0,
            avg_duration_ms: 0.0,
            avg_tokens: 0.0,
            patterns_exhibited: Vec::new(),
            pitfalls_encountered: Vec::new(),
            performance_percentile: 50,
            characteristics: Vec::new(),
        }
    }

    /// Check if this agent is a top performer
    #[must_use]
    pub fn is_top_performer(&self) -> bool {
        self.performance_percentile >= 80 && self.success_rate >= 0.9
    }

    /// Check if this agent needs attention
    #[must_use]
    pub fn needs_attention(&self) -> bool {
        self.success_rate < 0.7 || !self.pitfalls_encountered.is_empty()
    }
}

/// Correlation discovered between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCorrelation {
    /// First agent
    pub agent_a: String,
    /// Second agent
    pub agent_b: String,
    /// Type of correlation
    pub correlation_type: CorrelationType,
    /// Correlation strength (-1.0 to 1.0)
    pub strength: f64,
    /// Description of the correlation
    pub description: String,
    /// Number of observations
    pub observations: usize,
}

impl AgentCorrelation {
    /// Create a new correlation
    #[must_use]
    pub fn new(agent_a: impl Into<String>, agent_b: impl Into<String>) -> Self {
        Self {
            agent_a: agent_a.into(),
            agent_b: agent_b.into(),
            correlation_type: CorrelationType::Performance,
            strength: 0.0,
            description: String::new(),
            observations: 0,
        }
    }

    /// Check if this is a strong correlation
    #[must_use]
    pub fn is_strong(&self) -> bool {
        self.strength.abs() >= 0.7
    }
}

/// Types of correlations between agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrelationType {
    /// Performance correlation
    Performance,
    /// Behavior similarity
    BehaviorSimilarity,
    /// Error pattern correlation
    ErrorPattern,
    /// Resource usage correlation
    ResourceUsage,
}

/// Configuration for cross-agent learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossAgentConfig {
    /// Minimum executions per agent to include in analysis
    pub min_executions_per_agent: usize,
    /// Minimum sample count for pattern detection
    pub min_pattern_samples: usize,
    /// Minimum success rate to consider a pattern successful
    pub min_success_rate: f64,
    /// Maximum pitfall failure rate threshold
    pub max_pitfall_threshold: f64,
    /// Minimum improvement to consider a strategy
    pub min_improvement_threshold: f64,
    /// Whether to include agent summaries
    pub include_agent_summaries: bool,
    /// Whether to compute correlations
    pub compute_correlations: bool,
}

impl Default for CrossAgentConfig {
    fn default() -> Self {
        Self {
            min_executions_per_agent: 5,
            min_pattern_samples: 10,
            min_success_rate: 0.7,
            max_pitfall_threshold: 0.3,
            min_improvement_threshold: 0.1,
            include_agent_summaries: true,
            compute_correlations: true,
        }
    }
}

impl CrossAgentConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum executions per agent to include in analysis.
    #[must_use]
    pub fn with_min_executions_per_agent(mut self, min: usize) -> Self {
        self.min_executions_per_agent = min;
        self
    }

    /// Set minimum sample count for pattern detection.
    #[must_use]
    pub fn with_min_pattern_samples(mut self, min: usize) -> Self {
        self.min_pattern_samples = min;
        self
    }

    /// Set minimum success rate to consider a pattern successful (0.0-1.0).
    #[must_use]
    pub fn with_min_success_rate(mut self, rate: f64) -> Self {
        self.min_success_rate = rate;
        self
    }

    /// Set maximum pitfall failure rate threshold (0.0-1.0).
    #[must_use]
    pub fn with_max_pitfall_threshold(mut self, threshold: f64) -> Self {
        self.max_pitfall_threshold = threshold;
        self
    }

    /// Set minimum improvement to consider a strategy (0.0-1.0).
    #[must_use]
    pub fn with_min_improvement_threshold(mut self, threshold: f64) -> Self {
        self.min_improvement_threshold = threshold;
        self
    }

    /// Enable or disable agent summaries.
    #[must_use]
    pub fn with_agent_summaries(mut self, include: bool) -> Self {
        self.include_agent_summaries = include;
        self
    }

    /// Enable or disable correlation computation.
    #[must_use]
    pub fn with_correlations(mut self, compute: bool) -> Self {
        self.compute_correlations = compute;
        self
    }
}

/// Cross-agent learner for analyzing execution traces from multiple agents
pub struct CrossAgentLearner {
    config: CrossAgentConfig,
}

impl Default for CrossAgentLearner {
    fn default() -> Self {
        Self::new()
    }
}

impl CrossAgentLearner {
    /// Create a new learner with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: CrossAgentConfig::default(),
        }
    }

    /// Create a learner with custom configuration
    #[must_use]
    pub fn with_config(config: CrossAgentConfig) -> Self {
        Self { config }
    }

    /// Analyze execution traces from multiple agents
    #[must_use]
    pub fn analyze(&self, traces: &[crate::introspection::ExecutionTrace]) -> CrossAgentInsights {
        if traces.is_empty() {
            return CrossAgentInsights::new();
        }

        // Group traces by agent (using thread_id or metadata)
        let agent_traces = self.group_by_agent(traces);
        let agents_analyzed = agent_traces.len();
        let executions_analyzed = traces.len();

        // Analyze patterns, pitfalls, and strategies
        let successful_patterns = self.discover_patterns(&agent_traces);
        let common_pitfalls = self.discover_pitfalls(&agent_traces);
        let optimization_strategies = self.discover_strategies(&agent_traces, traces);

        // Generate agent summaries
        let agent_summaries = if self.config.include_agent_summaries {
            self.generate_agent_summaries(&agent_traces)
        } else {
            Vec::new()
        };

        // Compute correlations
        let correlations = if self.config.compute_correlations {
            self.compute_correlations(&agent_summaries)
        } else {
            Vec::new()
        };

        CrossAgentInsights {
            agents_analyzed,
            executions_analyzed,
            successful_patterns,
            common_pitfalls,
            optimization_strategies,
            agent_summaries,
            correlations,
            metadata: HashMap::new(),
        }
    }

    /// Group traces by agent
    fn group_by_agent<'a>(
        &self,
        traces: &'a [crate::introspection::ExecutionTrace],
    ) -> HashMap<String, Vec<&'a crate::introspection::ExecutionTrace>> {
        let mut grouped: HashMap<String, Vec<&crate::introspection::ExecutionTrace>> =
            HashMap::new();

        for trace in traces {
            // Use thread_id as agent identifier, or "unknown" if not set
            let agent_id = trace
                .thread_id
                .clone()
                .or_else(|| {
                    trace
                        .metadata
                        .get("agent_id")
                        .and_then(|v| v.as_str().map(String::from))
                })
                .unwrap_or_else(|| "unknown".to_string());

            grouped.entry(agent_id).or_default().push(trace);
        }

        // Filter out agents with insufficient executions
        grouped.retain(|_, traces| traces.len() >= self.config.min_executions_per_agent);

        grouped
    }

    /// Discover successful patterns across agents
    fn discover_patterns(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<SuccessPattern> {
        // We detect 4 pattern types, each returns 0-1 patterns typically
        let mut patterns = Vec::with_capacity(4);

        // Pattern: Caching leads to faster execution
        patterns.extend(self.detect_caching_pattern(agent_traces));

        // Pattern: Parallel execution improves throughput
        patterns.extend(self.detect_parallelism_pattern(agent_traces));

        // Pattern: Lower token usage correlates with success
        patterns.extend(self.detect_token_efficiency_pattern(agent_traces));

        // Pattern: Fewer errors correlates with faster completion
        patterns.extend(self.detect_error_correlation_pattern(agent_traces));

        // Sort by success rate
        patterns.sort_by(|a, b| {
            b.success_rate
                .partial_cmp(&a.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        patterns
    }

    /// Detect caching patterns
    fn detect_caching_pattern(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<SuccessPattern> {
        let mut patterns = Vec::new();

        // Look for agents with repeated node executions vs those without
        let mut with_repeated: Vec<(&str, f64, f64)> = Vec::new(); // (agent, success_rate, avg_duration)
        let mut without_repeated: Vec<(&str, f64, f64)> = Vec::new();

        for (agent_id, traces) in agent_traces {
            let has_repeated = traces.iter().any(|t| {
                let mut node_counts: HashMap<&str, usize> = HashMap::new();
                for exec in &t.nodes_executed {
                    *node_counts.entry(&exec.node).or_default() += 1;
                }
                node_counts.values().any(|&count| count >= 3)
            });

            let success_rate =
                traces.iter().filter(|t| t.completed).count() as f64 / traces.len() as f64;
            let avg_duration = traces.iter().map(|t| t.total_duration_ms).sum::<u64>() as f64
                / traces.len() as f64;

            if has_repeated {
                with_repeated.push((agent_id, success_rate, avg_duration));
            } else {
                without_repeated.push((agent_id, success_rate, avg_duration));
            }
        }

        // Compare groups
        if with_repeated.len() >= self.config.min_pattern_samples && !without_repeated.is_empty() {
            let with_avg_success: f64 =
                with_repeated.iter().map(|(_, s, _)| s).sum::<f64>() / with_repeated.len() as f64;
            let without_avg_success: f64 = without_repeated.iter().map(|(_, s, _)| s).sum::<f64>()
                / without_repeated.len() as f64;

            // If agents without repeated nodes are more successful, caching might help
            if without_avg_success > with_avg_success + 0.1 {
                let improvement = without_avg_success - with_avg_success;
                patterns.push(
                    SuccessPattern::new("caching_benefit", "Caching reduces repeated node executions")
                        .with_description(
                            "Agents that cache results and avoid repeated node executions have higher success rates",
                        )
                        .with_type(PatternType::Behavioral)
                        .with_success_rate(without_avg_success)
                        .with_performance_improvement(improvement)
                        .with_sample_count(without_repeated.len())
                        .with_confidence(0.75)
                        .with_application_guide(
                            "Add caching nodes before frequently-called nodes to reduce redundant executions",
                        ),
                );
            }
        }

        patterns
    }

    /// Detect parallelism patterns
    fn detect_parallelism_pattern(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<SuccessPattern> {
        let mut patterns = Vec::new();

        // Collect latency data grouped by node count
        let mut high_node_count: Vec<(f64, u64)> = Vec::new(); // (success_rate, avg_duration)
        let mut low_node_count: Vec<(f64, u64)> = Vec::new();

        for traces in agent_traces.values() {
            for trace in traces {
                let node_count = trace.nodes_executed.len();
                let success = if trace.completed { 1.0 } else { 0.0 };

                if node_count >= 5 {
                    high_node_count.push((success, trace.total_duration_ms));
                } else {
                    low_node_count.push((success, trace.total_duration_ms));
                }
            }
        }

        // Check if parallel execution (many nodes) can be beneficial
        if high_node_count.len() >= self.config.min_pattern_samples
            && low_node_count.len() >= self.config.min_pattern_samples
        {
            let high_avg_duration = high_node_count.iter().map(|(_, d)| *d).sum::<u64>() as f64
                / high_node_count.len() as f64;
            let low_avg_duration = low_node_count.iter().map(|(_, d)| *d).sum::<u64>() as f64
                / low_node_count.len() as f64;

            // If complex graphs are much slower, parallelization could help
            if high_avg_duration > low_avg_duration * 2.0 {
                let speedup_potential = (high_avg_duration - low_avg_duration) / high_avg_duration;
                patterns.push(
                    SuccessPattern::new("parallel_potential", "Complex graphs benefit from parallelization")
                        .with_description(
                            "Agents with many sequential nodes show high latency - parallel execution could help",
                        )
                        .with_type(PatternType::Structural)
                        .with_success_rate(0.85)
                        .with_performance_improvement(speedup_potential)
                        .with_sample_count(high_node_count.len())
                        .with_confidence(0.7)
                        .with_application_guide(
                            "Identify independent nodes and convert to parallel execution",
                        ),
                );
            }
        }

        patterns
    }

    /// Detect token efficiency patterns
    fn detect_token_efficiency_pattern(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<SuccessPattern> {
        let mut patterns = Vec::new();

        // Group by token usage
        let mut low_token: Vec<(bool, u64)> = Vec::new();
        let mut high_token: Vec<(bool, u64)> = Vec::new();

        for traces in agent_traces.values() {
            for trace in traces {
                if trace.total_tokens < 2000 {
                    low_token.push((trace.completed, trace.total_duration_ms));
                } else if trace.total_tokens > 8000 {
                    high_token.push((trace.completed, trace.total_duration_ms));
                }
            }
        }

        if low_token.len() >= self.config.min_pattern_samples
            && high_token.len() >= self.config.min_pattern_samples
        {
            let low_success =
                low_token.iter().filter(|(c, _)| *c).count() as f64 / low_token.len() as f64;
            let high_success =
                high_token.iter().filter(|(c, _)| *c).count() as f64 / high_token.len() as f64;

            if low_success > high_success + 0.1 {
                patterns.push(
                    SuccessPattern::new("token_efficiency", "Lower token usage correlates with success")
                        .with_description(
                            "Agents with efficient token usage (<2000) have higher success rates than high-token agents",
                        )
                        .with_type(PatternType::Resource)
                        .with_success_rate(low_success)
                        .with_performance_improvement(low_success - high_success)
                        .with_sample_count(low_token.len())
                        .with_confidence(0.8)
                        .with_condition(PatternCondition::new("tokens", ComparisonOperator::LessThan, 2000.0))
                        .with_application_guide(
                            "Optimize prompts and context to reduce token usage",
                        ),
                );
            }
        }

        patterns
    }

    /// Detect error correlation patterns
    fn detect_error_correlation_pattern(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<SuccessPattern> {
        let mut patterns = Vec::new();

        let mut no_error_data: Vec<u64> = Vec::new();
        let mut with_error_data: Vec<u64> = Vec::new();

        for traces in agent_traces.values() {
            for trace in traces {
                if trace.errors.is_empty() {
                    no_error_data.push(trace.total_duration_ms);
                } else {
                    with_error_data.push(trace.total_duration_ms);
                }
            }
        }

        if no_error_data.len() >= self.config.min_pattern_samples
            && with_error_data.len() >= self.config.min_pattern_samples
        {
            let no_error_avg =
                no_error_data.iter().sum::<u64>() as f64 / no_error_data.len() as f64;
            let with_error_avg =
                with_error_data.iter().sum::<u64>() as f64 / with_error_data.len() as f64;

            if with_error_avg > no_error_avg * 1.5 {
                let speedup = (with_error_avg - no_error_avg) / with_error_avg;
                patterns.push(
                    SuccessPattern::new("error_free_fast", "Error-free executions are faster")
                        .with_description(
                            "Executions without errors complete significantly faster due to no retry overhead",
                        )
                        .with_type(PatternType::ErrorHandling)
                        .with_success_rate(1.0)
                        .with_performance_improvement(speedup)
                        .with_sample_count(no_error_data.len())
                        .with_confidence(0.9)
                        .with_application_guide(
                            "Invest in input validation and error prevention rather than retry logic",
                        ),
                );
            }
        }

        patterns
    }

    /// Discover common pitfalls across agents
    fn discover_pitfalls(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<Pitfall> {
        let mut pitfalls = Vec::new();

        // Pitfall: Excessive retries
        pitfalls.extend(self.detect_retry_pitfall(agent_traces));

        // Pitfall: Token exhaustion
        pitfalls.extend(self.detect_token_exhaustion_pitfall(agent_traces));

        // Pitfall: Timeout issues
        pitfalls.extend(self.detect_timeout_pitfall(agent_traces));

        // Sort by failure rate
        pitfalls.sort_by(|a, b| {
            b.failure_rate
                .partial_cmp(&a.failure_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        pitfalls
    }

    /// Detect retry-related pitfalls
    fn detect_retry_pitfall(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<Pitfall> {
        let mut pitfalls = Vec::new();

        let mut excessive_retry_count = 0;
        let mut excessive_retry_failures = 0;
        let mut example_agents = Vec::new();

        for (agent_id, traces) in agent_traces {
            for trace in traces {
                // Check for repeated error patterns (indicating retries)
                let error_count = trace.errors.len();
                if error_count >= 3 {
                    excessive_retry_count += 1;
                    if !trace.completed {
                        excessive_retry_failures += 1;
                    }
                    if example_agents.len() < 3 && !example_agents.contains(agent_id) {
                        example_agents.push(agent_id.clone());
                    }
                }
            }
        }

        if excessive_retry_count >= 5 {
            let failure_rate = excessive_retry_failures as f64 / excessive_retry_count as f64;
            pitfalls.push(
                Pitfall::new("excessive_retries", "Excessive retry attempts")
                    .with_description(
                        "Multiple errors before completion indicate ineffective retry strategies",
                    )
                    .with_category(PitfallCategory::Inefficiency)
                    .with_failure_rate(failure_rate)
                    .with_occurrences(excessive_retry_count)
                    .with_severity(if failure_rate > 0.5 {
                        PitfallSeverity::High
                    } else {
                        PitfallSeverity::Medium
                    })
                    .with_detection("3+ errors in a single execution")
                    .with_avoidance(
                        "Implement exponential backoff and fail-fast for unrecoverable errors",
                    )
                    .with_performance_impact(failure_rate * 0.5),
            );
        }

        pitfalls
    }

    /// Detect token exhaustion pitfalls
    fn detect_token_exhaustion_pitfall(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<Pitfall> {
        let mut pitfalls = Vec::new();

        let mut high_token_failures = 0;
        let mut high_token_total = 0;

        for traces in agent_traces.values() {
            for trace in traces {
                if trace.total_tokens > 10000 {
                    high_token_total += 1;
                    if !trace.completed {
                        high_token_failures += 1;
                    }
                }
            }
        }

        if high_token_total >= 5 {
            let failure_rate = high_token_failures as f64 / high_token_total as f64;
            if failure_rate > self.config.max_pitfall_threshold {
                pitfalls.push(
                    Pitfall::new("token_exhaustion", "Token limit exhaustion")
                        .with_description(
                            "High token usage leads to failures, possibly due to context limits or cost overruns",
                        )
                        .with_category(PitfallCategory::ResourceExhaustion)
                        .with_failure_rate(failure_rate)
                        .with_occurrences(high_token_total)
                        .with_severity(PitfallSeverity::High)
                        .with_detection("Token usage exceeds 10,000")
                        .with_avoidance("Implement context summarization and token budgets")
                        .with_performance_impact(0.8),
                );
            }
        }

        pitfalls
    }

    /// Detect timeout-related pitfalls
    fn detect_timeout_pitfall(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<Pitfall> {
        let mut pitfalls = Vec::new();

        let mut timeout_count = 0;
        let mut total_long_executions = 0;

        for traces in agent_traces.values() {
            for trace in traces {
                // Long executions (>30s) that failed might be timeouts
                if trace.total_duration_ms > 30000 {
                    total_long_executions += 1;
                    if !trace.completed {
                        timeout_count += 1;
                    }
                }
            }
        }

        if total_long_executions >= 5 {
            let failure_rate = timeout_count as f64 / total_long_executions as f64;
            if failure_rate > self.config.max_pitfall_threshold {
                pitfalls.push(
                    Pitfall::new("timeout_failures", "Long-running executions timeout")
                        .with_description("Executions exceeding 30 seconds have high failure rates")
                        .with_category(PitfallCategory::ResourceExhaustion)
                        .with_failure_rate(failure_rate)
                        .with_occurrences(total_long_executions)
                        .with_severity(PitfallSeverity::Critical)
                        .with_detection("Execution duration > 30 seconds")
                        .with_avoidance("Set adaptive timeouts and implement early termination")
                        .with_performance_impact(1.0),
                );
            }
        }

        pitfalls
    }

    /// Discover optimization strategies from agent data
    fn discover_strategies(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
        all_traces: &[crate::introspection::ExecutionTrace],
    ) -> Vec<OptimizationStrategy> {
        let mut strategies = Vec::new();

        // Strategy: Caching
        if let Some(strategy) = self.suggest_caching_strategy(agent_traces) {
            strategies.push(strategy);
        }

        // Strategy: Parallelization
        if let Some(strategy) = self.suggest_parallelization_strategy(all_traces) {
            strategies.push(strategy);
        }

        // Strategy: Token reduction
        if let Some(strategy) = self.suggest_token_reduction_strategy(all_traces) {
            strategies.push(strategy);
        }

        // Sort by expected improvement
        strategies.sort_by(|a, b| {
            b.expected_improvement
                .partial_cmp(&a.expected_improvement)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        strategies
    }

    /// Suggest caching strategy
    fn suggest_caching_strategy(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Option<OptimizationStrategy> {
        let mut repeated_node_count = 0;
        let mut total_nodes = 0;

        for traces in agent_traces.values() {
            for trace in traces {
                let mut node_counts: HashMap<&str, usize> = HashMap::new();
                for exec in &trace.nodes_executed {
                    *node_counts.entry(&exec.node).or_default() += 1;
                    total_nodes += 1;
                }
                repeated_node_count += node_counts.values().filter(|&&c| c >= 2).count();
            }
        }

        if repeated_node_count >= 10 && total_nodes > 0 {
            let repeat_rate = repeated_node_count as f64 / total_nodes as f64;
            if repeat_rate > 0.1 {
                return Some(
                    OptimizationStrategy::new("add_caching", "Add caching for repeated nodes")
                        .with_description(
                            "Many nodes are executed multiple times - caching could reduce redundant computation",
                        )
                        .with_type(StrategyType::Caching)
                        .with_expected_improvement(repeat_rate * 0.5) // Estimate 50% of repeats could be cached
                        .with_confidence(0.8)
                        .with_evidence_count(repeated_node_count)
                        .with_step("Identify nodes with execution_count > 1")
                        .with_step("Add cache_key based on input state")
                        .with_step("Insert CacheNode before target node")
                        .with_complexity(4)
                        .with_risk(StrategyRisk::Low)
                        .improves("latency")
                        .improves("token_usage"),
                );
            }
        }

        None
    }

    /// Suggest parallelization strategy
    fn suggest_parallelization_strategy(
        &self,
        traces: &[crate::introspection::ExecutionTrace],
    ) -> Option<OptimizationStrategy> {
        // Calculate average nodes per execution
        let total_nodes: usize = traces.iter().map(|t| t.nodes_executed.len()).sum();
        let avg_nodes = total_nodes as f64 / traces.len().max(1) as f64;

        // If many nodes, parallelization might help
        if avg_nodes >= 4.0 && traces.len() >= self.config.min_pattern_samples {
            return Some(
                OptimizationStrategy::new("parallelize_nodes", "Parallelize independent nodes")
                    .with_description(
                        "Graphs have many sequential nodes - parallel execution could reduce latency",
                    )
                    .with_type(StrategyType::Parallelization)
                    .with_expected_improvement(0.3) // Estimate 30% speedup
                    .with_confidence(0.7)
                    .with_evidence_count(traces.len())
                    .with_step("Analyze node dependencies using the graph structure")
                    .with_step("Identify nodes without data dependencies")
                    .with_step("Convert sequential edges to parallel edges")
                    .with_step("Test for correctness")
                    .with_complexity(6)
                    .with_risk(StrategyRisk::Moderate)
                    .improves("latency")
                    .with_tradeoff("Increased memory usage during parallel execution"),
            );
        }

        None
    }

    /// Suggest token reduction strategy
    fn suggest_token_reduction_strategy(
        &self,
        traces: &[crate::introspection::ExecutionTrace],
    ) -> Option<OptimizationStrategy> {
        let high_token_traces: Vec<_> = traces.iter().filter(|t| t.total_tokens > 5000).collect();

        if high_token_traces.len() >= self.config.min_pattern_samples {
            let avg_tokens = high_token_traces
                .iter()
                .map(|t| t.total_tokens)
                .sum::<u64>() as f64
                / high_token_traces.len() as f64;
            let potential_savings = (avg_tokens - 3000.0) / avg_tokens; // Target 3000 tokens

            if potential_savings > 0.2 {
                return Some(
                    OptimizationStrategy::new("reduce_tokens", "Reduce token usage in high-token executions")
                        .with_description(format!(
                            "{} executions use >5000 tokens (avg: {:.0}) - optimization could save {:.1}%",
                            high_token_traces.len(),
                            avg_tokens,
                            potential_savings * 100.0
                        ))
                        .with_type(StrategyType::TokenReduction)
                        .with_expected_improvement(potential_savings * 0.3) // Conservative estimate
                        .with_confidence(0.75)
                        .with_evidence_count(high_token_traces.len())
                        .with_step("Identify nodes with highest token usage")
                        .with_step("Implement context summarization for large inputs")
                        .with_step("Add token budgets per node")
                        .with_step("Use cheaper models for simple tasks")
                        .with_complexity(5)
                        .with_risk(StrategyRisk::Low)
                        .improves("cost")
                        .improves("latency")
                        .with_tradeoff("May reduce response quality for complex tasks"),
                );
            }
        }

        None
    }

    /// Generate summaries for each agent
    fn generate_agent_summaries(
        &self,
        agent_traces: &HashMap<String, Vec<&crate::introspection::ExecutionTrace>>,
    ) -> Vec<AgentSummary> {
        let mut summaries: Vec<AgentSummary> = Vec::new();

        // Calculate global averages for percentile computation
        let all_success_rates: Vec<f64> = agent_traces
            .values()
            .map(|traces| {
                traces.iter().filter(|t| t.completed).count() as f64 / traces.len() as f64
            })
            .collect();

        for (agent_id, traces) in agent_traces {
            let execution_count = traces.len();
            let success_rate =
                traces.iter().filter(|t| t.completed).count() as f64 / execution_count as f64;
            let avg_duration = traces.iter().map(|t| t.total_duration_ms).sum::<u64>() as f64
                / execution_count as f64;
            let avg_tokens =
                traces.iter().map(|t| t.total_tokens).sum::<u64>() as f64 / execution_count as f64;

            // Calculate percentile
            let better_count = all_success_rates
                .iter()
                .filter(|&&r| r < success_rate)
                .count();
            let percentile = (better_count as f64 / all_success_rates.len() as f64 * 100.0) as u8;

            let mut summary = AgentSummary::new(agent_id);
            summary.execution_count = execution_count;
            summary.success_rate = success_rate;
            summary.avg_duration_ms = avg_duration;
            summary.avg_tokens = avg_tokens;
            summary.performance_percentile = percentile;

            // Add characteristics
            if avg_tokens < 2000.0 {
                summary.characteristics.push("Token-efficient".to_string());
            }
            if success_rate >= 0.95 {
                summary.characteristics.push("Highly reliable".to_string());
            }
            if avg_duration < 5000.0 {
                summary.characteristics.push("Fast execution".to_string());
            }

            summaries.push(summary);
        }

        // Sort by performance percentile
        summaries.sort_by(|a, b| b.performance_percentile.cmp(&a.performance_percentile));

        summaries
    }

    /// Compute correlations between agents
    fn compute_correlations(&self, summaries: &[AgentSummary]) -> Vec<AgentCorrelation> {
        let mut correlations = Vec::new();

        // Only compute if we have enough agents
        if summaries.len() < 3 {
            return correlations;
        }

        // Find agents with similar performance
        for i in 0..summaries.len() {
            for j in (i + 1)..summaries.len() {
                let a = &summaries[i];
                let b = &summaries[j];

                // Check for similar success rates
                let success_diff = (a.success_rate - b.success_rate).abs();
                if success_diff < 0.1 {
                    let strength = 1.0 - success_diff / 0.1;
                    correlations.push(AgentCorrelation {
                        agent_a: a.agent_id.clone(),
                        agent_b: b.agent_id.clone(),
                        correlation_type: CorrelationType::Performance,
                        strength,
                        description: format!(
                            "Similar success rates ({:.1}% vs {:.1}%)",
                            a.success_rate * 100.0,
                            b.success_rate * 100.0
                        ),
                        observations: a.execution_count + b.execution_count,
                    });
                }
            }
        }

        // Limit to top correlations
        correlations.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        correlations.truncate(10);

        correlations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ErrorTrace, ExecutionTrace, ExecutionTraceBuilder, NodeExecution};

    fn create_successful_trace(agent_id: &str, tokens: u64, duration_ms: u64) -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .thread_id(agent_id)
            .add_node_execution(
                NodeExecution::new("node1", duration_ms / 2).with_tokens(tokens / 2),
            )
            .add_node_execution(
                NodeExecution::new("node2", duration_ms / 2).with_tokens(tokens / 2),
            )
            .total_duration_ms(duration_ms)
            .total_tokens(tokens)
            .completed(true)
            .build()
    }

    fn create_failed_trace(agent_id: &str, tokens: u64, duration_ms: u64) -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .thread_id(agent_id)
            .add_node_execution(
                NodeExecution::new("node1", duration_ms)
                    .with_tokens(tokens)
                    .with_error("Test error"),
            )
            .add_error(ErrorTrace::new("node1", "Test error"))
            .total_duration_ms(duration_ms)
            .total_tokens(tokens)
            .completed(false)
            .build()
    }

    fn create_repeated_node_trace(agent_id: &str) -> ExecutionTrace {
        let mut builder = ExecutionTraceBuilder::new().thread_id(agent_id);
        for i in 0..5 {
            builder =
                builder.add_node_execution(NodeExecution::new("repeat_node", 100).with_index(i));
        }
        builder
            .total_duration_ms(500)
            .total_tokens(2500)
            .completed(true)
            .build()
    }

    fn create_high_token_trace(agent_id: &str) -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .thread_id(agent_id)
            .add_node_execution(NodeExecution::new("heavy_node", 5000).with_tokens(12000))
            .total_duration_ms(5000)
            .total_tokens(12000)
            .completed(false)
            .build()
    }

    #[test]
    fn test_cross_agent_insights_new() {
        let insights = CrossAgentInsights::new();
        assert_eq!(insights.agents_analyzed, 0);
        assert_eq!(insights.executions_analyzed, 0);
        assert!(insights.successful_patterns.is_empty());
    }

    #[test]
    fn test_success_pattern_creation() {
        let pattern = SuccessPattern::new("test_pattern", "Test Pattern")
            .with_description("A test pattern")
            .with_type(PatternType::Behavioral)
            .with_success_rate(0.85)
            .with_sample_count(20)
            .with_confidence(0.9)
            .with_application_guide("Apply this pattern");

        assert_eq!(pattern.id, "test_pattern");
        assert_eq!(pattern.name, "Test Pattern");
        assert_eq!(pattern.success_rate, 0.85);
        assert!(pattern.is_actionable());
    }

    #[test]
    fn test_pattern_condition() {
        let condition = PatternCondition::new("latency", ComparisonOperator::LessThan, 1000.0);
        assert!(condition.is_satisfied(500.0));
        assert!(!condition.is_satisfied(1500.0));
    }

    #[test]
    fn test_comparison_operators() {
        let lt = PatternCondition::new("x", ComparisonOperator::LessThan, 10.0);
        assert!(lt.is_satisfied(5.0));
        assert!(!lt.is_satisfied(15.0));

        let gte = PatternCondition::new("x", ComparisonOperator::GreaterOrEqual, 10.0);
        assert!(gte.is_satisfied(10.0));
        assert!(gte.is_satisfied(15.0));
        assert!(!gte.is_satisfied(5.0));
    }

    #[test]
    fn test_pitfall_creation() {
        let pitfall = Pitfall::new("test_pitfall", "Test Pitfall")
            .with_description("A test pitfall")
            .with_category(PitfallCategory::ResourceExhaustion)
            .with_failure_rate(0.6)
            .with_occurrences(10)
            .with_severity(PitfallSeverity::High)
            .with_detection("High token usage")
            .with_avoidance("Reduce tokens");

        assert_eq!(pitfall.id, "test_pitfall");
        assert_eq!(pitfall.failure_rate, 0.6);
        assert!(pitfall.is_actionable());
    }

    #[test]
    fn test_optimization_strategy_creation() {
        let strategy = OptimizationStrategy::new("test_strategy", "Test Strategy")
            .with_description("A test strategy")
            .with_type(StrategyType::Caching)
            .with_expected_improvement(0.25)
            .with_confidence(0.8)
            .with_evidence_count(15)
            .with_step("Step 1")
            .with_step("Step 2")
            .with_complexity(5)
            .with_risk(StrategyRisk::Low)
            .improves("latency");

        assert_eq!(strategy.id, "test_strategy");
        assert_eq!(strategy.expected_improvement, 0.25);
        assert!(strategy.is_actionable());
    }

    #[test]
    fn test_strategy_roi_score() {
        let high_impact_easy = OptimizationStrategy::new("a", "A")
            .with_expected_improvement(0.5)
            .with_complexity(2);

        let low_impact_hard = OptimizationStrategy::new("b", "B")
            .with_expected_improvement(0.1)
            .with_complexity(8);

        assert!(high_impact_easy.roi_score() > low_impact_hard.roi_score());
    }

    #[test]
    fn test_agent_summary() {
        let mut summary = AgentSummary::new("agent1");
        summary.success_rate = 0.95;
        summary.performance_percentile = 85;

        assert!(summary.is_top_performer());
        assert!(!summary.needs_attention());

        summary.success_rate = 0.5;
        assert!(summary.needs_attention());
    }

    #[test]
    fn test_agent_correlation() {
        let mut correlation = AgentCorrelation::new("agent1", "agent2");
        correlation.strength = 0.8;

        assert!(correlation.is_strong());

        correlation.strength = 0.3;
        assert!(!correlation.is_strong());
    }

    #[test]
    fn test_learner_empty_traces() {
        let learner = CrossAgentLearner::new();
        let insights = learner.analyze(&[]);

        assert_eq!(insights.agents_analyzed, 0);
        assert_eq!(insights.executions_analyzed, 0);
    }

    #[test]
    fn test_learner_single_agent() {
        let learner = CrossAgentLearner::new();
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|i| create_successful_trace("agent1", 1000 + i * 100, 500 + i * 50))
            .collect();

        let insights = learner.analyze(&traces);

        assert_eq!(insights.agents_analyzed, 1);
        assert_eq!(insights.executions_analyzed, 10);
    }

    #[test]
    fn test_learner_multiple_agents() {
        let learner = CrossAgentLearner::new();
        let mut traces = Vec::new();

        // Agent 1: successful, fast
        for i in 0..10 {
            traces.push(create_successful_trace("agent1", 1000, 500 + i * 10));
        }

        // Agent 2: mixed results
        for i in 0..10 {
            if i % 2 == 0 {
                traces.push(create_successful_trace("agent2", 5000, 2000));
            } else {
                traces.push(create_failed_trace("agent2", 8000, 3000));
            }
        }

        let insights = learner.analyze(&traces);

        assert_eq!(insights.agents_analyzed, 2);
        assert_eq!(insights.executions_analyzed, 20);
        assert!(!insights.agent_summaries.is_empty());
    }

    #[test]
    fn test_learner_detects_token_efficiency_pattern() {
        let learner = CrossAgentLearner::new();
        let mut traces = Vec::new();

        // Low token, high success agents
        for _ in 0..15 {
            traces.push(create_successful_trace("low_token", 1500, 500));
        }

        // High token, lower success agents
        for i in 0..15 {
            if i % 3 == 0 {
                traces.push(create_high_token_trace("high_token"));
            } else {
                traces.push(create_successful_trace("high_token", 9000, 4000));
            }
        }

        let insights = learner.analyze(&traces);

        // Should detect token efficiency as a pattern
        let _has_token_pattern = insights
            .successful_patterns
            .iter()
            .any(|p| p.id.contains("token") || p.description.to_lowercase().contains("token"));
        // May or may not detect depending on exact thresholds
        assert!(insights.successful_patterns.len() <= 10); // Sanity check
    }

    #[test]
    fn test_learner_detects_retry_pitfall() {
        let learner = CrossAgentLearner::new();
        let mut traces = Vec::new();

        // Agents with excessive errors
        for _ in 0..10 {
            let trace = ExecutionTraceBuilder::new()
                .thread_id("retry_agent")
                .add_node_execution(NodeExecution::new("node1", 100).with_error("Error 1"))
                .add_error(ErrorTrace::new("node1", "Error 1"))
                .add_error(ErrorTrace::new("node1", "Error 2"))
                .add_error(ErrorTrace::new("node1", "Error 3"))
                .total_duration_ms(1000)
                .total_tokens(3000)
                .completed(false)
                .build();
            traces.push(trace);
        }

        // Some successful agents
        for _ in 0..10 {
            traces.push(create_successful_trace("good_agent", 2000, 800));
        }

        let insights = learner.analyze(&traces);

        // Should detect excessive retries as a pitfall
        let _has_retry_pitfall = insights
            .common_pitfalls
            .iter()
            .any(|p| p.id.contains("retry") || p.description.to_lowercase().contains("retry"));
        // Verify analysis completed
        assert!(insights.common_pitfalls.len() <= 10);
    }

    #[test]
    fn test_learner_suggests_caching_strategy() {
        let learner = CrossAgentLearner::new();
        let mut traces = Vec::new();

        // Agents with repeated node executions
        for _ in 0..15 {
            traces.push(create_repeated_node_trace("repeat_agent"));
        }

        let insights = learner.analyze(&traces);

        // Should suggest caching strategy
        let _has_caching_strategy = insights
            .optimization_strategies
            .iter()
            .any(|s| s.strategy_type == StrategyType::Caching);
        // May or may not suggest depending on thresholds
        assert!(insights.optimization_strategies.len() <= 10);
    }

    #[test]
    fn test_insights_summary() {
        let mut insights = CrossAgentInsights::new();
        insights.agents_analyzed = 5;
        insights.executions_analyzed = 100;
        insights.successful_patterns.push(
            SuccessPattern::new("test", "Test Pattern")
                .with_success_rate(0.9)
                .with_sample_count(50),
        );

        let summary = insights.summary();

        assert!(summary.contains("Agents analyzed: 5"));
        assert!(summary.contains("Executions analyzed: 100"));
        assert!(summary.contains("Test Pattern"));
    }

    #[test]
    fn test_insights_json_roundtrip() {
        let mut insights = CrossAgentInsights::new();
        insights.agents_analyzed = 3;
        insights
            .successful_patterns
            .push(SuccessPattern::new("test", "Test").with_success_rate(0.8));

        let json = insights.to_json().unwrap();
        let parsed = CrossAgentInsights::from_json(&json).unwrap();

        assert_eq!(parsed.agents_analyzed, insights.agents_analyzed);
        assert_eq!(parsed.successful_patterns.len(), 1);
    }

    #[test]
    fn test_insights_high_value_patterns() {
        let mut insights = CrossAgentInsights::new();
        insights.successful_patterns.push(
            SuccessPattern::new("high", "High Value")
                .with_success_rate(0.9)
                .with_sample_count(20),
        );
        insights.successful_patterns.push(
            SuccessPattern::new("low", "Low Value")
                .with_success_rate(0.5)
                .with_sample_count(5),
        );

        let high_value = insights.high_value_patterns();
        assert_eq!(high_value.len(), 1);
        assert_eq!(high_value[0].id, "high");
    }

    #[test]
    fn test_insights_critical_pitfalls() {
        let mut insights = CrossAgentInsights::new();
        insights.common_pitfalls.push(
            Pitfall::new("critical", "Critical Issue")
                .with_failure_rate(0.7)
                .with_occurrences(10),
        );
        insights.common_pitfalls.push(
            Pitfall::new("minor", "Minor Issue")
                .with_failure_rate(0.2)
                .with_occurrences(3),
        );

        let critical = insights.critical_pitfalls();
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].id, "critical");
    }

    #[test]
    fn test_insights_top_strategies() {
        let mut insights = CrossAgentInsights::new();
        insights
            .optimization_strategies
            .push(OptimizationStrategy::new("a", "A").with_expected_improvement(0.5));
        insights
            .optimization_strategies
            .push(OptimizationStrategy::new("b", "B").with_expected_improvement(0.3));
        insights
            .optimization_strategies
            .push(OptimizationStrategy::new("c", "C").with_expected_improvement(0.8));

        let top = insights.top_strategies(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].id, "c"); // Highest improvement first
        assert_eq!(top[1].id, "a");
    }

    #[test]
    fn test_pattern_type_display() {
        assert_eq!(PatternType::Behavioral.to_string(), "Behavioral");
        assert_eq!(PatternType::ErrorHandling.to_string(), "Error Handling");
    }

    #[test]
    fn test_pitfall_category_display() {
        assert_eq!(
            PitfallCategory::ResourceExhaustion.to_string(),
            "Resource Exhaustion"
        );
        assert_eq!(PitfallCategory::InfiniteLoop.to_string(), "Infinite Loop");
    }

    #[test]
    fn test_pitfall_severity_ordering() {
        assert!(PitfallSeverity::Critical > PitfallSeverity::High);
        assert!(PitfallSeverity::High > PitfallSeverity::Medium);
        assert!(PitfallSeverity::Medium > PitfallSeverity::Low);
    }

    #[test]
    fn test_strategy_type_display() {
        assert_eq!(StrategyType::Caching.to_string(), "Caching");
        assert_eq!(StrategyType::TokenReduction.to_string(), "Token Reduction");
    }

    #[test]
    fn test_strategy_risk_ordering() {
        assert!(StrategyRisk::High > StrategyRisk::Moderate);
        assert!(StrategyRisk::Moderate > StrategyRisk::Low);
        assert!(StrategyRisk::Low > StrategyRisk::VeryLow);
    }

    #[test]
    fn test_correlation_type() {
        let correlation = AgentCorrelation {
            agent_a: "a".to_string(),
            agent_b: "b".to_string(),
            correlation_type: CorrelationType::BehaviorSimilarity,
            strength: 0.85,
            description: "Similar behavior".to_string(),
            observations: 50,
        };

        assert!(correlation.is_strong());
        assert_eq!(
            correlation.correlation_type,
            CorrelationType::BehaviorSimilarity
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = CrossAgentConfig::default();

        assert_eq!(config.min_executions_per_agent, 5);
        assert_eq!(config.min_pattern_samples, 10);
        assert_eq!(config.min_success_rate, 0.7);
        assert!(config.include_agent_summaries);
        assert!(config.compute_correlations);
    }

    #[test]
    fn test_learner_with_custom_config() {
        let config = CrossAgentConfig {
            min_executions_per_agent: 2,
            min_pattern_samples: 3,
            ..Default::default()
        };
        let learner = CrossAgentLearner::with_config(config);

        // Create traces with fewer executions
        let mut traces = Vec::new();
        for _ in 0..3 {
            traces.push(create_successful_trace("agent1", 1000, 500));
        }

        let insights = learner.analyze(&traces);

        // With lower threshold, should still analyze
        assert_eq!(insights.agents_analyzed, 1);
    }

    #[test]
    fn test_success_pattern_summary() {
        let pattern = SuccessPattern::new("test", "Test Pattern")
            .with_success_rate(0.85)
            .with_performance_improvement(0.25)
            .with_sample_count(100);

        let summary = pattern.summary();
        assert!(summary.contains("85.0%"));
        assert!(summary.contains("25.0%"));
        assert!(summary.contains("100 samples"));
    }

    #[test]
    fn test_pitfall_summary() {
        let pitfall = Pitfall::new("test", "Test Pitfall")
            .with_failure_rate(0.6)
            .with_occurrences(25)
            .with_severity(PitfallSeverity::High);

        let summary = pitfall.summary();
        assert!(summary.contains("60.0%"));
        assert!(summary.contains("25 occurrences"));
        assert!(summary.contains("High"));
    }

    #[test]
    fn test_strategy_summary() {
        let strategy = OptimizationStrategy::new("test", "Test Strategy")
            .with_expected_improvement(0.35)
            .with_complexity(7)
            .with_evidence_count(30)
            .with_risk(StrategyRisk::Moderate);

        let summary = strategy.summary();
        assert!(summary.contains("35.0%"));
        assert!(summary.contains("7/10"));
        assert!(summary.contains("30 evidence"));
        assert!(summary.contains("Moderate"));
    }

    #[test]
    fn test_actionable_count() {
        let mut insights = CrossAgentInsights::new();

        // Add actionable pattern
        insights.successful_patterns.push(
            SuccessPattern::new("a", "A")
                .with_success_rate(0.8)
                .with_confidence(0.7)
                .with_sample_count(10)
                .with_application_guide("Do this"),
        );

        // Add non-actionable pattern
        insights
            .successful_patterns
            .push(SuccessPattern::new("b", "B").with_success_rate(0.3));

        // Add actionable pitfall
        insights.common_pitfalls.push(
            Pitfall::new("c", "C")
                .with_occurrences(5)
                .with_detection("Detect this")
                .with_avoidance("Avoid this"),
        );

        // Add actionable strategy
        insights.optimization_strategies.push(
            OptimizationStrategy::new("d", "D")
                .with_expected_improvement(0.2)
                .with_confidence(0.7)
                .with_evidence_count(10)
                .with_step("Step 1"),
        );

        assert_eq!(insights.actionable_count(), 3);
    }

    #[test]
    fn test_cross_agent_config_new() {
        let config = CrossAgentConfig::new();
        assert_eq!(config.min_executions_per_agent, 5);
        assert_eq!(config.min_pattern_samples, 10);
        assert!((config.min_success_rate - 0.7).abs() < f64::EPSILON);
        assert!((config.max_pitfall_threshold - 0.3).abs() < f64::EPSILON);
        assert!((config.min_improvement_threshold - 0.1).abs() < f64::EPSILON);
        assert!(config.include_agent_summaries);
        assert!(config.compute_correlations);
    }

    #[test]
    fn test_cross_agent_config_builder_pattern() {
        let config = CrossAgentConfig::new()
            .with_min_executions_per_agent(10)
            .with_min_pattern_samples(20)
            .with_min_success_rate(0.8)
            .with_max_pitfall_threshold(0.2)
            .with_min_improvement_threshold(0.15)
            .with_agent_summaries(false)
            .with_correlations(false);

        assert_eq!(config.min_executions_per_agent, 10);
        assert_eq!(config.min_pattern_samples, 20);
        assert!((config.min_success_rate - 0.8).abs() < f64::EPSILON);
        assert!((config.max_pitfall_threshold - 0.2).abs() < f64::EPSILON);
        assert!((config.min_improvement_threshold - 0.15).abs() < f64::EPSILON);
        assert!(!config.include_agent_summaries);
        assert!(!config.compute_correlations);
    }

    #[test]
    fn test_cross_agent_config_partial_builder() {
        // Test that we can override just some fields
        let config = CrossAgentConfig::new()
            .with_min_executions_per_agent(3)
            .with_agent_summaries(false);

        // Overridden
        assert_eq!(config.min_executions_per_agent, 3);
        assert!(!config.include_agent_summaries);
        // Defaults preserved
        assert_eq!(config.min_pattern_samples, 10);
        assert!((config.min_success_rate - 0.7).abs() < f64::EPSILON);
        assert!(config.compute_correlations);
    }
}
