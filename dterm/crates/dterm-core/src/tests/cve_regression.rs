//! CVE Regression Tests
//!
//! This module contains tests for patterns derived from real CVE vulnerabilities
//! found in terminal emulators. Each test ensures that dterm-core handles these
//! attack patterns safely.
//!
//! ## References
//!
//! - CVE-2022-45063: xterm cursor position integer overflow
//! - CVE-2021-39537: ncurses heap buffer overflow
//! - CVE-2019-8741: iTerm2 title injection
//! - CVE-2003-0063: xterm window title manipulation
//! - Various rxvt, VTE, mintty vulnerabilities
//!
//! ## Test Categories
//!
//! 1. Integer overflow in cursor/scroll positioning
//! 2. Title/icon name injection attacks
//! 3. SGR parameter overflow
//! 4. OSC command injection
//! 5. Escape sequence nesting attacks
//! 6. UTF-8 encoding attacks
//! 7. Resource exhaustion attacks

use crate::terminal::Terminal;

/// Helper to verify terminal state is valid after feeding data.
fn verify_after_feed(data: &[u8]) {
    let mut terminal = Terminal::new(24, 80);
    terminal.process(data);

    // Verify basic invariants
    let cursor = terminal.cursor();
    assert!(cursor.col <= terminal.cols(), "Cursor col out of bounds");
    assert!(cursor.row < terminal.rows(), "Cursor row out of bounds");
}

// ============================================================================
// CVE-2022-45063 Pattern: Integer Overflow in Cursor Positioning (xterm)
// ============================================================================

#[test]
fn cve_cursor_position_max_u32() {
    // Pattern: \x1b[4294967295;4294967295H
    verify_after_feed(b"\x1b[4294967295;4294967295H");
}

#[test]
fn cve_cursor_position_max_i32() {
    // Pattern: \x1b[2147483647;2147483647H
    verify_after_feed(b"\x1b[2147483647;2147483647H");
}

#[test]
fn cve_cursor_position_overflow_arithmetic() {
    // Values that might cause overflow in row*cols calculations
    verify_after_feed(b"\x1b[65535;65535H");
    verify_after_feed(b"\x1b[99999999;99999999H");
}

#[test]
fn cve_cursor_relative_overflow() {
    // Large relative cursor movements
    verify_after_feed(b"\x1b[999999999A"); // Up
    verify_after_feed(b"\x1b[999999999B"); // Down
    verify_after_feed(b"\x1b[999999999C"); // Forward
    verify_after_feed(b"\x1b[999999999D"); // Back
}

// ============================================================================
// CVE-2003-0063 / CVE-2019-8741 Pattern: Title Injection
// ============================================================================

#[test]
fn cve_title_with_escape_injection() {
    // Title containing escape sequences
    let mut terminal = Terminal::new(24, 80);
    terminal.process(b"\x1b]0;Safe Title\x1b[31mRed Injection\x07");

    let title = terminal.title();
    // Title should NOT contain escape sequences interpreted as formatting
    assert!(
        !title.contains("\x1b["),
        "Title should not contain raw escapes"
    );
}

#[test]
fn cve_title_with_newline_injection() {
    // Title containing newlines (could cause log injection)
    let mut terminal = Terminal::new(24, 80);
    terminal.process(b"\x1b]0;Line1\nLine2\x07");

    // Should handle gracefully - implementation may strip or keep newlines
    let _title = terminal.title();
    // Key: no panic
}

#[test]
fn cve_title_with_null_injection() {
    // Title containing null bytes
    let mut terminal = Terminal::new(24, 80);
    terminal.process(b"\x1b]0;Before\x00After\x07");

    let _title = terminal.title();
    // Key: no panic, null handled
}

#[test]
fn cve_title_very_long() {
    // Extremely long title (potential buffer overflow)
    let mut long_title = b"\x1b]0;".to_vec();
    long_title.extend(std::iter::repeat_n(b'A', 100_000));
    long_title.push(0x07);

    let mut terminal = Terminal::new(24, 80);
    terminal.process(&long_title);
    // Key: no panic, memory bounded
}

#[test]
fn cve_title_unterminated() {
    // Unterminated OSC sequence
    let mut terminal = Terminal::new(24, 80);
    terminal.process(b"\x1b]0;Unterminated title without ST or BEL");

    // Feed more normal data - should recover
    terminal.process(b"Normal text\r\n");

    let _cursor = terminal.cursor();
    // Key: parser recovers
}

// ============================================================================
// SGR Parameter Overflow
// ============================================================================

#[test]
fn cve_sgr_huge_color_index() {
    // SGR with huge color index
    verify_after_feed(b"\x1b[38;5;999999999m");
    verify_after_feed(b"\x1b[48;5;4294967295m");
}

#[test]
fn cve_sgr_rgb_overflow() {
    // RGB with overflow values
    verify_after_feed(b"\x1b[38;2;999;999;999m");
    verify_after_feed(b"\x1b[38;2;4294967295;4294967295;4294967295m");
}

#[test]
fn cve_sgr_many_parameters() {
    // Many SGR parameters
    let mut seq = b"\x1b[".to_vec();
    for i in 0..100 {
        if i > 0 {
            seq.push(b';');
        }
        seq.extend(format!("{}", i % 256).as_bytes());
    }
    seq.push(b'm');

    verify_after_feed(&seq);
}

#[test]
fn cve_sgr_invalid_subcommand() {
    // Invalid SGR subcommand
    verify_after_feed(b"\x1b[38;99;1;2;3m");
    verify_after_feed(b"\x1b[38;256;0m");
}

// ============================================================================
// Scroll Region Attacks
// ============================================================================

#[test]
fn cve_scroll_region_inverted() {
    // Inverted scroll region (top > bottom)
    verify_after_feed(b"\x1b[100;1r");
    verify_after_feed(b"\x1b[24;1r");
}

#[test]
fn cve_scroll_region_huge() {
    // Huge scroll region
    verify_after_feed(b"\x1b[1;4294967295r");
    verify_after_feed(b"\x1b[1;999999999r");
}

#[test]
fn cve_scroll_region_zero() {
    // Zero scroll region
    verify_after_feed(b"\x1b[0;0r");
}

#[test]
fn cve_scroll_region_single_line() {
    // Single line scroll region
    verify_after_feed(b"\x1b[5;5r");
}

// ============================================================================
// Nested Escape Sequence Attacks
// ============================================================================

#[test]
fn cve_deeply_nested_csi() {
    // CSI inside CSI (state machine confusion)
    verify_after_feed(b"\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[\x1b[");
}

#[test]
fn cve_alternating_escapes() {
    // Rapidly alternating escape types
    verify_after_feed(b"\x1b]\x1bP\x1b[\x1b]\x1bP\x1b[\x1b]\x1bP\x1b[");
}

#[test]
fn cve_csi_within_osc() {
    // CSI inside OSC
    verify_after_feed(b"\x1b]0;\x1b[1;2H\x07");
}

#[test]
fn cve_dcs_within_osc() {
    // DCS inside OSC
    verify_after_feed(b"\x1b]0;\x1bP\x07");
}

#[test]
fn cve_osc_interrupt() {
    // OSC interrupted by CSI
    verify_after_feed(b"\x1b]0;test\x1b[1m\x07");
}

// ============================================================================
// UTF-8 Encoding Attacks
// ============================================================================

#[test]
fn cve_utf8_overlong_slash() {
    // Overlong encoding of '/' (path traversal bypass)
    verify_after_feed(b"\xc0\xaf");
}

#[test]
fn cve_utf8_overlong_lt() {
    // Overlong encoding of '<' (HTML injection bypass)
    verify_after_feed(b"\xc0\xbc");
}

#[test]
fn cve_utf8_invalid_continuation() {
    // Invalid UTF-8 continuation bytes
    verify_after_feed(b"\x80\x80\x80\x80");
}

#[test]
fn cve_utf8_truncated() {
    // Truncated UTF-8 sequences
    verify_after_feed(b"\xe0\xa0"); // 3-byte truncated
    verify_after_feed(b"\xf0\x90\x80"); // 4-byte truncated
}

#[test]
fn cve_utf8_invalid_start() {
    // Invalid start bytes
    verify_after_feed(b"\xff\xfe");
    verify_after_feed(b"\xfe\xff");
}

#[test]
fn cve_utf8_overlong_4byte() {
    // 4-byte overlong encoding
    verify_after_feed(b"\xf0\x80\x80\x80");
}

// ============================================================================
// Device Status / Query Attacks
// ============================================================================

#[test]
fn cve_decrqss_injection() {
    // DECRQSS should not leak internal state unsafely
    let mut terminal = Terminal::new(24, 80);
    terminal.process(b"\x1bP$q\"p\x1b\\");

    // Terminal should remain functional
    terminal.process(b"Test\r\n");
    let _cursor = terminal.cursor();
}

#[test]
fn cve_cursor_position_report() {
    // CPR request should not cause response injection
    let mut terminal = Terminal::new(24, 80);
    terminal.process(b"\x1b[6n");

    // Check for pending response
    // Note: actual response handling depends on implementation
    terminal.process(b"Test\r\n");
}

#[test]
fn cve_device_attributes() {
    // DA1, DA2, DA3 queries
    verify_after_feed(b"\x1b[c");
    verify_after_feed(b"\x1b[>c");
    verify_after_feed(b"\x1b[=c");
}

// ============================================================================
// Erase Operations Attacks
// ============================================================================

#[test]
fn cve_erase_huge_count() {
    // Erase with huge count
    verify_after_feed(b"\x1b[99999999X");
    verify_after_feed(b"\x1b[4294967295X");
}

#[test]
fn cve_erase_invalid_param() {
    // ED/EL with invalid parameter
    verify_after_feed(b"\x1b[999J");
    verify_after_feed(b"\x1b[999K");
}

#[test]
fn cve_insert_delete_huge() {
    // Insert/delete line with huge count
    verify_after_feed(b"\x1b[99999999L");
    verify_after_feed(b"\x1b[99999999M");
    verify_after_feed(b"\x1b[99999999@");
    verify_after_feed(b"\x1b[99999999P");
}

// ============================================================================
// Window Manipulation Attacks (xterm)
// ============================================================================

#[test]
fn cve_window_resize_huge() {
    // Window resize to huge size
    verify_after_feed(b"\x1b[8;99999;99999t");
}

#[test]
fn cve_window_move_negative() {
    // Window move to negative coordinates
    // Note: The "-" is not valid in CSI params, terminal should handle gracefully
    verify_after_feed(b"\x1b[3;-100;-100t");
}

// ============================================================================
// Mode Attacks
// ============================================================================

#[test]
fn cve_mode_unknown() {
    // Set/reset unknown mode
    verify_after_feed(b"\x1b[?99999h");
    verify_after_feed(b"\x1b[?99999l");
    verify_after_feed(b"\x1b[?4294967295h");
}

#[test]
fn cve_mode_flood() {
    // Rapid mode toggles
    let mut seq = Vec::new();
    for _ in 0..1000 {
        seq.extend(b"\x1b[?2004h\x1b[?2004l");
    }
    verify_after_feed(&seq);
}

// ============================================================================
// DCS Passthrough Attacks
// ============================================================================

#[test]
fn cve_dcs_huge_payload() {
    // DCS with huge payload
    let mut seq = b"\x1bP".to_vec();
    seq.extend(std::iter::repeat_n(b'A', 100_000));
    seq.extend(b"\x1b\\");

    verify_after_feed(&seq);
}

#[test]
fn cve_dcs_unterminated() {
    // Unterminated DCS
    let mut terminal = Terminal::new(24, 80);
    terminal.process(b"\x1bPUnterminated passthrough");

    // Should recover with normal input
    terminal.process(b"\x1b\\Normal text\r\n");
    let _cursor = terminal.cursor();
}

// ============================================================================
// Combined/Chained Attacks
// ============================================================================

#[test]
fn cve_state_confusion_csi_params() {
    // CSI params interrupted by another escape
    verify_after_feed(b"\x1b[1;2\x1b[3;4H");
}

#[test]
fn cve_stress_combined() {
    // Multiple attack vectors combined
    let attack = b"\x1b[99999999;99999999H\
                   \x1b]0;Malicious\x1b[31mTitle\x07\
                   \x1b[38;5;999999999m\
                   \x1b[\x1b[\x1b[\x1b[\
                   \x1b[?99999h\
                   \xc0\xaf\xff\xfe\
                   \x1b[99999999X";

    verify_after_feed(attack);
}

#[test]
fn cve_rapid_terminal_operations() {
    let mut terminal = Terminal::new(24, 80);

    // Stress test with rapid operations
    for i in 0_u16..1000 {
        match i % 10 {
            0 => terminal.process(b"\x1b[99999;99999H"),
            1 => terminal.process(b"\x1b]0;Title\x07"),
            2 => terminal.process(b"\x1b[38;5;255m"),
            3 => terminal.process(b"\x1b[2J"),
            4 => terminal.process(b"\x1b[?1049h"),
            5 => terminal.process(b"\x1b[?1049l"),
            6 => terminal.process(b"Normal text\r\n"),
            7 => terminal.resize(80 + (i % 20), 24 + (i % 10)),
            8 => terminal.process(b"\x1b[1;24r"),
            9 => terminal.process(b"\x1b[L\x1b[M"),
            _ => {}
        }

        // Verify state after each operation
        let cursor = terminal.cursor();
        assert!(cursor.col <= terminal.cols());
        assert!(cursor.row < terminal.rows());
    }
}

// ============================================================================
// Resource Exhaustion
// ============================================================================

#[test]
fn cve_title_stack_exhaustion() {
    // Title stack push flood (should be bounded)
    let mut terminal = Terminal::new(24, 80);

    for _ in 0..10000 {
        terminal.process(b"\x1b[22t"); // Push title
    }

    // Terminal should still function
    terminal.process(b"Test\r\n");
    let _cursor = terminal.cursor();
}

#[test]
fn cve_scrollback_exhaustion() {
    // Scrollback flood (should be bounded or lazy)
    let mut terminal = Terminal::new(24, 80);

    for i in 0..100000 {
        terminal.process(format!("Line {}\r\n", i).as_bytes());
    }

    // Terminal should still function
    let _cursor = terminal.cursor();
}

#[test]
fn cve_alternate_screen_toggle_exhaustion() {
    // Alternate screen toggle flood
    let mut terminal = Terminal::new(24, 80);

    for _ in 0..10000 {
        terminal.process(b"\x1b[?1049h\x1b[?1049l");
    }

    let _cursor = terminal.cursor();
}
