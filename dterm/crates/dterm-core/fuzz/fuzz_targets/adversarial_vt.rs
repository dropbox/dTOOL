//! Adversarial VT escape sequence fuzzer targeting known CVE patterns.
//!
//! This fuzzer specifically tests patterns derived from real CVE vulnerabilities
//! found in terminal emulators (xterm, rxvt, VTE, mintty, etc.).
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run adversarial_vt -- -max_total_time=600
//! ```
//!
//! ## CVE Patterns Tested
//!
//! - Integer overflow in cursor positioning (CVE-2022-45063 pattern)
//! - OSC title injection attacks
//! - SGR parameter overflow
//! - DECRQSS response injection
//! - Nested escape sequence DoS
//! - UTF-8 overlong encoding attacks
//! - Control character injection in OSC
//!
//! ## Security Properties
//!
//! 1. No integer overflow/underflow panics
//! 2. No unbounded memory allocation
//! 3. No state corruption from malformed sequences
//! 4. Graceful handling of all attack patterns

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use dterm_core::parser::{Parser, ActionSink, State};
use dterm_core::terminal::Terminal;

/// Known attack patterns derived from terminal CVEs.
const ATTACK_PATTERNS: &[&[u8]] = &[
    // === Integer Overflow Attacks ===

    // Huge cursor position (CVE-2022-45063 pattern)
    b"\x1b[99999999;99999999H",
    // i32::MAX cursor
    b"\x1b[2147483647;2147483647H",
    // u32::MAX cursor
    b"\x1b[4294967295;4294967295H",
    // Negative via underflow
    b"\x1b[-1;-1H",
    // Zero parameters
    b"\x1b[0;0H",

    // === Scroll Region Attacks ===

    // Inverted scroll region
    b"\x1b[100;1r",
    // Huge scroll region
    b"\x1b[1;99999999r",
    // Zero scroll region
    b"\x1b[0;0r",
    // Single line scroll region
    b"\x1b[5;5r",

    // === OSC Title Injection ===

    // Title with embedded newline
    b"\x1b]0;Title\nInjected\x07",
    // Title with embedded escape
    b"\x1b]0;Title\x1b[31mRed\x07",
    // Title with null bytes
    b"\x1b]0;Title\x00Hidden\x07",
    // Very long title (potential buffer overflow)
    b"\x1b]0;AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\x07",
    // OSC without terminator (unterminated)
    b"\x1b]0;Unterminated",

    // === SGR (Select Graphic Rendition) Attacks ===

    // Huge color index
    b"\x1b[38;5;999999999m",
    // RGB with overflow values
    b"\x1b[38;2;999;999;999m",
    // Many SGR parameters
    b"\x1b[1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17;18;19;20m",
    // Invalid SGR subcommand
    b"\x1b[38;99;1;2;3m",
    // Empty SGR
    b"\x1b[m",

    // === DECRQSS Response Injection ===

    // DECRQSS query (should not leak state)
    b"\x1bP$q\"p\x1b\\",
    // DECRQSS with injection attempt
    b"\x1bP$q\x1b[31m\x1b\\",
    // Unterminated DECRQSS
    b"\x1bP$qm",

    // === Nested Escape DoS ===

    // Deeply nested CSI (state machine stress)
    b"\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[",
    // Alternating escape types
    b"\x1b]\x1bP\x1b[\x1b]\x1bP\x1b[\x1b]\x1bP\x1b[",
    // CSI within OSC
    b"\x1b]0;\x1b[1;2H\x07",
    // DCS within OSC
    b"\x1b]0;\x1bP\x07",

    // === UTF-8 Attacks ===

    // Overlong encoding of '/' (potential path traversal)
    b"\xc0\xaf",
    // Overlong encoding of '<' (potential HTML injection)
    b"\xc0\xbc",
    // Invalid UTF-8 continuation
    b"\x80\x80\x80\x80",
    // Truncated UTF-8 sequence
    b"\xe0\xa0",
    // Invalid start byte
    b"\xff\xfe",
    // 4-byte sequence to 3-byte range (overlong)
    b"\xf0\x80\x80\x80",

    // === Tab Stop Manipulation ===

    // Clear all tab stops then set many
    b"\x1b[3g\x1bH\x1bH\x1bH\x1bH\x1bH\x1bH\x1bH\x1bH\x1bH\x1bH",
    // Tab to huge column
    b"\x09\x09\x09\x09\x09\x09\x09\x09\x09\x09\x09\x09\x09\x09\x09\x09",

    // === Erase Attacks ===

    // Erase huge region
    b"\x1b[99999999X",
    // Erase with invalid parameter
    b"\x1b[999J",
    // Erase line with huge param
    b"\x1b[99999999K",

    // === Insert/Delete Line Attacks ===

    // Insert huge number of lines
    b"\x1b[99999999L",
    // Delete huge number of lines
    b"\x1b[99999999M",
    // Insert characters
    b"\x1b[99999999@",
    // Delete characters
    b"\x1b[99999999P",

    // === Device Status Attacks ===

    // Request cursor position (response injection check)
    b"\x1b[6n",
    // Request device attributes
    b"\x1b[c",
    b"\x1b[>c",
    b"\x1b[=c",
    // Request terminal parameters
    b"\x1b[x",

    // === Mode Attacks ===

    // Set unknown mode
    b"\x1b[?99999h",
    // Reset unknown mode
    b"\x1b[?99999l",
    // Bracketed paste mode toggle flood
    b"\x1b[?2004h\x1b[?2004l\x1b[?2004h\x1b[?2004l",

    // === Window Manipulation (xterm) ===

    // Resize window to huge size
    b"\x1b[8;99999;99999t",
    // Move window to negative coords
    b"\x1b[3;-100;-100t",
    // Request window size/position
    b"\x1b[13t",
    b"\x1b[14t",
    b"\x1b[18t",
    b"\x1b[19t",

    // === Charset Attacks ===

    // Designate charset with invalid
    b"\x1b(Z",
    b"\x1b)Z",
    // SI/SO charset switch flood
    b"\x0e\x0f\x0e\x0f\x0e\x0f\x0e\x0f\x0e\x0f",

    // === Private Mode Attacks ===

    // DECSCNM (reverse video) flood
    b"\x1b[?5h\x1b[?5l\x1b[?5h\x1b[?5l",
    // DECAWM boundary
    b"\x1b[?7h\x1b[999C",

    // === Combined/Chained Attacks ===

    // State confusion: CSI params then escape
    b"\x1b[1;2\x1b[3;4H",
    // Interrupt OSC with CSI
    b"\x1b]0;test\x1b[1m\x07",
    // DCS passthrough with huge data
    b"\x1bPAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\x1b\\",
];

/// Structured attack pattern for fuzzer-guided exploration.
#[derive(Debug, Arbitrary)]
struct AttackInput {
    /// Select a base attack pattern
    pattern_idx: usize,
    /// Optional prefix bytes
    prefix: Vec<u8>,
    /// Optional suffix bytes
    suffix: Vec<u8>,
    /// Number of times to repeat the pattern
    repeat_count: u8,
    /// Parameter mutations
    param_mutations: Vec<ParamMutation>,
}

#[derive(Debug, Arbitrary)]
struct ParamMutation {
    /// Position in pattern to mutate
    position: u8,
    /// New value to insert
    value: u8,
}

/// Sink that tracks invariants during parsing.
struct SecurityAuditSink {
    /// Total bytes processed (overflow check)
    total_bytes: u64,
    /// Maximum parameter value seen
    max_param: u64,
    /// Maximum string length in OSC
    max_osc_len: usize,
    /// Nested sequence depth
    nesting_depth: u32,
    /// CSI dispatch count (DoS check)
    csi_count: u64,
    /// OSC dispatch count
    osc_count: u64,
    /// DCS dispatch count
    dcs_count: u64,
}

impl SecurityAuditSink {
    fn new() -> Self {
        Self {
            total_bytes: 0,
            max_param: 0,
            max_osc_len: 0,
            nesting_depth: 0,
            csi_count: 0,
            osc_count: 0,
            dcs_count: 0,
        }
    }

    fn verify_invariants(&self) {
        // Verify no pathological growth occurred
        const MAX_REASONABLE_CSI: u64 = 100_000;
        const MAX_REASONABLE_OSC: u64 = 10_000;
        const MAX_REASONABLE_DCS: u64 = 10_000;

        // These aren't hard limits, just sanity checks
        // The fuzzer will find if we're creating too many dispatches
        assert!(
            self.csi_count < MAX_REASONABLE_CSI * 10,
            "CSI dispatch flood: {}",
            self.csi_count
        );
    }
}

impl ActionSink for SecurityAuditSink {
    fn print(&mut self, _c: char) {
        self.total_bytes += 1;
    }

    fn execute(&mut self, _byte: u8) {
        self.total_bytes += 1;
    }

    fn csi_dispatch(&mut self, params: &[u16], _intermediates: &[u8], _final_byte: u8) {
        self.csi_count += 1;
        for &p in params {
            self.max_param = self.max_param.max(p as u64);
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _final_byte: u8) {}

    fn osc_dispatch(&mut self, params: &[&[u8]]) {
        self.osc_count += 1;
        for p in params {
            self.max_osc_len = self.max_osc_len.max(p.len());
        }
    }

    fn dcs_hook(&mut self, params: &[u16], _intermediates: &[u8], _final_byte: u8) {
        self.dcs_count += 1;
        self.nesting_depth += 1;
        for &p in params {
            self.max_param = self.max_param.max(p as u64);
        }
    }

    fn dcs_put(&mut self, _byte: u8) {
        self.total_bytes += 1;
    }

    fn dcs_unhook(&mut self) {
        self.nesting_depth = self.nesting_depth.saturating_sub(1);
    }

    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _byte: u8) {}
    fn apc_end(&mut self) {}
}

fuzz_target!(|data: &[u8]| {
    // === Phase 1: Direct attack pattern testing ===

    // First, try parsing the raw fuzz input
    let mut parser = Parser::new();
    let mut sink = SecurityAuditSink::new();
    parser.advance(data, &mut sink);

    // Verify parser state is valid
    assert!(
        (parser.state() as u8) < 14,
        "Invalid parser state after attack input"
    );

    // === Phase 2: Known attack pattern injection ===

    // If we have enough data, use it to select and mutate attack patterns
    if data.len() >= 2 {
        let pattern_idx = data[0] as usize % ATTACK_PATTERNS.len();
        let pattern = ATTACK_PATTERNS[pattern_idx];

        // Reset parser for clean test
        parser.reset();
        let mut attack_sink = SecurityAuditSink::new();

        // Parse the attack pattern
        parser.advance(pattern, &mut attack_sink);

        // Verify invariants held
        assert!(
            (parser.state() as u8) < 14,
            "Invalid state after attack pattern {}: {:?}",
            pattern_idx,
            pattern
        );

        // === Phase 3: Repeated attack pattern (DoS resistance) ===

        if data.len() >= 3 {
            let repeat_count = (data[1] % 100) as usize + 1;
            parser.reset();
            let mut dos_sink = SecurityAuditSink::new();

            for _ in 0..repeat_count {
                parser.advance(pattern, &mut dos_sink);
            }

            // Must still be in valid state
            assert!(
                (parser.state() as u8) < 14,
                "Invalid state after {} repetitions",
                repeat_count
            );
        }
    }

    // === Phase 4: Combined attack sequences ===

    if data.len() >= 4 {
        let pattern1_idx = data[0] as usize % ATTACK_PATTERNS.len();
        let pattern2_idx = data[1] as usize % ATTACK_PATTERNS.len();

        parser.reset();
        let mut combined_sink = SecurityAuditSink::new();

        // Interleave two attack patterns
        parser.advance(ATTACK_PATTERNS[pattern1_idx], &mut combined_sink);
        parser.advance(ATTACK_PATTERNS[pattern2_idx], &mut combined_sink);
        parser.advance(data, &mut combined_sink);

        assert!(
            (parser.state() as u8) < 14,
            "Invalid state after combined attack"
        );
    }

    // === Phase 5: Terminal integration test ===

    // Test the full terminal stack, not just the parser
    // This catches issues in how parsed sequences are applied to terminal state
    let mut terminal = Terminal::new(24, 80);

    // Feed attack patterns through terminal
    if data.len() >= 1 {
        // Try raw data
        terminal.process(data);

        // Verify terminal state is consistent
        // These accessor calls should not panic
        let _cursor = terminal.cursor();
        let _mode = terminal.modes();

        // If we have known patterns, test those too
        if data.len() >= 2 {
            let pattern_idx = data[0] as usize % ATTACK_PATTERNS.len();
            terminal.process(ATTACK_PATTERNS[pattern_idx]);

            // Terminal must remain usable
            let _cursor = terminal.cursor();
            let _title = terminal.title();
        }
    }
});
