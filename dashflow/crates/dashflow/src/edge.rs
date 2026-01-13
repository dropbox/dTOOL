// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Edge types for graph connections.
//!
//! Edges define how nodes are connected in a [`StateGraph`](crate::StateGraph).
//! DashFlow supports three types of edges:
//!
//! - **[`Edge`]** - Simple unconditional transitions (A → B)
//! - **[`ConditionalEdge`]** - Routing based on state (A → B or C)
//! - **[`ParallelEdge`]** - Fan-out to multiple nodes (A → B + C + D)
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, edge::{Edge, END, START}};
//!
//! let mut graph = StateGraph::new();
//!
//! // Simple edges
//! graph.add_edge(START, "process");
//! graph.add_edge("process", END);
//!
//! // Conditional edge
//! graph.add_conditional_edge("router", |state: &MyState| {
//!     if state.needs_review { "review" } else { "complete" }.to_string()
//! }, vec!["review", "complete"]);
//!
//! // Parallel edges (fan-out)
//! graph.add_parallel_edges("start", vec!["researcher", "analyst"]);
//! ```
//!
//! # Constants
//!
//! - [`START`] - Entry point marker (`"__start__"`)
//! - [`END`] - Termination marker (`"__end__"`)

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

// Serde helpers for Arc<String> serialization
fn serialize_arc_string<S>(arc: &Arc<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(arc.as_str())
}

fn deserialize_arc_string<'de, D>(deserializer: D) -> Result<Arc<String>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(Arc::new)
}

// Serde helpers for Arc<Vec<String>> serialization
fn serialize_arc_vec_string<S>(arc: &Arc<Vec<String>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    arc.as_ref().serialize(serializer)
}

fn deserialize_arc_vec_string<'de, D>(deserializer: D) -> Result<Arc<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer).map(Arc::new)
}

/// Simple unconditional edge connecting two nodes.
///
/// An `Edge` represents a direct transition from one node to another with no
/// conditions or branching. When the source node completes, execution always
/// proceeds to the destination node.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::edge::{Edge, END};
///
/// // Create edge directly (usually done via StateGraph::add_edge)
/// let edge = Edge::new("process_data", "format_output");
///
/// // Use with StateGraph
/// let mut graph = StateGraph::new();
/// graph.add_edge("start", "process_data");
/// graph.add_edge("process_data", "format_output");
/// graph.add_edge("format_output", END);
/// ```
///
/// # See Also
///
/// - [`ConditionalEdge`] - For state-based routing
/// - [`ParallelEdge`] - For fan-out to multiple nodes
/// - [`StateGraph::add_edge`](crate::StateGraph::add_edge) - Preferred way to add edges
/// - [`END`] / [`START`] - Special node markers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Source node name (where the edge originates)
    #[serde(
        serialize_with = "serialize_arc_string",
        deserialize_with = "deserialize_arc_string"
    )]
    pub from: Arc<String>,
    /// Destination node name (where execution continues)
    #[serde(
        serialize_with = "serialize_arc_string",
        deserialize_with = "deserialize_arc_string"
    )]
    pub to: Arc<String>,
}

impl Edge {
    /// Create a new edge
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: Arc::new(from.into()),
            to: Arc::new(to.into()),
        }
    }
}

/// Conditional edge that routes based on state.
///
/// A `ConditionalEdge` examines the current state and determines which node
/// to execute next. This enables dynamic routing, branching logic, and
/// implementing patterns like loops, retries, and agent supervisors.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::{StateGraph, edge::END};
///
/// #[derive(Clone, serde::Serialize, serde::Deserialize)]
/// struct WorkflowState {
///     needs_approval: bool,
///     quality_score: f32,
/// }
///
/// let mut graph = StateGraph::new();
///
/// // Add conditional routing based on state
/// graph.add_conditional_edge(
///     "evaluate",
///     |state: &WorkflowState| {
///         if state.quality_score < 0.7 {
///             "retry".to_string()      // Loop back for retry
///         } else if state.needs_approval {
///             "human_review".to_string() // Route to human
///         } else {
///             "__end__".to_string()    // Complete
///         }
///     },
///     vec!["retry", "human_review", "__end__"],
/// );
/// ```
///
/// # Routes Validation
///
/// The `routes` parameter declares all possible destinations. DashFlow validates
/// that the condition function only returns known routes, catching routing errors
/// at compile time.
///
/// # See Also
///
/// - [`Edge`] - For unconditional transitions
/// - [`ParallelEdge`] - For fan-out execution
/// - [`StateGraph::add_conditional_edge`](crate::StateGraph::add_conditional_edge) - Preferred way to add
pub struct ConditionalEdge<S>
where
    S: Send + Sync,
{
    /// Source node name (where this edge originates)
    pub from: Arc<String>,
    /// Condition function that determines the next node based on state
    pub condition: Arc<dyn Fn(&S) -> String + Send + Sync>,
    /// Map of possible destination nodes (for validation)
    /// Values are Arc-wrapped to avoid allocation in the hot path
    pub routes: HashMap<String, Arc<String>>,
}

impl<S> ConditionalEdge<S>
where
    S: Send + Sync,
{
    /// Create a new conditional edge
    ///
    /// # Arguments
    ///
    /// * `from` - Source node name
    /// * `condition` - Function that returns next node name based on state
    /// * `routes` - Map of possible next nodes (for validation)
    pub fn new<F>(from: impl Into<String>, condition: F, routes: HashMap<String, String>) -> Self
    where
        F: Fn(&S) -> String + Send + Sync + 'static,
    {
        // Wrap route values in Arc at construction time to avoid allocation in hot path
        let routes = routes.into_iter().map(|(k, v)| (k, Arc::new(v))).collect();
        Self {
            from: Arc::new(from.into()),
            condition: Arc::new(condition),
            routes,
        }
    }

    /// Evaluate the condition and determine next node
    pub fn evaluate(&self, state: &S) -> String {
        (self.condition)(state)
    }
}

impl<S> fmt::Debug for ConditionalEdge<S>
where
    S: Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConditionalEdge")
            .field("from", &self.from)
            .field("routes", &self.routes)
            .finish()
    }
}

/// Parallel edge that fans out to multiple nodes.
///
/// A `ParallelEdge` sends state to multiple nodes that execute **concurrently**.
/// This enables patterns like multi-agent collaboration, parallel search, and
/// map-reduce workflows.
///
/// # State Merging
///
/// After all parallel nodes complete, their results must be merged. Use
/// [`MergeableState`](crate::state::MergeableState) to define how state from
/// parallel branches combines (e.g., extend lists, take max values).
///
/// Without `MergeableState`, the default "last-write-wins" behavior may
/// cause data loss from earlier parallel branches.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::{StateGraph, state::MergeableState};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Clone, Serialize, Deserialize)]
/// struct ResearchState {
///     findings: Vec<String>,
/// }
///
/// impl MergeableState for ResearchState {
///     fn merge(&mut self, other: &Self) {
///         self.findings.extend(other.findings.clone());
///     }
/// }
///
/// let mut graph = StateGraph::new();
/// graph.add_node("researcher", research_node);
/// graph.add_node("analyst", analyst_node);
/// graph.add_node("aggregator", aggregate_node);
///
/// // Fan-out: both researcher and analyst run in parallel
/// graph.add_parallel_edges("start", vec!["researcher", "analyst"]);
///
/// // Both results flow to aggregator (merged via MergeableState)
/// graph.add_edge("researcher", "aggregator");
/// graph.add_edge("analyst", "aggregator");
/// ```
///
/// # See Also
///
/// - [`Edge`] - For unconditional single-target transitions
/// - [`ConditionalEdge`] - For state-based routing
/// - [`MergeableState`](crate::state::MergeableState) - For defining merge behavior
/// - [`StateGraph::add_parallel_edges`](crate::StateGraph::add_parallel_edges) - Preferred way to add
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelEdge {
    /// Source node name (where execution fans out from)
    #[serde(
        serialize_with = "serialize_arc_string",
        deserialize_with = "deserialize_arc_string"
    )]
    pub from: Arc<String>,
    /// Target node names (all execute concurrently)
    #[serde(
        serialize_with = "serialize_arc_vec_string",
        deserialize_with = "deserialize_arc_vec_string"
    )]
    pub to: Arc<Vec<String>>,
}

impl ParallelEdge {
    /// Create a new parallel edge
    pub fn new(from: impl Into<String>, to: Vec<String>) -> Self {
        Self {
            from: Arc::new(from.into()),
            to: Arc::new(to),
        }
    }
}

/// Type of edge in the graph
#[derive(Debug, Clone)]
pub enum EdgeType<S>
where
    S: Send + Sync,
{
    /// Simple unconditional edge
    Simple(Edge),
    /// Conditional edge with routing logic
    Conditional(Arc<ConditionalEdge<S>>),
    /// Parallel edge that fans out to multiple nodes
    Parallel(ParallelEdge),
}

/// Special end marker that terminates graph execution.
///
/// Use `END` (or `"__end__"`) as the destination of edges that should
/// terminate the graph. When a node transitions to `END`, execution stops
/// and the final state is returned.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::{StateGraph, edge::END};
///
/// let mut graph = StateGraph::new();
/// graph.add_edge("final_step", END);  // Graph terminates after final_step
/// ```
pub const END: &str = "__end__";

/// Special start marker for graph entry point.
///
/// Use `START` (or `"__start__"`) as the source of edges from the entry point.
/// The `START` marker represents the initial state before any nodes execute.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::{StateGraph, edge::START};
///
/// let mut graph = StateGraph::new();
/// graph.add_edge(START, "first_step");  // Execution begins at first_step
/// ```
///
/// # Note
///
/// Using `graph.set_entry_point("node")` is equivalent to
/// `graph.add_edge(START, "node")`.
pub const START: &str = "__start__";

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestState {
        next: String,
    }

    #[test]
    fn test_simple_edge() {
        let edge = Edge::new("node1", "node2");
        assert_eq!(edge.from.as_str(), "node1");
        assert_eq!(edge.to.as_str(), "node2");
    }

    #[test]
    fn test_conditional_edge() {
        let mut routes = HashMap::new();
        routes.insert("success".to_string(), "node2".to_string());
        routes.insert("failure".to_string(), "node3".to_string());

        let edge = ConditionalEdge::new("node1", |state: &TestState| state.next.clone(), routes);

        let state = TestState {
            next: "success".to_string(),
        };
        assert_eq!(edge.evaluate(&state), "success");

        let state = TestState {
            next: "failure".to_string(),
        };
        assert_eq!(edge.evaluate(&state), "failure");
    }

    #[test]
    fn test_end_marker() {
        assert_eq!(END, "__end__");
    }

    #[test]
    fn test_parallel_edge() {
        let edge = ParallelEdge::new("node1", vec!["node2".to_string(), "node3".to_string()]);
        assert_eq!(edge.from.as_str(), "node1");
        assert_eq!(edge.to.len(), 2);
        assert!(edge.to.contains(&"node2".to_string()));
        assert!(edge.to.contains(&"node3".to_string()));
    }

    #[test]
    fn test_edge_serialization() {
        // Test Edge serialization/deserialization (exercises Arc<String> helpers)
        let edge = Edge::new("start", "end");
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("start"));
        assert!(json.contains("end"));

        let deserialized: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.from.as_str(), "start");
        assert_eq!(deserialized.to.as_str(), "end");
    }

    #[test]
    fn test_parallel_edge_serialization() {
        // Test ParallelEdge serialization/deserialization (exercises Arc<Vec<String>> helpers)
        let edge = ParallelEdge::new(
            "parallel_start",
            vec!["branch1".to_string(), "branch2".to_string()],
        );
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("parallel_start"));
        assert!(json.contains("branch1"));
        assert!(json.contains("branch2"));

        let deserialized: ParallelEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.from.as_str(), "parallel_start");
        assert_eq!(deserialized.to.len(), 2);
    }

    #[test]
    fn test_conditional_edge_debug_format() {
        // Test ConditionalEdge Debug trait implementation
        let mut routes = HashMap::new();
        routes.insert("route1".to_string(), "node1".to_string());
        routes.insert("route2".to_string(), "node2".to_string());

        let edge = ConditionalEdge::new("start", |state: &TestState| state.next.clone(), routes);

        let debug_output = format!("{:?}", edge);
        assert!(debug_output.contains("ConditionalEdge"));
        assert!(debug_output.contains("start"));
        assert!(debug_output.contains("route1") || debug_output.contains("route2"));
    }

    #[test]
    fn test_edge_type_variants() {
        // Test EdgeType enum variants
        let simple_edge = EdgeType::<TestState>::Simple(Edge::new("a", "b"));
        assert!(matches!(simple_edge, EdgeType::Simple(_)));

        let mut routes = HashMap::new();
        routes.insert("next".to_string(), "node".to_string());
        let conditional_edge = EdgeType::<TestState>::Conditional(Arc::new(ConditionalEdge::new(
            "c",
            |state: &TestState| state.next.clone(),
            routes,
        )));
        assert!(matches!(conditional_edge, EdgeType::Conditional(_)));

        let parallel_edge =
            EdgeType::<TestState>::Parallel(ParallelEdge::new("d", vec!["e".to_string()]));
        assert!(matches!(parallel_edge, EdgeType::Parallel(_)));
    }

    #[test]
    fn test_start_marker() {
        // Test START constant
        assert_eq!(START, "__start__");
    }

    #[test]
    fn test_edge_clone() {
        // Test Edge Clone trait
        let edge1 = Edge::new("node1", "node2");
        let edge2 = edge1.clone();
        assert_eq!(edge1.from.as_str(), edge2.from.as_str());
        assert_eq!(edge1.to.as_str(), edge2.to.as_str());
    }

    #[test]
    fn test_parallel_edge_clone() {
        // Test ParallelEdge Clone trait
        let edge1 = ParallelEdge::new("start", vec!["a".to_string(), "b".to_string()]);
        let edge2 = edge1.clone();
        assert_eq!(edge1.from.as_str(), edge2.from.as_str());
        assert_eq!(edge1.to.len(), edge2.to.len());
    }

    #[test]
    fn test_conditional_edge_routes() {
        // Test that routes are stored correctly in ConditionalEdge
        let mut routes = HashMap::new();
        routes.insert("path_a".to_string(), "node_a".to_string());
        routes.insert("path_b".to_string(), "node_b".to_string());

        let edge = ConditionalEdge::new(
            "start",
            |state: &TestState| state.next.clone(),
            routes.clone(),
        );

        assert_eq!(edge.routes.len(), 2);
        assert_eq!(edge.routes.get("path_a").unwrap().as_str(), "node_a");
        assert_eq!(edge.routes.get("path_b").unwrap().as_str(), "node_b");
    }

    #[test]
    fn test_parallel_edge_empty() {
        // Test ParallelEdge with empty targets
        let edge = ParallelEdge::new("start", vec![]);
        assert_eq!(edge.from.as_str(), "start");
        assert_eq!(edge.to.len(), 0);
        assert!(edge.to.is_empty());
    }

    #[test]
    fn test_parallel_edge_single_target() {
        // Test ParallelEdge with single target (degenerate case)
        let edge = ParallelEdge::new("start", vec!["single".to_string()]);
        assert_eq!(edge.from.as_str(), "start");
        assert_eq!(edge.to.len(), 1);
        assert_eq!(edge.to[0], "single");
    }

    #[test]
    fn test_parallel_edge_many_targets() {
        // Test ParallelEdge with many targets (10 branches)
        let targets: Vec<String> = (0..10).map(|i| format!("node_{}", i)).collect();
        let edge = ParallelEdge::new("start", targets);
        assert_eq!(edge.from.as_str(), "start");
        assert_eq!(edge.to.len(), 10);
        for i in 0..10 {
            assert!(edge.to.contains(&format!("node_{}", i)));
        }
    }

    #[test]
    fn test_parallel_edge_duplicate_targets() {
        // Test ParallelEdge with duplicate targets (allowed but unusual)
        let edge = ParallelEdge::new("start", vec!["node".to_string(), "node".to_string()]);
        assert_eq!(edge.from.as_str(), "start");
        assert_eq!(edge.to.len(), 2);
        assert_eq!(edge.to[0], "node");
        assert_eq!(edge.to[1], "node");
    }

    #[test]
    fn test_parallel_edge_ordering_preserved() {
        // Test that ParallelEdge preserves target order
        let targets = vec![
            "z_node".to_string(),
            "a_node".to_string(),
            "m_node".to_string(),
        ];
        let edge = ParallelEdge::new("start", targets);
        assert_eq!(edge.to.len(), 3);
        assert_eq!(edge.to[0], "z_node");
        assert_eq!(edge.to[1], "a_node");
        assert_eq!(edge.to[2], "m_node");
    }

    #[test]
    fn test_conditional_edge_empty_routes() {
        // Test ConditionalEdge with empty routes map
        let routes = HashMap::new();
        let edge = ConditionalEdge::new("start", |state: &TestState| state.next.clone(), routes);
        assert_eq!(edge.from.as_str(), "start");
        assert_eq!(edge.routes.len(), 0);
        assert!(edge.routes.is_empty());
    }

    #[test]
    fn test_conditional_edge_complex_logic() {
        // Test ConditionalEdge with complex state-based routing
        #[derive(Clone)]
        struct ComplexState {
            score: i32,
            /// Test infrastructure field: Part of test struct for edge routing.
            ///
            /// Test-only field: Used in test_conditional_edge_complex_logic to verify
            /// ConditionalEdge can handle structs with multiple fields.
            /// Only score field is accessed in routing logic (line 414-420).
            /// Category field exists to ensure edge works with realistic multi-field state.
            /// Cannot remove without reducing test coverage of state handling.
            #[allow(dead_code)] // Test: Required for multi-field state test coverage
            category: String,
        }

        let mut routes = HashMap::new();
        routes.insert("high".to_string(), "premium_node".to_string());
        routes.insert("medium".to_string(), "standard_node".to_string());
        routes.insert("low".to_string(), "basic_node".to_string());

        let edge = ConditionalEdge::new(
            "evaluator",
            |state: &ComplexState| {
                if state.score > 80 {
                    "high".to_string()
                } else if state.score > 50 {
                    "medium".to_string()
                } else {
                    "low".to_string()
                }
            },
            routes,
        );

        let state_high = ComplexState {
            score: 90,
            category: "vip".to_string(),
        };
        assert_eq!(edge.evaluate(&state_high), "high");

        let state_medium = ComplexState {
            score: 60,
            category: "standard".to_string(),
        };
        assert_eq!(edge.evaluate(&state_medium), "medium");

        let state_low = ComplexState {
            score: 30,
            category: "basic".to_string(),
        };
        assert_eq!(edge.evaluate(&state_low), "low");
    }

    #[test]
    fn test_edge_with_empty_names() {
        // Test Edge with empty string names (unusual but allowed)
        let edge = Edge::new("", "");
        assert_eq!(edge.from.as_str(), "");
        assert_eq!(edge.to.as_str(), "");
    }

    #[test]
    fn test_edge_with_special_characters() {
        // Test Edge with special characters in names
        let edge = Edge::new("node-with-hyphens", "node.with.dots");
        assert_eq!(edge.from.as_str(), "node-with-hyphens");
        assert_eq!(edge.to.as_str(), "node.with.dots");

        let edge2 = Edge::new("node:with:colons", "node/with/slashes");
        assert_eq!(edge2.from.as_str(), "node:with:colons");
        assert_eq!(edge2.to.as_str(), "node/with/slashes");

        let edge3 = Edge::new("node_with_underscores", "node@with@at");
        assert_eq!(edge3.from.as_str(), "node_with_underscores");
        assert_eq!(edge3.to.as_str(), "node@with@at");
    }

    #[test]
    fn test_parallel_edge_special_characters() {
        // Test ParallelEdge targets with special characters
        let edge = ParallelEdge::new(
            "start",
            vec![
                "node-1".to_string(),
                "node.2".to_string(),
                "node:3".to_string(),
                "node/4".to_string(),
                "node_5".to_string(),
                "node@6".to_string(),
            ],
        );
        assert_eq!(edge.to.len(), 6);
        assert!(edge.to.contains(&"node-1".to_string()));
        assert!(edge.to.contains(&"node.2".to_string()));
        assert!(edge.to.contains(&"node:3".to_string()));
        assert!(edge.to.contains(&"node/4".to_string()));
        assert!(edge.to.contains(&"node_5".to_string()));
        assert!(edge.to.contains(&"node@6".to_string()));
    }

    #[test]
    fn test_edge_arc_sharing() {
        // Test that Edge uses Arc for memory efficiency
        let edge1 = Edge::new("shared_node", "target");
        let edge2 = edge1.clone();

        // Arc sharing means same pointer
        assert!(Arc::ptr_eq(&edge1.from, &edge2.from));
        assert!(Arc::ptr_eq(&edge1.to, &edge2.to));

        // But values are the same
        assert_eq!(edge1.from.as_str(), edge2.from.as_str());
        assert_eq!(edge1.to.as_str(), edge2.to.as_str());
    }

    #[test]
    fn test_parallel_edge_arc_sharing() {
        // Test that ParallelEdge uses Arc for memory efficiency
        let edge1 = ParallelEdge::new("start", vec!["a".to_string(), "b".to_string()]);
        let edge2 = edge1.clone();

        // Arc sharing means same pointer for both from and to
        assert!(Arc::ptr_eq(&edge1.from, &edge2.from));
        assert!(Arc::ptr_eq(&edge1.to, &edge2.to));

        // But values are the same
        assert_eq!(edge1.from.as_str(), edge2.from.as_str());
        assert_eq!(edge1.to.len(), edge2.to.len());
    }

    #[test]
    fn test_conditional_edge_arc_sharing() {
        // Test that ConditionalEdge uses Arc for condition function
        let mut routes = HashMap::new();
        routes.insert("next".to_string(), "node".to_string());

        let edge1 = ConditionalEdge::new("start", |state: &TestState| state.next.clone(), routes);

        // Arc is used internally for from
        assert_eq!(edge1.from.as_str(), "start");

        // Condition is also Arc-wrapped (can't test pointer equality but can verify it works)
        let state = TestState {
            next: "next".to_string(),
        };
        assert_eq!(edge1.evaluate(&state), "next");
    }

    #[test]
    fn test_conditional_edge_multiple_evaluations() {
        // Test that ConditionalEdge can be evaluated multiple times
        let mut routes = HashMap::new();
        routes.insert("path1".to_string(), "node1".to_string());
        routes.insert("path2".to_string(), "node2".to_string());

        let edge = ConditionalEdge::new("start", |state: &TestState| state.next.clone(), routes);

        // Multiple evaluations with different states
        let state1 = TestState {
            next: "path1".to_string(),
        };
        let state2 = TestState {
            next: "path2".to_string(),
        };
        let state3 = TestState {
            next: "path1".to_string(),
        };

        assert_eq!(edge.evaluate(&state1), "path1");
        assert_eq!(edge.evaluate(&state2), "path2");
        assert_eq!(edge.evaluate(&state3), "path1");
    }

    #[test]
    fn test_edge_type_simple_variant() {
        // Test EdgeType::Simple variant in detail
        let edge = Edge::new("from_node", "to_node");
        let edge_type = EdgeType::<TestState>::Simple(edge);

        match edge_type {
            EdgeType::Simple(e) => {
                assert_eq!(e.from.as_str(), "from_node");
                assert_eq!(e.to.as_str(), "to_node");
            }
            _ => panic!("Expected Simple variant"),
        }
    }

    #[test]
    fn test_edge_type_conditional_variant() {
        // Test EdgeType::Conditional variant in detail
        let mut routes = HashMap::new();
        routes.insert("route".to_string(), "target".to_string());

        let conditional =
            ConditionalEdge::new("start", |state: &TestState| state.next.clone(), routes);
        let edge_type = EdgeType::<TestState>::Conditional(Arc::new(conditional));

        match edge_type {
            EdgeType::Conditional(e) => {
                assert_eq!(e.from.as_str(), "start");
                assert_eq!(e.routes.len(), 1);

                let state = TestState {
                    next: "route".to_string(),
                };
                assert_eq!(e.evaluate(&state), "route");
            }
            _ => panic!("Expected Conditional variant"),
        }
    }

    #[test]
    fn test_edge_type_parallel_variant() {
        // Test EdgeType::Parallel variant in detail
        let parallel = ParallelEdge::new("start", vec!["a".to_string(), "b".to_string()]);
        let edge_type = EdgeType::<TestState>::Parallel(parallel);

        match edge_type {
            EdgeType::Parallel(e) => {
                assert_eq!(e.from.as_str(), "start");
                assert_eq!(e.to.len(), 2);
                assert!(e.to.contains(&"a".to_string()));
                assert!(e.to.contains(&"b".to_string()));
            }
            _ => panic!("Expected Parallel variant"),
        }
    }

    #[test]
    fn test_edge_type_clone() {
        // Test that EdgeType can be cloned (for Simple and Parallel variants)
        let simple = EdgeType::<TestState>::Simple(Edge::new("a", "b"));
        let simple_clone = simple.clone();

        match (simple, simple_clone) {
            (EdgeType::Simple(e1), EdgeType::Simple(e2)) => {
                assert_eq!(e1.from.as_str(), e2.from.as_str());
                assert_eq!(e1.to.as_str(), e2.to.as_str());
            }
            _ => panic!("Expected Simple variants"),
        }

        let parallel = EdgeType::<TestState>::Parallel(ParallelEdge::new(
            "x",
            vec!["y".to_string(), "z".to_string()],
        ));
        let parallel_clone = parallel.clone();

        match (parallel, parallel_clone) {
            (EdgeType::Parallel(e1), EdgeType::Parallel(e2)) => {
                assert_eq!(e1.from.as_str(), e2.from.as_str());
                assert_eq!(e1.to.len(), e2.to.len());
            }
            _ => panic!("Expected Parallel variants"),
        }
    }

    #[test]
    fn test_conditional_edge_with_stateful_logic() {
        // Test ConditionalEdge with state-dependent branching logic
        #[derive(Clone)]
        struct CounterState {
            count: i32,
        }

        let mut routes = HashMap::new();
        routes.insert("even".to_string(), "even_handler".to_string());
        routes.insert("odd".to_string(), "odd_handler".to_string());

        let edge = ConditionalEdge::new(
            "counter",
            |state: &CounterState| {
                if state.count % 2 == 0 {
                    "even".to_string()
                } else {
                    "odd".to_string()
                }
            },
            routes,
        );

        let even_state = CounterState { count: 10 };
        assert_eq!(edge.evaluate(&even_state), "even");

        let odd_state = CounterState { count: 7 };
        assert_eq!(edge.evaluate(&odd_state), "odd");

        let zero_state = CounterState { count: 0 };
        assert_eq!(edge.evaluate(&zero_state), "even");

        let negative_even = CounterState { count: -4 };
        assert_eq!(edge.evaluate(&negative_even), "even");

        let negative_odd = CounterState { count: -3 };
        assert_eq!(edge.evaluate(&negative_odd), "odd");
    }

    #[test]
    fn test_edge_serialization_roundtrip() {
        // Test that Edge can be serialized and deserialized without data loss
        let original = Edge::new("source_node", "destination_node");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Edge = serde_json::from_str(&json).unwrap();

        assert_eq!(original.from.as_str(), deserialized.from.as_str());
        assert_eq!(original.to.as_str(), deserialized.to.as_str());
    }

    #[test]
    fn test_parallel_edge_serialization_roundtrip() {
        // Test that ParallelEdge can be serialized and deserialized without data loss
        let original = ParallelEdge::new(
            "parallel_source",
            vec![
                "target1".to_string(),
                "target2".to_string(),
                "target3".to_string(),
            ],
        );
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ParallelEdge = serde_json::from_str(&json).unwrap();

        assert_eq!(original.from.as_str(), deserialized.from.as_str());
        assert_eq!(original.to.len(), deserialized.to.len());
        for i in 0..original.to.len() {
            assert_eq!(original.to[i], deserialized.to[i]);
        }
    }

    #[test]
    fn test_edge_with_unicode_names() {
        // Test Edge with Unicode characters in names
        let edge = Edge::new("节点一", "节点二");
        assert_eq!(edge.from.as_str(), "节点一");
        assert_eq!(edge.to.as_str(), "节点二");

        let edge2 = Edge::new("нода", "узел");
        assert_eq!(edge2.from.as_str(), "нода");
        assert_eq!(edge2.to.as_str(), "узел");

        let edge3 = Edge::new("ノード", "エッジ");
        assert_eq!(edge3.from.as_str(), "ノード");
        assert_eq!(edge3.to.as_str(), "エッジ");
    }

    #[test]
    fn test_parallel_edge_with_unicode_targets() {
        // Test ParallelEdge with Unicode target names
        let edge = ParallelEdge::new(
            "start",
            vec![
                "节点A".to_string(),
                "нода_B".to_string(),
                "ノードC".to_string(),
            ],
        );
        assert_eq!(edge.to.len(), 3);
        assert!(edge.to.contains(&"节点A".to_string()));
        assert!(edge.to.contains(&"нода_B".to_string()));
        assert!(edge.to.contains(&"ノードC".to_string()));
    }

    #[test]
    fn test_edge_with_very_long_names() {
        // Test Edge with very long node names (1000 characters)
        let long_name = "a".repeat(1000);
        let edge = Edge::new(long_name.clone(), long_name);
        assert_eq!(edge.from.as_str().len(), 1000);
        assert_eq!(edge.to.as_str().len(), 1000);
    }

    #[test]
    fn test_parallel_edge_with_many_unique_targets() {
        // Test ParallelEdge with 100 unique targets
        let targets: Vec<String> = (0..100).map(|i| format!("target_{:03}", i)).collect();
        let edge = ParallelEdge::new("massive_fanout", targets);
        assert_eq!(edge.from.as_str(), "massive_fanout");
        assert_eq!(edge.to.len(), 100);

        // Verify all targets are present
        for i in 0..100 {
            assert!(edge.to.contains(&format!("target_{:03}", i)));
        }
    }

    #[test]
    fn test_conditional_edge_with_large_routes_map() {
        // Test ConditionalEdge with large routes map (100 routes)
        let mut routes = HashMap::new();
        for i in 0..100 {
            routes.insert(format!("route_{}", i), format!("node_{}", i));
        }

        let edge = ConditionalEdge::new(
            "router",
            |state: &TestState| state.next.clone(),
            routes.clone(),
        );

        assert_eq!(edge.routes.len(), 100);

        // Test evaluation with one of the routes
        let state = TestState {
            next: "route_42".to_string(),
        };
        assert_eq!(edge.evaluate(&state), "route_42");
    }
}
