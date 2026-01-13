//! Exit status handling utilities
//!
//! Provides platform-specific handling of process exit statuses,
//! including proper signal handling on Unix systems.

use std::process::ExitStatus;

/// Handle a child process exit status by converting it to an appropriate exit code.
///
/// On Unix systems, if the process was terminated by a signal, the exit code
/// is computed as 128 + signal_number (following shell conventions).
///
/// On Windows, uses the exit code directly if available.
///
/// # Arguments
/// * `status` - The exit status from a child process
///
/// # Returns
/// Never returns - exits the current process with the derived exit code.
#[cfg(unix)]
pub fn handle_exit_status(status: ExitStatus) -> ! {
    use std::os::unix::process::ExitStatusExt;

    // Use ExitStatus to derive the exit code.
    if let Some(code) = status.code() {
        std::process::exit(code);
    } else if let Some(signal) = status.signal() {
        // Follow shell convention: 128 + signal number
        std::process::exit(128 + signal);
    } else {
        std::process::exit(1);
    }
}

/// Handle a child process exit status by converting it to an appropriate exit code.
///
/// On Windows, uses the exit code directly if available, otherwise defaults to 1.
#[cfg(windows)]
pub fn handle_exit_status(status: ExitStatus) -> ! {
    if let Some(code) = status.code() {
        std::process::exit(code);
    } else {
        // Rare on Windows, but if it happens: use fallback code.
        std::process::exit(1);
    }
}

/// Extract an exit code from an ExitStatus without terminating the process.
///
/// This is useful when you want to get the exit code to report but continue running.
///
/// # Arguments
/// * `status` - The exit status from a child process
///
/// # Returns
/// The exit code as an i32.
#[cfg(unix)]
pub fn exit_code_from_status(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;

    if let Some(code) = status.code() {
        code
    } else if let Some(signal) = status.signal() {
        128 + signal
    } else {
        1
    }
}

/// Extract an exit code from an ExitStatus without terminating the process.
#[cfg(windows)]
pub fn exit_code_from_status(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn test_exit_code_from_success() {
        // Run a command that exits successfully
        let status = Command::new("true")
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 0);
    }

    #[test]
    fn test_exit_code_from_failure() {
        // Run a command that exits with non-zero
        let status = Command::new("false")
            .status()
            .expect("failed to run command");
        assert_ne!(exit_code_from_status(status), 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_specific_code() {
        // Run bash with specific exit code
        let status = Command::new("sh")
            .args(["-c", "exit 42"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 42);
    }

    #[test]
    fn test_success_status() {
        let status = Command::new("true")
            .status()
            .expect("failed to run command");
        assert!(status.success());
        assert_eq!(exit_code_from_status(status), 0);
    }

    // ========================
    // Additional exit code tests
    // ========================

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_exit_1() {
        let status = Command::new("sh")
            .args(["-c", "exit 1"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 1);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_exit_255() {
        // Maximum single-byte exit code
        let status = Command::new("sh")
            .args(["-c", "exit 255"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 255);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_exit_128() {
        // Boundary value (signals start at 128 + signal)
        let status = Command::new("sh")
            .args(["-c", "exit 128"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 128);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_exit_127() {
        // Common error code for "command not found"
        let status = Command::new("sh")
            .args(["-c", "exit 127"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 127);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_exit_126() {
        // Common error code for "permission denied"
        let status = Command::new("sh")
            .args(["-c", "exit 126"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 126);
    }

    // ========================
    // Signal handling tests (Unix only)
    // ========================

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_sigkill() {
        // Create a process that will be killed
        let mut child = Command::new("sh")
            .args(["-c", "sleep 10"])
            .spawn()
            .expect("failed to spawn");

        // Send SIGKILL (signal 9)
        unsafe {
            libc::kill(child.id() as i32, libc::SIGKILL);
        }

        let status = child.wait().expect("failed to wait");
        // SIGKILL = 9, so exit code should be 128 + 9 = 137
        assert_eq!(exit_code_from_status(status), 137);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_sigterm() {
        // Create a process that will be terminated
        let mut child = Command::new("sh")
            .args(["-c", "sleep 10"])
            .spawn()
            .expect("failed to spawn");

        // Send SIGTERM (signal 15)
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }

        let status = child.wait().expect("failed to wait");
        // SIGTERM = 15, so exit code should be 128 + 15 = 143
        assert_eq!(exit_code_from_status(status), 143);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_sigint() {
        // Create a process that will be interrupted
        let mut child = Command::new("sh")
            .args(["-c", "sleep 10"])
            .spawn()
            .expect("failed to spawn");

        // Send SIGINT (signal 2)
        unsafe {
            libc::kill(child.id() as i32, libc::SIGINT);
        }

        let status = child.wait().expect("failed to wait");
        // SIGINT = 2, so exit code should be 128 + 2 = 130
        assert_eq!(exit_code_from_status(status), 130);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_sigquit() {
        // Create a process that will be quit
        let mut child = Command::new("sh")
            .args(["-c", "sleep 10"])
            .spawn()
            .expect("failed to spawn");

        // Send SIGQUIT (signal 3)
        unsafe {
            libc::kill(child.id() as i32, libc::SIGQUIT);
        }

        let status = child.wait().expect("failed to wait");
        // SIGQUIT = 3, so exit code should be 128 + 3 = 131
        assert_eq!(exit_code_from_status(status), 131);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_sighup() {
        // Create a process that will be hung up
        let mut child = Command::new("sh")
            .args(["-c", "sleep 10"])
            .spawn()
            .expect("failed to spawn");

        // Send SIGHUP (signal 1)
        unsafe {
            libc::kill(child.id() as i32, libc::SIGHUP);
        }

        let status = child.wait().expect("failed to wait");
        // SIGHUP = 1, so exit code should be 128 + 1 = 129
        assert_eq!(exit_code_from_status(status), 129);
    }

    // ========================
    // Process status assertions
    // ========================

    #[test]
    fn test_failed_status_is_not_success() {
        let status = Command::new("false")
            .status()
            .expect("failed to run command");
        assert!(!status.success());
    }

    #[test]
    fn test_success_status_is_success() {
        let status = Command::new("true")
            .status()
            .expect("failed to run command");
        assert!(status.success());
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_various_values() {
        // Test a range of exit codes
        for code in [0, 1, 2, 42, 100, 127, 128, 200, 255] {
            let status = Command::new("sh")
                .args(["-c", &format!("exit {}", code)])
                .status()
                .expect("failed to run command");
            assert_eq!(
                exit_code_from_status(status),
                code,
                "Exit code mismatch for input {}",
                code
            );
        }
    }

    // ========================
    // Edge case tests
    // ========================

    #[test]
    #[cfg(unix)]
    fn test_exit_code_overflow_wraps() {
        // Exit codes > 255 wrap around on Unix
        let status = Command::new("sh")
            .args(["-c", "exit 256"])
            .status()
            .expect("failed to run command");
        // 256 % 256 = 0
        assert_eq!(exit_code_from_status(status), 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_257_wraps() {
        // 257 % 256 = 1
        let status = Command::new("sh")
            .args(["-c", "exit 257"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 1);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_negative_wraps() {
        // Negative values become positive modulo 256
        let status = Command::new("sh")
            .args(["-c", "exit -1"])
            .status()
            .expect("failed to run command");
        // -1 wraps to 255
        assert_eq!(exit_code_from_status(status), 255);
    }

    // ========================
    // Command execution tests
    // ========================

    #[test]
    fn test_exit_code_from_echo() {
        // Echo always succeeds
        let status = Command::new("echo")
            .arg("test")
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_test_true() {
        // test -z "" is true
        let status = Command::new("test")
            .args(["-z", ""])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_test_false() {
        // test -z "nonempty" is false
        let status = Command::new("test")
            .args(["-z", "nonempty"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 1);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_grep_match() {
        // grep returns 0 on match
        let status = Command::new("sh")
            .args(["-c", "echo hello | grep hello"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_exit_code_from_grep_no_match() {
        // grep returns 1 on no match
        let status = Command::new("sh")
            .args(["-c", "echo hello | grep xyz"])
            .status()
            .expect("failed to run command");
        assert_eq!(exit_code_from_status(status), 1);
    }

    // ========================
    // Multiple calls consistency
    // ========================

    #[test]
    fn test_exit_code_consistent_across_calls() {
        // Same command should give same result
        for _ in 0..5 {
            let status = Command::new("true")
                .status()
                .expect("failed to run command");
            assert_eq!(exit_code_from_status(status), 0);
        }
    }

    #[test]
    fn test_exit_code_consistent_failure() {
        for _ in 0..5 {
            let status = Command::new("false")
                .status()
                .expect("failed to run command");
            assert_ne!(exit_code_from_status(status), 0);
        }
    }
}
