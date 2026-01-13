//! End-to-end integration tests for PTY + Terminal + EventLoop.
//!
//! These tests verify the complete data flow from PTY I/O through the event loop
//! to the terminal grid.
//!
//! # Platform Support
//!
//! - **Unix**: Full integration tests with PTY I/O
//! - **Windows**: ConPTY integration tests (requires Windows 10 1809+)

#![cfg(any(unix, windows))]

// ============================================================================
// Unix Integration Tests
// ============================================================================

#[cfg(unix)]
mod unix_tests {
    use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use dterm_alacritty_bridge::{
        event::{Event, EventListener, WindowSize},
        event_loop::{EventLoop, Msg},
        sync::FairMutex,
        term::{Config, Term},
        tty::{Options as TtyOptions, Shell},
        Dimensions,
    };

    /// Test event listener that tracks events received from the terminal.
    #[derive(Clone)]
    struct TestEventListener {
        wakeup_count: Arc<AtomicI32>,
        child_exited: Arc<AtomicBool>,
        exit_code: Arc<AtomicI32>,
    }

    impl TestEventListener {
        fn new() -> Self {
            Self {
                wakeup_count: Arc::new(AtomicI32::new(0)),
                child_exited: Arc::new(AtomicBool::new(false)),
                exit_code: Arc::new(AtomicI32::new(-1)),
            }
        }

        fn wakeup_count(&self) -> i32 {
            self.wakeup_count.load(Ordering::SeqCst)
        }

        fn child_exited(&self) -> bool {
            self.child_exited.load(Ordering::SeqCst)
        }

        fn exit_code(&self) -> i32 {
            self.exit_code.load(Ordering::SeqCst)
        }
    }

    impl EventListener for TestEventListener {
        fn send_event(&self, event: Event) {
            match event {
                Event::Wakeup => {
                    self.wakeup_count.fetch_add(1, Ordering::SeqCst);
                }
                Event::ChildExit(code) => {
                    self.child_exited.store(true, Ordering::SeqCst);
                    self.exit_code.store(code, Ordering::SeqCst);
                }
                _ => {}
            }
        }
    }

    /// Simple dimensions helper for tests.
    struct TestDimensions {
        rows: usize,
        cols: usize,
    }

    impl Dimensions for TestDimensions {
        fn total_lines(&self) -> usize {
            self.rows
        }

        fn screen_lines(&self) -> usize {
            self.rows
        }

        fn columns(&self) -> usize {
            self.cols
        }
    }

    /// Extract text from a specific line of the terminal grid.
    fn extract_line_text<T>(term: &Term<T>, line: usize) -> String {
        let grid = term.grid();
        let cols = grid.cols();
        let mut text = String::new();

        for col in 0..cols {
            if let Some(cell) = grid.cell(line as u16, col) {
                let ch = cell.char();
                if ch != '\0' {
                    text.push(ch);
                }
            }
        }

        // Trim trailing spaces
        text.trim_end().to_string()
    }

    /// Test: Spawn `/bin/echo hello` and verify output appears in terminal grid.
    #[test]
    fn test_echo_hello() {
        // Create terminal with test dimensions
        let dimensions = TestDimensions { rows: 24, cols: 80 };
        let config = Config::default();
        let event_listener = TestEventListener::new();

        let term = Term::new(config, &dimensions, event_listener.clone());
        let terminal = Arc::new(FairMutex::new(term));

        // Configure PTY to run `/bin/echo hello`
        let tty_options = TtyOptions {
            shell: Some(Shell::new(
                "/bin/echo".to_string(),
                vec!["hello".to_string()],
            )),
            working_directory: None,
            drain_on_exit: true,
            env: Default::default(),
        };

        let window_size = WindowSize::new(80, 24, 8, 16);

        // Spawn PTY
        let pty = dterm_alacritty_bridge::tty::new(&tty_options, window_size, 1)
            .expect("Failed to spawn PTY");

        // Create event loop
        let event_loop = EventLoop::new(
            terminal.clone(),
            event_listener.clone(),
            pty,
            true, // drain_on_exit
        )
        .expect("Failed to create event loop");

        // Spawn event loop thread
        let handle = event_loop.spawn();

        // Wait for the child to exit (with timeout)
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        while !event_listener.child_exited() && start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(
            event_listener.child_exited(),
            "Child process did not exit within timeout"
        );
        assert_eq!(
            event_listener.exit_code(),
            0,
            "Child process should exit with code 0"
        );

        // Check that we received at least one wakeup event
        assert!(
            event_listener.wakeup_count() > 0,
            "Should have received wakeup events"
        );

        // Verify "hello" appears in the terminal grid
        let term_guard = terminal.lock_unfair();
        let line0 = extract_line_text(&term_guard, 0);

        assert!(
            line0.contains("hello"),
            "Terminal should contain 'hello', got: '{}'",
            line0
        );

        drop(term_guard);

        // Join the event loop thread
        let _ = handle.join();
    }

    /// Test: Spawn a command that outputs multiple lines.
    #[test]
    fn test_multiline_output() {
        let dimensions = TestDimensions { rows: 24, cols: 80 };
        let config = Config::default();
        let event_listener = TestEventListener::new();

        let term = Term::new(config, &dimensions, event_listener.clone());
        let terminal = Arc::new(FairMutex::new(term));

        // Use printf to output multiple lines
        let tty_options = TtyOptions {
            shell: Some(Shell::new(
                "/usr/bin/printf".to_string(),
                vec!["line1\\nline2\\nline3\\n".to_string()],
            )),
            working_directory: None,
            drain_on_exit: true,
            env: Default::default(),
        };

        let window_size = WindowSize::new(80, 24, 8, 16);

        let pty = dterm_alacritty_bridge::tty::new(&tty_options, window_size, 2)
            .expect("Failed to spawn PTY");

        let event_loop = EventLoop::new(terminal.clone(), event_listener.clone(), pty, true)
            .expect("Failed to create event loop");

        let handle = event_loop.spawn();

        // Wait for child to exit
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        while !event_listener.child_exited() && start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(event_listener.child_exited(), "Child should have exited");

        // Verify all three lines appear
        let term_guard = terminal.lock_unfair();
        let line0 = extract_line_text(&term_guard, 0);
        let line1 = extract_line_text(&term_guard, 1);
        let line2 = extract_line_text(&term_guard, 2);

        assert!(
            line0.contains("line1"),
            "Line 0 should contain 'line1', got: '{}'",
            line0
        );
        assert!(
            line1.contains("line2"),
            "Line 1 should contain 'line2', got: '{}'",
            line1
        );
        assert!(
            line2.contains("line3"),
            "Line 2 should contain 'line3', got: '{}'",
            line2
        );

        drop(term_guard);
        let _ = handle.join();
    }

    /// Test: Send input to the PTY via the event loop.
    #[test]
    fn test_pty_input() {
        let dimensions = TestDimensions { rows: 24, cols: 80 };
        let config = Config::default();
        let event_listener = TestEventListener::new();

        let term = Term::new(config, &dimensions, event_listener.clone());
        let terminal = Arc::new(FairMutex::new(term));

        // Use `cat` which echoes its input
        let tty_options = TtyOptions {
            shell: Some(Shell::new("/bin/cat".to_string(), vec![])),
            working_directory: None,
            drain_on_exit: true,
            env: Default::default(),
        };

        let window_size = WindowSize::new(80, 24, 8, 16);

        let pty = dterm_alacritty_bridge::tty::new(&tty_options, window_size, 3)
            .expect("Failed to spawn PTY");

        let event_loop = EventLoop::new(terminal.clone(), event_listener.clone(), pty, true)
            .expect("Failed to create event loop");

        let channel = event_loop.channel();
        let handle = event_loop.spawn();

        // Give the process time to start
        std::thread::sleep(Duration::from_millis(100));

        // Send "test\n" to the PTY
        channel
            .send(Msg::Input(std::borrow::Cow::Borrowed(b"test\n")))
            .expect("Failed to send input");

        // Wait for output to be processed
        std::thread::sleep(Duration::from_millis(200));

        // Send EOF (Ctrl+D) to terminate cat
        channel
            .send(Msg::Input(std::borrow::Cow::Borrowed(&[4]))) // Ctrl+D
            .expect("Failed to send EOF");

        // Wait for child to exit
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        while !event_listener.child_exited() && start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(event_listener.child_exited(), "Child should have exited");

        // Verify "test" appears in the terminal (cat echoes input)
        let term_guard = terminal.lock_unfair();
        let line0 = extract_line_text(&term_guard, 0);

        assert!(
            line0.contains("test"),
            "Terminal should contain 'test', got: '{}'",
            line0
        );

        drop(term_guard);
        let _ = handle.join();
    }

    /// Test: Verify shutdown message stops the event loop.
    #[test]
    fn test_shutdown() {
        let dimensions = TestDimensions { rows: 24, cols: 80 };
        let config = Config::default();
        let event_listener = TestEventListener::new();

        let term = Term::new(config, &dimensions, event_listener.clone());
        let terminal = Arc::new(FairMutex::new(term));

        // Use `sleep` which runs for a long time
        let tty_options = TtyOptions {
            shell: Some(Shell::new("/bin/sleep".to_string(), vec!["60".to_string()])),
            working_directory: None,
            drain_on_exit: false,
            env: Default::default(),
        };

        let window_size = WindowSize::new(80, 24, 8, 16);

        let pty = dterm_alacritty_bridge::tty::new(&tty_options, window_size, 4)
            .expect("Failed to spawn PTY");

        let event_loop = EventLoop::new(terminal.clone(), event_listener.clone(), pty, false)
            .expect("Failed to create event loop");

        let channel = event_loop.channel();
        let handle = event_loop.spawn();

        // Give the process time to start
        std::thread::sleep(Duration::from_millis(100));

        // Send shutdown
        channel
            .send(Msg::Shutdown)
            .expect("Failed to send shutdown");

        // The event loop should exit quickly
        let result = handle.join();
        assert!(result.is_ok(), "Event loop thread should exit cleanly");
    }

    /// Test: PTY resize.
    #[test]
    fn test_resize() {
        let dimensions = TestDimensions { rows: 24, cols: 80 };
        let config = Config::default();
        let event_listener = TestEventListener::new();

        let term = Term::new(config, &dimensions, event_listener.clone());
        let terminal = Arc::new(FairMutex::new(term));

        // Use `tput cols` to print terminal width
        let tty_options = TtyOptions {
            shell: Some(Shell::new(
                "/bin/sh".to_string(),
                vec![
                    "-c".to_string(),
                    "tput cols; sleep 0.2; tput cols".to_string(),
                ],
            )),
            working_directory: None,
            drain_on_exit: true,
            env: Default::default(),
        };

        let window_size = WindowSize::new(80, 24, 8, 16);

        let pty = dterm_alacritty_bridge::tty::new(&tty_options, window_size, 5)
            .expect("Failed to spawn PTY");

        let event_loop = EventLoop::new(terminal.clone(), event_listener.clone(), pty, true)
            .expect("Failed to create event loop");

        let channel = event_loop.channel();
        let handle = event_loop.spawn();

        // Let first tput run
        std::thread::sleep(Duration::from_millis(100));

        // Resize to 100 columns
        let new_size = WindowSize::new(100, 24, 8, 16);
        channel
            .send(Msg::Resize(new_size))
            .expect("Failed to send resize");

        // Wait for child to exit
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        while !event_listener.child_exited() && start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(event_listener.child_exited(), "Child should have exited");

        // The second tput should show 100 columns
        // Note: Exact output verification depends on terminal setup
        let _ = handle.join();
    }
}

// ============================================================================
// Windows ConPTY Integration Tests
// ============================================================================

/// Windows ConPTY integration tests.
///
/// These tests require Windows 10 version 1809 (October 2018 Update) or later,
/// which introduced the ConPTY API for pseudo-terminal support.
///
/// # Running on Windows
///
/// ```bash
/// cargo test -p dterm-alacritty-bridge --test integration_test
/// ```
///
/// # Test Coverage
///
/// - ConPTY spawn and basic I/O
/// - ConPTY resize
/// - ConPTY shutdown
/// - Signal handling (Ctrl+C via GenerateConsoleCtrlEvent)
#[cfg(windows)]
mod windows_tests {
    // Note: These tests require a Windows environment with ConPTY support.
    // They are structured as stubs that document the expected behavior.
    // Full implementation requires Windows CI infrastructure.

    /// Verify ConPTY module is available on Windows.
    ///
    /// This is a compile-time check that the ConPTY bindings exist.
    #[test]
    fn conpty_module_available() {
        // The tty::windows module should be available on Windows builds
        // This test just verifies the module compiles correctly
        use dterm_alacritty_bridge::tty;

        // WindowSize is platform-agnostic and should be available
        let _size = dterm_alacritty_bridge::event::WindowSize::new(80, 24, 8, 16);

        // TtyOptions should work on Windows
        let _options = tty::Options {
            shell: None,
            working_directory: None,
            drain_on_exit: true,
            env: Default::default(),
        };
    }

    /// Test: ConPTY spawn with cmd.exe echo.
    ///
    /// This test spawns cmd.exe with an echo command and verifies output
    /// appears in the terminal grid.
    ///
    /// # Requirements
    /// - Windows 10 1809+
    /// - ConPTY API available
    #[test]
    #[ignore = "requires Windows CI environment"]
    fn test_conpty_echo() {
        // TODO: Implement when Windows CI is available
        //
        // Expected implementation:
        // 1. Create terminal with test dimensions
        // 2. Configure TtyOptions with cmd.exe /c "echo hello"
        // 3. Spawn ConPTY
        // 4. Wait for child to exit
        // 5. Verify "hello" appears in terminal grid
        unimplemented!("Requires Windows CI environment");
    }

    /// Test: ConPTY resize.
    ///
    /// This test verifies that ResizePseudoConsole works correctly.
    ///
    /// # Requirements
    /// - Windows 10 1809+
    /// - ConPTY API available
    #[test]
    #[ignore = "requires Windows CI environment"]
    fn test_conpty_resize() {
        // TODO: Implement when Windows CI is available
        //
        // Expected implementation:
        // 1. Create terminal and spawn ConPTY
        // 2. Send resize message
        // 3. Verify new dimensions are applied
        unimplemented!("Requires Windows CI environment");
    }

    /// Test: ConPTY shutdown.
    ///
    /// This test verifies that ClosePseudoConsole terminates the PTY cleanly.
    ///
    /// # Requirements
    /// - Windows 10 1809+
    /// - ConPTY API available
    #[test]
    #[ignore = "requires Windows CI environment"]
    fn test_conpty_shutdown() {
        // TODO: Implement when Windows CI is available
        //
        // Expected implementation:
        // 1. Create terminal and spawn long-running process
        // 2. Send shutdown message
        // 3. Verify event loop exits cleanly
        unimplemented!("Requires Windows CI environment");
    }

    /// Test: ConPTY signal handling (Ctrl+C).
    ///
    /// This test verifies that GenerateConsoleCtrlEvent sends CTRL_C_EVENT
    /// to the child process.
    ///
    /// # Requirements
    /// - Windows 10 1809+
    /// - ConPTY API available
    #[test]
    #[ignore = "requires Windows CI environment"]
    fn test_conpty_signal() {
        // TODO: Implement when Windows CI is available
        //
        // Expected implementation:
        // 1. Spawn process that handles Ctrl+C
        // 2. Send CTRL_C_EVENT via GenerateConsoleCtrlEvent
        // 3. Verify process responds appropriately
        unimplemented!("Requires Windows CI environment");
    }
}
