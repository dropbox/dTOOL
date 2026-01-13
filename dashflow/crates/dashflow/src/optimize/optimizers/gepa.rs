//! GEPA: Genetic Evolutionary Prompt Algorithm
//!
//! GEPA is an evolutionary optimizer that uses reflection to evolve text components
//! (instructions, prompts) of complex systems. GEPA is proposed in the paper
//! "GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning"
//! (<https://arxiv.org/abs/2507.19457>).
//!
//! ## Algorithm Overview
//!
//! GEPA works by:
//! 1. Initializing a population with the seed candidate (current prompt)
//! 2. Iteratively:
//!    a. Selecting a parent candidate from the population
//!    b. Sampling a minibatch of training examples
//!    c. Running the candidate on the minibatch and collecting feedback
//!    d. Using an LLM to reflect on the feedback and generate a mutated instruction
//!    e. Evaluating the new candidate on validation data
//!    f. Adding the new candidate to the population
//! 3. Returning the best candidate based on validation scores
//!
//! ## Key Features
//!
//! - **LLM-based reflection**: Uses an LLM to analyze feedback and propose improvements
//! - **Evolutionary search**: Maintains a population of candidates, selects parents probabilistically
//! - **Metric with feedback**: Metrics can return optional textual feedback for reflection
//! - **Pareto frontier selection**: Can select from Pareto-optimal candidates (multi-objective)
//! - **Budget control**: Supports limits on total metric calls or full evaluations
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use dashflow::optimize::optimizers::GEPA;
//!
//! let gepa = GEPA::new()
//!     .with_max_metric_calls(100)
//!     .with_reflection_minibatch_size(3)
//!     .with_candidate_selection_strategy(SelectionStrategy::Pareto);
//!
//! let result = gepa.optimize(node, &trainset, &valset, &metric, reflection_model).await?;
//! ```
//!
//! ## Adapted from DashOpt
//!
//! Source: ~/dsp_rs/dashopt_teleprompt/src/gepa.rs (15KB)
//!
//! This implementation adapts the DashOpt GEPA algorithm for DashFlow's `Node<S>` architecture.
//!
//! ## References
//!
//! - **Paper**: "GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning"
//! - **Link**: <https://arxiv.org/abs/2507.19457>
//! - **Key Feature**: Genetic evolutionary prompt optimization with LLM reflection

use crate::core::language_models::ChatModel;
use crate::core::messages::HumanMessage;
use crate::node::Node;
use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::optimize::{MetricFn, Optimizable, OptimizationState};
use crate::state::GraphState;
use crate::Result;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Configuration for GEPA optimization
#[derive(Debug, Clone)]
pub struct GEPAConfig {
    /// Maximum number of full evaluations to perform
    pub max_full_evals: Option<usize>,

    /// Maximum number of metric calls to perform
    pub max_metric_calls: Option<usize>,

    /// Size of reflection minibatch
    pub reflection_minibatch_size: usize,

    /// Candidate selection strategy
    pub candidate_selection_strategy: SelectionStrategy,

    /// Whether to skip examples with perfect scores during reflection
    pub skip_perfect_score: bool,

    /// Whether to use merge-based optimization
    ///
    /// M-2016: Currently unused - retained for API stability and future multi-candidate merging support.
    /// When implemented, merge will combine successful instruction fragments from multiple candidates.
    #[allow(dead_code)]
    pub use_merge: bool,

    /// Maximum number of merge invocations
    ///
    /// M-2016: Currently unused - retained for API stability. Will be used when merge support is added.
    #[allow(dead_code)]
    pub max_merge_invocations: usize,

    /// Number of threads for evaluation
    ///
    /// M-2016: Currently unused - evaluation is single-threaded. Retained for API stability
    /// and future parallel evaluation support.
    #[allow(dead_code)]
    pub num_threads: Option<usize>,

    /// Score assigned to failed examples
    pub failure_score: f64,

    /// Maximum score achievable by the metric
    pub perfect_score: f64,

    /// Whether to track detailed statistics
    ///
    /// M-2016: Currently unused - GEPAResult always includes full statistics.
    /// Retained for API stability and potential future optimization to skip stats collection.
    #[allow(dead_code)]
    pub track_stats: bool,

    /// Random seed for reproducibility
    pub seed: u64,
}

impl Default for GEPAConfig {
    fn default() -> Self {
        Self {
            max_full_evals: None,
            max_metric_calls: Some(100),
            reflection_minibatch_size: 3,
            candidate_selection_strategy: SelectionStrategy::Pareto,
            skip_perfect_score: true,
            use_merge: true,
            max_merge_invocations: 5,
            num_threads: None,
            failure_score: 0.0,
            perfect_score: 1.0,
            track_stats: false,
            seed: 0,
        }
    }
}

impl GEPAConfig {
    /// Validate the configuration.
    ///
    /// Returns a list of validation errors, or `Ok(())` if all values are valid.
    ///
    /// # Validation Rules
    ///
    /// - `reflection_minibatch_size` must be > 0
    /// - `failure_score` must be <= `perfect_score`
    /// - `max_full_evals` if set must be > 0
    /// - `max_metric_calls` if set must be > 0
    pub fn validate(&self) -> std::result::Result<(), Vec<super::ConfigValidationError>> {
        use super::ConfigValidationError;
        let mut errors = Vec::new();

        if self.reflection_minibatch_size == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "reflection_minibatch_size",
                "Reflection minibatch size must be greater than 0",
                "Set reflection_minibatch_size to at least 1",
            ));
        }

        if self.failure_score > self.perfect_score {
            errors.push(ConfigValidationError::new(
                "failure_score",
                format!(
                    "Failure score ({}) cannot be greater than perfect score ({})",
                    self.failure_score, self.perfect_score
                ),
            ));
        }

        if let Some(max_evals) = self.max_full_evals {
            if max_evals == 0 {
                errors.push(ConfigValidationError::with_suggestion(
                    "max_full_evals",
                    "Max full evaluations must be greater than 0",
                    "Set max_full_evals to at least 1 or use None for unlimited",
                ));
            }
        }

        if let Some(max_calls) = self.max_metric_calls {
            if max_calls == 0 {
                errors.push(ConfigValidationError::with_suggestion(
                    "max_metric_calls",
                    "Max metric calls must be greater than 0",
                    "Set max_metric_calls to at least 1 or use None for unlimited",
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Create a new `GEPAConfig` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of full evaluations.
    ///
    /// Set to `None` for unlimited evaluations.
    #[must_use]
    pub fn with_max_full_evals(mut self, max_evals: Option<usize>) -> Self {
        self.max_full_evals = max_evals;
        self
    }

    /// Set the maximum number of metric calls.
    ///
    /// Set to `None` for unlimited calls.
    /// Default: 100.
    #[must_use]
    pub fn with_max_metric_calls(mut self, max_calls: Option<usize>) -> Self {
        self.max_metric_calls = max_calls;
        self
    }

    /// Set the reflection minibatch size.
    ///
    /// Must be > 0. Default: 3.
    #[must_use]
    pub fn with_reflection_minibatch_size(mut self, size: usize) -> Self {
        self.reflection_minibatch_size = size;
        self
    }

    /// Set the candidate selection strategy.
    ///
    /// - `Pareto`: Select from Pareto frontier (multi-objective)
    /// - `CurrentBest`: Always select the best candidate (greedy)
    ///
    /// Default: `Pareto`.
    #[must_use]
    pub fn with_candidate_selection_strategy(mut self, strategy: SelectionStrategy) -> Self {
        self.candidate_selection_strategy = strategy;
        self
    }

    /// Set whether to skip examples with perfect scores during reflection.
    ///
    /// Default: true.
    #[must_use]
    pub fn with_skip_perfect_score(mut self, skip: bool) -> Self {
        self.skip_perfect_score = skip;
        self
    }

    /// Set whether to use merge-based optimization.
    ///
    /// NOTE: Currently unused - retained for API stability.
    #[must_use]
    pub fn with_use_merge(mut self, use_merge: bool) -> Self {
        self.use_merge = use_merge;
        self
    }

    /// Set the maximum number of merge invocations.
    ///
    /// NOTE: Currently unused - retained for API stability.
    #[must_use]
    pub fn with_max_merge_invocations(mut self, max: usize) -> Self {
        self.max_merge_invocations = max;
        self
    }

    /// Set the number of threads for evaluation.
    ///
    /// NOTE: Currently unused - evaluation is single-threaded.
    #[must_use]
    pub fn with_num_threads(mut self, threads: Option<usize>) -> Self {
        self.num_threads = threads;
        self
    }

    /// Set the score assigned to failed examples.
    ///
    /// Default: 0.0.
    #[must_use]
    pub fn with_failure_score(mut self, score: f64) -> Self {
        self.failure_score = score;
        self
    }

    /// Set the maximum score achievable by the metric.
    ///
    /// Default: 1.0.
    #[must_use]
    pub fn with_perfect_score(mut self, score: f64) -> Self {
        self.perfect_score = score;
        self
    }

    /// Set whether to track detailed statistics.
    ///
    /// NOTE: Currently unused - statistics are always tracked.
    #[must_use]
    pub fn with_track_stats(mut self, track: bool) -> Self {
        self.track_stats = track;
        self
    }

    /// Set the random seed for reproducibility.
    ///
    /// Default: 0.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}

/// Candidate selection strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionStrategy {
    /// Select from Pareto frontier (multi-objective optimization)
    Pareto,
    /// Always select the current best candidate (greedy)
    CurrentBest,
}

/// Score with optional textual feedback
///
/// Metrics can return feedback text that GEPA will use for reflection
/// to improve the prompt. The feedback should describe what went wrong
/// or what could be improved.
#[derive(Debug, Clone)]
pub struct ScoreWithFeedback {
    /// Numeric score (0.0 to 1.0)
    pub score: f64,
    /// Optional textual feedback for reflection
    pub feedback: Option<String>,
}

/// Metric function that returns score with optional feedback
///
/// This is used by GEPA to collect both quantitative scores and qualitative
/// feedback for LLM-based reflection.
///
/// # Arguments
/// * `expected` - The training example with expected outputs
/// * `predicted` - The state produced by the node
///
/// # Returns
/// Score (0.0 to 1.0) with optional feedback text
pub type GEPAMetricFn<S> = Arc<dyn Fn(&S, &S) -> Result<ScoreWithFeedback> + Send + Sync>;

/// A candidate program in the evolutionary population
#[derive(Debug, Clone)]
struct Candidate {
    /// The optimization state (instruction + examples)
    state: OptimizationState,

    /// Parent candidate indices
    parents: Vec<usize>,

    /// Aggregate validation score
    val_aggregate_score: f64,

    /// Per-instance validation scores
    val_subscores: Vec<f64>,
}

/// Detailed results from GEPA optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GEPAResult {
    /// All proposed candidates (optimization states)
    pub candidates: Vec<OptimizationState>,

    /// Parent indices for each candidate
    pub parents: Vec<Vec<usize>>,

    /// Aggregate validation scores
    pub val_aggregate_scores: Vec<f64>,

    /// Per-instance validation scores
    pub val_subscores: Vec<Vec<f64>>,

    /// Total number of metric calls
    pub total_metric_calls: usize,

    /// Number of full validation evaluations
    pub num_full_val_evals: usize,

    /// Index of best candidate
    pub best_idx: usize,
}

impl GEPAResult {
    /// Get the best candidate program (optimization state)
    pub fn best_candidate(&self) -> &OptimizationState {
        &self.candidates[self.best_idx]
    }

    /// Get the best validation score
    pub fn best_score(&self) -> f64 {
        self.val_aggregate_scores[self.best_idx]
    }
}

/// GEPA: Genetic Evolutionary Prompt Algorithm optimizer
///
/// GEPA uses LLM-based reflection to evolve prompts through evolutionary search.
/// It maintains a population of candidate prompts and uses feedback-driven mutation.
pub struct GEPA {
    config: GEPAConfig,
}

impl GEPA {
    /// Create a new GEPA optimizer with default configuration
    pub fn new() -> Self {
        Self {
            config: GEPAConfig::default(),
        }
    }

    /// Create a GEPA optimizer with custom configuration
    #[must_use]
    pub fn with_config(config: GEPAConfig) -> Self {
        Self { config }
    }

    /// Set maximum number of metric calls
    #[must_use]
    pub fn with_max_metric_calls(mut self, max: usize) -> Self {
        self.config.max_metric_calls = Some(max);
        self
    }

    /// Set maximum number of full evaluations
    #[must_use]
    pub fn with_max_full_evals(mut self, max: usize) -> Self {
        self.config.max_full_evals = Some(max);
        self
    }

    /// Set reflection minibatch size
    #[must_use]
    pub fn with_reflection_minibatch_size(mut self, size: usize) -> Self {
        self.config.reflection_minibatch_size = size;
        self
    }

    /// Set candidate selection strategy
    #[must_use]
    pub fn with_candidate_selection_strategy(mut self, strategy: SelectionStrategy) -> Self {
        self.config.candidate_selection_strategy = strategy;
        self
    }

    /// Set whether to skip perfect score examples during reflection
    #[must_use]
    pub fn with_skip_perfect_score(mut self, skip: bool) -> Self {
        self.config.skip_perfect_score = skip;
        self
    }

    /// Set random seed for reproducibility
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.config.seed = seed;
        self
    }

    /// Evaluate a node on a batch of examples using a GEPA metric
    async fn evaluate_candidate<S, N>(
        &self,
        node: &N,
        examples: &[S],
        metric: &GEPAMetricFn<S>,
    ) -> Result<Vec<ScoreWithFeedback>>
    where
        S: GraphState,
        N: Node<S>,
    {
        let mut results = Vec::new();

        for example in examples {
            let prediction = node.execute(example.clone()).await?;
            let result = metric(example, &prediction)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Generate a mutated instruction using LLM-based reflection
    async fn reflect_and_mutate(
        &self,
        reflection_model: &dyn ChatModel,
        current_instruction: &str,
        feedback: &str,
    ) -> Result<String> {
        // Construct reflection prompt
        let prompt = format!(
            "You are optimizing a prompt-based system. The current instruction is:\n\n\
             \"{}\"\n\n\
             Based on the following feedback from recent executions:\n\n\
             {}\n\n\
             Propose an improved instruction that addresses the feedback. \
             Output only the new instruction text, without explanations.",
            current_instruction, feedback
        );

        // Query reflection LM
        let messages = vec![HumanMessage::new(prompt).into()];

        let response = reflection_model
            .generate(&messages, None, None, None, None)
            .await?;

        if let Some(generation) = response.generations.first() {
            Ok(generation.message.content().as_text())
        } else {
            // Fallback: return current instruction with minor variation
            Ok(format!("{} [optimized]", current_instruction))
        }
    }

    /// Select a candidate from the population based on strategy
    fn select_candidate(&self, candidates: &[Candidate], rng: &mut impl Rng) -> usize {
        match self.config.candidate_selection_strategy {
            SelectionStrategy::CurrentBest => {
                // Select candidate with highest aggregate score
                candidates
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        a.val_aggregate_score
                            .partial_cmp(&b.val_aggregate_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(idx, _)| idx)
                    .unwrap_or(0)
            }
            SelectionStrategy::Pareto => {
                // Select from Pareto frontier (simplified: top-k by aggregate score)
                // Note: This is a simplified Pareto selection that uses aggregate scores.
                // A true Pareto dominance implementation would compare per-instance scores
                // and select non-dominated solutions. The current approach works well
                // for single-objective optimization and provides reasonable results.
                let mut scored: Vec<_> = candidates.iter().enumerate().collect();
                scored.sort_by(|(_, a), (_, b)| {
                    b.val_aggregate_score
                        .partial_cmp(&a.val_aggregate_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let pareto_size = (candidates.len() / 3).max(1);
                let pareto_candidates: Vec<_> = scored.into_iter().take(pareto_size).collect();

                if pareto_candidates.is_empty() {
                    0
                } else {
                    pareto_candidates[rng.gen_range(0..pareto_candidates.len())].0
                }
            }
        }
    }

    /// Run GEPA optimization
    ///
    /// # Arguments
    /// * `node` - The node to optimize
    /// * `trainset` - Training examples for reflection
    /// * `valset` - Validation examples for evaluation (if None, uses trainset)
    /// * `metric` - Metric function that returns score with optional feedback
    /// * `reflection_model` - LLM used for reflection and mutation
    ///
    /// # Returns
    /// Tuple of (optimized node, detailed result if track_stats enabled)
    pub async fn optimize<S, N>(
        &self,
        node: &mut N,
        trainset: &[S],
        valset: Option<&[S]>,
        metric: &GEPAMetricFn<S>,
        reflection_model: &dyn ChatModel,
    ) -> Result<GEPAResult>
    where
        S: GraphState,
        N: Node<S> + Optimizable<S>,
    {
        record_optimization_start("gepa");
        let start_time = Instant::now();
        let mut iterations_completed = 0u64;

        let mut rng = rand::rngs::StdRng::seed_from_u64(self.config.seed);

        let valset = valset.unwrap_or(trainset);

        // Calculate budget
        let max_metric_calls = if let Some(max_full_evals) = self.config.max_full_evals {
            max_full_evals * (trainset.len() + valset.len())
        } else {
            self.config.max_metric_calls.unwrap_or(100)
        };

        // Initialize population with seed candidate (current node state)
        let mut candidates = Vec::new();
        let initial_state = node.get_optimization_state();

        // Evaluate seed candidate
        let seed_results = self
            .evaluate_candidate(node, valset, metric)
            .await
            .map_err(|e| crate::Error::Validation(format!("GEPA seed evaluation failed: {}", e)))?;

        // M-905: Guard against empty valset which would cause division by zero / NaN
        if seed_results.is_empty() {
            return Err(crate::Error::Validation(
                "GEPA requires at least one validation example".into(),
            ));
        }

        let seed_scores: Vec<f64> = seed_results.iter().map(|r| r.score).collect();
        let seed_aggregate = seed_scores.iter().sum::<f64>() / seed_scores.len() as f64;

        candidates.push(Candidate {
            state: initial_state.clone(),
            parents: vec![],
            val_aggregate_score: seed_aggregate,
            val_subscores: seed_scores.clone(),
        });

        let initial_score = seed_aggregate;
        let mut total_metric_calls = seed_scores.len();
        let mut num_full_val_evals = 1;

        // Optimization loop
        while total_metric_calls < max_metric_calls {
            record_iteration("gepa");
            iterations_completed += 1;

            // Select parent candidate
            let parent_idx = self.select_candidate(&candidates, &mut rng);

            // Clone parent to avoid borrow conflicts
            let parent_state = candidates[parent_idx].state.clone();
            let parent_aggregate_score = candidates[parent_idx].val_aggregate_score;

            // Sample reflection minibatch
            let minibatch_size = self.config.reflection_minibatch_size.min(trainset.len());
            let minibatch: Vec<&S> = (0..minibatch_size)
                .map(|_| &trainset[rng.gen_range(0..trainset.len())])
                .collect();

            // Apply parent state to node for evaluation
            node.set_optimization_state(parent_state.clone());

            // Collect feedback from minibatch
            let mut feedback_texts = Vec::new();
            for example in &minibatch {
                let pred = node.execute((*example).clone()).await.map_err(|e| {
                    crate::Error::Validation(format!("GEPA minibatch execution failed: {}", e))
                })?;
                let result = metric(example, &pred)?;

                // Skip perfect scores if configured
                if self.config.skip_perfect_score && result.score >= self.config.perfect_score {
                    continue;
                }

                if let Some(feedback) = result.feedback {
                    feedback_texts.push(format!(
                        "Score: {:.2}, Feedback: {}",
                        result.score, feedback
                    ));
                }

                total_metric_calls += 1;
            }

            // If no feedback collected, generate generic feedback
            let combined_feedback = if feedback_texts.is_empty() {
                "No specific feedback available. Try to improve the instruction.".to_string()
            } else {
                feedback_texts.join("\n\n")
            };

            // Generate mutated instruction
            let new_instruction = self
                .reflect_and_mutate(
                    reflection_model,
                    &parent_state.instruction,
                    &combined_feedback,
                )
                .await
                .map_err(|e| {
                    crate::Error::Validation(format!("GEPA reflection/mutation failed: {}", e))
                })?;

            // Create new candidate state
            let mut new_state = parent_state.clone();
            new_state.instruction = new_instruction;

            // Apply new state to node
            node.set_optimization_state(new_state.clone());

            // Evaluate new candidate on valset
            let new_results = self.evaluate_candidate(node, valset, metric).await?;
            let new_scores: Vec<f64> = new_results.iter().map(|r| r.score).collect();
            let new_aggregate = new_scores.iter().sum::<f64>() / new_scores.len() as f64;

            total_metric_calls += new_scores.len();
            num_full_val_evals += 1;

            candidates.push(Candidate {
                state: new_state,
                parents: vec![parent_idx],
                val_aggregate_score: new_aggregate,
                val_subscores: new_scores,
            });

            // Log progress
            if new_aggregate > parent_aggregate_score {
                tracing::info!(
                    "GEPA iteration {}: Improved score from {:.3} to {:.3}",
                    candidates.len(),
                    parent_aggregate_score,
                    new_aggregate
                );
            }
        }

        // Find best candidate
        let best_idx = candidates
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.val_aggregate_score
                    .partial_cmp(&b.val_aggregate_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        let best_candidate = &candidates[best_idx];

        // Apply best state to node
        node.set_optimization_state(best_candidate.state.clone());

        // Build result
        let result = GEPAResult {
            candidates: candidates.iter().map(|c| c.state.clone()).collect(),
            parents: candidates.iter().map(|c| c.parents.clone()).collect(),
            val_aggregate_scores: candidates.iter().map(|c| c.val_aggregate_score).collect(),
            val_subscores: candidates.iter().map(|c| c.val_subscores.clone()).collect(),
            total_metric_calls,
            num_full_val_evals,
            best_idx,
        };

        let duration = start_time.elapsed().as_secs_f64();
        record_optimization_complete(
            "gepa",
            iterations_completed,
            candidates.len() as u64,
            initial_score,
            best_candidate.val_aggregate_score,
            duration,
        );

        Ok(result)
    }
}

impl Default for GEPA {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to convert a standard MetricFn to a GEPAMetricFn (without feedback)
///
/// This allows using existing metrics with GEPA, though they won't provide
/// feedback for reflection.
pub fn metric_to_gepa<S: GraphState>(metric: &MetricFn<S>) -> GEPAMetricFn<S> {
    let metric_clone = Arc::clone(metric);
    Arc::new(move |expected, predicted| {
        let score = metric_clone(expected, predicted)?;
        Ok(ScoreWithFeedback {
            score,
            feedback: None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::callbacks::CallbackManager;
    use crate::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use crate::core::messages::{BaseMessage, Message};
    use crate::node::Node;
    use crate::optimize::{Optimizable, OptimizationState};
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};

    // Test state
    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestState {
        input: String,
        output: String,
    }

    // Mock node for testing
    struct MockNode {
        state: OptimizationState,
        execution_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockNode {
        fn new() -> Self {
            Self {
                state: OptimizationState::new("Initial instruction"),
                execution_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
            }
        }
    }

    #[async_trait]
    impl Node<TestState> for MockNode {
        async fn execute(&self, mut state: TestState) -> Result<TestState> {
            let mut count = self.execution_count.lock().unwrap();
            *count += 1;

            // Simple uppercase transformation based on instruction
            if self.state.instruction.contains("uppercase") {
                state.output = state.input.to_uppercase();
            } else {
                state.output = state.input.clone();
            }
            Ok(state)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[async_trait]
    impl Optimizable<TestState> for MockNode {
        async fn optimize(
            &mut self,
            _examples: &[TestState],
            _metric: &MetricFn<TestState>,
            _config: &super::super::OptimizerConfig,
        ) -> Result<super::super::OptimizationResult> {
            Err(crate::Error::Generic(
                "MockNode.optimize() not implemented - use GEPA.optimize() directly".into(),
            ))
        }

        fn get_optimization_state(&self) -> OptimizationState {
            self.state.clone()
        }

        fn set_optimization_state(&mut self, state: OptimizationState) {
            self.state = state;
        }
    }

    // Mock ChatModel for reflection
    struct MockReflectionModel {
        response: String,
    }

    #[async_trait]
    impl ChatModel for MockReflectionModel {
        async fn _generate(
            &self,
            messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _callbacks: Option<&CallbackManager>,
        ) -> crate::core::Result<ChatResult> {
            // Check if reflection prompt is present
            let has_reflection = messages.iter().any(|m| {
                m.content()
                    .as_text()
                    .contains("Propose an improved instruction")
            });

            let response_text = if has_reflection {
                self.response.clone()
            } else {
                "Generated response".to_string()
            };

            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: Message::ai(response_text),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_reflection"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_gepa_config_defaults() {
        let config = GEPAConfig::default();
        assert_eq!(config.max_metric_calls, Some(100));
        assert_eq!(config.reflection_minibatch_size, 3);
        assert_eq!(
            config.candidate_selection_strategy,
            SelectionStrategy::Pareto
        );
        assert!(config.skip_perfect_score);
        assert_eq!(config.failure_score, 0.0);
        assert_eq!(config.perfect_score, 1.0);
    }

    #[tokio::test]
    async fn test_gepa_builder() {
        let gepa = GEPA::new()
            .with_max_metric_calls(50)
            .with_reflection_minibatch_size(5)
            .with_candidate_selection_strategy(SelectionStrategy::CurrentBest)
            .with_skip_perfect_score(false)
            .with_seed(42);

        assert_eq!(gepa.config.max_metric_calls, Some(50));
        assert_eq!(gepa.config.reflection_minibatch_size, 5);
        assert_eq!(
            gepa.config.candidate_selection_strategy,
            SelectionStrategy::CurrentBest
        );
        assert!(!gepa.config.skip_perfect_score);
        assert_eq!(gepa.config.seed, 42);
    }

    #[tokio::test]
    async fn test_selection_strategy_current_best() {
        let gepa = GEPA::new().with_candidate_selection_strategy(SelectionStrategy::CurrentBest);

        let candidates = vec![
            Candidate {
                state: OptimizationState::new("candidate 1"),
                parents: vec![],
                val_aggregate_score: 0.5,
                val_subscores: vec![0.5],
            },
            Candidate {
                state: OptimizationState::new("candidate 2"),
                parents: vec![],
                val_aggregate_score: 0.8, // Best
                val_subscores: vec![0.8],
            },
            Candidate {
                state: OptimizationState::new("candidate 3"),
                parents: vec![],
                val_aggregate_score: 0.6,
                val_subscores: vec![0.6],
            },
        ];

        // M-571: Use seeded RNG for deterministic test behavior
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let selected = gepa.select_candidate(&candidates, &mut rng);
        assert_eq!(selected, 1); // Should always select highest score
    }

    #[tokio::test]
    async fn test_selection_strategy_pareto() {
        let gepa = GEPA::new().with_candidate_selection_strategy(SelectionStrategy::Pareto);

        let candidates = vec![
            Candidate {
                state: OptimizationState::new("candidate 1"),
                parents: vec![],
                val_aggregate_score: 0.5,
                val_subscores: vec![0.5],
            },
            Candidate {
                state: OptimizationState::new("candidate 2"),
                parents: vec![],
                val_aggregate_score: 0.8,
                val_subscores: vec![0.8],
            },
            Candidate {
                state: OptimizationState::new("candidate 3"),
                parents: vec![],
                val_aggregate_score: 0.6,
                val_subscores: vec![0.6],
            },
        ];

        // M-571: Use seeded RNG for deterministic test behavior
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let selected = gepa.select_candidate(&candidates, &mut rng);
        // Pareto frontier = top (n/3).max(1) = 1 candidate (the highest scorer at index 1)
        // With only one candidate on the frontier, selection is deterministic
        assert_eq!(selected, 1);
    }

    #[tokio::test]
    async fn test_reflect_and_mutate() {
        let gepa = GEPA::new();
        let reflection_model = MockReflectionModel {
            response: "Convert input to uppercase format".to_string(),
        };

        let current = "Process the input";
        let feedback = "Score: 0.3, Feedback: Output should be uppercase";

        let result = gepa
            .reflect_and_mutate(&reflection_model, current, feedback)
            .await
            .unwrap();

        assert_eq!(result, "Convert input to uppercase format");
    }

    #[tokio::test]
    async fn test_evaluate_candidate() {
        let gepa = GEPA::new();
        let node = MockNode::new();

        let examples = vec![
            TestState {
                input: "hello".to_string(),
                output: "hello".to_string(),
            },
            TestState {
                input: "world".to_string(),
                output: "world".to_string(),
            },
        ];

        let metric: GEPAMetricFn<TestState> = Arc::new(|expected, predicted| {
            let score = if expected.output == predicted.output {
                1.0
            } else {
                0.0
            };
            Ok(ScoreWithFeedback {
                score,
                feedback: Some(format!(
                    "Expected: {}, Got: {}",
                    expected.output, predicted.output
                )),
            })
        });

        let results = gepa
            .evaluate_candidate(&node, &examples, &metric)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].score, 1.0);
        assert_eq!(results[1].score, 1.0);
        assert!(results[0].feedback.is_some());
    }

    #[tokio::test]
    async fn test_metric_to_gepa_conversion() {
        let standard_metric: MetricFn<TestState> = Arc::new(|expected, predicted| {
            if expected.output == predicted.output {
                Ok(1.0)
            } else {
                Ok(0.0)
            }
        });

        let gepa_metric = metric_to_gepa(&standard_metric);

        let expected = TestState {
            input: "test".to_string(),
            output: "test".to_string(),
        };
        let predicted = expected.clone();

        let result = gepa_metric(&expected, &predicted).unwrap();
        assert_eq!(result.score, 1.0);
        assert!(result.feedback.is_none()); // Converted metrics have no feedback
    }

    #[tokio::test]
    async fn test_gepa_optimize_basic() {
        let gepa = GEPA::new()
            .with_max_metric_calls(20)
            .with_reflection_minibatch_size(1)
            .with_seed(42);

        let mut node = MockNode::new();
        let reflection_model = MockReflectionModel {
            response: "Make output uppercase".to_string(),
        };

        let trainset = vec![
            TestState {
                input: "hello".to_string(),
                output: "HELLO".to_string(),
            },
            TestState {
                input: "world".to_string(),
                output: "WORLD".to_string(),
            },
        ];

        let metric: GEPAMetricFn<TestState> = Arc::new(|expected, predicted| {
            let score = if expected.output == predicted.output {
                1.0
            } else {
                0.0
            };
            let feedback = if score < 1.0 {
                Some(format!(
                    "Output mismatch. Expected: {}, Got: {}",
                    expected.output, predicted.output
                ))
            } else {
                None
            };
            Ok(ScoreWithFeedback { score, feedback })
        });

        let result = gepa
            .optimize(&mut node, &trainset, None, &metric, &reflection_model)
            .await
            .unwrap();

        // Should have explored multiple candidates
        assert!(result.candidates.len() > 1);
        assert!(result.total_metric_calls > 0);
        assert!(result.num_full_val_evals > 0);
        assert_eq!(result.best_idx, result.val_aggregate_scores.len() - 1);

        // Node should have best state applied
        let final_state = node.get_optimization_state();
        assert_eq!(final_state.instruction, result.best_candidate().instruction);
    }

    #[tokio::test]
    async fn test_gepa_result_accessors() {
        let result = GEPAResult {
            candidates: vec![
                OptimizationState::new("candidate 1"),
                OptimizationState::new("candidate 2"),
                OptimizationState::new("candidate 3"),
            ],
            parents: vec![vec![], vec![0], vec![1]],
            val_aggregate_scores: vec![0.5, 0.7, 0.9],
            val_subscores: vec![vec![0.5], vec![0.7], vec![0.9]],
            total_metric_calls: 30,
            num_full_val_evals: 3,
            best_idx: 2,
        };

        assert_eq!(result.best_score(), 0.9);
        assert_eq!(result.best_candidate().instruction, "candidate 3");
    }

    #[tokio::test]
    async fn test_score_with_feedback() {
        let score = ScoreWithFeedback {
            score: 0.85,
            feedback: Some("Good but could be better".to_string()),
        };

        assert_eq!(score.score, 0.85);
        assert_eq!(score.feedback.as_deref(), Some("Good but could be better"));
    }

    #[tokio::test]
    async fn test_gepa_empty_valset_error() {
        // M-905: Test that empty valset returns error instead of NaN
        let gepa = GEPA::new().with_max_metric_calls(10).with_seed(42);

        let mut node = MockNode::new();
        let reflection_model = MockReflectionModel {
            response: "Better instruction".to_string(),
        };

        let trainset = vec![TestState {
            input: "hello".to_string(),
            output: "hello".to_string(),
        }];

        let empty_valset: Vec<TestState> = vec![];

        let metric: GEPAMetricFn<TestState> = Arc::new(|_, _| {
            Ok(ScoreWithFeedback {
                score: 1.0,
                feedback: None,
            })
        });

        let result = gepa
            .optimize(
                &mut node,
                &trainset,
                Some(&empty_valset),
                &metric,
                &reflection_model,
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("at least one validation example"));
    }

    #[test]
    fn test_gepa_config_builder_new() {
        let config = GEPAConfig::new();
        let default_config = GEPAConfig::default();
        assert_eq!(config.max_metric_calls, default_config.max_metric_calls);
        assert_eq!(
            config.reflection_minibatch_size,
            default_config.reflection_minibatch_size
        );
        assert_eq!(config.failure_score, default_config.failure_score);
        assert_eq!(config.perfect_score, default_config.perfect_score);
    }

    #[test]
    fn test_gepa_config_builder_full_chain() {
        let config = GEPAConfig::new()
            .with_max_full_evals(Some(50))
            .with_max_metric_calls(Some(200))
            .with_reflection_minibatch_size(5)
            .with_candidate_selection_strategy(SelectionStrategy::CurrentBest)
            .with_skip_perfect_score(false)
            .with_use_merge(false)
            .with_max_merge_invocations(10)
            .with_num_threads(Some(4))
            .with_failure_score(-1.0)
            .with_perfect_score(10.0)
            .with_track_stats(true)
            .with_seed(12345);

        assert_eq!(config.max_full_evals, Some(50));
        assert_eq!(config.max_metric_calls, Some(200));
        assert_eq!(config.reflection_minibatch_size, 5);
        assert_eq!(
            config.candidate_selection_strategy,
            SelectionStrategy::CurrentBest
        );
        assert!(!config.skip_perfect_score);
        assert!(!config.use_merge);
        assert_eq!(config.max_merge_invocations, 10);
        assert_eq!(config.num_threads, Some(4));
        assert_eq!(config.failure_score, -1.0);
        assert_eq!(config.perfect_score, 10.0);
        assert!(config.track_stats);
        assert_eq!(config.seed, 12345);
    }

    #[test]
    fn test_gepa_config_builder_partial_chain() {
        // Test that partial builder chains preserve defaults
        let config = GEPAConfig::new()
            .with_max_metric_calls(Some(500))
            .with_seed(999);

        // Custom values
        assert_eq!(config.max_metric_calls, Some(500));
        assert_eq!(config.seed, 999);

        // Default values preserved
        assert_eq!(config.max_full_evals, None);
        assert_eq!(config.reflection_minibatch_size, 3);
        assert_eq!(
            config.candidate_selection_strategy,
            SelectionStrategy::Pareto
        );
        assert!(config.skip_perfect_score);
        assert_eq!(config.failure_score, 0.0);
        assert_eq!(config.perfect_score, 1.0);
    }

    #[test]
    fn test_gepa_config_builder_validates() {
        // Test that builder-created config validates correctly
        let valid_config = GEPAConfig::new()
            .with_reflection_minibatch_size(3)
            .with_failure_score(0.0)
            .with_perfect_score(1.0);
        assert!(valid_config.validate().is_ok());

        // Invalid: reflection_minibatch_size == 0
        let invalid_config = GEPAConfig::new().with_reflection_minibatch_size(0);
        assert!(invalid_config.validate().is_err());

        // Invalid: failure_score > perfect_score
        let invalid_config2 = GEPAConfig::new()
            .with_failure_score(2.0)
            .with_perfect_score(1.0);
        assert!(invalid_config2.validate().is_err());
    }
}
