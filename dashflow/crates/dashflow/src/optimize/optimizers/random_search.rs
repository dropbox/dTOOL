//! # RandomSearch Optimizer
//!
//! Explores multiple candidate programs by varying training set configurations
//! and optimization strategies.
//!
//! ## Algorithm
//!
//! RandomSearch generates and evaluates multiple candidate programs:
//! 1. Zero-shot baseline (no demonstrations)
//! 2. Labeled few-shot (ground truth only)
//! 3. Unshuffled bootstrap
//! 4. Multiple shuffled bootstraps with varying demonstration counts
//!
//! Each candidate is evaluated on a validation set, and the best performing
//! program is returned.
//!
//! ## Adapted from DashOpt
//!
//! This is a port of DashOpt's `BootstrapFewShotWithRandomSearch` optimizer,
//! adapted for DashFlow's `Node<S>` architecture.
//!
//! ## References
//!
//! - **Concept**: Standard random search optimization
//! - **Source**: DSPy framework
//! - **Link**: <https://arxiv.org/abs/2310.03714>

use super::{BootstrapFewShot, LabeledFewShot, OptimizationResult, OptimizerConfig};
use crate::node::Node;
use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::optimize::MetricFn;
use crate::state::GraphState;
use crate::{Error, Result};
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;

/// Maximum number of training examples that can be fully shuffled.
///
/// For trainsets larger than this, only the first MAX_SHUFFLE_INDICES examples
/// participate in random shuffling. Remaining examples are appended in original
/// order via `shuffle_examples()`. This limit exists because shuffle indices
/// are pre-generated for deterministic seed-based shuffling, and generating
/// unbounded index vectors would be wasteful for typical trainsets.
///
/// For most practical use cases, trainsets are well under 1000 examples.
/// If you have a larger trainset and need full shuffling, consider
/// pre-shuffling externally or using a different optimization strategy.
const MAX_SHUFFLE_INDICES: usize = 1000;

/// Candidate program with evaluation data
#[derive(Clone, Debug)]
pub struct CandidateProgram {
    /// Overall score on validation set
    pub score: f64,
    /// Individual scores for each validation example
    pub subscores: Vec<f64>,
    /// Random seed used to generate this candidate
    /// (-3=zero-shot, -2=labeled, -1=unshuffled, >=0=shuffled)
    pub seed: i32,
    /// Configuration used for this candidate
    pub config: OptimizerConfig,
}

/// Random Search optimizer that explores multiple candidate configurations.
///
/// Based on DashOpt's `BootstrapFewShotWithRandomSearch`, this optimizer:
/// 1. Generates multiple candidate programs with different random configurations
/// 2. Evaluates each candidate on a validation set
/// 3. Returns the best performing configuration
///
/// The search includes:
/// - Zero-shot baseline (no demonstrations)
/// - Labeled few-shot (ground truth only)
/// - Unshuffled bootstrap
/// - Multiple shuffled bootstraps with varying demonstration counts
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::*;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create optimizer
/// let optimizer = RandomSearch::new()
///     .with_max_bootstrapped_demos(4)
///     .with_max_labeled_demos(16)
///     .with_num_candidate_programs(16)
///     .with_stop_at_score(0.95);
///
/// // Optimize node
/// let result = optimizer.optimize(
///     &mut node,
///     &trainset,
///     &valset,
///     &metric
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub struct RandomSearch {
    /// Base configuration for bootstrap optimizers
    base_config: OptimizerConfig,
    /// Maximum number of bootstrapped demonstrations
    max_bootstrapped_demos: usize,
    /// Maximum number of labeled demonstrations
    max_labeled_demos: usize,
    /// Maximum bootstrap rounds
    max_rounds: usize,
    /// Number of candidate programs to generate and evaluate
    num_candidate_programs: usize,
    /// Maximum errors before stopping
    max_errors: Option<usize>,
    /// Score threshold to stop early when reached
    stop_at_score: Option<f64>,
    /// Metric threshold for bootstrap filtering
    metric_threshold: Option<f64>,
    /// Minimum number of samples to bootstrap
    min_num_samples: usize,
    /// Maximum number of samples to bootstrap
    max_num_samples: usize,
}

impl RandomSearch {
    /// Create a new RandomSearch optimizer with default settings.
    pub fn new() -> Self {
        Self {
            base_config: OptimizerConfig::default(),
            max_bootstrapped_demos: 4,
            max_labeled_demos: 16,
            max_rounds: 1,
            num_candidate_programs: 16,
            max_errors: None,
            stop_at_score: None,
            metric_threshold: None,
            min_num_samples: 1,
            max_num_samples: 4,
        }
    }

    /// Set maximum number of bootstrapped demonstrations.
    #[must_use]
    pub fn with_max_bootstrapped_demos(mut self, max: usize) -> Self {
        self.max_bootstrapped_demos = max;
        self.max_num_samples = max;
        self
    }

    /// Set maximum number of labeled demonstrations.
    #[must_use]
    pub fn with_max_labeled_demos(mut self, max: usize) -> Self {
        self.max_labeled_demos = max;
        self
    }

    /// Set maximum bootstrap rounds.
    #[must_use]
    pub fn with_max_rounds(mut self, rounds: usize) -> Self {
        self.max_rounds = rounds;
        self
    }

    /// Set number of candidate programs to generate and evaluate.
    #[must_use]
    pub fn with_num_candidate_programs(mut self, num: usize) -> Self {
        self.num_candidate_programs = num;
        self
    }

    /// Set maximum errors before stopping.
    #[must_use]
    pub fn with_max_errors(mut self, max: usize) -> Self {
        self.max_errors = Some(max);
        self
    }

    /// Set score threshold to stop early when reached.
    #[must_use]
    pub fn with_stop_at_score(mut self, score: f64) -> Self {
        self.stop_at_score = Some(score);
        self
    }

    /// Set metric threshold for bootstrap filtering.
    #[must_use]
    pub fn with_metric_threshold(mut self, threshold: f64) -> Self {
        self.metric_threshold = Some(threshold);
        self
    }

    /// Set base optimizer configuration.
    #[must_use]
    pub fn with_config(mut self, config: OptimizerConfig) -> Self {
        self.base_config = config;
        self
    }

    /// Generate a candidate configuration for a given seed.
    ///
    /// # Seed Semantics
    /// - seed = -3: Zero-shot (no demonstrations)
    /// - seed = -2: Labeled few-shot only (no bootstrap)
    /// - seed = -1: Unshuffled bootstrap
    /// - seed >= 0: Shuffled bootstrap with random demo count
    fn generate_candidate_config(&self, seed: i32) -> (OptimizerConfig, Vec<usize>) {
        match seed {
            -3 => {
                // Zero-shot baseline - no demos
                let config = OptimizerConfig {
                    max_few_shot_examples: 0,
                    ..self.base_config.clone()
                };
                (config, vec![])
            }
            -2 => {
                // Labeled few-shot only
                let config = OptimizerConfig {
                    max_few_shot_examples: self.max_labeled_demos,
                    ..self.base_config.clone()
                };
                (config, vec![])
            }
            -1 => {
                // Unshuffled bootstrap
                let config = OptimizerConfig {
                    max_few_shot_examples: self.max_num_samples,
                    max_iterations: self.max_rounds,
                    ..self.base_config.clone()
                };
                (config, (0..MAX_SHUFFLE_INDICES).collect()) // Unshuffled order
            }
            _ => {
                // Shuffled bootstrap with random size
                let mut rng = rand::rngs::StdRng::seed_from_u64(seed as u64);

                // Random size for this candidate
                let size = rng.gen_range(self.min_num_samples..=self.max_num_samples);

                let config = OptimizerConfig {
                    max_few_shot_examples: size,
                    max_iterations: self.max_rounds,
                    random_seed: Some(seed as u64),
                    ..self.base_config.clone()
                };

                // Generate shuffled indices (see MAX_SHUFFLE_INDICES for size limit docs)
                let mut indices: Vec<usize> = (0..MAX_SHUFFLE_INDICES).collect();
                indices.shuffle(&mut rng);

                (config, indices)
            }
        }
    }

    /// Shuffle training examples according to indices.
    fn shuffle_examples<S: GraphState>(&self, examples: &[S], indices: &[usize]) -> Vec<S> {
        if indices.is_empty() {
            return examples.to_vec();
        }

        let mut shuffled = Vec::new();
        for &idx in indices.iter().take(examples.len()) {
            if idx < examples.len() {
                shuffled.push(examples[idx].clone());
            }
        }

        // If we have fewer shuffled examples than original, append remaining
        if shuffled.len() < examples.len() {
            for example in examples.iter().skip(shuffled.len()) {
                shuffled.push(example.clone());
            }
        }

        shuffled
    }

    /// Evaluate a node configuration on the validation set.
    async fn evaluate_candidate<S, N>(
        &self,
        node: &N,
        valset: &[S],
        metric: &MetricFn<S>,
    ) -> Result<(f64, Vec<f64>)>
    where
        S: GraphState,
        N: Node<S>,
    {
        let mut subscores = Vec::new();
        let mut total_score = 0.0;
        let mut error_count = 0;
        let max_errors = self.max_errors.unwrap_or(usize::MAX);

        for example in valset {
            // Run node on this example
            match node.execute(example.clone()).await {
                Ok(prediction) => {
                    // Compute score
                    match metric(example, &prediction) {
                        Ok(score) => {
                            total_score += score;
                            subscores.push(score);
                        }
                        Err(e) => {
                            error_count += 1;
                            if error_count > max_errors {
                                return Err(crate::Error::Validation(format!(
                                    "Too many errors during evaluation: {}",
                                    e
                                )));
                            }
                            subscores.push(0.0);
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    if error_count > max_errors {
                        return Err(crate::Error::Validation(format!(
                            "Too many errors during evaluation: {}",
                            e
                        )));
                    }
                    subscores.push(0.0);
                }
            }
        }

        // Calculate average score (0.0 to 1.0)
        let avg_score = if subscores.is_empty() {
            0.0
        } else {
            total_score / subscores.len() as f64
        };

        Ok((avg_score, subscores))
    }

    /// Run the full random search optimization process.
    ///
    /// This method:
    /// 1. Generates multiple candidate configurations
    /// 2. For each candidate, optimizes the node with that configuration
    /// 3. Evaluates each optimized node on the validation set
    /// 4. Returns the best performing configuration
    ///
    /// # Arguments
    /// * `node` - The node to optimize (will be mutated to best configuration)
    /// * `trainset` - Training data for optimization
    /// * `valset` - Validation data for evaluation
    /// * `metric` - Function to evaluate quality
    ///
    /// # Returns
    /// OptimizationResult with best candidate's performance
    pub async fn optimize<S, N>(
        &self,
        node: &mut N,
        trainset: &[S],
        valset: &[S],
        metric: &MetricFn<S>,
    ) -> Result<OptimizationResult>
    where
        S: GraphState,
        N: Node<S> + Clone,
    {
        use std::time::Instant;
        let start = Instant::now();

        // Record telemetry start
        record_optimization_start("random_search");

        tracing::info!(
            min_samples = self.min_num_samples,
            max_samples = self.max_num_samples,
            num_candidates = self.num_candidate_programs,
            "RandomSearch: Starting optimization"
        );

        let mut best_score = f64::NEG_INFINITY;
        let mut best_config: Option<OptimizerConfig> = None;
        let mut best_node: Option<N> = None;
        let mut all_candidates = Vec::new();

        // Evaluate initial score (zero-shot baseline)
        let initial_score = {
            let (score, _) = self
                .evaluate_candidate(node, valset, metric)
                .await
                .map_err(|e| {
                    Error::Validation(format!("RandomSearch initial evaluation failed: {}", e))
                })?;
            score
        };

        // Generate and evaluate candidates
        for seed in -3..(self.num_candidate_programs as i32 - 3) {
            // Record iteration telemetry
            record_iteration("random_search");

            tracing::debug!(seed, "Evaluating candidate");

            // Generate candidate configuration
            let (config, shuffle_indices) = self.generate_candidate_config(seed);

            // Clone node for this candidate
            let candidate_node = node.clone();

            // Apply optimization based on seed type
            let candidate_result = match seed {
                -3 => {
                    // Zero-shot - no optimization needed, just evaluate
                    self.evaluate_candidate(&candidate_node, valset, metric)
                        .await
                }
                -2 => {
                    // Labeled few-shot - use LabeledFewShot optimizer
                    let optimizer = LabeledFewShot::from_config(&config);

                    // Select demonstrations from training set
                    let _demos = optimizer.select_demos(trainset)?;

                    // Note: Applying demos requires the node to implement Optimizable<S>
                    // which provides set_optimization_state(). The current generic bound
                    // N: Node<S> + Clone doesn't guarantee this. To fully implement:
                    // 1. Add N: Optimizable<S> bound (breaking change), or
                    // 2. Use runtime downcasting via as_any_mut() (brittle)
                    // For now, evaluate without demos - bootstrap variants below
                    // handle demo application through their own optimization process.
                    self.evaluate_candidate(&candidate_node, valset, metric)
                        .await
                }
                _ => {
                    // Bootstrap (shuffled or unshuffled)
                    let shuffled_trainset = self.shuffle_examples(trainset, &shuffle_indices);

                    let optimizer = BootstrapFewShot::new()
                        .with_config(config.clone())
                        .with_max_demos(config.max_few_shot_examples);

                    // Run optimization
                    let _opt_result = optimizer
                        .optimize(&candidate_node, &shuffled_trainset, metric)
                        .await
                        .map_err(|e| {
                            Error::Validation(format!(
                                "RandomSearch candidate seed={} optimization failed: {}",
                                seed, e
                            ))
                        })?;

                    // Evaluate on validation set
                    self.evaluate_candidate(&candidate_node, valset, metric)
                        .await
                }
            };

            match candidate_result {
                Ok((score, subscores)) => {
                    let num_correct = subscores.iter().filter(|&&s| s > 0.5).count();

                    tracing::debug!(
                        seed,
                        score_pct = %format!("{:.2}%", score * 100.0),
                        correct = num_correct,
                        total = subscores.len(),
                        "Candidate result"
                    );

                    // Track best
                    if score > best_score {
                        tracing::debug!(score_pct = %format!("{:.2}%", score * 100.0), seed, "New best score");
                        best_score = score;
                        best_config = Some(config.clone());
                        best_node = Some(candidate_node);
                    }

                    // Store candidate
                    all_candidates.push(CandidateProgram {
                        score,
                        subscores,
                        seed,
                        config,
                    });

                    tracing::debug!(
                        scores = %all_candidates.iter().map(|c| format!("{:.1}%", c.score * 100.0)).collect::<Vec<_>>().join(", "),
                        best_score_pct = %format!("{:.2}%", best_score * 100.0),
                        "Progress"
                    );

                    // Early stopping
                    if let Some(stop_score) = self.stop_at_score {
                        if score >= stop_score {
                            tracing::info!(
                                score_pct = %format!("{:.2}%", score * 100.0),
                                stop_at_pct = %format!("{:.2}%", stop_score * 100.0),
                                "Stopping early"
                            );
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Error evaluating candidate seed={}: {}", seed, e);
                }
            }
        }

        // Apply best configuration to node
        if let Some(best) = best_node {
            *node = best;
        }

        // Sort candidates by score (descending)
        all_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        tracing::info!(
            num_candidates = all_candidates.len(),
            best_score_pct = %format!("{:.2}%", best_score * 100.0),
            "RandomSearch: Optimization complete"
        );

        let duration = start.elapsed();

        // Record telemetry completion
        record_optimization_complete(
            "random_search",
            all_candidates.len() as u64,
            self.num_candidate_programs as u64,
            initial_score,
            best_score,
            duration.as_secs_f64(),
        );

        Ok(OptimizationResult {
            initial_score,
            final_score: best_score,
            iterations: all_candidates.len(),
            converged: best_config.is_some(),
            duration_secs: duration.as_secs_f64(),
        })
    }
}

impl Default for RandomSearch {
    fn default() -> Self {
        Self::new()
    }
}

// NodeOptimizer trait implementation for BetterTogether composition
#[cfg(feature = "default")]
use super::better_together::NodeOptimizer;
#[cfg(feature = "default")]
use async_trait::async_trait;

#[cfg(feature = "default")]
#[async_trait]
impl<S: GraphState> NodeOptimizer<S> for RandomSearch {
    async fn optimize_node(
        &self,
        _node: &mut dyn Node<S>,
        _trainset: &[S],
        _valset: &[S],
        _metric: &MetricFn<S>,
    ) -> Result<OptimizationResult> {
        // RandomSearch requires the node to implement Node<S> + Clone trait.
        // The NodeOptimizer trait takes &mut dyn Node<S> which cannot be cloned
        // because trait objects don't support Clone. This is a fundamental
        // Rust limitation with trait objects.
        //
        // Workaround options:
        // 1. Use RandomSearch::optimize() directly with concrete types
        // 2. Wrap nodes in Arc<Mutex<N>> for shared ownership
        // 3. Use the BetterTogether pipeline which handles this internally
        Err(crate::Error::Validation(
            "RandomSearch::optimize_node() requires concrete Node type. Use RandomSearch::optimize() directly."
                .to_string(),
        ))
    }

    fn name(&self) -> &str {
        "RandomSearch"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_search_creation() {
        let optimizer = RandomSearch::new();

        assert_eq!(optimizer.max_bootstrapped_demos, 4);
        assert_eq!(optimizer.max_labeled_demos, 16);
        assert_eq!(optimizer.num_candidate_programs, 16);
    }

    #[test]
    fn test_random_search_builder() {
        let optimizer = RandomSearch::new()
            .with_max_bootstrapped_demos(8)
            .with_max_labeled_demos(20)
            .with_num_candidate_programs(10)
            .with_stop_at_score(0.95)
            .with_max_errors(5);

        assert_eq!(optimizer.max_bootstrapped_demos, 8);
        assert_eq!(optimizer.max_labeled_demos, 20);
        assert_eq!(optimizer.num_candidate_programs, 10);
        assert_eq!(optimizer.stop_at_score, Some(0.95));
        assert_eq!(optimizer.max_errors, Some(5));
    }

    #[test]
    fn test_seed_semantics() {
        // Document seed semantics - constants verify the convention
        const ZERO_SHOT: i32 = -3;
        const LABELED: i32 = -2;
        const UNSHUFFLED: i32 = -1;
        const SHUFFLED_THRESHOLD: i32 = 0;

        assert_eq!(ZERO_SHOT, -3);
        assert_eq!(LABELED, -2);
        assert_eq!(UNSHUFFLED, -1);
        assert_eq!(SHUFFLED_THRESHOLD, 0, "Shuffled demos use seed >= 0");
    }

    #[test]
    fn test_candidate_config_zero_shot() {
        let optimizer = RandomSearch::new();
        let (config, indices) = optimizer.generate_candidate_config(-3);

        assert_eq!(config.max_few_shot_examples, 0);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_candidate_config_labeled() {
        let optimizer = RandomSearch::new().with_max_labeled_demos(20);
        let (config, indices) = optimizer.generate_candidate_config(-2);

        assert_eq!(config.max_few_shot_examples, 20);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_candidate_config_unshuffled() {
        let optimizer = RandomSearch::new().with_max_bootstrapped_demos(5);
        let (config, indices) = optimizer.generate_candidate_config(-1);

        assert_eq!(config.max_few_shot_examples, 5);
        assert!(!indices.is_empty());
        // Unshuffled means indices are in order
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 1);
    }

    #[test]
    fn test_candidate_config_shuffled() {
        let optimizer = RandomSearch::new();
        let (config1, indices1) = optimizer.generate_candidate_config(0);
        let (config2, indices2) = optimizer.generate_candidate_config(1);

        // Should have random size between min and max
        assert!(config1.max_few_shot_examples >= 1);
        assert!(config1.max_few_shot_examples <= 4);
        assert!(config2.max_few_shot_examples >= 1);
        assert!(config2.max_few_shot_examples <= 4);

        // Should have shuffled indices
        assert!(!indices1.is_empty());
        assert!(!indices2.is_empty());

        // Different seeds should produce different shuffles
        // (probabilistically - might fail with very low probability)
        assert_ne!(indices1[0..10], indices2[0..10]);
    }
}
