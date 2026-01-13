//! # dashflow-file-management
//!
//! File management toolkit for `DashFlow` agents.
//!
//! This crate provides a comprehensive set of tools for file system operations,
//! enabling agents to interact with files and directories in a controlled and
//! secure manner.
//!
//! ## Features
//!
//! - **File Operations**: Read, write, copy, move, delete files
//! - **Directory Operations**: List directory contents, search for files
//! - **Security**: Sandboxing with root directory restrictions
//! - **Path Validation**: Protection against directory traversal attacks
//!
//! ## Quick Start
//!
//! ```
//! use dashflow_file_management::FileManagementToolkit;
//! use dashflow::core::tools::BaseToolkit;
//!
//! // Create a toolkit with all tools
//! let toolkit = FileManagementToolkit::new(
//!     Some("/tmp/sandbox".to_string()),  // Restrict to this directory
//!     None,                                // Include all tools
//! );
//!
//! // Get the tools for use with an agent
//! let tools = toolkit.get_tools();
//! ```
//!
//! ## Available Tools
//!
//! - **`read_file`**: Read file contents
//! - **`write_file`**: Write or append to files
//! - **`copy_file`**: Copy files to new locations
//! - **`move_file`**: Move or rename files
//! - **`file_delete`**: Delete files
//! - **`list_directory`**: List directory contents
//! - **`file_search`**: Search for files matching patterns
//!
//! ## Security Considerations
//!
//! When using this toolkit with agents:
//!
//! 1. **Always set `root_dir`**: Restrict operations to a specific directory
//! 2. **Limit tools**: Only provide necessary tools using `selected_tools`
//! 3. **Use file permissions**: Leverage OS-level permissions for additional security
//! 4. **Sandbox execution**: Run agents in containers when possible
//!
//! ## Example with Specific Tools
//!
//! ```
//! use dashflow_file_management::FileManagementToolkit;
//! use dashflow::core::tools::BaseToolkit;
//!
//! // Create a toolkit with only read and list capabilities
//! let toolkit = FileManagementToolkit::new(
//!     Some("/home/user/documents".to_string()),
//!     Some(vec![
//!         "read_file".to_string(),
//!         "list_directory".to_string(),
//!     ]),
//! );
//!
//! let tools = toolkit.get_tools();
//! assert_eq!(tools.len(), 2);
//! ```

pub mod toolkit;
pub mod tools;
pub mod utils;

// Re-export main types
pub use toolkit::FileManagementToolkit;
pub use tools::{
    CopyFileTool, DeleteFileTool, FileSearchTool, ListDirectoryTool, MoveFileTool, ReadFileTool,
    WriteFileTool,
};
