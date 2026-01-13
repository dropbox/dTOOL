//! MCP server handler implementing the codex tool

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Content, JsonObject, ListToolsResult,
    PaginatedRequestParam, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};

use codex_dashflow_core::runner::{run_agent, RunnerConfig};
use codex_dashflow_core::sandbox::SandboxMode;
use codex_dashflow_core::state::{AgentState, Message};
use codex_dashflow_core::{ApprovalMode, ExecPolicy};

/// Arguments for the codex tool
#[derive(Debug, Clone, Deserialize)]
pub struct CodexToolArgs {
    /// The prompt/instruction for the coding agent
    pub prompt: String,

    /// Working directory for file operations (optional)
    #[serde(default)]
    pub working_dir: Option<String>,

    /// Maximum number of turns (0 = unlimited, optional)
    #[serde(default)]
    pub max_turns: Option<u32>,

    /// Model to use (optional, defaults to config or gpt-4)
    /// Reserved for future model selection support
    #[serde(default)]
    #[allow(dead_code)]
    pub model: Option<String>,

    /// Sandbox mode (optional, defaults to workspace-write)
    #[serde(default)]
    pub sandbox_mode: Option<String>,
}

/// Result content from the codex tool
#[derive(Debug, Clone, Serialize)]
pub struct CodexToolResult {
    /// The agent's final response
    pub response: String,
    /// Number of turns executed
    pub turns: u32,
    /// Completion status
    pub status: String,
    /// Tool calls made during execution
    pub tool_calls: Vec<String>,
}

/// MCP server handler that exposes the Codex agent as a tool
#[derive(Clone)]
pub struct CodexToolServer {
    /// Default working directory
    pub working_dir: PathBuf,
    /// Default sandbox mode
    pub sandbox_mode: SandboxMode,
    /// Whether to use mock LLM for testing
    pub mock_llm: bool,
}

impl CodexToolServer {
    /// Create a new CodexToolServer with default settings
    pub fn new() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_default(),
            sandbox_mode: SandboxMode::WorkspaceWrite,
            mock_llm: false,
        }
    }

    /// Set the default working directory
    pub fn with_working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = path.into();
        self
    }

    /// Set the default sandbox mode
    pub fn with_sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.sandbox_mode = mode;
        self
    }

    /// Enable mock LLM for testing
    pub fn with_mock_llm(mut self, mock: bool) -> Self {
        self.mock_llm = mock;
        self
    }

    /// Create the codex tool definition
    fn codex_tool() -> Tool {
        let schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The coding task or question to send to the Codex agent"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for file operations (optional)"
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Maximum number of agent turns (0 = unlimited, optional)"
                },
                "model": {
                    "type": "string",
                    "description": "LLM model to use (optional)"
                },
                "sandbox_mode": {
                    "type": "string",
                    "enum": ["read-only", "workspace-write", "danger-full-access"],
                    "description": "Sandbox mode for command execution (optional)"
                }
            },
            "required": ["prompt"],
            "additionalProperties": false
        }))
        .expect("codex tool schema should deserialize");

        Tool::new(
            Cow::Borrowed("codex"),
            Cow::Borrowed(
                "Run the Codex DashFlow coding agent. The agent can read/write files, \
                execute shell commands, and help with coding tasks. Returns the agent's \
                response and execution summary.",
            ),
            Arc::new(schema),
        )
    }

    /// Execute the codex tool with given arguments
    async fn execute_codex(&self, args: CodexToolArgs) -> Result<CodexToolResult, String> {
        let working_dir = args
            .working_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.working_dir.clone());

        // Parse sandbox mode
        let sandbox_mode = if let Some(mode_str) = &args.sandbox_mode {
            match mode_str.as_str() {
                "read-only" => SandboxMode::ReadOnly,
                "workspace-write" => SandboxMode::WorkspaceWrite,
                "danger-full-access" => SandboxMode::DangerFullAccess,
                _ => self.sandbox_mode,
            }
        } else {
            self.sandbox_mode
        };

        // Build agent state with auto-approve policy for MCP server
        let policy = Arc::new(ExecPolicy::new().with_approval_mode(ApprovalMode::Never));
        let mut state = AgentState::new()
            .with_working_directory(working_dir.to_string_lossy())
            .with_sandbox_mode(sandbox_mode)
            .with_exec_policy(policy);

        if self.mock_llm {
            state = state.with_mock_llm();
        }

        // Add the user prompt
        state.messages.push(Message::user(&args.prompt));

        // Set max turns if specified
        if let Some(max_turns) = args.max_turns {
            state.max_turns = max_turns;
        }

        // Configure runner
        let config = RunnerConfig::default();

        // Run the agent
        let result = run_agent(state, &config).await.map_err(|e| e.to_string())?;

        // Extract tool calls made
        let tool_calls: Vec<String> = result
            .state
            .tool_results
            .iter()
            .map(|r| r.tool.clone())
            .collect();

        // Get status string
        let status = match &result.state.status {
            codex_dashflow_core::state::CompletionStatus::Complete => "complete".to_string(),
            codex_dashflow_core::state::CompletionStatus::TurnLimitReached => {
                "turn_limit_reached".to_string()
            }
            codex_dashflow_core::state::CompletionStatus::Interrupted => "interrupted".to_string(),
            codex_dashflow_core::state::CompletionStatus::Error(e) => format!("error: {}", e),
            codex_dashflow_core::state::CompletionStatus::InProgress => "in_progress".to_string(),
        };

        // Get response
        let response = result
            .state
            .last_response
            .unwrap_or_else(|| "No response generated".to_string());

        Ok(CodexToolResult {
            response,
            turns: result.turns,
            status,
            tool_calls,
        })
    }
}

impl Default for CodexToolServer {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::manual_async_fn)]
impl ServerHandler for CodexToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                tools: vec![Self::codex_tool()],
                next_cursor: None,
            })
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        info!("MCP tool call: {}", request.name);

        match request.name.as_ref() {
            "codex" => {
                // Parse arguments
                let args: CodexToolArgs = match request.arguments {
                    Some(arguments) => serde_json::from_value(serde_json::Value::Object(
                        arguments.into_iter().collect(),
                    ))
                    .map_err(|err| McpError::invalid_params(err.to_string(), None))?,
                    None => {
                        return Err(McpError::invalid_params(
                            "missing arguments for codex tool; 'prompt' is required",
                            None,
                        ));
                    }
                };

                info!("Executing codex tool with prompt: {}", args.prompt);

                // Execute the codex agent
                match self.execute_codex(args).await {
                    Ok(result) => {
                        // Return both text content and structured content
                        let structured = json!({
                            "response": result.response,
                            "turns": result.turns,
                            "status": result.status,
                            "tool_calls": result.tool_calls,
                        });

                        Ok(CallToolResult {
                            content: vec![Content::text(result.response)],
                            structured_content: Some(structured),
                            is_error: Some(false),
                            meta: None,
                        })
                    }
                    Err(e) => {
                        error!("Codex tool execution failed: {}", e);
                        Ok(CallToolResult {
                            content: vec![Content::text(format!("Error: {}", e))],
                            structured_content: None,
                            is_error: Some(true),
                            meta: None,
                        })
                    }
                }
            }
            other => Err(McpError::invalid_params(
                format!("unknown tool: {}", other),
                None,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_tool_server_creation() {
        let server = CodexToolServer::new();
        assert!(!server.mock_llm);
        assert_eq!(server.sandbox_mode, SandboxMode::WorkspaceWrite);
    }

    #[test]
    fn test_codex_tool_server_with_options() {
        let server = CodexToolServer::new()
            .with_mock_llm(true)
            .with_sandbox_mode(SandboxMode::ReadOnly)
            .with_working_dir("/tmp");

        assert!(server.mock_llm);
        assert_eq!(server.sandbox_mode, SandboxMode::ReadOnly);
        assert_eq!(server.working_dir, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_codex_tool_definition() {
        let tool = CodexToolServer::codex_tool();
        assert_eq!(tool.name.as_ref(), "codex");
        assert!(tool.description.is_some());
    }

    #[test]
    fn test_server_info() {
        let server = CodexToolServer::new();
        let info = server.get_info();
        // Check that capabilities include tools
        assert!(info.capabilities.tools.is_some());
    }

    #[tokio::test]
    async fn test_execute_codex_mock() {
        let server = CodexToolServer::new().with_mock_llm(true);

        let args = CodexToolArgs {
            prompt: "Hello".to_string(),
            working_dir: None,
            max_turns: Some(1),
            model: None,
            sandbox_mode: None,
        };

        let result = server.execute_codex(args).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.status, "complete");
        assert!(!result.response.is_empty());
    }

    #[test]
    fn test_codex_tool_server_default() {
        let server: CodexToolServer = Default::default();
        assert!(!server.mock_llm);
        assert_eq!(server.sandbox_mode, SandboxMode::WorkspaceWrite);
    }

    #[test]
    fn test_codex_tool_server_chained_builders() {
        let server = CodexToolServer::new()
            .with_working_dir("/home/user")
            .with_sandbox_mode(SandboxMode::DangerFullAccess)
            .with_mock_llm(true);

        assert_eq!(server.working_dir, PathBuf::from("/home/user"));
        assert_eq!(server.sandbox_mode, SandboxMode::DangerFullAccess);
        assert!(server.mock_llm);
    }

    #[test]
    fn test_codex_tool_args_deserialization_minimal() {
        let json = r#"{"prompt": "test prompt"}"#;
        let args: CodexToolArgs = serde_json::from_str(json).unwrap();
        assert_eq!(args.prompt, "test prompt");
        assert!(args.working_dir.is_none());
        assert!(args.max_turns.is_none());
        assert!(args.model.is_none());
        assert!(args.sandbox_mode.is_none());
    }

    #[test]
    fn test_codex_tool_args_deserialization_full() {
        let json = r#"{
            "prompt": "code task",
            "working_dir": "/tmp/work",
            "max_turns": 10,
            "model": "gpt-4",
            "sandbox_mode": "read-only"
        }"#;
        let args: CodexToolArgs = serde_json::from_str(json).unwrap();
        assert_eq!(args.prompt, "code task");
        assert_eq!(args.working_dir, Some("/tmp/work".to_string()));
        assert_eq!(args.max_turns, Some(10));
        assert_eq!(args.model, Some("gpt-4".to_string()));
        assert_eq!(args.sandbox_mode, Some("read-only".to_string()));
    }

    #[test]
    fn test_codex_tool_result_serialization() {
        let result = CodexToolResult {
            response: "Done!".to_string(),
            turns: 3,
            status: "complete".to_string(),
            tool_calls: vec!["read_file".to_string(), "write_file".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Done!"));
        assert!(json.contains("\"turns\":3"));
        assert!(json.contains("complete"));
        assert!(json.contains("read_file"));
    }

    #[test]
    fn test_codex_tool_result_empty_tool_calls() {
        let result = CodexToolResult {
            response: "Simple response".to_string(),
            turns: 1,
            status: "complete".to_string(),
            tool_calls: vec![],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"tool_calls\":[]"));
    }

    #[test]
    fn test_codex_tool_description_contains_key_info() {
        let tool = CodexToolServer::codex_tool();
        let desc = tool.description.as_ref().unwrap();
        assert!(desc.contains("Codex"));
        assert!(desc.contains("coding"));
    }

    #[test]
    fn test_codex_tool_schema_has_required_prompt() {
        let tool = CodexToolServer::codex_tool();
        let schema_json = serde_json::to_string(&*tool.input_schema).unwrap();
        assert!(schema_json.contains("\"required\":[\"prompt\"]"));
    }

    #[tokio::test]
    async fn test_execute_codex_with_working_dir() {
        let dir = std::env::temp_dir();
        let server = CodexToolServer::new()
            .with_mock_llm(true)
            .with_working_dir(&dir);

        let args = CodexToolArgs {
            prompt: "List files".to_string(),
            working_dir: Some(dir.to_string_lossy().to_string()),
            max_turns: Some(1),
            model: None,
            sandbox_mode: None,
        };

        let result = server.execute_codex(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_codex_with_sandbox_mode_read_only() {
        let server = CodexToolServer::new().with_mock_llm(true);

        let args = CodexToolArgs {
            prompt: "Read a file".to_string(),
            working_dir: None,
            max_turns: Some(1),
            model: None,
            sandbox_mode: Some("read-only".to_string()),
        };

        let result = server.execute_codex(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_codex_with_sandbox_mode_workspace_write() {
        let server = CodexToolServer::new().with_mock_llm(true);

        let args = CodexToolArgs {
            prompt: "Write a file".to_string(),
            working_dir: None,
            max_turns: Some(1),
            model: None,
            sandbox_mode: Some("workspace-write".to_string()),
        };

        let result = server.execute_codex(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_codex_with_sandbox_mode_danger() {
        let server = CodexToolServer::new().with_mock_llm(true);

        let args = CodexToolArgs {
            prompt: "Run command".to_string(),
            working_dir: None,
            max_turns: Some(1),
            model: None,
            sandbox_mode: Some("danger-full-access".to_string()),
        };

        let result = server.execute_codex(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_codex_with_invalid_sandbox_falls_back_to_default() {
        let server = CodexToolServer::new()
            .with_mock_llm(true)
            .with_sandbox_mode(SandboxMode::ReadOnly);

        let args = CodexToolArgs {
            prompt: "Test".to_string(),
            working_dir: None,
            max_turns: Some(1),
            model: None,
            sandbox_mode: Some("invalid-mode".to_string()),
        };

        // Should not fail, uses default
        let result = server.execute_codex(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_codex_result_has_valid_turns() {
        let server = CodexToolServer::new().with_mock_llm(true);

        let args = CodexToolArgs {
            prompt: "Simple task".to_string(),
            working_dir: None,
            max_turns: Some(5),
            model: None,
            sandbox_mode: None,
        };

        let result = server.execute_codex(args).await.unwrap();
        // Mock LLM completes in 1 turn
        assert!(result.turns >= 1);
    }

    #[test]
    fn test_codex_tool_args_clone() {
        let args = CodexToolArgs {
            prompt: "test".to_string(),
            working_dir: Some("/tmp".to_string()),
            max_turns: Some(5),
            model: Some("gpt-4".to_string()),
            sandbox_mode: Some("read-only".to_string()),
        };

        let cloned = args.clone();
        assert_eq!(cloned.prompt, args.prompt);
        assert_eq!(cloned.working_dir, args.working_dir);
    }

    #[test]
    fn test_codex_tool_result_clone() {
        let result = CodexToolResult {
            response: "test".to_string(),
            turns: 2,
            status: "complete".to_string(),
            tool_calls: vec!["shell".to_string()],
        };

        let cloned = result.clone();
        assert_eq!(cloned.response, result.response);
        assert_eq!(cloned.turns, result.turns);
    }

    #[test]
    fn test_codex_tool_server_clone() {
        let server = CodexToolServer::new()
            .with_mock_llm(true)
            .with_working_dir("/test");

        let cloned = server.clone();
        assert_eq!(cloned.mock_llm, server.mock_llm);
        assert_eq!(cloned.working_dir, server.working_dir);
    }
}
