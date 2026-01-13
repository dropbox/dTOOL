// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Subgraph support - nested and modular graph composition
//!
//! Subgraphs enable building complex workflows from reusable components.
//! Each subgraph can have its own state type, with mapping functions
//! to convert between parent and child states.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, subgraph::SubgraphNode};
//!
//! // Create a reusable research subgraph
//! let mut research_graph = StateGraph::<ResearchState>::new();
//! research_graph.add_node("search", search_node);
//! research_graph.add_node("analyze", analyze_node);
//! research_graph.add_edge("search", "analyze");
//! research_graph.set_entry_point("search");
//!
//! // Create parent graph
//! let mut main_graph = StateGraph::<ProjectState>::new();
//!
//! // Add subgraph with state mapping
//! main_graph.add_subgraph_with_mapping(
//!     "research",
//!     research_graph,
//!     |parent: &ProjectState| ResearchState {
//!         query: parent.task.clone(),
//!         findings: Vec::new(),
//!     },
//!     |parent: ProjectState, child: ResearchState| ProjectState {
//!         research_results: child.into(),
//!         ..parent
//!     },
//! );
//! ```

use async_trait::async_trait;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::error::Result;
use crate::executor::CompiledGraph;
use crate::node::Node;
use crate::state::GraphState;

/// A node that wraps a subgraph with different state type
///
/// `SubgraphNode` allows embedding a graph with state type `C` (child)
/// inside a graph with state type `P` (parent). State mapping functions
/// convert between the two state types.
///
/// # Type Parameters
///
/// * `P` - Parent graph state type
/// * `C` - Child (subgraph) state type
pub struct SubgraphNode<P, C>
where
    P: GraphState,
    C: crate::state::MergeableState,
{
    /// Name of the subgraph (for debugging)
    name: String,
    /// The compiled child graph
    subgraph: Arc<CompiledGraph<C>>,
    /// Maps parent state → child state
    map_to_child: Arc<dyn Fn(&P) -> C + Send + Sync>,
    /// Maps (parent state, child result) → parent state
    map_from_child: Arc<dyn Fn(P, C) -> P + Send + Sync>,
    _phantom: PhantomData<(P, C)>,
}

impl<P, C> SubgraphNode<P, C>
where
    P: GraphState,
    C: crate::state::MergeableState,
{
    /// Create a new subgraph node
    ///
    /// # Arguments
    ///
    /// * `name` - Subgraph name (for debugging)
    /// * `subgraph` - Compiled child graph
    /// * `map_to_child` - Function to map parent state → child state
    /// * `map_from_child` - Function to merge child result back into parent state
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let subgraph_node = SubgraphNode::new(
    ///     "research",
    ///     compiled_research_graph,
    ///     |parent| ResearchState { query: parent.task.clone(), findings: vec![] },
    ///     |parent, child| ProjectState { research_results: child.into(), ..parent }
    /// );
    /// ```
    pub fn new<F1, F2>(
        name: impl Into<String>,
        subgraph: CompiledGraph<C>,
        map_to_child: F1,
        map_from_child: F2,
    ) -> Self
    where
        F1: Fn(&P) -> C + Send + Sync + 'static,
        F2: Fn(P, C) -> P + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            subgraph: Arc::new(subgraph),
            map_to_child: Arc::new(map_to_child),
            map_from_child: Arc::new(map_from_child),
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<P, C> Node<P> for SubgraphNode<P, C>
where
    P: GraphState,
    C: crate::state::MergeableState,
{
    async fn execute(&self, state: P) -> Result<P> {
        // Map parent state to child state
        let child_state = (self.map_to_child)(&state);

        // Execute the subgraph
        let result = self.subgraph.invoke(child_state).await?;

        // Map child result back to parent state
        let updated_parent = (self.map_from_child)(state, result.final_state);

        Ok(updated_parent)
    }

    fn name(&self) -> String {
        format!("Subgraph({})", self.name)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StateGraph, END};
    use serde::{Deserialize, Serialize};

    // Parent state for testing
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct ParentState {
        task: String,
        counter: i32,
        result: Option<String>,
    }

    impl crate::state::MergeableState for ParentState {
        fn merge(&mut self, other: &Self) {
            // Take max counter from parallel branches
            self.counter = self.counter.max(other.counter);
            // Keep result from self (last-write-wins)
        }
    }

    // Child state for testing
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct ChildState {
        input: String,
        count: i32,
    }

    impl crate::state::MergeableState for ChildState {
        fn merge(&mut self, other: &Self) {
            // Take max count from parallel branches
            self.count = self.count.max(other.count);
            // Concatenate input strings
            if !other.input.is_empty() && self.input != other.input {
                self.input.push_str(", ");
                self.input.push_str(&other.input);
            }
        }
    }

    #[tokio::test]
    async fn test_subgraph_basic_execution() -> Result<()> {
        // Create child graph
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("increment", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 1;
                Ok(state)
            })
        });
        child_graph.add_edge("increment", END);
        child_graph.set_entry_point("increment");

        let compiled_child = child_graph.compile()?;

        // Create subgraph node
        let subgraph_node = SubgraphNode::new(
            "child",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                result: Some(format!("Processed: {}", child.input)),
                counter: child.count,
                ..parent
            },
        );

        // Execute
        let initial = ParentState {
            task: "test".to_string(),
            counter: 0,
            result: None,
        };

        let result = subgraph_node.execute(initial).await?;

        // Verify mapping and execution
        assert_eq!(result.counter, 1); // Incremented by child
        assert_eq!(result.result, Some("Processed: test".to_string()));
        assert_eq!(result.task, "test"); // Preserved from parent

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_state_isolation() -> Result<()> {
        // Child graph modifies its state but shouldn't affect parent fields
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("process", |state| {
            Box::pin(async move {
                let mut state = state;
                state.input = "modified_by_child".to_string();
                state.count = 999;
                Ok(state)
            })
        });
        child_graph.add_edge("process", END);
        child_graph.set_entry_point("process");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "isolated",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            // Only update specific parent field
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                ..parent
            },
        );

        let initial = ParentState {
            task: "original_task".to_string(),
            counter: 5,
            result: Some("original_result".to_string()),
        };

        let result = subgraph_node.execute(initial).await?;

        // Only counter should change (from child), other fields preserved
        assert_eq!(result.task, "original_task");
        assert_eq!(result.result, Some("original_result".to_string()));
        assert_eq!(result.counter, 999); // Updated from child

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_with_add_subgraph_with_mapping() -> Result<()> {
        // Test the StateGraph::add_subgraph_with_mapping convenience method

        // Create child graph
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("double", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count *= 2;
                Ok(state)
            })
        });
        child_graph.add_edge("double", END);
        child_graph.set_entry_point("double");

        // Create parent graph and add subgraph
        let mut parent_graph = StateGraph::<ParentState>::new();
        parent_graph
            .add_subgraph_with_mapping(
                "child",
                child_graph,
                |parent: &ParentState| ChildState {
                    input: parent.task.clone(),
                    count: parent.counter,
                },
                |parent: ParentState, child: ChildState| ParentState {
                    counter: child.count,
                    result: Some(format!("Doubled: {}", child.count)),
                    ..parent
                },
            )?
            .add_edge("child", END)
            .set_entry_point("child");

        let compiled = parent_graph.compile()?;

        let initial = ParentState {
            task: "test".to_string(),
            counter: 5,
            result: None,
        };

        let result = compiled.invoke(initial).await?;

        assert_eq!(result.final_state.counter, 10); // 5 * 2
        assert_eq!(result.final_state.result, Some("Doubled: 10".to_string()));
        assert_eq!(result.nodes_executed.len(), 1); // Just the subgraph node

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_in_workflow() -> Result<()> {
        // Test subgraph as part of a larger workflow

        // Child graph (processing)
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("step1", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 10;
                Ok(state)
            })
        });
        child_graph.add_node_from_fn("step2", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count *= 2;
                Ok(state)
            })
        });
        child_graph.add_edge("step1", "step2");
        child_graph.add_edge("step2", END);
        child_graph.set_entry_point("step1");

        // Parent graph with before/after nodes
        let mut parent_graph = StateGraph::<ParentState>::new();

        // Pre-processing node
        parent_graph.add_node_from_fn("prepare", |state| {
            Box::pin(async move {
                let mut state = state;
                state.counter += 1; // Increment before subgraph
                Ok(state)
            })
        });

        // Add subgraph
        parent_graph
            .add_subgraph_with_mapping(
                "process",
                child_graph,
                |parent: &ParentState| ChildState {
                    input: parent.task.clone(),
                    count: parent.counter,
                },
                |parent: ParentState, child: ChildState| ParentState {
                    counter: child.count,
                    ..parent
                },
            )?
            .add_node_from_fn("finalize", |state| {
                Box::pin(async move {
                    let mut state = state;
                    state.result = Some(format!("Final: {}", state.counter));
                    Ok(state)
                })
            })
            .add_edge("prepare", "process")
            .add_edge("process", "finalize")
            .add_edge("finalize", END)
            .set_entry_point("prepare");

        let compiled = parent_graph.compile()?;

        let initial = ParentState {
            task: "workflow".to_string(),
            counter: 5,
            result: None,
        };

        let result = compiled.invoke(initial).await?;

        // Expected: (5 + 1) + 10 = 16, then 16 * 2 = 32
        assert_eq!(result.final_state.counter, 32);
        assert_eq!(result.final_state.result, Some("Final: 32".to_string()));
        assert_eq!(result.nodes_executed.len(), 3); // prepare, process (subgraph), finalize

        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_subgraphs() -> Result<()> {
        // Test multiple subgraphs in one parent graph

        // Child graph 1 (increment)
        let mut increment_graph = StateGraph::<ChildState>::new();
        increment_graph.add_node_from_fn("inc", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 100;
                Ok(state)
            })
        });
        increment_graph.add_edge("inc", END);
        increment_graph.set_entry_point("inc");

        // Child graph 2 (multiply)
        let mut multiply_graph = StateGraph::<ChildState>::new();
        multiply_graph.add_node_from_fn("mul", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count *= 3;
                Ok(state)
            })
        });
        multiply_graph.add_edge("mul", END);
        multiply_graph.set_entry_point("mul");

        // Parent graph with both subgraphs
        let mut parent_graph = StateGraph::<ParentState>::new();

        parent_graph
            .add_subgraph_with_mapping(
                "increment_sub",
                increment_graph,
                |parent: &ParentState| ChildState {
                    input: parent.task.clone(),
                    count: parent.counter,
                },
                |parent: ParentState, child: ChildState| ParentState {
                    counter: child.count,
                    ..parent
                },
            )?
            .add_subgraph_with_mapping(
                "multiply_sub",
                multiply_graph,
                |parent: &ParentState| ChildState {
                    input: parent.task.clone(),
                    count: parent.counter,
                },
                |parent: ParentState, child: ChildState| ParentState {
                    counter: child.count,
                    result: Some(format!("Result: {}", child.count)),
                    ..parent
                },
            )?
            .add_edge("increment_sub", "multiply_sub")
            .add_edge("multiply_sub", END)
            .set_entry_point("increment_sub");

        let compiled = parent_graph.compile()?;

        let initial = ParentState {
            task: "multi".to_string(),
            counter: 10,
            result: None,
        };

        let result = compiled.invoke(initial).await?;

        // Expected: (10 + 100) * 3 = 330
        assert_eq!(result.final_state.counter, 330);
        assert_eq!(result.final_state.result, Some("Result: 330".to_string()));
        assert_eq!(result.nodes_executed.len(), 2); // Two subgraph nodes

        Ok(())
    }

    #[test]
    fn test_subgraph_node_name() {
        // Test that SubgraphNode::name() returns the correct format
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
        child_graph.add_edge("test", END);
        child_graph.set_entry_point("test");

        let compiled_child = child_graph.compile().unwrap();

        let subgraph_node = SubgraphNode::new(
            "test_subgraph",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                ..parent
            },
        );

        assert_eq!(subgraph_node.name(), "Subgraph(test_subgraph)");
    }

    #[tokio::test]
    async fn test_subgraph_error_propagation() -> Result<()> {
        // Test that errors from subgraph are propagated correctly
        use crate::error::Error;

        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("error_node", |_state| {
            Box::pin(async move {
                Err(Error::NodeExecution {
                    node: "error_node".to_string(),
                    source: "test error".into(),
                })
            })
        });
        child_graph.add_edge("error_node", END);
        child_graph.set_entry_point("error_node");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "error_subgraph",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                ..parent
            },
        );

        let initial = ParentState {
            task: "test".to_string(),
            counter: 0,
            result: None,
        };

        let result = subgraph_node.execute(initial).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_nested_subgraphs() -> Result<()> {
        // Test subgraph within subgraph (3-level nesting)

        // Level 3 (innermost) - doubles count
        let mut inner_graph = StateGraph::<ChildState>::new();
        inner_graph.add_node_from_fn("inner_double", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count *= 2;
                Ok(state)
            })
        });
        inner_graph.add_edge("inner_double", END);
        inner_graph.set_entry_point("inner_double");

        // Level 2 (middle) - adds 10, then calls inner subgraph
        let mut middle_graph = StateGraph::<ChildState>::new();
        middle_graph.add_node_from_fn("add_ten", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 10;
                Ok(state)
            })
        });
        middle_graph
            .add_subgraph_with_mapping(
                "inner",
                inner_graph,
                |parent: &ChildState| ChildState {
                    input: parent.input.clone(),
                    count: parent.count,
                },
                |parent: ChildState, child: ChildState| ChildState {
                    count: child.count,
                    ..parent
                },
            )?
            .add_edge("add_ten", "inner")
            .add_edge("inner", END)
            .set_entry_point("add_ten");

        // Level 1 (outer parent) - calls middle subgraph
        let mut parent_graph = StateGraph::<ParentState>::new();
        parent_graph
            .add_subgraph_with_mapping(
                "middle",
                middle_graph,
                |parent: &ParentState| ChildState {
                    input: parent.task.clone(),
                    count: parent.counter,
                },
                |parent: ParentState, child: ChildState| ParentState {
                    counter: child.count,
                    result: Some(format!("Nested result: {}", child.count)),
                    ..parent
                },
            )?
            .add_edge("middle", END)
            .set_entry_point("middle");

        let compiled = parent_graph.compile()?;

        let initial = ParentState {
            task: "nested".to_string(),
            counter: 5,
            result: None,
        };

        let result = compiled.invoke(initial).await?;

        // Expected: (5 + 10) * 2 = 30
        assert_eq!(result.final_state.counter, 30);
        assert_eq!(
            result.final_state.result,
            Some("Nested result: 30".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_parallel_subgraphs() -> Result<()> {
        // Test parallel execution of multiple subgraphs

        // Subgraph A - adds 100
        let mut graph_a = StateGraph::<ChildState>::new();
        graph_a.add_node_from_fn("add_100", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 100;
                Ok(state)
            })
        });
        graph_a.add_edge("add_100", END);
        graph_a.set_entry_point("add_100");

        // Subgraph B - multiplies by 2
        let mut graph_b = StateGraph::<ChildState>::new();
        graph_b.add_node_from_fn("mul_2", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count *= 2;
                Ok(state)
            })
        });
        graph_b.add_edge("mul_2", END);
        graph_b.set_entry_point("mul_2");

        // Parent with parallel subgraphs and a merge node
        let mut parent_graph = StateGraph::<ParentState>::new();

        parent_graph
            .add_subgraph_with_mapping(
                "branch_a",
                graph_a,
                |parent: &ParentState| ChildState {
                    input: "branch_a".to_string(),
                    count: parent.counter,
                },
                |parent: ParentState, child: ChildState| ParentState {
                    counter: child.count,
                    ..parent
                },
            )?
            .add_subgraph_with_mapping(
                "branch_b",
                graph_b,
                |parent: &ParentState| ChildState {
                    input: "branch_b".to_string(),
                    count: parent.counter,
                },
                |parent: ParentState, child: ChildState| ParentState {
                    counter: child.count,
                    result: Some(format!("Branch B: {}", child.count)),
                    ..parent
                },
            )?
            .add_node_from_fn("merge", |state| {
                Box::pin(async move {
                    let mut state = state;
                    state.result = Some(format!("Merged: {}", state.counter));
                    Ok(state)
                })
            })
            .add_parallel_edges("branch_a", vec!["branch_b".to_string()])
            .add_edge("branch_b", "merge")
            .add_edge("merge", END)
            .set_entry_point("branch_a");

        let compiled = parent_graph.compile_with_merge()?;

        let initial = ParentState {
            task: "parallel".to_string(),
            counter: 10,
            result: None,
        };

        let result = compiled.invoke(initial).await?;

        // Branch B should execute after A: (10 + 100) * 2 = 220
        assert_eq!(result.final_state.counter, 220);
        assert_eq!(result.final_state.result, Some("Merged: 220".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_with_empty_state() -> Result<()> {
        // Test subgraph with minimal/empty state values
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("process", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count = 42;
                Ok(state)
            })
        });
        child_graph.add_edge("process", END);
        child_graph.set_entry_point("process");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "empty_state",
            compiled_child,
            |_parent: &ParentState| ChildState {
                input: String::new(), // Empty string
                count: 0,             // Zero value
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                result: Some(child.input), // Empty string from child
                ..parent
            },
        );

        let initial = ParentState {
            task: "".to_string(),
            counter: 0,
            result: None,
        };

        let result = subgraph_node.execute(initial).await?;

        assert_eq!(result.counter, 42);
        assert_eq!(result.result, Some(String::new()));

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_with_large_state() -> Result<()> {
        // Test subgraph with large state data
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("process_large", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count = state.input.len() as i32;
                Ok(state)
            })
        });
        child_graph.add_edge("process_large", END);
        child_graph.set_entry_point("process_large");

        let compiled_child = child_graph.compile()?;

        let large_string = "x".repeat(10000); // 10KB string

        let subgraph_node = SubgraphNode::new(
            "large_state",
            compiled_child,
            move |parent: &ParentState| ChildState {
                input: large_string.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                result: Some(format!("Processed {} bytes", child.count)),
                ..parent
            },
        );

        let initial = ParentState {
            task: "large".to_string(),
            counter: 0,
            result: None,
        };

        let result = subgraph_node.execute(initial).await?;

        assert_eq!(result.counter, 10000);
        assert_eq!(result.result, Some("Processed 10000 bytes".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_multiple_invocations() -> Result<()> {
        // Test invoking same subgraph multiple times with different states
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("increment", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 1;
                Ok(state)
            })
        });
        child_graph.add_edge("increment", END);
        child_graph.set_entry_point("increment");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "reusable",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                ..parent
            },
        );

        // First invocation
        let state1 = ParentState {
            task: "first".to_string(),
            counter: 10,
            result: None,
        };
        let result1 = subgraph_node.execute(state1).await?;
        assert_eq!(result1.counter, 11);

        // Second invocation with different state
        let state2 = ParentState {
            task: "second".to_string(),
            counter: 100,
            result: None,
        };
        let result2 = subgraph_node.execute(state2).await?;
        assert_eq!(result2.counter, 101);

        // Third invocation
        let state3 = ParentState {
            task: "third".to_string(),
            counter: 999,
            result: None,
        };
        let result3 = subgraph_node.execute(state3).await?;
        assert_eq!(result3.counter, 1000);

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_with_conditional_edges() -> Result<()> {
        // Test subgraph containing conditional edges
        use std::collections::HashMap;

        let mut child_graph = StateGraph::<ChildState>::new();

        child_graph.add_node_from_fn("check", |state| Box::pin(async move { Ok(state) }));

        child_graph.add_node_from_fn("positive_path", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count *= 2;
                Ok(state)
            })
        });

        child_graph.add_node_from_fn("negative_path", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count = 0;
                Ok(state)
            })
        });

        // Conditional: if count > 0, go positive, else negative
        let mut routes = HashMap::new();
        routes.insert("positive".to_string(), "positive_path".to_string());
        routes.insert("negative".to_string(), "negative_path".to_string());

        child_graph.add_conditional_edges(
            "check",
            |state: &ChildState| -> String {
                if state.count > 0 {
                    "positive".to_string()
                } else {
                    "negative".to_string()
                }
            },
            routes,
        );

        child_graph.add_edge("positive_path", END);
        child_graph.add_edge("negative_path", END);
        child_graph.set_entry_point("check");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "conditional_sub",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                result: Some(format!("Conditional result: {}", child.count)),
                ..parent
            },
        );

        // Test positive path
        let positive_state = ParentState {
            task: "positive".to_string(),
            counter: 5,
            result: None,
        };
        let positive_result = subgraph_node.execute(positive_state).await?;
        assert_eq!(positive_result.counter, 10); // Doubled

        // Test negative path
        let negative_state = ParentState {
            task: "negative".to_string(),
            counter: -5,
            result: None,
        };
        let negative_result = subgraph_node.execute(negative_state).await?;
        assert_eq!(negative_result.counter, 0); // Zeroed

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_with_loop() -> Result<()> {
        // Test subgraph with a loop (self-edge with exit condition)
        use std::collections::HashMap;

        let mut child_graph = StateGraph::<ChildState>::new();

        child_graph.add_node_from_fn("loop_node", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 1;
                Ok(state)
            })
        });

        // Conditional: if count < 5, loop back, else exit
        let mut routes = HashMap::new();
        routes.insert("continue".to_string(), "loop_node".to_string());
        routes.insert("exit".to_string(), END.to_string());

        child_graph.add_conditional_edges(
            "loop_node",
            |state: &ChildState| -> String {
                if state.count < 5 {
                    "continue".to_string()
                } else {
                    "exit".to_string()
                }
            },
            routes,
        );

        child_graph.set_entry_point("loop_node");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "loop_sub",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                result: Some(format!("Loop completed: {}", child.count)),
                ..parent
            },
        );

        let initial = ParentState {
            task: "loop".to_string(),
            counter: 0,
            result: None,
        };

        let result = subgraph_node.execute(initial).await?;

        // Should loop until count reaches 5
        assert_eq!(result.counter, 5);
        assert_eq!(result.result, Some("Loop completed: 5".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_state_transformation() -> Result<()> {
        // Test complex state transformation patterns
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("transform", |state| {
            Box::pin(async move {
                let mut state = state;
                // Reverse input string, update count
                state.input = state.input.chars().rev().collect();
                state.count = state.input.len() as i32;
                Ok(state)
            })
        });
        child_graph.add_edge("transform", END);
        child_graph.set_entry_point("transform");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "transformer",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |_parent: ParentState, child: ChildState| ParentState {
                task: child.input.clone(), // Put reversed string back
                counter: child.count,
                result: Some(format!("Transformed: {}", child.input)),
            },
        );

        let initial = ParentState {
            task: "hello".to_string(),
            counter: 0,
            result: None,
        };

        let result = subgraph_node.execute(initial).await?;

        assert_eq!(result.task, "olleh");
        assert_eq!(result.counter, 5);
        assert_eq!(result.result, Some("Transformed: olleh".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_preserves_unrelated_parent_fields() -> Result<()> {
        // Test that subgraph doesn't accidentally modify unrelated parent fields
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("modify", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count = 999;
                state.input = "modified".to_string();
                Ok(state)
            })
        });
        child_graph.add_edge("modify", END);
        child_graph.set_entry_point("modify");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "preserve_test",
            compiled_child,
            |_parent: &ParentState| ChildState {
                input: "".to_string(),
                count: 0,
            },
            // Only update counter, leave task and result untouched
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                ..parent
            },
        );

        let initial = ParentState {
            task: "important_data".to_string(),
            counter: 42,
            result: Some("critical_result".to_string()),
        };

        let result = subgraph_node.execute(initial).await?;

        // Only counter should change
        assert_eq!(result.task, "important_data");
        assert_eq!(result.result, Some("critical_result".to_string()));
        assert_eq!(result.counter, 999);

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_zero_and_negative_values() -> Result<()> {
        // Test edge cases with zero and negative values
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("negate", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count = -state.count;
                Ok(state)
            })
        });
        child_graph.add_edge("negate", END);
        child_graph.set_entry_point("negate");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "negate_sub",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                ..parent
            },
        );

        // Test with positive
        let pos = ParentState {
            task: "pos".to_string(),
            counter: 100,
            result: None,
        };
        let pos_result = subgraph_node.execute(pos).await?;
        assert_eq!(pos_result.counter, -100);

        // Test with negative
        let neg = ParentState {
            task: "neg".to_string(),
            counter: -50,
            result: None,
        };
        let neg_result = subgraph_node.execute(neg).await?;
        assert_eq!(neg_result.counter, 50);

        // Test with zero
        let zero = ParentState {
            task: "zero".to_string(),
            counter: 0,
            result: None,
        };
        let zero_result = subgraph_node.execute(zero).await?;
        assert_eq!(zero_result.counter, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_with_multi_step_child() -> Result<()> {
        // Test subgraph with multiple sequential steps
        let mut child_graph = StateGraph::<ChildState>::new();

        child_graph.add_node_from_fn("step1", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 5;
                Ok(state)
            })
        });

        child_graph.add_node_from_fn("step2", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count *= 3;
                Ok(state)
            })
        });

        child_graph.add_node_from_fn("step3", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count -= 2;
                Ok(state)
            })
        });

        child_graph.add_edge("step1", "step2");
        child_graph.add_edge("step2", "step3");
        child_graph.add_edge("step3", END);
        child_graph.set_entry_point("step1");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "multi_step",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                result: Some(format!("Multi-step result: {}", child.count)),
                ..parent
            },
        );

        let initial = ParentState {
            task: "multi".to_string(),
            counter: 10,
            result: None,
        };

        let result = subgraph_node.execute(initial).await?;

        // Expected: (10 + 5) * 3 - 2 = 45 - 2 = 43
        assert_eq!(result.counter, 43);
        assert_eq!(result.result, Some("Multi-step result: 43".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_name_with_special_chars() -> Result<()> {
        // Test subgraph names with special characters
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
        child_graph.add_edge("test", END);
        child_graph.set_entry_point("test");

        let names = vec![
            "sub-graph-123",
            "sub_graph_456",
            "sub.graph.789",
            "sub:graph:abc",
            "my/subgraph",
            "subgraph@v2",
        ];

        for name in names {
            // Recreate child graph for each iteration since we can't clone CompiledGraph
            let mut child = StateGraph::<ChildState>::new();
            child.add_node_from_fn("test", |state| Box::pin(async move { Ok(state) }));
            child.add_edge("test", END);
            child.set_entry_point("test");
            let compiled = child.compile()?;

            let subgraph_node = SubgraphNode::new(
                name,
                compiled,
                |parent: &ParentState| ChildState {
                    input: parent.task.clone(),
                    count: parent.counter,
                },
                |parent: ParentState, child: ChildState| ParentState {
                    counter: child.count,
                    ..parent
                },
            );

            assert_eq!(subgraph_node.name(), format!("Subgraph({})", name));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_arc_internal_sharing() -> Result<()> {
        // Test that SubgraphNode uses Arc internally for efficient memory usage
        // The Arc<CompiledGraph> is stored inside SubgraphNode
        let mut child_graph = StateGraph::<ChildState>::new();
        child_graph.add_node_from_fn("increment", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 1;
                Ok(state)
            })
        });
        child_graph.add_edge("increment", END);
        child_graph.set_entry_point("increment");

        let compiled_child = child_graph.compile()?;

        // Create a subgraph node - internally uses Arc<CompiledGraph>
        let node = SubgraphNode::new(
            "shared",
            compiled_child,
            |parent: &ParentState| ChildState {
                input: parent.task.clone(),
                count: parent.counter,
            },
            |parent: ParentState, child: ChildState| ParentState {
                counter: child.count,
                ..parent
            },
        );

        // Verify it works multiple times (Arc allows multiple borrows)
        let state1 = ParentState {
            task: "test1".to_string(),
            counter: 10,
            result: None,
        };
        let result1 = node.execute(state1).await?;
        assert_eq!(result1.counter, 11);

        let state2 = ParentState {
            task: "test2".to_string(),
            counter: 20,
            result: None,
        };
        let result2 = node.execute(state2).await?;
        assert_eq!(result2.counter, 21);

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_deep_nesting_4_levels() -> Result<()> {
        // Test deep nesting with 4 levels

        // Level 4 (innermost) - multiply by 2
        let mut level4 = StateGraph::<ChildState>::new();
        level4.add_node_from_fn("mul2", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count *= 2;
                Ok(state)
            })
        });
        level4.add_edge("mul2", END);
        level4.set_entry_point("mul2");

        // Level 3 - add 10, then call level4
        let mut level3 = StateGraph::<ChildState>::new();
        level3.add_node_from_fn("add10", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count += 10;
                Ok(state)
            })
        });
        level3
            .add_subgraph_with_mapping(
                "level4",
                level4,
                |p: &ChildState| ChildState {
                    input: p.input.clone(),
                    count: p.count,
                },
                |p: ChildState, c: ChildState| ChildState {
                    count: c.count,
                    ..p
                },
            )?
            .add_edge("add10", "level4")
            .add_edge("level4", END)
            .set_entry_point("add10");

        // Level 2 - subtract 3, then call level3
        let mut level2 = StateGraph::<ChildState>::new();
        level2.add_node_from_fn("sub3", |state| {
            Box::pin(async move {
                let mut state = state;
                state.count -= 3;
                Ok(state)
            })
        });
        level2
            .add_subgraph_with_mapping(
                "level3",
                level3,
                |p: &ChildState| ChildState {
                    input: p.input.clone(),
                    count: p.count,
                },
                |p: ChildState, c: ChildState| ChildState {
                    count: c.count,
                    ..p
                },
            )?
            .add_edge("sub3", "level3")
            .add_edge("level3", END)
            .set_entry_point("sub3");

        // Level 1 (parent) - call level2
        let mut level1 = StateGraph::<ParentState>::new();
        level1
            .add_subgraph_with_mapping(
                "level2",
                level2,
                |p: &ParentState| ChildState {
                    input: p.task.clone(),
                    count: p.counter,
                },
                |p: ParentState, c: ChildState| ParentState {
                    counter: c.count,
                    result: Some(format!("Deep: {}", c.count)),
                    ..p
                },
            )?
            .add_edge("level2", END)
            .set_entry_point("level2");

        let compiled = level1.compile()?;

        let initial = ParentState {
            task: "deep".to_string(),
            counter: 20,
            result: None,
        };

        let result = compiled.invoke(initial).await?;

        // Expected: ((20 - 3) + 10) * 2 = 27 * 2 = 54
        assert_eq!(result.final_state.counter, 54);
        assert_eq!(result.final_state.result, Some("Deep: 54".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_subgraph_with_same_state_type() -> Result<()> {
        // Test subgraph where parent and child have the same state type
        // (Identity mapping - unusual but valid)

        let mut child_graph = StateGraph::<ParentState>::new();
        child_graph.add_node_from_fn("process", |state| {
            Box::pin(async move {
                let mut state = state;
                state.counter *= 2;
                state.result = Some("Processed".to_string());
                Ok(state)
            })
        });
        child_graph.add_edge("process", END);
        child_graph.set_entry_point("process");

        let compiled_child = child_graph.compile()?;

        let subgraph_node = SubgraphNode::new(
            "same_type",
            compiled_child,
            |parent: &ParentState| parent.clone(), // Identity mapping to child
            |_parent: ParentState, child: ParentState| child, // Use child result directly
        );

        let initial = ParentState {
            task: "same".to_string(),
            counter: 15,
            result: None,
        };

        let result = subgraph_node.execute(initial).await?;

        assert_eq!(result.counter, 30);
        assert_eq!(result.result, Some("Processed".to_string()));
        assert_eq!(result.task, "same");

        Ok(())
    }
}
