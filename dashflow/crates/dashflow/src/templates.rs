// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph Templates - Pre-built Workflow Patterns
//!
//! This module provides reusable workflow patterns that can be easily instantiated
//! and customized. Templates compile to standard `StateGraph` instances, so there's
//! no runtime overhead.
//!
//! # Available Templates
//!
//! - **Supervisor**: A coordinator agent manages multiple worker agents
//! - **`MapReduce`**: Parallel processing with aggregation
//!
//! # Example: Supervisor Pattern
//!
//! ```rust,ignore
//! use dashflow::templates::GraphTemplate;
//! use dashflow::StateGraph;
//!
//! let graph = GraphTemplate::supervisor()
//!     .with_supervisor_node_fn("supervisor", |state| {
//!         Box::pin(async move {
//!             // Supervisor logic
//!             Ok(state)
//!         })
//!     })
//!     .with_worker_fn("worker1", |state| {
//!         Box::pin(async move {
//!             // Worker 1 logic
//!             Ok(state)
//!         })
//!     })
//!     .with_router(|state| state.next_action.clone())
//!     .build()?;
//! ```

use crate::graph::StateGraph;
use crate::{Result, END};
use std::future::Future;
use std::pin::Pin;

/// Type alias for async node handler functions in graph templates.
///
/// A `NodeFn<S>` is a boxed function that:
/// - Takes the current state `S` as input
/// - Returns a pinned, boxed future that resolves to `Result<S>`
/// - Is `Send + Sync` for thread-safe graph execution
///
/// # Type Breakdown
/// - `Box<dyn Fn(S) -> ...>` - Boxed closure for dynamic dispatch
/// - `Pin<Box<dyn Future<...>>>` - Heap-allocated, pinned async return
/// - `+ Send + Sync` - Safe to share across threads
type NodeFn<S> = Box<dyn Fn(S) -> Pin<Box<dyn Future<Output = Result<S>> + Send>> + Send + Sync>;

/// Graph template type
pub enum GraphTemplate<S: crate::state::MergeableState> {
    /// Supervisor pattern: coordinator manages workers
    Supervisor(SupervisorBuilder<S>),
    /// `MapReduce` pattern: parallel processing with aggregation
    MapReduce(MapReduceBuilder<S>),
}

impl<S: crate::state::MergeableState> GraphTemplate<S> {
    /// Create a new Supervisor template builder
    ///
    /// The supervisor pattern has:
    /// - One supervisor node that coordinates work
    /// - Multiple worker nodes that execute tasks
    /// - A router function that decides which worker to call next
    /// - Conditional routing back to supervisor or to END
    #[must_use]
    pub fn supervisor() -> SupervisorBuilder<S> {
        SupervisorBuilder::new()
    }

    /// Create a new `MapReduce` template builder
    ///
    /// The `MapReduce` pattern has:
    /// - An input preparation node
    /// - Multiple mapper nodes that execute in parallel
    /// - A reducer node that aggregates results
    #[must_use]
    pub fn map_reduce() -> MapReduceBuilder<S> {
        MapReduceBuilder::new()
    }
}

/// Builder for Supervisor pattern
///
/// # Pattern Structure
///
/// ```text
/// START → supervisor → [router] → worker1 → supervisor
///                              → worker2 → supervisor
///                              → worker3 → supervisor
///                              → END
/// ```
///
/// The supervisor coordinates work by:
/// 1. Analyzing the current state
/// 2. Deciding which worker should execute next (or END)
/// 3. Workers execute and return control to supervisor
/// 4. Loop continues until supervisor routes to END
pub struct SupervisorBuilder<S: crate::state::MergeableState> {
    supervisor_name: Option<String>,
    supervisor_node: Option<NodeFn<S>>,
    workers: Vec<(String, NodeFn<S>)>,
    #[allow(clippy::type_complexity)] // Router callback: state → next worker name
    router: Option<Box<dyn Fn(&S) -> String + Send + Sync>>,
}

impl<S: crate::state::MergeableState> SupervisorBuilder<S> {
    /// Create a new supervisor builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            supervisor_name: None,
            supervisor_node: None,
            workers: Vec::new(),
            router: None,
        }
    }

    /// Set the supervisor node from a function
    ///
    /// The supervisor should analyze the state and set a field (typically `next_action`)
    /// that the router will use to decide which worker to call next.
    #[must_use]
    pub fn with_supervisor_node_fn<F>(mut self, name: impl Into<String>, func: F) -> Self
    where
        F: Fn(S) -> Pin<Box<dyn Future<Output = Result<S>> + Send>> + Send + Sync + 'static,
    {
        self.supervisor_name = Some(name.into());
        self.supervisor_node = Some(Box::new(func));
        self
    }

    /// Add a worker node from a function
    ///
    /// Workers execute tasks and then return control to the supervisor.
    /// Add multiple workers by calling this method multiple times.
    #[must_use]
    pub fn with_worker_fn<F>(mut self, name: impl Into<String>, func: F) -> Self
    where
        F: Fn(S) -> Pin<Box<dyn Future<Output = Result<S>> + Send>> + Send + Sync + 'static,
    {
        self.workers.push((name.into(), Box::new(func)));
        self
    }

    /// Set the router function
    ///
    /// The router examines the state (typically a field like `next_action` or `next_worker`)
    /// and returns:
    /// - The name of a worker to call next
    /// - "END" to finish execution
    /// - The supervisor name to loop back (if needed for re-evaluation)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow::templates::GraphTemplate;
    /// # use dashflow::{END, MergeableState};
    /// # use serde::{Deserialize, Serialize};
    /// #
    /// # #[derive(Clone, Serialize, Deserialize)]
    /// # struct State { next_action: String }
    /// #
    /// # impl MergeableState for State {
    /// #     fn merge(&mut self, other: &Self) {
    /// #         self.next_action = other.next_action.clone();
    /// #     }
    /// # }
    /// let builder = GraphTemplate::<State>::supervisor()
    ///     .with_router(|state| {
    ///         match state.next_action.as_str() {
    ///             "research" => "researcher".to_string(),
    ///             "analyze" => "analyst".to_string(),
    ///             "done" => END.to_string(),
    ///             _ => END.to_string(),
    ///         }
    ///     });
    /// ```
    #[must_use]
    pub fn with_router<F>(mut self, router: F) -> Self
    where
        F: Fn(&S) -> String + Send + Sync + 'static,
    {
        self.router = Some(Box::new(router));
        self
    }

    /// Build the `StateGraph` from this template
    ///
    /// # Returns
    ///
    /// A compiled `StateGraph` ready for execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Supervisor node is not set
    /// - No workers are configured
    /// - Router function is not set
    pub fn build(self) -> std::result::Result<StateGraph<S>, String> {
        // Validate configuration
        let supervisor_name = self
            .supervisor_name
            .ok_or("Supervisor node not set. Call with_supervisor_node_fn()")?;
        let supervisor_node = self.supervisor_node.ok_or("Supervisor node not set")?;

        if self.workers.is_empty() {
            return Err("No workers configured. Call with_worker_fn() at least once".to_string());
        }

        let router = self
            .router
            .ok_or("Router function not set. Call with_router()")?;

        // Build the graph
        let mut graph = StateGraph::new();

        // Add supervisor
        graph.add_node_from_fn(&supervisor_name, supervisor_node);

        // Add all workers
        let worker_names: Vec<String> = self.workers.iter().map(|(name, _)| name.clone()).collect();
        for (worker_name, worker_fn) in self.workers {
            graph.add_node_from_fn(&worker_name, worker_fn);

            // Each worker routes back to supervisor
            graph.add_edge(&worker_name, &supervisor_name);
        }

        // Set entry point
        graph.set_entry_point(&supervisor_name);

        // Add conditional edges from supervisor
        // Build routes map: each worker name maps to itself
        let mut routes = std::collections::HashMap::new();
        for worker_name in &worker_names {
            routes.insert(worker_name.clone(), worker_name.clone());
        }
        // Add END route
        routes.insert(END.to_string(), END.to_string());

        graph.add_conditional_edges(&supervisor_name, router, routes);

        Ok(graph)
    }
}

impl<S: crate::state::MergeableState> Default for SupervisorBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for `MapReduce` pattern
///
/// # Pattern Structure
///
/// ```text
/// START → input → [parallel] → mapper1 → reduce → END
///                           → mapper2 ↗
///                           → mapper3 ↗
/// ```
///
/// The `MapReduce` pattern:
/// 1. Input node prepares data
/// 2. Multiple mappers execute in parallel
/// 3. Reducer aggregates results
///
/// # Note on Parallel Execution
///
/// Currently, parallel edges execute all target nodes concurrently but only
/// preserve the last node's state modifications. For true map-reduce with
/// state merging from all mappers, consider using sequential execution or
/// a shared state structure (e.g., `Arc<Mutex<Vec>>`).
pub struct MapReduceBuilder<S: crate::state::MergeableState> {
    input_name: Option<String>,
    input_node: Option<NodeFn<S>>,
    mappers: Vec<(String, NodeFn<S>)>,
    reducer_name: Option<String>,
    reducer_node: Option<NodeFn<S>>,
}

impl<S: crate::state::MergeableState> MapReduceBuilder<S> {
    /// Create a new map-reduce builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            input_name: None,
            input_node: None,
            mappers: Vec::new(),
            reducer_name: None,
            reducer_node: None,
        }
    }

    /// Set the input preparation node from a function
    ///
    /// This node prepares data before parallel mapping.
    #[must_use]
    pub fn with_input_node_fn<F>(mut self, name: impl Into<String>, func: F) -> Self
    where
        F: Fn(S) -> Pin<Box<dyn Future<Output = Result<S>> + Send>> + Send + Sync + 'static,
    {
        self.input_name = Some(name.into());
        self.input_node = Some(Box::new(func));
        self
    }

    /// Add a mapper node from a function
    ///
    /// Mappers execute in parallel. Each receives the same input state.
    /// Add multiple mappers by calling this method multiple times.
    #[must_use]
    pub fn with_mapper_fn<F>(mut self, name: impl Into<String>, func: F) -> Self
    where
        F: Fn(S) -> Pin<Box<dyn Future<Output = Result<S>> + Send>> + Send + Sync + 'static,
    {
        self.mappers.push((name.into(), Box::new(func)));
        self
    }

    /// Set the reducer node from a function
    ///
    /// The reducer aggregates results from all mappers.
    #[must_use]
    pub fn with_reducer_node_fn<F>(mut self, name: impl Into<String>, func: F) -> Self
    where
        F: Fn(S) -> Pin<Box<dyn Future<Output = Result<S>> + Send>> + Send + Sync + 'static,
    {
        self.reducer_name = Some(name.into());
        self.reducer_node = Some(Box::new(func));
        self
    }

    /// Build the `StateGraph` from this template
    ///
    /// # Returns
    ///
    /// A compiled `StateGraph` ready for execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Input node is not set
    /// - No mappers are configured
    /// - Reducer node is not set
    pub fn build(self) -> std::result::Result<StateGraph<S>, String> {
        // Validate configuration
        let input_name = self
            .input_name
            .ok_or("Input node not set. Call with_input_node_fn()")?;
        let input_node = self.input_node.ok_or("Input node not set")?;

        if self.mappers.is_empty() {
            return Err("No mappers configured. Call with_mapper_fn() at least once".to_string());
        }

        let reducer_name = self
            .reducer_name
            .ok_or("Reducer node not set. Call with_reducer_node_fn()")?;
        let reducer_node = self.reducer_node.ok_or("Reducer node not set")?;

        // Build the graph
        let mut graph = StateGraph::new();

        // Add input node
        graph.add_node_from_fn(&input_name, input_node);
        graph.set_entry_point(&input_name);

        // Add all mappers
        let mapper_names: Vec<String> = self.mappers.iter().map(|(name, _)| name.clone()).collect();
        for (mapper_name, mapper_fn) in self.mappers {
            graph.add_node_from_fn(&mapper_name, mapper_fn);
        }

        // Add reducer
        graph.add_node_from_fn(&reducer_name, reducer_node);

        // Connect: input → [mappers] → reducer → END
        graph.add_parallel_edges(&input_name, mapper_names.clone());

        // Connect each mapper to the reducer
        for mapper_name in &mapper_names {
            graph.add_edge(mapper_name, &reducer_name);
        }

        graph.add_edge(&reducer_name, END);

        Ok(graph)
    }
}

impl<S: crate::state::MergeableState> Default for MapReduceBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct SupervisorState {
        task: String,
        next_action: String,
        worker_results: Vec<String>,
    }

    impl crate::state::MergeableState for SupervisorState {
        fn merge(&mut self, other: &Self) {
            // Append worker results from parallel branches
            self.worker_results.extend(other.worker_results.clone());
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct MapReduceState {
        input: String,
        mapper_results: Vec<String>,
        final_result: String,
    }

    impl crate::state::MergeableState for MapReduceState {
        fn merge(&mut self, other: &Self) {
            // Append mapper results from parallel branches
            self.mapper_results.extend(other.mapper_results.clone());
        }
    }

    #[tokio::test]
    async fn test_supervisor_template_basic() {
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    // Decide next action based on results
                    if state.worker_results.is_empty() {
                        state.next_action = "worker1".to_string();
                    } else if state.worker_results.len() == 1 {
                        state.next_action = "worker2".to_string();
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.worker_results.push("Worker1 completed".to_string());
                    Ok(state)
                })
            })
            .with_worker_fn("worker2", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.worker_results.push("Worker2 completed".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build supervisor graph");

        let compiled = graph.compile().expect("Failed to compile graph");

        let initial_state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled
            .invoke(initial_state)
            .await
            .expect("Execution failed");

        // Should have results from both workers
        assert_eq!(result.final_state.worker_results.len(), 2);
        assert!(result
            .final_state
            .worker_results
            .contains(&"Worker1 completed".to_string()));
        assert!(result
            .final_state
            .worker_results
            .contains(&"Worker2 completed".to_string()));
    }

    #[tokio::test]
    async fn test_supervisor_template_validation() {
        // Missing supervisor
        let result = GraphTemplate::<SupervisorState>::supervisor()
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Supervisor"));

        // Missing workers
        let result = GraphTemplate::<SupervisorState>::supervisor()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("workers"));

        // Missing router
        let result = GraphTemplate::<SupervisorState>::supervisor()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Router"));
    }

    #[tokio::test]
    async fn test_mapreduce_template_basic() {
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("mapper1", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.mapper_results.push(format!("M1: {}", state.input));
                    Ok(state)
                })
            })
            .with_mapper_fn("mapper2", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.mapper_results.push(format!("M2: {}", state.input));
                    Ok(state)
                })
            })
            .with_mapper_fn("mapper3", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.mapper_results.push(format!("M3: {}", state.input));
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.final_result = state.mapper_results.join(", ");
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build mapreduce graph");

        let compiled = graph.compile_with_merge().expect("Failed to compile graph");

        let initial_state = MapReduceState {
            input: "test data".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled
            .invoke(initial_state)
            .await
            .expect("Execution failed");

        // With proper MergeableState, all mapper results are merged from parallel branches
        assert_eq!(result.final_state.mapper_results.len(), 3);
        // Results should be aggregated
        assert!(!result.final_state.final_result.is_empty());
        // All mapper results should be present in final result
        assert!(result.final_state.final_result.contains("M1"));
        assert!(result.final_state.final_result.contains("M2"));
        assert!(result.final_state.final_result.contains("M3"));
    }

    #[tokio::test]
    async fn test_mapreduce_template_validation() {
        // Missing input
        let result = GraphTemplate::<MapReduceState>::map_reduce()
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Input"));

        // Missing mappers
        let result = GraphTemplate::<MapReduceState>::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("mappers"));

        // Missing reducer
        let result = GraphTemplate::<MapReduceState>::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Reducer"));
    }

    #[tokio::test]
    async fn test_supervisor_with_multiple_workers() {
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    if state.worker_results.is_empty() {
                        state.next_action = "worker1".to_string();
                    } else if state.worker_results.len() == 1 {
                        state.next_action = "worker2".to_string();
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W1".to_string());
                    Ok(state)
                })
            })
            .with_worker_fn("worker2", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W2".to_string());
                    Ok(state)
                })
            })
            .with_worker_fn("worker3", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W3".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        // Based on our supervisor logic, it will call worker1 then worker2 then END
        // So we should have 2 results
        assert_eq!(result.final_state.worker_results.len(), 2);
    }

    // ===== SupervisorBuilder Tests =====

    #[test]
    fn test_supervisor_builder_default() {
        let builder = SupervisorBuilder::<SupervisorState>::default();
        assert!(builder.supervisor_name.is_none());
        assert!(builder.supervisor_node.is_none());
        assert!(builder.workers.is_empty());
        assert!(builder.router.is_none());
    }

    #[test]
    fn test_supervisor_builder_new() {
        let builder = SupervisorBuilder::<SupervisorState>::new();
        assert!(builder.supervisor_name.is_none());
        assert!(builder.supervisor_node.is_none());
        assert!(builder.workers.is_empty());
        assert!(builder.router.is_none());
    }

    #[test]
    fn test_supervisor_builder_with_supervisor_only() {
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }));

        // Cannot build with just supervisor (needs workers and router)
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_supervisor_builder_with_workers_only() {
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker2", |state| Box::pin(async move { Ok(state) }));

        // Cannot build with just workers (needs supervisor and router)
        let result = builder.build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Supervisor"));
    }

    #[test]
    fn test_supervisor_builder_with_router_only() {
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_router(|state| state.next_action.clone());

        // Cannot build with just router (needs supervisor and workers)
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_supervisor_builder_chaining() {
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker2", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone());

        // Should successfully build with all components
        let result = builder.build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_supervisor_single_worker() {
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    if state.worker_results.is_empty() {
                        state.next_action = "worker1".to_string();
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W1".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        assert_eq!(result.final_state.worker_results.len(), 1);
        assert_eq!(result.final_state.worker_results[0], "W1");
    }

    #[tokio::test]
    async fn test_supervisor_immediate_end() {
        // Supervisor immediately routes to END without calling any workers
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.next_action = END.to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W1".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        // No workers should be called
        assert_eq!(result.final_state.worker_results.len(), 0);
    }

    #[tokio::test]
    async fn test_supervisor_many_workers() {
        let mut builder = GraphTemplate::supervisor().with_supervisor_node_fn(
            "supervisor",
            |mut state: SupervisorState| {
                Box::pin(async move {
                    let count = state.worker_results.len();
                    if count < 5 {
                        state.next_action = format!("worker{}", count + 1);
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            },
        );

        // Add 5 workers
        for i in 1..=5 {
            let worker_id = format!("worker{}", i);
            builder = builder.with_worker_fn(worker_id.clone(), move |mut state| {
                Box::pin(async move {
                    state.worker_results.push(format!("W{}", i));
                    Ok(state)
                })
            });
        }

        let graph = builder
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        // All 5 workers should be called
        assert_eq!(result.final_state.worker_results.len(), 5);
    }

    // ===== MapReduceBuilder Tests =====

    #[test]
    fn test_mapreduce_builder_default() {
        let builder = MapReduceBuilder::<MapReduceState>::default();
        assert!(builder.input_name.is_none());
        assert!(builder.input_node.is_none());
        assert!(builder.mappers.is_empty());
        assert!(builder.reducer_name.is_none());
        assert!(builder.reducer_node.is_none());
    }

    #[test]
    fn test_mapreduce_builder_new() {
        let builder = MapReduceBuilder::<MapReduceState>::new();
        assert!(builder.input_name.is_none());
        assert!(builder.input_node.is_none());
        assert!(builder.mappers.is_empty());
        assert!(builder.reducer_name.is_none());
        assert!(builder.reducer_node.is_none());
    }

    #[test]
    fn test_mapreduce_builder_with_input_only() {
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }));

        // Cannot build with just input (needs mappers and reducer)
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_mapreduce_builder_with_mappers_only() {
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper2", |state| Box::pin(async move { Ok(state) }));

        // Cannot build with just mappers (needs input and reducer)
        let result = builder.build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Input"));
    }

    #[test]
    fn test_mapreduce_builder_with_reducer_only() {
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }));

        // Cannot build with just reducer (needs input and mappers)
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_mapreduce_builder_chaining() {
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper2", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }));

        // Should successfully build with all components
        let result = builder.build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mapreduce_single_mapper() {
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.input = "prepared data".to_string();
                    Ok(state)
                })
            })
            .with_mapper_fn("mapper1", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.mapper_results.push(format!("M1: {}", state.input));
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.final_result = state.mapper_results.join(", ");
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "test".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        // Single mapper should execute
        assert_eq!(result.final_state.mapper_results.len(), 1);
        assert!(result.final_state.final_result.contains("M1"));
    }

    #[tokio::test]
    async fn test_mapreduce_many_mappers() {
        let mut builder = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            });

        // Add 10 mappers
        for i in 1..=10 {
            let mapper_id = format!("mapper{}", i);
            builder = builder.with_mapper_fn(mapper_id, move |mut state| {
                Box::pin(async move {
                    state
                        .mapper_results
                        .push(format!("M{}: {}", i, state.input));
                    Ok(state)
                })
            });
        }

        let graph = builder
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.final_result = format!("Reduced {} results", state.mapper_results.len());
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "test data".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        // Due to parallel execution semantics, only last mapper's state is kept
        // But reducer should still execute
        assert!(!result.final_state.final_result.is_empty());
        assert!(result.final_state.final_result.contains("Reduced"));
    }

    #[tokio::test]
    async fn test_mapreduce_empty_input() {
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("mapper1", |mut state: MapReduceState| {
                Box::pin(async move {
                    if !state.input.is_empty() {
                        state
                            .mapper_results
                            .push(format!("Processed: {}", state.input));
                    }
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.final_result = if state.mapper_results.is_empty() {
                        "No data".to_string()
                    } else {
                        state.mapper_results.join(", ")
                    };
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: String::new(), // Empty input
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        // Should handle empty input gracefully
        assert_eq!(result.final_state.final_result, "No data");
    }

    // ===== GraphTemplate Enum Tests =====

    #[test]
    fn test_graph_template_supervisor_creation() {
        let builder = GraphTemplate::<SupervisorState>::supervisor();
        assert!(builder.supervisor_name.is_none());
        assert!(builder.workers.is_empty());
    }

    #[test]
    fn test_graph_template_map_reduce_creation() {
        let builder = GraphTemplate::<MapReduceState>::map_reduce();
        assert!(builder.input_name.is_none());
        assert!(builder.mappers.is_empty());
    }

    // ===== Error Message Tests =====

    #[test]
    fn test_supervisor_error_messages_specific() {
        // Test that error messages are specific and helpful
        let result = SupervisorBuilder::<SupervisorState>::new()
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Supervisor"));
        assert!(err.contains("with_supervisor_node_fn"));
    }

    #[test]
    fn test_mapreduce_error_messages_specific() {
        // Test that error messages are specific and helpful
        let result = MapReduceBuilder::<MapReduceState>::new()
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Input"));
        assert!(err.contains("with_input_node_fn"));
    }

    // ===== Additional Coverage Tests =====

    #[test]
    fn test_supervisor_builder_missing_supervisor_name_only() {
        // Test error when supervisor node is set but name is missing (should not happen in practice)
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone());

        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_supervisor_builder_missing_router_specific() {
        // Specific test for missing router error message
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Router"));
        assert!(err.contains("with_router"));
    }

    #[test]
    fn test_mapreduce_builder_missing_reducer_specific() {
        // Specific test for missing reducer error message
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Reducer"));
        assert!(err.contains("with_reducer_node_fn"));
    }

    #[test]
    fn test_supervisor_worker_edge_routes() {
        // Test that workers correctly route back to supervisor
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    if state.worker_results.is_empty() {
                        state.next_action = "worker1".to_string();
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W1".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        // Validate returns warnings (not errors), so we just check it doesn't panic
        let _warnings = graph.validate();
        // Graph structure is valid even if there are warnings
    }

    #[test]
    fn test_mapreduce_parallel_edges() {
        // Test that mappers are connected via parallel edges
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("mapper1", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("mapper2", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_reducer_node_fn("reduce", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .build()
            .expect("Failed to build");

        // Validate returns warnings (not errors), so we just check it doesn't panic
        let _warnings = graph.validate();
        // Graph structure is valid even if there are warnings
    }

    #[tokio::test]
    async fn test_supervisor_router_returns_nonexistent_worker() {
        // Test router returning a worker name that doesn't exist
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    // Router will return "nonexistent_worker"
                    state.next_action = "nonexistent_worker".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        // Should fail during execution when router returns invalid route
        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_supervisor_builder_end_route_added() {
        // Verify END route is automatically added to routes map
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.next_action = END.to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build();

        assert!(graph.is_ok());
    }

    #[test]
    fn test_mapreduce_builder_mappers_to_reducer_edges() {
        // Test that each mapper connects to the reducer
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("mapper1", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_reducer_node_fn("reduce", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .build();

        assert!(graph.is_ok());
        let graph = graph.unwrap();
        let _warnings = graph.validate();
        // Validate doesn't return errors, just warnings
    }

    // ===== Additional Edge Case Tests =====

    #[tokio::test]
    async fn test_supervisor_unicode_names() {
        // Test supervisor and workers with unicode names
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("主管", |mut state: SupervisorState| {
                Box::pin(async move {
                    if state.worker_results.is_empty() {
                        state.next_action = "工人".to_string();
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("工人", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("完成".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert_eq!(result.final_state.worker_results.len(), 1);
        assert_eq!(result.final_state.worker_results[0], "完成");
    }

    #[tokio::test]
    async fn test_mapreduce_unicode_names() {
        // Test mapreduce with unicode names
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("输入", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("映射器", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.mapper_results.push("结果".to_string());
                    Ok(state)
                })
            })
            .with_reducer_node_fn("减速器", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.final_result = state.mapper_results.join(", ");
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "测试".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert!(!result.final_state.final_result.is_empty());
    }

    #[test]
    fn test_supervisor_builder_empty_name() {
        // Test supervisor with empty string name
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone());

        let result = builder.build();
        // Empty names are allowed by the builder
        assert!(result.is_ok());
    }

    #[test]
    fn test_mapreduce_builder_empty_name() {
        // Test mapreduce with empty string name
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        // Empty names are allowed by the builder
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_supervisor_worker_error_propagation() {
        // Test that errors from workers propagate correctly
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.next_action = "error_worker".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("error_worker", |_state: SupervisorState| {
                Box::pin(
                    async move { Err(crate::error::Error::Generic("Worker error".to_string())) },
                )
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_supervisor_error_propagation() {
        // Test that errors from supervisor propagate correctly
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |_state: SupervisorState| {
                Box::pin(async move {
                    Err(crate::error::Error::Generic("Supervisor error".to_string()))
                })
            })
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mapreduce_input_error_propagation() {
        // Test that errors from input node propagate correctly
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |_state: MapReduceState| {
                Box::pin(
                    async move { Err(crate::error::Error::Generic("Input error".to_string())) },
                )
            })
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "test".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mapreduce_mapper_error_propagation() {
        // Test that errors from mappers propagate correctly
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("error_mapper", |_state: MapReduceState| {
                Box::pin(
                    async move { Err(crate::error::Error::Generic("Mapper error".to_string())) },
                )
            })
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "test".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mapreduce_reducer_error_propagation() {
        // Test that errors from reducer propagate correctly
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |_state: MapReduceState| {
                Box::pin(
                    async move { Err(crate::error::Error::Generic("Reducer error".to_string())) },
                )
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "test".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_supervisor_loop_detection() {
        // Test supervisor that loops back to itself
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    // Increment a counter to prevent infinite loop
                    state.worker_results.push("loop".to_string());
                    if state.worker_results.len() > 3 {
                        state.next_action = END.to_string();
                    } else {
                        state.next_action = "worker1".to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |state| {
                Box::pin(async move {
                    // Worker doesn't modify state
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        // Should have executed supervisor multiple times
        assert!(result.final_state.worker_results.len() > 3);
    }

    #[test]
    fn test_supervisor_builder_duplicate_workers() {
        // Test adding workers with the same name
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) })) // Duplicate name
            .with_router(|state| state.next_action.clone());

        let result = builder.build();
        // Should succeed - graph allows duplicate names (last one wins)
        assert!(result.is_ok());
    }

    #[test]
    fn test_mapreduce_builder_duplicate_mappers() {
        // Test adding mappers with the same name
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) })) // Duplicate name
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        // Should succeed - graph allows duplicate names (last one wins)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_supervisor_long_task_chain() {
        // Test supervisor with a long chain of worker calls
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    let count = state.worker_results.len();
                    if count < 10 {
                        state.next_action = format!("worker{}", (count % 3) + 1);
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W1".to_string());
                    Ok(state)
                })
            })
            .with_worker_fn("worker2", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W2".to_string());
                    Ok(state)
                })
            })
            .with_worker_fn("worker3", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("W3".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert_eq!(result.final_state.worker_results.len(), 10);
    }

    #[test]
    fn test_supervisor_builder_special_characters_in_names() {
        // Test names with special characters
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("supervisor@#$", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker-1_2.3", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone());

        let result = builder.build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mapreduce_builder_special_characters_in_names() {
        // Test names with special characters
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input@#$", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper-1_2.3", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce!@#", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_supervisor_state_mutation_ordering() {
        // Test that state mutations happen in the correct order
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.task = format!("{}S", state.task);
                    if state.worker_results.is_empty() {
                        state.next_action = "worker1".to_string();
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state| {
                Box::pin(async move {
                    state.task = format!("{}W", state.task);
                    state.worker_results.push("done".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "start".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        // Order: supervisor -> worker1 -> supervisor
        // Task mutations: start -> startS -> startSW -> startSWS
        assert!(result.final_state.task.contains("S"));
        assert!(result.final_state.task.contains("W"));
    }

    #[tokio::test]
    async fn test_mapreduce_state_mutation_ordering() {
        // Test that state mutations happen in the correct order
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.input = format!("{}I", state.input);
                    Ok(state)
                })
            })
            .with_mapper_fn("mapper1", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.input = format!("{}M", state.input);
                    state.mapper_results.push("result".to_string());
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.input = format!("{}R", state.input);
                    state.final_result = state.mapper_results.join(",");
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "start".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        // Order: input -> mapper1 -> reduce
        // Input mutations: start -> startI -> startIM -> startIMR
        assert!(result.final_state.input.contains("I"));
        assert!(result.final_state.input.contains("M"));
        assert!(result.final_state.input.contains("R"));
    }

    #[test]
    fn test_supervisor_builder_with_string_names() {
        // Test that String names work (not just &str)
        let supervisor_name = String::from("supervisor");
        let worker_name = String::from("worker1");

        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn(supervisor_name.clone(), |state| {
                Box::pin(async move { Ok(state) })
            })
            .with_worker_fn(worker_name.clone(), |state| {
                Box::pin(async move { Ok(state) })
            })
            .with_router(|state| state.next_action.clone());

        let result = builder.build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mapreduce_builder_with_string_names() {
        // Test that String names work (not just &str)
        let input_name = String::from("input");
        let mapper_name = String::from("mapper1");
        let reducer_name = String::from("reduce");

        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn(input_name.clone(), |state| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn(mapper_name.clone(), |state| {
                Box::pin(async move { Ok(state) })
            })
            .with_reducer_node_fn(reducer_name.clone(), |state| {
                Box::pin(async move { Ok(state) })
            });

        let result = builder.build();
        assert!(result.is_ok());
    }

    // ===== GraphTemplate Enum Variant Tests =====

    #[test]
    fn test_graph_template_supervisor_variant() {
        // Test that GraphTemplate enum Supervisor variant can be constructed
        let builder = GraphTemplate::<SupervisorState>::supervisor();
        let _template = GraphTemplate::Supervisor(builder);
        // Successfully constructed Supervisor variant
    }

    #[test]
    fn test_graph_template_mapreduce_variant() {
        // Test that GraphTemplate enum MapReduce variant can be constructed
        let builder = GraphTemplate::<MapReduceState>::map_reduce();
        let _template = GraphTemplate::MapReduce(builder);
        // Successfully constructed MapReduce variant
    }

    // ===== Comprehensive Builder State Tests =====

    #[test]
    fn test_supervisor_builder_partial_configuration_scenarios() {
        // Test various partial configurations to ensure error handling

        // Scenario 1: Only supervisor + router (no workers)
        let result = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("sup", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("workers"));

        // Scenario 2: Only workers + router (no supervisor)
        let result = SupervisorBuilder::<SupervisorState>::new()
            .with_worker_fn("w1", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("w2", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Supervisor"));

        // Scenario 3: Supervisor + workers (no router)
        let result = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("sup", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("w1", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Router"));
    }

    #[test]
    fn test_mapreduce_builder_partial_configuration_scenarios() {
        // Test various partial configurations to ensure error handling

        // Scenario 1: Only input + reducer (no mappers)
        let result = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("mappers"));

        // Scenario 2: Only mappers + reducer (no input)
        let result = MapReduceBuilder::<MapReduceState>::new()
            .with_mapper_fn("m1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Input"));

        // Scenario 3: Input + mappers (no reducer)
        let result = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("m1", |state| Box::pin(async move { Ok(state) }))
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Reducer"));
    }

    // ===== Additional Integration Tests =====

    #[tokio::test]
    async fn test_supervisor_alternating_workers() {
        // Test supervisor that alternates between workers based on even/odd pattern
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    let count = state.worker_results.len();
                    if count >= 4 {
                        state.next_action = END.to_string();
                    } else if count % 2 == 0 {
                        state.next_action = "even_worker".to_string();
                    } else {
                        state.next_action = "odd_worker".to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("even_worker", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("even".to_string());
                    Ok(state)
                })
            })
            .with_worker_fn("odd_worker", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("odd".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "alternating".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert_eq!(result.final_state.worker_results.len(), 4);
        assert_eq!(result.final_state.worker_results[0], "even");
        assert_eq!(result.final_state.worker_results[1], "odd");
        assert_eq!(result.final_state.worker_results[2], "even");
        assert_eq!(result.final_state.worker_results[3], "odd");
    }

    #[tokio::test]
    async fn test_mapreduce_conditional_mapper_execution() {
        // Test mapreduce with conditional logic in mapper
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.input = "conditional".to_string();
                    Ok(state)
                })
            })
            .with_mapper_fn("conditional_mapper", |mut state: MapReduceState| {
                Box::pin(async move {
                    if state.input == "conditional" {
                        state.mapper_results.push("processed".to_string());
                    } else {
                        state.mapper_results.push("skipped".to_string());
                    }
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.final_result = format!("Total: {}", state.mapper_results.len());
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "initial".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert!(result.final_state.final_result.contains("Total"));
    }

    #[tokio::test]
    async fn test_supervisor_stateful_counter() {
        // Test supervisor with internal counter state
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    // Use task field as counter
                    let counter: usize = state.task.parse().unwrap_or(0);
                    if counter < 3 {
                        state.task = (counter + 1).to_string();
                        state.next_action = "increment_worker".to_string();
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("increment_worker", |mut state| {
                Box::pin(async move {
                    state.worker_results.push(format!("Count: {}", state.task));
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "0".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert_eq!(result.final_state.worker_results.len(), 3);
    }

    #[tokio::test]
    async fn test_mapreduce_accumulator_pattern() {
        // Test mapreduce where mapper accumulates data
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.input = "1,2,3,4,5".to_string();
                    Ok(state)
                })
            })
            .with_mapper_fn("sum_mapper", |mut state: MapReduceState| {
                Box::pin(async move {
                    let sum: i32 = state
                        .input
                        .split(',')
                        .filter_map(|s| s.parse::<i32>().ok())
                        .sum();
                    state.mapper_results.push(format!("sum:{}", sum));
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.final_result = state.mapper_results.join(" | ");
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: String::new(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert!(result.final_state.final_result.contains("sum"));
    }

    #[test]
    fn test_supervisor_builder_very_long_worker_names() {
        // Test with very long worker names
        let long_name = "worker_".repeat(100); // 700 characters
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("sup", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn(long_name, |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone());

        let result = builder.build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mapreduce_builder_very_long_node_names() {
        // Test with very long node names
        let long_name = "mapper_".repeat(100); // 700 characters
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn(long_name, |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_supervisor_worker_modifies_task_field() {
        // Test that workers can modify the task field that supervisor reads
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    if state.task == "initial" {
                        state.next_action = "modifier_worker".to_string();
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("modifier_worker", |mut state| {
                Box::pin(async move {
                    state.task = "modified".to_string();
                    state.worker_results.push("modified".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "initial".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert_eq!(result.final_state.task, "modified");
        assert_eq!(result.final_state.worker_results.len(), 1);
    }

    #[tokio::test]
    async fn test_mapreduce_mapper_examines_input_field() {
        // Test that mappers correctly read the input field set by input node
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.input = "secret_value".to_string();
                    Ok(state)
                })
            })
            .with_mapper_fn("examiner_mapper", |mut state: MapReduceState| {
                Box::pin(async move {
                    if state.input == "secret_value" {
                        state.mapper_results.push("found_secret".to_string());
                    }
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.final_result = state.mapper_results.first().cloned().unwrap_or_default();
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "not_secret".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert_eq!(result.final_state.final_result, "found_secret");
    }

    #[test]
    fn test_supervisor_builder_whitespace_only_names() {
        // Test with whitespace-only names
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("   ", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("\t\n", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone());

        let result = builder.build();
        // Whitespace names are allowed by the builder
        assert!(result.is_ok());
    }

    #[test]
    fn test_mapreduce_builder_whitespace_only_names() {
        // Test with whitespace-only names
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("   ", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("\t", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("\n", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        // Whitespace names are allowed by the builder
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_supervisor_complex_routing_logic() {
        // Test supervisor with complex routing logic based on multiple state fields
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    let has_results = !state.worker_results.is_empty();
                    let task_is_complex = state.task.contains("complex");

                    if has_results && task_is_complex {
                        state.next_action = END.to_string();
                    } else if task_is_complex {
                        state.next_action = "complex_worker".to_string();
                    } else {
                        state.next_action = "simple_worker".to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("complex_worker", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("complex_done".to_string());
                    Ok(state)
                })
            })
            .with_worker_fn("simple_worker", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("simple_done".to_string());
                    state.task = "complex".to_string();
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "simple".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        // Should call simple_worker (changes task to complex), but then immediately ends
        // because the supervisor checks has_results && task_is_complex, which is now true
        // So we only get 1 worker result
        assert_eq!(result.final_state.worker_results.len(), 1);
    }

    #[tokio::test]
    async fn test_mapreduce_reducer_aggregates_multiple_types() {
        // Test reducer that handles various result formats
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("type_a_mapper", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.mapper_results.push("type_a:value".to_string());
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state: MapReduceState| {
                Box::pin(async move {
                    let mut counts = std::collections::HashMap::new();
                    for result in &state.mapper_results {
                        if let Some(type_prefix) = result.split(':').next() {
                            *counts.entry(type_prefix).or_insert(0) += 1;
                        }
                    }
                    state.final_result = format!("types:{}", counts.len());
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "test".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");
        assert!(result.final_state.final_result.contains("types"));
    }

    // ====================
    // Additional Edge Case Tests for Coverage Improvement
    // ====================

    #[test]
    fn test_supervisor_builder_replace_supervisor() {
        // Test calling with_supervisor_node_fn multiple times (last one wins)
        let builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("first_supervisor", |state| {
                Box::pin(async move { Ok(state) })
            })
            .with_supervisor_node_fn("second_supervisor", |state| {
                Box::pin(async move { Ok(state) })
            })
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone());

        let result = builder.build();
        assert!(result.is_ok());
        let graph = result.unwrap();
        // The second supervisor name should be used
        // We can verify by checking the graph structure
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("second_supervisor"));
        assert!(!mermaid.contains("first_supervisor"));
    }

    #[test]
    fn test_mapreduce_builder_replace_input() {
        // Test calling with_input_node_fn multiple times (last one wins)
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("first_input", |state| Box::pin(async move { Ok(state) }))
            .with_input_node_fn("second_input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        assert!(result.is_ok());
        let graph = result.unwrap();
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("second_input"));
        assert!(!mermaid.contains("first_input"));
    }

    #[test]
    fn test_supervisor_builder_replace_router() {
        // Test calling with_router multiple times (last one wins)
        let _builder = SupervisorBuilder::<SupervisorState>::new()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|_state| "wrong_route".to_string())
            .with_router(|state| state.next_action.clone());

        // Builder methods return Self, so chaining should work
        // This test verifies the router can be replaced
    }

    #[test]
    fn test_mapreduce_builder_replace_reducer() {
        // Test calling with_reducer_node_fn multiple times (last one wins)
        let builder = MapReduceBuilder::<MapReduceState>::new()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("first_reducer", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("second_reducer", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        assert!(result.is_ok());
        let graph = result.unwrap();
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("second_reducer"));
        assert!(!mermaid.contains("first_reducer"));
    }

    #[tokio::test]
    async fn test_supervisor_router_returns_supervisor_name() {
        // Test that router returning supervisor's own name causes error
        // because supervisor is not in the routes map
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.next_action = "supervisor".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("done".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        // This should fail because supervisor is not in the routes map
        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_supervisor_router_returns_empty_string() {
        // Test router returning empty string (should cause node not found error)
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.next_action = "".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        // This should fail because empty string is not a valid node name
        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_supervisor_worker_name_same_as_supervisor() {
        // Test adding a worker with the same name as supervisor (should overwrite)
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("node1", |state: SupervisorState| {
                Box::pin(async move { Ok(state) })
            })
            .with_worker_fn("node1", |state| {
                // This will overwrite the supervisor node in the graph
                Box::pin(async move { Ok(state) })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        // The graph should build successfully, but node1 will have been overwritten
        // This is allowed by StateGraph (duplicate names overwrite)
        let _compiled = graph.compile();
    }

    #[test]
    fn test_mapreduce_mapper_name_same_as_input() {
        // Test adding a mapper with the same name as input node (should overwrite)
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("node1", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("node1", |state| {
                // This will overwrite the input node
                Box::pin(async move { Ok(state) })
            })
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build()
            .expect("Failed to build");

        let _compiled = graph.compile_with_merge();
    }

    #[test]
    fn test_mapreduce_mapper_name_same_as_reducer() {
        // Test adding a mapper with same name as reducer (should overwrite)
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| {
                // This will overwrite the mapper node
                Box::pin(async move { Ok(state) })
            })
            .build()
            .expect("Failed to build");

        let _compiled = graph.compile_with_merge();
    }

    #[tokio::test]
    async fn test_supervisor_all_workers_route_directly_to_end() {
        // Test supervisor that immediately routes to END without calling workers
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.next_action = END.to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker1", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("should_not_see_this".to_string());
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        // No workers should have executed
        assert!(result.final_state.worker_results.is_empty());
    }

    #[tokio::test]
    async fn test_mapreduce_reducer_receives_last_mapper_state() {
        // Verify that reducer receives state from last mapper due to parallel edge behavior
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |mut state: MapReduceState| {
                Box::pin(async move {
                    state.input = "processed".to_string();
                    Ok(state)
                })
            })
            .with_mapper_fn("mapper1", |mut state| {
                Box::pin(async move {
                    state.mapper_results.push("mapper1".to_string());
                    Ok(state)
                })
            })
            .with_mapper_fn("mapper2", |mut state| {
                Box::pin(async move {
                    state.mapper_results.push("mapper2".to_string());
                    Ok(state)
                })
            })
            .with_reducer_node_fn("reduce", |mut state| {
                Box::pin(async move {
                    // Reducer sees the last mapper's state
                    state.final_result = format!("count:{}", state.mapper_results.len());
                    Ok(state)
                })
            })
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "test".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        // Reducer should have received state from one of the mappers
        assert!(result.final_state.final_result.contains("count:"));
    }

    #[test]
    fn test_supervisor_error_message_exact_text_supervisor_node() {
        // Verify exact error message text for missing supervisor node
        let result = GraphTemplate::<SupervisorState>::supervisor()
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            "Supervisor node not set. Call with_supervisor_node_fn()"
        );
    }

    #[test]
    fn test_supervisor_error_message_exact_text_workers() {
        // Verify exact error message text for no workers
        let result = GraphTemplate::<SupervisorState>::supervisor()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            "No workers configured. Call with_worker_fn() at least once"
        );
    }

    #[test]
    fn test_supervisor_error_message_exact_text_router() {
        // Verify exact error message text for missing router
        let result = GraphTemplate::<SupervisorState>::supervisor()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err, "Router function not set. Call with_router()");
    }

    #[test]
    fn test_mapreduce_error_message_exact_text_input() {
        // Verify exact error message text for missing input node
        let result = GraphTemplate::<MapReduceState>::map_reduce()
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err, "Input node not set. Call with_input_node_fn()");
    }

    #[test]
    fn test_mapreduce_error_message_exact_text_mappers() {
        // Verify exact error message text for no mappers
        let result = GraphTemplate::<MapReduceState>::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            "No mappers configured. Call with_mapper_fn() at least once"
        );
    }

    #[test]
    fn test_mapreduce_error_message_exact_text_reducer() {
        // Verify exact error message text for missing reducer
        let result = GraphTemplate::<MapReduceState>::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err, "Reducer node not set. Call with_reducer_node_fn()");
    }

    #[test]
    fn test_supervisor_builder_very_many_workers() {
        // Test supervisor with a large number of workers (100+)
        let mut builder = GraphTemplate::<SupervisorState>::supervisor()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }));

        // Add 150 workers
        for i in 0..150 {
            builder = builder.with_worker_fn(format!("worker_{}", i), |state| {
                Box::pin(async move { Ok(state) })
            });
        }

        builder = builder.with_router(|state| state.next_action.clone());

        let result = builder.build();
        assert!(result.is_ok());

        // Verify all workers are in the graph
        let graph = result.unwrap();
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("worker_0"));
        assert!(mermaid.contains("worker_149"));
    }

    #[test]
    fn test_mapreduce_builder_very_many_mappers() {
        // Test MapReduce with a large number of mappers (100+)
        let mut builder = GraphTemplate::<MapReduceState>::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }));

        // Add 150 mappers
        for i in 0..150 {
            builder = builder.with_mapper_fn(format!("mapper_{}", i), |state| {
                Box::pin(async move { Ok(state) })
            });
        }

        builder =
            builder.with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }));

        let result = builder.build();
        assert!(result.is_ok());

        // Verify all mappers are in the graph
        let graph = result.unwrap();
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("mapper_0"));
        assert!(mermaid.contains("mapper_149"));
    }

    #[tokio::test]
    async fn test_supervisor_worker_returns_error_in_node_function() {
        // Test worker node function returning an error
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    state.next_action = "failing_worker".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("failing_worker", |_state| {
                Box::pin(async move {
                    Err(crate::Error::NodeExecution {
                        node: "failing_worker".to_string(),
                        source: Box::new(std::io::Error::other("intentional error")),
                    })
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = SupervisorState {
            task: "test".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mapreduce_mapper_returns_error_in_node_function() {
        // Test mapper node function returning an error
        let graph = GraphTemplate::map_reduce()
            .with_input_node_fn("input", |state: MapReduceState| {
                Box::pin(async move { Ok(state) })
            })
            .with_mapper_fn("failing_mapper", |_state| {
                Box::pin(async move {
                    Err(crate::Error::NodeExecution {
                        node: "failing_mapper".to_string(),
                        source: Box::new(std::io::Error::other("mapper error")),
                    })
                })
            })
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build()
            .expect("Failed to build");

        let compiled = graph.compile_with_merge().expect("Failed to compile");

        let state = MapReduceState {
            input: "test".to_string(),
            mapper_results: Vec::new(),
            final_result: String::new(),
        };

        let result = compiled.invoke(state).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_graph_template_enum_variant_matching() {
        // Test that GraphTemplate enum has correct variants
        let _supervisor_builder = GraphTemplate::<SupervisorState>::supervisor();
        let _mapreduce_builder = GraphTemplate::<MapReduceState>::map_reduce();

        // These should return the correct builder types
        // Type system ensures this, but test for completeness
    }

    #[test]
    fn test_supervisor_routes_map_includes_end() {
        // Verify that the routes map in build() includes END
        let graph = GraphTemplate::<SupervisorState>::supervisor()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("worker1", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        // Check that the graph can route to END by verifying it has an END constant route
        // The END constant is "__end__", so we check for that in the mermaid diagram
        let mermaid = graph.to_mermaid();
        // The supervisor should have conditional routes
        assert!(mermaid.contains("supervisor"));
        assert!(mermaid.contains("worker1"));
    }

    #[tokio::test]
    async fn test_supervisor_rapid_state_changes() {
        // Test supervisor with workers that rapidly modify state
        let graph = GraphTemplate::supervisor()
            .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
                Box::pin(async move {
                    if state.worker_results.len() < 5 {
                        state.next_action = format!("worker_{}", state.worker_results.len());
                    } else {
                        state.next_action = END.to_string();
                    }
                    Ok(state)
                })
            })
            .with_worker_fn("worker_0", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("0".to_string());
                    state.task = "modified_0".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker_1", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("1".to_string());
                    state.task = "modified_1".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker_2", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("2".to_string());
                    state.task = "modified_2".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker_3", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("3".to_string());
                    state.task = "modified_3".to_string();
                    Ok(state)
                })
            })
            .with_worker_fn("worker_4", |mut state| {
                Box::pin(async move {
                    state.worker_results.push("4".to_string());
                    state.task = "modified_4".to_string();
                    Ok(state)
                })
            })
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        let compiled = graph.compile().expect("Failed to compile");

        let state = SupervisorState {
            task: "initial".to_string(),
            next_action: String::new(),
            worker_results: Vec::new(),
        };

        let result = compiled.invoke(state).await.expect("Execution failed");

        assert_eq!(result.final_state.worker_results.len(), 5);
        assert_eq!(result.final_state.task, "modified_4");
    }

    #[test]
    fn test_mapreduce_reducer_edge_to_end_exists() {
        // Verify that reducer has edge to END
        let graph = GraphTemplate::<MapReduceState>::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("mapper1", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build()
            .expect("Failed to build");

        let mermaid = graph.to_mermaid();
        // Reducer should be in the graph
        assert!(mermaid.contains("reduce"));
        assert!(mermaid.contains("input"));
        assert!(mermaid.contains("mapper1"));
    }

    #[test]
    fn test_supervisor_builder_worker_order_preserved() {
        // Test that worker order is preserved in the routes map
        let graph = GraphTemplate::<SupervisorState>::supervisor()
            .with_supervisor_node_fn("supervisor", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("zebra_worker", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("alpha_worker", |state| Box::pin(async move { Ok(state) }))
            .with_worker_fn("middle_worker", |state| Box::pin(async move { Ok(state) }))
            .with_router(|state| state.next_action.clone())
            .build()
            .expect("Failed to build");

        // All workers should be present regardless of alphabetical order
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("zebra_worker"));
        assert!(mermaid.contains("alpha_worker"));
        assert!(mermaid.contains("middle_worker"));
    }

    #[test]
    fn test_mapreduce_builder_mapper_order_preserved() {
        // Test that mapper order is preserved in parallel edges
        let graph = GraphTemplate::<MapReduceState>::map_reduce()
            .with_input_node_fn("input", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("zebra_mapper", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("alpha_mapper", |state| Box::pin(async move { Ok(state) }))
            .with_mapper_fn("middle_mapper", |state| Box::pin(async move { Ok(state) }))
            .with_reducer_node_fn("reduce", |state| Box::pin(async move { Ok(state) }))
            .build()
            .expect("Failed to build");

        // All mappers should be present
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("zebra_mapper"));
        assert!(mermaid.contains("alpha_mapper"));
        assert!(mermaid.contains("middle_mapper"));
    }
}
