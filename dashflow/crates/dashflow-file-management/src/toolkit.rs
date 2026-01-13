//! `FileManagementToolkit` - A collection of file operation tools for agents.
//!
//! This toolkit provides agents with capabilities to interact with the filesystem,
//! including reading, writing, copying, moving, deleting, listing, and searching files.
//!
//! # Security Notice
//!
//! This toolkit provides methods to interact with local files. When providing this
//! toolkit to an agent, ensure you scope the agent's permissions to only include
//! the necessary operations.
//!
//! By **default**, the agent will have access to all files within the `root_dir`
//! (or the current working directory if `root_dir` is not specified) and will be able
//! to Copy, Delete, Move, Read, Write, and List files in that directory.
//!
//! Consider the following security measures:
//! - Limit access to particular directories using `root_dir`
//! - Use filesystem permissions to restrict access to only required files
//! - Limit the tools available to the agent using `selected_tools`
//! - Sandbox the agent by running it in a container
//!
//! # Examples
//!
//! ```
//! use dashflow_file_management::FileManagementToolkit;
//! use dashflow::core::tools::BaseToolkit;
//!
//! // Create a toolkit with all tools, scoped to a directory
//! let toolkit = FileManagementToolkit::new(
//!     Some("/tmp/sandbox".to_string()),
//!     None,
//! );
//!
//! // Get all available tools
//! let tools = toolkit.get_tools();
//! assert_eq!(tools.len(), 7);
//!
//! // Create a toolkit with only specific tools
//! let limited_toolkit = FileManagementToolkit::new(
//!     Some("/tmp/sandbox".to_string()),
//!     Some(vec!["read_file".to_string(), "list_directory".to_string()]),
//! );
//!
//! let tools = limited_toolkit.get_tools();
//! assert_eq!(tools.len(), 2);
//! ```

use dashflow::core::tools::{BaseToolkit, Tool};
use std::collections::HashMap;
use std::sync::Arc;

use crate::tools::{
    CopyFileTool, DeleteFileTool, FileSearchTool, ListDirectoryTool, MoveFileTool, ReadFileTool,
    WriteFileTool,
};

/// A toolkit for file management operations.
///
/// Provides a collection of tools for file operations that can be used by agents
/// to interact with the filesystem in a controlled manner.
#[derive(Debug, Clone)]
pub struct FileManagementToolkit {
    /// Optional root directory - if specified, all operations are scoped to this directory
    pub root_dir: Option<String>,
    /// Optional list of tool names to include - if None, all tools are included
    pub selected_tools: Option<Vec<String>>,
}

impl FileManagementToolkit {
    /// Create a new `FileManagementToolkit`
    ///
    /// # Arguments
    ///
    /// * `root_dir` - Optional root directory to scope file operations
    /// * `selected_tools` - Optional list of tool names to include. Valid names are:
    ///   - "`read_file`"
    ///   - "`write_file`"
    ///   - "`copy_file`"
    ///   - "`move_file`"
    ///   - "`file_delete`"
    ///   - "`list_directory`"
    ///   - "`file_search`"
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_file_management::FileManagementToolkit;
    ///
    /// // All tools, no root restriction
    /// let toolkit = FileManagementToolkit::new(None, None);
    ///
    /// // All tools, scoped to /tmp
    /// let toolkit = FileManagementToolkit::new(Some("/tmp".to_string()), None);
    ///
    /// // Only read and list tools
    /// let toolkit = FileManagementToolkit::new(
    ///     Some("/tmp".to_string()),
    ///     Some(vec!["read_file".to_string(), "list_directory".to_string()])
    /// );
    /// ```
    #[must_use]
    pub fn new(root_dir: Option<String>, selected_tools: Option<Vec<String>>) -> Self {
        Self {
            root_dir,
            selected_tools,
        }
    }

    /// Validate that the selected tools are valid tool names
    ///
    /// # Errors
    ///
    /// Returns an error if any of the selected tools are not valid tool names
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref selected) = self.selected_tools {
            let valid_tools = Self::all_tool_names();
            for tool_name in selected {
                if !valid_tools.contains(&tool_name.as_str()) {
                    return Err(format!(
                        "File Tool of name {tool_name} not supported. Permitted tools: {valid_tools:?}"
                    ));
                }
            }
        }
        Ok(())
    }

    /// Get all valid tool names
    fn all_tool_names() -> Vec<&'static str> {
        vec![
            "read_file",
            "write_file",
            "copy_file",
            "move_file",
            "file_delete",
            "list_directory",
            "file_search",
        ]
    }

    /// Create all available tools with the current configuration
    fn create_all_tools(&self) -> HashMap<String, Arc<dyn Tool>> {
        let root = self.root_dir.clone();
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        tools.insert(
            "read_file".to_string(),
            Arc::new(ReadFileTool::new(root.clone())),
        );
        tools.insert(
            "write_file".to_string(),
            Arc::new(WriteFileTool::new(root.clone())),
        );
        tools.insert(
            "copy_file".to_string(),
            Arc::new(CopyFileTool::new(root.clone())),
        );
        tools.insert(
            "move_file".to_string(),
            Arc::new(MoveFileTool::new(root.clone())),
        );
        tools.insert(
            "file_delete".to_string(),
            Arc::new(DeleteFileTool::new(root.clone())),
        );
        tools.insert(
            "list_directory".to_string(),
            Arc::new(ListDirectoryTool::new(root.clone())),
        );
        tools.insert(
            "file_search".to_string(),
            Arc::new(FileSearchTool::new(root)),
        );

        tools
    }
}

impl BaseToolkit for FileManagementToolkit {
    fn get_tools(&self) -> Vec<Arc<dyn Tool>> {
        let all_tools = self.create_all_tools();

        if let Some(ref selected) = self.selected_tools {
            // Return only selected tools
            selected
                .iter()
                .filter_map(|name| all_tools.get(name).cloned())
                .collect()
        } else {
            // Return all tools
            all_tools.into_values().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolkit_all_tools() {
        let toolkit = FileManagementToolkit::new(None, None);
        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn test_toolkit_selected_tools() {
        let toolkit = FileManagementToolkit::new(
            None,
            Some(vec!["read_file".to_string(), "write_file".to_string()]),
        );
        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 2);

        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
    }

    #[test]
    fn test_toolkit_with_root_dir() {
        let toolkit = FileManagementToolkit::new(Some("/tmp".to_string()), None);
        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn test_toolkit_validation_valid() {
        let toolkit = FileManagementToolkit::new(
            None,
            Some(vec!["read_file".to_string(), "write_file".to_string()]),
        );
        assert!(toolkit.validate().is_ok());
    }

    #[test]
    fn test_toolkit_validation_invalid() {
        let toolkit = FileManagementToolkit::new(None, Some(vec!["invalid_tool".to_string()]));
        assert!(toolkit.validate().is_err());
    }

    #[test]
    fn test_toolkit_tool_names() {
        let toolkit = FileManagementToolkit::new(None, None);
        let tools = toolkit.get_tools();

        let mut names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        names.sort();

        let mut expected = vec![
            "copy_file",
            "file_delete",
            "file_search",
            "list_directory",
            "move_file",
            "read_file",
            "write_file",
        ];
        expected.sort();

        assert_eq!(names, expected);
    }

    #[test]
    fn test_toolkit_tool_descriptions() {
        let toolkit = FileManagementToolkit::new(None, None);
        let tools = toolkit.get_tools();

        for tool in tools {
            // Verify each tool has a non-empty description
            assert!(!tool.description().is_empty());
        }
    }

    #[test]
    fn test_toolkit_clone() {
        let toolkit = FileManagementToolkit::new(
            Some("/tmp".to_string()),
            Some(vec!["read_file".to_string()]),
        );

        let cloned = toolkit.clone();
        assert_eq!(toolkit.root_dir, cloned.root_dir);
        assert_eq!(toolkit.selected_tools, cloned.selected_tools);
    }
}
