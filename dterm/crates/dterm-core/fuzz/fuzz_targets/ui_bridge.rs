//! UI Bridge fuzz target.
//!
//! This fuzzer tests the UI Bridge event handling with arbitrary event sequences.
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run ui_bridge -- -max_total_time=3600
//! ```
//!
//! ## Properties Tested
//!
//! - UI Bridge never panics on any valid event sequence
//! - State machine invariants hold after every operation
//! - Event queue bounds are respected
//! - Terminal lifecycle transitions are valid
//! - Consistency invariant is preserved
//!
//! ## Correspondence to TLA+
//!
//! This fuzzer validates the Safety invariants from tla/UIStateMachine.tla:
//! - TypeInvariant: State is always valid
//! - EventsPreserved: No events are lost
//! - NoDuplicateEventIds: Event IDs are unique
//! - DisposedMonotonic: Disposed terminals stay disposed

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use dterm_core::ui::{
    CallbackId, Event, TerminalId, TerminalState, UIBridge, UIError,
    UIState, MAX_CALLBACKS, MAX_QUEUE,
};

/// Maximum terminal ID to use in fuzzing (keep small for better coverage).
const FUZZ_MAX_TERMINAL_ID: u32 = 8;

/// Maximum callback ID to use in fuzzing.
const FUZZ_MAX_CALLBACK_ID: u32 = 16;

/// Action that can be performed on the UI Bridge.
#[derive(Debug, Clone)]
enum FuzzAction {
    /// Create a terminal with the given ID.
    CreateTerminal(TerminalId),
    /// Destroy a terminal.
    DestroyTerminal(TerminalId),
    /// Send input to a terminal.
    Input(TerminalId, Vec<u8>),
    /// Resize a terminal.
    Resize(TerminalId, u16, u16),
    /// Request render for a terminal.
    Render(TerminalId),
    /// Request a callback.
    RequestCallback(TerminalId, CallbackId),
    /// Send shutdown event.
    Shutdown,
    /// Start processing an event (manual mode).
    StartProcessing,
    /// Complete processing current event.
    CompleteProcessing,
    /// Complete render for a terminal.
    CompleteRender(TerminalId),
    /// Complete a callback.
    CompleteCallback(CallbackId),
}

impl<'a> Arbitrary<'a> for FuzzAction {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let tag = u.int_in_range(0..=10)?;
        Ok(match tag {
            0 => FuzzAction::CreateTerminal(u.int_in_range(0..=FUZZ_MAX_TERMINAL_ID)?),
            1 => FuzzAction::DestroyTerminal(u.int_in_range(0..=FUZZ_MAX_TERMINAL_ID)?),
            2 => {
                let tid = u.int_in_range(0..=FUZZ_MAX_TERMINAL_ID)?;
                let len = u.int_in_range(0..=64)?;
                let data: Vec<u8> = (0..len).map(|_| u.arbitrary()).collect::<Result<_, _>>()?;
                FuzzAction::Input(tid, data)
            }
            3 => {
                let tid = u.int_in_range(0..=FUZZ_MAX_TERMINAL_ID)?;
                let rows = u.int_in_range(1..=500)?;
                let cols = u.int_in_range(1..=500)?;
                FuzzAction::Resize(tid, rows, cols)
            }
            4 => FuzzAction::Render(u.int_in_range(0..=FUZZ_MAX_TERMINAL_ID)?),
            5 => {
                let tid = u.int_in_range(0..=FUZZ_MAX_TERMINAL_ID)?;
                let cb = u.int_in_range(0..=FUZZ_MAX_CALLBACK_ID)?;
                FuzzAction::RequestCallback(tid, cb)
            }
            6 => FuzzAction::Shutdown,
            7 => FuzzAction::StartProcessing,
            8 => FuzzAction::CompleteProcessing,
            9 => FuzzAction::CompleteRender(u.int_in_range(0..=FUZZ_MAX_TERMINAL_ID)?),
            10 => FuzzAction::CompleteCallback(u.int_in_range(0..=FUZZ_MAX_CALLBACK_ID)?),
            _ => unreachable!(),
        })
    }
}

/// Execute an action on the UI Bridge, allowing expected errors.
fn execute_action(bridge: &mut UIBridge, action: &FuzzAction) -> Result<(), UIError> {
    match action {
        FuzzAction::CreateTerminal(tid) => bridge.handle_event(Event::create_terminal(*tid)),
        FuzzAction::DestroyTerminal(tid) => bridge.handle_event(Event::destroy_terminal(*tid)),
        FuzzAction::Input(tid, data) => bridge.handle_event(Event::input(*tid, data.clone())),
        FuzzAction::Resize(tid, rows, cols) => {
            bridge.handle_event(Event::resize(*tid, *rows, *cols))
        }
        FuzzAction::Render(tid) => bridge.handle_event(Event::render(*tid)),
        FuzzAction::RequestCallback(tid, cb) => {
            bridge.handle_event(Event::request_callback(*tid, *cb))
        }
        FuzzAction::Shutdown => bridge.handle_event(Event::shutdown()),
        FuzzAction::StartProcessing => bridge.start_processing().map(|_| ()),
        FuzzAction::CompleteProcessing => bridge.complete_processing(),
        FuzzAction::CompleteRender(tid) => bridge.complete_render(*tid),
        FuzzAction::CompleteCallback(cb) => bridge.complete_callback(*cb),
    }
}

fuzz_target!(|data: &[u8]| {
    // Parse actions from fuzz data
    let mut u = Unstructured::new(data);
    // Limit action count to avoid memory exhaustion in received_events/processed_events sets
    let action_count: usize = u.int_in_range(0..=100).unwrap_or(0);

    let actions: Vec<FuzzAction> = (0..action_count)
        .filter_map(|_| FuzzAction::arbitrary(&mut u).ok())
        .collect();

    // Create bridge and track state for invariant checking
    let mut bridge = UIBridge::new();

    // Initial state must be consistent
    assert!(
        bridge.is_consistent(),
        "New UIBridge must be consistent"
    );
    assert_eq!(
        bridge.state(),
        UIState::Idle,
        "New UIBridge must be Idle"
    );

    // Track which terminals have been observed as Disposed (for monotonicity check)
    // We track Disposed state by querying the bridge, not by tracking actions,
    // to ensure we're checking the actual state machine behavior.
    let mut observed_disposed: std::collections::HashSet<TerminalId> =
        std::collections::HashSet::new();

    // Execute all actions
    for action in &actions {
        // Before executing, record any terminals that are currently Disposed
        // (to check monotonicity - once Disposed, always Disposed)
        for tid in 0..=FUZZ_MAX_TERMINAL_ID {
            if bridge.terminal_state(tid) == TerminalState::Disposed {
                observed_disposed.insert(tid);
            }
        }

        // Execute action (errors are expected and valid)
        let _result = execute_action(&mut bridge, action);

        // INVARIANT 1: State machine is always in a valid state
        let state = bridge.state();
        assert!(
            matches!(
                state,
                UIState::Idle
                    | UIState::Processing
                    | UIState::Rendering
                    | UIState::WaitingForCallback
                    | UIState::ShuttingDown
            ),
            "Invalid state: {:?}",
            state
        );

        // INVARIANT 2: Queue bounds are respected
        assert!(
            bridge.pending_count() <= MAX_QUEUE,
            "Queue overflow: {} > {}",
            bridge.pending_count(),
            MAX_QUEUE
        );

        // INVARIANT 3: Callback bounds are respected
        assert!(
            bridge.callback_count() <= MAX_CALLBACKS,
            "Callback overflow: {} > {}",
            bridge.callback_count(),
            MAX_CALLBACKS
        );

        // INVARIANT 4: Consistency invariant (from TLA+ TypeInvariant)
        // This checks: state-specific conditions + events preserved + no duplicates
        assert!(
            bridge.is_consistent(),
            "Consistency violated after action {:?}",
            action
        );

        // INVARIANT 5: Terminal state transitions are monotonic for disposed
        // Any terminal that was previously Disposed must still be Disposed
        // Note: This invariant is critical for correctness and is part of the TLA+ spec
        // (DisposedMonotonic property).
        for tid in &observed_disposed {
            let term_state = bridge.terminal_state(*tid);
            assert!(
                term_state == TerminalState::Disposed,
                "DisposedMonotonic violated: terminal {} changed from Disposed to {:?}",
                tid,
                term_state
            );
        }

        // INVARIANT 6: State-specific conditions (from TLA+ spec)
        match bridge.state() {
            UIState::Idle => {
                // Idle has no pending async work
                // (callbacks and renders are only pending when in Rendering/WaitingForCallback)
            }
            UIState::Processing => {
                // Processing means we have a current event
            }
            UIState::Rendering => {
                // Rendering state must have pending renders
                assert!(
                    bridge.render_pending_count() > 0,
                    "Rendering state with no pending renders"
                );
            }
            UIState::WaitingForCallback => {
                // WaitingForCallback state must have pending callbacks
                assert!(
                    bridge.callback_count() > 0,
                    "WaitingForCallback state with no pending callbacks"
                );
            }
            UIState::ShuttingDown => {
                // ShuttingDown: no new events accepted
                // (checked by enqueue returning ShuttingDown error)
            }
        }
    }

    // Final consistency check
    assert!(
        bridge.is_consistent(),
        "Final state not consistent after {} actions",
        actions.len()
    );
});
