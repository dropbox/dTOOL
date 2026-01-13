//! @dashflow-module
//! @name optimizers
//! @category optimize
//! @status stable
//!
//! # DashOptimize: Prompt Optimization Algorithms
//!
//! This module provides 17 optimizer algorithms for improving LLM prompts using training data.
//! All optimizers are ported from DSPy with citations to original papers.
//!
//! ## Quick Start: Which Optimizer Should I Use?
//!
//! | Scenario | Optimizer | Why |
//! |----------|-----------|-----|
//! | Quick prototyping (~10 examples) | [`BootstrapFewShot`] | Fast, minimal data |
//! | Production optimization (50+ examples) | [`MIPROv2`] | Best benchmarked |
//! | Model finetuning available | `GRPO` | RL weight updates |
//! | Self-improving agents | [`SIMBA`] | Introspective |
//! | Instruction-only | [`COPRO`] | No few-shot |
//! | Gradient-free prompt search | [`AutoPrompt`] | Discrete optimization |
//! | Agent feedback-based | [`AvatarOptimizer`] | Positive/negative pattern analysis |
//! | Explicit rule extraction | [`InferRules`] | Human-readable rules |
//!
//! ## Tier 1: Recommended Defaults
//!
//! - **[`MIPROv2`]**: Best benchmarked optimizer. Outperforms baselines on 5/7 tasks,
//!   up to 13% accuracy improvement. Use for production with 50+ examples.
//!   *Reference: [arxiv:2406.11695](https://arxiv.org/abs/2406.11695)*
//!
//! - **[`BootstrapFewShot`]**: Quick start optimizer. Works with ~10 examples.
//!   Foundation of DSPy optimization.
//!   *Reference: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714)*
//!
//! - **`GRPO`**: Reinforcement learning optimizer for model finetuning.
//!   Only use when you can update model weights.
//!   *Reference: [arxiv:2402.03300](https://arxiv.org/abs/2402.03300)*
//!
//! ## Tier 2: Specialized
//!
//! - **[`SIMBA`]**: Self-reflective optimizer using trajectory analysis
//! - **[`COPRO`]**: Instruction-only optimization (no few-shot)
//! - **[`COPROv2`]**: COPRO with confidence-based scoring
//! - **`BootstrapFinetune`**: Model distillation (requires finetunable model)
//! - **[`AutoPrompt`]**: Gradient-free discrete prompt search
//!
//! ## Tier 3: Niche
//!
//! - [`RandomSearch`], [`GEPA`], [`Ensemble`], [`KNNFewShot`],
//!   [`LabeledFewShot`], [`BetterTogether`], [`BootstrapOptuna`],
//!   [`AvatarOptimizer`], [`InferRules`]
//!
//! ## All Optimizers Reference
//!
//! | Optimizer | Type | Min Examples | Citation |
//! |-----------|------|--------------|----------|
//! | [`MIPROv2`] | Instruction + Few-shot | 2 | arxiv:2406.11695 |
//! | [`BootstrapFewShot`] | Few-shot | 10 | arxiv:2310.03714 |
//! | `GRPO` | RL Finetuning | 10 | arxiv:2402.03300 |
//! | [`SIMBA`] | Self-reflective | 20 | DSPy |
//! | [`COPRO`] | Instruction | 10 | arxiv:2310.03714 |
//! | [`COPROv2`] | Instruction | 10 | arxiv:2310.03714 |
//! | [`GEPA`] | Genetic | 10 | arxiv:2507.19457 |
//! | [`AutoPrompt`] | Discrete search | 10 | arxiv:2010.15980 |
//! | `BootstrapFinetune` | Distillation | 50 | arxiv:2310.03714 |
//! | [`RandomSearch`] | Exploration | 50 | arxiv:2310.03714 |
//! | [`Ensemble`] | Combination | 0 | Standard |
//! | [`KNNFewShot`] | Example selection | 20 | arxiv:2310.03714 |
//! | [`LabeledFewShot`] | Direct | 5 | arxiv:2310.03714 |
//! | [`BetterTogether`] | Meta | 20 | DSPy |
//! | [`BootstrapOptuna`] | Hyperparameter | 50 | DSPy + Optuna |
//! | [`AvatarOptimizer`] | Feedback-based | 10 | DSPy |
//! | [`InferRules`] | Rule induction | 20 | DSPy |
//!
//! ## Data Requirements
//!
//! | Data Size | Recommended Optimizer |
//! |-----------|----------------------|
//! | 0 examples | Cannot optimize (need at least 2) |
//! | 2-10 examples | [`BootstrapFewShot`] |
//! | 10-50 examples | [`BootstrapFewShot`] → [`MIPROv2`] |
//! | 50-200 examples | [`MIPROv2`] (best results) |
//! | 200+ examples | [`MIPROv2`] with more trials, or `GRPO` if finetuning |
//!
//! ## Decision Tree
//!
//! ```text
//! Can you finetune the model?
//!   ├─ Yes → GRPO
//!   └─ No → How much training data?
//!             ├─ <10 examples → BootstrapFewShot
//!             ├─ 10-50 examples → BootstrapFewShot or MIPROv2
//!             └─ 50+ examples → MIPROv2
//! ```
//!
//! See [`OPTIMIZER_SELECTION.md`](./OPTIMIZER_SELECTION.md) for the complete selection guide.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Configuration validation error with helpful message.
///
/// Used by optimizer configs to report validation failures with actionable suggestions.
#[derive(Debug, Clone, Error)]
#[error("{field}: {message}{}", suggestion.as_ref().map(|s| format!(" ({})", s)).unwrap_or_default())]
pub struct ConfigValidationError {
    /// The field that failed validation
    pub field: String,
    /// Human-readable error message
    pub message: String,
    /// Optional suggestion for how to fix
    pub suggestion: Option<String>,
}

impl ConfigValidationError {
    /// Create a new validation error without a suggestion.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// Create a new validation error with a suggestion.
    pub fn with_suggestion(
        field: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            suggestion: Some(suggestion.into()),
        }
    }
}

pub mod autoprompt;
pub mod avatar;
pub mod better_together;

// Shared infrastructure
pub mod bootstrap;
#[cfg(feature = "dashstream")]
pub mod bootstrap_finetune;
pub mod bootstrap_optuna;
pub mod copro;
pub mod copro_v2;
pub mod ensemble;
pub mod eval_utils;
pub mod gepa;
#[cfg(feature = "dashstream")]
pub mod grpo;
pub mod infer_rules;
pub mod knn_fewshot;
pub mod labeled_fewshot;
pub mod mipro_v2;
pub mod random_search;
pub mod registry;
pub mod simba;
pub mod traits;
pub mod types;

pub use autoprompt::{AutoPrompt, AutoPromptBuilder, MetricFn as AutoPromptMetricFn};
pub use avatar::{
    AvatarConfig, AvatarOptimizer, AvatarOptimizerBuilder, MetricFn as AvatarMetricFn,
};
pub use better_together::{BetterTogether, CompositionStrategy, NodeOptimizer, PipelineStage};
pub use bootstrap::BootstrapFewShot;
#[cfg(feature = "dashstream")]
pub use bootstrap_finetune::{
    BootstrapFinetune, BootstrapFinetuneError, MetricFn as FinetuneMetricFn,
};
pub use bootstrap_optuna::BootstrapOptuna;
pub use copro::{COPROBuilder, COPRO};
pub use copro_v2::{COPROv2, COPROv2Builder, MetricFn as COPROv2MetricFn};
pub use ensemble::{Ensemble, EnsembleBuilder, ReduceFn};
pub use gepa::{
    metric_to_gepa, GEPAConfig, GEPAMetricFn, GEPAResult, ScoreWithFeedback, SelectionStrategy,
    GEPA,
};
#[cfg(feature = "dashstream")]
pub use grpo::{GRPOConfig, GRPOError, GRPOMetricFn, GRPO};
pub use infer_rules::{
    InferRules, InferRulesBuilder, InferRulesConfig, MetricFn as InferRulesMetricFn, RuleCandidate,
};
pub use knn_fewshot::KNNFewShot;
pub use labeled_fewshot::LabeledFewShot;
pub use mipro_v2::{AutoMode, MIPROv2, MIPROv2Builder, MetricFn};
pub use random_search::{CandidateProgram, RandomSearch};
// Note: TraceStep is deprecated - use ExecutionTrace and NodeExecution instead
#[allow(deprecated)]
pub use simba::{
    AppendADemo, AppendARule, SimbaOutput, SimbaStrategy, StrategyContext, TraceStep, SIMBA,
};

// Shared types - consolidated from individual optimizer modules
pub use types::{Candidate, CandidatePool, MetricFn as SharedMetricFn, MetricWithFeedbackFn};

// Shared evaluation utilities
pub use eval_utils::{
    average_score, evaluate_examples, min_max_normalize, percentile, rank_scores,
    softmax_normalize, std_dev, weighted_sample, weighted_sample_n,
};

// Optimizer traits for common interface
pub use traits::{DynOptimizer, OptimizationRun, OptimizerInfo, OptimizerTier, SignatureOptimizer};

/// Configuration for optimizers
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OptimizerConfig {
    /// Maximum number of few-shot examples to include
    pub max_few_shot_examples: usize,

    /// Maximum number of optimization iterations
    pub max_iterations: usize,

    /// Minimum improvement threshold to continue optimizing
    pub min_improvement: f64,

    /// Random seed for reproducibility
    pub random_seed: Option<u64>,

    /// Success threshold for example classification (M-889)
    ///
    /// Examples scoring above this threshold are considered "successful"
    /// and eligible for use as few-shot demonstrations.
    /// Default: 0.5 (50%)
    pub success_threshold: f64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            max_few_shot_examples: 4,
            max_iterations: 10,
            min_improvement: 0.01, // 1% improvement
            random_seed: None,
            success_threshold: 0.5, // M-889: 50% threshold for success classification
        }
    }
}

impl OptimizerConfig {
    /// Create a new optimizer configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of few-shot examples to use.
    #[must_use]
    pub fn with_max_few_shot_examples(mut self, max: usize) -> Self {
        self.max_few_shot_examples = max;
        self
    }

    /// Set the maximum number of optimization iterations.
    #[must_use]
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set the minimum improvement threshold to continue optimization.
    #[must_use]
    pub fn with_min_improvement(mut self, min: f64) -> Self {
        self.min_improvement = min;
        self
    }

    /// Set a fixed random seed for reproducibility.
    #[must_use]
    pub fn with_random_seed(mut self, seed: u64) -> Self {
        self.random_seed = Some(seed);
        self
    }

    /// Set the success threshold for example classification (M-889)
    ///
    /// Examples scoring above this threshold are considered "successful"
    /// and eligible for use as few-shot demonstrations.
    #[must_use]
    pub fn with_success_threshold(mut self, threshold: f64) -> Self {
        self.success_threshold = threshold;
        self
    }

    /// Validate the configuration.
    ///
    /// Returns a list of validation errors, or `Ok(())` if all values are valid.
    ///
    /// # Validation Rules
    ///
    /// - `max_iterations` must be > 0
    /// - `min_improvement` must be >= 0
    /// - `success_threshold` must be in range [0.0, 1.0]
    pub fn validate(&self) -> Result<(), Vec<ConfigValidationError>> {
        let mut errors = Vec::new();

        if self.max_iterations == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "max_iterations",
                "Maximum iterations must be greater than 0",
                "Set max_iterations to at least 1",
            ));
        }

        if self.min_improvement < 0.0 {
            errors.push(ConfigValidationError::with_suggestion(
                "min_improvement",
                format!(
                    "Minimum improvement {} must be non-negative",
                    self.min_improvement
                ),
                "Set min_improvement to 0.0 or higher",
            ));
        }

        // M-889: Validate success threshold
        if !(0.0..=1.0).contains(&self.success_threshold) {
            errors.push(ConfigValidationError::with_suggestion(
                "success_threshold",
                format!(
                    "Success threshold {} must be between 0.0 and 1.0",
                    self.success_threshold
                ),
                "Set success_threshold to a value in [0.0, 1.0]",
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Result of optimization
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OptimizationResult {
    /// Final score achieved
    pub final_score: f64,

    /// Initial score (before optimization)
    pub initial_score: f64,

    /// Number of iterations performed
    pub iterations: usize,

    /// Whether optimization converged
    pub converged: bool,

    /// Time spent optimizing (seconds)
    pub duration_secs: f64,
}

impl OptimizationResult {
    /// Create a new optimization result.
    #[must_use]
    pub fn new(
        initial_score: f64,
        final_score: f64,
        iterations: usize,
        converged: bool,
        duration_secs: f64,
    ) -> Self {
        Self {
            initial_score,
            final_score,
            iterations,
            converged,
            duration_secs,
        }
    }

    /// Calculate the absolute improvement (final - initial score).
    pub fn improvement(&self) -> f64 {
        self.final_score - self.initial_score
    }

    /// Calculate the improvement as a percentage of the initial score.
    pub fn improvement_percent(&self) -> f64 {
        if self.initial_score == 0.0 {
            return 0.0;
        }
        (self.improvement() / self.initial_score) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_config_validation_passes_for_defaults() {
        let config = OptimizerConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_optimizer_config_validation_fails_for_zero_iterations() {
        let config = OptimizerConfig {
            max_iterations: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "max_iterations"));
    }

    #[test]
    fn test_optimizer_config_validation_fails_for_negative_improvement() {
        let config = OptimizerConfig {
            min_improvement: -0.5,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "min_improvement"));
    }

    #[test]
    fn test_gepa_config_validation_passes_for_defaults() {
        let config = GEPAConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_gepa_config_validation_fails_for_zero_minibatch() {
        let config = GEPAConfig {
            reflection_minibatch_size: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.field == "reflection_minibatch_size"));
    }

    #[test]
    fn test_gepa_config_validation_fails_for_invalid_scores() {
        let config = GEPAConfig {
            failure_score: 1.5,
            perfect_score: 1.0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "failure_score"));
    }

    #[test]
    fn test_avatar_config_validation_passes_for_defaults() {
        let config = AvatarConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_avatar_config_validation_fails_for_invalid_bounds() {
        let config = AvatarConfig {
            optimize_bounds: (100, 5), // Lower > Upper
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "optimize_bounds"));
    }

    #[test]
    fn test_infer_rules_config_validation_passes_for_defaults() {
        let config = InferRulesConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_infer_rules_config_validation_fails_for_zero_candidates() {
        let config = InferRulesConfig {
            num_candidates: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "num_candidates"));
    }

    #[test]
    fn test_config_validation_error_display() {
        let error =
            ConfigValidationError::with_suggestion("test_field", "Test message", "Test suggestion");
        let display = format!("{}", error);
        assert!(display.contains("test_field"));
        assert!(display.contains("Test message"));
        assert!(display.contains("Test suggestion"));
    }
}
