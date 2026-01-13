//! Command execution with timeout and output capture
//!
//! This module provides command execution with:
//! - Configurable timeouts or cancellation
//! - Stdout/stderr capture and streaming
//! - Aggregated output collection
//! - Sandbox denial detection
//! - Process group termination on timeout (Unix)

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use async_channel::Sender;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;

use crate::text_encoding::bytes_to_string_smart;
use crate::Result;

/// Default timeout for command execution in milliseconds.
pub const DEFAULT_EXEC_COMMAND_TIMEOUT_MS: u64 = 10_000;

/// Exit code used when command times out.
pub const EXEC_TIMEOUT_EXIT_CODE: i32 = 124;

/// Max output delta events per exec call.
pub const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

// Signal codes
const SIGKILL_CODE: i32 = 9;
const TIMEOUT_CODE: i32 = 64;
const EXIT_CODE_SIGNAL_BASE: i32 = 128;

// I/O buffer sizing
const READ_CHUNK_SIZE: usize = 8192;
const AGGREGATE_BUFFER_INITIAL_CAPACITY: usize = 8 * 1024;
const IO_DRAIN_TIMEOUT_MS: u64 = 2_000;

/// Parameters for command execution.
#[derive(Debug, Clone)]
pub struct ExecParams {
    /// Command and arguments
    pub command: Vec<String>,
    /// Working directory
    pub cwd: PathBuf,
    /// Timeout/cancellation mechanism
    pub expiration: ExecExpiration,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Override arg0 (program name)
    pub arg0: Option<String>,
}

impl ExecParams {
    /// Create new exec params with command and cwd.
    pub fn new(command: Vec<String>, cwd: PathBuf) -> Self {
        Self {
            command,
            cwd,
            expiration: ExecExpiration::DefaultTimeout,
            env: HashMap::new(),
            arg0: None,
        }
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.expiration = ExecExpiration::Timeout(Duration::from_millis(timeout_ms));
        self
    }

    /// Set environment variables.
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Set cancellation token.
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.expiration = ExecExpiration::Cancellation(token);
        self
    }
}

/// Mechanism to terminate an exec invocation.
#[derive(Debug, Clone)]
pub enum ExecExpiration {
    /// Fixed timeout duration
    Timeout(Duration),
    /// Use default timeout
    DefaultTimeout,
    /// Wait for cancellation token
    Cancellation(CancellationToken),
}

impl ExecExpiration {
    async fn wait(&self) {
        match self {
            ExecExpiration::Timeout(duration) => tokio::time::sleep(*duration).await,
            ExecExpiration::DefaultTimeout => {
                tokio::time::sleep(Duration::from_millis(DEFAULT_EXEC_COMMAND_TIMEOUT_MS)).await
            }
            ExecExpiration::Cancellation(cancel) => {
                cancel.cancelled().await;
            }
        }
    }

    /// Get timeout in milliseconds if applicable.
    pub fn timeout_ms(&self) -> Option<u64> {
        match self {
            ExecExpiration::Timeout(d) => Some(d.as_millis() as u64),
            ExecExpiration::DefaultTimeout => Some(DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
            ExecExpiration::Cancellation(_) => None,
        }
    }
}

impl From<Option<u64>> for ExecExpiration {
    fn from(timeout_ms: Option<u64>) -> Self {
        timeout_ms.map_or(ExecExpiration::DefaultTimeout, |ms| {
            ExecExpiration::Timeout(Duration::from_millis(ms))
        })
    }
}

impl From<u64> for ExecExpiration {
    fn from(timeout_ms: u64) -> Self {
        ExecExpiration::Timeout(Duration::from_millis(timeout_ms))
    }
}

/// Sandbox type for execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SandboxType {
    /// No sandboxing
    #[default]
    None,
    /// macOS Seatbelt sandbox
    MacosSeatbelt,
    /// Linux seccomp/Landlock
    LinuxSeccomp,
    /// Windows restricted token
    WindowsRestrictedToken,
}

/// Stream output with optional truncation info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOutput<T: Clone> {
    /// The output content
    pub text: T,
    /// Line count if truncated
    pub truncated_after_lines: Option<u32>,
}

impl<T: Clone> StreamOutput<T> {
    /// Create new stream output.
    pub fn new(text: T) -> Self {
        Self {
            text,
            truncated_after_lines: None,
        }
    }
}

impl StreamOutput<Vec<u8>> {
    /// Convert bytes to string with encoding detection.
    pub fn to_string_lossy(&self) -> StreamOutput<String> {
        StreamOutput {
            text: bytes_to_string_smart(&self.text),
            truncated_after_lines: self.truncated_after_lines,
        }
    }
}

/// Output from command execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecOutput {
    /// Exit code
    pub exit_code: i32,
    /// Standard output
    pub stdout: StreamOutput<String>,
    /// Standard error
    pub stderr: StreamOutput<String>,
    /// Combined output in order received
    pub aggregated_output: StreamOutput<String>,
    /// How long execution took
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    /// Whether command timed out
    pub timed_out: bool,
}

mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ms = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(ms))
    }
}

impl Default for ExecOutput {
    fn default() -> Self {
        Self {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(String::new()),
            duration: Duration::ZERO,
            timed_out: false,
        }
    }
}

impl ExecOutput {
    /// Check if command succeeded (exit code 0).
    pub fn success(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }
}

/// Output stream sender for real-time output.
#[derive(Clone)]
pub struct OutputStreamSender {
    /// Submission ID
    pub sub_id: String,
    /// Call ID
    pub call_id: String,
    /// Channel sender for output chunks
    pub tx: Sender<OutputChunk>,
}

/// Chunk of output from exec.
#[derive(Debug, Clone)]
pub struct OutputChunk {
    /// Which stream this came from
    pub stream: OutputStream,
    /// The data
    pub data: Vec<u8>,
}

/// Output stream type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    /// Standard output
    Stdout,
    /// Standard error
    Stderr,
}

/// Execute a command with the given parameters.
pub async fn execute(params: ExecParams) -> Result<ExecOutput> {
    execute_with_stream(params, None).await
}

/// Execute a command with optional output streaming.
pub async fn execute_with_stream(
    params: ExecParams,
    stream: Option<OutputStreamSender>,
) -> Result<ExecOutput> {
    let ExecParams {
        command,
        cwd,
        env,
        arg0,
        expiration,
    } = params;

    let (program, args) = command.split_first().ok_or_else(|| {
        crate::Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command args are empty",
        ))
    })?;

    let start = Instant::now();

    // Build command
    let mut cmd = Command::new(program);
    // Note: arg0 override is not supported by tokio::process::Command.
    // The field is kept for API compatibility but ignored at runtime.
    #[allow(unused_variables)]
    let _ = &arg0;
    cmd.args(args);
    cmd.current_dir(&cwd);
    cmd.envs(&env);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Start new process group on Unix for clean termination
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            // Create new process group
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let child = cmd.spawn().map_err(crate::Error::Io)?;

    let raw_output = consume_output(child, expiration, stream).await?;
    let duration = start.elapsed();

    finalize_output(raw_output, duration)
}

/// Raw output before string conversion.
struct RawExecOutput {
    exit_status: ExitStatus,
    stdout: StreamOutput<Vec<u8>>,
    stderr: StreamOutput<Vec<u8>>,
    aggregated: StreamOutput<Vec<u8>>,
    timed_out: bool,
}

/// Consume and collect output from child process.
async fn consume_output(
    mut child: Child,
    expiration: ExecExpiration,
    stream: Option<OutputStreamSender>,
) -> Result<RawExecOutput> {
    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| crate::Error::Io(io::Error::other("stdout pipe not available")))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| crate::Error::Io(io::Error::other("stderr pipe not available")))?;

    // Channel for aggregated output
    let (agg_tx, agg_rx) = async_channel::unbounded::<Vec<u8>>();

    // Spawn readers
    let stdout_handle = tokio::spawn(read_stream(
        BufReader::new(stdout_pipe),
        stream.clone(),
        OutputStream::Stdout,
        Some(agg_tx.clone()),
    ));
    let stderr_handle = tokio::spawn(read_stream(
        BufReader::new(stderr_pipe),
        stream,
        OutputStream::Stderr,
        Some(agg_tx.clone()),
    ));

    // Wait for exit or expiration
    let (exit_status, timed_out) = tokio::select! {
        status = child.wait() => {
            let status = status.map_err(crate::Error::Io)?;
            (status, false)
        }
        _ = expiration.wait() => {
            kill_child_and_group(&mut child)?;
            (synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE), true)
        }
        _ = tokio::signal::ctrl_c() => {
            kill_child_and_group(&mut child)?;
            (synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE), false)
        }
    };

    // Collect output with timeout
    let stdout = await_with_timeout(
        &mut tokio::spawn(async move {
            stdout_handle
                .await
                .unwrap_or_else(|_| Ok(StreamOutput::new(vec![])))
        }),
        Duration::from_millis(IO_DRAIN_TIMEOUT_MS),
    )
    .await?;

    let stderr = await_with_timeout(
        &mut tokio::spawn(async move {
            stderr_handle
                .await
                .unwrap_or_else(|_| Ok(StreamOutput::new(vec![])))
        }),
        Duration::from_millis(IO_DRAIN_TIMEOUT_MS),
    )
    .await?;

    drop(agg_tx);

    // Collect aggregated output
    let mut combined = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    while let Ok(chunk) = agg_rx.recv().await {
        combined.extend_from_slice(&chunk);
    }

    Ok(RawExecOutput {
        exit_status,
        stdout,
        stderr,
        aggregated: StreamOutput::new(combined),
        timed_out,
    })
}

/// Read from a stream and optionally forward to sender.
async fn read_stream<R: AsyncRead + Unpin + Send>(
    mut reader: R,
    stream: Option<OutputStreamSender>,
    output_type: OutputStream,
    agg_tx: Option<Sender<Vec<u8>>>,
) -> io::Result<StreamOutput<Vec<u8>>> {
    let mut buf = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    let mut tmp = [0u8; READ_CHUNK_SIZE];
    let mut emitted = 0;

    loop {
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            break;
        }

        let chunk = tmp[..n].to_vec();

        // Send to stream if available
        if let Some(ref s) = stream {
            if emitted < MAX_EXEC_OUTPUT_DELTAS_PER_CALL {
                let _ =
                    s.tx.send(OutputChunk {
                        stream: output_type,
                        data: chunk.clone(),
                    })
                    .await;
                emitted += 1;
            }
        }

        // Send to aggregator
        if let Some(ref tx) = agg_tx {
            let _ = tx.send(chunk.clone()).await;
        }

        buf.extend_from_slice(&chunk);
    }

    Ok(StreamOutput::new(buf))
}

/// Await a task with timeout, returning empty on timeout.
async fn await_with_timeout(
    handle: &mut tokio::task::JoinHandle<io::Result<StreamOutput<Vec<u8>>>>,
    timeout: Duration,
) -> io::Result<StreamOutput<Vec<u8>>> {
    match tokio::time::timeout(timeout, &mut *handle).await {
        Ok(Ok(Ok(output))) => Ok(output),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(_)) => Ok(StreamOutput::new(vec![])),
        Err(_) => {
            handle.abort();
            Ok(StreamOutput::new(vec![]))
        }
    }
}

/// Convert raw output to final output.
fn finalize_output(raw: RawExecOutput, duration: Duration) -> Result<ExecOutput> {
    let mut exit_code = raw.exit_status.code().unwrap_or(-1);
    let mut timed_out = raw.timed_out;

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = raw.exit_status.signal() {
            if signal == TIMEOUT_CODE {
                timed_out = true;
            }
        }
    }

    if timed_out {
        exit_code = EXEC_TIMEOUT_EXIT_CODE;
    }

    Ok(ExecOutput {
        exit_code,
        stdout: raw.stdout.to_string_lossy(),
        stderr: raw.stderr.to_string_lossy(),
        aggregated_output: raw.aggregated.to_string_lossy(),
        duration,
        timed_out,
    })
}

/// Kill child and its process group on Unix.
fn kill_child_and_group(child: &mut Child) -> io::Result<()> {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            let pid = pid as libc::pid_t;
            let pgid = unsafe { libc::getpgid(pid) };
            if pgid != -1 {
                unsafe { libc::killpg(pgid, libc::SIGKILL) };
            }
        }
    }
    let _ = child.start_kill();
    Ok(())
}

#[cfg(unix)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}

#[cfg(windows)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code as u32)
}

/// Check if output indicates sandbox denial.
pub fn is_likely_sandbox_denied(sandbox_type: SandboxType, output: &ExecOutput) -> bool {
    if sandbox_type == SandboxType::None || output.exit_code == 0 {
        return false;
    }

    const SANDBOX_KEYWORDS: [&str; 7] = [
        "operation not permitted",
        "permission denied",
        "read-only file system",
        "seccomp",
        "sandbox",
        "landlock",
        "failed to write file",
    ];

    // Check for keywords
    let has_keyword = [
        &output.stderr.text,
        &output.stdout.text,
        &output.aggregated_output.text,
    ]
    .into_iter()
    .any(|text| {
        let lower = text.to_lowercase();
        SANDBOX_KEYWORDS.iter().any(|kw| lower.contains(kw))
    });

    if has_keyword {
        return true;
    }

    // Quick reject common non-sandbox exit codes
    const QUICK_REJECT: [i32; 3] = [2, 126, 127];
    if QUICK_REJECT.contains(&output.exit_code) {
        return false;
    }

    #[cfg(unix)]
    {
        const SIGSYS: i32 = 31; // libc::SIGSYS
        if sandbox_type == SandboxType::LinuxSeccomp
            && output.exit_code == EXIT_CODE_SIGNAL_BASE + SIGSYS
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_output(exit_code: i32, stdout: &str, stderr: &str, aggregated: &str) -> ExecOutput {
        ExecOutput {
            exit_code,
            stdout: StreamOutput::new(stdout.to_string()),
            stderr: StreamOutput::new(stderr.to_string()),
            aggregated_output: StreamOutput::new(aggregated.to_string()),
            duration: Duration::from_millis(1),
            timed_out: false,
        }
    }

    #[test]
    fn test_exec_params_new() {
        let params = ExecParams::new(
            vec!["echo".to_string(), "hello".to_string()],
            PathBuf::from("/tmp"),
        );
        assert_eq!(params.command.len(), 2);
        assert_eq!(params.cwd, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_exec_params_with_timeout() {
        let params = ExecParams::new(vec!["ls".to_string()], PathBuf::from(".")).with_timeout(5000);
        assert_eq!(params.expiration.timeout_ms(), Some(5000));
    }

    #[test]
    fn test_exec_expiration_from_option() {
        let exp: ExecExpiration = Some(1000u64).into();
        assert_eq!(exp.timeout_ms(), Some(1000));

        let exp: ExecExpiration = None.into();
        assert_eq!(exp.timeout_ms(), Some(DEFAULT_EXEC_COMMAND_TIMEOUT_MS));
    }

    #[test]
    fn test_exec_output_success() {
        let output = ExecOutput::default();
        assert!(output.success());

        let mut failed = output.clone();
        failed.exit_code = 1;
        assert!(!failed.success());

        let timed_out = ExecOutput {
            timed_out: true,
            ..Default::default()
        };
        assert!(!timed_out.success());
    }

    #[test]
    fn test_sandbox_detection_no_sandbox() {
        let output = make_output(1, "", "error", "");
        assert!(!is_likely_sandbox_denied(SandboxType::None, &output));
    }

    #[test]
    fn test_sandbox_detection_success_exit() {
        let output = make_output(0, "", "", "");
        assert!(!is_likely_sandbox_denied(
            SandboxType::LinuxSeccomp,
            &output
        ));
    }

    #[test]
    fn test_sandbox_detection_keyword_stderr() {
        let output = make_output(1, "", "Operation not permitted", "");
        assert!(is_likely_sandbox_denied(SandboxType::LinuxSeccomp, &output));
    }

    #[test]
    fn test_sandbox_detection_keyword_aggregated() {
        let output = make_output(101, "", "", "Read-only file system");
        assert!(is_likely_sandbox_denied(
            SandboxType::MacosSeatbelt,
            &output
        ));
    }

    #[test]
    fn test_sandbox_detection_quick_reject() {
        let output = make_output(127, "", "command not found", "");
        assert!(!is_likely_sandbox_denied(
            SandboxType::LinuxSeccomp,
            &output
        ));
    }

    #[test]
    fn test_stream_output_new() {
        let output = StreamOutput::new("hello".to_string());
        assert_eq!(output.text, "hello");
        assert!(output.truncated_after_lines.is_none());
    }

    #[test]
    fn test_stream_output_bytes_to_string() {
        let bytes = StreamOutput::new(b"hello world".to_vec());
        let string = bytes.to_string_lossy();
        assert_eq!(string.text, "hello world");
    }

    #[test]
    fn test_exec_output_serialization() {
        let output = ExecOutput::default();
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("exit_code"));
        assert!(json.contains("duration"));
    }

    #[tokio::test]
    async fn test_execute_simple_command() {
        let params = ExecParams::new(
            vec!["echo".to_string(), "hello".to_string()],
            std::env::current_dir().unwrap(),
        )
        .with_timeout(5000);

        let result = execute(params).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.text.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_with_env() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        #[cfg(unix)]
        let params = ExecParams::new(
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "echo $TEST_VAR".to_string(),
            ],
            std::env::current_dir().unwrap(),
        )
        .with_env(env)
        .with_timeout(5000);

        #[cfg(windows)]
        let params = ExecParams::new(
            vec![
                "cmd".to_string(),
                "/c".to_string(),
                "echo %TEST_VAR%".to_string(),
            ],
            std::env::current_dir().unwrap(),
        )
        .with_env(env)
        .with_timeout(5000);

        let result = execute(params).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.text.contains("test_value"));
    }

    #[tokio::test]
    async fn test_execute_empty_command() {
        let params = ExecParams::new(vec![], std::env::current_dir().unwrap());
        let result = execute(params).await;
        assert!(result.is_err());
    }

    // === Constants tests ===

    #[test]
    fn test_default_exec_command_timeout_ms() {
        assert_eq!(DEFAULT_EXEC_COMMAND_TIMEOUT_MS, 10_000);
    }

    #[test]
    fn test_exec_timeout_exit_code() {
        assert_eq!(EXEC_TIMEOUT_EXIT_CODE, 124);
    }

    #[test]
    fn test_max_exec_output_deltas() {
        assert_eq!(MAX_EXEC_OUTPUT_DELTAS_PER_CALL, 10_000);
    }

    // === ExecParams tests ===

    #[test]
    fn test_exec_params_debug() {
        let params = ExecParams::new(vec!["echo".to_string()], PathBuf::from("/tmp"));
        let debug_str = format!("{:?}", params);
        assert!(debug_str.contains("ExecParams"));
        assert!(debug_str.contains("echo"));
    }

    #[test]
    fn test_exec_params_clone() {
        let params = ExecParams::new(vec!["ls".to_string()], PathBuf::from("/tmp"));
        let cloned = params.clone();
        assert_eq!(cloned.command, vec!["ls"]);
        assert_eq!(cloned.cwd, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_exec_params_default_expiration() {
        let params = ExecParams::new(vec!["ls".to_string()], PathBuf::from("/tmp"));
        // Should have default timeout
        assert_eq!(
            params.expiration.timeout_ms(),
            Some(DEFAULT_EXEC_COMMAND_TIMEOUT_MS)
        );
    }

    #[test]
    fn test_exec_params_with_env_multiple() {
        let mut env = HashMap::new();
        env.insert("A".to_string(), "1".to_string());
        env.insert("B".to_string(), "2".to_string());
        let params = ExecParams::new(vec!["test".to_string()], PathBuf::from(".")).with_env(env);
        assert_eq!(params.env.len(), 2);
        assert_eq!(params.env.get("A"), Some(&"1".to_string()));
    }

    #[test]
    fn test_exec_params_with_cancellation() {
        let token = CancellationToken::new();
        let params =
            ExecParams::new(vec!["test".to_string()], PathBuf::from(".")).with_cancellation(token);
        // Cancellation has no timeout
        assert_eq!(params.expiration.timeout_ms(), None);
    }

    // === ExecExpiration tests ===

    #[test]
    fn test_exec_expiration_debug() {
        let exp = ExecExpiration::Timeout(Duration::from_secs(5));
        let debug_str = format!("{:?}", exp);
        assert!(debug_str.contains("Timeout"));
    }

    #[test]
    fn test_exec_expiration_clone() {
        let exp = ExecExpiration::DefaultTimeout;
        let cloned = exp.clone();
        assert_eq!(cloned.timeout_ms(), Some(DEFAULT_EXEC_COMMAND_TIMEOUT_MS));
    }

    #[test]
    fn test_exec_expiration_timeout_ms_variants() {
        let timeout = ExecExpiration::Timeout(Duration::from_millis(3000));
        assert_eq!(timeout.timeout_ms(), Some(3000));

        let default_timeout = ExecExpiration::DefaultTimeout;
        assert_eq!(
            default_timeout.timeout_ms(),
            Some(DEFAULT_EXEC_COMMAND_TIMEOUT_MS)
        );

        let cancel = ExecExpiration::Cancellation(CancellationToken::new());
        assert_eq!(cancel.timeout_ms(), None);
    }

    #[test]
    fn test_exec_expiration_from_u64() {
        let exp: ExecExpiration = 5000u64.into();
        assert_eq!(exp.timeout_ms(), Some(5000));
    }

    // === SandboxType tests ===

    #[test]
    fn test_sandbox_type_default() {
        let sandbox = SandboxType::default();
        assert_eq!(sandbox, SandboxType::None);
    }

    #[test]
    fn test_sandbox_type_clone_copy() {
        let sandbox = SandboxType::MacosSeatbelt;
        let cloned = sandbox;
        let copied: SandboxType = sandbox; // Copy
        assert_eq!(cloned, SandboxType::MacosSeatbelt);
        assert_eq!(copied, SandboxType::MacosSeatbelt);
    }

    #[test]
    fn test_sandbox_type_debug() {
        let sandbox = SandboxType::LinuxSeccomp;
        let debug_str = format!("{:?}", sandbox);
        assert!(debug_str.contains("LinuxSeccomp"));
    }

    #[test]
    fn test_sandbox_type_partial_eq() {
        assert_eq!(SandboxType::None, SandboxType::None);
        assert_ne!(SandboxType::None, SandboxType::MacosSeatbelt);
        assert_ne!(
            SandboxType::LinuxSeccomp,
            SandboxType::WindowsRestrictedToken
        );
    }

    // === StreamOutput tests ===

    #[test]
    fn test_stream_output_debug() {
        let output = StreamOutput::new("test".to_string());
        let debug_str = format!("{:?}", output);
        assert!(debug_str.contains("StreamOutput"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_stream_output_clone() {
        let output = StreamOutput::new("data".to_string());
        let cloned = output.clone();
        assert_eq!(cloned.text, "data");
    }

    #[test]
    fn test_stream_output_with_truncation() {
        let mut output = StreamOutput::new("content".to_string());
        output.truncated_after_lines = Some(100);
        assert_eq!(output.truncated_after_lines, Some(100));
    }

    #[test]
    fn test_stream_output_bytes_to_string_invalid_utf8() {
        let invalid_utf8 = vec![0xFF, 0xFE, b'h', b'i'];
        let bytes_output = StreamOutput::new(invalid_utf8);
        let string_output = bytes_output.to_string_lossy();
        // Should handle invalid UTF-8 gracefully (may use replacement chars)
        assert!(!string_output.text.is_empty());
    }

    #[test]
    fn test_stream_output_serialization() {
        let output = StreamOutput::new("hello".to_string());
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("hello"));
        assert!(json.contains("text"));
    }

    // === ExecOutput tests ===

    #[test]
    fn test_exec_output_debug() {
        let output = ExecOutput::default();
        let debug_str = format!("{:?}", output);
        assert!(debug_str.contains("ExecOutput"));
    }

    #[test]
    fn test_exec_output_clone() {
        let output = ExecOutput::default();
        let cloned = output.clone();
        assert_eq!(cloned.exit_code, 0);
    }

    #[test]
    fn test_exec_output_default_values() {
        let output = ExecOutput::default();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.text.is_empty());
        assert!(output.stderr.text.is_empty());
        assert_eq!(output.duration, Duration::ZERO);
        assert!(!output.timed_out);
    }

    #[test]
    fn test_exec_output_success_both_conditions() {
        let mut output = ExecOutput::default();
        assert!(output.success()); // exit_code=0, timed_out=false

        output.exit_code = 0;
        output.timed_out = true;
        assert!(!output.success()); // exit_code=0 but timed out

        output.exit_code = 1;
        output.timed_out = false;
        assert!(!output.success()); // non-zero exit code
    }

    #[test]
    fn test_exec_output_deserialization() {
        let json = r#"{
            "exit_code": 42,
            "stdout": {"text": "out", "truncated_after_lines": null},
            "stderr": {"text": "err", "truncated_after_lines": null},
            "aggregated_output": {"text": "agg", "truncated_after_lines": null},
            "duration": 1000,
            "timed_out": true
        }"#;
        let output: ExecOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.exit_code, 42);
        assert_eq!(output.stdout.text, "out");
        assert_eq!(output.stderr.text, "err");
        assert_eq!(output.duration, Duration::from_millis(1000));
        assert!(output.timed_out);
    }

    // === OutputChunk and OutputStream tests ===

    #[test]
    fn test_output_stream_debug() {
        let stream = OutputStream::Stdout;
        let debug_str = format!("{:?}", stream);
        assert!(debug_str.contains("Stdout"));
    }

    #[test]
    fn test_output_stream_clone_copy() {
        let stream = OutputStream::Stderr;
        let cloned = stream;
        let copied: OutputStream = stream;
        assert_eq!(cloned, OutputStream::Stderr);
        assert_eq!(copied, OutputStream::Stderr);
    }

    #[test]
    fn test_output_stream_partial_eq() {
        assert_eq!(OutputStream::Stdout, OutputStream::Stdout);
        assert_ne!(OutputStream::Stdout, OutputStream::Stderr);
    }

    #[test]
    fn test_output_chunk_debug() {
        let chunk = OutputChunk {
            stream: OutputStream::Stdout,
            data: vec![1, 2, 3],
        };
        let debug_str = format!("{:?}", chunk);
        assert!(debug_str.contains("OutputChunk"));
    }

    #[test]
    fn test_output_chunk_clone() {
        let chunk = OutputChunk {
            stream: OutputStream::Stderr,
            data: vec![4, 5, 6],
        };
        let cloned = chunk.clone();
        assert_eq!(cloned.stream, OutputStream::Stderr);
        assert_eq!(cloned.data, vec![4, 5, 6]);
    }

    // === is_likely_sandbox_denied additional tests ===

    #[test]
    fn test_sandbox_detection_keyword_stdout() {
        let output = make_output(1, "Permission denied", "", "");
        assert!(is_likely_sandbox_denied(
            SandboxType::MacosSeatbelt,
            &output
        ));
    }

    #[test]
    fn test_sandbox_detection_keyword_seccomp() {
        let output = make_output(1, "", "seccomp violation", "");
        assert!(is_likely_sandbox_denied(SandboxType::LinuxSeccomp, &output));
    }

    #[test]
    fn test_sandbox_detection_keyword_landlock() {
        let output = make_output(1, "", "", "landlock restriction");
        assert!(is_likely_sandbox_denied(SandboxType::LinuxSeccomp, &output));
    }

    #[test]
    fn test_sandbox_detection_keyword_sandbox() {
        let output = make_output(1, "sandbox policy", "", "");
        assert!(is_likely_sandbox_denied(
            SandboxType::WindowsRestrictedToken,
            &output
        ));
    }

    #[test]
    fn test_sandbox_detection_keyword_failed_write() {
        let output = make_output(1, "", "", "failed to write file");
        assert!(is_likely_sandbox_denied(
            SandboxType::MacosSeatbelt,
            &output
        ));
    }

    #[test]
    fn test_sandbox_detection_quick_reject_126() {
        let output = make_output(126, "", "cannot execute binary file", "");
        assert!(!is_likely_sandbox_denied(
            SandboxType::LinuxSeccomp,
            &output
        ));
    }

    #[test]
    fn test_sandbox_detection_quick_reject_2() {
        let output = make_output(2, "", "some error", "");
        assert!(!is_likely_sandbox_denied(
            SandboxType::MacosSeatbelt,
            &output
        ));
    }

    #[test]
    fn test_sandbox_detection_case_insensitive() {
        let output = make_output(1, "", "PERMISSION DENIED", "");
        assert!(is_likely_sandbox_denied(SandboxType::LinuxSeccomp, &output));
    }

    #[cfg(unix)]
    #[test]
    fn test_sandbox_detection_sigsys_exit_code() {
        // SIGSYS = 31, exit code would be 128 + 31 = 159
        let output = make_output(159, "", "", "");
        assert!(is_likely_sandbox_denied(SandboxType::LinuxSeccomp, &output));
    }

    // === Execute edge cases ===

    #[tokio::test]
    async fn test_execute_nonexistent_command() {
        let params = ExecParams::new(
            vec!["nonexistent_command_xyz_12345".to_string()],
            std::env::current_dir().unwrap(),
        )
        .with_timeout(5000);

        let result = execute(params).await;
        // Should fail to spawn
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_failing_command() {
        #[cfg(unix)]
        let params = ExecParams::new(
            vec!["sh".to_string(), "-c".to_string(), "exit 42".to_string()],
            std::env::current_dir().unwrap(),
        )
        .with_timeout(5000);

        #[cfg(windows)]
        let params = ExecParams::new(
            vec!["cmd".to_string(), "/c".to_string(), "exit 42".to_string()],
            std::env::current_dir().unwrap(),
        )
        .with_timeout(5000);

        let result = execute(params).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 42);
        assert!(!output.success());
    }

    #[tokio::test]
    async fn test_execute_stderr_output() {
        #[cfg(unix)]
        let params = ExecParams::new(
            vec![
                "sh".to_string(),
                "-c".to_string(),
                "echo error >&2".to_string(),
            ],
            std::env::current_dir().unwrap(),
        )
        .with_timeout(5000);

        #[cfg(windows)]
        let params = ExecParams::new(
            vec![
                "cmd".to_string(),
                "/c".to_string(),
                "echo error 1>&2".to_string(),
            ],
            std::env::current_dir().unwrap(),
        )
        .with_timeout(5000);

        let result = execute(params).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stderr.text.contains("error"));
    }

    #[tokio::test]
    async fn test_execute_with_working_directory() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(temp_dir.path().join("testfile.txt"), "content").expect("write file");

        #[cfg(unix)]
        let params = ExecParams::new(vec!["ls".to_string()], temp_dir.path().to_path_buf())
            .with_timeout(5000);

        #[cfg(windows)]
        let params = ExecParams::new(
            vec!["cmd".to_string(), "/c".to_string(), "dir".to_string()],
            temp_dir.path().to_path_buf(),
        )
        .with_timeout(5000);

        let result = execute(params).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.text.contains("testfile") || output.stdout.text.contains("TESTFILE"));
    }

    // === OutputStreamSender tests ===

    #[test]
    fn test_output_stream_sender_clone() {
        let (tx, _rx) = async_channel::unbounded();
        let sender = OutputStreamSender {
            sub_id: "sub1".to_string(),
            call_id: "call1".to_string(),
            tx,
        };
        let cloned = sender.clone();
        assert_eq!(cloned.sub_id, "sub1");
        assert_eq!(cloned.call_id, "call1");
    }
}
