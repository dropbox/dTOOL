//! OS signal handlers for graceful terminal cleanup.
//!
//! This module provides signal handling infrastructure to ensure the terminal
//! is properly restored when the application receives signals like SIGINT (Ctrl+C)
//! or SIGTERM.
//!
//! # Why Signal Handlers?
//!
//! When a terminal application is in raw mode or alternate screen mode, receiving
//! a signal can leave the terminal in a broken state if not handled properly.
//! This module ensures that:
//!
//! - Raw mode is disabled
//! - Alternate screen is exited
//! - Cursor is shown
//! - Terminal attributes are reset
//!
//! # Usage
//!
//! Signal handlers are automatically installed when you use [`crate::app::App::run()`].
//! However, you can also install them manually:
//!
//! ```rust,ignore
//! use inky::terminal::signals::install_signal_handlers;
//!
//! install_signal_handlers();
//! // Your application code...
//! ```
//!
//! # Platform Support
//!
//! - **Unix**: Full support for SIGINT, SIGTERM, SIGHUP, SIGQUIT
//! - **Windows**: Support via SetConsoleCtrlHandler
//!
//! # Thread Safety
//!
//! Signal handlers are installed globally and are thread-safe. The handlers
//! use atomic operations to avoid deadlocks.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::terminal::emergency_restore;

/// Global flag indicating a shutdown signal was received.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Global flag to track if handlers have been installed.
static HANDLERS_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Check if a shutdown signal has been received.
///
/// This can be polled in the application's event loop to check if
/// the user requested termination via Ctrl+C or similar.
///
/// # Example
///
/// ```rust,ignore
/// use inky::terminal::signals::shutdown_requested;
///
/// loop {
///     if shutdown_requested() {
///         break; // Clean exit
///     }
///     // ... event loop ...
/// }
/// ```
pub fn shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Request application shutdown.
///
/// This sets the shutdown flag that can be polled by the event loop.
/// Used internally by signal handlers.
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// Clear the shutdown request flag.
///
/// Useful if you want to handle the shutdown signal but continue running.
pub fn clear_shutdown_request() {
    SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
}

/// Install signal handlers for graceful terminal cleanup.
///
/// This function installs handlers for common termination signals:
/// - SIGINT (Ctrl+C)
/// - SIGTERM (kill command)
/// - SIGHUP (terminal closed) - Unix only
/// - SIGQUIT (Ctrl+\) - Unix only
///
/// The handlers will:
/// 1. Restore the terminal to its original state
/// 2. Set the shutdown flag for the event loop
/// 3. Exit with an appropriate exit code
///
/// This function is idempotent - calling it multiple times is safe.
///
/// # Example
///
/// ```rust,ignore
/// use inky::terminal::signals::install_signal_handlers;
///
/// install_signal_handlers();
/// ```
pub fn install_signal_handlers() {
    // Only install once
    if HANDLERS_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    #[cfg(unix)]
    install_unix_handlers();

    #[cfg(windows)]
    install_windows_handlers();
}

#[cfg(unix)]
fn install_unix_handlers() {
    // SAFETY: libc::signal is safe to call with valid signal numbers and a valid
    // handler function pointer. `signal_handler_graceful` has the correct signature
    // `extern "C" fn(c_int)` required by libc. The handler only uses async-signal-safe
    // operations (atomic writes, _exit). We check SIG_ERR to detect failures.
    unsafe {
        // SIGINT handler (Ctrl+C)
        let result = libc::signal(libc::SIGINT, signal_handler_graceful as libc::sighandler_t);
        if result == libc::SIG_ERR {
            #[cfg(debug_assertions)]
            eprintln!("Warning: failed to install SIGINT handler");
        }

        // SIGTERM handler
        let result = libc::signal(libc::SIGTERM, signal_handler_graceful as libc::sighandler_t);
        if result == libc::SIG_ERR {
            #[cfg(debug_assertions)]
            eprintln!("Warning: failed to install SIGTERM handler");
        }

        // SIGHUP handler (terminal closed)
        let result = libc::signal(libc::SIGHUP, signal_handler_graceful as libc::sighandler_t);
        if result == libc::SIG_ERR {
            #[cfg(debug_assertions)]
            eprintln!("Warning: failed to install SIGHUP handler");
        }

        // SIGQUIT handler (Ctrl+\)
        let result = libc::signal(libc::SIGQUIT, signal_handler_graceful as libc::sighandler_t);
        if result == libc::SIG_ERR {
            #[cfg(debug_assertions)]
            eprintln!("Warning: failed to install SIGQUIT handler");
        }
    }
}

/// Signal handler function that restores terminal and sets shutdown flag.
///
/// This is called from signal context, so we must be careful to only use
/// async-signal-safe operations.
#[cfg(unix)]
extern "C" fn signal_handler_graceful(signum: libc::c_int) {
    // Emergency restore is async-signal-safe (only uses write() syscall)
    emergency_restore();

    // Set shutdown flag (atomic operation is async-signal-safe)
    request_shutdown();

    // SAFETY: libc::_exit is safe to call at any time and immediately terminates
    // the process without running destructors or atexit handlers. This is appropriate
    // for signal handlers where we need a clean, immediate exit. The exit codes follow
    // the standard Unix convention of 128 + signal number.
    unsafe {
        match signum {
            libc::SIGINT => libc::_exit(130),  // 128 + 2
            libc::SIGTERM => libc::_exit(143), // 128 + 15
            libc::SIGHUP => libc::_exit(129),  // 128 + 1
            libc::SIGQUIT => libc::_exit(131), // 128 + 3
            _ => libc::_exit(128 + signum),
        }
    }
}

#[cfg(windows)]
fn install_windows_handlers() {
    // On Windows, we use SetConsoleCtrlHandler
    extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<extern "system" fn(u32) -> i32>, add: i32) -> i32;
    }

    extern "system" fn ctrl_handler(ctrl_type: u32) -> i32 {
        const CTRL_C_EVENT: u32 = 0;
        const CTRL_BREAK_EVENT: u32 = 1;
        const CTRL_CLOSE_EVENT: u32 = 2;

        match ctrl_type {
            CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
                emergency_restore();
                request_shutdown();
                // Return TRUE to indicate we handled it
                1
            }
            _ => 0, // Let system handle other events
        }
    }

    // SAFETY: SetConsoleCtrlHandler is safe to call with a valid handler function.
    // `ctrl_handler` has the correct signature `extern "system" fn(u32) -> i32`.
    // Passing `1` as the second argument adds the handler to the list. The handler
    // uses only async-signal-safe operations.
    unsafe {
        if SetConsoleCtrlHandler(Some(ctrl_handler), 1) == 0 {
            #[cfg(debug_assertions)]
            eprintln!("Warning: failed to install console control handler");
        }
    }
}

/// Install only the panic hook without signal handlers.
///
/// This is useful when you want panic recovery but the runtime handles signals.
/// This is a re-export of [`crate::terminal::install_panic_hook`].
pub use crate::terminal::install_panic_hook;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_flag() {
        // Should start as false
        clear_shutdown_request();
        assert!(!shutdown_requested());

        // Request shutdown
        request_shutdown();
        assert!(shutdown_requested());

        // Clear it
        clear_shutdown_request();
        assert!(!shutdown_requested());
    }

    #[test]
    fn test_install_handlers_idempotent() {
        // Reset the flag for testing
        HANDLERS_INSTALLED.store(false, Ordering::SeqCst);

        // First install should work
        install_signal_handlers();
        assert!(HANDLERS_INSTALLED.load(Ordering::SeqCst));

        // Second install should be a no-op (no crash)
        install_signal_handlers();
        assert!(HANDLERS_INSTALLED.load(Ordering::SeqCst));
    }
}
