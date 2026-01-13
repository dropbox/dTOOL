//! # MCP (Model Context Protocol) Integration
//!
//! Utilities for integrating MCP servers and tools with DashFlow workflows.
//!
//! The Model Context Protocol (MCP) is an emerging standard for connecting LLMs to
//! external tools, data sources, and contexts. This module provides:
//!
//! - **`McpResponse`**: Structured response type preserving MIME types and resources
//! - **`McpToolRegistry`**: Registry for dynamically discovering and registering MCP tools
//! - **`McpTool`**: Tool wrapper that preserves MCP semantic information
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::core::mcp::{McpToolRegistry, McpServerConfig};
//!
//! // Connect to MCP server and discover tools
//! let registry = McpToolRegistry::from_config(McpServerConfig {
//!     command: "npx".to_string(),
//!     args: vec!["-y", "@modelcontextprotocol/server-filesystem"],
//!     env: HashMap::new(),
//! }).await?;
//!
//! // Get tool definitions for LLM
//! let tool_definitions = registry.to_tool_definitions();
//!
//! // Execute a tool and get structured response
//! let response = registry.call_tool("read_file", json!({"path": "/tmp/file.txt"})).await?;
//! match response {
//!     McpResponse::Text(text) => println!("Text: {}", text),
//!     McpResponse::Resource { uri, mime_type, content } => {
//!         println!("Resource: {} ({})", uri, mime_type);
//!     }
//! }
//! ```

use crate::constants::DEFAULT_TIMEOUT_MS;
use crate::core::error::{Error, Result};
use crate::core::language_models::ToolDefinition;
use crate::core::tools::{Tool, ToolInput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// MCP server configuration for tool discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Command to spawn the MCP server (e.g., "npx", "python")
    pub command: String,

    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables for the server process
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory for the server
    #[serde(default)]
    pub working_dir: Option<String>,

    /// Connection timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_MS
}

impl McpServerConfig {
    /// Create a new config for a command-based MCP server.
    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            timeout_ms: default_timeout(),
        }
    }

    /// Add arguments to the server command.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Add environment variables.
    #[must_use]
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Set the working directory.
    #[must_use]
    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set the connection timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// Structured MCP response that preserves semantic information.
///
/// MCP tools can return various types of content. This enum preserves the
/// structure and metadata instead of flattening everything to strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpResponse {
    /// Plain text response.
    Text {
        /// The text content
        text: String,
    },

    /// Resource with URI and MIME type.
    Resource {
        /// Resource URI (e.g., "file:///path/to/file")
        uri: String,
        /// MIME type of the content (e.g., "text/plain", "application/json")
        mime_type: String,
        /// The resource content
        content: McpContent,
    },

    /// Image content.
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type (e.g., "image/png", "image/jpeg")
        mime_type: String,
    },

    /// Embedded resource reference (for prompts).
    EmbeddedResource {
        /// Resource URI to embed
        uri: String,
        /// Optional annotation or description
        annotation: Option<String>,
    },

    /// Multiple content items.
    Multi {
        /// List of content items
        items: Vec<McpResponse>,
    },

    /// Error response from the tool.
    Error {
        /// Error code
        code: i32,
        /// Error message
        message: String,
    },
}

impl McpResponse {
    /// Create a text response.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create a resource response.
    #[must_use]
    pub fn resource(
        uri: impl Into<String>,
        mime_type: impl Into<String>,
        content: McpContent,
    ) -> Self {
        Self::Resource {
            uri: uri.into(),
            mime_type: mime_type.into(),
            content,
        }
    }

    /// Create an image response.
    #[must_use]
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }

    /// Create an error response.
    #[must_use]
    pub fn error(code: i32, message: impl Into<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
        }
    }

    /// Check if this is an error response.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Convert to a string representation for LLM context.
    ///
    /// This flattens the response to a string while preserving useful metadata.
    #[must_use]
    pub fn to_string_lossy(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Resource {
                uri,
                mime_type,
                content,
            } => {
                format!(
                    "[Resource: {} ({})]\n{}",
                    uri,
                    mime_type,
                    content.to_string_lossy()
                )
            }
            Self::Image { mime_type, .. } => {
                format!("[Image: {}]", mime_type)
            }
            Self::EmbeddedResource { uri, annotation } => {
                if let Some(ann) = annotation {
                    format!("[Embedded: {} - {}]", uri, ann)
                } else {
                    format!("[Embedded: {}]", uri)
                }
            }
            Self::Multi { items } => items
                .iter()
                .map(|item| item.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n---\n"),
            Self::Error { code, message } => {
                format!("[Error {}: {}]", code, message)
            }
        }
    }

    /// Extract text content if this is a text response.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Extract resource URI if this is a resource response.
    #[must_use]
    pub fn as_resource_uri(&self) -> Option<&str> {
        match self {
            Self::Resource { uri, .. } => Some(uri),
            Self::EmbeddedResource { uri, .. } => Some(uri),
            _ => None,
        }
    }
}

/// Content type for MCP resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpContent {
    /// Text content
    Text(String),
    /// Binary content (base64-encoded)
    Binary(Vec<u8>),
    /// JSON content
    Json(serde_json::Value),
}

impl McpContent {
    /// Convert to string representation.
    #[must_use]
    pub fn to_string_lossy(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Binary(bytes) => {
                format!("[Binary: {} bytes]", bytes.len())
            }
            Self::Json(value) => {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            }
        }
    }
}

/// MCP tool definition from server discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: serde_json::Value,
}

impl McpToolDefinition {
    /// Convert to a standard DashFlow ToolDefinition.
    #[must_use]
    pub fn to_tool_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
        }
    }
}

/// Callback type for MCP tool execution.
///
/// Takes the tool name and input, returns the structured response.
pub type McpToolCallback = Arc<
    dyn Fn(
            &str,
            serde_json::Value,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<McpResponse>> + Send>>
        + Send
        + Sync,
>;

/// Registry for MCP tools discovered from a server.
///
/// Provides automatic tool discovery and structured response handling.
pub struct McpToolRegistry {
    /// Discovered tool definitions
    tools: Vec<McpToolDefinition>,
    /// Tool execution callback (for actual server communication)
    callback: Option<McpToolCallback>,
    /// Server configuration (for reference)
    config: Option<McpServerConfig>,
    /// Mock responses for testing
    mock_responses: Arc<RwLock<HashMap<String, McpResponse>>>,
    /// Per-call timeout in milliseconds (M-201: enforced during tool execution)
    timeout_ms: u64,
}

impl std::fmt::Debug for McpToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpToolRegistry")
            .field("tools", &self.tools)
            .field("config", &self.config)
            .field("has_callback", &self.callback.is_some())
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

impl Default for McpToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl McpToolRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            callback: None,
            config: None,
            mock_responses: Arc::new(RwLock::new(HashMap::new())),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Create a registry with pre-defined tool definitions.
    ///
    /// Useful for testing or when tool definitions are known ahead of time.
    #[must_use]
    pub fn with_tools(tools: Vec<McpToolDefinition>) -> Self {
        Self {
            tools,
            callback: None,
            config: None,
            mock_responses: Arc::new(RwLock::new(HashMap::new())),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Set the per-call timeout in milliseconds.
    ///
    /// This timeout is enforced during tool execution (M-201). If a tool call
    /// exceeds this timeout, it will be cancelled and an error returned.
    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Get the current timeout setting in milliseconds.
    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Set the tool execution callback.
    ///
    /// The callback is invoked when a tool is called, passing the tool name
    /// and input. It should return the structured MCP response.
    #[must_use]
    pub fn with_callback(mut self, callback: McpToolCallback) -> Self {
        self.callback = Some(callback);
        self
    }

    /// Add a single tool definition.
    pub fn add_tool(&mut self, tool: McpToolDefinition) {
        self.tools.push(tool);
    }

    /// Register a mock response for testing.
    ///
    /// # Security Note (M-211)
    /// This method is only available in test builds or when the `testing` feature is enabled.
    /// This prevents mock responses from being injected in production, which could bypass
    /// real tool execution.
    #[cfg(any(test, feature = "testing"))]
    pub async fn mock_response(&self, tool_name: &str, response: McpResponse) {
        let mut mocks = self.mock_responses.write().await;
        mocks.insert(tool_name.to_string(), response);
    }

    /// Get all registered tool definitions.
    #[must_use]
    pub fn tools(&self) -> &[McpToolDefinition] {
        &self.tools
    }

    /// Get tool by name.
    #[must_use]
    pub fn get_tool(&self, name: &str) -> Option<&McpToolDefinition> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Convert all tools to standard DashFlow ToolDefinitions.
    #[must_use]
    pub fn to_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.to_tool_definition()).collect()
    }

    /// Execute a tool by name with given input.
    ///
    /// Returns a structured McpResponse preserving MIME types and resources.
    ///
    /// # Timeout (M-201)
    ///
    /// This method enforces the configured `timeout_ms` for tool execution.
    /// If the tool call exceeds the timeout, it will be cancelled and an error returned.
    /// The timeout is applied to the callback execution only, not mock responses.
    pub async fn call_tool(&self, name: &str, input: serde_json::Value) -> Result<McpResponse> {
        // SECURITY (M-211): Mock response checking is only available in test builds.
        // This prevents production code from having mock responses injected that could
        // bypass real tool execution.
        #[cfg(any(test, feature = "testing"))]
        {
            let mocks = self.mock_responses.read().await;
            if let Some(response) = mocks.get(name) {
                return Ok(response.clone());
            }
        }

        // Check if tool exists
        if self.get_tool(name).is_none() {
            return Err(Error::tool_error(format!("Unknown MCP tool: {}", name)));
        }

        // Use callback if available, with timeout enforcement (M-201)
        if let Some(ref callback) = self.callback {
            let timeout = Duration::from_millis(self.timeout_ms);
            let future = callback(name, input);

            match tokio::time::timeout(timeout, future).await {
                Ok(result) => result,
                Err(_elapsed) => {
                    // Timeout occurred - the future is dropped which cancels it
                    tracing::warn!(
                        tool = %name,
                        timeout_ms = %self.timeout_ms,
                        "MCP tool call timed out"
                    );
                    Err(Error::tool_error(format!(
                        "MCP tool '{}' timed out after {}ms",
                        name, self.timeout_ms
                    )))
                }
            }
        } else {
            Err(Error::tool_error(format!(
                "No callback configured for MCP tool execution. Tool: {}",
                name
            )))
        }
    }

    /// Create Tool implementations for each registered MCP tool.
    ///
    /// This allows MCP tools to be used directly with the standard Tool interface.
    /// Each tool inherits the registry's timeout setting for per-call timeout enforcement (M-201).
    #[must_use]
    pub fn to_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools
            .iter()
            .map(|def| {
                let tool: Arc<dyn Tool> = Arc::new(McpTool {
                    definition: def.clone(),
                    registry: Arc::new(RwLock::new(RegistryRef {
                        callback: self.callback.clone(),
                        mock_responses: Arc::clone(&self.mock_responses),
                        timeout_ms: self.timeout_ms,
                    })),
                });
                tool
            })
            .collect()
    }

    /// Extend tool definitions from another registry.
    pub fn extend(&mut self, other: &McpToolRegistry) {
        self.tools.extend(other.tools.clone());
    }
}

/// Internal registry reference for McpTool.
struct RegistryRef {
    callback: Option<McpToolCallback>,
    // SECURITY (M-211): Mock responses only stored for test builds.
    // Suppressed dead_code warning since this field is conditionally used.
    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    mock_responses: Arc<RwLock<HashMap<String, McpResponse>>>,
    /// Per-call timeout in milliseconds (M-201: enforced during tool execution)
    timeout_ms: u64,
}

/// Tool wrapper for MCP server tools.
///
/// Implements the standard Tool trait while preserving MCP semantics.
pub struct McpTool {
    definition: McpToolDefinition,
    registry: Arc<RwLock<RegistryRef>>,
}

impl std::fmt::Debug for McpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTool")
            .field("definition", &self.definition)
            .finish()
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn description(&self) -> &str {
        &self.definition.description
    }

    fn args_schema(&self) -> serde_json::Value {
        self.definition.input_schema.clone()
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_value = match input {
            ToolInput::String(s) => {
                // Try to parse as JSON, or wrap as string
                serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!({ "input": s }))
            }
            ToolInput::Structured(v) => v,
        };

        let registry = self.registry.read().await;

        // SECURITY (M-211): Mock response checking is only available in test builds.
        // This prevents production code from having mock responses injected that could
        // bypass real tool execution.
        #[cfg(any(test, feature = "testing"))]
        {
            let mocks = registry.mock_responses.read().await;
            if let Some(response) = mocks.get(&self.definition.name) {
                return Ok(response.to_string_lossy());
            }
        }

        // Use callback if available, with timeout enforcement (M-201)
        if let Some(ref callback) = registry.callback {
            let timeout = Duration::from_millis(registry.timeout_ms);
            let tool_name = self.definition.name.clone();
            let future = callback(&tool_name, input_value);

            match tokio::time::timeout(timeout, future).await {
                Ok(result) => {
                    let response = result?;
                    Ok(response.to_string_lossy())
                }
                Err(_elapsed) => {
                    // Timeout occurred - the future is dropped which cancels it
                    tracing::warn!(
                        tool = %tool_name,
                        timeout_ms = %registry.timeout_ms,
                        "MCP tool call timed out"
                    );
                    Err(Error::tool_error(format!(
                        "MCP tool '{}' timed out after {}ms",
                        tool_name, registry.timeout_ms
                    )))
                }
            }
        } else {
            Err(Error::tool_error(format!(
                "No callback configured for MCP tool: {}",
                self.definition.name
            )))
        }
    }
}

/// Builder for McpToolRegistry with fluent API.
pub struct McpToolRegistryBuilder {
    tools: Vec<McpToolDefinition>,
    callback: Option<McpToolCallback>,
    config: Option<McpServerConfig>,
    timeout_ms: u64,
}

impl Default for McpToolRegistryBuilder {
    fn default() -> Self {
        Self {
            tools: Vec::new(),
            callback: None,
            config: None,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

impl McpToolRegistryBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tool definition.
    #[must_use]
    pub fn add_tool(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        schema: serde_json::Value,
    ) -> Self {
        self.tools.push(McpToolDefinition {
            name: name.into(),
            description: description.into(),
            input_schema: schema,
        });
        self
    }

    /// Add multiple tool definitions.
    #[must_use]
    pub fn add_tools(mut self, tools: Vec<McpToolDefinition>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Set the server configuration.
    ///
    /// This also sets the timeout from the config's `timeout_ms` field.
    #[must_use]
    pub fn with_config(mut self, config: McpServerConfig) -> Self {
        self.timeout_ms = config.timeout_ms;
        self.config = Some(config);
        self
    }

    /// Set the tool execution callback.
    #[must_use]
    pub fn with_callback(mut self, callback: McpToolCallback) -> Self {
        self.callback = Some(callback);
        self
    }

    /// Set the per-call timeout in milliseconds (M-201).
    ///
    /// This overrides any timeout set via `with_config()`.
    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Build the registry.
    #[must_use]
    pub fn build(self) -> McpToolRegistry {
        McpToolRegistry {
            tools: self.tools,
            callback: self.callback,
            config: self.config,
            mock_responses: Arc::new(RwLock::new(HashMap::new())),
            timeout_ms: self.timeout_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_response_text() {
        let response = McpResponse::text("Hello, world!");
        assert_eq!(response.to_string_lossy(), "Hello, world!");
        assert_eq!(response.as_text(), Some("Hello, world!"));
    }

    #[test]
    fn test_mcp_response_resource() {
        let response = McpResponse::resource(
            "file:///tmp/test.txt",
            "text/plain",
            McpContent::Text("File contents".to_string()),
        );
        let output = response.to_string_lossy();
        assert!(output.contains("file:///tmp/test.txt"));
        assert!(output.contains("text/plain"));
        assert!(output.contains("File contents"));
        assert_eq!(response.as_resource_uri(), Some("file:///tmp/test.txt"));
    }

    #[test]
    fn test_mcp_response_image() {
        let response = McpResponse::image("base64data", "image/png");
        assert!(response.to_string_lossy().contains("image/png"));
    }

    #[test]
    fn test_mcp_response_error() {
        let response = McpResponse::error(-1, "Something went wrong");
        assert!(response.is_error());
        assert!(response.to_string_lossy().contains("Something went wrong"));
    }

    #[test]
    fn test_mcp_response_multi() {
        let response = McpResponse::Multi {
            items: vec![McpResponse::text("First"), McpResponse::text("Second")],
        };
        let output = response.to_string_lossy();
        assert!(output.contains("First"));
        assert!(output.contains("Second"));
    }

    #[test]
    fn test_mcp_content_text() {
        let content = McpContent::Text("Hello".to_string());
        assert_eq!(content.to_string_lossy(), "Hello");
    }

    #[test]
    fn test_mcp_content_binary() {
        let content = McpContent::Binary(vec![1, 2, 3, 4]);
        assert!(content.to_string_lossy().contains("4 bytes"));
    }

    #[test]
    fn test_mcp_content_json() {
        let content = McpContent::Json(serde_json::json!({"key": "value"}));
        let output = content.to_string_lossy();
        assert!(output.contains("key"));
        assert!(output.contains("value"));
    }

    #[test]
    fn test_mcp_tool_definition_conversion() {
        let mcp_def = McpToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                }
            }),
        };

        let tool_def = mcp_def.to_tool_definition();
        assert_eq!(tool_def.name, "test_tool");
        assert_eq!(tool_def.description, "A test tool");
    }

    #[test]
    fn test_mcp_server_config_builder() {
        let config = McpServerConfig::new("npx")
            .with_args(vec!["arg1".to_string(), "arg2".to_string()])
            .with_timeout(60000);

        assert_eq!(config.command, "npx");
        assert_eq!(config.args.len(), 2);
        assert_eq!(config.timeout_ms, 60000);
    }

    #[test]
    fn test_registry_new() {
        let registry = McpToolRegistry::new();
        assert!(registry.tools().is_empty());
    }

    #[test]
    fn test_registry_with_tools() {
        let tools = vec![
            McpToolDefinition {
                name: "tool1".to_string(),
                description: "First tool".to_string(),
                input_schema: serde_json::json!({}),
            },
            McpToolDefinition {
                name: "tool2".to_string(),
                description: "Second tool".to_string(),
                input_schema: serde_json::json!({}),
            },
        ];

        let registry = McpToolRegistry::with_tools(tools);
        assert_eq!(registry.tools().len(), 2);
        assert!(registry.get_tool("tool1").is_some());
        assert!(registry.get_tool("tool2").is_some());
        assert!(registry.get_tool("tool3").is_none());
    }

    #[test]
    fn test_registry_add_tool() {
        let mut registry = McpToolRegistry::new();
        registry.add_tool(McpToolDefinition {
            name: "new_tool".to_string(),
            description: "New tool".to_string(),
            input_schema: serde_json::json!({}),
        });

        assert_eq!(registry.tools().len(), 1);
    }

    #[test]
    fn test_registry_to_tool_definitions() {
        let tools = vec![McpToolDefinition {
            name: "test".to_string(),
            description: "Test tool".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
        }];

        let registry = McpToolRegistry::with_tools(tools);
        let definitions = registry.to_tool_definitions();

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "test");
    }

    #[tokio::test]
    async fn test_registry_mock_response() {
        let tools = vec![McpToolDefinition {
            name: "mock_tool".to_string(),
            description: "Mock tool".to_string(),
            input_schema: serde_json::json!({}),
        }];

        let registry = McpToolRegistry::with_tools(tools);
        registry
            .mock_response("mock_tool", McpResponse::text("Mocked!"))
            .await;

        let response = registry.call_tool("mock_tool", serde_json::json!({})).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap().to_string_lossy(), "Mocked!");
    }

    #[tokio::test]
    async fn test_registry_unknown_tool() {
        let registry = McpToolRegistry::new();
        let result = registry.call_tool("unknown", serde_json::json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown MCP tool"));
    }

    #[test]
    fn test_registry_builder() {
        let registry = McpToolRegistryBuilder::new()
            .add_tool(
                "builder_tool",
                "Built via builder",
                serde_json::json!({ "type": "object" }),
            )
            .build();

        assert_eq!(registry.tools().len(), 1);
        assert_eq!(registry.tools()[0].name, "builder_tool");
    }

    #[test]
    fn test_registry_to_tools() {
        let tools = vec![McpToolDefinition {
            name: "wrapped".to_string(),
            description: "Wrapped tool".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
        }];

        let registry = McpToolRegistry::with_tools(tools);
        let arc_tools = registry.to_tools();

        assert_eq!(arc_tools.len(), 1);
        assert_eq!(arc_tools[0].name(), "wrapped");
        assert_eq!(arc_tools[0].description(), "Wrapped tool");
    }

    #[test]
    fn test_registry_extend() {
        let mut registry1 = McpToolRegistry::with_tools(vec![McpToolDefinition {
            name: "tool1".to_string(),
            description: "Tool 1".to_string(),
            input_schema: serde_json::json!({}),
        }]);

        let registry2 = McpToolRegistry::with_tools(vec![McpToolDefinition {
            name: "tool2".to_string(),
            description: "Tool 2".to_string(),
            input_schema: serde_json::json!({}),
        }]);

        registry1.extend(&registry2);
        assert_eq!(registry1.tools().len(), 2);
    }

    #[tokio::test]
    async fn test_mcp_tool_call_with_mock() {
        let tools = vec![McpToolDefinition {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                }
            }),
        }];

        let registry = McpToolRegistry::with_tools(tools);
        registry
            .mock_response("echo", McpResponse::text("Echo response"))
            .await;

        let arc_tools = registry.to_tools();
        let echo_tool = &arc_tools[0];

        let result = echo_tool
            ._call(ToolInput::Structured(serde_json::json!({
                "message": "Hello"
            })))
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Echo response"));
    }

    #[test]
    fn test_embedded_resource_response() {
        let response = McpResponse::EmbeddedResource {
            uri: "file:///path/to/doc.md".to_string(),
            annotation: Some("Reference documentation".to_string()),
        };

        let output = response.to_string_lossy();
        assert!(output.contains("file:///path/to/doc.md"));
        assert!(output.contains("Reference documentation"));
    }

    #[test]
    fn test_response_serialization() {
        let response = McpResponse::text("Test");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("text"));

        let parsed: McpResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_text(), Some("Test"));
    }

    // M-201: Timeout enforcement tests

    #[test]
    fn test_registry_default_timeout() {
        let registry = McpToolRegistry::new();
        assert_eq!(registry.timeout_ms(), crate::constants::DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn test_registry_with_timeout() {
        let registry = McpToolRegistry::new().with_timeout(5000);
        assert_eq!(registry.timeout_ms(), 5000);
    }

    #[test]
    fn test_builder_with_timeout() {
        let registry = McpToolRegistryBuilder::new().with_timeout(10000).build();
        assert_eq!(registry.timeout_ms(), 10000);
    }

    #[test]
    fn test_builder_config_sets_timeout() {
        let config = McpServerConfig::new("npx").with_timeout(15000);
        let registry = McpToolRegistryBuilder::new().with_config(config).build();
        assert_eq!(registry.timeout_ms(), 15000);
    }

    #[test]
    fn test_builder_timeout_overrides_config() {
        let config = McpServerConfig::new("npx").with_timeout(15000);
        let registry = McpToolRegistryBuilder::new()
            .with_config(config)
            .with_timeout(5000) // Override config timeout
            .build();
        assert_eq!(registry.timeout_ms(), 5000);
    }

    #[tokio::test]
    async fn test_call_tool_timeout() {
        // Create a callback that sleeps longer than the timeout
        let slow_callback: McpToolCallback = Arc::new(|_name, _input| {
            Box::pin(async {
                // Sleep for 200ms - longer than our 50ms timeout
                tokio::time::sleep(Duration::from_millis(200)).await;
                Ok(McpResponse::text("Should not reach here"))
            })
        });

        let tools = vec![McpToolDefinition {
            name: "slow_tool".to_string(),
            description: "A slow tool".to_string(),
            input_schema: serde_json::json!({}),
        }];

        let registry = McpToolRegistry::with_tools(tools)
            .with_callback(slow_callback)
            .with_timeout(50); // 50ms timeout

        let result = registry.call_tool("slow_tool", serde_json::json!({})).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("timed out"),
            "Error should mention timeout: {}",
            err_msg
        );
        assert!(
            err_msg.contains("slow_tool"),
            "Error should mention tool name: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_call_tool_success_within_timeout() {
        // Create a callback that completes quickly
        let fast_callback: McpToolCallback = Arc::new(|_name, _input| {
            Box::pin(async {
                // Sleep for just 10ms - well within our 500ms timeout
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(McpResponse::text("Fast response"))
            })
        });

        let tools = vec![McpToolDefinition {
            name: "fast_tool".to_string(),
            description: "A fast tool".to_string(),
            input_schema: serde_json::json!({}),
        }];

        let registry = McpToolRegistry::with_tools(tools)
            .with_callback(fast_callback)
            .with_timeout(500); // 500ms timeout - plenty of time

        let result = registry.call_tool("fast_tool", serde_json::json!({})).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string_lossy(), "Fast response");
    }

    #[tokio::test]
    async fn test_mcp_tool_trait_timeout() {
        // Test timeout through the Tool trait interface
        let slow_callback: McpToolCallback = Arc::new(|_name, _input| {
            Box::pin(async {
                tokio::time::sleep(Duration::from_millis(200)).await;
                Ok(McpResponse::text("Should not reach here"))
            })
        });

        let tools = vec![McpToolDefinition {
            name: "slow_wrapped".to_string(),
            description: "A slow wrapped tool".to_string(),
            input_schema: serde_json::json!({}),
        }];

        let registry = McpToolRegistry::with_tools(tools)
            .with_callback(slow_callback)
            .with_timeout(50); // 50ms timeout

        let arc_tools = registry.to_tools();
        let tool = &arc_tools[0];

        let result = tool
            ._call(ToolInput::Structured(serde_json::json!({})))
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("timed out"),
            "Error should mention timeout: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_mcp_tool_trait_success_within_timeout() {
        // Test success through the Tool trait interface
        let fast_callback: McpToolCallback = Arc::new(|_name, _input| {
            Box::pin(async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(McpResponse::text("Fast wrapped response"))
            })
        });

        let tools = vec![McpToolDefinition {
            name: "fast_wrapped".to_string(),
            description: "A fast wrapped tool".to_string(),
            input_schema: serde_json::json!({}),
        }];

        let registry = McpToolRegistry::with_tools(tools)
            .with_callback(fast_callback)
            .with_timeout(500);

        let arc_tools = registry.to_tools();
        let tool = &arc_tools[0];

        let result = tool
            ._call(ToolInput::Structured(serde_json::json!({})))
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Fast wrapped response"));
    }
}
