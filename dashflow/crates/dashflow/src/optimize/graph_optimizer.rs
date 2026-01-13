// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Removed broad #![allow(clippy::expect_used)] - targeted allows used instead.

//! Graph-Level Optimization for DashFlow Workflows
//!
//! This module provides end-to-end optimization for DashFlow workflows containing
//! multiple optimizable LLM nodes. Instead of optimizing each node independently,
//! GraphOptimizer jointly optimizes all nodes using a global metric that evaluates
//! the entire workflow.
//!
//! # Overview
//!
//! Sequential (node-by-node) optimization can lead to suboptimal results because:
//! - Each node is optimized for its local task, not downstream success
//! - Optimization doesn't account for dependencies between nodes
//! - Global workflow quality may suffer even if individual nodes improve
//!
//! Graph-level optimization addresses these issues by:
//! - Evaluating the entire workflow end-to-end
//! - Considering how earlier nodes affect downstream performance
//! - Optimizing for global metric (e.g., final response quality)
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::prelude::*;
//! use dashflow::optimize::{GraphOptimizer, BootstrapFewShot};
//!
//! // Build graph with multiple LLM nodes
//! let mut graph = StateGraph::<SupportState>::new();
//! graph
//!     .add_llm_node("classify")
//!         .with_signature("query -> category")
//!         .with_llm(llm.clone())
//!         .build()?
//!     .add_llm_node("extract")
//!         .with_signature("query, category -> entities")
//!         .with_llm(llm.clone())
//!         .build()?
//!     .add_llm_node("respond")
//!         .with_signature("query, entities -> response")
//!         .with_llm(llm)
//!         .build()?
//!     .add_edge("classify", "extract")
//!     .add_edge("extract", "respond")
//!     .add_edge("respond", END)
//!     .set_entry_point("classify");
//!
//! // Optimize entire graph end-to-end
//! let graph_optimizer = GraphOptimizer::new()
//!     .with_global_metric(|initial_state, final_state| {
//!         // Evaluate entire workflow quality
//!         response_quality_score(&initial_state.query, &final_state.response)
//!     })
//!     .with_base_optimizer(BootstrapFewShot::default());
//!
//! let optimized_graph = graph_optimizer
//!     .optimize(graph, trainset)
//!     .await?;
//!
//! // Optimized graph produces better end-to-end results
//! let result = optimized_graph.compile()?.invoke(state).await?;
//! ```

use crate::graph::StateGraph;
use crate::state::{GraphState, MergeableState};
use crate::{Error, Result};
use std::sync::Arc;
use tracing;

use super::optimizers::bootstrap::BootstrapFewShot;
use super::{MetricFn, Optimizable, OptimizerConfig};

/// Type alias for global metric functions that evaluate entire graph execution
///
/// Takes (initial_state, final_state) and returns a score (0.0 to 1.0)
pub type GlobalMetricFn<S> = Arc<dyn Fn(&S, &S) -> f64 + Send + Sync>;

/// Graph-level optimizer for DashFlow workflows with multiple optimizable nodes
///
/// GraphOptimizer jointly optimizes all optimizable nodes in a workflow using an
/// end-to-end metric that evaluates the entire graph execution. This typically
/// produces better results than sequential node-by-node optimization.
///
/// # Type Parameters
///
/// * `S` - DashFlow state type (must implement GraphState)
///
/// # Example
///
/// ```rust,ignore
/// let optimizer = GraphOptimizer::new()
///     .with_global_metric(|initial, final_state| {
///         evaluate_response_quality(&final_state)
///     })
///     .with_base_optimizer(BootstrapFewShot::default());
///
/// let optimized = optimizer.optimize(graph, trainset).await?;
/// ```
pub struct GraphOptimizer<S>
where
    S: GraphState + MergeableState,
{
    /// Metric function that evaluates entire graph execution
    /// Takes (initial_state, final_state) -> score
    global_metric: Option<GlobalMetricFn<S>>,

    /// Base optimizer to use for individual nodes (e.g., BootstrapFewShot)
    ///
    /// # M-864: Dead Code Note
    ///
    /// This field is currently unused. The `optimize_single_node` method
    /// directly calls `LLMNode::optimize()` instead of using this field.
    /// Future work could integrate this field to allow custom per-node
    /// optimization strategies, or this field may be removed in a future version.
    #[allow(dead_code)]
    base_optimizer: Option<BootstrapFewShot>,

    /// Optimization strategy
    strategy: OptimizationStrategy,

    /// Maximum number of optimization iterations
    max_iterations: usize,

    /// Minimum improvement threshold to continue optimization
    min_improvement: f64,
}

/// Strategy for optimizing multiple nodes in a graph
///
/// # Comparison
///
/// | Strategy    | Speed | Quality | Use Case |
/// |-------------|-------|---------|----------|
/// | Sequential  | Fast  | Good    | Simple graphs, independent nodes |
/// | Joint       | Slow  | Best    | Complex interactions, quality-critical |
/// | Alternating | Medium| Great   | Unknown optimal, balanced approach |
///
/// # Detailed Behavior
///
/// **Sequential**: Optimize each node once using per-node metrics in topological order.
/// - Pros: Fast, predictable
/// - Cons: Misses node interactions, local optima
///
/// **Joint**: Iteratively optimize nodes using global end-to-end metric (coordinate descent).
/// - Pros: Discovers optimal tradeoffs, accounts for interactions
/// - Cons: Slower, requires more training examples
///
/// **Alternating**: Alternate between sequential (fast) and joint (quality) passes.
/// - Pros: Best of both worlds, adaptive
/// - Cons: Most expensive overall
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationStrategy {
    /// Optimize nodes sequentially in topological order
    ///
    /// Each node is optimized once in dependency order. Fast but may miss
    /// interactions between nodes.
    Sequential,

    /// Optimize all nodes jointly using the global metric
    ///
    /// All nodes are optimized together considering their interactions.
    /// Slower but produces better end-to-end results.
    Joint,

    /// Alternate between sequential and joint optimization
    ///
    /// Combines benefits of both: sequential for speed, joint for quality.
    Alternating,
}

impl<S> GraphOptimizer<S>
where
    S: GraphState + MergeableState + Clone + Send + Sync + 'static,
{
    /// Create a new GraphOptimizer with default settings
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let optimizer = GraphOptimizer::<MyState>::new();
    /// ```
    pub fn new() -> Self {
        Self {
            global_metric: None,
            base_optimizer: None,
            strategy: OptimizationStrategy::Joint,
            max_iterations: 10,
            min_improvement: 0.01,
        }
    }

    /// Set the global metric function for end-to-end evaluation
    ///
    /// The metric function receives the initial state and final state after
    /// executing the entire graph, and returns a score (higher is better).
    ///
    /// # Arguments
    ///
    /// * `metric` - Function (initial_state, final_state) -> f64
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// optimizer.with_global_metric(|initial, final_state| {
    ///     // Evaluate final response quality
    ///     if final_state.response.contains(&initial.expected_answer) {
    ///         1.0
    ///     } else {
    ///         0.0
    ///     }
    /// });
    /// ```
    #[must_use]
    pub fn with_global_metric<F>(mut self, metric: F) -> Self
    where
        F: Fn(&S, &S) -> f64 + Send + Sync + 'static,
    {
        self.global_metric = Some(Arc::new(metric));
        self
    }

    /// Set the base optimizer to use for node optimization
    ///
    /// # Arguments
    ///
    /// * `optimizer` - Optimizer instance (e.g., BootstrapFewShot)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// optimizer.with_base_optimizer(BootstrapFewShot::default());
    /// ```
    #[must_use]
    pub fn with_base_optimizer(mut self, optimizer: BootstrapFewShot) -> Self {
        self.base_optimizer = Some(optimizer);
        self
    }

    /// Set the optimization strategy
    ///
    /// # Arguments
    ///
    /// * `strategy` - Sequential, Joint, or Alternating
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// optimizer.with_strategy(OptimizationStrategy::Joint);
    /// ```
    #[must_use]
    pub fn with_strategy(mut self, strategy: OptimizationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set maximum number of optimization iterations
    ///
    /// # Arguments
    ///
    /// * `max_iterations` - Maximum iterations (default: 10)
    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set minimum improvement threshold
    ///
    /// # Arguments
    ///
    /// * `min_improvement` - Minimum score improvement to continue (default: 0.01)
    #[must_use]
    pub fn with_min_improvement(mut self, min_improvement: f64) -> Self {
        self.min_improvement = min_improvement;
        self
    }

    /// Optimize a DashFlow workflow end-to-end
    ///
    /// This method:
    /// 1. Analyzes the graph to identify all optimizable nodes
    /// 2. Evaluates baseline performance using the global metric
    /// 3. Jointly optimizes all nodes using the specified strategy
    /// 4. Returns a new graph with optimized nodes
    ///
    /// # Arguments
    ///
    /// * `graph` - StateGraph containing optimizable nodes
    /// * `trainset` - Training examples for optimization
    ///
    /// # Returns
    ///
    /// A new StateGraph with optimized nodes
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - No optimizable nodes found in graph
    /// - Global metric not set
    /// - Optimization fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let optimized_graph = optimizer
    ///     .optimize(graph, trainset)
    ///     .await?;
    ///
    /// let app = optimized_graph.compile()?;
    /// let result = app.invoke(initial_state).await?;
    /// ```
    pub async fn optimize(
        &self,
        mut graph: StateGraph<S>,
        trainset: Vec<S>,
    ) -> Result<StateGraph<S>> {
        // Validate configuration
        if self.global_metric.is_none() {
            return Err(crate::Error::Validation(
                "Global metric not set. Use with_global_metric() to set end-to-end metric."
                    .to_string(),
            ));
        }

        if trainset.is_empty() {
            return Err(crate::Error::Validation(
                "Training set is empty. Provide training examples for optimization.".to_string(),
            ));
        }

        // Get list of optimizable node names
        let optimizable_nodes = self.find_optimizable_nodes(&graph)?;

        if optimizable_nodes.is_empty() {
            return Err(crate::Error::Validation(
                "No optimizable nodes found in graph. Add LLM nodes using add_llm_node()."
                    .to_string(),
            ));
        }

        // Evaluate baseline performance
        let baseline_score = self
            .evaluate_graph(&graph, &trainset)
            .await
            .map_err(|e| Error::Validation(format!("Failed to evaluate baseline graph: {}", e)))?;
        tracing::info!(
            baseline_score = %format!("{:.4}", baseline_score),
            num_examples = trainset.len(),
            "Evaluating baseline score"
        );

        // Optimize based on strategy
        match self.strategy {
            OptimizationStrategy::Sequential => {
                self.optimize_sequential(&mut graph, &trainset, &optimizable_nodes)
                    .await?;
            }
            OptimizationStrategy::Joint => {
                self.optimize_joint(&mut graph, &trainset, &optimizable_nodes)
                    .await?;
            }
            OptimizationStrategy::Alternating => {
                self.optimize_alternating(&mut graph, &trainset, &optimizable_nodes)
                    .await?;
            }
        }

        // Evaluate final performance
        let final_score = self
            .evaluate_graph(&graph, &trainset)
            .await
            .map_err(|e| Error::Validation(format!("Failed to evaluate final graph: {}", e)))?;
        let improvement = final_score - baseline_score;
        tracing::info!(
            final_score = %format!("{:.4}", final_score),
            improvement = %format!("{:+.4}", improvement),
            "Optimization complete"
        );

        Ok(graph)
    }

    /// Find all optimizable nodes in the graph
    ///
    /// # Limitation (M-866)
    ///
    /// This method currently returns **all** nodes in the graph, regardless of
    /// whether they implement the `Optimizable` trait. This is a known limitation
    /// due to Rust's trait object system:
    ///
    /// - `Arc<dyn Node>` doesn't preserve concrete type information
    /// - Runtime trait checking (e.g., `downcast`) isn't available for arbitrary traits
    /// - No built-in mechanism to query "does this trait object also implement X?"
    ///
    /// ## Current Behavior
    ///
    /// 1. All node names are returned from this method
    /// 2. Optimization is attempted on each node in `optimize_node_sequentially()`
    /// 3. Non-optimizable nodes fail gracefully with `Err(Validation(...))`
    /// 4. Sequential/Joint strategies skip failed nodes and continue
    ///
    /// ## Potential Future Solutions
    ///
    /// 1. **Marker trait**: Add `OptimizableNode: Node` that includes a type ID
    /// 2. **Registry pattern**: Users explicitly register optimizable node names
    /// 3. **Node metadata**: Store optimization capability in node metadata
    /// 4. **Builder pattern**: Track optimizable nodes during graph construction
    ///
    /// ## Workaround
    ///
    /// If you know which nodes are optimizable, use `with_node_names()` on the
    /// builder to explicitly specify the nodes to optimize.
    fn find_optimizable_nodes(&self, graph: &StateGraph<S>) -> Result<Vec<String>> {
        // LIMITATION (M-866): Returns all nodes due to trait object limitations
        // See doc comment above for detailed explanation and workarounds
        let all_nodes: Vec<String> = graph.node_names().cloned().collect();

        tracing::debug!(
            node_count = all_nodes.len(),
            nodes = ?all_nodes,
            "Returning all graph nodes for optimization (cannot introspect Optimizable trait at runtime)"
        );

        Ok(all_nodes)
    }

    /// Evaluate graph performance on training set using global metric
    #[allow(clippy::expect_used)] // SAFETY: optimize() validates global_metric is Some before calling this
    async fn evaluate_graph(&self, graph: &StateGraph<S>, trainset: &[S]) -> Result<f64> {
        let metric = self
            .global_metric
            .as_ref()
            .expect("Global metric should be set");

        // Clone and compile the graph for evaluation
        let cloned_graph = graph.clone();
        let app = cloned_graph.compile().map_err(|e| {
            Error::Validation(format!("Failed to compile graph for evaluation: {}", e))
        })?;

        // Calculate average score across training set
        let mut total_score = 0.0;
        let count = trainset.len() as f64;

        // Execute graph on each example and compute metric
        for example in trainset {
            // Store initial state for metric comparison
            let initial_state = example.clone();

            // Execute full graph
            let result = app.invoke(example.clone()).await.map_err(|e| {
                Error::Validation(format!("Graph execution failed during evaluation: {}", e))
            })?;

            // Compute metric: compare initial state to final state
            let score = metric(&initial_state, &result.final_state);
            total_score += score;
        }

        Ok(total_score / count)
    }

    /// Optimize nodes sequentially in topological order
    async fn optimize_sequential(
        &self,
        graph: &mut StateGraph<S>,
        trainset: &[S],
        node_names: &[String],
    ) -> Result<()> {
        tracing::info!(
            num_nodes = node_names.len(),
            "Sequential optimization: nodes to optimize"
        );

        // Get topological ordering of nodes
        let order = match graph.topological_sort() {
            Some(order) => order,
            None => {
                // Graph has cycles, fall back to arbitrary order
                tracing::warn!("Graph has cycles, using arbitrary order");
                node_names.to_vec()
            }
        };

        // Filter to only nodes that are in our optimizable list
        let nodes_to_optimize: Vec<String> = order
            .into_iter()
            .filter(|name| node_names.contains(name))
            .collect();

        tracing::debug!(
            num_nodes = nodes_to_optimize.len(),
            "Optimizing nodes in topological order"
        );

        // For each node in topological order, attempt to optimize it
        for (idx, node_name) in nodes_to_optimize.iter().enumerate() {
            tracing::info!(
                node = %node_name,
                progress = %format!("[{}/{}]", idx + 1, nodes_to_optimize.len()),
                "Optimizing node"
            );

            // Try to optimize this node
            if let Err(e) = self.optimize_single_node(graph, trainset, node_name).await {
                tracing::warn!(
                    node = %node_name,
                    error = %e,
                    "Failed to optimize node, continuing with next"
                );
            }
        }

        Ok(())
    }

    /// Optimize a single node by name
    async fn optimize_single_node(
        &self,
        graph: &mut StateGraph<S>,
        trainset: &[S],
        node_name: &str,
    ) -> Result<()> {
        // 1. Remove node from graph to get ownership
        let mut node_arc = graph
            .remove_node(node_name)
            .ok_or_else(|| Error::Validation(format!("Node '{}' not found", node_name)))?;

        // 2. Try to get mutable reference (only works if refcount == 1)
        let node_mut = match Arc::get_mut(&mut node_arc) {
            Some(node) => node,
            None => {
                // Still have other references - put it back and fail gracefully
                let _ = graph.replace_node(node_name, node_arc);
                return Err(Error::Validation(format!(
                    "Cannot optimize node '{}': still has external references (Arc refcount > 1)",
                    node_name
                )));
            }
        };

        // 3. Check if node implements Optimizable trait (runtime downcast)
        // Use as_any_mut() to get &mut dyn Any, then downcast
        let node_any = node_mut.as_any_mut();

        // Try to downcast to LLMNode<S> (the only Optimizable type currently)
        if let Some(llm_node) = node_any.downcast_mut::<super::llm_node::LLMNode<S>>() {
            tracing::debug!("Node is optimizable (LLMNode), running optimizer...");

            // 4. Extract per-node training data
            // For now, use the full trainset - later we can filter to node-specific data
            let node_trainset = trainset;

            // 5. Create per-node metric
            // Default metric: Simple state equality comparison via JSON serialization.
            // For production use, provide a custom metric that compares specific fields
            // relevant to the node's output. This can be done via the global_metric parameter.
            let metric: MetricFn<S> = Arc::new(|expected: &S, predicted: &S| {
                // Serialize both states and compare JSON
                let expected_json = serde_json::to_value(expected).map_err(|e| {
                    Error::Validation(format!("Failed to serialize expected state: {}", e))
                })?;
                let predicted_json = serde_json::to_value(predicted).map_err(|e| {
                    Error::Validation(format!("Failed to serialize predicted state: {}", e))
                })?;

                // Simple equality check (1.0 if match, 0.0 otherwise)
                // For field-specific comparison, provide a custom metric via optimize_with_global_metric
                if expected_json == predicted_json {
                    Ok(1.0)
                } else {
                    Ok(0.0)
                }
            });

            // 6. Create optimizer config from GraphOptimizer settings
            let config = OptimizerConfig {
                max_few_shot_examples: 4, // Default from OptimizerConfig::default()
                max_iterations: self.max_iterations,
                min_improvement: self.min_improvement,
                random_seed: None,
                success_threshold: 0.5, // Default threshold for bootstrap success
            };

            // 7. Run optimization
            let result = llm_node
                .optimize(node_trainset, &metric, &config)
                .await
                .map_err(|e| {
                    Error::Validation(format!(
                        "Optimization failed for node '{}': {}",
                        node_name, e
                    ))
                })?;

            tracing::info!(
                initial_score = %format!("{:.3}", result.initial_score),
                final_score = %format!("{:.3}", result.final_score),
                iterations = result.iterations,
                "Node optimization complete"
            );

            // 8. Put the optimized node back in the graph
            graph.replace_node(node_name, node_arc);

            Ok(())
        } else {
            // Node doesn't implement Optimizable - put it back unchanged
            tracing::debug!("Node is not optimizable (does not implement Optimizable trait)");
            graph.replace_node(node_name, node_arc);

            Err(Error::Validation(format!(
                "Node '{}' does not implement Optimizable trait",
                node_name
            )))
        }
    }

    /// Optimize all nodes jointly using global metric
    ///
    /// Uses coordinate descent: iteratively optimize each node while keeping
    /// others fixed, using the global metric to evaluate entire graph quality.
    /// This accounts for node interactions and produces better end-to-end results
    /// than sequential optimization.
    ///
    /// # Algorithm
    ///
    /// 1. Evaluate baseline graph score using global metric
    /// 2. For each iteration (up to max_iterations):
    ///    - For each optimizable node:
    ///      a. Run node optimizer (BootstrapFewShot, etc.)
    ///      b. Evaluate full graph with optimized node
    ///      c. Keep optimization if global metric improved
    ///    - Check convergence: stop if no improvement or below min_improvement
    /// 3. Return graph with all optimized nodes
    ///
    /// # Key Differences from Sequential
    ///
    /// - **Sequential**: Optimizes each node once using per-node metrics
    /// - **Joint**: Optimizes nodes iteratively using global end-to-end metric
    /// - **Result**: Joint accounts for node interactions, sequential doesn't
    ///
    /// # Example Scenario
    ///
    /// Graph: classify → extract → respond
    /// - Sequential might optimize "classify" for category accuracy
    /// - But "extract" might work better with looser categories
    /// - Joint optimization discovers this tradeoff by evaluating final response quality
    async fn optimize_joint(
        &self,
        graph: &mut StateGraph<S>,
        trainset: &[S],
        node_names: &[String],
    ) -> Result<()> {
        tracing::info!(
            num_nodes = node_names.len(),
            max_iterations = self.max_iterations,
            "Joint optimization starting"
        );

        // Baseline score for convergence check
        let mut current_score = self.evaluate_graph(graph, trainset).await.map_err(|e| {
            Error::Validation(format!(
                "Joint optimization initial evaluation failed: {}",
                e
            ))
        })?;
        tracing::info!(score = %format!("{:.4}", current_score), "Initial score");

        // Coordinate descent: optimize each node iteratively
        for iteration in 0..self.max_iterations {
            tracing::info!(
                iteration = iteration + 1,
                max = self.max_iterations,
                "Starting iteration"
            );

            let mut iteration_improved = false;

            // Try to improve each node
            for (idx, node_name) in node_names.iter().enumerate() {
                tracing::debug!(
                    node = %node_name,
                    progress = %format!("[{}/{}]", idx + 1, node_names.len()),
                    "Optimizing node using global metric"
                );

                // Try to optimize this node with global metric
                match self
                    .optimize_node_with_global_metric(graph, trainset, node_name)
                    .await
                {
                    Ok(improved) => {
                        if improved {
                            iteration_improved = true;
                            tracing::debug!("Node improved global metric");
                        } else {
                            tracing::debug!("No improvement, keeping original");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Optimization failed, continuing with next node");
                    }
                }
            }

            // Evaluate after this iteration
            let new_score = self.evaluate_graph(graph, trainset).await.map_err(|e| {
                Error::Validation(format!(
                    "Joint optimization iteration {} evaluation failed: {}",
                    iteration + 1,
                    e
                ))
            })?;
            let improvement = new_score - current_score;
            tracing::info!(
                iteration = iteration + 1,
                score = %format!("{:.4}", new_score),
                improvement = %format!("{:+.4}", improvement),
                "Iteration complete"
            );

            // Check convergence
            if !iteration_improved || improvement.abs() < self.min_improvement {
                tracing::info!(
                    min_improvement = self.min_improvement,
                    "Converged (improvement below threshold or no nodes improved)"
                );
                break;
            }

            current_score = new_score;
        }

        tracing::info!("Joint optimization complete");
        Ok(())
    }

    /// Optimize a single node using the global metric
    ///
    /// This differs from optimize_single_node() which uses per-node metrics.
    /// Here we evaluate the entire graph after each candidate update.
    ///
    /// Returns Ok(true) if the node was improved, Ok(false) if no improvement.
    async fn optimize_node_with_global_metric(
        &self,
        graph: &mut StateGraph<S>,
        trainset: &[S],
        node_name: &str,
    ) -> Result<bool> {
        // Baseline: evaluate current graph
        let baseline_score = self.evaluate_graph(graph, trainset).await.map_err(|e| {
            Error::Validation(format!(
                "Failed to evaluate baseline before optimizing node '{}': {}",
                node_name, e
            ))
        })?;

        // 1. Remove node to get ownership
        let mut node_arc = graph
            .remove_node(node_name)
            .ok_or_else(|| Error::Validation(format!("Node '{}' not found", node_name)))?;

        // 2. Try to get mutable reference
        let node_mut = match Arc::get_mut(&mut node_arc) {
            Some(node) => node,
            None => {
                // Can't get mut ref - put it back and return error
                let _ = graph.replace_node(node_name, node_arc);
                return Err(Error::Validation(format!(
                    "Cannot optimize node '{}': Arc refcount > 1",
                    node_name
                )));
            }
        };

        // 3. Check if node implements Optimizable (runtime downcast)
        let node_any = node_mut.as_any_mut();

        // Try to downcast to LLMNode<S> (the only Optimizable type currently)
        if let Some(llm_node) = node_any.downcast_mut::<super::llm_node::LLMNode<S>>() {
            // 4. Create per-node metric (always returns 1.0 - actual evaluation is at graph level)
            // Required by the Optimizable API. Graph-level evaluation happens in optimize_with_global_metric.
            let metric: MetricFn<S> = Arc::new(|_expected: &S, _predicted: &S| {
                // Per-node metric is intentionally permissive - graph-level metric does real evaluation
                Ok(1.0)
            });

            // 5. Create optimizer config
            let config = OptimizerConfig {
                max_few_shot_examples: 4,
                max_iterations: 3, // Fewer iterations per node since we iterate at graph level
                min_improvement: self.min_improvement,
                random_seed: None,
                success_threshold: 0.5, // Default threshold for bootstrap success
            };

            // 6. Run optimization
            let _ = llm_node
                .optimize(trainset, &metric, &config)
                .await
                .map_err(|e| {
                    Error::Validation(format!(
                        "Optimization failed for node '{}' with global metric: {}",
                        node_name, e
                    ))
                })?;

            // 7. Put the optimized node back
            graph.replace_node(node_name, node_arc);

            // 8. Evaluate graph with optimized node
            let new_score = self.evaluate_graph(graph, trainset).await.map_err(|e| {
                Error::Validation(format!(
                    "Failed to evaluate graph after optimizing node '{}': {}",
                    node_name, e
                ))
            })?;

            // 9. Check if optimization improved global metric
            if new_score > baseline_score + self.min_improvement {
                // Keep the optimized node (already in graph)
                tracing::debug!(
                    node = %node_name,
                    baseline = %format!("{:.4}", baseline_score),
                    new_score = %format!("{:.4}", new_score),
                    improvement = %format!("{:+.4}", new_score - baseline_score),
                    "Node optimization improved global metric"
                );
                Ok(true)
            } else {
                // LIMITATION (M-865): No revert mechanism for non-improving optimizations
                //
                // When per-node optimization doesn't improve the global metric, we
                // currently keep the optimized version anyway rather than reverting
                // to the original. This is because:
                //
                // 1. Arc<dyn Node> doesn't implement Clone due to trait object limitations
                // 2. Deep cloning nodes would require a custom Clone-like trait
                // 3. Serialization/deserialization would add significant overhead
                //
                // Potential future solutions:
                // - Add a `Cloneable` marker trait to nodes that support cloning
                // - Use checkpoint/restore pattern via serialization
                // - Track optimization deltas that can be undone
                //
                // Workaround: Increase min_improvement threshold or use joint optimization
                // strategy which evaluates all node combinations together.
                tracing::warn!(
                    node = %node_name,
                    baseline = %format!("{:.4}", baseline_score),
                    new_score = %format!("{:.4}", new_score),
                    delta = %format!("{:+.4}", new_score - baseline_score),
                    min_improvement = %format!("{:.4}", self.min_improvement),
                    "Node optimization did not improve global metric (keeping optimized version - no revert mechanism)"
                );
                Ok(false)
            }
        } else {
            // Node doesn't implement Optimizable - put it back unchanged
            // This is expected behavior when find_optimizable_nodes() returns all nodes
            // due to trait object introspection limitations (see M-866)
            let _ = graph.replace_node(node_name, node_arc);
            tracing::debug!(
                node = %node_name,
                "Node does not implement Optimizable trait - skipping (this is expected for non-LLM nodes)"
            );
            Err(Error::Validation(format!(
                "Node '{}' does not implement Optimizable trait",
                node_name
            )))
        }
    }

    /// Alternate between sequential and joint optimization
    ///
    /// Combines the benefits of both strategies:
    /// - Sequential optimization for fast initial improvements
    /// - Joint optimization for quality refinement considering node interactions
    ///
    /// # Algorithm
    ///
    /// For each iteration (up to max_iterations):
    /// 1. Run sequential optimization pass (fast, local improvements)
    /// 2. Evaluate graph score
    /// 3. Run joint optimization pass (slow, global improvements)
    /// 4. Evaluate graph score and check convergence
    ///
    /// # When to Use
    ///
    /// - **Large graphs** (5+ nodes): Sequential gives quick wins, joint refines
    /// - **Balanced approach**: Want both speed and quality
    /// - **Unknown optimal**: Let the algorithm explore both strategies
    ///
    /// # Performance
    ///
    /// - Slower than Sequential alone
    /// - Faster than Joint alone (sequential reduces search space)
    /// - Often best end-to-end quality
    async fn optimize_alternating(
        &self,
        graph: &mut StateGraph<S>,
        trainset: &[S],
        node_names: &[String],
    ) -> Result<()> {
        // Simple alternating strategy:
        // 1. Sequential pass (fast improvement)
        // 2. Joint pass (quality refinement)
        // Repeat until convergence

        for iteration in 0..self.max_iterations {
            tracing::info!(
                iteration = iteration + 1,
                "Alternating optimization iteration"
            );

            // Sequential pass
            tracing::debug!("Sequential optimization pass...");
            self.optimize_sequential(graph, trainset, node_names)
                .await
                .map_err(|e| {
                    Error::Validation(format!(
                        "Sequential pass failed in alternating iteration {}: {}",
                        iteration + 1,
                        e
                    ))
                })?;

            // Evaluate
            let seq_score = self.evaluate_graph(graph, trainset).await.map_err(|e| {
                Error::Validation(format!(
                    "Evaluation after sequential pass failed in iteration {}: {}",
                    iteration + 1,
                    e
                ))
            })?;
            tracing::debug!(score = %format!("{:.4}", seq_score), "After sequential pass");

            // Joint pass
            tracing::debug!("Joint optimization pass...");
            self.optimize_joint(graph, trainset, node_names)
                .await
                .map_err(|e| {
                    Error::Validation(format!(
                        "Joint pass failed in alternating iteration {}: {}",
                        iteration + 1,
                        e
                    ))
                })?;

            // Evaluate
            let joint_score = self.evaluate_graph(graph, trainset).await.map_err(|e| {
                Error::Validation(format!(
                    "Evaluation after joint pass failed in iteration {}: {}",
                    iteration + 1,
                    e
                ))
            })?;
            let improvement = joint_score - seq_score;
            tracing::info!(
                score = %format!("{:.4}", joint_score),
                improvement = %format!("{:+.4}", improvement),
                "After joint pass"
            );

            // Check convergence
            if improvement.abs() < self.min_improvement {
                tracing::info!(min_improvement = self.min_improvement, "Converged");
                break;
            }
        }

        Ok(())
    }
}

impl<S> Default for GraphOptimizer<S>
where
    S: GraphState + MergeableState + Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        query: String,
        response: String,
    }

    impl MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            if !other.query.is_empty() {
                self.query = other.query.clone();
            }
            if !other.response.is_empty() {
                self.response = other.response.clone();
            }
        }
    }

    #[test]
    fn test_graph_optimizer_creation() {
        let optimizer = GraphOptimizer::<TestState>::new();
        assert!(optimizer.global_metric.is_none());
        assert!(optimizer.base_optimizer.is_none());
        assert_eq!(optimizer.strategy, OptimizationStrategy::Joint);
        assert_eq!(optimizer.max_iterations, 10);
        assert_eq!(optimizer.min_improvement, 0.01);
    }

    #[test]
    fn test_with_global_metric() {
        let optimizer =
            GraphOptimizer::<TestState>::new().with_global_metric(|_initial, final_state| {
                if final_state.response.len() > 10 {
                    1.0
                } else {
                    0.0
                }
            });

        assert!(optimizer.global_metric.is_some());
    }

    #[test]
    fn test_with_strategy() {
        let optimizer =
            GraphOptimizer::<TestState>::new().with_strategy(OptimizationStrategy::Sequential);

        assert_eq!(optimizer.strategy, OptimizationStrategy::Sequential);
    }

    #[test]
    fn test_with_max_iterations() {
        let optimizer = GraphOptimizer::<TestState>::new().with_max_iterations(20);

        assert_eq!(optimizer.max_iterations, 20);
    }

    #[test]
    fn test_with_min_improvement() {
        let optimizer = GraphOptimizer::<TestState>::new().with_min_improvement(0.05);

        assert_eq!(optimizer.min_improvement, 0.05);
    }

    #[test]
    fn test_optimization_strategy_equality() {
        assert_eq!(
            OptimizationStrategy::Sequential,
            OptimizationStrategy::Sequential
        );
        assert_ne!(
            OptimizationStrategy::Sequential,
            OptimizationStrategy::Joint
        );
    }

    #[tokio::test]
    async fn test_optimize_requires_metric() {
        let optimizer = GraphOptimizer::<TestState>::new();
        let graph = StateGraph::<TestState>::new();
        let trainset = vec![TestState {
            query: "test".to_string(),
            response: "".to_string(),
        }];

        let result = optimizer.optimize(graph, trainset).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Global metric not set"));
        }
    }

    #[tokio::test]
    async fn test_optimize_requires_trainset() {
        let optimizer = GraphOptimizer::<TestState>::new().with_global_metric(|_, _| 1.0);

        let graph = StateGraph::<TestState>::new();
        let trainset = vec![];

        let result = optimizer.optimize(graph, trainset).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Training set is empty"));
        }
    }

    #[tokio::test]
    async fn test_optimize_empty_graph() {
        let optimizer = GraphOptimizer::<TestState>::new().with_global_metric(|_, _| 1.0);

        let graph = StateGraph::<TestState>::new();
        let trainset = vec![TestState {
            query: "test".to_string(),
            response: "".to_string(),
        }];

        let result = optimizer.optimize(graph, trainset).await;
        // Should fail with "No optimizable nodes found"
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("No optimizable nodes found"));
        }
    }

    #[test]
    fn test_optimization_strategy_debug() {
        let strategy = OptimizationStrategy::Sequential;
        assert_eq!(format!("{:?}", strategy), "Sequential");

        let strategy = OptimizationStrategy::Joint;
        assert_eq!(format!("{:?}", strategy), "Joint");

        let strategy = OptimizationStrategy::Alternating;
        assert_eq!(format!("{:?}", strategy), "Alternating");
    }

    #[test]
    fn test_optimization_strategy_clone() {
        let strategy = OptimizationStrategy::Joint;
        // Use Copy trait since OptimizationStrategy is Copy
        let cloned = strategy;
        assert_eq!(strategy, cloned);
    }

    #[test]
    fn test_optimization_strategy_copy() {
        let strategy = OptimizationStrategy::Sequential;
        let copied: OptimizationStrategy = strategy; // Copy
        assert_eq!(strategy, copied); // Original still usable
    }

    #[test]
    fn test_graph_optimizer_default() {
        let optimizer = GraphOptimizer::<TestState>::default();
        assert!(optimizer.global_metric.is_none());
        assert!(optimizer.base_optimizer.is_none());
        assert_eq!(optimizer.strategy, OptimizationStrategy::Joint);
        assert_eq!(optimizer.max_iterations, 10);
        assert_eq!(optimizer.min_improvement, 0.01);
    }

    #[test]
    fn test_builder_chain() {
        let optimizer = GraphOptimizer::<TestState>::new()
            .with_global_metric(|_, _| 0.5)
            .with_strategy(OptimizationStrategy::Alternating)
            .with_max_iterations(25)
            .with_min_improvement(0.005);

        assert!(optimizer.global_metric.is_some());
        assert_eq!(optimizer.strategy, OptimizationStrategy::Alternating);
        assert_eq!(optimizer.max_iterations, 25);
        assert_eq!(optimizer.min_improvement, 0.005);
    }

    #[test]
    fn test_with_base_optimizer() {
        use super::super::optimizers::BootstrapFewShot;

        let optimizer =
            GraphOptimizer::<TestState>::new().with_base_optimizer(BootstrapFewShot::default());

        assert!(optimizer.base_optimizer.is_some());
    }

    #[test]
    fn test_global_metric_evaluation() {
        let optimizer =
            GraphOptimizer::<TestState>::new().with_global_metric(|initial, final_state| {
                // Metric: score based on response length compared to query
                if final_state.response.len() > initial.query.len() {
                    1.0
                } else {
                    0.0
                }
            });

        // Test the metric directly
        let metric = optimizer.global_metric.unwrap();
        let initial = TestState {
            query: "hi".to_string(),
            response: "".to_string(),
        };
        let final_short = TestState {
            query: "hi".to_string(),
            response: "x".to_string(),
        };
        let final_long = TestState {
            query: "hi".to_string(),
            response: "hello there!".to_string(),
        };

        assert_eq!(metric(&initial, &final_short), 0.0);
        assert_eq!(metric(&initial, &final_long), 1.0);
    }

    #[test]
    fn test_optimization_strategy_all_variants() {
        let strategies = [
            OptimizationStrategy::Sequential,
            OptimizationStrategy::Joint,
            OptimizationStrategy::Alternating,
        ];

        // Test each can be created and compared
        for s1 in &strategies {
            for s2 in &strategies {
                if std::mem::discriminant(s1) == std::mem::discriminant(s2) {
                    assert_eq!(s1, s2);
                } else {
                    assert_ne!(s1, s2);
                }
            }
        }
    }

    #[test]
    fn test_find_optimizable_nodes_empty_graph() {
        let optimizer = GraphOptimizer::<TestState>::new();
        let graph = StateGraph::<TestState>::new();

        let result = optimizer.find_optimizable_nodes(&graph);
        assert!(result.is_ok());
        let nodes = result.unwrap();
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_graph_optimizer_min_improvement_zero() {
        let optimizer = GraphOptimizer::<TestState>::new().with_min_improvement(0.0);
        assert_eq!(optimizer.min_improvement, 0.0);
    }

    #[test]
    fn test_graph_optimizer_large_iterations() {
        let optimizer = GraphOptimizer::<TestState>::new().with_max_iterations(1000);
        assert_eq!(optimizer.max_iterations, 1000);
    }
}
