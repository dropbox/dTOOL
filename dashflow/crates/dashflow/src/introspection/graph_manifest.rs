//! Graph Manifest Generation
//!
//! This module provides the [`GraphManifest`] type and related structures for
//! AI agents to understand their own graph structure.

use crate::schema::{
    EdgeSchema, EdgeType as SchemaEdgeType, GraphSchema, NodeSchema, NodeType as SchemaNodeType,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Graph manifest - complete structure for AI consumption.
///
/// This is the primary data structure for AI self-awareness. It contains
/// everything an AI needs to understand its own graph structure, including
/// nodes, edges, state schema, and runtime configurations.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::introspection::GraphManifest;
///
/// let manifest = graph.manifest();
///
/// // Check if I have a specific node
/// if manifest.nodes.contains_key("tool_executor") {
///     println!("I can execute tools!");
/// }
///
/// // Find my decision points
/// let decisions: Vec<_> = manifest.edges.iter()
///     .filter(|(_, edges)| edges.iter().any(|e| e.is_conditional))
///     .collect();
///
/// // Export as JSON for external consumption
/// let json = manifest.to_json()?;
/// ```
///
/// # Errors
///
/// - [`serde_json::Error`] - Returned by [`GraphManifest::to_json`] and
///   [`GraphManifest::from_json`] on
///   serialization/deserialization failure
///
/// # See Also
///
/// - [`NodeManifest`] - Details about individual nodes
/// - [`EdgeManifest`] - Details about edges and routing
/// - [`StateSchema`] - Schema of the graph state
/// - [`crate::ExecutionContext`] - Runtime execution context
/// - [`crate::ExecutionTrace`] - Execution history and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphManifest {
    /// Unique identifier for this graph (optional, user-provided)
    pub graph_id: Option<String>,
    /// Human-readable name of the graph
    pub graph_name: Option<String>,
    /// Entry point node name
    pub entry_point: String,
    /// All nodes in the graph with their metadata
    pub nodes: HashMap<String, NodeManifest>,
    /// All edges grouped by source node
    pub edges: HashMap<String, Vec<EdgeManifest>>,
    /// State schema information (if available)
    pub state_schema: Option<StateSchema>,
    /// Graph-level metadata
    pub metadata: GraphMetadata,
    /// Runtime-mutable node configurations (prompts, parameters, etc.)
    /// AI agents can use this to inspect and modify their own configurations.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub node_configs: HashMap<String, NodeConfig>,
}

impl GraphManifest {
    /// Create a new graph manifest builder
    #[must_use]
    pub fn builder() -> GraphManifestBuilder {
        GraphManifestBuilder::new()
    }

    /// Convert manifest to JSON string for AI consumption
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert manifest to compact JSON (smaller size)
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse manifest from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Get total number of nodes
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get total number of edges
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }

    /// Check if a specific node exists
    #[must_use]
    pub fn has_node(&self, name: &str) -> bool {
        self.nodes.contains_key(name)
    }

    /// Get all node names
    #[must_use]
    pub fn node_names(&self) -> Vec<&str> {
        self.nodes.keys().map(String::as_str).collect()
    }

    /// Get all decision points (nodes with conditional edges)
    #[must_use]
    pub fn decision_points(&self) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|(_, edges)| edges.iter().any(|e| e.is_conditional))
            .map(|(node, _)| node.as_str())
            .collect()
    }

    /// Get all parallel fan-out points
    #[must_use]
    pub fn parallel_points(&self) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|(_, edges)| edges.iter().any(|e| e.is_parallel))
            .map(|(node, _)| node.as_str())
            .collect()
    }

    /// Get all terminal nodes (nodes that end execution)
    #[must_use]
    pub fn terminal_nodes(&self) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|(_, edges)| edges.iter().any(|e| e.to == "__end__"))
            .map(|(node, _)| node.as_str())
            .collect()
    }

    /// Get nodes reachable from the entry point
    #[must_use]
    pub fn reachable_from_entry(&self) -> Vec<&str> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        queue.push_back(self.entry_point.as_str());
        visited.insert(self.entry_point.as_str());

        while let Some(current) = queue.pop_front() {
            if let Some(edges) = self.edges.get(current) {
                for edge in edges {
                    if edge.to != "__end__" && !visited.contains(edge.to.as_str()) {
                        visited.insert(edge.to.as_str());
                        queue.push_back(&edge.to);
                    }
                }
            }
        }

        visited.into_iter().collect()
    }

    /// Convert GraphManifest to GraphSchema for UI consumption.
    ///
    /// GraphSchema uses arrays (not maps) for nodes/edges, which the UI expects.
    /// This method also computes a stable schema_id for version tracking.
    #[must_use]
    pub fn to_schema(&self) -> GraphSchema {
        // Convert nodes
        let nodes: Vec<NodeSchema> = self
            .nodes
            .iter()
            .map(|(name, manifest)| {
                let node_type = match manifest.node_type {
                    NodeType::Function => SchemaNodeType::Transform,
                    NodeType::Agent => SchemaNodeType::Llm,
                    NodeType::ToolExecutor => SchemaNodeType::Tool,
                    NodeType::Subgraph => SchemaNodeType::Custom("subgraph".to_string()),
                    NodeType::Approval => SchemaNodeType::HumanInLoop,
                    NodeType::Custom(ref s) => SchemaNodeType::Custom(s.clone()),
                };

                // Extract input/output fields from metadata if present
                let input_fields = manifest
                    .metadata
                    .get("input_fields")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let output_fields = manifest
                    .metadata
                    .get("output_fields")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                NodeSchema {
                    name: name.clone(),
                    description: manifest.description.clone(),
                    node_type,
                    input_fields,
                    output_fields,
                    position: None,
                    attributes: HashMap::new(),
                }
            })
            .collect();

        // Convert edges (flatten from map to array)
        let edges: Vec<EdgeSchema> = self
            .edges
            .iter()
            .flat_map(|(from, edge_list)| {
                edge_list.iter().map(move |edge| {
                    let edge_type = if edge.is_parallel {
                        SchemaEdgeType::Parallel
                    } else if edge.is_conditional {
                        SchemaEdgeType::Conditional
                    } else {
                        SchemaEdgeType::Direct
                    };

                    EdgeSchema {
                        from: from.clone(),
                        to: edge.to.clone(),
                        edge_type,
                        label: edge.condition_label.clone(),
                        conditional_targets: None,
                    }
                })
            })
            .collect();

        let mut schema = GraphSchema::new(
            self.graph_name
                .clone()
                .unwrap_or_else(|| "unnamed".to_string()),
            &self.entry_point,
        );
        schema.nodes = nodes;
        schema.edges = edges;
        // GraphMetadata doesn't have description field; use custom field if present
        if let Some(desc) = self
            .metadata
            .custom
            .get("description")
            .and_then(|v| v.as_str())
        {
            schema.description = Some(desc.to_string());
        }

        schema
    }

    /// Compute a stable content-addressed schema ID (SHA256 hash).
    ///
    /// This ID changes when the graph structure changes (nodes, edges, entry point).
    /// Used for schema version tracking and mismatch detection.
    #[must_use]
    pub fn compute_schema_id(&self) -> String {
        // Create canonical representation: sorted nodes and edges
        let mut node_names: Vec<_> = self.nodes.keys().cloned().collect();
        node_names.sort();

        let mut edge_strs: Vec<String> = self
            .edges
            .iter()
            .flat_map(|(from, edges)| {
                edges
                    .iter()
                    .map(move |e| format!("{}->{}:{}", from, e.to, e.is_conditional))
            })
            .collect();
        edge_strs.sort();

        // Build canonical string
        let canonical = format!(
            "entry:{};nodes:{};edges:{}",
            self.entry_point,
            node_names.join(","),
            edge_strs.join(",")
        );

        // Compute SHA256 and return first 16 hex chars
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8]) // 16 hex chars = 64 bits
    }
}

/// Builder for creating graph manifests
#[derive(Debug, Default)]
pub struct GraphManifestBuilder {
    graph_id: Option<String>,
    graph_name: Option<String>,
    entry_point: Option<String>,
    nodes: HashMap<String, NodeManifest>,
    edges: HashMap<String, Vec<EdgeManifest>>,
    state_schema: Option<StateSchema>,
    metadata: GraphMetadata,
    node_configs: HashMap<String, NodeConfig>,
}

impl GraphManifestBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set graph ID
    #[must_use]
    pub fn graph_id(mut self, id: impl Into<String>) -> Self {
        self.graph_id = Some(id.into());
        self
    }

    /// Set graph name
    #[must_use]
    pub fn graph_name(mut self, name: impl Into<String>) -> Self {
        self.graph_name = Some(name.into());
        self
    }

    /// Set entry point
    #[must_use]
    pub fn entry_point(mut self, entry: impl Into<String>) -> Self {
        self.entry_point = Some(entry.into());
        self
    }

    /// Add a node
    #[must_use]
    pub fn add_node(mut self, name: impl Into<String>, manifest: NodeManifest) -> Self {
        self.nodes.insert(name.into(), manifest);
        self
    }

    /// Add an edge
    #[must_use]
    pub fn add_edge(mut self, from: impl Into<String>, edge: EdgeManifest) -> Self {
        self.edges.entry(from.into()).or_default().push(edge);
        self
    }

    /// Set state schema
    #[must_use]
    pub fn state_schema(mut self, schema: StateSchema) -> Self {
        self.state_schema = Some(schema);
        self
    }

    /// Set metadata
    #[must_use]
    pub fn metadata(mut self, metadata: GraphMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Add node configurations
    #[must_use]
    pub fn node_configs(mut self, configs: HashMap<String, NodeConfig>) -> Self {
        self.node_configs = configs;
        self
    }

    /// Add a single node configuration
    #[must_use]
    pub fn add_node_config(mut self, name: impl Into<String>, config: NodeConfig) -> Self {
        self.node_configs.insert(name.into(), config);
        self
    }

    /// Build the manifest
    ///
    /// # Errors
    ///
    /// Returns error if entry point is not set
    pub fn build(self) -> Result<GraphManifest, &'static str> {
        Ok(GraphManifest {
            graph_id: self.graph_id,
            graph_name: self.graph_name,
            entry_point: self.entry_point.ok_or("Entry point is required")?,
            nodes: self.nodes,
            edges: self.edges,
            state_schema: self.state_schema,
            metadata: self.metadata,
            node_configs: self.node_configs,
        })
    }
}

/// Node manifest - metadata about a single node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeManifest {
    /// Node name
    pub name: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Node type (function, agent, tool_executor, subgraph, etc.)
    pub node_type: NodeType,
    /// Tools available in this node (if any)
    pub tools_available: Vec<String>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl NodeManifest {
    /// Create a new node manifest
    #[must_use]
    pub fn new(name: impl Into<String>, node_type: NodeType) -> Self {
        Self {
            name: name.into(),
            description: None,
            node_type,
            tools_available: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add available tools
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools_available = tools;
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Node type classification
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Simple function node
    #[default]
    Function,
    /// LLM agent node
    Agent,
    /// Tool execution node
    ToolExecutor,
    /// Embedded subgraph
    Subgraph,
    /// Human-in-the-loop approval node
    Approval,
    /// Custom node type
    Custom(String),
}

/// Runtime configuration for a node, separate from node logic.
///
/// This struct enables AI agents to modify node parameters (prompts, temperature, etc.)
/// at runtime without rebuilding the graph structure. Each update increments the version
/// and recomputes the config hash for telemetry attribution.
///
/// # Example
///
/// ```rust
/// use dashflow::introspection::NodeConfig;
/// use serde_json::json;
///
/// let config = NodeConfig::new("researcher", "llm.chat")
///     .with_config(json!({
///         "system_prompt": "You are a helpful assistant.",
///         "temperature": 0.7,
///         "max_tokens": 1000
///     }))
///     .with_updated_by("human");
///
/// assert_eq!(config.version, 1);
/// assert!(config.config_hash.starts_with("sha256:"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Unique node identifier
    pub name: String,
    /// Node type (maps to factory in registry, e.g., "llm.chat", "tool.search")
    pub node_type: String,
    /// Runtime-mutable configuration (prompts, parameters, etc.)
    pub config: serde_json::Value,
    /// Auto-incremented on each update
    pub version: u64,
    /// SHA256 hash for deduplication/caching
    pub config_hash: String,
    /// Timestamp of last update
    pub updated_at: DateTime<Utc>,
    /// Attribution: "human", "self_improvement", "ab_test", etc.
    pub updated_by: Option<String>,
}

impl NodeConfig {
    /// Create a new node configuration with default values.
    ///
    /// Initializes with:
    /// - Empty JSON object config
    /// - Version 1
    /// - Current timestamp
    /// - Computed hash
    #[must_use]
    pub fn new(name: impl Into<String>, node_type: impl Into<String>) -> Self {
        let config = serde_json::Value::Object(serde_json::Map::new());
        let config_hash = Self::compute_hash(&config);
        Self {
            name: name.into(),
            node_type: node_type.into(),
            config,
            version: 1,
            config_hash,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    /// Create a node config with a specific configuration.
    #[must_use]
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config_hash = Self::compute_hash(&config);
        self.config = config;
        self
    }

    /// Set the updated_by attribution.
    #[must_use]
    pub fn with_updated_by(mut self, updated_by: impl Into<String>) -> Self {
        self.updated_by = Some(updated_by.into());
        self
    }

    /// Update the configuration, incrementing version and recomputing hash.
    ///
    /// Returns the previous configuration for potential rollback.
    pub fn update(
        &mut self,
        new_config: serde_json::Value,
        updated_by: Option<String>,
    ) -> serde_json::Value {
        let previous = std::mem::replace(&mut self.config, new_config);
        self.version += 1;
        self.config_hash = Self::compute_hash(&self.config);
        self.updated_at = Utc::now();
        self.updated_by = updated_by;
        previous
    }

    /// Compute SHA256 hash of the configuration.
    #[must_use]
    pub fn compute_hash(config: &serde_json::Value) -> String {
        let json_bytes = serde_json::to_vec(config).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&json_bytes);
        let result = hasher.finalize();
        format!("sha256:{:x}", result)
    }

    /// Get a specific field from the configuration.
    #[must_use]
    pub fn get_field(&self, key: &str) -> Option<&serde_json::Value> {
        self.config.get(key)
    }

    /// Get the system prompt if present.
    #[must_use]
    pub fn system_prompt(&self) -> Option<&str> {
        self.config.get("system_prompt").and_then(|v| v.as_str())
    }

    /// Get the temperature if present.
    #[must_use]
    pub fn temperature(&self) -> Option<f64> {
        self.config.get("temperature").and_then(|v| v.as_f64())
    }

    /// Get max tokens if present.
    #[must_use]
    pub fn max_tokens(&self) -> Option<i64> {
        self.config.get("max_tokens").and_then(|v| v.as_i64())
    }
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self::new("", "")
    }
}

impl PartialEq for NodeConfig {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.node_type == other.node_type && self.config == other.config
    }
}

/// Edge manifest - metadata about a connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeManifest {
    /// Source node name
    pub from: String,
    /// Target node name (or "__end__" for termination)
    pub to: String,
    /// Whether this is a conditional edge
    pub is_conditional: bool,
    /// Whether this is a parallel edge
    pub is_parallel: bool,
    /// Condition label (for conditional edges)
    pub condition_label: Option<String>,
    /// Description of the edge
    pub description: Option<String>,
}

impl EdgeManifest {
    /// Create a simple edge
    #[must_use]
    pub fn simple(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            is_conditional: false,
            is_parallel: false,
            condition_label: None,
            description: None,
        }
    }

    /// Create a conditional edge
    #[must_use]
    pub fn conditional(
        from: impl Into<String>,
        to: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            is_conditional: true,
            is_parallel: false,
            condition_label: Some(label.into()),
            description: None,
        }
    }

    /// Create a parallel edge
    #[must_use]
    pub fn parallel(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            is_conditional: false,
            is_parallel: true,
            condition_label: None,
            description: None,
        }
    }

    /// Add description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// State schema information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateSchema {
    /// Name of the state type
    pub type_name: String,
    /// Fields in the state
    pub fields: Vec<FieldSchema>,
    /// Description of the state
    pub description: Option<String>,
}

impl StateSchema {
    /// Create a new state schema
    #[must_use]
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            fields: Vec::new(),
            description: None,
        }
    }

    /// Add a field
    #[must_use]
    pub fn with_field(mut self, field: FieldSchema) -> Self {
        self.fields.push(field);
        self
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Field schema for state introspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    /// Field name
    pub name: String,
    /// Field type (as string)
    pub field_type: String,
    /// Whether the field is optional
    pub optional: bool,
    /// Description
    pub description: Option<String>,
}

impl FieldSchema {
    /// Create a new field schema
    #[must_use]
    pub fn new(name: impl Into<String>, field_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field_type: field_type.into(),
            optional: false,
            description: None,
        }
    }

    /// Mark as optional
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Add description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Graph-level metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphMetadata {
    /// Version of the graph definition
    pub version: Option<String>,
    /// Author/creator
    pub author: Option<String>,
    /// Creation timestamp
    pub created_at: Option<String>,
    /// Whether the graph has cycles
    pub has_cycles: bool,
    /// Whether the graph uses parallel edges
    pub has_parallel_edges: bool,
    /// Custom metadata
    pub custom: HashMap<String, serde_json::Value>,
}

impl GraphMetadata {
    /// Create new metadata
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set version
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set author
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Mark as having cycles
    #[must_use]
    pub fn with_cycles(mut self, has_cycles: bool) -> Self {
        self.has_cycles = has_cycles;
        self
    }

    /// Mark as having parallel edges
    #[must_use]
    pub fn with_parallel_edges(mut self, has_parallel: bool) -> Self {
        self.has_parallel_edges = has_parallel;
        self
    }

    /// Add custom metadata
    #[must_use]
    pub fn with_custom(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.custom.insert(key.into(), value);
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // =========================================================================
    // Helper functions for tests
    // =========================================================================

    fn create_simple_manifest() -> GraphManifest {
        GraphManifest::builder()
            .graph_id("test_graph")
            .graph_name("Test Graph")
            .entry_point("start")
            .add_node("start", NodeManifest::new("start", NodeType::Function))
            .add_node("process", NodeManifest::new("process", NodeType::Agent))
            .add_node(
                "end_node",
                NodeManifest::new("end_node", NodeType::Function),
            )
            .add_edge("start", EdgeManifest::simple("start", "process"))
            .add_edge("process", EdgeManifest::simple("process", "end_node"))
            .add_edge("end_node", EdgeManifest::simple("end_node", "__end__"))
            .build()
            .unwrap()
    }

    fn create_complex_manifest() -> GraphManifest {
        GraphManifest::builder()
            .graph_id("complex_graph")
            .graph_name("Complex Graph")
            .entry_point("entry")
            .add_node("entry", NodeManifest::new("entry", NodeType::Function))
            .add_node("router", NodeManifest::new("router", NodeType::Function))
            .add_node("path_a", NodeManifest::new("path_a", NodeType::Agent))
            .add_node("path_b", NodeManifest::new("path_b", NodeType::Agent))
            .add_node(
                "parallel_1",
                NodeManifest::new("parallel_1", NodeType::ToolExecutor),
            )
            .add_node(
                "parallel_2",
                NodeManifest::new("parallel_2", NodeType::ToolExecutor),
            )
            .add_node(
                "aggregator",
                NodeManifest::new("aggregator", NodeType::Function),
            )
            .add_edge("entry", EdgeManifest::simple("entry", "router"))
            .add_edge(
                "router",
                EdgeManifest::conditional("router", "path_a", "choice_a"),
            )
            .add_edge(
                "router",
                EdgeManifest::conditional("router", "path_b", "choice_b"),
            )
            .add_edge("path_a", EdgeManifest::parallel("path_a", "parallel_1"))
            .add_edge("path_a", EdgeManifest::parallel("path_a", "parallel_2"))
            .add_edge(
                "parallel_1",
                EdgeManifest::simple("parallel_1", "aggregator"),
            )
            .add_edge(
                "parallel_2",
                EdgeManifest::simple("parallel_2", "aggregator"),
            )
            .add_edge("path_b", EdgeManifest::simple("path_b", "aggregator"))
            .add_edge("aggregator", EdgeManifest::simple("aggregator", "__end__"))
            .build()
            .unwrap()
    }

    // =========================================================================
    // NodeType Tests
    // =========================================================================

    #[test]
    fn test_node_type_default() {
        assert_eq!(NodeType::default(), NodeType::Function);
    }

    #[test]
    fn test_node_type_variants() {
        let function = NodeType::Function;
        let agent = NodeType::Agent;
        let tool = NodeType::ToolExecutor;
        let subgraph = NodeType::Subgraph;
        let approval = NodeType::Approval;
        let custom = NodeType::Custom("my_type".to_string());

        assert_eq!(function, NodeType::Function);
        assert_eq!(agent, NodeType::Agent);
        assert_eq!(tool, NodeType::ToolExecutor);
        assert_eq!(subgraph, NodeType::Subgraph);
        assert_eq!(approval, NodeType::Approval);
        assert_eq!(custom, NodeType::Custom("my_type".to_string()));
    }

    #[test]
    fn test_node_type_clone_eq() {
        let original = NodeType::Agent;
        let cloned = original.clone();
        assert_eq!(original, cloned);

        let custom1 = NodeType::Custom("test".to_string());
        let custom2 = NodeType::Custom("test".to_string());
        assert_eq!(custom1, custom2);

        let custom3 = NodeType::Custom("other".to_string());
        assert_ne!(custom1, custom3);
    }

    #[test]
    fn test_node_type_serialize_deserialize() {
        let types = vec![
            NodeType::Function,
            NodeType::Agent,
            NodeType::ToolExecutor,
            NodeType::Subgraph,
            NodeType::Approval,
            NodeType::Custom("custom_type".to_string()),
        ];

        for nt in types {
            let json = serde_json::to_string(&nt).unwrap();
            let deserialized: NodeType = serde_json::from_str(&json).unwrap();
            assert_eq!(nt, deserialized);
        }
    }

    // =========================================================================
    // NodeManifest Tests
    // =========================================================================

    #[test]
    fn test_node_manifest_new() {
        let manifest = NodeManifest::new("test_node", NodeType::Agent);

        assert_eq!(manifest.name, "test_node");
        assert_eq!(manifest.node_type, NodeType::Agent);
        assert!(manifest.description.is_none());
        assert!(manifest.tools_available.is_empty());
        assert!(manifest.metadata.is_empty());
    }

    #[test]
    fn test_node_manifest_with_description() {
        let manifest =
            NodeManifest::new("test", NodeType::Function).with_description("A test node");

        assert_eq!(manifest.description.as_deref(), Some("A test node"));
    }

    #[test]
    fn test_node_manifest_with_tools() {
        let tools = vec!["search".to_string(), "calculate".to_string()];
        let manifest =
            NodeManifest::new("tool_node", NodeType::ToolExecutor).with_tools(tools.clone());

        assert_eq!(manifest.tools_available, tools);
    }

    #[test]
    fn test_node_manifest_with_metadata() {
        let manifest = NodeManifest::new("meta_node", NodeType::Function)
            .with_metadata("key1", json!("value1"))
            .with_metadata("key2", json!(42));

        assert_eq!(manifest.metadata.get("key1"), Some(&json!("value1")));
        assert_eq!(manifest.metadata.get("key2"), Some(&json!(42)));
    }

    #[test]
    fn test_node_manifest_serialize_deserialize() {
        let manifest = NodeManifest::new("complete_node", NodeType::Agent)
            .with_description("Complete test node")
            .with_tools(vec!["tool1".to_string()])
            .with_metadata("custom", json!({"nested": true}));

        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: NodeManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(manifest.name, deserialized.name);
        assert_eq!(manifest.node_type, deserialized.node_type);
        assert_eq!(manifest.description, deserialized.description);
        assert_eq!(manifest.tools_available, deserialized.tools_available);
        assert_eq!(manifest.metadata, deserialized.metadata);
    }

    // =========================================================================
    // EdgeManifest Tests
    // =========================================================================

    #[test]
    fn test_edge_manifest_simple() {
        let edge = EdgeManifest::simple("from_node", "to_node");

        assert_eq!(edge.from, "from_node");
        assert_eq!(edge.to, "to_node");
        assert!(!edge.is_conditional);
        assert!(!edge.is_parallel);
        assert!(edge.condition_label.is_none());
        assert!(edge.description.is_none());
    }

    #[test]
    fn test_edge_manifest_conditional() {
        let edge = EdgeManifest::conditional("router", "target", "when_true");

        assert_eq!(edge.from, "router");
        assert_eq!(edge.to, "target");
        assert!(edge.is_conditional);
        assert!(!edge.is_parallel);
        assert_eq!(edge.condition_label.as_deref(), Some("when_true"));
    }

    #[test]
    fn test_edge_manifest_parallel() {
        let edge = EdgeManifest::parallel("fan_out", "worker");

        assert_eq!(edge.from, "fan_out");
        assert_eq!(edge.to, "worker");
        assert!(!edge.is_conditional);
        assert!(edge.is_parallel);
        assert!(edge.condition_label.is_none());
    }

    #[test]
    fn test_edge_manifest_with_description() {
        let edge = EdgeManifest::simple("a", "b").with_description("Process the data");

        assert_eq!(edge.description.as_deref(), Some("Process the data"));
    }

    #[test]
    fn test_edge_manifest_serialize_deserialize() {
        let edges = vec![
            EdgeManifest::simple("a", "b"),
            EdgeManifest::conditional("a", "b", "label"),
            EdgeManifest::parallel("a", "b").with_description("desc"),
        ];

        for edge in edges {
            let json = serde_json::to_string(&edge).unwrap();
            let deserialized: EdgeManifest = serde_json::from_str(&json).unwrap();

            assert_eq!(edge.from, deserialized.from);
            assert_eq!(edge.to, deserialized.to);
            assert_eq!(edge.is_conditional, deserialized.is_conditional);
            assert_eq!(edge.is_parallel, deserialized.is_parallel);
            assert_eq!(edge.condition_label, deserialized.condition_label);
        }
    }

    // =========================================================================
    // FieldSchema Tests
    // =========================================================================

    #[test]
    fn test_field_schema_new() {
        let field = FieldSchema::new("user_id", "String");

        assert_eq!(field.name, "user_id");
        assert_eq!(field.field_type, "String");
        assert!(!field.optional);
        assert!(field.description.is_none());
    }

    #[test]
    fn test_field_schema_optional() {
        let field = FieldSchema::new("email", "String").optional();

        assert!(field.optional);
    }

    #[test]
    fn test_field_schema_with_description() {
        let field = FieldSchema::new("count", "i32").with_description("Number of items");

        assert_eq!(field.description.as_deref(), Some("Number of items"));
    }

    #[test]
    fn test_field_schema_chained_builders() {
        let field = FieldSchema::new("metadata", "HashMap<String, Value>")
            .optional()
            .with_description("Optional metadata map");

        assert_eq!(field.name, "metadata");
        assert_eq!(field.field_type, "HashMap<String, Value>");
        assert!(field.optional);
        assert!(field.description.is_some());
    }

    // =========================================================================
    // StateSchema Tests
    // =========================================================================

    #[test]
    fn test_state_schema_new() {
        let schema = StateSchema::new("MyState");

        assert_eq!(schema.type_name, "MyState");
        assert!(schema.fields.is_empty());
        assert!(schema.description.is_none());
    }

    #[test]
    fn test_state_schema_default() {
        let schema = StateSchema::default();

        assert_eq!(schema.type_name, "");
        assert!(schema.fields.is_empty());
    }

    #[test]
    fn test_state_schema_with_field() {
        let schema = StateSchema::new("ChatState")
            .with_field(FieldSchema::new("messages", "Vec<Message>"))
            .with_field(FieldSchema::new("user_id", "String").optional());

        assert_eq!(schema.fields.len(), 2);
        assert_eq!(schema.fields[0].name, "messages");
        assert_eq!(schema.fields[1].name, "user_id");
    }

    #[test]
    fn test_state_schema_with_description() {
        let schema =
            StateSchema::new("AgentState").with_description("State for the AI agent workflow");

        assert_eq!(
            schema.description.as_deref(),
            Some("State for the AI agent workflow")
        );
    }

    #[test]
    fn test_state_schema_serialize_deserialize() {
        let schema = StateSchema::new("TestState")
            .with_field(FieldSchema::new("field1", "String"))
            .with_field(FieldSchema::new("field2", "i32").optional())
            .with_description("Test state schema");

        let json = serde_json::to_string(&schema).unwrap();
        let deserialized: StateSchema = serde_json::from_str(&json).unwrap();

        assert_eq!(schema.type_name, deserialized.type_name);
        assert_eq!(schema.fields.len(), deserialized.fields.len());
        assert_eq!(schema.description, deserialized.description);
    }

    // =========================================================================
    // GraphMetadata Tests
    // =========================================================================

    #[test]
    fn test_graph_metadata_new() {
        let metadata = GraphMetadata::new();

        assert!(metadata.version.is_none());
        assert!(metadata.author.is_none());
        assert!(metadata.created_at.is_none());
        assert!(!metadata.has_cycles);
        assert!(!metadata.has_parallel_edges);
        assert!(metadata.custom.is_empty());
    }

    #[test]
    fn test_graph_metadata_default() {
        let metadata = GraphMetadata::default();

        assert!(!metadata.has_cycles);
        assert!(!metadata.has_parallel_edges);
    }

    #[test]
    fn test_graph_metadata_with_version() {
        let metadata = GraphMetadata::new().with_version("1.0.0");

        assert_eq!(metadata.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn test_graph_metadata_with_author() {
        let metadata = GraphMetadata::new().with_author("Test Author");

        assert_eq!(metadata.author.as_deref(), Some("Test Author"));
    }

    #[test]
    fn test_graph_metadata_with_cycles() {
        let metadata = GraphMetadata::new().with_cycles(true);

        assert!(metadata.has_cycles);
    }

    #[test]
    fn test_graph_metadata_with_parallel_edges() {
        let metadata = GraphMetadata::new().with_parallel_edges(true);

        assert!(metadata.has_parallel_edges);
    }

    #[test]
    fn test_graph_metadata_with_custom() {
        let metadata = GraphMetadata::new()
            .with_custom("key1", json!("value1"))
            .with_custom("key2", json!({"nested": true}));

        assert_eq!(metadata.custom.get("key1"), Some(&json!("value1")));
        assert_eq!(metadata.custom.get("key2"), Some(&json!({"nested": true})));
    }

    #[test]
    fn test_graph_metadata_chained_builders() {
        let metadata = GraphMetadata::new()
            .with_version("2.0.0")
            .with_author("Author")
            .with_cycles(true)
            .with_parallel_edges(true)
            .with_custom("custom_field", json!(123));

        assert_eq!(metadata.version.as_deref(), Some("2.0.0"));
        assert_eq!(metadata.author.as_deref(), Some("Author"));
        assert!(metadata.has_cycles);
        assert!(metadata.has_parallel_edges);
        assert_eq!(metadata.custom.get("custom_field"), Some(&json!(123)));
    }

    // =========================================================================
    // NodeConfig Tests
    // =========================================================================

    #[test]
    fn test_node_config_new() {
        let config = NodeConfig::new("my_node", "llm.chat");

        assert_eq!(config.name, "my_node");
        assert_eq!(config.node_type, "llm.chat");
        assert_eq!(config.version, 1);
        assert!(config.config_hash.starts_with("sha256:"));
        assert!(config.updated_by.is_none());
    }

    #[test]
    fn test_node_config_default() {
        let config = NodeConfig::default();

        assert_eq!(config.name, "");
        assert_eq!(config.node_type, "");
        assert_eq!(config.version, 1);
    }

    #[test]
    fn test_node_config_with_config() {
        let config = NodeConfig::new("node", "type").with_config(json!({
            "system_prompt": "You are helpful.",
            "temperature": 0.7
        }));

        assert_eq!(config.system_prompt(), Some("You are helpful."));
        assert_eq!(config.temperature(), Some(0.7));
    }

    #[test]
    fn test_node_config_with_updated_by() {
        let config = NodeConfig::new("node", "type").with_updated_by("human");

        assert_eq!(config.updated_by.as_deref(), Some("human"));
    }

    #[test]
    fn test_node_config_update() {
        let mut config = NodeConfig::new("node", "type").with_config(json!({"temperature": 0.5}));

        let previous = config.update(
            json!({"temperature": 0.9}),
            Some("self_improvement".to_string()),
        );

        assert_eq!(previous, json!({"temperature": 0.5}));
        assert_eq!(config.version, 2);
        assert_eq!(config.temperature(), Some(0.9));
        assert_eq!(config.updated_by.as_deref(), Some("self_improvement"));
    }

    #[test]
    fn test_node_config_compute_hash() {
        let config1 = json!({"key": "value"});
        let config2 = json!({"key": "value"});
        let config3 = json!({"key": "different"});

        let hash1 = NodeConfig::compute_hash(&config1);
        let hash2 = NodeConfig::compute_hash(&config2);
        let hash3 = NodeConfig::compute_hash(&config3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert!(hash1.starts_with("sha256:"));
    }

    #[test]
    fn test_node_config_get_field() {
        let config = NodeConfig::new("node", "type").with_config(json!({
            "field1": "value1",
            "field2": 42
        }));

        assert_eq!(config.get_field("field1"), Some(&json!("value1")));
        assert_eq!(config.get_field("field2"), Some(&json!(42)));
        assert_eq!(config.get_field("nonexistent"), None);
    }

    #[test]
    fn test_node_config_system_prompt() {
        let config_with_prompt =
            NodeConfig::new("node", "type").with_config(json!({"system_prompt": "Be helpful"}));

        let config_without_prompt =
            NodeConfig::new("node", "type").with_config(json!({"temperature": 0.5}));

        assert_eq!(config_with_prompt.system_prompt(), Some("Be helpful"));
        assert_eq!(config_without_prompt.system_prompt(), None);
    }

    #[test]
    fn test_node_config_temperature() {
        let config_with_temp =
            NodeConfig::new("node", "type").with_config(json!({"temperature": 0.8}));

        let config_without_temp =
            NodeConfig::new("node", "type").with_config(json!({"system_prompt": "test"}));

        assert_eq!(config_with_temp.temperature(), Some(0.8));
        assert_eq!(config_without_temp.temperature(), None);
    }

    #[test]
    fn test_node_config_max_tokens() {
        let config_with_tokens =
            NodeConfig::new("node", "type").with_config(json!({"max_tokens": 1000}));

        let config_without_tokens = NodeConfig::new("node", "type").with_config(json!({}));

        assert_eq!(config_with_tokens.max_tokens(), Some(1000));
        assert_eq!(config_without_tokens.max_tokens(), None);
    }

    #[test]
    fn test_node_config_partial_eq() {
        let config1 = NodeConfig::new("node", "type").with_config(json!({"key": "value"}));
        let config2 = NodeConfig::new("node", "type").with_config(json!({"key": "value"}));
        let config3 = NodeConfig::new("node", "type").with_config(json!({"key": "different"}));
        let config4 = NodeConfig::new("other_node", "type").with_config(json!({"key": "value"}));

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
        assert_ne!(config1, config4);
    }

    // =========================================================================
    // GraphManifestBuilder Tests
    // =========================================================================

    #[test]
    fn test_graph_manifest_builder_new() {
        let builder = GraphManifestBuilder::new();

        // Verify it's in initial state (will fail build without entry_point)
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_graph_manifest_builder_minimal() {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .build()
            .unwrap();

        assert_eq!(manifest.entry_point, "start");
        assert!(manifest.graph_id.is_none());
        assert!(manifest.graph_name.is_none());
        assert!(manifest.nodes.is_empty());
        assert!(manifest.edges.is_empty());
    }

    #[test]
    fn test_graph_manifest_builder_graph_id() {
        let manifest = GraphManifest::builder()
            .graph_id("my_graph_123")
            .entry_point("start")
            .build()
            .unwrap();

        assert_eq!(manifest.graph_id.as_deref(), Some("my_graph_123"));
    }

    #[test]
    fn test_graph_manifest_builder_graph_name() {
        let manifest = GraphManifest::builder()
            .graph_name("My Graph")
            .entry_point("start")
            .build()
            .unwrap();

        assert_eq!(manifest.graph_name.as_deref(), Some("My Graph"));
    }

    #[test]
    fn test_graph_manifest_builder_add_node() {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .add_node("start", NodeManifest::new("start", NodeType::Function))
            .add_node("end", NodeManifest::new("end", NodeType::Function))
            .build()
            .unwrap();

        assert_eq!(manifest.nodes.len(), 2);
        assert!(manifest.nodes.contains_key("start"));
        assert!(manifest.nodes.contains_key("end"));
    }

    #[test]
    fn test_graph_manifest_builder_add_edge() {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .add_edge("start", EdgeManifest::simple("start", "middle"))
            .add_edge("start", EdgeManifest::simple("start", "other"))
            .add_edge("middle", EdgeManifest::simple("middle", "__end__"))
            .build()
            .unwrap();

        assert_eq!(manifest.edges.len(), 2);
        assert_eq!(manifest.edges.get("start").unwrap().len(), 2);
        assert_eq!(manifest.edges.get("middle").unwrap().len(), 1);
    }

    #[test]
    fn test_graph_manifest_builder_state_schema() {
        let schema = StateSchema::new("TestState").with_field(FieldSchema::new("field", "String"));

        let manifest = GraphManifest::builder()
            .entry_point("start")
            .state_schema(schema)
            .build()
            .unwrap();

        assert!(manifest.state_schema.is_some());
        assert_eq!(manifest.state_schema.unwrap().type_name, "TestState");
    }

    #[test]
    fn test_graph_manifest_builder_metadata() {
        let metadata = GraphMetadata::new()
            .with_version("1.0.0")
            .with_author("Test");

        let manifest = GraphManifest::builder()
            .entry_point("start")
            .metadata(metadata)
            .build()
            .unwrap();

        assert_eq!(manifest.metadata.version.as_deref(), Some("1.0.0"));
        assert_eq!(manifest.metadata.author.as_deref(), Some("Test"));
    }

    #[test]
    fn test_graph_manifest_builder_node_configs() {
        let mut configs = HashMap::new();
        configs.insert("node1".to_string(), NodeConfig::new("node1", "type1"));
        configs.insert("node2".to_string(), NodeConfig::new("node2", "type2"));

        let manifest = GraphManifest::builder()
            .entry_point("start")
            .node_configs(configs)
            .build()
            .unwrap();

        assert_eq!(manifest.node_configs.len(), 2);
    }

    #[test]
    fn test_graph_manifest_builder_add_node_config() {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .add_node_config("node1", NodeConfig::new("node1", "llm.chat"))
            .add_node_config("node2", NodeConfig::new("node2", "tool.search"))
            .build()
            .unwrap();

        assert_eq!(manifest.node_configs.len(), 2);
        assert!(manifest.node_configs.contains_key("node1"));
        assert!(manifest.node_configs.contains_key("node2"));
    }

    #[test]
    fn test_graph_manifest_builder_build_error_no_entry() {
        let result = GraphManifest::builder().build();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Entry point is required");
    }

    // =========================================================================
    // GraphManifest Tests
    // =========================================================================

    #[test]
    fn test_graph_manifest_builder_method() {
        let builder = GraphManifest::builder();
        // Just verify it returns a builder
        let _ = builder.entry_point("test");
    }

    #[test]
    fn test_graph_manifest_to_json() {
        let manifest = create_simple_manifest();
        let json = manifest.to_json().unwrap();

        assert!(json.contains("test_graph"));
        assert!(json.contains("Test Graph"));
        assert!(json.contains("start"));
    }

    #[test]
    fn test_graph_manifest_to_json_compact() {
        let manifest = create_simple_manifest();
        let pretty = manifest.to_json().unwrap();
        let compact = manifest.to_json_compact().unwrap();

        // Compact should be smaller (no whitespace)
        assert!(compact.len() < pretty.len());
        // But contain the same data
        assert!(compact.contains("test_graph"));
    }

    #[test]
    fn test_graph_manifest_from_json() {
        let manifest = create_simple_manifest();
        let json = manifest.to_json().unwrap();
        let parsed = GraphManifest::from_json(&json).unwrap();

        assert_eq!(manifest.graph_id, parsed.graph_id);
        assert_eq!(manifest.graph_name, parsed.graph_name);
        assert_eq!(manifest.entry_point, parsed.entry_point);
        assert_eq!(manifest.nodes.len(), parsed.nodes.len());
        assert_eq!(manifest.edges.len(), parsed.edges.len());
    }

    #[test]
    fn test_graph_manifest_node_count() {
        let manifest = create_simple_manifest();

        assert_eq!(manifest.node_count(), 3);
    }

    #[test]
    fn test_graph_manifest_edge_count() {
        let manifest = create_simple_manifest();

        assert_eq!(manifest.edge_count(), 3);
    }

    #[test]
    fn test_graph_manifest_has_node() {
        let manifest = create_simple_manifest();

        assert!(manifest.has_node("start"));
        assert!(manifest.has_node("process"));
        assert!(!manifest.has_node("nonexistent"));
    }

    #[test]
    fn test_graph_manifest_node_names() {
        let manifest = create_simple_manifest();
        let names = manifest.node_names();

        assert_eq!(names.len(), 3);
        assert!(names.contains(&"start"));
        assert!(names.contains(&"process"));
        assert!(names.contains(&"end_node"));
    }

    #[test]
    fn test_graph_manifest_decision_points() {
        let manifest = create_complex_manifest();
        let decision_points = manifest.decision_points();

        assert!(!decision_points.is_empty());
        assert!(decision_points.contains(&"router"));
    }

    #[test]
    fn test_graph_manifest_decision_points_empty() {
        let manifest = create_simple_manifest();
        let decision_points = manifest.decision_points();

        assert!(decision_points.is_empty());
    }

    #[test]
    fn test_graph_manifest_parallel_points() {
        let manifest = create_complex_manifest();
        let parallel_points = manifest.parallel_points();

        assert!(!parallel_points.is_empty());
        assert!(parallel_points.contains(&"path_a"));
    }

    #[test]
    fn test_graph_manifest_parallel_points_empty() {
        let manifest = create_simple_manifest();
        let parallel_points = manifest.parallel_points();

        assert!(parallel_points.is_empty());
    }

    #[test]
    fn test_graph_manifest_terminal_nodes() {
        let manifest = create_simple_manifest();
        let terminal = manifest.terminal_nodes();

        assert!(!terminal.is_empty());
        assert!(terminal.contains(&"end_node"));
    }

    #[test]
    fn test_graph_manifest_terminal_nodes_complex() {
        let manifest = create_complex_manifest();
        let terminal = manifest.terminal_nodes();

        assert!(terminal.contains(&"aggregator"));
    }

    #[test]
    fn test_graph_manifest_reachable_from_entry() {
        let manifest = create_simple_manifest();
        let reachable = manifest.reachable_from_entry();

        assert_eq!(reachable.len(), 3);
        assert!(reachable.contains(&"start"));
        assert!(reachable.contains(&"process"));
        assert!(reachable.contains(&"end_node"));
    }

    #[test]
    fn test_graph_manifest_reachable_from_entry_complex() {
        let manifest = create_complex_manifest();
        let reachable = manifest.reachable_from_entry();

        // All nodes should be reachable from entry
        assert!(reachable.contains(&"entry"));
        assert!(reachable.contains(&"router"));
        assert!(reachable.contains(&"path_a"));
        assert!(reachable.contains(&"path_b"));
        assert!(reachable.contains(&"parallel_1"));
        assert!(reachable.contains(&"parallel_2"));
        assert!(reachable.contains(&"aggregator"));
    }

    #[test]
    fn test_graph_manifest_compute_schema_id() {
        let manifest1 = create_simple_manifest();
        let manifest2 = create_simple_manifest();

        let id1 = manifest1.compute_schema_id();
        let id2 = manifest2.compute_schema_id();

        // Same structure = same ID
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16); // 16 hex chars
    }

    #[test]
    fn test_graph_manifest_compute_schema_id_different() {
        let manifest1 = create_simple_manifest();
        let manifest2 = create_complex_manifest();

        let id1 = manifest1.compute_schema_id();
        let id2 = manifest2.compute_schema_id();

        // Different structure = different ID
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_graph_manifest_to_schema() {
        let manifest = GraphManifest::builder()
            .graph_name("Test Graph")
            .entry_point("start")
            .add_node(
                "start",
                NodeManifest::new("start", NodeType::Function).with_description("Start node"),
            )
            .add_node("agent", NodeManifest::new("agent", NodeType::Agent))
            .add_node("tool", NodeManifest::new("tool", NodeType::ToolExecutor))
            .add_edge("start", EdgeManifest::simple("start", "agent"))
            .add_edge(
                "agent",
                EdgeManifest::conditional("agent", "tool", "use_tool"),
            )
            .add_edge("tool", EdgeManifest::simple("tool", "__end__"))
            .metadata(GraphMetadata::new().with_custom("description", json!("A test graph")))
            .build()
            .unwrap();

        let schema = manifest.to_schema();

        assert_eq!(schema.name, "Test Graph");
        assert_eq!(schema.entry_point, "start");
        assert_eq!(schema.nodes.len(), 3);
        assert_eq!(schema.edges.len(), 3);
        assert_eq!(schema.description.as_deref(), Some("A test graph"));
    }

    #[test]
    fn test_graph_manifest_to_schema_node_types() {
        let manifest = GraphManifest::builder()
            .entry_point("start")
            .add_node("func", NodeManifest::new("func", NodeType::Function))
            .add_node("agent", NodeManifest::new("agent", NodeType::Agent))
            .add_node("tool", NodeManifest::new("tool", NodeType::ToolExecutor))
            .add_node(
                "subgraph",
                NodeManifest::new("subgraph", NodeType::Subgraph),
            )
            .add_node(
                "approval",
                NodeManifest::new("approval", NodeType::Approval),
            )
            .add_node(
                "custom",
                NodeManifest::new("custom", NodeType::Custom("my_type".to_string())),
            )
            .build()
            .unwrap();

        let schema = manifest.to_schema();

        // Verify node types are converted correctly
        let node_types: HashMap<_, _> = schema
            .nodes
            .iter()
            .map(|n| (n.name.as_str(), &n.node_type))
            .collect();

        assert_eq!(*node_types.get("func").unwrap(), &SchemaNodeType::Transform);
        assert_eq!(*node_types.get("agent").unwrap(), &SchemaNodeType::Llm);
        assert_eq!(*node_types.get("tool").unwrap(), &SchemaNodeType::Tool);
        assert_eq!(
            *node_types.get("subgraph").unwrap(),
            &SchemaNodeType::Custom("subgraph".to_string())
        );
        assert_eq!(
            *node_types.get("approval").unwrap(),
            &SchemaNodeType::HumanInLoop
        );
        assert_eq!(
            *node_types.get("custom").unwrap(),
            &SchemaNodeType::Custom("my_type".to_string())
        );
    }

    #[test]
    fn test_graph_manifest_to_schema_edge_types() {
        let manifest = GraphManifest::builder()
            .entry_point("a")
            .add_edge("a", EdgeManifest::simple("a", "b"))
            .add_edge("b", EdgeManifest::conditional("b", "c", "cond"))
            .add_edge("c", EdgeManifest::parallel("c", "d"))
            .build()
            .unwrap();

        let schema = manifest.to_schema();

        // Find edges by their targets
        let edge_to_b = schema.edges.iter().find(|e| e.to == "b").unwrap();
        let edge_to_c = schema.edges.iter().find(|e| e.to == "c").unwrap();
        let edge_to_d = schema.edges.iter().find(|e| e.to == "d").unwrap();

        assert_eq!(edge_to_b.edge_type, SchemaEdgeType::Direct);
        assert_eq!(edge_to_c.edge_type, SchemaEdgeType::Conditional);
        assert_eq!(edge_to_d.edge_type, SchemaEdgeType::Parallel);
    }

    #[test]
    fn test_graph_manifest_serialize_deserialize_complete() {
        let manifest = GraphManifest::builder()
            .graph_id("complete_test")
            .graph_name("Complete Test Graph")
            .entry_point("start")
            .add_node(
                "start",
                NodeManifest::new("start", NodeType::Function)
                    .with_description("Entry point")
                    .with_tools(vec!["tool1".to_string()])
                    .with_metadata("key", json!("value")),
            )
            .add_edge("start", EdgeManifest::simple("start", "__end__"))
            .state_schema(
                StateSchema::new("TestState")
                    .with_field(FieldSchema::new("field", "String"))
                    .with_description("Test state"),
            )
            .metadata(
                GraphMetadata::new()
                    .with_version("1.0.0")
                    .with_author("Test Author")
                    .with_cycles(false)
                    .with_parallel_edges(false)
                    .with_custom("custom", json!(true)),
            )
            .add_node_config(
                "start",
                NodeConfig::new("start", "function")
                    .with_config(json!({"param": "value"}))
                    .with_updated_by("test"),
            )
            .build()
            .unwrap();

        let json = manifest.to_json().unwrap();
        let parsed = GraphManifest::from_json(&json).unwrap();

        assert_eq!(manifest.graph_id, parsed.graph_id);
        assert_eq!(manifest.graph_name, parsed.graph_name);
        assert_eq!(manifest.entry_point, parsed.entry_point);
        assert_eq!(manifest.nodes.len(), parsed.nodes.len());
        assert_eq!(manifest.edges.len(), parsed.edges.len());
        assert!(parsed.state_schema.is_some());
        assert_eq!(manifest.metadata.version, parsed.metadata.version);
        assert_eq!(manifest.node_configs.len(), parsed.node_configs.len());
    }
}
