//! # LabeledFewShot Optimizer
//!
//! Uses pre-labeled training examples as few-shot demonstrations.
//!
//! ## Algorithm
//! 1. Take k examples from the training set
//! 2. Either sample randomly or take the first k examples
//! 3. Convert them to few-shot demonstrations
//! 4. Return them for use in prompts
//!
//! ## Comparison to BootstrapFewShot
//! - **BootstrapFewShot**: Runs the model, evaluates, collects successful traces
//! - **LabeledFewShot**: Uses pre-labeled examples directly (faster, no LLM calls)
//!
//! ## When to Use
//! - You have high-quality labeled examples
//! - You want deterministic, cheap optimization
//! - You don't need model-generated reasoning traces
//!
//! ## Ported from DashOptimize
//! Based on `dashoptimize/teleprompt/vanilla.py` - LabeledFewShot class
//!
//! ## References
//!
//! - **Source**: DSPy framework
//! - **Paper**: "DSPy: Compiling Declarative Language Model Calls into Self-Improving Pipelines"
//! - **Link**: <https://arxiv.org/abs/2310.03714>
//! - **Original**: <https://github.com/stanfordnlp/dspy/blob/main/dspy/teleprompt/>

use super::OptimizerConfig;
use crate::optimize::telemetry::{record_optimization_complete, record_optimization_start};
use crate::optimize::FewShotExample;
use crate::state::GraphState;
use crate::Result;
use rand::prelude::*;

/// LabeledFewShot optimizer
///
/// This optimizer directly uses pre-labeled training examples as few-shot
/// demonstrations without running the model or evaluating predictions.
///
/// # Example
/// ```rust,ignore
/// use dashflow::optimize::optimizers::LabeledFewShot;
///
/// let optimizer = LabeledFewShot::new()
///     .with_max_demos(8)
///     .with_sample(true);  // Random sampling
///
/// let demos = optimizer.select_demos(&training_data)?;
/// ```
pub struct LabeledFewShot {
    /// Maximum number of demonstrations to select
    k: usize,

    /// Whether to randomly sample (true) or take first k (false)
    sample: bool,

    /// Random seed for reproducibility (default: 0, matching Python)
    random_seed: u64,
}

impl LabeledFewShot {
    /// Create a new LabeledFewShot optimizer with default settings
    ///
    /// Defaults:
    /// - k = 16 (matches DashOptimize default)
    /// - sample = true (random sampling)
    /// - random_seed = 0 (matches DashOptimize for reproducibility)
    pub fn new() -> Self {
        Self {
            k: 16,
            sample: true,
            random_seed: 0,
        }
    }

    /// Set the maximum number of demonstrations to select
    #[must_use]
    pub fn with_max_demos(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set whether to randomly sample (true) or take first k (false)
    #[must_use]
    pub fn with_sample(mut self, sample: bool) -> Self {
        self.sample = sample;
        self
    }

    /// Set the random seed for reproducibility
    #[must_use]
    pub fn with_random_seed(mut self, seed: u64) -> Self {
        self.random_seed = seed;
        self
    }

    /// Create from OptimizerConfig
    pub fn from_config(config: &OptimizerConfig) -> Self {
        let mut optimizer = Self::new().with_max_demos(config.max_few_shot_examples);

        if let Some(seed) = config.random_seed {
            optimizer = optimizer.with_random_seed(seed);
        }

        optimizer
    }

    /// Select demonstrations from the training set
    ///
    /// # Arguments
    /// * `trainset` - Training examples with input/output fields
    ///
    /// # Returns
    /// Vector of few-shot examples, either sampled randomly or taken sequentially
    ///
    /// # Algorithm (matches DashOptimize vanilla.py)
    /// ```text
    /// if trainset is empty:
    ///     return []
    ///
    /// if sample:
    ///     random.seed(random_seed)
    ///     return random.sample(trainset, min(k, len(trainset)))
    /// else:
    ///     return trainset[:min(k, len(trainset))]
    /// ```
    pub fn select_demos<S>(&self, trainset: &[S]) -> Result<Vec<FewShotExample>>
    where
        S: GraphState,
    {
        use std::time::Instant;
        let start = Instant::now();

        // Record telemetry start
        record_optimization_start("labeled_fewshot");

        // Handle empty training set
        if trainset.is_empty() {
            // Record telemetry completion even for empty case
            record_optimization_complete(
                "labeled_fewshot",
                0,
                0,
                0.0,
                1.0,
                start.elapsed().as_secs_f64(),
            );
            return Ok(Vec::new());
        }

        let num_to_select = self.k.min(trainset.len());

        let selected: Vec<&S> = if self.sample {
            // Random sampling with fixed seed (matches Python's random.Random(0))
            let mut rng = StdRng::seed_from_u64(self.random_seed);
            trainset.choose_multiple(&mut rng, num_to_select).collect()
        } else {
            // Take first k examples
            trainset.iter().take(num_to_select).collect()
        };

        // Convert selected examples to FewShotExample format
        let result: Result<Vec<FewShotExample>> = selected
            .into_iter()
            .map(|example| {
                // Serialize state to JSON
                let json = serde_json::to_value(example).map_err(|e| {
                    crate::Error::Validation(format!("Failed to serialize example: {}", e))
                })?;

                // For now, use the full example as both input and output
                // In practice, this would be split based on the signature's
                // input_fields and output_fields
                Ok(FewShotExample {
                    input: json.clone(),
                    output: json,
                    reasoning: None, // Labeled examples don't have reasoning traces
                })
            })
            .collect();

        // Record telemetry completion
        record_optimization_complete(
            "labeled_fewshot",
            num_to_select as u64,
            trainset.len() as u64,
            0.0, // No initial score for demo selection
            1.0, // Success
            start.elapsed().as_secs_f64(),
        );

        result
    }
}

impl Default for LabeledFewShot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestState {
        question: String,
        answer: String,
    }

    // GraphState is auto-implemented for types that match trait bounds
    // (Clone + Send + Sync + Serialize + Deserialize + 'static)

    #[test]
    fn test_labeled_fewshot_default() {
        let optimizer = LabeledFewShot::new();
        assert_eq!(optimizer.k, 16);
        assert!(optimizer.sample);
        assert_eq!(optimizer.random_seed, 0);
    }

    #[test]
    fn test_labeled_fewshot_builder() {
        let optimizer = LabeledFewShot::new()
            .with_max_demos(8)
            .with_sample(false)
            .with_random_seed(42);

        assert_eq!(optimizer.k, 8);
        assert!(!optimizer.sample);
        assert_eq!(optimizer.random_seed, 42);
    }

    #[test]
    fn test_select_demos_empty_trainset() {
        let optimizer = LabeledFewShot::new();
        let trainset: Vec<TestState> = vec![];

        let demos = optimizer.select_demos(&trainset).unwrap();
        assert_eq!(demos.len(), 0);
    }

    #[test]
    fn test_select_demos_sequential() {
        let optimizer = LabeledFewShot::new().with_max_demos(3).with_sample(false);

        let trainset = vec![
            TestState {
                question: "Q1".to_string(),
                answer: "A1".to_string(),
            },
            TestState {
                question: "Q2".to_string(),
                answer: "A2".to_string(),
            },
            TestState {
                question: "Q3".to_string(),
                answer: "A3".to_string(),
            },
            TestState {
                question: "Q4".to_string(),
                answer: "A4".to_string(),
            },
        ];

        let demos = optimizer.select_demos(&trainset).unwrap();
        assert_eq!(demos.len(), 3);

        // Should take first 3 in order
        assert_eq!(demos[0].input["question"], "Q1");
        assert_eq!(demos[0].output["answer"], "A1");
        assert_eq!(demos[1].input["question"], "Q2");
        assert_eq!(demos[2].input["question"], "Q3");
    }

    #[test]
    fn test_select_demos_sampling() {
        let optimizer = LabeledFewShot::new()
            .with_max_demos(2)
            .with_sample(true)
            .with_random_seed(42);

        let trainset = vec![
            TestState {
                question: "Q1".to_string(),
                answer: "A1".to_string(),
            },
            TestState {
                question: "Q2".to_string(),
                answer: "A2".to_string(),
            },
            TestState {
                question: "Q3".to_string(),
                answer: "A3".to_string(),
            },
            TestState {
                question: "Q4".to_string(),
                answer: "A4".to_string(),
            },
        ];

        let demos = optimizer.select_demos(&trainset).unwrap();
        assert_eq!(demos.len(), 2);

        // With fixed seed, sampling should be deterministic
        // (we don't check exact order since that depends on rand implementation)
        let questions: Vec<String> = demos
            .iter()
            .map(|d| d.input["question"].as_str().unwrap().to_string())
            .collect();

        // Should have 2 distinct questions
        assert_eq!(questions.len(), 2);
    }

    #[test]
    fn test_select_demos_k_larger_than_trainset() {
        let optimizer = LabeledFewShot::new().with_max_demos(10).with_sample(false);

        let trainset = vec![
            TestState {
                question: "Q1".to_string(),
                answer: "A1".to_string(),
            },
            TestState {
                question: "Q2".to_string(),
                answer: "A2".to_string(),
            },
        ];

        let demos = optimizer.select_demos(&trainset).unwrap();
        // Should return all 2 examples, not try to get 10
        assert_eq!(demos.len(), 2);
    }

    #[test]
    fn test_from_config() {
        let config = OptimizerConfig::default()
            .with_max_few_shot_examples(10)
            .with_random_seed(123);

        let optimizer = LabeledFewShot::from_config(&config);

        assert_eq!(optimizer.k, 10);
        assert_eq!(optimizer.random_seed, 123);
    }

    #[test]
    fn test_deterministic_sampling_with_same_seed() {
        let trainset = vec![
            TestState {
                question: "Q1".to_string(),
                answer: "A1".to_string(),
            },
            TestState {
                question: "Q2".to_string(),
                answer: "A2".to_string(),
            },
            TestState {
                question: "Q3".to_string(),
                answer: "A3".to_string(),
            },
            TestState {
                question: "Q4".to_string(),
                answer: "A4".to_string(),
            },
        ];

        let optimizer1 = LabeledFewShot::new()
            .with_max_demos(2)
            .with_sample(true)
            .with_random_seed(0);

        let optimizer2 = LabeledFewShot::new()
            .with_max_demos(2)
            .with_sample(true)
            .with_random_seed(0);

        let demos1 = optimizer1.select_demos(&trainset).unwrap();
        let demos2 = optimizer2.select_demos(&trainset).unwrap();

        // Same seed should produce same samples
        assert_eq!(demos1.len(), demos2.len());

        let questions1: Vec<String> = demos1
            .iter()
            .map(|d| d.input["question"].as_str().unwrap().to_string())
            .collect();
        let questions2: Vec<String> = demos2
            .iter()
            .map(|d| d.input["question"].as_str().unwrap().to_string())
            .collect();

        assert_eq!(questions1, questions2);
    }
}
