// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! MCP Self-Documentation Response Types
//!
//! All response types follow the DashFlow MCP Self-Documentation Protocol:
//! - schema_version: Enables forward compatibility and schema evolution
//! - protocol: Identifies the protocol for interoperability
//! - Consistent field naming (snake_case)
//! - Required vs optional fields clearly documented
//!
//! JSON Schema: <https://dashflow.dev/schemas/mcp-self-doc/1.0>

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// MCP Self-Documentation Server Response Types
// ============================================================================

/// Response for `/mcp/about` endpoint.
///
/// Provides high-level application information for quick understanding.
/// This is the "tl;dr" level of the progressive disclosure pattern.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "protocol", "name", "version", "description", "capabilities", "dashflow_version"],
///   "properties": {
///     "schema_version": { "type": "string", "pattern": "^\\d+\\.\\d+\\.\\d+$" },
///     "protocol": { "type": "string" },
///     "name": { "type": "string" },
///     "version": { "type": "string" },
///     "description": { "type": "string" },
///     "capabilities": { "type": "array", "items": { "type": "string" } },
///     "dashflow_version": { "type": "string" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAboutResponse {
    /// Schema version for forward compatibility (e.g., "1.0.0")
    pub schema_version: String,
    /// MCP protocol version (e.g., "dashflow-self-doc/1.0")
    pub protocol: String,
    /// Application name
    pub name: String,
    /// Application version (semver recommended)
    pub version: String,
    /// Human-readable application description
    pub description: String,
    /// High-level capabilities list (tool names or feature keywords)
    pub capabilities: Vec<String>,
    /// DashFlow framework version
    pub dashflow_version: String,
}

/// Response for `/mcp/capabilities` endpoint.
///
/// Provides detailed capability information including tools and nodes.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "tools", "nodes", "features"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "tools": { "type": "array", "items": { "$ref": "#/$defs/McpToolInfo" } },
///     "nodes": { "type": "array", "items": { "$ref": "#/$defs/McpNodeInfo" } },
///     "features": { "type": "array", "items": { "type": "string" } }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCapabilitiesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Available tools with their schemas
    pub tools: Vec<McpToolInfo>,
    /// Node capabilities
    pub nodes: Vec<McpNodeInfo>,
    /// DashFlow features used (e.g., "checkpointing", "streaming")
    pub features: Vec<String>,
}

/// Tool information following MCP tool schema conventions.
///
/// Compatible with Model Context Protocol tool definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// Tool name (unique identifier, snake_case recommended)
    pub name: String,
    /// Human-readable tool description
    pub description: String,
    /// JSON Schema for tool input parameters
    pub input_schema: serde_json::Value,
}

/// Node information for MCP discovery.
///
/// Describes a processing unit in the DashFlow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNodeInfo {
    /// Node name (unique within graph)
    pub name: String,
    /// Node type: "function", "agent", "tool", "subgraph", "conditional"
    pub node_type: String,
    /// Human-readable node description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tools available within this node
    pub tools: Vec<String>,
    /// Node version (semver format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Node metadata (category, tags, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Response for `/mcp/architecture` endpoint.
///
/// Provides graph structure and execution flow information.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "graph", "dashflow_features", "execution_flow"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "graph": { "$ref": "#/$defs/McpGraphInfo" },
///     "dashflow_features": { "type": "array", "items": { "type": "string" } },
///     "execution_flow": { "type": "string" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpArchitectureResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Graph structure information
    pub graph: McpGraphInfo,
    /// DashFlow features used with descriptions
    pub dashflow_features: Vec<String>,
    /// Human-readable execution flow description
    pub execution_flow: String,
}

/// Graph structure information.
///
/// Describes the topology of a DashFlow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpGraphInfo {
    /// Entry point node name
    pub entry_point: String,
    /// Total number of nodes
    pub node_count: usize,
    /// Total number of edges
    pub edge_count: usize,
    /// Terminal node names (execution endpoints)
    pub terminal_nodes: Vec<String>,
    /// Decision point node names (conditional routing)
    pub decision_points: Vec<String>,
    /// Whether the graph contains cycles
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_cycles: Option<bool>,
    /// Whether the graph has parallel execution paths
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_parallel_paths: Option<bool>,
}

/// Response for `/mcp/implementation` endpoint.
///
/// Provides detailed implementation information for debugging and versioning.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "node_versions", "dashflow_version", "dependencies"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "node_versions": { "type": "object", "additionalProperties": { "type": "string" } },
///     "dashflow_version": { "type": "string" },
///     "dependencies": { "type": "array", "items": { "$ref": "#/$defs/McpDependencyInfo" } }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpImplementationResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Node name to version mapping
    pub node_versions: HashMap<String, String>,
    /// DashFlow framework version
    pub dashflow_version: String,
    /// Dependencies (DashFlow crates and external)
    pub dependencies: Vec<McpDependencyInfo>,
}

/// Dependency information for MCP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpDependencyInfo {
    /// Dependency name
    pub name: String,
    /// Dependency version
    pub version: String,
    /// Purpose
    pub purpose: String,
    /// Is DashFlow crate
    pub is_dashflow: bool,
}

/// MCP query request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpQueryRequest {
    /// Natural language question
    pub question: String,
}

/// MCP query response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpQueryResponse {
    /// Answer to the question
    pub answer: String,
    /// Source references
    pub sources: Vec<String>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

/// Response for `/mcp/nodes` endpoint.
///
/// Lists all nodes in the DashFlow graph with summary information.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "nodes", "total_count"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "nodes": { "type": "array", "items": { "$ref": "#/$defs/McpNodeSummary" } },
///     "total_count": { "type": "integer" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNodesListResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// List of node summaries
    pub nodes: Vec<McpNodeSummary>,
    /// Total number of nodes
    pub total_count: usize,
}

/// Summary information for a node in the nodes list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNodeSummary {
    /// Node name (unique identifier)
    pub name: String,
    /// Node type: "function", "agent", "tool", "subgraph", "conditional"
    pub node_type: String,
    /// Human-readable node description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this is the entry point
    pub is_entry_point: bool,
    /// Whether this is a terminal node
    pub is_terminal: bool,
}

/// Response for `/mcp/nodes/:name` endpoint.
///
/// Detailed information about a specific node.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "name", "node_type"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "name": { "type": "string" },
///     "node_type": { "type": "string" },
///     "description": { "type": "string" },
///     "version": { "type": "string" },
///     "incoming_edges": { "type": "array", "items": { "type": "string" } },
///     "outgoing_edges": { "type": "array", "items": { "type": "string" } },
///     "tools": { "type": "array", "items": { "type": "string" } },
///     "metadata": { "type": "object" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNodeDetailResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Node name
    pub name: String,
    /// Node type
    pub node_type: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Node version (semver)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Nodes that connect to this node
    pub incoming_edges: Vec<String>,
    /// Nodes this node connects to
    pub outgoing_edges: Vec<String>,
    /// Tools available in this node
    pub tools: Vec<String>,
    /// Whether this is the entry point
    pub is_entry_point: bool,
    /// Whether this is a terminal node
    pub is_terminal: bool,
    /// Custom metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Response for `/mcp/features` endpoint.
///
/// Lists DashFlow features used by this application.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "features", "total_count"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "features": { "type": "array", "items": { "$ref": "#/$defs/McpFeatureInfo" } },
///     "total_count": { "type": "integer" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpFeaturesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// List of features
    pub features: Vec<McpFeatureInfo>,
    /// Total number of features
    pub total_count: usize,
}

/// Information about a DashFlow feature configured for this application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpFeatureInfo {
    /// Feature name
    pub name: String,
    /// Feature description
    pub description: String,
    /// Whether this feature is enabled for this application
    pub enabled: bool,
    /// Configuration details (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<serde_json::Value>,
    /// Method to opt-out of this feature (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opt_out_method: Option<String>,
}

/// Response for `/mcp/dependencies` endpoint.
///
/// Lists all dependencies (DashFlow and external) used by this application.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "dependencies", "total_count"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "dependencies": { "type": "array", "items": { "$ref": "#/$defs/McpDependencyInfo" } },
///     "total_count": { "type": "integer" },
///     "dashflow_count": { "type": "integer" },
///     "external_count": { "type": "integer" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpDependenciesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// List of dependencies
    pub dependencies: Vec<McpDependencyInfo>,
    /// Total number of dependencies
    pub total_count: usize,
    /// Number of DashFlow crate dependencies
    pub dashflow_count: usize,
    /// Number of external dependencies
    pub external_count: usize,
}

/// Response for `/mcp/edges` endpoint.
///
/// Lists all edges (connections) in the DashFlow graph.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "edges", "total_count"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "edges": { "type": "array", "items": { "$ref": "#/$defs/McpEdgeInfo" } },
///     "total_count": { "type": "integer" },
///     "conditional_count": { "type": "integer" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpEdgesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// List of edges
    pub edges: Vec<McpEdgeInfo>,
    /// Total number of edges
    pub total_count: usize,
    /// Number of conditional edges
    pub conditional_count: usize,
}

/// Information about an edge (connection) in the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpEdgeInfo {
    /// Source node name
    pub from: String,
    /// Target node name
    pub to: String,
    /// Whether this is a conditional edge
    pub is_conditional: bool,
    /// Condition label (if conditional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

// ============================================================================
// App Introspection Enhancement Response Types
// ============================================================================

/// Response for `/mcp/tools` endpoint.
///
/// Lists all tools available to this application.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "tools", "total_count"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "tools": { "type": "array", "items": { "$ref": "#/$defs/McpToolInfo" } },
///     "total_count": { "type": "integer" },
///     "categories": { "type": "array", "items": { "type": "string" } }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// List of tools available to this application
    pub tools: Vec<McpAppToolInfo>,
    /// Total number of tools
    pub total_count: usize,
    /// Unique tool categories
    pub categories: Vec<String>,
}

/// Extended tool information for app introspection.
///
/// Unlike `McpToolInfo` (which follows MCP protocol conventions),
/// this struct provides richer metadata for AI agents to understand
/// the tools available in this specific application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAppToolInfo {
    /// Tool name (identifier)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Category for grouping (e.g., "web", "filesystem", "code")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Parameters the tool accepts
    pub parameters: Vec<McpToolParameter>,
    /// Return type description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
    /// Whether the tool has side effects
    pub has_side_effects: bool,
    /// Whether the tool requires confirmation before execution
    pub requires_confirmation: bool,
}

/// Information about a tool parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type (e.g., "string", "number", "boolean")
    pub param_type: String,
    /// Parameter description
    pub description: String,
    /// Whether the parameter is required
    pub required: bool,
    /// Default value (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

/// Response for `/mcp/state-schema` endpoint.
///
/// Describes the state schema used by this application.
///
/// # JSON Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "required": ["schema_version", "has_schema"],
///   "properties": {
///     "schema_version": { "type": "string" },
///     "has_schema": { "type": "boolean" },
///     "state_type_name": { "type": "string" },
///     "description": { "type": "string" },
///     "fields": { "type": "array", "items": { "$ref": "#/$defs/McpFieldInfo" } }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStateSchemaResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Whether a state schema is defined
    pub has_schema: bool,
    /// Name of the state type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_type_name: Option<String>,
    /// Description of the state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Fields in the state schema
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fields: Vec<McpFieldInfo>,
}

/// Information about a field in the state schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpFieldInfo {
    /// Field name
    pub name: String,
    /// Field type (as string)
    pub field_type: String,
    /// Whether the field is optional
    pub optional: bool,
    /// Description of the field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ============================================================================
// Platform Introspection Response Types
// ============================================================================

/// Response for `/mcp/platform/version` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformVersionResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// DashFlow version
    pub version: String,
    /// Minimum supported Rust version
    pub rust_version: String,
    /// Cargo features enabled at compile time
    pub features_enabled: Vec<String>,
    /// Build timestamp (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_timestamp: Option<String>,
}

/// Response for `/mcp/platform/features` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformFeaturesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Available features
    pub features: Vec<McpPlatformFeatureInfo>,
    /// Total count
    pub total_count: usize,
}

/// Information about a platform feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformFeatureInfo {
    /// Feature name
    pub name: String,
    /// Feature description
    pub description: String,
    /// Whether enabled by default
    pub default_enabled: bool,
    /// Opt-out method if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opt_out_method: Option<String>,
}

/// Response for `/mcp/platform/node-types` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformNodeTypesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Supported node types
    pub node_types: Vec<McpPlatformNodeTypeInfo>,
    /// Total count
    pub total_count: usize,
}

/// Information about a supported node type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformNodeTypeInfo {
    /// Node type name
    pub name: String,
    /// Description
    pub description: String,
    /// Example usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// Whether built-in
    pub is_builtin: bool,
}

/// Response for `/mcp/platform/edge-types` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformEdgeTypesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Supported edge types
    pub edge_types: Vec<McpPlatformEdgeTypeInfo>,
    /// Total count
    pub total_count: usize,
}

/// Information about a supported edge type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformEdgeTypeInfo {
    /// Edge type name
    pub name: String,
    /// Description
    pub description: String,
    /// Example usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

/// Response for `/mcp/platform/templates` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformTemplatesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Built-in templates
    pub templates: Vec<McpPlatformTemplateInfo>,
    /// Total count
    pub total_count: usize,
}

/// Information about a built-in template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformTemplateInfo {
    /// Template name
    pub name: String,
    /// Description
    pub description: String,
    /// Use cases
    pub use_cases: Vec<String>,
    /// Example usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

/// Response for `/mcp/platform/states` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformStatesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// State implementations
    pub states: Vec<McpPlatformStateInfo>,
    /// Total count
    pub total_count: usize,
}

/// Information about a state implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformStateInfo {
    /// State type name
    pub name: String,
    /// Description
    pub description: String,
    /// Whether built-in
    pub is_builtin: bool,
}

// =============================================================================
// Hypothesis Dashboard Types
// =============================================================================

/// Response for `/mcp/hypotheses` endpoint.
///
/// Provides a dashboard view of the hypothesis tracking system for AI learning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHypothesesResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Overall accuracy statistics
    pub accuracy: McpHypothesisAccuracy,
    /// Active hypotheses (awaiting evaluation)
    pub active_hypotheses: Vec<McpHypothesisInfo>,
    /// Recently evaluated hypotheses
    pub recent_evaluations: Vec<McpHypothesisInfo>,
    /// Learning insights derived from hypothesis patterns
    pub insights: Vec<String>,
}

/// Accuracy statistics for hypothesis tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHypothesisAccuracy {
    /// Overall accuracy rate (0.0 - 1.0)
    pub overall: f64,
    /// Total hypotheses evaluated
    pub total_evaluated: usize,
    /// Total correct predictions
    pub correct: usize,
    /// Total incorrect predictions
    pub incorrect: usize,
    /// Active hypotheses count
    pub active_count: usize,
    /// Accuracy breakdown by source type
    pub by_source: Vec<McpSourceAccuracy>,
}

/// Accuracy for a specific hypothesis source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSourceAccuracy {
    /// Source type (e.g., "Capability Gap", "Execution Plan")
    pub source: String,
    /// Accuracy for this source (0.0 - 1.0)
    pub accuracy: f64,
    /// Total evaluated from this source
    pub total: usize,
    /// Correct from this source
    pub correct: usize,
}

/// Information about a single hypothesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHypothesisInfo {
    /// Unique identifier
    pub id: String,
    /// The hypothesis statement
    pub statement: String,
    /// Source type
    pub source: String,
    /// Current status
    pub status: String,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Whether evaluated
    pub evaluated: bool,
    /// Outcome if evaluated
    pub outcome: Option<McpHypothesisOutcome>,
}

/// Outcome of a hypothesis evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHypothesisOutcome {
    /// Was the hypothesis correct?
    pub correct: bool,
    /// Analysis of the result
    pub analysis: String,
    /// Lessons learned
    pub improvements: Vec<String>,
}

/// Response for `/mcp/platform/query` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPlatformQueryResponse {
    /// Answer to the query
    pub answer: String,
    /// Category of the capability found (feature, node_type, edge_type, template)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Whether the capability is available
    pub available: bool,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

// ============================================================================
// Live Execution Introspection Response Types
// ============================================================================

/// Response for `/mcp/live/executions` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLiveExecutionsResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// List of active executions
    pub executions: Vec<McpExecutionSummary>,
    /// Number of active executions
    pub active_count: usize,
    /// Total tracked executions (including completed)
    pub total_count: usize,
}

/// Summary of an execution for MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpExecutionSummary {
    /// Unique execution identifier
    pub execution_id: String,
    /// Name of the graph being executed
    pub graph_name: String,
    /// When execution started (ISO 8601)
    pub started_at: String,
    /// Current node being executed
    pub current_node: String,
    /// Current iteration count
    pub iteration: u32,
    /// Current status
    pub status: String,
}

/// Response for `/mcp/live/executions/:id` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLiveExecutionDetailResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Unique execution identifier
    pub execution_id: String,
    /// Name of the graph being executed
    pub graph_name: String,
    /// When execution started (ISO 8601)
    pub started_at: String,
    /// Current node being executed
    pub current_node: String,
    /// Previous node (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_node: Option<String>,
    /// Current iteration count
    pub iteration: u32,
    /// Total number of nodes visited
    pub total_nodes_visited: u32,
    /// Current state values (JSON snapshot)
    pub state: serde_json::Value,
    /// Performance metrics
    pub metrics: McpExecutionMetrics,
    /// Checkpoint status
    pub checkpoint: McpCheckpointStatus,
    /// Current status
    pub status: String,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Execution metrics for MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpExecutionMetrics {
    /// Total execution time so far in milliseconds
    pub total_duration_ms: u64,
    /// Number of nodes executed
    pub nodes_executed: u32,
    /// Number of successful node executions
    pub nodes_succeeded: u32,
    /// Number of failed node executions
    pub nodes_failed: u32,
    /// Average node execution time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_node_duration_ms: Option<f64>,
    /// Slowest node name and duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slowest_node: Option<McpSlowestNode>,
    /// Current iteration count
    pub iteration: u32,
}

/// Information about the slowest node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSlowestNode {
    /// Node name
    pub name: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Checkpoint status for MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCheckpointStatus {
    /// Whether checkpointing is enabled
    pub enabled: bool,
    /// Thread ID if checkpointing is active
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Last checkpoint timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint_at: Option<String>,
    /// Number of checkpoints created
    pub checkpoint_count: u32,
    /// Total checkpoint size in bytes
    pub total_size_bytes: u64,
}

/// Response for `/mcp/live/executions/:id/node` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLiveCurrentNodeResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Execution ID
    pub execution_id: String,
    /// Current node being executed
    pub current_node: String,
    /// Previous node (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_node: Option<String>,
}

/// Response for `/mcp/live/executions/:id/state` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLiveCurrentStateResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Execution ID
    pub execution_id: String,
    /// Current state values
    pub state: serde_json::Value,
}

/// Response for `/mcp/live/executions/:id/history` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLiveHistoryResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Execution ID
    pub execution_id: String,
    /// Execution history steps
    pub steps: Vec<McpExecutionStep>,
    /// Total steps in history
    pub total_steps: usize,
}

/// A single execution step for MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpExecutionStep {
    /// Step number (1-indexed)
    pub step_number: u32,
    /// Name of the node executed
    pub node_name: String,
    /// When the step started (ISO 8601)
    pub started_at: String,
    /// When the step completed (ISO 8601), if completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// Duration in milliseconds, if completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Outcome of this step
    pub outcome: String,
}

/// Response for `/mcp/live/executions/:id/metrics` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLiveMetricsResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Execution ID
    pub execution_id: String,
    /// Metrics
    pub metrics: McpExecutionMetrics,
}

/// Response for `/mcp/live/executions/:id/checkpoint` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLiveCheckpointResponse {
    /// Schema version for forward compatibility
    pub schema_version: String,
    /// Execution ID
    pub execution_id: String,
    /// Checkpoint status
    pub checkpoint: McpCheckpointStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // ============================================================================
    // McpAboutResponse Tests
    // ============================================================================

    #[test]
    fn test_mcp_about_response_creation() {
        let response = McpAboutResponse {
            schema_version: "1.0.0".to_string(),
            protocol: "dashflow-self-doc/1.0".to_string(),
            name: "TestApp".to_string(),
            version: "0.1.0".to_string(),
            description: "A test application".to_string(),
            capabilities: vec!["search".to_string(), "chat".to_string()],
            dashflow_version: "1.11.3".to_string(),
        };
        assert_eq!(response.name, "TestApp");
        assert_eq!(response.capabilities.len(), 2);
    }

    #[test]
    fn test_mcp_about_response_serialization() {
        let response = McpAboutResponse {
            schema_version: "1.0.0".to_string(),
            protocol: "dashflow-self-doc/1.0".to_string(),
            name: "SerializeTest".to_string(),
            version: "1.0.0".to_string(),
            description: "Testing serialization".to_string(),
            capabilities: vec!["cap1".to_string()],
            dashflow_version: "1.11.3".to_string(),
        };
        let json = serde_json::to_string(&response).expect("Serialization failed");
        assert!(json.contains("SerializeTest"));
        assert!(json.contains("dashflow-self-doc/1.0"));
    }

    #[test]
    fn test_mcp_about_response_deserialization() {
        let json = r#"{
            "schema_version": "1.0.0",
            "protocol": "dashflow-self-doc/1.0",
            "name": "DeserTest",
            "version": "2.0.0",
            "description": "Deserialization test",
            "capabilities": ["a", "b", "c"],
            "dashflow_version": "1.11.0"
        }"#;
        let response: McpAboutResponse = serde_json::from_str(json).expect("Deserialization failed");
        assert_eq!(response.name, "DeserTest");
        assert_eq!(response.capabilities.len(), 3);
    }

    #[test]
    fn test_mcp_about_response_clone() {
        let response = McpAboutResponse {
            schema_version: "1.0.0".to_string(),
            protocol: "proto".to_string(),
            name: "CloneTest".to_string(),
            version: "1.0.0".to_string(),
            description: "desc".to_string(),
            capabilities: vec!["cap".to_string()],
            dashflow_version: "1.0.0".to_string(),
        };
        let cloned = response.clone();
        assert_eq!(response.name, cloned.name);
        assert_eq!(response.capabilities.len(), cloned.capabilities.len());
    }

    #[test]
    fn test_mcp_about_response_empty_capabilities() {
        let response = McpAboutResponse {
            schema_version: "1.0.0".to_string(),
            protocol: "proto".to_string(),
            name: "NoCapabilities".to_string(),
            version: "1.0.0".to_string(),
            description: "desc".to_string(),
            capabilities: vec![],
            dashflow_version: "1.0.0".to_string(),
        };
        assert!(response.capabilities.is_empty());
        let json = serde_json::to_string(&response).expect("Serialization failed");
        assert!(json.contains("\"capabilities\":[]"));
    }

    // ============================================================================
    // McpToolInfo Tests
    // ============================================================================

    #[test]
    fn test_mcp_tool_info_creation() {
        let tool = McpToolInfo {
            name: "calculator".to_string(),
            description: "Performs calculations".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string"}
                }
            }),
        };
        assert_eq!(tool.name, "calculator");
    }

    #[test]
    fn test_mcp_tool_info_serialization() {
        let tool = McpToolInfo {
            name: "search".to_string(),
            description: "Search the web".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let json = serde_json::to_string(&tool).expect("Serialization failed");
        assert!(json.contains("search"));
        assert!(json.contains("input_schema"));
    }

    #[test]
    fn test_mcp_tool_info_roundtrip() {
        let original = McpToolInfo {
            name: "roundtrip_tool".to_string(),
            description: "A roundtrip test tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "param1": {"type": "string"},
                    "param2": {"type": "number"}
                },
                "required": ["param1"]
            }),
        };
        let json = serde_json::to_string(&original).expect("Serialization failed");
        let restored: McpToolInfo = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(original.name, restored.name);
        assert_eq!(original.description, restored.description);
    }

    // ============================================================================
    // McpNodeInfo Tests
    // ============================================================================

    #[test]
    fn test_mcp_node_info_creation() {
        let node = McpNodeInfo {
            name: "agent_node".to_string(),
            node_type: "agent".to_string(),
            description: Some("An agent node".to_string()),
            tools: vec!["search".to_string(), "calculator".to_string()],
            version: Some("1.0.0".to_string()),
            metadata: None,
        };
        assert_eq!(node.name, "agent_node");
        assert_eq!(node.tools.len(), 2);
    }

    #[test]
    fn test_mcp_node_info_optional_fields() {
        let node = McpNodeInfo {
            name: "minimal_node".to_string(),
            node_type: "function".to_string(),
            description: None,
            tools: vec![],
            version: None,
            metadata: None,
        };
        let json = serde_json::to_string(&node).expect("Serialization failed");
        // Optional fields should be skipped when None
        assert!(!json.contains("description"));
        assert!(!json.contains("version"));
        assert!(!json.contains("metadata"));
    }

    #[test]
    fn test_mcp_node_info_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), serde_json::json!("processing"));
        metadata.insert("tags".to_string(), serde_json::json!(["important", "core"]));

        let node = McpNodeInfo {
            name: "meta_node".to_string(),
            node_type: "function".to_string(),
            description: Some("Node with metadata".to_string()),
            tools: vec![],
            version: Some("2.0.0".to_string()),
            metadata: Some(metadata),
        };
        let json = serde_json::to_string(&node).expect("Serialization failed");
        assert!(json.contains("category"));
        assert!(json.contains("processing"));
    }

    // ============================================================================
    // McpGraphInfo Tests
    // ============================================================================

    #[test]
    fn test_mcp_graph_info_creation() {
        let graph = McpGraphInfo {
            entry_point: "start".to_string(),
            node_count: 5,
            edge_count: 7,
            terminal_nodes: vec!["end".to_string()],
            decision_points: vec!["router".to_string()],
            has_cycles: Some(false),
            has_parallel_paths: Some(true),
        };
        assert_eq!(graph.entry_point, "start");
        assert_eq!(graph.node_count, 5);
    }

    #[test]
    fn test_mcp_graph_info_optional_flags() {
        let graph = McpGraphInfo {
            entry_point: "main".to_string(),
            node_count: 3,
            edge_count: 2,
            terminal_nodes: vec!["exit".to_string()],
            decision_points: vec![],
            has_cycles: None,
            has_parallel_paths: None,
        };
        let json = serde_json::to_string(&graph).expect("Serialization failed");
        assert!(!json.contains("has_cycles"));
        assert!(!json.contains("has_parallel_paths"));
    }

    // ============================================================================
    // McpCapabilitiesResponse Tests
    // ============================================================================

    #[test]
    fn test_mcp_capabilities_response_creation() {
        let response = McpCapabilitiesResponse {
            schema_version: "1.0.0".to_string(),
            tools: vec![
                McpToolInfo {
                    name: "tool1".to_string(),
                    description: "First tool".to_string(),
                    input_schema: serde_json::json!({}),
                },
            ],
            nodes: vec![
                McpNodeInfo {
                    name: "node1".to_string(),
                    node_type: "function".to_string(),
                    description: None,
                    tools: vec![],
                    version: None,
                    metadata: None,
                },
            ],
            features: vec!["streaming".to_string(), "checkpointing".to_string()],
        };
        assert_eq!(response.tools.len(), 1);
        assert_eq!(response.nodes.len(), 1);
        assert_eq!(response.features.len(), 2);
    }

    // ============================================================================
    // McpArchitectureResponse Tests
    // ============================================================================

    #[test]
    fn test_mcp_architecture_response_creation() {
        let response = McpArchitectureResponse {
            schema_version: "1.0.0".to_string(),
            graph: McpGraphInfo {
                entry_point: "start".to_string(),
                node_count: 3,
                edge_count: 2,
                terminal_nodes: vec!["end".to_string()],
                decision_points: vec![],
                has_cycles: Some(false),
                has_parallel_paths: None,
            },
            dashflow_features: vec!["feature1".to_string()],
            execution_flow: "start -> process -> end".to_string(),
        };
        assert_eq!(response.graph.node_count, 3);
        assert!(response.execution_flow.contains("start"));
    }

    // ============================================================================
    // McpDependencyInfo Tests
    // ============================================================================

    #[test]
    fn test_mcp_dependency_info_creation() {
        let dep = McpDependencyInfo {
            name: "serde".to_string(),
            version: "1.0.0".to_string(),
            purpose: "Serialization".to_string(),
            is_dashflow: false,
        };
        assert_eq!(dep.name, "serde");
        assert!(!dep.is_dashflow);
    }

    #[test]
    fn test_mcp_dependency_info_dashflow_crate() {
        let dep = McpDependencyInfo {
            name: "dashflow-openai".to_string(),
            version: "1.11.3".to_string(),
            purpose: "OpenAI integration".to_string(),
            is_dashflow: true,
        };
        assert!(dep.is_dashflow);
    }

    // ============================================================================
    // McpQueryRequest/Response Tests
    // ============================================================================

    #[test]
    fn test_mcp_query_request_creation() {
        let request = McpQueryRequest {
            question: "What tools are available?".to_string(),
        };
        assert!(!request.question.is_empty());
    }

    #[test]
    fn test_mcp_query_response_creation() {
        let response = McpQueryResponse {
            answer: "There are 5 tools available".to_string(),
            sources: vec!["tools.rs".to_string(), "config.toml".to_string()],
            confidence: 0.95,
        };
        assert_eq!(response.confidence, 0.95);
        assert_eq!(response.sources.len(), 2);
    }

    #[test]
    fn test_mcp_query_response_serialization() {
        let response = McpQueryResponse {
            answer: "Test answer".to_string(),
            sources: vec!["source1".to_string()],
            confidence: 0.8,
        };
        let json = serde_json::to_string(&response).expect("Serialization failed");
        assert!(json.contains("0.8"));
        assert!(json.contains("source1"));
    }

    // ============================================================================
    // McpNodeSummary Tests
    // ============================================================================

    #[test]
    fn test_mcp_node_summary_creation() {
        let summary = McpNodeSummary {
            name: "main".to_string(),
            node_type: "function".to_string(),
            description: Some("Main processing node".to_string()),
            is_entry_point: true,
            is_terminal: false,
        };
        assert!(summary.is_entry_point);
        assert!(!summary.is_terminal);
    }

    #[test]
    fn test_mcp_node_summary_terminal() {
        let summary = McpNodeSummary {
            name: "end".to_string(),
            node_type: "function".to_string(),
            description: None,
            is_entry_point: false,
            is_terminal: true,
        };
        assert!(!summary.is_entry_point);
        assert!(summary.is_terminal);
    }

    // ============================================================================
    // McpEdgeInfo Tests
    // ============================================================================

    #[test]
    fn test_mcp_edge_info_simple() {
        let edge = McpEdgeInfo {
            from: "start".to_string(),
            to: "process".to_string(),
            is_conditional: false,
            condition: None,
        };
        assert!(!edge.is_conditional);
        assert!(edge.condition.is_none());
    }

    #[test]
    fn test_mcp_edge_info_conditional() {
        let edge = McpEdgeInfo {
            from: "router".to_string(),
            to: "branch_a".to_string(),
            is_conditional: true,
            condition: Some("input > 10".to_string()),
        };
        assert!(edge.is_conditional);
        assert!(edge.condition.is_some());
    }

    // ============================================================================
    // McpToolParameter Tests
    // ============================================================================

    #[test]
    fn test_mcp_tool_parameter_required() {
        let param = McpToolParameter {
            name: "query".to_string(),
            param_type: "string".to_string(),
            description: "Search query".to_string(),
            required: true,
            default: None,
        };
        assert!(param.required);
        assert!(param.default.is_none());
    }

    #[test]
    fn test_mcp_tool_parameter_with_default() {
        let param = McpToolParameter {
            name: "limit".to_string(),
            param_type: "number".to_string(),
            description: "Maximum results".to_string(),
            required: false,
            default: Some(serde_json::json!(10)),
        };
        assert!(!param.required);
        assert_eq!(param.default, Some(serde_json::json!(10)));
    }

    // ============================================================================
    // McpFieldInfo Tests
    // ============================================================================

    #[test]
    fn test_mcp_field_info_creation() {
        let field = McpFieldInfo {
            name: "messages".to_string(),
            field_type: "Vec<Message>".to_string(),
            optional: false,
            description: Some("Conversation messages".to_string()),
        };
        assert_eq!(field.name, "messages");
        assert!(!field.optional);
    }

    #[test]
    fn test_mcp_field_info_optional() {
        let field = McpFieldInfo {
            name: "context".to_string(),
            field_type: "Option<String>".to_string(),
            optional: true,
            description: None,
        };
        assert!(field.optional);
        assert!(field.description.is_none());
    }

    // ============================================================================
    // McpHypothesisAccuracy Tests
    // ============================================================================

    #[test]
    fn test_mcp_hypothesis_accuracy_creation() {
        let accuracy = McpHypothesisAccuracy {
            overall: 0.85,
            total_evaluated: 100,
            correct: 85,
            incorrect: 15,
            active_count: 5,
            by_source: vec![
                McpSourceAccuracy {
                    source: "Capability Gap".to_string(),
                    accuracy: 0.9,
                    total: 50,
                    correct: 45,
                },
            ],
        };
        assert_eq!(accuracy.overall, 0.85);
        assert_eq!(accuracy.correct + accuracy.incorrect, accuracy.total_evaluated);
    }

    #[test]
    fn test_mcp_hypothesis_accuracy_empty() {
        let accuracy = McpHypothesisAccuracy {
            overall: 0.0,
            total_evaluated: 0,
            correct: 0,
            incorrect: 0,
            active_count: 0,
            by_source: vec![],
        };
        assert!(accuracy.by_source.is_empty());
    }

    // ============================================================================
    // McpExecutionMetrics Tests
    // ============================================================================

    #[test]
    fn test_mcp_execution_metrics_creation() {
        let metrics = McpExecutionMetrics {
            total_duration_ms: 5000,
            nodes_executed: 10,
            nodes_succeeded: 9,
            nodes_failed: 1,
            avg_node_duration_ms: Some(500.0),
            slowest_node: Some(McpSlowestNode {
                name: "heavy_processing".to_string(),
                duration_ms: 2000,
            }),
            iteration: 3,
        };
        assert_eq!(metrics.nodes_succeeded + metrics.nodes_failed, metrics.nodes_executed);
        assert!(metrics.slowest_node.is_some());
    }

    #[test]
    fn test_mcp_execution_metrics_minimal() {
        let metrics = McpExecutionMetrics {
            total_duration_ms: 100,
            nodes_executed: 1,
            nodes_succeeded: 1,
            nodes_failed: 0,
            avg_node_duration_ms: None,
            slowest_node: None,
            iteration: 1,
        };
        assert!(metrics.avg_node_duration_ms.is_none());
        assert!(metrics.slowest_node.is_none());
    }

    // ============================================================================
    // McpCheckpointStatus Tests
    // ============================================================================

    #[test]
    fn test_mcp_checkpoint_status_enabled() {
        let status = McpCheckpointStatus {
            enabled: true,
            thread_id: Some("thread-123".to_string()),
            last_checkpoint_at: Some("2025-01-01T00:00:00Z".to_string()),
            checkpoint_count: 5,
            total_size_bytes: 10240,
        };
        assert!(status.enabled);
        assert!(status.thread_id.is_some());
    }

    #[test]
    fn test_mcp_checkpoint_status_disabled() {
        let status = McpCheckpointStatus {
            enabled: false,
            thread_id: None,
            last_checkpoint_at: None,
            checkpoint_count: 0,
            total_size_bytes: 0,
        };
        assert!(!status.enabled);
        assert_eq!(status.checkpoint_count, 0);
    }

    // ============================================================================
    // McpExecutionStep Tests
    // ============================================================================

    #[test]
    fn test_mcp_execution_step_completed() {
        let step = McpExecutionStep {
            step_number: 1,
            node_name: "process".to_string(),
            started_at: "2025-01-01T00:00:00Z".to_string(),
            completed_at: Some("2025-01-01T00:00:05Z".to_string()),
            duration_ms: Some(5000),
            outcome: "success".to_string(),
        };
        assert!(step.completed_at.is_some());
        assert_eq!(step.outcome, "success");
    }

    #[test]
    fn test_mcp_execution_step_in_progress() {
        let step = McpExecutionStep {
            step_number: 2,
            node_name: "analyzing".to_string(),
            started_at: "2025-01-01T00:01:00Z".to_string(),
            completed_at: None,
            duration_ms: None,
            outcome: "running".to_string(),
        };
        assert!(step.completed_at.is_none());
        assert!(step.duration_ms.is_none());
    }

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[test]
    fn test_full_about_response_roundtrip() {
        let original = McpAboutResponse {
            schema_version: "1.0.0".to_string(),
            protocol: "dashflow-self-doc/1.0".to_string(),
            name: "RoundtripApp".to_string(),
            version: "3.0.0".to_string(),
            description: "Full roundtrip test".to_string(),
            capabilities: vec!["search".to_string(), "chat".to_string(), "analyze".to_string()],
            dashflow_version: "1.11.3".to_string(),
        };
        let json = serde_json::to_string_pretty(&original).expect("Serialization failed");
        let restored: McpAboutResponse = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(original.name, restored.name);
        assert_eq!(original.version, restored.version);
        assert_eq!(original.capabilities.len(), restored.capabilities.len());
    }

    #[test]
    fn test_nested_response_serialization() {
        let response = McpLiveExecutionDetailResponse {
            schema_version: "1.0.0".to_string(),
            execution_id: "exec-123".to_string(),
            graph_name: "test_graph".to_string(),
            started_at: "2025-01-01T00:00:00Z".to_string(),
            current_node: "processing".to_string(),
            previous_node: Some("start".to_string()),
            iteration: 5,
            total_nodes_visited: 10,
            state: serde_json::json!({"messages": [], "context": "test"}),
            metrics: McpExecutionMetrics {
                total_duration_ms: 10000,
                nodes_executed: 10,
                nodes_succeeded: 10,
                nodes_failed: 0,
                avg_node_duration_ms: Some(1000.0),
                slowest_node: None,
                iteration: 5,
            },
            checkpoint: McpCheckpointStatus {
                enabled: true,
                thread_id: Some("thread-456".to_string()),
                last_checkpoint_at: None,
                checkpoint_count: 3,
                total_size_bytes: 4096,
            },
            status: "running".to_string(),
            error: None,
        };
        let json = serde_json::to_string(&response).expect("Serialization failed");
        assert!(json.contains("exec-123"));
        assert!(json.contains("processing"));
        assert!(json.contains("thread-456"));
    }

    #[test]
    fn test_all_response_types_debug() {
        // Ensure Debug is implemented for all key types
        let about = McpAboutResponse {
            schema_version: "1.0.0".to_string(),
            protocol: "proto".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "desc".to_string(),
            capabilities: vec![],
            dashflow_version: "1.0.0".to_string(),
        };
        let debug_str = format!("{:?}", about);
        assert!(debug_str.contains("McpAboutResponse"));

        let tool = McpToolInfo {
            name: "tool".to_string(),
            description: "desc".to_string(),
            input_schema: serde_json::json!({}),
        };
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("McpToolInfo"));

        let metrics = McpExecutionMetrics {
            total_duration_ms: 0,
            nodes_executed: 0,
            nodes_succeeded: 0,
            nodes_failed: 0,
            avg_node_duration_ms: None,
            slowest_node: None,
            iteration: 0,
        };
        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("McpExecutionMetrics"));
    }

    #[test]
    fn test_unicode_in_responses() {
        let response = McpAboutResponse {
            schema_version: "1.0.0".to_string(),
            protocol: "协议/1.0".to_string(),
            name: "テストアプリ".to_string(),
            version: "1.0.0".to_string(),
            description: "Приложение для тестирования 🚀".to_string(),
            capabilities: vec!["搜索".to_string(), "聊天".to_string()],
            dashflow_version: "1.0.0".to_string(),
        };
        let json = serde_json::to_string(&response).expect("Serialization failed");
        let restored: McpAboutResponse = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(response.name, restored.name);
        assert_eq!(response.description, restored.description);
    }
}
