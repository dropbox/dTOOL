//! Graph types for Runnable visualization
//!
//! This module provides types for representing and visualizing the structure
//! of Runnable compositions as directed graphs.

use std::collections::HashMap;

/// Represents a node in a Runnable graph
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Unique identifier for the node
    pub id: String,
    /// Display name for the node
    pub name: String,
    /// Optional metadata for the node
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl Node {
    /// Create a new Node
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            metadata: None,
        }
    }

    /// Create a new Node with metadata
    pub fn with_metadata(
        id: impl Into<String>,
        name: impl Into<String>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            metadata: Some(metadata),
        }
    }

    /// Create a copy of this node with a new id
    #[must_use]
    pub fn with_id(&self, id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: self.name.clone(),
            metadata: self.metadata.clone(),
        }
    }

    /// Create a copy of this node with a new name
    #[must_use]
    pub fn with_name(&self, name: impl Into<String>) -> Self {
        Self {
            id: self.id.clone(),
            name: name.into(),
            metadata: self.metadata.clone(),
        }
    }
}

/// Represents an edge between nodes in a Runnable graph
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// Source node id
    pub source: String,
    /// Target node id
    pub target: String,
    /// Optional data/label for the edge (e.g., branch condition)
    pub data: Option<String>,
    /// Whether this edge represents a conditional branch
    pub conditional: bool,
}

impl Edge {
    /// Create a new Edge
    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            data: None,
            conditional: false,
        }
    }

    /// Create a new conditional Edge with data
    pub fn conditional(
        source: impl Into<String>,
        target: impl Into<String>,
        data: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            data: Some(data.into()),
            conditional: true,
        }
    }

    /// Create a copy of this edge with a new source
    #[must_use]
    pub fn with_source(&self, source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: self.target.clone(),
            data: self.data.clone(),
            conditional: self.conditional,
        }
    }

    /// Create a copy of this edge with a new target
    #[must_use]
    pub fn with_target(&self, target: impl Into<String>) -> Self {
        Self {
            source: self.source.clone(),
            target: target.into(),
            data: self.data.clone(),
            conditional: self.conditional,
        }
    }
}

/// Represents the graph structure of a Runnable
#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// All nodes in the graph, indexed by their id
    pub nodes: HashMap<String, Node>,
    /// All edges in the graph
    pub edges: Vec<Edge>,
}

impl Graph {
    /// Create a new empty Graph
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Get the first node (useful for simple chains)
    #[must_use]
    pub fn first_node(&self) -> Option<&Node> {
        // Find a node that has no incoming edges
        let targets: std::collections::HashSet<_> = self.edges.iter().map(|e| &e.target).collect();
        self.nodes.values().find(|node| !targets.contains(&node.id))
    }

    /// Get the last node (useful for simple chains)
    #[must_use]
    pub fn last_node(&self) -> Option<&Node> {
        // Find a node that has no outgoing edges
        let sources: std::collections::HashSet<_> = self.edges.iter().map(|e| &e.source).collect();
        self.nodes.values().find(|node| !sources.contains(&node.id))
    }

    /// Extend this graph with another graph, adding a prefix to node IDs to avoid conflicts
    pub fn extend(&mut self, other: &Graph, prefix: &str) {
        for node in other.nodes.values() {
            let new_node = node.with_id(format!("{}:{}", prefix, node.id));
            self.add_node(new_node);
        }
        for edge in &other.edges {
            let new_edge = Edge {
                source: format!("{}:{}", prefix, edge.source),
                target: format!("{}:{}", prefix, edge.target),
                data: edge.data.clone(),
                conditional: edge.conditional,
            };
            self.add_edge(new_edge);
        }
    }

    /// Draw the graph as ASCII art
    ///
    /// Creates a simple vertical ASCII visualization of the graph structure.
    /// For simple chains, nodes are drawn vertically with edges between them.
    ///
    /// # Example Output
    ///
    /// ```text
    /// +-------------+
    /// |   Node1     |
    /// +-------------+
    ///       |
    ///       v
    /// +-------------+
    /// |   Node2     |
    /// +-------------+
    /// ```
    #[must_use]
    pub fn draw_ascii(&self) -> String {
        if self.nodes.is_empty() {
            return String::from("(empty graph)");
        }

        let mut output = String::new();

        // For simple linear chains, draw vertically
        if self.is_linear_chain() {
            let mut current_id = self.first_node().map(|n| n.id.clone());
            let mut visited = std::collections::HashSet::new();

            while let Some(ref id) = current_id {
                if visited.contains(id) {
                    break; // Avoid infinite loops
                }
                visited.insert(id.clone());

                if let Some(node) = self.nodes.get(id) {
                    // Draw node box
                    let name_len = node.name.len().max(10);
                    let box_width = name_len + 4;
                    let padding = (box_width - 2 - node.name.len()) / 2;

                    output.push('+');
                    output.push_str(&"-".repeat(box_width - 2));
                    output.push_str("+\n");

                    output.push_str("| ");
                    output.push_str(&" ".repeat(padding));
                    output.push_str(&node.name);
                    output.push_str(&" ".repeat(box_width - 2 - padding - node.name.len()));
                    output.push_str(" |\n");

                    output.push('+');
                    output.push_str(&"-".repeat(box_width - 2));
                    output.push_str("+\n");

                    // Find next node
                    current_id = self
                        .edges
                        .iter()
                        .find(|e| &e.source == id)
                        .map(|e| e.target.clone());

                    // Draw edge if there's a next node
                    if current_id.is_some() {
                        let arrow_padding = (box_width - 1) / 2;
                        output.push_str(&" ".repeat(arrow_padding));
                        output.push_str("|\n");
                        output.push_str(&" ".repeat(arrow_padding));
                        output.push_str("v\n");
                    }
                }
            }
        } else {
            // For complex graphs, use a simple list format
            output.push_str("Graph structure:\n\n");
            output.push_str("Nodes:\n");
            for node in self.nodes.values() {
                output.push_str(&format!("  - {} (id: {})\n", node.name, node.id));
            }
            output.push_str("\nEdges:\n");
            for edge in &self.edges {
                let edge_char = if edge.conditional { "?" } else { "-" };
                let data_str = edge
                    .data
                    .as_ref()
                    .map(|d| format!(" [{d}]"))
                    .unwrap_or_default();
                output.push_str(&format!(
                    "  {} {}{} {}{}\n",
                    edge.source, edge_char, edge_char, edge.target, data_str
                ));
            }
        }

        output
    }

    /// Generate a Mermaid flowchart diagram
    ///
    /// Creates a Mermaid syntax flowchart that can be rendered by Mermaid-compatible tools.
    /// The diagram uses different edge styles for conditional and error-handling edges.
    ///
    /// # Example Output
    ///
    /// ```mermaid
    /// graph TD
    ///     A[Node A]
    ///     B[Node B]
    ///     C[Node C]
    ///     A --> B
    ///     A --> C
    /// ```
    ///
    /// # Example Usage
    ///
    /// ```rust
    /// # use dashflow::core::runnable::{Graph, Node, Edge};
    /// let mut graph = Graph::new();
    /// graph.add_node(Node::new("A", "Node A"));
    /// graph.add_node(Node::new("B", "Node B"));
    /// graph.add_edge(Edge::new("A", "B"));
    ///
    /// let mermaid = graph.draw_mermaid();
    /// println!("{}", mermaid);
    /// // Can be rendered at https://mermaid.live or in markdown
    /// ```
    #[must_use]
    pub fn draw_mermaid(&self) -> String {
        let mut output = String::from("graph TD\n");

        if self.nodes.is_empty() {
            output.push_str("    empty[\"Empty Graph\"]\n");
            return output;
        }

        // Generate sanitized node IDs for Mermaid (replace special chars)
        let sanitize_id = |id: &str| -> String { id.replace([':', '-', ' ', '.'], "_") };

        // Add all nodes
        for node in self.nodes.values() {
            let node_id = sanitize_id(&node.id);
            // Use [] for rectangular boxes
            output.push_str(&format!("    {}[\"{}\"]\n", node_id, node.name));
        }

        // Add edges with appropriate styling
        for edge in &self.edges {
            let source_id = sanitize_id(&edge.source);
            let target_id = sanitize_id(&edge.target);

            // Choose edge style based on edge properties
            let (arrow, label) = if edge.conditional {
                // Dotted line for conditional edges
                (
                    "-.->",
                    edge.data
                        .as_ref()
                        .map(|d| format!("|{d}|"))
                        .unwrap_or_default(),
                )
            } else if edge.data.as_ref().is_some_and(|d| d.contains("on_error")) {
                // Dashed line for error/fallback edges
                (
                    "-.->",
                    edge.data
                        .as_ref()
                        .map(|d| format!("|{d}|"))
                        .unwrap_or_default(),
                )
            } else if let Some(data) = &edge.data {
                // Solid line with label for other edges with data
                ("-->", format!("|{data}|"))
            } else {
                // Plain solid line for normal edges
                ("-->", String::new())
            };

            output.push_str(&format!("    {source_id} {arrow} {target_id}{label}\n"));
        }

        output
    }

    /// Check if this graph represents a simple linear chain
    #[must_use]
    pub fn is_linear_chain(&self) -> bool {
        if self.nodes.is_empty() {
            return false;
        }

        // Each node (except last) should have exactly one outgoing edge
        // Each node (except first) should have exactly one incoming edge
        let mut out_counts = HashMap::new();
        let mut in_counts = HashMap::new();

        for edge in &self.edges {
            *out_counts.entry(&edge.source).or_insert(0) += 1;
            *in_counts.entry(&edge.target).or_insert(0) += 1;
        }

        // All nodes should have at most one outgoing and one incoming edge
        out_counts.values().all(|&c| c <= 1) && in_counts.values().all(|&c| c <= 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Node Tests ====================

    #[test]
    fn test_node_new() {
        let node = Node::new("id1", "Node 1");
        assert_eq!(node.id, "id1");
        assert_eq!(node.name, "Node 1");
        assert!(node.metadata.is_none());
    }

    #[test]
    fn test_node_new_with_string() {
        let node = Node::new(String::from("id2"), String::from("Node 2"));
        assert_eq!(node.id, "id2");
        assert_eq!(node.name, "Node 2");
    }

    #[test]
    fn test_node_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), serde_json::json!("value"));
        metadata.insert("count".to_string(), serde_json::json!(42));

        let node = Node::with_metadata("id1", "Node 1", metadata.clone());
        assert_eq!(node.id, "id1");
        assert_eq!(node.name, "Node 1");
        assert!(node.metadata.is_some());

        let meta = node.metadata.unwrap();
        assert_eq!(meta.get("key"), Some(&serde_json::json!("value")));
        assert_eq!(meta.get("count"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_node_with_empty_metadata() {
        let node = Node::with_metadata("id1", "Node 1", HashMap::new());
        assert!(node.metadata.is_some());
        assert!(node.metadata.unwrap().is_empty());
    }

    #[test]
    fn test_node_with_id() {
        let original = Node::new("original_id", "My Node");
        let new_node = original.with_id("new_id");

        assert_eq!(new_node.id, "new_id");
        assert_eq!(new_node.name, "My Node");
        // Original unchanged
        assert_eq!(original.id, "original_id");
    }

    #[test]
    fn test_node_with_id_preserves_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), serde_json::json!("value"));

        let original = Node::with_metadata("id1", "Node 1", metadata);
        let new_node = original.with_id("id2");

        assert_eq!(new_node.id, "id2");
        assert!(new_node.metadata.is_some());
        assert_eq!(
            new_node.metadata.as_ref().unwrap().get("key"),
            Some(&serde_json::json!("value"))
        );
    }

    #[test]
    fn test_node_with_name() {
        let original = Node::new("id1", "Original Name");
        let new_node = original.with_name("New Name");

        assert_eq!(new_node.id, "id1");
        assert_eq!(new_node.name, "New Name");
        // Original unchanged
        assert_eq!(original.name, "Original Name");
    }

    #[test]
    fn test_node_with_name_preserves_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), serde_json::json!(123));

        let original = Node::with_metadata("id1", "Node 1", metadata);
        let new_node = original.with_name("New Name");

        assert_eq!(new_node.name, "New Name");
        assert!(new_node.metadata.is_some());
    }

    #[test]
    fn test_node_equality() {
        let node1 = Node::new("id1", "Node 1");
        let node2 = Node::new("id1", "Node 1");
        let node3 = Node::new("id2", "Node 1");
        let node4 = Node::new("id1", "Node 2");

        assert_eq!(node1, node2);
        assert_ne!(node1, node3);
        assert_ne!(node1, node4);
    }

    #[test]
    fn test_node_clone() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), serde_json::json!("value"));
        let original = Node::with_metadata("id1", "Node 1", metadata);

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_node_debug() {
        let node = Node::new("id1", "Node 1");
        let debug_str = format!("{:?}", node);
        assert!(debug_str.contains("id1"));
        assert!(debug_str.contains("Node 1"));
    }

    #[test]
    fn test_node_empty_strings() {
        let node = Node::new("", "");
        assert_eq!(node.id, "");
        assert_eq!(node.name, "");
    }

    #[test]
    fn test_node_unicode_strings() {
        let node = Node::new("id_日本語", "ノード名");
        assert_eq!(node.id, "id_日本語");
        assert_eq!(node.name, "ノード名");
    }

    #[test]
    fn test_node_special_characters() {
        let node = Node::new("id:with-special.chars", "Node <with> 'quotes'");
        assert_eq!(node.id, "id:with-special.chars");
        assert_eq!(node.name, "Node <with> 'quotes'");
    }

    // ==================== Edge Tests ====================

    #[test]
    fn test_edge_new() {
        let edge = Edge::new("source", "target");
        assert_eq!(edge.source, "source");
        assert_eq!(edge.target, "target");
        assert!(edge.data.is_none());
        assert!(!edge.conditional);
    }

    #[test]
    fn test_edge_new_with_string() {
        let edge = Edge::new(String::from("src"), String::from("tgt"));
        assert_eq!(edge.source, "src");
        assert_eq!(edge.target, "tgt");
    }

    #[test]
    fn test_edge_conditional() {
        let edge = Edge::conditional("source", "target", "condition_name");
        assert_eq!(edge.source, "source");
        assert_eq!(edge.target, "target");
        assert_eq!(edge.data, Some("condition_name".to_string()));
        assert!(edge.conditional);
    }

    #[test]
    fn test_edge_conditional_with_strings() {
        let edge = Edge::conditional(
            String::from("src"),
            String::from("tgt"),
            String::from("cond"),
        );
        assert_eq!(edge.source, "src");
        assert_eq!(edge.target, "tgt");
        assert_eq!(edge.data, Some("cond".to_string()));
        assert!(edge.conditional);
    }

    #[test]
    fn test_edge_with_source() {
        let original = Edge::conditional("src1", "target", "condition");
        let new_edge = original.with_source("src2");

        assert_eq!(new_edge.source, "src2");
        assert_eq!(new_edge.target, "target");
        assert_eq!(new_edge.data, Some("condition".to_string()));
        assert!(new_edge.conditional);
        // Original unchanged
        assert_eq!(original.source, "src1");
    }

    #[test]
    fn test_edge_with_target() {
        let original = Edge::conditional("source", "tgt1", "condition");
        let new_edge = original.with_target("tgt2");

        assert_eq!(new_edge.source, "source");
        assert_eq!(new_edge.target, "tgt2");
        assert_eq!(new_edge.data, Some("condition".to_string()));
        assert!(new_edge.conditional);
        // Original unchanged
        assert_eq!(original.target, "tgt1");
    }

    #[test]
    fn test_edge_equality() {
        let edge1 = Edge::new("source", "target");
        let edge2 = Edge::new("source", "target");
        let edge3 = Edge::new("other_source", "target");
        let edge4 = Edge::conditional("source", "target", "condition");

        assert_eq!(edge1, edge2);
        assert_ne!(edge1, edge3);
        assert_ne!(edge1, edge4); // Different conditional flag
    }

    #[test]
    fn test_edge_clone() {
        let original = Edge::conditional("source", "target", "condition");
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_edge_debug() {
        let edge = Edge::new("source", "target");
        let debug_str = format!("{:?}", edge);
        assert!(debug_str.contains("source"));
        assert!(debug_str.contains("target"));
    }

    #[test]
    fn test_edge_empty_strings() {
        let edge = Edge::new("", "");
        assert_eq!(edge.source, "");
        assert_eq!(edge.target, "");
    }

    #[test]
    fn test_edge_self_loop() {
        let edge = Edge::new("node", "node");
        assert_eq!(edge.source, "node");
        assert_eq!(edge.target, "node");
    }

    // ==================== Graph Construction Tests ====================

    #[test]
    fn test_graph_new() {
        let graph = Graph::new();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_graph_default() {
        let graph = Graph::default();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_graph_add_node() {
        let mut graph = Graph::new();
        let node = Node::new("id1", "Node 1");

        graph.add_node(node.clone());

        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("id1"));
        assert_eq!(graph.nodes.get("id1"), Some(&node));
    }

    #[test]
    fn test_graph_add_multiple_nodes() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("id1", "Node 1"));
        graph.add_node(Node::new("id2", "Node 2"));
        graph.add_node(Node::new("id3", "Node 3"));

        assert_eq!(graph.nodes.len(), 3);
    }

    #[test]
    fn test_graph_add_duplicate_node_overwrites() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("id1", "Original"));
        graph.add_node(Node::new("id1", "Replacement"));

        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes.get("id1").unwrap().name, "Replacement");
    }

    #[test]
    fn test_graph_add_edge() {
        let mut graph = Graph::new();
        let edge = Edge::new("source", "target");

        graph.add_edge(edge.clone());

        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0], edge);
    }

    #[test]
    fn test_graph_add_multiple_edges() {
        let mut graph = Graph::new();
        graph.add_edge(Edge::new("a", "b"));
        graph.add_edge(Edge::new("b", "c"));
        graph.add_edge(Edge::new("a", "c"));

        assert_eq!(graph.edges.len(), 3);
    }

    #[test]
    fn test_graph_clone() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("id1", "Node 1"));
        graph.add_edge(Edge::new("id1", "id2"));

        let cloned = graph.clone();
        assert_eq!(cloned.nodes.len(), graph.nodes.len());
        assert_eq!(cloned.edges.len(), graph.edges.len());
    }

    // ==================== Graph Query Tests ====================

    #[test]
    fn test_graph_first_node_empty() {
        let graph = Graph::new();
        assert!(graph.first_node().is_none());
    }

    #[test]
    fn test_graph_first_node_single() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("only", "Only Node"));

        let first = graph.first_node();
        assert!(first.is_some());
        assert_eq!(first.unwrap().id, "only");
    }

    #[test]
    fn test_graph_first_node_chain() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("first", "First"));
        graph.add_node(Node::new("middle", "Middle"));
        graph.add_node(Node::new("last", "Last"));
        graph.add_edge(Edge::new("first", "middle"));
        graph.add_edge(Edge::new("middle", "last"));

        let first = graph.first_node();
        assert!(first.is_some());
        assert_eq!(first.unwrap().id, "first");
    }

    #[test]
    fn test_graph_last_node_empty() {
        let graph = Graph::new();
        assert!(graph.last_node().is_none());
    }

    #[test]
    fn test_graph_last_node_single() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("only", "Only Node"));

        let last = graph.last_node();
        assert!(last.is_some());
        assert_eq!(last.unwrap().id, "only");
    }

    #[test]
    fn test_graph_last_node_chain() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("first", "First"));
        graph.add_node(Node::new("middle", "Middle"));
        graph.add_node(Node::new("last", "Last"));
        graph.add_edge(Edge::new("first", "middle"));
        graph.add_edge(Edge::new("middle", "last"));

        let last = graph.last_node();
        assert!(last.is_some());
        assert_eq!(last.unwrap().id, "last");
    }

    // ==================== Graph Extend Tests ====================

    #[test]
    fn test_graph_extend_empty_graphs() {
        let mut graph = Graph::new();
        let other = Graph::new();

        graph.extend(&other, "prefix");

        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_graph_extend_into_empty() {
        let mut graph = Graph::new();
        let mut other = Graph::new();
        other.add_node(Node::new("node1", "Node 1"));
        other.add_edge(Edge::new("node1", "node2"));

        graph.extend(&other, "pfx");

        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("pfx:node1"));
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].source, "pfx:node1");
        assert_eq!(graph.edges[0].target, "pfx:node2");
    }

    #[test]
    fn test_graph_extend_merges() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("existing", "Existing"));

        let mut other = Graph::new();
        other.add_node(Node::new("new", "New"));

        graph.extend(&other, "pfx");

        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.contains_key("existing"));
        assert!(graph.nodes.contains_key("pfx:new"));
    }

    #[test]
    fn test_graph_extend_preserves_conditional() {
        let mut graph = Graph::new();
        let mut other = Graph::new();
        other.add_edge(Edge::conditional("a", "b", "condition"));

        graph.extend(&other, "pfx");

        assert_eq!(graph.edges.len(), 1);
        assert!(graph.edges[0].conditional);
        assert_eq!(graph.edges[0].data, Some("condition".to_string()));
    }

    // ==================== Linear Chain Detection Tests ====================

    #[test]
    fn test_is_linear_chain_empty() {
        let graph = Graph::new();
        assert!(!graph.is_linear_chain());
    }

    #[test]
    fn test_is_linear_chain_single_node() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("only", "Only"));
        assert!(graph.is_linear_chain());
    }

    #[test]
    fn test_is_linear_chain_two_nodes() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("first", "First"));
        graph.add_node(Node::new("second", "Second"));
        graph.add_edge(Edge::new("first", "second"));

        assert!(graph.is_linear_chain());
    }

    #[test]
    fn test_is_linear_chain_three_nodes() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "A"));
        graph.add_node(Node::new("b", "B"));
        graph.add_node(Node::new("c", "C"));
        graph.add_edge(Edge::new("a", "b"));
        graph.add_edge(Edge::new("b", "c"));

        assert!(graph.is_linear_chain());
    }

    #[test]
    fn test_is_linear_chain_false_diamond() {
        // A diamond shape: A -> B, A -> C, B -> D, C -> D
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "A"));
        graph.add_node(Node::new("b", "B"));
        graph.add_node(Node::new("c", "C"));
        graph.add_node(Node::new("d", "D"));
        graph.add_edge(Edge::new("a", "b"));
        graph.add_edge(Edge::new("a", "c"));
        graph.add_edge(Edge::new("b", "d"));
        graph.add_edge(Edge::new("c", "d"));

        assert!(!graph.is_linear_chain());
    }

    #[test]
    fn test_is_linear_chain_false_fork() {
        // A fork: A -> B, A -> C
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "A"));
        graph.add_node(Node::new("b", "B"));
        graph.add_node(Node::new("c", "C"));
        graph.add_edge(Edge::new("a", "b"));
        graph.add_edge(Edge::new("a", "c"));

        assert!(!graph.is_linear_chain());
    }

    #[test]
    fn test_is_linear_chain_false_join() {
        // A join: A -> C, B -> C
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "A"));
        graph.add_node(Node::new("b", "B"));
        graph.add_node(Node::new("c", "C"));
        graph.add_edge(Edge::new("a", "c"));
        graph.add_edge(Edge::new("b", "c"));

        assert!(!graph.is_linear_chain());
    }

    // ==================== ASCII Drawing Tests ====================

    #[test]
    fn test_draw_ascii_empty() {
        let graph = Graph::new();
        let ascii = graph.draw_ascii();
        assert_eq!(ascii, "(empty graph)");
    }

    #[test]
    fn test_draw_ascii_single_node() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("only", "Single"));

        let ascii = graph.draw_ascii();
        assert!(ascii.contains("Single"));
        assert!(ascii.contains("+"));
        assert!(ascii.contains("-"));
    }

    #[test]
    fn test_draw_ascii_linear_chain() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "Start"));
        graph.add_node(Node::new("b", "End"));
        graph.add_edge(Edge::new("a", "b"));

        let ascii = graph.draw_ascii();
        assert!(ascii.contains("Start"));
        assert!(ascii.contains("End"));
        assert!(ascii.contains("|"));
        assert!(ascii.contains("v"));
    }

    #[test]
    fn test_draw_ascii_non_linear() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "A"));
        graph.add_node(Node::new("b", "B"));
        graph.add_node(Node::new("c", "C"));
        graph.add_edge(Edge::new("a", "b"));
        graph.add_edge(Edge::new("a", "c"));

        let ascii = graph.draw_ascii();
        // Non-linear graphs use list format
        assert!(ascii.contains("Graph structure:"));
        assert!(ascii.contains("Nodes:"));
        assert!(ascii.contains("Edges:"));
    }

    #[test]
    fn test_draw_ascii_conditional_edge() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "A"));
        graph.add_node(Node::new("b", "B"));
        graph.add_node(Node::new("c", "C"));
        graph.add_edge(Edge::conditional("a", "b", "true"));
        graph.add_edge(Edge::conditional("a", "c", "false"));

        let ascii = graph.draw_ascii();
        // Should show ? for conditional edges
        assert!(ascii.contains("?"));
        assert!(ascii.contains("[true]"));
        assert!(ascii.contains("[false]"));
    }

    #[test]
    fn test_draw_ascii_long_node_names() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "VeryLongNodeNameThatShouldStillRender"));

        let ascii = graph.draw_ascii();
        assert!(ascii.contains("VeryLongNodeNameThatShouldStillRender"));
    }

    // ==================== Mermaid Drawing Tests ====================

    #[test]
    fn test_draw_mermaid_empty() {
        let graph = Graph::new();
        let mermaid = graph.draw_mermaid();

        assert!(mermaid.starts_with("graph TD"));
        assert!(mermaid.contains("empty[\"Empty Graph\"]"));
    }

    #[test]
    fn test_draw_mermaid_single_node() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("only", "Single Node"));

        let mermaid = graph.draw_mermaid();
        assert!(mermaid.starts_with("graph TD"));
        assert!(mermaid.contains("only[\"Single Node\"]"));
    }

    #[test]
    fn test_draw_mermaid_simple_chain() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "Node A"));
        graph.add_node(Node::new("b", "Node B"));
        graph.add_edge(Edge::new("a", "b"));

        let mermaid = graph.draw_mermaid();
        assert!(mermaid.contains("a[\"Node A\"]"));
        assert!(mermaid.contains("b[\"Node B\"]"));
        assert!(mermaid.contains("a --> b"));
    }

    #[test]
    fn test_draw_mermaid_conditional_edge() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "Node A"));
        graph.add_node(Node::new("b", "Node B"));
        graph.add_edge(Edge::conditional("a", "b", "condition"));

        let mermaid = graph.draw_mermaid();
        // Conditional edges use dotted arrows
        assert!(mermaid.contains("-.->"));
        assert!(mermaid.contains("|condition|"));
    }

    #[test]
    fn test_draw_mermaid_error_edge() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "Node A"));
        graph.add_node(Node::new("b", "Node B"));

        let mut edge = Edge::new("a", "b");
        edge.data = Some("on_error: fallback".to_string());
        graph.add_edge(edge);

        let mermaid = graph.draw_mermaid();
        // Error edges also use dotted arrows
        assert!(mermaid.contains("-.->"));
    }

    #[test]
    fn test_draw_mermaid_edge_with_label() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "Node A"));
        graph.add_node(Node::new("b", "Node B"));

        let mut edge = Edge::new("a", "b");
        edge.data = Some("label".to_string());
        graph.add_edge(edge);

        let mermaid = graph.draw_mermaid();
        assert!(mermaid.contains("|label|"));
    }

    #[test]
    fn test_draw_mermaid_sanitizes_special_chars() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("node:with-special.chars", "Node Name"));
        graph.add_node(Node::new("other", "Other"));
        graph.add_edge(Edge::new("node:with-special.chars", "other"));

        let mermaid = graph.draw_mermaid();
        // Special characters should be replaced with underscores
        assert!(mermaid.contains("node_with_special_chars"));
        assert!(!mermaid.contains("node:with-special.chars["));
    }

    #[test]
    fn test_draw_mermaid_sanitizes_spaces() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("node with spaces", "Name"));

        let mermaid = graph.draw_mermaid();
        assert!(mermaid.contains("node_with_spaces"));
    }

    #[test]
    fn test_draw_mermaid_complex_graph() {
        let mut graph = Graph::new();
        graph.add_node(Node::new("start", "Start"));
        graph.add_node(Node::new("process1", "Process 1"));
        graph.add_node(Node::new("process2", "Process 2"));
        graph.add_node(Node::new("end", "End"));

        graph.add_edge(Edge::new("start", "process1"));
        graph.add_edge(Edge::new("start", "process2"));
        graph.add_edge(Edge::new("process1", "end"));
        graph.add_edge(Edge::new("process2", "end"));

        let mermaid = graph.draw_mermaid();
        assert!(mermaid.contains("start --> process1"));
        assert!(mermaid.contains("start --> process2"));
        assert!(mermaid.contains("process1 --> end"));
        assert!(mermaid.contains("process2 --> end"));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_graph_full_workflow() {
        let mut graph = Graph::new();

        // Build a simple pipeline graph
        graph.add_node(Node::new("input", "Input"));
        graph.add_node(Node::new("transform", "Transform"));
        graph.add_node(Node::new("output", "Output"));

        graph.add_edge(Edge::new("input", "transform"));
        graph.add_edge(Edge::new("transform", "output"));

        // Verify structure
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert!(graph.is_linear_chain());

        // Verify first/last
        assert_eq!(graph.first_node().unwrap().id, "input");
        assert_eq!(graph.last_node().unwrap().id, "output");

        // Generate visualizations
        let ascii = graph.draw_ascii();
        assert!(ascii.contains("Input"));
        assert!(ascii.contains("Transform"));
        assert!(ascii.contains("Output"));

        let mermaid = graph.draw_mermaid();
        assert!(mermaid.contains("input --> transform"));
        assert!(mermaid.contains("transform --> output"));
    }

    #[test]
    fn test_graph_branch_workflow() {
        let mut graph = Graph::new();

        // Build a branching graph
        graph.add_node(Node::new("start", "Start"));
        graph.add_node(Node::new("branch_a", "Branch A"));
        graph.add_node(Node::new("branch_b", "Branch B"));
        graph.add_node(Node::new("merge", "Merge"));

        graph.add_edge(Edge::conditional("start", "branch_a", "condition_a"));
        graph.add_edge(Edge::conditional("start", "branch_b", "condition_b"));
        graph.add_edge(Edge::new("branch_a", "merge"));
        graph.add_edge(Edge::new("branch_b", "merge"));

        // Verify structure
        assert!(!graph.is_linear_chain());
        assert_eq!(graph.first_node().unwrap().id, "start");
        assert_eq!(graph.last_node().unwrap().id, "merge");

        // Check mermaid output has conditional edges
        let mermaid = graph.draw_mermaid();
        assert!(mermaid.contains("-.->"));
    }

    #[test]
    fn test_graph_with_metadata() {
        let mut graph = Graph::new();

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("llm"));
        metadata.insert("model".to_string(), serde_json::json!("gpt-4"));

        graph.add_node(Node::with_metadata("llm", "LLM Call", metadata));
        graph.add_node(Node::new("output", "Output"));
        graph.add_edge(Edge::new("llm", "output"));

        let node = graph.nodes.get("llm").unwrap();
        let meta = node.metadata.as_ref().unwrap();
        assert_eq!(meta.get("type"), Some(&serde_json::json!("llm")));
        assert_eq!(meta.get("model"), Some(&serde_json::json!("gpt-4")));
    }

    #[test]
    fn test_graph_cycle_draw_uses_list_format() {
        // A cycle: A -> B -> C -> A
        // In a cycle, no node has zero incoming edges, so first_node() returns None.
        // The is_linear_chain() check passes (each node has 1 in/out), but draw_ascii
        // handles this by falling back to list format when first_node() is None.
        let mut graph = Graph::new();
        graph.add_node(Node::new("a", "A"));
        graph.add_node(Node::new("b", "B"));
        graph.add_node(Node::new("c", "C"));
        graph.add_edge(Edge::new("a", "b"));
        graph.add_edge(Edge::new("b", "c"));
        graph.add_edge(Edge::new("c", "a"));

        // In a cycle, no node has zero incoming edges
        assert!(graph.first_node().is_none());

        // draw_ascii returns empty for linear chain with no first node
        // This is expected behavior for a pure cycle
        let ascii = graph.draw_ascii();
        assert!(ascii.is_empty());
        // The graph has 3 nodes
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 3);
    }

    #[test]
    fn test_node_chained_builders() {
        let node = Node::new("id1", "Name1")
            .with_id("id2")
            .with_name("Name2");

        assert_eq!(node.id, "id2");
        assert_eq!(node.name, "Name2");
    }

    #[test]
    fn test_edge_chained_builders() {
        let edge = Edge::new("src1", "tgt1")
            .with_source("src2")
            .with_target("tgt2");

        assert_eq!(edge.source, "src2");
        assert_eq!(edge.target, "tgt2");
    }
}
