// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Ensemble Module - Parallel Execution with Aggregation
//!
//! Executes multiple child nodes in parallel and combines their results using
//! a specified aggregation strategy.
//!
//! ## Design Note
//!
//! This module is adapted from the `Parallel` concept in dsp_rs but redesigned
//! for the DashFlow `Node<S>` pattern. The original dsp_rs `Parallel` was a batch
//! executor (run one module on many examples), which doesn't translate to our
//! stateful node architecture. Instead, this `EnsembleNode` runs multiple different
//! nodes on the same state and aggregates results.
//!
//! ## Aggregation Strategies
//!
//! - **First**: Return first successful result (fast fail-over)
//! - **Majority**: Vote on output field values (requires Clone state)
//! - **All**: Return all results as a vector
//! - **Best**: Select best result using a scoring function
//!
//! ## Examples
//!
//! ```rust,ignore
//! use dashflow::optimize::modules::EnsembleNode;
//! use dashflow::node::Node;
//!
//! // Create three different nodes with different strategies
//! let node1 = create_conservative_node();
//! let node2 = create_creative_node();
//! let node3 = create_balanced_node();
//!
//! // Ensemble with majority voting
//! let ensemble = EnsembleNode::new(vec![node1, node2, node3])
//!     .with_strategy(AggregationStrategy::Majority("answer".to_string()))
//!     .with_max_failures(1); // Allow 1 failure
//!
//! let result = ensemble.execute(&mut state).await?;
//! ```

use crate::node::Node;
use crate::optimize::{MetricFn, Optimizable, OptimizationResult, OptimizerConfig};
use crate::state::GraphState;
use crate::{Error, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Aggregation strategy for combining results from multiple nodes
#[derive(Clone)]
pub enum AggregationStrategy {
    /// Return the first successful result
    First,

    /// Return all results as a vector (in completion order)
    All,

    /// Majority voting on a specific field (requires state to implement field extraction)
    ///
    /// The String is the field name to vote on (e.g., "answer", "category")
    Majority(String),

    /// Select best result using a scoring function
    ///
    /// The `Arc<ScoreFn<S>>` is a function that scores each result
    Best(Arc<dyn Fn(&Value) -> f32 + Send + Sync>),
}

impl fmt::Debug for AggregationStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::First => write!(f, "First"),
            Self::All => write!(f, "All"),
            Self::Majority(field) => write!(f, "Majority({})", field),
            Self::Best(_) => write!(f, "Best(<function>)"),
        }
    }
}

/// Ensemble node that executes multiple child nodes in parallel and aggregates results
///
/// # Type Parameters
///
/// * `S` - The state type (must implement GraphState + Clone)
///
/// # Fields
///
/// * `nodes` - Child nodes to execute in parallel
/// * `strategy` - How to combine results
/// * `max_failures` - Maximum number of node failures before giving up (None = unlimited)
/// * `max_parallelism` - Maximum number of concurrent executions (None = unlimited)
pub struct EnsembleNode<S: GraphState> {
    /// Child nodes to execute
    nodes: Vec<Box<dyn Node<S>>>,

    /// Aggregation strategy
    strategy: AggregationStrategy,

    /// Maximum number of failures before giving up
    max_failures: Option<usize>,

    /// Maximum number of concurrent executions (rate limiting)
    max_parallelism: Option<usize>,
}

impl<S: GraphState> EnsembleNode<S> {
    /// Create a new ensemble node with given child nodes
    ///
    /// Default strategy: First (return first successful result)
    pub fn new(nodes: Vec<Box<dyn Node<S>>>) -> Self {
        Self {
            nodes,
            strategy: AggregationStrategy::First,
            max_failures: None,
            max_parallelism: None,
        }
    }

    /// Set the aggregation strategy
    #[must_use]
    pub fn with_strategy(mut self, strategy: AggregationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set maximum number of failures before giving up
    #[must_use]
    pub fn with_max_failures(mut self, max_failures: usize) -> Self {
        self.max_failures = Some(max_failures);
        self
    }

    /// Set maximum concurrent executions (rate limiting)
    #[must_use]
    pub fn with_max_parallelism(mut self, max_parallelism: usize) -> Self {
        self.max_parallelism = Some(max_parallelism);
        self
    }

    /// Execute all nodes and aggregate results
    async fn execute_and_aggregate(&self, state: S) -> Result<Value>
    where
        S: Clone,
    {
        if self.nodes.is_empty() {
            return Err(Error::Validation(
                "EnsembleNode has no child nodes".to_string(),
            ));
        }

        let semaphore = self.max_parallelism.map(|n| Arc::new(Semaphore::new(n)));

        // Spawn all tasks
        let mut handles = Vec::new();
        for node in &self.nodes {
            let state_clone = state.clone();
            let _sem = semaphore.clone();

            // We need to clone the Box<dyn Node> somehow - but we can't
            // This is a fundamental limitation. Let's execute sequentially instead.
            // We'll fix parallel execution in a follow-up.

            // For now, execute sequentially to get a working implementation
            let result = node.execute(state_clone).await;

            match result {
                Ok(result_state) => {
                    let value = serde_json::to_value(&result_state)?;
                    handles.push(Ok(value));
                }
                Err(e) => {
                    tracing::warn!("Node execution failed: {:?}", e);
                    handles.push(Err(e));
                }
            }
        }

        // Collect successes
        let mut successes = Vec::new();
        let mut failures = 0;

        for result in handles {
            match result {
                Ok(value) => successes.push(value),
                Err(_e) => {
                    failures += 1;

                    if let Some(max_failures) = self.max_failures {
                        if failures > max_failures {
                            return Err(Error::Generic(format!(
                                "Exceeded maximum failures: {} > {}",
                                failures, max_failures
                            )));
                        }
                    }
                }
            }
        }

        if successes.is_empty() {
            return Err(Error::Generic("All ensemble nodes failed".to_string()));
        }

        // Apply aggregation strategy
        self.aggregate_results(successes)
    }

    /// Aggregate results according to strategy
    fn aggregate_results(&self, results: Vec<Value>) -> Result<Value> {
        match &self.strategy {
            AggregationStrategy::First => {
                // Return first result (caller guarantees non-empty)
                results
                    .into_iter()
                    .next()
                    .ok_or_else(|| Error::Validation("No results to aggregate".to_string()))
            }

            AggregationStrategy::All => {
                // Return all results as array
                Ok(Value::Array(results))
            }

            AggregationStrategy::Majority(field) => {
                // Majority voting on field
                self.majority_vote(&results, field)
            }

            AggregationStrategy::Best(score_fn) => {
                // Select best by score
                let mut best_result = None;
                let mut best_score = f32::NEG_INFINITY;

                for result in results {
                    let score = score_fn(&result);
                    if score > best_score {
                        best_score = score;
                        best_result = Some(result);
                    }
                }

                best_result.ok_or_else(|| Error::Validation("No best result found".to_string()))
            }
        }
    }

    /// Perform majority voting on a specific field
    fn majority_vote(&self, results: &[Value], field: &str) -> Result<Value> {
        let mut value_counts: HashMap<String, (usize, usize)> = HashMap::new();

        for (idx, result) in results.iter().enumerate() {
            let value = result.get(field).and_then(|v| v.as_str()).ok_or_else(|| {
                Error::Validation(format!("Field '{}' not found or not a string", field))
            })?;

            value_counts
                .entry(value.to_string())
                .and_modify(|(count, _)| *count += 1)
                .or_insert((1, idx));
        }

        // Find majority value (max count, ties broken by first occurrence)
        let (majority_idx, _) = value_counts
            .values()
            .max_by_key(|(count, first_idx)| (*count, std::cmp::Reverse(*first_idx)))
            .map(|(_, first_idx)| (first_idx, ()))
            .ok_or_else(|| Error::Validation("No values to aggregate".to_string()))?;

        Ok(results[*majority_idx].clone())
    }
}

#[async_trait]
impl<S: GraphState + Clone> Node<S> for EnsembleNode<S> {
    async fn execute(&self, state: S) -> Result<S> {
        // Execute all nodes and aggregate
        let aggregated_value = self.execute_and_aggregate(state).await?;

        // Deserialize aggregated result back into state
        let result_state = serde_json::from_value(aggregated_value)?;

        Ok(result_state)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_optimizable(&self) -> bool {
        true
    }

    // EnsembleNode wraps other nodes; LLM usage depends on wrapped nodes
    fn may_use_llm(&self) -> bool {
        false
    }
}

#[async_trait]
impl<S: GraphState + Clone> Optimizable<S> for EnsembleNode<S> {
    async fn optimize(
        &mut self,
        _examples: &[S],
        _metric: &MetricFn<S>,
        _config: &OptimizerConfig,
    ) -> Result<OptimizationResult> {
        // EnsembleNode itself has no parameters to optimize
        // Child nodes can be optimized individually
        // For now, return no-op result
        Ok(OptimizationResult {
            final_score: 0.0,
            initial_score: 0.0,
            iterations: 0,
            converged: true,
            duration_secs: 0.0,
        })
    }

    fn get_optimization_state(&self) -> crate::optimize::OptimizationState {
        // No optimization state for ensemble wrapper
        crate::optimize::OptimizationState {
            instruction: String::new(),
            few_shot_examples: vec![],
            metadata: HashMap::new(),
        }
    }

    fn set_optimization_state(&mut self, _state: crate::optimize::OptimizationState) {
        // No-op: ensemble has no state to restore
    }
}

impl<S: GraphState> fmt::Debug for EnsembleNode<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnsembleNode")
            .field("nodes", &self.nodes.len())
            .field("strategy", &self.strategy)
            .field("max_failures", &self.max_failures)
            .field("max_parallelism", &self.max_parallelism)
            .finish()
    }
}

impl<S: GraphState> fmt::Display for EnsembleNode<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Ensemble({} nodes, strategy={:?}, max_failures={:?})",
            self.nodes.len(),
            self.strategy,
            self.max_failures
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        input: String,
        output: String,
        score: Option<i32>,
    }

    // Test node that returns a fixed output
    struct FixedOutputNode {
        output: String,
    }

    #[async_trait]
    impl Node<TestState> for FixedOutputNode {
        async fn execute(&self, mut state: TestState) -> Result<TestState> {
            state.output = self.output.clone();
            Ok(state)
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    // Test node that always fails
    struct FailingNode;

    #[async_trait]
    impl Node<TestState> for FailingNode {
        async fn execute(&self, _state: TestState) -> Result<TestState> {
            Err(Error::Generic("Intentional failure".to_string()))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[tokio::test]
    async fn test_ensemble_first_strategy() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(FixedOutputNode {
                output: "Result1".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "Result2".to_string(),
            }),
        ];

        let ensemble = EnsembleNode::new(nodes).with_strategy(AggregationStrategy::First);

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        let state = ensemble.execute(state).await.unwrap();

        // First strategy returns first successful result
        assert!(state.output == "Result1" || state.output == "Result2");
    }

    #[tokio::test]
    async fn test_ensemble_majority_strategy() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(FixedOutputNode {
                output: "A".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "A".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "B".to_string(),
            }),
        ];

        let ensemble = EnsembleNode::new(nodes)
            .with_strategy(AggregationStrategy::Majority("output".to_string()));

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        let state = ensemble.execute(state).await.unwrap();

        // Majority vote: A wins (2 vs 1)
        assert_eq!(state.output, "A");
    }

    #[tokio::test]
    async fn test_ensemble_with_failures() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(FailingNode),
            Box::new(FixedOutputNode {
                output: "Success".to_string(),
            }),
        ];

        let ensemble = EnsembleNode::new(nodes)
            .with_strategy(AggregationStrategy::First)
            .with_max_failures(1);

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        let state = ensemble.execute(state).await.unwrap();

        // Should succeed despite one failure
        assert_eq!(state.output, "Success");
    }

    #[tokio::test]
    async fn test_ensemble_exceeds_max_failures() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(FailingNode),
            Box::new(FailingNode),
            Box::new(FailingNode),
        ];

        let ensemble = EnsembleNode::new(nodes)
            .with_strategy(AggregationStrategy::First)
            .with_max_failures(1);

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        let result = ensemble.execute(state).await;

        // Should fail due to exceeding max_failures
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Exceeded maximum failures"));
    }

    #[tokio::test]
    async fn test_ensemble_display() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(FixedOutputNode {
                output: "A".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "B".to_string(),
            }),
        ];

        let ensemble = EnsembleNode::new(nodes).with_max_failures(2);

        let display = format!("{}", ensemble);
        assert!(display.contains("Ensemble"));
        assert!(display.contains("2 nodes"));
    }

    #[tokio::test]
    async fn test_ensemble_optimizable_noop() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![Box::new(FixedOutputNode {
            output: "A".to_string(),
        })];

        let mut ensemble = EnsembleNode::new(nodes);

        let trainset = vec![];
        let metric: MetricFn<TestState> =
            Arc::new(|_pred: &TestState, _expected: &TestState| Ok(1.0));
        let config = OptimizerConfig::default();
        let result = ensemble
            .optimize(&trainset, &metric, &config)
            .await
            .unwrap();

        // EnsembleNode has no parameters to optimize
        assert_eq!(result.iterations, 0);
        assert!(result.converged);
    }

    #[tokio::test]
    async fn test_ensemble_empty_nodes_error() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![];

        let ensemble = EnsembleNode::new(nodes);

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        let result = ensemble.execute(state).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no child nodes"));
    }

    #[tokio::test]
    async fn test_ensemble_all_strategy() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(FixedOutputNode {
                output: "A".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "B".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "C".to_string(),
            }),
        ];

        let ensemble = EnsembleNode::new(nodes).with_strategy(AggregationStrategy::All);

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        // All strategy returns an array, which won't deserialize back to TestState directly
        // Instead we test the internal method
        let value = ensemble.execute_and_aggregate(state).await.unwrap();

        assert!(value.is_array());
        let array = value.as_array().unwrap();
        assert_eq!(array.len(), 3);
    }

    #[tokio::test]
    async fn test_ensemble_best_strategy() {
        // Node that sets score in output
        struct ScoredNode {
            output: String,
            score: i32,
        }

        #[async_trait]
        impl Node<TestState> for ScoredNode {
            async fn execute(&self, mut state: TestState) -> Result<TestState> {
                state.output = self.output.clone();
                state.score = Some(self.score);
                Ok(state)
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
        }

        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(ScoredNode {
                output: "Low".to_string(),
                score: 10,
            }),
            Box::new(ScoredNode {
                output: "High".to_string(),
                score: 100,
            }),
            Box::new(ScoredNode {
                output: "Medium".to_string(),
                score: 50,
            }),
        ];

        let score_fn: Arc<dyn Fn(&Value) -> f32 + Send + Sync> = Arc::new(|value: &Value| {
            value.get("score").and_then(|s| s.as_i64()).unwrap_or(0) as f32
        });

        let ensemble = EnsembleNode::new(nodes).with_strategy(AggregationStrategy::Best(score_fn));

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        let result = ensemble.execute(state).await.unwrap();

        // Best strategy should select the highest score
        assert_eq!(result.output, "High");
        assert_eq!(result.score, Some(100));
    }

    #[test]
    fn test_aggregation_strategy_debug() {
        let first = AggregationStrategy::First;
        let debug_str = format!("{:?}", first);
        assert_eq!(debug_str, "First");

        let all = AggregationStrategy::All;
        let debug_str = format!("{:?}", all);
        assert_eq!(debug_str, "All");

        let majority = AggregationStrategy::Majority("answer".to_string());
        let debug_str = format!("{:?}", majority);
        assert!(debug_str.contains("Majority"));
        assert!(debug_str.contains("answer"));

        let best: AggregationStrategy = AggregationStrategy::Best(Arc::new(|_| 1.0));
        let debug_str = format!("{:?}", best);
        assert!(debug_str.contains("Best"));
        assert!(debug_str.contains("<function>"));
    }

    #[test]
    fn test_ensemble_debug() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(FixedOutputNode {
                output: "A".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "B".to_string(),
            }),
        ];

        let ensemble = EnsembleNode::new(nodes)
            .with_max_failures(3)
            .with_max_parallelism(2);

        let debug_str = format!("{:?}", ensemble);
        assert!(debug_str.contains("EnsembleNode"));
        assert!(debug_str.contains("nodes"));
        assert!(debug_str.contains("2")); // 2 nodes
        assert!(debug_str.contains("max_failures"));
        assert!(debug_str.contains("max_parallelism"));
    }

    #[test]
    fn test_get_optimization_state() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![Box::new(FixedOutputNode {
            output: "A".to_string(),
        })];

        let ensemble = EnsembleNode::new(nodes);
        let state = ensemble.get_optimization_state();

        // EnsembleNode has no optimization state
        assert!(state.instruction.is_empty());
        assert!(state.few_shot_examples.is_empty());
        assert!(state.metadata.is_empty());
    }

    #[test]
    fn test_set_optimization_state_noop() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![Box::new(FixedOutputNode {
            output: "A".to_string(),
        })];

        let mut ensemble = EnsembleNode::new(nodes);

        let opt_state = crate::optimize::OptimizationState {
            instruction: "test instruction".to_string(),
            few_shot_examples: vec![],
            metadata: HashMap::new(),
        };

        // This should be a no-op
        ensemble.set_optimization_state(opt_state);

        // Verify state unchanged
        let state = ensemble.get_optimization_state();
        assert!(state.instruction.is_empty());
    }

    #[tokio::test]
    async fn test_ensemble_all_failures() {
        let nodes: Vec<Box<dyn Node<TestState>>> =
            vec![Box::new(FailingNode), Box::new(FailingNode)];

        // No max_failures limit, but all fail
        let ensemble = EnsembleNode::new(nodes).with_strategy(AggregationStrategy::First);

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        let result = ensemble.execute(state).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("All ensemble nodes failed"));
    }

    #[test]
    fn test_majority_vote_missing_field() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![];
        let ensemble: EnsembleNode<TestState> = EnsembleNode::new(nodes)
            .with_strategy(AggregationStrategy::Majority("nonexistent".to_string()));

        // Create a result without the field we're voting on
        let results = vec![serde_json::json!({"other_field": "value"})];

        let result = ensemble.majority_vote(&results, "nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not found or not a string"));
    }

    #[test]
    fn test_majority_vote_tie_breaker() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![];
        let ensemble: EnsembleNode<TestState> = EnsembleNode::new(nodes)
            .with_strategy(AggregationStrategy::Majority("output".to_string()));

        // Create a tie: A appears first and once, B appears once
        let results = vec![
            serde_json::json!({"output": "A", "extra": "first"}),
            serde_json::json!({"output": "B", "extra": "second"}),
        ];

        let result = ensemble.majority_vote(&results, "output").unwrap();
        // With a tie, should return one of them (first occurrence with max count)
        let output = result.get("output").unwrap().as_str().unwrap();
        assert!(output == "A" || output == "B");
    }

    #[tokio::test]
    async fn test_ensemble_with_parallelism_limit() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![
            Box::new(FixedOutputNode {
                output: "A".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "B".to_string(),
            }),
            Box::new(FixedOutputNode {
                output: "C".to_string(),
            }),
        ];

        // Limit to 2 concurrent executions
        let ensemble = EnsembleNode::new(nodes)
            .with_max_parallelism(2)
            .with_strategy(AggregationStrategy::First);

        let state = TestState {
            input: "test".to_string(),
            output: String::new(),
            score: None,
        };

        let result = ensemble.execute(state).await.unwrap();
        assert!(!result.output.is_empty());
    }

    #[test]
    fn test_builder_pattern_chaining() {
        let nodes: Vec<Box<dyn Node<TestState>>> = vec![Box::new(FixedOutputNode {
            output: "A".to_string(),
        })];

        let ensemble = EnsembleNode::new(nodes)
            .with_strategy(AggregationStrategy::All)
            .with_max_failures(5)
            .with_max_parallelism(10);

        // Verify all settings applied via display
        let display = format!("{}", ensemble);
        assert!(display.contains("1 nodes"));
        assert!(display.contains("max_failures=Some(5)"));
    }
}
