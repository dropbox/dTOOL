//! Coding tools for the agent
//!
//! Provides tools for file operations and shell execution.

use async_trait::async_trait;
use dashflow::core::error::Result;
use dashflow::core::tools::{Tool, ToolInput};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout as tokio_timeout;
use tracing::{info, warn};

/// Tool for reading file contents
#[derive(Clone)]
pub struct ReadFileTool {
    working_dir: PathBuf,
}

impl ReadFileTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.working_dir.join(p)
        }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Input should be a JSON object with 'path' field."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let args: ReadFileArgs = match input {
            ToolInput::String(s) => serde_json::from_str(&s)
                .unwrap_or(ReadFileArgs { path: s }),
            ToolInput::Structured(v) => serde_json::from_value(v)
                .map_err(|e| dashflow::core::error::Error::tool_error(format!("Invalid args: {}", e)))?,
        };

        let path = self.resolve_path(&args.path);
        info!(path = %path.display(), "Reading file");

        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let lines = content.lines().count();
                Ok(format!("File: {}\nLines: {}\n\n{}", path.display(), lines, content))
            }
            Err(e) => Ok(format!("Error reading file '{}': {}", path.display(), e)),
        }
    }
}

#[derive(Deserialize)]
struct ReadFileArgs {
    path: String,
}

/// Tool for writing file contents
#[derive(Clone)]
pub struct WriteFileTool {
    working_dir: PathBuf,
}

impl WriteFileTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.working_dir.join(p)
        }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file (creates or overwrites). Input should be a JSON object with 'path' and 'content' fields."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
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
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let args: WriteFileArgs = match input {
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| dashflow::core::error::Error::tool_error(format!("Invalid args: {}", e)))?,
            ToolInput::Structured(v) => serde_json::from_value(v)
                .map_err(|e| dashflow::core::error::Error::tool_error(format!("Invalid args: {}", e)))?,
        };

        let path = self.resolve_path(&args.path);
        info!(path = %path.display(), content_len = args.content.len(), "Writing file");

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| {
                    dashflow::core::error::Error::tool_error(format!(
                        "Failed to create directories: {}",
                        e
                    ))
                })?;
        }

        match tokio::fs::write(&path, &args.content).await {
            Ok(()) => Ok(format!("Successfully wrote {} bytes to '{}'", args.content.len(), path.display())),
            Err(e) => Ok(format!("Error writing file '{}': {}", path.display(), e)),
        }
    }
}

#[derive(Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

/// Tool for editing a specific part of a file
#[derive(Clone)]
pub struct EditFileTool {
    working_dir: PathBuf,
}

impl EditFileTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.working_dir.join(p)
        }
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing specific text. Input should be a JSON object with 'path', 'old_text', and 'new_text' fields."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_text": {
                    "type": "string",
                    "description": "Text to find and replace"
                },
                "new_text": {
                    "type": "string",
                    "description": "Text to replace with"
                }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let args: EditFileArgs = match input {
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| dashflow::core::error::Error::tool_error(format!("Invalid args: {}", e)))?,
            ToolInput::Structured(v) => serde_json::from_value(v)
                .map_err(|e| dashflow::core::error::Error::tool_error(format!("Invalid args: {}", e)))?,
        };

        let path = self.resolve_path(&args.path);
        info!(path = %path.display(), "Editing file");

        // Read current content
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => return Ok(format!("Error reading file '{}': {}", path.display(), e)),
        };

        // Check if old_text exists
        if !content.contains(&args.old_text) {
            return Ok(format!(
                "Error: Could not find the specified text in '{}'. The file may have changed.",
                path.display()
            ));
        }

        // Check for uniqueness
        let count = content.matches(&args.old_text).count();
        if count > 1 {
            warn!(path = %path.display(), count, "Multiple matches found for edit");
            return Ok(format!(
                "Error: Found {} occurrences of the text in '{}'. Please provide more context to make the match unique.",
                count, path.display()
            ));
        }

        // Perform replacement
        let new_content = content.replace(&args.old_text, &args.new_text);

        // Write back
        match tokio::fs::write(&path, &new_content).await {
            Ok(()) => Ok(format!(
                "Successfully edited '{}'. Replaced {} chars with {} chars.",
                path.display(),
                args.old_text.len(),
                args.new_text.len()
            )),
            Err(e) => Ok(format!("Error writing file '{}': {}", path.display(), e)),
        }
    }
}

#[derive(Deserialize)]
struct EditFileArgs {
    path: String,
    old_text: String,
    new_text: String,
}

/// Tool for listing files in a directory
#[derive(Clone)]
pub struct ListFilesTool {
    working_dir: PathBuf,
}

impl ListFilesTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.working_dir.join(p)
        }
    }
}

#[async_trait]
impl Tool for ListFilesTool {
    fn name(&self) -> &str {
        "list_files"
    }

    fn description(&self) -> &str {
        "List files in a directory. Input should be a JSON object with optional 'path' field (defaults to current directory)."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list (defaults to current directory)"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to list recursively (default: false)"
                }
            }
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let args: ListFilesArgs = match input {
            ToolInput::String(s) => {
                if s.is_empty() || s == "{}" {
                    ListFilesArgs { path: None, recursive: None }
                } else {
                    serde_json::from_str(&s).unwrap_or(ListFilesArgs {
                        path: Some(s),
                        recursive: None,
                    })
                }
            }
            ToolInput::Structured(v) => serde_json::from_value(v)
                .map_err(|e| dashflow::core::error::Error::tool_error(format!("Invalid args: {}", e)))?,
        };

        let path = args
            .path
            .map(|p| self.resolve_path(&p))
            .unwrap_or_else(|| self.working_dir.clone());

        let recursive = args.recursive.unwrap_or(false);

        info!(path = %path.display(), recursive, "Listing files");

        let metadata = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(format!("Error: Directory '{}' does not exist", path.display()));
            }
            Err(e) => return Ok(format!("Error accessing '{}': {}", path.display(), e)),
        };

        if !metadata.is_dir() {
            return Ok(format!("Error: '{}' is not a directory", path.display()));
        }

        let mut entries = if recursive {
            let base = path.clone();
            match tokio::task::spawn_blocking(move || {
                let mut entries = Vec::new();
                collect_files_recursive(&base, &base, &mut entries);
                entries
            })
            .await
            {
                Ok(entries) => entries,
                Err(e) => return Ok(format!("Error listing directory '{}': {}", path.display(), e)),
            }
        } else {
            let mut entries = Vec::new();
            let mut dir = match tokio::fs::read_dir(&path).await {
                Ok(d) => d,
                Err(e) => return Ok(format!("Error listing directory '{}': {}", path.display(), e)),
            };

            loop {
                let entry = match dir.next_entry().await {
                    Ok(Some(e)) => e,
                    Ok(None) => break,
                    Err(e) => return Ok(format!("Error listing directory '{}': {}", path.display(), e)),
                };

                let file_type = match entry.file_type().await {
                    Ok(t) => t,
                    Err(e) => return Ok(format!("Error listing directory '{}': {}", path.display(), e)),
                };
                let kind = if file_type.is_dir() { "dir" } else { "file" };
                entries.push(format!(
                    "[{}] {}",
                    kind,
                    entry.file_name().to_string_lossy()
                ));
            }
            entries
        };

        entries.sort();

        Ok(format!(
            "Directory: {}\nEntries: {}\n\n{}",
            path.display(),
            entries.len(),
            entries.join("\n")
        ))
    }
}

fn collect_files_recursive(base: &PathBuf, current: &PathBuf, entries: &mut Vec<String>) {
    if let Ok(dir) = std::fs::read_dir(current) {
        for entry in dir.flatten() {
            let path = entry.path();
            let relative = path.strip_prefix(base).unwrap_or(&path);
            let file_type = if path.is_dir() { "dir" } else { "file" };
            entries.push(format!("[{}] {}", file_type, relative.display()));

            if path.is_dir() {
                // Skip common non-essential directories
                let name = entry.file_name();
                let skip = matches!(
                    name.to_string_lossy().as_ref(),
                    ".git" | "node_modules" | "target" | "__pycache__" | ".venv" | "venv"
                );
                if !skip {
                    collect_files_recursive(base, &path, entries);
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct ListFilesArgs {
    path: Option<String>,
    recursive: Option<bool>,
}

/// Tool for executing shell commands
#[derive(Clone)]
pub struct ShellExecTool {
    working_dir: PathBuf,
}

impl ShellExecTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for ShellExecTool {
    fn name(&self) -> &str {
        "shell_exec"
    }

    fn description(&self) -> &str {
        "Execute a shell command. Input should be a JSON object with 'command' field. Use for running tests, builds, git commands, etc."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 60)"
                }
            },
            "required": ["command"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let args: ShellExecArgs = match input {
            ToolInput::String(s) => serde_json::from_str(&s)
                .unwrap_or(ShellExecArgs {
                    command: s,
                    timeout_secs: None,
                }),
            ToolInput::Structured(v) => serde_json::from_value(v)
                .map_err(|e| dashflow::core::error::Error::tool_error(format!("Invalid args: {}", e)))?,
        };

        let timeout_secs = args.timeout_secs.unwrap_or(60);
        info!(command = %args.command, timeout = timeout_secs, "Executing shell command");

        // Use sh -c to execute command
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(&args.command)
            .current_dir(&self.working_dir);

        match tokio_timeout(Duration::from_secs(timeout_secs), command.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let status = output.status;

                let mut result = format!("Command: {}\n", args.command);
                result.push_str(&format!("Exit code: {}\n", status.code().unwrap_or(-1)));

                if !stdout.is_empty() {
                    result.push_str(&format!("\n--- stdout ---\n{}", stdout));
                }
                if !stderr.is_empty() {
                    result.push_str(&format!("\n--- stderr ---\n{}", stderr));
                }

                // Truncate if too long
                if result.len() > 10000 {
                    result.truncate(10000);
                    result.push_str("\n... (output truncated)");
                }

                Ok(result)
            }
            Ok(Err(e)) => Ok(format!("Error executing command '{}': {}", args.command, e)),
            Err(_) => Ok(format!(
                "Command timed out after {} seconds: {}",
                timeout_secs, args.command
            )),
        }
    }
}

#[derive(Deserialize)]
struct ShellExecArgs {
    command: String,
    timeout_secs: Option<u64>,
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[tokio::test]
    async fn test_read_file_tool() {
        let tool = ReadFileTool::new(PathBuf::from("."));
        assert_eq!(tool.name(), "read_file");

        // Test reading Cargo.toml
        let result = tool
            ._call(ToolInput::String(r#"{"path": "Cargo.toml"}"#.to_string()))
            .await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("[workspace]") || content.contains("[package]"));
    }

    #[tokio::test]
    async fn test_list_files_tool() {
        let tool = ListFilesTool::new(PathBuf::from("."));
        assert_eq!(tool.name(), "list_files");

        let result = tool._call(ToolInput::String("{}".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shell_exec_tool() {
        let tool = ShellExecTool::new(PathBuf::from("."));
        assert_eq!(tool.name(), "shell_exec");

        let result = tool
            ._call(ToolInput::String(r#"{"command": "echo hello"}"#.to_string()))
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }
}
