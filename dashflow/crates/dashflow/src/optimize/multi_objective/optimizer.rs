// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Multi-objective optimizer implementation.

use crate::optimize::multi_objective::objectives::{Objective, ObjectiveType, ObjectiveValue};
use crate::optimize::multi_objective::pareto::{ParetoFrontier, ParetoSolution};
use std::sync::Arc;
use thiserror::Error;

// Note: A previous design included a quality_metric field with type
// Arc<dyn Fn(&S, &S) -> f64 + Send + Sync>, but this was removed because
// it couldn't work without access to model predictions. Use Candidate::with_eval_fn()
// instead for quality evaluation.

/// Errors specific to multi-objective optimization.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum MultiObjectiveError {
    /// No optimization objectives were defined.
    ///
    /// At least one objective (quality, cost, latency, etc.) must be specified.
    #[error("No objectives defined")]
    NoObjectives,

    /// The optimization process failed with the given reason.
    #[error("Optimization failed: {0}")]
    OptimizationFailed(String),
}

/// Configuration for multi-objective optimization.
#[derive(Debug, Clone)]
pub struct MultiObjectiveConfig {
    /// Whether to evaluate on validation set
    pub use_valset: bool,

    /// Number of samples to use for cost/latency estimation (None = use all)
    pub estimation_sample_size: Option<usize>,

    /// Cost per evaluation (in dollars). Default: $0.001 per API call estimate.
    /// Set based on your LLM pricing (e.g., GPT-4: ~$0.03/1k tokens, Claude: ~$0.008/1k tokens).
    pub cost_per_evaluation: f64,

    /// Estimated latency per evaluation (in milliseconds). Default: 500ms.
    /// Set based on typical LLM response times for your model/provider.
    pub latency_per_evaluation_ms: f64,

    /// Estimated tokens per evaluation. Default: 1000 tokens.
    /// Set based on your typical prompt + completion token count.
    pub tokens_per_evaluation: u64,
}

impl Default for MultiObjectiveConfig {
    fn default() -> Self {
        Self {
            use_valset: true,
            estimation_sample_size: Some(10), // Default to 10 samples for estimation
            cost_per_evaluation: 0.001,       // $0.001 per evaluation (conservative estimate)
            latency_per_evaluation_ms: 500.0, // 500ms per evaluation
            tokens_per_evaluation: 1000,      // 1000 tokens per evaluation
        }
    }
}

/// A candidate solution to be evaluated.
///
/// This is generic over the module type M and state type S,
/// allowing flexibility in what gets optimized.
pub struct Candidate<M, S> {
    /// The module/model/configuration to evaluate
    pub module: M,

    /// Unique identifier for this candidate
    pub id: String,

    /// Optional evaluation function (overrides default quality metric)
    #[allow(clippy::type_complexity)] // Custom evaluator: module + samples → quality score
    pub eval_fn: Option<Arc<dyn Fn(&M, &[S]) -> f64 + Send + Sync>>,
}

impl<M: std::fmt::Debug, S> std::fmt::Debug for Candidate<M, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Candidate")
            .field("module", &self.module)
            .field("id", &self.id)
            .field("eval_fn", &self.eval_fn.as_ref().map(|_| "<fn>"))
            .finish()
    }
}

impl<M, S> Candidate<M, S> {
    /// Create a new candidate with the given module and identifier.
    pub fn new(module: M, id: impl Into<String>) -> Self {
        Self {
            module,
            id: id.into(),
            eval_fn: None,
        }
    }

    /// Set a custom evaluation function for this candidate.
    #[must_use]
    pub fn with_eval_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&M, &[S]) -> f64 + Send + Sync + 'static,
    {
        self.eval_fn = Some(Arc::new(f));
        self
    }
}

/// Multi-objective optimizer that finds Pareto-optimal solutions.
///
/// This optimizer evaluates candidates on multiple objectives (quality, cost, latency, etc.)
/// and builds a Pareto frontier of non-dominated solutions.
///
/// # Type Parameters
/// * `S` - State type (e.g., your graph state struct)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::multi_objective::*;
///
/// let optimizer = MultiObjectiveOptimizer::new()
///     .add_objective(Objective::new(ObjectiveType::Quality, 0.7))
///     .add_objective(Objective::new(ObjectiveType::Cost, 0.3));
///
/// // Create candidates with eval_fn for quality evaluation
/// let candidate = Candidate::new(model, "model-1")
///     .with_eval_fn(|model, eval_set| {
///         // Evaluate model quality on eval_set
///         // Return score in [0.0, 1.0]
///         0.95
///     });
///
/// // Evaluate multiple candidates and build frontier
/// let frontier = optimizer.evaluate_candidates(candidates, &trainset, Some(&valset));
/// ```
pub struct MultiObjectiveOptimizer<S> {
    /// Objectives to optimize with their weights
    objectives: Vec<Objective>,

    /// Configuration
    config: MultiObjectiveConfig,

    /// Phantom data for state type (quality evaluation is done via Candidate::eval_fn)
    _phantom: std::marker::PhantomData<S>,
}

impl<S> MultiObjectiveOptimizer<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Create a new multi-objective optimizer.
    pub fn new() -> Self {
        Self {
            objectives: Vec::new(),
            config: MultiObjectiveConfig::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add an objective to optimize.
    #[must_use]
    pub fn add_objective(mut self, objective: Objective) -> Self {
        self.objectives.push(objective);
        self
    }

    /// Set the configuration.
    #[must_use]
    pub fn with_config(mut self, config: MultiObjectiveConfig) -> Self {
        self.config = config;
        self
    }

    /// Validate the optimizer configuration.
    fn validate(&self) -> Result<(), MultiObjectiveError> {
        if self.objectives.is_empty() {
            return Err(MultiObjectiveError::NoObjectives);
        }

        Ok(())
    }

    /// Evaluate a set of candidates and return a Pareto frontier.
    ///
    /// # Arguments
    /// * `candidates` - Vector of candidates to evaluate
    /// * `trainset` - Training examples (used if no valset)
    /// * `valset` - Optional validation examples
    ///
    /// # Returns
    /// A Pareto frontier containing non-dominated solutions with their objective values.
    pub fn evaluate_candidates<M>(
        &self,
        candidates: Vec<Candidate<M, S>>,
        trainset: &[S],
        valset: Option<&[S]>,
    ) -> Result<ParetoFrontier, MultiObjectiveError>
    where
        M: Send + Sync,
    {
        self.validate()?;

        let mut frontier = ParetoFrontier::new();

        tracing::info!(
            "Evaluating {} candidates on {} objectives",
            candidates.len(),
            self.objectives.len()
        );

        for candidate in candidates {
            match self.evaluate_solution(&candidate, trainset, valset) {
                Ok(solution) => {
                    tracing::info!("Solution '{}' evaluated, adding to frontier", candidate.id);
                    frontier.add_solution(solution);
                }
                Err(e) => {
                    tracing::warn!("Failed to evaluate solution '{}': {}", candidate.id, e);
                }
            }
        }

        tracing::info!("Evaluation complete. Frontier size: {}", frontier.len());

        Ok(frontier)
    }

    /// Evaluate a single solution on all objectives.
    fn evaluate_solution<M>(
        &self,
        candidate: &Candidate<M, S>,
        trainset: &[S],
        valset: Option<&[S]>,
    ) -> Result<ParetoSolution, MultiObjectiveError>
    where
        M: Send + Sync,
    {
        let eval_set = if self.config.use_valset {
            valset.unwrap_or(trainset)
        } else {
            trainset
        };

        let mut solution = ParetoSolution::new(&candidate.id);

        // Evaluate each objective
        for objective in &self.objectives {
            let value = match objective.objective_type {
                ObjectiveType::Quality => {
                    // Use candidate's custom eval function for quality evaluation.
                    // Quality evaluation requires running the model on the eval set,
                    // so it must be provided via Candidate::with_eval_fn().
                    if let Some(ref eval_fn) = candidate.eval_fn {
                        eval_fn(&candidate.module, eval_set)
                    } else {
                        tracing::warn!(
                            candidate_id = %candidate.id,
                            "No eval_fn provided for candidate - quality score will be 0.0. \
                             Use Candidate::with_eval_fn() to provide a quality evaluation function."
                        );
                        0.0
                    }
                }
                ObjectiveType::Cost => {
                    // Estimate total cost based on evaluation set size and configured cost per evaluation.
                    // For more accurate results, configure cost_per_evaluation based on your LLM pricing.
                    let num_evals = self
                        .config
                        .estimation_sample_size
                        .unwrap_or(eval_set.len())
                        .min(eval_set.len());
                    num_evals as f64 * self.config.cost_per_evaluation
                }
                ObjectiveType::Latency => {
                    // Estimate total latency based on evaluation set size and configured latency per evaluation.
                    // For more accurate results, configure latency_per_evaluation_ms based on your model.
                    let num_evals = self
                        .config
                        .estimation_sample_size
                        .unwrap_or(eval_set.len())
                        .min(eval_set.len());
                    num_evals as f64 * self.config.latency_per_evaluation_ms
                }
                ObjectiveType::TokenUsage => {
                    // Estimate total token usage based on evaluation set size and configured tokens per evaluation.
                    // For more accurate results, configure tokens_per_evaluation based on your prompt/response sizes.
                    let num_evals = self
                        .config
                        .estimation_sample_size
                        .unwrap_or(eval_set.len())
                        .min(eval_set.len());
                    (num_evals as u64 * self.config.tokens_per_evaluation) as f64
                }
            };

            solution =
                solution.with_objective(ObjectiveValue::new(objective.objective_type, value));
        }

        Ok(solution)
    }
}

impl<S> Default for MultiObjectiveOptimizer<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_objective_config_default() {
        let config = MultiObjectiveConfig::default();
        assert!(config.use_valset);
        assert_eq!(config.estimation_sample_size, Some(10));
        assert_eq!(config.cost_per_evaluation, 0.001);
        assert_eq!(config.latency_per_evaluation_ms, 500.0);
        assert_eq!(config.tokens_per_evaluation, 1000);
    }

    #[test]
    fn test_cost_estimation() {
        // Default config: 10 samples × $0.001/eval = $0.01 total cost
        let optimizer = MultiObjectiveOptimizer::<String>::new()
            .add_objective(Objective::new(ObjectiveType::Cost, 1.0));

        let candidate: Candidate<&str, String> = Candidate::new("test_module", "candidate_1");
        let trainset: Vec<String> = (0..100).map(|i| format!("sample_{}", i)).collect();

        let frontier = optimizer
            .evaluate_candidates(vec![candidate], &trainset, None)
            .unwrap();
        assert_eq!(frontier.len(), 1);

        // 10 samples (estimation_sample_size) × $0.001 = $0.01
        let solution = &frontier.solutions[0];
        let cost_value = solution.get_value(ObjectiveType::Cost).unwrap();
        assert!(
            (cost_value - 0.01).abs() < 0.0001,
            "Expected ~$0.01, got ${}",
            cost_value
        );
    }

    #[test]
    fn test_latency_estimation() {
        let config = MultiObjectiveConfig {
            estimation_sample_size: Some(5),
            latency_per_evaluation_ms: 200.0,
            ..Default::default()
        };
        let optimizer = MultiObjectiveOptimizer::<String>::new()
            .with_config(config)
            .add_objective(Objective::new(ObjectiveType::Latency, 1.0));

        let candidate: Candidate<&str, String> = Candidate::new("test_module", "candidate_1");
        let trainset: Vec<String> = (0..100).map(|i| format!("sample_{}", i)).collect();

        let frontier = optimizer
            .evaluate_candidates(vec![candidate], &trainset, None)
            .unwrap();
        let solution = &frontier.solutions[0];
        let latency_value = solution.get_value(ObjectiveType::Latency).unwrap();

        // 5 samples × 200ms = 1000ms
        assert!(
            (latency_value - 1000.0).abs() < 0.1,
            "Expected ~1000ms, got {}ms",
            latency_value
        );
    }

    #[test]
    fn test_token_estimation() {
        let config = MultiObjectiveConfig {
            estimation_sample_size: Some(8),
            tokens_per_evaluation: 500,
            ..Default::default()
        };
        let optimizer = MultiObjectiveOptimizer::<String>::new()
            .with_config(config)
            .add_objective(Objective::new(ObjectiveType::TokenUsage, 1.0));

        let candidate: Candidate<&str, String> = Candidate::new("test_module", "candidate_1");
        let trainset: Vec<String> = (0..100).map(|i| format!("sample_{}", i)).collect();

        let frontier = optimizer
            .evaluate_candidates(vec![candidate], &trainset, None)
            .unwrap();
        let solution = &frontier.solutions[0];
        let token_value = solution.get_value(ObjectiveType::TokenUsage).unwrap();

        // 8 samples × 500 tokens = 4000 tokens
        assert!(
            (token_value - 4000.0).abs() < 0.1,
            "Expected ~4000 tokens, got {}",
            token_value
        );
    }

    #[test]
    fn test_optimizer_new() {
        let optimizer = MultiObjectiveOptimizer::<String>::new();
        assert_eq!(optimizer.objectives.len(), 0);
    }

    #[test]
    fn test_optimizer_add_objective() {
        let optimizer = MultiObjectiveOptimizer::<String>::new()
            .add_objective(Objective::new(ObjectiveType::Quality, 0.7))
            .add_objective(Objective::new(ObjectiveType::Cost, 0.3));

        assert_eq!(optimizer.objectives.len(), 2);
    }

    #[test]
    fn test_optimizer_validate_no_objectives() {
        let optimizer = MultiObjectiveOptimizer::<String>::new();
        let result = optimizer.validate();
        assert!(matches!(result, Err(MultiObjectiveError::NoObjectives)));
    }

    #[test]
    fn test_candidate_new() {
        let candidate: Candidate<&str, String> = Candidate::new("test_module", "candidate_1");
        assert_eq!(candidate.id, "candidate_1");
        assert!(candidate.eval_fn.is_none());
    }

    #[test]
    fn test_candidate_with_eval_fn() {
        let candidate: Candidate<&str, String> = Candidate::new("test_module", "candidate_1")
            .with_eval_fn(|_module, _data: &[String]| 0.95);

        assert!(candidate.eval_fn.is_some());
    }
}
