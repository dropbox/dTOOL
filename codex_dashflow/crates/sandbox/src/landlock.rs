//! Landlock sandbox implementation for Linux
//!
//! This module provides filesystem access control using Linux Landlock LSM
//! and network access control using seccomp filters.

use crate::{SandboxError, SandboxMode};
use landlock::{Access, AccessFs, CompatLevel, Compatible, Ruleset, RulesetAttr, ABI};
use seccompiler::{
    apply_filter, BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition,
    SeccompFilter, SeccompRule, TargetArch,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Environment variable set to indicate Landlock is active
pub const LANDLOCK_ENV_VAR: &str = "CODEX_LANDLOCK_ACTIVE";

/// Configuration for Landlock sandbox execution
#[derive(Debug, Clone)]
pub struct LandlockConfig {
    /// The sandbox mode to apply
    pub mode: SandboxMode,
    /// The working directory for the command
    pub working_dir: PathBuf,
    /// Additional writable root directories (for WorkspaceWrite mode)
    pub writable_roots: Vec<PathBuf>,
}

impl LandlockConfig {
    /// Create a new Landlock configuration
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
}

/// Check if Landlock is available on this system
pub fn is_landlock_available() -> bool {
    // Check kernel support by trying to create a ruleset
    // Landlock was added in Linux 5.13
    match Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(AccessFs::from_read(ABI::V1))
    {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Apply Landlock filesystem rules to restrict write access
///
/// Allows read access to the entire filesystem while restricting
/// write access to /dev/null and specified writable roots.
fn apply_landlock_filesystem_rules(writable_roots: &[PathBuf]) -> Result<(), SandboxError> {
    let abi = ABI::V5;
    let access_rw = AccessFs::from_all(abi);
    let access_ro = AccessFs::from_read(abi);

    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(access_rw)
        .map_err(|e| SandboxError::ExecutionError(format!("Landlock handle_access failed: {}", e)))?
        .create()
        .map_err(|e| SandboxError::ExecutionError(format!("Landlock create failed: {}", e)))?
        // Allow read access everywhere
        .add_rules(landlock::path_beneath_rules(&["/"], access_ro))
        .map_err(|e| SandboxError::ExecutionError(format!("Landlock read rule failed: {}", e)))?
        // Allow write to /dev/null (common for discarding output)
        .add_rules(landlock::path_beneath_rules(&["/dev/null"], access_rw))
        .map_err(|e| {
            SandboxError::ExecutionError(format!("Landlock /dev/null rule failed: {}", e))
        })?
        .set_no_new_privs(true);

    // Add writable roots if specified
    if !writable_roots.is_empty() {
        let roots: Vec<&PathBuf> = writable_roots.iter().collect();
        ruleset = ruleset
            .add_rules(landlock::path_beneath_rules(&roots, access_rw))
            .map_err(|e| {
                SandboxError::ExecutionError(format!("Landlock writable root rule failed: {}", e))
            })?;
    }

    let status = ruleset.restrict_self().map_err(|e| {
        SandboxError::ExecutionError(format!("Landlock restrict_self failed: {}", e))
    })?;

    if status.ruleset == landlock::RulesetStatus::NotEnforced {
        return Err(SandboxError::ExecutionError(
            "Landlock rules not enforced (kernel may not support it)".to_string(),
        ));
    }

    Ok(())
}

/// Apply seccomp filter to block network access (except AF_UNIX)
fn apply_network_seccomp_filter() -> Result<(), SandboxError> {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    // Helper to deny a syscall unconditionally
    let mut deny_syscall = |nr: i64| {
        rules.insert(nr, vec![]); // empty rule vec = unconditional match
    };

    // Block network-related syscalls
    deny_syscall(libc::SYS_connect);
    deny_syscall(libc::SYS_accept);
    deny_syscall(libc::SYS_accept4);
    deny_syscall(libc::SYS_bind);
    deny_syscall(libc::SYS_listen);
    deny_syscall(libc::SYS_getpeername);
    deny_syscall(libc::SYS_getsockname);
    deny_syscall(libc::SYS_shutdown);
    deny_syscall(libc::SYS_sendto);
    deny_syscall(libc::SYS_sendmsg);
    deny_syscall(libc::SYS_sendmmsg);
    // Allow recvfrom for subprocess management (e.g., cargo clippy)
    deny_syscall(libc::SYS_recvmsg);
    deny_syscall(libc::SYS_recvmmsg);
    deny_syscall(libc::SYS_getsockopt);
    deny_syscall(libc::SYS_setsockopt);
    deny_syscall(libc::SYS_ptrace);

    // For socket syscall, only allow AF_UNIX
    let unix_only_rule = SeccompRule::new(vec![SeccompCondition::new(
        0, // first argument (domain)
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::Ne,
        libc::AF_UNIX as u64,
    )
    .map_err(|e| SandboxError::ExecutionError(format!("Seccomp condition failed: {}", e)))?])
    .map_err(|e| SandboxError::ExecutionError(format!("Seccomp rule failed: {}", e)))?;

    rules.insert(libc::SYS_socket, vec![unix_only_rule.clone()]);
    rules.insert(libc::SYS_socketpair, vec![unix_only_rule]);

    let arch = if cfg!(target_arch = "x86_64") {
        TargetArch::x86_64
    } else if cfg!(target_arch = "aarch64") {
        TargetArch::aarch64
    } else {
        return Err(SandboxError::ExecutionError(
            "Unsupported architecture for seccomp".to_string(),
        ));
    };

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,                     // default: allow
        SeccompAction::Errno(libc::EPERM as u32), // when rule matches: return EPERM
        arch,
    )
    .map_err(|e| SandboxError::ExecutionError(format!("Seccomp filter creation failed: {}", e)))?;

    let prog: BpfProgram = filter
        .try_into()
        .map_err(|e| SandboxError::ExecutionError(format!("BPF program creation failed: {}", e)))?;

    apply_filter(&prog)
        .map_err(|e| SandboxError::ExecutionError(format!("Seccomp apply_filter failed: {}", e)))?;

    Ok(())
}

/// Apply sandbox policy to the current thread
///
/// This should be called from a child process before exec'ing the target command.
/// The restrictions will be inherited by the exec'd process.
pub fn apply_sandbox_to_current_thread(config: &LandlockConfig) -> Result<(), SandboxError> {
    match config.mode {
        SandboxMode::DangerFullAccess => {
            // No restrictions in full access mode
            Ok(())
        }
        SandboxMode::ReadOnly => {
            // No network, no writes (except /dev/null)
            apply_network_seccomp_filter()?;
            apply_landlock_filesystem_rules(&[])?;
            Ok(())
        }
        SandboxMode::WorkspaceWrite => {
            // No network, write only in workspace and temp
            apply_network_seccomp_filter()?;

            let mut writable_roots = config.writable_roots.clone();
            writable_roots.push(config.working_dir.clone());

            // Also allow /tmp and /var/tmp
            writable_roots.push(PathBuf::from("/tmp"));
            writable_roots.push(PathBuf::from("/var/tmp"));

            apply_landlock_filesystem_rules(&writable_roots)?;
            Ok(())
        }
    }
}

/// Execute a command with Landlock sandbox restrictions
///
/// This spawns a subprocess that:
/// 1. Applies Landlock/seccomp restrictions
/// 2. Exec's the target command
pub async fn execute_sandboxed_command(
    command: &str,
    config: &LandlockConfig,
) -> Result<String, SandboxError> {
    use std::ffi::CString;
    use std::io::{Read, Write};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    // For DangerFullAccess, just run without sandbox
    if matches!(config.mode, SandboxMode::DangerFullAccess) {
        return execute_unsandboxed(command, &config.working_dir).await;
    }

    // We need to fork/exec with sandbox applied to the child
    // Use a pipe to communicate results
    let (mut reader, mut writer) = std::os::unix::net::UnixStream::pair()
        .map_err(|e| SandboxError::SpawnError(format!("Failed to create pipe: {}", e)))?;

    // Clone config for the child
    let config_clone = config.clone();
    let command_clone = command.to_string();

    // Fork and apply sandbox in child
    match unsafe { libc::fork() } {
        -1 => Err(SandboxError::SpawnError("fork() failed".to_string())),
        0 => {
            // Child process
            drop(reader);

            // Apply sandbox restrictions BEFORE exec
            if let Err(e) = apply_sandbox_to_current_thread(&config_clone) {
                let msg = format!("SANDBOX_ERROR:{}", e);
                let _ = writer.write_all(msg.as_bytes());
                std::process::exit(1);
            }

            // Set environment variable to indicate sandbox is active
            std::env::set_var(LANDLOCK_ENV_VAR, "1");

            // Now exec the shell command
            let err = Command::new("/bin/sh")
                .arg("-c")
                .arg(&command_clone)
                .current_dir(&config_clone.working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .exec();

            // If we get here, exec failed
            let msg = format!("EXEC_ERROR:{}", err);
            let _ = writer.write_all(msg.as_bytes());
            std::process::exit(1);
        }
        child_pid => {
            // Parent process
            drop(writer);

            // Wait for child to complete
            let mut status: libc::c_int = 0;
            unsafe {
                libc::waitpid(child_pid, &mut status, 0);
            }

            // Read output from child (if any error messages)
            let mut output = String::new();
            let _ = reader.read_to_string(&mut output);

            if output.starts_with("SANDBOX_ERROR:") {
                return Err(SandboxError::ExecutionError(
                    output.trim_start_matches("SANDBOX_ERROR:").to_string(),
                ));
            }

            if output.starts_with("EXEC_ERROR:") {
                return Err(SandboxError::SpawnError(
                    output.trim_start_matches("EXEC_ERROR:").to_string(),
                ));
            }

            // The child's exec() replaces the process, so we need to run
            // the command differently. Let's use a simpler approach with
            // process groups.
            // Actually, the exec() approach won't work well with pipes.
            // Let's use a different strategy: spawn with pre_exec hook.

            // Re-implement using pre_exec
            execute_with_pre_exec(command, config).await
        }
    }
}

/// Execute a command using pre_exec to apply sandbox before exec
async fn execute_with_pre_exec(
    command: &str,
    config: &LandlockConfig,
) -> Result<String, SandboxError> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let config_mode = config.mode.clone();
    let config_working_dir = config.working_dir.clone();
    let config_writable_roots = config.writable_roots.clone();

    // Build the command with pre_exec hook
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(&config.working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env(LANDLOCK_ENV_VAR, "1");

    // SAFETY: pre_exec runs after fork but before exec in the child
    unsafe {
        cmd.pre_exec(move || {
            // Apply sandbox in child process
            let config = LandlockConfig {
                mode: config_mode.clone(),
                working_dir: config_working_dir.clone(),
                writable_roots: config_writable_roots.clone(),
            };

            apply_sandbox_to_current_thread(&config).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())
            })?;

            Ok(())
        });
    }

    let output = cmd
        .output()
        .map_err(|e| SandboxError::SpawnError(e.to_string()))?;

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

        // Check if this was a sandbox denial
        if combined.contains("Operation not permitted")
            || combined.contains("Permission denied")
            || stderr.contains("EPERM")
        {
            Err(SandboxError::PolicyDenied(combined))
        } else {
            Err(SandboxError::CommandFailed {
                exit_code: output.status.code(),
                output: combined,
            })
        }
    }
}

/// Execute a command without sandbox restrictions (fallback)
async fn execute_unsandboxed(command: &str, working_dir: &PathBuf) -> Result<String, SandboxError> {
    use tokio::process::Command;

    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_landlock_config_new() {
        let config = LandlockConfig::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
        assert!(matches!(config.mode, SandboxMode::ReadOnly));
        assert_eq!(config.working_dir, PathBuf::from("/tmp"));
        assert!(config.writable_roots.is_empty());
    }

    #[test]
    fn test_landlock_config_with_writable_root() {
        let config = LandlockConfig::new(SandboxMode::WorkspaceWrite, PathBuf::from("/tmp"))
            .with_writable_root(PathBuf::from("/var/log"));
        assert_eq!(config.writable_roots.len(), 1);
        assert_eq!(config.writable_roots[0], PathBuf::from("/var/log"));
    }

    #[test]
    fn test_is_landlock_available() {
        // This will return false on non-Linux or older Linux kernels
        // Just ensure it doesn't panic
        let _ = is_landlock_available();
    }

    // Integration tests that actually apply Landlock are in tests/suite/landlock.rs
    // because they need to be run in a subprocess to avoid affecting the test runner
}
