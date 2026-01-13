//! Codex DashFlow MCP
//!
//! Model Context Protocol (MCP) client support for connecting to external tool servers.
//! This enables the agent to use tools provided by MCP servers like filesystem, git, etc.

mod client;
mod config;
mod error;
mod types;

pub use client::McpClient;
pub use config::{McpServerConfig, McpTransport};
pub use error::McpError;
pub use types::{
    is_mcp_tool, parse_qualified_tool_name, McpContent, McpTool, McpToolResult, MCP_TOOL_DELIMITER,
    MCP_TOOL_PREFIX,
};
