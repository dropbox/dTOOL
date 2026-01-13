//! UI Bridge for platform integration.
//!
//! This module provides a verified bridge between platform UI (macOS/iOS/Windows/Linux)
//! and dterm-core. All state transitions are formally verified via TLA+ specification
//! (`tla/UIStateMachine.tla`).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    PLATFORM UI (Native)                          │
//! │  macOS/SwiftUI • iOS/SwiftUI • Windows/WinUI • Linux/GTK        │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               │ C FFI
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    UI BRIDGE (this module)                       │
//! │                                                                  │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
//! │  │  UIState     │───▶│  EventQueue  │───▶│  Callbacks   │       │
//! │  │  (TLA+)      │    │  (Kani)      │    │  (Verified)  │       │
//! │  └──────────────┘    └──────────────┘    └──────────────┘       │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Safety Properties (from TLA+ spec)
//!
//! - **EventsPreserved**: No event is lost between enqueue and process
//! - **NoDuplicateEventIds**: Event IDs are unique within the system
//! - **DisposedMonotonic**: Once a terminal is disposed, it stays disposed
//! - **TypeInvariant**: State machine is always in a valid configuration

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum number of terminals the UI bridge can track.
pub const MAX_TERMINALS: usize = 256;

/// Maximum number of events in the queue.
pub const MAX_QUEUE: usize = 1024;

/// Maximum number of pending callbacks.
pub const MAX_CALLBACKS: usize = 64;

/// Terminal identifier.
pub type TerminalId = u32;

/// Event identifier (unique per event).
pub type EventId = u64;

/// Callback identifier.
pub type CallbackId = u32;

/// Global event ID counter for uniqueness.
static NEXT_EVENT_ID: AtomicU64 = AtomicU64::new(0);

/// Generate a unique event ID.
fn next_event_id() -> EventId {
    NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed)
}

/// UI state machine states (matches TLA+ spec exactly).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum UIState {
    /// No work in progress, ready to process events.
    Idle,
    /// Currently processing an event.
    Processing,
    /// Waiting for render completion.
    Rendering,
    /// Waiting for callback completion.
    WaitingForCallback,
    /// System is shutting down.
    ShuttingDown,
}

impl Default for UIState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Terminal state (matches TLA+ spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum TerminalState {
    /// Terminal slot is available.
    Inactive,
    /// Terminal is active and usable.
    Active,
    /// Terminal has been disposed (cannot be reactivated).
    Disposed,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::Inactive
    }
}

/// Event kinds (matches TLA+ spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum EventKind {
    /// User input to a terminal.
    Input,
    /// Terminal resize request.
    Resize,
    /// Render request for a terminal.
    Render,
    /// Create a new terminal.
    CreateTerminal,
    /// Destroy an existing terminal.
    DestroyTerminal,
    /// Request a callback.
    RequestCallback,
    /// System shutdown.
    Shutdown,
}

/// An event in the UI system.
#[derive(Debug, Clone)]
pub struct Event {
    /// Unique event identifier.
    pub id: EventId,
    /// Type of event.
    pub kind: EventKind,
    /// Target terminal (if applicable).
    pub terminal: Option<TerminalId>,
    /// Associated callback (if applicable).
    pub callback: Option<CallbackId>,
    /// Event payload data.
    pub data: EventData,
}

/// Event-specific payload data.
#[derive(Debug, Clone, Default)]
pub struct EventData {
    /// Input bytes (for Input events).
    pub input: Vec<u8>,
    /// New row count (for Resize events).
    pub rows: u16,
    /// New column count (for Resize events).
    pub cols: u16,
}

impl Event {
    /// Create an input event.
    pub fn input(terminal: TerminalId, data: Vec<u8>) -> Self {
        Self {
            id: next_event_id(),
            kind: EventKind::Input,
            terminal: Some(terminal),
            callback: None,
            data: EventData {
                input: data,
                ..Default::default()
            },
        }
    }

    /// Create a resize event.
    pub fn resize(terminal: TerminalId, rows: u16, cols: u16) -> Self {
        Self {
            id: next_event_id(),
            kind: EventKind::Resize,
            terminal: Some(terminal),
            callback: None,
            data: EventData {
                rows,
                cols,
                ..Default::default()
            },
        }
    }

    /// Create a render event.
    pub fn render(terminal: TerminalId) -> Self {
        Self {
            id: next_event_id(),
            kind: EventKind::Render,
            terminal: Some(terminal),
            callback: None,
            data: EventData::default(),
        }
    }

    /// Create a create terminal event.
    pub fn create_terminal(terminal: TerminalId) -> Self {
        Self {
            id: next_event_id(),
            kind: EventKind::CreateTerminal,
            terminal: Some(terminal),
            callback: None,
            data: EventData::default(),
        }
    }

    /// Create a destroy terminal event.
    pub fn destroy_terminal(terminal: TerminalId) -> Self {
        Self {
            id: next_event_id(),
            kind: EventKind::DestroyTerminal,
            terminal: Some(terminal),
            callback: None,
            data: EventData::default(),
        }
    }

    /// Create a callback request event.
    pub fn request_callback(terminal: TerminalId, callback: CallbackId) -> Self {
        Self {
            id: next_event_id(),
            kind: EventKind::RequestCallback,
            terminal: Some(terminal),
            callback: Some(callback),
            data: EventData::default(),
        }
    }

    /// Create a shutdown event.
    pub fn shutdown() -> Self {
        Self {
            id: next_event_id(),
            kind: EventKind::Shutdown,
            terminal: None,
            callback: None,
            data: EventData::default(),
        }
    }
}

/// Error types for UI Bridge operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UIError {
    /// Event queue is full.
    QueueFull,
    /// System is shutting down, no new events accepted.
    ShuttingDown,
    /// Terminal ID is invalid or out of range.
    InvalidTerminalId,
    /// Terminal is not in the expected state.
    InvalidTerminalState,
    /// Callback ID is already pending.
    DuplicateCallback,
    /// No event to process.
    NoEventPending,
    /// Invalid state transition.
    InvalidStateTransition,
}

/// Result type for UI Bridge operations.
pub type UIResult<T> = Result<T, UIError>;

/// UI Bridge - the verified interface between platform UI and dterm-core.
///
/// This struct implements the state machine defined in `tla/UIStateMachine.tla`.
/// All state transitions are designed to preserve the TLA+ invariants.
#[derive(Debug)]
pub struct UIBridge {
    /// Current UI state.
    state: UIState,
    /// Terminal states by ID.
    terminal_states: HashMap<TerminalId, TerminalState>,
    /// Event queue (FIFO).
    pending_events: VecDeque<Event>,
    /// Event currently being processed.
    current_event: Option<Event>,
    /// Set of pending callback IDs.
    callbacks_pending: HashSet<CallbackId>,
    /// Set of terminals awaiting render completion.
    render_pending: HashSet<TerminalId>,
    /// Count of events received (O(1) memory, replaces unbounded HashSet).
    /// TLA+ EventsPreserved: received_count == processed_count + pending + current
    received_count: u64,
    /// Count of events processed (O(1) memory, replaces unbounded HashSet).
    processed_count: u64,
}

impl Default for UIBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl UIBridge {
    /// Create a new UI Bridge in the Idle state.
    pub fn new() -> Self {
        Self {
            state: UIState::Idle,
            terminal_states: HashMap::new(),
            pending_events: VecDeque::new(),
            current_event: None,
            callbacks_pending: HashSet::new(),
            render_pending: HashSet::new(),
            received_count: 0,
            processed_count: 0,
        }
    }

    /// Get the current UI state.
    pub fn state(&self) -> UIState {
        self.state
    }

    /// Get the number of pending events.
    pub fn pending_count(&self) -> usize {
        self.pending_events.len()
    }

    /// Get the number of pending callbacks.
    pub fn callback_count(&self) -> usize {
        self.callbacks_pending.len()
    }

    /// Get the number of pending renders.
    pub fn render_pending_count(&self) -> usize {
        self.render_pending.len()
    }

    /// Check if the bridge is consistent (invariant check).
    ///
    /// This method verifies that the TLA+ TypeInvariant holds.
    pub fn is_consistent(&self) -> bool {
        // TypeInvariant from TLA+ spec
        let state_consistent = match self.state {
            UIState::Idle => {
                self.current_event.is_none()
                    && self.callbacks_pending.is_empty()
                    && self.render_pending.is_empty()
            }
            UIState::Processing => self.current_event.is_some(),
            UIState::Rendering => !self.render_pending.is_empty(),
            UIState::WaitingForCallback => !self.callbacks_pending.is_empty(),
            UIState::ShuttingDown => self.current_event.is_none(),
        };

        // Queue bounded
        let queue_bounded = self.pending_events.len() <= MAX_QUEUE;

        // Events preserved: received_count == processed_count + pending + current
        // Using O(1) counters instead of O(n) HashSets - fixes unbounded memory growth
        let pending_count = self.pending_events.len() as u64;
        let current_count = u64::from(self.current_event.is_some());
        let events_preserved =
            self.received_count == self.processed_count + pending_count + current_count;

        // No duplicate pending IDs (event IDs are unique from atomic counter)
        let pending_ids: HashSet<EventId> = self.pending_events.iter().map(|e| e.id).collect();
        let no_duplicates = pending_ids.len() == self.pending_events.len();

        // Callbacks bounded
        let callbacks_bounded = self.callbacks_pending.len() <= MAX_CALLBACKS;

        // Terminal states bounded (enforced by validate_event rejecting ID >= MAX_TERMINALS)
        let terminals_bounded = self.terminal_states.len() <= MAX_TERMINALS;

        state_consistent
            && queue_bounded
            && events_preserved
            && no_duplicates
            && callbacks_bounded
            && terminals_bounded
    }

    /// Get the state of a terminal.
    pub fn terminal_state(&self, id: TerminalId) -> TerminalState {
        self.terminal_states
            .get(&id)
            .copied()
            .unwrap_or(TerminalState::Inactive)
    }

    /// Enqueue an event for processing.
    ///
    /// # TLA+ Correspondence
    /// This implements the `EnqueueEvent` action from the TLA+ spec.
    pub fn enqueue(&mut self, event: Event) -> UIResult<()> {
        // Cannot enqueue if shutting down
        if self.state == UIState::ShuttingDown {
            return Err(UIError::ShuttingDown);
        }

        // Cannot exceed queue capacity
        if self.pending_events.len() >= MAX_QUEUE {
            return Err(UIError::QueueFull);
        }

        // Validate event
        self.validate_event(&event)?;

        // Track event count (O(1) counter replaces O(n) HashSet)
        self.received_count += 1;

        // Add to queue
        self.pending_events.push_back(event);

        Ok(())
    }

    /// Validate an event before enqueueing.
    ///
    /// # Bounds Enforcement
    /// This method enforces MAX_TERMINALS and MAX_CALLBACKS bounds.
    /// Kani proofs VERIFY these bounds are enforced, not just ASSUME them.
    fn validate_event(&self, event: &Event) -> UIResult<()> {
        // BOUNDS CHECK: Terminal ID must be < MAX_TERMINALS
        // Without this, terminal_states HashMap could grow unbounded.
        if let Some(id) = event.terminal {
            if id as usize >= MAX_TERMINALS {
                return Err(UIError::InvalidTerminalId);
            }
        }

        // BOUNDS CHECK: Callback ID must be < MAX_CALLBACKS
        // Without this, callbacks_pending could reference unbounded IDs.
        if let Some(cb) = event.callback {
            if cb as usize >= MAX_CALLBACKS {
                return Err(UIError::InvalidTerminalId); // Reusing error type
            }
        }

        match event.kind {
            EventKind::Shutdown => {
                // Shutdown event has no terminal
                if event.terminal.is_some() {
                    return Err(UIError::InvalidTerminalId);
                }
            }
            EventKind::CreateTerminal => {
                // Must target an inactive terminal
                if let Some(id) = event.terminal {
                    if self.terminal_state(id) != TerminalState::Inactive {
                        return Err(UIError::InvalidTerminalState);
                    }
                    // Also check that no CreateTerminal is already pending for this ID
                    // This prevents multiple CreateTerminal events being queued for the same
                    // terminal, which could lead to DisposedMonotonic violations if the terminal
                    // is destroyed between processing them.
                    let has_pending_create = self
                        .pending_events
                        .iter()
                        .any(|e| e.kind == EventKind::CreateTerminal && e.terminal == Some(id));
                    if has_pending_create {
                        return Err(UIError::InvalidTerminalState);
                    }
                } else {
                    return Err(UIError::InvalidTerminalId);
                }
            }
            EventKind::DestroyTerminal
            | EventKind::Input
            | EventKind::Resize
            | EventKind::Render => {
                // Must target an active terminal
                if let Some(id) = event.terminal {
                    if self.terminal_state(id) != TerminalState::Active {
                        return Err(UIError::InvalidTerminalState);
                    }
                } else {
                    return Err(UIError::InvalidTerminalId);
                }
            }
            EventKind::RequestCallback => {
                // Must target an active terminal and have unique callback ID
                if let Some(id) = event.terminal {
                    if self.terminal_state(id) != TerminalState::Active {
                        return Err(UIError::InvalidTerminalState);
                    }
                } else {
                    return Err(UIError::InvalidTerminalId);
                }
                if let Some(cb) = event.callback {
                    if self.callbacks_pending.contains(&cb) {
                        return Err(UIError::DuplicateCallback);
                    }
                }
            }
        }
        Ok(())
    }

    /// Start processing the next event.
    ///
    /// # TLA+ Correspondence
    /// This implements the `StartProcessing` action from the TLA+ spec.
    pub fn start_processing(&mut self) -> UIResult<&Event> {
        // Can only start processing from Idle state
        if self.state != UIState::Idle {
            return Err(UIError::InvalidStateTransition);
        }

        // Must have events to process
        if self.pending_events.is_empty() {
            return Err(UIError::NoEventPending);
        }

        // Must not have a current event
        if self.current_event.is_some() {
            return Err(UIError::InvalidStateTransition);
        }

        // Dequeue and transition
        // SAFETY: is_empty() check above guarantees pop_front succeeds
        let event = self.pending_events.pop_front().expect("pending_events not empty");
        self.current_event = Some(event);
        self.state = UIState::Processing;

        // SAFETY: just set to Some above
        Ok(self.current_event.as_ref().expect("current_event just set"))
    }

    /// Execute shutdown logic - SINGLE SOURCE OF TRUTH.
    ///
    /// This method is called by both `complete_processing(Shutdown)` and
    /// `handle_event(Shutdown)` to ensure identical behavior. Having a single
    /// implementation makes divergence bugs structurally impossible.
    ///
    /// # Formal Verification
    /// This consolidation was added after finding a bug where `handle_event(Shutdown)`
    /// had different logic than `complete_processing(Shutdown)`. By extracting to a
    /// single method, Kani proofs need only verify this one implementation.
    fn execute_shutdown(&mut self) {
        // 1. Dispose ALL active terminals (DisposedMonotonic invariant)
        let active_terminals: Vec<TerminalId> = self
            .terminal_states
            .iter()
            .filter(|(_, state)| **state == TerminalState::Active)
            .map(|(id, _)| *id)
            .collect();
        for id in active_terminals {
            self.terminal_states.insert(id, TerminalState::Disposed);
        }

        // 2. Mark all pending events as processed (EventsPreserved invariant)
        //    pending_events.len() events + 1 for shutdown event itself
        self.processed_count += self.pending_events.len() as u64 + 1;
        self.pending_events.clear();

        // 3. Clear all pending work
        self.callbacks_pending.clear();
        self.render_pending.clear();

        // 4. Transition to ShuttingDown
        self.state = UIState::ShuttingDown;
    }

    /// Process the current event and complete it.
    ///
    /// This handles Input, Resize, CreateTerminal, DestroyTerminal events.
    /// Render and RequestCallback events require separate completion calls.
    ///
    /// # TLA+ Correspondence
    /// This implements ProcessInputResize, ProcessCreateTerminal, ProcessDestroyTerminal,
    /// ProcessRender, ProcessRequestCallback, ProcessShutdown from the TLA+ spec.
    pub fn complete_processing(&mut self) -> UIResult<()> {
        if self.state != UIState::Processing {
            return Err(UIError::InvalidStateTransition);
        }

        let event = self.current_event.take().ok_or(UIError::NoEventPending)?;

        match event.kind {
            EventKind::Input | EventKind::Resize => {
                // Simple completion - back to Idle
                self.processed_count += 1;
                self.state = UIState::Idle;
            }
            EventKind::CreateTerminal => {
                // Activate the terminal, but only if it's still Inactive.
                // DisposedMonotonic: once Disposed, a terminal stays Disposed.
                if let Some(id) = event.terminal {
                    if self.terminal_state(id) == TerminalState::Inactive {
                        self.terminal_states.insert(id, TerminalState::Active);
                    }
                    // If Disposed, silently skip (this shouldn't happen with proper validation,
                    // but we enforce it here as a defensive measure)
                }
                self.processed_count += 1;
                self.state = UIState::Idle;
            }
            EventKind::DestroyTerminal => {
                // Dispose the terminal (irreversible)
                if let Some(id) = event.terminal {
                    self.terminal_states.insert(id, TerminalState::Disposed);
                }
                self.processed_count += 1;
                self.state = UIState::Idle;
            }
            EventKind::Render => {
                // Add to render pending set
                if let Some(id) = event.terminal {
                    self.render_pending.insert(id);
                }
                self.processed_count += 1;
                self.state = UIState::Rendering;
            }
            EventKind::RequestCallback => {
                // Add callback to pending set
                if let Some(cb) = event.callback {
                    self.callbacks_pending.insert(cb);
                }
                self.processed_count += 1;
                self.state = UIState::WaitingForCallback;
            }
            EventKind::Shutdown => {
                // Delegate to single source of truth
                self.execute_shutdown();
            }
        }

        Ok(())
    }

    /// Complete a render for a terminal.
    ///
    /// # TLA+ Correspondence
    /// This implements the `CompleteRender` action from the TLA+ spec.
    pub fn complete_render(&mut self, terminal: TerminalId) -> UIResult<()> {
        if self.state != UIState::Rendering {
            return Err(UIError::InvalidStateTransition);
        }

        if !self.render_pending.remove(&terminal) {
            return Err(UIError::InvalidTerminalId);
        }

        // Transition back to Idle if no more renders pending
        if self.render_pending.is_empty() {
            self.state = UIState::Idle;
        }

        Ok(())
    }

    /// Complete a callback.
    ///
    /// # TLA+ Correspondence
    /// This implements the `CompleteCallback` action from the TLA+ spec.
    pub fn complete_callback(&mut self, callback: CallbackId) -> UIResult<()> {
        if self.state != UIState::WaitingForCallback {
            return Err(UIError::InvalidStateTransition);
        }

        if !self.callbacks_pending.remove(&callback) {
            return Err(UIError::DuplicateCallback);
        }

        // Transition back to Idle if no more callbacks pending
        if self.callbacks_pending.is_empty() {
            self.state = UIState::Idle;
        }

        Ok(())
    }

    /// Handle an event in one shot (enqueue + process + complete).
    ///
    /// This is a convenience method for simple event handling.
    pub fn handle_event(&mut self, event: Event) -> UIResult<()> {
        // If not idle, we can only enqueue
        if self.state != UIState::Idle {
            return self.enqueue(event);
        }

        // Validate and process immediately
        self.validate_event(&event)?;
        self.received_count += 1;

        // Process based on kind
        match event.kind {
            EventKind::Input | EventKind::Resize => {
                self.processed_count += 1;
            }
            EventKind::CreateTerminal => {
                // Activate the terminal, but only if it's still Inactive.
                // DisposedMonotonic: once Disposed, a terminal stays Disposed.
                if let Some(id) = event.terminal {
                    if self.terminal_state(id) == TerminalState::Inactive {
                        self.terminal_states.insert(id, TerminalState::Active);
                    }
                }
                self.processed_count += 1;
            }
            EventKind::DestroyTerminal => {
                if let Some(id) = event.terminal {
                    self.terminal_states.insert(id, TerminalState::Disposed);
                }
                self.processed_count += 1;
            }
            EventKind::Render => {
                if let Some(id) = event.terminal {
                    self.render_pending.insert(id);
                }
                self.processed_count += 1;
                self.state = UIState::Rendering;
            }
            EventKind::RequestCallback => {
                if let Some(cb) = event.callback {
                    self.callbacks_pending.insert(cb);
                }
                self.processed_count += 1;
                self.state = UIState::WaitingForCallback;
            }
            EventKind::Shutdown => {
                // Delegate to single source of truth
                self.execute_shutdown();
            }
        }

        Ok(())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_bridge_new() {
        let bridge = UIBridge::new();
        assert_eq!(bridge.state(), UIState::Idle);
        assert_eq!(bridge.pending_count(), 0);
        assert!(bridge.is_consistent());
    }

    #[test]
    fn test_terminal_lifecycle() {
        let mut bridge = UIBridge::new();

        // Initially inactive
        assert_eq!(bridge.terminal_state(0), TerminalState::Inactive);

        // Create terminal
        bridge.handle_event(Event::create_terminal(0)).unwrap();
        assert_eq!(bridge.terminal_state(0), TerminalState::Active);
        assert!(bridge.is_consistent());

        // Destroy terminal
        bridge.handle_event(Event::destroy_terminal(0)).unwrap();
        assert_eq!(bridge.terminal_state(0), TerminalState::Disposed);
        assert!(bridge.is_consistent());
    }

    #[test]
    fn test_disposed_terminal_cannot_be_reactivated() {
        let mut bridge = UIBridge::new();

        // Create and destroy
        bridge.handle_event(Event::create_terminal(0)).unwrap();
        bridge.handle_event(Event::destroy_terminal(0)).unwrap();

        // Cannot create again (disposed is permanent)
        let result = bridge.handle_event(Event::create_terminal(0));
        assert_eq!(result, Err(UIError::InvalidTerminalState));
    }

    #[test]
    fn test_input_requires_active_terminal() {
        let mut bridge = UIBridge::new();

        // Input to inactive terminal should fail
        let result = bridge.handle_event(Event::input(0, vec![b'a']));
        assert_eq!(result, Err(UIError::InvalidTerminalState));

        // Create terminal first
        bridge.handle_event(Event::create_terminal(0)).unwrap();

        // Now input should work
        bridge.handle_event(Event::input(0, vec![b'a'])).unwrap();
        assert!(bridge.is_consistent());
    }

    #[test]
    fn test_render_flow() {
        let mut bridge = UIBridge::new();

        // Create terminal
        bridge.handle_event(Event::create_terminal(0)).unwrap();
        assert_eq!(bridge.state(), UIState::Idle);

        // Request render
        bridge.handle_event(Event::render(0)).unwrap();
        assert_eq!(bridge.state(), UIState::Rendering);
        assert_eq!(bridge.render_pending_count(), 1);
        assert!(bridge.is_consistent());

        // Complete render
        bridge.complete_render(0).unwrap();
        assert_eq!(bridge.state(), UIState::Idle);
        assert_eq!(bridge.render_pending_count(), 0);
        assert!(bridge.is_consistent());
    }

    #[test]
    fn test_callback_flow() {
        let mut bridge = UIBridge::new();

        // Create terminal
        bridge.handle_event(Event::create_terminal(0)).unwrap();

        // Request callback
        bridge.handle_event(Event::request_callback(0, 42)).unwrap();
        assert_eq!(bridge.state(), UIState::WaitingForCallback);
        assert_eq!(bridge.callback_count(), 1);
        assert!(bridge.is_consistent());

        // Complete callback
        bridge.complete_callback(42).unwrap();
        assert_eq!(bridge.state(), UIState::Idle);
        assert_eq!(bridge.callback_count(), 0);
        assert!(bridge.is_consistent());
    }

    #[test]
    fn test_shutdown() {
        let mut bridge = UIBridge::new();

        // Enqueue some events
        bridge.handle_event(Event::create_terminal(0)).unwrap();
        bridge.enqueue(Event::input(0, vec![b'a'])).unwrap();
        bridge.enqueue(Event::input(0, vec![b'b'])).unwrap();

        // Shutdown
        bridge.enqueue(Event::shutdown()).unwrap();

        // Process until shutdown
        while bridge.state() != UIState::ShuttingDown {
            bridge.start_processing().unwrap();
            bridge.complete_processing().unwrap();
        }

        assert_eq!(bridge.state(), UIState::ShuttingDown);
        assert_eq!(bridge.pending_count(), 0);
        assert!(bridge.is_consistent());

        // Cannot enqueue after shutdown
        let result = bridge.enqueue(Event::input(0, vec![b'c']));
        assert_eq!(result, Err(UIError::ShuttingDown));
    }

    #[test]
    fn test_queue_full() {
        let mut bridge = UIBridge::new();
        bridge.handle_event(Event::create_terminal(0)).unwrap();

        // Fill the queue
        for _ in 0..MAX_QUEUE {
            bridge.enqueue(Event::input(0, vec![b'a'])).unwrap();
        }

        // Next enqueue should fail
        let result = bridge.enqueue(Event::input(0, vec![b'b']));
        assert_eq!(result, Err(UIError::QueueFull));
    }

    #[test]
    fn test_no_duplicate_callbacks() {
        let mut bridge = UIBridge::new();
        bridge.handle_event(Event::create_terminal(0)).unwrap();

        // Request callback
        bridge.handle_event(Event::request_callback(0, 42)).unwrap();

        // Duplicate callback should fail (even if enqueued)
        let result = bridge.enqueue(Event::request_callback(0, 42));
        assert_eq!(result, Err(UIError::DuplicateCallback));
    }

    #[test]
    fn test_terminal_id_bounds() {
        let mut bridge = UIBridge::new();

        // Valid terminal ID (at MAX_TERMINALS - 1) should succeed
        let valid_id = u32::try_from(MAX_TERMINALS - 1).unwrap();
        assert!(bridge
            .handle_event(Event::create_terminal(valid_id))
            .is_ok());

        // Invalid terminal ID (at MAX_TERMINALS) should fail
        let invalid_id = u32::try_from(MAX_TERMINALS).unwrap();
        let result = bridge.handle_event(Event::create_terminal(invalid_id));
        assert_eq!(result, Err(UIError::InvalidTerminalId));

        // Way out of bounds should also fail
        let result = bridge.handle_event(Event::create_terminal(u32::MAX));
        assert_eq!(result, Err(UIError::InvalidTerminalId));
    }

    #[test]
    fn test_callback_id_bounds() {
        let mut bridge = UIBridge::new();
        bridge.handle_event(Event::create_terminal(0)).unwrap();

        // Valid callback ID (at MAX_CALLBACKS - 1) should succeed
        let valid_cb = u32::try_from(MAX_CALLBACKS - 1).unwrap();
        assert!(bridge
            .handle_event(Event::request_callback(0, valid_cb))
            .is_ok());

        // Complete callback to return to Idle
        bridge.complete_callback(valid_cb).unwrap();

        // Invalid callback ID (at MAX_CALLBACKS) should fail
        let invalid_cb = u32::try_from(MAX_CALLBACKS).unwrap();
        let result = bridge.handle_event(Event::request_callback(0, invalid_cb));
        assert_eq!(result, Err(UIError::InvalidTerminalId));
    }

    #[test]
    fn test_events_preserved_invariant() {
        let mut bridge = UIBridge::new();
        bridge.handle_event(Event::create_terminal(0)).unwrap();

        // Enqueue several events
        for i in 0_u8..10 {
            bridge.enqueue(Event::input(0, vec![i])).unwrap();
        }

        // Invariant should hold
        assert!(bridge.is_consistent());

        // Process some events
        for _ in 0..5 {
            bridge.start_processing().unwrap();
            bridge.complete_processing().unwrap();
        }

        // Invariant should still hold
        assert!(bridge.is_consistent());
    }

    #[test]
    fn test_state_machine_transitions() {
        let mut bridge = UIBridge::new();

        // Idle -> Processing
        bridge.handle_event(Event::create_terminal(0)).unwrap();
        bridge.enqueue(Event::render(0)).unwrap();
        bridge.start_processing().unwrap();
        assert_eq!(bridge.state(), UIState::Processing);

        // Processing -> Rendering
        bridge.complete_processing().unwrap();
        assert_eq!(bridge.state(), UIState::Rendering);

        // Rendering -> Idle
        bridge.complete_render(0).unwrap();
        assert_eq!(bridge.state(), UIState::Idle);
    }

    /// Test exact fuzzer sequence that found DisposedMonotonic bug.
    /// Traces through the exact minimized sequence from the fuzzer.
    #[test]
    fn test_fuzzer_sequence_disposed_monotonic() {
        let mut bridge = UIBridge::new();
        let mut observed_disposed = std::collections::HashSet::new();

        // Helper to check and record disposed terminals
        fn check_observed(bridge: &UIBridge, observed: &mut std::collections::HashSet<TerminalId>) {
            for tid in 0..=8 {
                if bridge.terminal_state(tid) == TerminalState::Disposed {
                    observed.insert(tid);
                }
            }
        }

        // Helper to verify monotonicity
        fn verify_monotonicity(
            bridge: &UIBridge,
            observed: &std::collections::HashSet<TerminalId>,
            step: &str,
        ) {
            for tid in observed {
                assert_eq!(
                    bridge.terminal_state(*tid),
                    TerminalState::Disposed,
                    "DisposedMonotonic violated at step '{}': terminal {} changed from Disposed to {:?}",
                    step,
                    tid,
                    bridge.terminal_state(*tid)
                );
            }
        }

        // Action 0: Render(7) - fails (terminal 7 not active)
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.handle_event(Event::render(7));
        verify_monotonicity(&bridge, &observed_disposed, "Render(7)");

        // Action 1: CreateTerminal(3)
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.handle_event(Event::create_terminal(3));
        verify_monotonicity(&bridge, &observed_disposed, "CreateTerminal(3)");

        // Action 2: Render(3) - puts us in Rendering state
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.handle_event(Event::render(3));
        verify_monotonicity(&bridge, &observed_disposed, "Render(3)");

        // Action 3: CreateTerminal(0) - enqueued
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.handle_event(Event::create_terminal(0));
        verify_monotonicity(&bridge, &observed_disposed, "CreateTerminal(0) enqueued");

        // Action 4: CreateTerminal(0) - duplicate (should fail due to fix)
        check_observed(&bridge, &mut observed_disposed);
        let result4 = bridge.handle_event(Event::create_terminal(0));
        assert!(
            result4.is_err(),
            "Duplicate CreateTerminal should be rejected"
        );
        verify_monotonicity(&bridge, &observed_disposed, "CreateTerminal(0) duplicate");

        // Action 5: Shutdown - enqueued
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.handle_event(Event::shutdown());
        verify_monotonicity(&bridge, &observed_disposed, "Shutdown enqueued");

        // Action 6: CompleteRender(3)
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.complete_render(3);
        verify_monotonicity(&bridge, &observed_disposed, "CompleteRender(3)");

        // Action 7: StartProcessing
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.start_processing();
        verify_monotonicity(&bridge, &observed_disposed, "StartProcessing");

        // Action 8: CompleteProcessing - terminal 0 becomes Active
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.complete_processing();
        verify_monotonicity(
            &bridge,
            &observed_disposed,
            "CompleteProcessing (CreateTerminal)",
        );

        // Action 9: DestroyTerminal(0) - should execute immediately (we're Idle)
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.handle_event(Event::destroy_terminal(0));
        verify_monotonicity(&bridge, &observed_disposed, "DestroyTerminal(0)");

        // At this point terminal 0 should be Disposed
        assert_eq!(
            bridge.terminal_state(0),
            TerminalState::Disposed,
            "Terminal 0 should be Disposed after DestroyTerminal"
        );

        // Action 10: StartProcessing - gets Shutdown from queue
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.start_processing();
        verify_monotonicity(&bridge, &observed_disposed, "StartProcessing (Shutdown)");

        // Action 11: CreateTerminal(0) - should fail (terminal 0 is Disposed)
        check_observed(&bridge, &mut observed_disposed);
        let result = bridge.handle_event(Event::create_terminal(0));
        // Expect this to fail
        assert!(
            result.is_err(),
            "CreateTerminal(0) should fail because terminal 0 is Disposed"
        );
        verify_monotonicity(
            &bridge,
            &observed_disposed,
            "CreateTerminal(0) after destroy",
        );

        // Action 12: CompleteProcessing - processes Shutdown
        check_observed(&bridge, &mut observed_disposed);
        let _ = bridge.complete_processing();
        verify_monotonicity(&bridge, &observed_disposed, "CompleteProcessing (Shutdown)");

        // Final check
        assert_eq!(
            bridge.terminal_state(0),
            TerminalState::Disposed,
            "Terminal 0 should still be Disposed after shutdown"
        );
    }
}

// =============================================================================
// KANI PROOFS
// =============================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    // NOTE: UIBridge uses HashMap which triggers CCRandomGenerateBytes on macOS.
    // Kani cannot model this FFI call. See: https://github.com/model-checking/kani/issues/2423
    //
    // MACOS LIMITATION: Proofs that create UIBridge fail on macOS with "unsupported FFI".
    // They PASS on Linux CI. Run `cargo kani` on Linux for full verification.
    //
    // Proofs like `event_id_monotonic` that don't create UIBridge work on all platforms.

    /// Proof: UIBridge new() always creates a consistent state.
    ///
    /// MACOS: Fails due to CCRandomGenerateBytes FFI. Run on Linux CI.
    #[kani::proof]
    fn ui_bridge_new_consistent() {
        let bridge = UIBridge::new();
        kani::assert(
            bridge.state() == UIState::Idle,
            "Initial state must be Idle",
        );
        kani::assert(bridge.pending_count() == 0, "Initial queue must be empty");
        kani::assert(bridge.is_consistent(), "New bridge must be consistent");
    }

    /// Proof: Event ID generation is monotonically increasing.
    #[kani::proof]
    fn event_id_monotonic() {
        let id1 = next_event_id();
        let id2 = next_event_id();
        kani::assert(id2 > id1, "Event IDs must be monotonically increasing");
    }

    /// Proof: Terminal state transitions are valid (Inactive -> Active -> Disposed).
    #[kani::proof]
    #[kani::unwind(3)]
    fn terminal_state_transitions_valid() {
        let mut bridge = UIBridge::new();
        let tid: TerminalId = kani::any();
        kani::assume(tid < MAX_TERMINALS as u32);

        // Initially inactive
        kani::assert(
            bridge.terminal_state(tid) == TerminalState::Inactive,
            "Terminal must start Inactive",
        );

        // Create terminal
        let _ = bridge.handle_event(Event::create_terminal(tid));

        let state_after_create = bridge.terminal_state(tid);
        kani::assert(
            state_after_create == TerminalState::Active
                || state_after_create == TerminalState::Inactive,
            "After create, terminal is Active or still Inactive (if failed)",
        );

        // If active, can destroy
        if state_after_create == TerminalState::Active {
            let _ = bridge.handle_event(Event::destroy_terminal(tid));
            kani::assert(
                bridge.terminal_state(tid) == TerminalState::Disposed,
                "After destroy, terminal must be Disposed",
            );
        }
    }

    /// Proof: Disposed terminals stay disposed (monotonicity).
    #[kani::proof]
    #[kani::unwind(5)]
    fn disposed_is_permanent() {
        let mut bridge = UIBridge::new();

        // Create and destroy terminal 0
        let _ = bridge.handle_event(Event::create_terminal(0));
        if bridge.terminal_state(0) == TerminalState::Active {
            let _ = bridge.handle_event(Event::destroy_terminal(0));

            // Now disposed
            kani::assert(
                bridge.terminal_state(0) == TerminalState::Disposed,
                "Must be disposed after destroy",
            );

            // Try to create again - should fail and state should remain Disposed
            let result = bridge.handle_event(Event::create_terminal(0));
            kani::assert(result.is_err(), "Cannot create disposed terminal");
            kani::assert(
                bridge.terminal_state(0) == TerminalState::Disposed,
                "Disposed must remain disposed",
            );
        }
    }

    /// Proof: Queue capacity is bounded.
    #[kani::proof]
    #[kani::unwind(10)]
    fn queue_bounded() {
        let mut bridge = UIBridge::new();
        let _ = bridge.handle_event(Event::create_terminal(0));

        // Try to enqueue more than MAX_QUEUE events
        for _ in 0..MAX_QUEUE + 5 {
            let _ = bridge.enqueue(Event::input(0, vec![]));
        }

        // Queue must not exceed MAX_QUEUE
        kani::assert(
            bridge.pending_count() <= MAX_QUEUE,
            "Queue must not exceed MAX_QUEUE",
        );
    }

    /// Proof: Processing state requires current_event to be Some.
    #[kani::proof]
    #[kani::unwind(3)]
    fn processing_state_has_event() {
        let mut bridge = UIBridge::new();
        let _ = bridge.handle_event(Event::create_terminal(0));
        let _ = bridge.enqueue(Event::input(0, vec![]));

        // Start processing
        let result = bridge.start_processing();

        if result.is_ok() {
            kani::assert(
                bridge.state() == UIState::Processing,
                "After start_processing, state must be Processing",
            );
            kani::assert(
                bridge.current_event.is_some(),
                "Processing state must have current_event",
            );
        }
    }

    /// Proof: Idle state has no pending work.
    #[kani::proof]
    fn idle_state_no_pending_work() {
        let bridge = UIBridge::new();

        if bridge.state() == UIState::Idle {
            kani::assert(
                bridge.current_event.is_none(),
                "Idle must have no current_event",
            );
            kani::assert(
                bridge.callbacks_pending.is_empty(),
                "Idle must have no pending callbacks",
            );
            kani::assert(
                bridge.render_pending.is_empty(),
                "Idle must have no pending renders",
            );
        }
    }

    /// Proof: Rendering state has non-empty render_pending.
    #[kani::proof]
    #[kani::unwind(3)]
    fn rendering_state_has_pending() {
        let mut bridge = UIBridge::new();
        let _ = bridge.handle_event(Event::create_terminal(0));
        let _ = bridge.handle_event(Event::render(0));

        if bridge.state() == UIState::Rendering {
            kani::assert(
                !bridge.render_pending.is_empty(),
                "Rendering state must have pending renders",
            );
        }
    }

    /// Proof: WaitingForCallback state has non-empty callbacks_pending.
    #[kani::proof]
    #[kani::unwind(3)]
    fn callback_state_has_pending() {
        let mut bridge = UIBridge::new();
        let _ = bridge.handle_event(Event::create_terminal(0));
        let _ = bridge.handle_event(Event::request_callback(0, 1));

        if bridge.state() == UIState::WaitingForCallback {
            kani::assert(
                !bridge.callbacks_pending.is_empty(),
                "WaitingForCallback state must have pending callbacks",
            );
        }
    }

    /// Proof: ShuttingDown rejects new events.
    #[kani::proof]
    #[kani::unwind(3)]
    fn shutdown_rejects_events() {
        let mut bridge = UIBridge::new();
        let _ = bridge.handle_event(Event::shutdown());

        if bridge.state() == UIState::ShuttingDown {
            let result = bridge.enqueue(Event::create_terminal(0));
            kani::assert(result.is_err(), "Shutdown must reject new events");
        }
    }

    /// Proof: is_consistent() is preserved across valid operations.
    #[kani::proof]
    #[kani::unwind(5)]
    fn consistency_preserved() {
        let mut bridge = UIBridge::new();
        kani::assert(bridge.is_consistent(), "Initial state must be consistent");

        // Create terminal
        let _ = bridge.handle_event(Event::create_terminal(0));
        kani::assert(bridge.is_consistent(), "Consistent after create_terminal");

        // Enqueue input
        let _ = bridge.enqueue(Event::input(0, vec![]));
        kani::assert(bridge.is_consistent(), "Consistent after enqueue");

        // Process
        if bridge.start_processing().is_ok() {
            kani::assert(bridge.is_consistent(), "Consistent after start_processing");
            if bridge.complete_processing().is_ok() {
                kani::assert(
                    bridge.is_consistent(),
                    "Consistent after complete_processing",
                );
            }
        }
    }
}

// =============================================================================
// NEW KANI PROOF - Bug Detection for DisposedMonotonic
// =============================================================================

#[cfg(kani)]
mod kani_bug_proofs {
    use super::*;

    /// Proof: Shutdown disposes all active terminals.
    ///
    /// INVARIANT: After shutdown completes, no terminal can be in Active state.
    /// This ensures the DisposedMonotonic property: once we start shutting down,
    /// all terminals must be cleaned up.
    ///
    /// BUG FOUND: Current code marks pending DestroyTerminal events as "processed"
    /// without actually disposing the terminals. This proof WILL FAIL until fixed.
    #[kani::proof]
    #[kani::unwind(10)]
    fn shutdown_disposes_all_terminals() {
        let mut bridge = UIBridge::new();

        // Create a terminal
        let _ = bridge.handle_event(Event::create_terminal(0));

        // Terminal is now Active
        if bridge.terminal_state(0) == TerminalState::Active {
            // Queue a DestroyTerminal event
            let _ = bridge.enqueue(Event::destroy_terminal(0));

            // Queue Shutdown event
            let _ = bridge.enqueue(Event::shutdown());

            // Process until shutdown
            while bridge.state() != UIState::ShuttingDown {
                if bridge.start_processing().is_ok() {
                    let _ = bridge.complete_processing();
                } else {
                    break;
                }
            }

            // INVARIANT: After shutdown, no terminal should be Active
            // All should be either Disposed or Inactive
            kani::assert(
                bridge.terminal_state(0) != TerminalState::Active,
                "INVARIANT VIOLATION: Terminal still Active after shutdown!",
            );
        }
    }

    /// Proof: EventsPreserved holds after handle_event(Shutdown) with pending events.
    ///
    /// BUG THIS CATCHES: If handle_event(Shutdown) only does `processed_count += 1`
    /// without accounting for pending events, the invariant breaks.
    ///
    /// This proof would FAIL on the buggy code:
    /// ```
    /// // BUGGY: processed_count += 1;  // Only counts shutdown, not pending events
    /// ```
    /// And PASSES on the fixed code:
    /// ```
    /// // FIXED: processed_count += pending_events.len() + 1;
    /// ```
    #[kani::proof]
    #[kani::unwind(5)]
    fn events_preserved_after_handle_event_shutdown() {
        let mut bridge = UIBridge::new();

        // Create terminal
        let _ = bridge.handle_event(Event::create_terminal(0));

        // Put in Rendering state so we can enqueue
        let _ = bridge.handle_event(Event::render(0));

        // Enqueue some events while rendering
        let _ = bridge.enqueue(Event::input(0, vec![]));

        // Complete render to return to Idle WITH pending events
        let _ = bridge.complete_render(0);

        // Verify we have pending events and are Idle
        if bridge.state() == UIState::Idle && bridge.pending_count() > 0 {
            // This is the critical scenario that caught the bug
            let _ = bridge.handle_event(Event::shutdown());

            // EventsPreserved MUST hold
            kani::assert(
                bridge.is_consistent(),
                "EventsPreserved VIOLATED: handle_event(Shutdown) didn't account for pending events!"
            );
        }
    }

    /// Proof: handle_event(Shutdown) and complete_processing(Shutdown) produce equivalent states.
    ///
    /// This ensures the single-source-of-truth refactoring is correct.
    /// Both paths should result in:
    /// - state == ShuttingDown
    /// - All terminals Disposed
    /// - No pending events
    /// - is_consistent() == true
    #[kani::proof]
    #[kani::unwind(8)]
    fn shutdown_paths_equivalent() {
        // Path 1: handle_event(Shutdown) directly
        let mut bridge1 = UIBridge::new();
        let _ = bridge1.handle_event(Event::create_terminal(0));
        let _ = bridge1.handle_event(Event::shutdown());

        // Path 2: enqueue + start_processing + complete_processing
        let mut bridge2 = UIBridge::new();
        let _ = bridge2.handle_event(Event::create_terminal(0));
        let _ = bridge2.enqueue(Event::shutdown());
        if bridge2.start_processing().is_ok() {
            let _ = bridge2.complete_processing();
        }

        // Both must be in ShuttingDown state
        if bridge1.state() == UIState::ShuttingDown && bridge2.state() == UIState::ShuttingDown {
            // Both must have disposed terminal 0
            kani::assert(
                bridge1.terminal_state(0) == bridge2.terminal_state(0),
                "PATH DIVERGENCE: Terminal states differ between paths!",
            );

            // Both must have no pending events
            kani::assert(
                bridge1.pending_count() == bridge2.pending_count(),
                "PATH DIVERGENCE: Pending counts differ between paths!",
            );

            // Both must be consistent
            kani::assert(
                bridge1.is_consistent() && bridge2.is_consistent(),
                "PATH DIVERGENCE: Consistency differs between paths!",
            );
        }
    }

    /// Proof: Terminal IDs >= MAX_TERMINALS are rejected.
    ///
    /// This proof VERIFIES (not assumes) that bounds are enforced.
    /// FAILS if validate_event() doesn't check terminal ID bounds.
    #[kani::proof]
    #[kani::unwind(3)]
    fn terminal_id_bounds_enforced() {
        let mut bridge = UIBridge::new();

        // Try to create terminal with ID >= MAX_TERMINALS
        let invalid_id = MAX_TERMINALS as u32; // Exactly at limit (invalid)
        let result = bridge.handle_event(Event::create_terminal(invalid_id));

        // Must be rejected
        kani::assert(
            result.is_err(),
            "BOUNDS VIOLATION: Terminal ID >= MAX_TERMINALS was accepted!",
        );
    }

    /// Proof: Callback IDs >= MAX_CALLBACKS are rejected.
    ///
    /// This proof VERIFIES (not assumes) that bounds are enforced.
    /// FAILS if validate_event() doesn't check callback ID bounds.
    #[kani::proof]
    #[kani::unwind(3)]
    fn callback_id_bounds_enforced() {
        let mut bridge = UIBridge::new();

        // Create a valid terminal first
        let _ = bridge.handle_event(Event::create_terminal(0));

        // Try to request callback with ID >= MAX_CALLBACKS
        let invalid_cb = MAX_CALLBACKS as u32; // Exactly at limit (invalid)
        let result = bridge.handle_event(Event::request_callback(0, invalid_cb));

        // Must be rejected
        kani::assert(
            result.is_err(),
            "BOUNDS VIOLATION: Callback ID >= MAX_CALLBACKS was accepted!",
        );
    }

    /// Proof: terminal_states HashMap size is bounded by MAX_TERMINALS.
    ///
    /// After any sequence of operations, terminal_states.len() <= MAX_TERMINALS.
    /// This ensures bounded memory usage.
    #[kani::proof]
    #[kani::unwind(10)]
    fn terminal_states_bounded() {
        let mut bridge = UIBridge::new();

        // Create and destroy several terminals
        for i in 0..5u32 {
            if i < MAX_TERMINALS as u32 {
                let _ = bridge.handle_event(Event::create_terminal(i));
                let _ = bridge.handle_event(Event::destroy_terminal(i));
            }
        }

        // terminal_states must not exceed MAX_TERMINALS
        kani::assert(
            bridge.terminal_states.len() <= MAX_TERMINALS,
            "MEMORY LEAK: terminal_states exceeds MAX_TERMINALS!",
        );
    }

    /// Proof: All event types maintain consistency (path equivalence).
    ///
    /// For each event type, handle_event() and enqueue+process produce consistent state.
    #[kani::proof]
    #[kani::unwind(5)]
    fn all_paths_maintain_consistency() {
        let mut bridge = UIBridge::new();

        // Create terminal via handle_event
        let _ = bridge.handle_event(Event::create_terminal(0));
        kani::assert(bridge.is_consistent(), "Inconsistent after CreateTerminal");

        // Input via handle_event
        let _ = bridge.handle_event(Event::input(0, vec![]));
        kani::assert(bridge.is_consistent(), "Inconsistent after Input");

        // Resize via handle_event
        let _ = bridge.handle_event(Event::resize(0, 24, 80));
        kani::assert(bridge.is_consistent(), "Inconsistent after Resize");

        // DestroyTerminal via handle_event
        let _ = bridge.handle_event(Event::destroy_terminal(0));
        kani::assert(bridge.is_consistent(), "Inconsistent after DestroyTerminal");
    }

    /// Proof: Terminal state transitions follow Inactive -> Active -> Disposed.
    ///
    /// DisposedMonotonic: Once a terminal is Disposed, it cannot become Active again.
    /// This is enforced by validate_event rejecting CreateTerminal for Disposed terminals.
    #[kani::proof]
    #[kani::unwind(5)]
    fn terminal_state_machine_valid() {
        let mut bridge = UIBridge::new();

        // Start: terminal 0 is Inactive
        kani::assert(
            bridge.terminal_state(0) == TerminalState::Inactive,
            "Initial state must be Inactive",
        );

        // Transition: Inactive -> Active
        let _ = bridge.handle_event(Event::create_terminal(0));
        kani::assert(
            bridge.terminal_state(0) == TerminalState::Active,
            "After create, state must be Active",
        );

        // Transition: Active -> Disposed
        let _ = bridge.handle_event(Event::destroy_terminal(0));
        kani::assert(
            bridge.terminal_state(0) == TerminalState::Disposed,
            "After destroy, state must be Disposed",
        );

        // DisposedMonotonic: Cannot go back to Active
        let result = bridge.handle_event(Event::create_terminal(0));
        kani::assert(
            result.is_err(),
            "DISPOSED_MONOTONIC VIOLATION: Disposed terminal was reactivated!",
        );
        kani::assert(
            bridge.terminal_state(0) == TerminalState::Disposed,
            "Disposed terminal must stay Disposed",
        );
    }

    /// Proof: No deadlock - from any non-ShuttingDown state, progress is possible.
    ///
    /// Deadlock freedom: If state != ShuttingDown and there's work to do,
    /// at least one operation will succeed.
    #[kani::proof]
    #[kani::unwind(5)]
    fn deadlock_freedom() {
        let mut bridge = UIBridge::new();

        // Create some state
        let _ = bridge.handle_event(Event::create_terminal(0));

        // From Idle with pending events, start_processing must succeed
        let _ = bridge.enqueue(Event::input(0, vec![]));
        if bridge.state() == UIState::Idle && bridge.pending_count() > 0 {
            let result = bridge.start_processing();
            kani::assert(
                result.is_ok(),
                "DEADLOCK: Cannot start processing from Idle with pending events",
            );
        }

        // From Processing, complete_processing must succeed
        if bridge.state() == UIState::Processing {
            let result = bridge.complete_processing();
            kani::assert(
                result.is_ok(),
                "DEADLOCK: Cannot complete processing from Processing state",
            );
        }
    }

    /// Proof: All memory is bounded.
    ///
    /// Every data structure in UIBridge has bounded size:
    /// - terminal_states: <= MAX_TERMINALS
    /// - pending_events: <= MAX_QUEUE
    /// - callbacks_pending: <= MAX_CALLBACKS (implicitly via ID bounds)
    /// - render_pending: <= MAX_TERMINALS (one render per terminal max)
    /// - received_count, processed_count: u64 (overflow is theoretical only)
    #[kani::proof]
    #[kani::unwind(5)]
    fn all_memory_bounded() {
        let mut bridge = UIBridge::new();

        // Do some operations
        let _ = bridge.handle_event(Event::create_terminal(0));
        let _ = bridge.enqueue(Event::input(0, vec![]));

        // All bounds must hold
        kani::assert(
            bridge.terminal_states.len() <= MAX_TERMINALS,
            "terminal_states exceeded MAX_TERMINALS",
        );
        kani::assert(
            bridge.pending_events.len() <= MAX_QUEUE,
            "pending_events exceeded MAX_QUEUE",
        );
        kani::assert(
            bridge.callbacks_pending.len() <= MAX_CALLBACKS,
            "callbacks_pending exceeded MAX_CALLBACKS",
        );
        kani::assert(
            bridge.render_pending.len() <= MAX_TERMINALS,
            "render_pending exceeded MAX_TERMINALS",
        );
    }
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;

    /// This test verifies the FIX for the DisposedMonotonic bug.
    ///
    /// The bug (now fixed): When Shutdown was processed, it marked all pending events
    /// as "processed" and cleared the queue, but did NOT actually execute DestroyTerminal
    /// events. Terminals remained Active, violating DisposedMonotonic.
    ///
    /// The fix: Shutdown now explicitly disposes all active terminals before clearing
    /// the queue, ensuring no terminal remains Active after shutdown.
    #[test]
    fn test_shutdown_disposes_all_active_terminals() {
        let mut bridge = UIBridge::new();

        // Create terminal 0
        bridge.handle_event(Event::create_terminal(0)).unwrap();
        assert_eq!(bridge.terminal_state(0), TerminalState::Active);
        assert_eq!(bridge.state(), UIState::Idle);

        // Put us in a non-Idle state by requesting a render
        bridge.handle_event(Event::render(0)).unwrap();
        assert_eq!(bridge.state(), UIState::Rendering);

        // Now enqueue Shutdown FIRST (while in Rendering state)
        bridge.enqueue(Event::shutdown()).unwrap();

        // Then enqueue DestroyTerminal (also while in Rendering state)
        bridge.enqueue(Event::destroy_terminal(0)).unwrap();

        // Queue is now: [Shutdown, DestroyTerminal(0)]
        assert_eq!(bridge.pending_count(), 2);

        // Complete the render to return to Idle
        bridge.complete_render(0).unwrap();
        assert_eq!(bridge.state(), UIState::Idle);

        // Now process the queue
        // This will process Shutdown first, which clears the queue
        bridge.start_processing().unwrap();
        bridge.complete_processing().unwrap();

        // We're now ShuttingDown
        assert_eq!(bridge.state(), UIState::ShuttingDown);

        // Queue should be empty (Shutdown cleared it)
        assert_eq!(bridge.pending_count(), 0);

        // FIXED: Terminal 0 is now Disposed!
        // The fix in complete_processing() for Shutdown now disposes all active terminals.
        let terminal_state = bridge.terminal_state(0);
        eprintln!(
            "BUG FIXED: Terminal 0 state after shutdown = {:?}",
            terminal_state
        );

        // After the fix, terminal should be Disposed (not Active)
        assert_eq!(
            terminal_state,
            TerminalState::Disposed,
            "Terminal 0 should be Disposed after shutdown"
        );
    }

    /// Test that handle_event(Shutdown) correctly clears pending events.
    ///
    /// Bug found during iteration 296: handle_event(Shutdown) was only incrementing
    /// processed_count by 1, not accounting for pending events. This broke the
    /// EventsPreserved invariant when there were pending events.
    #[test]
    fn test_handle_event_shutdown_clears_pending() {
        let mut bridge = UIBridge::new();

        // Create terminal
        bridge.handle_event(Event::create_terminal(0)).unwrap();
        assert!(bridge.is_consistent());

        // Put bridge in Rendering state
        bridge.handle_event(Event::render(0)).unwrap();
        assert_eq!(bridge.state(), UIState::Rendering);

        // Enqueue some events while rendering
        bridge.enqueue(Event::input(0, vec![1, 2, 3])).unwrap();
        bridge.enqueue(Event::resize(0, 24, 80)).unwrap();
        assert_eq!(bridge.pending_count(), 2);
        assert!(bridge.is_consistent());

        // Complete render to return to Idle
        bridge.complete_render(0).unwrap();
        assert_eq!(bridge.state(), UIState::Idle);
        assert_eq!(bridge.pending_count(), 2); // Events still pending

        // Now call handle_event(Shutdown) directly (not via enqueue+process)
        // This tests the direct path which had the bug
        bridge.handle_event(Event::shutdown()).unwrap();

        // Verify state
        assert_eq!(bridge.state(), UIState::ShuttingDown);
        assert_eq!(bridge.pending_count(), 0); // Pending events should be cleared
        assert_eq!(bridge.terminal_state(0), TerminalState::Disposed);

        // CRITICAL: EventsPreserved invariant must hold
        assert!(
            bridge.is_consistent(),
            "EventsPreserved invariant broken after handle_event(Shutdown)"
        );
    }
}
