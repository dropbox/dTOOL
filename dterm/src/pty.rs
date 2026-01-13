//! PTY (Pseudo-Terminal) support for Unix systems.
//!
//! This module provides a cross-platform interface for spawning and managing
//! shell processes through a pseudo-terminal.

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

/// A pseudo-terminal master/slave pair.
pub struct Pty {
    master: OwnedFd,
    pid: libc::pid_t,
}

/// Error type for PTY operations.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("Failed to open PTY: {0}")]
    Open(io::Error),
    #[error("Failed to fork: {0}")]
    Fork(io::Error),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

impl Pty {
    /// Spawn a new shell in a PTY.
    ///
    /// # Arguments
    /// * `rows` - Initial terminal rows
    /// * `cols` - Initial terminal columns
    ///
    /// # Safety
    /// This function uses `fork()` which requires careful handling.
    pub fn spawn(rows: u16, cols: u16) -> Result<Self, PtyError> {
        Self::spawn_command(None, rows, cols)
    }

    /// Spawn a specific command in a PTY.
    ///
    /// # Arguments
    /// * `command` - Command to run (None uses $SHELL or /bin/bash)
    /// * `rows` - Initial terminal rows
    /// * `cols` - Initial terminal columns
    pub fn spawn_command(command: Option<&str>, rows: u16, cols: u16) -> Result<Self, PtyError> {
        // Open a new PTY pair
        let mut master_fd: RawFd = -1;
        let mut slave_fd: RawFd = -1;

        unsafe {
            // Open PTY master/slave pair
            if libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) != 0
            {
                return Err(PtyError::Open(io::Error::last_os_error()));
            }

            // Set initial window size
            let winsize = libc::winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            libc::ioctl(master_fd, libc::TIOCSWINSZ, &winsize);

            // Fork process
            let pid = libc::fork();
            if pid < 0 {
                libc::close(master_fd);
                libc::close(slave_fd);
                return Err(PtyError::Fork(io::Error::last_os_error()));
            }

            if pid == 0 {
                // Child process
                libc::close(master_fd);

                // Create a new session
                if libc::setsid() < 0 {
                    libc::_exit(1);
                }

                // Set controlling terminal
                if libc::ioctl(slave_fd, libc::TIOCSCTTY as libc::c_ulong, 0) < 0 {
                    libc::_exit(1);
                }

                // Redirect stdin/stdout/stderr to slave
                libc::dup2(slave_fd, libc::STDIN_FILENO);
                libc::dup2(slave_fd, libc::STDOUT_FILENO);
                libc::dup2(slave_fd, libc::STDERR_FILENO);

                if slave_fd > libc::STDERR_FILENO {
                    libc::close(slave_fd);
                }

                // Set TERM environment variable
                let term = CString::new("TERM=xterm-256color").unwrap();
                libc::putenv(term.as_ptr() as *mut _);

                // Get shell from command, $SHELL, or default to /bin/bash
                let shell = command
                    .map(String::from)
                    .or_else(|| std::env::var("SHELL").ok())
                    .unwrap_or_else(|| "/bin/bash".to_string());

                let shell_cstr = CString::new(shell.as_str()).unwrap();
                let shell_name = std::path::Path::new(&shell)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("bash");
                let login_shell = format!("-{}", shell_name);
                let login_shell_cstr = CString::new(login_shell).unwrap();

                // Execute shell as login shell
                let args: [*const libc::c_char; 2] = [login_shell_cstr.as_ptr(), std::ptr::null()];

                libc::execvp(shell_cstr.as_ptr(), args.as_ptr());

                // If exec fails, exit
                libc::_exit(1);
            }

            // Parent process
            libc::close(slave_fd);

            // Set master to non-blocking
            let flags = libc::fcntl(master_fd, libc::F_GETFL);
            libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);

            Ok(Pty {
                master: OwnedFd::from_raw_fd(master_fd),
                pid,
            })
        }
    }

    /// Read available data from the PTY.
    ///
    /// Returns `Ok(None)` if no data is available (non-blocking).
    /// Returns `Ok(Some(vec))` with data if available.
    /// Returns `Err` on actual errors.
    pub fn read(&self) -> Result<Option<Vec<u8>>, PtyError> {
        let mut buf = vec![0u8; 4096];
        let fd = self.master.as_raw_fd();

        let result = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };

        if result < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock || err.kind() == io::ErrorKind::Interrupted {
                return Ok(None);
            }
            return Err(PtyError::Io(err));
        }

        if result == 0 {
            // EOF - process exited
            return Ok(None);
        }

        // SAFETY: result > 0 guaranteed by checks above, so cast to usize is safe
        #[allow(clippy::cast_sign_loss)]
        buf.truncate(result as usize);
        Ok(Some(buf))
    }

    /// Write data to the PTY (sends to shell stdin).
    pub fn write(&self, data: &[u8]) -> Result<(), PtyError> {
        let fd = self.master.as_raw_fd();
        let mut written = 0;

        while written < data.len() {
            let result = unsafe {
                libc::write(
                    fd,
                    data[written..].as_ptr() as *const _,
                    data.len() - written,
                )
            };

            if result < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(PtyError::Io(err));
            }

            // SAFETY: result >= 0 guaranteed by check above, so cast to usize is safe
            #[allow(clippy::cast_sign_loss)]
            {
                written += result as usize;
            }
        }

        Ok(())
    }

    /// Resize the PTY window.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let result = unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) };

        if result < 0 {
            return Err(PtyError::Io(io::Error::last_os_error()));
        }

        Ok(())
    }

    /// Check if the child process is still running.
    pub fn is_running(&self) -> bool {
        unsafe {
            let mut status: libc::c_int = 0;
            let result = libc::waitpid(self.pid, &mut status, libc::WNOHANG);
            result == 0
        }
    }

    /// Get the child process ID.
    #[allow(dead_code)]
    pub fn pid(&self) -> i32 {
        self.pid
    }

    /// Get the master file descriptor (for polling).
    #[allow(dead_code)]
    pub fn as_raw_fd(&self) -> RawFd {
        self.master.as_raw_fd()
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // Send SIGHUP to child process
        unsafe {
            libc::kill(self.pid, libc::SIGHUP);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_spawn() {
        let pty = Pty::spawn(24, 80);
        assert!(pty.is_ok());

        let pty = pty.unwrap();
        assert!(pty.is_running());
    }

    #[test]
    fn test_pty_write_read() {
        let pty = Pty::spawn(24, 80).unwrap();

        // Write a simple command
        pty.write(b"echo hello\n").unwrap();

        // Give the shell time to process
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Try to read output
        let data = pty.read();
        assert!(data.is_ok());
    }

    #[test]
    fn test_pty_resize() {
        let pty = Pty::spawn(24, 80).unwrap();
        assert!(pty.resize(48, 120).is_ok());
    }
}
