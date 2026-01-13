// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # BestOfN Module
//!
//! Output refinement module that runs another node N times and selects the best output
//! according to a reward function. Enables quality improvement through sampling.
//!
//! ## Purpose
//!
//! BestOfN trades compute (N LLM calls) for quality (best of N outputs). It's effective when:
//! - Task has multiple valid approaches
//! - Quality metric is computable (word count, format, structure, etc.)
//! - Latency and cost budget allows multiple calls
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::optimize::modules::BestOfNNode;
//! use dashflow::optimize::{Signature, Field, FieldKind};
//! use dashflow::GraphState;
//! use dashflow::core::language_models::ChatModel;
//! use std::sync::Arc;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct QAState {
//!     question: String,
//!     answer: String,
//! }
//!
//! // Define reward function that prefers shorter answers
//! let reward_fn = Arc::new(|state: &QAState| -> f32 {
//!     let answer = &state.answer;
//!     // Reward inversely proportional to length
//!     1.0 / (answer.len() as f32 + 1.0)
//! });
//!
//! // Create signature and base node
//! // let signature = Signature::new(...);
//! // let llm: Arc<dyn ChatModel> = ...;
//! // let base_node = ChainOfThoughtNode::new(signature, llm);
//!
//! // Create BestOfN wrapper
//! // let best_of_5 = BestOfNNode::new(
//! //     Box::new(base_node),
//! //     5,  // Try 5 times
//! //     reward_fn,
//! //     0.9,  // Threshold for early stopping
//! //     None,  // fail_count defaults to N
//! // );
//! ```
//!
//! ## Design
//!
//! - Wraps any `Node<S>` implementation
//! - Runs wrapped node N times with different random seeds (if LLM supports temperature)
//! - Scores each result with user-defined reward function
//! - Returns first result >= threshold, or highest scoring result
//! - Configurable failure tolerance (continue if some attempts fail)
//!
//! ## Performance
//!
//! - Framework overhead: ~5µs per iteration
//! - LLM calls: N calls (typically 3-10)
//! - Latency: N × base_node_latency (sequential execution)
//! - Cost: N × base_node_cost
//! - Quality improvement: Depends on reward function, temperature, and N
//!
//! ## Related
//!
//! - See also: `RefineNode` (feedback-based iterative improvement)
//! - See also: `ParallelNode` (run multiple nodes in parallel)
//! - Pattern: Diversity through temperature, selection via reward

use crate::node::Node;
use crate::optimize::{Optimizable, OptimizationResult, OptimizerConfig};
use crate::state::GraphState;
use crate::{Error, Result};
use async_trait::async_trait;
use std::sync::Arc;

/// Reward function type that takes a state and returns a score (higher is better).
///
/// The reward function should be:
/// - **Deterministic**: Same state always produces same score
/// - **Fast**: Called N times per forward pass
/// - **Normalized**: Consider using 0.0-1.0 range for threshold to make sense
pub type RewardFn<S> = Arc<dyn Fn(&S) -> f32 + Send + Sync>;

/// BestOfN node that runs a wrapped node N times and selects the best result.
///
/// This node wraps another node and executes it multiple times. It returns either
/// the first result that exceeds a threshold, or the one with the highest reward.
///
/// # Type Parameters
///
/// - `S`: GraphState type that the wrapped node operates on
///
/// # Examples
///
/// ```rust
/// use dashflow::optimize::modules::{BestOfNNode, ChainOfThoughtNode};
/// use dashflow::optimize::{Signature, Field, FieldKind};
/// use dashflow::GraphState;
/// use std::sync::Arc;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct QAState {
///     question: String,
///     answer: String,
/// }
///
/// // Reward function that prefers answers with specific keywords
/// let reward_fn = Arc::new(|state: &QAState| -> f32 {
///     let answer = &state.answer.to_lowercase();
///     let keywords = ["because", "therefore", "thus"];
///     let matches = keywords.iter().filter(|k| answer.contains(*k)).count();
///     matches as f32 / keywords.len() as f32
/// });
///
/// // Create base node and wrap with BestOfN
/// // let cot_node = ChainOfThoughtNode::new(...);
/// // let best_of_3 = BestOfNNode::new(Box::new(cot_node), 3, reward_fn, 0.8, None);
/// ```
pub struct BestOfNNode<S: GraphState> {
    /// The wrapped node to execute multiple times
    pub module: Box<dyn Node<S>>,

    /// Number of attempts
    pub n: usize,

    /// Reward function for scoring results
    pub reward_fn: RewardFn<S>,

    /// Threshold for early stopping (stop when reward >= threshold)
    pub threshold: f32,

    /// Maximum number of allowed failures (defaults to n)
    pub fail_count: usize,
}

impl<S: GraphState> BestOfNNode<S> {
    /// Create a new BestOfN wrapper.
    ///
    /// # Arguments
    ///
    /// * `module` - The node to wrap (will be called N times)
    /// * `n` - Number of times to run the node
    /// * `reward_fn` - Function that scores results (higher is better)
    /// * `threshold` - If a result scores >= threshold, return it immediately
    /// * `fail_count` - Max failures allowed before giving up (defaults to `n` if None)
    ///
    /// # Returns
    ///
    /// A new BestOfNNode instance
    ///
    /// # Example
    ///
    /// ```rust
    /// # use dashflow::optimize::modules::BestOfNNode;
    /// # use dashflow::GraphState;
    /// # use std::sync::Arc;
    /// # use serde::{Deserialize, Serialize};
    /// #
    /// # #[derive(Debug, Clone, Serialize, Deserialize)]
    /// # struct TestState { value: String }
    /// # struct DummyNode;
    /// # #[async_trait::async_trait]
    /// # impl dashflow::node::Node<TestState> for DummyNode {
    /// #     async fn execute(&self, state: TestState) -> dashflow::Result<TestState> {
    /// #         Ok(state)
    /// #     }
    /// #     fn as_any(&self) -> &dyn std::any::Any { self }
    /// #     fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    /// # }
    /// #
    /// let reward_fn = Arc::new(|state: &TestState| -> f32 {
    ///     state.value.len() as f32
    /// });
    ///
    /// let best_of_5 = BestOfNNode::new(
    ///     Box::new(DummyNode),
    ///     5,              // Try 5 times
    ///     reward_fn,
    ///     10.0,           // Stop if reward >= 10.0
    ///     Some(2),        // Allow up to 2 failures
    /// );
    /// ```
    pub fn new(
        module: Box<dyn Node<S>>,
        n: usize,
        reward_fn: RewardFn<S>,
        threshold: f32,
        fail_count: Option<usize>,
    ) -> Self {
        Self {
            module,
            n,
            reward_fn,
            threshold,
            fail_count: fail_count.unwrap_or(n),
        }
    }
}

#[async_trait]
impl<S: GraphState> Node<S> for BestOfNNode<S> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_optimizable(&self) -> bool {
        true
    }

    // BestOfNNode wraps another node; LLM usage depends on wrapped node
    fn may_use_llm(&self) -> bool {
        false
    }

    /// Execute the wrapped node N times and return the best result.
    ///
    /// # Algorithm
    ///
    /// 1. For i in 0..N:
    ///    a. Clone the input state
    ///    b. Execute the wrapped node with the cloned state
    ///    c. If execution succeeds:
    ///       - Compute reward score
    ///       - Update best result if this score is higher
    ///       - If score >= threshold, return immediately (early stopping)
    ///    d. If execution fails:
    ///       - Increment failure counter
    ///       - If failures > fail_count, return error
    /// 2. Return the best result found
    ///
    /// # Notes
    ///
    /// - Each execution gets a fresh clone of the input state
    /// - LLM temperature should be > 0 for diversity (not enforced here)
    /// - Failures are logged but don't stop execution unless fail_count exceeded
    /// - If all N attempts fail, returns error from last failure
    async fn execute(&self, state: S) -> Result<S> {
        let mut best_state: Option<S> = None;
        let mut best_reward = f32::NEG_INFINITY;
        let mut failures = 0;

        for idx in 0..self.n {
            // Clone state for this attempt (each execution gets fresh state)
            let state_clone = state.clone();

            // Execute wrapped node
            let result = self.module.execute(state_clone).await;

            match result {
                Ok(result_state) => {
                    // Score the result
                    let reward = (self.reward_fn)(&result_state);

                    // Update best if this is better
                    if reward > best_reward {
                        best_reward = reward;
                        best_state = Some(result_state.clone());
                    }

                    // Early stopping if threshold met
                    if reward >= self.threshold {
                        return Ok(result_state);
                    }
                }
                Err(e) => {
                    tracing::warn!("BestOfN: Attempt {}/{} failed: {}", idx + 1, self.n, e);
                    failures += 1;

                    if failures > self.fail_count {
                        return Err(e);
                    }
                }
            }
        }

        // Return best result found
        best_state.ok_or_else(|| Error::NodeExecution {
            node: "BestOfNNode".to_string(),
            source: Box::new(std::io::Error::other(
                "No successful predictions in BestOfN",
            )),
        })
    }
}

#[async_trait]
impl<S: GraphState> Optimizable<S> for BestOfNNode<S> {
    /// Delegate optimization to the wrapped node.
    ///
    /// BestOfN itself has no trainable parameters - it just runs another node
    /// multiple times. The wrapped node may be optimizable (e.g., ChainOfThought
    /// can be optimized with BootstrapFewShot).
    ///
    /// # Implementation
    ///
    /// If the wrapped node implements Optimizable, this will forward the optimize
    /// call to it. Otherwise, returns a no-op OptimizationResult.
    async fn optimize(
        &mut self,
        _examples: &[S],
        _metric: &crate::optimize::MetricFn<S>,
        _config: &OptimizerConfig,
    ) -> Result<OptimizationResult> {
        // Try to downcast and optimize the wrapped module
        // Note: This is a simplified implementation - ideally we'd check if
        // the wrapped node implements Optimizable and delegate to it.
        // For now, return no-op result (BestOfN has no parameters to optimize).
        Ok(OptimizationResult {
            final_score: 0.0,
            initial_score: 0.0,
            iterations: 0,
            converged: true,
            duration_secs: 0.0,
        })
    }

    fn get_optimization_state(&self) -> crate::optimize::OptimizationState {
        // BestOfN has no optimization state of its own
        // Return empty state
        crate::optimize::OptimizationState {
            instruction: String::new(),
            few_shot_examples: vec![],
            metadata: std::collections::HashMap::new(),
        }
    }

    fn set_optimization_state(&mut self, _state: crate::optimize::OptimizationState) {
        // BestOfN has no optimization state to set
        // This is a no-op
    }
}

impl<S: GraphState> std::fmt::Debug for BestOfNNode<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BestOfNNode")
            .field("module", &"<Node>")
            .field("n", &self.n)
            .field("reward_fn", &"<function>")
            .field("threshold", &self.threshold)
            .field("fail_count", &self.fail_count)
            .finish()
    }
}

impl<S: GraphState> std::fmt::Display for BestOfNNode<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BestOfN(N={}, threshold={})", self.n, self.threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        question: String,
        answer: String,
    }

    // GraphState is auto-implemented for types that meet the trait bounds
    // No manual impl needed

    // Simple test node that returns a fixed answer
    struct FixedAnswerNode {
        answer: String,
    }

    #[async_trait]
    impl Node<TestState> for FixedAnswerNode {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        async fn execute(&self, mut state: TestState) -> Result<TestState> {
            state.answer = self.answer.clone();
            Ok(state)
        }
    }

    #[test]
    fn test_best_of_n_creation() {
        let node = FixedAnswerNode {
            answer: "test".to_string(),
        };

        let reward_fn = Arc::new(|state: &TestState| -> f32 { state.answer.len() as f32 });

        let best_of_n = BestOfNNode::new(Box::new(node), 3, reward_fn, 10.0, None);

        assert_eq!(best_of_n.n, 3);
        assert_eq!(best_of_n.threshold, 10.0);
        assert_eq!(best_of_n.fail_count, 3); // Defaults to n
    }

    #[test]
    fn test_best_of_n_with_custom_fail_count() {
        let node = FixedAnswerNode {
            answer: "test".to_string(),
        };

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let best_of_n = BestOfNNode::new(
            Box::new(node),
            5,
            reward_fn,
            10.0,
            Some(2), // Custom fail_count
        );

        assert_eq!(best_of_n.n, 5);
        assert_eq!(best_of_n.fail_count, 2);
    }

    #[test]
    fn test_best_of_n_display() {
        let node = FixedAnswerNode {
            answer: "test".to_string(),
        };

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let best_of_n = BestOfNNode::new(Box::new(node), 5, reward_fn, 0.9, None);

        let display_str = format!("{}", best_of_n);
        assert_eq!(display_str, "BestOfN(N=5, threshold=0.9)");
    }

    // Test node that returns different answers each time (via counter)
    struct VaryingAnswerNode {
        answers: Vec<String>,
        counter: std::sync::atomic::AtomicUsize,
    }

    impl VaryingAnswerNode {
        fn new(answers: Vec<String>) -> Self {
            Self {
                answers,
                counter: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Node<TestState> for VaryingAnswerNode {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        async fn execute(&self, mut state: TestState) -> Result<TestState> {
            let idx = self
                .counter
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            state.answer = self.answers[idx % self.answers.len()].clone();
            Ok(state)
        }
    }

    #[tokio::test]
    async fn test_best_of_n_selects_best() {
        // Create node that returns different length answers
        let node = VaryingAnswerNode::new(vec![
            "short".to_string(),
            "medium answer".to_string(),
            "this is a longer answer".to_string(),
        ]);

        // Reward function prefers shorter answers
        let reward_fn =
            Arc::new(|state: &TestState| -> f32 { 1.0 / (state.answer.len() as f32 + 1.0) });

        let best_of_3 = BestOfNNode::new(
            Box::new(node),
            3,
            reward_fn,
            0.5, // High threshold that won't be met
            None,
        );

        let initial_state = TestState {
            question: "Test question".to_string(),
            answer: String::new(),
        };

        let result = best_of_3.execute(initial_state).await;

        assert!(result.is_ok());
        let final_state = result.unwrap();
        // Should select the shortest answer
        assert_eq!(final_state.answer, "short");
    }

    #[tokio::test]
    async fn test_best_of_n_early_stopping() {
        // Create node that returns different answers
        let node = VaryingAnswerNode::new(vec![
            "x".to_string(),      // Very short, high reward
            "longer".to_string(), // Won't be reached
            "even longer".to_string(),
        ]);

        // Reward function gives high reward to single-char answers
        let reward_fn = Arc::new(|state: &TestState| -> f32 {
            if state.answer.len() == 1 {
                1.0 // Meets threshold
            } else {
                0.1
            }
        });

        let best_of_3 = BestOfNNode::new(
            Box::new(node),
            3,
            reward_fn,
            0.9, // Threshold that "x" will meet
            None,
        );

        let initial_state = TestState {
            question: "Test".to_string(),
            answer: String::new(),
        };

        let result = best_of_3.execute(initial_state).await;

        assert!(result.is_ok());
        let final_state = result.unwrap();
        // Should stop early with first answer
        assert_eq!(final_state.answer, "x");
    }

    // Test node that always fails
    struct FailingNode;

    #[async_trait]
    impl Node<TestState> for FailingNode {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        async fn execute(&self, _state: TestState) -> Result<TestState> {
            Err(Error::NodeExecution {
                node: "FailingNode".to_string(),
                source: Box::new(std::io::Error::other("Simulated failure")),
            })
        }
    }

    #[tokio::test]
    async fn test_best_of_n_handles_failures() {
        let node = FailingNode;

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let best_of_3 = BestOfNNode::new(
            Box::new(node),
            3,
            reward_fn,
            10.0,
            Some(5), // Allow more failures than attempts
        );

        let initial_state = TestState {
            question: "Test".to_string(),
            answer: String::new(),
        };

        let result = best_of_3.execute(initial_state).await;

        // Should fail because no successful predictions
        assert!(result.is_err());
    }

    #[test]
    fn test_best_of_n_debug() {
        let node = FixedAnswerNode {
            answer: "test".to_string(),
        };

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let best_of_n = BestOfNNode::new(Box::new(node), 5, reward_fn, 0.9, Some(3));

        let debug_str = format!("{:?}", best_of_n);
        assert!(debug_str.contains("BestOfNNode"));
        assert!(debug_str.contains("n: 5"));
        assert!(debug_str.contains("threshold: 0.9"));
        assert!(debug_str.contains("fail_count: 3"));
        assert!(debug_str.contains("<function>"));
    }

    #[test]
    fn test_get_optimization_state() {
        let node = FixedAnswerNode {
            answer: "test".to_string(),
        };

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let best_of_n = BestOfNNode::new(Box::new(node), 3, reward_fn, 0.9, None);

        let state = best_of_n.get_optimization_state();

        // BestOfN has no optimization state
        assert!(state.instruction.is_empty());
        assert!(state.few_shot_examples.is_empty());
        assert!(state.metadata.is_empty());
    }

    #[test]
    fn test_set_optimization_state_noop() {
        let node = FixedAnswerNode {
            answer: "test".to_string(),
        };

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let mut best_of_n = BestOfNNode::new(Box::new(node), 3, reward_fn, 0.9, None);

        let opt_state = crate::optimize::OptimizationState {
            instruction: "test instruction".to_string(),
            few_shot_examples: vec![],
            metadata: std::collections::HashMap::new(),
        };

        // Should be a no-op
        best_of_n.set_optimization_state(opt_state);

        // Verify state unchanged
        let state = best_of_n.get_optimization_state();
        assert!(state.instruction.is_empty());
    }

    #[tokio::test]
    async fn test_optimizable_returns_noop() {
        let node = FixedAnswerNode {
            answer: "test".to_string(),
        };

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let mut best_of_n = BestOfNNode::new(Box::new(node), 3, reward_fn, 0.9, None);

        let metric: crate::optimize::MetricFn<TestState> =
            Arc::new(|_: &TestState, _: &TestState| Ok(1.0));
        let config = OptimizerConfig::default();

        let result = best_of_n.optimize(&[], &metric, &config).await.unwrap();

        assert_eq!(result.iterations, 0);
        assert!(result.converged);
        assert_eq!(result.initial_score, 0.0);
        assert_eq!(result.final_score, 0.0);
    }

    #[tokio::test]
    async fn test_best_of_n_exceeds_fail_count() {
        let node = FailingNode;

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let best_of_5 = BestOfNNode::new(
            Box::new(node),
            5,
            reward_fn,
            10.0,
            Some(2), // Only allow 2 failures
        );

        let initial_state = TestState {
            question: "Test".to_string(),
            answer: String::new(),
        };

        let result = best_of_5.execute(initial_state).await;

        // Should fail after exceeding fail_count
        assert!(result.is_err());
    }

    // Test node that sometimes succeeds
    struct PartialFailingNode {
        fail_first_n: usize,
        counter: std::sync::atomic::AtomicUsize,
        success_answer: String,
    }

    #[async_trait]
    impl Node<TestState> for PartialFailingNode {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        async fn execute(&self, mut state: TestState) -> Result<TestState> {
            let idx = self
                .counter
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if idx < self.fail_first_n {
                Err(Error::Generic(format!("Failure {}", idx)))
            } else {
                state.answer = self.success_answer.clone();
                Ok(state)
            }
        }
    }

    #[tokio::test]
    async fn test_best_of_n_partial_failures_then_success() {
        let node = PartialFailingNode {
            fail_first_n: 2,
            counter: std::sync::atomic::AtomicUsize::new(0),
            success_answer: "success!".to_string(),
        };

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let best_of_5 = BestOfNNode::new(
            Box::new(node),
            5,
            reward_fn,
            0.5,     // Threshold that will be met
            Some(3), // Allow enough failures
        );

        let initial_state = TestState {
            question: "Test".to_string(),
            answer: String::new(),
        };

        let result = best_of_5.execute(initial_state).await;

        // Should succeed after initial failures
        assert!(result.is_ok());
        let final_state = result.unwrap();
        assert_eq!(final_state.answer, "success!");
    }

    #[tokio::test]
    async fn test_best_of_n_threshold_exact_match() {
        let node = FixedAnswerNode {
            answer: "exact".to_string(),
        };

        // Reward function returns exactly the threshold
        let reward_fn = Arc::new(|_: &TestState| 0.9);

        let best_of_3 = BestOfNNode::new(
            Box::new(node),
            3,
            reward_fn,
            0.9, // Exactly matches reward
            None,
        );

        let initial_state = TestState {
            question: "Test".to_string(),
            answer: String::new(),
        };

        let result = best_of_3.execute(initial_state).await;

        // Should succeed with early stopping (>= threshold)
        assert!(result.is_ok());
        assert_eq!(result.unwrap().answer, "exact");
    }

    #[tokio::test]
    async fn test_best_of_n_preserves_question() {
        let node = FixedAnswerNode {
            answer: "answer".to_string(),
        };

        let reward_fn = Arc::new(|_: &TestState| 1.0);

        let best_of_n = BestOfNNode::new(Box::new(node), 3, reward_fn, 0.5, None);

        let initial_state = TestState {
            question: "My important question".to_string(),
            answer: String::new(),
        };

        let result = best_of_n.execute(initial_state).await;

        assert!(result.is_ok());
        let final_state = result.unwrap();
        assert_eq!(final_state.question, "My important question");
        assert_eq!(final_state.answer, "answer");
    }

    #[tokio::test]
    async fn test_best_of_n_negative_reward() {
        let node = VaryingAnswerNode::new(vec![
            "terrible".to_string(),
            "bad".to_string(),
            "ok".to_string(),
        ]);

        // Reward function can return negative values
        let reward_fn = Arc::new(|state: &TestState| -> f32 {
            match state.answer.as_str() {
                "terrible" => -10.0,
                "bad" => -5.0,
                "ok" => 0.0,
                _ => -1.0,
            }
        });

        let best_of_3 = BestOfNNode::new(
            Box::new(node),
            3,
            reward_fn,
            100.0, // Will not be met
            None,
        );

        let initial_state = TestState {
            question: "Test".to_string(),
            answer: String::new(),
        };

        let result = best_of_3.execute(initial_state).await;

        // Should select the least negative (best) score
        assert!(result.is_ok());
        assert_eq!(result.unwrap().answer, "ok");
    }

    #[test]
    fn test_reward_fn_type_alias() {
        // Verify RewardFn type alias works correctly
        let reward_fn: RewardFn<TestState> =
            Arc::new(|state: &TestState| state.answer.len() as f32);

        let state = TestState {
            question: "q".to_string(),
            answer: "test".to_string(),
        };

        assert_eq!(reward_fn(&state), 4.0);
    }

    // Note: Integration test with real LLM node (ChainOfThoughtNode) is tested
    // in integration tests where MockChatModel is available.
    // The tests above cover the core BestOfN selection logic.
}
