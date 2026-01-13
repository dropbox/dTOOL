//! Windows PTY implementation using ConPTY.
//!
//! This module provides PTY functionality for Windows using the ConPTY API
//! (introduced in Windows 10 version 1809).

mod conpty;

use std::ffi::OsStr;
use std::io::{self, Read, Result, Write};
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::IntoRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use log::error;
use miow::pipe::{AnonRead, AnonWrite};
use parking_lot::Mutex;
use polling::{Event, PollMode, Poller};

use self::conpty::{ChildProcess, Conpty};
use super::{ChildEvent, EventedPty, EventedReadWrite, OnResize, Options, WindowSize};

/// Interest token for PTY read/write events.
pub const PTY_READ_WRITE_TOKEN: usize = 0;

/// Interest token for child events.
pub const PTY_CHILD_EVENT_TOKEN: usize = 1;

/// Converts an OsStr to a null-terminated wide string (UTF-16).
fn win32_string<S: AsRef<OsStr>>(value: S) -> Vec<u16> {
    OsStr::new(value.as_ref())
        .encode_wide()
        .chain(once(0))
        .collect()
}

/// Escape command-line arguments for Windows.
///
/// This follows the Windows command-line argument escaping rules described at:
/// https://docs.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments
fn cmdline_arg_escape<T: AsRef<OsStr>>(arg: T) -> Vec<u16> {
    let arg = arg.as_ref();
    let arg_str = arg.to_string_lossy();

    // Check if escaping is needed
    let needs_escape = arg_str.is_empty()
        || arg_str
            .chars()
            .any(|c| c == ' ' || c == '\t' || c == '\n' || c == '\x0b' || c == '"');

    if !needs_escape {
        return win32_string(arg);
    }

    let mut result: Vec<u16> = Vec::new();
    result.push(b'"' as u16);

    let mut backslash_count = 0;
    for c in arg_str.chars() {
        if c == '\\' {
            backslash_count += 1;
        } else if c == '"' {
            // Escape all preceding backslashes and the quote
            for _ in 0..backslash_count {
                result.push(b'\\' as u16);
            }
            backslash_count = 0;
            result.push(b'\\' as u16);
            result.push(b'"' as u16);
        } else {
            // Flush backslashes
            for _ in 0..backslash_count {
                result.push(b'\\' as u16);
            }
            backslash_count = 0;
            // Encode the character
            let mut buf = [0u16; 2];
            for &unit in c.encode_utf16(&mut buf) {
                if unit != 0 {
                    result.push(unit);
                }
            }
        }
    }

    // Escape trailing backslashes before closing quote
    for _ in 0..backslash_count {
        result.push(b'\\' as u16);
    }

    result.push(b'"' as u16);
    result.push(0); // Null terminate
    result
}

/// Windows PTY handle using ConPTY.
pub struct Pty {
    /// ConPTY backend.
    backend: Conpty,
    /// Output reader (from console to terminal).
    conout: UnblockedReader,
    /// Input writer (from terminal to console).
    conin: UnblockedWriter,
    /// Child process exit watcher.
    child_watcher: ChildExitWatcher,
}

/// Unblocked reader wrapper for non-blocking pipe reads.
struct UnblockedReader {
    inner: AnonRead,
}

impl UnblockedReader {
    fn new(pipe: AnonRead) -> Self {
        Self { inner: pipe }
    }
}

impl Read for UnblockedReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.read(buf)
    }
}

/// Unblocked writer wrapper for pipe writes.
struct UnblockedWriter {
    inner: AnonWrite,
}

impl UnblockedWriter {
    fn new(pipe: AnonWrite) -> Self {
        Self { inner: pipe }
    }
}

impl Write for UnblockedWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

/// Watches for child process exit.
struct ChildExitWatcher {
    child: Arc<Mutex<Option<ChildProcess>>>,
    exited: AtomicBool,
}

impl ChildExitWatcher {
    fn new(child: ChildProcess) -> Self {
        Self {
            child: Arc::new(Mutex::new(Some(child))),
            exited: AtomicBool::new(false),
        }
    }

    fn check_exit(&mut self) -> Option<Option<i32>> {
        if self.exited.load(Ordering::SeqCst) {
            return None;
        }

        let guard = self.child.lock();
        if let Some(ref child) = *guard {
            match child.try_wait() {
                Ok(Some(exit_code)) => {
                    self.exited.store(true, Ordering::SeqCst);
                    Some(Some(exit_code))
                }
                Ok(None) => None,
                Err(e) => {
                    error!("Error checking child process status: {}", e);
                    self.exited.store(true, Ordering::SeqCst);
                    Some(None)
                }
            }
        } else {
            None
        }
    }

    fn kill(&self) {
        let guard = self.child.lock();
        if let Some(ref child) = *guard {
            if let Err(e) = child.kill() {
                error!("Failed to kill child process: {}", e);
            }
        }
    }
}

/// Create a new PTY with ConPTY backend.
pub fn new(config: &Options, window_size: WindowSize, _window_id: u64) -> Result<Pty> {
    // Create anonymous pipes for console I/O
    let (conout_reader, conout_writer) = miow::pipe::anonymous(0)?;
    let (conin_reader, conin_writer) = miow::pipe::anonymous(0)?;

    // Create the ConPTY
    let backend = Conpty::new(
        window_size.num_cols as i16,
        window_size.num_lines as i16,
        conin_reader.into_raw_handle() as isize,
        conout_writer.into_raw_handle() as isize,
    )?;

    // Get the shell to run
    let shell = config
        .shell
        .as_ref()
        .map(|s| s.program.clone())
        .unwrap_or_else(default_shell);

    let shell_args = config
        .shell
        .as_ref()
        .map(|s| s.args.clone())
        .unwrap_or_default();

    // Build the command line
    let mut cmdline = cmdline_arg_escape(&shell);
    cmdline.pop(); // Remove null terminator
    for arg in &shell_args {
        cmdline.push(b' ' as u16);
        let escaped = cmdline_arg_escape(arg);
        // Remove null terminator from escaped arg
        let len = escaped.len().saturating_sub(1);
        cmdline.extend_from_slice(&escaped[..len]);
    }
    cmdline.push(0); // Add final null terminator

    // Spawn the child process with ConPTY
    let child = backend.spawn(&cmdline, config.working_directory.as_deref(), &config.env)?;

    Ok(Pty {
        backend,
        conout: UnblockedReader::new(conout_reader),
        conin: UnblockedWriter::new(conin_writer),
        child_watcher: ChildExitWatcher::new(child),
    })
}

/// Get the default shell on Windows (PowerShell).
fn default_shell() -> String {
    // Try PowerShell first, then fall back to cmd.exe
    let pwsh_path = std::env::var("SystemRoot")
        .map(|root| {
            format!(
                "{}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                root
            )
        })
        .unwrap_or_else(|_| "powershell.exe".to_string());

    if std::path::Path::new(&pwsh_path).exists() {
        return pwsh_path;
    }

    // Fall back to cmd.exe via COMSPEC
    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
}

impl Pty {
    /// Get the child process exit watcher.
    pub fn child_watcher(&mut self) -> &mut ChildExitWatcher {
        &mut self.child_watcher
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // Kill the child process on drop
        self.child_watcher.kill();
    }
}

impl EventedReadWrite for Pty {
    type Reader = UnblockedReader;
    type Writer = UnblockedWriter;

    unsafe fn register(
        &mut self,
        _poll: &Arc<Poller>,
        _interest: Event,
        _mode: PollMode,
    ) -> Result<()> {
        // Windows pipes don't support polling in the same way as Unix.
        // The actual I/O is handled through blocking reads in a separate thread.
        // For now, we'll use a simplified polling model.
        Ok(())
    }

    fn reregister(&mut self, _poll: &Arc<Poller>, _interest: Event, _mode: PollMode) -> Result<()> {
        Ok(())
    }

    fn deregister(&mut self, _poll: &Arc<Poller>) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.conout
    }

    #[inline]
    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.conin
    }
}

impl EventedPty for Pty {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        self.child_watcher.check_exit().map(ChildEvent::Exited)
    }
}

impl OnResize for Pty {
    fn on_resize(&mut self, window_size: WindowSize) {
        if let Err(e) = self
            .backend
            .resize(window_size.num_cols as i16, window_size.num_lines as i16)
        {
            error!("Failed to resize ConPTY: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_win32_string() {
        let result = win32_string("hello");
        assert_eq!(result, vec![104, 101, 108, 108, 111, 0]); // 'h', 'e', 'l', 'l', 'o', '\0'
    }

    #[test]
    fn test_cmdline_arg_escape_simple() {
        let result = cmdline_arg_escape("hello");
        assert_eq!(result, win32_string("hello"));
    }

    #[test]
    fn test_cmdline_arg_escape_with_spaces() {
        let result = cmdline_arg_escape("hello world");
        // Should be wrapped in quotes: "hello world"
        let expected: Vec<u16> = "\"hello world\"".encode_utf16().chain(once(0)).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_cmdline_arg_escape_empty() {
        let result = cmdline_arg_escape("");
        // Empty string needs quotes: ""
        let expected: Vec<u16> = "\"\"".encode_utf16().chain(once(0)).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_cmdline_arg_escape_with_quotes() {
        let result = cmdline_arg_escape("hello\"world");
        // Quote should be escaped: "hello\"world"
        let expected: Vec<u16> = "\"hello\\\"world\"".encode_utf16().chain(once(0)).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_default_shell() {
        let shell = default_shell();
        // Should return either powershell.exe or cmd.exe
        assert!(
            shell.contains("powershell") || shell.contains("cmd"),
            "Expected PowerShell or cmd.exe, got: {}",
            shell
        );
    }
}
