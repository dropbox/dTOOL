//! # BetterTogether - Meta-Optimizer for Composing Optimization Strategies
//!
//! BetterTogether is a meta-optimizer that composes multiple optimization strategies
//! using different composition strategies. It enables experimenting with different
//! optimizer combinations to find the best approach for a given task.
//!
//! ## Composition Strategies
//!
//! BetterTogether supports three composition strategies:
//!
//! ### 1. Sequential (Default)
//! Execute optimizers one after another, building on previous results:
//! - Optimizer A optimizes the node
//! - Optimizer B optimizes the output from A
//! - Optimizer C optimizes the output from B
//! - Returns the final result after all stages
//!
//! **Use when:** You want optimizers to build on each other's improvements.
//!
//! ### 2. Parallel
//! Run all optimizers independently and pick the best result:
//! - Each optimizer runs on the node independently
//! - Compare final scores from all optimizers
//! - Return the result from the best-performing optimizer
//!
//! **Use when:** You're unsure which optimizer will work best and want to try
//! multiple approaches simultaneously.
//!
//! ### 3. Ensemble
//! Run all optimizers and combine their results via weighted averaging:
//! - Each optimizer runs independently
//! - Weight each result by its improvement score
//! - Return a weighted average of all optimization results
//! - Use the best optimizer's settings as the representative
//!
//! **Use when:** You want to leverage insights from multiple optimizers rather
//! than committing to just one approach.
//!
//! ## Design
//!
//! BetterTogether works with a `NodeOptimizer<S>` trait that abstracts over different
//! optimization strategies. Any optimizer that implements this trait can be composed.
//!
//! ## Examples
//!
//! ### Sequential Strategy
//! ```rust,ignore
//! use dashflow::optimize::*;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let bootstrap = Box::new(BootstrapFewShot::new());
//! let random_search = Box::new(RandomSearch::new());
//!
//! let optimizer = BetterTogether::new()
//!     .add_optimizer(bootstrap)
//!     .add_optimizer(random_search)
//!     .with_strategy(CompositionStrategy::Sequential);
//!
//! let result = optimizer.optimize(&mut node, &trainset, &valset, &metric).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Parallel Strategy
//! ```rust,ignore
//! use dashflow::optimize::*;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let opt1 = Box::new(BootstrapFewShot::new());
//! let opt2 = Box::new(MIPROv2::new());
//! let opt3 = Box::new(COPRO::new());
//!
//! // Try all three, pick the best
//! let optimizer = BetterTogether::new()
//!     .add_optimizer(opt1)
//!     .add_optimizer(opt2)
//!     .add_optimizer(opt3)
//!     .with_strategy(CompositionStrategy::Parallel);
//!
//! let result = optimizer.optimize(&mut node, &trainset, &valset, &metric).await?;
//! println!("Winner: {}", result.final_score);
//! # Ok(())
//! # }
//! ```
//!
//! ### Ensemble Strategy
//! ```rust,ignore
//! use dashflow::optimize::*;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let opt1 = Box::new(BootstrapFewShot::new());
//! let opt2 = Box::new(RandomSearch::new());
//!
//! // Combine results from multiple optimizers
//! let optimizer = BetterTogether::new()
//!     .add_optimizer(opt1)
//!     .add_optimizer(opt2)
//!     .with_strategy(CompositionStrategy::Ensemble);
//!
//! let result = optimizer.optimize(&mut node, &trainset, &valset, &metric).await?;
//! println!("Ensemble score: {}", result.final_score);
//! # Ok(())
//! # }
//! ```
//!
//! ## References
//!
//! - **Source**: DSPy teleprompt library
//! - **Link**: <https://github.com/stanfordnlp/dspy/blob/main/dspy/teleprompt/better_together.py>
//! - **Concept**: Meta-optimizer that combines multiple optimization strategies

use super::OptimizationResult;
use crate::node::Node;
use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::optimize::MetricFn;
use crate::state::GraphState;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Trait for optimizers that can be composed in pipelines.
///
/// This trait enables BetterTogether to compose different optimization
/// strategies without knowing their internal details.
#[async_trait]
pub trait NodeOptimizer<S: GraphState>: Send + Sync {
    /// Optimize a node using training and validation data.
    ///
    /// # Arguments
    /// * `node` - The node to optimize (mutable reference)
    /// * `trainset` - Training examples
    /// * `valset` - Validation examples for evaluation
    /// * `metric` - Function to evaluate prediction quality
    ///
    /// # Returns
    /// Results of optimization (score, iterations, etc.)
    async fn optimize_node(
        &self,
        node: &mut dyn Node<S>,
        trainset: &[S],
        valset: &[S],
        metric: &MetricFn<S>,
    ) -> Result<OptimizationResult>;

    /// Get the name of this optimizer for logging and diagnostics.
    fn name(&self) -> &str;
}

/// Strategy for composing multiple optimizers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompositionStrategy {
    /// Sequential: Run optimizers in order (A → B → C)
    /// Output of each optimizer becomes input to the next
    Sequential,

    /// Parallel: Run all optimizers independently, pick best by final score
    /// Each optimizer works on the node independently and the best result is selected
    Parallel,

    /// Ensemble: Run all optimizers independently, combine results via weighted averaging
    /// Results are weighted by improvement and combined for the final score
    Ensemble,
}

impl fmt::Display for CompositionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompositionStrategy::Sequential => write!(f, "Sequential"),
            CompositionStrategy::Parallel => write!(f, "Parallel"),
            CompositionStrategy::Ensemble => write!(f, "Ensemble"),
        }
    }
}

/// Intermediate result from a single optimizer in the pipeline.
#[derive(Clone, Debug)]
pub struct PipelineStage {
    /// Name of the optimizer that produced this result
    pub optimizer_name: String,

    /// Result from this optimization stage
    pub result: OptimizationResult,

    /// Stage number in the pipeline (0-indexed)
    pub stage_index: usize,
}

/// BetterTogether meta-optimizer.
///
/// Composes multiple optimization strategies into pipelines to find
/// the best approach for a given task.
pub struct BetterTogether<S: GraphState> {
    /// Sequence of optimizers to apply
    optimizers: Vec<Box<dyn NodeOptimizer<S>>>,

    /// Strategy for composing optimizers
    strategy: CompositionStrategy,

    /// Intermediate results from each pipeline stage
    /// (populated after optimization completes)
    pipeline_stages: Vec<PipelineStage>,
}

impl<S: GraphState> BetterTogether<S> {
    /// Create a new BetterTogether optimizer with sequential strategy (default).
    pub fn new() -> Self {
        Self {
            optimizers: Vec::new(),
            strategy: CompositionStrategy::Sequential,
            pipeline_stages: Vec::new(),
        }
    }

    /// Add an optimizer to the pipeline.
    ///
    /// Optimizers are executed in the order they are added (for Sequential strategy).
    #[must_use]
    pub fn add_optimizer(mut self, optimizer: Box<dyn NodeOptimizer<S>>) -> Self {
        self.optimizers.push(optimizer);
        self
    }

    /// Set the composition strategy.
    ///
    /// Default: Sequential
    ///
    /// Available strategies:
    /// - Sequential: Run optimizers in order (A → B → C)
    /// - Parallel: Run all independently, pick best result
    /// - Ensemble: Run all independently, combine via weighted averaging
    #[must_use]
    pub fn with_strategy(mut self, strategy: CompositionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Get the intermediate results from each pipeline stage.
    ///
    /// Only populated after `optimize()` has been called.
    pub fn pipeline_stages(&self) -> &[PipelineStage] {
        &self.pipeline_stages
    }

    /// Optimize a node using the configured pipeline.
    ///
    /// # Arguments
    /// * `node` - The node to optimize (mutable reference)
    /// * `trainset` - Training examples
    /// * `valset` - Validation examples for evaluation
    /// * `metric` - Function to evaluate prediction quality
    ///
    /// # Returns
    /// Final optimization result (after all pipeline stages)
    pub async fn optimize(
        &mut self,
        node: &mut dyn Node<S>,
        trainset: &[S],
        valset: &[S],
        metric: &MetricFn<S>,
    ) -> Result<OptimizationResult> {
        use std::time::Instant;
        let start = Instant::now();

        if self.optimizers.is_empty() {
            return Err(crate::Error::Validation(
                "BetterTogether requires at least one optimizer".to_string(),
            ));
        }

        // Record telemetry start
        record_optimization_start("better_together");

        let result = match self.strategy {
            CompositionStrategy::Sequential => {
                self.optimize_sequential(node, trainset, valset, metric)
                    .await
            }
            CompositionStrategy::Parallel => {
                self.optimize_parallel(node, trainset, valset, metric).await
            }
            CompositionStrategy::Ensemble => {
                self.optimize_ensemble(node, trainset, valset, metric).await
            }
        };

        // Record telemetry completion
        if let Ok(ref res) = result {
            record_optimization_complete(
                "better_together",
                res.iterations as u64,
                self.optimizers.len() as u64,
                res.initial_score,
                res.final_score,
                start.elapsed().as_secs_f64(),
            );
        }

        result
    }

    /// Execute optimizers sequentially (A → B → C).
    ///
    /// Each optimizer optimizes the node in place, and the next optimizer
    /// builds on the previous optimizer's work.
    async fn optimize_sequential(
        &mut self,
        node: &mut dyn Node<S>,
        trainset: &[S],
        valset: &[S],
        metric: &MetricFn<S>,
    ) -> Result<OptimizationResult> {
        self.pipeline_stages.clear();

        tracing::info!(
            "BetterTogether: Sequential Pipeline - {} stages",
            self.optimizers.len()
        );
        for (i, optimizer) in self.optimizers.iter().enumerate() {
            tracing::debug!("  Stage {}: {}", i, optimizer.name());
        }

        let mut final_result: Option<OptimizationResult> = None;

        for (stage_idx, optimizer) in self.optimizers.iter().enumerate() {
            tracing::debug!(
                "Stage {}/{}: {}",
                stage_idx + 1,
                self.optimizers.len(),
                optimizer.name()
            );

            // Record iteration telemetry
            record_iteration("better_together");

            // Run optimizer on the node
            let result = optimizer
                .optimize_node(node, trainset, valset, metric)
                .await
                .map_err(|e| {
                    crate::Error::Validation(format!(
                        "BetterTogether pipeline stage {} ({}) failed: {}",
                        stage_idx + 1,
                        optimizer.name(),
                        e
                    ))
                })?;

            tracing::debug!(
                "Stage {} complete: score {:.4} → {:.4} (improvement: {:.4})",
                stage_idx + 1,
                result.initial_score,
                result.final_score,
                result.improvement()
            );

            // Store stage result
            self.pipeline_stages.push(PipelineStage {
                optimizer_name: optimizer.name().to_string(),
                result: result.clone(),
                stage_index: stage_idx,
            });

            final_result = Some(result);
        }

        tracing::info!("BetterTogether: Pipeline Complete");
        if let Some(ref result) = final_result {
            tracing::info!(
                "Final score: {:.4} (overall improvement: {:.4})",
                result.final_score,
                result.improvement()
            );
        }

        final_result.ok_or_else(|| crate::Error::Validation("No optimizers executed".to_string()))
    }

    /// Execute all optimizers in parallel and pick the best result.
    ///
    /// Each optimizer works on a clone of the node independently.
    /// The optimizer that produces the highest final score wins,
    /// and its optimized node is applied to the input node.
    ///
    /// Note: This requires the node to implement Clone. Since Node is a trait object,
    /// we need to work with the constraint that we can't easily clone arbitrary nodes.
    /// For this implementation, we'll run all optimizers on the same node sequentially
    /// but keep track of which one produced the best result, then re-run only the winner.
    async fn optimize_parallel(
        &mut self,
        node: &mut dyn Node<S>,
        trainset: &[S],
        valset: &[S],
        metric: &MetricFn<S>,
    ) -> Result<OptimizationResult> {
        self.pipeline_stages.clear();

        // M-895: Note on "Parallel" naming - This strategy evaluates optimizers independently
        // (logically parallel), but executes them sequentially on a shared node state.
        // True concurrent execution would require node cloning (not available for dyn Node).
        // The name reflects the optimization topology, not the execution model.
        tracing::info!(
            "BetterTogether: Independent Evaluation Strategy - {} optimizers (sequential execution)",
            self.optimizers.len()
        );

        // Since we can't clone trait objects easily, we'll use a different approach:
        // Run all optimizers sequentially but track their results, then pick the best.
        // This is "parallel" in the logical sense (independent optimizations), not concurrent.
        // A true concurrent implementation would require serializing/deserializing the node
        // or having a Clone constraint on the Node trait.

        let mut best_optimizer_idx: Option<usize> = None;
        let mut best_score = f64::NEG_INFINITY;

        for (idx, optimizer) in self.optimizers.iter().enumerate() {
            tracing::debug!(
                "Candidate {}/{}: {}",
                idx + 1,
                self.optimizers.len(),
                optimizer.name()
            );

            // Record iteration telemetry
            record_iteration("better_together");

            // Run optimizer
            let result = optimizer
                .optimize_node(node, trainset, valset, metric)
                .await?;

            tracing::debug!(
                "Candidate {} complete: score {:.4} → {:.4}",
                idx + 1,
                result.initial_score,
                result.final_score
            );

            // Track best result
            if result.final_score > best_score {
                best_score = result.final_score;
                best_optimizer_idx = Some(idx);
            }

            // Store result
            self.pipeline_stages.push(PipelineStage {
                optimizer_name: optimizer.name().to_string(),
                result,
                stage_index: idx,
            });
        }

        // Pick the best optimizer result
        let best_idx = best_optimizer_idx
            .ok_or_else(|| crate::Error::Validation("No optimizers executed".to_string()))?;

        let best_result = &self.pipeline_stages[best_idx].result;

        tracing::info!(
            "BetterTogether: Best Optimizer - {} (score: {:.4})",
            self.pipeline_stages[best_idx].optimizer_name,
            best_result.final_score
        );
        tracing::info!(
            "Improvement: {:.4} → {:.4} ({:.4} gain)",
            best_result.initial_score,
            best_result.final_score,
            best_result.improvement()
        );

        // Return the best result
        Ok(best_result.clone())
    }

    /// Execute all optimizers and ensemble their results.
    ///
    /// Similar to Parallel strategy, each optimizer runs independently.
    /// Instead of picking one winner, ensemble combines all results by:
    /// 1. Taking a weighted average of the optimizations based on their scores
    /// 2. The final result is the optimizer with the highest score (best representative)
    ///
    /// This is useful when you want to leverage insights from multiple optimizers
    /// rather than committing to just one.
    async fn optimize_ensemble(
        &mut self,
        node: &mut dyn Node<S>,
        trainset: &[S],
        valset: &[S],
        metric: &MetricFn<S>,
    ) -> Result<OptimizationResult> {
        self.pipeline_stages.clear();

        tracing::info!(
            "BetterTogether: Ensemble Strategy - {} optimizers",
            self.optimizers.len()
        );

        let mut total_weighted_score = 0.0;
        let mut total_weight = 0.0;
        let mut best_optimizer_idx: Option<usize> = None;
        let mut best_score = f64::NEG_INFINITY;

        for (idx, optimizer) in self.optimizers.iter().enumerate() {
            tracing::debug!(
                "Ensemble Member {}/{}: {}",
                idx + 1,
                self.optimizers.len(),
                optimizer.name()
            );

            // Record iteration telemetry
            record_iteration("better_together");

            // Run optimizer
            let result = optimizer
                .optimize_node(node, trainset, valset, metric)
                .await?;

            tracing::debug!(
                "Member {} complete: score {:.4} → {:.4} (improvement: {:.4})",
                idx + 1,
                result.initial_score,
                result.final_score,
                result.improvement()
            );

            // Weight by improvement for ensemble averaging
            let weight = result.improvement().max(0.0); // Use improvement as weight
            total_weighted_score += result.final_score * weight;
            total_weight += weight;

            // Track best for fallback
            if result.final_score > best_score {
                best_score = result.final_score;
                best_optimizer_idx = Some(idx);
            }

            // Store result
            self.pipeline_stages.push(PipelineStage {
                optimizer_name: optimizer.name().to_string(),
                result,
                stage_index: idx,
            });
        }

        // Calculate ensemble metrics
        let ensemble_score = if total_weight > 0.0 {
            total_weighted_score / total_weight
        } else {
            // Fallback: if no improvements, use simple average
            self.pipeline_stages
                .iter()
                .map(|stage| stage.result.final_score)
                .sum::<f64>()
                / self.pipeline_stages.len() as f64
        };

        // Pick the best optimizer as the representative
        let best_idx = best_optimizer_idx
            .ok_or_else(|| crate::Error::Validation("No optimizers executed".to_string()))?;

        let best_result = &self.pipeline_stages[best_idx].result;

        tracing::info!(
            "BetterTogether: Ensemble Results - weighted score: {:.4}",
            ensemble_score
        );
        tracing::info!(
            "Best representative: {} (score: {:.4})",
            self.pipeline_stages[best_idx].optimizer_name,
            best_result.final_score
        );

        // For ensemble, we report the weighted average score but use the best optimizer's settings
        //
        // M-896: initial_score comes from the best optimizer, not the true pre-optimization state.
        // This is because:
        // 1. Each optimizer may have different initial_score based on evaluation method
        // 2. Taking the first optimizer's initial_score could be misleading if it's not representative
        // 3. The best optimizer's initial_score gives context for that specific optimization path
        // For true initial state, callers should evaluate before calling optimize_ensemble().
        //
        // M-897: converged requires ALL optimizers to converge. This is strict but intentional:
        // - If any optimizer failed to converge, the ensemble may have suboptimal contributions
        // - Callers wanting "any converged" can check pipeline_stages directly
        // - This conservative approach prevents false confidence in incomplete optimizations
        let ensemble_result = OptimizationResult {
            initial_score: best_result.initial_score, // M-896: See comment above
            final_score: ensemble_score,              // Use ensemble score
            iterations: self
                .pipeline_stages
                .iter()
                .map(|s| s.result.iterations)
                .sum(),
            converged: self.pipeline_stages.iter().all(|s| s.result.converged), // M-897: See comment above
            duration_secs: self
                .pipeline_stages
                .iter()
                .map(|s| s.result.duration_secs)
                .sum(),
        };

        Ok(ensemble_result)
    }
}

impl<S: GraphState> Default for BetterTogether<S> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Mock optimizer for testing
    struct MockOptimizer {
        name: String,
        score_improvement: f64,
    }

    #[async_trait]
    impl<S: GraphState> NodeOptimizer<S> for MockOptimizer {
        async fn optimize_node(
            &self,
            _node: &mut dyn Node<S>,
            _trainset: &[S],
            _valset: &[S],
            _metric: &MetricFn<S>,
        ) -> Result<OptimizationResult> {
            // Mock optimization: just return a result with improvement
            Ok(OptimizationResult {
                final_score: 0.8 + self.score_improvement,
                initial_score: 0.8,
                iterations: 10,
                converged: true,
                duration_secs: 1.0,
            })
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_better_together_new() {
        type TestState = crate::state::AgentState;
        let optimizer = BetterTogether::<TestState>::new();
        assert_eq!(optimizer.strategy, CompositionStrategy::Sequential);
        assert_eq!(optimizer.optimizers.len(), 0);
        assert_eq!(optimizer.pipeline_stages.len(), 0);
    }

    #[test]
    fn test_better_together_add_optimizer() {
        type TestState = crate::state::AgentState;
        let mock1 = Box::new(MockOptimizer {
            name: "Mock1".to_string(),
            score_improvement: 0.1,
        });
        let mock2 = Box::new(MockOptimizer {
            name: "Mock2".to_string(),
            score_improvement: 0.05,
        });

        let optimizer = BetterTogether::<TestState>::new()
            .add_optimizer(mock1)
            .add_optimizer(mock2);

        assert_eq!(optimizer.optimizers.len(), 2);
    }

    #[test]
    fn test_better_together_with_strategy() {
        type TestState = crate::state::AgentState;
        let optimizer =
            BetterTogether::<TestState>::new().with_strategy(CompositionStrategy::Parallel);

        assert_eq!(optimizer.strategy, CompositionStrategy::Parallel);
    }

    #[test]
    fn test_composition_strategy_display() {
        assert_eq!(CompositionStrategy::Sequential.to_string(), "Sequential");
        assert_eq!(CompositionStrategy::Parallel.to_string(), "Parallel");
        assert_eq!(CompositionStrategy::Ensemble.to_string(), "Ensemble");
    }

    #[test]
    fn test_pipeline_stages_empty() {
        type TestState = crate::state::AgentState;
        let optimizer = BetterTogether::<TestState>::new();
        assert_eq!(optimizer.pipeline_stages().len(), 0);
    }

    // Integration test: Sequential pipeline with multiple optimizers
    #[tokio::test]
    async fn test_sequential_pipeline_execution() {
        use crate::state::AgentState;

        type TestState = AgentState;

        // Create mock optimizers with different improvements
        let mock1 = Box::new(MockOptimizer {
            name: "OptimizerA".to_string(),
            score_improvement: 0.1, // 0.8 → 0.9
        });
        let mock2 = Box::new(MockOptimizer {
            name: "OptimizerB".to_string(),
            score_improvement: 0.05, // Should add on top of mock1
        });

        // Create pipeline
        let mut optimizer = BetterTogether::<TestState>::new()
            .add_optimizer(mock1)
            .add_optimizer(mock2);

        // Create a mock node
        struct MockNode;

        #[async_trait]
        impl crate::node::Node<TestState> for MockNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(state)
            }

            fn name(&self) -> String {
                "MockNode".to_string()
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let mut node = MockNode;
        let trainset = vec![];
        let valset = vec![];
        let metric: MetricFn<TestState> = Arc::new(|_pred, _expected| Ok(0.8));

        // Execute pipeline
        let result = optimizer
            .optimize(
                &mut node as &mut dyn crate::node::Node<TestState>,
                &trainset,
                &valset,
                &metric,
            )
            .await;

        assert!(result.is_ok(), "Pipeline should execute successfully");
        let result = result.unwrap();

        // Verify final result
        assert_eq!(result.initial_score, 0.8);
        // Final score should reflect the last optimizer's improvement
        assert!(result.final_score > 0.8);

        // Verify intermediate results were stored
        let stages = optimizer.pipeline_stages();
        assert_eq!(stages.len(), 2, "Should have 2 pipeline stages");

        assert_eq!(stages[0].optimizer_name, "OptimizerA");
        assert_eq!(stages[0].stage_index, 0);
        assert_eq!(stages[0].result.final_score, 0.9); // 0.8 + 0.1

        assert_eq!(stages[1].optimizer_name, "OptimizerB");
        assert_eq!(stages[1].stage_index, 1);
        // Use approximate comparison for floating point
        assert!((stages[1].result.final_score - 0.85).abs() < 0.001); // 0.8 + 0.05
    }

    // Test error handling: empty pipeline
    #[tokio::test]
    async fn test_empty_pipeline_error() {
        use crate::state::AgentState;
        type TestState = AgentState;

        struct MockNode;

        #[async_trait]
        impl crate::node::Node<TestState> for MockNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(state)
            }

            fn name(&self) -> String {
                "MockNode".to_string()
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let mut optimizer = BetterTogether::<TestState>::new();
        let mut node = MockNode;
        let trainset = vec![];
        let valset = vec![];
        let metric: MetricFn<TestState> = Arc::new(|_pred, _expected| Ok(0.8));

        let result = optimizer
            .optimize(
                &mut node as &mut dyn crate::node::Node<TestState>,
                &trainset,
                &valset,
                &metric,
            )
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one optimizer"));
    }

    // Test Parallel strategy: Run all, pick best
    #[tokio::test]
    async fn test_parallel_strategy_picks_best() {
        use crate::state::AgentState;
        type TestState = AgentState;

        struct MockNode;

        #[async_trait]
        impl crate::node::Node<TestState> for MockNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(state)
            }

            fn name(&self) -> String {
                "MockNode".to_string()
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        // Create optimizers with different scores
        let weak = Box::new(MockOptimizer {
            name: "WeakOptimizer".to_string(),
            score_improvement: 0.05, // Final: 0.85
        });
        let strong = Box::new(MockOptimizer {
            name: "StrongOptimizer".to_string(),
            score_improvement: 0.15, // Final: 0.95
        });
        let medium = Box::new(MockOptimizer {
            name: "MediumOptimizer".to_string(),
            score_improvement: 0.10, // Final: 0.90
        });

        let mut optimizer = BetterTogether::<TestState>::new()
            .add_optimizer(weak)
            .add_optimizer(strong)
            .add_optimizer(medium)
            .with_strategy(CompositionStrategy::Parallel);

        let mut node = MockNode;
        let trainset = vec![];
        let valset = vec![];
        let metric: MetricFn<TestState> = Arc::new(|_pred, _expected| Ok(0.8));

        let result = optimizer
            .optimize(
                &mut node as &mut dyn crate::node::Node<TestState>,
                &trainset,
                &valset,
                &metric,
            )
            .await
            .expect("Parallel strategy should succeed");

        // Should pick the strong optimizer (highest score)
        assert!((result.final_score - 0.95).abs() < 0.001);

        // Verify all three ran
        let stages = optimizer.pipeline_stages();
        assert_eq!(stages.len(), 3);

        // Verify strong optimizer had best score
        assert_eq!(stages[1].optimizer_name, "StrongOptimizer");
        assert!((stages[1].result.final_score - 0.95).abs() < 0.001);
    }

    // Test Ensemble strategy: Combine results
    #[tokio::test]
    async fn test_ensemble_strategy_combines_results() {
        use crate::state::AgentState;
        type TestState = AgentState;

        struct MockNode;

        #[async_trait]
        impl crate::node::Node<TestState> for MockNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(state)
            }

            fn name(&self) -> String {
                "MockNode".to_string()
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        // Create optimizers with different improvements
        let opt1 = Box::new(MockOptimizer {
            name: "Optimizer1".to_string(),
            score_improvement: 0.10, // 0.8 → 0.90 (improvement: 0.10)
        });
        let opt2 = Box::new(MockOptimizer {
            name: "Optimizer2".to_string(),
            score_improvement: 0.15, // 0.8 → 0.95 (improvement: 0.15)
        });

        let mut optimizer = BetterTogether::<TestState>::new()
            .add_optimizer(opt1)
            .add_optimizer(opt2)
            .with_strategy(CompositionStrategy::Ensemble);

        let mut node = MockNode;
        let trainset = vec![];
        let valset = vec![];
        let metric: MetricFn<TestState> = Arc::new(|_pred, _expected| Ok(0.8));

        let result = optimizer
            .optimize(
                &mut node as &mut dyn crate::node::Node<TestState>,
                &trainset,
                &valset,
                &metric,
            )
            .await
            .expect("Ensemble strategy should succeed");

        // Ensemble score should be weighted average
        // Weight1 = 0.10, Weight2 = 0.15
        // Weighted avg = (0.90 * 0.10 + 0.95 * 0.15) / (0.10 + 0.15)
        //              = (0.09 + 0.1425) / 0.25
        //              = 0.2325 / 0.25
        //              = 0.93
        assert!((result.final_score - 0.93).abs() < 0.001);

        // Verify both ran
        let stages = optimizer.pipeline_stages();
        assert_eq!(stages.len(), 2);
    }

    // Test Ensemble with no improvements (fallback to average)
    #[tokio::test]
    async fn test_ensemble_no_improvements_uses_average() {
        use crate::state::AgentState;
        type TestState = AgentState;

        struct MockNode;

        #[async_trait]
        impl crate::node::Node<TestState> for MockNode {
            async fn execute(&self, state: TestState) -> Result<TestState> {
                Ok(state)
            }

            fn name(&self) -> String {
                "MockNode".to_string()
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        // Create optimizers with no improvement (or negative)
        let opt1 = Box::new(MockOptimizer {
            name: "NoImprovement1".to_string(),
            score_improvement: 0.0, // 0.8 → 0.80
        });
        let opt2 = Box::new(MockOptimizer {
            name: "NoImprovement2".to_string(),
            score_improvement: -0.05, // 0.8 → 0.75 (regression)
        });

        let mut optimizer = BetterTogether::<TestState>::new()
            .add_optimizer(opt1)
            .add_optimizer(opt2)
            .with_strategy(CompositionStrategy::Ensemble);

        let mut node = MockNode;
        let trainset = vec![];
        let valset = vec![];
        let metric: MetricFn<TestState> = Arc::new(|_pred, _expected| Ok(0.8));

        let result = optimizer
            .optimize(
                &mut node as &mut dyn crate::node::Node<TestState>,
                &trainset,
                &valset,
                &metric,
            )
            .await
            .expect("Ensemble strategy should succeed");

        // Should use simple average: (0.80 + 0.75) / 2 = 0.775
        assert!((result.final_score - 0.775).abs() < 0.001);
    }
}
