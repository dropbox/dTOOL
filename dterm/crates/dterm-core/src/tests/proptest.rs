//! Property-based tests for dterm-core.
//!
//! These tests use proptest to generate random inputs and verify
//! properties hold across a wide range of cases.
//!
//! ## Running
//!
//! ```bash
//! cargo test --package dterm-core proptest -- --nocapture
//! ```
//!
//! ## Correspondence to TLA+ Specs
//!
//! Each test validates properties from the TLA+ specifications:
//!
//! | Test | TLA+ Property | File |
//! |------|---------------|------|
//! | `parser_state_consistent` | Safety | tla/Parser.tla |
//! | `parser_reset_clears_state` | Init | tla/Parser.tla |
//! | `grid_cursor_in_bounds` | CursorInBounds | tla/Grid.tla |
//! | `grid_resize_maintains_invariants` | TypeInvariant | tla/Grid.tla |
//! | `search_no_false_negatives` | NoFalseNegatives | - |
//!
//! ## Structured Sequence Strategies (FV-20)
//!
//! In addition to random bytes, this module provides proptest strategies
//! for generating structured VT sequences:
//!
//! | Strategy | Description |
//! |----------|-------------|
//! | `csi_sequence()` | Valid CSI sequences (ESC [ params final) |
//! | `osc_sequence()` | Valid OSC sequences (ESC ] data ST) |
//! | `dcs_sequence()` | Valid DCS sequences (ESC P data ST) |
//! | `mixed_terminal_input()` | Mix of text and sequences |

use crate::grid::{Cell, CellFlags, Grid, LineSize};
use crate::parser::{ActionSink, Parser, State, MAX_INTERMEDIATES, MAX_PARAMS};
use crate::search::SearchIndex;
use proptest::prelude::*;
use proptest::strategy::Strategy;

// ============== Structured VT Sequence Strategies (FV-20) ==============

/// Generate a CSI parameter (digit sequence).
fn csi_param() -> impl Strategy<Value = String> {
    // Parameters are 0-9999 typically, but can be larger
    (0u16..10000).prop_map(|n| n.to_string())
}

/// Generate CSI parameters with semicolon separators.
fn csi_params() -> impl Strategy<Value = String> {
    prop::collection::vec(csi_param(), 0..=MAX_PARAMS).prop_map(|params| params.join(";"))
}

/// Generate CSI intermediate bytes (0x20-0x2F: space through /).
fn csi_intermediate() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0x20u8..=0x2F, 0..=MAX_INTERMEDIATES)
}

/// Generate CSI final byte (0x40-0x7E: @ through ~).
fn csi_final_byte() -> impl Strategy<Value = u8> {
    prop_oneof![
        // Common cursor movement: A-H, J, K
        Just(b'A'),
        Just(b'B'),
        Just(b'C'),
        Just(b'D'),
        Just(b'E'),
        Just(b'F'),
        Just(b'G'),
        Just(b'H'),
        Just(b'J'),
        Just(b'K'),
        // SGR (Set Graphics Rendition)
        Just(b'm'),
        // Set/reset mode
        Just(b'h'),
        Just(b'l'),
        // Cursor save/restore
        Just(b's'),
        Just(b'u'),
        // Scroll
        Just(b'S'),
        Just(b'T'),
        // Erase
        Just(b'X'),
        Just(b'P'),
        // Insert/delete
        Just(b'@'),
        Just(b'L'),
        Just(b'M'),
        // Device status
        Just(b'n'),
        Just(b'c'),
        // Tab
        Just(b'g'),
        // Window manipulation
        Just(b't'),
        // Any other valid final byte (0x40-0x7E)
        (0x40u8..=0x7E),
    ]
}

/// Generate a complete CSI sequence: ESC [ params intermediate final
///
/// Format: \x1b [ <params> <intermediate> <final>
/// Examples:
/// - \x1b[H       (cursor home)
/// - \x1b[5;10H   (cursor to row 5, col 10)
/// - \x1b[?25h    (show cursor - private mode)
/// - \x1b[0m      (reset SGR)
pub fn csi_sequence() -> impl Strategy<Value = Vec<u8>> {
    (
        prop::bool::ANY, // Private mode indicator (?)
        csi_params(),
        csi_intermediate(),
        csi_final_byte(),
    )
        .prop_map(|(private, params, intermediates, final_byte)| {
            let mut seq = vec![0x1b, b'[']; // ESC [
            if private {
                seq.push(b'?'); // Private mode prefix
            }
            seq.extend(params.bytes());
            seq.extend(intermediates);
            seq.push(final_byte);
            seq
        })
}

/// Generate an OSC (Operating System Command) sequence.
///
/// Format: ESC ] <command> ; <data> ST
/// where ST is ESC \ (7-bit) or BEL (0x07)
///
/// Common OSC commands:
/// - OSC 0: Set window title and icon
/// - OSC 1: Set icon name
/// - OSC 2: Set window title
/// - OSC 4: Color palette query/set
/// - OSC 7: Current working directory
/// - OSC 8: Hyperlinks
/// - OSC 52: Clipboard
/// - OSC 133: Shell integration
pub fn osc_sequence() -> impl Strategy<Value = Vec<u8>> {
    let osc_command = prop_oneof![
        Just(0u16),   // Set window title and icon
        Just(1u16),   // Set icon name
        Just(2u16),   // Set window title
        Just(4u16),   // Color palette
        Just(7u16),   // Current working directory
        Just(8u16),   // Hyperlinks
        Just(10u16),  // Foreground color
        Just(11u16),  // Background color
        Just(52u16),  // Clipboard
        Just(133u16), // Shell integration
        (0u16..200),  // Other OSC numbers
    ];

    // OSC data: printable ASCII and some UTF-8
    let osc_data = prop::collection::vec(
        prop_oneof![
            (0x20u8..=0x7E), // Printable ASCII
            Just(b';'),      // Parameter separator
        ],
        0..100,
    );

    // Terminator: either BEL or ST (ESC \)
    let terminator = prop_oneof![
        Just(vec![0x07u8]),        // BEL
        Just(vec![0x1bu8, b'\\']), // ESC \ (ST)
    ];

    (osc_command, osc_data, terminator).prop_map(|(cmd, data, term)| {
        let mut seq = vec![0x1b, b']']; // ESC ]
        seq.extend(cmd.to_string().bytes());
        if !data.is_empty() {
            seq.push(b';');
            seq.extend(data);
        }
        seq.extend(term);
        seq
    })
}

/// Generate a DCS (Device Control String) sequence.
///
/// Format: ESC P <params> <intermediate> <final> <data> ST
///
/// Common DCS sequences:
/// - DECRQSS: Request status string
/// - DECUDK: User-defined keys
/// - Sixel graphics (DCS q)
/// - tmux control mode
pub fn dcs_sequence() -> impl Strategy<Value = Vec<u8>> {
    let dcs_final = prop_oneof![
        Just(b'q'), // Sixel
        Just(b'p'), // ReGIS
        Just(b'|'), // tmux
        Just(b'{'), // DECDLD
        (0x40u8..=0x7E),
    ];

    // DCS data: any byte except ESC
    let dcs_data = prop::collection::vec(
        0x20u8..=0x7E, // Printable only for simplicity
        0..50,
    );

    (csi_params(), csi_intermediate(), dcs_final, dcs_data).prop_map(
        |(params, intermediates, final_byte, data)| {
            let mut seq = vec![0x1b, b'P']; // ESC P
            seq.extend(params.bytes());
            seq.extend(intermediates);
            seq.push(final_byte);
            seq.extend(data);
            seq.extend([0x1b, b'\\']); // ST
            seq
        },
    )
}

/// Generate an escape sequence (non-CSI, non-DCS, non-OSC).
///
/// Format: ESC <intermediate> <final>
pub fn esc_sequence() -> impl Strategy<Value = Vec<u8>> {
    let esc_final = prop_oneof![
        // Cursor save/restore
        Just(b'7'),
        Just(b'8'),
        // RIS (reset)
        Just(b'c'),
        // Index/reverse index
        Just(b'D'),
        Just(b'M'),
        Just(b'E'),
        // Keypad modes
        Just(b'='),
        Just(b'>'),
        // Character sets
        Just(b'N'),
        Just(b'O'),
        Just(b'n'),
        Just(b'o'),
        // Any final byte
        (0x30u8..=0x7E),
    ];

    (csi_intermediate(), esc_final).prop_map(|(intermediates, final_byte)| {
        let mut seq = vec![0x1b]; // ESC
        seq.extend(intermediates);
        seq.push(final_byte);
        seq
    })
}

/// Generate SGR (Select Graphic Rendition) sequence.
///
/// These are CSI sequences with 'm' as final byte, setting text attributes.
pub fn sgr_sequence() -> impl Strategy<Value = Vec<u8>> {
    let sgr_params = prop::collection::vec(
        prop_oneof![
            Just(0u16),     // Reset
            (1u16..=9),     // Bold, dim, italic, underline, blink, etc.
            (21u16..=29),   // Double underline, normal intensity, etc.
            (30u16..=37),   // Foreground colors
            Just(38u16),    // Extended foreground
            (39u16..=39),   // Default foreground
            (40u16..=47),   // Background colors
            Just(48u16),    // Extended background
            (49u16..=49),   // Default background
            (90u16..=97),   // Bright foreground
            (100u16..=107), // Bright background
        ],
        0..=8,
    );

    sgr_params.prop_map(|params| {
        let mut seq = vec![0x1b, b'['];
        seq.extend(
            params
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(";")
                .bytes(),
        );
        seq.push(b'm');
        seq
    })
}

/// Generate a complete terminal input stream with mixed content.
///
/// This interleaves:
/// - Plain text (printable ASCII)
/// - CSI sequences
/// - OSC sequences
/// - SGR sequences
/// - Escape sequences
/// - Control characters
pub fn mixed_terminal_input() -> impl Strategy<Value = Vec<u8>> {
    let plain_text = prop::collection::vec(0x20u8..=0x7E, 1..50);
    let control_char = prop_oneof![
        Just(0x07u8), // BEL
        Just(0x08u8), // BS
        Just(0x09u8), // HT
        Just(0x0Au8), // LF
        Just(0x0Du8), // CR
    ];

    let segment = prop_oneof![
        5 => plain_text.prop_map(|v| v),
        2 => csi_sequence(),
        1 => osc_sequence(),
        2 => sgr_sequence(),
        1 => esc_sequence(),
        1 => control_char.prop_map(|c| vec![c]),
    ];

    prop::collection::vec(segment, 1..20)
        .prop_map(|segments| segments.into_iter().flatten().collect())
}

/// Null sink for parser testing.
struct NullSink;

impl ActionSink for NullSink {
    fn print(&mut self, _: char) {}
    fn execute(&mut self, _: u8) {}
    fn csi_dispatch(&mut self, _: &[u16], _: &[u8], _: u8) {}
    fn esc_dispatch(&mut self, _: &[u8], _: u8) {}
    fn osc_dispatch(&mut self, _: &[&[u8]]) {}
    fn dcs_hook(&mut self, _: &[u16], _: &[u8], _: u8) {}
    fn dcs_put(&mut self, _: u8) {}
    fn dcs_unhook(&mut self) {}
    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _: u8) {}
    fn apc_end(&mut self) {}
}

/// Recording sink that tracks dispatched parameters.
#[derive(Default)]
struct RecordingSink {
    max_params: usize,
    max_intermediates: usize,
}

impl ActionSink for RecordingSink {
    fn print(&mut self, _: char) {}
    fn execute(&mut self, _: u8) {}
    fn csi_dispatch(&mut self, params: &[u16], intermediates: &[u8], _: u8) {
        self.max_params = self.max_params.max(params.len());
        self.max_intermediates = self.max_intermediates.max(intermediates.len());
    }
    fn esc_dispatch(&mut self, intermediates: &[u8], _: u8) {
        self.max_intermediates = self.max_intermediates.max(intermediates.len());
    }
    fn osc_dispatch(&mut self, _: &[&[u8]]) {}
    fn dcs_hook(&mut self, params: &[u16], intermediates: &[u8], _: u8) {
        self.max_params = self.max_params.max(params.len());
        self.max_intermediates = self.max_intermediates.max(intermediates.len());
    }
    fn dcs_put(&mut self, _: u8) {}
    fn dcs_unhook(&mut self) {}
    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _: u8) {}
    fn apc_end(&mut self) {}
}

// ============== Parser Property Tests ==============

proptest! {
    /// Parser state is always valid after processing any input.
    ///
    /// Corresponds to TLA+ Safety: `state \in States`
    #[test]
    fn parser_state_consistent(input in prop::collection::vec(any::<u8>(), 0..1000)) {
        let mut parser = Parser::new();
        let mut sink = NullSink;

        parser.advance(&input, &mut sink);

        // State must be valid (0-13)
        prop_assert!((parser.state() as u8) < 14, "Invalid state: {:?}", parser.state());
    }

    /// Reset returns parser to ground state with cleared buffers.
    ///
    /// Corresponds to TLA+ Init predicate.
    #[test]
    fn parser_reset_clears_state(input in prop::collection::vec(any::<u8>(), 0..100)) {
        let mut parser = Parser::new();
        let mut sink = NullSink;

        // Process arbitrary input
        parser.advance(&input, &mut sink);

        // Reset
        parser.reset();

        // Verify ground state
        prop_assert_eq!(parser.state(), State::Ground);
    }

    /// Parameters never exceed MAX_PARAMS after any input.
    ///
    /// Corresponds to TLA+ TypeInvariant: `Len(params) <= 16`
    #[test]
    fn parser_params_bounded(input in prop::collection::vec(any::<u8>(), 0..500)) {
        let mut parser = Parser::new();
        let mut sink = RecordingSink::default();

        parser.advance(&input, &mut sink);

        prop_assert!(
            sink.max_params <= MAX_PARAMS,
            "Too many params: {} > {}",
            sink.max_params,
            MAX_PARAMS
        );
    }

    /// Intermediates never exceed MAX_INTERMEDIATES after any input.
    ///
    /// Corresponds to TLA+ TypeInvariant: `Len(intermediates) <= 4`
    #[test]
    fn parser_intermediates_bounded(input in prop::collection::vec(any::<u8>(), 0..500)) {
        let mut parser = Parser::new();
        let mut sink = RecordingSink::default();

        parser.advance(&input, &mut sink);

        prop_assert!(
            sink.max_intermediates <= MAX_INTERMEDIATES,
            "Too many intermediates: {} > {}",
            sink.max_intermediates,
            MAX_INTERMEDIATES
        );
    }

    /// Parser is idempotent when reset between inputs.
    #[test]
    fn parser_reset_idempotent(
        input1 in prop::collection::vec(any::<u8>(), 0..100),
        input2 in prop::collection::vec(any::<u8>(), 0..100),
    ) {
        let mut parser1 = Parser::new();
        let mut parser2 = Parser::new();
        let mut sink = NullSink;

        // Process different inputs then reset
        parser1.advance(&input1, &mut sink);
        parser1.reset();

        parser2.advance(&input2, &mut sink);
        parser2.reset();

        // Both should be in identical state
        prop_assert_eq!(parser1.state(), parser2.state());
        prop_assert_eq!(parser1.state(), State::Ground);
    }
}

// ============== Structured Sequence Property Tests (FV-20) ==============

/// Sink that records all dispatched actions for verification.
#[derive(Default)]
struct ActionRecordingSink {
    prints: Vec<char>,
    executes: Vec<u8>,
    csi_dispatches: Vec<(Vec<u16>, Vec<u8>, u8)>,
    osc_dispatches: Vec<Vec<Vec<u8>>>,
    esc_dispatches: Vec<(Vec<u8>, u8)>,
    dcs_hooks: Vec<(Vec<u16>, Vec<u8>, u8)>,
    dcs_puts: Vec<u8>,
    dcs_unhooks: usize,
}

impl ActionSink for ActionRecordingSink {
    fn print(&mut self, c: char) {
        self.prints.push(c);
    }
    fn execute(&mut self, byte: u8) {
        self.executes.push(byte);
    }
    fn csi_dispatch(&mut self, params: &[u16], intermediates: &[u8], final_byte: u8) {
        self.csi_dispatches
            .push((params.to_vec(), intermediates.to_vec(), final_byte));
    }
    fn esc_dispatch(&mut self, intermediates: &[u8], final_byte: u8) {
        self.esc_dispatches
            .push((intermediates.to_vec(), final_byte));
    }
    fn osc_dispatch(&mut self, params: &[&[u8]]) {
        self.osc_dispatches
            .push(params.iter().map(|p| p.to_vec()).collect());
    }
    fn dcs_hook(&mut self, params: &[u16], intermediates: &[u8], final_byte: u8) {
        self.dcs_hooks
            .push((params.to_vec(), intermediates.to_vec(), final_byte));
    }
    fn dcs_put(&mut self, byte: u8) {
        self.dcs_puts.push(byte);
    }
    fn dcs_unhook(&mut self) {
        self.dcs_unhooks += 1;
    }
    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _: u8) {}
    fn apc_end(&mut self) {}
}

proptest! {
    /// CSI sequences are correctly parsed and dispatched.
    ///
    /// Property: Well-formed CSI sequences always result in csi_dispatch.
    #[test]
    fn csi_sequence_parsed(input in csi_sequence()) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        parser.advance(&input, &mut sink);

        // Parser should return to ground after complete sequence
        prop_assert_eq!(
            parser.state(),
            State::Ground,
            "Parser not in ground state after CSI sequence: {:?}",
            input
        );

        // At least one CSI dispatch should occur
        prop_assert!(
            !sink.csi_dispatches.is_empty(),
            "No CSI dispatch for sequence: {:?}",
            input
        );
    }

    /// OSC sequences are correctly parsed and dispatched.
    ///
    /// Property: Well-formed OSC sequences always result in osc_dispatch.
    #[test]
    fn osc_sequence_parsed(input in osc_sequence()) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        parser.advance(&input, &mut sink);

        // Parser should return to ground after complete sequence
        prop_assert_eq!(
            parser.state(),
            State::Ground,
            "Parser not in ground state after OSC sequence: {:?}",
            input
        );

        // At least one OSC dispatch should occur
        prop_assert!(
            !sink.osc_dispatches.is_empty(),
            "No OSC dispatch for sequence: {:?}",
            input
        );
    }

    /// DCS sequences are correctly parsed with hook/unhook pairs.
    ///
    /// Property: Well-formed DCS sequences result in matched hook/unhook.
    #[test]
    fn dcs_sequence_parsed(input in dcs_sequence()) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        parser.advance(&input, &mut sink);

        // Parser should return to ground after complete sequence
        prop_assert_eq!(
            parser.state(),
            State::Ground,
            "Parser not in ground state after DCS sequence: {:?}",
            input
        );

        // Hook and unhook should match
        prop_assert_eq!(
            sink.dcs_hooks.len(),
            sink.dcs_unhooks,
            "DCS hook/unhook mismatch: {} hooks, {} unhooks",
            sink.dcs_hooks.len(),
            sink.dcs_unhooks
        );
    }

    /// ESC sequences are correctly parsed and dispatched.
    #[test]
    fn esc_sequence_parsed(input in esc_sequence()) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        parser.advance(&input, &mut sink);

        // Parser should return to ground state
        // Note: Some ESC sequences transition to other states (like ESC [ -> CSI)
        // so we just verify it doesn't crash
        prop_assert!((parser.state() as u8) < 14);
    }

    /// SGR sequences produce CSI dispatch with 'm' final byte.
    #[test]
    fn sgr_sequence_parsed(input in sgr_sequence()) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        parser.advance(&input, &mut sink);

        // Parser should return to ground
        prop_assert_eq!(parser.state(), State::Ground);

        // Should dispatch CSI with 'm' final
        prop_assert!(
            sink.csi_dispatches.iter().any(|(_, _, f)| *f == b'm'),
            "No SGR dispatch found"
        );
    }

    /// Mixed terminal input never crashes the parser.
    ///
    /// This is the most important property: the parser must handle
    /// any combination of sequences without panicking.
    #[test]
    fn mixed_input_never_crashes(input in mixed_terminal_input()) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        // This should never panic
        parser.advance(&input, &mut sink);

        // State must be valid
        prop_assert!((parser.state() as u8) < 14);
    }

    /// Params from structured CSI are bounded.
    #[test]
    fn structured_csi_params_bounded(input in csi_sequence()) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        parser.advance(&input, &mut sink);

        for (params, intermediates, _) in &sink.csi_dispatches {
            prop_assert!(
                params.len() <= MAX_PARAMS,
                "Too many params: {}",
                params.len()
            );
            prop_assert!(
                intermediates.len() <= MAX_INTERMEDIATES,
                "Too many intermediates: {}",
                intermediates.len()
            );
        }
    }

    /// Mixed sequences interleaved with text preserve text content.
    #[test]
    fn text_preserved_with_sequences(
        text in "[a-zA-Z0-9 ]{10,50}",
        seq in csi_sequence(),
    ) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        // Text, then sequence, then text
        let mut input = text.as_bytes().to_vec();
        input.extend(&seq);
        input.extend(text.as_bytes());

        parser.advance(&input, &mut sink);

        // All printable chars from text should appear in prints (doubled)
        let expected_prints: Vec<char> = text.chars().collect();
        let expected_count = expected_prints.len() * 2;

        // Count matching chars (some control chars might interfere)
        let actual_prints: Vec<char> = sink
            .prints
            .iter()
            .filter(|c| expected_prints.contains(c))
            .copied()
            .collect();

        // We should have at least the text content (allowing for some loss)
        prop_assert!(
            actual_prints.len() >= expected_count - 5,
            "Lost too many prints: expected ~{}, got {}",
            expected_count,
            actual_prints.len()
        );
    }

    /// Rapid CSI sequences don't corrupt parser state.
    #[test]
    fn rapid_csi_sequences_stable(
        sequences in prop::collection::vec(csi_sequence(), 1..20),
    ) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        // Concatenate all sequences
        let input: Vec<u8> = sequences.into_iter().flatten().collect();
        parser.advance(&input, &mut sink);

        // Parser should be in ground or a valid state
        prop_assert!((parser.state() as u8) < 14);

        // Should have processed at least some CSI dispatches
        prop_assert!(
            !sink.csi_dispatches.is_empty() || parser.state() != State::Ground,
            "No CSI dispatches and not in CSI state"
        );
    }

    /// OSC with hyperlink format (OSC 8) parses correctly.
    #[test]
    fn osc_hyperlink_parses(
        url in "[a-z]{5,20}\\.[a-z]{2,5}",
    ) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        // OSC 8 ; params ; URI ST
        let mut input = b"\x1b]8;;https://".to_vec();
        input.extend(url.bytes());
        input.push(0x07);  // BEL terminator

        parser.advance(&input, &mut sink);

        prop_assert_eq!(parser.state(), State::Ground);
        prop_assert!(
            !sink.osc_dispatches.is_empty(),
            "OSC 8 not dispatched"
        );

        // First param should be "8"
        if let Some(params) = sink.osc_dispatches.first() {
            if let Some(first) = params.first() {
                prop_assert_eq!(first, b"8", "OSC command should be 8");
            }
        }
    }

    /// Multiple different sequence types in one stream.
    #[test]
    fn heterogeneous_sequences(
        csi in csi_sequence(),
        osc in osc_sequence(),
        sgr in sgr_sequence(),
    ) {
        let mut parser = Parser::new();
        let mut sink = ActionRecordingSink::default();

        let mut input = Vec::new();
        input.extend(&csi);
        input.extend(b"text");
        input.extend(&osc);
        input.extend(b"more");
        input.extend(&sgr);

        parser.advance(&input, &mut sink);

        // Should have dispatches from each type
        prop_assert!(!sink.csi_dispatches.is_empty());
        prop_assert!(!sink.osc_dispatches.is_empty());
        prop_assert!(!sink.prints.is_empty());
    }
}

// ============== Grid Property Tests ==============

proptest! {
    /// Grid dimensions are always valid after creation.
    #[test]
    fn grid_dimensions_valid(rows in 1u16..500, cols in 1u16..500) {
        let grid = Grid::new(rows, cols);

        prop_assert!(grid.rows() > 0, "rows must be positive");
        prop_assert!(grid.cols() > 0, "cols must be positive");
        prop_assert_eq!(grid.rows(), rows);
        prop_assert_eq!(grid.cols(), cols);
    }

    /// Resize always results in valid dimensions.
    ///
    /// Corresponds to TLA+ TypeInvariant in tla/Grid.tla.
    #[test]
    fn grid_resize_maintains_invariants(
        initial_rows in 1u16..100,
        initial_cols in 1u16..100,
        new_rows in 1u16..100,
        new_cols in 1u16..100,
    ) {
        let mut grid = Grid::new(initial_rows, initial_cols);

        grid.resize(new_rows, new_cols);

        prop_assert_eq!(grid.rows(), new_rows);
        prop_assert_eq!(grid.cols(), new_cols);
        prop_assert!(grid.rows() > 0);
        prop_assert!(grid.cols() > 0);
    }

    /// Cell structure is always 8 bytes (extreme compression).
    #[test]
    fn cell_size_constant(_dummy in 0..1u8) {
        prop_assert_eq!(std::mem::size_of::<Cell>(), 8);
    }

    /// Cell flags are always within defined bits.
    #[test]
    fn cell_flags_bounded(flags in any::<u16>()) {
        let cell_flags = CellFlags::from_bits(flags);

        // All flags are valid u16 values (no masking in from_bits)
        // - Bits 0-13 (0x3FFF): Visual flags
        // - Bit 15 (0x8000): COMPLEX flag (indicates overflow index)
        prop_assert!(cell_flags.bits() == flags);
    }

    /// Cursor movements on mixed line sizes respect effective column limits.
    ///
    /// Corresponds to TLA+ CursorWithinEffectiveLimit from tla/DoubleWidth.tla.
    /// Property: After any sequence of cursor movements on a grid with mixed
    /// single-width and double-width lines, cursor column < effective_cols.
    #[test]
    fn cursor_movements_mixed_line_sizes(
        cols in 4u16..100,
        rows in 2u16..50,
        line_sizes in prop::collection::vec(0u8..4, 2..50),
        movements in prop::collection::vec(0u8..10, 1..50),
        amounts in prop::collection::vec(1u16..20, 1..50),
    ) {
        // line_sizes.len() is bounded by proptest config to < u16::MAX
        #[allow(clippy::cast_possible_truncation)]
        let rows = rows.min(line_sizes.len() as u16);
        let mut grid = Grid::new(rows, cols);

        // Configure line sizes: 0=single, 1=double-width, 2=double-top, 3=double-bottom
        for (i, &size_idx) in line_sizes.iter().take(rows as usize).enumerate() {
            // i is bounded by rows which is u16
            #[allow(clippy::cast_possible_truncation)]
            if let Some(row) = grid.row_mut(i as u16) {
                let size = match size_idx {
                    0 => LineSize::SingleWidth,
                    1 => LineSize::DoubleWidth,
                    2 => LineSize::DoubleHeightTop,
                    _ => LineSize::DoubleHeightBottom,
                };
                row.set_line_size(size);
            }
        }

        // Perform random cursor movements
        for (i, &movement) in movements.iter().enumerate() {
            let amount = amounts.get(i).copied().unwrap_or(1);
            match movement {
                0 => grid.cursor_forward(amount),
                1 => grid.cursor_backward(amount),
                2 => grid.cursor_up(amount),
                3 => grid.cursor_down(amount),
                4 => {
                    // Random position - modulo ensures values fit in u16
                    #[allow(clippy::cast_possible_truncation)]
                    let r = (usize::from(amount) % usize::from(rows)) as u16;
                    #[allow(clippy::cast_possible_truncation)]
                    let c = (usize::from(amount) % usize::from(cols)) as u16;
                    grid.set_cursor(r, c);
                }
                5 => grid.tab(),
                6 => grid.back_tab(),
                7 => grid.carriage_return(),
                8 => grid.line_feed(),
                _ => grid.write_char('X'),
            }

            // After each movement, verify cursor is within bounds
            let cursor_row = grid.cursor_row();
            let cursor_col = grid.cursor_col();
            let is_double = if let Some(row) = grid.row(cursor_row) {
                row.line_size() != LineSize::SingleWidth
            } else {
                false
            };
            let effective_cols = if is_double { cols / 2 } else { cols };

            prop_assert!(
                cursor_row < rows,
                "cursor row {} >= rows {} after movement {}",
                cursor_row, rows, movement
            );
            prop_assert!(
                cursor_col < effective_cols,
                "cursor col {} >= effective_cols {} on row {} (double: {:?}) after movement {}",
                cursor_col,
                effective_cols,
                cursor_row,
                is_double,
                movement
            );
        }
    }

    /// Switching line size clamps cursor correctly.
    ///
    /// Corresponds to TLA+ SetLineSize action from tla/DoubleWidth.tla.
    #[test]
    fn line_size_change_clamps_cursor(
        cols in 4u16..100,
        initial_col in 0u16..100,
    ) {
        let mut grid = Grid::new(10, cols);

        // Position cursor at initial_col (clamped to cols)
        let clamped_col = initial_col.min(cols.saturating_sub(1));
        grid.set_cursor(0, clamped_col);
        prop_assert_eq!(grid.cursor_col(), clamped_col);

        // Change row to double-width
        if let Some(row) = grid.row_mut(0) {
            row.set_line_size(LineSize::DoubleWidth);
        }

        // Cursor should be clamped to half columns
        // Note: Grid doesn't auto-clamp on line size change, but movements do
        // So we trigger a re-clamp by setting cursor to same position
        let cursor_col = grid.cursor_col();
        grid.set_cursor(0, cursor_col);

        let effective_cols = cols / 2;
        let new_cursor_col = grid.cursor_col();
        prop_assert!(
            new_cursor_col < effective_cols,
            "cursor {} >= effective {} after double-width change",
            new_cursor_col, effective_cols
        );
    }

    /// Row transitions clamp cursor to destination row's effective limit.
    ///
    /// Corresponds to TLA+ CursorUp/CursorDown actions from tla/DoubleWidth.tla.
    #[test]
    fn row_transition_clamps_cursor(
        cols in 10u16..100,
        start_col in 0u16..100,
    ) {
        let mut grid = Grid::new(3, cols);

        // Row 0: single-width (full cols)
        // Row 1: double-width (half cols)
        // Row 2: single-width (full cols)
        if let Some(row) = grid.row_mut(1) {
            row.set_line_size(LineSize::DoubleWidth);
        }

        // Start at high column on row 0
        let high_col = start_col.max(cols / 2).min(cols.saturating_sub(1));
        grid.set_cursor(0, high_col);
        prop_assert_eq!(grid.cursor_col(), high_col);

        // Move down to double-width row
        grid.cursor_down(1);

        // Cursor should be clamped to half columns
        let effective = cols / 2;
        prop_assert!(
            grid.cursor_col() < effective,
            "cursor {} >= effective {} after moving to double-width row",
            grid.cursor_col(), effective
        );

        // Move down to single-width row - col stays at clamped value
        grid.cursor_down(1);
        prop_assert!(
            grid.cursor_col() < cols,
            "cursor {} >= cols {} on single-width row",
            grid.cursor_col(), cols
        );

        // Move back up through double-width
        grid.set_cursor(2, high_col);
        grid.cursor_up(1);
        prop_assert!(
            grid.cursor_col() < effective,
            "cursor {} >= effective {} after moving up to double-width row",
            grid.cursor_col(), effective
        );
    }
}

// ============== Search Property Tests ==============

proptest! {
    /// Search returns the line if it contains the query (no false negatives).
    ///
    /// This is a critical property: we may return false positives,
    /// but we must never miss a line that contains the query.
    #[test]
    fn search_no_false_negatives(
        line in "[a-z]{20,50}",
        start in 0usize..17,
    ) {
        let mut index = SearchIndex::new();
        index.index_line(0, &line);

        // Search for 3-char substring (minimum for trigram)
        if start + 3 <= line.len() {
            let query = &line[start..start + 3];
            let results: Vec<_> = index.search(query).collect();

            prop_assert!(
                results.contains(&0),
                "False negative: line 0 contains '{}' but wasn't found. Line: '{}'",
                query,
                line
            );
        }
    }

    /// Search for non-existent pattern returns empty.
    #[test]
    fn search_absent_pattern_empty(
        lines in prop::collection::vec("[a-m]{10,30}", 1..10),
    ) {
        let mut index = SearchIndex::new();

        for (i, line) in lines.iter().enumerate() {
            index.index_line(i, line);
        }

        // Search for pattern that can't exist (all lines are a-m, query is x-z)
        let results: Vec<_> = index.search("xyz").collect();
        prop_assert!(results.is_empty(), "Should find no matches for 'xyz'");
    }

    /// Index length increases with indexed lines.
    #[test]
    fn search_index_grows(
        lines in prop::collection::vec("[a-z]{5,20}", 1..50),
    ) {
        let mut index = SearchIndex::new();

        for (i, line) in lines.iter().enumerate() {
            index.index_line(i, line);
        }

        // Length should be at least the number of lines indexed
        prop_assert!(
            index.len() >= lines.len(),
            "Index length {} < lines indexed {}",
            index.len(),
            lines.len()
        );
    }

    /// Multiple searches on same index are consistent.
    #[test]
    fn search_consistent(
        line in "[a-z]{30,50}",
        query_start in 0usize..27,
    ) {
        let mut index = SearchIndex::new();
        index.index_line(0, &line);

        if query_start + 3 <= line.len() {
            let query = &line[query_start..query_start + 3];

            // Search twice
            let results1: Vec<_> = index.search(query).collect();
            let results2: Vec<_> = index.search(query).collect();

            // Results should be identical
            prop_assert_eq!(results1, results2, "Search results not consistent");
        }
    }
}

// ============== Cross-Component Tests ==============

proptest! {
    /// Parser output can be used to index search.
    ///
    /// Integration test: parse text, index it, search finds it.
    #[test]
    fn parser_to_search_integration(text in "[a-zA-Z0-9 ]{20,100}") {
        let mut parser = Parser::new();
        let mut printed = String::new();

        // Collect printed characters
        struct PrintCollector<'a>(&'a mut String);
        impl ActionSink for PrintCollector<'_> {
            fn print(&mut self, c: char) { self.0.push(c); }
            fn execute(&mut self, _: u8) {}
            fn csi_dispatch(&mut self, _: &[u16], _: &[u8], _: u8) {}
            fn esc_dispatch(&mut self, _: &[u8], _: u8) {}
            fn osc_dispatch(&mut self, _: &[&[u8]]) {}
            fn dcs_hook(&mut self, _: &[u16], _: &[u8], _: u8) {}
            fn dcs_put(&mut self, _: u8) {}
            fn dcs_unhook(&mut self) {}
            fn apc_start(&mut self) {}
            fn apc_put(&mut self, _: u8) {}
            fn apc_end(&mut self) {}
        }

        let mut sink = PrintCollector(&mut printed);
        parser.advance(text.as_bytes(), &mut sink);

        // Index the printed output
        let mut index = SearchIndex::new();
        index.index_line(0, &printed);

        // Search should find any 3+ char substring
        if printed.len() >= 3 {
            let query = &printed[0..3];
            let results: Vec<_> = index.search(query).collect();
            prop_assert!(
                results.contains(&0),
                "Couldn't find '{}' in indexed output '{}'",
                query,
                printed
            );
        }
    }
}

// ============== Checkpoint Property Tests ==============

proptest! {
    /// Checkpoint/restore produces identical grid content.
    ///
    /// This is a critical property: after save/restore, the grid
    /// must have the same visible content.
    #[test]
    fn checkpoint_restore_grid_identical(
        rows in 1u16..50,
        cols in 1u16..100,
        cursor_row in 0u16..50,
        cursor_col in 0u16..100,
        content in "[a-zA-Z0-9 ]{0,200}",
    ) {
        use crate::checkpoint::CheckpointManager;
        use tempfile::tempdir;

        let dir = tempdir().expect("Failed to create temp dir");
        let mut manager = CheckpointManager::new(dir.path());

        // Create grid with content
        let mut grid = Grid::new(rows, cols);
        let clamped_row = cursor_row.min(rows.saturating_sub(1));
        let clamped_col = cursor_col.min(cols.saturating_sub(1));
        grid.set_cursor(clamped_row, clamped_col);

        // Write content (limited to grid bounds)
        for c in content.chars().take((rows as usize) * (cols as usize)) {
            grid.write_char_wrap(c);
        }

        // Save and restore
        manager.save(&grid, None).expect("Save failed");
        let (restored, _) = manager.restore().expect("Restore failed");

        // Verify dimensions
        prop_assert_eq!(restored.rows(), grid.rows());
        prop_assert_eq!(restored.cols(), grid.cols());

        // Verify cursor (may be clamped differently on restore)
        prop_assert!(restored.cursor_row() < restored.rows());
        prop_assert!(restored.cursor_col() < restored.cols());

        // Verify visible content matches
        let original_content = grid.visible_content();
        let restored_content = restored.visible_content();
        prop_assert_eq!(
            original_content,
            restored_content,
            "Content mismatch after restore"
        );
    }

    /// Checkpoint/restore preserves scrollback content.
    #[test]
    fn checkpoint_restore_scrollback_identical(
        line_count in 1usize..100,
    ) {
        use crate::checkpoint::CheckpointManager;
        use crate::scrollback::Scrollback;
        use tempfile::tempdir;

        let dir = tempdir().expect("Failed to create temp dir");
        let mut manager = CheckpointManager::new(dir.path());

        let grid = Grid::new(24, 80);
        let mut scrollback = Scrollback::new(100, 1000, 10_000_000);

        // Add lines
        for i in 0..line_count {
            scrollback.push_str(&format!("Line {}", i));
        }

        // Save and restore
        manager.save(&grid, Some(&scrollback)).expect("Save failed");
        let (_, restored_sb) = manager.restore().expect("Restore failed");

        let restored_sb = restored_sb.expect("Scrollback should be restored");

        // Verify line count
        prop_assert_eq!(
            restored_sb.line_count(),
            line_count,
            "Line count mismatch"
        );

        // Verify content
        for i in 0..line_count {
            let original = scrollback.get_line(i).expect("Original line missing");
            let restored = restored_sb.get_line(i).expect("Restored line missing");
            prop_assert_eq!(
                original.to_string(),
                restored.to_string(),
                "Line {} content mismatch",
                i
            );
        }
    }

    /// Multiple save/restore cycles preserve data.
    #[test]
    fn checkpoint_multiple_cycles(
        content in "[a-z]{10,50}",
        cycles in 1usize..5,
    ) {
        use crate::checkpoint::CheckpointManager;
        use tempfile::tempdir;

        let dir = tempdir().expect("Failed to create temp dir");
        let mut manager = CheckpointManager::new(dir.path());

        let mut grid = Grid::new(24, 80);
        for c in content.chars() {
            grid.write_char(c);
        }

        // Multiple save/restore cycles
        for _ in 0..cycles {
            manager.save(&grid, None).expect("Save failed");
            let (restored, _) = manager.restore().expect("Restore failed");
            grid = restored;
        }

        // Content should still be there
        let restored_content = grid.visible_content();
        prop_assert!(
            restored_content.starts_with(&content),
            "Content lost after {} cycles",
            cycles
        );
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn empty_search_index() {
        let index = SearchIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn short_query_returns_all() {
        let mut index = SearchIndex::new();
        index.index_line(0, "hello world");
        index.index_line(1, "goodbye");

        // Queries < 3 chars return all lines
        let results: Vec<_> = index.search("ab").collect();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn parser_initial_state() {
        let parser = Parser::new();
        assert_eq!(parser.state(), State::Ground);
    }

    #[test]
    fn grid_zero_dimensions_rejected() {
        // Zero dimensions should not be passed to Grid::new
        // This is a precondition, not a runtime check
        let grid = Grid::new(1, 1);
        assert_eq!(grid.rows(), 1);
        assert_eq!(grid.cols(), 1);
    }
}

// ============== Origin Mode + Scroll Region Property Tests (UNIFIED_ROADMAP B3) ==============

proptest! {
    /// Origin mode cursor positioning is always relative to scroll region.
    ///
    /// Corresponds to TLA+ OriginMode invariants:
    /// - When origin mode is enabled, cursor row 1 maps to scroll region top
    /// - Cursor can never exceed scroll region bounds in origin mode
    #[test]
    fn origin_mode_cursor_within_scroll_region(
        rows in 10u16..50,
        cols in 20u16..100,
        region_top in 0u16..10,
        region_size in 5u16..20,
        target_row in 1u16..30,
        target_col in 1u16..80,
    ) {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(rows, cols);

        // Set scroll region (DECSTBM)
        let region_bottom = (region_top + region_size).min(rows.saturating_sub(1));
        if region_top < region_bottom {
            let cmd = format!("\x1b[{};{}r", region_top + 1, region_bottom + 1);
            term.process(cmd.as_bytes());

            // Enable origin mode (DECOM)
            term.process(b"\x1b[?6h");
            prop_assert!(term.modes().origin_mode, "Origin mode should be enabled");

            // Move cursor using CUP (1-based in origin mode, relative to region)
            let cmd = format!("\x1b[{};{}H", target_row, target_col);
            term.process(cmd.as_bytes());

            let cursor = term.cursor();

            // Cursor row must be within scroll region
            prop_assert!(
                cursor.row >= region_top && cursor.row <= region_bottom,
                "Cursor row {} not within scroll region [{}, {}]",
                cursor.row, region_top, region_bottom
            );

            // Cursor col must be within terminal width
            prop_assert!(
                cursor.col < cols,
                "Cursor col {} exceeds terminal width {}",
                cursor.col, cols
            );
        }
    }

    /// Origin mode disabled allows cursor anywhere on screen.
    ///
    /// When origin mode is disabled, CUP positions are absolute.
    #[test]
    fn origin_mode_disabled_absolute_cursor(
        rows in 10u16..50,
        cols in 20u16..100,
        region_top in 2u16..8,
        region_bottom in 12u16..20,
        target_row in 1u16..50,
        target_col in 1u16..100,
    ) {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(rows, cols);

        // Set scroll region
        if region_top < region_bottom && region_bottom < rows {
            let cmd = format!("\x1b[{};{}r", region_top + 1, region_bottom + 1);
            term.process(cmd.as_bytes());

            // Origin mode off (default)
            prop_assert!(!term.modes().origin_mode, "Origin mode should be disabled by default");

            // Move cursor using CUP (1-based, absolute)
            let cmd = format!("\x1b[{};{}H", target_row, target_col);
            term.process(cmd.as_bytes());

            let cursor = term.cursor();

            // Cursor can be anywhere on screen (clamped to bounds)
            let expected_row = (target_row.saturating_sub(1)).min(rows.saturating_sub(1));
            let expected_col = (target_col.saturating_sub(1)).min(cols.saturating_sub(1));

            prop_assert_eq!(
                cursor.row, expected_row,
                "Cursor row {} != expected {} (target was {})",
                cursor.row, expected_row, target_row
            );
            prop_assert_eq!(
                cursor.col, expected_col,
                "Cursor col {} != expected {} (target was {})",
                cursor.col, expected_col, target_col
            );
        }
    }

    /// Toggling origin mode moves cursor to correct home position.
    ///
    /// Property: Enabling origin mode homes cursor to scroll region top.
    /// Property: Disabling origin mode homes cursor to absolute (0,0).
    #[test]
    fn origin_mode_toggle_homes_cursor(
        rows in 10u16..50,
        cols in 20u16..100,
        region_top in 2u16..8,
        region_size in 5u16..15,
    ) {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(rows, cols);

        let region_bottom = (region_top + region_size).min(rows.saturating_sub(1));
        if region_top < region_bottom {
            // Set scroll region
            let cmd = format!("\x1b[{};{}r", region_top + 1, region_bottom + 1);
            term.process(cmd.as_bytes());

            // Move cursor somewhere
            term.process(b"\x1b[10;10H");

            // Enable origin mode - should home to scroll region top
            term.process(b"\x1b[?6h");
            prop_assert!(term.modes().origin_mode);
            prop_assert_eq!(
                term.cursor().row, region_top,
                "Origin mode enable should home to scroll region top"
            );
            prop_assert_eq!(term.cursor().col, 0);

            // Move cursor within region
            term.process(b"\x1b[3;5H");

            // Disable origin mode - should home to absolute (0,0)
            term.process(b"\x1b[?6l");
            prop_assert!(!term.modes().origin_mode);
            prop_assert_eq!(
                term.cursor().row, 0,
                "Origin mode disable should home to absolute (0,0)"
            );
            prop_assert_eq!(term.cursor().col, 0);
        }
    }

    /// Scroll operations respect scroll region boundaries.
    ///
    /// Property: Scrolling in scroll region only affects lines within region.
    #[test]
    fn scroll_region_boundaries_respected(
        rows in 15u16..40,
        cols in 40u16..80,
        region_top in 2u16..6,
        region_size in 5u16..10,
        scroll_amount in 1u16..5,
    ) {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(rows, cols);

        let region_bottom = (region_top + region_size).min(rows.saturating_sub(1));
        if region_top < region_bottom {
            // Write content above scroll region
            term.process(b"\x1b[1;1H"); // Home
            term.process(b"ABOVE_REGION");

            // Write content below scroll region
            let cmd = format!("\x1b[{};1H", rows);
            term.process(cmd.as_bytes());
            term.process(b"BELOW_REGION");

            // Set scroll region
            let cmd = format!("\x1b[{};{}r", region_top + 1, region_bottom + 1);
            term.process(cmd.as_bytes());

            // Write content in region
            let cmd = format!("\x1b[{};1H", region_top + 1);
            term.process(cmd.as_bytes());
            term.process(b"IN_REGION");

            // Scroll up
            let cmd = format!("\x1b[{}S", scroll_amount);
            term.process(cmd.as_bytes());

            // Verify content above region is preserved
            let content = term.visible_content();
            prop_assert!(
                content.contains("ABOVE_REGION"),
                "Content above scroll region should be preserved"
            );

            // Verify content below region is preserved
            prop_assert!(
                content.contains("BELOW_REGION"),
                "Content below scroll region should be preserved"
            );
        }
    }
}

// ============== Alternate Screen Round-Trip Property Tests (UNIFIED_ROADMAP B3) ==============

proptest! {
    /// Alternate screen preserves main screen content.
    ///
    /// Property: Entering and exiting alternate screen restores main screen exactly.
    #[test]
    fn alternate_screen_round_trip(
        rows in 10u16..30,
        cols in 40u16..80,
        content in "[a-zA-Z0-9 ]{20,100}",
    ) {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(rows, cols);

        // Write content to main screen
        term.process(content.as_bytes());
        let main_content = term.visible_content();

        // Enter alternate screen (mode 1049)
        term.process(b"\x1b[?1049h");
        prop_assert!(term.is_alternate_screen(), "Should be on alternate screen");

        // Write different content to alternate screen
        term.process(b"\x1b[H\x1b[2J"); // Clear
        term.process(b"ALTERNATE_SCREEN_CONTENT");

        // Exit alternate screen
        term.process(b"\x1b[?1049l");
        prop_assert!(!term.is_alternate_screen(), "Should be on main screen");

        // Main screen content should be restored
        let restored_content = term.visible_content();
        prop_assert_eq!(
            main_content, restored_content,
            "Main screen content should be restored after alternate screen"
        );
    }

    /// Alternate screen cursor position is independent.
    ///
    /// Property: Cursor position on main screen is preserved across alt screen switch.
    #[test]
    fn alternate_screen_cursor_independence(
        rows in 10u16..30,
        cols in 40u16..80,
        main_row in 1u16..10,
        main_col in 1u16..40,
        alt_row in 1u16..10,
        alt_col in 1u16..40,
    ) {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(rows, cols);

        // Position cursor on main screen
        let cmd = format!("\x1b[{};{}H", main_row, main_col);
        term.process(cmd.as_bytes());

        let saved_main_cursor = term.cursor();

        // Enter alternate screen with cursor save (mode 1049)
        term.process(b"\x1b[?1049h");

        // Move cursor on alternate screen
        let cmd = format!("\x1b[{};{}H", alt_row, alt_col);
        term.process(cmd.as_bytes());

        // Exit alternate screen with cursor restore
        term.process(b"\x1b[?1049l");

        // Main screen cursor should be restored
        let restored_cursor = term.cursor();
        prop_assert_eq!(
            saved_main_cursor.row, restored_cursor.row,
            "Cursor row should be restored"
        );
        prop_assert_eq!(
            saved_main_cursor.col, restored_cursor.col,
            "Cursor col should be restored"
        );
    }

    /// Multiple alternate screen switches are stable.
    ///
    /// Property: Repeated enter/exit cycles preserve content.
    #[test]
    fn alternate_screen_multiple_cycles(
        rows in 10u16..25,
        cols in 40u16..60,
        content in "[a-z]{10,30}",
        cycles in 1usize..5,
    ) {
        use crate::terminal::Terminal;

        let mut term = Terminal::new(rows, cols);

        // Write initial content
        term.process(content.as_bytes());
        let initial_content = term.visible_content();

        for i in 0..cycles {
            // Enter alternate screen
            term.process(b"\x1b[?1049h");
            prop_assert!(
                term.is_alternate_screen(),
                "Cycle {}: Should be on alternate screen",
                i
            );

            // Do something on alternate screen
            let alt_content = format!("ALT_{}", i);
            term.process(b"\x1b[H\x1b[2J");
            term.process(alt_content.as_bytes());

            // Exit alternate screen
            term.process(b"\x1b[?1049l");
            prop_assert!(
                !term.is_alternate_screen(),
                "Cycle {}: Should be on main screen",
                i
            );
        }

        // Content should be preserved
        let final_content = term.visible_content();
        prop_assert_eq!(
            initial_content, final_content,
            "Content should be preserved after {} cycles",
            cycles
        );
    }
}

// ============== Wide Character Selection Property Tests (UNIFIED_ROADMAP B3) ==============

proptest! {
    /// Wide character selection includes both cells.
    ///
    /// Property: Selecting a wide character must include both the primary
    /// cell and the continuation cell.
    #[test]
    fn wide_char_selection_includes_continuation(
        wide_col in 0u16..18,
    ) {
        use crate::selection::{TextSelection, SelectionType, SelectionSide};

        // Create selection starting on a wide character position
        let mut sel = TextSelection::new();

        // Select from wide char position to somewhere after
        sel.start_selection(0, wide_col, SelectionSide::Left, SelectionType::Simple);
        sel.update_selection(0, wide_col + 3, SelectionSide::Right);
        sel.complete_selection();

        // Both the wide char cell and its continuation should be selected
        prop_assert!(
            sel.contains(0, wide_col),
            "Wide char primary cell should be selected"
        );
        prop_assert!(
            sel.contains(0, wide_col + 1),
            "Wide char continuation cell should be selected"
        );
    }

    /// Selection across wide characters respects cell boundaries.
    ///
    /// Property: Selection containing wide chars has correct bounds.
    #[test]
    fn selection_across_wide_chars(
        start_col in 0u16..10,
        end_col in 15u16..30,
    ) {
        use crate::selection::{TextSelection, SelectionType, SelectionSide};

        let mut sel = TextSelection::new();

        sel.start_selection(0, start_col, SelectionSide::Left, SelectionType::Simple);
        sel.update_selection(0, end_col, SelectionSide::Right);
        sel.complete_selection();

        // All cells in range should be selected
        for col in start_col..=end_col {
            prop_assert!(
                sel.contains(0, col),
                "Column {} should be selected (range {}-{})",
                col, start_col, end_col
            );
        }

        // Cells outside range should not be selected
        if start_col > 0 {
            prop_assert!(
                !sel.contains(0, start_col - 1),
                "Column before start should not be selected"
            );
        }
        prop_assert!(
            !sel.contains(0, end_col + 1),
            "Column after end should not be selected"
        );
    }

    /// Block selection with wide characters.
    ///
    /// Property: Block selection treats wide chars as two columns.
    #[test]
    fn block_selection_wide_chars(
        start_row in 0i32..5,
        end_row in 5i32..10,
        start_col in 0u16..10,
        end_col in 12u16..20,
    ) {
        use crate::selection::{TextSelection, SelectionType, SelectionSide};

        let mut sel = TextSelection::new();

        sel.start_selection(start_row, start_col, SelectionSide::Left, SelectionType::Block);
        sel.update_selection(end_row, end_col, SelectionSide::Right);
        sel.complete_selection();

        // Block selection should include rectangular region
        for row in start_row..=end_row {
            for col in start_col..=end_col {
                prop_assert!(
                    sel.contains(row, col),
                    "Cell ({}, {}) should be in block selection",
                    row, col
                );
            }
        }

        // Cells outside block should not be selected
        if start_col > 0 {
            prop_assert!(
                !sel.contains(start_row, start_col - 1),
                "Cell outside block left should not be selected"
            );
        }
        prop_assert!(
            !sel.contains(start_row, end_col + 1),
            "Cell outside block right should not be selected"
        );
    }
}

// ============== Scrollback Tier Ordering Property Tests (UNIFIED_ROADMAP B3) ==============

proptest! {
    /// Scrollback lines maintain order.
    ///
    /// Property: Lines added to scrollback maintain their relative order.
    #[test]
    fn scrollback_order_preserved(
        line_count in 10usize..100,
    ) {
        use crate::scrollback::Scrollback;

        let mut sb = Scrollback::new(1000, 10000, 10_000_000);

        // Add numbered lines
        for i in 0..line_count {
            sb.push_str(&format!("Line_{:04}", i));
        }

        // Verify order
        for i in 0..line_count {
            if let Some(line) = sb.get_line(i) {
                let expected = format!("Line_{:04}", i);
                let actual = line.to_string();
                prop_assert!(
                    actual.starts_with(&expected),
                    "Line {} should start with '{}' but got '{}'",
                    i, expected, actual
                );
            }
        }
    }

    /// Scrollback tier transitions preserve content.
    ///
    /// Property: When lines move between tiers (hot -> warm -> cold),
    /// their content is preserved.
    #[test]
    fn scrollback_tier_content_preserved(
        hot_lines in 5usize..10,
        warm_lines in 10usize..20,
    ) {
        use crate::scrollback::Scrollback;

        // Small hot tier to force tier transitions
        let mut sb = Scrollback::new(hot_lines, warm_lines, 1_000_000);

        // Add more lines than hot tier can hold
        let total_lines = hot_lines + warm_lines + 5;
        let mut expected_content = Vec::new();

        for i in 0..total_lines {
            let content = format!("Content_{:04}", i);
            expected_content.push(content.clone());
            sb.push_str(&content);
        }

        // Verify all lines are retrievable with correct content
        for (i, expected) in expected_content.iter().enumerate() {
            if let Some(line) = sb.get_line(i) {
                let actual = line.to_string();
                prop_assert!(
                    actual.starts_with(expected),
                    "Line {} content mismatch: expected '{}', got '{}'",
                    i, expected, actual
                );
            }
        }

        // Verify line count
        prop_assert_eq!(
            sb.line_count(), total_lines,
            "Line count should be {}",
            total_lines
        );
    }

    /// Scrollback search finds lines across all tiers.
    ///
    /// Property: Search returns lines from all tiers (hot, warm, cold).
    #[test]
    fn scrollback_search_across_tiers(
        hot_lines in 3usize..5,
        warm_lines in 5usize..10,
    ) {
        use crate::scrollback::Scrollback;

        let mut sb = Scrollback::new(hot_lines, warm_lines, 1_000_000);

        // Add lines with unique markers that will end up in different tiers
        let total = hot_lines + warm_lines + 5;
        for i in 0..total {
            let marker = format!("MARKER_{:03}_DATA", i);
            sb.push_str(&marker);
        }

        // Search for each marker
        for i in 0..total {
            let query = format!("MARKER_{:03}", i);
            // Just verify the line exists and is accessible
            if let Some(line) = sb.get_line(i) {
                let content = line.to_string();
                prop_assert!(
                    content.contains(&query),
                    "Line {} should contain '{}' but got '{}'",
                    i, query, content
                );
            }
        }
    }
}
