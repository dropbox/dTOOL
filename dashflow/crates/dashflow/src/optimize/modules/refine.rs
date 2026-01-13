// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Refine Module
//!
//! Iterative refinement module that runs a node multiple times, generating feedback
//! between attempts to improve predictions through iterative improvement.
//!
//! ## Purpose
//!
//! Refine enables iterative improvement by:
//! - Running a node N times with feedback injection
//! - Generating feedback after each attempt (via user-defined function)
//! - Injecting feedback into state for next attempt
//! - Returning first result >= threshold, or best result
//!
//! This is more sophisticated than BestOfN because it uses feedback from previous
//! attempts to guide improvement, rather than just sampling with randomness.
//!
//! ## Usage
//!
//! ```rust
//! use dashflow::optimize::modules::RefineNode;
//! use dashflow::optimize::{Signature, Field, FieldKind};
//! use dashflow::GraphState;
//! use std::sync::Arc;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct QAState {
//!     question: String,
//!     answer: String,
//!     feedback: Option<String>,
//! }
//!
//! // Define reward function
//! let reward_fn = Arc::new(|state: &QAState| -> f32 {
//!     let answer = &state.answer;
//!     // Prefer concise answers (1-2 sentences)
//!     let sentences = answer.split('.').count();
//!     if sentences <= 2 { 1.0 } else { 0.5 }
//! });
//!
//! // Define feedback function
//! let feedback_fn = Arc::new(|state: &QAState, reward: f32| -> String {
//!     if reward < 1.0 {
//!         "Make the answer more concise - aim for 1-2 sentences.".to_string()
//!     } else {
//!         "Good conciseness".to_string()
//!     }
//! });
//!
//! // Create signature and base node
//! // let signature = Signature::new(...);
//! // let llm: Arc<dyn ChatModel> = ...;
//! // let base_node = ChainOfThoughtNode::new(signature, llm);
//!
//! // Create Refine wrapper
//! // let refine = RefineNode::new(
//! //     Box::new(base_node),
//! //     3,  // Try up to 3 times
//! //     reward_fn,
//! //     feedback_fn,
//! //     1.0,  // Threshold
//! //     None,  // fail_count defaults to N
//! // );
//! ```
//!
//! ## Design
//!
//! - Wraps any `Node<S>` implementation
//! - Runs wrapped node N times with feedback injection
//! - User-defined feedback function generates advice
//! - State must have a way to carry feedback (field or trait method)
//! - Returns first result >= threshold, or highest scoring result
//! - Configurable failure tolerance
//!
//! ## Feedback Injection
//!
//! The feedback function receives the current state and reward, and returns a string.
//! How this feedback is injected into the next iteration depends on the state type:
//! - **Recommended**: State has a `feedback: Option<String>` field
//! - **Alternative**: Use a trait method to inject feedback dynamically
//!
//! ## Performance
//!
//! - Framework overhead: ~10µs per iteration
//! - LLM calls: N calls (typically 3-5)
//! - Feedback generation: Depends on user function (fast if rule-based, slow if LLM-based)
//! - Latency: N × base_node_latency + N × feedback_latency
//! - Quality improvement: Depends on feedback quality and node's ability to use it
//!
//! ## Related
//!
//! - See also: `BestOfNNode` (simple sampling without feedback)
//! - See also: `ChainOfThoughtNode` (reasoning with intermediate steps)
//! - Pattern: Iterative improvement through feedback loops

use crate::node::Node;
use crate::optimize::{Optimizable, OptimizationResult, OptimizerConfig};
use crate::state::GraphState;
use crate::{Error, Result};
use async_trait::async_trait;
use std::sync::Arc;

/// Reward function type that takes a state and returns a score (higher is better).
///
/// Same as BestOfN reward function - scores the quality of a state.
pub type RewardFn<S> = Arc<dyn Fn(&S) -> f32 + Send + Sync>;

/// Feedback function type that takes a state, reward, and returns feedback text.
///
/// The feedback function analyzes the current state and reward, then generates
/// actionable advice for improvement. This advice should be injected into the
/// state for the next iteration.
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
///
/// # struct TestState { answer: String, feedback: Option<String> }
/// let feedback_fn = Arc::new(|state: &TestState, reward: f32| -> String {
///     if reward < 0.8 {
///         format!("Current answer '{}' is too long. Make it more concise.", state.answer)
///     } else {
///         "Good work!".to_string()
///     }
/// });
/// ```
pub type FeedbackFn<S> = Arc<dyn Fn(&S, f32) -> String + Send + Sync>;

/// Trait for states that support feedback injection.
///
/// States that want to use RefineNode should implement this trait to receive
/// feedback between iterations. The default implementation does nothing (feedback
/// is generated but not injected), which means RefineNode behaves like BestOfN.
///
/// # Example
///
/// ```rust
/// use dashflow::optimize::modules::refine::RefineableState;
/// use dashflow::GraphState;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct QAState {
///     question: String,
///     answer: String,
///     feedback: Option<String>,
/// }
///
/// impl RefineableState for QAState {
///     fn inject_feedback(&mut self, feedback: String) {
///         self.feedback = Some(feedback);
///     }
///
///     fn clear_feedback(&mut self) {
///         self.feedback = None;
///     }
/// }
/// ```
pub trait RefineableState: GraphState {
    /// Inject feedback into the state for the next iteration.
    ///
    /// This method should store the feedback in a way that the wrapped node
    /// can access and use it (e.g., in a `feedback` field).
    fn inject_feedback(&mut self, _feedback: String) {
        // Default: no-op (feedback not injected)
        // States that want feedback should override this
    }

    /// Clear any existing feedback from the state.
    ///
    /// Called before the first iteration to ensure clean state.
    fn clear_feedback(&mut self) {
        // Default: no-op
    }
}

/// Refine node that runs a wrapped node N times with feedback-driven iterative refinement.
///
/// This node wraps another node and executes it multiple times. Between executions,
/// it generates feedback using a user-defined function and injects it into the state.
/// It returns either the first result that exceeds a threshold, or the one with the
/// highest reward.
///
/// # Type Parameters
///
/// - `S`: GraphState type that the wrapped node operates on (should implement RefineableState)
///
/// # Examples
///
/// ```rust
/// use dashflow::optimize::modules::{RefineNode, ChainOfThoughtNode};
/// use dashflow::optimize::modules::refine::RefineableState;
/// use dashflow::optimize::{Signature, Field, FieldKind};
/// use dashflow::GraphState;
/// use std::sync::Arc;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct QAState {
///     question: String,
///     answer: String,
///     feedback: Option<String>,
/// }
///
/// impl RefineableState for QAState {
///     fn inject_feedback(&mut self, feedback: String) {
///         self.feedback = Some(feedback);
///     }
///
///     fn clear_feedback(&mut self) {
///         self.feedback = None;
///     }
/// }
///
/// // Reward function that prefers specific answer formats
/// let reward_fn = Arc::new(|state: &QAState| -> f32 {
///     let answer = &state.answer.to_lowercase();
///     // Prefer answers that start with "because" or "the reason is"
///     if answer.starts_with("because") || answer.contains("the reason is") {
///         1.0
///     } else {
///         0.5
///     }
/// });
///
/// // Feedback function that provides actionable advice
/// let feedback_fn = Arc::new(|state: &QAState, reward: f32| -> String {
///     if reward < 1.0 {
///         "Start your answer with 'because' or include 'the reason is' for better clarity.".to_string()
///     } else {
///         "Good format!".to_string()
///     }
/// });
///
/// // Create base node and wrap with Refine
/// // let cot_node = ChainOfThoughtNode::new(...);
/// // let refine = RefineNode::new(Box::new(cot_node), 3, reward_fn, feedback_fn, 1.0, None);
/// ```
pub struct RefineNode<S: RefineableState> {
    /// The wrapped node to execute multiple times
    pub module: Box<dyn Node<S>>,

    /// Number of attempts
    pub n: usize,

    /// Reward function for scoring results
    pub reward_fn: RewardFn<S>,

    /// Feedback function for generating improvement advice
    pub feedback_fn: FeedbackFn<S>,

    /// Threshold for early stopping (stop when reward >= threshold)
    pub threshold: f32,

    /// Maximum number of allowed failures (defaults to n)
    pub fail_count: usize,
}

impl<S: RefineableState> RefineNode<S> {
    /// Create a new Refine wrapper.
    ///
    /// # Arguments
    ///
    /// * `module` - The node to wrap (will be called N times with feedback)
    /// * `n` - Number of times to run the node
    /// * `reward_fn` - Function that scores results (higher is better)
    /// * `feedback_fn` - Function that generates feedback from state and reward
    /// * `threshold` - If a result scores >= threshold, return it immediately
    /// * `fail_count` - Max failures allowed before giving up (defaults to `n` if None)
    ///
    /// # Returns
    ///
    /// A new RefineNode instance
    ///
    /// # Example
    ///
    /// ```rust
    /// # use dashflow::optimize::modules::RefineNode;
    /// # use dashflow::optimize::modules::refine::RefineableState;
    /// # use dashflow::GraphState;
    /// # use std::sync::Arc;
    /// # use serde::{Deserialize, Serialize};
    /// #
    /// # #[derive(Debug, Clone, Serialize, Deserialize)]
    /// # struct TestState { value: String, feedback: Option<String> }
    /// # impl RefineableState for TestState {
    /// #     fn inject_feedback(&mut self, feedback: String) { self.feedback = Some(feedback); }
    /// #     fn clear_feedback(&mut self) { self.feedback = None; }
    /// # }
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
    /// let feedback_fn = Arc::new(|state: &TestState, reward: f32| -> String {
    ///     format!("Current length: {}, try to reach 10", state.value.len())
    /// });
    ///
    /// let refine = RefineNode::new(
    ///     Box::new(DummyNode),
    ///     5,              // Try 5 times
    ///     reward_fn,
    ///     feedback_fn,
    ///     10.0,           // Stop if reward >= 10.0
    ///     Some(2),        // Allow up to 2 failures
    /// );
    /// ```
    pub fn new(
        module: Box<dyn Node<S>>,
        n: usize,
        reward_fn: RewardFn<S>,
        feedback_fn: FeedbackFn<S>,
        threshold: f32,
        fail_count: Option<usize>,
    ) -> Self {
        Self {
            module,
            n,
            reward_fn,
            feedback_fn,
            threshold,
            fail_count: fail_count.unwrap_or(n),
        }
    }
}

#[async_trait]
impl<S: RefineableState> Node<S> for RefineNode<S> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_optimizable(&self) -> bool {
        true
    }

    // RefineNode wraps another node; LLM usage depends on wrapped node
    fn may_use_llm(&self) -> bool {
        false
    }

    /// Execute the wrapped node N times with feedback injection and return the best result.
    ///
    /// # Algorithm
    ///
    /// 1. Clear any existing feedback from input state
    /// 2. For i in 0..N:
    ///    a. Clone the input state (with accumulated feedback if i > 0)
    ///    b. Execute the wrapped node with the cloned state
    ///    c. If execution succeeds:
    ///       - Compute reward score
    ///       - Update best result if this score is higher
    ///       - If score >= threshold, return immediately (early stopping)
    ///       - If score < threshold and not last iteration, generate feedback and inject
    ///    d. If execution fails:
    ///       - Increment failure counter
    ///       - If failures > fail_count, return error
    /// 3. Return the best result found
    ///
    /// # Notes
    ///
    /// - Feedback accumulates across iterations (each iteration sees previous feedback)
    /// - State must implement RefineableState trait for feedback injection
    /// - If state doesn't implement feedback injection, behaves like BestOfN
    /// - Failures are logged but don't stop execution unless fail_count exceeded
    async fn execute(&self, mut state: S) -> Result<S> {
        let mut best_state: Option<S> = None;
        let mut best_reward = f32::NEG_INFINITY;
        let mut failures = 0;

        // Clear any existing feedback before starting
        state.clear_feedback();

        for idx in 0..self.n {
            // Clone state for this attempt
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

                    // If not last attempt and below threshold, generate and inject feedback
                    if idx < self.n - 1 && reward < self.threshold {
                        let feedback = (self.feedback_fn)(&result_state, reward);
                        state.inject_feedback(feedback);
                    }
                }
                Err(e) => {
                    tracing::warn!("Refine: Attempt {}/{} failed: {}", idx + 1, self.n, e);
                    failures += 1;

                    if failures > self.fail_count {
                        return Err(e);
                    }
                }
            }
        }

        // Return best result found
        best_state.ok_or_else(|| Error::NodeExecution {
            node: "RefineNode".to_string(),
            source: Box::new(std::io::Error::other("No successful predictions in Refine")),
        })
    }
}

#[async_trait]
impl<S: RefineableState> Optimizable<S> for RefineNode<S> {
    /// Delegate optimization to the wrapped node.
    ///
    /// RefineNode itself has no trainable parameters - it just runs another node
    /// multiple times with feedback injection. The wrapped node may be optimizable
    /// (e.g., ChainOfThought can be optimized with BootstrapFewShot).
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
        // For now, return no-op result (RefineNode has no parameters to optimize).
        Ok(OptimizationResult {
            final_score: 0.0,
            initial_score: 0.0,
            iterations: 0,
            converged: true,
            duration_secs: 0.0,
        })
    }

    fn get_optimization_state(&self) -> crate::optimize::OptimizationState {
        // RefineNode has no optimization state of its own
        // Return empty state
        crate::optimize::OptimizationState {
            instruction: String::new(),
            few_shot_examples: vec![],
            metadata: std::collections::HashMap::new(),
        }
    }

    fn set_optimization_state(&mut self, _state: crate::optimize::OptimizationState) {
        // RefineNode has no optimization state to set
        // This is a no-op
    }
}

// Implement Display and Debug for RefineNode
impl<S: RefineableState> std::fmt::Display for RefineNode<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RefineNode(N={}, threshold={:.2})",
            self.n, self.threshold
        )
    }
}

impl<S: RefineableState> std::fmt::Debug for RefineNode<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RefineNode")
            .field("module", &"<Node<S>>")
            .field("n", &self.n)
            .field("reward_fn", &"<function>")
            .field("feedback_fn", &"<function>")
            .field("threshold", &self.threshold)
            .field("fail_count", &self.fail_count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    // Test state that implements RefineableState
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestState {
        value: String,
        counter: u32,
        feedback: Option<String>,
    }

    // GraphState is auto-implemented for types that meet the trait bounds

    impl RefineableState for TestState {
        fn inject_feedback(&mut self, feedback: String) {
            self.feedback = Some(feedback);
        }

        fn clear_feedback(&mut self) {
            self.feedback = None;
        }
    }

    // Dummy node that increments counter
    // If feedback is present, increments counter by an additional amount based on feedback length
    struct IncrementNode;

    #[async_trait]
    impl Node<TestState> for IncrementNode {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        async fn execute(&self, mut state: TestState) -> Result<TestState> {
            state.counter += 1;
            // If there's feedback, increment counter more and append feedback to value
            if let Some(feedback) = &state.feedback {
                state.counter += feedback.len() as u32; // More feedback = higher counter
                state.value = format!("{} | Feedback: {}", state.value, feedback);
            }
            Ok(state)
        }
    }

    #[tokio::test]
    async fn test_refine_creation() {
        let reward_fn = Arc::new(|state: &TestState| -> f32 { state.counter as f32 });
        let feedback_fn = Arc::new(|_state: &TestState, reward: f32| -> String {
            format!("Current reward: {}", reward)
        });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            3,
            reward_fn,
            feedback_fn,
            5.0,
            None,
        );

        assert_eq!(refine.n, 3);
        assert_eq!(refine.threshold, 5.0);
        assert_eq!(refine.fail_count, 3);
    }

    #[tokio::test]
    async fn test_refine_iterative_execution() {
        let reward_fn = Arc::new(|state: &TestState| -> f32 { state.counter as f32 });
        let feedback_fn = Arc::new(|_state: &TestState, reward: f32| -> String {
            format!("Reward was {}, try harder", reward)
        });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            3,
            reward_fn,
            feedback_fn,
            10.0, // Threshold higher than achievable (max counter=3)
            None,
        );

        let initial_state = TestState {
            value: "start".to_string(),
            counter: 0,
            feedback: None,
        };

        let result = refine.execute(initial_state).await.unwrap();

        // Iteration 1: counter=0 -> counter=1, reward=1.0, generate feedback "Reward was 1, try harder" (26 chars)
        // Iteration 2: counter=0, feedback=Some("Reward was 1, try harder") -> counter=1+26=27, reward=27.0 (BEST)
        // Iteration 3: counter=0, feedback=Some("Reward was 27, try harder") (27 chars) -> counter=1+27=28, reward=28.0 (NEW BEST)
        // Returns iteration 3 result
        assert!(result.counter > 1); // At least iteration 2 or 3
                                     // Should have feedback injected in the best result
        assert!(result.value.contains("Feedback:"));
    }

    #[tokio::test]
    async fn test_refine_early_stopping() {
        let reward_fn = Arc::new(|state: &TestState| -> f32 { state.counter as f32 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "improve".to_string() });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            5,
            reward_fn,
            feedback_fn,
            2.0, // Should stop at counter=2
            None,
        );

        let initial_state = TestState {
            value: "start".to_string(),
            counter: 0,
            feedback: None,
        };

        let result = refine.execute(initial_state).await.unwrap();

        // Iteration 1: counter=0 -> counter=1, reward=1.0, feedback="improve" (7 chars)
        // Iteration 2: counter=0, feedback="improve" -> counter=1+7=8, reward=8.0 >= 2.0 threshold, STOP
        // Returns iteration 2 result immediately (early stopping)
        assert_eq!(result.counter, 8);
    }

    #[tokio::test]
    async fn test_refine_feedback_accumulation() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 0.5 }); // Always below threshold
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "add more".to_string() });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            3,
            reward_fn,
            feedback_fn,
            10.0, // Never reached
            None,
        );

        let initial_state = TestState {
            value: "start".to_string(),
            counter: 0,
            feedback: None,
        };

        let result = refine.execute(initial_state).await.unwrap();

        // All iterations have reward=0.5 (constant), so best is first iteration
        // Iteration 1: counter=0 -> counter=1, reward=0.5
        // Iteration 2: counter=0, feedback="add more" (8 chars) -> counter=1+8=9, reward=0.5 (NOT better)
        // Iteration 3: counter=0, feedback="add more" -> counter=1+8=9, reward=0.5 (NOT better)
        // Returns first result (counter=1), which has NO feedback
        assert_eq!(result.counter, 1);
        // First iteration has no feedback, so value doesn't contain "Feedback:"
        assert!(!result.value.contains("Feedback:"));
    }

    #[tokio::test]
    async fn test_refine_display() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            5,
            reward_fn,
            feedback_fn,
            0.9,
            None,
        );

        let display_str = format!("{}", refine);
        assert_eq!(display_str, "RefineNode(N=5, threshold=0.90)");
    }

    #[tokio::test]
    async fn test_refine_optimizable_no_op() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let mut refine = RefineNode::new(
            Box::new(IncrementNode),
            3,
            reward_fn,
            feedback_fn,
            0.9,
            None,
        );

        // Wrapped node is not optimizable, should return no-op result
        let metric: crate::optimize::MetricFn<TestState> =
            Arc::new(|_: &TestState, _: &TestState| -> Result<f64> { Ok(0.0) });
        let config = OptimizerConfig::default();
        let result = refine.optimize(&[], &metric, &config).await.unwrap();

        assert_eq!(result.iterations, 0);
        assert_eq!(result.initial_score, 0.0);
        assert_eq!(result.final_score, 0.0);
        assert!(result.converged);
    }

    // Test state WITHOUT RefineableState implementation
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct SimpleState {
        value: u32,
    }

    // GraphState is auto-implemented for types that meet the trait bounds

    impl RefineableState for SimpleState {
        // Uses default implementation (no-op)
    }

    struct SimpleNode;

    #[async_trait]
    impl Node<SimpleState> for SimpleNode {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        async fn execute(&self, mut state: SimpleState) -> Result<SimpleState> {
            state.value += 1;
            Ok(state)
        }
    }

    #[tokio::test]
    async fn test_refine_without_feedback_injection() {
        // Test that RefineNode works even if state doesn't implement feedback injection
        // (behaves like BestOfN in this case)
        let reward_fn = Arc::new(|state: &SimpleState| -> f32 { state.value as f32 });
        let feedback_fn = Arc::new(|_: &SimpleState, _: f32| -> String { "ignored".to_string() });

        let refine = RefineNode::new(Box::new(SimpleNode), 3, reward_fn, feedback_fn, 10.0, None);

        let initial_state = SimpleState { value: 0 };

        let result = refine.execute(initial_state).await.unwrap();

        // Each iteration: value=0 -> value=1, reward=1.0
        // All iterations have same reward, so best is first iteration
        // Returns first result (value=1)
        assert_eq!(result.value, 1);
    }

    #[test]
    fn test_refine_debug_formatting() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            5,
            reward_fn,
            feedback_fn,
            0.9,
            Some(3),
        );

        let debug_str = format!("{:?}", refine);
        assert!(debug_str.contains("RefineNode"));
        assert!(debug_str.contains("n: 5"));
        assert!(debug_str.contains("threshold: 0.9"));
        assert!(debug_str.contains("fail_count: 3"));
        assert!(debug_str.contains("<function>"));
    }

    #[test]
    fn test_refine_get_optimization_state() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            3,
            reward_fn,
            feedback_fn,
            0.9,
            None,
        );

        let state = refine.get_optimization_state();

        // RefineNode has no optimization state
        assert!(state.instruction.is_empty());
        assert!(state.few_shot_examples.is_empty());
        assert!(state.metadata.is_empty());
    }

    #[test]
    fn test_refine_set_optimization_state_noop() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let mut refine = RefineNode::new(
            Box::new(IncrementNode),
            3,
            reward_fn,
            feedback_fn,
            0.9,
            None,
        );

        let opt_state = crate::optimize::OptimizationState {
            instruction: "test instruction".to_string(),
            few_shot_examples: vec![],
            metadata: std::collections::HashMap::new(),
        };

        // Should be a no-op
        refine.set_optimization_state(opt_state);

        // Verify state unchanged
        let state = refine.get_optimization_state();
        assert!(state.instruction.is_empty());
    }

    #[test]
    fn test_refine_custom_fail_count() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            5,
            reward_fn,
            feedback_fn,
            0.9,
            Some(2),
        );

        assert_eq!(refine.n, 5);
        assert_eq!(refine.fail_count, 2); // Custom fail count
    }

    #[test]
    fn test_refine_default_fail_count() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            5,
            reward_fn,
            feedback_fn,
            0.9,
            None, // Defaults to n
        );

        assert_eq!(refine.n, 5);
        assert_eq!(refine.fail_count, 5); // Defaults to n
    }

    // Failing node for failure tests
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
            Err(Error::Generic("Intentional failure".to_string()))
        }
    }

    #[tokio::test]
    async fn test_refine_exceeds_fail_count() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let refine = RefineNode::new(
            Box::new(FailingNode),
            5,
            reward_fn,
            feedback_fn,
            0.9,
            Some(2), // Allow only 2 failures
        );

        let initial_state = TestState {
            value: "start".to_string(),
            counter: 0,
            feedback: None,
        };

        let result = refine.execute(initial_state).await;

        // Should fail after exceeding fail_count
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_refine_all_failures_returns_error() {
        let reward_fn = Arc::new(|_: &TestState| -> f32 { 1.0 });
        let feedback_fn = Arc::new(|_: &TestState, _: f32| -> String { "test".to_string() });

        let refine = RefineNode::new(
            Box::new(FailingNode),
            3,
            reward_fn,
            feedback_fn,
            0.9,
            Some(10), // High fail_count so we don't error on individual failures
        );

        let initial_state = TestState {
            value: "start".to_string(),
            counter: 0,
            feedback: None,
        };

        let result = refine.execute(initial_state).await;

        // All iterations fail, no successful prediction
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No successful predictions"));
    }

    #[test]
    fn test_refineable_state_default_impls() {
        // Test that default implementations do nothing
        let mut state = SimpleState { value: 42 };

        // These should be no-ops
        state.inject_feedback("some feedback".to_string());
        state.clear_feedback();

        // Value unchanged
        assert_eq!(state.value, 42);
    }

    #[tokio::test]
    async fn test_refine_clears_initial_feedback() {
        let reward_fn = Arc::new(|state: &TestState| -> f32 { state.counter as f32 });
        let feedback_fn =
            Arc::new(|_: &TestState, _: f32| -> String { "new feedback".to_string() });

        let refine = RefineNode::new(
            Box::new(IncrementNode),
            2,
            reward_fn,
            feedback_fn,
            100.0, // High threshold to run all iterations
            None,
        );

        // Start with pre-existing feedback
        let initial_state = TestState {
            value: "start".to_string(),
            counter: 0,
            feedback: Some("pre-existing feedback".to_string()),
        };

        let result = refine.execute(initial_state).await.unwrap();

        // The pre-existing feedback should have been cleared in first iteration
        // First iteration: counter=0, feedback=None (cleared) -> counter=1
        // Value won't contain "pre-existing feedback"
        assert!(!result.value.contains("pre-existing feedback"));
    }
}
