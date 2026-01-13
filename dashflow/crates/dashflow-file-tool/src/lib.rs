//! File management tools for `DashFlow` Rust.
//!
//! This crate provides a comprehensive set of tools for file and directory operations,
//! enabling AI agents to interact with the file system safely and efficiently.
//!
//! # Tools
//!
//! - **`ReadFileTool`**: Read file contents
//! - **`WriteFileTool`**: Write or append to files
//! - **`ListDirectoryTool`**: List directory contents
//! - **`CopyFileTool`**: Copy files
//! - **`MoveFileTool`**: Move or rename files
//! - **`DeleteFileTool`**: Delete files
//! - **`FileSearchTool`**: Search for files by name pattern
//!
//! # Security
//!
//! All tools support optional directory allowlisting to restrict file operations
//! to specific directories. Use `with_allowed_dirs()` to configure security boundaries.
//!
//! # Example
//!
//! ```rust
//! use dashflow_file_tool::ReadFileTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//! use serde_json::json;
//! use std::path::PathBuf;
//!
//! # tokio_test::block_on(async {
//! // Create a read tool with directory restriction
//! let tool = ReadFileTool::new()
//!     .with_allowed_dirs(vec![PathBuf::from("/tmp")]);
//!
//! let input = json!({"file_path": "/tmp/example.txt"});
//! // let result = tool._call(ToolInput::Structured(input)).await.unwrap();
//! # });
//! ```
//!
//! # See Also
//!
//! - [`Tool`] - The trait these tools implement
//! - [`dashflow-shell-tool`](https://docs.rs/dashflow-shell-tool) - Shell command execution (more powerful but less safe)
//! - [`dashflow-json-tool`](https://docs.rs/dashflow-json-tool) - JSON parsing and querying tool
//! - [`dashflow-webscrape`](https://docs.rs/dashflow-webscrape) - Web content retrieval tool

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Error;
use serde_json::json;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::fs;
use tokio::io::AsyncWriteExt;

// ============================================================================
// Security Helper
// ============================================================================

/// Check for path traversal patterns that could escape directory restrictions.
///
/// This is a defense-in-depth check that runs before canonicalization.
fn contains_path_traversal(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Check for null bytes (can cause truncation in some systems)
    if path_str.contains('\0') {
        return true;
    }

    // Check for explicit path traversal components.
    //
    // IMPORTANT: Don't treat any occurrence of ".." as traversal, since it's valid for
    // filenames to contain ".." (e.g. "report..backup.json"). Only block the actual
    // parent-directory component.
    if path_str
        .split(|c| c == '/' || c == '\\')
        .any(|component| component == "..")
    {
        return true;
    }

    // Check for double slashes which could be used to confuse parsers
    // (allow single slash at start for absolute paths)
    let without_prefix = path_str.trim_start_matches('/');
    if without_prefix.contains("//") {
        return true;
    }

    false
}

/// Normalize a path for security checking.
///
/// For existing paths, uses canonicalize().
/// For non-existent paths (new files), normalizes using parent directory.
fn normalize_path_for_check(path: &Path) -> Option<PathBuf> {
    // First try canonicalize (works for existing files)
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }

    // For non-existent files, canonicalize the parent and append the filename
    let parent = path.parent()?;
    let file_name = path.file_name()?;

    // Try to canonicalize the parent directory
    // If parent doesn't exist either, try its parent recursively
    let canonical_parent = if parent.as_os_str().is_empty() {
        // No parent (relative path with just filename)
        std::env::current_dir().ok()?
    } else if let Ok(p) = parent.canonicalize() {
        p
    } else {
        // Parent doesn't exist - this is okay for new nested directories
        // Use the most existing ancestor we can find
        let mut current = parent;
        let mut components_to_add = Vec::new();

        loop {
            if let Ok(canonical) = current.canonicalize() {
                // Found an existing ancestor, rebuild the path
                let mut result = canonical;
                for component in components_to_add.into_iter().rev() {
                    result = result.join(component);
                }
                return Some(result.join(file_name));
            }

            // Move up one level
            if let (Some(p), Some(name)) = (current.parent(), current.file_name()) {
                components_to_add.push(name.to_os_string());
                current = p;
                if current.as_os_str().is_empty() {
                    // Reached root of relative path
                    let mut result = std::env::current_dir().ok()?;
                    for component in components_to_add.into_iter().rev() {
                        result = result.join(component);
                    }
                    return Some(result.join(file_name));
                }
            } else {
                // Can't go further up
                return None;
            }
        }
    };

    Some(canonical_parent.join(file_name))
}

/// Check if a path is within allowed directories (synchronous version for tests)
fn is_path_allowed(path: &Path, allowed_dirs: &[PathBuf]) -> bool {
    if allowed_dirs.is_empty() {
        return true; // No restrictions
    }

    // Defense-in-depth: Check for path traversal patterns before canonicalization
    // This catches attempts to escape restrictions that might succeed due to
    // canonicalization quirks or race conditions
    if contains_path_traversal(path) {
        return false;
    }

    // Normalize path (works for both existing and new files)
    let normalized_path = match normalize_path_for_check(path) {
        Some(p) => p,
        None => return false,
    };

    // Check if normalized path is within any allowed directory
    allowed_dirs.iter().any(|allowed| {
        if let Ok(allowed_abs) = allowed.canonicalize() {
            normalized_path.starts_with(&allowed_abs)
        } else {
            false
        }
    })
}

/// Check if a path is within allowed directories (async version using spawn_blocking)
///
/// This wraps the blocking `canonicalize()` calls in `spawn_blocking` to avoid
/// blocking the async runtime (M-633).
async fn is_path_allowed_async(path: PathBuf, allowed_dirs: Vec<PathBuf>) -> bool {
    tokio::task::spawn_blocking(move || is_path_allowed(&path, &allowed_dirs))
        .await
        .unwrap_or(false)
}

// ============================================================================
// ReadFileTool
// ============================================================================

/// Default maximum file size for read operations (10 MB).
const DEFAULT_MAX_READ_SIZE: u64 = 10 * 1024 * 1024;

/// Tool for reading file contents.
///
/// Supports both absolute and relative paths. Can be restricted to specific
/// directories using `with_allowed_dirs()` and size-limited using `with_max_size()`.
///
/// # Input Format
///
/// - **String**: File path
/// - **Structured**: `{"file_path": "path/to/file.txt"}`
///
/// # Example
///
/// ```rust
/// use dashflow_file_tool::ReadFileTool;
/// use dashflow::core::tools::Tool;
///
/// let tool = ReadFileTool::new();
/// assert_eq!(tool.name(), "read_file");
/// ```
#[derive(Clone, Debug)]
pub struct ReadFileTool {
    allowed_dirs: Vec<PathBuf>,
    /// Maximum file size in bytes (default: 10 MB)
    max_size: u64,
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self {
            allowed_dirs: Vec::new(),
            max_size: DEFAULT_MAX_READ_SIZE,
        }
    }
}

impl ReadFileTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict file operations to specific directories
    #[must_use]
    pub fn with_allowed_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.allowed_dirs = dirs;
        self
    }

    /// Set maximum file size in bytes (default: 10 MB)
    ///
    /// Files larger than this limit will return an error to prevent memory exhaustion.
    #[must_use]
    pub fn with_max_size(mut self, max_size: u64) -> Self {
        self.max_size = max_size;
        self
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the entire contents of a file. Provide the file path as input. \
         Returns the file contents as a string."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let file_path = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(obj) => obj
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::tool_error("Missing or invalid 'file_path' field"))?
                .to_string(),
        };

        let path = PathBuf::from(&file_path);

        // Security check (async to avoid blocking on canonicalize - M-633)
        if !is_path_allowed_async(path.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: '{file_path}' is outside allowed directories"
            )));
        }

        // Size check to prevent memory exhaustion
        let metadata = fs::metadata(&path)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to read file metadata: {e}")))?;

        if metadata.len() > self.max_size {
            return Err(Error::tool_error(format!(
                "File too large: {} bytes exceeds maximum of {} bytes ({} MB). \
                 Use with_max_size() to increase the limit.",
                metadata.len(),
                self.max_size,
                self.max_size / (1024 * 1024)
            )));
        }

        let contents = fs::read_to_string(path)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to read file: {e}")))?;

        Ok(contents)
    }
}

// ============================================================================
// WriteFileTool
// ============================================================================

/// Tool for writing to files.
///
/// Supports both creating new files and appending to existing files.
/// Can be restricted to specific directories using `with_allowed_dirs()`.
///
/// # Input Format
///
/// ```json
/// {
///     "file_path": "path/to/file.txt",
///     "text": "Content to write",
///     "append": false  // Optional, defaults to false
/// }
/// ```
///
/// # Example
///
/// ```rust
/// use dashflow_file_tool::WriteFileTool;
/// use dashflow::core::tools::Tool;
///
/// let tool = WriteFileTool::new();
/// assert_eq!(tool.name(), "write_file");
/// ```
#[derive(Clone, Debug, Default)]
pub struct WriteFileTool {
    allowed_dirs: Vec<PathBuf>,
}

impl WriteFileTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict file operations to specific directories
    #[must_use]
    pub fn with_allowed_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.allowed_dirs = dirs;
        self
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write text to a file. If the file exists, it will be overwritten unless \
         'append' is set to true. Provide 'file_path' and 'text' as input."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "text": {
                    "type": "string",
                    "description": "Text content to write to the file"
                },
                "append": {
                    "type": "boolean",
                    "description": "Whether to append to the file (default: false)",
                    "default": false
                }
            },
            "required": ["file_path", "text"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let (file_path, text, append) = match input {
            ToolInput::Structured(obj) => {
                let file_path = obj
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing or invalid 'file_path' field"))?
                    .to_string();
                let text = obj
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing or invalid 'text' field"))?
                    .to_string();
                let append = obj
                    .get("append")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                (file_path, text, append)
            }
            ToolInput::String(_) => {
                return Err(Error::tool_error(
                    "WriteFileTool requires structured input with 'file_path' and 'text'",
                ))
            }
        };

        let path = PathBuf::from(&file_path);

        // Security check (async to avoid blocking on canonicalize - M-633)
        if !is_path_allowed_async(path.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: '{file_path}' is outside allowed directories"
            )));
        }

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::tool_error(format!("Failed to create directory: {e}")))?;
        }

        // Write or append
        if append {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await
                .map_err(|e| Error::tool_error(format!("Failed to open file: {e}")))?;
            file.write_all(text.as_bytes())
                .await
                .map_err(|e| Error::tool_error(format!("Failed to write file: {e}")))?;
            Ok(format!(
                "Successfully appended {} bytes to '{}'",
                text.len(),
                file_path
            ))
        } else {
            fs::write(path, text.as_bytes())
                .await
                .map_err(|e| Error::tool_error(format!("Failed to write file: {e}")))?;
            Ok(format!(
                "Successfully wrote {} bytes to '{}'",
                text.len(),
                file_path
            ))
        }
    }
}

// ============================================================================
// ListDirectoryTool
// ============================================================================

/// Tool for listing directory contents.
///
/// Returns a list of files and directories in the specified path.
/// Can be restricted to specific directories using `with_allowed_dirs()`.
///
/// # Input Format
///
/// - **String**: Directory path
/// - **Structured**: `{"dir_path": "path/to/directory"}`
///
/// # Example
///
/// ```rust
/// use dashflow_file_tool::ListDirectoryTool;
/// use dashflow::core::tools::Tool;
///
/// let tool = ListDirectoryTool::new();
/// assert_eq!(tool.name(), "list_directory");
/// ```
#[derive(Clone, Debug, Default)]
pub struct ListDirectoryTool {
    allowed_dirs: Vec<PathBuf>,
}

impl ListDirectoryTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict operations to specific directories
    #[must_use]
    pub fn with_allowed_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.allowed_dirs = dirs;
        self
    }
}

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &'static str {
        "list_directory"
    }

    fn description(&self) -> &'static str {
        "List all files and directories in a given directory path. \
         Returns a formatted list with file/directory indicators."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "dir_path": {
                    "type": "string",
                    "description": "Path to the directory to list"
                }
            },
            "required": ["dir_path"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let dir_path = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(obj) => obj
                .get("dir_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::tool_error("Missing or invalid 'dir_path' field"))?
                .to_string(),
        };

        let path = PathBuf::from(&dir_path);

        // Security check (async to avoid blocking on canonicalize - M-633)
        if !is_path_allowed_async(path.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: '{dir_path}' is outside allowed directories"
            )));
        }

        let mut entries = fs::read_dir(&path)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to read directory: {e}")))?;

        let mut items = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to read entry: {e}")))?
        {
            let metadata = entry
                .metadata()
                .await
                .map_err(|e| Error::tool_error(format!("Failed to read metadata: {e}")))?;
            let file_type = if metadata.is_dir() { "[DIR]" } else { "[FILE]" };
            let file_name = entry.file_name();
            items.push(format!("{} {}", file_type, file_name.to_string_lossy()));
        }

        items.sort();

        if items.is_empty() {
            Ok("Directory is empty".to_string())
        } else {
            Ok(format!("Contents of '{}':\n{}", dir_path, items.join("\n")))
        }
    }
}

// ============================================================================
// CopyFileTool
// ============================================================================

/// Tool for copying files.
///
/// Copies a file from source to destination. Creates parent directories if needed.
///
/// # Input Format
///
/// ```json
/// {
///     "source_path": "path/to/source.txt",
///     "destination_path": "path/to/dest.txt"
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct CopyFileTool {
    allowed_dirs: Vec<PathBuf>,
}

impl CopyFileTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict operations to specific directories
    #[must_use]
    pub fn with_allowed_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.allowed_dirs = dirs;
        self
    }
}

#[async_trait]
impl Tool for CopyFileTool {
    fn name(&self) -> &'static str {
        "copy_file"
    }

    fn description(&self) -> &'static str {
        "Copy a file from source path to destination path. \
         Creates parent directories if needed."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "destination_path": {
                    "type": "string",
                    "description": "Path to the destination file"
                }
            },
            "required": ["source_path", "destination_path"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let (source_path, dest_path) = match input {
            ToolInput::Structured(obj) => {
                let source = obj
                    .get("source_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing or invalid 'source_path' field"))?
                    .to_string();
                let dest = obj
                    .get("destination_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::tool_error("Missing or invalid 'destination_path' field")
                    })?
                    .to_string();
                (source, dest)
            }
            ToolInput::String(_) => return Err(Error::tool_error(
                "CopyFileTool requires structured input with 'source_path' and 'destination_path'",
            )),
        };

        let source = PathBuf::from(&source_path);
        let dest = PathBuf::from(&dest_path);

        // Security checks (async to avoid blocking on canonicalize - M-633)
        if !is_path_allowed_async(source.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: source '{source_path}' is outside allowed directories"
            )));
        }
        if !is_path_allowed_async(dest.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: destination '{dest_path}' is outside allowed directories"
            )));
        }

        // Create parent directory if needed
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::tool_error(format!("Failed to create directory: {e}")))?;
        }

        fs::copy(&source, &dest)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to copy file: {e}")))?;

        Ok(format!(
            "Successfully copied '{source_path}' to '{dest_path}'"
        ))
    }
}

// ============================================================================
// MoveFileTool
// ============================================================================

/// Tool for moving or renaming files.
///
/// Moves a file from source to destination. Can also be used to rename files.
///
/// # Input Format
///
/// ```json
/// {
///     "source_path": "path/to/source.txt",
///     "destination_path": "path/to/dest.txt"
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct MoveFileTool {
    allowed_dirs: Vec<PathBuf>,
}

impl MoveFileTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict operations to specific directories
    #[must_use]
    pub fn with_allowed_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.allowed_dirs = dirs;
        self
    }
}

#[async_trait]
impl Tool for MoveFileTool {
    fn name(&self) -> &'static str {
        "move_file"
    }

    fn description(&self) -> &'static str {
        "Move or rename a file from source path to destination path. \
         Creates parent directories if needed."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "destination_path": {
                    "type": "string",
                    "description": "Path to the destination file"
                }
            },
            "required": ["source_path", "destination_path"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let (source_path, dest_path) = match input {
            ToolInput::Structured(obj) => {
                let source = obj
                    .get("source_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing or invalid 'source_path' field"))?
                    .to_string();
                let dest = obj
                    .get("destination_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::tool_error("Missing or invalid 'destination_path' field")
                    })?
                    .to_string();
                (source, dest)
            }
            ToolInput::String(_) => return Err(Error::tool_error(
                "MoveFileTool requires structured input with 'source_path' and 'destination_path'",
            )),
        };

        let source = PathBuf::from(&source_path);
        let dest = PathBuf::from(&dest_path);

        // Security checks (async to avoid blocking on canonicalize - M-633)
        if !is_path_allowed_async(source.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: source '{source_path}' is outside allowed directories"
            )));
        }
        if !is_path_allowed_async(dest.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: destination '{dest_path}' is outside allowed directories"
            )));
        }

        // Create parent directory if needed
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::tool_error(format!("Failed to create directory: {e}")))?;
        }

        fs::rename(&source, &dest)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to move file: {e}")))?;

        Ok(format!(
            "Successfully moved '{source_path}' to '{dest_path}'"
        ))
    }
}

// ============================================================================
// DeleteFileTool
// ============================================================================

/// Tool for deleting files.
///
/// Permanently deletes a file. Use with caution.
///
/// # Input Format
///
/// - **String**: File path
/// - **Structured**: `{"file_path": "path/to/file.txt"}`
#[derive(Clone, Debug, Default)]
pub struct DeleteFileTool {
    allowed_dirs: Vec<PathBuf>,
}

impl DeleteFileTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict operations to specific directories
    #[must_use]
    pub fn with_allowed_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.allowed_dirs = dirs;
        self
    }
}

#[async_trait]
impl Tool for DeleteFileTool {
    fn name(&self) -> &'static str {
        "delete_file"
    }

    fn description(&self) -> &'static str {
        "Delete a file permanently. Use with caution as this operation cannot be undone."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to delete"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let file_path = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(obj) => obj
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::tool_error("Missing or invalid 'file_path' field"))?
                .to_string(),
        };

        let path = PathBuf::from(&file_path);

        // Security check (async to avoid blocking on canonicalize - M-633)
        if !is_path_allowed_async(path.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: '{file_path}' is outside allowed directories"
            )));
        }

        fs::remove_file(&path)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to delete file: {e}")))?;

        Ok(format!("Successfully deleted '{file_path}'"))
    }
}

// ============================================================================
// FileSearchTool
// ============================================================================

/// Tool for searching files by name pattern.
///
/// Recursively searches a directory for files matching a pattern.
/// Supports glob-style wildcards (* and ?).
///
/// # Input Format
///
/// ```json
/// {
///     "dir_path": "path/to/search",
///     "pattern": "*.txt"
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct FileSearchTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileSearchTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict operations to specific directories
    #[must_use]
    pub fn with_allowed_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.allowed_dirs = dirs;
        self
    }

    /// Recursively search directory for files matching pattern
    fn search_recursive<'a>(
        dir: &'a Path,
        pattern: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PathBuf>, Error>> + Send + 'a>> {
        Box::pin(async move {
            let mut results = Vec::new();
            let mut entries = fs::read_dir(dir)
                .await
                .map_err(|e| Error::tool_error(format!("Failed to read directory: {e}")))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| Error::tool_error(format!("Failed to read entry: {e}")))?
            {
                let path = entry.path();
                let metadata = entry
                    .metadata()
                    .await
                    .map_err(|e| Error::tool_error(format!("Failed to read metadata: {e}")))?;

                if metadata.is_dir() {
                    // Recurse into subdirectory
                    if let Ok(mut subdir_results) = Self::search_recursive(&path, pattern).await {
                        results.append(&mut subdir_results);
                    }
                } else if metadata.is_file() {
                    // Check if file name matches pattern
                    if let Some(file_name) = path.file_name() {
                        if let Some(name_str) = file_name.to_str() {
                            if Self::matches_pattern(name_str, pattern) {
                                results.push(path);
                            }
                        }
                    }
                }
            }

            Ok(results)
        })
    }

    /// Simple glob-style pattern matching (supports * and ?)
    fn matches_pattern(name: &str, pattern: &str) -> bool {
        // Convert glob pattern to regex
        let regex_pattern = pattern
            .replace('.', "\\.")
            .replace('*', ".*")
            .replace('?', ".");
        let regex_pattern = format!("^{regex_pattern}$");

        // Use bounded regex compilation to prevent ReDoS on malicious patterns
        if let Ok(re) = regex::RegexBuilder::new(&regex_pattern)
            .size_limit(256 * 1024) // 256KB limit
            .dfa_size_limit(256 * 1024)
            .build()
        {
            re.is_match(name)
        } else {
            false
        }
    }
}

#[async_trait]
impl Tool for FileSearchTool {
    fn name(&self) -> &'static str {
        "file_search"
    }

    fn description(&self) -> &'static str {
        "Search for files in a directory matching a pattern. \
         Supports wildcards: * (any characters) and ? (single character). \
         Searches recursively through subdirectories."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "dir_path": {
                    "type": "string",
                    "description": "Directory path to search in"
                },
                "pattern": {
                    "type": "string",
                    "description": "File name pattern with wildcards (e.g., '*.txt', 'file?.log')"
                }
            },
            "required": ["dir_path", "pattern"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let (dir_path, pattern) = match input {
            ToolInput::Structured(obj) => {
                let dir = obj
                    .get("dir_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing or invalid 'dir_path' field"))?
                    .to_string();
                let pat = obj
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing or invalid 'pattern' field"))?
                    .to_string();
                (dir, pat)
            }
            ToolInput::String(_) => {
                return Err(Error::tool_error(
                    "FileSearchTool requires structured input with 'dir_path' and 'pattern'",
                ))
            }
        };

        let path = PathBuf::from(&dir_path);

        // Security check (async to avoid blocking on canonicalize - M-633)
        if !is_path_allowed_async(path.clone(), self.allowed_dirs.clone()).await {
            return Err(Error::tool_error(format!(
                "Access denied: '{dir_path}' is outside allowed directories"
            )));
        }

        let results = Self::search_recursive(&path, &pattern).await?;

        if results.is_empty() {
            Ok(format!(
                "No files matching '{pattern}' found in '{dir_path}'"
            ))
        } else {
            let file_list: Vec<String> = results
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            Ok(format!(
                "Found {} file(s) matching '{}':\n{}",
                results.len(),
                pattern,
                file_list.join("\n")
            ))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use dashflow_standard_tests::tool_comprehensive_tests::ToolComprehensiveTests;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_read_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!").await.unwrap();

        let tool = ReadFileTool::new();
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[tokio::test]
    async fn test_read_file_size_limit_enforced() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_file.txt");

        // Create a file larger than our custom limit (100 bytes)
        let large_content = "x".repeat(200);
        fs::write(&file_path, &large_content).await.unwrap();

        // Use a small max_size to test the limit
        let tool = ReadFileTool::new().with_max_size(100);
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("File too large"),
            "Expected 'File too large' error, got: {err}"
        );
        assert!(err.contains("200 bytes"), "Expected file size in error");
    }

    #[tokio::test]
    async fn test_read_file_within_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("small_file.txt");

        // Create a file smaller than our custom limit
        let content = "small content";
        fs::write(&file_path, content).await.unwrap();

        // Use a limit larger than the file
        let tool = ReadFileTool::new().with_max_size(100);
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert_eq!(result, content);
    }

    #[tokio::test]
    async fn test_write_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("write_test.txt");

        let tool = WriteFileTool::new();
        let input = json!({
            "file_path": file_path.to_str().unwrap(),
            "text": "Test content"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Successfully wrote"));

        let contents = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(contents, "Test content");
    }

    #[tokio::test]
    async fn test_write_file_append() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("append_test.txt");
        fs::write(&file_path, "First\n").await.unwrap();

        let tool = WriteFileTool::new();
        let input = json!({
            "file_path": file_path.to_str().unwrap(),
            "text": "Second\n",
            "append": true
        });
        tool._call(ToolInput::Structured(input)).await.unwrap();

        let contents = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(contents, "First\nSecond\n");
    }

    #[tokio::test]
    async fn test_list_directory_tool() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "")
            .await
            .unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "")
            .await
            .unwrap();
        fs::create_dir(temp_dir.path().join("subdir"))
            .await
            .unwrap();

        let tool = ListDirectoryTool::new();
        let input = json!({"dir_path": temp_dir.path().to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("[FILE] file1.txt"));
        assert!(result.contains("[FILE] file2.txt"));
        assert!(result.contains("[DIR] subdir"));
    }

    #[tokio::test]
    async fn test_copy_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        fs::write(&source, "Copy me").await.unwrap();

        let tool = CopyFileTool::new();
        let input = json!({
            "source_path": source.to_str().unwrap(),
            "destination_path": dest.to_str().unwrap()
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Successfully copied"));

        let contents = fs::read_to_string(&dest).await.unwrap();
        assert_eq!(contents, "Copy me");
    }

    #[tokio::test]
    async fn test_move_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        fs::write(&source, "Move me").await.unwrap();

        let tool = MoveFileTool::new();
        let input = json!({
            "source_path": source.to_str().unwrap(),
            "destination_path": dest.to_str().unwrap()
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Successfully moved"));

        assert!(!source.exists());
        let contents = fs::read_to_string(&dest).await.unwrap();
        assert_eq!(contents, "Move me");
    }

    #[tokio::test]
    async fn test_delete_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("delete.txt");
        fs::write(&file_path, "Delete me").await.unwrap();

        let tool = DeleteFileTool::new();
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Successfully deleted"));

        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_file_search_tool() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test1.txt"), "")
            .await
            .unwrap();
        fs::write(temp_dir.path().join("test2.txt"), "")
            .await
            .unwrap();
        fs::write(temp_dir.path().join("other.log"), "")
            .await
            .unwrap();

        let tool = FileSearchTool::new();
        let input = json!({
            "dir_path": temp_dir.path().to_str().unwrap(),
            "pattern": "*.txt"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("test1.txt"));
        assert!(result.contains("test2.txt"));
        assert!(!result.contains("other.log"));
    }

    #[tokio::test]
    async fn test_security_allowed_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Secret").await.unwrap();

        let other_dir = TempDir::new().unwrap();

        // Tool restricted to other_dir (not temp_dir)
        let tool = ReadFileTool::new().with_allowed_dirs(vec![other_dir.path().to_path_buf()]);

        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Access denied"));
    }

    // ========================================================================
    // Comprehensive Tests - Error Scenarios & Edge Cases
    // ========================================================================

    /// Test helper struct for comprehensive tests on ReadFileTool
    struct ReadFileToolComprehensiveTests {
        tool: ReadFileTool,
        temp_dir: TempDir,
    }

    impl ReadFileToolComprehensiveTests {
        fn new() -> Self {
            let temp_dir = TempDir::new().unwrap();
            Self {
                tool: ReadFileTool::new(),
                temp_dir,
            }
        }

        fn valid_file_path(&self) -> String {
            let path = self.temp_dir.path().join("valid.txt");
            std::fs::write(&path, "test content").unwrap();
            path.to_str().unwrap().to_string()
        }
    }

    #[async_trait::async_trait]
    impl dashflow_standard_tests::tool_comprehensive_tests::ToolComprehensiveTests
        for ReadFileToolComprehensiveTests
    {
        fn tool(&self) -> &dyn Tool {
            &self.tool
        }

        fn valid_input(&self) -> serde_json::Value {
            json!({"file_path": self.valid_file_path()})
        }
    }

    #[tokio::test]
    async fn test_read_file_comprehensive_missing_required_field() {
        let tests = ReadFileToolComprehensiveTests::new();
        tests.test_error_missing_required_field().await.unwrap();
    }

    #[tokio::test]
    async fn test_read_file_comprehensive_invalid_field_type() {
        let tests = ReadFileToolComprehensiveTests::new();
        tests.test_error_invalid_field_type().await.unwrap();
    }

    #[tokio::test]
    async fn test_read_file_comprehensive_empty_string() {
        let tests = ReadFileToolComprehensiveTests::new();
        tests.test_edge_case_empty_string().await.unwrap();
    }

    #[tokio::test]
    async fn test_read_file_comprehensive_unicode_and_special_chars() {
        let tests = ReadFileToolComprehensiveTests::new();
        tests
            .test_edge_case_unicode_and_special_chars()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_read_file_comprehensive_repeated_calls() {
        let tests = ReadFileToolComprehensiveTests::new();
        tests.test_robustness_repeated_calls().await.unwrap();
    }

    // File-specific comprehensive tests

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let tool = ReadFileTool::new();
        let input = json!({"file_path": "/nonexistent/path/file.txt"});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_directory_as_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReadFileTool::new();
        let input = json!({"file_path": temp_dir.path().to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;
        // Should error (can't read directory as file)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_file_with_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("unicode.txt");
        let unicode_content = "Hello 世界 🌍 مرحبا שלום";
        fs::write(&file_path, unicode_content).await.unwrap();

        let tool = ReadFileTool::new();
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, unicode_content);
    }

    #[tokio::test]
    async fn test_write_file_path_traversal_attempt() {
        let temp_dir = TempDir::new().unwrap();
        let tool = WriteFileTool::new();

        // Try to write outside temp dir using path traversal
        let malicious_path = temp_dir.path().join("..").join("..").join("malicious.txt");
        let input = json!({
            "file_path": malicious_path.to_str().unwrap(),
            "text": "malicious"
        });

        // Tool may succeed or fail depending on OS permissions, but shouldn't panic
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_write_very_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");

        let tool = WriteFileTool::new();
        // Write 10MB of data
        let large_text = "A".repeat(10 * 1024 * 1024);
        let input = json!({
            "file_path": file_path.to_str().unwrap(),
            "text": large_text
        });

        // Should complete within reasonable time
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tool._call(ToolInput::Structured(input)),
        )
        .await;

        assert!(
            result.is_ok(),
            "Writing large file should complete within 10 seconds"
        );
    }

    #[tokio::test]
    async fn test_list_directory_empty() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ListDirectoryTool::new();
        let input = json!({"dir_path": temp_dir.path().to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        // Should return message about no entries or empty list
        assert!(result.contains("0 entries") || result.contains("empty") || result.is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReadFileTool::new();

        // Create 5 test files
        let mut paths = vec![];
        for i in 0..5 {
            let path = temp_dir.path().join(format!("file{}.txt", i));
            fs::write(&path, format!("content {}", i)).await.unwrap();
            paths.push(path);
        }

        // Read all files concurrently
        let mut handles = vec![];
        for (i, path) in paths.iter().enumerate() {
            let tool_clone = tool.clone();
            let path_str = path.to_str().unwrap().to_string();
            let handle = tokio::spawn(async move {
                let input = json!({"file_path": path_str});
                tool_clone._call(ToolInput::Structured(input)).await
            });
            handles.push((i, handle));
        }

        // All should succeed
        for (i, handle) in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "File {} read should succeed", i);
        }
    }

    // ========================================================================
    // Security Tests (M-343: File tool security tests)
    // ========================================================================

    /// Test path traversal blocking with allowed_dirs
    #[tokio::test]
    async fn test_security_path_traversal_blocked_read() {
        let allowed_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create a file in target_dir (outside allowed_dir)
        let target_file = target_dir.path().join("secret.txt");
        fs::write(&target_file, "SECRET DATA").await.unwrap();

        // Create tool restricted to allowed_dir only
        let tool = ReadFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to read secret file using path traversal
        let traversal_path = allowed_dir
            .path()
            .join("..")
            .join(target_dir.path().file_name().unwrap())
            .join("secret.txt");
        let input = json!({"file_path": traversal_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Path traversal should be blocked");
        assert!(
            result.unwrap_err().to_string().contains("Access denied"),
            "Error should indicate access denial"
        );
    }

    /// Test path traversal blocking with allowed_dirs - write
    #[tokio::test]
    async fn test_security_path_traversal_blocked_write() {
        let allowed_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create tool restricted to allowed_dir only
        let tool = WriteFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to write outside allowed_dir using path traversal
        let traversal_path = allowed_dir
            .path()
            .join("..")
            .join(target_dir.path().file_name().unwrap())
            .join("malicious.txt");
        let input = json!({
            "file_path": traversal_path.to_str().unwrap(),
            "text": "MALICIOUS CONTENT"
        });
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Path traversal should be blocked");
        assert!(
            result.unwrap_err().to_string().contains("Access denied"),
            "Error should indicate access denial"
        );

        // Verify file was NOT created
        let check_path = target_dir.path().join("malicious.txt");
        assert!(
            !check_path.exists(),
            "Malicious file should not have been created"
        );
    }

    /// Test null byte injection is blocked
    #[tokio::test]
    async fn test_security_null_byte_injection_blocked() {
        let allowed_dir = TempDir::new().unwrap();

        let tool = ReadFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to use null byte to potentially truncate path
        let malicious_path = format!("{}/safe.txt\x00../../../etc/passwd", allowed_dir.path().display());
        let input = json!({"file_path": malicious_path});
        let result = tool._call(ToolInput::Structured(input)).await;

        // Should fail due to null byte detection or file not found
        assert!(
            result.is_err(),
            "Null byte injection should be blocked or fail"
        );
    }

    /// Test double slash injection is blocked
    #[tokio::test]
    async fn test_security_double_slash_blocked() {
        let allowed_dir = TempDir::new().unwrap();

        let tool = ReadFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to use double slashes to confuse path parsing
        let malicious_path = format!("{}//../../etc/passwd", allowed_dir.path().display());
        let input = json!({"file_path": malicious_path});
        let result = tool._call(ToolInput::Structured(input)).await;

        // Should fail due to double slash detection or file not found
        assert!(
            result.is_err(),
            "Double slash injection should be blocked or fail"
        );
    }

    /// Test symlink escape is blocked when following to outside allowed_dirs
    #[cfg(unix)]
    #[tokio::test]
    async fn test_security_symlink_escape_blocked() {
        use std::os::unix::fs::symlink;

        let allowed_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create a secret file outside allowed_dir
        let secret_file = target_dir.path().join("secret.txt");
        fs::write(&secret_file, "SECRET DATA").await.unwrap();

        // Create symlink inside allowed_dir pointing to secret file
        let symlink_path = allowed_dir.path().join("link_to_secret.txt");
        symlink(&secret_file, &symlink_path).unwrap();

        let tool = ReadFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to read through symlink
        let input = json!({"file_path": symlink_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;

        // After canonicalization, the real path is outside allowed_dirs
        // This should be blocked
        assert!(
            result.is_err(),
            "Symlink escape to outside allowed_dirs should be blocked"
        );
        assert!(
            result.unwrap_err().to_string().contains("Access denied"),
            "Error should indicate access denial"
        );
    }

    /// Test symlink within allowed_dirs is permitted
    #[cfg(unix)]
    #[tokio::test]
    async fn test_security_symlink_within_allowed_dirs_permitted() {
        use std::os::unix::fs::symlink;

        let allowed_dir = TempDir::new().unwrap();

        // Create a real file in allowed_dir
        let real_file = allowed_dir.path().join("real.txt");
        fs::write(&real_file, "REAL CONTENT").await.unwrap();

        // Create symlink in allowed_dir pointing to another file in allowed_dir
        let symlink_path = allowed_dir.path().join("link.txt");
        symlink(&real_file, &symlink_path).unwrap();

        let tool = ReadFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Should succeed - symlink target is within allowed_dirs
        let input = json!({"file_path": symlink_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_ok(), "Symlink within allowed_dirs should work");
        assert_eq!(result.unwrap(), "REAL CONTENT");
    }

    /// Test multiple allowed_dirs work correctly
    #[tokio::test]
    async fn test_security_multiple_allowed_dirs() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        let dir3 = TempDir::new().unwrap();

        // Create files in dir1 and dir2
        let file1 = dir1.path().join("file1.txt");
        let file2 = dir2.path().join("file2.txt");
        let file3 = dir3.path().join("file3.txt");
        fs::write(&file1, "content1").await.unwrap();
        fs::write(&file2, "content2").await.unwrap();
        fs::write(&file3, "content3").await.unwrap();

        // Allow dir1 and dir2, but NOT dir3
        let tool = ReadFileTool::new().with_allowed_dirs(vec![
            dir1.path().to_path_buf(),
            dir2.path().to_path_buf(),
        ]);

        // Reading from dir1 should work
        let input1 = json!({"file_path": file1.to_str().unwrap()});
        assert!(tool._call(ToolInput::Structured(input1)).await.is_ok());

        // Reading from dir2 should work
        let input2 = json!({"file_path": file2.to_str().unwrap()});
        assert!(tool._call(ToolInput::Structured(input2)).await.is_ok());

        // Reading from dir3 should fail
        let input3 = json!({"file_path": file3.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input3)).await;
        assert!(result.is_err(), "Reading from non-allowed dir should fail");
    }

    /// Test empty allowed_dirs means no restrictions
    #[tokio::test]
    async fn test_security_empty_allowed_dirs_permits_all() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "content").await.unwrap();

        // Empty allowed_dirs = no restrictions
        let tool = ReadFileTool::new().with_allowed_dirs(vec![]);
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_ok(), "Empty allowed_dirs should permit all access");
    }

    /// Test relative path traversal patterns
    #[tokio::test]
    async fn test_security_relative_path_traversal_patterns() {
        let allowed_dir = TempDir::new().unwrap();
        let tool = ReadFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Various path traversal patterns that should all be blocked
        let traversal_patterns = vec![
            "../etc/passwd",
            "..\\etc\\passwd",
            "./../../etc/passwd",
            "foo/../../../etc/passwd",
            "foo/bar/../../../etc/passwd",
        ];

        for pattern in traversal_patterns {
            let full_path = allowed_dir.path().join(pattern);
            let input = json!({"file_path": full_path.to_str().unwrap()});
            let result = tool._call(ToolInput::Structured(input)).await;
            assert!(
                result.is_err(),
                "Pattern '{}' should be blocked",
                pattern
            );
        }
    }

    /// Test delete tool respects allowed_dirs
    #[tokio::test]
    async fn test_security_delete_respects_allowed_dirs() {
        let allowed_dir = TempDir::new().unwrap();
        let protected_dir = TempDir::new().unwrap();

        // Create file in protected directory
        let protected_file = protected_dir.path().join("important.txt");
        fs::write(&protected_file, "IMPORTANT").await.unwrap();

        // Tool only allowed to delete in allowed_dir
        let tool = DeleteFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to delete protected file
        let input = json!({"file_path": protected_file.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Delete outside allowed_dirs should fail");
        assert!(
            protected_file.exists(),
            "Protected file should not be deleted"
        );
    }

    /// Test copy tool respects allowed_dirs for both source and destination
    #[tokio::test]
    async fn test_security_copy_respects_allowed_dirs_both_paths() {
        let allowed_dir = TempDir::new().unwrap();
        let protected_dir = TempDir::new().unwrap();

        // Create source file in allowed_dir
        let source = allowed_dir.path().join("source.txt");
        fs::write(&source, "content").await.unwrap();

        // Create secret file in protected_dir
        let secret = protected_dir.path().join("secret.txt");
        fs::write(&secret, "SECRET").await.unwrap();

        let tool = CopyFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to copy FROM protected dir - should fail
        let input1 = json!({
            "source_path": secret.to_str().unwrap(),
            "destination_path": allowed_dir.path().join("stolen.txt").to_str().unwrap()
        });
        let result1 = tool._call(ToolInput::Structured(input1)).await;
        assert!(result1.is_err(), "Copy from protected dir should fail");

        // Try to copy TO protected dir - should fail
        let input2 = json!({
            "source_path": source.to_str().unwrap(),
            "destination_path": protected_dir.path().join("planted.txt").to_str().unwrap()
        });
        let result2 = tool._call(ToolInput::Structured(input2)).await;
        assert!(result2.is_err(), "Copy to protected dir should fail");
    }

    /// Test move tool respects allowed_dirs for both source and destination
    #[tokio::test]
    async fn test_security_move_respects_allowed_dirs_both_paths() {
        let allowed_dir = TempDir::new().unwrap();
        let protected_dir = TempDir::new().unwrap();

        // Create source file in allowed_dir
        let source = allowed_dir.path().join("source.txt");
        fs::write(&source, "content").await.unwrap();

        // Create secret file in protected_dir
        let secret = protected_dir.path().join("secret.txt");
        fs::write(&secret, "SECRET").await.unwrap();

        let tool = MoveFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to move FROM protected dir - should fail
        let input1 = json!({
            "source_path": secret.to_str().unwrap(),
            "destination_path": allowed_dir.path().join("stolen.txt").to_str().unwrap()
        });
        let result1 = tool._call(ToolInput::Structured(input1)).await;
        assert!(result1.is_err(), "Move from protected dir should fail");
        assert!(secret.exists(), "Secret file should not be moved");

        // Try to move TO protected dir - should fail
        let input2 = json!({
            "source_path": source.to_str().unwrap(),
            "destination_path": protected_dir.path().join("planted.txt").to_str().unwrap()
        });
        let result2 = tool._call(ToolInput::Structured(input2)).await;
        assert!(result2.is_err(), "Move to protected dir should fail");
    }

    /// Test file search respects allowed_dirs
    #[tokio::test]
    async fn test_security_file_search_respects_allowed_dirs() {
        let allowed_dir = TempDir::new().unwrap();
        let protected_dir = TempDir::new().unwrap();

        // Create files in protected directory
        fs::write(protected_dir.path().join("secret.txt"), "SECRET")
            .await
            .unwrap();

        let tool = FileSearchTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to search protected directory
        let input = json!({
            "dir_path": protected_dir.path().to_str().unwrap(),
            "pattern": "*.txt"
        });
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Search in protected dir should fail");
    }

    /// Test list directory respects allowed_dirs
    #[tokio::test]
    async fn test_security_list_directory_respects_allowed_dirs() {
        let allowed_dir = TempDir::new().unwrap();
        let protected_dir = TempDir::new().unwrap();

        // Create files in protected directory
        fs::write(protected_dir.path().join("secret.txt"), "SECRET")
            .await
            .unwrap();

        let tool = ListDirectoryTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Try to list protected directory
        let input = json!({"dir_path": protected_dir.path().to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "List of protected dir should fail");
    }

    /// Test writing to nested path in allowed_dir works
    #[tokio::test]
    async fn test_security_write_nested_path_in_allowed_dir() {
        let allowed_dir = TempDir::new().unwrap();

        let tool = WriteFileTool::new().with_allowed_dirs(vec![allowed_dir.path().to_path_buf()]);

        // Write to nested path that doesn't exist yet
        let nested_path = allowed_dir.path().join("a").join("b").join("c").join("file.txt");
        let input = json!({
            "file_path": nested_path.to_str().unwrap(),
            "text": "nested content"
        });
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_ok(), "Writing to nested path in allowed_dir should work");
        assert!(nested_path.exists(), "Nested file should exist");
    }

    // ========================================================================
    // Unit Tests for Security Helper Functions
    // ========================================================================

    #[test]
    fn test_contains_path_traversal_detection() {
        // Should detect path traversal
        assert!(contains_path_traversal(Path::new("../etc/passwd")));
        assert!(contains_path_traversal(Path::new("foo/../bar")));
        assert!(contains_path_traversal(Path::new("foo/bar/..")));
        assert!(contains_path_traversal(Path::new("..\\windows\\system32")));

        // Should detect null bytes
        assert!(contains_path_traversal(Path::new("foo\x00bar")));

        // Should detect double slashes (potential parser confusion)
        assert!(contains_path_traversal(Path::new("foo//bar")));
        assert!(contains_path_traversal(Path::new("/foo//bar")));

        // Should NOT flag these as path traversal
        assert!(!contains_path_traversal(Path::new("foo..bar/baz")));
        assert!(!contains_path_traversal(Path::new("dir/report..backup.json")));
        assert!(!contains_path_traversal(Path::new("/foo/bar")));
        assert!(!contains_path_traversal(Path::new("./foo/bar")));
        assert!(!contains_path_traversal(Path::new("foo/bar")));
        assert!(!contains_path_traversal(Path::new("/absolute/path/file.txt")));
    }

    #[test]
    fn test_is_path_allowed_empty_allowlist() {
        // Empty allowlist = no restrictions
        let empty: Vec<PathBuf> = vec![];
        assert!(is_path_allowed(Path::new("/any/path"), &empty));
        assert!(is_path_allowed(Path::new("relative/path"), &empty));
    }

    // ========================================================================
    // Tool Trait Method Tests
    // ========================================================================

    #[test]
    fn test_read_file_tool_trait_methods() {
        let tool = ReadFileTool::new();
        assert_eq!(tool.name(), "read_file");
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("Read"));

        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["file_path"].is_object());
        assert_eq!(schema["properties"]["file_path"]["type"], "string");
        assert!(schema["required"].as_array().unwrap().contains(&json!("file_path")));
    }

    #[test]
    fn test_write_file_tool_trait_methods() {
        let tool = WriteFileTool::new();
        assert_eq!(tool.name(), "write_file");
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("Write") || tool.description().contains("write"));

        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["file_path"].is_object());
        assert!(schema["properties"]["text"].is_object());
        assert!(schema["properties"]["append"].is_object());
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("file_path")));
        assert!(required.contains(&json!("text")));
    }

    #[test]
    fn test_list_directory_tool_trait_methods() {
        let tool = ListDirectoryTool::new();
        assert_eq!(tool.name(), "list_directory");
        assert!(!tool.description().is_empty());

        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["dir_path"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("dir_path")));
    }

    #[test]
    fn test_copy_file_tool_trait_methods() {
        let tool = CopyFileTool::new();
        assert_eq!(tool.name(), "copy_file");
        assert!(!tool.description().is_empty());

        let schema = tool.args_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("source_path")));
        assert!(required.contains(&json!("destination_path")));
    }

    #[test]
    fn test_move_file_tool_trait_methods() {
        let tool = MoveFileTool::new();
        assert_eq!(tool.name(), "move_file");
        assert!(!tool.description().is_empty());

        let schema = tool.args_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("source_path")));
        assert!(required.contains(&json!("destination_path")));
    }

    #[test]
    fn test_delete_file_tool_trait_methods() {
        let tool = DeleteFileTool::new();
        assert_eq!(tool.name(), "delete_file");
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("Delete") || tool.description().contains("delete"));

        let schema = tool.args_schema();
        assert!(schema["required"].as_array().unwrap().contains(&json!("file_path")));
    }

    #[test]
    fn test_file_search_tool_trait_methods() {
        let tool = FileSearchTool::new();
        assert_eq!(tool.name(), "file_search");
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("Search") || tool.description().contains("search"));

        let schema = tool.args_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("dir_path")));
        assert!(required.contains(&json!("pattern")));
    }

    // ========================================================================
    // FileSearchTool Pattern Matching Tests
    // ========================================================================

    #[test]
    fn test_matches_pattern_basic() {
        assert!(FileSearchTool::matches_pattern("file.txt", "*.txt"));
        assert!(FileSearchTool::matches_pattern("test.rs", "*.rs"));
        assert!(FileSearchTool::matches_pattern("a.b.c", "*.c"));
        assert!(!FileSearchTool::matches_pattern("file.txt", "*.rs"));
    }

    #[test]
    fn test_matches_pattern_question_mark() {
        assert!(FileSearchTool::matches_pattern("file1.txt", "file?.txt"));
        assert!(FileSearchTool::matches_pattern("fileA.txt", "file?.txt"));
        assert!(!FileSearchTool::matches_pattern("file12.txt", "file?.txt"));
        assert!(!FileSearchTool::matches_pattern("file.txt", "file?.txt"));
    }

    #[test]
    fn test_matches_pattern_multiple_wildcards() {
        assert!(FileSearchTool::matches_pattern("test_file.txt", "*_*.txt"));
        assert!(FileSearchTool::matches_pattern("a_b_c.txt", "*_*.txt"));
        assert!(!FileSearchTool::matches_pattern("testfile.txt", "*_*.txt"));
    }

    #[test]
    fn test_matches_pattern_exact_match() {
        assert!(FileSearchTool::matches_pattern("exact.txt", "exact.txt"));
        assert!(!FileSearchTool::matches_pattern("exact.txt", "exact.rs"));
        assert!(!FileSearchTool::matches_pattern("exactx.txt", "exact.txt"));
    }

    #[test]
    fn test_matches_pattern_star_only() {
        assert!(FileSearchTool::matches_pattern("anything", "*"));
        assert!(FileSearchTool::matches_pattern("", "*"));
        assert!(FileSearchTool::matches_pattern("file.with.dots.txt", "*"));
    }

    #[test]
    fn test_matches_pattern_prefix() {
        assert!(FileSearchTool::matches_pattern("test_something.rs", "test_*"));
        assert!(FileSearchTool::matches_pattern("test_", "test_*"));
        assert!(!FileSearchTool::matches_pattern("testing.rs", "test_*"));
    }

    #[test]
    fn test_matches_pattern_suffix() {
        assert!(FileSearchTool::matches_pattern("file_test", "*_test"));
        assert!(FileSearchTool::matches_pattern("_test", "*_test"));
        assert!(!FileSearchTool::matches_pattern("file_testing", "*_test"));
    }

    #[test]
    fn test_matches_pattern_dots_escaped() {
        // The '.' in pattern should be literal, not regex wildcard
        assert!(FileSearchTool::matches_pattern("file.txt", "file.txt"));
        assert!(!FileSearchTool::matches_pattern("fileXtxt", "file.txt"));
    }

    #[test]
    fn test_matches_pattern_complex() {
        assert!(FileSearchTool::matches_pattern("mod_test_v2.rs", "mod_*_v?.rs"));
        assert!(FileSearchTool::matches_pattern("mod_something_v1.rs", "mod_*_v?.rs"));
        assert!(!FileSearchTool::matches_pattern("mod_test_v12.rs", "mod_*_v?.rs"));
    }

    #[test]
    fn test_matches_pattern_empty_filename() {
        assert!(FileSearchTool::matches_pattern("", "*"));
        assert!(!FileSearchTool::matches_pattern("", "?"));
        assert!(!FileSearchTool::matches_pattern("", "a*"));
    }

    // ========================================================================
    // String Input Tests
    // ========================================================================

    #[tokio::test]
    async fn test_read_file_string_input() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "string input test").await.unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            ._call(ToolInput::String(file_path.to_str().unwrap().to_string()))
            .await
            .unwrap();
        assert_eq!(result, "string input test");
    }

    #[tokio::test]
    async fn test_list_directory_string_input() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "").await.unwrap();

        let tool = ListDirectoryTool::new();
        let result = tool
            ._call(ToolInput::String(temp_dir.path().to_str().unwrap().to_string()))
            .await
            .unwrap();
        assert!(result.contains("file.txt"));
    }

    #[tokio::test]
    async fn test_delete_file_string_input() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("to_delete.txt");
        fs::write(&file_path, "delete me").await.unwrap();

        let tool = DeleteFileTool::new();
        let result = tool
            ._call(ToolInput::String(file_path.to_str().unwrap().to_string()))
            .await
            .unwrap();
        assert!(result.contains("Successfully deleted"));
        assert!(!file_path.exists());
    }

    // ========================================================================
    // Input Validation Error Tests
    // ========================================================================

    #[tokio::test]
    async fn test_write_file_requires_structured_input() {
        let tool = WriteFileTool::new();
        let result = tool._call(ToolInput::String("some/path".to_string())).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("structured input"));
    }

    #[tokio::test]
    async fn test_copy_file_requires_structured_input() {
        let tool = CopyFileTool::new();
        let result = tool._call(ToolInput::String("some/path".to_string())).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("structured input"));
    }

    #[tokio::test]
    async fn test_move_file_requires_structured_input() {
        let tool = MoveFileTool::new();
        let result = tool._call(ToolInput::String("some/path".to_string())).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("structured input"));
    }

    #[tokio::test]
    async fn test_file_search_requires_structured_input() {
        let tool = FileSearchTool::new();
        let result = tool._call(ToolInput::String("some/path".to_string())).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("structured input"));
    }

    #[tokio::test]
    async fn test_read_file_missing_field() {
        let tool = ReadFileTool::new();
        let input = json!({});  // Missing file_path
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("file_path"));
    }

    #[tokio::test]
    async fn test_write_file_missing_text_field() {
        let temp_dir = TempDir::new().unwrap();
        let tool = WriteFileTool::new();
        let input = json!({
            "file_path": temp_dir.path().join("file.txt").to_str().unwrap()
            // Missing "text" field
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("text"));
    }

    #[tokio::test]
    async fn test_copy_file_missing_destination() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        fs::write(&source, "content").await.unwrap();

        let tool = CopyFileTool::new();
        let input = json!({
            "source_path": source.to_str().unwrap()
            // Missing destination_path
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("destination_path"));
    }

    #[tokio::test]
    async fn test_read_file_invalid_field_type() {
        let tool = ReadFileTool::new();
        let input = json!({"file_path": 12345});  // Should be string
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[tokio::test]
    async fn test_read_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.txt");
        fs::write(&file_path, "").await.unwrap();

        let tool = ReadFileTool::new();
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_write_empty_text() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.txt");

        let tool = WriteFileTool::new();
        let input = json!({
            "file_path": file_path.to_str().unwrap(),
            "text": ""
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("0 bytes"));

        let contents = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(contents, "");
    }

    #[tokio::test]
    async fn test_read_file_with_newlines() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("multiline.txt");
        let content = "line1\nline2\nline3\n";
        fs::write(&file_path, content).await.unwrap();

        let tool = ReadFileTool::new();
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, content);
    }

    #[tokio::test]
    async fn test_special_characters_in_filename() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file with spaces & special.txt");
        fs::write(&file_path, "content").await.unwrap();

        let tool = ReadFileTool::new();
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, "content");
    }

    #[tokio::test]
    async fn test_unicode_filename() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("文件_файл_αρχείο.txt");
        fs::write(&file_path, "unicode test").await.unwrap();

        let tool = ReadFileTool::new();
        let input = json!({"file_path": file_path.to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, "unicode test");
    }

    #[tokio::test]
    async fn test_list_directory_with_many_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create 100 files
        for i in 0..100 {
            fs::write(temp_dir.path().join(format!("file_{:03}.txt", i)), "")
                .await
                .unwrap();
        }

        let tool = ListDirectoryTool::new();
        let input = json!({"dir_path": temp_dir.path().to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        // Should contain all files (sorted)
        assert!(result.contains("file_000.txt"));
        assert!(result.contains("file_099.txt"));
        assert!(result.contains("file_050.txt"));
    }

    #[tokio::test]
    async fn test_file_search_recursive() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested structure
        let sub1 = temp_dir.path().join("sub1");
        let sub2 = temp_dir.path().join("sub1").join("sub2");
        fs::create_dir_all(&sub2).await.unwrap();

        fs::write(temp_dir.path().join("root.txt"), "").await.unwrap();
        fs::write(sub1.join("level1.txt"), "").await.unwrap();
        fs::write(sub2.join("level2.txt"), "").await.unwrap();
        fs::write(sub2.join("other.log"), "").await.unwrap();

        let tool = FileSearchTool::new();
        let input = json!({
            "dir_path": temp_dir.path().to_str().unwrap(),
            "pattern": "*.txt"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("root.txt"));
        assert!(result.contains("level1.txt"));
        assert!(result.contains("level2.txt"));
        assert!(!result.contains("other.log"));
    }

    #[tokio::test]
    async fn test_file_search_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "").await.unwrap();

        let tool = FileSearchTool::new();
        let input = json!({
            "dir_path": temp_dir.path().to_str().unwrap(),
            "pattern": "*.rs"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("No files matching"));
    }

    #[tokio::test]
    async fn test_copy_creates_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("a").join("b").join("c").join("dest.txt");
        fs::write(&source, "copy content").await.unwrap();

        let tool = CopyFileTool::new();
        let input = json!({
            "source_path": source.to_str().unwrap(),
            "destination_path": dest.to_str().unwrap()
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Successfully copied"));
        assert!(dest.exists());

        let contents = fs::read_to_string(&dest).await.unwrap();
        assert_eq!(contents, "copy content");
    }

    #[tokio::test]
    async fn test_move_creates_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("x").join("y").join("z").join("dest.txt");
        fs::write(&source, "move content").await.unwrap();

        let tool = MoveFileTool::new();
        let input = json!({
            "source_path": source.to_str().unwrap(),
            "destination_path": dest.to_str().unwrap()
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Successfully moved"));
        assert!(!source.exists());
        assert!(dest.exists());

        let contents = fs::read_to_string(&dest).await.unwrap();
        assert_eq!(contents, "move content");
    }

    #[tokio::test]
    async fn test_write_overwrites_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("overwrite.txt");
        fs::write(&file_path, "original content").await.unwrap();

        let tool = WriteFileTool::new();
        let input = json!({
            "file_path": file_path.to_str().unwrap(),
            "text": "new content"
        });
        tool._call(ToolInput::Structured(input)).await.unwrap();

        let contents = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(contents, "new content");
    }

    #[tokio::test]
    async fn test_append_to_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_append.txt");

        let tool = WriteFileTool::new();
        let input = json!({
            "file_path": file_path.to_str().unwrap(),
            "text": "appended content",
            "append": true
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("appended"));

        let contents = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(contents, "appended content");
    }

    #[tokio::test]
    async fn test_multiple_appends() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("multi_append.txt");

        let tool = WriteFileTool::new();

        for i in 1..=5 {
            let input = json!({
                "file_path": file_path.to_str().unwrap(),
                "text": format!("line {}\n", i),
                "append": true
            });
            tool._call(ToolInput::Structured(input)).await.unwrap();
        }

        let contents = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(contents, "line 1\nline 2\nline 3\nline 4\nline 5\n");
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[tokio::test]
    async fn test_copy_nonexistent_source() {
        let temp_dir = TempDir::new().unwrap();
        let tool = CopyFileTool::new();
        let input = json!({
            "source_path": temp_dir.path().join("nonexistent.txt").to_str().unwrap(),
            "destination_path": temp_dir.path().join("dest.txt").to_str().unwrap()
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_move_nonexistent_source() {
        let temp_dir = TempDir::new().unwrap();
        let tool = MoveFileTool::new();
        let input = json!({
            "source_path": temp_dir.path().join("nonexistent.txt").to_str().unwrap(),
            "destination_path": temp_dir.path().join("dest.txt").to_str().unwrap()
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = DeleteFileTool::new();
        let input = json!({"file_path": temp_dir.path().join("nonexistent.txt").to_str().unwrap()});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_nonexistent_directory() {
        let tool = ListDirectoryTool::new();
        let input = json!({"dir_path": "/nonexistent/directory/path"});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_nonexistent_directory() {
        let tool = FileSearchTool::new();
        let input = json!({
            "dir_path": "/nonexistent/directory/path",
            "pattern": "*.txt"
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    // ========================================================================
    // Builder Pattern Tests
    // ========================================================================

    #[test]
    fn test_read_file_tool_builder_chain() {
        let tool = ReadFileTool::new()
            .with_allowed_dirs(vec![PathBuf::from("/tmp"), PathBuf::from("/var")])
            .with_max_size(1024 * 1024);

        assert_eq!(tool.allowed_dirs.len(), 2);
        assert_eq!(tool.max_size, 1024 * 1024);
    }

    #[test]
    fn test_write_file_tool_builder_chain() {
        let tool = WriteFileTool::new()
            .with_allowed_dirs(vec![PathBuf::from("/tmp")]);

        assert_eq!(tool.allowed_dirs.len(), 1);
    }

    #[test]
    fn test_tools_implement_clone() {
        let read = ReadFileTool::new();
        let _cloned = read.clone();

        let write = WriteFileTool::new();
        let _cloned = write.clone();

        let list = ListDirectoryTool::new();
        let _cloned = list.clone();

        let copy = CopyFileTool::new();
        let _cloned = copy.clone();

        let mv = MoveFileTool::new();
        let _cloned = mv.clone();

        let del = DeleteFileTool::new();
        let _cloned = del.clone();

        let search = FileSearchTool::new();
        let _cloned = search.clone();
    }

    #[test]
    fn test_tools_implement_debug() {
        let read = ReadFileTool::new();
        let debug = format!("{:?}", read);
        assert!(debug.contains("ReadFileTool"));

        let write = WriteFileTool::new();
        let debug = format!("{:?}", write);
        assert!(debug.contains("WriteFileTool"));
    }

    #[test]
    fn test_tools_implement_default() {
        let _ = ReadFileTool::default();
        let _ = WriteFileTool::default();
        let _ = ListDirectoryTool::default();
        let _ = CopyFileTool::default();
        let _ = MoveFileTool::default();
        let _ = DeleteFileTool::default();
        let _ = FileSearchTool::default();
    }

    // ========================================================================
    // Normalize Path Tests
    // ========================================================================

    #[test]
    fn test_normalize_path_for_check_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let normalized = normalize_path_for_check(&file_path);
        assert!(normalized.is_some());
        let normalized = normalized.unwrap();
        // Canonical path should be absolute
        assert!(normalized.is_absolute());
    }

    #[test]
    fn test_normalize_path_for_check_new_file_in_existing_dir() {
        let temp_dir = TempDir::new().unwrap();
        let new_file = temp_dir.path().join("new_file.txt");

        let normalized = normalize_path_for_check(&new_file);
        assert!(normalized.is_some());
        let normalized = normalized.unwrap();
        assert!(normalized.is_absolute());
        assert!(normalized.ends_with("new_file.txt"));
    }

    // ========================================================================
    // Path Traversal Edge Cases
    // ========================================================================

    #[test]
    fn test_contains_path_traversal_windows_style() {
        assert!(contains_path_traversal(Path::new("..\\etc\\passwd")));
        assert!(contains_path_traversal(Path::new("foo\\..\\bar")));
    }

    #[test]
    fn test_contains_path_traversal_mixed_slashes() {
        assert!(contains_path_traversal(Path::new("foo/../bar")));
        assert!(contains_path_traversal(Path::new("foo\\..\\bar")));
    }

    #[test]
    fn test_contains_path_traversal_dots_in_filename() {
        // These should NOT be flagged as traversal
        assert!(!contains_path_traversal(Path::new("file..name.txt")));
        assert!(!contains_path_traversal(Path::new("...hidden")));
        assert!(!contains_path_traversal(Path::new("test...")));
    }

    // ========================================================================
    // Async Path Allowed Tests
    // ========================================================================

    #[tokio::test]
    async fn test_is_path_allowed_async_within_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let allowed = vec![temp_dir.path().to_path_buf()];
        let result = is_path_allowed_async(file_path, allowed).await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_is_path_allowed_async_outside_allowed() {
        let allowed_dir = TempDir::new().unwrap();
        let other_dir = TempDir::new().unwrap();
        let file_path = other_dir.path().join("secret.txt");
        std::fs::write(&file_path, "secret").unwrap();

        let allowed = vec![allowed_dir.path().to_path_buf()];
        let result = is_path_allowed_async(file_path, allowed).await;
        assert!(!result);
    }

    // ========================================================================
    // Default Max Size Test
    // ========================================================================

    #[test]
    fn test_default_max_read_size() {
        assert_eq!(DEFAULT_MAX_READ_SIZE, 10 * 1024 * 1024); // 10 MB
    }

    #[test]
    fn test_read_file_default_max_size() {
        let tool = ReadFileTool::new();
        assert_eq!(tool.max_size, DEFAULT_MAX_READ_SIZE);
    }

    #[test]
    fn test_read_file_custom_max_size() {
        let tool = ReadFileTool::new().with_max_size(1024);
        assert_eq!(tool.max_size, 1024);
    }
}
