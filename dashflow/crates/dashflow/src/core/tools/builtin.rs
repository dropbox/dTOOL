// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Built-in tools that can be used with agents.
//!
//! This module provides ready-to-use tool implementations including:
//! - File system operations (read, write, list)
//! - Shell command execution
//! - Path security helpers

use super::*;
use serde_json::json;

// ============================================================================
// Path Security Helpers (M-57)
// ============================================================================

/// Check for path traversal patterns that could escape intended directories.
///
/// This is a defense-in-depth check that blocks common path traversal attacks:
/// - `..` sequences that could escape to parent directories
/// - Null bytes that could truncate paths in some systems
/// - Double slashes that could confuse path parsers
fn contains_path_traversal(path: &std::path::Path) -> bool {
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

/// Validate a path is safe for file operations.
///
/// Returns `Ok(canonical_path)` if the path is safe, or `Err(message)` if blocked.
fn validate_safe_path(input: &str) -> std::result::Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(input);

    // First check: block obvious traversal patterns before any filesystem access
    if contains_path_traversal(path) {
        return Err(format!(
                "Path traversal detected in '{}'. Use absolute or relative paths without '..' or special characters.",
                input
            ));
    }

    // For existing paths, canonicalize to resolve any symlinks
    if path.exists() {
        path.canonicalize()
            .map_err(|e| format!("Failed to resolve path '{}': {}", input, e))
    } else {
        // For non-existent paths (new files), validate parent exists and is safe
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && parent.exists() {
                // Canonicalize the parent and append the filename
                let canonical_parent = parent
                    .canonicalize()
                    .map_err(|e| format!("Failed to resolve parent directory: {}", e))?;
                if let Some(file_name) = path.file_name() {
                    return Ok(canonical_parent.join(file_name));
                }
            }
        }
        // For completely new paths, just return as-is after the traversal check
        Ok(path.to_path_buf())
    }
}

/// Creates an echo tool that returns the input unchanged.
///
/// This is useful for testing and debugging agent workflows.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::builtin::echo_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = echo_tool();
/// let result = tool._call_str("Hello, world!".to_string()).await.unwrap();
/// assert_eq!(result, "Hello, world!");
/// # }
/// ```
#[must_use]
pub fn echo_tool() -> impl Tool {
    sync_function_tool(
        "echo",
        "Returns the input text unchanged. Useful for testing.",
        |input: String| -> std::result::Result<String, String> { Ok(input) },
    )
    .with_args_schema(json!({
        "type": "object",
        "properties": {
            "text": {
                "type": "string",
                "description": "The text to echo back"
            }
        },
        "required": ["text"]
    }))
}

/// Creates a calculator tool that evaluates mathematical expressions.
///
/// Supports basic arithmetic operations (+, -, *, /), parentheses, and common
/// mathematical functions (sin, cos, tan, sqrt, etc.).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::builtin::calculator_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = calculator_tool();
/// let result = tool._call_str("2 + 2 * 3".to_string()).await.unwrap();
/// assert_eq!(result, "8");
///
/// let result = tool._call_str("sqrt(16) + 2".to_string()).await.unwrap();
/// assert_eq!(result, "6");
/// # }
/// ```
#[must_use]
pub fn calculator_tool() -> impl Tool {
    sync_function_tool(
            "calculator",
            "Evaluates mathematical expressions. Supports +, -, *, /, parentheses, and functions like sqrt, sin, cos, etc.",
            |input: String| -> std::result::Result<String, String> {
                // fasteval2 supports basic math operations and we add custom functions
                // Custom functions: sqrt, ln
                // Built-in: abs, floor, round, ceil, sin, cos, tan, asin, acos, atan,
                //           sinh, cosh, tanh, exp, log, min, max, int, sign
                // Built-in constants: e(), pi()

                // Custom namespace with sqrt and ln functions
                struct MathNamespace;
                impl fasteval2::EvalNamespace for MathNamespace {
                    fn lookup(&mut self, name: &str, args: Vec<f64>, _keybuf: &mut String) -> Option<f64> {
                        match name {
                            "sqrt" => {
                                if args.len() == 1 {
                                    Some(args[0].sqrt())
                                } else {
                                    None
                                }
                            }
                            "ln" => {
                                if args.len() == 1 {
                                    Some(args[0].ln())
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    }
                }

                let mut ns = MathNamespace;
                match fasteval2::ez_eval(&input, &mut ns) {
                    Ok(result) => {
                        // Format the result, removing unnecessary decimal places for whole numbers
                        // Safety: Only cast to i64 if the value is finite, has no fractional part,
                        // and fits within i64 range to avoid undefined behavior.
                        let is_whole_number = result.fract() == 0.0 && result.is_finite();
                        let fits_in_i64 =
                            result >= i64::MIN as f64 && result <= i64::MAX as f64;
                        if is_whole_number && fits_in_i64 {
                            Ok(format!("{}", result as i64))
                        } else {
                            Ok(format!("{result}"))
                        }
                    }
                    Err(e) => Err(format!("Error evaluating expression: {e}")),
                }
            },
        )
        .with_args_schema(json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate (e.g., '2 + 2', 'sqrt(16)', '3 * (4 + 5)')"
                }
            },
            "required": ["expression"]
        }))
}

/// Creates an uppercase tool that converts text to uppercase.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::builtin::uppercase_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = uppercase_tool();
/// let result = tool._call_str("hello world".to_string()).await.unwrap();
/// assert_eq!(result, "HELLO WORLD");
/// # }
/// ```
#[must_use]
pub fn uppercase_tool() -> impl Tool {
    sync_function_tool(
        "uppercase",
        "Converts text to uppercase letters.",
        |input: String| -> std::result::Result<String, String> { Ok(input.to_uppercase()) },
    )
    .with_args_schema(json!({
        "type": "object",
        "properties": {
            "text": {
                "type": "string",
                "description": "The text to convert to uppercase"
            }
        },
        "required": ["text"]
    }))
}

/// Creates a lowercase tool that converts text to lowercase.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::builtin::lowercase_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = lowercase_tool();
/// let result = tool._call_str("HELLO WORLD".to_string()).await.unwrap();
/// assert_eq!(result, "hello world");
/// # }
/// ```
#[must_use]
pub fn lowercase_tool() -> impl Tool {
    sync_function_tool(
        "lowercase",
        "Converts text to lowercase letters.",
        |input: String| -> std::result::Result<String, String> { Ok(input.to_lowercase()) },
    )
    .with_args_schema(json!({
        "type": "object",
        "properties": {
            "text": {
                "type": "string",
                "description": "The text to convert to lowercase"
            }
        },
        "required": ["text"]
    }))
}

/// Creates an HTTP fetch tool that retrieves content from URLs.
///
/// Performs HTTP GET requests and returns the response body as a string.
/// Supports both HTTP and HTTPS URLs.
///
/// # Example
///
/// ```rust,ignore,no_run
/// use dashflow::core::tools::builtin::http_fetch_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = http_fetch_tool();
/// let result = tool._call_str("https://api.github.com".to_string()).await.unwrap();
/// assert!(result.contains("GitHub"));
/// # }
/// ```
#[must_use]
pub fn http_fetch_tool() -> impl Tool {
    blocking_function_tool(
        "http_fetch",
        "Fetches content from a URL via HTTP GET request. Returns the response body as text.",
        |input: String| -> std::result::Result<String, String> {
            // M-550: SSRF protection - validate URL before fetching
            crate::core::http_client::validate_url_for_ssrf(&input).map_err(|e| e.to_string())?;

            // Use blocking client wrapped in spawn_blocking
            reqwest::blocking::get(&input)
                .map_err(|e| format!("Failed to fetch URL: {e}"))
                .and_then(|resp| {
                    if resp.status().is_success() {
                        resp.text()
                            .map_err(|e| format!("Failed to read response body: {e}"))
                    } else {
                        Err(format!(
                            "HTTP request failed with status: {}",
                            resp.status()
                        ))
                    }
                })
        },
    )
    .with_args_schema(json!({
        "type": "object",
        "properties": {
            "url": {
                "type": "string",
                "description": "The URL to fetch (must start with http:// or https://)"
            }
        },
        "required": ["url"]
    }))
}

/// Creates a file read tool that reads text files from the filesystem.
///
/// Reads the entire contents of a text file and returns it as a string.
/// Use with caution as it reads the entire file into memory.
///
/// # Example
///
/// ```rust,ignore,no_run
/// use dashflow::core::tools::builtin::file_read_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = file_read_tool();
/// let result = tool._call_str("/tmp/example.txt".to_string()).await.unwrap();
/// assert!(!result.is_empty());
/// # }
/// ```
#[must_use]
pub fn file_read_tool() -> impl Tool {
    blocking_function_tool(
            "file_read",
            "Reads the contents of a text file from the filesystem. Returns the file contents as a string.",
            |input: String| -> std::result::Result<String, String> {
                // M-57: Validate path is safe before reading
                let safe_path = validate_safe_path(&input)?;
                std::fs::read_to_string(&safe_path)
                    .map_err(|e| format!("Failed to read file '{input}': {e}"))
            },
        )
        .with_args_schema(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The filesystem path to the file to read"
                }
            },
            "required": ["path"]
        }))
}

/// Creates a file write tool that writes content to the filesystem.
///
/// Writes text content to a file. Creates parent directories if they don't exist.
/// Overwrites the file if it already exists.
///
/// # Example
///
/// ```rust,ignore,no_run
/// use dashflow::core::tools::builtin::file_write_tool;
/// use dashflow::core::tools::Tool;
/// use dashflow::core::tools::ToolInput;
/// use serde_json::json;
///
/// # async fn example() {
/// let tool = file_write_tool();
/// let input = json!({"path": "/tmp/test.txt", "content": "Hello, world!"});
/// let result = tool._call(ToolInput::Structured(input)).await.unwrap();
/// # }
/// ```
#[must_use]
pub fn file_write_tool() -> impl Tool {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct WriteArgs {
        path: String,
        content: String,
    }

    blocking_structured_tool(
            "file_write",
            "Writes content to a file. Creates parent directories if needed. Overwrites existing files.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The filesystem path where to write the file"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
            |args: WriteArgs| -> std::result::Result<String, String> {
                // M-57: Validate path is safe before writing
                let safe_path = validate_safe_path(&args.path)?;

                // Create parent directories if they don't exist
                if let Some(parent) = safe_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create directories: {e}"))?;
                }

                // Write the file
                std::fs::write(&safe_path, &args.content)
                    .map_err(|e| format!("Failed to write file '{}': {}", args.path, e))?;

                Ok(format!(
                    "Successfully wrote {} bytes to '{}'",
                    args.content.len(),
                    args.path
                ))
            },
        )
}

/// Creates a directory listing tool that lists files and directories.
///
/// Lists the contents of a directory, showing files and subdirectories.
/// Returns a formatted list with file types and sizes.
///
/// # Example
///
/// ```rust,ignore,no_run
/// use dashflow::core::tools::builtin::list_directory_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = list_directory_tool();
/// let result = tool._call_str("/tmp".to_string()).await.unwrap();
/// # }
/// ```
#[must_use]
pub fn list_directory_tool() -> impl Tool {
    blocking_function_tool(
        "list_directory",
        "Lists files and directories in a given path. Returns names, types, and sizes.",
        |input: String| -> std::result::Result<String, String> {
            // M-57: Validate path is safe before listing
            let safe_path = validate_safe_path(&input)?;

            if !safe_path.exists() {
                return Err(format!("Path '{input}' does not exist"));
            }

            if !safe_path.is_dir() {
                return Err(format!("Path '{input}' is not a directory"));
            }

            let entries = std::fs::read_dir(&safe_path)
                .map_err(|e| format!("Failed to read directory '{input}': {e}"))?;

            let mut files = Vec::new();
            let mut dirs = Vec::new();

            for entry in entries {
                let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
                let metadata = entry
                    .metadata()
                    .map_err(|e| format!("Failed to read metadata: {e}"))?;

                let name = entry.file_name().to_string_lossy().to_string();

                if metadata.is_dir() {
                    dirs.push(name);
                } else {
                    let size = metadata.len();
                    files.push((name, size));
                }
            }

            // Sort for consistent output
            dirs.sort();
            files.sort_by(|a, b| a.0.cmp(&b.0));

            let mut output = format!("Contents of '{input}':\n\n");

            if !dirs.is_empty() {
                output.push_str("Directories:\n");
                for dir in &dirs {
                    output.push_str(&format!("  [DIR]  {dir}\n"));
                }
                output.push('\n');
            }

            if !files.is_empty() {
                output.push_str("Files:\n");
                for (name, size) in &files {
                    let size_str = if *size < 1024 {
                        format!("{size} B")
                    } else if *size < 1024 * 1024 {
                        format!("{:.1} KB", *size as f64 / 1024.0)
                    } else {
                        format!("{:.1} MB", *size as f64 / (1024.0 * 1024.0))
                    };
                    output.push_str(&format!("  [FILE] {name} ({size_str})\n"));
                }
            }

            if dirs.is_empty() && files.is_empty() {
                output.push_str("(empty directory)\n");
            }

            Ok(output)
        },
    )
    .with_args_schema(json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The directory path to list"
            }
        },
        "required": ["path"]
    }))
}

/// Creates a file delete tool that removes files from the filesystem.
///
/// Deletes a file from the filesystem. Use with caution as this operation
/// cannot be undone.
///
/// # Example
///
/// ```rust,ignore,no_run
/// use dashflow::core::tools::builtin::file_delete_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = file_delete_tool();
/// let result = tool._call_str("/tmp/file_to_delete.txt".to_string()).await.unwrap();
/// # }
/// ```
#[must_use]
pub fn file_delete_tool() -> impl Tool {
    blocking_function_tool(
        "file_delete",
        "Deletes a file from the filesystem. This operation cannot be undone. Use with caution.",
        |input: String| -> std::result::Result<String, String> {
            // M-57: Validate path is safe before deleting
            let safe_path = validate_safe_path(&input)?;

            if !safe_path.exists() {
                return Err(format!("File '{input}' does not exist"));
            }

            if safe_path.is_dir() {
                return Err(format!(
                    "Path '{input}' is a directory. Use a directory removal tool instead."
                ));
            }

            std::fs::remove_file(&safe_path)
                .map_err(|e| format!("Failed to delete file '{input}': {e}"))?;

            Ok(format!("Successfully deleted file '{input}'"))
        },
    )
    .with_args_schema(json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The path to the file to delete"
            }
        },
        "required": ["path"]
    }))
}

/// Creates a JSON parse tool that parses and pretty-prints JSON strings.
///
/// Takes a JSON string, parses it to validate syntax, and returns a formatted
/// version with proper indentation. Useful for making JSON more readable.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::tools::builtin::json_parse_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = json_parse_tool();
/// let result = tool._call_str(r#"{"name":"Alice","age":30}"#.to_string()).await.unwrap();
/// assert!(result.contains("Alice"));
/// assert!(result.contains("\n")); // Pretty-printed
/// # }
/// ```
#[must_use]
pub fn json_parse_tool() -> impl Tool {
    sync_function_tool(
        "json_parse",
        "Parses a JSON string and returns it in a pretty-printed format. Validates JSON syntax.",
        |input: String| -> std::result::Result<String, String> {
            serde_json::from_str::<serde_json::Value>(&input)
                .map_err(|e| format!("Invalid JSON: {e}"))
                .and_then(|value| {
                    serde_json::to_string_pretty(&value)
                        .map_err(|e| format!("Failed to format JSON: {e}"))
                })
        },
    )
    .with_args_schema(json!({
        "type": "object",
        "properties": {
            "json_string": {
                "type": "string",
                "description": "The JSON string to parse and format"
            }
        },
        "required": ["json_string"]
    }))
}

/// Creates a shell execution tool that runs shell commands.
///
/// Executes shell commands in the system shell (bash/sh on Unix, cmd on Windows).
/// **WARNING**: This tool can execute arbitrary commands and should be used with
/// extreme caution. Only use in trusted environments or with strict command validation.
///
/// # Deprecation Notice
///
/// For production use, prefer the `dashflow-shell-tool` crate's `ShellTool` which provides:
/// - Command allowlists and prefix restrictions
/// - OS-native sandboxing (macOS Seatbelt, Linux Landlock)
/// - Configurable timeouts and output limits
/// - Async execution with proper cancellation
///
/// This basic `shell_tool` is intended for quick prototyping and trusted environments only.
///
/// Safety features:
/// - Shell metacharacter blocking (`;`, `|`, `&`, backticks, `$(`, etc.)
/// - Dangerous pattern blocking (`rm -rf /`, fork bombs, etc.)
/// - Command output is captured and returned
/// - Non-zero exit codes result in errors
/// - stderr is captured and included in errors
///
/// # Security Warning
///
/// This tool provides shell access and should never be exposed in untrusted contexts.
/// For production deployments, use `dashflow-shell-tool` with:
/// - Allowlist of permitted commands
/// - Sandboxing enabled
/// - Proper timeout configuration
///
/// # Example
///
/// ```rust,ignore,no_run
/// use dashflow::core::tools::builtin::shell_tool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() {
/// let tool = shell_tool();
/// let result = tool._call_str("echo 'Hello, World!'".to_string()).await.unwrap();
/// assert!(result.contains("Hello, World!"));
/// # }
/// ```
#[must_use]
pub fn shell_tool() -> impl Tool {
    sync_function_tool(
            "shell",
            "Executes a shell command and returns the output. WARNING: Use with extreme caution as this can execute arbitrary commands. For production, use dashflow-shell-tool with sandboxing.",
            |input: String| -> std::result::Result<String, String> {
                use std::process::Command;

                // Trim whitespace and check for empty command
                let command = input.trim();
                if command.is_empty() {
                    return Err("Command cannot be empty".to_string());
                }

                // Shell metacharacters that enable command injection.
                // These are blocked to prevent chaining/substitution attacks.
                const SHELL_METACHARACTERS: &[char] = &[
                    ';',  // Command separator
                    '|',  // Pipe
                    '&',  // Background/AND
                    '`',  // Command substitution (backticks)
                    '\n', // Newline (command separator)
                    '\r', // Carriage return
                ];

                // Patterns that indicate command substitution or injection attempts
                const INJECTION_PATTERNS: &[&str] = &[
                    "$(",  // Command substitution
                    "${",  // Variable expansion (can be exploited)
                    "||",  // OR operator (command chaining)
                    "&&",  // AND operator (command chaining)
                ];

                // Check for shell metacharacters
                for &c in SHELL_METACHARACTERS {
                    if command.contains(c) {
                        let char_repr = match c {
                            '\n' => "\\n".to_string(),
                            '\r' => "\\r".to_string(),
                            _ => c.to_string(),
                        };
                        return Err(format!(
                            "Command rejected: contains shell metacharacter '{}' (potential injection)",
                            char_repr
                        ));
                    }
                }

                // Check for injection patterns
                for &pattern in INJECTION_PATTERNS {
                    if command.contains(pattern) {
                        return Err(format!(
                            "Command rejected: contains pattern '{}' (potential injection)",
                            pattern
                        ));
                    }
                }

                // Basic safety checks - reject obviously dangerous patterns
                let dangerous_patterns = [
                    "rm -rf /",
                    "mkfs",
                    "dd if=/dev",
                    "> /dev/sda",
                    ":(){ :|:& };:",  // fork bomb
                ];

                for pattern in &dangerous_patterns {
                    if command.contains(pattern) {
                        return Err(format!(
                            "Command rejected for safety: contains dangerous pattern '{pattern}'"
                        ));
                    }
                }

                // Execute command with shell
                #[cfg(unix)]
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output();

                #[cfg(windows)]
                let output = Command::new("cmd")
                    .arg("/C")
                    .arg(command)
                    .output();

                match output {
                    Ok(output) => {
                        if output.status.success() {
                            // Return stdout, or a message if empty
                            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                            if stdout.is_empty() {
                                Ok("Command executed successfully (no output)".to_string())
                            } else {
                                Ok(stdout)
                            }
                        } else {
                            // Command failed - include stderr in error
                            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                            let exit_code = output.status.code().unwrap_or(-1);

                            if stderr.is_empty() {
                                Err(format!("Command failed with exit code {exit_code}"))
                            } else {
                                Err(format!(
                                    "Command failed with exit code {exit_code}:\n{stderr}"
                                ))
                            }
                        }
                    }
                    Err(e) => Err(format!("Failed to execute command: {e}")),
                }
            },
        )
        .with_args_schema(json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute. Use with caution."
                }
            },
            "required": ["command"]
        }))
}

#[cfg(test)]
mod tests {
    use super::{
        echo_tool, file_read_tool, file_write_tool, http_fetch_tool, json_parse_tool,
        lowercase_tool, uppercase_tool,
    };
    use crate::test_prelude::*;

    #[tokio::test]
    async fn test_echo_tool() {
        let tool = echo_tool();
        assert_eq!(tool.name(), "echo");
        assert!(tool.description().contains("unchanged"));

        let result = tool._call_str("test message".to_string()).await.unwrap();
        assert_eq!(result, "test message");
    }

    #[tokio::test]
    async fn test_calculator_tool_basic() {
        let tool = calculator_tool();
        assert_eq!(tool.name(), "calculator");

        // Basic arithmetic
        let result = tool._call_str("2 + 2".to_string()).await.unwrap();
        assert_eq!(result, "4");

        let result = tool._call_str("10 - 3".to_string()).await.unwrap();
        assert_eq!(result, "7");

        let result = tool._call_str("5 * 6".to_string()).await.unwrap();
        assert_eq!(result, "30");

        let result = tool._call_str("20 / 4".to_string()).await.unwrap();
        assert_eq!(result, "5");
    }

    #[tokio::test]
    async fn test_calculator_tool_complex() {
        let tool = calculator_tool();

        // Order of operations
        let result = tool._call_str("2 + 2 * 3".to_string()).await.unwrap();
        assert_eq!(result, "8");

        // Parentheses
        let result = tool._call_str("(2 + 2) * 3".to_string()).await.unwrap();
        assert_eq!(result, "12");

        // Functions
        let result = tool._call_str("sqrt(16)".to_string()).await.unwrap();
        assert_eq!(result, "4");

        let result = tool._call_str("sqrt(16) + 2".to_string()).await.unwrap();
        assert_eq!(result, "6");
    }

    #[tokio::test]
    async fn test_calculator_tool_error() {
        let tool = calculator_tool();

        // Test invalid expression
        let result = tool._call_str("2 + * 3".to_string()).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Error evaluating"));
    }

    #[tokio::test]
    async fn test_uppercase_tool() {
        let tool = uppercase_tool();
        assert_eq!(tool.name(), "uppercase");

        let result = tool._call_str("hello world".to_string()).await.unwrap();
        assert_eq!(result, "HELLO WORLD");

        let result = tool._call_str("MiXeD CaSe".to_string()).await.unwrap();
        assert_eq!(result, "MIXED CASE");
    }

    #[tokio::test]
    async fn test_lowercase_tool() {
        let tool = lowercase_tool();
        assert_eq!(tool.name(), "lowercase");

        let result = tool._call_str("HELLO WORLD".to_string()).await.unwrap();
        assert_eq!(result, "hello world");

        let result = tool._call_str("MiXeD CaSe".to_string()).await.unwrap();
        assert_eq!(result, "mixed case");
    }

    #[tokio::test]
    async fn test_file_read_tool() {
        let tool = file_read_tool();
        assert_eq!(tool.name(), "file_read");
        assert!(tool.description().contains("filesystem"));

        // Create a temporary file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello from file!").unwrap();

        // Test successful read
        let result = tool
            ._call_str(file_path.to_string_lossy().to_string())
            .await
            .unwrap();
        assert_eq!(result, "Hello from file!");

        // Test file not found
        let result = tool
            ._call_str("/tmp/nonexistent_file_12345.txt".to_string())
            .await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to read file"));
    }

    #[tokio::test]
    async fn test_json_parse_tool() {
        let tool = json_parse_tool();
        assert_eq!(tool.name(), "json_parse");
        assert!(tool.description().contains("JSON"));

        // Test valid JSON - compact to pretty
        let result = tool
            ._call_str(r#"{"name":"Alice","age":30}"#.to_string())
            .await
            .unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("30"));
        assert!(result.contains("\n")); // Should be pretty-printed

        // Test already pretty JSON
        let pretty_json = r#"{
  "name": "Bob",
  "age": 25
}"#;
        let result = tool._call_str(pretty_json.to_string()).await.unwrap();
        assert!(result.contains("Bob"));
        assert!(result.contains("25"));

        // Test array
        let result = tool._call_str(r#"[1,2,3,4,5]"#.to_string()).await.unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("5"));

        // Test invalid JSON
        let result = tool._call_str(r#"{"invalid": json}"#.to_string()).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid JSON"));
    }

    // Note: HTTP fetch tool test is omitted as it requires network access
    // and would make tests flaky. It can be manually tested or tested in
    // integration tests with a mock server.
    #[tokio::test]
    async fn test_http_fetch_tool_metadata() {
        let tool = http_fetch_tool();
        assert_eq!(tool.name(), "http_fetch");
        assert!(tool.description().contains("HTTP"));
        assert!(tool.description().contains("URL"));

        // Verify schema is present and has expected structure
        let schema = tool.args_schema();
        assert!(schema["properties"]["url"]["type"] == "string");
    }

    #[tokio::test]
    async fn test_file_write_tool() {
        let tool = file_write_tool();
        assert_eq!(tool.name(), "file_write");
        assert!(tool.description().contains("Writes content"));

        // Create a temp directory
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test_write.txt");

        // Test writing a file
        let input = json!({
            "path": file_path.to_string_lossy().to_string(),
            "content": "Hello, file system!"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Successfully wrote"));
        assert!(result.contains("19 bytes"));

        // Verify file was created
        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, file system!");
    }

    #[tokio::test]
    async fn test_file_write_tool_creates_dirs() {
        let tool = file_write_tool();

        // Create a temp directory
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir
            .path()
            .join("subdir1")
            .join("subdir2")
            .join("test.txt");

        // Write file in nested directory that doesn't exist
        let input = json!({
            "path": file_path.to_string_lossy().to_string(),
            "content": "Nested content"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Successfully wrote"));

        // Verify file and directories were created
        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Nested content");
    }

    #[tokio::test]
    async fn test_list_directory_tool() {
        let tool = list_directory_tool();
        assert_eq!(tool.name(), "list_directory");
        assert!(tool.description().contains("Lists files"));

        // Create a temp directory with some files
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        // Test listing
        let result = tool
            ._call_str(temp_dir.path().to_string_lossy().to_string())
            .await
            .unwrap();

        assert!(result.contains("Directories:"));
        assert!(result.contains("subdir"));
        assert!(result.contains("Files:"));
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
    }

    #[tokio::test]
    async fn test_list_directory_tool_empty() {
        let tool = list_directory_tool();

        // Create an empty temp directory
        let temp_dir = tempfile::tempdir().unwrap();

        // Test listing empty directory
        let result = tool
            ._call_str(temp_dir.path().to_string_lossy().to_string())
            .await
            .unwrap();

        assert!(result.contains("empty directory"));
    }

    #[tokio::test]
    async fn test_list_directory_tool_not_exists() {
        let tool = list_directory_tool();

        // Test with non-existent path
        let result = tool._call_str("/nonexistent/path/12345".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_file_delete_tool() {
        let tool = file_delete_tool();
        assert_eq!(tool.name(), "file_delete");
        assert!(tool.description().contains("Deletes a file"));

        // Create a temp file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("to_delete.txt");
        std::fs::write(&file_path, "delete me").unwrap();
        assert!(file_path.exists());

        // Test deleting the file
        let result = tool
            ._call_str(file_path.to_string_lossy().to_string())
            .await
            .unwrap();
        assert!(result.contains("Successfully deleted"));

        // Verify file was deleted
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_file_delete_tool_not_exists() {
        let tool = file_delete_tool();

        // Test deleting non-existent file
        let result = tool
            ._call_str("/tmp/nonexistent_file_xyz_123.txt".to_string())
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_file_delete_tool_is_directory() {
        let tool = file_delete_tool();

        // Create a temp directory
        let temp_dir = tempfile::tempdir().unwrap();

        // Test trying to delete a directory
        let result = tool
            ._call_str(temp_dir.path().to_string_lossy().to_string())
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("is a directory"));
    }

    #[tokio::test]
    async fn test_shell_tool_basic() {
        let tool = shell_tool();
        assert_eq!(tool.name(), "shell");
        assert!(tool.description().contains("shell command"));
        assert!(tool.description().contains("WARNING"));

        // Test simple echo command
        #[cfg(unix)]
        let result = tool
            ._call_str("echo 'test output'".to_string())
            .await
            .unwrap();
        #[cfg(windows)]
        let result = tool._call_str("echo test output".to_string()).await.unwrap();

        assert!(result.contains("test output"));
    }

    #[tokio::test]
    async fn test_shell_tool_empty_command() {
        let tool = shell_tool();

        // Test empty command
        let result = tool._call_str("   ".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_shell_tool_dangerous_command() {
        let tool = shell_tool();

        // Test dangerous pattern rejection
        let result = tool._call_str("rm -rf /".to_string()).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("dangerous pattern"));
    }

    #[tokio::test]
    async fn test_shell_tool_failed_command() {
        let tool = shell_tool();

        // Test command that fails
        #[cfg(unix)]
        let result = tool
            ._call_str("ls /nonexistent_dir_xyz_123".to_string())
            .await;
        #[cfg(windows)]
        let result = tool
            ._call_str("dir C:\\nonexistent_dir_xyz_123".to_string())
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("failed") || err_msg.contains("exit code"));
    }

    #[tokio::test]
    async fn test_shell_tool_no_output() {
        let tool = shell_tool();

        // Test command with no output (should succeed with message)
        #[cfg(unix)]
        let result = tool._call_str("true".to_string()).await.unwrap();
        #[cfg(windows)]
        let result = tool._call_str("cd .".to_string()).await.unwrap();

        assert!(result.contains("successfully") || result.contains("output"));
    }
}

#[tokio::test]
async fn test_tool_to_definition() {
    let calculator = sync_function_tool(
        "calculator",
        "Performs basic arithmetic operations",
        |input: String| Ok(format!("Result: {}", input)),
    );

    let definition = calculator.to_definition();

    assert_eq!(definition.name, "calculator");
    assert_eq!(
        definition.description,
        "Performs basic arithmetic operations"
    );
    assert!(definition.parameters.is_object());
    assert_eq!(definition.parameters["type"], "object");
    assert!(definition.parameters["properties"].is_object());
}

#[tokio::test]
async fn test_tools_to_definitions_multiple() {
    let calculator = sync_function_tool(
        "calculator",
        "Performs arithmetic operations",
        |input: String| Ok(format!("Result: {}", input)),
    );

    let search = sync_function_tool("search", "Search the web", |input: String| {
        Ok(format!("Search results for: {}", input))
    });

    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(calculator), Arc::new(search)];
    let definitions = tools_to_definitions(&tools);

    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0].name, "calculator");
    assert_eq!(definitions[0].description, "Performs arithmetic operations");
    assert_eq!(definitions[1].name, "search");
    assert_eq!(definitions[1].description, "Search the web");
}

#[tokio::test]
async fn test_tools_to_definitions_empty() {
    let tools: Vec<Arc<dyn Tool>> = vec![];
    let definitions = tools_to_definitions(&tools);
    assert_eq!(definitions.len(), 0);
}

// ============================================================================
// M-57: Path Traversal Security Tests
// ============================================================================
// These tests verify that the file tools block path traversal attacks.
// The internal helper functions are tested indirectly through the tool interfaces.

#[tokio::test]
async fn test_file_read_tool_blocks_traversal() {
    let tool = file_read_tool();

    // Attempt path traversal - should be rejected
    let result = tool._call_str("../../../etc/passwd".to_string()).await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("traversal") || err.to_string().contains(".."),
        "Expected path traversal error, got: {}",
        err
    );
}

#[tokio::test]
async fn test_file_write_tool_blocks_traversal() {
    let tool = file_write_tool();
    let input = serde_json::json!({
        "path": "../../../tmp/malicious.txt",
        "content": "test"
    });

    // Attempt path traversal - should be rejected
    let result = tool
        ._call(crate::core::tools::ToolInput::Structured(input))
        .await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("traversal") || err.to_string().contains(".."),
        "Expected path traversal error, got: {}",
        err
    );
}

#[tokio::test]
async fn test_list_directory_tool_blocks_traversal() {
    let tool = list_directory_tool();

    // Attempt path traversal - should be rejected
    let result = tool._call_str("../../../etc".to_string()).await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("traversal") || err.to_string().contains(".."),
        "Expected path traversal error, got: {}",
        err
    );
}

/// Regression test: filenames containing ".." should NOT be blocked.
///
/// Tests that filenames like "report..backup.json" are not incorrectly flagged
/// as path traversal attempts. Added in Worker #2670.
#[tokio::test]
async fn test_file_read_allows_dotdot_in_filename() {
    use std::io::Write;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("report..backup.json");
    let mut file = std::fs::File::create(&file_path).unwrap();
    writeln!(file, r#"{{"test": "data"}}"#).unwrap();
    drop(file);

    let tool = file_read_tool();
    let result = tool._call_str(file_path.to_string_lossy().to_string()).await;

    // Should succeed - the ".." is in the filename, not a path component
    assert!(
        result.is_ok(),
        "Expected file read to succeed for filename containing '..', got: {:?}",
        result
    );
    let content = result.unwrap();
    assert!(
        content.contains("test"),
        "Expected file content, got: {}",
        content
    );
}

/// Regression test: filenames with leading dots should not be blocked.
#[tokio::test]
async fn test_file_read_allows_leading_dots_filename() {
    use std::io::Write;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("...hidden");
    let mut file = std::fs::File::create(&file_path).unwrap();
    writeln!(file, "hidden content").unwrap();
    drop(file);

    let tool = file_read_tool();
    let result = tool._call_str(file_path.to_string_lossy().to_string()).await;

    assert!(
        result.is_ok(),
        "Expected file read to succeed for '...hidden' filename, got: {:?}",
        result
    );
}
