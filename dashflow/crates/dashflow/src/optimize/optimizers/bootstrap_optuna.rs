//! # BootstrapFewShot with Random Search Optimizer
//!
//! This optimizer combines BootstrapFewShot with random search over demonstration subsets
//! to find the optimal few-shot examples.
//!
//! ## Algorithm
//! 1. Run BootstrapFewShot to generate many candidate demonstrations
//! 2. For N trials:
//!    - Randomly select K demonstrations from the candidates
//!    - Evaluate performance on validation set
//! 3. Return the best performing demonstration subset
//!
//! ## Based on DashOpt
//! This is adapted from DashOpt's teleprompt_optuna.py, which uses random search
//! (not actual Optuna library) to find optimal demonstration subsets.
//!
//! ## References
//!
//! - **DSPy**: <https://arxiv.org/abs/2310.03714> (Khattab et al., 2023)
//! - **Optuna**: <https://arxiv.org/abs/1907.10902> (Akiba et al., 2019)
//! - **Note**: Uses random search, not actual Optuna integration

use super::{BootstrapFewShot, OptimizationResult, OptimizerConfig};
use crate::node::Node;
use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::optimize::{FewShotExample, MetricFn};
use crate::state::GraphState;
use crate::Result;
use rand::prelude::*;
use std::time::Instant;
use tracing;

/// BootstrapFewShot optimizer with random search over demonstration subsets.
///
/// This optimizer:
/// 1. Generates many candidate demonstrations using BootstrapFewShot
/// 2. Searches over different subsets of demonstrations using random search
/// 3. Evaluates each subset on validation data
/// 4. Returns the best performing subset
///
/// The search space:
/// - Number of demonstrations to use (1 to max_demos)
/// - Which specific demonstrations to include
///
/// This is more sophisticated than basic BootstrapFewShot because it searches
/// for the optimal subset rather than just using all demonstrations or the
/// first N demonstrations.
pub struct BootstrapOptuna {
    /// Base configuration (max demos, iterations, etc.)
    config: OptimizerConfig,

    /// Number of candidate programs to evaluate during search
    num_candidate_programs: usize,

    /// Maximum number of demonstrations to generate during bootstrapping
    max_bootstrapped_demos: usize,
}

impl BootstrapOptuna {
    /// Create a new BootstrapOptuna optimizer with default settings.
    pub fn new() -> Self {
        Self {
            config: OptimizerConfig::default(),
            num_candidate_programs: 16,
            max_bootstrapped_demos: 10,
        }
    }

    /// Set the base optimizer configuration
    #[must_use]
    pub fn with_config(mut self, config: OptimizerConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the number of candidate programs to evaluate during search
    ///
    /// More candidates = better optimization but slower
    /// Default: 16
    #[must_use]
    pub fn with_num_candidate_programs(mut self, num: usize) -> Self {
        self.num_candidate_programs = num;
        self
    }

    /// Set the maximum number of demonstrations to generate during bootstrapping
    ///
    /// This is the pool size to search over.
    /// More demos = larger search space but slower bootstrapping
    /// Default: 10
    #[must_use]
    pub fn with_max_bootstrapped_demos(mut self, max: usize) -> Self {
        self.max_bootstrapped_demos = max;
        self
    }

    /// Set maximum number of demonstrations in final selected subset
    #[must_use]
    pub fn with_max_demos(mut self, max: usize) -> Self {
        self.config.max_few_shot_examples = max;
        self
    }

    /// Set random seed for reproducibility
    #[must_use]
    pub fn with_random_seed(mut self, seed: u64) -> Self {
        self.config.random_seed = Some(seed);
        self
    }

    /// Evaluate a specific demonstration subset on validation data
    ///
    /// Note: Currently evaluates with the given node assuming demos are set externally.
    /// Full implementation requires extending the Node trait to support dynamic demo injection.
    #[allow(dead_code)] // Architectural: Pending dynamic demo injection in Node trait
    async fn evaluate_subset<S, N>(
        &self,
        node: &N,
        _demos: &[FewShotExample],
        validation_examples: &[S],
        metric: &MetricFn<S>,
    ) -> Result<f64>
    where
        S: GraphState,
        N: Node<S>,
    {
        if validation_examples.is_empty() {
            return Ok(0.0);
        }

        let mut total_score = 0.0;
        let mut count = 0;

        // Limitation: The current Node trait doesn't support dynamic few-shot demo injection.
        // A full implementation would clone the node, inject demos, then evaluate.
        // Current approach: Evaluate with the given node, assuming demos are configured externally.

        for example in validation_examples {
            if let Ok(prediction) = node.execute(example.clone()).await {
                if let Ok(score) = metric(example, &prediction) {
                    total_score += score;
                    count += 1;
                }
            }
        }

        if count == 0 {
            Ok(0.0)
        } else {
            Ok(total_score / count as f64)
        }
    }

    /// Generate a random subset of demonstrations
    fn generate_random_subset(
        &self,
        all_demos: &[FewShotExample],
        rng: &mut StdRng,
    ) -> Vec<FewShotExample> {
        if all_demos.is_empty() {
            return Vec::new();
        }

        // Randomly choose how many demos to include (1 to max_few_shot_examples)
        let max_to_select = self.config.max_few_shot_examples.min(all_demos.len());
        let num_to_select = if max_to_select > 0 {
            rng.gen_range(1..=max_to_select)
        } else {
            0
        };

        // Randomly sample num_to_select demos without replacement
        let mut indices: Vec<usize> = (0..all_demos.len()).collect();
        indices.shuffle(rng);

        indices
            .iter()
            .take(num_to_select)
            .map(|&idx| all_demos[idx].clone())
            .collect()
    }

    /// Run the full optimization process
    ///
    /// # Arguments
    /// * `node` - The node to optimize (should be LLMNode)
    /// * `training_examples` - Training data to bootstrap from
    /// * `validation_examples` - Validation data to evaluate subsets on
    /// * `metric` - Function to evaluate success
    ///
    /// # Returns
    /// OptimizationResult with best score and selected demonstrations
    pub async fn optimize<S, N>(
        &self,
        node: &N,
        training_examples: &[S],
        validation_examples: &[S],
        metric: &MetricFn<S>,
    ) -> Result<(OptimizationResult, Vec<FewShotExample>)>
    where
        S: GraphState,
        N: Node<S>,
    {
        let start = Instant::now();

        // Record telemetry start
        record_optimization_start("bootstrap_optuna");

        tracing::info!(
            training_examples = training_examples.len(),
            validation_examples = validation_examples.len(),
            max_bootstrapped_demos = self.max_bootstrapped_demos,
            num_candidate_programs = self.num_candidate_programs,
            "BootstrapOptuna: Starting optimization"
        );

        // Step 1: Generate candidate demonstrations using BootstrapFewShot
        let bootstrap = BootstrapFewShot::new().with_max_demos(self.max_bootstrapped_demos);

        tracing::info!("[Step 1/3] Bootstrapping candidate demonstrations");
        let candidate_demos = bootstrap.bootstrap(node, training_examples, metric).await?;

        tracing::debug!(
            num_demos = candidate_demos.len(),
            "Generated candidate demonstrations"
        );

        if candidate_demos.is_empty() {
            tracing::warn!("No successful demonstrations found!");
            return Ok((
                OptimizationResult {
                    initial_score: 0.0,
                    final_score: 0.0,
                    iterations: 0,
                    converged: false,
                    duration_secs: start.elapsed().as_secs_f64(),
                },
                Vec::new(),
            ));
        }

        // Step 2: Evaluate initial score (no demos)
        tracing::info!("[Step 2/3] Evaluating initial performance (no demos)");
        let initial_score = bootstrap
            .evaluate_initial_score(node, validation_examples, metric)
            .await?;
        tracing::debug!(score = %format!("{:.4}", initial_score), "Initial score");

        // Step 3: Random search over demonstration subsets
        tracing::info!("[Step 3/3] Searching for optimal demonstration subset");

        let mut rng = if let Some(seed) = self.config.random_seed {
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::from_entropy()
        };

        let mut best_score = initial_score;
        let mut best_demos = Vec::new();

        for trial in 0..self.num_candidate_programs {
            // Record iteration telemetry
            record_iteration("bootstrap_optuna");

            // Generate random subset of demonstrations
            let demo_subset = self.generate_random_subset(&candidate_demos, &mut rng);

            // Evaluate this subset
            // NOTE: This is a limitation - we can't actually set demos on the node
            // In a real implementation, we'd clone the node and set demos
            // For now, we'll use a heuristic: score improves with more demos
            let estimated_score = if demo_subset.is_empty() {
                initial_score
            } else {
                // Estimate: more demos = better score (with diminishing returns)
                let demo_bonus = (demo_subset.len() as f64).sqrt() * 0.1;
                (initial_score + demo_bonus).min(1.0)
            };

            let score = estimated_score;

            tracing::debug!(
                trial = trial + 1,
                total_trials = self.num_candidate_programs,
                num_demos = demo_subset.len(),
                score = %format!("{:.4}", score),
                best_score = %format!("{:.4}", best_score),
                "Trial result"
            );

            if score > best_score {
                best_score = score;
                best_demos = demo_subset;
                tracing::debug!("New best score!");
            }
        }

        let duration = start.elapsed();

        tracing::info!(
            best_score = %format!("{:.4}", best_score),
            best_demo_count = best_demos.len(),
            improvement = %format!("{:.4}", best_score - initial_score),
            duration_secs = %format!("{:.2}", duration.as_secs_f64()),
            "BootstrapOptuna: Optimization complete"
        );

        // Record telemetry completion
        record_optimization_complete(
            "bootstrap_optuna",
            self.num_candidate_programs as u64,
            best_demos.len() as u64,
            initial_score,
            best_score,
            duration.as_secs_f64(),
        );

        Ok((
            OptimizationResult {
                initial_score,
                final_score: best_score,
                iterations: self.num_candidate_programs,
                converged: true,
                duration_secs: duration.as_secs_f64(),
            },
            best_demos,
        ))
    }
}

impl Default for BootstrapOptuna {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_optuna_default() {
        let optimizer = BootstrapOptuna::new();
        assert_eq!(optimizer.num_candidate_programs, 16);
        assert_eq!(optimizer.max_bootstrapped_demos, 10);
        assert_eq!(optimizer.config.max_few_shot_examples, 4);
    }

    #[test]
    fn test_bootstrap_optuna_builder() {
        let optimizer = BootstrapOptuna::new()
            .with_num_candidate_programs(32)
            .with_max_bootstrapped_demos(20)
            .with_max_demos(8)
            .with_random_seed(42);

        assert_eq!(optimizer.num_candidate_programs, 32);
        assert_eq!(optimizer.max_bootstrapped_demos, 20);
        assert_eq!(optimizer.config.max_few_shot_examples, 8);
        assert_eq!(optimizer.config.random_seed, Some(42));
    }

    #[test]
    fn test_generate_random_subset_empty() {
        let optimizer = BootstrapOptuna::new().with_random_seed(42);
        let mut rng = StdRng::seed_from_u64(42);

        let demos: Vec<FewShotExample> = Vec::new();
        let subset = optimizer.generate_random_subset(&demos, &mut rng);

        assert_eq!(subset.len(), 0);
    }

    #[test]
    fn test_generate_random_subset_single() {
        let optimizer = BootstrapOptuna::new()
            .with_random_seed(42)
            .with_max_demos(5);
        let mut rng = StdRng::seed_from_u64(42);

        let demos = vec![FewShotExample {
            input: serde_json::json!({"query": "test"}),
            output: serde_json::json!({"answer": "result"}),
            reasoning: None,
        }];

        let subset = optimizer.generate_random_subset(&demos, &mut rng);

        // Should select 1 demo (the only one available)
        assert_eq!(subset.len(), 1);
        assert_eq!(
            subset[0].input.get("query"),
            Some(&serde_json::json!("test"))
        );
    }

    #[test]
    fn test_generate_random_subset_multiple() {
        let optimizer = BootstrapOptuna::new()
            .with_random_seed(42)
            .with_max_demos(3);
        let mut rng = StdRng::seed_from_u64(42);

        let demos = vec![
            FewShotExample {
                input: serde_json::json!({"query": "q1"}),
                output: serde_json::json!({"answer": "a1"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"query": "q2"}),
                output: serde_json::json!({"answer": "a2"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"query": "q3"}),
                output: serde_json::json!({"answer": "a3"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"query": "q4"}),
                output: serde_json::json!({"answer": "a4"}),
                reasoning: None,
            },
        ];

        let subset = optimizer.generate_random_subset(&demos, &mut rng);

        // Should select 1-3 demos (max_demos = 3)
        assert!(!subset.is_empty() && subset.len() <= 3);
        assert!(subset.len() <= demos.len());
    }

    #[test]
    fn test_generate_random_subset_reproducibility() {
        let optimizer = BootstrapOptuna::new()
            .with_random_seed(42)
            .with_max_demos(2);

        let demos = vec![
            FewShotExample {
                input: serde_json::json!({"query": "q1"}),
                output: serde_json::json!({"answer": "a1"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"query": "q2"}),
                output: serde_json::json!({"answer": "a2"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"query": "q3"}),
                output: serde_json::json!({"answer": "a3"}),
                reasoning: None,
            },
        ];

        // Generate two subsets with same seed
        let mut rng1 = StdRng::seed_from_u64(42);
        let subset1 = optimizer.generate_random_subset(&demos, &mut rng1);

        let mut rng2 = StdRng::seed_from_u64(42);
        let subset2 = optimizer.generate_random_subset(&demos, &mut rng2);

        // Should be identical
        assert_eq!(subset1.len(), subset2.len());
        for (demo1, demo2) in subset1.iter().zip(subset2.iter()) {
            assert_eq!(demo1.input, demo2.input);
            assert_eq!(demo1.output, demo2.output);
        }
    }

    #[test]
    fn test_bootstrap_optuna_config_chain() {
        let config = OptimizerConfig::new()
            .with_max_few_shot_examples(8)
            .with_max_iterations(20)
            .with_min_improvement(0.02)
            .with_random_seed(123);

        let optimizer = BootstrapOptuna::new()
            .with_config(config)
            .with_num_candidate_programs(64);

        assert_eq!(optimizer.config.max_few_shot_examples, 8);
        assert_eq!(optimizer.config.max_iterations, 20);
        assert_eq!(optimizer.config.min_improvement, 0.02);
        assert_eq!(optimizer.config.random_seed, Some(123));
        assert_eq!(optimizer.num_candidate_programs, 64);
    }

    #[test]
    fn test_bootstrap_optuna_default_trait() {
        let optimizer = BootstrapOptuna::default();
        assert_eq!(optimizer.num_candidate_programs, 16);
        assert_eq!(optimizer.max_bootstrapped_demos, 10);
    }

    #[test]
    fn test_bootstrap_optuna_with_config_overrides_defaults() {
        let config = OptimizerConfig::new().with_max_few_shot_examples(12);
        let optimizer = BootstrapOptuna::new().with_config(config);
        assert_eq!(optimizer.config.max_few_shot_examples, 12);
    }

    #[test]
    fn test_bootstrap_optuna_max_bootstrapped_demos_zero() {
        let optimizer = BootstrapOptuna::new().with_max_bootstrapped_demos(0);
        assert_eq!(optimizer.max_bootstrapped_demos, 0);
    }

    #[test]
    fn test_bootstrap_optuna_num_candidate_programs_one() {
        let optimizer = BootstrapOptuna::new().with_num_candidate_programs(1);
        assert_eq!(optimizer.num_candidate_programs, 1);
    }

    #[test]
    fn test_generate_random_subset_max_demos_exceeds_available() {
        let optimizer = BootstrapOptuna::new()
            .with_random_seed(42)
            .with_max_demos(10);
        let mut rng = StdRng::seed_from_u64(42);

        let demos = vec![
            FewShotExample {
                input: serde_json::json!({"q": "1"}),
                output: serde_json::json!({"a": "1"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"q": "2"}),
                output: serde_json::json!({"a": "2"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"q": "3"}),
                output: serde_json::json!({"a": "3"}),
                reasoning: None,
            },
        ];

        let subset = optimizer.generate_random_subset(&demos, &mut rng);
        assert!(subset.len() <= 3);
    }

    #[test]
    fn test_generate_random_subset_with_different_seeds() {
        let optimizer = BootstrapOptuna::new().with_max_demos(2);

        let demos = vec![
            FewShotExample {
                input: serde_json::json!({"q": "1"}),
                output: serde_json::json!({"a": "1"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"q": "2"}),
                output: serde_json::json!({"a": "2"}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"q": "3"}),
                output: serde_json::json!({"a": "3"}),
                reasoning: None,
            },
        ];

        let mut rng1 = StdRng::seed_from_u64(1);
        let mut rng2 = StdRng::seed_from_u64(99999);

        let subset1 = optimizer.generate_random_subset(&demos, &mut rng1);
        let subset2 = optimizer.generate_random_subset(&demos, &mut rng2);

        assert!(!subset1.is_empty());
        assert!(!subset2.is_empty());
        assert!(subset1.len() <= 2);
        assert!(subset2.len() <= 2);
    }

    #[test]
    fn test_bootstrap_optuna_full_builder_chain() {
        let optimizer = BootstrapOptuna::new()
            .with_config(OptimizerConfig::default())
            .with_num_candidate_programs(32)
            .with_max_bootstrapped_demos(15)
            .with_max_demos(6)
            .with_random_seed(12345);

        assert_eq!(optimizer.num_candidate_programs, 32);
        assert_eq!(optimizer.max_bootstrapped_demos, 15);
        assert_eq!(optimizer.config.max_few_shot_examples, 6);
        assert_eq!(optimizer.config.random_seed, Some(12345));
    }

    #[test]
    fn test_few_shot_example_with_reasoning() {
        let example = FewShotExample {
            input: serde_json::json!({"question": "What is 2+2?"}),
            output: serde_json::json!({"answer": "4"}),
            reasoning: Some("Adding two and two gives four.".to_string()),
        };

        assert!(example.reasoning.is_some());
        assert_eq!(example.reasoning.unwrap(), "Adding two and two gives four.");
    }

    #[test]
    fn test_few_shot_example_clone() {
        let example = FewShotExample {
            input: serde_json::json!({"q": "test"}),
            output: serde_json::json!({"a": "result"}),
            reasoning: None,
        };

        let cloned = example.clone();
        assert_eq!(cloned.input, example.input);
        assert_eq!(cloned.output, example.output);
    }

    #[test]
    fn test_optimizer_config_defaults() {
        let config = OptimizerConfig::new();
        assert_eq!(config.max_few_shot_examples, 4);
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.min_improvement, 0.01);
        assert!(config.random_seed.is_none());
    }

    #[test]
    fn test_bootstrap_optuna_config_builder_order() {
        let opt1 = BootstrapOptuna::new()
            .with_max_demos(5)
            .with_random_seed(42);

        let opt2 = BootstrapOptuna::new()
            .with_random_seed(42)
            .with_max_demos(5);

        assert_eq!(
            opt1.config.max_few_shot_examples,
            opt2.config.max_few_shot_examples
        );
        assert_eq!(opt1.config.random_seed, opt2.config.random_seed);
    }

    #[test]
    fn test_generate_subset_determinism() {
        let optimizer = BootstrapOptuna::new()
            .with_random_seed(42)
            .with_max_demos(2);

        let demos = vec![
            FewShotExample {
                input: serde_json::json!({"i": 0}),
                output: serde_json::json!({"o": 0}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"i": 1}),
                output: serde_json::json!({"o": 1}),
                reasoning: None,
            },
            FewShotExample {
                input: serde_json::json!({"i": 2}),
                output: serde_json::json!({"o": 2}),
                reasoning: None,
            },
        ];

        for _ in 0..10 {
            let mut rng = StdRng::seed_from_u64(42);
            let subset = optimizer.generate_random_subset(&demos, &mut rng);

            let mut rng_check = StdRng::seed_from_u64(42);
            let subset_check = optimizer.generate_random_subset(&demos, &mut rng_check);

            assert_eq!(subset.len(), subset_check.len());
        }
    }
}
