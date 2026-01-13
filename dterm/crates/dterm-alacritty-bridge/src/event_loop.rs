//! The main event loop which performs I/O on the pseudoterminal.
//!
//! This module provides the event loop that ties together PTY I/O and terminal
//! parsing. It follows the same patterns as Alacritty's event loop for
//! compatibility.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt::{self, Display, Formatter};
use std::io::{self, ErrorKind, Read, Write};
use std::num::NonZeroUsize;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;

use log::error;
use polling::{Event as PollingEvent, Events, PollMode};

use crate::event::{self, Event, EventListener, WindowSize};
use crate::sync::FairMutex;
use crate::term::Term;
use crate::tty::{self, EventedPty, OnResize};

/// Max bytes to read from the PTY before forced terminal synchronization.
pub const READ_BUFFER_SIZE: usize = 0x10_0000; // 1MB

/// Max bytes to read from the PTY while the terminal is locked.
const MAX_LOCKED_READ: usize = u16::MAX as usize; // 64KB

/// Messages that may be sent to the `EventLoop`.
#[derive(Debug)]
pub enum Msg {
    /// Data that should be written to the PTY.
    Input(Cow<'static, [u8]>),

    /// Indicates that the `EventLoop` should shut down.
    Shutdown,

    /// Instruction to resize the PTY.
    Resize(WindowSize),
}

/// The main event loop.
///
/// Handles all the PTY I/O and runs terminal processing which updates terminal
/// state. The event loop runs in a dedicated thread and communicates with the
/// main thread via message passing.
///
/// # Type Parameters
///
/// * `T` - The PTY type, must implement `EventedPty` for I/O and `OnResize`
/// * `U` - The event listener type for notifying the UI of terminal events
pub struct EventLoop<T: EventedPty, U: EventListener> {
    poll: Arc<polling::Poller>,
    pty: T,
    rx: PeekableReceiver<Msg>,
    tx: Sender<Msg>,
    terminal: Arc<FairMutex<Term<U>>>,
    event_proxy: U,
    drain_on_exit: bool,
}

impl<T, U> EventLoop<T, U>
where
    T: EventedPty + OnResize + Send + 'static,
    U: EventListener + Clone + Send + Sync + 'static,
{
    /// Create a new event loop.
    ///
    /// # Arguments
    ///
    /// * `terminal` - Shared terminal state protected by a fair mutex
    /// * `event_proxy` - Event listener for UI notifications
    /// * `pty` - The PTY handle for I/O
    /// * `drain_on_exit` - Whether to drain PTY output before exiting
    pub fn new(
        terminal: Arc<FairMutex<Term<U>>>,
        event_proxy: U,
        pty: T,
        drain_on_exit: bool,
    ) -> io::Result<EventLoop<T, U>> {
        let (tx, rx) = mpsc::channel();
        let poll = polling::Poller::new()?.into();
        Ok(EventLoop {
            poll,
            pty,
            tx,
            rx: PeekableReceiver::new(rx),
            terminal,
            event_proxy,
            drain_on_exit,
        })
    }

    /// Get a sender for communicating with the event loop.
    ///
    /// The returned `EventLoopSender` can be cloned and sent to other threads.
    pub fn channel(&self) -> EventLoopSender {
        EventLoopSender {
            sender: self.tx.clone(),
            poller: self.poll.clone(),
        }
    }

    /// Drain the message channel.
    ///
    /// Returns `false` when a shutdown message was received.
    fn drain_recv_channel(&mut self, state: &mut State) -> bool {
        while let Some(msg) = self.rx.recv() {
            match msg {
                Msg::Input(input) => state.write_list.push_back(input),
                Msg::Resize(window_size) => self.pty.on_resize(window_size),
                Msg::Shutdown => return false,
            }
        }

        true
    }

    /// Read from the PTY and process through the terminal.
    #[inline]
    fn pty_read(&mut self, _state: &mut State, buf: &mut [u8]) -> io::Result<()> {
        let mut unprocessed = 0;
        let mut processed = 0;

        // Reserve the next terminal lock for PTY reading.
        let _terminal_lease = Some(self.terminal.lease());
        let mut terminal = None;

        loop {
            // Read from the PTY.
            match self.pty.reader().read(&mut buf[unprocessed..]) {
                // This is received on Windows/macOS when no more data is readable from the PTY.
                Ok(0) if unprocessed == 0 => break,
                Ok(got) => unprocessed += got,
                Err(err) => match err.kind() {
                    ErrorKind::Interrupted | ErrorKind::WouldBlock => {
                        // Go back to polling if we're caught up on parsing and the PTY would block.
                        if unprocessed == 0 {
                            break;
                        }
                    }
                    _ => return Err(err),
                },
            }

            // Attempt to lock the terminal.
            let terminal = match &mut terminal {
                Some(terminal) => terminal,
                None => terminal.insert(match self.terminal.try_lock_unfair() {
                    // Force block if we are at the buffer size limit.
                    None if unprocessed >= READ_BUFFER_SIZE => self.terminal.lock_unfair(),
                    None => continue,
                    Some(terminal) => terminal,
                }),
            };

            // Parse the incoming bytes through dterm-core's terminal.
            terminal.process(&buf[..unprocessed]);

            processed += unprocessed;
            unprocessed = 0;

            // Assure we're not blocking the terminal too long unnecessarily.
            if processed >= MAX_LOCKED_READ {
                break;
            }
        }

        // Queue terminal redraw if we processed any data.
        if processed > 0 {
            self.event_proxy.send_event(Event::Wakeup);
        }

        Ok(())
    }

    /// Write pending data to the PTY.
    #[inline]
    fn pty_write(&mut self, state: &mut State) -> io::Result<()> {
        state.ensure_next();

        'write_many: while let Some(mut current) = state.take_current() {
            'write_one: loop {
                match self.pty.writer().write(current.remaining_bytes()) {
                    Ok(0) => {
                        state.set_current(Some(current));
                        break 'write_many;
                    }
                    Ok(n) => {
                        current.advance(n);
                        if current.finished() {
                            state.goto_next();
                            break 'write_one;
                        }
                    }
                    Err(err) => {
                        state.set_current(Some(current));
                        match err.kind() {
                            ErrorKind::Interrupted | ErrorKind::WouldBlock => break 'write_many,
                            _ => return Err(err),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Spawn the event loop on a dedicated thread.
    ///
    /// Returns a `JoinHandle` that can be used to wait for the thread to complete
    /// and retrieve the final state.
    pub fn spawn(mut self) -> JoinHandle<(Self, State)> {
        std::thread::Builder::new()
            .name("PTY reader".into())
            .spawn(move || {
                let mut state = State::default();
                let mut buf = [0u8; READ_BUFFER_SIZE];

                let poll_opts = PollMode::Level;
                let mut interest = PollingEvent::readable(0);

                // Register TTY through EventedRW interface.
                if let Err(err) = unsafe { self.pty.register(&self.poll, interest, poll_opts) } {
                    error!("Event loop registration error: {err}");
                    return (self, state);
                }

                let mut events = Events::with_capacity(NonZeroUsize::new(1024).unwrap());

                'event_loop: loop {
                    events.clear();

                    // Calculate poll timeout based on synchronized output mode.
                    // If sync mode is active, we need to wake up to check the timeout.
                    let sync_timeout = {
                        let terminal = self.terminal.lock();
                        terminal.sync_timeout()
                    };
                    let poll_timeout = sync_timeout.map(|deadline| {
                        deadline.saturating_duration_since(std::time::Instant::now())
                    });

                    if let Err(err) = self.poll.wait(&mut events, poll_timeout) {
                        match err.kind() {
                            ErrorKind::Interrupted => continue,
                            _ => {
                                error!("Event loop polling error: {err}");
                                break 'event_loop;
                            }
                        }
                    }

                    // Check if synchronized output mode timed out.
                    // This can happen if the poll timed out due to sync deadline.
                    if events.is_empty() && self.rx.peek().is_none() {
                        if sync_timeout.is_some_and(|d| d <= std::time::Instant::now()) {
                            // Sync mode timed out - force it off to prevent screen freeze.
                            self.terminal.lock().stop_sync();
                            self.event_proxy.send_event(Event::Wakeup);
                        }
                        continue;
                    }

                    // Handle channel events first (input, resize, shutdown).
                    if !self.drain_recv_channel(&mut state) {
                        break;
                    }

                    for event in events.iter() {
                        match event.key {
                            tty::PTY_CHILD_EVENT_TOKEN => {
                                if let Some(tty::ChildEvent::Exited(code)) =
                                    self.pty.next_child_event()
                                {
                                    if let Some(code) = code {
                                        self.event_proxy.send_event(Event::ChildExit(code));
                                    }
                                    if self.drain_on_exit {
                                        let _ = self.pty_read(&mut state, &mut buf);
                                    }
                                    self.event_proxy.send_event(Event::Wakeup);
                                    break 'event_loop;
                                }
                            }

                            tty::PTY_READ_WRITE_TOKEN => {
                                if event.is_interrupt() {
                                    // Don't try to do I/O on a dead PTY.
                                    continue;
                                }

                                if event.readable {
                                    if let Err(err) = self.pty_read(&mut state, &mut buf) {
                                        // On Linux, a `read` on the master side of a PTY can fail
                                        // with `EIO` if the client side hangs up. In that case,
                                        // just loop back round for the inevitable `Exited` event.
                                        #[cfg(target_os = "linux")]
                                        if err.raw_os_error() == Some(libc::EIO) {
                                            continue;
                                        }

                                        error!("Error reading from PTY in event loop: {err}");
                                        break 'event_loop;
                                    }
                                }

                                if event.writable {
                                    if let Err(err) = self.pty_write(&mut state) {
                                        error!("Error writing to PTY in event loop: {err}");
                                        break 'event_loop;
                                    }
                                }
                            }
                            _ => (),
                        }
                    }

                    // Register write interest if necessary.
                    let needs_write = state.needs_write();
                    if needs_write != interest.writable {
                        interest.writable = needs_write;

                        // Re-register with new interest.
                        if let Err(err) = self.pty.reregister(&self.poll, interest, poll_opts) {
                            error!("Failed to re-register PTY: {err}");
                            break 'event_loop;
                        }
                    }
                }

                // The evented instances are not dropped here so deregister them explicitly.
                let _ = self.pty.deregister(&self.poll);

                (self, state)
            })
            .expect("failed to spawn PTY reader thread")
    }
}

/// Helper type which tracks how much of a buffer has been written.
struct Writing {
    source: Cow<'static, [u8]>,
    written: usize,
}

impl Writing {
    #[inline]
    fn new(c: Cow<'static, [u8]>) -> Writing {
        Writing {
            source: c,
            written: 0,
        }
    }

    #[inline]
    fn advance(&mut self, n: usize) {
        self.written += n;
    }

    #[inline]
    fn remaining_bytes(&self) -> &[u8] {
        &self.source[self.written..]
    }

    #[inline]
    fn finished(&self) -> bool {
        self.written >= self.source.len()
    }
}

/// Notifier for sending data to the PTY via the event loop.
///
/// Implements `Notify` and `OnResize` traits for integration with terminal
/// input handling.
pub struct Notifier(pub EventLoopSender);

impl event::Notify for Notifier {
    fn notify<B>(&self, bytes: B)
    where
        B: Into<Cow<'static, [u8]>>,
    {
        let bytes = bytes.into();
        // Terminal hangs if we send 0 bytes through.
        if bytes.is_empty() {
            return;
        }

        let _ = self.0.send(Msg::Input(bytes));
    }
}

impl event::OnResize for Notifier {
    fn on_resize(&mut self, window_size: WindowSize) {
        let _ = self.0.send(Msg::Resize(window_size));
    }
}

/// Error type for event loop send operations.
#[derive(Debug)]
pub enum EventLoopSendError {
    /// Error polling the event loop.
    Io(io::Error),

    /// Error sending a message to the event loop.
    Send(mpsc::SendError<Msg>),
}

impl Display for EventLoopSendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            EventLoopSendError::Io(err) => err.fmt(f),
            EventLoopSendError::Send(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for EventLoopSendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EventLoopSendError::Io(err) => err.source(),
            EventLoopSendError::Send(err) => err.source(),
        }
    }
}

/// Sender handle for communicating with the event loop.
///
/// This is a cloneable handle that can be shared across threads to send
/// messages to the event loop.
#[derive(Clone)]
pub struct EventLoopSender {
    sender: Sender<Msg>,
    poller: Arc<polling::Poller>,
}

impl EventLoopSender {
    /// Send a message to the event loop.
    ///
    /// Wakes up the event loop after sending the message.
    pub fn send(&self, msg: Msg) -> Result<(), EventLoopSendError> {
        self.sender.send(msg).map_err(EventLoopSendError::Send)?;
        self.poller.notify().map_err(EventLoopSendError::Io)
    }
}

/// All of the mutable state needed to run the event loop.
///
/// Contains list of items to write, current write state, etc. Anything that
/// would otherwise be mutated on the `EventLoop` goes here.
#[derive(Default)]
pub struct State {
    write_list: VecDeque<Cow<'static, [u8]>>,
    writing: Option<Writing>,
}

impl State {
    #[inline]
    fn ensure_next(&mut self) {
        if self.writing.is_none() {
            self.goto_next();
        }
    }

    #[inline]
    fn goto_next(&mut self) {
        self.writing = self.write_list.pop_front().map(Writing::new);
    }

    #[inline]
    fn take_current(&mut self) -> Option<Writing> {
        self.writing.take()
    }

    #[inline]
    fn needs_write(&self) -> bool {
        self.writing.is_some() || !self.write_list.is_empty()
    }

    #[inline]
    fn set_current(&mut self, new: Option<Writing>) {
        self.writing = new;
    }
}

/// Receiver that supports peeking at the next message.
struct PeekableReceiver<T> {
    rx: Receiver<T>,
    peeked: Option<T>,
}

impl<T> PeekableReceiver<T> {
    fn new(rx: Receiver<T>) -> Self {
        Self { rx, peeked: None }
    }

    #[allow(dead_code)] // Used for synchronized update timeout checking (future use)
    fn peek(&mut self) -> Option<&T> {
        if self.peeked.is_none() {
            self.peeked = self.rx.try_recv().ok();
        }

        self.peeked.as_ref()
    }

    fn recv(&mut self) -> Option<T> {
        if self.peeked.is_some() {
            self.peeked.take()
        } else {
            match self.rx.try_recv() {
                Err(TryRecvError::Disconnected) => None,
                res => res.ok(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writing_new() {
        let data: Cow<'static, [u8]> = Cow::Borrowed(b"hello");
        let writing = Writing::new(data);
        assert_eq!(writing.remaining_bytes(), b"hello");
        assert!(!writing.finished());
    }

    #[test]
    fn test_writing_advance() {
        let data: Cow<'static, [u8]> = Cow::Borrowed(b"hello");
        let mut writing = Writing::new(data);

        writing.advance(2);
        assert_eq!(writing.remaining_bytes(), b"llo");
        assert!(!writing.finished());

        writing.advance(3);
        assert_eq!(writing.remaining_bytes(), b"");
        assert!(writing.finished());
    }

    #[test]
    fn test_state_write_list() {
        let mut state = State::default();
        assert!(!state.needs_write());

        state.write_list.push_back(Cow::Borrowed(b"test"));
        assert!(state.needs_write());

        state.ensure_next();
        assert!(state.writing.is_some());

        let writing = state.take_current().unwrap();
        assert_eq!(writing.remaining_bytes(), b"test");
    }

    #[test]
    fn test_peekable_receiver() {
        let (tx, rx) = mpsc::channel();
        let mut peekable = PeekableReceiver::new(rx);

        tx.send(42).unwrap();

        assert_eq!(peekable.peek(), Some(&42));
        assert_eq!(peekable.peek(), Some(&42)); // Peek again should return same value
        assert_eq!(peekable.recv(), Some(42));
        assert_eq!(peekable.recv(), None);
    }
}
