//! Tool definitions for the Codex agent
//!
//! This module defines the tools available to the agent for code operations.
//! Tools are executed within a sandbox for security. Supports both built-in
//! tools and MCP (Model Context Protocol) tools from external servers.

use codex_dashflow_mcp::McpTool;
use serde::{Deserialize, Serialize};

/// Supported tools
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    /// Execute shell commands
    Shell,
    /// Read file contents
    ReadFile,
    /// Write file contents
    WriteFile,
    /// Apply a unified diff patch
    ApplyPatch,
    /// Search files in the codebase
    SearchFiles,
    /// List directory contents with hierarchy
    ListDir,
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolType::Shell => write!(f, "shell"),
            ToolType::ReadFile => write!(f, "read_file"),
            ToolType::WriteFile => write!(f, "write_file"),
            ToolType::ApplyPatch => write!(f, "apply_patch"),
            ToolType::SearchFiles => write!(f, "search_files"),
            ToolType::ListDir => write!(f, "list_dir"),
        }
    }
}

impl TryFrom<&str> for ToolType {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "shell" => Ok(ToolType::Shell),
            "read_file" => Ok(ToolType::ReadFile),
            "write_file" => Ok(ToolType::WriteFile),
            "apply_patch" => Ok(ToolType::ApplyPatch),
            "search_files" => Ok(ToolType::SearchFiles),
            "list_dir" => Ok(ToolType::ListDir),
            _ => Err(format!("Unknown tool: {}", s)),
        }
    }
}

/// Tool definition for LLM function calling
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for parameters
    pub parameters: serde_json::Value,
}

impl From<&McpTool> for ToolDefinition {
    fn from(mcp_tool: &McpTool) -> Self {
        Self {
            name: mcp_tool.qualified_name.clone(),
            description: mcp_tool
                .description
                .clone()
                .unwrap_or_else(|| format!("MCP tool from server '{}'", mcp_tool.server)),
            parameters: mcp_tool.input_schema.clone(),
        }
    }
}

impl From<McpTool> for ToolDefinition {
    fn from(mcp_tool: McpTool) -> Self {
        Self::from(&mcp_tool)
    }
}

/// Get all tool definitions for LLM function calling
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "shell".to_string(),
            description: "Execute a shell command. Use for running build tools, git commands, \
                          or other system operations."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Write content to a file, creating it if it doesn't exist.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "apply_patch".to_string(),
            description: "Apply a unified diff patch to files.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "The unified diff patch to apply"
                    }
                },
                "required": ["patch"]
            }),
        },
        ToolDefinition {
            name: "search_files".to_string(),
            description: "Search for files. Supports three modes: \
                          fuzzy (default) - fuzzy match file names like fzf/telescope, \
                          content - search file contents with ripgrep, \
                          glob - find files matching glob patterns."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query. For fuzzy: partial file name (e.g. 'toolexec' finds 'tool_execution.rs'). For content: regex pattern. For glob: glob pattern (*.rs, **/*.ts)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (default: current directory)"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["fuzzy", "content", "glob"],
                        "description": "Search mode: fuzzy (default) for file names, content for file contents, glob for glob patterns"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 50)"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "list_dir".to_string(),
            description: "List directory contents with hierarchical tree output. Shows files and \
                          subdirectories with indentation. Supports depth control and pagination \
                          for exploring directory structures."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the directory to list"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "1-indexed starting entry number for pagination (default: 1)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of entries to return (default: 25)"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Maximum depth of subdirectory traversal (default: 2)"
                    }
                },
                "required": ["path"]
            }),
        },
    ]
}

/// Get built-in tool definitions plus MCP tool definitions
///
/// This merges the standard built-in tools with any tools discovered from MCP servers.
/// MCP tools use a qualified name format: `mcp__<server>__<tool>`
pub fn get_tool_definitions_with_mcp(mcp_tools: &[McpTool]) -> Vec<ToolDefinition> {
    let mut tools = get_tool_definitions();

    // Add MCP tools
    for mcp_tool in mcp_tools {
        tools.push(ToolDefinition::from(mcp_tool));
    }

    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_definitions() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 6);
        assert!(tools.iter().any(|t| t.name == "shell"));
        assert!(tools.iter().any(|t| t.name == "read_file"));
        assert!(tools.iter().any(|t| t.name == "write_file"));
        assert!(tools.iter().any(|t| t.name == "list_dir"));
    }

    #[test]
    fn test_mcp_tool_to_definition() {
        let mcp_tool = McpTool::new(
            "filesystem",
            "read_file",
            Some("Read a file from the filesystem".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        );

        let def = ToolDefinition::from(&mcp_tool);
        assert_eq!(def.name, "mcp__filesystem__read_file");
        assert_eq!(def.description, "Read a file from the filesystem");
    }

    #[test]
    fn test_get_tool_definitions_with_mcp() {
        let mcp_tools = vec![
            McpTool::new(
                "git",
                "commit",
                Some("Create a git commit".to_string()),
                serde_json::json!({"type": "object"}),
            ),
            McpTool::new("git", "status", None, serde_json::json!({"type": "object"})),
        ];

        let tools = get_tool_definitions_with_mcp(&mcp_tools);

        // 6 built-in + 2 MCP
        assert_eq!(tools.len(), 8);
        assert!(tools.iter().any(|t| t.name == "mcp__git__commit"));
        assert!(tools.iter().any(|t| t.name == "mcp__git__status"));
    }

    #[test]
    fn test_tool_type_display() {
        assert_eq!(ToolType::Shell.to_string(), "shell");
        assert_eq!(ToolType::ReadFile.to_string(), "read_file");
        assert_eq!(ToolType::WriteFile.to_string(), "write_file");
        assert_eq!(ToolType::ApplyPatch.to_string(), "apply_patch");
        assert_eq!(ToolType::SearchFiles.to_string(), "search_files");
        assert_eq!(ToolType::ListDir.to_string(), "list_dir");
    }

    #[test]
    fn test_tool_type_try_from() {
        assert_eq!(ToolType::try_from("shell").unwrap(), ToolType::Shell);
        assert_eq!(ToolType::try_from("read_file").unwrap(), ToolType::ReadFile);
        assert_eq!(
            ToolType::try_from("write_file").unwrap(),
            ToolType::WriteFile
        );
        assert_eq!(
            ToolType::try_from("apply_patch").unwrap(),
            ToolType::ApplyPatch
        );
        assert_eq!(
            ToolType::try_from("search_files").unwrap(),
            ToolType::SearchFiles
        );
        assert_eq!(ToolType::try_from("list_dir").unwrap(), ToolType::ListDir);
    }

    #[test]
    fn test_tool_type_try_from_invalid() {
        let result = ToolType::try_from("unknown_tool");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool: unknown_tool"));
    }

    #[test]
    fn test_tool_type_serialize() {
        let tool = ToolType::Shell;
        let json = serde_json::to_string(&tool).unwrap();
        assert_eq!(json, r#""shell""#);

        let tool = ToolType::ReadFile;
        let json = serde_json::to_string(&tool).unwrap();
        assert_eq!(json, r#""read_file""#);
    }

    #[test]
    fn test_tool_type_deserialize() {
        let tool: ToolType = serde_json::from_str(r#""shell""#).unwrap();
        assert_eq!(tool, ToolType::Shell);

        let tool: ToolType = serde_json::from_str(r#""read_file""#).unwrap();
        assert_eq!(tool, ToolType::ReadFile);
    }

    #[test]
    fn test_tool_type_roundtrip_serde() {
        for tool in [
            ToolType::Shell,
            ToolType::ReadFile,
            ToolType::WriteFile,
            ToolType::ApplyPatch,
            ToolType::SearchFiles,
            ToolType::ListDir,
        ] {
            let json = serde_json::to_string(&tool).unwrap();
            let parsed: ToolType = serde_json::from_str(&json).unwrap();
            assert_eq!(tool, parsed);
        }
    }

    #[test]
    fn test_tool_definition_serialize() {
        let def = ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };

        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains(r#""name":"test_tool""#));
        assert!(json.contains(r#""description":"A test tool""#));
    }

    #[test]
    fn test_tool_definition_from_mcp_without_description() {
        let mcp_tool = McpTool::new(
            "server",
            "tool",
            None, // No description
            serde_json::json!({"type": "object"}),
        );

        let def = ToolDefinition::from(&mcp_tool);
        assert_eq!(def.name, "mcp__server__tool");
        assert!(def.description.contains("MCP tool from server 'server'"));
    }

    #[test]
    fn test_tool_definition_from_mcp_owned() {
        let mcp_tool = McpTool::new(
            "test",
            "action",
            Some("Does something".to_string()),
            serde_json::json!({"type": "object"}),
        );

        // Test From<McpTool> (owned)
        let def = ToolDefinition::from(mcp_tool);
        assert_eq!(def.name, "mcp__test__action");
        assert_eq!(def.description, "Does something");
    }

    // === Additional tests for comprehensive coverage ===

    // ToolType trait tests
    #[test]
    fn test_tool_type_clone() {
        let tool = ToolType::Shell;
        let cloned = tool.clone();
        assert_eq!(tool, cloned);
    }

    #[test]
    fn test_tool_type_debug() {
        let tool = ToolType::Shell;
        let debug = format!("{:?}", tool);
        assert_eq!(debug, "Shell");

        let tool = ToolType::ReadFile;
        let debug = format!("{:?}", tool);
        assert_eq!(debug, "ReadFile");
    }

    #[test]
    fn test_tool_type_eq() {
        assert_eq!(ToolType::Shell, ToolType::Shell);
        assert_ne!(ToolType::Shell, ToolType::ReadFile);
    }

    #[test]
    fn test_tool_type_all_variants_debug() {
        // Verify all variants have Debug output
        let variants = [
            ToolType::Shell,
            ToolType::ReadFile,
            ToolType::WriteFile,
            ToolType::ApplyPatch,
            ToolType::SearchFiles,
            ToolType::ListDir,
        ];
        for variant in variants {
            let debug = format!("{:?}", variant);
            assert!(!debug.is_empty());
        }
    }

    // ToolDefinition trait tests
    #[test]
    fn test_tool_definition_clone() {
        let def = ToolDefinition {
            name: "test".to_string(),
            description: "desc".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let cloned = def.clone();
        assert_eq!(cloned.name, "test");
        assert_eq!(cloned.description, "desc");
    }

    #[test]
    fn test_tool_definition_debug() {
        let def = ToolDefinition {
            name: "test".to_string(),
            description: "desc".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let debug = format!("{:?}", def);
        assert!(debug.contains("ToolDefinition"));
        assert!(debug.contains("test"));
        assert!(debug.contains("desc"));
    }

    #[test]
    fn test_tool_definition_deserialize() {
        let json = r#"{
            "name": "test_tool",
            "description": "A test tool",
            "parameters": {"type": "object"}
        }"#;
        let def: ToolDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "test_tool");
        assert_eq!(def.description, "A test tool");
    }

    #[test]
    fn test_tool_definition_serde_roundtrip() {
        let def = ToolDefinition {
            name: "round_trip".to_string(),
            description: "Testing roundtrip".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "arg1": {"type": "string"},
                    "arg2": {"type": "integer"}
                },
                "required": ["arg1"]
            }),
        };
        let json = serde_json::to_string(&def).unwrap();
        let parsed: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, def.name);
        assert_eq!(parsed.description, def.description);
        assert_eq!(parsed.parameters, def.parameters);
    }

    // get_tool_definitions validation tests
    #[test]
    fn test_tool_definitions_shell_schema() {
        let tools = get_tool_definitions();
        let shell = tools.iter().find(|t| t.name == "shell").unwrap();
        let props = shell.parameters.get("properties").unwrap();
        assert!(props.get("command").is_some());
        let required = shell.parameters.get("required").unwrap();
        assert!(required
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("command")));
    }

    #[test]
    fn test_tool_definitions_read_file_schema() {
        let tools = get_tool_definitions();
        let read_file = tools.iter().find(|t| t.name == "read_file").unwrap();
        let props = read_file.parameters.get("properties").unwrap();
        assert!(props.get("path").is_some());
    }

    #[test]
    fn test_tool_definitions_write_file_schema() {
        let tools = get_tool_definitions();
        let write_file = tools.iter().find(|t| t.name == "write_file").unwrap();
        let props = write_file.parameters.get("properties").unwrap();
        assert!(props.get("path").is_some());
        assert!(props.get("content").is_some());
        let required = write_file.parameters.get("required").unwrap();
        let required_arr = required.as_array().unwrap();
        assert!(required_arr.contains(&serde_json::json!("path")));
        assert!(required_arr.contains(&serde_json::json!("content")));
    }

    #[test]
    fn test_tool_definitions_apply_patch_schema() {
        let tools = get_tool_definitions();
        let apply_patch = tools.iter().find(|t| t.name == "apply_patch").unwrap();
        let props = apply_patch.parameters.get("properties").unwrap();
        assert!(props.get("patch").is_some());
    }

    #[test]
    fn test_tool_definitions_search_files_schema() {
        let tools = get_tool_definitions();
        let search_files = tools.iter().find(|t| t.name == "search_files").unwrap();
        let props = search_files.parameters.get("properties").unwrap();
        assert!(props.get("query").is_some());
        assert!(props.get("path").is_some());
        assert!(props.get("mode").is_some());
        assert!(props.get("limit").is_some());
        // Check mode enum values
        let mode_enum = props.get("mode").unwrap().get("enum").unwrap();
        let mode_values = mode_enum.as_array().unwrap();
        assert!(mode_values.contains(&serde_json::json!("fuzzy")));
        assert!(mode_values.contains(&serde_json::json!("content")));
        assert!(mode_values.contains(&serde_json::json!("glob")));
    }

    #[test]
    fn test_tool_definitions_list_dir_schema() {
        let tools = get_tool_definitions();
        let list_dir = tools.iter().find(|t| t.name == "list_dir").unwrap();
        let props = list_dir.parameters.get("properties").unwrap();
        assert!(props.get("path").is_some());
        assert!(props.get("offset").is_some());
        assert!(props.get("limit").is_some());
        assert!(props.get("depth").is_some());
    }

    #[test]
    fn test_tool_definitions_all_have_descriptions() {
        let tools = get_tool_definitions();
        for tool in &tools {
            assert!(
                !tool.description.is_empty(),
                "Tool {} has empty description",
                tool.name
            );
        }
    }

    #[test]
    fn test_tool_definitions_all_have_valid_schemas() {
        let tools = get_tool_definitions();
        for tool in &tools {
            assert!(
                tool.parameters.is_object(),
                "Tool {} parameters should be object",
                tool.name
            );
            assert!(
                tool.parameters.get("type").is_some(),
                "Tool {} missing type",
                tool.name
            );
            assert_eq!(tool.parameters.get("type").unwrap(), "object");
        }
    }

    // get_tool_definitions_with_mcp tests
    #[test]
    fn test_get_tool_definitions_with_empty_mcp() {
        let mcp_tools: Vec<McpTool> = vec![];
        let tools = get_tool_definitions_with_mcp(&mcp_tools);
        assert_eq!(tools.len(), 6); // Just built-in tools
    }

    #[test]
    fn test_get_tool_definitions_with_mcp_preserves_order() {
        let mcp_tools = vec![McpTool::new(
            "test",
            "action",
            Some("Test".to_string()),
            serde_json::json!({"type": "object"}),
        )];
        let tools = get_tool_definitions_with_mcp(&mcp_tools);
        // Built-in tools should come first
        assert_eq!(tools[0].name, "shell");
        // MCP tool should be last
        assert_eq!(tools.last().unwrap().name, "mcp__test__action");
    }

    // MCP tool conversion tests
    #[test]
    fn test_mcp_tool_preserves_input_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "file": {"type": "string", "description": "File path"},
                "content": {"type": "string"}
            },
            "required": ["file"]
        });
        let mcp_tool = McpTool::new("server", "tool", Some("desc".to_string()), schema.clone());
        let def = ToolDefinition::from(&mcp_tool);
        assert_eq!(def.parameters, schema);
    }

    #[test]
    fn test_mcp_tool_qualified_name_format() {
        let mcp_tool = McpTool::new(
            "my-server",
            "my-tool",
            Some("desc".to_string()),
            serde_json::json!({"type": "object"}),
        );
        let def = ToolDefinition::from(&mcp_tool);
        // Qualified name should be mcp__<server>__<tool>
        assert_eq!(def.name, "mcp__my-server__my-tool");
    }

    // ToolType Display vs TryFrom consistency
    #[test]
    fn test_tool_type_display_try_from_consistency() {
        let variants = [
            ToolType::Shell,
            ToolType::ReadFile,
            ToolType::WriteFile,
            ToolType::ApplyPatch,
            ToolType::SearchFiles,
            ToolType::ListDir,
        ];
        for variant in variants {
            let display = variant.to_string();
            let parsed = ToolType::try_from(display.as_str()).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_tool_type_serde_display_consistency() {
        // Serde and Display should produce the same string representation
        let variants = [
            ToolType::Shell,
            ToolType::ReadFile,
            ToolType::WriteFile,
            ToolType::ApplyPatch,
            ToolType::SearchFiles,
            ToolType::ListDir,
        ];
        for variant in variants {
            let display = variant.to_string();
            let serde_json = serde_json::to_string(&variant).unwrap();
            // serde_json adds quotes, so compare with quoted display
            assert_eq!(serde_json, format!("\"{}\"", display));
        }
    }
}
