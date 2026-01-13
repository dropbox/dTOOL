// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for StateGraph
// - panic: panic!() for invalid graph configurations (node not found, edge validation)
// - unwrap_used: unwrap() on graph builder operations with preconditions
// - needless_pass_by_value: Builder methods take owned values for chaining
#![allow(clippy::panic, clippy::unwrap_used, clippy::needless_pass_by_value)]

//! `StateGraph` - Core graph builder
//!
//! `StateGraph` is the main interface for building graphs. You define nodes,
//! edges, and the entry point, then compile the graph for execution.
//!
//! # Performance Considerations
//!
//! DashFlow passes state as an owned value between nodes, requiring cloning at each step.
//! State design significantly impacts performance:
//!
//! - **Vec** (34% overhead): Best for sequential collections (messages, steps)
//! - **`HashMap`** (53-56% overhead): Avoid for large collections; use Vec<(K,V)> instead
//! - **Arc** (<1% overhead): Use for large read-only data (documents, embeddings)
//! - **Minimal state**: Only store data needed for decision-making
//!
//! See `docs/ARCHITECTURE.md` for detailed benchmarks and design patterns.

use std::collections::HashMap;
use std::sync::Arc;

#[cfg(kani)]
type StateGraphBuildHasher =
    std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>;
#[cfg(not(kani))]
type StateGraphBuildHasher = std::collections::hash_map::RandomState;

type StateGraphMap<K, V> = HashMap<K, V, StateGraphBuildHasher>;

use crate::edge::{ConditionalEdge, Edge, ParallelEdge, END};
use crate::error::{Error, Result};
use crate::executor::CompiledGraph;
use crate::node::{BoxedNode, FunctionNode, Node};
use crate::schema::{EdgeSchema, EdgeType, GraphSchema, NodeMetadata, NodeSchema};
use crate::subgraph::SubgraphNode;

/// A graph of nodes and edges with typed state
///
/// `StateGraph` is the builder for creating graphs. Add nodes and edges,
/// set the entry point, then compile the graph for execution.
///
/// All methods return `&mut Self` for fluent chaining:
///
/// # Example: Basic usage
///
/// ```rust,ignore
/// use dashflow::StateGraph;
///
/// let mut graph = StateGraph::new();
///
/// // Add nodes
/// graph.add_node("researcher", research_node);
/// graph.add_node("writer", writer_node);
///
/// // Add edges
/// graph.add_edge("researcher", "writer");
/// graph.add_edge("writer", END);
///
/// // Set entry point
/// graph.set_entry_point("researcher");
///
/// // Compile and run
/// let app = graph.compile()?;
/// let result = app.invoke(initial_state).await?;
/// ```
///
/// # Example: Fluent API (recommended)
///
/// ```rust,ignore
/// use dashflow::GraphBuilder;
///
/// // Create a mutable graph and chain method calls
/// let mut graph = GraphBuilder::new();
/// graph
///     .add_node("researcher", research_node)
///     .add_node("writer", writer_node)
///     .add_edge("researcher", "writer")
///     .add_edge("writer", END)
///     .set_entry_point("researcher");
///
/// // Compile consumes the graph
/// let app = graph.compile()?;
/// let result = app.invoke(initial_state).await?;
/// ```
///
/// # See Also
///
/// - [`GraphBuilder`] - Type alias with better IDE hints
/// - [`CompiledGraph`] - The compiled form ready for execution
/// - [`Node`] - The trait for graph nodes
/// - [`Edge`] - Unconditional transitions between nodes
/// - [`ConditionalEdge`] - Dynamic routing based on state
/// - [`ParallelEdge`] - Fan-out to multiple nodes
/// - [`END`] - Terminal node constant
/// - [`MergeableState`](crate::state::MergeableState) - Required for parallel edges
/// - [`RunnableConfig`](crate::core::config::RunnableConfig) - Execution configuration
///
/// **Note (v1.12.0):** The struct now accepts `S: GraphState`. Methods that require
/// parallel edge support (like `add_parallel_edges` and `compile_with_merge`) are
/// only available when `S: MergeableState`.
pub struct StateGraph<S>
where
    S: crate::state::GraphState,
{
    /// Nodes in the graph
    nodes: StateGraphMap<String, BoxedNode<S>>,
    /// Simple edges
    edges: Vec<Edge>,
    /// Conditional edges
    conditional_edges: Vec<Arc<ConditionalEdge<S>>>,
    /// Parallel edges (fan-out to multiple nodes)
    parallel_edges: Vec<ParallelEdge>,
    /// Entry point node
    entry_point: Option<String>,
    /// Track if parallel edges are used (for compile-time `MergeableState` check)
    has_parallel_edges: bool,
    /// Strict mode: error on duplicate node names instead of warning
    strict_mode: bool,
    /// Runtime-mutable node configurations (prompts, parameters, etc.)
    node_configs: StateGraphMap<String, crate::introspection::NodeConfig>,
    /// Node metadata for visualization and introspection
    node_metadata: StateGraphMap<String, NodeMetadata>,
    /// Graph-level description
    graph_description: Option<String>,
}

impl<S> Clone for StateGraph<S>
where
    S: crate::state::GraphState,
{
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(), // Arc clone is cheap
            edges: self.edges.clone(),
            conditional_edges: self.conditional_edges.clone(),
            parallel_edges: self.parallel_edges.clone(),
            entry_point: self.entry_point.clone(),
            has_parallel_edges: self.has_parallel_edges,
            strict_mode: self.strict_mode,
            node_configs: self.node_configs.clone(),
            node_metadata: self.node_metadata.clone(),
            graph_description: self.graph_description.clone(),
        }
    }
}

impl<S> std::fmt::Debug for StateGraph<S>
where
    S: crate::state::GraphState,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateGraph")
            .field("nodes", &format!("[{} nodes]", self.nodes.len()))
            .field("edges", &self.edges.len())
            .field("conditional_edges", &self.conditional_edges.len())
            .field("parallel_edges", &self.parallel_edges.len())
            .field("entry_point", &self.entry_point)
            .field("has_parallel_edges", &self.has_parallel_edges)
            .field("strict_mode", &self.strict_mode)
            .field("graph_description", &self.graph_description)
            .finish()
    }
}

impl<S> StateGraph<S>
where
    S: crate::state::GraphState,
{
    /// Create a new empty graph
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: StateGraphMap::new(),
            edges: Vec::new(),
            conditional_edges: Vec::new(),
            parallel_edges: Vec::new(),
            entry_point: None,
            has_parallel_edges: false,
            strict_mode: false,
            node_configs: StateGraphMap::new(),
            node_metadata: StateGraphMap::new(),
            graph_description: None,
        }
    }

    /// Enable strict mode - duplicate node names will return an error instead of a warning.
    ///
    /// In normal mode (default), adding a duplicate node logs a warning and overwrites.
    /// In strict mode, adding a duplicate node causes `try_add_node` to return an error.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut graph = StateGraph::new().strict();
    /// graph.try_add_node("my_node", node1)?;  // Ok
    /// graph.try_add_node("my_node", node2)?;  // Err(DuplicateNodeName)
    /// ```
    pub fn strict(mut self) -> Self {
        self.strict_mode = true;
        self
    }

    /// Check if strict mode is enabled
    pub fn is_strict(&self) -> bool {
        self.strict_mode
    }

    /// Add a node to the graph
    ///
    /// If a node with the same name already exists, it will be overwritten and
    /// a warning will be logged. For stricter behavior, use `try_add_node` with
    /// a graph created using `StateGraph::new().strict()`.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the node
    /// * `node` - Node implementation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph.add_node("researcher", ResearchNode::new());
    /// ```
    pub fn add_node(&mut self, name: impl Into<String>, node: impl Node<S> + 'static) -> &mut Self {
        let name = name.into();
        if self.nodes.contains_key(&name) {
            tracing::warn!(
                node_name = %name,
                "Node '{}' already exists, overwriting. Use add_node_or_replace() to suppress this warning.",
                name
            );
        }
        self.nodes.insert(name, Arc::new(node));
        self
    }

    /// Add a node to the graph, returning an error if the name already exists.
    ///
    /// This is useful in strict mode or when you want explicit error handling
    /// for duplicate node names.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the node
    /// * `node` - Node implementation
    ///
    /// # Errors
    ///
    /// Returns `Error::DuplicateNodeName` if a node with the same name already exists.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph.try_add_node("researcher", ResearchNode::new())?;
    /// ```
    pub fn try_add_node(
        &mut self,
        name: impl Into<String>,
        node: impl Node<S> + 'static,
    ) -> Result<&mut Self> {
        let name = name.into();
        if self.nodes.contains_key(&name) {
            return Err(Error::DuplicateNodeName(name));
        }
        self.nodes.insert(name, Arc::new(node));
        Ok(self)
    }

    /// Add or replace a node in the graph without warnings.
    ///
    /// Use this method when you intentionally want to replace an existing node.
    /// Unlike `add_node`, this does not log a warning on overwrite.
    ///
    /// # Arguments
    ///
    /// * `name` - Name for the node (may already exist)
    /// * `node` - Node implementation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Add initial node
    /// graph.add_node("researcher", ResearchNode::new());
    /// // Intentionally replace with a different implementation
    /// graph.add_node_or_replace("researcher", BetterResearchNode::new());
    /// ```
    pub fn add_node_or_replace(
        &mut self,
        name: impl Into<String>,
        node: impl Node<S> + 'static,
    ) -> &mut Self {
        self.nodes.insert(name.into(), Arc::new(node));
        self
    }

    /// Add a node from an async function
    ///
    /// This is a convenience method that wraps a function in a `FunctionNode`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph.add_node_from_fn("researcher", |state| {
    ///     Box::pin(async move {
    ///         // Process state
    ///         Ok(state)
    ///     })
    /// });
    /// ```
    pub fn add_node_from_fn<F>(&mut self, name: impl Into<String>, func: F) -> &mut Self
    where
        F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        let name_str = name.into();
        let node = FunctionNode::new(name_str.clone(), func);
        self.add_node(name_str, node)
    }

    /// Add a node with metadata for visualization and introspection
    ///
    /// This method allows attaching descriptions, type information, and other
    /// metadata to nodes for graph visualization dashboards.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::schema::{NodeMetadata, NodeType};
    ///
    /// graph.add_node_with_metadata(
    ///     "researcher",
    ///     NodeMetadata::new("Gathers research from Wikipedia, ArXiv, and other sources")
    ///         .with_node_type(NodeType::Tool)
    ///         .with_input_fields(vec!["topic"])
    ///         .with_output_fields(vec!["findings"]),
    ///     |state| Box::pin(async move { Ok(state) }),
    /// );
    /// ```
    pub fn add_node_with_metadata<F>(
        &mut self,
        name: impl Into<String>,
        metadata: NodeMetadata,
        func: F,
    ) -> &mut Self
    where
        F: Fn(S) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<S>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        let name_str = name.into();
        self.node_metadata.insert(name_str.clone(), metadata);
        let node = FunctionNode::new(name_str.clone(), func);
        self.add_node(name_str, node)
    }

    /// Set a description for an existing node
    ///
    /// Use this to add metadata to nodes that were added without metadata.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph.add_node("researcher", my_node);
    /// graph.set_node_metadata("researcher", NodeMetadata::new("Researches topics"));
    /// ```
    pub fn set_node_metadata(
        &mut self,
        name: impl Into<String>,
        metadata: NodeMetadata,
    ) -> &mut Self {
        self.node_metadata.insert(name.into(), metadata);
        self
    }

    /// Get metadata for a node
    pub fn get_node_metadata(&self, name: &str) -> Option<&NodeMetadata> {
        self.node_metadata.get(name)
    }

    /// Set a description for the entire graph
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph.with_description("Research agent that gathers and analyzes information");
    /// ```
    pub fn with_description(&mut self, description: impl Into<String>) -> &mut Self {
        self.graph_description = Some(description.into());
        self
    }

    /// Export the graph schema for visualization
    ///
    /// This creates a serializable representation of the graph structure
    /// including nodes, edges, and metadata.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let schema = graph.export_schema("my-research-agent");
    /// println!("{}", schema.to_json_pretty()?);
    /// ```
    pub fn export_schema(&self, name: impl Into<String>) -> GraphSchema {
        let name = name.into();
        let entry_point = self
            .entry_point
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let mut schema = GraphSchema::new(&name, &entry_point);
        schema.description = self.graph_description.clone();
        schema.state_type = Some(std::any::type_name::<S>().to_string());

        // Add nodes with metadata
        for node_name in self.nodes.keys() {
            let node_schema = if let Some(metadata) = self.node_metadata.get(node_name) {
                NodeSchema::from_metadata(node_name, metadata)
            } else {
                NodeSchema::from_name(node_name)
            };
            schema.add_node(node_schema);
        }

        // Add simple edges
        for edge in &self.edges {
            schema.add_edge(EdgeSchema::direct(edge.from.as_str(), edge.to.as_str()));
        }

        // Add conditional edges
        for cond_edge in &self.conditional_edges {
            // Get all possible targets from the routes
            let targets: Vec<String> = cond_edge
                .routes
                .values()
                .map(|s| s.as_str().to_string())
                .collect();
            schema.add_edge(EdgeSchema {
                from: cond_edge.from.as_str().to_string(),
                to: targets.first().cloned().unwrap_or_default(),
                edge_type: EdgeType::Conditional,
                label: Some("conditional".to_string()),
                conditional_targets: Some(targets),
            });
        }

        // Add parallel edges
        for par_edge in &self.parallel_edges {
            schema.add_edge(EdgeSchema::parallel(
                par_edge.from.as_str(),
                par_edge.to.iter().cloned().collect::<Vec<_>>(),
            ));
        }

        schema
    }

    /// Add a subgraph as a node with state mapping
    ///
    /// This allows embedding a graph with a different state type (`C`) inside
    /// this graph (with state type `S`). State mapping functions convert between
    /// the two state types.
    ///
    /// # Arguments
    ///
    /// * `name` - Name for the subgraph node
    /// * `child_graph` - The child graph to embed (must be a `StateGraph`, not compiled)
    /// * `map_to_child` - Function to map parent state → child state
    /// * `map_from_child` - Function to merge child result back into parent state
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Create a research subgraph
    /// let mut research_graph = StateGraph::<ResearchState>::new();
    /// research_graph.add_node("search", search_node);
    /// research_graph.add_node("analyze", analyze_node);
    /// research_graph.add_edge("search", "analyze");
    /// research_graph.set_entry_point("search");
    ///
    /// // Add to parent graph with state mapping
    /// main_graph.add_subgraph_with_mapping(
    ///     "research",
    ///     research_graph,
    ///     |parent: &ProjectState| ResearchState {
    ///         query: parent.task.clone(),
    ///         findings: Vec::new(),
    ///     },
    ///     |parent: ProjectState, child: ResearchState| ProjectState {
    ///         research_results: child.into(),
    ///         ..parent
    ///     },
    /// );
    /// ```
    pub fn add_subgraph_with_mapping<C, F1, F2>(
        &mut self,
        name: impl Into<String>,
        child_graph: StateGraph<C>,
        map_to_child: F1,
        map_from_child: F2,
    ) -> Result<&mut Self>
    where
        C: crate::state::MergeableState,
        F1: Fn(&S) -> C + Send + Sync + 'static,
        F2: Fn(S, C) -> S + Send + Sync + 'static,
    {
        let name_str = name.into();

        // Compile the child graph
        // Uses compile_with_merge since child state now requires MergeableState
        let compiled_child = child_graph.compile_with_merge()?;

        // Create subgraph node with mapping
        let subgraph_node = SubgraphNode::new(
            name_str.clone(),
            compiled_child,
            map_to_child,
            map_from_child,
        );

        // Add as a regular node
        self.add_node(name_str, subgraph_node);

        Ok(self)
    }

    /// Add a simple edge between two nodes
    ///
    /// # Arguments
    ///
    /// * `from` - Source node name
    /// * `to` - Destination node name (or END to finish execution)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph.add_edge("researcher", "writer");
    /// graph.add_edge("writer", END);
    /// ```
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) -> &mut Self {
        self.edges.push(Edge::new(from, to));
        self
    }

    /// Add a conditional edge
    ///
    /// The condition function examines the state and returns the name of
    /// the next node to execute.
    ///
    /// # Arguments
    ///
    /// * `from` - Source node name
    /// * `condition` - Function that returns next node name based on state
    /// * `routes` - Map of possible next nodes (for validation)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut routes = HashMap::new();
    /// routes.insert("continue".to_string(), "writer".to_string());
    /// routes.insert("end".to_string(), END.to_string());
    ///
    /// graph.add_conditional_edges(
    ///     "reviewer",
    ///     |state: &AgentState| {
    ///         if state.iteration < 3 {
    ///             "continue".to_string()
    ///         } else {
    ///             "end".to_string()
    ///         }
    ///     },
    ///     routes,
    /// );
    /// ```
    pub fn add_conditional_edges<F>(
        &mut self,
        from: impl Into<String>,
        condition: F,
        routes: HashMap<String, String>,
    ) -> &mut Self
    where
        F: Fn(&S) -> String + Send + Sync + 'static,
    {
        let edge = ConditionalEdge::new(from, condition, routes);
        self.conditional_edges.push(Arc::new(edge));
        self
    }

    /// Add a parallel edge that fans out to multiple nodes
    ///
    /// All target nodes execute concurrently with the same state.
    /// After all complete, execution continues with the last node
    /// in the parallel list's result.
    ///
    /// # Arguments
    ///
    /// * `from` - Source node name
    /// * `to` - List of target node names (all execute in parallel)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // After research, run writer and critic in parallel
    /// graph.add_parallel_edges("research", vec!["writer".to_string(), "critic".to_string()]);
    /// ```
    pub fn add_parallel_edges(&mut self, from: impl Into<String>, to: Vec<String>) -> &mut Self {
        self.has_parallel_edges = true; // Mark that this graph uses parallel edges
        let edge = ParallelEdge::new(from, to);
        self.parallel_edges.push(edge);
        self
    }

    /// Add a conditional edge (v1.0 compatibility)
    ///
    /// **DEPRECATED:** Use `add_conditional_edges` (plural) instead.
    ///
    /// This method is provided for backward compatibility with v1.0 code.
    /// It delegates to `add_conditional_edges`.
    ///
    /// # Migration
    ///
    /// ```rust,ignore
    /// // v1.0 (deprecated):
    /// graph.add_conditional_edge(from, condition, routes);
    ///
    /// // v1.6 (recommended):
    /// graph.add_conditional_edges(from, condition, routes);
    /// ```
    #[deprecated(
        since = "1.1.0",
        note = "Use `add_conditional_edges` (plural) instead for consistency with upstream API naming"
    )]
    pub fn add_conditional_edge<F>(
        &mut self,
        from: impl Into<String>,
        condition: F,
        routes: HashMap<String, String>,
    ) -> &mut Self
    where
        F: Fn(&S) -> String + Send + Sync + 'static,
    {
        self.add_conditional_edges(from, condition, routes)
    }

    /// Add a parallel edge (v1.0 compatibility)
    ///
    /// **DEPRECATED:** Use `add_parallel_edges` (plural) instead.
    ///
    /// This method is provided for backward compatibility with v1.0 code.
    /// It delegates to `add_parallel_edges`.
    ///
    /// # Migration
    ///
    /// ```rust,ignore
    /// // v1.0 (deprecated):
    /// graph.add_parallel_edge(from, targets);
    ///
    /// // v1.6 (recommended):
    /// graph.add_parallel_edges(from, targets);
    /// ```
    #[deprecated(
        since = "1.1.0",
        note = "Use `add_parallel_edges` (plural) instead for consistency with upstream API naming"
    )]
    pub fn add_parallel_edge(&mut self, from: impl Into<String>, to: Vec<String>) -> &mut Self {
        self.add_parallel_edges(from, to)
    }

    /// Set the entry point node
    ///
    /// The entry point is the first node executed when the graph runs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph.set_entry_point("researcher");
    /// ```
    pub fn set_entry_point(&mut self, node: impl Into<String>) -> &mut Self {
        self.entry_point = Some(node.into());
        self
    }

    /// Get a reference to a node by name
    ///
    /// Returns `None` if the node doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(node) = graph.get_node("researcher") {
    ///     // Access node
    /// }
    /// ```
    pub fn get_node(&self, name: &str) -> Option<&BoxedNode<S>> {
        self.nodes.get(name)
    }

    /// Get a mutable reference to a node by name
    ///
    /// Returns `None` if the node doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(node) = graph.get_node_mut("researcher") {
    ///     // Modify node
    /// }
    /// ```
    pub fn get_node_mut(&mut self, name: &str) -> Option<&mut BoxedNode<S>> {
        self.nodes.get_mut(name)
    }

    /// Replace a node with a new implementation
    ///
    /// This is useful for optimization where you want to keep the graph structure
    /// but swap out node implementations.
    ///
    /// Returns the old node if replacement was successful, or `None` if the node
    /// doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the node to replace
    /// * `new_node` - New node implementation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Replace an existing node with an optimized version
    /// if let Some(old_node) = graph.replace_node("classifier", optimized_node) {
    ///     println!("Replaced node");
    /// }
    /// ```
    pub fn replace_node(&mut self, name: &str, new_node: BoxedNode<S>) -> Option<BoxedNode<S>> {
        self.nodes.insert(name.to_string(), new_node)
    }

    /// Remove a node from the graph
    ///
    /// Returns the removed node if it existed, or `None` if it doesn't exist.
    ///
    /// Note: This does NOT remove edges connected to this node. You should
    /// manually remove those edges if needed.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the node to remove
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(removed) = graph.remove_node("old_node") {
    ///     println!("Removed node");
    /// }
    /// ```
    pub fn remove_node(&mut self, name: &str) -> Option<BoxedNode<S>> {
        self.nodes.remove(name)
    }

    /// Get an iterator over all node names in the graph
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// for name in graph.node_names() {
    ///     println!("Node: {}", name);
    /// }
    /// ```
    pub fn node_names(&self) -> impl Iterator<Item = &String> {
        self.nodes.keys()
    }

    /// Get an iterator over all nodes in the graph
    ///
    /// Returns an iterator of (name, node) pairs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// for (name, node) in graph.nodes() {
    ///     println!("Node: {}", name);
    /// }
    /// ```
    pub fn nodes(&self) -> impl Iterator<Item = (&String, &BoxedNode<S>)> {
        self.nodes.iter()
    }

    /// Get the entry point node name
    ///
    /// Returns `None` if no entry point has been set.
    pub fn get_entry_point(&self) -> Option<&str> {
        self.entry_point.as_deref()
    }

    /// Get all simple edges in the graph
    pub fn get_edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Get all conditional edges in the graph
    pub fn get_conditional_edges(&self) -> &[Arc<ConditionalEdge<S>>] {
        &self.conditional_edges
    }

    /// Get all parallel edges in the graph
    pub fn get_parallel_edges(&self) -> &[ParallelEdge] {
        &self.parallel_edges
    }

    // ========================================================================
    // Node Configuration API (Runtime Mutation)
    // ========================================================================

    /// Get the current configuration for a node.
    ///
    /// Returns `None` if no configuration has been set for the node.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let graph = StateGraph::<MyState>::new();
    /// graph.add_node("llm_agent", agent_node);
    /// graph.set_node_config("llm_agent", json!({"temperature": 0.7}), None);
    ///
    /// if let Some(config) = graph.get_node_config("llm_agent") {
    ///     println!("Temperature: {:?}", config.temperature());
    /// }
    /// ```
    #[must_use]
    pub fn get_node_config(&self, name: &str) -> Option<&crate::introspection::NodeConfig> {
        self.node_configs.get(name)
    }

    /// Get a mutable reference to a node configuration.
    #[must_use]
    pub fn get_node_config_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut crate::introspection::NodeConfig> {
        self.node_configs.get_mut(name)
    }

    /// Get all node configurations.
    #[must_use]
    #[cfg(not(kani))]
    pub fn get_all_node_configs(&self) -> &HashMap<String, crate::introspection::NodeConfig> {
        &self.node_configs
    }

    /// Get all node configurations.
    #[must_use]
    #[cfg(kani)]
    pub fn get_all_node_configs(
        &self,
    ) -> &StateGraphMap<String, crate::introspection::NodeConfig> {
        &self.node_configs
    }

    /// Set the configuration for a node.
    ///
    /// This creates or replaces the configuration for the specified node.
    /// For updating existing configurations with version tracking, use `update_node_config()`.
    ///
    /// # Arguments
    ///
    /// * `name` - The node name
    /// * `config` - The configuration to set
    /// * `updated_by` - Optional attribution (e.g., "human", "self_improvement", "ab_test")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// graph.set_node_config("researcher", json!({
    ///     "system_prompt": "You are a research assistant.",
    ///     "temperature": 0.3,
    ///     "max_tokens": 2000
    /// }), Some("human"));
    /// ```
    pub fn set_node_config(
        &mut self,
        name: impl Into<String>,
        config: serde_json::Value,
        updated_by: Option<&str>,
    ) -> &mut Self {
        let name = name.into();
        let node_type = if self.nodes.contains_key(&name) {
            "function" // Default type for existing nodes
        } else {
            "unknown"
        };
        let node_config = crate::introspection::NodeConfig::new(&name, node_type)
            .with_config(config)
            .with_updated_by(updated_by.unwrap_or(""));
        // Remove empty updated_by
        let mut node_config = node_config;
        if updated_by.is_none() {
            node_config.updated_by = None;
        }
        self.node_configs.insert(name, node_config);
        self
    }

    /// Update an existing node configuration, incrementing version and recomputing hash.
    ///
    /// This method is for updating existing configurations with proper versioning.
    /// Use `set_node_config()` to create new configurations.
    ///
    /// # Arguments
    ///
    /// * `name` - The node name
    /// * `config` - The new configuration
    /// * `updated_by` - Optional attribution
    ///
    /// # Returns
    ///
    /// Returns `Ok(previous_config)` on success, or `Err` if the node has no existing config.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Initial config
    /// graph.set_node_config("llm", json!({"temperature": 0.7}), None);
    ///
    /// // Update with version tracking
    /// let previous = graph.update_node_config("llm", json!({"temperature": 0.3}), Some("ab_test"))?;
    /// assert_eq!(graph.get_node_config("llm").unwrap().version, 2);
    /// ```
    pub fn update_node_config(
        &mut self,
        name: &str,
        config: serde_json::Value,
        updated_by: Option<String>,
    ) -> Result<serde_json::Value> {
        let node_config = self
            .node_configs
            .get_mut(name)
            .ok_or_else(|| Error::NodeNotFound(name.to_string()))?;
        Ok(node_config.update(config, updated_by))
    }

    /// Batch update multiple node configurations atomically.
    ///
    /// All updates are applied, and a map of previous configurations is returned.
    /// If any node doesn't have an existing config, this method creates a new one
    /// instead of returning an error (unlike `update_node_config`).
    ///
    /// # Arguments
    ///
    /// * `updates` - Map of node names to new configurations
    /// * `updated_by` - Optional attribution applied to all updates
    ///
    /// # Returns
    ///
    /// Map of node names to their previous configurations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    ///
    /// let mut updates = HashMap::new();
    /// updates.insert("node_a".to_string(), json!({"prompt": "A"}));
    /// updates.insert("node_b".to_string(), json!({"prompt": "B"}));
    ///
    /// let previous = graph.update_configs(updates, Some("bulk_update".to_string()));
    /// ```
    pub fn update_configs(
        &mut self,
        updates: HashMap<String, serde_json::Value>,
        updated_by: Option<String>,
    ) -> HashMap<String, serde_json::Value> {
        let mut previous = HashMap::new();
        for (name, config) in updates {
            if let Some(node_config) = self.node_configs.get_mut(&name) {
                // Update existing config
                let prev = node_config.update(config, updated_by.clone());
                previous.insert(name, prev);
            } else {
                // Create new config
                let node_type = if self.nodes.contains_key(&name) {
                    "function"
                } else {
                    "unknown"
                };
                let mut new_config =
                    crate::introspection::NodeConfig::new(&name, node_type).with_config(config);
                new_config.updated_by = updated_by.clone();
                previous.insert(name.clone(), serde_json::Value::Null);
                self.node_configs.insert(name, new_config);
            }
        }
        previous
    }

    /// Check if a node has a configuration.
    #[must_use]
    pub fn has_node_config(&self, name: &str) -> bool {
        self.node_configs.contains_key(name)
    }

    /// Remove a node configuration.
    ///
    /// Returns the removed configuration if it existed.
    pub fn remove_node_config(&mut self, name: &str) -> Option<crate::introspection::NodeConfig> {
        self.node_configs.remove(name)
    }

    /// Insert a complete NodeConfig directly.
    ///
    /// Unlike `set_node_config` which creates a NodeConfig from parts,
    /// this method accepts a pre-constructed NodeConfig. Useful for:
    /// - Importing graphs from manifests
    /// - Cloning configurations between graphs
    /// - Preserving version/hash/timestamp from external sources
    ///
    /// # Arguments
    ///
    /// * `name` - The node name (must match config.name for consistency)
    /// * `config` - The complete NodeConfig to insert
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Import a config from a manifest
    /// let config = NodeConfig::new("agent", "llm.chat")
    ///     .with_config(json!({"model": "gpt-4"}));
    /// graph.insert_node_config("agent", config);
    /// ```
    pub fn insert_node_config(
        &mut self,
        name: impl Into<String>,
        config: crate::introspection::NodeConfig,
    ) -> &mut Self {
        self.node_configs.insert(name.into(), config);
        self
    }

    /// Add a pre-constructed boxed node to the graph.
    ///
    /// This method accepts a `BoxedNode<S>` (`Arc<dyn Node<S>>`), useful for:
    /// - Dynamic graph construction from factories
    /// - Importing nodes from registries
    /// - Cloning nodes between graphs
    ///
    /// If a node with the same name exists, it will be overwritten with a warning.
    /// Use `add_boxed_node_or_replace` to suppress warnings.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the node
    /// * `node` - Pre-constructed boxed node
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Create node from factory
    /// let node = registry.create("llm.chat", &config)?;
    /// graph.add_boxed_node("agent", node);
    /// ```
    pub fn add_boxed_node(&mut self, name: impl Into<String>, node: BoxedNode<S>) -> &mut Self {
        let name = name.into();
        if self.nodes.contains_key(&name) {
            tracing::warn!(
                node_name = %name,
                "Node '{}' already exists, overwriting. Use add_boxed_node_or_replace() to suppress this warning.",
                name
            );
        }
        self.nodes.insert(name, node);
        self
    }

    /// Add or replace a pre-constructed boxed node without warnings.
    ///
    /// Like `add_boxed_node`, but does not emit a warning when replacing
    /// an existing node. Use this for intentional node replacement.
    ///
    /// # Arguments
    ///
    /// * `name` - Node name
    /// * `node` - Pre-constructed boxed node
    pub fn add_boxed_node_or_replace(
        &mut self,
        name: impl Into<String>,
        node: BoxedNode<S>,
    ) -> &mut Self {
        self.nodes.insert(name.into(), node);
        self
    }

    /// Check if a node with the given name exists.
    #[must_use]
    pub fn has_node(&self, name: &str) -> bool {
        self.nodes.contains_key(name)
    }

    /// Generate a graph manifest for AI introspection
    ///
    /// Creates a complete manifest of the graph structure that an AI agent
    /// can use to understand its own capabilities and structure.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let graph = StateGraph::new();
    /// graph.add_node("agent", agent_node);
    /// graph.add_edge("agent", END);
    /// graph.set_entry_point("agent");
    ///
    /// let manifest = graph.manifest();
    ///
    /// // AI can query: "What nodes do I have?"
    /// for name in manifest.node_names() {
    ///     println!("I have node: {}", name);
    /// }
    ///
    /// // Export as JSON for AI consumption
    /// let json = manifest.to_json().unwrap();
    /// ```
    #[must_use]
    pub fn manifest(&self) -> crate::introspection::GraphManifest {
        use crate::introspection::{
            EdgeManifest, GraphManifest, GraphMetadata, NodeManifest, NodeType,
        };

        let mut builder = GraphManifest::builder();

        // Set entry point
        if let Some(entry) = &self.entry_point {
            builder = builder.entry_point(entry.clone());
        }

        // Add nodes
        for name in self.nodes.keys() {
            let node_manifest = NodeManifest::new(name.clone(), NodeType::Function);
            builder = builder.add_node(name.clone(), node_manifest);
        }

        // Add simple edges
        for edge in &self.edges {
            let edge_manifest = EdgeManifest::simple(edge.from.as_str(), edge.to.as_str());
            builder = builder.add_edge(edge.from.as_str(), edge_manifest);
        }

        // Add conditional edges
        for cond_edge in &self.conditional_edges {
            for (label, target) in &cond_edge.routes {
                let edge_manifest = EdgeManifest::conditional(
                    cond_edge.from.as_str(),
                    target.as_str(),
                    label.clone(),
                );
                builder = builder.add_edge(cond_edge.from.as_str(), edge_manifest);
            }
        }

        // Add parallel edges
        for par_edge in &self.parallel_edges {
            for target in par_edge.to.iter() {
                let edge_manifest = EdgeManifest::parallel(par_edge.from.as_str(), target.as_str());
                builder = builder.add_edge(par_edge.from.as_str(), edge_manifest);
            }
        }

        // Add metadata
        let metadata = GraphMetadata::new()
            .with_cycles(self.has_cycles())
            .with_parallel_edges(self.has_parallel_edges);

        builder = builder.metadata(metadata);

        // Add node configurations if present
        if !self.node_configs.is_empty() {
            #[cfg(not(kani))]
            {
                builder = builder.node_configs(self.node_configs.clone());
            }
            #[cfg(kani)]
            {
                let configs: HashMap<String, crate::introspection::NodeConfig> =
                    self.node_configs.clone().into_iter().collect();
                builder = builder.node_configs(configs);
            }
        }

        // Build (entry_point required, but might not be set in uncompiled graph)
        builder.build().unwrap_or_else(|_| {
            // Fallback for graphs without entry point - hardcoded values are always valid.
            // Allow expect_used: These hardcoded values cannot fail to build a valid manifest.
            #[allow(clippy::expect_used)]
            GraphManifest::builder()
                .entry_point("__unset__")
                .metadata(GraphMetadata::new())
                .build()
                .expect("hardcoded manifest with __unset__ entry point is always valid")
        })
    }

    /// Get topological sort of nodes (dependency order)
    ///
    /// Returns nodes in an order such that if there is an edge from A to B,
    /// then A appears before B in the ordering. Uses Kahn's algorithm.
    ///
    /// # Returns
    ///
    /// Vector of node names in topological order, or None if graph has cycles
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let sorted = graph.topological_sort();
    /// if let Some(order) = sorted {
    ///     for node_name in order {
    ///         println!("Process node: {}", node_name);
    ///     }
    /// }
    /// ```
    pub fn topological_sort(&self) -> Option<Vec<String>> {
        use std::collections::{HashSet, VecDeque};

        // Build adjacency list and in-degree count
        let mut adj_list: StateGraphMap<String, Vec<String>> = StateGraphMap::new();
        let mut in_degree: StateGraphMap<String, usize> = StateGraphMap::new();

        // Initialize all nodes with 0 in-degree
        for node_name in self.nodes.keys() {
            in_degree.insert(node_name.clone(), 0);
            adj_list.insert(node_name.clone(), Vec::new());
        }

        // Build adjacency list and count in-degrees from simple edges
        for edge in &self.edges {
            let from = edge.from.as_str();
            let to = edge.to.as_str();

            // Skip END node
            if to == END {
                continue;
            }

            if let Some(neighbors) = adj_list.get_mut(from) {
                neighbors.push(to.to_string());
            }

            if let Some(degree) = in_degree.get_mut(to) {
                *degree += 1;
            }
        }

        // Add conditional edges (all possible routes)
        for edge in &self.conditional_edges {
            let from = edge.from.as_str();
            for to in edge.routes.values() {
                let to_str = to.as_str();
                if to_str == END {
                    continue;
                }

                if let Some(neighbors) = adj_list.get_mut(from) {
                    if !neighbors.contains(&to_str.to_string()) {
                        neighbors.push(to_str.to_string());
                    }
                }

                if let Some(degree) = in_degree.get_mut(to_str) {
                    *degree += 1;
                }
            }
        }

        // Add parallel edges
        for edge in &self.parallel_edges {
            let from = edge.from.as_str();
            for to in edge.to.iter() {
                if to == END {
                    continue;
                }

                if let Some(neighbors) = adj_list.get_mut(from) {
                    if !neighbors.contains(to) {
                        neighbors.push(to.clone());
                    }
                }

                if let Some(degree) = in_degree.get_mut(to) {
                    *degree += 1;
                }
            }
        }

        // Kahn's algorithm: Start with nodes that have no incoming edges
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut sorted = Vec::new();
        let mut visited = HashSet::new();

        while let Some(node) = queue.pop_front() {
            if visited.contains(&node) {
                continue;
            }

            visited.insert(node.clone());
            sorted.push(node.clone());

            // Reduce in-degree for all neighbors
            if let Some(neighbors) = adj_list.get(&node) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        // If not all nodes were visited, there's a cycle
        if sorted.len() != self.nodes.len() {
            return None;
        }

        Some(sorted)
    }

    /// Find all nodes reachable from the entry point
    ///
    /// This performs a breadth-first search from the entry point to find
    /// all nodes that can be reached through edges.
    fn find_reachable_nodes(&self) -> std::collections::HashSet<String> {
        use std::collections::{HashSet, VecDeque};

        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(entry) = &self.entry_point {
            queue.push_back(entry.clone());
            reachable.insert(entry.clone());
        }

        while let Some(current) = queue.pop_front() {
            // Check simple edges
            for edge in &self.edges {
                if edge.from.as_str() == current
                    && edge.to.as_str() != END
                    && !reachable.contains(edge.to.as_str())
                {
                    reachable.insert(edge.to.as_str().to_string());
                    queue.push_back(edge.to.as_str().to_string());
                }
            }

            // Check conditional edges
            for edge in &self.conditional_edges {
                if edge.from.as_str() == current {
                    for target in edge.routes.values() {
                        let target_str = target.as_str();
                        if target_str != END && !reachable.contains(target_str) {
                            reachable.insert(target_str.to_string());
                            queue.push_back(target_str.to_string());
                        }
                    }
                }
            }

            // Check parallel edges
            for edge in &self.parallel_edges {
                if edge.from.as_str() == current {
                    for target in edge.to.iter() {
                        if target != END && !reachable.contains(target) {
                            reachable.insert(target.clone());
                            queue.push_back(target.clone());
                        }
                    }
                }
            }
        }

        reachable
    }

    /// Check if the graph contains any cycles
    ///
    /// Returns true if there is a path from any node back to itself.
    /// Uses DFS with a recursion stack to correctly handle diamond patterns.
    fn has_cycles(&self) -> bool {
        use std::collections::HashSet;

        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        // Helper function for DFS
        fn dfs<S: crate::state::GraphState>(
            node: &str,
            graph: &StateGraph<S>,
            visited: &mut HashSet<String>,
            rec_stack: &mut HashSet<String>,
        ) -> bool {
            // Mark node as visited and add to recursion stack
            visited.insert(node.to_string());
            rec_stack.insert(node.to_string());

            // Check all outgoing edges
            // Simple edges
            for edge in &graph.edges {
                if edge.from.as_str() == node && edge.to.as_str() != END {
                    let target = edge.to.as_str();

                    // If target is in current path, we found a cycle
                    if rec_stack.contains(target) {
                        return true;
                    }

                    // If not visited, recurse
                    if !visited.contains(target) && dfs(target, graph, visited, rec_stack) {
                        return true;
                    }
                }
            }

            // Conditional edges
            for edge in &graph.conditional_edges {
                if edge.from.as_str() == node {
                    for target in edge.routes.values() {
                        let target_str = target.as_str();
                        if target_str != END {
                            // If target is in current path, we found a cycle
                            if rec_stack.contains(target_str) {
                                return true;
                            }

                            // If not visited, recurse
                            if !visited.contains(target_str)
                                && dfs(target_str, graph, visited, rec_stack)
                            {
                                return true;
                            }
                        }
                    }
                }
            }

            // Parallel edges
            for edge in &graph.parallel_edges {
                if edge.from.as_str() == node {
                    for target in edge.to.iter() {
                        if target != END {
                            // If target is in current path, we found a cycle
                            if rec_stack.contains(target.as_str()) {
                                return true;
                            }

                            // If not visited, recurse
                            if !visited.contains(target.as_str())
                                && dfs(target, graph, visited, rec_stack)
                            {
                                return true;
                            }
                        }
                    }
                }
            }

            // Remove from recursion stack before returning
            rec_stack.remove(node);
            false
        }

        // Check for cycles starting from each node
        for start_node in self.nodes.keys() {
            if !visited.contains(start_node.as_str())
                && dfs(start_node, self, &mut visited, &mut rec_stack)
            {
                return true;
            }
        }

        false
    }

    /// Validate graph structure and return warnings
    ///
    /// This performs advanced validation beyond basic compile-time checks.
    /// Returns a list of warnings about potential issues.
    ///
    /// Checks:
    /// - Unreachable nodes (nodes with no path from entry point)
    /// - Cycles in the graph
    /// - Conditional edges with incomplete route coverage
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check for entry point
        if self.entry_point.is_none() {
            warnings.push("No entry point set".to_string());
            return warnings; // Can't do further validation without entry point
        }

        // Find unreachable nodes
        let reachable = self.find_reachable_nodes();
        for node_name in self.nodes.keys() {
            if !reachable.contains(node_name) {
                warnings.push(format!(
                    "Node '{node_name}' is unreachable from entry point"
                ));
            }
        }

        // Check for cycles (warning, not error - cycles are valid in DashFlow)
        if self.has_cycles() {
            warnings.push(
                "Graph contains cycles (this is allowed but may lead to infinite loops)"
                    .to_string(),
            );
        }

        // Check conditional edges for potential missing routes
        for edge in &self.conditional_edges {
            if edge.routes.is_empty() {
                warnings.push(format!(
                    "Conditional edge from '{}' has no routes defined",
                    edge.from
                ));
            }
        }

        // Check for LLM-using nodes that are not optimizable (Runtime Validation)
        // This catches patterns like raw ChatModel usage instead of LLMNode
        for (name, node) in &self.nodes {
            if node.may_use_llm() && !node.is_optimizable() {
                warnings.push(format!(
                    "Node '{}' uses LLM but is not optimizable. Consider using LLMNode \
                     or other DashOpt-aware nodes for production use. \
                     See: ROADMAP_CURRENT.md",
                    name
                ));
            }
        }

        warnings
    }

    /// Generate a Mermaid flowchart diagram of the graph
    ///
    /// This creates a Mermaid diagram showing the graph structure,
    /// which can be rendered in documentation or debugging tools.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let diagram = graph.to_mermaid();
    /// println!("{}", diagram);
    /// // Save to file for rendering
    /// std::fs::write("graph.mmd", diagram)?;
    /// ```
    ///
    /// The generated diagram can be rendered in:
    /// - GitHub markdown (```mermaid ... ```)
    /// - Mermaid Live Editor (<https://mermaid.live>)
    /// - VS Code with Mermaid extension
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut diagram = String::from("flowchart TD\n");

        // Add entry point indicator
        if let Some(entry) = &self.entry_point {
            diagram.push_str(&format!("    Start([Start]) --> {entry}\n"));
        }

        // Add all nodes
        for node_name in self.nodes.keys() {
            diagram.push_str(&format!("    {node_name}[{node_name}]\n"));
        }

        // Add END node if referenced
        let has_end_ref = self.edges.iter().any(|e| e.to.as_str() == END)
            || self
                .conditional_edges
                .iter()
                .any(|e| e.routes.values().any(|v| v.as_str() == END))
            || self
                .parallel_edges
                .iter()
                .any(|e| e.to.iter().any(|t| t == END));

        if has_end_ref {
            diagram.push_str("    End([End])\n");
        }

        // Add simple edges
        for edge in &self.edges {
            let target = if edge.to.as_str() == END {
                "End".to_string()
            } else {
                edge.to.to_string()
            };
            diagram.push_str(&format!("    {} --> {}\n", edge.from, target));
        }

        // Add conditional edges
        for edge in &self.conditional_edges {
            for (condition, target) in &edge.routes {
                let target_str = target.as_str();
                let target_node = if target_str == END {
                    "End".to_string()
                } else {
                    target_str.to_string()
                };
                diagram.push_str(&format!(
                    "    {} -->|{}| {}\n",
                    edge.from, condition, target_node
                ));
            }
        }

        // Add parallel edges
        for edge in &self.parallel_edges {
            for target in edge.to.iter() {
                let target_node = if target == END {
                    "End".to_string()
                } else {
                    target.clone()
                };
                diagram.push_str(&format!("    {} ==> {}\n", edge.from, target_node));
            }
        }

        // Add styling
        diagram.push_str("\n    %% Styling\n");
        diagram.push_str("    classDef startEnd fill:#e1f5e1,stroke:#4caf50,stroke-width:3px\n");
        diagram.push_str("    classDef nodeStyle fill:#e3f2fd,stroke:#2196f3,stroke-width:2px\n");
        diagram.push_str("    class Start,End startEnd\n");

        // Apply node styling
        let node_list: Vec<_> = self.nodes.keys().cloned().collect();
        if !node_list.is_empty() {
            diagram.push_str(&format!("    class {} nodeStyle\n", node_list.join(",")));
        }

        diagram
    }

    /// Compile the graph for execution with full validation (default)
    ///
    /// This validates the graph structure and returns a `CompiledGraph`
    /// that can be invoked. **Validation is enabled by default** to catch
    /// common issues early.
    ///
    /// # Validation (Default-Enabled)
    ///
    /// The following validations are performed automatically:
    /// - Entry point exists and is valid
    /// - All edges reference existing nodes
    /// - No mixed edge types on a single node
    /// - No unreachable nodes (nodes with no path from entry point)
    /// - Conditional edges have defined routes
    /// - Cycles are detected (warning only, cycles are allowed)
    ///
    /// To skip advanced validation, use [`Self::compile_without_validation`].
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No entry point is set
    /// - Edges reference non-existent nodes
    /// - Graph structure is invalid
    /// - Unreachable nodes detected
    /// - Empty conditional routes detected
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = graph.compile()?;
    /// let result = app.invoke(initial_state).await?;
    /// ```
    pub fn compile(self) -> Result<CompiledGraph<S>> {
        // Perform advanced validation by default (opt-out pattern)
        self.compile_internal(true, false)
    }

    /// Compile the graph without advanced validation (opt-out)
    ///
    /// This skips the advanced validation checks (unreachable nodes,
    /// conditional route coverage, cycle detection) and only performs
    /// basic structural validation.
    ///
    /// **Use sparingly** - advanced validation helps catch issues early.
    /// This is useful for:
    /// - Dynamically generated graphs where unreachable nodes are expected
    /// - Performance-critical hot paths where validation overhead matters
    /// - Graphs where you've already manually validated structure
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Skip advanced validation for dynamic graphs
    /// let app = graph.compile_without_validation()?;
    /// ```
    pub fn compile_without_validation(self) -> Result<CompiledGraph<S>> {
        self.compile_internal(false, false)
    }

    /// Internal compile implementation with configurable validation
    fn compile_internal(
        self,
        advanced_validation: bool,
        allow_parallel: bool,
    ) -> Result<CompiledGraph<S>> {
        // Validate entry point exists
        let entry_point = self.entry_point.clone().ok_or(Error::NoEntryPoint)?;

        if !self.nodes.contains_key(&entry_point) {
            return Err(Error::NodeNotFound(entry_point));
        }

        // Validate all edges reference existing nodes
        for edge in &self.edges {
            if !self.nodes.contains_key(edge.from.as_str()) {
                return Err(Error::NodeNotFound(edge.from.as_str().to_string()));
            }
            if edge.to.as_str() != END && !self.nodes.contains_key(edge.to.as_str()) {
                return Err(Error::NodeNotFound(edge.to.as_str().to_string()));
            }
        }

        // Validate conditional edges
        for edge in &self.conditional_edges {
            if !self.nodes.contains_key(edge.from.as_str()) {
                return Err(Error::NodeNotFound(edge.from.as_str().to_string()));
            }
            for target in edge.routes.values() {
                let target_str = target.as_str();
                if target_str != END && !self.nodes.contains_key(target_str) {
                    return Err(Error::NodeNotFound(target_str.to_string()));
                }
            }
        }

        // Validate parallel edges
        for edge in &self.parallel_edges {
            if !self.nodes.contains_key(edge.from.as_str()) {
                return Err(Error::NodeNotFound(edge.from.as_str().to_string()));
            }
            for target in edge.to.iter() {
                if target != END && !self.nodes.contains_key(target) {
                    return Err(Error::NodeNotFound(target.clone()));
                }
            }
        }

        // Validate no mixed edge types (prevent edge priority confusion)
        // When a node has multiple edge types, only the highest priority executes:
        // Priority: conditional > parallel > simple
        let mut warnings = Vec::new();
        for node_name in self.nodes.keys() {
            let has_simple = self.edges.iter().any(|e| e.from.as_str() == node_name);
            let has_conditional = self
                .conditional_edges
                .iter()
                .any(|e| e.from.as_str() == node_name);
            let has_parallel = self
                .parallel_edges
                .iter()
                .any(|e| e.from.as_str() == node_name);

            let edge_type_count =
                u8::from(has_simple) + u8::from(has_conditional) + u8::from(has_parallel);

            if edge_type_count > 1 {
                let mut types = Vec::new();
                if has_conditional {
                    types.push("conditional");
                }
                if has_parallel {
                    types.push("parallel");
                }
                if has_simple {
                    types.push("simple");
                }

                warnings.push(format!(
                    "Node '{}' has multiple edge types ({}). Due to edge priority (conditional > parallel > simple), only {} edges will execute. This is likely unintentional.",
                    node_name,
                    types.join(", "),
                    types[0] // highest priority type
                ));
            }
        }

        // If there are warnings, return validation error
        if !warnings.is_empty() {
            return Err(Error::Validation(format!(
                "Mixed edge types detected:\n  - {}\n\nTo fix: Use only one edge type per node, or use explicit routing logic.",
                warnings.join("\n  - ")
            )));
        }

        // Check for parallel edges (requires MergeableState)
        if !allow_parallel && !self.parallel_edges.is_empty() {
            return Err(Error::Validation(
                "Graph uses parallel edges (add_parallel_edges), which requires MergeableState.\n\n\
                 To fix:\n\
                 1. Implement MergeableState for your state type\n\
                 2. Use compile_with_merge() instead of compile()\n\n\
                 Example:\n\
                 impl MergeableState for YourState {\n\
                     fn merge(&mut self, other: &Self) {\n\
                         // Your merge logic\n\
                     }\n\
                 }\n\
                 let app = graph.compile_with_merge()?;\n\n\
                 See: https://github.com/dropbox/dTOOL/dashflow for documentation"
                    .to_string(),
            ));
        }

        // Advanced validation (default-enabled, opt-out via compile_without_validation)
        if advanced_validation {
            let mut validation_errors = Vec::new();

            // Check for unreachable nodes
            let reachable = self.find_reachable_nodes();
            for node_name in self.nodes.keys() {
                if !reachable.contains(node_name) {
                    validation_errors.push(format!(
                        "Node '{}' is unreachable from entry point '{}'",
                        node_name, entry_point
                    ));
                }
            }

            // Check conditional edges for empty routes (definite bug)
            for edge in &self.conditional_edges {
                if edge.routes.is_empty() {
                    validation_errors.push(format!(
                        "Conditional edge from '{}' has no routes defined",
                        edge.from
                    ));
                }
            }

            // Return validation errors if any found
            if !validation_errors.is_empty() {
                return Err(Error::Validation(format!(
                    "Graph validation failed:\n  - {}\n\nTo skip validation, use compile_without_validation().",
                    validation_errors.join("\n  - ")
                )));
            }

            // Check for cycles (warning only - cycles are allowed but risky)
            if self.has_cycles() {
                // Log warning but don't fail - cycles are valid in DashFlow
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    "Graph contains cycles. This is allowed but may cause infinite loops. \
                     Consider adding recursion limits using with_recursion_limit()."
                );
            }

            // Runtime Validation: Warn on non-optimizable LLM nodes
            #[cfg(feature = "tracing")]
            for (name, node) in &self.nodes {
                if node.may_use_llm() && !node.is_optimizable() {
                    tracing::warn!(
                        node_name = %name,
                        "Node '{}' uses LLM but is not optimizable. \
                         Consider using LLMNode or other DashOpt-aware nodes for production use. \
                         See: ROADMAP_CURRENT.md",
                        name
                    );
                }
            }
            #[cfg(not(feature = "tracing"))]
            let _ = &self.nodes; // Suppress unused warning when tracing is disabled

            // Runtime Validation: Warn on missing telemetry
            #[cfg(not(feature = "dashstream"))]
            {
                // Check if any node uses LLM (likely production graph)
                let has_llm_nodes = self.nodes.values().any(|n| n.may_use_llm());
                if has_llm_nodes {
                    #[cfg(feature = "tracing")]
                    tracing::warn!(
                        "Graph contains LLM nodes but 'dashstream' feature is disabled. \
                         Enable dashstream for production telemetry/observability. \
                         Add: dashflow = {{ features = [\"dashstream\"] }}"
                    );
                }
            }
        }

        #[cfg(not(kani))]
        let (nodes, node_metadata, node_configs) = (self.nodes, self.node_metadata, self.node_configs);
        #[cfg(kani)]
        let (nodes, node_metadata, node_configs) = (
            self.nodes.into_iter().collect::<HashMap<_, _>>(),
            self.node_metadata.into_iter().collect::<HashMap<_, _>>(),
            self.node_configs.into_iter().collect::<HashMap<_, _>>(),
        );

        Ok(CompiledGraph::new(
            nodes,
            self.edges,
            self.conditional_edges,
            self.parallel_edges,
            entry_point,
        )
        .with_node_metadata(node_metadata)
        .with_node_configs(node_configs))
    }

    // =========================================================================
    // Interpreter Mode (Fast Optimization Loops)
    // =========================================================================

    /// Execute the graph directly without a compile step (interpreter mode).
    ///
    /// This method allows rapid iteration during optimization loops by skipping
    /// the compilation phase entirely. The graph is interpreted directly, which
    /// means:
    ///
    /// - **No validation** - Graph structure is not validated before execution.
    ///   Runtime errors may occur if the graph is malformed.
    /// - **No freezing** - The graph structure is not frozen, allowing mutations
    ///   between executions without recompilation.
    /// - **Minimal overhead** - No intermediate `CompiledGraph` is created.
    ///
    /// # Warning
    ///
    /// This method skips all validation. Use only when:
    /// 1. You have already validated the graph structure separately
    /// 2. You are in a tight optimization loop where compile overhead matters
    /// 3. You accept that runtime errors may occur for invalid graphs
    ///
    /// For production use, prefer `compile()?.invoke()` which validates upfront.
    ///
    /// # Arguments
    ///
    /// * `state` - Initial state to execute the graph with
    ///
    /// # Returns
    ///
    /// `ExecutionResult<S>` containing the final state and execution metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No entry point is set
    /// - A referenced node does not exist
    /// - Node execution fails
    /// - Recursion limit exceeded (default: 25 steps)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::StateGraph;
    ///
    /// // Optimization loop pattern
    /// let mut graph = base_graph.clone();
    /// let mut best_result = None;
    ///
    /// for variant in variants {
    ///     graph.apply_mutation(&variant);
    ///
    ///     // Fast path: no compile step
    ///     let result = graph.execute_unvalidated(state.clone()).await?;
    ///
    ///     if result.score > best_result.score {
    ///         best_result = Some(result);
    ///         // Validate winning variant
    ///         graph.clone().compile()?;
    ///     }
    /// }
    /// ```
    pub async fn execute_unvalidated(
        &self,
        initial_state: S,
    ) -> Result<crate::executor::ExecutionResult<S>> {
        use std::time::Duration;

        // Basic validation: entry point must exist
        let entry_point = self.entry_point.as_ref().ok_or(Error::NoEntryPoint)?;

        if !self.nodes.contains_key(entry_point) {
            return Err(Error::NodeNotFound(entry_point.clone()));
        }

        // Interpret the graph directly
        let mut state = initial_state;
        let mut current_node = entry_point.clone();
        let mut nodes_executed = Vec::with_capacity(16);
        let recursion_limit: u32 = 25; // Default recursion limit

        for iteration in 0..=recursion_limit {
            if iteration == recursion_limit {
                return Err(Error::RecursionLimit {
                    limit: recursion_limit,
                });
            }

            // Check for END
            if current_node == END {
                return Ok(crate::executor::ExecutionResult {
                    final_state: state,
                    nodes_executed,
                    interrupted_at: None,
                    next_nodes: vec![],
                });
            }

            // Get the node
            let node = self
                .nodes
                .get(&current_node)
                .ok_or_else(|| Error::NodeNotFound(current_node.clone()))?;

            nodes_executed.push(current_node.clone());

            // Execute node (with default timeout of 5 minutes)
            let execution = node.execute(state);
            let node_timeout = Duration::from_secs(300);
            let new_state = match tokio::time::timeout(node_timeout, execution).await {
                Ok(result) => result.map_err(|e| Error::NodeExecution {
                    node: current_node.clone(),
                    source: Box::new(e),
                })?,
                Err(_) => return Err(Error::Timeout(node_timeout)),
            };
            state = new_state;

            // Find next node(s)
            let next = self.find_next_node_for_interpreter(&current_node, &state)?;
            match next {
                InterpreterNextNode::Single(next_node) => {
                    current_node = next_node;
                }
                InterpreterNextNode::Parallel(_targets) => {
                    // Parallel execution in interpreter mode is not supported
                    // Caller should use compile() for graphs with parallel edges
                    return Err(Error::Validation(
                        "execute_unvalidated() does not support parallel edges.\n\n\
                         To execute graphs with parallel edges:\n\
                         1. Use compile().invoke() or compile_with_merge().invoke()\n\
                         2. Or restructure your graph to avoid parallel edges\n\n\
                         Parallel edges require MergeableState and state merging logic."
                            .to_string(),
                    ));
                }
            }
        }

        // Should not reach here due to recursion limit check, but return proper error if it does
        Err(Error::InternalExecutionError(
            "Execution loop completed without reaching END or recursion limit".to_string(),
        ))
    }

    /// Helper enum for interpreter mode next node routing
    fn find_next_node_for_interpreter(
        &self,
        current: &str,
        state: &S,
    ) -> Result<InterpreterNextNode> {
        // Check conditional edges first (highest priority)
        for cond_edge in &self.conditional_edges {
            if cond_edge.from.as_str() == current {
                let next = cond_edge.evaluate(state);
                if let Some(target) = cond_edge.routes.get(&next) {
                    return Ok(InterpreterNextNode::Single(target.as_ref().clone()));
                }
                return Err(Error::InvalidEdge(format!(
                    "Conditional edge from '{}' returned '{}' but no route exists for it",
                    current, next
                )));
            }
        }

        // Check parallel edges (second priority)
        for edge in &self.parallel_edges {
            if edge.from.as_str() == current {
                return Ok(InterpreterNextNode::Parallel(edge.to.as_ref().clone()));
            }
        }

        // Check simple edges (third priority)
        for edge in &self.edges {
            if edge.from.as_str() == current {
                return Ok(InterpreterNextNode::Single(edge.to.as_ref().clone()));
            }
        }

        // No edge found - implicit end
        Ok(InterpreterNextNode::Single(END.to_string()))
    }

    /// Compile the graph with delta - only revalidate changed parts.
    ///
    /// This method enables incremental compilation by reusing validation results
    /// and structures from a previously compiled graph. This is useful when making
    /// small modifications to a graph and wanting to avoid full revalidation.
    ///
    /// # How It Works
    ///
    /// 1. Computes a structural hash of the current graph
    /// 2. Compares with the hash of the previous compiled graph
    /// 3. If unchanged, returns a clone of the previous compiled graph
    /// 4. If changed, performs full compilation (current implementation)
    ///
    /// Future versions may implement true delta compilation that:
    /// - Only validates newly added nodes/edges
    /// - Reuses validated structures from unchanged parts
    /// - Tracks dependencies to minimize revalidation
    ///
    /// # Arguments
    ///
    /// * `previous` - A reference to a previously compiled graph to compare against
    ///
    /// # Returns
    ///
    /// A new `CompiledGraph` that may share structures with `previous` if unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if compilation fails (same errors as `compile()`).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::StateGraph;
    ///
    /// // Initial compilation
    /// let mut graph = build_graph();
    /// let compiled = graph.clone().compile()?;
    ///
    /// // Make small modification
    /// graph.update_node_config("llm_agent", new_config)?;
    ///
    /// // Delta compilation - reuses validation if structure unchanged
    /// let recompiled = graph.compile_delta(&compiled)?;
    /// ```
    pub fn compile_delta(self, previous: &CompiledGraph<S>) -> Result<CompiledGraph<S>> {
        // Compute structural hash of current graph
        let current_hash = self.structural_hash();
        let previous_hash = previous.structural_hash();

        if current_hash == previous_hash {
            // Structure unchanged - skip validation since structure was already validated
            // The previous CompiledGraph validated this exact structure, so we can
            // safely skip re-validation
            self.compile_without_validation()
        } else {
            // Structure changed - full recompilation with validation
            // Future: implement true delta compilation that only
            // revalidates changed parts
            self.compile()
        }
    }

    /// Compute a structural hash of the graph for change detection.
    ///
    /// The hash is based on:
    /// - Node names (not node implementations)
    /// - Edge definitions (from/to)
    /// - Entry point
    ///
    /// Node configurations are NOT included in the hash since they
    /// can change without affecting graph structure.
    ///
    /// # Returns
    ///
    /// A 64-bit hash of the graph structure.
    pub fn structural_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash entry point
        self.entry_point.hash(&mut hasher);

        // Hash node names (sorted for determinism)
        let mut node_names: Vec<_> = self.nodes.keys().collect();
        node_names.sort();
        for name in node_names {
            name.hash(&mut hasher);
        }

        // Hash simple edges (sorted for determinism)
        let mut edge_strs: Vec<_> = self
            .edges
            .iter()
            .map(|e| format!("{}→{}", e.from, e.to))
            .collect();
        edge_strs.sort();
        for edge in edge_strs {
            edge.hash(&mut hasher);
        }

        // Hash conditional edges (source nodes only, sorted)
        let mut cond_strs: Vec<_> = self
            .conditional_edges
            .iter()
            .map(|e| {
                let mut routes: Vec<_> = e
                    .routes
                    .iter()
                    .map(|(k, v)| format!("{}:{}", k, v))
                    .collect();
                routes.sort();
                format!("{}?{}", e.from, routes.join(","))
            })
            .collect();
        cond_strs.sort();
        for cond in cond_strs {
            cond.hash(&mut hasher);
        }

        // Hash parallel edges (sorted)
        let mut par_strs: Vec<_> = self
            .parallel_edges
            .iter()
            .map(|e| {
                let mut targets: Vec<_> = e.to.to_vec();
                targets.sort();
                format!("{}||{}", e.from, targets.join(","))
            })
            .collect();
        par_strs.sort();
        for par in par_strs {
            par.hash(&mut hasher);
        }

        hasher.finish()
    }
}

/// Internal enum for interpreter mode next node routing
#[derive(Debug)]
enum InterpreterNextNode {
    /// Single next node
    Single(String),
    /// Parallel next nodes (not supported in interpreter mode)
    Parallel(Vec<String>),
}

impl<S> Default for StateGraph<S>
where
    S: crate::state::GraphState,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Specialized impl for `MergeableState` types (parallel edges support)
impl<S> StateGraph<S>
where
    S: crate::state::MergeableState,
{
    /// Compile the graph for execution with automatic parallel state merging (default validation)
    ///
    /// This method is available when S implements `MergeableState`.
    /// Use this for graphs that use `add_parallel_edges()`.
    ///
    /// During parallel execution, this ensures results from parallel branches are
    /// properly merged using your state's `merge()` implementation, preventing the
    /// 71% data loss bug (Gap #1) from last-write-wins semantics.
    ///
    /// # Validation (Default-Enabled)
    ///
    /// The following validations are performed automatically:
    /// - Entry point exists and is valid
    /// - All edges reference existing nodes
    /// - No mixed edge types on a single node
    /// - No unreachable nodes (nodes with no path from entry point)
    /// - Conditional edges have defined routes
    /// - Cycles are detected (warning only, cycles are allowed)
    ///
    /// To skip advanced validation, use [`Self::compile_with_merge_without_validation`].
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No entry point is set
    /// - Edges reference non-existent nodes
    /// - Graph structure is invalid
    /// - Unreachable nodes detected
    /// - Empty conditional routes detected
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::{StateGraph, MergeableState};
    ///
    /// #[derive(Clone, serde::Serialize, serde::Deserialize)]
    /// struct ResearchState {
    ///     findings: Vec<String>,
    ///     scores: Vec<i32>,
    /// }
    ///
    /// impl MergeableState for ResearchState {
    ///     fn merge(&mut self, other: &Self) {
    ///         self.findings.extend(other.findings.clone());
    ///         self.scores.extend(other.scores.clone());
    ///     }
    /// }
    ///
    /// let mut graph = StateGraph::new();
    /// graph.add_node("researcher1", research_fn1);
    /// graph.add_node("researcher2", research_fn2);
    /// graph.add_parallel_edges("start", vec!["researcher1", "researcher2"]);
    ///
    /// // Use compile_with_merge() for graphs with parallel edges
    /// let app = graph.compile_with_merge()?;
    /// ```
    pub fn compile_with_merge(self) -> Result<CompiledGraph<S>> {
        // Perform advanced validation by default (opt-out pattern)
        self.compile_internal(true, true)
    }

    /// Compile the graph with merge support but without advanced validation (opt-out)
    ///
    /// This skips the advanced validation checks (unreachable nodes,
    /// conditional route coverage, cycle detection) and only performs
    /// basic structural validation.
    ///
    /// **Use sparingly** - advanced validation helps catch issues early.
    /// This is useful for:
    /// - Dynamically generated graphs where unreachable nodes are expected
    /// - Performance-critical hot paths where validation overhead matters
    /// - Graphs where you've already manually validated structure
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Skip advanced validation for dynamic graphs with parallel edges
    /// let app = graph.compile_with_merge_without_validation()?;
    /// ```
    pub fn compile_with_merge_without_validation(self) -> Result<CompiledGraph<S>> {
        self.compile_internal(false, true)
    }
}

/// Fluent builder for constructing graphs (alias for [`StateGraph`])
///
/// `GraphBuilder` is a type alias for `StateGraph` that emphasizes the fluent builder pattern.
/// All methods return `&mut Self` for chaining, making graph construction concise and readable.
///
/// # Example: Fluent API
///
/// ```rust,ignore
/// use dashflow::{GraphBuilder, END};
///
/// let mut graph = GraphBuilder::new();
/// graph
///     .add_node("researcher", research_node)
///     .add_node("writer", writer_node)
///     .add_edge("researcher", "writer")
///     .add_edge("writer", END)
///     .set_entry_point("researcher");
///
/// let app = graph.compile()?;
/// ```
///
/// # Example: Conditional routing
///
/// ```rust,ignore
/// let mut graph = GraphBuilder::new();
/// graph
///     .add_node("start", start_node)
///     .add_node("path_a", path_a_node)
///     .add_node("path_b", path_b_node)
///     .add_conditional_edges(
///         "start",
///         |state: &AgentState| {
///             if state.should_continue {
///                 "continue".to_string()
///             } else {
///                 "end".to_string()
///             }
///         },
///         [
///             ("continue".to_string(), "path_a".to_string()),
///             ("end".to_string(), END.to_string()),
///         ].into_iter().collect(),
///     )
///     .add_edge("path_a", "path_b")
///     .add_edge("path_b", END)
///     .set_entry_point("start");
///
/// let app = graph.compile()?;
/// ```
///
/// # Example: Parallel execution
///
/// ```rust,ignore
/// let mut graph = GraphBuilder::new();
/// graph
///     .add_node("research", research_node)
///     .add_node("writer", writer_node)
///     .add_node("critic", critic_node)
///     .add_node("synthesize", synthesize_node)
///     .add_parallel_edges("research", vec!["writer".to_string(), "critic".to_string()])
///     .add_edge("writer", "synthesize")
///     .add_edge("critic", "synthesize")
///     .add_edge("synthesize", END)
///     .set_entry_point("research");
///
/// let app = graph.compile()?;
/// ```
///
/// # Benefits
///
/// - **Concise**: No intermediate variables needed
/// - **Readable**: Builder pattern flows naturally top-to-bottom
/// - **IDE-friendly**: Type inference works well, autocomplete shows available methods
/// - **Compile-time safety**: All validation happens at `compile()` time
///
/// # Relationship to `StateGraph`
///
/// `GraphBuilder<S>` is exactly the same as `StateGraph<S>` - they are type aliases.
/// Use whichever name is clearer in your context:
/// - `GraphBuilder` emphasizes the fluent builder pattern
/// - `StateGraph` emphasizes the graph structure and state type
pub type GraphBuilder<S> = StateGraph<S>;

impl<S> GraphBuilder<S>
where
    S: crate::state::MergeableState,
{
    /// Create a new `GraphBuilder` (same as [`StateGraph::new`])
    ///
    /// This is a convenience constructor that emphasizes the builder pattern.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::GraphBuilder;
    ///
    /// let graph = GraphBuilder::new()
    ///     .add_node("start", start_node)
    ///     .add_edge("start", END)
    ///     .set_entry_point("start")
    ///     .compile_with_merge()?;
    /// ```
    #[must_use]
    pub fn builder() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
