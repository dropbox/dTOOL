//! Seatbelt sandbox implementation for macOS
//!
//! Uses Apple's sandbox-exec to restrict process capabilities.
//!
//! Note: This module is only available on macOS (gated by cfg in lib.rs).

use std::ffi::CStr;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::{Child, Command};

use crate::SandboxMode;

const MACOS_SEATBELT_BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");
const MACOS_SEATBELT_NETWORK_POLICY: &str = include_str!("seatbelt_network_policy.sbpl");

/// Path to the sandbox-exec binary. Only use the system path to prevent
/// injection attacks via PATH manipulation.
pub const MACOS_PATH_TO_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

/// Environment variable set when running under sandbox
pub const SANDBOX_ENV_VAR: &str = "CODEX_DASHFLOW_SANDBOX";

/// Configuration for a sandboxed execution
#[derive(Clone, Debug)]
pub struct SeatbeltConfig {
    /// The sandbox mode to apply
    pub mode: SandboxMode,
    /// Working directory for the command (also becomes a writable root in WorkspaceWrite mode)
    pub working_dir: PathBuf,
    /// Additional directories that should be writable (for WorkspaceWrite mode)
    pub writable_roots: Vec<PathBuf>,
}

impl SeatbeltConfig {
    /// Create a new seatbelt config with the given mode and working directory
    pub fn new(mode: SandboxMode, working_dir: PathBuf) -> Self {
        Self {
            mode,
            working_dir,
            writable_roots: Vec::new(),
        }
    }

    /// Add an additional writable root directory
    pub fn with_writable_root(mut self, root: PathBuf) -> Self {
        self.writable_roots.push(root);
        self
    }

    /// Returns true if full disk write access is allowed
    fn has_full_disk_write_access(&self) -> bool {
        matches!(self.mode, SandboxMode::DangerFullAccess)
    }

    /// Returns true if full disk read access is allowed
    fn has_full_disk_read_access(&self) -> bool {
        // All modes allow full disk read access
        true
    }

    /// Returns true if network access is allowed
    fn has_network_access(&self) -> bool {
        matches!(self.mode, SandboxMode::DangerFullAccess)
    }

    /// Get all writable roots for the current configuration
    fn get_writable_roots(&self) -> Vec<PathBuf> {
        match self.mode {
            SandboxMode::ReadOnly => Vec::new(),
            SandboxMode::WorkspaceWrite => {
                let mut roots = vec![self.working_dir.clone()];
                roots.extend(self.writable_roots.clone());
                // Also include TMPDIR and /tmp for common operations
                if let Ok(tmpdir) = std::env::var("TMPDIR") {
                    roots.push(PathBuf::from(tmpdir));
                }
                roots.push(PathBuf::from("/tmp"));
                roots
            }
            SandboxMode::DangerFullAccess => Vec::new(), // Full access, no restrictions
        }
    }
}

/// Spawn a command under the seatbelt sandbox
pub async fn spawn_command_under_seatbelt(
    command: Vec<String>,
    config: &SeatbeltConfig,
) -> std::io::Result<Child> {
    let args = create_seatbelt_command_args(&command, config);

    tracing::debug!(
        mode = ?config.mode,
        command = ?command,
        "Spawning command under seatbelt"
    );

    let mut cmd = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE);
    cmd.args(&args)
        .current_dir(&config.working_dir)
        .env(SANDBOX_ENV_VAR, "seatbelt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.spawn()
}

/// Create the command-line arguments for sandbox-exec
fn create_seatbelt_command_args(command: &[String], config: &SeatbeltConfig) -> Vec<String> {
    let (file_write_policy, file_write_dir_params) = build_file_write_policy(config);
    let file_read_policy = build_file_read_policy(config);
    let network_policy = build_network_policy(config);

    let full_policy = format!(
        "{MACOS_SEATBELT_BASE_POLICY}\n{file_read_policy}\n{file_write_policy}\n{network_policy}"
    );

    let dir_params = [file_write_dir_params, macos_dir_params()].concat();

    let mut seatbelt_args: Vec<String> = vec!["-p".to_string(), full_policy];

    // Add directory parameters
    let definition_args = dir_params
        .into_iter()
        .map(|(key, value)| format!("-D{key}={value}", value = value.to_string_lossy()));
    seatbelt_args.extend(definition_args);

    // Add separator and the actual command
    seatbelt_args.push("--".to_string());
    seatbelt_args.extend(command.iter().cloned());

    seatbelt_args
}

/// Build the file write policy based on sandbox config
fn build_file_write_policy(config: &SeatbeltConfig) -> (String, Vec<(String, PathBuf)>) {
    if config.has_full_disk_write_access() {
        // Full write access
        (
            r#"(allow file-write* (regex #"^/"))"#.to_string(),
            Vec::new(),
        )
    } else {
        let writable_roots = config.get_writable_roots();

        if writable_roots.is_empty() {
            return (String::new(), Vec::new());
        }

        let mut writable_folder_policies: Vec<String> = Vec::new();
        let mut file_write_params: Vec<(String, PathBuf)> = Vec::new();

        for (index, root) in writable_roots.iter().enumerate() {
            // Canonicalize to avoid mismatches like /var vs /private/var on macOS
            let canonical_root = root.canonicalize().unwrap_or_else(|_| root.clone());
            let root_param = format!("WRITABLE_ROOT_{index}");
            file_write_params.push((root_param.clone(), canonical_root));

            // Exclude .git directories from write access for security
            let git_path = root.join(".git");
            if git_path.exists() {
                let git_canonical = git_path.canonicalize().unwrap_or(git_path);
                let ro_param = format!("WRITABLE_ROOT_{index}_RO_0");
                file_write_params.push((ro_param.clone(), git_canonical));
                writable_folder_policies.push(format!(
                    "(require-all (subpath (param \"{root_param}\")) (require-not (subpath (param \"{ro_param}\"))))"
                ));
            } else {
                writable_folder_policies.push(format!("(subpath (param \"{root_param}\"))"));
            }
        }

        let file_write_policy = format!(
            "(allow file-write*\n{}\n)",
            writable_folder_policies.join(" ")
        );
        (file_write_policy, file_write_params)
    }
}

/// Build the file read policy
fn build_file_read_policy(config: &SeatbeltConfig) -> &'static str {
    if config.has_full_disk_read_access() {
        "; allow read-only file operations\n(allow file-read*)"
    } else {
        ""
    }
}

/// Build the network policy
fn build_network_policy(config: &SeatbeltConfig) -> &'static str {
    if config.has_network_access() {
        MACOS_SEATBELT_NETWORK_POLICY
    } else {
        ""
    }
}

/// Get macOS-specific directory parameters for the seatbelt profile
fn macos_dir_params() -> Vec<(String, PathBuf)> {
    if let Some(p) = confstr_path(libc::_CS_DARWIN_USER_CACHE_DIR) {
        return vec![("DARWIN_USER_CACHE_DIR".to_string(), p)];
    }
    vec![]
}

/// Wraps libc::confstr to return a String
fn confstr(name: libc::c_int) -> Option<String> {
    let mut buf = vec![0_i8; (libc::PATH_MAX as usize) + 1];
    let len = unsafe { libc::confstr(name, buf.as_mut_ptr(), buf.len()) };
    if len == 0 {
        return None;
    }
    // confstr guarantees NUL-termination when len > 0
    let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
    cstr.to_str().ok().map(ToString::to_string)
}

/// Wraps confstr to return a canonicalized PathBuf
fn confstr_path(name: libc::c_int) -> Option<PathBuf> {
    let s = confstr(name)?;
    let path = PathBuf::from(s);
    path.canonicalize().ok().or(Some(path))
}

/// Execute a shell command under seatbelt and return the output
pub async fn execute_sandboxed_command(
    command: &str,
    config: &SeatbeltConfig,
) -> Result<String, crate::SandboxError> {
    // Build the shell command
    let shell_command = vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()];

    let child = spawn_command_under_seatbelt(shell_command, config)
        .await
        .map_err(|e| crate::SandboxError::SpawnError(e.to_string()))?;

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| crate::SandboxError::ExecutionError(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(stdout.to_string())
    } else {
        // Include both stdout and stderr in the error for debugging
        let combined = if stderr.is_empty() {
            stdout.to_string()
        } else if stdout.is_empty() {
            stderr.to_string()
        } else {
            format!("{}\n{}", stdout, stderr)
        };
        Err(crate::SandboxError::CommandFailed {
            exit_code: output.status.code(),
            output: combined,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_seatbelt_config_new() {
        let config = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        assert_eq!(config.mode, SandboxMode::ReadOnly);
        assert_eq!(config.working_dir, PathBuf::from("/tmp"));
        assert!(config.writable_roots.is_empty());
    }

    #[test]
    fn test_seatbelt_config_with_writable_root() {
        let config = SeatbeltConfig::new(SandboxMode::WorkspaceWrite, PathBuf::from("/tmp"))
            .with_writable_root(PathBuf::from("/var/log"));
        assert_eq!(config.writable_roots.len(), 1);
        assert_eq!(config.writable_roots[0], PathBuf::from("/var/log"));
    }

    #[test]
    fn test_has_full_disk_write_access() {
        let read_only = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        assert!(!read_only.has_full_disk_write_access());

        let workspace = SeatbeltConfig::new(SandboxMode::WorkspaceWrite, PathBuf::from("/tmp"));
        assert!(!workspace.has_full_disk_write_access());

        let full = SeatbeltConfig::new(SandboxMode::DangerFullAccess, PathBuf::from("/tmp"));
        assert!(full.has_full_disk_write_access());
    }

    #[test]
    fn test_has_network_access() {
        let read_only = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        assert!(!read_only.has_network_access());

        let workspace = SeatbeltConfig::new(SandboxMode::WorkspaceWrite, PathBuf::from("/tmp"));
        assert!(!workspace.has_network_access());

        let full = SeatbeltConfig::new(SandboxMode::DangerFullAccess, PathBuf::from("/tmp"));
        assert!(full.has_network_access());
    }

    #[test]
    fn test_get_writable_roots_read_only() {
        let config = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/workspace"));
        let roots = config.get_writable_roots();
        assert!(roots.is_empty());
    }

    #[test]
    fn test_get_writable_roots_workspace_write() {
        let config = SeatbeltConfig::new(SandboxMode::WorkspaceWrite, PathBuf::from("/workspace"))
            .with_writable_root(PathBuf::from("/extra"));
        let roots = config.get_writable_roots();
        assert!(roots.contains(&PathBuf::from("/workspace")));
        assert!(roots.contains(&PathBuf::from("/extra")));
        assert!(roots.contains(&PathBuf::from("/tmp")));
    }

    #[test]
    fn test_create_seatbelt_command_args_basic() {
        let config = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        let command = vec!["echo".to_string(), "hello".to_string()];
        let args = create_seatbelt_command_args(&command, &config);

        // Should have -p flag
        assert!(args.contains(&"-p".to_string()));
        // Should have -- separator
        assert!(args.contains(&"--".to_string()));
        // Should have the command
        assert!(args.contains(&"echo".to_string()));
        assert!(args.contains(&"hello".to_string()));
    }

    #[test]
    fn test_build_file_read_policy() {
        let config = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        let policy = build_file_read_policy(&config);
        assert!(policy.contains("(allow file-read*)"));
    }

    #[test]
    fn test_build_file_write_policy_read_only() {
        let config = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        let (policy, params) = build_file_write_policy(&config);
        assert!(policy.is_empty());
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_file_write_policy_full_access() {
        let config = SeatbeltConfig::new(SandboxMode::DangerFullAccess, PathBuf::from("/tmp"));
        let (policy, params) = build_file_write_policy(&config);
        assert!(policy.contains("(allow file-write*"));
        assert!(params.is_empty());
    }

    #[tokio::test]
    async fn test_execute_sandboxed_command_echo() {
        // This test only runs on macOS where seatbelt is available
        if !Path::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE).exists() {
            return;
        }

        let config = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        let result = execute_sandboxed_command("echo 'hello from sandbox'", &config).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello from sandbox"));
    }

    #[tokio::test]
    async fn test_execute_sandboxed_command_read_only_prevents_write() {
        // This test only runs on macOS where seatbelt is available
        if !Path::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE).exists() {
            return;
        }

        let config = SeatbeltConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        // Try to write to a file - should fail in read-only mode
        let result =
            execute_sandboxed_command("touch /tmp/sandbox_test_file_should_fail", &config).await;
        // The command should fail because write is denied
        assert!(result.is_err());
    }
}
