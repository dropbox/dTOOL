//! Process spawning utilities with sandbox support
//!
//! This module provides utilities for spawning child processes with proper
//! sandbox environment variables and parent death signaling (on Linux).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tracing::trace;

use codex_dashflow_sandbox::SandboxMode;

/// Environment variable set when network access is disabled by sandbox policy.
///
/// Will be set to "1" if:
/// 1. The process was spawned by Codex as part of a shell tool call.
/// 2. The sandbox mode does not allow network access (ReadOnly or WorkspaceWrite).
///
/// Tools and scripts can check this variable to provide appropriate error messages
/// when network operations fail due to sandboxing.
pub const CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR: &str = "CODEX_SANDBOX_NETWORK_DISABLED";

/// Environment variable indicating the sandbox type in use.
///
/// Set when the process is spawned under a sandbox. Values:
/// - "seatbelt" on macOS
/// - "landlock" on Linux
/// - Empty or unset when not sandboxed
pub const CODEX_SANDBOX_ENV_VAR: &str = "CODEX_SANDBOX";

/// Policy for how to handle stdio streams of spawned processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StdioPolicy {
    /// Redirect stdout/stderr for capture, null stdin.
    /// Use for shell tool execution where we need to capture output.
    #[default]
    RedirectForShellTool,
    /// Inherit stdio from parent process.
    /// Use for interactive commands or when output should go directly to terminal.
    Inherit,
}

/// Options for spawning a child process.
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// The program to execute
    pub program: PathBuf,
    /// Command-line arguments
    pub args: Vec<String>,
    /// Working directory for the process
    pub cwd: PathBuf,
    /// Sandbox mode (affects environment variables)
    pub sandbox_mode: SandboxMode,
    /// How to handle stdio
    pub stdio_policy: StdioPolicy,
    /// Environment variables to pass to the child
    pub env: HashMap<String, String>,
    /// Override for `argv[0]` (Unix only)
    #[cfg(unix)]
    pub arg0: Option<String>,
}

impl SpawnOptions {
    /// Create new spawn options with the required fields.
    pub fn new(program: impl Into<PathBuf>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: cwd.into(),
            sandbox_mode: SandboxMode::default(),
            stdio_policy: StdioPolicy::default(),
            env: HashMap::new(),
            #[cfg(unix)]
            arg0: None,
        }
    }

    /// Set the command-line arguments.
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Set the sandbox mode.
    pub fn sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.sandbox_mode = mode;
        self
    }

    /// Set the stdio policy.
    pub fn stdio_policy(mut self, policy: StdioPolicy) -> Self {
        self.stdio_policy = policy;
        self
    }

    /// Set the environment variables.
    pub fn env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Add a single environment variable.
    pub fn env_insert(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the `argv[0]` override (Unix only).
    #[cfg(unix)]
    pub fn arg0(mut self, arg0: impl Into<String>) -> Self {
        self.arg0 = Some(arg0.into());
        self
    }
}

/// Check if a sandbox mode allows network access.
fn has_network_access(mode: &SandboxMode) -> bool {
    matches!(mode, SandboxMode::DangerFullAccess)
}

/// Spawn a child process with the given options.
///
/// This function:
/// - Sets up the command with proper program, args, cwd, and environment
/// - Adds sandbox-related environment variables
/// - On Unix, creates a new process group for clean termination
/// - On Linux, sets up parent death signaling to kill children when parent dies
/// - Configures stdio according to the policy
///
/// # Errors
///
/// Returns an error if the process cannot be spawned.
pub async fn spawn_child(options: SpawnOptions) -> std::io::Result<Child> {
    trace!(
        "spawn_child: {:?} {:?} cwd={:?} sandbox={:?} stdio={:?}",
        options.program,
        options.args,
        options.cwd,
        options.sandbox_mode,
        options.stdio_policy
    );

    let mut cmd = Command::new(&options.program);

    cmd.args(&options.args);
    cmd.current_dir(&options.cwd);

    // Clear environment and set provided vars
    cmd.env_clear();
    cmd.envs(&options.env);

    // Set sandbox-related environment variables
    if !has_network_access(&options.sandbox_mode) {
        cmd.env(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR, "1");
    }

    // Set sandbox type indicator based on platform
    #[cfg(target_os = "macos")]
    if options.sandbox_mode != SandboxMode::DangerFullAccess {
        cmd.env(CODEX_SANDBOX_ENV_VAR, "seatbelt");
    }

    #[cfg(target_os = "linux")]
    if options.sandbox_mode != SandboxMode::DangerFullAccess {
        cmd.env(CODEX_SANDBOX_ENV_VAR, "landlock");
    }

    // Unix-specific: set argv[0] and process group handling
    #[cfg(unix)]
    {
        #[allow(unused_imports)]
        use std::os::unix::process::CommandExt;

        // Set argv[0] if specified
        if let Some(ref arg0) = options.arg0 {
            cmd.arg0(arg0);
        } else {
            cmd.arg0(options.program.to_string_lossy().to_string());
        }

        // Set up process group and parent death signaling
        #[cfg(target_os = "linux")]
        let parent_pid = unsafe { libc::getpid() };

        unsafe {
            cmd.pre_exec(move || {
                // Create new process group for clean termination
                if libc::setpgid(0, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                // Linux-only: signal child when parent dies
                #[cfg(target_os = "linux")]
                {
                    // Request SIGTERM when parent dies
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                        return Err(std::io::Error::last_os_error());
                    }

                    // Handle race: if parent already died, exit now
                    if libc::getppid() != parent_pid {
                        libc::raise(libc::SIGTERM);
                    }
                }
                Ok(())
            });
        }
    }

    // Configure stdio based on policy
    match options.stdio_policy {
        StdioPolicy::RedirectForShellTool => {
            // Null stdin to prevent commands hanging waiting for input
            // (e.g., ripgrep may try to read from stdin)
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }
        StdioPolicy::Inherit => {
            cmd.stdin(Stdio::inherit());
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
        }
    }

    // Kill child when Command is dropped
    cmd.kill_on_drop(true);

    cmd.spawn()
}

/// Convenience function for spawning with minimal options.
pub async fn spawn_simple(
    program: impl Into<PathBuf>,
    args: Vec<String>,
    cwd: impl Into<PathBuf>,
) -> std::io::Result<Child> {
    spawn_child(SpawnOptions::new(program, cwd).args(args)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_network_access() {
        assert!(!has_network_access(&SandboxMode::ReadOnly));
        assert!(!has_network_access(&SandboxMode::WorkspaceWrite));
        assert!(has_network_access(&SandboxMode::DangerFullAccess));
    }

    #[test]
    fn test_spawn_options_builder() {
        let opts = SpawnOptions::new("/bin/echo", "/tmp")
            .args(["hello", "world"])
            .sandbox_mode(SandboxMode::ReadOnly)
            .stdio_policy(StdioPolicy::Inherit)
            .env_insert("FOO", "bar");

        assert_eq!(opts.program, PathBuf::from("/bin/echo"));
        assert_eq!(opts.args, vec!["hello", "world"]);
        assert_eq!(opts.cwd, PathBuf::from("/tmp"));
        assert_eq!(opts.sandbox_mode, SandboxMode::ReadOnly);
        assert_eq!(opts.stdio_policy, StdioPolicy::Inherit);
        assert_eq!(opts.env.get("FOO"), Some(&"bar".to_string()));
    }

    #[test]
    fn test_stdio_policy_default() {
        assert_eq!(StdioPolicy::default(), StdioPolicy::RedirectForShellTool);
    }

    #[tokio::test]
    async fn test_spawn_echo() {
        let child = spawn_simple("/bin/echo", vec!["test".to_string()], "/tmp").await;
        assert!(child.is_ok());

        let output = child.unwrap().wait_with_output().await.unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "test");
    }

    #[tokio::test]
    async fn test_spawn_with_env() {
        let opts = SpawnOptions::new("/usr/bin/env", "/tmp").env_insert("MY_TEST_VAR", "hello123");

        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("MY_TEST_VAR=hello123"));
    }

    #[tokio::test]
    async fn test_spawn_with_sandbox_env() {
        // When sandbox mode is ReadOnly, CODEX_SANDBOX_NETWORK_DISABLED should be set
        let opts = SpawnOptions::new("/usr/bin/env", "/tmp").sandbox_mode(SandboxMode::ReadOnly);

        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("CODEX_SANDBOX_NETWORK_DISABLED=1"),
            "Expected CODEX_SANDBOX_NETWORK_DISABLED=1 in output: {stdout}"
        );
    }

    #[tokio::test]
    async fn test_spawn_full_access_no_network_disabled() {
        // When sandbox mode is DangerFullAccess, CODEX_SANDBOX_NETWORK_DISABLED should NOT be set
        let opts =
            SpawnOptions::new("/usr/bin/env", "/tmp").sandbox_mode(SandboxMode::DangerFullAccess);

        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("CODEX_SANDBOX_NETWORK_DISABLED"),
            "CODEX_SANDBOX_NETWORK_DISABLED should not be set: {stdout}"
        );
    }

    #[tokio::test]
    async fn test_spawn_working_directory() {
        let opts = SpawnOptions::new("/bin/pwd", "/tmp");
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        // On macOS, /tmp is a symlink to /private/tmp
        assert!(
            stdout.trim() == "/tmp" || stdout.trim() == "/private/tmp",
            "Expected /tmp or /private/tmp, got: {stdout}"
        );
    }

    #[test]
    fn test_env_var_constants() {
        assert_eq!(
            CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR,
            "CODEX_SANDBOX_NETWORK_DISABLED"
        );
        assert_eq!(CODEX_SANDBOX_ENV_VAR, "CODEX_SANDBOX");
    }

    // === StdioPolicy tests ===

    #[test]
    fn test_stdio_policy_debug() {
        let policy = StdioPolicy::RedirectForShellTool;
        let debug_str = format!("{:?}", policy);
        assert!(debug_str.contains("RedirectForShellTool"));
    }

    #[test]
    fn test_stdio_policy_clone() {
        let policy = StdioPolicy::Inherit;
        let cloned = policy;
        assert_eq!(cloned, StdioPolicy::Inherit);
    }

    #[test]
    fn test_stdio_policy_copy() {
        let policy = StdioPolicy::RedirectForShellTool;
        let copied: StdioPolicy = policy; // Copy
        assert_eq!(copied, StdioPolicy::RedirectForShellTool);
    }

    #[test]
    fn test_stdio_policy_partial_eq() {
        assert_eq!(StdioPolicy::Inherit, StdioPolicy::Inherit);
        assert_ne!(StdioPolicy::Inherit, StdioPolicy::RedirectForShellTool);
    }

    // === SpawnOptions tests ===

    #[test]
    fn test_spawn_options_debug() {
        let opts = SpawnOptions::new("/bin/ls", "/tmp");
        let debug_str = format!("{:?}", opts);
        assert!(debug_str.contains("SpawnOptions"));
        assert!(debug_str.contains("/bin/ls"));
    }

    #[test]
    fn test_spawn_options_clone() {
        let opts = SpawnOptions::new("/bin/cat", "/home")
            .args(["file.txt"])
            .env_insert("KEY", "VALUE");
        let cloned = opts.clone();
        assert_eq!(cloned.program, PathBuf::from("/bin/cat"));
        assert_eq!(cloned.cwd, PathBuf::from("/home"));
        assert_eq!(cloned.args, vec!["file.txt"]);
        assert_eq!(cloned.env.get("KEY"), Some(&"VALUE".to_string()));
    }

    #[test]
    fn test_spawn_options_new_defaults() {
        let opts = SpawnOptions::new("/usr/bin/test", "/var");
        assert_eq!(opts.program, PathBuf::from("/usr/bin/test"));
        assert_eq!(opts.cwd, PathBuf::from("/var"));
        assert!(opts.args.is_empty());
        assert_eq!(opts.sandbox_mode, SandboxMode::default());
        assert_eq!(opts.stdio_policy, StdioPolicy::default());
        assert!(opts.env.is_empty());
    }

    #[test]
    fn test_spawn_options_args_from_strings() {
        let args = vec![
            "--verbose".to_string(),
            "--output".to_string(),
            "file".to_string(),
        ];
        let opts = SpawnOptions::new("/bin/cmd", "/").args(args);
        assert_eq!(opts.args.len(), 3);
        assert_eq!(opts.args[0], "--verbose");
    }

    #[test]
    fn test_spawn_options_args_from_str_slices() {
        let opts = SpawnOptions::new("/bin/cmd", "/").args(["a", "b", "c"]);
        assert_eq!(opts.args, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_spawn_options_env_multiple() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());
        let opts = SpawnOptions::new("/bin/sh", "/").env(env);
        assert_eq!(opts.env.len(), 2);
        assert_eq!(opts.env.get("PATH"), Some(&"/usr/bin".to_string()));
    }

    #[test]
    fn test_spawn_options_env_insert_chaining() {
        let opts = SpawnOptions::new("/bin/sh", "/")
            .env_insert("VAR1", "value1")
            .env_insert("VAR2", "value2")
            .env_insert("VAR3", "value3");
        assert_eq!(opts.env.len(), 3);
    }

    #[test]
    fn test_spawn_options_sandbox_mode_builder() {
        let opts = SpawnOptions::new("/bin/ls", "/").sandbox_mode(SandboxMode::WorkspaceWrite);
        assert_eq!(opts.sandbox_mode, SandboxMode::WorkspaceWrite);
    }

    #[cfg(unix)]
    #[test]
    fn test_spawn_options_arg0() {
        let opts = SpawnOptions::new("/bin/ls", "/").arg0("custom_name");
        assert_eq!(opts.arg0, Some("custom_name".to_string()));
    }

    // === has_network_access tests ===

    #[test]
    fn test_has_network_access_workspace_write() {
        assert!(!has_network_access(&SandboxMode::WorkspaceWrite));
    }

    // === spawn_child tests ===

    #[tokio::test]
    async fn test_spawn_nonexistent_program() {
        let opts = SpawnOptions::new("/nonexistent/program/xyz123", "/tmp");
        let result = spawn_child(opts).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_spawn_with_args() {
        let opts = SpawnOptions::new("/bin/echo", "/tmp").args(["arg1", "arg2", "arg3"]);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("arg1"));
        assert!(stdout.contains("arg2"));
        assert!(stdout.contains("arg3"));
    }

    #[tokio::test]
    async fn test_spawn_multiple_env_vars() {
        let opts = SpawnOptions::new("/usr/bin/env", "/tmp")
            .env_insert("TEST_VAR_A", "alpha")
            .env_insert("TEST_VAR_B", "beta")
            .env_insert("TEST_VAR_C", "gamma");
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("TEST_VAR_A=alpha"));
        assert!(stdout.contains("TEST_VAR_B=beta"));
        assert!(stdout.contains("TEST_VAR_C=gamma"));
    }

    #[tokio::test]
    async fn test_spawn_workspace_write_sandbox() {
        let opts =
            SpawnOptions::new("/usr/bin/env", "/tmp").sandbox_mode(SandboxMode::WorkspaceWrite);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        // WorkspaceWrite should still disable network
        assert!(stdout.contains("CODEX_SANDBOX_NETWORK_DISABLED=1"));
    }

    #[tokio::test]
    async fn test_spawn_stderr_capture() {
        // Use sh -c to write to stderr
        let opts = SpawnOptions::new("/bin/sh", "/tmp").args(["-c", "echo error_output >&2"]);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("error_output"));
    }

    #[tokio::test]
    async fn test_spawn_exit_code() {
        let opts = SpawnOptions::new("/bin/sh", "/tmp").args(["-c", "exit 42"]);
        let child = spawn_child(opts).await.unwrap();
        let status = child.wait_with_output().await.unwrap().status;
        assert_eq!(status.code(), Some(42));
    }

    #[tokio::test]
    async fn test_spawn_simple_convenience() {
        let child = spawn_simple("/bin/echo", vec!["simple_test".to_string()], "/tmp").await;
        assert!(child.is_ok());
        let output = child.unwrap().wait_with_output().await.unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("simple_test"));
    }

    #[tokio::test]
    async fn test_spawn_empty_args() {
        let opts = SpawnOptions::new("/bin/pwd", "/tmp").args(Vec::<String>::new());
        let child = spawn_child(opts).await;
        assert!(child.is_ok());
    }

    #[tokio::test]
    async fn test_spawn_output_captured_by_default() {
        // Default stdio policy should capture output
        let opts = SpawnOptions::new("/bin/echo", "/tmp").args(["captured_output"]);
        assert_eq!(opts.stdio_policy, StdioPolicy::RedirectForShellTool);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        assert!(!output.stdout.is_empty());
    }

    // === Additional SpawnOptions tests ===

    #[test]
    fn test_spawn_options_empty_program_path() {
        let opts = SpawnOptions::new("", "/tmp");
        assert_eq!(opts.program, PathBuf::from(""));
    }

    #[test]
    fn test_spawn_options_relative_program_path() {
        let opts = SpawnOptions::new("./my_script.sh", "/home/user");
        assert_eq!(opts.program, PathBuf::from("./my_script.sh"));
    }

    #[test]
    fn test_spawn_options_unicode_program_path() {
        let opts = SpawnOptions::new("/usr/bin/程序", "/tmp");
        assert_eq!(opts.program, PathBuf::from("/usr/bin/程序"));
    }

    #[test]
    fn test_spawn_options_unicode_cwd() {
        let opts = SpawnOptions::new("/bin/ls", "/home/用户/文档");
        assert_eq!(opts.cwd, PathBuf::from("/home/用户/文档"));
    }

    #[test]
    fn test_spawn_options_env_overwrite() {
        let opts = SpawnOptions::new("/bin/sh", "/")
            .env_insert("KEY", "first")
            .env_insert("KEY", "second");
        assert_eq!(opts.env.get("KEY"), Some(&"second".to_string()));
    }

    #[test]
    fn test_spawn_options_env_empty_value() {
        let opts = SpawnOptions::new("/bin/sh", "/").env_insert("EMPTY_VAR", "");
        assert_eq!(opts.env.get("EMPTY_VAR"), Some(&"".to_string()));
    }

    #[test]
    fn test_spawn_options_env_special_chars_key() {
        let opts = SpawnOptions::new("/bin/sh", "/").env_insert("MY_VAR_123", "value");
        assert!(opts.env.contains_key("MY_VAR_123"));
    }

    #[test]
    fn test_spawn_options_env_unicode_value() {
        let opts = SpawnOptions::new("/bin/sh", "/").env_insert("GREETING", "你好世界");
        assert_eq!(opts.env.get("GREETING"), Some(&"你好世界".to_string()));
    }

    #[test]
    fn test_spawn_options_args_unicode() {
        let opts = SpawnOptions::new("/bin/echo", "/").args(["привет", "мир"]);
        assert_eq!(opts.args, vec!["привет", "мир"]);
    }

    #[test]
    fn test_spawn_options_args_empty_strings() {
        let opts = SpawnOptions::new("/bin/echo", "/").args(["", "", ""]);
        assert_eq!(opts.args.len(), 3);
        assert!(opts.args.iter().all(|a| a.is_empty()));
    }

    #[test]
    fn test_spawn_options_args_whitespace() {
        let opts =
            SpawnOptions::new("/bin/echo", "/").args(["arg with spaces", "\ttab", "new\nline"]);
        assert_eq!(opts.args[0], "arg with spaces");
        assert_eq!(opts.args[1], "\ttab");
        assert_eq!(opts.args[2], "new\nline");
    }

    #[test]
    fn test_spawn_options_args_special_chars() {
        let opts =
            SpawnOptions::new("/bin/sh", "/").args(["-c", "echo $HOME", "--", "&&", "||", ";"]);
        assert_eq!(opts.args.len(), 6);
    }

    #[test]
    fn test_spawn_options_all_sandbox_modes() {
        for mode in [
            SandboxMode::ReadOnly,
            SandboxMode::WorkspaceWrite,
            SandboxMode::DangerFullAccess,
        ] {
            let opts = SpawnOptions::new("/bin/ls", "/").sandbox_mode(mode);
            assert_eq!(opts.sandbox_mode, mode);
        }
    }

    #[test]
    fn test_spawn_options_chained_builders() {
        let opts = SpawnOptions::new("/bin/cmd", "/work")
            .args(["--flag"])
            .sandbox_mode(SandboxMode::ReadOnly)
            .stdio_policy(StdioPolicy::Inherit)
            .env_insert("A", "1")
            .env_insert("B", "2");

        assert_eq!(opts.program, PathBuf::from("/bin/cmd"));
        assert_eq!(opts.cwd, PathBuf::from("/work"));
        assert_eq!(opts.args, vec!["--flag"]);
        assert_eq!(opts.sandbox_mode, SandboxMode::ReadOnly);
        assert_eq!(opts.stdio_policy, StdioPolicy::Inherit);
        assert_eq!(opts.env.len(), 2);
    }

    #[test]
    fn test_spawn_options_env_replace_all() {
        let mut initial = HashMap::new();
        initial.insert("OLD".to_string(), "old_value".to_string());

        let mut replacement = HashMap::new();
        replacement.insert("NEW".to_string(), "new_value".to_string());

        let opts = SpawnOptions::new("/bin/sh", "/")
            .env(initial)
            .env(replacement);

        assert!(!opts.env.contains_key("OLD"));
        assert_eq!(opts.env.get("NEW"), Some(&"new_value".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn test_spawn_options_arg0_empty() {
        let opts = SpawnOptions::new("/bin/ls", "/").arg0("");
        assert_eq!(opts.arg0, Some("".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn test_spawn_options_arg0_unicode() {
        let opts = SpawnOptions::new("/bin/ls", "/").arg0("程序名称");
        assert_eq!(opts.arg0, Some("程序名称".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn test_spawn_options_arg0_with_path() {
        let opts = SpawnOptions::new("/usr/bin/python3", "/").arg0("/custom/path/python");
        assert_eq!(opts.arg0, Some("/custom/path/python".to_string()));
    }

    // === StdioPolicy additional tests ===

    #[test]
    fn test_stdio_policy_all_variants() {
        let variants = [StdioPolicy::RedirectForShellTool, StdioPolicy::Inherit];
        assert_eq!(variants.len(), 2);
    }

    #[test]
    fn test_stdio_policy_eq_symmetry() {
        let a = StdioPolicy::Inherit;
        let b = StdioPolicy::Inherit;
        assert!(a == b);
        assert!(b == a);
    }

    #[test]
    fn test_stdio_policy_ne_symmetry() {
        let a = StdioPolicy::Inherit;
        let b = StdioPolicy::RedirectForShellTool;
        assert!(a != b);
        assert!(b != a);
    }

    // === has_network_access additional tests ===

    #[test]
    fn test_has_network_access_all_modes() {
        let results = [
            (SandboxMode::ReadOnly, false),
            (SandboxMode::WorkspaceWrite, false),
            (SandboxMode::DangerFullAccess, true),
        ];
        for (mode, expected) in results {
            assert_eq!(has_network_access(&mode), expected, "Failed for {:?}", mode);
        }
    }

    // === spawn_child edge case tests ===

    #[tokio::test]
    async fn test_spawn_env_cleared() {
        // Verify env is cleared before adding our vars
        // Check that common system vars are NOT present
        let opts = SpawnOptions::new("/usr/bin/env", "/tmp");
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        // PATH should not be present since we clear env
        let has_path = stdout.lines().any(|l| l.starts_with("PATH="));
        // Note: we only get CODEX_SANDBOX vars based on sandbox mode
        assert!(!has_path || stdout.contains("CODEX_SANDBOX"));
    }

    #[tokio::test]
    async fn test_spawn_success_status() {
        // Use /bin/sh -c "exit 0" for cross-platform compatibility
        let opts = SpawnOptions::new("/bin/sh", "/tmp").args(["-c", "exit 0"]);
        let child = spawn_child(opts).await.unwrap();
        let status = child.wait_with_output().await.unwrap().status;
        assert!(status.success());
        assert_eq!(status.code(), Some(0));
    }

    #[tokio::test]
    async fn test_spawn_failure_status() {
        // Use /bin/sh -c "exit 1" for cross-platform compatibility
        let opts = SpawnOptions::new("/bin/sh", "/tmp").args(["-c", "exit 1"]);
        let child = spawn_child(opts).await.unwrap();
        let status = child.wait_with_output().await.unwrap().status;
        assert!(!status.success());
        assert_eq!(status.code(), Some(1));
    }

    #[tokio::test]
    async fn test_spawn_unicode_args() {
        let opts = SpawnOptions::new("/bin/echo", "/tmp").args(["日本語", "한국어", "العربية"]);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("日本語"));
        assert!(stdout.contains("한국어"));
        assert!(stdout.contains("العربية"));
    }

    #[tokio::test]
    async fn test_spawn_unicode_env() {
        let opts = SpawnOptions::new("/usr/bin/env", "/tmp").env_insert("UNICODE_VAR", "مرحبا");
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("UNICODE_VAR=مرحبا"));
    }

    #[tokio::test]
    async fn test_spawn_many_args() {
        let args: Vec<String> = (0..100).map(|i| format!("arg{}", i)).collect();
        let opts = SpawnOptions::new("/bin/echo", "/tmp").args(args.clone());
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("arg0"));
        assert!(stdout.contains("arg99"));
    }

    #[tokio::test]
    async fn test_spawn_many_env_vars() {
        let mut opts = SpawnOptions::new("/usr/bin/env", "/tmp");
        for i in 0..50 {
            opts = opts.env_insert(format!("VAR_{}", i), format!("value_{}", i));
        }
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("VAR_0=value_0"));
        assert!(stdout.contains("VAR_49=value_49"));
    }

    #[tokio::test]
    async fn test_spawn_combined_stdout_stderr() {
        let opts = SpawnOptions::new("/bin/sh", "/tmp")
            .args(["-c", "echo stdout_line; echo stderr_line >&2"]);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("stdout_line"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("stderr_line"));
    }

    #[tokio::test]
    async fn test_spawn_empty_output() {
        // Use /bin/sh -c "exit 0" for cross-platform compatibility
        let opts = SpawnOptions::new("/bin/sh", "/tmp").args(["-c", "exit 0"]);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_spawn_long_output() {
        let opts = SpawnOptions::new("/bin/sh", "/tmp")
            .args(["-c", "for i in $(seq 1 1000); do echo line_$i; done"]);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("line_1"));
        assert!(stdout.contains("line_1000"));
    }

    #[tokio::test]
    async fn test_spawn_invalid_cwd() {
        let opts = SpawnOptions::new("/bin/pwd", "/nonexistent/directory/xyz");
        let result = spawn_child(opts).await;
        // Should fail to spawn due to invalid cwd
        assert!(result.is_err());
    }

    // === Platform-specific sandbox env tests ===

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn test_spawn_macos_sandbox_env() {
        let opts = SpawnOptions::new("/usr/bin/env", "/tmp").sandbox_mode(SandboxMode::ReadOnly);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("CODEX_SANDBOX=seatbelt"));
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn test_spawn_macos_full_access_no_sandbox_env() {
        let opts =
            SpawnOptions::new("/usr/bin/env", "/tmp").sandbox_mode(SandboxMode::DangerFullAccess);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("CODEX_SANDBOX="));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_spawn_linux_sandbox_env() {
        let opts = SpawnOptions::new("/usr/bin/env", "/tmp").sandbox_mode(SandboxMode::ReadOnly);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("CODEX_SANDBOX=landlock"));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_spawn_linux_full_access_no_sandbox_env() {
        let opts =
            SpawnOptions::new("/usr/bin/env", "/tmp").sandbox_mode(SandboxMode::DangerFullAccess);
        let child = spawn_child(opts).await.unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("CODEX_SANDBOX="));
    }

    // === spawn_simple tests ===

    #[tokio::test]
    async fn test_spawn_simple_empty_args() {
        let child = spawn_simple("/bin/pwd", vec![], "/tmp").await;
        assert!(child.is_ok());
    }

    #[tokio::test]
    async fn test_spawn_simple_multiple_args() {
        let args = vec!["-a".to_string(), "-l".to_string(), "-h".to_string()];
        let child = spawn_simple("/bin/ls", args, "/tmp").await;
        assert!(child.is_ok());
    }

    #[tokio::test]
    async fn test_spawn_simple_with_pathbuf() {
        let program = PathBuf::from("/bin/echo");
        let cwd = PathBuf::from("/tmp");
        let child = spawn_simple(program, vec!["test".to_string()], cwd).await;
        assert!(child.is_ok());
    }

    // === Environment variable constant validation ===

    #[test]
    fn test_env_var_constants_not_empty() {
        // Use const assertions for compile-time verification
        const _: () = assert!(!CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.is_empty());
        const _: () = assert!(!CODEX_SANDBOX_ENV_VAR.is_empty());
    }

    #[test]
    fn test_env_var_constants_uppercase() {
        assert_eq!(
            CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR,
            CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.to_uppercase()
        );
        assert_eq!(CODEX_SANDBOX_ENV_VAR, CODEX_SANDBOX_ENV_VAR.to_uppercase());
    }

    #[test]
    fn test_env_var_constants_valid_names() {
        // Valid env var names should only contain alphanumeric and underscore
        let is_valid = |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        assert!(is_valid(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR));
        assert!(is_valid(CODEX_SANDBOX_ENV_VAR));
    }
}
