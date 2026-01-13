//! Computation Graph Engine for DashTerm
//!
//! Provides a LangGraph-style reactive computation graph with:
//! - Nodes that can execute async computations
//! - Typed edges connecting nodes
//! - State that flows through the graph
//! - Real-time visualization data for the UI

pub mod edge;
pub mod node;
pub mod state;
pub mod executor;

pub use edge::{Edge, EdgeType};
pub use node::{Node, NodeId, NodeStatus, NodeType, GroupId, NodeGroup};
pub use state::{GraphState, StateValue};
pub use executor::{GraphExecutor, ExecutionEvent};

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Graph computation errors
#[derive(Debug, Error)]
pub enum GraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("Cycle detected in graph")]
    CycleDetected,

    #[error("Execution error in node {node}: {message}")]
    ExecutionError { node: NodeId, message: String },

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

pub type Result<T> = std::result::Result<T, GraphError>;

/// A computation graph
#[derive(Debug, Serialize, Deserialize)]
pub struct ComputationGraph {
    /// The underlying directed graph
    #[serde(skip)]
    graph: DiGraph<Node, Edge>,
    /// Map from NodeId to graph index
    #[serde(skip)]
    node_indices: HashMap<NodeId, NodeIndex>,
    /// Graph metadata
    pub name: String,
    pub description: String,
    /// Current execution state
    #[serde(skip)]
    execution_state: Option<GraphState>,
    /// Node groups for collapsible visualization
    groups: HashMap<GroupId, NodeGroup>,
}

impl Default for ComputationGraph {
    fn default() -> Self {
        Self::new("Unnamed Graph")
    }
}

impl ComputationGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            name: name.into(),
            description: String::new(),
            execution_state: None,
            groups: HashMap::new(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let id = node.id.clone();
        let idx = self.graph.add_node(node);
        self.node_indices.insert(id.clone(), idx);
        id
    }

    /// Add an edge between nodes
    pub fn add_edge(&mut self, from: &NodeId, to: &NodeId, edge: Edge) -> Result<()> {
        let from_idx = self.node_indices.get(from)
            .ok_or_else(|| GraphError::NodeNotFound(from.clone()))?;
        let to_idx = self.node_indices.get(to)
            .ok_or_else(|| GraphError::NodeNotFound(to.clone()))?;

        self.graph.add_edge(*from_idx, *to_idx, edge);
        Ok(())
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &NodeId) -> Option<&Node> {
        self.node_indices.get(id)
            .and_then(|idx| self.graph.node_weight(*idx))
    }

    /// Get a mutable node by ID
    pub fn get_node_mut(&mut self, id: &NodeId) -> Option<&mut Node> {
        self.node_indices.get(id)
            .and_then(|idx| self.graph.node_weight_mut(*idx))
    }

    /// Get all nodes
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.graph.node_weights()
    }

    /// Get all edges
    pub fn edges(&self) -> impl Iterator<Item = (&NodeId, &NodeId, &Edge)> {
        self.graph.edge_indices().filter_map(|idx| {
            let (from, to) = self.graph.edge_endpoints(idx)?;
            let from_node = self.graph.node_weight(from)?;
            let to_node = self.graph.node_weight(to)?;
            let edge = self.graph.edge_weight(idx)?;
            Some((&from_node.id, &to_node.id, edge))
        })
    }

    /// Get visualization data for the UI
    pub fn get_layout_data(&self) -> GraphLayoutData {
        let nodes: Vec<NodeLayoutData> = self.graph.node_weights()
            .map(|n| NodeLayoutData {
                id: n.id.clone(),
                label: n.label.clone(),
                node_type: n.node_type,
                status: n.status,
                position: n.position,
                group_id: n.group_id.clone(),
            })
            .collect();

        let edges: Vec<EdgeLayoutData> = self.edges()
            .map(|(from, to, edge)| EdgeLayoutData {
                from: from.clone(),
                to: to.clone(),
                edge_type: edge.edge_type,
                label: edge.label.clone(),
            })
            .collect();

        let groups: Vec<GroupLayoutData> = self.groups.values()
            .map(|g| GroupLayoutData {
                id: g.id.clone(),
                label: g.label.clone(),
                collapsed: g.collapsed,
                node_count: g.node_count,
                status: g.status,
                position: g.position,
            })
            .collect();

        GraphLayoutData { nodes, edges, groups }
    }

    // MARK: - Group Management

    /// Create a new node group
    pub fn create_group(&mut self, group: NodeGroup) -> GroupId {
        let id = group.id.clone();
        self.groups.insert(id.clone(), group);
        id
    }

    /// Add a node to a group
    pub fn add_node_to_group(&mut self, node_id: &NodeId, group_id: &GroupId) -> Result<()> {
        // Verify node exists
        if !self.node_indices.contains_key(node_id) {
            return Err(GraphError::NodeNotFound(node_id.clone()));
        }

        // Update the node's group_id
        if let Some(idx) = self.node_indices.get(node_id) {
            if let Some(node) = self.graph.node_weight_mut(*idx) {
                node.group_id = Some(group_id.clone());
            }
        }

        // Update group status
        self.update_group_status(group_id);
        Ok(())
    }

    /// Get all nodes in a group
    pub fn get_nodes_in_group(&self, group_id: &GroupId) -> Vec<&Node> {
        self.graph.node_weights()
            .filter(|n| n.group_id.as_ref() == Some(group_id))
            .collect()
    }

    /// Update group status based on its child nodes
    pub fn update_group_status(&mut self, group_id: &GroupId) {
        let statuses: Vec<NodeStatus> = self.graph.node_weights()
            .filter(|n| n.group_id.as_ref() == Some(group_id))
            .map(|n| n.status)
            .collect();

        if let Some(group) = self.groups.get_mut(group_id) {
            group.update_status_from_children(&statuses);
        }
    }

    /// Toggle group collapsed state
    pub fn toggle_group(&mut self, group_id: &GroupId) -> bool {
        if let Some(group) = self.groups.get_mut(group_id) {
            group.collapsed = !group.collapsed;
            group.collapsed
        } else {
            false
        }
    }

    /// Get group by ID
    pub fn get_group(&self, group_id: &GroupId) -> Option<&NodeGroup> {
        self.groups.get(group_id)
    }

    /// Get all groups
    pub fn groups(&self) -> impl Iterator<Item = &NodeGroup> {
        self.groups.values()
    }

    /// Automatically group consecutive tool nodes that share a common parent.
    /// Returns the number of groups created.
    pub fn auto_group_tool_sequences(&mut self, min_tools: usize) -> usize {
        use petgraph::Direction;

        let mut groups_created = 0;
        let mut processed: std::collections::HashSet<NodeId> = std::collections::HashSet::new();

        // Find all model nodes (they typically precede tool sequences)
        let model_indices: Vec<NodeIndex> = self.graph.node_indices()
            .filter(|&idx| {
                self.graph.node_weight(idx)
                    .map(|n| matches!(n.node_type, NodeType::Model))
                    .unwrap_or(false)
            })
            .collect();

        for model_idx in model_indices {
            // Get all direct tool children
            let tool_children: Vec<(NodeIndex, NodeId, String)> = self.graph
                .neighbors_directed(model_idx, Direction::Outgoing)
                .filter_map(|idx| {
                    let node = self.graph.node_weight(idx)?;
                    if matches!(node.node_type, NodeType::Tool) && !processed.contains(&node.id) {
                        Some((idx, node.id.clone(), node.label.clone()))
                    } else {
                        None
                    }
                })
                .collect();

            if tool_children.len() >= min_tools {
                // Create a group for these tools
                let model_id = self.graph.node_weight(model_idx)
                    .map(|n| n.id.clone())
                    .unwrap_or_default();
                let group_id = format!("group_{}", model_id);
                let tool_names: Vec<&str> = tool_children.iter()
                    .map(|(_, _, label)| label.as_str())
                    .collect();

                let group = NodeGroup::tool_group(&group_id, &tool_names);
                self.groups.insert(group_id.clone(), group);

                // Assign nodes to group
                for (_, node_id, _) in &tool_children {
                    if let Some(idx) = self.node_indices.get(node_id) {
                        if let Some(node) = self.graph.node_weight_mut(*idx) {
                            node.group_id = Some(group_id.clone());
                        }
                    }
                    processed.insert(node_id.clone());
                }

                self.update_group_status(&group_id);
                groups_created += 1;
            }
        }

        groups_created
    }

    /// Perform topological sort
    pub fn topological_order(&self) -> Result<Vec<NodeId>> {
        use petgraph::algo::toposort;

        toposort(&self.graph, None)
            .map(|indices| {
                indices.into_iter()
                    .filter_map(|idx| self.graph.node_weight(idx).map(|n| n.id.clone()))
                    .collect()
            })
            .map_err(|_| GraphError::CycleDetected)
    }

    /// Compute layout positions for all nodes using a hierarchical/layered algorithm.
    /// Assigns (x, y) positions based on topological depth and horizontal distribution.
    pub fn compute_layout(&mut self) {
        use petgraph::Direction;

        // Layout parameters
        const LAYER_SPACING: f32 = 120.0;  // Vertical spacing between layers
        const NODE_SPACING: f32 = 150.0;   // Horizontal spacing between nodes
        const START_X: f32 = 0.0;
        const START_Y: f32 = 0.0;

        // If graph is empty, nothing to do
        if self.graph.node_count() == 0 {
            return;
        }

        // Compute depth for each node (longest path from any root)
        let mut depths: HashMap<NodeId, usize> = HashMap::new();

        // Find all roots (nodes with no incoming edges)
        let roots: Vec<NodeIndex> = self.graph.node_indices()
            .filter(|&idx| {
                self.graph.neighbors_directed(idx, Direction::Incoming).count() == 0
            })
            .collect();

        // BFS to compute maximum depth for each node
        fn compute_max_depth(
            graph: &DiGraph<Node, Edge>,
            node: NodeIndex,
            depths: &mut HashMap<NodeId, usize>,
            current_depth: usize,
        ) {
            if let Some(node_data) = graph.node_weight(node) {
                let node_id = node_data.id.clone();
                let existing_depth = depths.get(&node_id).copied().unwrap_or(0);
                if current_depth > existing_depth {
                    depths.insert(node_id, current_depth);
                }
            }

            for neighbor in graph.neighbors_directed(node, Direction::Outgoing) {
                compute_max_depth(graph, neighbor, depths, current_depth + 1);
            }
        }

        // Start from each root
        for root in roots {
            compute_max_depth(&self.graph, root, &mut depths, 0);
        }

        // Handle disconnected nodes (give them depth 0)
        for node in self.graph.node_weights() {
            if !depths.contains_key(&node.id) {
                depths.insert(node.id.clone(), 0);
            }
        }

        // Group nodes by depth (layer)
        let max_depth = depths.values().copied().max().unwrap_or(0);
        let mut layers: Vec<Vec<NodeId>> = vec![Vec::new(); max_depth + 1];

        for (node_id, depth) in &depths {
            layers[*depth].push(node_id.clone());
        }

        // Sort nodes within each layer for consistent ordering
        for layer in &mut layers {
            layer.sort();
        }

        // Assign positions
        for (layer_idx, layer_nodes) in layers.iter().enumerate() {
            let layer_width = (layer_nodes.len() as f32 - 1.0) * NODE_SPACING;
            let layer_start_x = START_X - layer_width / 2.0;

            for (node_idx, node_id) in layer_nodes.iter().enumerate() {
                let x = layer_start_x + (node_idx as f32) * NODE_SPACING;
                let y = START_Y + (layer_idx as f32) * LAYER_SPACING;

                if let Some(idx) = self.node_indices.get(node_id) {
                    if let Some(node) = self.graph.node_weight_mut(*idx) {
                        node.position = Some((x, y));
                    }
                }
            }
        }
    }

    /// Compute the bounding box of all nodes in the graph.
    /// Includes node dimensions (assumes 80x40 node size).
    pub fn compute_bounding_box(&self) -> BoundingBox {
        const NODE_WIDTH: f32 = 80.0;
        const NODE_HEIGHT: f32 = 40.0;
        const HALF_WIDTH: f32 = NODE_WIDTH / 2.0;
        const HALF_HEIGHT: f32 = NODE_HEIGHT / 2.0;

        let mut bbox = BoundingBox::empty();

        for node in self.graph.node_weights() {
            if let Some((x, y)) = node.position {
                // Include node bounds, not just center point
                bbox.include_point(x - HALF_WIDTH, y - HALF_HEIGHT);
                bbox.include_point(x + HALF_WIDTH, y + HALF_HEIGHT);
            }
        }

        bbox
    }

    /// Compute zoom and pan values to fit all nodes in a viewport of the given size.
    /// Returns (zoom, pan_x, pan_y) where zoom is between 0.1 and 2.0.
    pub fn compute_zoom_to_fit(&self, viewport_width: f32, viewport_height: f32) -> (f32, f32, f32) {
        let bbox = self.compute_bounding_box();

        if bbox.is_empty() {
            return (1.0, 0.0, 0.0);
        }

        // Add padding around the content
        let padded = bbox.with_padding(50.0);

        let content_width = padded.width();
        let content_height = padded.height();

        // Avoid division by zero
        if content_width <= 0.0 || content_height <= 0.0 {
            return (1.0, 0.0, 0.0);
        }

        // Calculate zoom to fit content in viewport
        let zoom_x = viewport_width / content_width;
        let zoom_y = viewport_height / content_height;
        let zoom = zoom_x.min(zoom_y).clamp(0.1, 2.0);

        // Calculate pan to center the content
        let (center_x, center_y) = padded.center();
        let pan_x = -center_x * zoom;
        let pan_y = -center_y * zoom;

        (zoom, pan_x, pan_y)
    }
}

/// Bounding box for graph visualization
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl BoundingBox {
    /// Create an empty bounding box
    pub fn empty() -> Self {
        Self {
            min_x: f32::INFINITY,
            min_y: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            max_y: f32::NEG_INFINITY,
        }
    }

    /// Check if the bounding box is empty (no valid bounds)
    pub fn is_empty(&self) -> bool {
        self.min_x > self.max_x || self.min_y > self.max_y
    }

    /// Get the width of the bounding box
    pub fn width(&self) -> f32 {
        if self.is_empty() { 0.0 } else { self.max_x - self.min_x }
    }

    /// Get the height of the bounding box
    pub fn height(&self) -> f32 {
        if self.is_empty() { 0.0 } else { self.max_y - self.min_y }
    }

    /// Get the center of the bounding box
    pub fn center(&self) -> (f32, f32) {
        if self.is_empty() {
            (0.0, 0.0)
        } else {
            ((self.min_x + self.max_x) / 2.0, (self.min_y + self.max_y) / 2.0)
        }
    }

    /// Expand the bounding box to include a point
    pub fn include_point(&mut self, x: f32, y: f32) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }

    /// Add padding to all sides
    pub fn with_padding(&self, padding: f32) -> Self {
        if self.is_empty() {
            *self
        } else {
            Self {
                min_x: self.min_x - padding,
                min_y: self.min_y - padding,
                max_x: self.max_x + padding,
                max_y: self.max_y + padding,
            }
        }
    }
}

/// Layout data for graph visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLayoutData {
    pub nodes: Vec<NodeLayoutData>,
    pub edges: Vec<EdgeLayoutData>,
    #[serde(default)]
    pub groups: Vec<GroupLayoutData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLayoutData {
    pub id: NodeId,
    pub label: String,
    pub node_type: NodeType,
    pub status: NodeStatus,
    pub position: Option<(f32, f32)>,
    /// Group this node belongs to (if any)
    pub group_id: Option<GroupId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeLayoutData {
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

/// Layout data for a node group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupLayoutData {
    pub id: GroupId,
    pub label: String,
    pub collapsed: bool,
    pub node_count: usize,
    pub status: NodeStatus,
    pub position: Option<(f32, f32)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_empty_graph() {
        let mut graph = ComputationGraph::new("Test");
        graph.compute_layout();
        // Should not panic on empty graph
        assert_eq!(graph.nodes().count(), 0);
    }

    #[test]
    fn test_layout_single_node() {
        let mut graph = ComputationGraph::new("Test");
        let node = Node::start("start");
        graph.add_node(node);

        graph.compute_layout();

        let layout = graph.get_layout_data();
        assert_eq!(layout.nodes.len(), 1);
        assert!(layout.nodes[0].position.is_some());

        let (x, y) = layout.nodes[0].position.unwrap();
        // Single node should be at origin
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn test_layout_linear_chain() {
        let mut graph = ComputationGraph::new("Test");

        // Create a linear chain: start -> model -> tool -> end
        graph.add_node(Node::start("start"));
        graph.add_node(Node::model("model", "LLM"));
        graph.add_node(Node::tool("tool", "Search"));
        graph.add_node(Node::end("end"));

        graph.add_edge(&"start".to_string(), &"model".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"model".to_string(), &"tool".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"tool".to_string(), &"end".to_string(), Edge::normal()).unwrap();

        graph.compute_layout();

        let layout = graph.get_layout_data();

        // All nodes should have positions
        for node in &layout.nodes {
            assert!(node.position.is_some(), "Node {} should have position", node.id);
        }

        // Verify nodes are in different layers (y increases with depth)
        let pos_map: std::collections::HashMap<_, _> = layout.nodes.iter()
            .map(|n| (n.id.clone(), n.position.unwrap()))
            .collect();

        let start_y = pos_map["start"].1;
        let model_y = pos_map["model"].1;
        let tool_y = pos_map["tool"].1;
        let end_y = pos_map["end"].1;

        assert!(start_y < model_y, "model should be below start");
        assert!(model_y < tool_y, "tool should be below model");
        assert!(tool_y < end_y, "end should be below tool");
    }

    #[test]
    fn test_layout_parallel_branches() {
        let mut graph = ComputationGraph::new("Test");

        // Create a diamond: start -> (model1, model2) -> end
        graph.add_node(Node::start("start"));
        graph.add_node(Node::model("model1", "LLM 1"));
        graph.add_node(Node::model("model2", "LLM 2"));
        graph.add_node(Node::end("end"));

        graph.add_edge(&"start".to_string(), &"model1".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"start".to_string(), &"model2".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"model1".to_string(), &"end".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"model2".to_string(), &"end".to_string(), Edge::normal()).unwrap();

        graph.compute_layout();

        let layout = graph.get_layout_data();

        let pos_map: std::collections::HashMap<_, _> = layout.nodes.iter()
            .map(|n| (n.id.clone(), n.position.unwrap()))
            .collect();

        // model1 and model2 should be on the same layer (same y)
        let model1_y = pos_map["model1"].1;
        let model2_y = pos_map["model2"].1;
        assert_eq!(model1_y, model2_y, "parallel nodes should be on same layer");

        // model1 and model2 should have different x positions
        let model1_x = pos_map["model1"].0;
        let model2_x = pos_map["model2"].0;
        assert_ne!(model1_x, model2_x, "parallel nodes should have different x positions");
    }

    #[test]
    fn test_layout_disconnected_nodes() {
        let mut graph = ComputationGraph::new("Test");

        // Add disconnected nodes (no edges)
        graph.add_node(Node::model("node1", "Model 1"));
        graph.add_node(Node::tool("node2", "Tool 1"));
        graph.add_node(Node::model("node3", "Model 2"));

        graph.compute_layout();

        let layout = graph.get_layout_data();

        // All nodes should have positions
        for node in &layout.nodes {
            assert!(node.position.is_some(), "Node {} should have position", node.id);
        }

        // Disconnected nodes should all be on layer 0 (same y)
        let positions: Vec<_> = layout.nodes.iter()
            .map(|n| n.position.unwrap())
            .collect();

        let first_y = positions[0].1;
        for pos in &positions {
            assert_eq!(pos.1, first_y, "disconnected nodes should be on same layer");
        }
    }

    #[test]
    fn test_layout_preserves_positions_after_recompute() {
        let mut graph = ComputationGraph::new("Test");

        graph.add_node(Node::start("start"));
        graph.add_node(Node::end("end"));
        graph.add_edge(&"start".to_string(), &"end".to_string(), Edge::normal()).unwrap();

        graph.compute_layout();
        let layout1 = graph.get_layout_data();

        // Compute layout again
        graph.compute_layout();
        let layout2 = graph.get_layout_data();

        // Positions should be the same
        for (n1, n2) in layout1.nodes.iter().zip(layout2.nodes.iter()) {
            assert_eq!(n1.position, n2.position, "positions should be stable");
        }
    }

    #[test]
    fn test_layout_ffi_via_graph() {
        let mut graph = ComputationGraph::new("Test");

        graph.add_node(Node::start("start"));
        graph.add_node(Node::tool("search", "Web Search"));
        graph.add_node(Node::end("end"));

        graph.add_edge(&"start".to_string(), &"search".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"search".to_string(), &"end".to_string(), Edge::normal()).unwrap();

        graph.compute_layout();

        // Verify layout data is properly serializable
        let layout = graph.get_layout_data();
        let json = serde_json::to_string(&layout).expect("should serialize to JSON");

        // Deserialize and verify
        let parsed: GraphLayoutData = serde_json::from_str(&json).expect("should deserialize from JSON");
        assert_eq!(parsed.nodes.len(), 3);
        assert_eq!(parsed.edges.len(), 2);

        for node in &parsed.nodes {
            assert!(node.position.is_some());
        }
    }

    #[test]
    fn test_bounding_box_empty() {
        let bbox = BoundingBox::empty();
        assert!(bbox.is_empty());
        assert_eq!(bbox.width(), 0.0);
        assert_eq!(bbox.height(), 0.0);
        assert_eq!(bbox.center(), (0.0, 0.0));
    }

    #[test]
    fn test_bounding_box_single_point() {
        let mut bbox = BoundingBox::empty();
        bbox.include_point(10.0, 20.0);

        assert!(!bbox.is_empty());
        assert_eq!(bbox.min_x, 10.0);
        assert_eq!(bbox.max_x, 10.0);
        assert_eq!(bbox.min_y, 20.0);
        assert_eq!(bbox.max_y, 20.0);
        assert_eq!(bbox.width(), 0.0);
        assert_eq!(bbox.height(), 0.0);
        assert_eq!(bbox.center(), (10.0, 20.0));
    }

    #[test]
    fn test_bounding_box_multiple_points() {
        let mut bbox = BoundingBox::empty();
        bbox.include_point(-50.0, -30.0);
        bbox.include_point(50.0, 90.0);

        assert_eq!(bbox.width(), 100.0);
        assert_eq!(bbox.height(), 120.0);
        assert_eq!(bbox.center(), (0.0, 30.0));
    }

    #[test]
    fn test_bounding_box_with_padding() {
        let mut bbox = BoundingBox::empty();
        bbox.include_point(0.0, 0.0);
        bbox.include_point(100.0, 50.0);

        let padded = bbox.with_padding(10.0);
        assert_eq!(padded.min_x, -10.0);
        assert_eq!(padded.max_x, 110.0);
        assert_eq!(padded.min_y, -10.0);
        assert_eq!(padded.max_y, 60.0);
    }

    #[test]
    fn test_compute_bounding_box_empty_graph() {
        let graph = ComputationGraph::new("Test");
        let bbox = graph.compute_bounding_box();
        assert!(bbox.is_empty());
    }

    #[test]
    fn test_compute_bounding_box_with_nodes() {
        let mut graph = ComputationGraph::new("Test");

        graph.add_node(Node::start("start"));
        graph.add_node(Node::model("model", "LLM"));
        graph.add_node(Node::end("end"));

        graph.add_edge(&"start".to_string(), &"model".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"model".to_string(), &"end".to_string(), Edge::normal()).unwrap();

        graph.compute_layout();
        let bbox = graph.compute_bounding_box();

        assert!(!bbox.is_empty());
        // With 3 nodes in vertical chain, should have significant height
        assert!(bbox.height() > 200.0);
    }

    #[test]
    fn test_zoom_to_fit_empty_graph() {
        let graph = ComputationGraph::new("Test");
        let (zoom, pan_x, pan_y) = graph.compute_zoom_to_fit(800.0, 600.0);

        assert_eq!(zoom, 1.0);
        assert_eq!(pan_x, 0.0);
        assert_eq!(pan_y, 0.0);
    }

    #[test]
    fn test_zoom_to_fit_single_node() {
        let mut graph = ComputationGraph::new("Test");
        graph.add_node(Node::start("start"));
        graph.compute_layout();

        let (zoom, pan_x, pan_y) = graph.compute_zoom_to_fit(800.0, 600.0);

        // Single node at origin should result in large zoom (capped at 2.0)
        assert_eq!(zoom, 2.0);
        // Pan should center on origin
        assert!(pan_x.abs() < 1.0);
        assert!(pan_y.abs() < 1.0);
    }

    #[test]
    fn test_zoom_to_fit_large_graph() {
        let mut graph = ComputationGraph::new("Test");

        // Create a large graph that exceeds viewport
        for i in 0..10 {
            graph.add_node(Node::tool(&format!("node{}", i), &format!("Tool {}", i)));
        }

        // Create chain
        for i in 0..9 {
            let from = format!("node{}", i);
            let to = format!("node{}", i + 1);
            graph.add_edge(&from, &to, Edge::normal()).unwrap();
        }

        graph.compute_layout();
        let (zoom, _pan_x, _pan_y) = graph.compute_zoom_to_fit(400.0, 300.0);

        // Large graph should require zooming out
        assert!(zoom < 1.0);
        assert!(zoom >= 0.1);
    }

    #[test]
    fn test_create_group() {
        let mut graph = ComputationGraph::new("Test");
        let group = NodeGroup::new("group1", "Test Group");
        let group_id = graph.create_group(group);

        assert_eq!(group_id, "group1");
        assert!(graph.get_group(&group_id).is_some());
    }

    #[test]
    fn test_add_node_to_group() {
        let mut graph = ComputationGraph::new("Test");

        // Create a group and a node
        let group = NodeGroup::new("group1", "Test Group");
        graph.create_group(group);

        let node = Node::tool("tool1", "Read File");
        graph.add_node(node);

        // Add node to group
        graph.add_node_to_group(&"tool1".to_string(), &"group1".to_string()).unwrap();

        // Verify
        let nodes_in_group = graph.get_nodes_in_group(&"group1".to_string());
        assert_eq!(nodes_in_group.len(), 1);
        assert_eq!(nodes_in_group[0].id, "tool1");
    }

    #[test]
    fn test_auto_group_tool_sequences() {
        let mut graph = ComputationGraph::new("Test");

        // Create: model -> (tool1, tool2, tool3)
        graph.add_node(Node::model("model1", "LLM"));
        graph.add_node(Node::tool("tool1", "Read"));
        graph.add_node(Node::tool("tool2", "Edit"));
        graph.add_node(Node::tool("tool3", "Write"));

        graph.add_edge(&"model1".to_string(), &"tool1".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"model1".to_string(), &"tool2".to_string(), Edge::normal()).unwrap();
        graph.add_edge(&"model1".to_string(), &"tool3".to_string(), Edge::normal()).unwrap();

        // Auto-group with min 2 tools
        let groups_created = graph.auto_group_tool_sequences(2);
        assert_eq!(groups_created, 1);

        // Verify all tools are in the same group
        let layout = graph.get_layout_data();
        let grouped_nodes: Vec<_> = layout.nodes.iter()
            .filter(|n| n.group_id.is_some())
            .collect();
        assert_eq!(grouped_nodes.len(), 3);

        // Verify group exists in layout
        assert_eq!(layout.groups.len(), 1);
        assert_eq!(layout.groups[0].node_count, 3);
    }

    #[test]
    fn test_toggle_group() {
        let mut graph = ComputationGraph::new("Test");
        let group = NodeGroup::new("group1", "Test Group");
        graph.create_group(group);

        // Initially not collapsed
        assert!(!graph.get_group(&"group1".to_string()).unwrap().collapsed);

        // Toggle to collapsed
        let collapsed = graph.toggle_group(&"group1".to_string());
        assert!(collapsed);

        // Toggle back to expanded
        let collapsed = graph.toggle_group(&"group1".to_string());
        assert!(!collapsed);
    }

    #[test]
    fn test_group_status_aggregation() {
        let mut graph = ComputationGraph::new("Test");

        // Create group and nodes
        let group = NodeGroup::new("group1", "Test Group");
        graph.create_group(group);

        let mut node1 = Node::tool("tool1", "Read");
        node1.status = NodeStatus::Success;
        node1.group_id = Some("group1".to_string());
        graph.add_node(node1);

        let mut node2 = Node::tool("tool2", "Edit");
        node2.status = NodeStatus::Running;
        node2.group_id = Some("group1".to_string());
        graph.add_node(node2);

        // Update group status
        graph.update_group_status(&"group1".to_string());

        // Group should be Running (because one child is running)
        let group = graph.get_group(&"group1".to_string()).unwrap();
        assert_eq!(group.status, NodeStatus::Running);
    }
}
