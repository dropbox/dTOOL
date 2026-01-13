// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph Schema Export
//!
//! This module provides types for exporting graph structure and metadata
//! for visualization and introspection purposes.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, schema::{NodeMetadata, NodeType}};
//!
//! let mut graph: StateGraph<MyState> = StateGraph::new();
//!
//! graph.add_node_with_metadata(
//!     "researcher",
//!     NodeMetadata::new("Gathers research from multiple sources")
//!         .with_node_type(NodeType::Tool)
//!         .with_input_fields(vec!["topic"])
//!         .with_output_fields(vec!["findings"]),
//!     |state| Box::pin(async move { /* ... */ state }),
//! );
//!
//! let schema = graph.export_schema("my-graph");
//! println!("{}", serde_json::to_string_pretty(&schema)?);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of node in the graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Generic transformation node
    #[default]
    Transform,
    /// Node that calls an LLM
    Llm,
    /// Node that uses external tools
    Tool,
    /// Node that makes routing decisions
    Router,
    /// Node that aggregates data from multiple sources
    Aggregator,
    /// Node that validates or filters data
    Validator,
    /// Node that handles human-in-the-loop interactions
    HumanInLoop,
    /// Checkpoint/persistence node
    Checkpoint,
    /// Custom node type with description
    Custom(String),
}

impl NodeType {
    /// Get a display name for the node type
    pub fn display_name(&self) -> &str {
        match self {
            NodeType::Transform => "Transform",
            NodeType::Llm => "LLM",
            NodeType::Tool => "Tool",
            NodeType::Router => "Router",
            NodeType::Aggregator => "Aggregator",
            NodeType::Validator => "Validator",
            NodeType::HumanInLoop => "Human-in-Loop",
            NodeType::Checkpoint => "Checkpoint",
            NodeType::Custom(name) => name,
        }
    }

    /// Get an icon hint for the node type (for UI rendering)
    pub fn icon_hint(&self) -> &str {
        match self {
            NodeType::Transform => "⚙️",
            NodeType::Llm => "🤖",
            NodeType::Tool => "🔧",
            NodeType::Router => "🔀",
            NodeType::Aggregator => "📊",
            NodeType::Validator => "✓",
            NodeType::HumanInLoop => "👤",
            NodeType::Checkpoint => "💾",
            NodeType::Custom(_) => "📦",
        }
    }
}

/// Metadata for a single node
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeMetadata {
    /// Human-readable description of what this node does
    pub description: Option<String>,
    /// Type of node (LLM, Tool, Transform, etc.)
    pub node_type: NodeType,
    /// Fields from state that this node reads
    pub input_fields: Vec<String>,
    /// Fields in state that this node modifies
    pub output_fields: Vec<String>,
    /// Optional position hint for graph layout (x, y)
    pub position: Option<(f64, f64)>,
    /// Additional custom attributes
    pub attributes: HashMap<String, String>,
}

impl NodeMetadata {
    /// Create new metadata with a description
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: Some(description.into()),
            ..Default::default()
        }
    }

    /// Create empty metadata
    pub fn empty() -> Self {
        Self::default()
    }

    /// Set the node type
    #[must_use]
    pub fn with_node_type(mut self, node_type: NodeType) -> Self {
        self.node_type = node_type;
        self
    }

    /// Set input fields
    #[must_use]
    pub fn with_input_fields(mut self, fields: Vec<impl Into<String>>) -> Self {
        self.input_fields = fields.into_iter().map(Into::into).collect();
        self
    }

    /// Set output fields
    #[must_use]
    pub fn with_output_fields(mut self, fields: Vec<impl Into<String>>) -> Self {
        self.output_fields = fields.into_iter().map(Into::into).collect();
        self
    }

    /// Set position hint for layout
    #[must_use]
    pub fn with_position(mut self, x: f64, y: f64) -> Self {
        self.position = Some((x, y));
        self
    }

    /// Add a custom attribute
    #[must_use]
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// Schema for a single node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSchema {
    /// Node name (unique identifier)
    pub name: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Type of node
    pub node_type: NodeType,
    /// Fields this node reads from state
    pub input_fields: Vec<String>,
    /// Fields this node writes to state
    pub output_fields: Vec<String>,
    /// Position hint for visualization (x, y)
    pub position: Option<(f64, f64)>,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
}

impl NodeSchema {
    /// Create a basic node schema from just a name
    pub fn from_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            node_type: NodeType::Transform,
            input_fields: Vec::new(),
            output_fields: Vec::new(),
            position: None,
            attributes: HashMap::new(),
        }
    }

    /// Create a node schema from name and metadata
    pub fn from_metadata(name: impl Into<String>, metadata: &NodeMetadata) -> Self {
        Self {
            name: name.into(),
            description: metadata.description.clone(),
            node_type: metadata.node_type.clone(),
            input_fields: metadata.input_fields.clone(),
            output_fields: metadata.output_fields.clone(),
            position: metadata.position,
            attributes: metadata.attributes.clone(),
        }
    }
}

/// Type of edge in the graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Simple direct edge
    Direct,
    /// Conditional edge with routing logic
    Conditional,
    /// Parallel fan-out edge
    Parallel,
}

/// Schema for an edge in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSchema {
    /// Source node name
    pub from: String,
    /// Target node name (or "__end__" for terminal)
    pub to: String,
    /// Type of edge
    pub edge_type: EdgeType,
    /// Optional label for conditional edges
    pub label: Option<String>,
    /// For conditional edges, the possible targets
    pub conditional_targets: Option<Vec<String>>,
}

impl EdgeSchema {
    /// Create a direct edge
    pub fn direct(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            edge_type: EdgeType::Direct,
            label: None,
            conditional_targets: None,
        }
    }

    /// Create a conditional edge
    pub fn conditional(
        from: impl Into<String>,
        targets: Vec<impl Into<String>>,
        label: Option<String>,
    ) -> Self {
        let targets: Vec<String> = targets.into_iter().map(Into::into).collect();
        Self {
            from: from.into(),
            to: targets.first().cloned().unwrap_or_default(),
            edge_type: EdgeType::Conditional,
            label,
            conditional_targets: Some(targets),
        }
    }

    /// Create a parallel edge
    pub fn parallel(from: impl Into<String>, targets: Vec<impl Into<String>>) -> Self {
        let targets: Vec<String> = targets.into_iter().map(Into::into).collect();
        Self {
            from: from.into(),
            to: format!("[{}]", targets.join(", ")),
            edge_type: EdgeType::Parallel,
            label: None,
            conditional_targets: Some(targets),
        }
    }
}

/// Complete schema for a graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSchema {
    /// Graph name/identifier
    pub name: String,
    /// Graph version (for schema evolution)
    pub version: String,
    /// Human-readable description of the graph
    pub description: Option<String>,
    /// All nodes in the graph
    pub nodes: Vec<NodeSchema>,
    /// All edges in the graph
    pub edges: Vec<EdgeSchema>,
    /// Entry point node name
    pub entry_point: String,
    /// Name of the state type (for documentation)
    pub state_type: Option<String>,
    /// Timestamp when schema was exported
    pub exported_at: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl GraphSchema {
    /// Create a new graph schema
    pub fn new(name: impl Into<String>, entry_point: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "1.0.0".to_string(),
            description: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            entry_point: entry_point.into(),
            state_type: None,
            exported_at: Some(chrono::Utc::now().to_rfc3339()),
            metadata: HashMap::new(),
        }
    }

    /// Add a description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the state type name
    #[must_use]
    pub fn with_state_type(mut self, state_type: impl Into<String>) -> Self {
        self.state_type = Some(state_type.into());
        self
    }

    /// Add a node schema
    pub fn add_node(&mut self, node: NodeSchema) {
        self.nodes.push(node);
    }

    /// Add an edge schema
    pub fn add_edge(&mut self, edge: EdgeSchema) {
        self.edges.push(edge);
    }

    /// Get a node by name
    pub fn get_node(&self, name: &str) -> Option<&NodeSchema> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// Get all outgoing edges from a node
    pub fn get_outgoing_edges(&self, from: &str) -> Vec<&EdgeSchema> {
        self.edges.iter().filter(|e| e.from == from).collect()
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Convert to pretty JSON string
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_metadata_builder() {
        let metadata = NodeMetadata::new("Test node")
            .with_node_type(NodeType::Llm)
            .with_input_fields(vec!["input1", "input2"])
            .with_output_fields(vec!["output1"]);

        assert_eq!(metadata.description, Some("Test node".to_string()));
        assert_eq!(metadata.node_type, NodeType::Llm);
        assert_eq!(metadata.input_fields, vec!["input1", "input2"]);
        assert_eq!(metadata.output_fields, vec!["output1"]);
    }

    #[test]
    fn test_graph_schema_serialization() {
        let mut schema = GraphSchema::new("test-graph", "start")
            .with_description("A test graph")
            .with_state_type("TestState");

        schema.add_node(NodeSchema::from_name("start"));
        schema.add_node(NodeSchema::from_name("end"));
        schema.add_edge(EdgeSchema::direct("start", "end"));

        let json = schema.to_json().unwrap();
        assert!(json.contains("test-graph"));
        assert!(json.contains("start"));
    }

    #[test]
    fn test_node_type_display() {
        assert_eq!(NodeType::Llm.display_name(), "LLM");
        assert_eq!(NodeType::Tool.icon_hint(), "🔧");
    }
}
