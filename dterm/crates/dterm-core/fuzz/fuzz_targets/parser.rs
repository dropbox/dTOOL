//! Parser fuzz target.
//!
//! This fuzzer tests the VT100/ANSI parser with arbitrary byte sequences.
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run parser -- -max_total_time=60
//! ```
//!
//! ## Properties Tested
//!
//! - Parser never panics on any input
//! - State is always valid (0-13)
//! - Parameters never exceed MAX_PARAMS (16)
//! - Intermediates never exceed MAX_INTERMEDIATES (4)
//!
//! ## Correspondence to TLA+
//!
//! This fuzzer validates the Safety and TypeInvariant properties
//! from tla/Parser.tla through exhaustive random testing.

#![no_main]

use libfuzzer_sys::fuzz_target;
use dterm_core::parser::{Parser, ActionSink, State, MAX_PARAMS, MAX_INTERMEDIATES};

/// Counting sink that tracks action counts for verification.
struct CountingSink {
    prints: usize,
    executes: usize,
    csi_dispatches: usize,
    esc_dispatches: usize,
    osc_dispatches: usize,
    dcs_hooks: usize,
    dcs_puts: usize,
    dcs_unhooks: usize,
    max_params_seen: usize,
    max_intermediates_seen: usize,
}

impl CountingSink {
    fn new() -> Self {
        Self {
            prints: 0,
            executes: 0,
            csi_dispatches: 0,
            esc_dispatches: 0,
            osc_dispatches: 0,
            dcs_hooks: 0,
            dcs_puts: 0,
            dcs_unhooks: 0,
            max_params_seen: 0,
            max_intermediates_seen: 0,
        }
    }
}

impl ActionSink for CountingSink {
    fn print(&mut self, _c: char) {
        self.prints += 1;
    }

    fn execute(&mut self, _byte: u8) {
        self.executes += 1;
    }

    fn csi_dispatch(&mut self, params: &[u16], intermediates: &[u8], _final_byte: u8) {
        self.csi_dispatches += 1;
        self.max_params_seen = self.max_params_seen.max(params.len());
        self.max_intermediates_seen = self.max_intermediates_seen.max(intermediates.len());
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _final_byte: u8) {
        self.esc_dispatches += 1;
        self.max_intermediates_seen = self.max_intermediates_seen.max(intermediates.len());
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]]) {
        self.osc_dispatches += 1;
    }

    fn dcs_hook(&mut self, params: &[u16], intermediates: &[u8], _final_byte: u8) {
        self.dcs_hooks += 1;
        self.max_params_seen = self.max_params_seen.max(params.len());
        self.max_intermediates_seen = self.max_intermediates_seen.max(intermediates.len());
    }

    fn dcs_put(&mut self, _byte: u8) {
        self.dcs_puts += 1;
    }

    fn dcs_unhook(&mut self) {
        self.dcs_unhooks += 1;
    }

    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _byte: u8) {}
    fn apc_end(&mut self) {}
}

fuzz_target!(|data: &[u8]| {
    let mut parser = Parser::new();
    let mut sink = CountingSink::new();

    // Process all bytes - must not panic
    parser.advance(data, &mut sink);

    // Verify state is valid (corresponds to TLA+ StateAlwaysValid)
    assert!(
        (parser.state() as u8) < 14,
        "Invalid state after processing {} bytes: {:?}",
        data.len(),
        parser.state()
    );

    // Verify bounds (corresponds to TLA+ TypeInvariant)
    assert!(
        sink.max_params_seen <= MAX_PARAMS,
        "Too many params: {} > {}",
        sink.max_params_seen,
        MAX_PARAMS
    );
    assert!(
        sink.max_intermediates_seen <= MAX_INTERMEDIATES,
        "Too many intermediates: {} > {}",
        sink.max_intermediates_seen,
        MAX_INTERMEDIATES
    );

    // Test reset functionality
    parser.reset();
    assert_eq!(
        parser.state(),
        State::Ground,
        "Reset didn't return to ground state"
    );
});
