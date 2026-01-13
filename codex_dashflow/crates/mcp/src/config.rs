//! MCP server configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for an MCP server
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name (used for tool namespacing as `mcp__<name>__<tool>`)
    pub name: String,

    /// Transport type
    #[serde(flatten)]
    pub transport: McpTransport,

    /// Environment variables to set for the server process
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory for the server process (stdio transport only)
    #[serde(default)]
    pub cwd: Option<PathBuf>,

    /// Timeout in seconds for server operations (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    30
}

/// MCP transport configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransport {
    /// Stdio transport - spawn a child process
    Stdio {
        /// Command to run
        command: String,
        /// Command arguments
        #[serde(default)]
        args: Vec<String>,
    },
    /// HTTP transport - connect to HTTP server
    Http {
        /// Server URL
        url: String,
        /// Optional bearer token for authentication
        #[serde(default)]
        bearer_token: Option<String>,
        /// Optional custom HTTP headers
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

impl McpServerConfig {
    /// Create a new stdio-based MCP server config
    pub fn new_stdio(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::Stdio {
                command: command.into(),
                args: Vec::new(),
            },
            env: HashMap::new(),
            cwd: None,
            timeout_secs: default_timeout(),
        }
    }

    /// Create a new HTTP-based MCP server config
    pub fn new_http(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::Http {
                url: url.into(),
                bearer_token: None,
                headers: HashMap::new(),
            },
            env: HashMap::new(),
            cwd: None,
            timeout_secs: default_timeout(),
        }
    }

    /// Add arguments (for stdio transport)
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        if let McpTransport::Stdio {
            args: ref mut existing_args,
            ..
        } = self.transport
        {
            *existing_args = args;
        }
        self
    }

    /// Add environment variables
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Set working directory
    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Set bearer token for HTTP authentication
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        if let McpTransport::Http {
            ref mut bearer_token,
            ..
        } = self.transport
        {
            *bearer_token = Some(token.into());
        }
        self
    }

    /// Add custom HTTP headers
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        if let McpTransport::Http {
            headers: ref mut existing_headers,
            ..
        } = self.transport
        {
            *existing_headers = headers;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_config() {
        let config = McpServerConfig::new_stdio("filesystem", "mcp-server-filesystem")
            .with_args(vec!["/home/user".to_string()]);

        assert_eq!(config.name, "filesystem");
        match config.transport {
            McpTransport::Stdio { command, args } => {
                assert_eq!(command, "mcp-server-filesystem");
                assert_eq!(args, vec!["/home/user"]);
            }
            _ => panic!("Expected stdio transport"),
        }
    }

    #[test]
    fn test_http_config() {
        let config = McpServerConfig::new_http("api", "https://api.example.com/mcp");

        assert_eq!(config.name, "api");
        match config.transport {
            McpTransport::Http { url, .. } => {
                assert_eq!(url, "https://api.example.com/mcp");
            }
            _ => panic!("Expected HTTP transport"),
        }
    }

    #[test]
    fn test_config_deserialization() {
        let toml = r#"
name = "filesystem"
type = "stdio"
command = "mcp-server-filesystem"
args = ["/home/user"]
timeout_secs = 60

[env]
HOME = "/home/user"
"#;
        let config: McpServerConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.name, "filesystem");
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.env.get("HOME"), Some(&"/home/user".to_string()));
    }

    #[test]
    fn test_default_timeout() {
        let config = McpServerConfig::new_stdio("test", "test-cmd");
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_with_timeout() {
        let config = McpServerConfig::new_stdio("test", "test-cmd").with_timeout(120);
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    fn test_with_cwd() {
        let config =
            McpServerConfig::new_stdio("test", "test-cmd").with_cwd(PathBuf::from("/tmp/test"));
        assert_eq!(config.cwd, Some(PathBuf::from("/tmp/test")));
    }

    #[test]
    fn test_with_env() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret".to_string());
        let config = McpServerConfig::new_stdio("test", "test-cmd").with_env(env);
        assert_eq!(config.env.get("API_KEY"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = McpServerConfig::new_stdio("filesystem", "mcp-server-filesystem")
            .with_args(vec!["/home/user".to_string()])
            .with_timeout(45);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpServerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.name, deserialized.name);
        assert_eq!(config.timeout_secs, deserialized.timeout_secs);
    }

    #[test]
    fn test_http_config_deserialization() {
        let toml = r#"
name = "api"
type = "http"
url = "https://api.example.com/mcp"
bearer_token = "secret-token"
"#;
        let config: McpServerConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.name, "api");
        match config.transport {
            McpTransport::Http {
                url, bearer_token, ..
            } => {
                assert_eq!(url, "https://api.example.com/mcp");
                assert_eq!(bearer_token, Some("secret-token".to_string()));
            }
            _ => panic!("Expected HTTP transport"),
        }
    }

    #[test]
    fn test_with_args_on_http_transport() {
        // with_args should be a no-op for HTTP transport
        let config = McpServerConfig::new_http("api", "https://example.com")
            .with_args(vec!["ignored".to_string()]);

        match config.transport {
            McpTransport::Http { .. } => {}
            _ => panic!("Expected HTTP transport"),
        }
    }

    #[test]
    fn test_with_bearer_token() {
        let config =
            McpServerConfig::new_http("api", "https://example.com").with_bearer_token("my-token");

        match config.transport {
            McpTransport::Http { bearer_token, .. } => {
                assert_eq!(bearer_token, Some("my-token".to_string()));
            }
            _ => panic!("Expected HTTP transport"),
        }
    }

    #[test]
    fn test_with_bearer_token_on_stdio_transport() {
        // with_bearer_token should be a no-op for stdio transport
        let config =
            McpServerConfig::new_stdio("fs", "mcp-server").with_bearer_token("ignored-token");

        match config.transport {
            McpTransport::Stdio { .. } => {}
            _ => panic!("Expected stdio transport"),
        }
    }

    #[test]
    fn test_with_headers() {
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
        headers.insert("X-Another".to_string(), "another-value".to_string());

        let config =
            McpServerConfig::new_http("api", "https://example.com").with_headers(headers.clone());

        match config.transport {
            McpTransport::Http {
                headers: actual_headers,
                ..
            } => {
                assert_eq!(
                    actual_headers.get("X-Custom-Header"),
                    Some(&"custom-value".to_string())
                );
                assert_eq!(
                    actual_headers.get("X-Another"),
                    Some(&"another-value".to_string())
                );
            }
            _ => panic!("Expected HTTP transport"),
        }
    }

    #[test]
    fn test_with_headers_on_stdio_transport() {
        // with_headers should be a no-op for stdio transport
        let mut headers = HashMap::new();
        headers.insert("X-Header".to_string(), "value".to_string());

        let config = McpServerConfig::new_stdio("fs", "mcp-server").with_headers(headers);

        match config.transport {
            McpTransport::Stdio { .. } => {}
            _ => panic!("Expected stdio transport"),
        }
    }

    #[test]
    fn test_http_config_with_all_options() {
        let mut headers = HashMap::new();
        headers.insert("X-Api-Key".to_string(), "key123".to_string());

        let config = McpServerConfig::new_http("api", "https://api.example.com/mcp")
            .with_bearer_token("secret-token")
            .with_headers(headers)
            .with_timeout(60);

        assert_eq!(config.name, "api");
        assert_eq!(config.timeout_secs, 60);
        match config.transport {
            McpTransport::Http {
                url,
                bearer_token,
                headers,
            } => {
                assert_eq!(url, "https://api.example.com/mcp");
                assert_eq!(bearer_token, Some("secret-token".to_string()));
                assert_eq!(headers.get("X-Api-Key"), Some(&"key123".to_string()));
            }
            _ => panic!("Expected HTTP transport"),
        }
    }
}
