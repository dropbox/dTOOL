//! Pseudoterminal (PTY) handling for macOS
//!
//! Creates and manages PTY master/slave pairs for subprocess communication.

use crate::{Result, TerminalError};
use std::ffi::CString;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};

/// PTY master/slave pair
pub struct Pty {
    master: OwnedFd,
    slave_name: String,
    child_pid: Option<libc::pid_t>,
}

impl Pty {
    /// Create a new PTY and spawn a shell
    pub fn spawn_shell(cols: u16, rows: u16) -> Result<Self> {
        Self::spawn_command("/bin/zsh", &[], cols, rows)
    }

    /// Create a new PTY and spawn a command
    pub fn spawn_command(cmd: &str, args: &[&str], cols: u16, rows: u16) -> Result<Self> {
        unsafe {
            // Open PTY master
            let master_fd = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
            if master_fd < 0 {
                return Err(TerminalError::Pty("Failed to open PTY master".into()));
            }

            // Grant access to slave
            if libc::grantpt(master_fd) != 0 {
                libc::close(master_fd);
                return Err(TerminalError::Pty("Failed to grant PTY access".into()));
            }

            // Unlock slave
            if libc::unlockpt(master_fd) != 0 {
                libc::close(master_fd);
                return Err(TerminalError::Pty("Failed to unlock PTY".into()));
            }

            // Get slave name
            let slave_name_ptr = libc::ptsname(master_fd);
            if slave_name_ptr.is_null() {
                libc::close(master_fd);
                return Err(TerminalError::Pty("Failed to get PTY slave name".into()));
            }
            let slave_name = std::ffi::CStr::from_ptr(slave_name_ptr)
                .to_string_lossy()
                .into_owned();

            // Set terminal size
            let winsize = libc::winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            libc::ioctl(master_fd, libc::TIOCSWINSZ, &winsize);

            // Fork
            let pid = libc::fork();
            if pid < 0 {
                libc::close(master_fd);
                return Err(TerminalError::Pty("Fork failed".into()));
            }

            if pid == 0 {
                // Child process
                libc::setsid();

                // Open slave
                let slave_cstr = CString::new(slave_name.clone()).unwrap();
                let slave_fd = libc::open(slave_cstr.as_ptr(), libc::O_RDWR);
                if slave_fd < 0 {
                    libc::_exit(1);
                }

                // Set controlling terminal
                libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0);

                // Duplicate to stdin/stdout/stderr
                libc::dup2(slave_fd, 0);
                libc::dup2(slave_fd, 1);
                libc::dup2(slave_fd, 2);

                if slave_fd > 2 {
                    libc::close(slave_fd);
                }

                // Close master in child
                libc::close(master_fd);

                // Set up environment
                let term = CString::new("TERM=xterm-256color").unwrap();
                libc::putenv(term.as_ptr() as *mut _);

                // Execute command
                let cmd_cstr = CString::new(cmd).unwrap();
                let mut argv: Vec<CString> = vec![cmd_cstr.clone()];
                argv.extend(args.iter().map(|a| CString::new(*a).unwrap()));
                let argv_ptrs: Vec<*const libc::c_char> = argv
                    .iter()
                    .map(|s| s.as_ptr())
                    .chain(std::iter::once(std::ptr::null()))
                    .collect();

                libc::execvp(cmd_cstr.as_ptr(), argv_ptrs.as_ptr());
                libc::_exit(1);
            }

            // Parent process
            Ok(Self {
                master: OwnedFd::from_raw_fd(master_fd),
                slave_name,
                child_pid: Some(pid),
            })
        }
    }

    /// Get the master file descriptor for reading/writing
    pub fn master_fd(&self) -> RawFd {
        self.master.as_raw_fd()
    }

    /// Read from the PTY
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let n = unsafe { libc::read(self.master_fd(), buf.as_mut_ptr() as *mut _, buf.len()) };
        if n < 0 {
            Err(TerminalError::Io(std::io::Error::last_os_error()))
        } else {
            Ok(n as usize)
        }
    }

    /// Write to the PTY
    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        let n = unsafe { libc::write(self.master_fd(), buf.as_ptr() as *const _, buf.len()) };
        if n < 0 {
            Err(TerminalError::Io(std::io::Error::last_os_error()))
        } else {
            Ok(n as usize)
        }
    }

    /// Resize the PTY
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let result = unsafe { libc::ioctl(self.master_fd(), libc::TIOCSWINSZ, &winsize) };
        if result < 0 {
            Err(TerminalError::Io(std::io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    /// Check if the child process is still running
    pub fn is_running(&self) -> bool {
        if let Some(pid) = self.child_pid {
            unsafe {
                let mut status: libc::c_int = 0;
                let result = libc::waitpid(pid, &mut status, libc::WNOHANG);
                result == 0
            }
        } else {
            false
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if let Some(pid) = self.child_pid {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
                let mut status: libc::c_int = 0;
                libc::waitpid(pid, &mut status, 0);
            }
        }
    }
}
