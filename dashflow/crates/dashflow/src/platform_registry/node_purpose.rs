// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Node purpose explanation for graph introspection.
//!
//! Provides AI agents with understanding of what each node in their graph does.

use serde::{Deserialize, Serialize};

/// Node purpose explanation
///
/// Provides AI agents with understanding of what each node in their graph does:
/// - What is the purpose of this node?
/// - What state fields does it read?
/// - What state fields does it write?
/// - What DashFlow APIs does it use?
/// - What external services does it call?
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::platform_registry::NodePurpose;
///
/// let purpose = graph.explain_node("reasoning");
///
/// // AI asks: "What does my reasoning node do?"
/// println!("Purpose: {}", purpose.purpose);
///
/// // AI asks: "What does it read from state?"
/// for input in &purpose.inputs {
///     println!("Reads: {} - {}", input.field_name, input.description);
/// }
///
/// // AI asks: "What external services does it call?"
/// for call in &purpose.external_calls {
///     println!("Calls: {} - {}", call.service_name, call.description);
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePurpose {
    /// Node name
    pub node_name: String,
    /// Human-readable purpose description
    pub purpose: String,
    /// State fields read by this node
    pub inputs: Vec<StateFieldUsage>,
    /// State fields written by this node
    pub outputs: Vec<StateFieldUsage>,
    /// DashFlow APIs used by this node
    pub apis_used: Vec<ApiUsage>,
    /// External service calls made by this node
    pub external_calls: Vec<ExternalCall>,
    /// Node type/category
    pub node_type: NodeType,
    /// Additional metadata
    pub metadata: NodePurposeMetadata,
}

impl NodePurpose {
    /// Create a new node purpose builder
    #[must_use]
    pub fn builder(node_name: impl Into<String>) -> NodePurposeBuilder {
        NodePurposeBuilder::new(node_name)
    }

    /// Create a simple node purpose
    #[must_use]
    pub fn new(node_name: impl Into<String>, purpose: impl Into<String>) -> Self {
        Self {
            node_name: node_name.into(),
            purpose: purpose.into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            apis_used: Vec::new(),
            external_calls: Vec::new(),
            node_type: NodeType::Processing,
            metadata: NodePurposeMetadata::default(),
        }
    }

    /// Convert to JSON string for AI consumption
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get a brief summary of the node purpose
    #[must_use]
    pub fn summary(&self) -> String {
        let inputs = self.inputs.len();
        let outputs = self.outputs.len();
        let apis = self.apis_used.len();
        let calls = self.external_calls.len();
        format!(
            "Node '{}' ({}): {} inputs, {} outputs, {} APIs, {} external calls",
            self.node_name, self.node_type, inputs, outputs, apis, calls
        )
    }

    /// Check if this node reads from state
    #[must_use]
    pub fn has_inputs(&self) -> bool {
        !self.inputs.is_empty()
    }

    /// Check if this node writes to state
    #[must_use]
    pub fn has_outputs(&self) -> bool {
        !self.outputs.is_empty()
    }

    /// Check if this node makes external calls
    #[must_use]
    pub fn has_external_calls(&self) -> bool {
        !self.external_calls.is_empty()
    }

    /// Check if this node uses DashFlow APIs
    #[must_use]
    pub fn uses_dashflow_apis(&self) -> bool {
        !self.apis_used.is_empty()
    }

    /// Get input field by name
    #[must_use]
    pub fn get_input(&self, field_name: &str) -> Option<&StateFieldUsage> {
        self.inputs.iter().find(|f| f.field_name == field_name)
    }

    /// Get output field by name
    #[must_use]
    pub fn get_output(&self, field_name: &str) -> Option<&StateFieldUsage> {
        self.outputs.iter().find(|f| f.field_name == field_name)
    }

    /// Check if this node reads a specific field
    #[must_use]
    pub fn reads_field(&self, field_name: &str) -> bool {
        self.inputs.iter().any(|f| f.field_name == field_name)
    }

    /// Check if this node writes a specific field
    #[must_use]
    pub fn writes_field(&self, field_name: &str) -> bool {
        self.outputs.iter().any(|f| f.field_name == field_name)
    }

    /// Check if this node uses a specific API
    #[must_use]
    pub fn uses_api(&self, api_name: &str) -> bool {
        let api_lower = api_name.to_lowercase();
        self.apis_used
            .iter()
            .any(|a| a.api_name.to_lowercase().contains(&api_lower))
    }

    /// Check if this node calls a specific service
    #[must_use]
    pub fn calls_service(&self, service_name: &str) -> bool {
        let service_lower = service_name.to_lowercase();
        self.external_calls
            .iter()
            .any(|c| c.service_name.to_lowercase().contains(&service_lower))
    }

    /// Get all LLM API calls
    #[must_use]
    pub fn llm_calls(&self) -> Vec<&ExternalCall> {
        self.external_calls
            .iter()
            .filter(|c| c.call_type == ExternalCallType::LlmApi)
            .collect()
    }

    /// Get all tool execution calls
    #[must_use]
    pub fn tool_calls(&self) -> Vec<&ExternalCall> {
        self.external_calls
            .iter()
            .filter(|c| c.call_type == ExternalCallType::ToolExecution)
            .collect()
    }

    /// Get external service calls (HTTP, database, etc.)
    #[must_use]
    pub fn service_calls(&self) -> Vec<&ExternalCall> {
        self.external_calls
            .iter()
            .filter(|c| {
                matches!(
                    c.call_type,
                    ExternalCallType::HttpRequest | ExternalCallType::DatabaseQuery
                )
            })
            .collect()
    }

    /// Generate a natural language explanation of the node
    #[must_use]
    pub fn explain(&self) -> String {
        let mut explanation = String::new();

        // Purpose
        explanation.push_str(&format!("**{}** - {}\n\n", self.node_name, self.purpose));

        // Node type
        explanation.push_str(&format!("Type: {}\n\n", self.node_type));

        // Inputs
        if !self.inputs.is_empty() {
            explanation.push_str("Reads from state:\n");
            for input in &self.inputs {
                let required = if input.required { " (required)" } else { "" };
                explanation.push_str(&format!(
                    "  - {}: {}{}\n",
                    input.field_name, input.description, required
                ));
            }
            explanation.push('\n');
        }

        // Outputs
        if !self.outputs.is_empty() {
            explanation.push_str("Writes to state:\n");
            for output in &self.outputs {
                explanation.push_str(&format!(
                    "  - {}: {}\n",
                    output.field_name, output.description
                ));
            }
            explanation.push('\n');
        }

        // APIs used
        if !self.apis_used.is_empty() {
            explanation.push_str("DashFlow APIs used:\n");
            for api in &self.apis_used {
                explanation.push_str(&format!(
                    "  - {}: {}\n",
                    api.api_name, api.usage_description
                ));
            }
            explanation.push('\n');
        }

        // External calls
        if !self.external_calls.is_empty() {
            explanation.push_str("External services called:\n");
            for call in &self.external_calls {
                explanation.push_str(&format!(
                    "  - {} ({}): {}\n",
                    call.service_name, call.call_type, call.description
                ));
            }
        }

        explanation
    }
}

/// Builder for NodePurpose
#[derive(Debug, Default)]
pub struct NodePurposeBuilder {
    node_name: String,
    purpose: Option<String>,
    inputs: Vec<StateFieldUsage>,
    outputs: Vec<StateFieldUsage>,
    apis_used: Vec<ApiUsage>,
    external_calls: Vec<ExternalCall>,
    node_type: Option<NodeType>,
    metadata: Option<NodePurposeMetadata>,
}

impl NodePurposeBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new(node_name: impl Into<String>) -> Self {
        Self {
            node_name: node_name.into(),
            ..Default::default()
        }
    }

    /// Set the purpose description
    #[must_use]
    pub fn purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purpose = Some(purpose.into());
        self
    }

    /// Add an input field
    pub fn add_input(&mut self, input: StateFieldUsage) -> &mut Self {
        self.inputs.push(input);
        self
    }

    /// Add an output field
    pub fn add_output(&mut self, output: StateFieldUsage) -> &mut Self {
        self.outputs.push(output);
        self
    }

    /// Add an API usage
    pub fn add_api(&mut self, api: ApiUsage) -> &mut Self {
        self.apis_used.push(api);
        self
    }

    /// Add an external call
    pub fn add_external_call(&mut self, call: ExternalCall) -> &mut Self {
        self.external_calls.push(call);
        self
    }

    /// Set the node type
    #[must_use]
    pub fn node_type(mut self, node_type: NodeType) -> Self {
        self.node_type = Some(node_type);
        self
    }

    /// Set metadata
    #[must_use]
    pub fn metadata(mut self, metadata: NodePurposeMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Build the node purpose
    #[must_use]
    pub fn build(self) -> NodePurpose {
        NodePurpose {
            node_name: self.node_name,
            purpose: self
                .purpose
                .unwrap_or_else(|| "No purpose specified".to_string()),
            inputs: self.inputs,
            outputs: self.outputs,
            apis_used: self.apis_used,
            external_calls: self.external_calls,
            node_type: self.node_type.unwrap_or(NodeType::Processing),
            metadata: self.metadata.unwrap_or_default(),
        }
    }
}

/// State field usage - describes how a node uses a state field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateFieldUsage {
    /// Field name in state
    pub field_name: String,
    /// Human-readable description
    pub description: String,
    /// Field type (e.g., `Vec<Message>`, `String`)
    pub field_type: Option<String>,
    /// Whether this field is required
    pub required: bool,
    /// Default value (if any)
    pub default_value: Option<String>,
}

impl StateFieldUsage {
    /// Create a new state field usage
    #[must_use]
    pub fn new(field_name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            description: description.into(),
            field_type: None,
            required: false,
            default_value: None,
        }
    }

    /// Set the field type
    #[must_use]
    pub fn with_type(mut self, field_type: impl Into<String>) -> Self {
        self.field_type = Some(field_type.into());
        self
    }

    /// Mark as required
    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set default value
    #[must_use]
    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default_value = Some(default.into());
        self
    }
}

/// API usage - describes how a node uses a DashFlow API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiUsage {
    /// API name (e.g., "ChatOpenAI::invoke")
    pub api_name: String,
    /// Description of how the API is used
    pub usage_description: String,
    /// Module the API belongs to
    pub module: Option<String>,
    /// Whether this is a critical API (failure blocks execution)
    pub is_critical: bool,
}

impl ApiUsage {
    /// Create a new API usage
    #[must_use]
    pub fn new(api_name: impl Into<String>, usage_description: impl Into<String>) -> Self {
        Self {
            api_name: api_name.into(),
            usage_description: usage_description.into(),
            module: None,
            is_critical: false,
        }
    }

    /// Set the module
    #[must_use]
    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    /// Mark as critical
    #[must_use]
    pub fn critical(mut self) -> Self {
        self.is_critical = true;
        self
    }
}

/// External call - describes an external service call made by a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCall {
    /// Service name (e.g., "OpenAI API", "PostgreSQL")
    pub service_name: String,
    /// Description of the call
    pub description: String,
    /// Type of external call
    pub call_type: ExternalCallType,
    /// Endpoint or resource (if applicable)
    pub endpoint: Option<String>,
    /// Whether this call might fail (needs error handling)
    pub may_fail: bool,
    /// Typical latency description
    pub latency: Option<String>,
}

impl ExternalCall {
    /// Create a new external call
    #[must_use]
    pub fn new(
        service_name: impl Into<String>,
        description: impl Into<String>,
        call_type: ExternalCallType,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            description: description.into(),
            call_type,
            endpoint: None,
            may_fail: true,
            latency: None,
        }
    }

    /// Set the endpoint
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Mark as reliable (may_fail = false)
    #[must_use]
    pub fn reliable(mut self) -> Self {
        self.may_fail = false;
        self
    }

    /// Set latency description
    #[must_use]
    pub fn with_latency(mut self, latency: impl Into<String>) -> Self {
        self.latency = Some(latency.into());
        self
    }
}

/// Type of external call
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalCallType {
    /// LLM API call (OpenAI, Anthropic, etc.)
    #[default]
    LlmApi,
    /// Tool execution (shell, file, etc.)
    ToolExecution,
    /// HTTP request to external service
    HttpRequest,
    /// Database query
    DatabaseQuery,
    /// Vector store operation
    VectorStoreOp,
    /// Message queue operation
    MessageQueue,
    /// File system operation
    FileSystem,
    /// External API (non-LLM)
    ExternalApi,
    /// Other
    Other,
}

impl std::fmt::Display for ExternalCallType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LlmApi => write!(f, "LLM API"),
            Self::ToolExecution => write!(f, "Tool Execution"),
            Self::HttpRequest => write!(f, "HTTP Request"),
            Self::DatabaseQuery => write!(f, "Database Query"),
            Self::VectorStoreOp => write!(f, "Vector Store"),
            Self::MessageQueue => write!(f, "Message Queue"),
            Self::FileSystem => write!(f, "File System"),
            Self::ExternalApi => write!(f, "External API"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Type of node in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Entry point node
    EntryPoint,
    /// Exit point node
    ExitPoint,
    /// General processing node
    #[default]
    Processing,
    /// Routing/decision node
    Router,
    /// LLM inference node
    LlmNode,
    /// Tool execution node
    ToolNode,
    /// State transformation node
    Transform,
    /// Human-in-the-loop node
    HumanInLoop,
    /// Aggregation/merge node
    Aggregator,
    /// Validation/guard node
    Validator,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EntryPoint => write!(f, "Entry Point"),
            Self::ExitPoint => write!(f, "Exit Point"),
            Self::Processing => write!(f, "Processing"),
            Self::Router => write!(f, "Router"),
            Self::LlmNode => write!(f, "LLM Node"),
            Self::ToolNode => write!(f, "Tool Node"),
            Self::Transform => write!(f, "Transform"),
            Self::HumanInLoop => write!(f, "Human-in-Loop"),
            Self::Aggregator => write!(f, "Aggregator"),
            Self::Validator => write!(f, "Validator"),
        }
    }
}

/// Metadata about a node purpose
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodePurposeMetadata {
    /// Source file where node is defined
    pub source_file: Option<String>,
    /// Line number in source
    pub source_line: Option<usize>,
    /// Author/owner of the node
    pub author: Option<String>,
    /// Notes about the node
    pub notes: Vec<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl NodePurposeMetadata {
    /// Create new metadata
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set source file
    #[must_use]
    pub fn with_source(mut self, file: impl Into<String>, line: usize) -> Self {
        self.source_file = Some(file.into());
        self.source_line = Some(line);
        self
    }

    /// Set author
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Add a note
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Add a tag
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// Collection of node purposes for a graph
///
/// Provides AI agents with understanding of all nodes in their graph.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::platform_registry::NodePurposeCollection;
///
/// let purposes = graph.explain_all_nodes();
///
/// // AI asks: "What does each node do?"
/// for (name, purpose) in purposes.iter() {
///     println!("{}: {}", name, purpose.purpose);
/// }
///
/// // AI asks: "Which nodes use external services?"
/// for purpose in purposes.nodes_with_external_calls() {
///     println!("{} calls external services", purpose.node_name);
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodePurposeCollection {
    /// All node purposes, keyed by node name
    pub nodes: std::collections::HashMap<String, NodePurpose>,
    /// Graph ID this collection belongs to
    pub graph_id: Option<String>,
    /// Metadata about the collection
    pub metadata: NodePurposeCollectionMetadata,
}

impl NodePurposeCollection {
    /// Create a new empty collection
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a collection for a specific graph
    #[must_use]
    pub fn for_graph(graph_id: impl Into<String>) -> Self {
        Self {
            graph_id: Some(graph_id.into()),
            ..Default::default()
        }
    }

    /// Add a node purpose
    pub fn add(&mut self, purpose: NodePurpose) -> &mut Self {
        self.nodes.insert(purpose.node_name.clone(), purpose);
        self
    }

    /// Get a node purpose by name
    #[must_use]
    pub fn get(&self, node_name: &str) -> Option<&NodePurpose> {
        self.nodes.get(node_name)
    }

    /// Check if a node exists
    #[must_use]
    pub fn contains(&self, node_name: &str) -> bool {
        self.nodes.contains_key(node_name)
    }

    /// Get all node names
    #[must_use]
    pub fn node_names(&self) -> Vec<&str> {
        self.nodes.keys().map(String::as_str).collect()
    }

    /// Get node count
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Iterate over node purposes
    pub fn iter(&self) -> impl Iterator<Item = (&String, &NodePurpose)> {
        self.nodes.iter()
    }

    /// Get nodes that have external calls
    #[must_use]
    pub fn nodes_with_external_calls(&self) -> Vec<&NodePurpose> {
        self.nodes
            .values()
            .filter(|p| p.has_external_calls())
            .collect()
    }

    /// Get nodes by type
    #[must_use]
    pub fn nodes_by_type(&self, node_type: NodeType) -> Vec<&NodePurpose> {
        self.nodes
            .values()
            .filter(|p| p.node_type == node_type)
            .collect()
    }

    /// Get all LLM nodes
    #[must_use]
    pub fn llm_nodes(&self) -> Vec<&NodePurpose> {
        self.nodes_by_type(NodeType::LlmNode)
    }

    /// Get all tool nodes
    #[must_use]
    pub fn tool_nodes(&self) -> Vec<&NodePurpose> {
        self.nodes_by_type(NodeType::ToolNode)
    }

    /// Get all router nodes
    #[must_use]
    pub fn router_nodes(&self) -> Vec<&NodePurpose> {
        self.nodes_by_type(NodeType::Router)
    }

    /// Get nodes that read a specific field
    #[must_use]
    pub fn nodes_reading_field(&self, field_name: &str) -> Vec<&NodePurpose> {
        self.nodes
            .values()
            .filter(|p| p.reads_field(field_name))
            .collect()
    }

    /// Get nodes that write a specific field
    #[must_use]
    pub fn nodes_writing_field(&self, field_name: &str) -> Vec<&NodePurpose> {
        self.nodes
            .values()
            .filter(|p| p.writes_field(field_name))
            .collect()
    }

    /// Get nodes that use a specific API
    #[must_use]
    pub fn nodes_using_api(&self, api_name: &str) -> Vec<&NodePurpose> {
        self.nodes
            .values()
            .filter(|p| p.uses_api(api_name))
            .collect()
    }

    /// Get nodes that call a specific service
    #[must_use]
    pub fn nodes_calling_service(&self, service_name: &str) -> Vec<&NodePurpose> {
        self.nodes
            .values()
            .filter(|p| p.calls_service(service_name))
            .collect()
    }

    /// Get summary of the collection
    #[must_use]
    pub fn summary(&self) -> String {
        let total = self.len();
        let with_external = self.nodes_with_external_calls().len();
        let llm_count = self.llm_nodes().len();
        let tool_count = self.tool_nodes().len();
        let router_count = self.router_nodes().len();

        format!(
            "Node collection: {} total ({} LLM, {} tool, {} router), {} with external calls",
            total, llm_count, tool_count, router_count, with_external
        )
    }

    /// Convert to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Generate explanation for all nodes
    #[must_use]
    pub fn explain_all(&self) -> String {
        let mut explanation = String::new();

        if let Some(graph_id) = &self.graph_id {
            explanation.push_str(&format!("# Node Explanations for '{}'\n\n", graph_id));
        } else {
            explanation.push_str("# Node Explanations\n\n");
        }

        explanation.push_str(&format!("{}\n\n", self.summary()));

        for purpose in self.nodes.values() {
            explanation.push_str(&purpose.explain());
            explanation.push_str("\n---\n\n");
        }

        explanation
    }
}

/// Metadata about a node purpose collection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodePurposeCollectionMetadata {
    /// When the collection was generated
    pub generated_at: Option<String>,
    /// Source of the analysis
    pub source: Option<String>,
    /// Notes about the collection
    pub notes: Vec<String>,
}

impl NodePurposeCollectionMetadata {
    /// Create new metadata
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set generation time
    #[must_use]
    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.generated_at = Some(timestamp.into());
        self
    }

    /// Set source
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Add a note
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Helper function to infer node type from node name and configuration
///
/// # Arguments
///
/// * `node_name` - The name of the node
/// * `has_llm_calls` - Whether the node makes LLM API calls
/// * `has_tool_calls` - Whether the node executes tools
/// * `has_routing` - Whether the node performs routing decisions
///
/// # Returns
///
/// The inferred node type
#[must_use]
pub fn infer_node_type(
    node_name: &str,
    has_llm_calls: bool,
    has_tool_calls: bool,
    has_routing: bool,
) -> NodeType {
    let name_lower = node_name.to_lowercase();

    // Check by name patterns first
    if name_lower.contains("entry") || name_lower.contains("start") || name_lower.contains("input")
    {
        return NodeType::EntryPoint;
    }
    if name_lower.contains("exit")
        || name_lower.contains("end")
        || name_lower.contains("output")
        || name_lower.contains("response")
    {
        return NodeType::ExitPoint;
    }
    if name_lower.contains("route")
        || name_lower.contains("router")
        || name_lower.contains("dispatch")
    {
        return NodeType::Router;
    }
    if name_lower.contains("human")
        || name_lower.contains("approval")
        || name_lower.contains("review")
    {
        return NodeType::HumanInLoop;
    }
    if name_lower.contains("valid") || name_lower.contains("guard") || name_lower.contains("check")
    {
        return NodeType::Validator;
    }
    if name_lower.contains("aggregat")
        || name_lower.contains("merge")
        || name_lower.contains("combine")
    {
        return NodeType::Aggregator;
    }
    if name_lower.contains("transform")
        || name_lower.contains("convert")
        || name_lower.contains("parse")
    {
        return NodeType::Transform;
    }

    // Infer from behavior
    if has_routing {
        return NodeType::Router;
    }
    if has_llm_calls {
        return NodeType::LlmNode;
    }
    if has_tool_calls {
        return NodeType::ToolNode;
    }

    NodeType::Processing
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================================================
    // NodePurpose Tests
    // =============================================================================

    #[test]
    fn test_node_purpose_new() {
        let purpose = NodePurpose::new("test_node", "Test purpose");
        assert_eq!(purpose.node_name, "test_node");
        assert_eq!(purpose.purpose, "Test purpose");
        assert!(purpose.inputs.is_empty());
        assert!(purpose.outputs.is_empty());
        assert!(purpose.apis_used.is_empty());
        assert!(purpose.external_calls.is_empty());
        assert_eq!(purpose.node_type, NodeType::Processing);
    }

    #[test]
    fn test_node_purpose_builder_basic() {
        let purpose = NodePurpose::builder("reasoning")
            .purpose("Process reasoning steps")
            .build();
        assert_eq!(purpose.node_name, "reasoning");
        assert_eq!(purpose.purpose, "Process reasoning steps");
    }

    #[test]
    fn test_node_purpose_builder_with_node_type() {
        let purpose = NodePurpose::builder("llm_node")
            .purpose("Call LLM")
            .node_type(NodeType::LlmNode)
            .build();
        assert_eq!(purpose.node_type, NodeType::LlmNode);
    }

    #[test]
    fn test_node_purpose_builder_default_purpose() {
        let purpose = NodePurpose::builder("test").build();
        assert_eq!(purpose.purpose, "No purpose specified");
    }

    #[test]
    fn test_node_purpose_builder_add_input() {
        let mut builder = NodePurpose::builder("processor");
        builder.add_input(StateFieldUsage::new("messages", "Input messages"));
        let purpose = builder.purpose("Process messages").build();
        assert_eq!(purpose.inputs.len(), 1);
        assert_eq!(purpose.inputs[0].field_name, "messages");
    }

    #[test]
    fn test_node_purpose_builder_add_output() {
        let mut builder = NodePurpose::builder("generator");
        builder.add_output(StateFieldUsage::new("result", "Generated result"));
        let purpose = builder.purpose("Generate output").build();
        assert_eq!(purpose.outputs.len(), 1);
        assert_eq!(purpose.outputs[0].field_name, "result");
    }

    #[test]
    fn test_node_purpose_builder_add_api() {
        let mut builder = NodePurpose::builder("llm");
        builder.add_api(ApiUsage::new("ChatOpenAI::invoke", "Call the LLM"));
        let purpose = builder.purpose("LLM call").build();
        assert_eq!(purpose.apis_used.len(), 1);
        assert_eq!(purpose.apis_used[0].api_name, "ChatOpenAI::invoke");
    }

    #[test]
    fn test_node_purpose_builder_add_external_call() {
        let mut builder = NodePurpose::builder("tool");
        builder.add_external_call(ExternalCall::new("OpenAI", "API call", ExternalCallType::LlmApi));
        let purpose = builder.purpose("External call").build();
        assert_eq!(purpose.external_calls.len(), 1);
        assert_eq!(purpose.external_calls[0].service_name, "OpenAI");
    }

    #[test]
    fn test_node_purpose_builder_with_metadata() {
        let metadata = NodePurposeMetadata::new()
            .with_author("test_author")
            .with_tag("important");
        let purpose = NodePurpose::builder("meta_node")
            .purpose("With metadata")
            .metadata(metadata)
            .build();
        assert_eq!(purpose.metadata.author, Some("test_author".to_string()));
        assert_eq!(purpose.metadata.tags, vec!["important"]);
    }

    #[test]
    fn test_node_purpose_to_json() {
        let purpose = NodePurpose::new("test", "Test purpose");
        let json = purpose.to_json().unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("Test purpose"));
    }

    #[test]
    fn test_node_purpose_summary() {
        let mut builder = NodePurpose::builder("summary_node");
        builder.add_input(StateFieldUsage::new("in1", "Input 1"));
        builder.add_input(StateFieldUsage::new("in2", "Input 2"));
        builder.add_output(StateFieldUsage::new("out", "Output"));
        builder.add_api(ApiUsage::new("api1", "API usage"));
        builder.add_external_call(ExternalCall::new("svc", "Service call", ExternalCallType::HttpRequest));
        let purpose = builder.purpose("Summary test").node_type(NodeType::Processing).build();

        let summary = purpose.summary();
        assert!(summary.contains("summary_node"));
        assert!(summary.contains("Processing"));
        assert!(summary.contains("2 inputs"));
        assert!(summary.contains("1 outputs"));
        assert!(summary.contains("1 APIs"));
        assert!(summary.contains("1 external calls"));
    }

    #[test]
    fn test_node_purpose_has_inputs() {
        let empty = NodePurpose::new("empty", "Empty");
        assert!(!empty.has_inputs());

        let mut builder = NodePurpose::builder("with_input");
        builder.add_input(StateFieldUsage::new("input", "Input"));
        let with_input = builder.purpose("With input").build();
        assert!(with_input.has_inputs());
    }

    #[test]
    fn test_node_purpose_has_outputs() {
        let empty = NodePurpose::new("empty", "Empty");
        assert!(!empty.has_outputs());

        let mut builder = NodePurpose::builder("with_output");
        builder.add_output(StateFieldUsage::new("output", "Output"));
        let with_output = builder.purpose("With output").build();
        assert!(with_output.has_outputs());
    }

    #[test]
    fn test_node_purpose_has_external_calls() {
        let empty = NodePurpose::new("empty", "Empty");
        assert!(!empty.has_external_calls());

        let mut builder = NodePurpose::builder("with_call");
        builder.add_external_call(ExternalCall::new("svc", "Call", ExternalCallType::HttpRequest));
        let with_call = builder.purpose("With call").build();
        assert!(with_call.has_external_calls());
    }

    #[test]
    fn test_node_purpose_uses_dashflow_apis() {
        let empty = NodePurpose::new("empty", "Empty");
        assert!(!empty.uses_dashflow_apis());

        let mut builder = NodePurpose::builder("with_api");
        builder.add_api(ApiUsage::new("SomeApi", "Usage"));
        let with_api = builder.purpose("With API").build();
        assert!(with_api.uses_dashflow_apis());
    }

    #[test]
    fn test_node_purpose_get_input() {
        let mut builder = NodePurpose::builder("inputs");
        builder.add_input(StateFieldUsage::new("messages", "Messages"));
        builder.add_input(StateFieldUsage::new("context", "Context"));
        let purpose = builder.purpose("Test").build();

        assert!(purpose.get_input("messages").is_some());
        assert_eq!(purpose.get_input("messages").unwrap().description, "Messages");
        assert!(purpose.get_input("nonexistent").is_none());
    }

    #[test]
    fn test_node_purpose_get_output() {
        let mut builder = NodePurpose::builder("outputs");
        builder.add_output(StateFieldUsage::new("result", "Result"));
        let purpose = builder.purpose("Test").build();

        assert!(purpose.get_output("result").is_some());
        assert!(purpose.get_output("nonexistent").is_none());
    }

    #[test]
    fn test_node_purpose_reads_field() {
        let mut builder = NodePurpose::builder("reader");
        builder.add_input(StateFieldUsage::new("messages", "Messages"));
        let purpose = builder.purpose("Test").build();

        assert!(purpose.reads_field("messages"));
        assert!(!purpose.reads_field("other"));
    }

    #[test]
    fn test_node_purpose_writes_field() {
        let mut builder = NodePurpose::builder("writer");
        builder.add_output(StateFieldUsage::new("result", "Result"));
        let purpose = builder.purpose("Test").build();

        assert!(purpose.writes_field("result"));
        assert!(!purpose.writes_field("other"));
    }

    #[test]
    fn test_node_purpose_uses_api() {
        let mut builder = NodePurpose::builder("api_user");
        builder.add_api(ApiUsage::new("ChatOpenAI::invoke", "LLM call"));
        let purpose = builder.purpose("Test").build();

        assert!(purpose.uses_api("ChatOpenAI"));
        assert!(purpose.uses_api("chatopenai")); // Case insensitive
        assert!(purpose.uses_api("invoke"));
        assert!(!purpose.uses_api("Anthropic"));
    }

    #[test]
    fn test_node_purpose_calls_service() {
        let mut builder = NodePurpose::builder("service_caller");
        builder.add_external_call(ExternalCall::new("OpenAI API", "Call", ExternalCallType::LlmApi));
        let purpose = builder.purpose("Test").build();

        assert!(purpose.calls_service("OpenAI"));
        assert!(purpose.calls_service("openai")); // Case insensitive
        assert!(!purpose.calls_service("Anthropic"));
    }

    #[test]
    fn test_node_purpose_llm_calls() {
        let mut builder = NodePurpose::builder("llm_test");
        builder.add_external_call(ExternalCall::new("OpenAI", "LLM", ExternalCallType::LlmApi));
        builder.add_external_call(ExternalCall::new("DB", "Query", ExternalCallType::DatabaseQuery));
        let purpose = builder.purpose("Test").build();

        let llm_calls = purpose.llm_calls();
        assert_eq!(llm_calls.len(), 1);
        assert_eq!(llm_calls[0].service_name, "OpenAI");
    }

    #[test]
    fn test_node_purpose_tool_calls() {
        let mut builder = NodePurpose::builder("tool_test");
        builder.add_external_call(ExternalCall::new("shell", "Execute", ExternalCallType::ToolExecution));
        builder.add_external_call(ExternalCall::new("API", "Call", ExternalCallType::LlmApi));
        let purpose = builder.purpose("Test").build();

        let tool_calls = purpose.tool_calls();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].service_name, "shell");
    }

    #[test]
    fn test_node_purpose_service_calls() {
        let mut builder = NodePurpose::builder("service_test");
        builder.add_external_call(ExternalCall::new("REST", "HTTP", ExternalCallType::HttpRequest));
        builder.add_external_call(ExternalCall::new("Postgres", "Query", ExternalCallType::DatabaseQuery));
        builder.add_external_call(ExternalCall::new("OpenAI", "LLM", ExternalCallType::LlmApi));
        let purpose = builder.purpose("Test").build();

        let service_calls = purpose.service_calls();
        assert_eq!(service_calls.len(), 2);
    }

    #[test]
    fn test_node_purpose_explain() {
        let mut builder = NodePurpose::builder("explain_node");
        builder.add_input(StateFieldUsage::new("input", "Input data").required());
        builder.add_output(StateFieldUsage::new("output", "Output data"));
        builder.add_api(ApiUsage::new("SomeApi", "Usage"));
        builder.add_external_call(ExternalCall::new("Service", "Call", ExternalCallType::HttpRequest));
        let purpose = builder.purpose("Test explanation").node_type(NodeType::LlmNode).build();

        let explanation = purpose.explain();
        assert!(explanation.contains("explain_node"));
        assert!(explanation.contains("Test explanation"));
        assert!(explanation.contains("LLM Node"));
        assert!(explanation.contains("Reads from state"));
        assert!(explanation.contains("input"));
        assert!(explanation.contains("(required)"));
        assert!(explanation.contains("Writes to state"));
        assert!(explanation.contains("DashFlow APIs used"));
        assert!(explanation.contains("External services called"));
    }

    // =============================================================================
    // StateFieldUsage Tests
    // =============================================================================

    #[test]
    fn test_state_field_usage_new() {
        let field = StateFieldUsage::new("messages", "Chat messages");
        assert_eq!(field.field_name, "messages");
        assert_eq!(field.description, "Chat messages");
        assert!(field.field_type.is_none());
        assert!(!field.required);
        assert!(field.default_value.is_none());
    }

    #[test]
    fn test_state_field_usage_with_type() {
        let field = StateFieldUsage::new("messages", "Messages")
            .with_type("Vec<Message>");
        assert_eq!(field.field_type, Some("Vec<Message>".to_string()));
    }

    #[test]
    fn test_state_field_usage_required() {
        let field = StateFieldUsage::new("input", "Input").required();
        assert!(field.required);
    }

    #[test]
    fn test_state_field_usage_with_default() {
        let field = StateFieldUsage::new("count", "Count")
            .with_default("0");
        assert_eq!(field.default_value, Some("0".to_string()));
    }

    #[test]
    fn test_state_field_usage_chained() {
        let field = StateFieldUsage::new("config", "Configuration")
            .with_type("Config")
            .required()
            .with_default("Config::default()");
        assert_eq!(field.field_type, Some("Config".to_string()));
        assert!(field.required);
        assert_eq!(field.default_value, Some("Config::default()".to_string()));
    }

    // =============================================================================
    // ApiUsage Tests
    // =============================================================================

    #[test]
    fn test_api_usage_new() {
        let api = ApiUsage::new("ChatModel::invoke", "Call the chat model");
        assert_eq!(api.api_name, "ChatModel::invoke");
        assert_eq!(api.usage_description, "Call the chat model");
        assert!(api.module.is_none());
        assert!(!api.is_critical);
    }

    #[test]
    fn test_api_usage_with_module() {
        let api = ApiUsage::new("invoke", "Call")
            .with_module("dashflow::core::chat_models");
        assert_eq!(api.module, Some("dashflow::core::chat_models".to_string()));
    }

    #[test]
    fn test_api_usage_critical() {
        let api = ApiUsage::new("critical_api", "Must succeed").critical();
        assert!(api.is_critical);
    }

    #[test]
    fn test_api_usage_chained() {
        let api = ApiUsage::new("api", "Usage")
            .with_module("module")
            .critical();
        assert_eq!(api.module, Some("module".to_string()));
        assert!(api.is_critical);
    }

    // =============================================================================
    // ExternalCall Tests
    // =============================================================================

    #[test]
    fn test_external_call_new() {
        let call = ExternalCall::new("OpenAI", "API call", ExternalCallType::LlmApi);
        assert_eq!(call.service_name, "OpenAI");
        assert_eq!(call.description, "API call");
        assert_eq!(call.call_type, ExternalCallType::LlmApi);
        assert!(call.endpoint.is_none());
        assert!(call.may_fail);
        assert!(call.latency.is_none());
    }

    #[test]
    fn test_external_call_with_endpoint() {
        let call = ExternalCall::new("API", "Call", ExternalCallType::HttpRequest)
            .with_endpoint("https://api.example.com/v1");
        assert_eq!(call.endpoint, Some("https://api.example.com/v1".to_string()));
    }

    #[test]
    fn test_external_call_reliable() {
        let call = ExternalCall::new("Local", "Call", ExternalCallType::FileSystem)
            .reliable();
        assert!(!call.may_fail);
    }

    #[test]
    fn test_external_call_with_latency() {
        let call = ExternalCall::new("API", "Call", ExternalCallType::LlmApi)
            .with_latency("100-500ms");
        assert_eq!(call.latency, Some("100-500ms".to_string()));
    }

    #[test]
    fn test_external_call_chained() {
        let call = ExternalCall::new("Service", "Desc", ExternalCallType::HttpRequest)
            .with_endpoint("https://example.com")
            .reliable()
            .with_latency("10ms");
        assert_eq!(call.endpoint, Some("https://example.com".to_string()));
        assert!(!call.may_fail);
        assert_eq!(call.latency, Some("10ms".to_string()));
    }

    // =============================================================================
    // ExternalCallType Tests
    // =============================================================================

    #[test]
    fn test_external_call_type_default() {
        let default = ExternalCallType::default();
        assert_eq!(default, ExternalCallType::LlmApi);
    }

    #[test]
    fn test_external_call_type_display() {
        assert_eq!(format!("{}", ExternalCallType::LlmApi), "LLM API");
        assert_eq!(format!("{}", ExternalCallType::ToolExecution), "Tool Execution");
        assert_eq!(format!("{}", ExternalCallType::HttpRequest), "HTTP Request");
        assert_eq!(format!("{}", ExternalCallType::DatabaseQuery), "Database Query");
        assert_eq!(format!("{}", ExternalCallType::VectorStoreOp), "Vector Store");
        assert_eq!(format!("{}", ExternalCallType::MessageQueue), "Message Queue");
        assert_eq!(format!("{}", ExternalCallType::FileSystem), "File System");
        assert_eq!(format!("{}", ExternalCallType::ExternalApi), "External API");
        assert_eq!(format!("{}", ExternalCallType::Other), "Other");
    }

    #[test]
    fn test_external_call_type_equality() {
        assert_eq!(ExternalCallType::LlmApi, ExternalCallType::LlmApi);
        assert_ne!(ExternalCallType::LlmApi, ExternalCallType::HttpRequest);
    }

    #[test]
    fn test_external_call_type_clone() {
        let original = ExternalCallType::DatabaseQuery;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    // =============================================================================
    // NodeType Tests
    // =============================================================================

    #[test]
    fn test_node_type_default() {
        let default = NodeType::default();
        assert_eq!(default, NodeType::Processing);
    }

    #[test]
    fn test_node_type_display() {
        assert_eq!(format!("{}", NodeType::EntryPoint), "Entry Point");
        assert_eq!(format!("{}", NodeType::ExitPoint), "Exit Point");
        assert_eq!(format!("{}", NodeType::Processing), "Processing");
        assert_eq!(format!("{}", NodeType::Router), "Router");
        assert_eq!(format!("{}", NodeType::LlmNode), "LLM Node");
        assert_eq!(format!("{}", NodeType::ToolNode), "Tool Node");
        assert_eq!(format!("{}", NodeType::Transform), "Transform");
        assert_eq!(format!("{}", NodeType::HumanInLoop), "Human-in-Loop");
        assert_eq!(format!("{}", NodeType::Aggregator), "Aggregator");
        assert_eq!(format!("{}", NodeType::Validator), "Validator");
    }

    #[test]
    fn test_node_type_equality() {
        assert_eq!(NodeType::Router, NodeType::Router);
        assert_ne!(NodeType::Router, NodeType::LlmNode);
    }

    // =============================================================================
    // NodePurposeMetadata Tests
    // =============================================================================

    #[test]
    fn test_node_purpose_metadata_new() {
        let meta = NodePurposeMetadata::new();
        assert!(meta.source_file.is_none());
        assert!(meta.source_line.is_none());
        assert!(meta.author.is_none());
        assert!(meta.notes.is_empty());
        assert!(meta.tags.is_empty());
    }

    #[test]
    fn test_node_purpose_metadata_with_source() {
        let meta = NodePurposeMetadata::new()
            .with_source("src/graph.rs", 42);
        assert_eq!(meta.source_file, Some("src/graph.rs".to_string()));
        assert_eq!(meta.source_line, Some(42));
    }

    #[test]
    fn test_node_purpose_metadata_with_author() {
        let meta = NodePurposeMetadata::new()
            .with_author("alice");
        assert_eq!(meta.author, Some("alice".to_string()));
    }

    #[test]
    fn test_node_purpose_metadata_with_note() {
        let meta = NodePurposeMetadata::new()
            .with_note("First note")
            .with_note("Second note");
        assert_eq!(meta.notes, vec!["First note", "Second note"]);
    }

    #[test]
    fn test_node_purpose_metadata_with_tag() {
        let meta = NodePurposeMetadata::new()
            .with_tag("important")
            .with_tag("reviewed");
        assert_eq!(meta.tags, vec!["important", "reviewed"]);
    }

    // =============================================================================
    // NodePurposeCollection Tests
    // =============================================================================

    #[test]
    fn test_node_purpose_collection_new() {
        let collection = NodePurposeCollection::new();
        assert!(collection.is_empty());
        assert_eq!(collection.len(), 0);
        assert!(collection.graph_id.is_none());
    }

    #[test]
    fn test_node_purpose_collection_for_graph() {
        let collection = NodePurposeCollection::for_graph("my_graph");
        assert_eq!(collection.graph_id, Some("my_graph".to_string()));
    }

    #[test]
    fn test_node_purpose_collection_add_and_get() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::new("node1", "First node"));
        collection.add(NodePurpose::new("node2", "Second node"));

        assert_eq!(collection.len(), 2);
        assert!(!collection.is_empty());
        assert!(collection.get("node1").is_some());
        assert!(collection.get("node2").is_some());
        assert!(collection.get("node3").is_none());
    }

    #[test]
    fn test_node_purpose_collection_contains() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::new("exists", "Node"));

        assert!(collection.contains("exists"));
        assert!(!collection.contains("nonexistent"));
    }

    #[test]
    fn test_node_purpose_collection_node_names() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::new("alpha", "Alpha"));
        collection.add(NodePurpose::new("beta", "Beta"));

        let names = collection.node_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn test_node_purpose_collection_iter() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::new("a", "A"));
        collection.add(NodePurpose::new("b", "B"));

        let count = collection.iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_node_purpose_collection_nodes_with_external_calls() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::new("no_calls", "No external calls"));

        let mut builder = NodePurpose::builder("with_calls");
        builder.add_external_call(ExternalCall::new("API", "Call", ExternalCallType::LlmApi));
        collection.add(builder.purpose("Has calls").build());

        let with_calls = collection.nodes_with_external_calls();
        assert_eq!(with_calls.len(), 1);
        assert_eq!(with_calls[0].node_name, "with_calls");
    }

    #[test]
    fn test_node_purpose_collection_nodes_by_type() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::builder("llm1").purpose("LLM 1").node_type(NodeType::LlmNode).build());
        collection.add(NodePurpose::builder("llm2").purpose("LLM 2").node_type(NodeType::LlmNode).build());
        collection.add(NodePurpose::builder("tool").purpose("Tool").node_type(NodeType::ToolNode).build());

        let llm_nodes = collection.nodes_by_type(NodeType::LlmNode);
        assert_eq!(llm_nodes.len(), 2);

        let tool_nodes = collection.nodes_by_type(NodeType::ToolNode);
        assert_eq!(tool_nodes.len(), 1);
    }

    #[test]
    fn test_node_purpose_collection_llm_nodes() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::builder("llm").purpose("LLM").node_type(NodeType::LlmNode).build());
        collection.add(NodePurpose::builder("other").purpose("Other").node_type(NodeType::Processing).build());

        let llm_nodes = collection.llm_nodes();
        assert_eq!(llm_nodes.len(), 1);
    }

    #[test]
    fn test_node_purpose_collection_tool_nodes() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::builder("tool").purpose("Tool").node_type(NodeType::ToolNode).build());

        let tool_nodes = collection.tool_nodes();
        assert_eq!(tool_nodes.len(), 1);
    }

    #[test]
    fn test_node_purpose_collection_router_nodes() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::builder("router").purpose("Router").node_type(NodeType::Router).build());

        let router_nodes = collection.router_nodes();
        assert_eq!(router_nodes.len(), 1);
    }

    #[test]
    fn test_node_purpose_collection_nodes_reading_field() {
        let mut collection = NodePurposeCollection::new();

        let mut builder1 = NodePurpose::builder("reader1");
        builder1.add_input(StateFieldUsage::new("messages", "Messages"));
        collection.add(builder1.purpose("Reader 1").build());

        let mut builder2 = NodePurpose::builder("reader2");
        builder2.add_input(StateFieldUsage::new("messages", "Messages"));
        collection.add(builder2.purpose("Reader 2").build());

        collection.add(NodePurpose::new("other", "Other"));

        let readers = collection.nodes_reading_field("messages");
        assert_eq!(readers.len(), 2);
    }

    #[test]
    fn test_node_purpose_collection_nodes_writing_field() {
        let mut collection = NodePurposeCollection::new();

        let mut builder = NodePurpose::builder("writer");
        builder.add_output(StateFieldUsage::new("result", "Result"));
        collection.add(builder.purpose("Writer").build());

        collection.add(NodePurpose::new("other", "Other"));

        let writers = collection.nodes_writing_field("result");
        assert_eq!(writers.len(), 1);
    }

    #[test]
    fn test_node_purpose_collection_nodes_using_api() {
        let mut collection = NodePurposeCollection::new();

        let mut builder = NodePurpose::builder("api_user");
        builder.add_api(ApiUsage::new("ChatOpenAI::invoke", "Call"));
        collection.add(builder.purpose("API user").build());

        collection.add(NodePurpose::new("other", "Other"));

        let api_users = collection.nodes_using_api("ChatOpenAI");
        assert_eq!(api_users.len(), 1);
    }

    #[test]
    fn test_node_purpose_collection_nodes_calling_service() {
        let mut collection = NodePurposeCollection::new();

        let mut builder = NodePurpose::builder("caller");
        builder.add_external_call(ExternalCall::new("OpenAI", "Call", ExternalCallType::LlmApi));
        collection.add(builder.purpose("Caller").build());

        collection.add(NodePurpose::new("other", "Other"));

        let callers = collection.nodes_calling_service("OpenAI");
        assert_eq!(callers.len(), 1);
    }

    #[test]
    fn test_node_purpose_collection_summary() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::builder("llm").purpose("LLM").node_type(NodeType::LlmNode).build());
        collection.add(NodePurpose::builder("tool").purpose("Tool").node_type(NodeType::ToolNode).build());
        collection.add(NodePurpose::builder("router").purpose("Router").node_type(NodeType::Router).build());

        let mut ext_builder = NodePurpose::builder("external");
        ext_builder.add_external_call(ExternalCall::new("API", "Call", ExternalCallType::LlmApi));
        collection.add(ext_builder.purpose("External").build());

        let summary = collection.summary();
        assert!(summary.contains("4 total"));
        assert!(summary.contains("1 LLM"));
        assert!(summary.contains("1 tool"));
        assert!(summary.contains("1 router"));
        assert!(summary.contains("1 with external calls"));
    }

    #[test]
    fn test_node_purpose_collection_to_json() {
        let mut collection = NodePurposeCollection::for_graph("test_graph");
        collection.add(NodePurpose::new("node", "Node"));

        let json = collection.to_json().unwrap();
        assert!(json.contains("test_graph"));
        assert!(json.contains("node"));
    }

    #[test]
    fn test_node_purpose_collection_explain_all() {
        let mut collection = NodePurposeCollection::for_graph("explained");
        collection.add(NodePurpose::new("node1", "First node"));
        collection.add(NodePurpose::new("node2", "Second node"));

        let explanation = collection.explain_all();
        assert!(explanation.contains("explained"));
        assert!(explanation.contains("node1") || explanation.contains("node2"));
    }

    #[test]
    fn test_node_purpose_collection_explain_all_without_graph_id() {
        let mut collection = NodePurposeCollection::new();
        collection.add(NodePurpose::new("node", "Node"));

        let explanation = collection.explain_all();
        assert!(explanation.contains("# Node Explanations"));
    }

    // =============================================================================
    // NodePurposeCollectionMetadata Tests
    // =============================================================================

    #[test]
    fn test_node_purpose_collection_metadata_new() {
        let meta = NodePurposeCollectionMetadata::new();
        assert!(meta.generated_at.is_none());
        assert!(meta.source.is_none());
        assert!(meta.notes.is_empty());
    }

    #[test]
    fn test_node_purpose_collection_metadata_with_timestamp() {
        let meta = NodePurposeCollectionMetadata::new()
            .with_timestamp("2025-01-01T00:00:00Z");
        assert_eq!(meta.generated_at, Some("2025-01-01T00:00:00Z".to_string()));
    }

    #[test]
    fn test_node_purpose_collection_metadata_with_source() {
        let meta = NodePurposeCollectionMetadata::new()
            .with_source("automated analysis");
        assert_eq!(meta.source, Some("automated analysis".to_string()));
    }

    #[test]
    fn test_node_purpose_collection_metadata_with_note() {
        let meta = NodePurposeCollectionMetadata::new()
            .with_note("Note 1")
            .with_note("Note 2");
        assert_eq!(meta.notes, vec!["Note 1", "Note 2"]);
    }

    // =============================================================================
    // infer_node_type Tests
    // =============================================================================

    #[test]
    fn test_infer_node_type_entry_patterns() {
        assert_eq!(infer_node_type("entry_point", false, false, false), NodeType::EntryPoint);
        assert_eq!(infer_node_type("start_node", false, false, false), NodeType::EntryPoint);
        assert_eq!(infer_node_type("input_handler", false, false, false), NodeType::EntryPoint);
    }

    #[test]
    fn test_infer_node_type_exit_patterns() {
        assert_eq!(infer_node_type("exit_node", false, false, false), NodeType::ExitPoint);
        assert_eq!(infer_node_type("end_point", false, false, false), NodeType::ExitPoint);
        assert_eq!(infer_node_type("output_node", false, false, false), NodeType::ExitPoint);
        assert_eq!(infer_node_type("response_handler", false, false, false), NodeType::ExitPoint);
    }

    #[test]
    fn test_infer_node_type_router_patterns() {
        assert_eq!(infer_node_type("route_decision", false, false, false), NodeType::Router);
        assert_eq!(infer_node_type("main_router", false, false, false), NodeType::Router);
        assert_eq!(infer_node_type("dispatch_node", false, false, false), NodeType::Router);
    }

    #[test]
    fn test_infer_node_type_human_patterns() {
        assert_eq!(infer_node_type("human_review", false, false, false), NodeType::HumanInLoop);
        assert_eq!(infer_node_type("approval_step", false, false, false), NodeType::HumanInLoop);
        assert_eq!(infer_node_type("review_node", false, false, false), NodeType::HumanInLoop);
    }

    #[test]
    fn test_infer_node_type_validator_patterns() {
        // Note: "validate_input" would match "input" -> EntryPoint, so we use names without conflicting patterns
        assert_eq!(infer_node_type("validate_data", false, false, false), NodeType::Validator);
        assert_eq!(infer_node_type("guard_node", false, false, false), NodeType::Validator);
        assert_eq!(infer_node_type("check_permissions", false, false, false), NodeType::Validator);
    }

    #[test]
    fn test_infer_node_type_aggregator_patterns() {
        assert_eq!(infer_node_type("aggregator", false, false, false), NodeType::Aggregator);
        // Note: "merge_results" would match "end" in merge -> ExitPoint, so we use names without conflicting patterns
        assert_eq!(infer_node_type("my_merge", false, false, false), NodeType::Aggregator);
        assert_eq!(infer_node_type("data_combine", false, false, false), NodeType::Aggregator);
    }

    #[test]
    fn test_infer_node_type_transform_patterns() {
        assert_eq!(infer_node_type("transform_data", false, false, false), NodeType::Transform);
        // Note: "convert_format" could match "vert" but doesn't; "parse_response" matches "response" -> ExitPoint
        assert_eq!(infer_node_type("convert_format", false, false, false), NodeType::Transform);
        assert_eq!(infer_node_type("data_parse", false, false, false), NodeType::Transform);
    }

    #[test]
    fn test_infer_node_type_from_behavior_routing() {
        assert_eq!(infer_node_type("generic_node", false, false, true), NodeType::Router);
    }

    #[test]
    fn test_infer_node_type_from_behavior_llm() {
        assert_eq!(infer_node_type("generic_node", true, false, false), NodeType::LlmNode);
    }

    #[test]
    fn test_infer_node_type_from_behavior_tool() {
        assert_eq!(infer_node_type("generic_node", false, true, false), NodeType::ToolNode);
    }

    #[test]
    fn test_infer_node_type_default() {
        assert_eq!(infer_node_type("generic_node", false, false, false), NodeType::Processing);
    }

    #[test]
    fn test_infer_node_type_name_takes_precedence() {
        // Name pattern should override behavior flags
        assert_eq!(infer_node_type("entry_with_llm", true, false, false), NodeType::EntryPoint);
        assert_eq!(infer_node_type("router_with_tool", false, true, false), NodeType::Router);
    }

    // =============================================================================
    // Serialization Tests
    // =============================================================================

    #[test]
    fn test_node_purpose_serialization_roundtrip() {
        let original = NodePurpose::builder("test")
            .purpose("Test purpose")
            .node_type(NodeType::LlmNode)
            .build();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: NodePurpose = serde_json::from_str(&json).unwrap();

        assert_eq!(original.node_name, deserialized.node_name);
        assert_eq!(original.purpose, deserialized.purpose);
        assert_eq!(original.node_type, deserialized.node_type);
    }

    #[test]
    fn test_external_call_type_serde() {
        let types = vec![
            ExternalCallType::LlmApi,
            ExternalCallType::ToolExecution,
            ExternalCallType::HttpRequest,
            ExternalCallType::DatabaseQuery,
            ExternalCallType::VectorStoreOp,
            ExternalCallType::MessageQueue,
            ExternalCallType::FileSystem,
            ExternalCallType::ExternalApi,
            ExternalCallType::Other,
        ];

        for call_type in types {
            let json = serde_json::to_string(&call_type).unwrap();
            let deserialized: ExternalCallType = serde_json::from_str(&json).unwrap();
            assert_eq!(call_type, deserialized);
        }
    }

    #[test]
    fn test_node_type_serde() {
        let types = vec![
            NodeType::EntryPoint,
            NodeType::ExitPoint,
            NodeType::Processing,
            NodeType::Router,
            NodeType::LlmNode,
            NodeType::ToolNode,
            NodeType::Transform,
            NodeType::HumanInLoop,
            NodeType::Aggregator,
            NodeType::Validator,
        ];

        for node_type in types {
            let json = serde_json::to_string(&node_type).unwrap();
            let deserialized: NodeType = serde_json::from_str(&json).unwrap();
            assert_eq!(node_type, deserialized);
        }
    }

    #[test]
    fn test_node_purpose_collection_serialization() {
        let mut collection = NodePurposeCollection::for_graph("test");
        collection.add(NodePurpose::new("node1", "Node 1"));
        collection.add(NodePurpose::new("node2", "Node 2"));

        let json = serde_json::to_string(&collection).unwrap();
        let deserialized: NodePurposeCollection = serde_json::from_str(&json).unwrap();

        assert_eq!(collection.len(), deserialized.len());
        assert_eq!(collection.graph_id, deserialized.graph_id);
    }
}
