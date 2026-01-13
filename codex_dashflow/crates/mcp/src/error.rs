//! MCP error types

use thiserror::Error;

/// Errors that can occur during MCP operations
#[derive(Debug, Error)]
pub enum McpError {
    /// Failed to spawn MCP server process
    #[error("Failed to spawn MCP server '{name}': {source}")]
    SpawnError {
        name: String,
        #[source]
        source: std::io::Error,
    },

    /// MCP server initialization failed
    #[error("MCP server '{name}' initialization failed: {message}")]
    InitError { name: String, message: String },

    /// MCP server connection failed
    #[error("Failed to connect to MCP server '{name}': {message}")]
    ConnectionError { name: String, message: String },

    /// Tool call failed
    #[error("Tool call '{tool}' on server '{server}' failed: {message}")]
    ToolCallError {
        server: String,
        tool: String,
        message: String,
    },

    /// Unknown MCP server
    #[error("Unknown MCP server: {0}")]
    UnknownServer(String),

    /// Unknown tool
    #[error("Unknown tool '{tool}' on server '{server}'")]
    UnknownTool { server: String, tool: String },

    /// Invalid tool name format
    #[error("Invalid MCP tool name format: {0}")]
    InvalidToolName(String),

    /// Server not initialized
    #[error("MCP server '{0}' not initialized")]
    NotInitialized(String),

    /// Timeout waiting for MCP server
    #[error("Timeout waiting for MCP server '{name}' after {timeout_secs}s")]
    Timeout { name: String, timeout_secs: u64 },

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for McpError {
    fn from(err: anyhow::Error) -> Self {
        McpError::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_spawn_error() {
        let err = McpError::SpawnError {
            name: "test-server".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "command not found"),
        };
        let msg = err.to_string();
        assert!(msg.contains("test-server"));
        assert!(msg.contains("Failed to spawn"));
    }

    #[test]
    fn test_error_display_init_error() {
        let err = McpError::InitError {
            name: "test-server".to_string(),
            message: "invalid protocol".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("test-server"));
        assert!(msg.contains("invalid protocol"));
    }

    #[test]
    fn test_error_display_tool_call_error() {
        let err = McpError::ToolCallError {
            server: "filesystem".to_string(),
            tool: "read_file".to_string(),
            message: "file not found".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("filesystem"));
        assert!(msg.contains("read_file"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn test_error_display_unknown_server() {
        let err = McpError::UnknownServer("missing-server".to_string());
        let msg = err.to_string();
        assert!(msg.contains("missing-server"));
    }

    #[test]
    fn test_error_display_unknown_tool() {
        let err = McpError::UnknownTool {
            server: "fs".to_string(),
            tool: "unknown_tool".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("fs"));
        assert!(msg.contains("unknown_tool"));
    }

    #[test]
    fn test_error_display_timeout() {
        let err = McpError::Timeout {
            name: "slow-server".to_string(),
            timeout_secs: 30,
        };
        let msg = err.to_string();
        assert!(msg.contains("slow-server"));
        assert!(msg.contains("30"));
    }

    #[test]
    fn test_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let mcp_err: McpError = anyhow_err.into();
        match mcp_err {
            McpError::Other(msg) => assert!(msg.contains("something went wrong")),
            _ => panic!("Expected Other variant"),
        }
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let mcp_err: McpError = json_err.into();
        match mcp_err {
            McpError::SerializationError(_) => {}
            _ => panic!("Expected SerializationError variant"),
        }
    }
}
