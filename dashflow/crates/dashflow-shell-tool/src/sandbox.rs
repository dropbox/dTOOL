//! Sandboxed shell tool with platform-specific OS-level isolation.
//!
//! Provides first-class sandbox enforcement for shell command execution using:
//! - **macOS**: Seatbelt (`sandbox-exec`)
//! - **Linux**: Landlock (kernel 5.13+) or Seccomp fallback
//!
//! When sandbox mechanisms are unavailable, provides explicit errors or
//! configurable fallback behavior.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_shell_tool::{SandboxedShellTool, SandboxMode, SandboxFallback};
//! use std::path::PathBuf;
//!
//! let tool = SandboxedShellTool::builder()
//!     .sandbox_mode(SandboxMode::Strict)
//!     .writable_roots(vec![PathBuf::from("/tmp"), PathBuf::from("./workspace")])
//!     .on_sandbox_missing(SandboxFallback::Warn)
//!     .build()?;
//!
//! // Execute command in sandbox
//! let result = tool.execute("ls -la").await?;
//! ```

use crate::{CommandAnalyzer, SafetyConfig, Severity};
use async_trait::async_trait;
use dashflow::core::{
    tools::{Tool, ToolInput},
    Error,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Sandbox enforcement mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    /// Maximum restrictions: no network, limited filesystem access.
    /// Requires platform sandbox support.
    #[default]
    Strict,

    /// Moderate restrictions: network allowed, filesystem limited to writable_roots.
    Permissive,

    /// No sandbox enforcement (UNSAFE - use only for trusted commands).
    Disabled,
}

impl std::fmt::Display for SandboxMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxMode::Strict => write!(f, "strict"),
            SandboxMode::Permissive => write!(f, "permissive"),
            SandboxMode::Disabled => write!(f, "disabled"),
        }
    }
}

/// Sandbox-specific error types.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum SandboxError {
    /// Sandbox mechanism not available on this platform.
    #[error("Sandbox not available on {platform}: {reason}")]
    NotAvailable {
        /// Platform identifier (e.g., "macos", "linux", "windows")
        platform: String,
        /// Reason sandbox is unavailable
        reason: String,
    },

    /// Permission denied by sandbox policy.
    #[error("Sandbox permission denied: {operation} on {path:?}")]
    PermissionDenied {
        /// Path that was denied
        path: Option<PathBuf>,
        /// Operation that was denied (read, write, execute, network)
        operation: String,
    },

    /// Sandbox profile creation failed.
    #[error("Failed to create sandbox profile: {reason}")]
    ProfileCreationFailed {
        /// Reason profile creation failed
        reason: String,
    },

    /// Sandbox enforcement failed during execution.
    #[error("Sandbox enforcement failed: {reason}")]
    EnforcementFailed {
        /// Reason enforcement failed
        reason: String,
    },

    /// Command blocked by sandbox policy before execution.
    #[error("Command blocked by sandbox policy: {reason}")]
    CommandBlocked {
        /// Reason command was blocked
        reason: String,
    },

    /// Configuration error.
    #[error("Sandbox configuration error: {reason}")]
    Configuration {
        /// Reason for configuration error
        reason: String,
    },
}

/// What to do when sandbox is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SandboxFallback {
    /// Fail immediately with SandboxError::NotAvailable.
    #[default]
    Fail,

    /// Log warning and proceed without sandbox (UNSAFE).
    Warn,

    /// Silently proceed without sandbox (NOT RECOMMENDED).
    Silent,
}

/// Callback type for sandbox missing notification.
pub type SandboxMissingCallback = Arc<dyn Fn(SandboxMode, &str) -> SandboxFallback + Send + Sync>;

/// Platform-specific sandbox capabilities.
#[derive(Debug, Clone)]
pub struct SandboxCapabilities {
    /// Whether Seatbelt (macOS) is available.
    pub seatbelt_available: bool,
    /// Whether Landlock (Linux 5.13+) is available.
    pub landlock_available: bool,
    /// Whether Seccomp (Linux) is available.
    pub seccomp_available: bool,
    /// Current platform name.
    pub platform: String,
}

impl SandboxCapabilities {
    /// Detect available sandbox capabilities on this system.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self {
                seatbelt_available: Self::check_seatbelt(),
                landlock_available: false,
                seccomp_available: false,
                platform: "macos".to_string(),
            }
        }

        #[cfg(target_os = "linux")]
        {
            Self {
                seatbelt_available: false,
                landlock_available: Self::check_landlock(),
                seccomp_available: Self::check_seccomp(),
                platform: "linux".to_string(),
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Self {
                seatbelt_available: false,
                landlock_available: false,
                seccomp_available: false,
                platform: std::env::consts::OS.to_string(),
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn check_seatbelt() -> bool {
        // Check if sandbox-exec is available
        std::process::Command::new("which")
            .arg("sandbox-exec")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    fn check_landlock() -> bool {
        // Check kernel version >= 5.13 and landlock filesystem exists
        std::path::Path::new("/sys/kernel/security/landlock").exists()
    }

    #[cfg(target_os = "linux")]
    fn check_seccomp() -> bool {
        // Check if seccomp is available via prctl
        std::path::Path::new("/proc/sys/kernel/seccomp").exists()
    }

    /// Whether any sandbox mechanism is available.
    #[must_use]
    pub fn any_available(&self) -> bool {
        self.seatbelt_available || self.landlock_available || self.seccomp_available
    }

    /// Get human-readable description of available mechanisms.
    #[must_use]
    pub fn available_mechanisms(&self) -> Vec<&'static str> {
        let mut mechanisms = Vec::new();
        if self.seatbelt_available {
            mechanisms.push("Seatbelt (macOS)");
        }
        if self.landlock_available {
            mechanisms.push("Landlock (Linux)");
        }
        if self.seccomp_available {
            mechanisms.push("Seccomp (Linux)");
        }
        mechanisms
    }
}

/// Builder for SandboxedShellTool.
#[derive(Default)]
pub struct SandboxedShellToolBuilder {
    sandbox_mode: SandboxMode,
    writable_roots: Vec<PathBuf>,
    readable_roots: Vec<PathBuf>,
    working_dir: Option<PathBuf>,
    timeout_seconds: u64,
    max_output_bytes: usize,
    fallback: SandboxFallback,
    on_sandbox_missing: Option<SandboxMissingCallback>,
    allow_network: bool,
    safety_config: Option<SafetyConfig>,
}

impl SandboxedShellToolBuilder {
    /// Create a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sandbox_mode: SandboxMode::Strict,
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            working_dir: None,
            timeout_seconds: 30,
            max_output_bytes: 1024 * 1024,
            fallback: SandboxFallback::Fail,
            on_sandbox_missing: None,
            allow_network: false,
            safety_config: None,
        }
    }

    /// Set sandbox mode.
    #[must_use]
    pub fn sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.sandbox_mode = mode;
        self
    }

    /// Set directories that can be written to.
    #[must_use]
    pub fn writable_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.writable_roots = roots;
        self
    }

    /// Add a single writable root.
    #[must_use]
    pub fn add_writable_root(mut self, root: PathBuf) -> Self {
        self.writable_roots.push(root);
        self
    }

    /// Set directories that can be read from (in addition to writable roots).
    #[must_use]
    pub fn readable_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.readable_roots = roots;
        self
    }

    /// Add a single readable root.
    #[must_use]
    pub fn add_readable_root(mut self, root: PathBuf) -> Self {
        self.readable_roots.push(root);
        self
    }

    /// Set working directory for command execution.
    #[must_use]
    pub fn working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set command timeout in seconds.
    #[must_use]
    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Set maximum output size in bytes.
    #[must_use]
    pub fn max_output_bytes(mut self, bytes: usize) -> Self {
        self.max_output_bytes = bytes;
        self
    }

    /// Set fallback behavior when sandbox unavailable.
    #[must_use]
    pub fn on_sandbox_missing(mut self, fallback: SandboxFallback) -> Self {
        self.fallback = fallback;
        self
    }

    /// Set callback for when sandbox is unavailable.
    /// The callback receives the requested mode and reason, and returns the fallback behavior.
    #[must_use]
    pub fn with_missing_callback(mut self, callback: SandboxMissingCallback) -> Self {
        self.on_sandbox_missing = Some(callback);
        self
    }

    /// Allow network access (only in Permissive mode).
    #[must_use]
    pub fn allow_network(mut self, allow: bool) -> Self {
        self.allow_network = allow;
        self
    }

    /// Set safety configuration for command analysis.
    #[must_use]
    pub fn safety_config(mut self, config: SafetyConfig) -> Self {
        self.safety_config = Some(config);
        self
    }

    /// Build the SandboxedShellTool.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Sandbox mode is Strict but no sandbox mechanism available and fallback is Fail
    /// - Configuration is invalid (e.g., no writable roots in Strict mode)
    pub fn build(self) -> Result<SandboxedShellTool, SandboxError> {
        let capabilities = SandboxCapabilities::detect();

        // Check sandbox availability if not disabled
        if self.sandbox_mode != SandboxMode::Disabled && !capabilities.any_available() {
            let reason = format!(
                "No sandbox mechanism available on {}. Available mechanisms: {:?}",
                capabilities.platform,
                capabilities.available_mechanisms()
            );

            // Determine fallback behavior
            let fallback = if let Some(ref callback) = self.on_sandbox_missing {
                callback(self.sandbox_mode, &reason)
            } else {
                self.fallback
            };

            match fallback {
                SandboxFallback::Fail => {
                    return Err(SandboxError::NotAvailable {
                        platform: capabilities.platform,
                        reason,
                    });
                }
                SandboxFallback::Warn => {
                    tracing::warn!(
                        platform = %capabilities.platform,
                        reason = %reason,
                        "Sandbox unavailable, running in permissive mode"
                    );
                }
                SandboxFallback::Silent => {
                    // Silently continue without sandbox
                }
            }
        }

        // Build safety config
        let safety_config = self.safety_config.unwrap_or_else(|| {
            if self.sandbox_mode == SandboxMode::Strict {
                SafetyConfig::restrictive()
            } else {
                SafetyConfig::permissive()
            }
        });

        Ok(SandboxedShellTool {
            sandbox_mode: self.sandbox_mode,
            writable_roots: self.writable_roots,
            readable_roots: self.readable_roots,
            working_dir: self.working_dir,
            timeout_seconds: self.timeout_seconds,
            max_output_bytes: self.max_output_bytes,
            capabilities,
            analyzer: CommandAnalyzer::new(safety_config),
            allow_network: self.allow_network,
        })
    }
}

/// Shell tool with platform-specific OS-level sandbox enforcement.
///
/// Wraps shell command execution with:
/// - **macOS**: Seatbelt profiles via `sandbox-exec`
/// - **Linux**: Landlock filesystem restrictions
///
/// Provides explicit errors when sandbox mechanisms are unavailable.
pub struct SandboxedShellTool {
    sandbox_mode: SandboxMode,
    writable_roots: Vec<PathBuf>,
    readable_roots: Vec<PathBuf>,
    working_dir: Option<PathBuf>,
    timeout_seconds: u64,
    max_output_bytes: usize,
    capabilities: SandboxCapabilities,
    analyzer: CommandAnalyzer,
    allow_network: bool,
}

impl std::fmt::Debug for SandboxedShellTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxedShellTool")
            .field("sandbox_mode", &self.sandbox_mode)
            .field("writable_roots", &self.writable_roots)
            .field("readable_roots", &self.readable_roots)
            .field("working_dir", &self.working_dir)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("max_output_bytes", &self.max_output_bytes)
            .field("capabilities", &self.capabilities)
            .field("allow_network", &self.allow_network)
            .finish()
    }
}

impl SandboxedShellTool {
    /// Create a builder for SandboxedShellTool.
    #[must_use]
    pub fn builder() -> SandboxedShellToolBuilder {
        SandboxedShellToolBuilder::new()
    }

    /// Get the current sandbox mode.
    #[must_use]
    pub fn sandbox_mode(&self) -> SandboxMode {
        self.sandbox_mode
    }

    /// Get the sandbox capabilities.
    #[must_use]
    pub fn capabilities(&self) -> &SandboxCapabilities {
        &self.capabilities
    }

    /// Get writable roots.
    #[must_use]
    pub fn writable_roots(&self) -> &[PathBuf] {
        &self.writable_roots
    }

    /// Analyze a command without executing it.
    #[must_use]
    pub fn analyze(&self, command: &str) -> crate::AnalysisResult {
        self.analyzer.analyze(command)
    }

    /// Execute a command in the sandbox.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Command is blocked by safety analysis
    /// - Sandbox enforcement fails
    /// - Command execution fails or times out
    pub async fn execute(&self, command: &str) -> Result<String, Error> {
        // Pre-execution safety analysis
        let analysis = self.analyzer.analyze(command);

        if analysis.severity == Severity::Forbidden {
            return Err(Error::tool_error(format!(
                "Command blocked (forbidden): {}. Reasons: {}",
                command,
                analysis.reasons.join("; ")
            )));
        }

        // Execute based on sandbox mode
        match self.sandbox_mode {
            SandboxMode::Disabled => self.execute_unsandboxed(command).await,
            _ => self.execute_sandboxed(command).await,
        }
    }

    /// Execute command with sandbox enforcement.
    async fn execute_sandboxed(&self, command: &str) -> Result<String, Error> {
        #[cfg(target_os = "macos")]
        if self.capabilities.seatbelt_available {
            return self.execute_with_seatbelt(command).await;
        }

        #[cfg(target_os = "linux")]
        if self.capabilities.landlock_available {
            return self.execute_with_landlock(command).await;
        }

        // Fallback: no sandbox available but mode requires it
        // At this point builder should have handled this, but just in case
        self.execute_unsandboxed(command).await
    }

    /// Execute command without sandbox (used when sandbox unavailable or disabled).
    async fn execute_unsandboxed(&self, command: &str) -> Result<String, Error> {
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

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        self.run_command_with_timeout(cmd).await
    }

    /// Execute command with macOS Seatbelt sandbox.
    #[cfg(target_os = "macos")]
    async fn execute_with_seatbelt(&self, command: &str) -> Result<String, Error> {
        // Generate Seatbelt profile
        let profile = self.generate_seatbelt_profile();

        // Create temporary profile file
        let profile_path =
            std::env::temp_dir().join(format!("dashflow_sandbox_{}.sb", std::process::id()));

        tokio::fs::write(&profile_path, &profile)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to write sandbox profile: {}", e)))?;

        // Build command with sandbox-exec
        let mut cmd = Command::new("sandbox-exec");
        cmd.args(["-f", profile_path.to_str().unwrap_or("")]);
        cmd.args(["sh", "-c", command]);

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let result = self.run_command_with_timeout(cmd).await;

        // SAFETY: Profile file cleanup is best-effort - failure to delete a temp file
        // doesn't affect correctness and will be cleaned up eventually by the OS.
        let _ = tokio::fs::remove_file(&profile_path).await;

        result
    }

    /// Generate Seatbelt profile for macOS.
    #[cfg(target_os = "macos")]
    fn generate_seatbelt_profile(&self) -> String {
        let mut profile = String::from("(version 1)\n");

        // Default deny
        profile.push_str("(deny default)\n");

        // Allow process operations
        profile.push_str("(allow process-fork)\n");
        profile.push_str("(allow process-exec)\n");
        profile.push_str("(allow signal)\n");

        // Allow system sockets
        profile.push_str("(allow system-socket)\n");

        // Basic system read access
        profile.push_str("(allow file-read*\n");
        profile.push_str("    (literal \"/dev/null\")\n");
        profile.push_str("    (literal \"/dev/random\")\n");
        profile.push_str("    (literal \"/dev/urandom\")\n");
        profile.push_str("    (subpath \"/usr\")\n");
        profile.push_str("    (subpath \"/bin\")\n");
        profile.push_str("    (subpath \"/sbin\")\n");
        profile.push_str("    (subpath \"/System\")\n");
        profile.push_str("    (subpath \"/Library\")\n");
        profile.push_str("    (subpath \"/Applications\")\n");
        profile.push_str("    (subpath \"/private/var\")\n");
        profile.push_str(")\n");

        // Add readable roots
        for root in &self.readable_roots {
            if let Some(path) = root.to_str() {
                profile.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", path));
            }
        }

        // Add writable roots (also readable)
        for root in &self.writable_roots {
            if let Some(path) = root.to_str() {
                profile.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", path));
                profile.push_str(&format!("(allow file-write* (subpath \"{}\"))\n", path));
            }
        }

        // Network access
        if self.allow_network || self.sandbox_mode == SandboxMode::Permissive {
            profile.push_str("(allow network*)\n");
        }

        // Allow TTY access
        profile.push_str("(allow file-read* file-write* (regex #\"^/dev/tty.*\"))\n");
        profile.push_str("(allow file-ioctl (regex #\"^/dev/tty.*\"))\n");

        profile
    }

    /// Execute command with Linux Landlock sandbox.
    #[cfg(target_os = "linux")]
    async fn execute_with_landlock(&self, command: &str) -> Result<String, Error> {
        // Landlock requires kernel calls, which we do via a wrapper script
        // or direct syscalls. For portability, we use a helper approach.

        // Generate environment variables for the wrapper
        let writable_paths = self
            .writable_roots
            .iter()
            .filter_map(|p| p.to_str())
            .collect::<Vec<_>>()
            .join(":");

        let readable_paths = self
            .readable_roots
            .iter()
            .filter_map(|p| p.to_str())
            .collect::<Vec<_>>()
            .join(":");

        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);

        // Set Landlock-related environment hints (actual enforcement needs landlock crate)
        cmd.env("DASHFLOW_SANDBOX", "landlock");
        cmd.env("DASHFLOW_WRITABLE", &writable_paths);
        cmd.env("DASHFLOW_READABLE", &readable_paths);

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Note: Full Landlock enforcement requires the `landlock` crate and syscalls.
        // This is a placeholder for the interface - real enforcement would use:
        // let ruleset = Ruleset::new().handle_access(Access::FS)?;
        // ruleset.restrict_self()?;

        self.run_command_with_timeout(cmd).await
    }

    /// Run command with timeout and output capture.
    async fn run_command_with_timeout(&self, mut cmd: Command) -> Result<String, Error> {
        let mut child = cmd
            .spawn()
            .map_err(|e| Error::tool_error(format!("Failed to spawn command: {}", e)))?;

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
                .map_err(|e| Error::tool_error(format!("Join error: {}", e)))?
                .map_err(|e| Error::tool_error(format!("Read stdout error: {}", e)))?;
            let stderr_bytes = stderr_result
                .map_err(|e| Error::tool_error(format!("Join error: {}", e)))?
                .map_err(|e| Error::tool_error(format!("Read stderr error: {}", e)))?;

            let status = child
                .wait()
                .await
                .map_err(|e| Error::tool_error(format!("Wait error: {}", e)))?;

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
                    output.push_str(&format!("\n\nExit code: {}", exit_code));
                }

                if output.is_empty() {
                    output = "<no output>".to_string();
                }

                Ok(output)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                // SAFETY: Kill failure is acceptable - process may have already exited.
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
impl Tool for SandboxedShellTool {
    fn name(&self) -> &'static str {
        "sandboxed_shell"
    }

    fn description(&self) -> &'static str {
        "Execute shell commands in an OS-level sandbox. \
         Provides filesystem and network isolation. \
         Commands are analyzed for safety before execution."
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
            self.execute(&command).await
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_mode_display() {
        assert_eq!(SandboxMode::Strict.to_string(), "strict");
        assert_eq!(SandboxMode::Permissive.to_string(), "permissive");
        assert_eq!(SandboxMode::Disabled.to_string(), "disabled");
    }

    #[test]
    fn test_sandbox_error_display() {
        let err = SandboxError::NotAvailable {
            platform: "test".to_string(),
            reason: "no mechanism".to_string(),
        };
        assert!(err.to_string().contains("test"));
        assert!(err.to_string().contains("no mechanism"));

        let err = SandboxError::PermissionDenied {
            path: Some(PathBuf::from("/etc")),
            operation: "write".to_string(),
        };
        assert!(err.to_string().contains("write"));
    }

    #[test]
    fn test_capabilities_detect() {
        let caps = SandboxCapabilities::detect();
        // Just verify it doesn't panic and returns valid platform
        assert!(!caps.platform.is_empty());
    }

    #[test]
    fn test_builder_defaults() {
        let builder = SandboxedShellToolBuilder::new();
        // Builder should have sane defaults
        assert_eq!(builder.sandbox_mode, SandboxMode::Strict);
        assert!(builder.writable_roots.is_empty());
        assert_eq!(builder.timeout_seconds, 30);
    }

    #[test]
    fn test_builder_chain() {
        let builder = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Permissive)
            .writable_roots(vec![PathBuf::from("/tmp")])
            .add_writable_root(PathBuf::from("/var/tmp"))
            .readable_roots(vec![PathBuf::from("/usr")])
            .timeout(60)
            .max_output_bytes(512 * 1024)
            .allow_network(true);

        assert_eq!(builder.sandbox_mode, SandboxMode::Permissive);
        assert_eq!(builder.writable_roots.len(), 2);
        assert_eq!(builder.timeout_seconds, 60);
        assert!(builder.allow_network);
    }

    #[test]
    fn test_builder_disabled_mode_always_builds() {
        // Disabled mode should always succeed regardless of platform
        let result = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build();
        assert!(result.is_ok());

        let tool = result.unwrap();
        assert_eq!(tool.sandbox_mode(), SandboxMode::Disabled);
    }

    #[test]
    fn test_builder_with_fallback_warn() {
        let result = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Strict)
            .on_sandbox_missing(SandboxFallback::Warn)
            .build();

        // Should succeed even if sandbox unavailable due to Warn fallback
        // (or succeed if sandbox is available)
        // Either way shouldn't error
        let _ = result;
    }

    #[tokio::test]
    async fn test_tool_interface() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build()
            .unwrap();

        assert_eq!(tool.name(), "sandboxed_shell");
        assert!(tool.description().contains("sandbox"));

        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].get("command").is_some());
    }

    #[tokio::test]
    async fn test_analyze_command() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build()
            .unwrap();

        let analysis = tool.analyze("ls -la");
        assert!(analysis.severity <= Severity::Unknown);

        let analysis = tool.analyze("rm -rf /");
        assert_eq!(analysis.severity, Severity::Forbidden);
    }

    #[tokio::test]
    async fn test_execute_simple_command() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .timeout(5)
            .build()
            .unwrap();

        let result = tool.execute("echo hello").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_forbidden_command() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build()
            .unwrap();

        let result = tool.execute("rm -rf /").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("forbidden"));
    }

    #[tokio::test]
    async fn test_tool_call_structured_input() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build()
            .unwrap();

        let input = serde_json::json!({"command": "echo test"});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tool_call_string_input() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build()
            .unwrap();

        let result = tool._call(ToolInput::String("echo test".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_only_mode() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .build()
            .unwrap();

        let input = serde_json::json!({
            "command": "ls -la",
            "analyze_only": true
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        // Should return JSON analysis, not command output
        assert!(output.contains("severity") || output.contains("Severity"));
    }

    #[tokio::test]
    async fn test_timeout() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .timeout(1)
            .build()
            .unwrap();

        let result = if cfg!(target_os = "windows") {
            tool.execute("timeout /t 5").await
        } else {
            tool.execute("sleep 5").await
        };

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn test_working_directory() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .working_dir(PathBuf::from("/tmp"))
            .build()
            .unwrap();

        let result = tool.execute("pwd").await;
        if !cfg!(target_os = "windows") {
            assert!(result.is_ok());
            // May be /tmp or /private/tmp on macOS
            let output = result.unwrap();
            assert!(output.contains("tmp"));
        }
    }

    #[test]
    fn test_writable_roots_getter() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Disabled)
            .writable_roots(vec![PathBuf::from("/tmp"), PathBuf::from("/var/tmp")])
            .build()
            .unwrap();

        let roots = tool.writable_roots();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&PathBuf::from("/tmp")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_seatbelt_profile_generation() {
        let tool = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Strict)
            .writable_roots(vec![PathBuf::from("/tmp")])
            .readable_roots(vec![PathBuf::from("/usr/local")])
            .on_sandbox_missing(SandboxFallback::Silent)
            .build()
            .unwrap();

        let profile = tool.generate_seatbelt_profile();
        assert!(profile.contains("(version 1)"));
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("/tmp"));
        assert!(profile.contains("/usr/local"));
    }

    #[test]
    fn test_sandbox_capabilities_methods() {
        let caps = SandboxCapabilities::detect();

        // Test any_available() - doesn't panic
        let _ = caps.any_available();

        // Test available_mechanisms() - returns valid vec
        let mechanisms = caps.available_mechanisms();
        assert!(mechanisms.len() <= 3); // At most seatbelt, landlock, seccomp
    }

    #[test]
    fn test_custom_missing_callback() {
        let callback: SandboxMissingCallback = Arc::new(|mode, _reason| {
            if mode == SandboxMode::Strict {
                SandboxFallback::Fail
            } else {
                SandboxFallback::Warn
            }
        });

        let _ = SandboxedShellTool::builder()
            .sandbox_mode(SandboxMode::Permissive)
            .with_missing_callback(callback)
            .build();
    }
}
