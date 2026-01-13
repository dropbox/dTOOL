//! TTY related functionality.
//!
//! This module provides PTY (pseudo-terminal) abstraction for the dterm-alacritty-bridge.
//! It follows the same patterns as Alacritty's tty module for compatibility.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use polling::{Event, PollMode, Poller};

// Re-export WindowSize from event module for consistency
pub use crate::event::WindowSize;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use self::unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

/// Configuration for the `Pty` interface.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Options {
    /// Shell options.
    ///
    /// [`None`] will use the default shell.
    pub shell: Option<Shell>,

    /// Shell startup directory.
    pub working_directory: Option<PathBuf>,

    /// Drain the child process output before exiting the terminal.
    pub drain_on_exit: bool,

    /// Extra environment variables.
    pub env: HashMap<String, String>,
}

/// Shell options.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Shell {
    /// Path to a shell program to run on startup.
    pub program: String,
    /// Arguments passed to shell.
    pub args: Vec<String>,
}

impl Shell {
    pub fn new(program: String, args: Vec<String>) -> Self {
        Self { program, args }
    }
}

/// Stream read and/or write behavior.
///
/// This defines an abstraction over polling's interface in order to allow either
/// one read/write object or a separate read and write object.
pub trait EventedReadWrite {
    type Reader: io::Read;
    type Writer: io::Write;

    /// Register for polling events.
    ///
    /// # Safety
    ///
    /// The underlying sources must outlive their registration in the `Poller`.
    unsafe fn register(
        &mut self,
        poll: &Arc<Poller>,
        interest: Event,
        mode: PollMode,
    ) -> io::Result<()>;

    /// Re-register for polling events.
    fn reregister(&mut self, poll: &Arc<Poller>, interest: Event, mode: PollMode)
        -> io::Result<()>;

    /// Deregister from polling.
    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()>;

    /// Get the reader.
    fn reader(&mut self) -> &mut Self::Reader;

    /// Get the writer.
    fn writer(&mut self) -> &mut Self::Writer;
}

/// Events concerning TTY child processes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildEvent {
    /// Indicates the child has exited, with an error code if available.
    Exited(Option<i32>),
}

/// A pseudoterminal (or PTY).
///
/// This is a refinement of EventedReadWrite that also provides a channel through which we can be
/// notified if the PTY child process does something we care about (other than writing to the TTY).
/// In particular, this allows for race-free child exit notification on UNIX (cf. `SIGCHLD`).
pub trait EventedPty: EventedReadWrite {
    /// Tries to retrieve an event.
    ///
    /// Returns `Some(event)` on success, or `None` if there are no events to retrieve.
    fn next_child_event(&mut self) -> Option<ChildEvent>;
}

/// Trait for types that can be resized.
pub trait OnResize {
    /// Called when the terminal window is resized.
    fn on_resize(&mut self, size: WindowSize);
}

/// Setup environment variables for terminal.
pub fn setup_env() {
    // Default to 'xterm-256color' terminfo. May be overridden by user's config.
    let terminfo = if terminfo_exists("alacritty") {
        "alacritty"
    } else {
        "xterm-256color"
    };

    // SAFETY: Environment variable setting
    unsafe {
        std::env::set_var("TERM", terminfo);
        // Advertise 24-bit color support.
        std::env::set_var("COLORTERM", "truecolor");
    }
}

/// Check if a terminfo entry exists on the system.
fn terminfo_exists(terminfo: &str) -> bool {
    use std::env;
    use std::path::Path;

    // Get first terminfo character for the parent directory.
    let first = terminfo.get(..1).unwrap_or_default();
    let first_hex = format!("{:x}", first.chars().next().unwrap_or_default() as usize);

    // Helper function to check a path
    fn check_path(path: &Path, first: &str, first_hex: &str, terminfo: &str) -> bool {
        path.join(first).join(terminfo).exists() || path.join(first_hex).join(terminfo).exists()
    }

    if let Some(dir) = env::var_os("TERMINFO") {
        let path = PathBuf::from(&dir);
        if check_path(&path, first, &first_hex, terminfo) {
            return true;
        }
    } else if let Some(home) = home::home_dir() {
        let path = home.join(".terminfo");
        if check_path(&path, first, &first_hex, terminfo) {
            return true;
        }
    }

    if let Ok(dirs) = env::var("TERMINFO_DIRS") {
        for dir in dirs.split(':') {
            let path = PathBuf::from(dir);
            if check_path(&path, first, &first_hex, terminfo) {
                return true;
            }
        }
    }

    if let Ok(prefix) = env::var("PREFIX") {
        let base = PathBuf::from(prefix);
        for subdir in ["etc/terminfo", "lib/terminfo", "share/terminfo"] {
            let path = base.join(subdir);
            if check_path(&path, first, &first_hex, terminfo) {
                return true;
            }
        }
    }

    // Check standard paths
    for standard_path in [
        "/etc/terminfo",
        "/lib/terminfo",
        "/usr/share/terminfo",
        "/boot/system/data/terminfo",
    ] {
        let path = PathBuf::from(standard_path);
        if check_path(&path, first, &first_hex, terminfo) {
            return true;
        }
    }

    // No valid terminfo path has been found.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_new() {
        let shell = Shell::new("/bin/bash".to_string(), vec!["-l".to_string()]);
        assert_eq!(shell.program, "/bin/bash");
        assert_eq!(shell.args, vec!["-l"]);
    }

    #[test]
    fn test_options_default() {
        let opts = Options::default();
        assert!(opts.shell.is_none());
        assert!(opts.working_directory.is_none());
        assert!(!opts.drain_on_exit);
        assert!(opts.env.is_empty());
    }
}
