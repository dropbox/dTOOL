//! Codex DashFlow Sandbox
//!
//! Sandbox execution using Seatbelt (macOS) or Landlock (Linux).
//! This crate provides secure command execution for the agent.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;

#[cfg(target_os = "macos")]
pub mod seatbelt;

#[cfg(target_os = "macos")]
pub use seatbelt::{execute_sandboxed_command, SeatbeltConfig, SANDBOX_ENV_VAR};

#[cfg(target_os = "linux")]
pub mod landlock;

#[cfg(target_os = "linux")]
pub use landlock::{
    execute_sandboxed_command, is_landlock_available, LandlockConfig, LANDLOCK_ENV_VAR,
};

/// Errors that can occur during sandboxed execution
#[derive(Debug, Error)]
pub enum SandboxError {
    /// Failed to spawn the sandboxed process
    #[error("Failed to spawn sandboxed process: {0}")]
    SpawnError(String),

    /// The sandboxed command failed to execute
    #[error("Sandboxed command execution failed: {0}")]
    ExecutionError(String),

    /// The command was denied by sandbox policy
    #[error("Command denied by sandbox policy: {0}")]
    PolicyDenied(String),

    /// The command exited with an error
    #[error("Command failed with exit code {exit_code:?}: {output}")]
    CommandFailed {
        exit_code: Option<i32>,
        output: String,
    },

    /// Sandbox is not available on this platform
    #[error("Sandbox not available on this platform")]
    NotAvailable,
}

/// Sandbox mode for tool execution
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// Read-only: read filesystem, no writes, no network
    #[default]
    ReadOnly,
    /// Workspace write: write within workspace, no network
    WorkspaceWrite,
    /// Full access: no restrictions (for containerized environments)
    DangerFullAccess,
}

impl fmt::Display for SandboxMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SandboxMode::ReadOnly => write!(f, "read-only"),
            SandboxMode::WorkspaceWrite => write!(f, "workspace-write"),
            SandboxMode::DangerFullAccess => write!(f, "full-access"),
        }
    }
}

impl FromStr for SandboxMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "read-only" | "readonly" | "read_only" => Ok(SandboxMode::ReadOnly),
            "workspace-write" | "workspace_write" | "workspacewrite" => {
                Ok(SandboxMode::WorkspaceWrite)
            }
            "full-access" | "full_access" | "fullaccess" | "danger-full-access" => {
                Ok(SandboxMode::DangerFullAccess)
            }
            _ => Err(format!(
                "Unknown sandbox mode: '{}'. Valid modes: read-only, workspace-write, full-access",
                s
            )),
        }
    }
}

impl SandboxMode {
    /// Returns true if this mode allows writing to the filesystem
    pub fn allows_write(&self) -> bool {
        matches!(
            self,
            SandboxMode::WorkspaceWrite | SandboxMode::DangerFullAccess
        )
    }

    /// Returns true if this mode allows network access
    pub fn allows_network(&self) -> bool {
        matches!(self, SandboxMode::DangerFullAccess)
    }

    /// Returns true if this mode is the dangerous unrestricted mode
    pub fn is_unrestricted(&self) -> bool {
        matches!(self, SandboxMode::DangerFullAccess)
    }

    /// Returns true if this mode is read-only (no writes allowed)
    /// Audit #47: Added helper for sandbox enforcement in file operations
    pub fn is_read_only(&self) -> bool {
        matches!(self, SandboxMode::ReadOnly)
    }
}

/// A sandboxed executor that can run commands with platform-specific restrictions
pub struct SandboxExecutor {
    mode: SandboxMode,
    working_dir: PathBuf,
    writable_roots: Vec<PathBuf>,
}

impl SandboxExecutor {
    /// Create a new sandbox executor with the given mode and working directory
    pub fn new(mode: SandboxMode, working_dir: PathBuf) -> Self {
        Self {
            mode,
            working_dir,
            writable_roots: Vec::new(),
        }
    }

    /// Add an additional writable root directory (only used in WorkspaceWrite mode)
    pub fn with_writable_root(mut self, root: PathBuf) -> Self {
        self.writable_roots.push(root);
        self
    }

    /// Get the current sandbox mode
    pub fn mode(&self) -> &SandboxMode {
        &self.mode
    }

    /// Execute a shell command within the sandbox
    ///
    /// On macOS, this uses Seatbelt. On Linux, this uses Landlock + seccomp.
    /// On other platforms, the command runs without sandbox restrictions.
    pub async fn execute(&self, command: &str) -> Result<String, SandboxError> {
        #[cfg(target_os = "macos")]
        {
            let mut config = SeatbeltConfig::new(self.mode, self.working_dir.clone());
            for root in &self.writable_roots {
                config = config.with_writable_root(root.clone());
            }
            execute_sandboxed_command(command, &config).await
        }

        #[cfg(target_os = "linux")]
        {
            let mut config = LandlockConfig::new(self.mode, self.working_dir.clone());
            for root in &self.writable_roots {
                config = config.with_writable_root(root.clone());
            }
            execute_sandboxed_command(command, &config).await
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            // Audit #67: Explicit warning for Windows and other unsupported platforms
            #[cfg(target_os = "windows")]
            tracing::warn!(
                "SECURITY WARNING: Sandbox not available on Windows. \
                 Commands will run WITHOUT any sandbox protection. \
                 Network access, full filesystem access, and process creation are all allowed. \
                 Consider running in a container or VM for isolation."
            );
            #[cfg(not(target_os = "windows"))]
            tracing::warn!(
                "Sandbox not available on this platform (requires macOS Seatbelt or Linux Landlock), \
                 running unsandboxed"
            );
            self.execute_unsandboxed(command).await
        }
    }

    /// Execute a command without sandbox restrictions (fallback)
    #[allow(dead_code)]
    async fn execute_unsandboxed(&self, command: &str) -> Result<String, SandboxError> {
        use tokio::process::Command;

        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.working_dir)
            .output()
            .await
            .map_err(|e| SandboxError::ExecutionError(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(stdout.to_string())
        } else {
            let combined = if stderr.is_empty() {
                stdout.to_string()
            } else if stdout.is_empty() {
                stderr.to_string()
            } else {
                format!("{}\n{}", stdout, stderr)
            };
            Err(SandboxError::CommandFailed {
                exit_code: output.status.code(),
                output: combined,
            })
        }
    }

    /// Check if sandboxing is available on the current platform
    pub fn is_available() -> bool {
        #[cfg(target_os = "macos")]
        {
            std::path::Path::new(seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE).exists()
        }

        #[cfg(target_os = "linux")]
        {
            is_landlock_available()
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_mode_default() {
        let mode = SandboxMode::default();
        assert_eq!(mode, SandboxMode::ReadOnly);
    }

    #[test]
    fn test_sandbox_mode_display() {
        assert_eq!(SandboxMode::ReadOnly.to_string(), "read-only");
        assert_eq!(SandboxMode::WorkspaceWrite.to_string(), "workspace-write");
        assert_eq!(SandboxMode::DangerFullAccess.to_string(), "full-access");
    }

    #[test]
    fn test_sandbox_mode_from_str() {
        // ReadOnly variants
        assert_eq!(
            SandboxMode::from_str("read-only").unwrap(),
            SandboxMode::ReadOnly
        );
        assert_eq!(
            SandboxMode::from_str("readonly").unwrap(),
            SandboxMode::ReadOnly
        );
        assert_eq!(
            SandboxMode::from_str("read_only").unwrap(),
            SandboxMode::ReadOnly
        );

        // WorkspaceWrite variants
        assert_eq!(
            SandboxMode::from_str("workspace-write").unwrap(),
            SandboxMode::WorkspaceWrite
        );
        assert_eq!(
            SandboxMode::from_str("workspace_write").unwrap(),
            SandboxMode::WorkspaceWrite
        );
        assert_eq!(
            SandboxMode::from_str("workspacewrite").unwrap(),
            SandboxMode::WorkspaceWrite
        );

        // DangerFullAccess variants
        assert_eq!(
            SandboxMode::from_str("full-access").unwrap(),
            SandboxMode::DangerFullAccess
        );
        assert_eq!(
            SandboxMode::from_str("full_access").unwrap(),
            SandboxMode::DangerFullAccess
        );
        assert_eq!(
            SandboxMode::from_str("danger-full-access").unwrap(),
            SandboxMode::DangerFullAccess
        );
    }

    #[test]
    fn test_sandbox_mode_from_str_invalid() {
        let result = SandboxMode::from_str("invalid");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Unknown sandbox mode: 'invalid'"));
    }

    #[test]
    fn test_sandbox_mode_allows_write() {
        assert!(!SandboxMode::ReadOnly.allows_write());
        assert!(SandboxMode::WorkspaceWrite.allows_write());
        assert!(SandboxMode::DangerFullAccess.allows_write());
    }

    #[test]
    fn test_sandbox_mode_allows_network() {
        assert!(!SandboxMode::ReadOnly.allows_network());
        assert!(!SandboxMode::WorkspaceWrite.allows_network());
        assert!(SandboxMode::DangerFullAccess.allows_network());
    }

    #[test]
    fn test_sandbox_mode_is_unrestricted() {
        assert!(!SandboxMode::ReadOnly.is_unrestricted());
        assert!(!SandboxMode::WorkspaceWrite.is_unrestricted());
        assert!(SandboxMode::DangerFullAccess.is_unrestricted());
    }

    #[test]
    fn test_sandbox_mode_is_read_only() {
        // Audit #47: Test is_read_only helper
        assert!(SandboxMode::ReadOnly.is_read_only());
        assert!(!SandboxMode::WorkspaceWrite.is_read_only());
        assert!(!SandboxMode::DangerFullAccess.is_read_only());
    }

    #[test]
    fn test_sandbox_mode_copy() {
        let mode = SandboxMode::WorkspaceWrite;
        let copied = mode; // SandboxMode implements Copy
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_sandbox_mode_debug() {
        let debug_str = format!("{:?}", SandboxMode::ReadOnly);
        assert_eq!(debug_str, "ReadOnly");
    }

    #[test]
    fn test_sandbox_mode_roundtrip() {
        for mode in [
            SandboxMode::ReadOnly,
            SandboxMode::WorkspaceWrite,
            SandboxMode::DangerFullAccess,
        ] {
            let s = mode.to_string();
            let parsed = SandboxMode::from_str(&s).unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn test_sandbox_executor_new() {
        let executor = SandboxExecutor::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        assert_eq!(executor.mode(), &SandboxMode::ReadOnly);
    }

    #[test]
    fn test_sandbox_executor_with_writable_root() {
        let executor = SandboxExecutor::new(SandboxMode::WorkspaceWrite, PathBuf::from("/tmp"))
            .with_writable_root(PathBuf::from("/var/log"));
        assert_eq!(executor.writable_roots.len(), 1);
    }

    #[test]
    fn test_sandbox_error_display() {
        let error = SandboxError::SpawnError("test".to_string());
        assert!(error.to_string().contains("test"));

        let error = SandboxError::CommandFailed {
            exit_code: Some(1),
            output: "failed".to_string(),
        };
        assert!(error.to_string().contains("failed"));
    }

    #[tokio::test]
    async fn test_sandbox_executor_execute_echo() {
        let executor = SandboxExecutor::new(SandboxMode::DangerFullAccess, PathBuf::from("/tmp"));
        let result = executor.execute("echo 'hello'").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }
}
