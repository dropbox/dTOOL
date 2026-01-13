//!  Optimization Telemetry
//!
//! This module provides telemetry types for tracking optimization decisions
//! and their outcomes during graph execution.

use super::graph_manifest::NodeConfig;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

//  Optimization Telemetry (Meta-Learning)
// ============================================================================

/// Telemetry for the optimization loop itself, enabling meta-learning.
///
/// This struct tracks the optimization process - not individual executions, but the
/// meta-level of how optimization proceeds. This enables questions like:
/// - "Which optimization strategies work best for prompt tuning?"
/// - "How many variants are typically needed to find improvement?"
/// - "What's the average improvement delta for temperature tuning?"
///
/// # Example
///
/// ```rust
/// use dashflow::introspection::{OptimizationTrace, VariantResult, TerminationReason};
/// use dashflow::introspection::NodeConfig;
/// use serde_json::json;
///
/// let variant = VariantResult::new("variant_001")
///     .with_config(NodeConfig::new("llm_node", "llm.chat")
///         .with_config(json!({"temperature": 0.8})))
///     .with_execution_trace_id("exec_abc123")
///     .with_score(0.85)
///     .with_metric("latency_ms", 450.0);
///
/// let trace = OptimizationTrace::new("opt_001")
///     .with_strategy(dashflow::optimize::OptimizationStrategy::Joint)
///     .with_target_node("llm_node")
///     .with_target_param("temperature")
///     .with_variant(variant.clone())
///     .with_best_variant(variant)
///     .with_termination_reason(TerminationReason::ConvergenceThreshold(0.01))
///     .with_duration_ms(120000)
///     .with_improvement_delta(0.15);
///
/// // Query meta-learning data
/// assert!(trace.improvement_delta > 0.0);
/// assert_eq!(trace.variants_tested.len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationTrace {
    /// Unique identifier for this optimization run
    pub optimization_id: String,
    /// Strategy used for optimization (Sequential, Joint, Alternating)
    pub strategy: Option<String>,
    /// Target node being optimized
    pub target_node: String,
    /// Target parameter within the node (e.g., "temperature", "system_prompt")
    pub target_param: String,
    /// All variants tested during optimization
    pub variants_tested: Vec<VariantResult>,
    /// The best variant found (if any improvement was made)
    pub best_variant: Option<VariantResult>,
    /// Why the optimization terminated
    pub termination_reason: TerminationReason,
    /// Total duration of optimization in milliseconds
    pub total_duration_ms: u64,
    /// Score improvement: best_score - initial_score
    pub improvement_delta: f64,
    /// Initial score before optimization
    pub initial_score: Option<f64>,
    /// Timestamp when optimization started (ISO 8601)
    pub started_at: Option<String>,
    /// Timestamp when optimization ended (ISO 8601)
    pub ended_at: Option<String>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for OptimizationTrace {
    fn default() -> Self {
        Self {
            optimization_id: String::new(),
            strategy: None,
            target_node: String::new(),
            target_param: String::new(),
            variants_tested: Vec::new(),
            best_variant: None,
            termination_reason: TerminationReason::Unknown,
            total_duration_ms: 0,
            improvement_delta: 0.0,
            initial_score: None,
            started_at: None,
            ended_at: None,
            metadata: HashMap::new(),
        }
    }
}

impl OptimizationTrace {
    /// Create a new optimization trace with the given ID
    #[must_use]
    pub fn new(optimization_id: impl Into<String>) -> Self {
        Self {
            optimization_id: optimization_id.into(),
            started_at: Some(Utc::now().to_rfc3339()),
            ..Default::default()
        }
    }

    /// Set the optimization strategy
    #[must_use]
    pub fn with_strategy(mut self, strategy: crate::optimize::OptimizationStrategy) -> Self {
        self.strategy = Some(format!("{:?}", strategy));
        self
    }

    /// Set the optimization strategy from a string
    #[must_use]
    pub fn with_strategy_name(mut self, strategy: impl Into<String>) -> Self {
        self.strategy = Some(strategy.into());
        self
    }

    /// Set the target node
    #[must_use]
    pub fn with_target_node(mut self, node: impl Into<String>) -> Self {
        self.target_node = node.into();
        self
    }

    /// Set the target parameter
    #[must_use]
    pub fn with_target_param(mut self, param: impl Into<String>) -> Self {
        self.target_param = param.into();
        self
    }

    /// Add a variant result
    #[must_use]
    pub fn with_variant(mut self, variant: VariantResult) -> Self {
        self.variants_tested.push(variant);
        self
    }

    /// Set the best variant found
    #[must_use]
    pub fn with_best_variant(mut self, variant: VariantResult) -> Self {
        self.best_variant = Some(variant);
        self
    }

    /// Set the termination reason
    #[must_use]
    pub fn with_termination_reason(mut self, reason: TerminationReason) -> Self {
        self.termination_reason = reason;
        self
    }

    /// Set the total duration in milliseconds
    #[must_use]
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.total_duration_ms = ms;
        self
    }

    /// Set the improvement delta
    #[must_use]
    pub fn with_improvement_delta(mut self, delta: f64) -> Self {
        self.improvement_delta = delta;
        self
    }

    /// Set the initial score
    #[must_use]
    pub fn with_initial_score(mut self, score: f64) -> Self {
        self.initial_score = Some(score);
        self
    }

    /// Mark the optimization as complete
    #[must_use]
    pub fn complete(mut self) -> Self {
        self.ended_at = Some(Utc::now().to_rfc3339());
        self
    }

    /// Add custom metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get the number of variants tested
    #[must_use]
    pub fn variant_count(&self) -> usize {
        self.variants_tested.len()
    }

    /// Check if optimization found an improvement
    #[must_use]
    pub fn found_improvement(&self) -> bool {
        self.improvement_delta > 0.0 && self.best_variant.is_some()
    }

    /// Get the best score achieved
    #[must_use]
    pub fn best_score(&self) -> Option<f64> {
        self.best_variant.as_ref().map(|v| v.score)
    }

    /// Calculate duration in seconds
    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        self.total_duration_ms as f64 / 1000.0
    }

    /// Generate a human-readable summary
    #[must_use]
    pub fn summary(&self) -> String {
        let strategy_str = self.strategy.as_deref().unwrap_or("Unknown");
        let result = if self.found_improvement() {
            format!(
                "improved by {:.1}% (best score: {:.3})",
                self.improvement_delta * 100.0,
                self.best_score().unwrap_or(0.0)
            )
        } else {
            "no improvement found".to_string()
        };

        format!(
            "Optimization '{}' on {}.{}: {} variants tested using {} strategy, {} in {:.1}s. Terminated: {}",
            self.optimization_id,
            self.target_node,
            self.target_param,
            self.variant_count(),
            strategy_str,
            result,
            self.duration_seconds(),
            self.termination_reason.description()
        )
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Result from testing a single variant during optimization.
///
/// Links a specific config variant to its execution trace and score,
/// enabling correlation between config changes and performance outcomes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantResult {
    /// Unique identifier for this variant
    pub variant_id: String,
    /// The config that was tested
    pub config: Option<NodeConfig>,
    /// Links to the ExecutionTrace for this variant's run
    pub execution_trace_id: String,
    /// Score achieved by this variant (higher is better)
    pub score: f64,
    /// Additional metrics collected during evaluation
    pub metrics: HashMap<String, f64>,
    /// Timestamp when this variant was tested (ISO 8601)
    pub tested_at: Option<String>,
    /// How long this variant took to evaluate in milliseconds
    pub evaluation_duration_ms: u64,
}

impl Default for VariantResult {
    fn default() -> Self {
        Self {
            variant_id: String::new(),
            config: None,
            execution_trace_id: String::new(),
            score: 0.0,
            metrics: HashMap::new(),
            tested_at: None,
            evaluation_duration_ms: 0,
        }
    }
}

impl VariantResult {
    /// Create a new variant result
    #[must_use]
    pub fn new(variant_id: impl Into<String>) -> Self {
        Self {
            variant_id: variant_id.into(),
            tested_at: Some(Utc::now().to_rfc3339()),
            ..Default::default()
        }
    }

    /// Set the config for this variant
    #[must_use]
    pub fn with_config(mut self, config: NodeConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the execution trace ID
    #[must_use]
    pub fn with_execution_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.execution_trace_id = trace_id.into();
        self
    }

    /// Set the score
    #[must_use]
    pub fn with_score(mut self, score: f64) -> Self {
        self.score = score;
        self
    }

    /// Add a metric
    #[must_use]
    pub fn with_metric(mut self, name: impl Into<String>, value: f64) -> Self {
        self.metrics.insert(name.into(), value);
        self
    }

    /// Set multiple metrics at once
    #[must_use]
    pub fn with_metrics(mut self, metrics: HashMap<String, f64>) -> Self {
        self.metrics = metrics;
        self
    }

    /// Set the evaluation duration
    #[must_use]
    pub fn with_evaluation_duration_ms(mut self, ms: u64) -> Self {
        self.evaluation_duration_ms = ms;
        self
    }

    /// Get a specific metric value
    #[must_use]
    pub fn get_metric(&self, name: &str) -> Option<f64> {
        self.metrics.get(name).copied()
    }

    /// Get the config hash if available
    #[must_use]
    pub fn config_hash(&self) -> Option<&str> {
        self.config.as_ref().map(|c| c.config_hash.as_str())
    }

    /// Get the config version if available
    #[must_use]
    pub fn config_version(&self) -> Option<u64> {
        self.config.as_ref().map(|c| c.version)
    }
}

/// Reason why an optimization run terminated.
///
/// Captures the termination condition along with relevant parameters,
/// enabling analysis of optimization efficiency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum TerminationReason {
    /// Hit the maximum number of iterations
    MaxIterations(usize),
    /// Score improvement fell below convergence threshold
    ConvergenceThreshold(f64),
    /// Exceeded time limit (milliseconds)
    TimeLimit(u64),
    /// No improvement for N consecutive iterations
    NoImprovement {
        /// Number of iterations without improvement
        iterations: usize,
    },
    /// User or system requested stop
    UserStopped,
    /// Error occurred during optimization
    Error(String),
    /// Unknown termination reason
    #[default]
    Unknown,
}

impl TerminationReason {
    /// Get a human-readable description
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::MaxIterations(n) => format!("max iterations ({n})"),
            Self::ConvergenceThreshold(t) => format!("converged (threshold: {t:.4})"),
            Self::TimeLimit(ms) => format!("time limit ({:.1}s)", *ms as f64 / 1000.0),
            Self::NoImprovement { iterations } => {
                format!("no improvement ({iterations} iterations)")
            }
            Self::UserStopped => "user stopped".to_string(),
            Self::Error(msg) => format!("error: {msg}"),
            Self::Unknown => "unknown".to_string(),
        }
    }

    /// Check if termination was successful (not an error)
    #[must_use]
    pub fn is_success(&self) -> bool {
        !matches!(self, Self::Error(_) | Self::Unknown)
    }

    /// Check if optimization converged
    #[must_use]
    pub fn converged(&self) -> bool {
        matches!(self, Self::ConvergenceThreshold(_))
    }
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimize::OptimizationStrategy;
    use serde_json::json;

    // ========================================================================
    // OptimizationTrace Tests
    // ========================================================================

    #[test]
    fn test_optimization_trace_new() {
        let trace = OptimizationTrace::new("opt_001");
        assert_eq!(trace.optimization_id, "opt_001");
        assert!(trace.started_at.is_some());
        assert!(trace.strategy.is_none());
        assert!(trace.target_node.is_empty());
        assert!(trace.target_param.is_empty());
        assert!(trace.variants_tested.is_empty());
        assert!(trace.best_variant.is_none());
        assert_eq!(trace.termination_reason, TerminationReason::Unknown);
        assert_eq!(trace.total_duration_ms, 0);
        assert_eq!(trace.improvement_delta, 0.0);
    }

    #[test]
    fn test_optimization_trace_default() {
        let trace = OptimizationTrace::default();
        assert!(trace.optimization_id.is_empty());
        assert!(trace.started_at.is_none());
    }

    #[test]
    fn test_optimization_trace_with_strategy() {
        let trace = OptimizationTrace::new("opt_001").with_strategy(OptimizationStrategy::Joint);

        assert!(trace.strategy.is_some());
        assert!(trace.strategy.unwrap().contains("Joint"));
    }

    #[test]
    fn test_optimization_trace_with_strategy_name() {
        let trace = OptimizationTrace::new("opt_001").with_strategy_name("CustomStrategy");

        assert_eq!(trace.strategy, Some("CustomStrategy".to_string()));
    }

    #[test]
    fn test_optimization_trace_with_target_node() {
        let trace = OptimizationTrace::new("opt_001").with_target_node("llm_node");

        assert_eq!(trace.target_node, "llm_node");
    }

    #[test]
    fn test_optimization_trace_with_target_param() {
        let trace = OptimizationTrace::new("opt_001").with_target_param("temperature");

        assert_eq!(trace.target_param, "temperature");
    }

    #[test]
    fn test_optimization_trace_with_variant() {
        let variant = VariantResult::new("var_001").with_score(0.85);
        let trace = OptimizationTrace::new("opt_001").with_variant(variant);

        assert_eq!(trace.variants_tested.len(), 1);
        assert_eq!(trace.variants_tested[0].variant_id, "var_001");
    }

    #[test]
    fn test_optimization_trace_with_multiple_variants() {
        let trace = OptimizationTrace::new("opt_001")
            .with_variant(VariantResult::new("var_001").with_score(0.80))
            .with_variant(VariantResult::new("var_002").with_score(0.85))
            .with_variant(VariantResult::new("var_003").with_score(0.90));

        assert_eq!(trace.variants_tested.len(), 3);
    }

    #[test]
    fn test_optimization_trace_with_best_variant() {
        let best = VariantResult::new("best_001").with_score(0.95);
        let trace = OptimizationTrace::new("opt_001").with_best_variant(best);

        assert!(trace.best_variant.is_some());
        assert_eq!(trace.best_variant.unwrap().score, 0.95);
    }

    #[test]
    fn test_optimization_trace_with_termination_reason() {
        let trace = OptimizationTrace::new("opt_001")
            .with_termination_reason(TerminationReason::MaxIterations(100));

        assert_eq!(
            trace.termination_reason,
            TerminationReason::MaxIterations(100)
        );
    }

    #[test]
    fn test_optimization_trace_with_duration_ms() {
        let trace = OptimizationTrace::new("opt_001").with_duration_ms(120000);

        assert_eq!(trace.total_duration_ms, 120000);
    }

    #[test]
    fn test_optimization_trace_with_improvement_delta() {
        let trace = OptimizationTrace::new("opt_001").with_improvement_delta(0.15);

        assert_eq!(trace.improvement_delta, 0.15);
    }

    #[test]
    fn test_optimization_trace_with_initial_score() {
        let trace = OptimizationTrace::new("opt_001").with_initial_score(0.70);

        assert_eq!(trace.initial_score, Some(0.70));
    }

    #[test]
    fn test_optimization_trace_complete() {
        let trace = OptimizationTrace::new("opt_001").complete();

        assert!(trace.ended_at.is_some());
    }

    #[test]
    fn test_optimization_trace_with_metadata() {
        let trace = OptimizationTrace::new("opt_001")
            .with_metadata("key1", json!("value1"))
            .with_metadata("key2", json!(42));

        assert_eq!(trace.metadata.len(), 2);
        assert_eq!(trace.metadata.get("key1"), Some(&json!("value1")));
    }

    #[test]
    fn test_optimization_trace_variant_count() {
        let trace = OptimizationTrace::new("opt_001")
            .with_variant(VariantResult::new("v1"))
            .with_variant(VariantResult::new("v2"));

        assert_eq!(trace.variant_count(), 2);
    }

    #[test]
    fn test_optimization_trace_found_improvement() {
        let no_improvement = OptimizationTrace::new("opt_001").with_improvement_delta(0.0);
        assert!(!no_improvement.found_improvement());

        let negative = OptimizationTrace::new("opt_002").with_improvement_delta(-0.1);
        assert!(!negative.found_improvement());

        let positive_no_best = OptimizationTrace::new("opt_003").with_improvement_delta(0.1);
        assert!(!positive_no_best.found_improvement());

        let positive_with_best = OptimizationTrace::new("opt_004")
            .with_improvement_delta(0.1)
            .with_best_variant(VariantResult::new("best"));
        assert!(positive_with_best.found_improvement());
    }

    #[test]
    fn test_optimization_trace_best_score() {
        let no_best = OptimizationTrace::new("opt_001");
        assert!(no_best.best_score().is_none());

        let with_best = OptimizationTrace::new("opt_002")
            .with_best_variant(VariantResult::new("best").with_score(0.95));
        assert_eq!(with_best.best_score(), Some(0.95));
    }

    #[test]
    fn test_optimization_trace_duration_seconds() {
        let trace = OptimizationTrace::new("opt_001").with_duration_ms(60000);
        assert_eq!(trace.duration_seconds(), 60.0);

        let trace2 = OptimizationTrace::new("opt_002").with_duration_ms(90500);
        assert_eq!(trace2.duration_seconds(), 90.5);
    }

    #[test]
    fn test_optimization_trace_summary_with_improvement() {
        let trace = OptimizationTrace::new("opt_001")
            .with_strategy(OptimizationStrategy::Sequential)
            .with_target_node("llm_node")
            .with_target_param("temperature")
            .with_variant(VariantResult::new("v1"))
            .with_variant(VariantResult::new("v2"))
            .with_best_variant(VariantResult::new("best").with_score(0.90))
            .with_improvement_delta(0.15)
            .with_duration_ms(120000)
            .with_termination_reason(TerminationReason::ConvergenceThreshold(0.01));

        let summary = trace.summary();
        assert!(summary.contains("opt_001"));
        assert!(summary.contains("llm_node"));
        assert!(summary.contains("temperature"));
        assert!(summary.contains("2 variants"));
        assert!(summary.contains("improved"));
        assert!(summary.contains("converged"));
    }

    #[test]
    fn test_optimization_trace_summary_no_improvement() {
        let trace = OptimizationTrace::new("opt_001")
            .with_target_node("node")
            .with_target_param("param")
            .with_termination_reason(TerminationReason::MaxIterations(50));

        let summary = trace.summary();
        assert!(summary.contains("no improvement"));
        assert!(summary.contains("max iterations"));
    }

    #[test]
    fn test_optimization_trace_to_json() {
        let trace = OptimizationTrace::new("opt_001")
            .with_target_node("node")
            .with_target_param("param");

        let json = trace.to_json().unwrap();
        assert!(json.contains("opt_001"));
        assert!(json.contains("node"));
        assert!(json.contains("param"));
    }

    #[test]
    fn test_optimization_trace_serialization() {
        let trace = OptimizationTrace::new("opt_001")
            .with_strategy(OptimizationStrategy::Joint)
            .with_target_node("llm_node")
            .with_target_param("temperature")
            .with_improvement_delta(0.1)
            .with_termination_reason(TerminationReason::MaxIterations(100));

        let json = serde_json::to_string(&trace).unwrap();
        let restored: OptimizationTrace = serde_json::from_str(&json).unwrap();

        assert_eq!(trace.optimization_id, restored.optimization_id);
        assert_eq!(trace.target_node, restored.target_node);
        assert_eq!(trace.improvement_delta, restored.improvement_delta);
    }

    // ========================================================================
    // VariantResult Tests
    // ========================================================================

    #[test]
    fn test_variant_result_new() {
        let variant = VariantResult::new("var_001");
        assert_eq!(variant.variant_id, "var_001");
        assert!(variant.tested_at.is_some());
        assert!(variant.config.is_none());
        assert!(variant.execution_trace_id.is_empty());
        assert_eq!(variant.score, 0.0);
        assert!(variant.metrics.is_empty());
        assert_eq!(variant.evaluation_duration_ms, 0);
    }

    #[test]
    fn test_variant_result_default() {
        let variant = VariantResult::default();
        assert!(variant.variant_id.is_empty());
        assert!(variant.tested_at.is_none());
    }

    #[test]
    fn test_variant_result_with_config() {
        let config =
            NodeConfig::new("llm_node", "llm.chat").with_config(json!({"temperature": 0.8}));
        let variant = VariantResult::new("var_001").with_config(config);

        assert!(variant.config.is_some());
        let cfg = variant.config.unwrap();
        assert_eq!(cfg.name, "llm_node");
    }

    #[test]
    fn test_variant_result_with_execution_trace_id() {
        let variant = VariantResult::new("var_001").with_execution_trace_id("exec_abc123");

        assert_eq!(variant.execution_trace_id, "exec_abc123");
    }

    #[test]
    fn test_variant_result_with_score() {
        let variant = VariantResult::new("var_001").with_score(0.85);

        assert_eq!(variant.score, 0.85);
    }

    #[test]
    fn test_variant_result_with_metric() {
        let variant = VariantResult::new("var_001")
            .with_metric("latency_ms", 450.0)
            .with_metric("tokens_used", 1500.0);

        assert_eq!(variant.metrics.len(), 2);
        assert_eq!(variant.get_metric("latency_ms"), Some(450.0));
        assert_eq!(variant.get_metric("tokens_used"), Some(1500.0));
    }

    #[test]
    fn test_variant_result_with_metrics() {
        let mut metrics = HashMap::new();
        metrics.insert("a".to_string(), 1.0);
        metrics.insert("b".to_string(), 2.0);

        let variant = VariantResult::new("var_001").with_metrics(metrics);

        assert_eq!(variant.metrics.len(), 2);
        assert_eq!(variant.get_metric("a"), Some(1.0));
    }

    #[test]
    fn test_variant_result_with_evaluation_duration_ms() {
        let variant = VariantResult::new("var_001").with_evaluation_duration_ms(5000);

        assert_eq!(variant.evaluation_duration_ms, 5000);
    }

    #[test]
    fn test_variant_result_get_metric_nonexistent() {
        let variant = VariantResult::new("var_001");
        assert!(variant.get_metric("nonexistent").is_none());
    }

    #[test]
    fn test_variant_result_config_hash() {
        let variant_no_config = VariantResult::new("var_001");
        assert!(variant_no_config.config_hash().is_none());

        let config = NodeConfig::new("node", "type").with_config(json!({"key": "value"}));
        let variant_with_config = VariantResult::new("var_002").with_config(config);
        assert!(variant_with_config.config_hash().is_some());
    }

    #[test]
    fn test_variant_result_config_version() {
        let variant_no_config = VariantResult::new("var_001");
        assert!(variant_no_config.config_version().is_none());

        let config = NodeConfig::new("node", "type");
        let variant_with_config = VariantResult::new("var_002").with_config(config);
        // Default version is 1
        assert_eq!(variant_with_config.config_version(), Some(1));
    }

    #[test]
    fn test_variant_result_serialization() {
        let variant = VariantResult::new("var_001")
            .with_score(0.85)
            .with_metric("latency", 100.0);

        let json = serde_json::to_string(&variant).unwrap();
        let restored: VariantResult = serde_json::from_str(&json).unwrap();

        assert_eq!(variant.variant_id, restored.variant_id);
        assert_eq!(variant.score, restored.score);
        assert_eq!(variant.metrics.len(), restored.metrics.len());
    }

    // ========================================================================
    // TerminationReason Tests
    // ========================================================================

    #[test]
    fn test_termination_reason_default() {
        let reason = TerminationReason::default();
        assert_eq!(reason, TerminationReason::Unknown);
    }

    #[test]
    fn test_termination_reason_description_max_iterations() {
        let reason = TerminationReason::MaxIterations(100);
        let desc = reason.description();
        assert!(desc.contains("max iterations"));
        assert!(desc.contains("100"));
    }

    #[test]
    fn test_termination_reason_description_convergence() {
        let reason = TerminationReason::ConvergenceThreshold(0.01);
        let desc = reason.description();
        assert!(desc.contains("converged"));
        assert!(desc.contains("0.01"));
    }

    #[test]
    fn test_termination_reason_description_time_limit() {
        let reason = TerminationReason::TimeLimit(60000);
        let desc = reason.description();
        assert!(desc.contains("time limit"));
        assert!(desc.contains("60.0"));
    }

    #[test]
    fn test_termination_reason_description_no_improvement() {
        let reason = TerminationReason::NoImprovement { iterations: 10 };
        let desc = reason.description();
        assert!(desc.contains("no improvement"));
        assert!(desc.contains("10"));
    }

    #[test]
    fn test_termination_reason_description_user_stopped() {
        let reason = TerminationReason::UserStopped;
        assert_eq!(reason.description(), "user stopped");
    }

    #[test]
    fn test_termination_reason_description_error() {
        let reason = TerminationReason::Error("Something went wrong".to_string());
        let desc = reason.description();
        assert!(desc.contains("error"));
        assert!(desc.contains("Something went wrong"));
    }

    #[test]
    fn test_termination_reason_description_unknown() {
        let reason = TerminationReason::Unknown;
        assert_eq!(reason.description(), "unknown");
    }

    #[test]
    fn test_termination_reason_is_success() {
        assert!(TerminationReason::MaxIterations(100).is_success());
        assert!(TerminationReason::ConvergenceThreshold(0.01).is_success());
        assert!(TerminationReason::TimeLimit(60000).is_success());
        assert!(TerminationReason::NoImprovement { iterations: 10 }.is_success());
        assert!(TerminationReason::UserStopped.is_success());

        assert!(!TerminationReason::Error("err".to_string()).is_success());
        assert!(!TerminationReason::Unknown.is_success());
    }

    #[test]
    fn test_termination_reason_converged() {
        assert!(!TerminationReason::MaxIterations(100).converged());
        assert!(TerminationReason::ConvergenceThreshold(0.01).converged());
        assert!(!TerminationReason::TimeLimit(60000).converged());
        assert!(!TerminationReason::NoImprovement { iterations: 10 }.converged());
        assert!(!TerminationReason::UserStopped.converged());
        assert!(!TerminationReason::Error("err".to_string()).converged());
        assert!(!TerminationReason::Unknown.converged());
    }

    #[test]
    fn test_termination_reason_serialization() {
        let reasons = vec![
            TerminationReason::MaxIterations(50),
            TerminationReason::ConvergenceThreshold(0.001),
            TerminationReason::TimeLimit(30000),
            TerminationReason::NoImprovement { iterations: 5 },
            TerminationReason::UserStopped,
            TerminationReason::Error("test error".to_string()),
            TerminationReason::Unknown,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            let restored: TerminationReason = serde_json::from_str(&json).unwrap();
            assert_eq!(reason, restored);
        }
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_full_optimization_trace() {
        // Simulate a complete optimization run
        let variant1 = VariantResult::new("var_001")
            .with_config(
                NodeConfig::new("llm_node", "llm.chat").with_config(json!({"temperature": 0.5})),
            )
            .with_execution_trace_id("exec_001")
            .with_score(0.75)
            .with_metric("latency_ms", 500.0)
            .with_metric("tokens", 1000.0)
            .with_evaluation_duration_ms(30000);

        let variant2 = VariantResult::new("var_002")
            .with_config(
                NodeConfig::new("llm_node", "llm.chat").with_config(json!({"temperature": 0.7})),
            )
            .with_execution_trace_id("exec_002")
            .with_score(0.82)
            .with_metric("latency_ms", 480.0)
            .with_metric("tokens", 950.0)
            .with_evaluation_duration_ms(28000);

        let variant3 = VariantResult::new("var_003")
            .with_config(
                NodeConfig::new("llm_node", "llm.chat").with_config(json!({"temperature": 0.8})),
            )
            .with_execution_trace_id("exec_003")
            .with_score(0.90)
            .with_metric("latency_ms", 520.0)
            .with_metric("tokens", 1100.0)
            .with_evaluation_duration_ms(32000);

        let trace = OptimizationTrace::new("opt_full_test")
            .with_strategy(OptimizationStrategy::Sequential)
            .with_target_node("llm_node")
            .with_target_param("temperature")
            .with_initial_score(0.70)
            .with_variant(variant1)
            .with_variant(variant2)
            .with_variant(variant3.clone())
            .with_best_variant(variant3)
            .with_termination_reason(TerminationReason::ConvergenceThreshold(0.01))
            .with_duration_ms(120000)
            .with_improvement_delta(0.20)
            .with_metadata("experiment", json!("temp_tuning"))
            .complete();

        // Verify the trace
        assert_eq!(trace.optimization_id, "opt_full_test");
        assert!(trace.strategy.is_some());
        assert_eq!(trace.target_node, "llm_node");
        assert_eq!(trace.target_param, "temperature");
        assert_eq!(trace.variant_count(), 3);
        assert!(trace.found_improvement());
        assert_eq!(trace.best_score(), Some(0.90));
        assert!(trace.termination_reason.converged());
        assert_eq!(trace.duration_seconds(), 120.0);
        assert!(trace.started_at.is_some());
        assert!(trace.ended_at.is_some());

        // Verify JSON roundtrip
        let json = trace.to_json().unwrap();
        let restored: OptimizationTrace = serde_json::from_str(&json).unwrap();
        assert_eq!(trace.optimization_id, restored.optimization_id);
        assert_eq!(trace.variant_count(), restored.variant_count());
    }

    #[test]
    fn test_optimization_without_improvement() {
        let trace = OptimizationTrace::new("opt_no_improve")
            .with_strategy(OptimizationStrategy::Joint)
            .with_target_node("prompt_node")
            .with_target_param("system_prompt")
            .with_initial_score(0.85)
            .with_variant(VariantResult::new("v1").with_score(0.80))
            .with_variant(VariantResult::new("v2").with_score(0.82))
            .with_variant(VariantResult::new("v3").with_score(0.78))
            .with_termination_reason(TerminationReason::NoImprovement { iterations: 3 })
            .with_duration_ms(60000)
            .with_improvement_delta(0.0)
            .complete();

        assert!(!trace.found_improvement());
        assert!(trace.best_variant.is_none());
        assert!(!trace.termination_reason.converged());
        assert!(trace.termination_reason.is_success());

        let summary = trace.summary();
        assert!(summary.contains("no improvement"));
    }

    #[test]
    fn test_builder_pattern_chaining() {
        // Verify all builder methods can be chained
        let trace = OptimizationTrace::new("opt_chain")
            .with_strategy(OptimizationStrategy::Joint)
            .with_target_node("node")
            .with_target_param("param")
            .with_initial_score(0.5)
            .with_variant(VariantResult::new("v1"))
            .with_best_variant(VariantResult::new("best"))
            .with_termination_reason(TerminationReason::MaxIterations(10))
            .with_duration_ms(1000)
            .with_improvement_delta(0.1)
            .with_metadata("k", json!("v"))
            .complete();

        assert_eq!(trace.optimization_id, "opt_chain");
        assert!(trace.strategy.is_some());
        assert!(!trace.target_node.is_empty());
        assert!(!trace.target_param.is_empty());
        assert!(trace.initial_score.is_some());
        assert!(!trace.variants_tested.is_empty());
        assert!(trace.best_variant.is_some());
        assert!(trace.ended_at.is_some());
    }
}
