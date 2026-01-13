//! MCP (Model Context Protocol) stdio server for Codex DashFlow
//!
//! Exposes Codex DashFlow tools via the MCP protocol over stdin/stdout.
//! This allows LLM clients (Claude, OpenAI, etc.) to connect and use the tools.
//!
//! ## Usage
//!
//! ```bash
//! # Start the MCP stdio server
//! codex-dashflow mcp-server
//!
//! # With custom working directory
//! codex-dashflow mcp-server --working-dir /path/to/project
//! ```
//!
//! ## Protocol
//!
//! Uses JSON-RPC 2.0 over stdin/stdout with MCP extensions:
//! - `initialize` - Handshake and capability negotiation
//! - `tools/list` - List available tools
//! - `tools/call` - Execute a tool
//! - `notifications/initialized` - Client ready notification
//!
//! ## Tools Exposed
//!
//! | Tool | Description |
//! |------|-------------|
//! | `read_file` | Read file contents |
//! | `write_file` | Write content to file |
//! | `edit_file` | Edit file by replacing text |
//! | `list_files` | List directory contents |
//! | `shell_exec` | Execute shell commands |

use crate::agent::tools::{EditFileTool, ListFilesTool, ReadFileTool, ShellExecTool, WriteFileTool};
use dashflow::core::tools::{Tool, ToolInput};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// MCP protocol version
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// Server information
const SERVER_NAME: &str = "codex-dashflow";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON-RPC 2.0 request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// MCP tool definition
#[derive(Debug, Serialize)]
struct McpToolDef {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

/// MCP stdio server
pub struct McpStdioServer {
    tools: HashMap<String, Arc<dyn Tool>>,
    working_dir: PathBuf,
    initialized: bool,
}

impl McpStdioServer {
    /// Create a new MCP server with the given working directory
    pub fn new(working_dir: PathBuf) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        // Register all codex-dashflow tools
        let read_file = Arc::new(ReadFileTool::new(working_dir.clone()));
        let write_file = Arc::new(WriteFileTool::new(working_dir.clone()));
        let edit_file = Arc::new(EditFileTool::new(working_dir.clone()));
        let list_files = Arc::new(ListFilesTool::new(working_dir.clone()));
        let shell_exec = Arc::new(ShellExecTool::new(working_dir.clone()));

        tools.insert(read_file.name().to_string(), read_file);
        tools.insert(write_file.name().to_string(), write_file);
        tools.insert(edit_file.name().to_string(), edit_file);
        tools.insert(list_files.name().to_string(), list_files);
        tools.insert(shell_exec.name().to_string(), shell_exec);

        Self {
            tools,
            working_dir,
            initialized: false,
        }
    }

    /// Run the server, reading from stdin and writing to stdout
    pub async fn run(&mut self) -> io::Result<()> {
        info!(
            working_dir = %self.working_dir.display(),
            "Starting MCP stdio server"
        );

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    error!(?e, "Error reading from stdin");
                    break;
                }
            };

            if line.is_empty() {
                continue;
            }

            debug!(request = %line, "Received request");

            let response = self.handle_message(&line).await;

            if let Some(resp) = response {
                let json = match serde_json::to_string(&resp) {
                    Ok(j) => j,
                    Err(e) => {
                        error!(?e, "Error serializing response");
                        continue;
                    }
                };

                debug!(response = %json, "Sending response");

                writeln!(stdout, "{}", json)?;
                stdout.flush()?;
            }
        }

        info!("MCP server shutting down");
        Ok(())
    }

    /// Handle a single JSON-RPC message
    async fn handle_message(&mut self, message: &str) -> Option<JsonRpcResponse> {
        let request: JsonRpcRequest = match serde_json::from_str(message) {
            Ok(r) => r,
            Err(e) => {
                warn!(?e, "Invalid JSON-RPC request");
                return Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: "Parse error".to_string(),
                        data: Some(json!({ "details": e.to_string() })),
                    }),
                });
            }
        };

        if request.jsonrpc != "2.0" {
            return Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32600,
                    message: "Invalid Request: jsonrpc must be '2.0'".to_string(),
                    data: None,
                }),
            });
        }

        // Handle notifications (no id = no response expected)
        if request.id.is_none() {
            self.handle_notification(&request.method, &request.params);
            return None;
        }

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(&request.params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(&request.params).await,
            "ping" => Ok(json!({ "pong": true })),
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        };

        Some(match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(value),
                error: None,
            },
            Err(error) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(error),
            },
        })
    }

    /// Handle notifications (no response expected)
    fn handle_notification(&mut self, method: &str, _params: &Value) {
        match method {
            "notifications/initialized" => {
                info!("Client initialized");
                self.initialized = true;
            }
            "notifications/cancelled" => {
                debug!("Request cancelled by client");
            }
            _ => {
                debug!(method, "Unknown notification");
            }
        }
    }

    /// Handle the initialize request
    fn handle_initialize(&mut self, params: &Value) -> Result<Value, JsonRpcError> {
        let client_info = params.get("clientInfo");
        if let Some(info) = client_info {
            info!(
                client_name = ?info.get("name"),
                client_version = ?info.get("version"),
                "Client connected"
            );
        }

        let protocol_version = params
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        info!(
            protocol_version,
            server_version = SERVER_VERSION,
            "Initializing MCP session"
        );

        Ok(json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            },
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            }
        }))
    }

    /// Handle tools/list request
    fn handle_tools_list(&self) -> Result<Value, JsonRpcError> {
        let tools: Vec<McpToolDef> = self
            .tools
            .values()
            .map(|tool| McpToolDef {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.args_schema(),
            })
            .collect();

        debug!(tool_count = tools.len(), "Listing tools");

        Ok(json!({ "tools": tools }))
    }

    /// Handle tools/call request
    async fn handle_tools_call(&self, params: &Value) -> Result<Value, JsonRpcError> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing 'name' parameter".to_string(),
                data: None,
            })?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        info!(tool = name, "Calling tool");

        let tool = self.tools.get(name).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Unknown tool: {}", name),
            data: Some(json!({
                "available_tools": self.tools.keys().collect::<Vec<_>>()
            })),
        })?;

        let input = ToolInput::Structured(arguments);

        match tool._call(input).await {
            Ok(result) => {
                debug!(tool = name, result_len = result.len(), "Tool call succeeded");
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": result
                    }],
                    "isError": false
                }))
            }
            Err(e) => {
                warn!(tool = name, error = ?e, "Tool call failed");
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: {}", e)
                    }],
                    "isError": true
                }))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_server() -> (McpStdioServer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let server = McpStdioServer::new(temp_dir.path().to_path_buf());
        (server, temp_dir)
    }

    #[test]
    fn test_server_creation() {
        let (server, _temp) = create_test_server();
        assert_eq!(server.tools.len(), 5);
        assert!(server.tools.contains_key("read_file"));
        assert!(server.tools.contains_key("write_file"));
        assert!(server.tools.contains_key("edit_file"));
        assert!(server.tools.contains_key("list_files"));
        assert!(server.tools.contains_key("shell_exec"));
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let (mut server, _temp) = create_test_server();

        let params = json!({
            "protocolVersion": "2024-11-05",
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        });

        let result = server.handle_initialize(&params);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(
            response.get("protocolVersion").and_then(|v| v.as_str()),
            Some(MCP_PROTOCOL_VERSION)
        );
        assert!(response.get("serverInfo").is_some());
        assert!(response.get("capabilities").is_some());
    }

    #[test]
    fn test_handle_tools_list() {
        let (server, _temp) = create_test_server();

        let result = server.handle_tools_list();
        assert!(result.is_ok());

        let response = result.unwrap();
        let tools = response.get("tools").and_then(|v| v.as_array());
        assert!(tools.is_some());
        assert_eq!(tools.unwrap().len(), 5);
    }

    #[tokio::test]
    async fn test_handle_tools_call_read_file() {
        let (server, temp) = create_test_server();

        // Create a test file
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let params = json!({
            "name": "read_file",
            "arguments": {
                "path": "test.txt"
            }
        });

        let result = server.handle_tools_call(&params).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.get("isError"), Some(&json!(false)));

        let content = response
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str());

        assert!(content.is_some());
        assert!(content.unwrap().contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_handle_tools_call_unknown_tool() {
        let (server, _temp) = create_test_server();

        let params = json!({
            "name": "unknown_tool",
            "arguments": {}
        });

        let result = server.handle_tools_call(&params).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.code, -32602);
        assert!(error.message.contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_handle_tools_call_shell_exec() {
        let (server, _temp) = create_test_server();

        let params = json!({
            "name": "shell_exec",
            "arguments": {
                "command": "echo test_output"
            }
        });

        let result = server.handle_tools_call(&params).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let content = response
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str());

        assert!(content.is_some());
        assert!(content.unwrap().contains("test_output"));
    }

    #[tokio::test]
    async fn test_handle_tools_call_list_files() {
        let (server, temp) = create_test_server();

        // Create some test files
        std::fs::write(temp.path().join("file1.txt"), "").unwrap();
        std::fs::write(temp.path().join("file2.txt"), "").unwrap();

        let params = json!({
            "name": "list_files",
            "arguments": {}
        });

        let result = server.handle_tools_call(&params).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let content = response
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str());

        assert!(content.is_some());
        let text = content.unwrap();
        assert!(text.contains("file1.txt"));
        assert!(text.contains("file2.txt"));
    }

    #[tokio::test]
    async fn test_handle_message_parse_error() {
        let (mut server, _temp) = create_test_server();

        let response = server.handle_message("not valid json").await;
        assert!(response.is_some());

        let resp = response.unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, -32700);
    }

    #[tokio::test]
    async fn test_handle_message_invalid_jsonrpc() {
        let (mut server, _temp) = create_test_server();

        let response = server
            .handle_message(r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#)
            .await;
        assert!(response.is_some());

        let resp = response.unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, -32600);
    }

    #[tokio::test]
    async fn test_handle_message_method_not_found() {
        let (mut server, _temp) = create_test_server();

        let response = server
            .handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"unknown_method"}"#)
            .await;
        assert!(response.is_some());

        let resp = response.unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_handle_message_ping() {
        let (mut server, _temp) = create_test_server();

        let response = server
            .handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
            .await;
        assert!(response.is_some());

        let resp = response.unwrap();
        assert!(resp.result.is_some());
        assert_eq!(resp.result.as_ref().unwrap().get("pong"), Some(&json!(true)));
    }

    #[tokio::test]
    async fn test_handle_notification_no_response() {
        let (mut server, _temp) = create_test_server();

        // Notifications (no id) should not produce a response
        let response = server
            .handle_message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .await;
        assert!(response.is_none());
        assert!(server.initialized);
    }
}
