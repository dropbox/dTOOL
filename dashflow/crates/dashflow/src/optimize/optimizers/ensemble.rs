// Allow clippy warnings for ensemble optimizer
// - needless_pass_by_value: Ensemble members passed by value for parallel execution
#![allow(clippy::needless_pass_by_value)]

//! # Ensemble Optimizer
//!
//! Combines multiple optimized programs/graphs into an ensemble that runs all of them
//! and optionally reduces their outputs using a combination function.
//!
//! ## Algorithm
//! 1. Takes a list of already-compiled/optimized graphs
//! 2. For each input, runs all graphs (or a random subset if `size` is specified)
//! 3. Collects all outputs
//! 4. Optionally applies a reduce function (e.g., majority voting, averaging)
//!
//! ## Use Cases
//! - **Majority voting**: Combine multiple classifiers for more robust predictions
//! - **Averaging**: Average numeric outputs from multiple models
//! - **Validation**: Compare outputs from different optimization strategies
//!
//! ## Adapted from DashOptimize
//! Based on DashOptimize's Ensemble teleprompter (dashoptimize/teleprompt/ensemble.py).
//! Simplified for DashFlow StateGraph integration - works with graphs instead of Module.
//!
//! ## References
//!
//! - **Concept**: Standard ensemble learning technique
//! - **Source**: DSPy teleprompt ensemble.py
//! - **Link**: <https://github.com/stanfordnlp/dspy/blob/main/dspy/teleprompt/>

use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::state::GraphState;
use crate::Result;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::marker::PhantomData;

/// Reduce function type - combines multiple outputs into one
pub type ReduceFn<S> = Box<dyn Fn(Vec<S>) -> S + Send + Sync>;

/// Ensemble optimizer configuration
#[derive(Default)]
pub struct EnsembleBuilder<S>
where
    S: GraphState,
{
    /// Optional reduce function to combine outputs
    reduce_fn: Option<ReduceFn<S>>,
    /// Optional size limit - randomly sample this many graphs per execution
    size: Option<usize>,
    /// Whether to use deterministic sampling (fixed seed=42 for reproducibility)
    deterministic: bool,
    _phantom: PhantomData<S>,
}

impl<S> EnsembleBuilder<S>
where
    S: GraphState,
{
    /// Create a new ensemble builder
    pub fn new() -> Self {
        Self {
            reduce_fn: None,
            size: None,
            deterministic: false,
            _phantom: PhantomData,
        }
    }

    /// Set the reduce function to combine outputs
    ///
    /// Common reduce functions:
    /// - Majority voting for classification
    /// - Averaging for numeric outputs
    /// - Max/min for ranking
    #[must_use]
    pub fn with_reduce_fn<F>(mut self, reduce_fn: F) -> Self
    where
        F: Fn(Vec<S>) -> S + Send + Sync + 'static,
    {
        self.reduce_fn = Some(Box::new(reduce_fn));
        self
    }

    /// Limit the number of graphs used per execution
    ///
    /// If specified, randomly samples `size` graphs from the ensemble.
    /// Useful for reducing computation cost while maintaining diversity.
    #[must_use]
    pub fn with_size(mut self, size: usize) -> Self {
        self.size = Some(size);
        self
    }

    /// Enable deterministic sampling
    ///
    /// When deterministic mode is enabled and a size limit is set, the ensemble
    /// will use a fixed seed for random sampling. This ensures reproducible results
    /// across multiple runs with the same configuration.
    ///
    /// For truly input-dependent determinism (same input → same graphs selected),
    /// you would need to hash the input state and use that as a seed. This basic
    /// implementation uses a fixed seed for reproducibility across runs.
    #[must_use]
    pub fn with_deterministic(mut self, deterministic: bool) -> Self {
        self.deterministic = deterministic;
        self
    }

    /// Build the ensemble
    ///
    /// # Arguments
    /// * `graphs` - List of already-optimized graphs to ensemble
    ///
    /// # Returns
    /// An `Ensemble` instance that can execute the ensemble
    ///
    /// # Panics
    /// Warns (via tracing) if `graphs` is empty, as ensemble will produce no outputs.
    pub fn build(self, graphs: Vec<Box<dyn Fn(S) -> Result<S> + Send + Sync>>) -> Ensemble<S> {
        if graphs.is_empty() {
            tracing::warn!(
                "Ensemble created with no graphs - execute() will return error, execute_all() will return empty vec"
            );
        }
        if let Some(size) = self.size {
            if size > graphs.len() {
                tracing::warn!(
                    size = size,
                    graphs_count = graphs.len(),
                    "Ensemble size ({}) exceeds graph count ({}); will use all {} graphs",
                    size,
                    graphs.len(),
                    graphs.len()
                );
            }
        }
        Ensemble {
            graphs,
            reduce_fn: self.reduce_fn,
            size: self.size,
            deterministic: self.deterministic,
            _phantom: PhantomData,
        }
    }
}

/// Ensemble of multiple graphs
///
/// Executes multiple graphs on the same input and optionally combines their outputs.
pub struct Ensemble<S>
where
    S: GraphState,
{
    /// List of graph execution functions
    graphs: Vec<Box<dyn Fn(S) -> Result<S> + Send + Sync>>,
    /// Optional reduce function
    reduce_fn: Option<ReduceFn<S>>,
    /// Optional size limit for random sampling
    size: Option<usize>,
    /// Whether to use deterministic sampling (fixed seed for reproducibility)
    deterministic: bool,
    _phantom: PhantomData<S>,
}

impl<S> Ensemble<S>
where
    S: GraphState,
{
    /// Create a new ensemble builder
    pub fn builder() -> EnsembleBuilder<S> {
        EnsembleBuilder::new()
    }

    /// Execute the ensemble on an input
    ///
    /// # Arguments
    /// * `input` - The input state
    ///
    /// # Returns
    /// - If `reduce_fn` is set: A single reduced output
    /// - If `reduce_fn` is None: Returns the first graph's output
    /// - If ensemble is empty: Returns error
    ///
    /// # Notes
    /// For all outputs without reduction, use `execute_all()` instead.
    pub fn execute(&self, input: S) -> Result<S> {
        let outputs = self.execute_all(input)?;

        if let Some(ref reduce_fn) = self.reduce_fn {
            Ok(reduce_fn(outputs))
        } else {
            // If no reduce function, return the first output
            // (multi-output requires different API)
            outputs
                .into_iter()
                .next()
                .ok_or_else(|| crate::Error::Generic("Ensemble returned no outputs".to_string()))
        }
    }

    /// Execute the ensemble and return all outputs (no reduction)
    ///
    /// # Arguments
    /// * `input` - The input state
    ///
    /// # Returns
    /// Vector of outputs from each graph in the ensemble
    pub fn execute_all(&self, input: S) -> Result<Vec<S>> {
        use std::time::Instant;
        let start = Instant::now();

        // Record telemetry start
        record_optimization_start("ensemble");

        // Select graphs to run
        let graphs_to_run: Vec<_> = if let Some(size) = self.size {
            let mut indices: Vec<usize> = (0..self.graphs.len()).collect();
            if self.deterministic {
                // Deterministic sampling with fixed seed for reproducibility
                let mut rng = StdRng::seed_from_u64(42);
                indices.shuffle(&mut rng);
            } else {
                // Random sampling
                let mut rng = rand::thread_rng();
                indices.shuffle(&mut rng);
            }
            indices.into_iter().take(size).collect()
        } else {
            // Run all graphs
            (0..self.graphs.len()).collect()
        };

        // Execute selected graphs
        let mut outputs = Vec::new();
        for idx in &graphs_to_run {
            // Record iteration telemetry
            record_iteration("ensemble");

            let graph_fn = &self.graphs[*idx];
            let output = graph_fn(input.clone())?;
            outputs.push(output);
        }

        // Record telemetry completion
        // For ensemble, iterations = graphs run, candidates = total graphs
        record_optimization_complete(
            "ensemble",
            graphs_to_run.len() as u64,
            self.graphs.len() as u64,
            0.0, // No initial score for ensemble execution
            1.0, // Success rate (we don't have scoring in ensemble)
            start.elapsed().as_secs_f64(),
        );

        Ok(outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
    }
    // GraphState is automatically implemented via blanket impl

    // Mock graph function that adds a constant to the value
    fn make_adder(add: i32) -> Box<dyn Fn(TestState) -> Result<TestState> + Send + Sync> {
        Box::new(move |mut state: TestState| {
            state.value += add;
            Ok(state)
        })
    }

    #[test]
    fn test_ensemble_execute_all_without_reduction() {
        // Create ensemble with 3 graphs that add 1, 2, 3 respectively
        let graphs = vec![make_adder(1), make_adder(2), make_adder(3)];

        let ensemble = Ensemble::builder().build(graphs);

        let input = TestState { value: 10 };
        let outputs = ensemble.execute_all(input).unwrap();

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].value, 11); // 10 + 1
        assert_eq!(outputs[1].value, 12); // 10 + 2
        assert_eq!(outputs[2].value, 13); // 10 + 3
    }

    #[test]
    fn test_ensemble_with_reduction() {
        // Create ensemble with 3 graphs
        let graphs = vec![make_adder(1), make_adder(2), make_adder(3)];

        // Average reduce function
        let reduce_fn = |outputs: Vec<TestState>| {
            let sum: i32 = outputs.iter().map(|s| s.value).sum();
            let avg = sum / outputs.len() as i32;
            TestState { value: avg }
        };

        let ensemble = Ensemble::builder().with_reduce_fn(reduce_fn).build(graphs);

        let input = TestState { value: 10 };
        let output = ensemble.execute(input).unwrap();

        // Average of (11, 12, 13) = 36 / 3 = 12
        assert_eq!(output.value, 12);
    }

    #[test]
    fn test_ensemble_with_size_limitation() {
        // Create ensemble with 5 graphs
        let graphs = vec![
            make_adder(1),
            make_adder(2),
            make_adder(3),
            make_adder(4),
            make_adder(5),
        ];

        let ensemble = Ensemble::builder().with_size(2).build(graphs);

        let input = TestState { value: 10 };
        let outputs = ensemble.execute_all(input).unwrap();

        // Should only run 2 graphs
        assert_eq!(outputs.len(), 2);

        // Values should be between 11-15 (10 + 1 through 10 + 5)
        for output in outputs {
            assert!(output.value >= 11 && output.value <= 15);
        }
    }

    #[test]
    fn test_ensemble_deterministic_sampling() {
        // Create ensemble with deterministic sampling
        let graphs = vec![
            make_adder(1),
            make_adder(2),
            make_adder(3),
            make_adder(4),
            make_adder(5),
        ];

        let ensemble = Ensemble::builder()
            .with_deterministic(true)
            .with_size(2)
            .build(graphs);

        let input = TestState { value: 10 };

        // Run multiple times - deterministic mode should give same results
        let outputs1 = ensemble.execute_all(input.clone()).unwrap();
        let outputs2 = ensemble.execute_all(input.clone()).unwrap();

        // Results should be identical with deterministic mode
        assert_eq!(outputs1.len(), outputs2.len());
        for (o1, o2) in outputs1.iter().zip(outputs2.iter()) {
            assert_eq!(o1.value, o2.value);
        }
    }

    #[test]
    fn test_ensemble_majority_voting() {
        // Simulate classification with majority voting
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct ClassState {
            prediction: String,
        }
        // GraphState is automatically implemented via blanket impl

        // Three classifiers that predict different classes
        let classifier1: Box<dyn Fn(ClassState) -> Result<ClassState> + Send + Sync> =
            Box::new(|_| {
                Ok(ClassState {
                    prediction: "A".to_string(),
                })
            });
        let classifier2: Box<dyn Fn(ClassState) -> Result<ClassState> + Send + Sync> =
            Box::new(|_| {
                Ok(ClassState {
                    prediction: "A".to_string(),
                })
            });
        let classifier3: Box<dyn Fn(ClassState) -> Result<ClassState> + Send + Sync> =
            Box::new(|_| {
                Ok(ClassState {
                    prediction: "B".to_string(),
                })
            });

        let graphs = vec![classifier1, classifier2, classifier3];

        // Majority voting reduce function with deterministic tie-breaking
        // (alphabetically first prediction wins ties for reproducibility)
        let reduce_fn = |outputs: Vec<ClassState>| {
            use std::collections::HashMap;
            let mut counts: HashMap<String, usize> = HashMap::new();
            for output in outputs {
                *counts.entry(output.prediction).or_insert(0) += 1;
            }
            // Sort by (count DESC, prediction ASC) for deterministic tie-breaking
            let mut sorted: Vec<_> = counts.into_iter().collect();
            sorted.sort_by(|(pred_a, count_a), (pred_b, count_b)| {
                count_b.cmp(count_a).then_with(|| pred_a.cmp(pred_b))
            });
            let majority = sorted.into_iter().next().map(|(pred, _)| pred).unwrap();
            ClassState {
                prediction: majority,
            }
        };

        let ensemble = Ensemble::builder().with_reduce_fn(reduce_fn).build(graphs);

        let input = ClassState {
            prediction: String::new(),
        };
        let output = ensemble.execute(input).unwrap();

        // Majority is "A" (2 out of 3)
        assert_eq!(output.prediction, "A");
    }

    #[test]
    fn test_empty_ensemble_execute_all_returns_empty() {
        let graphs: Vec<Box<dyn Fn(TestState) -> Result<TestState> + Send + Sync>> = vec![];
        let ensemble = Ensemble::builder().build(graphs);

        let input = TestState { value: 10 };
        let outputs = ensemble.execute_all(input).unwrap();

        // Empty ensemble produces no outputs
        assert!(outputs.is_empty());
    }

    #[test]
    fn test_empty_ensemble_execute_returns_error() {
        let graphs: Vec<Box<dyn Fn(TestState) -> Result<TestState> + Send + Sync>> = vec![];
        let ensemble = Ensemble::builder().build(graphs);

        let input = TestState { value: 10 };
        let result = ensemble.execute(input);

        // Empty ensemble without reduce_fn should return error
        assert!(result.is_err());
    }
}
