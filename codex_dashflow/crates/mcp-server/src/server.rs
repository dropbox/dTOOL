//! MCP server runner with stdio transport

use std::path::PathBuf;

use rmcp::ServiceExt;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::tool_handler::CodexToolServer;
use codex_dashflow_core::sandbox::SandboxMode;

/// Configuration for the MCP server
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Working directory for the codex tool
    pub working_dir: PathBuf,
    /// Default sandbox mode
    pub sandbox_mode: SandboxMode,
    /// Whether to use mock LLM (for testing)
    pub mock_llm: bool,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_default(),
            sandbox_mode: SandboxMode::WorkspaceWrite,
            mock_llm: false,
        }
    }
}

impl McpServerConfig {
    /// Create a new config with a working directory
    pub fn with_working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = path.into();
        self
    }

    /// Set the sandbox mode
    pub fn with_sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.sandbox_mode = mode;
        self
    }

    /// Enable mock LLM for testing
    pub fn with_mock_llm(mut self, mock: bool) -> Self {
        self.mock_llm = mock;
        self
    }
}

/// Get stdin/stdout for stdio transport
fn stdio() -> (tokio::io::Stdin, tokio::io::Stdout) {
    (tokio::io::stdin(), tokio::io::stdout())
}

/// Run the MCP server with stdio transport
///
/// This function blocks until the client disconnects.
pub async fn run_mcp_server(config: McpServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (logs to stderr so they don't interfere with MCP protocol on stdout)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .ok(); // Ignore error if already initialized

    info!(
        "Starting Codex DashFlow MCP server (working_dir={}, sandbox_mode={:?})",
        config.working_dir.display(),
        config.sandbox_mode
    );

    // Create the server handler
    let server = CodexToolServer::new()
        .with_working_dir(&config.working_dir)
        .with_sandbox_mode(config.sandbox_mode)
        .with_mock_llm(config.mock_llm);

    // Serve with stdio transport
    let running = server.serve(stdio()).await?;

    info!("MCP server running, waiting for client requests...");

    // Wait for the client to disconnect
    running.waiting().await?;

    info!("MCP server shutting down");

    // Allow background tasks to drain
    tokio::task::yield_now().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = McpServerConfig::default();
        assert_eq!(config.sandbox_mode, SandboxMode::WorkspaceWrite);
        assert!(!config.mock_llm);
    }

    #[test]
    fn test_config_builder() {
        let config = McpServerConfig::default()
            .with_working_dir("/tmp")
            .with_sandbox_mode(SandboxMode::ReadOnly)
            .with_mock_llm(true);

        assert_eq!(config.working_dir, PathBuf::from("/tmp"));
        assert_eq!(config.sandbox_mode, SandboxMode::ReadOnly);
        assert!(config.mock_llm);
    }
}
