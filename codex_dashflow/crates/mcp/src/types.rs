//! MCP types for tool definitions and results

use serde::{Deserialize, Serialize};

/// Prefix for MCP tool names
pub const MCP_TOOL_PREFIX: &str = "mcp";
/// Delimiter between parts of MCP tool names
pub const MCP_TOOL_DELIMITER: &str = "__";

/// An MCP tool definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpTool {
    /// Qualified tool name (`mcp__<server>__<tool>`)
    pub qualified_name: String,
    /// Original tool name from the server
    pub name: String,
    /// Server name
    pub server: String,
    /// Tool description
    pub description: Option<String>,
    /// JSON schema for input parameters
    pub input_schema: serde_json::Value,
}

impl McpTool {
    /// Create a new MCP tool
    pub fn new(
        server: impl Into<String>,
        name: impl Into<String>,
        description: Option<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        let server = server.into();
        let name = name.into();
        let qualified_name = format!(
            "{}{}{}{}{}",
            MCP_TOOL_PREFIX, MCP_TOOL_DELIMITER, server, MCP_TOOL_DELIMITER, name
        );
        Self {
            qualified_name,
            name,
            server,
            description,
            input_schema,
        }
    }
}

/// Result of an MCP tool call
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpToolResult {
    /// Content blocks returned by the tool
    pub content: Vec<McpContent>,
    /// Whether the tool call resulted in an error
    pub is_error: bool,
}

impl McpToolResult {
    /// Create a successful text result
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent::Text { text: text.into() }],
            is_error: false,
        }
    }

    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent::Text {
                text: message.into(),
            }],
            is_error: true,
        }
    }
}

/// Content returned by MCP tools
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContent {
    /// Text content
    Text { text: String },
    /// Image content (base64 encoded)
    Image { data: String, mime_type: String },
    /// Resource content
    Resource { uri: String, text: Option<String> },
}

/// Parse an MCP qualified tool name into (server, tool) parts
///
/// Format: `mcp__<server>__<tool>`
pub fn parse_qualified_tool_name(qualified_name: &str) -> Option<(String, String)> {
    let mut parts = qualified_name.split(MCP_TOOL_DELIMITER);

    // Check prefix is "mcp"
    let prefix = parts.next()?;
    if prefix != MCP_TOOL_PREFIX {
        return None;
    }

    // Get server name
    let server = parts.next()?;
    if server.is_empty() {
        return None;
    }

    // Get tool name (may contain delimiters)
    let tool: String = parts.collect::<Vec<_>>().join(MCP_TOOL_DELIMITER);
    if tool.is_empty() {
        return None;
    }

    Some((server.to_string(), tool))
}

/// Check if a tool name is an MCP tool
pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with(&format!("{}{}", MCP_TOOL_PREFIX, MCP_TOOL_DELIMITER))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_qualified_tool_name() {
        assert_eq!(
            parse_qualified_tool_name("mcp__filesystem__read_file"),
            Some(("filesystem".to_string(), "read_file".to_string()))
        );

        assert_eq!(
            parse_qualified_tool_name("mcp__git__commit__message"),
            Some(("git".to_string(), "commit__message".to_string()))
        );
    }

    #[test]
    fn test_parse_qualified_tool_name_invalid() {
        assert_eq!(parse_qualified_tool_name("shell"), None);
        assert_eq!(parse_qualified_tool_name("mcp__"), None);
        assert_eq!(parse_qualified_tool_name("mcp__server__"), None);
        assert_eq!(parse_qualified_tool_name("other__server__tool"), None);
    }

    #[test]
    fn test_is_mcp_tool() {
        assert!(is_mcp_tool("mcp__filesystem__read_file"));
        assert!(!is_mcp_tool("shell"));
        assert!(!is_mcp_tool("read_file"));
    }

    #[test]
    fn test_mcp_tool_creation() {
        let tool = McpTool::new(
            "filesystem",
            "read_file",
            Some("Read a file".to_string()),
            serde_json::json!({"type": "object"}),
        );

        assert_eq!(tool.qualified_name, "mcp__filesystem__read_file");
        assert_eq!(tool.server, "filesystem");
        assert_eq!(tool.name, "read_file");
    }

    #[test]
    fn test_mcp_tool_result_text() {
        let result = McpToolResult::text("Hello, world!");
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
        match &result.content[0] {
            McpContent::Text { text } => assert_eq!(text, "Hello, world!"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_mcp_tool_result_error() {
        let result = McpToolResult::error("Something went wrong");
        assert!(result.is_error);
        assert_eq!(result.content.len(), 1);
        match &result.content[0] {
            McpContent::Text { text } => assert_eq!(text, "Something went wrong"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_mcp_content_serialization() {
        let content = McpContent::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"hello\""));

        let image = McpContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };
        let json = serde_json::to_string(&image).unwrap();
        assert!(json.contains("\"type\":\"image\""));

        let resource = McpContent::Resource {
            uri: "file:///test.txt".to_string(),
            text: Some("content".to_string()),
        };
        let json = serde_json::to_string(&resource).unwrap();
        assert!(json.contains("\"type\":\"resource\""));
    }

    #[test]
    fn test_mcp_content_deserialization() {
        let json = r#"{"type":"text","text":"hello"}"#;
        let content: McpContent = serde_json::from_str(json).unwrap();
        match content {
            McpContent::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_mcp_tool_serialization_roundtrip() {
        let tool = McpTool::new(
            "test-server",
            "test-tool",
            Some("A test tool".to_string()),
            serde_json::json!({"type": "object", "properties": {}}),
        );

        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: McpTool = serde_json::from_str(&json).unwrap();

        assert_eq!(tool.qualified_name, deserialized.qualified_name);
        assert_eq!(tool.name, deserialized.name);
        assert_eq!(tool.server, deserialized.server);
        assert_eq!(tool.description, deserialized.description);
    }

    #[test]
    fn test_mcp_tool_no_description() {
        let tool = McpTool::new(
            "server",
            "tool",
            None,
            serde_json::json!({"type": "object"}),
        );

        assert!(tool.description.is_none());
        assert_eq!(tool.qualified_name, "mcp__server__tool");
    }

    #[test]
    fn test_parse_tool_with_underscores() {
        // Tool names can contain underscores
        assert_eq!(
            parse_qualified_tool_name("mcp__server_name__tool_name"),
            Some(("server_name".to_string(), "tool_name".to_string()))
        );
    }

    #[test]
    fn test_is_mcp_tool_edge_cases() {
        // Edge cases
        assert!(!is_mcp_tool("")); // empty string
        assert!(!is_mcp_tool("mcp")); // just prefix
        assert!(!is_mcp_tool("mcp_")); // single underscore
        assert!(is_mcp_tool("mcp__a__b")); // minimal valid
    }
}
