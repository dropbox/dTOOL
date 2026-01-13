//! Codex DashFlow MCP Server
//!
//! Exposes the Codex DashFlow coding agent as an MCP server.
//! Other MCP clients can invoke the agent via the `codex` tool.
//!
//! ## Usage
//!
//! Run as an MCP server (stdio transport):
//! ```bash
//! codex-dashflow mcp-server
//! ```
//!
//! The server exposes:
//! - `codex` tool: Run the coding agent with a prompt

mod server;
mod tool_handler;

pub use server::{run_mcp_server, McpServerConfig};
pub use tool_handler::CodexToolServer;
