//! I/O driver for agent command execution.
//!
//! ## Phase 12: Async-Agnostic Execution I/O
//!
//! This module provides traits and implementations for driving I/O between
//! panes and terminals during command execution. The design is async-agnostic,
//! allowing higher layers to decide scheduling (sync polling, tokio, async-std, etc.).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │   Pane (PTY)    │────▶│  ExecutionDriver │────▶│    Terminal     │
//! │  (raw output)   │     │   (I/O bridge)   │     │  (parsed state) │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//!                                │
//!                                ▼
//!                         ┌─────────────┐
//!                         │  Execution  │
//!                         │ (stdout buf)│
//!                         └─────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dterm_core::agent::{ExecutionIoDriver, SlotExecutionDriver};
//!
//! // Create driver for a terminal slot
//! let mut driver = SlotExecutionDriver::new(&mut slot, &mut execution);
//!
//! // Poll in a loop (runtime decides when to call)
//! while !driver.is_complete() {
//!     let has_more = driver.poll()?;
//!     if !has_more {
//!         // No data available, runtime can yield/sleep
//!     }
//! }
//!
//! // Get exit status
//! if let Some(code) = driver.exit_status() {
//!     println!("Command exited with code: {}", code);
//! }
//! ```

use crate::domain::{DomainError, Pane};
use crate::terminal::Terminal;
use std::sync::Arc;

/// Result type for I/O driver operations.
pub type IoDriverResult<T> = Result<T, IoDriverError>;

/// Errors that can occur during execution I/O.
#[derive(Debug)]
pub enum IoDriverError {
    /// Domain-level I/O error.
    Domain(DomainError),
    /// No pane attached to drive.
    NoPaneAttached,
    /// No terminal attached to parse output.
    NoTerminalAttached,
    /// Read buffer error.
    BufferError(String),
}

impl std::fmt::Display for IoDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Domain(err) => write!(f, "domain error: {err}"),
            Self::NoPaneAttached => write!(f, "no pane attached to terminal slot"),
            Self::NoTerminalAttached => write!(f, "no terminal attached to terminal slot"),
            Self::BufferError(msg) => write!(f, "buffer error: {msg}"),
        }
    }
}

impl std::error::Error for IoDriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Domain(err) => Some(err),
            _ => None,
        }
    }
}

impl From<DomainError> for IoDriverError {
    fn from(err: DomainError) -> Self {
        Self::Domain(err)
    }
}

/// Trait for driving execution I/O in an async-agnostic way.
///
/// Implementations bridge pane output to terminal parsing and execution buffers.
/// The runtime layer decides when to call `poll()` - this could be in a sync loop,
/// a tokio task, or any other scheduling model.
///
/// ## Contract
///
/// - `poll()` is non-blocking and returns immediately with available data
/// - `poll()` returns `Ok(true)` if data was processed, `Ok(false)` if no data available
/// - `is_complete()` returns true once the underlying process has exited
/// - `exit_status()` returns the exit code after completion
pub trait ExecutionIoDriver: Send {
    /// Poll the pane for output and process it.
    ///
    /// Reads available data from the pane, feeds it to the terminal parser,
    /// and appends raw output to the execution buffer.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if data was read and processed
    /// - `Ok(false)` if no data was available (pane may still be alive)
    /// - `Err(_)` if an I/O error occurred
    fn poll(&mut self) -> IoDriverResult<bool>;

    /// Check if the execution has completed.
    ///
    /// Returns true when `pane.is_alive()` becomes false.
    fn is_complete(&self) -> bool;

    /// Get the exit status if completed.
    ///
    /// Returns `None` if the execution is still running or if the exit
    /// status is not available.
    fn exit_status(&self) -> Option<i32>;

    /// Write data to the pane (send input to the process).
    ///
    /// # Arguments
    ///
    /// * `data` - Bytes to write to the pane's input
    ///
    /// # Returns
    ///
    /// Number of bytes written, or an error.
    fn write(&self, data: &[u8]) -> IoDriverResult<usize>;

    /// Get the total bytes read so far.
    fn bytes_read(&self) -> usize;

    /// Get the total bytes written so far.
    fn bytes_written(&self) -> usize;
}

/// I/O driver that operates on a pane and terminal directly.
///
/// This driver reads from the pane, parses output through the terminal,
/// and captures raw output for the execution record. It does not own
/// the resources - they are borrowed for the duration of driving.
pub struct DirectIoDriver<'a> {
    /// The pane to read from.
    pane: &'a Arc<dyn Pane>,
    /// The terminal to parse output into.
    terminal: &'a mut Terminal,
    /// Buffer for raw output capture.
    output_buffer: &'a mut Vec<u8>,
    /// Read buffer for pane I/O.
    read_buf: Vec<u8>,
    /// Total bytes read.
    total_read: usize,
    /// Total bytes written.
    total_written: usize,
}

impl<'a> DirectIoDriver<'a> {
    /// Create a new direct I/O driver.
    ///
    /// # Arguments
    ///
    /// * `pane` - The pane to read output from
    /// * `terminal` - The terminal to parse output into
    /// * `output_buffer` - Buffer to capture raw output (usually `Execution.stdout`)
    ///
    /// # Returns
    ///
    /// A new driver ready for polling.
    pub fn new(
        pane: &'a Arc<dyn Pane>,
        terminal: &'a mut Terminal,
        output_buffer: &'a mut Vec<u8>,
    ) -> Self {
        Self {
            pane,
            terminal,
            output_buffer,
            read_buf: vec![0u8; 8192], // 8KB read buffer
            total_read: 0,
            total_written: 0,
        }
    }

    /// Create a driver with a custom read buffer size.
    pub fn with_buffer_size(
        pane: &'a Arc<dyn Pane>,
        terminal: &'a mut Terminal,
        output_buffer: &'a mut Vec<u8>,
        buffer_size: usize,
    ) -> Self {
        Self {
            pane,
            terminal,
            output_buffer,
            read_buf: vec![0u8; buffer_size],
            total_read: 0,
            total_written: 0,
        }
    }
}

impl ExecutionIoDriver for DirectIoDriver<'_> {
    fn poll(&mut self) -> IoDriverResult<bool> {
        // Read available data from pane
        let n = self.pane.read(&mut self.read_buf)?;

        if n == 0 {
            return Ok(false);
        }

        let data = &self.read_buf[..n];

        // Append to raw output buffer for execution record
        self.output_buffer.extend_from_slice(data);

        // Parse through terminal
        self.terminal.process(data);

        self.total_read += n;
        Ok(true)
    }

    fn is_complete(&self) -> bool {
        !self.pane.is_alive()
    }

    fn exit_status(&self) -> Option<i32> {
        self.pane.exit_status()
    }

    fn write(&self, data: &[u8]) -> IoDriverResult<usize> {
        let n = self.pane.write(data)?;
        Ok(n)
    }

    fn bytes_read(&self) -> usize {
        self.total_read
    }

    fn bytes_written(&self) -> usize {
        self.total_written
    }
}

/// Poll result indicating what happened during a poll cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollResult {
    /// Data was read and processed.
    DataProcessed(usize),
    /// No data available, but process is still running.
    NoData,
    /// Process has completed.
    Complete(Option<i32>),
}

impl PollResult {
    /// Returns true if data was processed.
    pub fn has_data(&self) -> bool {
        matches!(self, PollResult::DataProcessed(_))
    }

    /// Returns true if the process has completed.
    pub fn is_complete(&self) -> bool {
        matches!(self, PollResult::Complete(_))
    }

    /// Get the number of bytes processed, if any.
    pub fn bytes(&self) -> usize {
        match self {
            PollResult::DataProcessed(n) => *n,
            _ => 0,
        }
    }
}

/// Extended driver trait with richer poll results.
pub trait ExecutionIoDriverExt: ExecutionIoDriver {
    /// Poll with detailed result information.
    fn poll_ext(&mut self) -> IoDriverResult<PollResult>;
}

impl<T: ExecutionIoDriver> ExecutionIoDriverExt for T {
    fn poll_ext(&mut self) -> IoDriverResult<PollResult> {
        if self.is_complete() {
            return Ok(PollResult::Complete(self.exit_status()));
        }

        let before = self.bytes_read();
        let has_data = self.poll()?;

        if has_data {
            let bytes = self.bytes_read() - before;
            Ok(PollResult::DataProcessed(bytes))
        } else if self.is_complete() {
            Ok(PollResult::Complete(self.exit_status()))
        } else {
            Ok(PollResult::NoData)
        }
    }
}

/// Helper to drive an execution to completion synchronously.
///
/// This is useful for simple use cases or testing where async is not needed.
///
/// # Arguments
///
/// * `driver` - The I/O driver to poll
/// * `max_iterations` - Maximum poll iterations (safety limit)
///
/// # Returns
///
/// The exit status if the execution completed, or an error.
pub fn drive_to_completion<D: ExecutionIoDriver>(
    driver: &mut D,
    max_iterations: usize,
) -> IoDriverResult<Option<i32>> {
    let mut iterations = 0;

    while !driver.is_complete() && iterations < max_iterations {
        driver.poll()?;
        iterations += 1;
    }

    Ok(driver.exit_status())
}

/// I/O driver that operates on a `TerminalSlot`.
///
/// This driver extracts the pane and terminal from a slot and drives I/O
/// between them. It captures output to the provided execution's stdout buffer.
///
/// ## Usage
///
/// ```rust,ignore
/// use dterm_core::agent::{SlotExecutionDriver, TerminalSlot, Execution};
///
/// // Slot must have pane and terminal attached
/// let mut driver = SlotExecutionDriver::new(&mut slot, &mut execution)?;
///
/// while !driver.is_complete() {
///     driver.poll()?;
/// }
/// ```
pub struct SlotExecutionDriver<'a> {
    /// Reference to the pane from the slot.
    pane: Arc<dyn Pane>,
    /// Reference to the terminal from the slot.
    terminal: &'a mut Terminal,
    /// Execution stdout buffer.
    stdout: &'a mut Vec<u8>,
    /// Read buffer.
    read_buf: Vec<u8>,
    /// Total bytes read.
    total_read: usize,
    /// Total bytes written.
    total_written: usize,
}

impl<'a> SlotExecutionDriver<'a> {
    /// Create a new slot execution driver.
    ///
    /// # Arguments
    ///
    /// * `slot` - The terminal slot with attached pane and terminal
    /// * `execution` - The execution to capture output to
    ///
    /// # Errors
    ///
    /// Returns `IoDriverError::NoPaneAttached` if the slot has no pane.
    /// Returns `IoDriverError::NoTerminalAttached` if the slot has no terminal.
    pub fn new(
        slot: &'a mut super::TerminalSlot,
        execution: &'a mut super::Execution,
    ) -> IoDriverResult<Self> {
        let pane = slot.pane().cloned().ok_or(IoDriverError::NoPaneAttached)?;

        let terminal = slot
            .terminal_mut()
            .ok_or(IoDriverError::NoTerminalAttached)?;

        Ok(Self {
            pane,
            terminal,
            stdout: &mut execution.stdout,
            read_buf: vec![0u8; 8192],
            total_read: 0,
            total_written: 0,
        })
    }

    /// Create from individual components (for cases where you don't have a slot).
    pub fn from_components(
        pane: Arc<dyn Pane>,
        terminal: &'a mut Terminal,
        stdout: &'a mut Vec<u8>,
    ) -> Self {
        Self {
            pane,
            terminal,
            stdout,
            read_buf: vec![0u8; 8192],
            total_read: 0,
            total_written: 0,
        }
    }
}

impl ExecutionIoDriver for SlotExecutionDriver<'_> {
    fn poll(&mut self) -> IoDriverResult<bool> {
        let n = self.pane.read(&mut self.read_buf)?;

        if n == 0 {
            return Ok(false);
        }

        let data = &self.read_buf[..n];

        // Capture raw output
        self.stdout.extend_from_slice(data);

        // Parse through terminal
        self.terminal.process(data);

        self.total_read += n;
        Ok(true)
    }

    fn is_complete(&self) -> bool {
        !self.pane.is_alive()
    }

    fn exit_status(&self) -> Option<i32> {
        self.pane.exit_status()
    }

    fn write(&self, data: &[u8]) -> IoDriverResult<usize> {
        let n = self.pane.write(data)?;
        Ok(n)
    }

    fn bytes_read(&self) -> usize {
        self.total_read
    }

    fn bytes_written(&self) -> usize {
        self.total_written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DomainId, DomainResult, PaneId};
    use std::sync::Mutex;

    /// Mock pane for testing I/O driver.
    struct MockPaneForDriver {
        id: PaneId,
        domain_id: DomainId,
        alive: Mutex<bool>,
        exit_status: Mutex<Option<i32>>,
        output_queue: Mutex<Vec<Vec<u8>>>,
        written: Mutex<Vec<u8>>,
    }

    impl MockPaneForDriver {
        fn new() -> Self {
            Self {
                id: PaneId::new(),
                domain_id: DomainId::new(),
                alive: Mutex::new(true),
                exit_status: Mutex::new(None),
                output_queue: Mutex::new(Vec::new()),
                written: Mutex::new(Vec::new()),
            }
        }

        fn queue_output(&self, data: &[u8]) {
            self.output_queue.lock().unwrap().push(data.to_vec());
        }

        fn set_exit(&self, code: i32) {
            *self.alive.lock().unwrap() = false;
            *self.exit_status.lock().unwrap() = Some(code);
        }

        fn get_written(&self) -> Vec<u8> {
            self.written.lock().unwrap().clone()
        }
    }

    impl Pane for MockPaneForDriver {
        fn pane_id(&self) -> PaneId {
            self.id
        }

        fn domain_id(&self) -> DomainId {
            self.domain_id
        }

        fn size(&self) -> (u16, u16) {
            (80, 24)
        }

        fn resize(&self, _cols: u16, _rows: u16) -> DomainResult<()> {
            Ok(())
        }

        fn write(&self, data: &[u8]) -> DomainResult<usize> {
            self.written.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }

        fn read(&self, buf: &mut [u8]) -> DomainResult<usize> {
            let mut queue = self.output_queue.lock().unwrap();
            if let Some(data) = queue.first() {
                let len = data.len().min(buf.len());
                buf[..len].copy_from_slice(&data[..len]);
                if len == data.len() {
                    queue.remove(0);
                } else {
                    queue[0] = data[len..].to_vec();
                }
                Ok(len)
            } else {
                Ok(0)
            }
        }

        fn is_alive(&self) -> bool {
            *self.alive.lock().unwrap()
        }

        fn exit_status(&self) -> Option<i32> {
            *self.exit_status.lock().unwrap()
        }

        fn kill(&self) -> DomainResult<()> {
            self.set_exit(-9);
            Ok(())
        }
    }

    #[test]
    fn test_direct_io_driver_poll() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"Hello, ");
        mock_pane.queue_output(b"World!\n");

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut output = Vec::new();

        let mut driver = DirectIoDriver::new(&pane, &mut terminal, &mut output);

        // First poll - should get "Hello, "
        assert!(driver.poll().unwrap());
        assert_eq!(driver.bytes_read(), 7);

        // Second poll - should get "World!\n"
        assert!(driver.poll().unwrap());
        assert_eq!(driver.bytes_read(), 14);

        // Third poll - no data
        assert!(!driver.poll().unwrap());

        // Output buffer should have all data
        assert_eq!(output, b"Hello, World!\n");
    }

    #[test]
    fn test_direct_io_driver_completion() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"output\n");
        mock_pane.set_exit(42);

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut output = Vec::new();

        let mut driver = DirectIoDriver::new(&pane, &mut terminal, &mut output);

        // Process output
        driver.poll().unwrap();

        // Should be complete
        assert!(driver.is_complete());
        assert_eq!(driver.exit_status(), Some(42));
    }

    #[test]
    fn test_direct_io_driver_write() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut output = Vec::new();

        let driver = DirectIoDriver::new(&pane, &mut terminal, &mut output);

        // Write to pane
        let n = driver.write(b"echo hello\n").unwrap();
        assert_eq!(n, 11);

        // Verify pane received it
        assert_eq!(mock_pane.get_written(), b"echo hello\n");
    }

    #[test]
    fn test_poll_result() {
        assert!(PollResult::DataProcessed(10).has_data());
        assert!(!PollResult::NoData.has_data());
        assert!(!PollResult::Complete(Some(0)).has_data());

        assert!(PollResult::Complete(Some(0)).is_complete());
        assert!(PollResult::Complete(None).is_complete());
        assert!(!PollResult::DataProcessed(10).is_complete());
        assert!(!PollResult::NoData.is_complete());

        assert_eq!(PollResult::DataProcessed(42).bytes(), 42);
        assert_eq!(PollResult::NoData.bytes(), 0);
        assert_eq!(PollResult::Complete(Some(0)).bytes(), 0);
    }

    #[test]
    fn test_poll_ext() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"data");

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut output = Vec::new();

        let mut driver = DirectIoDriver::new(&pane, &mut terminal, &mut output);

        // Poll with data
        let result = driver.poll_ext().unwrap();
        assert!(matches!(result, PollResult::DataProcessed(4)));

        // Poll without data
        let result = driver.poll_ext().unwrap();
        assert!(matches!(result, PollResult::NoData));

        // Set complete
        mock_pane.set_exit(0);
        let result = driver.poll_ext().unwrap();
        assert!(matches!(result, PollResult::Complete(Some(0))));
    }

    #[test]
    fn test_drive_to_completion() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"line 1\n");
        mock_pane.queue_output(b"line 2\n");

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut output = Vec::new();

        {
            let mut driver = DirectIoDriver::new(&pane, &mut terminal, &mut output);

            // Poll until we've read output
            driver.poll().unwrap();
            driver.poll().unwrap();

            // Now set exit
            mock_pane.set_exit(0);

            let exit = drive_to_completion(&mut driver, 100).unwrap();
            assert_eq!(exit, Some(0));
        }

        assert_eq!(output, b"line 1\nline 2\n");
    }

    #[test]
    fn test_drive_to_completion_max_iterations() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut output = Vec::new();

        // Pane never exits, so we hit max iterations
        let mut driver = DirectIoDriver::new(&pane, &mut terminal, &mut output);

        let exit = drive_to_completion(&mut driver, 10).unwrap();
        assert_eq!(exit, None); // Still running
    }

    #[test]
    fn test_io_driver_error_display() {
        let err = IoDriverError::NoPaneAttached;
        assert_eq!(err.to_string(), "no pane attached to terminal slot");

        let err = IoDriverError::NoTerminalAttached;
        assert_eq!(err.to_string(), "no terminal attached to terminal slot");

        let err = IoDriverError::BufferError("overflow".to_string());
        assert_eq!(err.to_string(), "buffer error: overflow");

        let domain_err = DomainError::SpawnFailed("test".to_string());
        let err = IoDriverError::Domain(domain_err);
        assert!(err.to_string().contains("domain error"));
    }

    #[test]
    fn test_custom_buffer_size() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut output = Vec::new();

        // Create with small buffer
        let driver = DirectIoDriver::with_buffer_size(&pane, &mut terminal, &mut output, 256);

        // Verify buffer size
        assert_eq!(driver.read_buf.len(), 256);
    }

    // =========================================================================
    // SlotExecutionDriver Tests
    // =========================================================================

    #[test]
    fn test_slot_driver_from_components() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut stdout = Vec::new();

        let driver = SlotExecutionDriver::from_components(pane, &mut terminal, &mut stdout);

        // Initially not complete
        assert!(!driver.is_complete());
        assert_eq!(driver.exit_status(), None);
        assert_eq!(driver.bytes_read(), 0);
    }

    #[test]
    fn test_slot_driver_poll_and_capture() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"Hello from pane!\n");

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut stdout = Vec::new();

        {
            let mut driver = SlotExecutionDriver::from_components(pane, &mut terminal, &mut stdout);

            // Poll should capture output
            assert!(driver.poll().unwrap());
            assert_eq!(driver.bytes_read(), 17);

            // No more data
            assert!(!driver.poll().unwrap());
        }

        // stdout should have captured data (check after driver is dropped)
        assert_eq!(stdout, b"Hello from pane!\n");
    }

    #[test]
    fn test_slot_driver_write() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut stdout = Vec::new();

        let driver = SlotExecutionDriver::from_components(pane, &mut terminal, &mut stdout);

        // Write to pane
        let n = driver.write(b"input data\n").unwrap();
        assert_eq!(n, 11);

        // Verify pane received it
        assert_eq!(mock_pane.get_written(), b"input data\n");
    }

    #[test]
    fn test_slot_driver_completion() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"final output\n");
        mock_pane.set_exit(42);

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut stdout = Vec::new();

        {
            let mut driver = SlotExecutionDriver::from_components(pane, &mut terminal, &mut stdout);

            // Poll until complete
            driver.poll().unwrap();

            assert!(driver.is_complete());
            assert_eq!(driver.exit_status(), Some(42));
        }

        // Check stdout after driver is dropped
        assert_eq!(stdout, b"final output\n");
    }

    #[test]
    fn test_slot_driver_multiple_chunks() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"chunk1");
        mock_pane.queue_output(b"chunk2");
        mock_pane.queue_output(b"chunk3");

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut stdout = Vec::new();

        {
            let mut driver = SlotExecutionDriver::from_components(pane, &mut terminal, &mut stdout);

            // Poll three times
            assert!(driver.poll().unwrap());
            assert!(driver.poll().unwrap());
            assert!(driver.poll().unwrap());
            assert!(!driver.poll().unwrap()); // No more data

            assert_eq!(driver.bytes_read(), 18);
        }

        // Check stdout after driver is dropped
        assert_eq!(stdout, b"chunk1chunk2chunk3");
    }

    #[test]
    fn test_slot_driver_with_slot_and_execution() {
        use super::super::{
            AgentId, CommandId, Execution, ExecutionId, TerminalSlot, TerminalSlotId,
        };

        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"slot output\n");

        // Create slot with resources
        let mut slot = TerminalSlot::new(TerminalSlotId(0));
        let domain_id = DomainId::new();
        slot.attach_pane(mock_pane.clone() as Arc<dyn Pane>, domain_id);
        slot.attach_terminal(Terminal::new(80, 24));

        // Create execution
        let mut execution =
            Execution::new(ExecutionId(1), AgentId(1), CommandId(1), TerminalSlotId(0));

        // Create driver from slot
        let mut driver = SlotExecutionDriver::new(&mut slot, &mut execution).unwrap();

        // Poll
        assert!(driver.poll().unwrap());
        assert_eq!(driver.bytes_read(), 12);

        // Execution stdout should have output
        // Note: We need to drop driver to access execution
        drop(driver);
        assert_eq!(execution.stdout, b"slot output\n");
    }

    #[test]
    fn test_slot_driver_no_pane_error() {
        use super::super::{
            AgentId, CommandId, Execution, ExecutionId, TerminalSlot, TerminalSlotId,
        };

        // Create slot WITHOUT pane
        let mut slot = TerminalSlot::new(TerminalSlotId(0));
        slot.attach_terminal(Terminal::new(80, 24));

        let mut execution =
            Execution::new(ExecutionId(1), AgentId(1), CommandId(1), TerminalSlotId(0));

        // Should fail
        let result = SlotExecutionDriver::new(&mut slot, &mut execution);
        assert!(matches!(result, Err(IoDriverError::NoPaneAttached)));
    }

    #[test]
    fn test_slot_driver_no_terminal_error() {
        use super::super::{
            AgentId, CommandId, Execution, ExecutionId, TerminalSlot, TerminalSlotId,
        };

        let mock_pane = Arc::new(MockPaneForDriver::new());

        // Create slot WITHOUT terminal
        let mut slot = TerminalSlot::new(TerminalSlotId(0));
        let domain_id = DomainId::new();
        slot.attach_pane(mock_pane as Arc<dyn Pane>, domain_id);
        // NOT attaching terminal

        let mut execution =
            Execution::new(ExecutionId(1), AgentId(1), CommandId(1), TerminalSlotId(0));

        // Should fail
        let result = SlotExecutionDriver::new(&mut slot, &mut execution);
        assert!(matches!(result, Err(IoDriverError::NoTerminalAttached)));
    }

    #[test]
    fn test_slot_driver_poll_ext() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"data");

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut stdout = Vec::new();

        {
            let mut driver = SlotExecutionDriver::from_components(pane, &mut terminal, &mut stdout);

            // Poll with data
            let result = driver.poll_ext().unwrap();
            assert!(matches!(result, PollResult::DataProcessed(4)));

            // Poll without data
            let result = driver.poll_ext().unwrap();
            assert!(matches!(result, PollResult::NoData));

            // Set complete
            mock_pane.set_exit(0);
            let result = driver.poll_ext().unwrap();
            assert!(matches!(result, PollResult::Complete(Some(0))));
        }
    }

    #[test]
    fn test_slot_driver_drive_to_completion() {
        let mock_pane = Arc::new(MockPaneForDriver::new());
        mock_pane.queue_output(b"line 1\n");
        mock_pane.queue_output(b"line 2\n");

        let pane: Arc<dyn Pane> = mock_pane.clone();
        let mut terminal = Terminal::new(80, 24);
        let mut stdout = Vec::new();

        {
            let mut driver = SlotExecutionDriver::from_components(pane, &mut terminal, &mut stdout);

            // Poll until we've read output
            driver.poll().unwrap();
            driver.poll().unwrap();

            // Now set exit
            mock_pane.set_exit(0);

            let exit = drive_to_completion(&mut driver, 100).unwrap();
            assert_eq!(exit, Some(0));
        }

        // Check stdout after driver is dropped
        assert_eq!(stdout, b"line 1\nline 2\n");
    }
}
