//! RunnableBranch - Conditional routing of inputs to different runnables
//!
//! Routes input to different branches based on conditions.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use super::graph::{Edge, Graph, Node};
use super::Runnable;
use crate::core::config::RunnableConfig;
use crate::core::error::Result;

/// Type alias for a branch condition and its corresponding runnable
type BranchPair<Input, Output> = (
    Box<dyn Fn(&Input) -> bool + Send + Sync>,
    Arc<dyn Runnable<Input = Input, Output = Output>>,
);

/// A Runnable that routes input to different branches based on conditions.
///
/// `RunnableBranch` evaluates conditions in order and executes the first matching branch.
/// If no condition matches, the default branch is executed.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::runnable::{RunnableBranch, RunnableLambda};
///
/// let branch = RunnableBranch::new()
///     .add_branch(
///         |x: &i32| *x > 10,
///         RunnableLambda::new(|x: i32| format!("Large: {}", x)),
///     )
///     .add_branch(
///         |x: &i32| *x > 0,
///         RunnableLambda::new(|x: i32| format!("Small: {}", x)),
///     )
///     .default(RunnableLambda::new(|x: i32| format!("Zero or negative: {}", x)));
///
/// let result = branch.invoke(15, None).await?;
/// // result = "Large: 15"
/// ```
pub struct RunnableBranch<Input, Output>
where
    Input: Send + Sync,
    Output: Send + Sync,
{
    branches: Vec<BranchPair<Input, Output>>,
    default: Arc<dyn Runnable<Input = Input, Output = Output>>,
}

impl<Input, Output> RunnableBranch<Input, Output>
where
    Input: Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    /// Create a new `RunnableBranch` with a default branch
    pub fn new<R>(default: R) -> Self
    where
        R: Runnable<Input = Input, Output = Output> + 'static,
    {
        Self {
            branches: Vec::new(),
            default: Arc::new(default),
        }
    }

    /// Add a conditional branch
    #[must_use]
    pub fn add_branch<F, R>(mut self, condition: F, runnable: R) -> Self
    where
        F: Fn(&Input) -> bool + Send + Sync + 'static,
        R: Runnable<Input = Input, Output = Output> + 'static,
    {
        self.branches
            .push((Box::new(condition), Arc::new(runnable)));
        self
    }

    /// Set the default branch (replaces existing default)
    pub fn default<R>(mut self, runnable: R) -> Self
    where
        R: Runnable<Input = Input, Output = Output> + 'static,
    {
        self.default = Arc::new(runnable);
        self
    }
}

#[async_trait]
impl<Input, Output> Runnable for RunnableBranch<Input, Output>
where
    Input: Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    type Input = Input;
    type Output = Output;

    fn name(&self) -> String {
        format!("Branch[{} conditions]", self.branches.len())
    }

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // Setup callbacks
        let mut config = config.unwrap_or_default();
        let run_id = config.ensure_run_id();
        let callback_manager = config.get_callback_manager();

        // Create serialized info
        let mut serialized = HashMap::new();
        serialized.insert("name".to_string(), serde_json::json!(self.name()));

        // Start chain
        callback_manager
            .on_chain_start(
                &serialized,
                &HashMap::new(),
                run_id,
                None,
                &config.tags,
                &config.metadata,
            )
            .await?;

        // Execute branching logic
        let result = async {
            // Check each condition in order
            for (condition, runnable) in &self.branches {
                if condition(&input) {
                    return runnable.invoke(input, Some(config.clone())).await;
                }
            }

            // No condition matched, use default
            self.default.invoke(input, Some(config.clone())).await
        }
        .await;

        // End chain or report error
        match &result {
            Ok(_) => {
                callback_manager
                    .on_chain_end(&HashMap::new(), run_id, None)
                    .await?;
            }
            Err(e) => {
                callback_manager
                    .on_chain_error(&e.to_string(), run_id, None)
                    .await?;
            }
        }

        result
    }

    fn get_graph(&self, config: Option<&RunnableConfig>) -> Graph {
        let mut graph = Graph::new();

        // Create a root node for the branch
        let root_node = Node::new(self.name(), self.name());
        graph.add_node(root_node);

        // Add each conditional branch
        for (idx, (_condition, runnable)) in self.branches.iter().enumerate() {
            let branch_graph = runnable.get_graph(config);
            let branch_prefix = format!("branch_{idx}");

            // Add nodes from branch graph with prefix
            for node in branch_graph.nodes.values() {
                let new_id = format!("{}:{}", branch_prefix, node.id);
                let new_node = node.with_id(new_id);
                graph.add_node(new_node);
            }

            // Add edges from branch graph with updated IDs
            for edge in &branch_graph.edges {
                let new_source = format!("{}:{}", branch_prefix, edge.source);
                let new_target = format!("{}:{}", branch_prefix, edge.target);
                let mut new_edge = Edge::new(new_source, new_target);
                new_edge.conditional = true;
                graph.add_edge(new_edge);
            }

            // Connect root to the first node of this branch with conditional edge
            if let Some(first_node) = branch_graph.first_node() {
                let first_node_id = format!("{}:{}", branch_prefix, first_node.id);
                let mut edge = Edge::new(self.name(), first_node_id);
                edge.conditional = true;
                edge.data = Some(format!("condition_{idx}"));
                graph.add_edge(edge);
            }
        }

        // Add default branch
        let default_graph = self.default.get_graph(config);
        let default_prefix = "default";

        // Add nodes from default graph with prefix
        for node in default_graph.nodes.values() {
            let new_id = format!("{}:{}", default_prefix, node.id);
            let new_node = node.with_id(new_id);
            graph.add_node(new_node);
        }

        // Add edges from default graph with updated IDs
        for edge in &default_graph.edges {
            let new_source = format!("{}:{}", default_prefix, edge.source);
            let new_target = format!("{}:{}", default_prefix, edge.target);
            graph.add_edge(Edge::new(new_source, new_target));
        }

        // Connect root to the first node of default branch
        if let Some(first_node) = default_graph.first_node() {
            let first_node_id = format!("{}:{}", default_prefix, first_node.id);
            let mut edge = Edge::new(self.name(), first_node_id);
            edge.data = Some("default".to_string());
            graph.add_edge(edge);
        }

        graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runnable::RunnableLambda;

    // ============================================
    // RunnableBranch Construction Tests
    // ============================================

    #[test]
    fn test_branch_new() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch: RunnableBranch<i32, i32> = RunnableBranch::new(default_fn);
        assert_eq!(branch.branches.len(), 0);
    }

    #[test]
    fn test_branch_add_single_branch() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn).add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));
        assert_eq!(branch.branches.len(), 1);
    }

    #[test]
    fn test_branch_add_multiple_branches() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 100, RunnableLambda::new(|x: i32| x * 10))
            .add_branch(|x: &i32| *x > 50, RunnableLambda::new(|x: i32| x * 5))
            .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));
        assert_eq!(branch.branches.len(), 3);
    }

    #[test]
    fn test_branch_replace_default() {
        let default1 = RunnableLambda::new(|x: i32| x);
        let default2 = RunnableLambda::new(|x: i32| x + 1);
        let branch = RunnableBranch::new(default1).default(default2);
        assert_eq!(branch.branches.len(), 0);
    }

    #[test]
    fn test_branch_builder_chain() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 50, RunnableLambda::new(|x: i32| x * 5))
            .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2))
            .default(RunnableLambda::new(|x: i32| -x));
        assert_eq!(branch.branches.len(), 2);
    }

    // ============================================
    // RunnableBranch Name Tests
    // ============================================

    #[test]
    fn test_branch_name_zero_conditions() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch: RunnableBranch<i32, i32> = RunnableBranch::new(default_fn);
        assert_eq!(branch.name(), "Branch[0 conditions]");
    }

    #[test]
    fn test_branch_name_one_condition() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn).add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));
        assert_eq!(branch.name(), "Branch[1 conditions]");
    }

    #[test]
    fn test_branch_name_many_conditions() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|_: &i32| false, RunnableLambda::new(|x: i32| x + 1))
            .add_branch(|_: &i32| false, RunnableLambda::new(|x: i32| x + 2))
            .add_branch(|_: &i32| false, RunnableLambda::new(|x: i32| x + 3))
            .add_branch(|_: &i32| false, RunnableLambda::new(|x: i32| x + 4))
            .add_branch(|_: &i32| false, RunnableLambda::new(|x: i32| x + 5));
        assert_eq!(branch.name(), "Branch[5 conditions]");
    }

    // ============================================
    // RunnableBranch Invoke Tests
    // ============================================

    #[tokio::test]
    async fn test_branch_invoke_first_matching_condition() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 100, RunnableLambda::new(|x: i32| x * 10))
            .add_branch(|x: &i32| *x > 50, RunnableLambda::new(|x: i32| x * 5))
            .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));

        // 150 matches first condition (> 100)
        let result = branch.invoke(150, None).await.unwrap();
        assert_eq!(result, 1500);
    }

    #[tokio::test]
    async fn test_branch_invoke_second_matching_condition() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 100, RunnableLambda::new(|x: i32| x * 10))
            .add_branch(|x: &i32| *x > 50, RunnableLambda::new(|x: i32| x * 5))
            .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));

        // 75 matches second condition (> 50)
        let result = branch.invoke(75, None).await.unwrap();
        assert_eq!(result, 375);
    }

    #[tokio::test]
    async fn test_branch_invoke_third_matching_condition() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 100, RunnableLambda::new(|x: i32| x * 10))
            .add_branch(|x: &i32| *x > 50, RunnableLambda::new(|x: i32| x * 5))
            .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));

        // 25 matches third condition (> 10)
        let result = branch.invoke(25, None).await.unwrap();
        assert_eq!(result, 50);
    }

    #[tokio::test]
    async fn test_branch_invoke_default_when_no_match() {
        let default_fn = RunnableLambda::new(|x: i32| -x);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 100, RunnableLambda::new(|x: i32| x * 10))
            .add_branch(|x: &i32| *x > 50, RunnableLambda::new(|x: i32| x * 5))
            .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));

        // 5 matches no condition, goes to default
        let result = branch.invoke(5, None).await.unwrap();
        assert_eq!(result, -5);
    }

    #[tokio::test]
    async fn test_branch_invoke_default_only() {
        let default_fn = RunnableLambda::new(|x: i32| x * 100);
        let branch: RunnableBranch<i32, i32> = RunnableBranch::new(default_fn);

        let result = branch.invoke(7, None).await.unwrap();
        assert_eq!(result, 700);
    }

    #[tokio::test]
    async fn test_branch_invoke_with_zero_input() {
        let default_fn = RunnableLambda::new(|x: i32| x * 2);
        let branch = RunnableBranch::new(default_fn).add_branch(|x: &i32| *x > 0, RunnableLambda::new(|x: i32| x + 1));

        let result = branch.invoke(0, None).await.unwrap();
        assert_eq!(result, 0); // default: 0 * 2 = 0
    }

    #[tokio::test]
    async fn test_branch_invoke_with_negative_input() {
        let default_fn = RunnableLambda::new(|x: i32| x.abs());
        let branch = RunnableBranch::new(default_fn).add_branch(|x: &i32| *x >= 0, RunnableLambda::new(|x: i32| x));

        let result = branch.invoke(-42, None).await.unwrap();
        assert_eq!(result, 42); // default abs(-42) = 42
    }

    #[tokio::test]
    async fn test_branch_invoke_boundary_condition_inclusive() {
        let default_fn = RunnableLambda::new(|_x: i32| 0);
        let branch = RunnableBranch::new(default_fn).add_branch(|x: &i32| *x >= 10, RunnableLambda::new(|x: i32| x));

        let result = branch.invoke(10, None).await.unwrap();
        assert_eq!(result, 10); // 10 >= 10 is true
    }

    #[tokio::test]
    async fn test_branch_invoke_boundary_condition_exclusive() {
        let default_fn = RunnableLambda::new(|_x: i32| 0);
        let branch = RunnableBranch::new(default_fn).add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x));

        let result = branch.invoke(10, None).await.unwrap();
        assert_eq!(result, 0); // 10 > 10 is false, goes to default
    }

    #[tokio::test]
    async fn test_branch_invoke_with_string_input() {
        let default_fn = RunnableLambda::new(|s: String| format!("default: {}", s));
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|s: &String| s.starts_with("hello"), RunnableLambda::new(|s: String| format!("greeting: {}", s)))
            .add_branch(|s: &String| s.len() > 10, RunnableLambda::new(|s: String| format!("long: {}", s)));

        let result = branch.invoke("hello world".to_string(), None).await.unwrap();
        assert_eq!(result, "greeting: hello world");
    }

    #[tokio::test]
    async fn test_branch_invoke_string_second_condition() {
        let default_fn = RunnableLambda::new(|s: String| format!("default: {}", s));
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|s: &String| s.starts_with("hello"), RunnableLambda::new(|s: String| format!("greeting: {}", s)))
            .add_branch(|s: &String| s.len() > 10, RunnableLambda::new(|s: String| format!("long: {}", s)));

        let result = branch.invoke("this is a long string".to_string(), None).await.unwrap();
        assert_eq!(result, "long: this is a long string");
    }

    #[tokio::test]
    async fn test_branch_invoke_string_default() {
        let default_fn = RunnableLambda::new(|s: String| format!("default: {}", s));
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|s: &String| s.starts_with("hello"), RunnableLambda::new(|s: String| format!("greeting: {}", s)))
            .add_branch(|s: &String| s.len() > 20, RunnableLambda::new(|s: String| format!("long: {}", s)));

        let result = branch.invoke("short".to_string(), None).await.unwrap();
        assert_eq!(result, "default: short");
    }

    #[tokio::test]
    async fn test_branch_invoke_empty_string() {
        let default_fn = RunnableLambda::new(|s: String| format!("empty? {}", s.is_empty()));
        let branch = RunnableBranch::new(default_fn).add_branch(|s: &String| !s.is_empty(), RunnableLambda::new(|s: String| format!("has content: {}", s)));

        let result = branch.invoke(String::new(), None).await.unwrap();
        assert_eq!(result, "empty? true");
    }

    // ============================================
    // RunnableBranch with Config Tests
    // ============================================

    #[tokio::test]
    async fn test_branch_invoke_with_config() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn).add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));

        let config = RunnableConfig::default();
        let result = branch.invoke(15, Some(config)).await.unwrap();
        assert_eq!(result, 30);
    }

    #[tokio::test]
    async fn test_branch_invoke_with_tags() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn);

        let mut config = RunnableConfig::default();
        config.tags.push("test-tag".to_string());
        let result = branch.invoke(42, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_branch_invoke_with_metadata() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn);

        let mut config = RunnableConfig::default();
        config.metadata.insert("key".to_string(), serde_json::json!("value"));
        let result = branch.invoke(42, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    // ============================================
    // RunnableBranch get_graph Tests
    // ============================================

    #[test]
    fn test_branch_get_graph_default_only() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch: RunnableBranch<i32, i32> = RunnableBranch::new(default_fn);

        let graph = branch.get_graph(None);
        // Should have root node and default branch node
        assert!(graph.nodes.len() >= 1);
        assert!(graph.nodes.contains_key(&branch.name()));
    }

    #[test]
    fn test_branch_get_graph_with_branches() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 50, RunnableLambda::new(|x: i32| x * 5))
            .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));

        let graph = branch.get_graph(None);
        // Should have root + branch nodes + default node
        assert!(graph.nodes.len() >= 3);
    }

    #[test]
    fn test_branch_get_graph_conditional_edges() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn).add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2));

        let graph = branch.get_graph(None);
        // Should have conditional edges
        let conditional_edges: Vec<_> = graph.edges.iter().filter(|e| e.conditional).collect();
        assert!(!conditional_edges.is_empty());
    }

    #[test]
    fn test_branch_get_graph_with_config() {
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch: RunnableBranch<i32, i32> = RunnableBranch::new(default_fn);

        let config = RunnableConfig::default();
        let graph = branch.get_graph(Some(&config));
        assert!(graph.nodes.len() >= 1);
    }

    // ============================================
    // RunnableBranch Edge Cases
    // ============================================

    #[tokio::test]
    async fn test_branch_all_conditions_false() {
        let default_fn = RunnableLambda::new(|_: i32| -999);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|_: &i32| false, RunnableLambda::new(|x: i32| x + 1))
            .add_branch(|_: &i32| false, RunnableLambda::new(|x: i32| x + 2))
            .add_branch(|_: &i32| false, RunnableLambda::new(|x: i32| x + 3));

        let result = branch.invoke(100, None).await.unwrap();
        assert_eq!(result, -999); // Always default
    }

    #[tokio::test]
    async fn test_branch_first_condition_always_true() {
        let default_fn = RunnableLambda::new(|_: i32| -999);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|_: &i32| true, RunnableLambda::new(|x: i32| x + 1))
            .add_branch(|_: &i32| true, RunnableLambda::new(|x: i32| x + 2));

        let result = branch.invoke(10, None).await.unwrap();
        assert_eq!(result, 11); // First branch always wins
    }

    #[tokio::test]
    async fn test_branch_order_matters() {
        let default_fn = RunnableLambda::new(|_: i32| 0);
        // Both conditions match for x=20, but first one wins
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|_: i32| 1))
            .add_branch(|x: &i32| *x > 5, RunnableLambda::new(|_: i32| 2));

        let result = branch.invoke(20, None).await.unwrap();
        assert_eq!(result, 1); // First matching condition

        // For x=8, only second condition matches
        let result2 = branch.invoke(8, None).await.unwrap();
        assert_eq!(result2, 2);
    }

    #[tokio::test]
    async fn test_branch_with_vec_input() {
        let default_fn = RunnableLambda::new(|v: Vec<i32>| v.len());
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|v: &Vec<i32>| v.is_empty(), RunnableLambda::new(|_: Vec<i32>| 0))
            .add_branch(|v: &Vec<i32>| v.len() > 5, RunnableLambda::new(|v: Vec<i32>| v.len() * 2));

        let result = branch.invoke(vec![1, 2, 3, 4, 5, 6, 7], None).await.unwrap();
        assert_eq!(result, 14); // 7 * 2

        let result2 = branch.invoke(vec![], None).await.unwrap();
        assert_eq!(result2, 0); // Empty

        let result3 = branch.invoke(vec![1, 2, 3], None).await.unwrap();
        assert_eq!(result3, 3); // Default: len()
    }

    #[tokio::test]
    async fn test_branch_with_option_input() {
        let default_fn = RunnableLambda::new(|_: Option<i32>| -1);
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|o: &Option<i32>| o.is_some(), RunnableLambda::new(|o: Option<i32>| o.unwrap_or(0)));

        let result = branch.invoke(Some(42), None).await.unwrap();
        assert_eq!(result, 42);

        let result2 = branch.invoke(None, None).await.unwrap();
        assert_eq!(result2, -1);
    }

    #[tokio::test]
    async fn test_branch_type_transformation() {
        // Input: i32, Output: String
        let default_fn = RunnableLambda::new(|x: i32| format!("default: {}", x));
        let branch = RunnableBranch::new(default_fn)
            .add_branch(|x: &i32| *x > 0, RunnableLambda::new(|x: i32| format!("positive: {}", x)))
            .add_branch(|x: &i32| *x < 0, RunnableLambda::new(|x: i32| format!("negative: {}", x)));

        let result = branch.invoke(5, None).await.unwrap();
        assert_eq!(result, "positive: 5");

        let result2 = branch.invoke(-3, None).await.unwrap();
        assert_eq!(result2, "negative: -3");

        let result3 = branch.invoke(0, None).await.unwrap();
        assert_eq!(result3, "default: 0");
    }

    // ============================================
    // RunnableBranch Complex Condition Tests
    // ============================================

    #[tokio::test]
    async fn test_branch_complex_conditions() {
        let default_fn = RunnableLambda::new(|s: String| format!("default: {}", s));
        let branch = RunnableBranch::new(default_fn)
            .add_branch(
                |s: &String| s.contains("error") && s.len() > 10,
                RunnableLambda::new(|s: String| format!("long error: {}", s)),
            )
            .add_branch(
                |s: &String| s.contains("error"),
                RunnableLambda::new(|s: String| format!("error: {}", s)),
            )
            .add_branch(
                |s: &String| s.contains("warn"),
                RunnableLambda::new(|s: String| format!("warning: {}", s)),
            );

        let result = branch.invoke("error occurred in module".to_string(), None).await.unwrap();
        assert_eq!(result, "long error: error occurred in module");

        let result2 = branch.invoke("error".to_string(), None).await.unwrap();
        assert_eq!(result2, "error: error");

        let result3 = branch.invoke("warn message".to_string(), None).await.unwrap();
        assert_eq!(result3, "warning: warn message");

        let result4 = branch.invoke("info".to_string(), None).await.unwrap();
        assert_eq!(result4, "default: info");
    }

    #[tokio::test]
    async fn test_branch_with_closure_capturing_values() {
        let threshold = 50;
        let default_fn = RunnableLambda::new(|x: i32| x);
        let branch = RunnableBranch::new(default_fn).add_branch(
            move |x: &i32| *x > threshold,
            RunnableLambda::new(|x: i32| x * 2),
        );

        let result = branch.invoke(60, None).await.unwrap();
        assert_eq!(result, 120);

        let result2 = branch.invoke(40, None).await.unwrap();
        assert_eq!(result2, 40);
    }
}
