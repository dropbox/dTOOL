// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for MCP self-doc
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! # MCP Self-Documentation Protocol
//!
//! This module enables DashFlow applications to expose themselves as MCP servers
//! for AI-to-AI understanding and self-introspection.
//!
//! ## Overview
//!
//! Every DashFlow app can be an MCP server that describes itself:
//! - AI agents can query "what are you?" and get structured responses
//! - Progressive disclosure: tl;dr -> detailed -> implementation-level
//! - Standardized format that all DashFlow apps follow
//!
//! ## Features
//!
//! - **CLI Help Generation**: Auto-generate `--help`, `--help-more`, `--help-implementation`
//! - **MCP Server**: Expose `/mcp/about`, `/mcp/capabilities`, `/mcp/architecture`, etc.
//! - **Progressive Disclosure**: Three levels of detail for different audiences
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, mcp_self_doc::{HelpLevel, McpSelfDocServer}};
//!
//! // Generate CLI help
//! let compiled = graph.compile()?;
//! let help = compiled.generate_help(HelpLevel::Brief);
//! println!("{}", help);
//!
//! // Start MCP self-documentation server
//! let server = McpSelfDocServer::new(compiled.introspect(), 8080);
//! server.start().await?;
//! ```

// Submodules
mod help;
mod response_types;

// Re-exports
pub use help::*;
pub use response_types::*;

use crate::executor::GraphIntrospection;
use crate::live_introspection::{
    ExecutionState, ExecutionStep, ExecutionTracker, LiveExecutionMetrics, StepOutcome,
};
use crate::platform_introspection::PlatformIntrospection;
use crate::self_improvement::{HypothesisTracker, IntrospectionStorage};
use std::net::SocketAddr;
use std::sync::Arc;

// ============================================================================
// Schema Version Constants
// ============================================================================

/// Current schema version for MCP Self-Documentation Protocol.
/// Format: MAJOR.MINOR.PATCH
/// - MAJOR: Breaking changes to response structure
/// - MINOR: New fields added (backward compatible)
/// - PATCH: Bug fixes, documentation changes
pub const SCHEMA_VERSION: &str = "1.0.0";

/// Protocol identifier for DashFlow MCP Self-Documentation.
pub const PROTOCOL_ID: &str = "dashflow-self-doc";

/// Full protocol version string.
pub const PROTOCOL_VERSION: &str = "dashflow-self-doc/1.0";

// ============================================================================
// Standard Node Metadata Fields
// ============================================================================

/// Standard metadata keys that all nodes should use for consistency.
pub mod node_metadata_keys {
    /// Node version (semver format). Default: "1.0.0"
    pub const VERSION: &str = "version";
    /// Node author/owner. Optional.
    pub const AUTHOR: &str = "author";
    /// Creation timestamp (ISO 8601). Optional.
    pub const CREATED_AT: &str = "created_at";
    /// Last updated timestamp (ISO 8601). Optional.
    pub const UPDATED_AT: &str = "updated_at";
    /// Node category for organization. Optional.
    pub const CATEGORY: &str = "category";
    /// Tags for filtering/search. Optional.
    pub const TAGS: &str = "tags";
    /// Deprecation notice. Optional.
    pub const DEPRECATED: &str = "deprecated";
    /// Deprecation replacement. Optional (used with deprecated).
    pub const DEPRECATED_REPLACEMENT: &str = "deprecated_replacement";
}

/// Standard metadata keys that all graphs should use for consistency.
pub mod graph_metadata_keys {
    /// Graph description. Required for good documentation.
    pub const DESCRIPTION: &str = "description";
    /// Graph version (semver format). Default: "1.0.0"
    pub const VERSION: &str = "version";
    /// Graph author/owner. Optional.
    pub const AUTHOR: &str = "author";
    /// License. Optional.
    pub const LICENSE: &str = "license";
    /// Repository URL. Optional.
    pub const REPOSITORY: &str = "repository";
    /// Homepage URL. Optional.
    pub const HOMEPAGE: &str = "homepage";
}

/// MCP Self-Documentation Server.
///
/// Exposes DashFlow application introspection via HTTP endpoints.
///
/// # Tool Registration
///
/// Tools can be registered to appear in MCP responses:
///
/// ```rust,ignore
/// use dashflow::introspection::ToolManifest;
///
/// let server = compiled.mcp_server(8080)
///     .with_app_name("My Agent")
///     .with_tool(ToolManifest::new("search", "Search the web")
///         .with_category("web")
///         .with_parameter("query", "string", "Search query", true))
///     .with_tool(ToolManifest::new("calculate", "Perform calculations")
///         .with_category("math"));
/// ```
#[derive(Clone)]
pub struct McpSelfDocServer {
    introspection: Arc<GraphIntrospection>,
    port: u16,
    /// Application name override
    app_name: Option<String>,
    /// Application version override
    app_version: Option<String>,
    /// Additional tools registered with the server
    additional_tools: Vec<crate::introspection::ToolManifest>,
    /// Execution tracker for live introspection
    execution_tracker: Option<ExecutionTracker>,
}

impl McpSelfDocServer {
    /// Create a new MCP self-documentation server.
    #[must_use]
    pub fn new(introspection: GraphIntrospection, port: u16) -> Self {
        Self {
            introspection: Arc::new(introspection),
            port,
            app_name: None,
            app_version: None,
            additional_tools: Vec::new(),
            execution_tracker: None,
        }
    }

    /// Attach an execution tracker for live introspection.
    ///
    /// This enables the `/mcp/live/*` endpoints for querying runtime state.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::live_introspection::ExecutionTracker;
    ///
    /// let tracker = ExecutionTracker::new();
    /// let server = compiled.mcp_server(8080)
    ///     .with_execution_tracker(tracker.clone());
    ///
    /// // Pass tracker to executor for live updates
    /// // The tracker is thread-safe and can be shared
    /// ```
    #[must_use]
    pub fn with_execution_tracker(mut self, tracker: ExecutionTracker) -> Self {
        self.execution_tracker = Some(tracker);
        self
    }

    /// Get the execution tracker, if attached.
    #[must_use]
    pub fn execution_tracker(&self) -> Option<&ExecutionTracker> {
        self.execution_tracker.as_ref()
    }

    /// Set custom application name.
    #[must_use]
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    /// Set custom application version.
    #[must_use]
    pub fn with_app_version(mut self, version: impl Into<String>) -> Self {
        self.app_version = Some(version.into());
        self
    }

    /// Register a tool that this application uses.
    ///
    /// Tools registered here will appear in MCP responses for
    /// `/mcp/about`, `/mcp/capabilities`, and tool-related queries.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::introspection::ToolManifest;
    ///
    /// let server = compiled.mcp_server(8080)
    ///     .with_tool(ToolManifest::new("web_search", "Search the web")
    ///         .with_category("search")
    ///         .with_parameter("query", "string", "Search query", true)
    ///         .with_parameter("limit", "number", "Max results", false));
    /// ```
    #[must_use]
    pub fn with_tool(mut self, tool: crate::introspection::ToolManifest) -> Self {
        self.additional_tools.push(tool);
        self
    }

    /// Register multiple tools at once.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::introspection::ToolManifest;
    ///
    /// let tools = vec![
    ///     ToolManifest::new("search", "Search the web"),
    ///     ToolManifest::new("calculate", "Perform calculations"),
    /// ];
    /// let server = compiled.mcp_server(8080).with_tools(tools);
    /// ```
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<crate::introspection::ToolManifest>) -> Self {
        self.additional_tools.extend(tools);
        self
    }

    /// Get all tools (from introspection + additionally registered).
    fn all_tools(&self) -> Vec<&crate::introspection::ToolManifest> {
        self.introspection
            .capabilities
            .tools
            .iter()
            .chain(self.additional_tools.iter())
            .collect()
    }

    /// Get the socket address.
    #[must_use]
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.port))
    }

    /// Generate `/mcp/about` response.
    #[must_use]
    pub fn about_response(&self) -> McpAboutResponse {
        let manifest = &self.introspection.manifest;

        McpAboutResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            protocol: PROTOCOL_VERSION.to_string(),
            name: self
                .app_name
                .clone()
                .or(manifest.graph_name.clone())
                .unwrap_or_else(|| "DashFlow Application".to_string()),
            version: self
                .app_version
                .clone()
                .unwrap_or_else(|| "1.0.0".to_string()),
            description: manifest
                .metadata
                .custom
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| "A DashFlow-powered AI agent".to_string()),
            capabilities: self.all_tools().iter().map(|t| t.name.clone()).collect(),
            dashflow_version: self.introspection.platform.version.clone(),
        }
    }

    /// Generate `/mcp/capabilities` response.
    #[must_use]
    pub fn capabilities_response(&self) -> McpCapabilitiesResponse {
        let tools = self
            .all_tools()
            .iter()
            .map(|t| {
                // Convert Vec<ToolParameter> to JSON schema
                let params_json = serde_json::json!({
                    "type": "object",
                    "properties": t.parameters.iter().map(|p| {
                        (p.name.clone(), serde_json::json!({
                            "type": p.param_type.clone(),
                            "description": &p.description
                        }))
                    }).collect::<std::collections::HashMap<_, _>>()
                });
                McpToolInfo {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: params_json,
                }
            })
            .collect();

        let nodes = self
            .introspection
            .manifest
            .nodes
            .iter()
            .map(|(name, node)| {
                // Extract version from metadata
                let version = node
                    .metadata
                    .get(node_metadata_keys::VERSION)
                    .and_then(|v| v.as_str())
                    .map(String::from);

                // Build metadata map for additional fields
                let metadata = if node.metadata.is_empty() {
                    None
                } else {
                    Some(node.metadata.clone())
                };

                McpNodeInfo {
                    name: name.clone(),
                    node_type: format!("{:?}", node.node_type).to_lowercase(),
                    description: node.description.clone(),
                    tools: node.tools_available.clone(),
                    version,
                    metadata,
                }
            })
            .collect();

        let features = self
            .introspection
            .architecture
            .dashflow_features_used
            .iter()
            .map(|f| f.name.clone())
            .collect();

        McpCapabilitiesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            tools,
            nodes,
            features,
        }
    }

    /// Generate `/mcp/architecture` response.
    #[must_use]
    pub fn architecture_response(&self) -> McpArchitectureResponse {
        let manifest = &self.introspection.manifest;
        let arch = &self.introspection.architecture;

        let graph = McpGraphInfo {
            entry_point: manifest.entry_point.clone(),
            node_count: manifest.node_count(),
            edge_count: manifest.edge_count(),
            terminal_nodes: manifest
                .terminal_nodes()
                .into_iter()
                .map(String::from)
                .collect(),
            decision_points: manifest
                .decision_points()
                .into_iter()
                .map(String::from)
                .collect(),
            has_cycles: Some(arch.graph_structure.has_cycles),
            has_parallel_paths: Some(arch.graph_structure.has_parallel_edges),
        };

        let dashflow_features = self
            .introspection
            .architecture
            .dashflow_features_used
            .iter()
            .map(|f| format!("{} ({})", f.name, f.description))
            .collect();

        let execution_flow = format!(
            "Execution starts at '{}', passes through {} nodes, and terminates at one of: {}",
            manifest.entry_point,
            manifest.node_count(),
            manifest
                .terminal_nodes()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        );

        McpArchitectureResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            graph,
            dashflow_features,
            execution_flow,
        }
    }

    /// Generate `/mcp/implementation` response.
    #[must_use]
    pub fn implementation_response(&self) -> McpImplementationResponse {
        let mut node_versions = std::collections::HashMap::new();
        for (name, node) in &self.introspection.manifest.nodes {
            let version = node
                .metadata
                .get(node_metadata_keys::VERSION)
                .and_then(|v| v.as_str())
                .unwrap_or("1.0.0");
            node_versions.insert(name.clone(), version.to_string());
        }

        let dependencies = self
            .introspection
            .architecture
            .dependencies
            .iter()
            .map(|d| McpDependencyInfo {
                name: d.name.clone(),
                version: d.version.clone().unwrap_or_else(|| "unknown".to_string()),
                purpose: d.purpose.clone(),
                is_dashflow: d.is_dashflow,
            })
            .collect();

        McpImplementationResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            node_versions,
            dashflow_version: self.introspection.platform.version.clone(),
            dependencies,
        }
    }

    /// Generate `/mcp/nodes` response.
    ///
    /// Lists all nodes in the graph with summary information.
    #[must_use]
    pub fn nodes_list_response(&self) -> McpNodesListResponse {
        let manifest = &self.introspection.manifest;
        let terminals: std::collections::HashSet<&str> =
            manifest.terminal_nodes().into_iter().collect();

        let nodes: Vec<McpNodeSummary> = manifest
            .nodes
            .iter()
            .map(|(name, node)| McpNodeSummary {
                name: name.clone(),
                node_type: format!("{:?}", node.node_type).to_lowercase(),
                description: node.description.clone(),
                is_entry_point: name == &manifest.entry_point,
                is_terminal: terminals.contains(name.as_str()),
            })
            .collect();

        let total_count = nodes.len();

        McpNodesListResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            nodes,
            total_count,
        }
    }

    /// Generate `/mcp/nodes/:name` response for a specific node.
    ///
    /// Returns `None` if the node doesn't exist.
    #[must_use]
    pub fn node_detail_response(&self, node_name: &str) -> Option<McpNodeDetailResponse> {
        let manifest = &self.introspection.manifest;
        let node = manifest.nodes.get(node_name)?;

        let terminals: std::collections::HashSet<&str> =
            manifest.terminal_nodes().into_iter().collect();

        // Find incoming edges (edges where this node is the target)
        // edges is HashMap<source_node, Vec<EdgeManifest>>
        let incoming_edges: Vec<String> = manifest
            .edges
            .iter()
            .flat_map(|(source, edges)| {
                edges
                    .iter()
                    .filter(|e| e.to == node_name)
                    .map(move |_| source.clone())
            })
            .collect();

        // Find outgoing edges (edges where this node is the source)
        let outgoing_edges: Vec<String> = manifest
            .edges
            .get(node_name)
            .map(|edges| edges.iter().map(|e| e.to.clone()).collect())
            .unwrap_or_default();

        // Get version from metadata
        let version = node
            .metadata
            .get(node_metadata_keys::VERSION)
            .and_then(|v| v.as_str())
            .map(String::from);

        // Convert metadata to JSON
        let metadata = if node.metadata.is_empty() {
            None
        } else {
            Some(
                node.metadata
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                    .collect(),
            )
        };

        Some(McpNodeDetailResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            name: node_name.to_string(),
            node_type: format!("{:?}", node.node_type).to_lowercase(),
            description: node.description.clone(),
            version,
            incoming_edges,
            outgoing_edges,
            tools: node.tools_available.clone(),
            is_entry_point: node_name == manifest.entry_point,
            is_terminal: terminals.contains(node_name),
            metadata,
        })
    }

    /// Generate `/mcp/features` response.
    ///
    /// Lists DashFlow features used by this application with configuration details.
    #[must_use]
    pub fn features_response(&self) -> McpFeaturesResponse {
        // Get platform introspection for opt-out methods
        let platform = PlatformIntrospection::discover();
        let platform_features = platform.available_features();

        let features: Vec<McpFeatureInfo> = self
            .introspection
            .architecture
            .dashflow_features_used
            .iter()
            .map(|f| {
                // Try to find matching platform feature for opt-out method
                let opt_out_method = platform_features
                    .iter()
                    .find(|pf| pf.name.eq_ignore_ascii_case(&f.name))
                    .and_then(|pf| pf.opt_out_method().map(String::from));

                // Build configuration object from APIs used
                let configuration = if f.apis_used.is_empty() {
                    None
                } else {
                    Some(serde_json::json!({
                        "category": f.category,
                        "apis_used": f.apis_used,
                        "is_core": f.is_core,
                    }))
                };

                McpFeatureInfo {
                    name: f.name.clone(),
                    description: f.description.clone(),
                    enabled: true,
                    configuration,
                    opt_out_method,
                }
            })
            .collect();

        let total_count = features.len();

        McpFeaturesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            features,
            total_count,
        }
    }

    /// Generate `/mcp/dependencies` response.
    ///
    /// Lists all dependencies (DashFlow and external) used by this application.
    #[must_use]
    pub fn dependencies_response(&self) -> McpDependenciesResponse {
        let dependencies: Vec<McpDependencyInfo> = self
            .introspection
            .architecture
            .dependencies
            .iter()
            .map(|d| McpDependencyInfo {
                name: d.name.clone(),
                version: d.version.clone().unwrap_or_else(|| "unknown".to_string()),
                purpose: d.purpose.clone(),
                is_dashflow: d.is_dashflow,
            })
            .collect();

        let dashflow_count = dependencies.iter().filter(|d| d.is_dashflow).count();
        let external_count = dependencies.iter().filter(|d| !d.is_dashflow).count();
        let total_count = dependencies.len();

        McpDependenciesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            dependencies,
            total_count,
            dashflow_count,
            external_count,
        }
    }

    /// Generate `/mcp/edges` response.
    ///
    /// Lists all edges (connections) in the DashFlow graph.
    #[must_use]
    pub fn edges_response(&self) -> McpEdgesResponse {
        let manifest = &self.introspection.manifest;

        let edges: Vec<McpEdgeInfo> = manifest
            .edges
            .iter()
            .flat_map(|(source, edge_list)| {
                edge_list.iter().map(move |edge| McpEdgeInfo {
                    from: source.clone(),
                    to: edge.to.clone(),
                    is_conditional: edge.is_conditional,
                    condition: edge.condition_label.clone(),
                })
            })
            .collect();

        let conditional_count = edges.iter().filter(|e| e.is_conditional).count();
        let total_count = edges.len();

        McpEdgesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            edges,
            total_count,
            conditional_count,
        }
    }

    // ========================================================================
    // App Introspection Enhancement Endpoints
    // ========================================================================

    /// Generate `/mcp/tools` response.
    ///
    /// Lists all tools available to this application, combining tools from
    /// the capability manifest and any additional tools registered with the server.
    #[must_use]
    pub fn tools_response(&self) -> McpToolsResponse {
        // Combine tools from capability manifest and additional registered tools
        let mut all_tools: Vec<McpAppToolInfo> = self
            .introspection
            .capabilities
            .tools
            .iter()
            .map(|t| McpAppToolInfo {
                name: t.name.clone(),
                description: t.description.clone(),
                category: t.category.clone(),
                parameters: t
                    .parameters
                    .iter()
                    .map(|p| McpToolParameter {
                        name: p.name.clone(),
                        param_type: p.param_type.clone(),
                        description: p.description.clone(),
                        required: p.required,
                        default: p.default_value.clone(),
                    })
                    .collect(),
                returns: t.returns.clone(),
                has_side_effects: t.has_side_effects,
                requires_confirmation: t.requires_confirmation,
            })
            .collect();

        // Add additional tools registered with the server
        for tool in &self.additional_tools {
            all_tools.push(McpAppToolInfo {
                name: tool.name.clone(),
                description: tool.description.clone(),
                category: tool.category.clone(),
                parameters: tool
                    .parameters
                    .iter()
                    .map(|p| McpToolParameter {
                        name: p.name.clone(),
                        param_type: p.param_type.clone(),
                        description: p.description.clone(),
                        required: p.required,
                        default: p.default_value.clone(),
                    })
                    .collect(),
                returns: tool.returns.clone(),
                has_side_effects: tool.has_side_effects,
                requires_confirmation: tool.requires_confirmation,
            });
        }

        // Collect unique categories
        let mut categories: Vec<String> = all_tools
            .iter()
            .filter_map(|t| t.category.clone())
            .collect();
        categories.sort();
        categories.dedup();

        let total_count = all_tools.len();

        McpToolsResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            tools: all_tools,
            total_count,
            categories,
        }
    }

    /// Generate `/mcp/state-schema` response.
    ///
    /// Describes the state schema used by this application.
    #[must_use]
    pub fn state_schema_response(&self) -> McpStateSchemaResponse {
        let manifest = &self.introspection.manifest;

        match &manifest.state_schema {
            Some(schema) => McpStateSchemaResponse {
                schema_version: SCHEMA_VERSION.to_string(),
                has_schema: true,
                state_type_name: Some(schema.type_name.clone()),
                description: schema.description.clone(),
                fields: schema
                    .fields
                    .iter()
                    .map(|f| McpFieldInfo {
                        name: f.name.clone(),
                        field_type: f.field_type.clone(),
                        optional: f.optional,
                        description: f.description.clone(),
                    })
                    .collect(),
            },
            None => McpStateSchemaResponse {
                schema_version: SCHEMA_VERSION.to_string(),
                has_schema: false,
                state_type_name: None,
                description: None,
                fields: Vec::new(),
            },
        }
    }

    // ========================================================================
    // Platform Introspection Endpoints
    // ========================================================================

    /// Generate `/mcp/platform/version` response.
    #[must_use]
    pub fn platform_version_response(&self) -> McpPlatformVersionResponse {
        let platform = PlatformIntrospection::discover();
        let info = platform.version_info();

        McpPlatformVersionResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            version: info.version.clone(),
            rust_version: info.rust_version.clone(),
            features_enabled: info.features_enabled.clone(),
            build_timestamp: info.build_timestamp.clone(),
        }
    }

    /// Generate `/mcp/platform/features` response.
    #[must_use]
    pub fn platform_features_response(&self) -> McpPlatformFeaturesResponse {
        let platform = PlatformIntrospection::discover();
        let features: Vec<McpPlatformFeatureInfo> = platform
            .available_features()
            .iter()
            .map(|f| McpPlatformFeatureInfo {
                name: f.name.clone(),
                description: f.description.clone(),
                default_enabled: f.default_enabled(),
                opt_out_method: f.opt_out_method().map(String::from),
            })
            .collect();

        let total_count = features.len();

        McpPlatformFeaturesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            features,
            total_count,
        }
    }

    /// Generate `/mcp/platform/node-types` response.
    #[must_use]
    pub fn platform_node_types_response(&self) -> McpPlatformNodeTypesResponse {
        let platform = PlatformIntrospection::discover();
        let node_types: Vec<McpPlatformNodeTypeInfo> = platform
            .supported_node_types()
            .iter()
            .map(|n| McpPlatformNodeTypeInfo {
                name: n.name.clone(),
                description: n.description.clone(),
                example: n.example.clone(),
                is_builtin: n.is_builtin,
            })
            .collect();

        let total_count = node_types.len();

        McpPlatformNodeTypesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            node_types,
            total_count,
        }
    }

    /// Generate `/mcp/platform/edge-types` response.
    #[must_use]
    pub fn platform_edge_types_response(&self) -> McpPlatformEdgeTypesResponse {
        let platform = PlatformIntrospection::discover();
        let edge_types: Vec<McpPlatformEdgeTypeInfo> = platform
            .supported_edge_types()
            .iter()
            .map(|e| McpPlatformEdgeTypeInfo {
                name: e.name.clone(),
                description: e.description.clone(),
                example: e.example.clone(),
            })
            .collect();

        let total_count = edge_types.len();

        McpPlatformEdgeTypesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            edge_types,
            total_count,
        }
    }

    /// Generate `/mcp/platform/templates` response.
    #[must_use]
    pub fn platform_templates_response(&self) -> McpPlatformTemplatesResponse {
        let platform = PlatformIntrospection::discover();
        let templates: Vec<McpPlatformTemplateInfo> = platform
            .built_in_templates()
            .iter()
            .map(|t| McpPlatformTemplateInfo {
                name: t.name.clone(),
                description: t.description.clone(),
                use_cases: t.use_cases.clone(),
                example: t.example.clone(),
            })
            .collect();

        let total_count = templates.len();

        McpPlatformTemplatesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            templates,
            total_count,
        }
    }

    /// Generate `/mcp/platform/states` response.
    #[must_use]
    pub fn platform_states_response(&self) -> McpPlatformStatesResponse {
        let platform = PlatformIntrospection::discover();
        let states: Vec<McpPlatformStateInfo> = platform
            .state_implementations()
            .iter()
            .map(|s| McpPlatformStateInfo {
                name: s.name.clone(),
                description: s.description.clone(),
                is_builtin: s.is_builtin,
            })
            .collect();

        let total_count = states.len();

        McpPlatformStatesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            states,
            total_count,
        }
    }

    /// Handle `/mcp/platform/query` for querying platform capabilities.
    #[must_use]
    pub fn handle_platform_query(&self, query_str: &str) -> McpPlatformQueryResponse {
        let platform = PlatformIntrospection::discover();

        if let Some(cap) = platform.query_capability(query_str) {
            McpPlatformQueryResponse {
                answer: format!("{} ({}) - {}", cap.name, cap.category, cap.description),
                category: Some(cap.category),
                available: cap.available,
                confidence: 0.95,
            }
        } else {
            McpPlatformQueryResponse {
                answer: format!(
                    "No capability found matching '{}'. Try querying: checkpointing, streaming, \
                    function, agent, conditional, supervisor, react_agent",
                    query_str
                ),
                category: None,
                available: false,
                confidence: 0.3,
            }
        }
    }

    /// Generate `/mcp/hypotheses` response for the hypothesis dashboard.
    ///
    /// Provides a view of the AI learning system's hypothesis tracking,
    /// including accuracy statistics, active hypotheses, and insights.
    #[must_use]
    pub fn hypotheses_response(&self) -> McpHypothesesResponse {
        // Create storage and tracker
        let storage = IntrospectionStorage::default();
        let tracker = HypothesisTracker::with_storage(storage.clone());

        // Get accuracy stats
        let accuracy_stats = tracker.accuracy_stats().unwrap_or_default();

        // Convert to MCP format
        let by_source: Vec<McpSourceAccuracy> = accuracy_stats
            .by_source
            .into_iter()
            .map(|(source, stats)| McpSourceAccuracy {
                source,
                accuracy: stats.accuracy,
                total: stats.total,
                correct: stats.correct,
            })
            .collect();

        let accuracy = McpHypothesisAccuracy {
            overall: accuracy_stats.accuracy,
            total_evaluated: accuracy_stats.total_evaluated,
            correct: accuracy_stats.correct,
            incorrect: accuracy_stats.incorrect,
            active_count: accuracy_stats.active_count,
            by_source,
        };

        // Get active hypotheses
        let active_hypotheses: Vec<McpHypothesisInfo> = tracker
            .get_active_hypotheses()
            .unwrap_or_default()
            .into_iter()
            .take(10) // Limit for dashboard
            .map(|h| McpHypothesisInfo {
                id: h.id.to_string(),
                statement: h.statement.clone(),
                source: h.source.to_string(),
                status: format!("{:?}", h.status),
                created_at: h.created_at.to_rfc3339(),
                evaluated: false,
                outcome: None,
            })
            .collect();

        // Get recent evaluations
        let recent_evaluations: Vec<McpHypothesisInfo> = tracker
            .get_evaluated_hypotheses()
            .unwrap_or_default()
            .into_iter()
            .take(10) // Limit for dashboard
            .map(|h| McpHypothesisInfo {
                id: h.id.to_string(),
                statement: h.statement.clone(),
                source: h.source.to_string(),
                status: format!("{:?}", h.status),
                created_at: h.created_at.to_rfc3339(),
                evaluated: true,
                outcome: h.outcome.map(|o| McpHypothesisOutcome {
                    correct: o.correct,
                    analysis: o.analysis,
                    improvements: o.improvements_for_future,
                }),
            })
            .collect();

        // Generate insights
        let mut insights = Vec::new();

        if accuracy_stats.total_evaluated > 0 {
            if accuracy_stats.accuracy >= 0.8 {
                insights.push(format!(
                    "High prediction accuracy ({:.0}%): hypothesis generation is well-calibrated.",
                    accuracy_stats.accuracy * 100.0
                ));
            } else if accuracy_stats.accuracy < 0.5 {
                insights.push(format!(
                    "Low prediction accuracy ({:.0}%): consider more conservative estimates.",
                    accuracy_stats.accuracy * 100.0
                ));
            }

            // Add source-specific insights
            for stats in &accuracy.by_source {
                if stats.total >= 3 && stats.accuracy < 0.5 {
                    insights.push(format!(
                        "{} hypotheses have low accuracy ({:.0}%): review hypothesis generation for this source.",
                        stats.source,
                        stats.accuracy * 100.0
                    ));
                }
            }
        }

        if accuracy_stats.active_count > 10 {
            insights.push(format!(
                "{} active hypotheses: consider evaluating more frequently.",
                accuracy_stats.active_count
            ));
        }

        McpHypothesesResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            accuracy,
            active_hypotheses,
            recent_evaluations,
            insights,
        }
    }

    /// Handle a natural language query about the application.
    ///
    /// Supports a wide range of natural language patterns including:
    /// - Node queries: "what nodes?", "list nodes", "show me the nodes"
    /// - Tool queries: "what tools?", "available tools", "capabilities"
    /// - Flow queries: "how does it work?", "execution flow", "what happens?"
    /// - Entry point: "where does it start?", "entry point", "beginning"
    /// - Terminal nodes: "where does it end?", "terminal nodes", "exit points"
    /// - Edge queries: "connections", "edges", "how are nodes connected?"
    /// - Feature queries: "what features?", "dashflow features used"
    /// - Version queries: "version", "what version?"
    /// - Specific node: "tell me about `<node_name>`", "what does `<node>` do?"
    #[must_use]
    pub fn handle_query(&self, query: &McpQueryRequest) -> McpQueryResponse {
        let question = query.question.to_lowercase();
        let words: Vec<&str> = question.split_whitespace().collect();

        // Try to match specific patterns in order of specificity
        let (answer, sources, confidence) = self
            .try_specific_node_query(&question, &words)
            .or_else(|| self.try_entry_point_query(&question))
            .or_else(|| self.try_terminal_nodes_query(&question))
            .or_else(|| self.try_edges_query(&question))
            .or_else(|| self.try_features_query(&question))
            .or_else(|| self.try_nodes_query(&question))
            .or_else(|| self.try_tools_query(&question))
            .or_else(|| self.try_how_work_query(&question))
            .or_else(|| self.try_version_query(&question))
            .or_else(|| self.try_description_query(&question))
            .or_else(|| self.try_count_query(&question))
            .unwrap_or_else(|| self.fallback_response());

        McpQueryResponse {
            answer,
            sources,
            confidence,
        }
    }

    /// Try to match a query about a specific node by name.
    fn try_specific_node_query(
        &self,
        question: &str,
        _words: &[&str],
    ) -> Option<(String, Vec<String>, f64)> {
        // Only match if the question explicitly asks about a specific node
        // Patterns: "about {node}", "what does {node} do", "describe {node}", "the {node} node"
        // Avoid matching generic questions like "what nodes do you have"

        // Don't match if asking about nodes in general
        let generic_patterns = [
            "what node",
            "all node",
            "list node",
            "show node",
            "which node",
            "nodes do you have",
        ];
        if generic_patterns.iter().any(|p| question.contains(p)) {
            return None;
        }

        let node_names: Vec<&String> = self.introspection.manifest.nodes.keys().collect();

        // Find if any node name appears in the question with specific context
        let matching_node = node_names.iter().find(|name| {
            let name_lower = name.to_lowercase();
            // Require explicit mention of the node name
            // and some indicator that we're asking about this specific node
            if !question.contains(&name_lower) {
                return false;
            }
            // Must have some query intent about this specific node
            question.contains("about")
                || question.contains("describe")
                || question.contains("what does")
                || question.contains("tell me")
                || question.contains(&format!("{} node", name_lower))
                || question.contains(&format!("node {}", name_lower))
        });

        if let Some(node_name) = matching_node {
            let node = self.introspection.manifest.nodes.get(*node_name)?;
            let node_type = format!("{:?}", node.node_type).to_lowercase();
            let description = node
                .description
                .as_deref()
                .unwrap_or("No description available");
            let version = node
                .metadata
                .get(node_metadata_keys::VERSION)
                .and_then(|v| v.as_str())
                .unwrap_or("1.0.0");

            let tools_info = if node.tools_available.is_empty() {
                String::new()
            } else {
                format!(" Tools: {}.", node.tools_available.join(", "))
            };

            return Some((
                format!(
                    "Node '{}' (type: {}, version: {}): {}.{}",
                    node_name, node_type, version, description, tools_info
                ),
                vec![format!("manifest.nodes.{}", node_name)],
                0.95,
            ));
        }
        None
    }

    /// Try to match entry point queries.
    fn try_entry_point_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = [
            "entry point",
            "entry_point",
            "start",
            "begin",
            "first node",
            "initial",
            "where does it start",
            "where does execution start",
            "starting point",
        ];

        if patterns.iter().any(|p| question.contains(p)) {
            let entry = &self.introspection.manifest.entry_point;
            let node = self.introspection.manifest.nodes.get(entry);
            let desc = node
                .and_then(|n| n.description.as_deref())
                .unwrap_or("the starting point");

            return Some((
                format!("Execution starts at node '{}', which is {}.", entry, desc),
                vec!["manifest.entry_point".to_string()],
                0.95,
            ));
        }
        None
    }

    /// Try to match terminal node queries.
    fn try_terminal_nodes_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = [
            "terminal",
            "end node",
            "exit",
            "where does it end",
            "final node",
            "last node",
            "finish",
            "completion",
            "endpoints",
        ];

        if patterns.iter().any(|p| question.contains(p)) {
            let terminals = self.introspection.manifest.terminal_nodes();
            if terminals.is_empty() {
                return Some((
                    "This graph has no explicit terminal nodes (may use __end__ implicitly)."
                        .to_string(),
                    vec!["manifest.edges".to_string()],
                    0.8,
                ));
            }

            return Some((
                format!(
                    "Execution can end at {} terminal node(s): {}.",
                    terminals.len(),
                    terminals.join(", ")
                ),
                vec!["manifest.terminal_nodes".to_string()],
                0.95,
            ));
        }
        None
    }

    /// Try to match edge/connection queries.
    fn try_edges_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = [
            "edge",
            "connection",
            "connect",
            "link",
            "flow between",
            "how are nodes connected",
            "graph structure",
            "routing",
        ];

        if patterns.iter().any(|p| question.contains(p)) {
            let edge_count = self.introspection.manifest.edge_count();
            let decision_points = self.introspection.manifest.decision_points();
            let has_conditionals = !decision_points.is_empty();

            let conditional_info = if has_conditionals {
                format!(
                    " Decision points (conditional routing): {}.",
                    decision_points.join(", ")
                )
            } else {
                " No conditional routing.".to_string()
            };

            return Some((
                format!(
                    "The graph has {} edge(s) connecting {} node(s).{}",
                    edge_count,
                    self.introspection.manifest.node_count(),
                    conditional_info
                ),
                vec!["manifest.edges".to_string()],
                0.9,
            ));
        }
        None
    }

    /// Try to match DashFlow feature queries.
    fn try_features_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = [
            "feature",
            "dashflow feature",
            "what feature",
            "which feature",
            "using which",
            "built with",
        ];

        if patterns.iter().any(|p| question.contains(p)) {
            let features = &self.introspection.architecture.dashflow_features_used;
            if features.is_empty() {
                return Some((
                    "This application uses core DashFlow StateGraph functionality.".to_string(),
                    vec!["architecture.dashflow_features_used".to_string()],
                    0.8,
                ));
            }

            let feature_list: Vec<String> = features
                .iter()
                .map(|f| format!("{} ({})", f.name, f.description))
                .collect();

            return Some((
                format!("DashFlow features used: {}.", feature_list.join("; ")),
                vec!["architecture.dashflow_features_used".to_string()],
                0.9,
            ));
        }
        None
    }

    /// Try to match node listing queries.
    fn try_nodes_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = [
            "what node",
            "list node",
            "show node",
            "all node",
            "which node",
            "nodes do you have",
            "available node",
        ];

        if patterns.iter().any(|p| question.contains(p)) {
            let nodes: Vec<_> = self.introspection.manifest.nodes.keys().collect();
            return Some((
                format!(
                    "This application has {} node(s): {}.",
                    nodes.len(),
                    nodes.into_iter().cloned().collect::<Vec<_>>().join(", ")
                ),
                vec!["manifest.nodes".to_string()],
                0.9,
            ));
        }
        None
    }

    /// Try to match tool listing queries.
    fn try_tools_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = [
            "what tool",
            "list tool",
            "show tool",
            "available tool",
            "which tool",
            "tools do you have",
            "capabilit",
        ];

        if patterns.iter().any(|p| question.contains(p)) {
            let tools = self.all_tools();

            if tools.is_empty() {
                return Some((
                    "This application has no explicitly registered tools.".to_string(),
                    vec!["capabilities.tools".to_string()],
                    0.85,
                ));
            }

            return Some((
                format!(
                    "This application has {} tool(s): {}.",
                    tools.len(),
                    tools
                        .iter()
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                vec!["capabilities.tools".to_string()],
                0.9,
            ));
        }
        None
    }

    /// Try to match "how does it work" queries.
    fn try_how_work_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = [
            "how does",
            "how do you",
            "how it work",
            "what does it do",
            "what do you do",
            "explain",
            "describe the app",
            "overview",
            "summary",
        ];

        if patterns.iter().any(|p| question.contains(p)) {
            let about = self.about_response();
            let terminals = self.introspection.manifest.terminal_nodes();
            let terminal_str = if terminals.is_empty() {
                "__end__".to_string()
            } else {
                terminals.join(" or ")
            };

            return Some((
                format!(
                    "{} is a DashFlow application. {} It starts at node '{}', \
                     processes through {} node(s), and terminates at {}.",
                    about.name,
                    about.description,
                    self.introspection.manifest.entry_point,
                    self.introspection.manifest.node_count(),
                    terminal_str
                ),
                vec!["manifest".to_string(), "architecture".to_string()],
                0.85,
            ));
        }
        None
    }

    /// Try to match version queries.
    fn try_version_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        if question.contains("version") {
            return Some((
                format!(
                    "DashFlow version: {}. Application version: {}.",
                    self.introspection.platform.version,
                    self.app_version.as_deref().unwrap_or("1.0.0")
                ),
                vec!["platform.version".to_string()],
                0.95,
            ));
        }
        None
    }

    /// Try to match description queries.
    fn try_description_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = [
            "what are you",
            "who are you",
            "what is this",
            "describe yourself",
            "introduce",
            "about yourself",
        ];

        if patterns.iter().any(|p| question.contains(p)) {
            let about = self.about_response();
            return Some((
                format!(
                    "I am {}, version {}. {}",
                    about.name, about.version, about.description
                ),
                vec!["about".to_string()],
                0.9,
            ));
        }
        None
    }

    /// Try to match count/statistics queries.
    fn try_count_query(&self, question: &str) -> Option<(String, Vec<String>, f64)> {
        let patterns = ["how many", "count", "number of", "total"];

        if patterns.iter().any(|p| question.contains(p)) {
            let node_count = self.introspection.manifest.node_count();
            let edge_count = self.introspection.manifest.edge_count();
            let tool_count = self.all_tools().len();

            return Some((
                format!(
                    "Statistics: {} node(s), {} edge(s), {} tool(s).",
                    node_count, edge_count, tool_count
                ),
                vec!["manifest".to_string(), "capabilities".to_string()],
                0.85,
            ));
        }
        None
    }

    /// Fallback response when no pattern matches.
    fn fallback_response(&self) -> (String, Vec<String>, f64) {
        let node_names: Vec<_> = self
            .introspection
            .manifest
            .nodes
            .keys()
            .take(3)
            .cloned()
            .collect();
        let node_examples = if node_names.is_empty() {
            String::new()
        } else {
            format!(
                " Or ask about specific nodes like '{}'.",
                node_names.join("', '")
            )
        };

        (
            format!(
                "I can answer questions about: nodes, tools, entry point, terminal nodes, \
                 edges, features, versions, and how I work. \
                 Try: 'What nodes do you have?', 'Where does execution start?', \
                 'How does this app work?', or 'What version?'.{}",
                node_examples
            ),
            vec![],
            0.3,
        )
    }

    // ========================================================================
    // Live Execution Introspection Methods
    // ========================================================================

    /// Generate `/mcp/live/executions` response.
    ///
    /// Returns a list of all active and recent executions.
    #[must_use]
    pub fn live_executions_response(&self) -> McpLiveExecutionsResponse {
        match &self.execution_tracker {
            Some(tracker) => {
                let executions: Vec<McpExecutionSummary> = tracker
                    .all_executions()
                    .into_iter()
                    .map(|s| McpExecutionSummary {
                        execution_id: s.execution_id,
                        graph_name: s.graph_name,
                        started_at: s.started_at,
                        current_node: s.current_node,
                        iteration: s.iteration,
                        status: s.status.to_string(),
                    })
                    .collect();

                McpLiveExecutionsResponse {
                    schema_version: SCHEMA_VERSION.to_string(),
                    executions,
                    active_count: tracker.active_count(),
                    total_count: tracker.total_count(),
                }
            }
            None => McpLiveExecutionsResponse {
                schema_version: SCHEMA_VERSION.to_string(),
                executions: vec![],
                active_count: 0,
                total_count: 0,
            },
        }
    }

    /// Generate `/mcp/live/executions/:id` response.
    ///
    /// Returns detailed state for a specific execution.
    #[must_use]
    pub fn live_execution_detail_response(
        &self,
        execution_id: &str,
    ) -> Option<McpLiveExecutionDetailResponse> {
        let tracker = self.execution_tracker.as_ref()?;
        let state = tracker.get_execution(execution_id)?;

        Some(self.state_to_detail_response(state))
    }

    /// Convert an ExecutionState to a detail response.
    fn state_to_detail_response(&self, state: ExecutionState) -> McpLiveExecutionDetailResponse {
        McpLiveExecutionDetailResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            execution_id: state.execution_id,
            graph_name: state.graph_name,
            started_at: state.started_at,
            current_node: state.current_node,
            previous_node: state.previous_node,
            iteration: state.iteration,
            total_nodes_visited: state.total_nodes_visited,
            state: state.state,
            metrics: self.metrics_to_mcp(&state.metrics),
            checkpoint: self.checkpoint_to_mcp(&state.checkpoint),
            status: state.status.to_string(),
            error: state.error,
        }
    }

    /// Convert LiveExecutionMetrics to MCP response format.
    fn metrics_to_mcp(&self, metrics: &LiveExecutionMetrics) -> McpExecutionMetrics {
        McpExecutionMetrics {
            total_duration_ms: metrics.total_duration_ms,
            nodes_executed: metrics.nodes_executed,
            nodes_succeeded: metrics.nodes_succeeded,
            nodes_failed: metrics.nodes_failed,
            avg_node_duration_ms: metrics.avg_node_duration_ms,
            slowest_node: metrics
                .slowest_node
                .as_ref()
                .map(|(name, duration)| McpSlowestNode {
                    name: name.clone(),
                    duration_ms: *duration,
                }),
            iteration: metrics.iteration,
        }
    }

    /// Convert CheckpointStatusInfo to MCP response format.
    fn checkpoint_to_mcp(
        &self,
        checkpoint: &crate::live_introspection::CheckpointStatusInfo,
    ) -> McpCheckpointStatus {
        McpCheckpointStatus {
            enabled: checkpoint.enabled,
            thread_id: checkpoint.thread_id.clone(),
            last_checkpoint_at: checkpoint.last_checkpoint_at.clone(),
            checkpoint_count: checkpoint.checkpoint_count,
            total_size_bytes: checkpoint.total_size_bytes,
        }
    }

    /// Generate `/mcp/live/executions/:id/node` response.
    #[must_use]
    pub fn live_current_node_response(
        &self,
        execution_id: &str,
    ) -> Option<McpLiveCurrentNodeResponse> {
        let tracker = self.execution_tracker.as_ref()?;
        let state = tracker.get_execution(execution_id)?;

        Some(McpLiveCurrentNodeResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            execution_id: state.execution_id,
            current_node: state.current_node,
            previous_node: state.previous_node,
        })
    }

    /// Generate `/mcp/live/executions/:id/state` response.
    #[must_use]
    pub fn live_current_state_response(
        &self,
        execution_id: &str,
    ) -> Option<McpLiveCurrentStateResponse> {
        let tracker = self.execution_tracker.as_ref()?;
        let state = tracker.current_state(execution_id)?;

        Some(McpLiveCurrentStateResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            execution_id: execution_id.to_string(),
            state,
        })
    }

    /// Generate `/mcp/live/executions/:id/history` response.
    #[must_use]
    pub fn live_history_response(&self, execution_id: &str) -> Option<McpLiveHistoryResponse> {
        let tracker = self.execution_tracker.as_ref()?;

        // Verify execution exists
        tracker.get_execution(execution_id)?;

        let history = tracker.execution_history(execution_id);
        let steps: Vec<McpExecutionStep> = history
            .into_iter()
            .map(|step| self.step_to_mcp(&step))
            .collect();

        let total_steps = steps.len();

        Some(McpLiveHistoryResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            execution_id: execution_id.to_string(),
            steps,
            total_steps,
        })
    }

    /// Convert ExecutionStep to MCP response format.
    fn step_to_mcp(&self, step: &ExecutionStep) -> McpExecutionStep {
        McpExecutionStep {
            step_number: step.step_number,
            node_name: step.node_name.clone(),
            started_at: step.started_at.clone(),
            completed_at: step.completed_at.clone(),
            duration_ms: step.duration_ms,
            outcome: match &step.outcome {
                StepOutcome::Success => "success".to_string(),
                StepOutcome::Error(e) => format!("error: {e}"),
                StepOutcome::Skipped => "skipped".to_string(),
                StepOutcome::InProgress => "in_progress".to_string(),
            },
        }
    }

    /// Generate `/mcp/live/executions/:id/metrics` response.
    #[must_use]
    pub fn live_metrics_response(&self, execution_id: &str) -> Option<McpLiveMetricsResponse> {
        let tracker = self.execution_tracker.as_ref()?;
        let metrics = tracker.execution_metrics(execution_id)?;

        Some(McpLiveMetricsResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            execution_id: execution_id.to_string(),
            metrics: self.metrics_to_mcp(&metrics),
        })
    }

    /// Generate `/mcp/live/executions/:id/checkpoint` response.
    #[must_use]
    pub fn live_checkpoint_response(
        &self,
        execution_id: &str,
    ) -> Option<McpLiveCheckpointResponse> {
        let tracker = self.execution_tracker.as_ref()?;
        let checkpoint = tracker.checkpoint_status(execution_id)?;

        Some(McpLiveCheckpointResponse {
            schema_version: SCHEMA_VERSION.to_string(),
            execution_id: execution_id.to_string(),
            checkpoint: self.checkpoint_to_mcp(&checkpoint),
        })
    }

    /// Start the MCP server (requires axum feature).
    ///
    /// # Errors
    ///
    /// Returns error if the server fails to start.
    #[cfg(feature = "mcp-server")]
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use axum::{
            routing::{get, post},
            Router,
        };

        let app = Router::new()
            // App-level endpoints
            .route("/mcp/about", get(handle_about))
            .route("/mcp/capabilities", get(handle_capabilities))
            .route("/mcp/architecture", get(handle_architecture))
            .route("/mcp/implementation", get(handle_implementation))
            .route("/mcp/nodes", get(handle_nodes_list))
            .route("/mcp/nodes/:name", get(handle_node_detail))
            .route("/mcp/features", get(handle_features))
            .route("/mcp/dependencies", get(handle_dependencies))
            .route("/mcp/edges", get(handle_edges))
            .route("/mcp/tools", get(handle_tools))
            .route("/mcp/state-schema", get(handle_state_schema))
            .route("/mcp/introspect", post(handle_query))
            // Platform-level endpoints
            .route("/mcp/platform/version", get(handle_platform_version))
            .route("/mcp/platform/features", get(handle_platform_features))
            .route("/mcp/platform/node-types", get(handle_platform_node_types))
            .route("/mcp/platform/edge-types", get(handle_platform_edge_types))
            .route("/mcp/platform/templates", get(handle_platform_templates))
            .route("/mcp/platform/states", get(handle_platform_states))
            .route("/mcp/platform/query", get(handle_platform_query))
            // Self-improvement hypothesis dashboard
            .route("/mcp/hypotheses", get(handle_hypotheses))
            // Live execution introspection endpoints
            .route("/mcp/live/executions", get(handle_live_executions))
            .route(
                "/mcp/live/executions/:id",
                get(handle_live_execution_detail),
            )
            .route(
                "/mcp/live/executions/:id/node",
                get(handle_live_current_node),
            )
            .route(
                "/mcp/live/executions/:id/state",
                get(handle_live_current_state),
            )
            .route("/mcp/live/executions/:id/history", get(handle_live_history))
            .route("/mcp/live/executions/:id/metrics", get(handle_live_metrics))
            .route(
                "/mcp/live/executions/:id/checkpoint",
                get(handle_live_checkpoint),
            )
            .route("/mcp/live/executions/:id/events", get(handle_live_events))
            .route("/mcp/live/events", get(handle_live_all_events))
            .with_state(self.clone());

        let listener = tokio::net::TcpListener::bind(self.socket_addr()).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

// Axum handlers (only when mcp-server feature is enabled)
#[cfg(feature = "mcp-server")]
async fn handle_about(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpAboutResponse> {
    axum::Json(server.about_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_capabilities(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpCapabilitiesResponse> {
    axum::Json(server.capabilities_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_architecture(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpArchitectureResponse> {
    axum::Json(server.architecture_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_implementation(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpImplementationResponse> {
    axum::Json(server.implementation_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_query(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::Json(query): axum::Json<McpQueryRequest>,
) -> axum::Json<McpQueryResponse> {
    axum::Json(server.handle_query(&query))
}

#[cfg(feature = "mcp-server")]
async fn handle_nodes_list(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpNodesListResponse> {
    axum::Json(server.nodes_list_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_node_detail(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match server.node_detail_response(&name) {
        Some(detail) => axum::Json(detail).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Node not found",
                "node_name": name
            })),
        )
            .into_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn handle_features(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpFeaturesResponse> {
    axum::Json(server.features_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_dependencies(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpDependenciesResponse> {
    axum::Json(server.dependencies_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_edges(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpEdgesResponse> {
    axum::Json(server.edges_response())
}

// App introspection enhancement handlers
#[cfg(feature = "mcp-server")]
async fn handle_tools(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpToolsResponse> {
    axum::Json(server.tools_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_state_schema(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpStateSchemaResponse> {
    axum::Json(server.state_schema_response())
}

// Platform introspection handlers
#[cfg(feature = "mcp-server")]
async fn handle_platform_version(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpPlatformVersionResponse> {
    axum::Json(server.platform_version_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_platform_features(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpPlatformFeaturesResponse> {
    axum::Json(server.platform_features_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_platform_node_types(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpPlatformNodeTypesResponse> {
    axum::Json(server.platform_node_types_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_platform_edge_types(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpPlatformEdgeTypesResponse> {
    axum::Json(server.platform_edge_types_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_platform_templates(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpPlatformTemplatesResponse> {
    axum::Json(server.platform_templates_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_platform_states(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpPlatformStatesResponse> {
    axum::Json(server.platform_states_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_platform_query(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::Json<McpPlatformQueryResponse> {
    let query = params.get("q").map(|s| s.as_str()).unwrap_or("");
    axum::Json(server.handle_platform_query(query))
}

// Self-improvement hypothesis dashboard handler
#[cfg(feature = "mcp-server")]
async fn handle_hypotheses(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpHypothesesResponse> {
    axum::Json(server.hypotheses_response())
}

// Live execution introspection handlers
#[cfg(feature = "mcp-server")]
async fn handle_live_executions(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::Json<McpLiveExecutionsResponse> {
    axum::Json(server.live_executions_response())
}

#[cfg(feature = "mcp-server")]
async fn handle_live_execution_detail(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match server.live_execution_detail_response(&id) {
        Some(detail) => axum::Json(detail).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Execution not found",
                "execution_id": id
            })),
        )
            .into_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn handle_live_current_node(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match server.live_current_node_response(&id) {
        Some(response) => axum::Json(response).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Execution not found",
                "execution_id": id
            })),
        )
            .into_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn handle_live_current_state(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match server.live_current_state_response(&id) {
        Some(response) => axum::Json(response).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Execution not found",
                "execution_id": id
            })),
        )
            .into_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn handle_live_history(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match server.live_history_response(&id) {
        Some(response) => axum::Json(response).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Execution not found",
                "execution_id": id
            })),
        )
            .into_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn handle_live_metrics(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match server.live_metrics_response(&id) {
        Some(response) => axum::Json(response).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Execution not found",
                "execution_id": id
            })),
        )
            .into_response(),
    }
}

#[cfg(feature = "mcp-server")]
async fn handle_live_checkpoint(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match server.live_checkpoint_response(&id) {
        Some(response) => axum::Json(response).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Execution not found",
                "execution_id": id
            })),
        )
            .into_response(),
    }
}

/// Handle GET /mcp/live/executions/:id/events - SSE stream for execution events
#[cfg(feature = "mcp-server")]
async fn handle_live_events(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::sse::{Event, KeepAlive, Sse};
    use axum::response::IntoResponse;
    use futures::stream::StreamExt;
    use std::convert::Infallible;
    use tokio_stream::wrappers::BroadcastStream;

    let Some(tracker) = server.execution_tracker() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "error": "Execution tracking not enabled"
            })),
        )
            .into_response();
    };

    // Verify execution exists
    if tracker.get_execution(&id).is_none() {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "Execution not found",
                "execution_id": id
            })),
        )
            .into_response();
    }

    // Subscribe to events for this execution
    let stream = tracker.subscribe_to_execution(&id);

    // Convert to SSE stream
    let sse_stream = BroadcastStream::new(stream.receiver).filter_map(move |result| {
        let id_clone = id.clone();
        async move {
            match result {
                Ok(event) => {
                    // Filter to only this execution's events
                    if event.execution_id() == id_clone {
                        let json = serde_json::to_string(&event).ok()?;
                        Some(Ok::<_, Infallible>(
                            Event::default().event(event.event_type()).data(json),
                        ))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }
    });

    Sse::new(sse_stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Handle GET /mcp/live/events - SSE stream for all execution events
#[cfg(feature = "mcp-server")]
async fn handle_live_all_events(
    axum::extract::State(server): axum::extract::State<McpSelfDocServer>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::sse::{Event, KeepAlive, Sse};
    use axum::response::IntoResponse;
    use futures::stream::StreamExt;
    use std::convert::Infallible;
    use tokio_stream::wrappers::BroadcastStream;

    let Some(tracker) = server.execution_tracker() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "error": "Execution tracking not enabled"
            })),
        )
            .into_response();
    };

    // Subscribe to all events
    let stream = tracker.subscribe();

    // Convert to SSE stream
    let sse_stream = BroadcastStream::new(stream.receiver).filter_map(|result| async {
        match result {
            Ok(event) => {
                let json = serde_json::to_string(&event).ok()?;
                Some(Ok::<_, Infallible>(
                    Event::default().event(event.event_type()).data(json),
                ))
            }
            Err(_) => None,
        }
    });

    Sse::new(sse_stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

// ============================================================================
// Tests
// ============================================================================

// The tests module contains both:
// 1. Unit tests (gated by #[cfg(test)] inside the file)
// 2. UnifiedMcpServer and related types (gated by #[cfg(feature = "mcp-server")])
mod tests;

// Re-export UnifiedMcpServer for CLI usage
#[cfg(feature = "mcp-server")]
pub use tests::UnifiedMcpServer;
