//! MCP client implementation using the rmcp SDK

use crate::config::{McpServerConfig, McpTransport};
use crate::error::McpError;
use crate::types::{McpContent, McpTool, McpToolResult};

use rmcp::model::CallToolRequestParam;
use rmcp::service::{serve_client, RunningService};
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::StreamableHttpClientTransport;
use std::collections::HashMap;
use std::ffi::OsString;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, info};

/// Handler for MCP client events (logging, etc.)
#[derive(Clone, Default)]
struct ClientHandler;

impl rmcp::handler::client::ClientHandler for ClientHandler {}

/// State of a connected MCP server
struct ConnectedServer {
    service: Arc<RunningService<rmcp::service::RoleClient, ClientHandler>>,
    tools: Vec<McpTool>,
}

/// MCP client for managing connections to MCP servers
pub struct McpClient {
    /// Connected servers by name
    servers: RwLock<HashMap<String, ConnectedServer>>,
}

impl McpClient {
    /// Create a new MCP client
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
        }
    }

    /// Connect to an MCP server
    pub async fn connect(&self, config: &McpServerConfig) -> Result<(), McpError> {
        let timeout_duration = Duration::from_secs(config.timeout_secs);

        match &config.transport {
            McpTransport::Stdio { command, args } => {
                self.connect_stdio(config, command, args, timeout_duration)
                    .await
            }
            McpTransport::Http {
                url,
                bearer_token,
                headers,
            } => {
                self.connect_http(
                    config,
                    url,
                    bearer_token.as_deref(),
                    headers,
                    timeout_duration,
                )
                .await
            }
        }
    }

    /// Connect to a stdio-based MCP server
    async fn connect_stdio(
        &self,
        config: &McpServerConfig,
        command: &str,
        args: &[String],
        timeout_duration: Duration,
    ) -> Result<(), McpError> {
        info!("Connecting to MCP server '{}' via stdio", config.name);

        // Build environment
        let mut envs: HashMap<OsString, OsString> = std::env::vars_os().collect();
        for (key, value) in &config.env {
            envs.insert(key.into(), value.into());
        }

        // Build command
        let mut cmd = Command::new(command);
        cmd.kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .envs(envs)
            .args(args);

        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }

        // Spawn process
        let (transport, _stderr) = TokioChildProcess::builder(cmd)
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| McpError::SpawnError {
                name: config.name.clone(),
                source: e,
            })?;

        // Create client handler
        let handler = ClientHandler;

        // Initialize the service
        let service = timeout(timeout_duration, serve_client(handler, transport))
            .await
            .map_err(|_| McpError::Timeout {
                name: config.name.clone(),
                timeout_secs: config.timeout_secs,
            })?
            .map_err(|e| McpError::InitError {
                name: config.name.clone(),
                message: e.to_string(),
            })?;

        // The service is already initialized by serve_client
        let service = Arc::new(service);

        // List available tools
        let tools_result = timeout(
            timeout_duration,
            service.list_tools(None::<rmcp::model::PaginatedRequestParam>),
        )
        .await
        .map_err(|_| McpError::Timeout {
            name: config.name.clone(),
            timeout_secs: config.timeout_secs,
        })?
        .map_err(|e| McpError::InitError {
            name: config.name.clone(),
            message: format!("Failed to list tools: {}", e),
        })?;

        // Convert tools to our format
        let tools: Vec<McpTool> = tools_result
            .tools
            .into_iter()
            .map(|tool| {
                McpTool::new(
                    &config.name,
                    tool.name.to_string(),
                    tool.description.as_ref().map(|d| d.to_string()),
                    serde_json::to_value(&tool.input_schema)
                        .unwrap_or_else(|_| serde_json::json!({"type": "object"})),
                )
            })
            .collect();

        info!(
            "Connected to MCP server '{}' with {} tools",
            config.name,
            tools.len()
        );
        for tool in &tools {
            debug!("  - {}", tool.qualified_name);
        }

        // Store the connected server
        let connected = ConnectedServer { service, tools };
        let mut servers = self.servers.write().await;
        servers.insert(config.name.clone(), connected);

        Ok(())
    }

    /// Connect to an HTTP-based MCP server
    async fn connect_http(
        &self,
        config: &McpServerConfig,
        url: &str,
        bearer_token: Option<&str>,
        headers: &HashMap<String, String>,
        timeout_duration: Duration,
    ) -> Result<(), McpError> {
        info!(
            "Connecting to MCP server '{}' via HTTP at {}",
            config.name, url
        );

        // Build reqwest client with custom headers
        let mut client_builder = reqwest::Client::builder();

        // Add custom headers
        let mut header_map = reqwest::header::HeaderMap::new();

        // Add bearer token if provided
        if let Some(token) = bearer_token {
            let auth_value = format!("Bearer {}", token);
            let header_value =
                reqwest::header::HeaderValue::from_str(&auth_value).map_err(|e| {
                    McpError::InitError {
                        name: config.name.clone(),
                        message: format!("Invalid bearer token: {}", e),
                    }
                })?;
            header_map.insert(reqwest::header::AUTHORIZATION, header_value);
        }
        for (key, value) in headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    McpError::InitError {
                        name: config.name.clone(),
                        message: format!("Invalid header name '{}': {}", key, e),
                    }
                })?;
            let header_value =
                reqwest::header::HeaderValue::from_str(value).map_err(|e| McpError::InitError {
                    name: config.name.clone(),
                    message: format!("Invalid header value for '{}': {}", key, e),
                })?;
            header_map.insert(header_name, header_value);
        }

        client_builder = client_builder.default_headers(header_map);
        let client = client_builder.build().map_err(|e| McpError::InitError {
            name: config.name.clone(),
            message: format!("Failed to build HTTP client: {}", e),
        })?;

        // Create the transport
        let transport = StreamableHttpClientTransport::with_client(
            client,
            rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig {
                uri: url.to_string().into(),
                ..Default::default()
            },
        );

        // Create client handler
        let handler = ClientHandler;

        // Initialize the service
        let service = timeout(timeout_duration, serve_client(handler, transport))
            .await
            .map_err(|_| McpError::Timeout {
                name: config.name.clone(),
                timeout_secs: config.timeout_secs,
            })?
            .map_err(|e| McpError::InitError {
                name: config.name.clone(),
                message: format!("HTTP transport error: {}", e),
            })?;

        let service = Arc::new(service);

        // List available tools
        let tools_result = timeout(
            timeout_duration,
            service.list_tools(None::<rmcp::model::PaginatedRequestParam>),
        )
        .await
        .map_err(|_| McpError::Timeout {
            name: config.name.clone(),
            timeout_secs: config.timeout_secs,
        })?
        .map_err(|e| McpError::InitError {
            name: config.name.clone(),
            message: format!("Failed to list tools: {}", e),
        })?;

        // Convert tools to our format
        let tools: Vec<McpTool> = tools_result
            .tools
            .into_iter()
            .map(|tool| {
                McpTool::new(
                    &config.name,
                    tool.name.to_string(),
                    tool.description.as_ref().map(|d| d.to_string()),
                    serde_json::to_value(&tool.input_schema)
                        .unwrap_or_else(|_| serde_json::json!({"type": "object"})),
                )
            })
            .collect();

        info!(
            "Connected to MCP server '{}' via HTTP with {} tools",
            config.name,
            tools.len()
        );
        for tool in &tools {
            debug!("  - {}", tool.qualified_name);
        }

        // Store the connected server
        let connected = ConnectedServer { service, tools };
        let mut servers = self.servers.write().await;
        servers.insert(config.name.clone(), connected);

        Ok(())
    }

    /// Disconnect from an MCP server
    pub async fn disconnect(&self, name: &str) -> Result<(), McpError> {
        let mut servers = self.servers.write().await;
        if servers.remove(name).is_some() {
            info!("Disconnected from MCP server '{}'", name);
            Ok(())
        } else {
            Err(McpError::UnknownServer(name.to_string()))
        }
    }

    /// Disconnect from all MCP servers
    pub async fn disconnect_all(&self) {
        let mut servers = self.servers.write().await;
        let names: Vec<_> = servers.keys().cloned().collect();
        for name in names {
            servers.remove(&name);
            info!("Disconnected from MCP server '{}'", name);
        }
    }

    /// Get all tools from all connected servers
    pub async fn list_tools(&self) -> Vec<McpTool> {
        let servers = self.servers.read().await;
        servers.values().flat_map(|s| s.tools.clone()).collect()
    }

    /// Get tools from a specific server
    pub async fn list_server_tools(&self, server_name: &str) -> Result<Vec<McpTool>, McpError> {
        let servers = self.servers.read().await;
        servers
            .get(server_name)
            .map(|s| s.tools.clone())
            .ok_or_else(|| McpError::UnknownServer(server_name.to_string()))
    }

    /// Call a tool on an MCP server
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<McpToolResult, McpError> {
        let servers = self.servers.read().await;
        let server = servers
            .get(server_name)
            .ok_or_else(|| McpError::UnknownServer(server_name.to_string()))?;

        // Verify tool exists
        let tool_exists = server.tools.iter().any(|t| t.name == tool_name);
        if !tool_exists {
            return Err(McpError::UnknownTool {
                server: server_name.to_string(),
                tool: tool_name.to_string(),
            });
        }

        debug!(
            "Calling MCP tool '{}' on server '{}'",
            tool_name, server_name
        );

        // Build the call parameters
        let params = CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments: arguments.map(|v| {
                v.as_object()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
            }),
        };

        // Call the tool
        let result =
            server
                .service
                .call_tool(params)
                .await
                .map_err(|e| McpError::ToolCallError {
                    server: server_name.to_string(),
                    tool: tool_name.to_string(),
                    message: e.to_string(),
                })?;

        // Convert result to our format
        let content: Vec<McpContent> = result
            .content
            .into_iter()
            .filter_map(|c| {
                let raw = c.raw;
                match raw {
                    rmcp::model::RawContent::Text(text) => {
                        Some(McpContent::Text { text: text.text })
                    }
                    rmcp::model::RawContent::Image(image) => Some(McpContent::Image {
                        data: image.data,
                        mime_type: image.mime_type,
                    }),
                    rmcp::model::RawContent::Resource(res) => {
                        let (uri, text) = match res.resource {
                            rmcp::model::ResourceContents::TextResourceContents {
                                uri,
                                text,
                                ..
                            } => (uri, Some(text)),
                            rmcp::model::ResourceContents::BlobResourceContents { uri, .. } => {
                                (uri, None)
                            }
                        };
                        Some(McpContent::Resource { uri, text })
                    }
                    _ => None,
                }
            })
            .collect();

        Ok(McpToolResult {
            content,
            is_error: result.is_error.unwrap_or(false),
        })
    }

    /// Check if a server is connected
    pub async fn is_connected(&self, name: &str) -> bool {
        let servers = self.servers.read().await;
        servers.contains_key(name)
    }

    /// Get list of connected server names
    pub async fn connected_servers(&self) -> Vec<String> {
        let servers = self.servers.read().await;
        servers.keys().cloned().collect()
    }

    /// Call a tool with retry logic and exponential backoff
    ///
    /// Audit #93: This method provides automatic retry with exponential backoff
    /// for transient MCP failures. It will retry up to `max_retries` times,
    /// with delays of `base_delay_ms * 2^attempt` between attempts.
    ///
    /// # Arguments
    /// * `server_name` - Name of the MCP server
    /// * `tool_name` - Name of the tool to call
    /// * `arguments` - Optional JSON arguments for the tool
    /// * `max_retries` - Maximum number of retry attempts (default: 3)
    /// * `base_delay_ms` - Base delay in milliseconds for backoff (default: 100)
    ///
    /// # Returns
    /// The tool result on success, or the last error after all retries are exhausted.
    pub async fn call_tool_with_retry(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Option<serde_json::Value>,
        max_retries: Option<u32>,
        base_delay_ms: Option<u64>,
    ) -> Result<McpToolResult, McpError> {
        let max_retries = max_retries.unwrap_or(3);
        let base_delay_ms = base_delay_ms.unwrap_or(100);
        let mut last_error: Option<McpError> = None;

        for attempt in 0..=max_retries {
            // Try to call the tool
            match self
                .call_tool(server_name, tool_name, arguments.clone())
                .await
            {
                Ok(result) => {
                    if attempt > 0 {
                        info!(
                            "MCP tool call succeeded on attempt {} of {}",
                            attempt + 1,
                            max_retries + 1
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    // Check if the error is retryable
                    let is_retryable = matches!(
                        &e,
                        McpError::ToolCallError { .. }
                            | McpError::Timeout { .. }
                            | McpError::ConnectionError { .. }
                    );

                    if !is_retryable || attempt >= max_retries {
                        // Non-retryable error or exhausted retries
                        if attempt > 0 {
                            debug!("MCP tool call failed after {} attempts: {}", attempt + 1, e);
                        }
                        return Err(e);
                    }

                    // Log the retry attempt
                    let delay_ms = base_delay_ms * (1 << attempt); // Exponential backoff
                    debug!(
                        "MCP tool call attempt {} failed: {}. Retrying in {}ms...",
                        attempt + 1,
                        e,
                        delay_ms
                    );

                    last_error = Some(e);

                    // Wait before retrying
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }

        // Should not reach here, but return the last error if we do
        Err(last_error
            .unwrap_or_else(|| McpError::Other("Retry loop completed without result".to_string())))
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = McpClient::new();
        assert!(client.connected_servers().await.is_empty());
    }

    #[tokio::test]
    async fn test_list_tools_empty() {
        let client = McpClient::new();
        let tools = client.list_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_client_default() {
        let client = McpClient::default();
        assert!(client.connected_servers().await.is_empty());
    }

    #[tokio::test]
    async fn test_is_connected_false() {
        let client = McpClient::new();
        assert!(!client.is_connected("nonexistent").await);
    }

    #[tokio::test]
    async fn test_disconnect_unknown_server() {
        let client = McpClient::new();
        let result = client.disconnect("nonexistent").await;
        assert!(result.is_err());
        match result.err().unwrap() {
            McpError::UnknownServer(name) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected UnknownServer error"),
        }
    }

    #[tokio::test]
    async fn test_list_server_tools_unknown() {
        let client = McpClient::new();
        let result = client.list_server_tools("nonexistent").await;
        assert!(result.is_err());
        match result.err().unwrap() {
            McpError::UnknownServer(name) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected UnknownServer error"),
        }
    }

    #[tokio::test]
    async fn test_call_tool_unknown_server() {
        let client = McpClient::new();
        let result = client.call_tool("nonexistent", "some_tool", None).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            McpError::UnknownServer(name) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected UnknownServer error"),
        }
    }

    #[tokio::test]
    async fn test_disconnect_all_empty() {
        let client = McpClient::new();
        // Should not panic on empty client
        client.disconnect_all().await;
        assert!(client.connected_servers().await.is_empty());
    }

    #[tokio::test]
    async fn test_connect_http_unreachable() {
        // Test that HTTP transport is implemented and returns appropriate errors
        let client = McpClient::new();
        // Use a definitely unreachable URL with short timeout
        let mut config = McpServerConfig::new_http("api", "http://127.0.0.1:1/mcp");
        config.timeout_secs = 1; // Short timeout for test

        let result = client.connect(&config).await;
        // HTTP transport is now implemented, so we expect a timeout or init error
        // (not "not implemented" error)
        assert!(result.is_err());
        let err = result.err().unwrap();
        match &err {
            McpError::Timeout { .. } | McpError::InitError { .. } => {
                // Expected - server unreachable causes timeout or transport error
            }
            _ => panic!("Expected Timeout or InitError, got: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_connect_http_invalid_header() {
        // Test that invalid headers are caught
        let client = McpClient::new();
        let mut config = McpServerConfig::new_http("api", "http://localhost:8080/mcp");
        // Add header with invalid characters (newlines are not allowed)
        config.transport = McpTransport::Http {
            url: "http://localhost:8080/mcp".to_string(),
            bearer_token: None,
            headers: {
                let mut h = HashMap::new();
                h.insert(
                    "X-Bad-Header".to_string(),
                    "value\nwith\nnewlines".to_string(),
                );
                h
            },
        };

        let result = client.connect(&config).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            McpError::InitError { message, .. } => {
                assert!(message.contains("Invalid header"));
            }
            err => panic!("Expected InitError for invalid header, got: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_connect_http_with_bearer_token() {
        // Test that bearer token is processed (will fail to connect but should not error on token)
        let client = McpClient::new();
        let config = McpServerConfig::new_http("api", "http://127.0.0.1:1/mcp")
            .with_bearer_token("valid-token-123")
            .with_timeout(1);

        let result = client.connect(&config).await;
        // Should fail with timeout/init error, not invalid token error
        assert!(result.is_err());
        let err = result.err().unwrap();
        match &err {
            McpError::Timeout { .. } | McpError::InitError { .. } => {
                // Expected - token is valid, server just unreachable
            }
            _ => panic!("Expected Timeout or InitError, got: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_connect_http_with_invalid_bearer_token() {
        // Test that invalid bearer token characters are caught
        let client = McpClient::new();
        let config = McpServerConfig::new_http("api", "http://localhost:8080/mcp")
            .with_bearer_token("invalid\ntoken\nwith\nnewlines")
            .with_timeout(1);

        let result = client.connect(&config).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            McpError::InitError { message, .. } => {
                assert!(
                    message.contains("Invalid bearer token"),
                    "Expected 'Invalid bearer token' in message, got: {}",
                    message
                );
            }
            err => panic!(
                "Expected InitError for invalid bearer token, got: {:?}",
                err
            ),
        }
    }

    #[tokio::test]
    async fn test_connect_http_bearer_token_with_custom_headers() {
        // Test that bearer token works alongside custom headers
        let client = McpClient::new();
        let mut headers = HashMap::new();
        headers.insert("X-Custom".to_string(), "value".to_string());

        let config = McpServerConfig::new_http("api", "http://127.0.0.1:1/mcp")
            .with_bearer_token("my-token")
            .with_headers(headers)
            .with_timeout(1);

        let result = client.connect(&config).await;
        // Should fail with timeout/init error (token and headers are valid)
        assert!(result.is_err());
        let err = result.err().unwrap();
        match &err {
            McpError::Timeout { .. } | McpError::InitError { .. } => {
                // Expected - headers and token are valid, server just unreachable
            }
            _ => panic!("Expected Timeout or InitError, got: {:?}", err),
        }
    }

    // Tests for retry with backoff (Audit #93)

    #[tokio::test]
    async fn test_call_tool_with_retry_unknown_server() {
        // Test that unknown server errors are not retried (immediate failure)
        let client = McpClient::new();
        let result = client
            .call_tool_with_retry("nonexistent", "tool", None, Some(3), Some(10))
            .await;

        assert!(result.is_err());
        match result.err().unwrap() {
            McpError::UnknownServer(name) => assert_eq!(name, "nonexistent"),
            err => panic!("Expected UnknownServer, got: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_call_tool_with_retry_defaults() {
        // Test that default parameters are applied
        let client = McpClient::new();
        // Should use default retries (3) and delay (100ms)
        let result = client
            .call_tool_with_retry("nonexistent", "tool", None, None, None)
            .await;

        // Should fail immediately since UnknownServer is not retryable
        assert!(result.is_err());
        match result.err().unwrap() {
            McpError::UnknownServer(name) => assert_eq!(name, "nonexistent"),
            err => panic!("Expected UnknownServer, got: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_call_tool_with_retry_zero_retries() {
        // Test with zero retries (single attempt)
        let client = McpClient::new();
        let result = client
            .call_tool_with_retry("nonexistent", "tool", None, Some(0), Some(10))
            .await;

        assert!(result.is_err());
        match result.err().unwrap() {
            McpError::UnknownServer(name) => assert_eq!(name, "nonexistent"),
            err => panic!("Expected UnknownServer, got: {:?}", err),
        }
    }
}
