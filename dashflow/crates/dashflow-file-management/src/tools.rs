//! File management tools for agents.
//!
//! This module provides individual tools for file operations including:
//! - Reading files
//! - Writing files (with append support)
//! - Copying files
//! - Moving files
//! - Deleting files
//! - Listing directories
//! - Searching for files with patterns

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::{format_invalid_path_error, get_validated_relative_path};

/// Tool for reading files from disk.
///
/// # Security
///
/// When using `root_dir`, file operations are restricted to paths within that directory.
/// Directory traversal attacks (e.g., "../../../etc/passwd") are prevented.
///
/// # Examples
///
/// ```no_run
/// use dashflow_file_management::tools::ReadFileTool;
/// use dashflow::core::tools::Tool;
///
/// #[tokio::main]
/// async fn main() {
///     let tool = ReadFileTool::new(Some("/tmp/sandbox".to_string()));
///     // Can only read files within /tmp/sandbox
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ReadFileTool {
    /// Optional root directory to scope file operations
    pub root_dir: Option<String>,
}

impl ReadFileTool {
    /// Create a new `ReadFileTool` with optional root directory
    #[must_use]
    pub fn new(root_dir: Option<String>) -> Self {
        Self { root_dir }
    }

    /// Get the validated path relative to `root_dir`
    fn get_relative_path(&self, file_path: &str) -> std::result::Result<PathBuf, String> {
        if let Some(ref root) = self.root_dir {
            let root_path = Path::new(root);
            get_validated_relative_path(root_path, file_path).map_err(|_e| {
                format!("Path {file_path} is outside of the allowed directory {root}")
            })
        } else {
            Ok(PathBuf::from(file_path))
        }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read file from disk"
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let file_path = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(ref v) => v
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    dashflow::core::Error::tool_error("Missing file_path parameter".to_string())
                })?
                .to_string(),
        };

        let read_path = match self.get_relative_path(&file_path) {
            Ok(path) => path,
            Err(_) => return Ok(format_invalid_path_error("file_path", &file_path)),
        };

        // Wrap blocking I/O in spawn_blocking to avoid blocking async runtime (M-634)
        let file_path_clone = file_path.clone();
        tokio::task::spawn_blocking(move || {
            if !read_path.exists() {
                return format!("Error: no such file or directory: {file_path_clone}");
            }

            match fs::read_to_string(&read_path) {
                Ok(content) => content,
                Err(e) => format!("Error: {e}"),
            }
        })
        .await
        .map_err(|e| dashflow::core::Error::tool_error(format!("Task join failed: {e}")))
    }
}

/// Tool for writing files to disk.
///
/// Supports both overwrite and append modes.
#[derive(Debug, Clone)]
pub struct WriteFileTool {
    /// Optional root directory to scope file operations
    pub root_dir: Option<String>,
}

impl WriteFileTool {
    /// Create a new `WriteFileTool` with optional root directory
    #[must_use]
    pub fn new(root_dir: Option<String>) -> Self {
        Self { root_dir }
    }

    fn get_relative_path(&self, file_path: &str) -> std::result::Result<PathBuf, String> {
        if let Some(ref root) = self.root_dir {
            let root_path = Path::new(root);
            get_validated_relative_path(root_path, file_path).map_err(|_e| {
                format!("Path {file_path} is outside of the allowed directory {root}")
            })
        } else {
            Ok(PathBuf::from(file_path))
        }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write file to disk"
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let (file_path, text, append) = match input {
            ToolInput::String(s) => {
                // Simple mode: just the text content, write to "output.txt"
                ("output.txt".to_string(), s, false)
            }
            ToolInput::Structured(ref v) => {
                let file_path = v
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        dashflow::core::Error::tool_error("Missing file_path parameter".to_string())
                    })?
                    .to_string();
                let text = v
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        dashflow::core::Error::tool_error("Missing text parameter".to_string())
                    })?
                    .to_string();
                let append = v
                    .get("append")
                    .and_then(serde_json::value::Value::as_bool)
                    .unwrap_or(false);
                (file_path, text, append)
            }
        };

        let write_path = match self.get_relative_path(&file_path) {
            Ok(path) => path,
            Err(_) => return Ok(format_invalid_path_error("file_path", &file_path)),
        };

        // Wrap blocking I/O in spawn_blocking to avoid blocking async runtime (M-634)
        let file_path_clone = file_path.clone();
        tokio::task::spawn_blocking(move || {
            // Create parent directories if needed
            if let Some(parent) = write_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return format!("Error: {e}");
                }
            }

            let result = if append {
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&write_path)
                    .and_then(|mut file| {
                        use std::io::Write;
                        file.write_all(text.as_bytes())
                    })
            } else {
                fs::write(&write_path, text)
            };

            match result {
                Ok(()) => format!("File written successfully to {file_path_clone}."),
                Err(e) => format!("Error: {e}"),
            }
        })
        .await
        .map_err(|e| dashflow::core::Error::tool_error(format!("Task join failed: {e}")))
    }
}

/// Tool for copying files.
#[derive(Debug, Clone)]
pub struct CopyFileTool {
    pub root_dir: Option<String>,
}

impl CopyFileTool {
    #[must_use]
    pub fn new(root_dir: Option<String>) -> Self {
        Self { root_dir }
    }

    fn get_relative_path(&self, file_path: &str) -> std::result::Result<PathBuf, String> {
        if let Some(ref root) = self.root_dir {
            let root_path = Path::new(root);
            get_validated_relative_path(root_path, file_path).map_err(|_e| {
                format!("Path {file_path} is outside of the allowed directory {root}")
            })
        } else {
            Ok(PathBuf::from(file_path))
        }
    }
}

#[async_trait]
impl Tool for CopyFileTool {
    fn name(&self) -> &'static str {
        "copy_file"
    }

    fn description(&self) -> &'static str {
        "Create a copy of a file in a specified location"
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let (source_path, destination_path) = match input {
            ToolInput::String(_s) => {
                return Ok("Error: copy_file requires structured input with source_path and destination_path".to_string());
            }
            ToolInput::Structured(ref v) => {
                let source = v
                    .get("source_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        dashflow::core::Error::tool_error(
                            "Missing source_path parameter".to_string(),
                        )
                    })?
                    .to_string();
                let dest = v
                    .get("destination_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        dashflow::core::Error::tool_error(
                            "Missing destination_path parameter".to_string(),
                        )
                    })?
                    .to_string();
                (source, dest)
            }
        };

        let source_path_resolved = match self.get_relative_path(&source_path) {
            Ok(path) => path,
            Err(_) => return Ok(format_invalid_path_error("source_path", &source_path)),
        };

        let dest_path_resolved = match self.get_relative_path(&destination_path) {
            Ok(path) => path,
            Err(_) => {
                return Ok(format_invalid_path_error(
                    "destination_path",
                    &destination_path,
                ))
            }
        };

        // Wrap blocking I/O in spawn_blocking to avoid blocking async runtime (M-634)
        let source_path_clone = source_path.clone();
        let dest_path_clone = destination_path.clone();
        tokio::task::spawn_blocking(move || {
            match fs::copy(&source_path_resolved, &dest_path_resolved) {
                Ok(_) => format!(
                    "File copied successfully from {source_path_clone} to {dest_path_clone}."
                ),
                Err(e) => format!("Error: {e}"),
            }
        })
        .await
        .map_err(|e| dashflow::core::Error::tool_error(format!("Task join failed: {e}")))
    }
}

/// Tool for moving/renaming files.
#[derive(Debug, Clone)]
pub struct MoveFileTool {
    pub root_dir: Option<String>,
}

impl MoveFileTool {
    #[must_use]
    pub fn new(root_dir: Option<String>) -> Self {
        Self { root_dir }
    }

    fn get_relative_path(&self, file_path: &str) -> std::result::Result<PathBuf, String> {
        if let Some(ref root) = self.root_dir {
            let root_path = Path::new(root);
            get_validated_relative_path(root_path, file_path).map_err(|_e| {
                format!("Path {file_path} is outside of the allowed directory {root}")
            })
        } else {
            Ok(PathBuf::from(file_path))
        }
    }
}

#[async_trait]
impl Tool for MoveFileTool {
    fn name(&self) -> &'static str {
        "move_file"
    }

    fn description(&self) -> &'static str {
        "Move or rename a file from one location to another"
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let (source_path, destination_path) = match input {
            ToolInput::String(_s) => {
                return Ok("Error: move_file requires structured input with source_path and destination_path".to_string());
            }
            ToolInput::Structured(ref v) => {
                let source = v
                    .get("source_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        dashflow::core::Error::tool_error(
                            "Missing source_path parameter".to_string(),
                        )
                    })?
                    .to_string();
                let dest = v
                    .get("destination_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        dashflow::core::Error::tool_error(
                            "Missing destination_path parameter".to_string(),
                        )
                    })?
                    .to_string();
                (source, dest)
            }
        };

        let source_path_resolved = match self.get_relative_path(&source_path) {
            Ok(path) => path,
            Err(_) => return Ok(format_invalid_path_error("source_path", &source_path)),
        };

        let dest_path_resolved = match self.get_relative_path(&destination_path) {
            Ok(path) => path,
            Err(_) => {
                return Ok(format_invalid_path_error(
                    "destination_path",
                    &destination_path,
                ))
            }
        };

        // Wrap blocking I/O in spawn_blocking to avoid blocking async runtime (M-634)
        let source_path_clone = source_path.clone();
        let dest_path_clone = destination_path.clone();
        tokio::task::spawn_blocking(move || {
            if !source_path_resolved.exists() {
                return format!("Error: no such file or directory {source_path_clone}");
            }

            match fs::rename(&source_path_resolved, &dest_path_resolved) {
                Ok(()) => format!(
                    "File moved successfully from {source_path_clone} to {dest_path_clone}."
                ),
                Err(e) => format!("Error: {e}"),
            }
        })
        .await
        .map_err(|e| dashflow::core::Error::tool_error(format!("Task join failed: {e}")))
    }
}

/// Tool for deleting files.
#[derive(Debug, Clone)]
pub struct DeleteFileTool {
    pub root_dir: Option<String>,
}

impl DeleteFileTool {
    #[must_use]
    pub fn new(root_dir: Option<String>) -> Self {
        Self { root_dir }
    }

    fn get_relative_path(&self, file_path: &str) -> std::result::Result<PathBuf, String> {
        if let Some(ref root) = self.root_dir {
            let root_path = Path::new(root);
            get_validated_relative_path(root_path, file_path).map_err(|_e| {
                format!("Path {file_path} is outside of the allowed directory {root}")
            })
        } else {
            Ok(PathBuf::from(file_path))
        }
    }
}

#[async_trait]
impl Tool for DeleteFileTool {
    fn name(&self) -> &'static str {
        "file_delete"
    }

    fn description(&self) -> &'static str {
        "Delete a file"
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let file_path = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(ref v) => v
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    dashflow::core::Error::tool_error("Missing file_path parameter".to_string())
                })?
                .to_string(),
        };

        let file_path_resolved = match self.get_relative_path(&file_path) {
            Ok(path) => path,
            Err(_) => return Ok(format_invalid_path_error("file_path", &file_path)),
        };

        // Wrap blocking I/O in spawn_blocking to avoid blocking async runtime (M-634)
        let file_path_clone = file_path.clone();
        tokio::task::spawn_blocking(move || {
            if !file_path_resolved.exists() {
                return format!("Error: no such file or directory: {file_path_clone}");
            }

            match fs::remove_file(&file_path_resolved) {
                Ok(()) => format!("File deleted successfully: {file_path_clone}."),
                Err(e) => format!("Error: {e}"),
            }
        })
        .await
        .map_err(|e| dashflow::core::Error::tool_error(format!("Task join failed: {e}")))
    }
}

/// Tool for listing directory contents.
#[derive(Debug, Clone)]
pub struct ListDirectoryTool {
    pub root_dir: Option<String>,
}

impl ListDirectoryTool {
    #[must_use]
    pub fn new(root_dir: Option<String>) -> Self {
        Self { root_dir }
    }

    fn get_relative_path(&self, file_path: &str) -> std::result::Result<PathBuf, String> {
        if let Some(ref root) = self.root_dir {
            let root_path = Path::new(root);
            get_validated_relative_path(root_path, file_path).map_err(|_e| {
                format!("Path {file_path} is outside of the allowed directory {root}")
            })
        } else {
            Ok(PathBuf::from(file_path))
        }
    }
}

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &'static str {
        "list_directory"
    }

    fn description(&self) -> &'static str {
        "List files and directories in a specified folder"
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let dir_path = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(ref v) => v
                .get("dir_path")
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .to_string(),
        };

        let dir_path_resolved = match self.get_relative_path(&dir_path) {
            Ok(path) => path,
            Err(_) => return Ok(format_invalid_path_error("dir_path", &dir_path)),
        };

        // Wrap blocking I/O in spawn_blocking to avoid blocking async runtime (M-634)
        let dir_path_clone = dir_path.clone();
        tokio::task::spawn_blocking(move || {
            match fs::read_dir(&dir_path_resolved) {
                Ok(entries) => {
                    let names: Vec<String> = entries
                        .filter_map(|entry| {
                            entry.ok().and_then(|e| {
                                e.file_name().to_str().map(std::string::ToString::to_string)
                            })
                        })
                        .collect();

                    if names.is_empty() {
                        format!("No files found in directory {dir_path_clone}")
                    } else {
                        names.join("\n")
                    }
                }
                Err(e) => format!("Error: {e}"),
            }
        })
        .await
        .map_err(|e| dashflow::core::Error::tool_error(format!("Task join failed: {e}")))
    }
}

/// Tool for searching files with glob patterns.
#[derive(Debug, Clone)]
pub struct FileSearchTool {
    pub root_dir: Option<String>,
}

impl FileSearchTool {
    #[must_use]
    pub fn new(root_dir: Option<String>) -> Self {
        Self { root_dir }
    }

    fn get_relative_path(&self, file_path: &str) -> std::result::Result<PathBuf, String> {
        if let Some(ref root) = self.root_dir {
            let root_path = Path::new(root);
            get_validated_relative_path(root_path, file_path).map_err(|_e| {
                format!("Path {file_path} is outside of the allowed directory {root}")
            })
        } else {
            Ok(PathBuf::from(file_path))
        }
    }

    /// Match a filename against a pattern (Unix shell style: * matches everything)
    fn matches_pattern(filename: &str, pattern: &str) -> bool {
        // Simple glob matching: * matches any sequence of characters
        let pattern_parts: Vec<&str> = pattern.split('*').collect();

        if pattern_parts.len() == 1 {
            // No wildcards, exact match
            return filename == pattern;
        }

        let mut pos = 0;
        for (i, part) in pattern_parts.iter().enumerate() {
            if i == 0 {
                // First part must match the start
                if !filename.starts_with(part) {
                    return false;
                }
                pos = part.len();
            } else if i == pattern_parts.len() - 1 {
                // Last part must match the end
                if !filename.ends_with(part) {
                    return false;
                }
            } else {
                // Middle parts must be found in order
                if let Some(idx) = filename[pos..].find(part) {
                    pos += idx + part.len();
                } else {
                    return false;
                }
            }
        }
        true
    }
}

#[async_trait]
impl Tool for FileSearchTool {
    fn name(&self) -> &'static str {
        "file_search"
    }

    fn description(&self) -> &'static str {
        "Recursively search for files in a subdirectory that match the regex pattern"
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let (dir_path, pattern) = match input {
            ToolInput::String(s) => (".".to_string(), s),
            ToolInput::Structured(ref v) => {
                let dir = v.get("dir_path").and_then(|v| v.as_str()).unwrap_or(".");
                let pat = v.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| {
                    dashflow::core::Error::tool_error("Missing pattern parameter".to_string())
                })?;
                (dir.to_string(), pat.to_string())
            }
        };

        let dir_path_resolved = match self.get_relative_path(&dir_path) {
            Ok(path) => path,
            Err(_) => return Ok(format_invalid_path_error("dir_path", &dir_path)),
        };

        // Wrap blocking I/O in spawn_blocking to avoid blocking async runtime (M-634)
        let dir_path_clone = dir_path.clone();
        let pattern_clone = pattern.clone();
        tokio::task::spawn_blocking(move || {
            let mut matches = Vec::new();

            fn visit_dirs(
                dir: &Path,
                pattern: &str,
                base_dir: &Path,
                matches: &mut Vec<String>,
            ) -> std::io::Result<()> {
                if dir.is_dir() {
                    for entry in fs::read_dir(dir)? {
                        let entry = entry?;
                        let path = entry.path();
                        if path.is_dir() {
                            visit_dirs(&path, pattern, base_dir, matches)?;
                        } else if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                            if FileSearchTool::matches_pattern(filename, pattern) {
                                if let Ok(relative) = path.strip_prefix(base_dir) {
                                    matches.push(relative.display().to_string());
                                }
                            }
                        }
                    }
                }
                Ok(())
            }

            match visit_dirs(
                &dir_path_resolved,
                &pattern_clone,
                &dir_path_resolved,
                &mut matches,
            ) {
                Ok(()) => {
                    if matches.is_empty() {
                        format!(
                            "No files found for pattern {pattern_clone} in directory {dir_path_clone}"
                        )
                    } else {
                        matches.join("\n")
                    }
                }
                Err(e) => format!("Error: {e}"),
            }
        })
        .await
        .map_err(|e| dashflow::core::Error::tool_error(format!("Task join failed: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_read_file_tool() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Hello, World!").unwrap();

        let tool = ReadFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));
        let result = tool
            ._call(ToolInput::String("test.txt".to_string()))
            .await
            .unwrap();

        assert_eq!(result, "Hello, World!");
    }

    #[tokio::test]
    async fn test_read_file_tool_missing_file() {
        let temp_dir = tempdir().unwrap();
        let tool = ReadFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));
        let result = tool
            ._call(ToolInput::String("nonexistent.txt".to_string()))
            .await
            .unwrap();

        assert!(result.contains("no such file"));
    }

    #[tokio::test]
    async fn test_read_file_tool_traversal_attack() {
        let temp_dir = tempdir().unwrap();
        let tool = ReadFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));
        let result = tool
            ._call(ToolInput::String("../../../etc/passwd".to_string()))
            .await
            .unwrap();

        assert!(result.contains("Access denied") || result.contains("outside"));
    }

    #[tokio::test]
    async fn test_write_file_tool() {
        let temp_dir = tempdir().unwrap();
        let tool = WriteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "file_path".to_string(),
            Value::String("output.txt".to_string()),
        );
        input_map.insert(
            "text".to_string(),
            Value::String("Test content".to_string()),
        );
        input_map.insert("append".to_string(), Value::Bool(false));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("successfully"));

        let content = fs::read_to_string(temp_dir.path().join("output.txt")).unwrap();
        assert_eq!(content, "Test content");
    }

    #[tokio::test]
    async fn test_write_file_tool_append() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("append.txt");
        fs::write(&test_file, "Line 1\n").unwrap();

        let tool = WriteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "file_path".to_string(),
            Value::String("append.txt".to_string()),
        );
        input_map.insert("text".to_string(), Value::String("Line 2\n".to_string()));
        input_map.insert("append".to_string(), Value::Bool(true));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("successfully"));

        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "Line 1\nLine 2\n");
    }

    #[tokio::test]
    async fn test_copy_file_tool() {
        let temp_dir = tempdir().unwrap();
        let source = temp_dir.path().join("source.txt");
        fs::write(&source, "Original").unwrap();

        let tool = CopyFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "source_path".to_string(),
            Value::String("source.txt".to_string()),
        );
        input_map.insert(
            "destination_path".to_string(),
            Value::String("dest.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("successfully"));

        let dest_content = fs::read_to_string(temp_dir.path().join("dest.txt")).unwrap();
        assert_eq!(dest_content, "Original");
    }

    #[tokio::test]
    async fn test_move_file_tool() {
        let temp_dir = tempdir().unwrap();
        let source = temp_dir.path().join("move_source.txt");
        fs::write(&source, "Move me").unwrap();

        let tool = MoveFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "source_path".to_string(),
            Value::String("move_source.txt".to_string()),
        );
        input_map.insert(
            "destination_path".to_string(),
            Value::String("moved.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("successfully"));

        assert!(!source.exists());
        let dest_content = fs::read_to_string(temp_dir.path().join("moved.txt")).unwrap();
        assert_eq!(dest_content, "Move me");
    }

    #[tokio::test]
    async fn test_delete_file_tool() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("delete_me.txt");
        fs::write(&test_file, "Delete").unwrap();

        let tool = DeleteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));
        let result = tool
            ._call(ToolInput::String("delete_me.txt".to_string()))
            .await
            .unwrap();

        assert!(result.contains("successfully"));
        assert!(!test_file.exists());
    }

    #[tokio::test]
    async fn test_list_directory_tool() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "2").unwrap();

        let tool = ListDirectoryTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));
        let result = tool._call(ToolInput::String(".".to_string())).await.unwrap();

        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
    }

    #[tokio::test]
    async fn test_list_directory_tool_empty() {
        let temp_dir = tempdir().unwrap();
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir(&empty_dir).unwrap();

        let tool = ListDirectoryTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));
        let result = tool
            ._call(ToolInput::String("empty".to_string()))
            .await
            .unwrap();

        assert!(result.contains("No files found"));
    }

    #[tokio::test]
    async fn test_file_search_tool() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("test1.txt"), "1").unwrap();
        fs::write(temp_dir.path().join("test2.txt"), "2").unwrap();
        fs::write(temp_dir.path().join("other.dat"), "3").unwrap();

        let tool = FileSearchTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert("dir_path".to_string(), Value::String(".".to_string()));
        input_map.insert("pattern".to_string(), Value::String("*.txt".to_string()));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("test1.txt"));
        assert!(result.contains("test2.txt"));
        assert!(!result.contains("other.dat"));
    }

    #[tokio::test]
    async fn test_file_search_tool_nested() {
        let temp_dir = tempdir().unwrap();
        let nested = temp_dir.path().join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("nested.txt"), "nested").unwrap();
        fs::write(temp_dir.path().join("root.txt"), "root").unwrap();

        let tool = FileSearchTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert("dir_path".to_string(), Value::String(".".to_string()));
        input_map.insert("pattern".to_string(), Value::String("*.txt".to_string()));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("root.txt"));
        assert!(result.contains("nested"));
    }

    #[test]
    fn test_matches_pattern() {
        assert!(FileSearchTool::matches_pattern("test.txt", "*.txt"));
        assert!(FileSearchTool::matches_pattern("test.txt", "test.*"));
        assert!(FileSearchTool::matches_pattern("test.txt", "test.txt"));
        assert!(FileSearchTool::matches_pattern("test.txt", "*"));
        assert!(!FileSearchTool::matches_pattern("test.dat", "*.txt"));
        assert!(FileSearchTool::matches_pattern(
            "prefix_middle_suffix.txt",
            "prefix*suffix.txt"
        ));
    }

    // ============================================
    // Tool metadata tests
    // ============================================

    #[test]
    fn test_read_file_tool_metadata() {
        let tool = ReadFileTool::new(None);
        assert_eq!(tool.name(), "read_file");
        assert_eq!(tool.description(), "Read file from disk");
    }

    #[test]
    fn test_write_file_tool_metadata() {
        let tool = WriteFileTool::new(None);
        assert_eq!(tool.name(), "write_file");
        assert_eq!(tool.description(), "Write file to disk");
    }

    #[test]
    fn test_copy_file_tool_metadata() {
        let tool = CopyFileTool::new(None);
        assert_eq!(tool.name(), "copy_file");
        assert_eq!(tool.description(), "Create a copy of a file in a specified location");
    }

    #[test]
    fn test_move_file_tool_metadata() {
        let tool = MoveFileTool::new(None);
        assert_eq!(tool.name(), "move_file");
        assert_eq!(tool.description(), "Move or rename a file from one location to another");
    }

    #[test]
    fn test_delete_file_tool_metadata() {
        let tool = DeleteFileTool::new(None);
        assert_eq!(tool.name(), "file_delete");
        assert_eq!(tool.description(), "Delete a file");
    }

    #[test]
    fn test_list_directory_tool_metadata() {
        let tool = ListDirectoryTool::new(None);
        assert_eq!(tool.name(), "list_directory");
        assert_eq!(tool.description(), "List files and directories in a specified folder");
    }

    #[test]
    fn test_file_search_tool_metadata() {
        let tool = FileSearchTool::new(None);
        assert_eq!(tool.name(), "file_search");
        assert!(tool.description().contains("Recursively search"));
    }

    // ============================================
    // ReadFileTool additional tests
    // ============================================

    #[tokio::test]
    async fn test_read_file_tool_structured_input() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("structured_test.txt");
        fs::write(&test_file, "Structured input test").unwrap();

        let tool = ReadFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "file_path".to_string(),
            Value::String("structured_test.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert_eq!(result, "Structured input test");
    }

    #[tokio::test]
    async fn test_read_file_tool_empty_file() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("empty.txt");
        fs::write(&test_file, "").unwrap();

        let tool = ReadFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));
        let result = tool
            ._call(ToolInput::String("empty.txt".to_string()))
            .await
            .unwrap();

        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_read_file_tool_unicode_content() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("unicode.txt");
        fs::write(&test_file, "日本語 中文 한국어 🎉").unwrap();

        let tool = ReadFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));
        let result = tool
            ._call(ToolInput::String("unicode.txt".to_string()))
            .await
            .unwrap();

        assert!(result.contains("日本語"));
        assert!(result.contains("🎉"));
    }

    #[tokio::test]
    async fn test_read_file_tool_no_root_dir() {
        // Test without root_dir restriction (absolute path)
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("no_root.txt");
        fs::write(&test_file, "No root test").unwrap();

        let tool = ReadFileTool::new(None);
        let result = tool
            ._call(ToolInput::String(test_file.to_str().unwrap().to_string()))
            .await
            .unwrap();

        assert_eq!(result, "No root test");
    }

    // ============================================
    // WriteFileTool additional tests
    // ============================================

    #[tokio::test]
    async fn test_write_file_tool_creates_parent_dirs() {
        let temp_dir = tempdir().unwrap();
        let tool = WriteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "file_path".to_string(),
            Value::String("nested/deep/file.txt".to_string()),
        );
        input_map.insert(
            "text".to_string(),
            Value::String("Nested content".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("successfully"));

        let content = fs::read_to_string(temp_dir.path().join("nested/deep/file.txt")).unwrap();
        assert_eq!(content, "Nested content");
    }

    #[tokio::test]
    async fn test_write_file_tool_string_input() {
        let temp_dir = tempdir().unwrap();
        let tool = WriteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        // String input should write to output.txt by default
        let result = tool
            ._call(ToolInput::String("Simple text content".to_string()))
            .await
            .unwrap();

        assert!(result.contains("successfully"));

        let content = fs::read_to_string(temp_dir.path().join("output.txt")).unwrap();
        assert_eq!(content, "Simple text content");
    }

    #[tokio::test]
    async fn test_write_file_tool_unicode_content() {
        let temp_dir = tempdir().unwrap();
        let tool = WriteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "file_path".to_string(),
            Value::String("unicode_write.txt".to_string()),
        );
        input_map.insert(
            "text".to_string(),
            Value::String("こんにちは世界 🌍".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("successfully"));

        let content = fs::read_to_string(temp_dir.path().join("unicode_write.txt")).unwrap();
        assert_eq!(content, "こんにちは世界 🌍");
    }

    #[tokio::test]
    async fn test_write_file_tool_traversal_attack() {
        let temp_dir = tempdir().unwrap();
        let tool = WriteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "file_path".to_string(),
            Value::String("../../../tmp/evil.txt".to_string()),
        );
        input_map.insert("text".to_string(), Value::String("evil".to_string()));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("Access denied"));
    }

    // ============================================
    // CopyFileTool additional tests
    // ============================================

    #[tokio::test]
    async fn test_copy_file_tool_string_input() {
        let temp_dir = tempdir().unwrap();
        let tool = CopyFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        // String input should return error message
        let result = tool
            ._call(ToolInput::String("invalid".to_string()))
            .await
            .unwrap();

        assert!(result.contains("requires structured input"));
    }

    #[tokio::test]
    async fn test_copy_file_tool_missing_source() {
        let temp_dir = tempdir().unwrap();
        let tool = CopyFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "source_path".to_string(),
            Value::String("nonexistent.txt".to_string()),
        );
        input_map.insert(
            "destination_path".to_string(),
            Value::String("dest.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("Error"));
    }

    #[tokio::test]
    async fn test_copy_file_tool_traversal_source() {
        let temp_dir = tempdir().unwrap();
        let tool = CopyFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "source_path".to_string(),
            Value::String("../../../etc/passwd".to_string()),
        );
        input_map.insert(
            "destination_path".to_string(),
            Value::String("copy.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("Access denied"));
    }

    #[tokio::test]
    async fn test_copy_file_tool_traversal_dest() {
        let temp_dir = tempdir().unwrap();
        let source = temp_dir.path().join("source.txt");
        fs::write(&source, "Content").unwrap();

        let tool = CopyFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "source_path".to_string(),
            Value::String("source.txt".to_string()),
        );
        input_map.insert(
            "destination_path".to_string(),
            Value::String("../../../tmp/evil.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("Access denied"));
    }

    // ============================================
    // MoveFileTool additional tests
    // ============================================

    #[tokio::test]
    async fn test_move_file_tool_string_input() {
        let temp_dir = tempdir().unwrap();
        let tool = MoveFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let result = tool
            ._call(ToolInput::String("invalid".to_string()))
            .await
            .unwrap();

        assert!(result.contains("requires structured input"));
    }

    #[tokio::test]
    async fn test_move_file_tool_missing_source() {
        let temp_dir = tempdir().unwrap();
        let tool = MoveFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "source_path".to_string(),
            Value::String("nonexistent.txt".to_string()),
        );
        input_map.insert(
            "destination_path".to_string(),
            Value::String("dest.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("no such file"));
    }

    #[tokio::test]
    async fn test_move_file_tool_rename_in_place() {
        let temp_dir = tempdir().unwrap();
        let source = temp_dir.path().join("original.txt");
        fs::write(&source, "Content to move").unwrap();

        let tool = MoveFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "source_path".to_string(),
            Value::String("original.txt".to_string()),
        );
        input_map.insert(
            "destination_path".to_string(),
            Value::String("renamed.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("successfully"));
        assert!(!source.exists());
        let content = fs::read_to_string(temp_dir.path().join("renamed.txt")).unwrap();
        assert_eq!(content, "Content to move");
    }

    // ============================================
    // DeleteFileTool additional tests
    // ============================================

    #[tokio::test]
    async fn test_delete_file_tool_structured_input() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("delete_struct.txt");
        fs::write(&test_file, "Delete me").unwrap();

        let tool = DeleteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert(
            "file_path".to_string(),
            Value::String("delete_struct.txt".to_string()),
        );

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("successfully"));
        assert!(!test_file.exists());
    }

    #[tokio::test]
    async fn test_delete_file_tool_missing_file() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let result = tool
            ._call(ToolInput::String("nonexistent.txt".to_string()))
            .await
            .unwrap();

        assert!(result.contains("no such file"));
    }

    #[tokio::test]
    async fn test_delete_file_tool_traversal_attack() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let result = tool
            ._call(ToolInput::String("../../../etc/passwd".to_string()))
            .await
            .unwrap();

        assert!(result.contains("Access denied"));
    }

    // ============================================
    // ListDirectoryTool additional tests
    // ============================================

    #[tokio::test]
    async fn test_list_directory_tool_structured_input() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "content").unwrap();

        let tool = ListDirectoryTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert("dir_path".to_string(), Value::String(".".to_string()));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("file.txt"));
    }

    #[tokio::test]
    async fn test_list_directory_tool_default_dir() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("default.txt"), "content").unwrap();

        let tool = ListDirectoryTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        // Structured input without dir_path should default to "."
        let input_map = serde_json::Map::new();
        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("default.txt"));
    }

    #[tokio::test]
    async fn test_list_directory_tool_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let tool = ListDirectoryTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let result = tool
            ._call(ToolInput::String("nonexistent_dir".to_string()))
            .await
            .unwrap();

        assert!(result.contains("Error"));
    }

    #[tokio::test]
    async fn test_list_directory_tool_traversal_attack() {
        let temp_dir = tempdir().unwrap();
        let tool = ListDirectoryTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let result = tool
            ._call(ToolInput::String("../../../".to_string()))
            .await
            .unwrap();

        assert!(result.contains("Access denied"));
    }

    // ============================================
    // FileSearchTool additional tests
    // ============================================

    #[tokio::test]
    async fn test_file_search_tool_string_input() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("pattern_test.txt"), "content").unwrap();

        let tool = FileSearchTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        // String input uses "." as dir_path and the string as pattern
        let result = tool
            ._call(ToolInput::String("*.txt".to_string()))
            .await
            .unwrap();

        assert!(result.contains("pattern_test.txt"));
    }

    #[tokio::test]
    async fn test_file_search_tool_no_matches() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "content").unwrap();

        let tool = FileSearchTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert("dir_path".to_string(), Value::String(".".to_string()));
        input_map.insert("pattern".to_string(), Value::String("*.xyz".to_string()));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("No files found"));
    }

    #[tokio::test]
    async fn test_file_search_tool_deeply_nested() {
        let temp_dir = tempdir().unwrap();
        let deep = temp_dir.path().join("a/b/c/d");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("deep.txt"), "deep content").unwrap();

        let tool = FileSearchTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert("dir_path".to_string(), Value::String(".".to_string()));
        input_map.insert("pattern".to_string(), Value::String("deep.txt".to_string()));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("deep.txt"));
    }

    #[tokio::test]
    async fn test_file_search_tool_traversal_attack() {
        let temp_dir = tempdir().unwrap();
        let tool = FileSearchTool::new(Some(temp_dir.path().to_str().unwrap().to_string()));

        let mut input_map = serde_json::Map::new();
        input_map.insert("dir_path".to_string(), Value::String("../../../".to_string()));
        input_map.insert("pattern".to_string(), Value::String("*".to_string()));

        let result = tool
            ._call(ToolInput::Structured(Value::Object(input_map)))
            .await
            .unwrap();

        assert!(result.contains("Access denied"));
    }

    // ============================================
    // matches_pattern edge cases
    // ============================================

    #[test]
    fn test_matches_pattern_empty_pattern() {
        assert!(FileSearchTool::matches_pattern("", ""));
        assert!(!FileSearchTool::matches_pattern("file.txt", ""));
    }

    #[test]
    fn test_matches_pattern_empty_filename() {
        assert!(FileSearchTool::matches_pattern("", ""));
        assert!(!FileSearchTool::matches_pattern("", "*.txt"));
    }

    #[test]
    fn test_matches_pattern_multiple_wildcards() {
        assert!(FileSearchTool::matches_pattern("a_b_c.txt", "a*b*c.txt"));
        assert!(FileSearchTool::matches_pattern("abc.txt", "a*b*c.txt"));
        assert!(!FileSearchTool::matches_pattern("a_x_y.txt", "a*b*c.txt"));
    }

    #[test]
    fn test_matches_pattern_wildcard_only() {
        assert!(FileSearchTool::matches_pattern("anything.txt", "*"));
        assert!(FileSearchTool::matches_pattern("", "*"));
        assert!(FileSearchTool::matches_pattern("a", "*"));
    }

    #[test]
    fn test_matches_pattern_adjacent_wildcards() {
        // ** should match anything
        assert!(FileSearchTool::matches_pattern("test.txt", "**"));
        assert!(FileSearchTool::matches_pattern("test.txt", "t**t"));
    }

    #[test]
    fn test_matches_pattern_special_chars() {
        // Test with special characters in filename
        assert!(FileSearchTool::matches_pattern("file-name_v1.2.txt", "file-name*.txt"));
        assert!(FileSearchTool::matches_pattern("file (1).txt", "file (*).txt"));
    }

    #[test]
    fn test_matches_pattern_case_sensitivity() {
        // Pattern matching should be case-sensitive
        assert!(!FileSearchTool::matches_pattern("TEST.TXT", "test.txt"));
        assert!(FileSearchTool::matches_pattern("TEST.TXT", "TEST.TXT"));
        assert!(FileSearchTool::matches_pattern("TEST.TXT", "*.TXT"));
    }

    // ============================================
    // Tool construction tests
    // ============================================

    #[test]
    fn test_tool_construction_with_root() {
        let root_dir = Some("/tmp/sandbox".to_string());

        let read_tool = ReadFileTool::new(root_dir.clone());
        assert_eq!(read_tool.root_dir, root_dir);

        let write_tool = WriteFileTool::new(root_dir.clone());
        assert_eq!(write_tool.root_dir, root_dir);

        let copy_tool = CopyFileTool::new(root_dir.clone());
        assert_eq!(copy_tool.root_dir, root_dir);

        let move_tool = MoveFileTool::new(root_dir.clone());
        assert_eq!(move_tool.root_dir, root_dir);

        let delete_tool = DeleteFileTool::new(root_dir.clone());
        assert_eq!(delete_tool.root_dir, root_dir);

        let list_tool = ListDirectoryTool::new(root_dir.clone());
        assert_eq!(list_tool.root_dir, root_dir);

        let search_tool = FileSearchTool::new(root_dir);
        assert_eq!(search_tool.root_dir, Some("/tmp/sandbox".to_string()));
    }

    #[test]
    fn test_tool_construction_without_root() {
        let read_tool = ReadFileTool::new(None);
        assert!(read_tool.root_dir.is_none());

        let write_tool = WriteFileTool::new(None);
        assert!(write_tool.root_dir.is_none());
    }

    // ============================================
    // Debug trait tests
    // ============================================

    #[test]
    fn test_tools_implement_debug() {
        let read_tool = ReadFileTool::new(Some("/tmp".to_string()));
        let debug_str = format!("{:?}", read_tool);
        assert!(debug_str.contains("ReadFileTool"));
        assert!(debug_str.contains("/tmp"));

        let write_tool = WriteFileTool::new(None);
        let debug_str = format!("{:?}", write_tool);
        assert!(debug_str.contains("WriteFileTool"));
    }

    #[test]
    fn test_tools_implement_clone() {
        let original = ReadFileTool::new(Some("/tmp".to_string()));
        let cloned = original.clone();
        assert_eq!(original.root_dir, cloned.root_dir);

        let original = WriteFileTool::new(None);
        let cloned = original.clone();
        assert_eq!(original.root_dir, cloned.root_dir);
    }
}
