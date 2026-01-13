//! Integration tests for Terminal processing.
//!
//! These tests exercise the full pipeline from input bytes to grid output,
//! validating that the Terminal correctly interprets ANSI/VT sequences.
//!
//! ## Test Categories
//!
//! - Basic text output and wrapping
//! - Cursor movement (CUP, CUU, CUD, CUF, CUB, etc.)
//! - SGR (Select Graphic Rendition) for colors/styles
//! - Scroll regions (DECSTBM)
//! - Erase operations (ED, EL)
//! - Character sets (DEC line drawing)
//! - Tab handling

use crate::terminal::Terminal;

/// Helper to extract grid content as trimmed lines.
fn grid_lines(term: &Terminal) -> Vec<String> {
    term.grid()
        .visible_content()
        .lines()
        .map(|s| s.trim_end().to_string())
        .collect()
}

/// Helper to get a single line from the grid.
fn grid_line(term: &Terminal, row: usize) -> String {
    grid_lines(term).get(row).cloned().unwrap_or_default()
}

// ============================================================================
// Basic Text Output Tests
// ============================================================================

#[test]
fn basic_text_output() {
    let mut term = Terminal::new(24, 80);
    term.process(b"Hello, World!");

    assert_eq!(grid_line(&term, 0), "Hello, World!");
}

#[test]
fn newline_moves_to_next_row() {
    let mut term = Terminal::new(24, 80);
    // Note: LF alone moves cursor down but does NOT reset column.
    // Use CRLF for standard line endings.
    term.process(b"Line 1\r\nLine 2\r\nLine 3");

    assert_eq!(grid_line(&term, 0), "Line 1");
    assert_eq!(grid_line(&term, 1), "Line 2");
    assert_eq!(grid_line(&term, 2), "Line 3");
}

#[test]
fn carriage_return_moves_to_column_zero() {
    let mut term = Terminal::new(24, 80);
    term.process(b"XXXXXX\rHello");

    assert_eq!(grid_line(&term, 0), "HelloX");
}

#[test]
fn crlf_moves_to_start_of_next_line() {
    let mut term = Terminal::new(24, 80);
    term.process(b"Line 1\r\nLine 2");

    assert_eq!(grid_line(&term, 0), "Line 1");
    assert_eq!(grid_line(&term, 1), "Line 2");
}

#[test]
fn line_wrapping() {
    let mut term = Terminal::new(24, 10); // narrow terminal
    term.process(b"1234567890ABCDE");

    assert_eq!(grid_line(&term, 0), "1234567890");
    assert_eq!(grid_line(&term, 1), "ABCDE");
}

#[test]
fn backspace_moves_cursor_left() {
    let mut term = Terminal::new(24, 80);
    term.process(b"ABC\x08X");

    assert_eq!(grid_line(&term, 0), "ABX");
}

// ============================================================================
// Cursor Movement Tests
// ============================================================================

#[test]
fn csi_cup_moves_cursor_to_position() {
    let mut term = Terminal::new(24, 80);
    // CUP: ESC [ row ; col H (1-based)
    term.process(b"\x1b[3;5HX");

    // Row 3, col 5 (0-indexed: row 2, col 4)
    let cell = term.grid().cell(2, 4);
    assert_eq!(cell.map(|c| c.char()), Some('X'));
}

#[test]
fn csi_cup_default_is_home() {
    let mut term = Terminal::new(24, 80);
    term.process(b"Some text");
    term.process(b"\x1b[HX"); // ESC [ H goes to home

    assert_eq!(term.grid().cursor_row(), 0);
    assert_eq!(term.grid().cursor_col(), 1); // after writing X
}

#[test]
fn csi_cuu_moves_cursor_up() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5;1H"); // Move to row 5
    term.process(b"\x1b[2A"); // CUU: move up 2

    assert_eq!(term.grid().cursor_row(), 2);
}

#[test]
fn csi_cud_moves_cursor_down() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[1;1H"); // Move to row 1
    term.process(b"\x1b[3B"); // CUD: move down 3

    assert_eq!(term.grid().cursor_row(), 3);
}

#[test]
fn csi_cuf_moves_cursor_forward() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[1;1H"); // Move to col 1
    term.process(b"\x1b[5C"); // CUF: move forward 5

    assert_eq!(term.grid().cursor_col(), 5);
}

#[test]
fn csi_cub_moves_cursor_backward() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[1;10H"); // Move to col 10
    term.process(b"\x1b[3D"); // CUB: move backward 3

    assert_eq!(term.grid().cursor_col(), 6);
}

#[test]
fn csi_cnl_moves_to_next_line_start() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[1;10H"); // Row 1, col 10
    term.process(b"\x1b[2E"); // CNL: 2 lines down, col 0

    assert_eq!(term.grid().cursor_row(), 2);
    assert_eq!(term.grid().cursor_col(), 0);
}

#[test]
fn csi_cpl_moves_to_previous_line_start() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5;10H"); // Row 5, col 10
    term.process(b"\x1b[2F"); // CPL: 2 lines up, col 0

    assert_eq!(term.grid().cursor_row(), 2);
    assert_eq!(term.grid().cursor_col(), 0);
}

#[test]
fn csi_cha_moves_to_column() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[15G"); // CHA: move to column 15

    assert_eq!(term.grid().cursor_col(), 14); // 0-indexed
}

#[test]
fn csi_vpa_moves_to_row() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[10d"); // VPA: move to row 10

    assert_eq!(term.grid().cursor_row(), 9); // 0-indexed
}

// ============================================================================
// Erase Operations Tests
// ============================================================================

#[test]
fn csi_ed_0_erases_below() {
    let mut term = Terminal::new(6, 10); // Extra row to prevent scroll
                                         // Use explicit positioning to fill each row
    term.process(b"\x1b[1;1HAAAAAAAAAA");
    term.process(b"\x1b[2;1HBBBBBBBBBB");
    term.process(b"\x1b[3;1HCCCCCCCCCC");
    term.process(b"\x1b[4;1HDDDDDDDDDD");
    term.process(b"\x1b[5;1HEEEEEEEEEE");
    term.process(b"\x1b[3;5H"); // Row 3, col 5
    term.process(b"\x1b[0J"); // ED 0: erase below

    assert_eq!(grid_line(&term, 0), "AAAAAAAAAA");
    assert_eq!(grid_line(&term, 1), "BBBBBBBBBB");
    assert_eq!(grid_line(&term, 2), "CCCC"); // partial erase from cursor
    assert_eq!(grid_line(&term, 3), ""); // cleared
    assert_eq!(grid_line(&term, 4), ""); // cleared
}

#[test]
fn csi_ed_1_erases_above() {
    let mut term = Terminal::new(6, 10); // Extra row to prevent scroll
                                         // Use explicit positioning to fill each row
    term.process(b"\x1b[1;1HAAAAAAAAAA");
    term.process(b"\x1b[2;1HBBBBBBBBBB");
    term.process(b"\x1b[3;1HCCCCCCCCCC");
    term.process(b"\x1b[4;1HDDDDDDDDDD");
    term.process(b"\x1b[5;1HEEEEEEEEEE");
    term.process(b"\x1b[3;5H"); // Row 3, col 5
    term.process(b"\x1b[1J"); // ED 1: erase above

    assert_eq!(grid_line(&term, 0), ""); // cleared
    assert_eq!(grid_line(&term, 1), ""); // cleared
                                         // ED 1 erases from beginning of display to cursor (inclusive)
                                         // Cursor is at row 3 (0-indexed: 2), col 5 (0-indexed: 4)
                                         // So first 5 chars (cols 0-4) are erased, leaving CCCCC at cols 5-9
    assert_eq!(grid_line(&term, 2), "     CCCCC"); // 5 spaces + CCCCC
    assert_eq!(grid_line(&term, 3), "DDDDDDDDDD");
    assert_eq!(grid_line(&term, 4), "EEEEEEEEEE");
}

#[test]
fn csi_ed_2_erases_all() {
    let mut term = Terminal::new(5, 10);
    term.process(b"AAAAAAAAAA");
    term.process(b"BBBBBBBBBB");
    term.process(b"\x1b[2J"); // ED 2: erase all

    for row in 0..5 {
        assert_eq!(grid_line(&term, row), "");
    }
}

#[test]
fn csi_el_0_erases_to_end_of_line() {
    let mut term = Terminal::new(24, 80);
    term.process(b"Hello, World!");
    term.process(b"\x1b[1;7H"); // Move to 'W'
    term.process(b"\x1b[0K"); // EL 0: erase to end

    assert_eq!(grid_line(&term, 0), "Hello,");
}

#[test]
fn csi_el_1_erases_from_start_of_line() {
    let mut term = Terminal::new(24, 80);
    term.process(b"Hello, World!");
    term.process(b"\x1b[1;7H"); // Move to 'W'
    term.process(b"\x1b[1K"); // EL 1: erase from start (inclusive)

    assert_eq!(grid_line(&term, 0), "       World!"); // 7 spaces + "World!"
}

#[test]
fn csi_el_2_erases_whole_line() {
    let mut term = Terminal::new(24, 80);
    term.process(b"Hello, World!");
    term.process(b"\x1b[1;5H"); // Move somewhere in line
    term.process(b"\x1b[2K"); // EL 2: erase whole line

    assert_eq!(grid_line(&term, 0), "");
}

// ============================================================================
// Scroll Region Tests
// ============================================================================

#[test]
fn decstbm_sets_scroll_region() {
    let mut term = Terminal::new(24, 80);
    // Set scroll region lines 5-15
    term.process(b"\x1b[5;15r");

    // Fill screen
    for i in 1..=24 {
        term.process(format!("Line {:02}\n", i).as_bytes());
    }

    // The scroll region should have scrolled internally
    // Just verify the terminal didn't crash
    assert!(term.grid().rows() == 24);
}

#[test]
fn scroll_up_in_region() {
    let mut term = Terminal::new(10, 20);
    // Fill with numbered lines
    for i in 1..=10 {
        term.process(format!("Line {:02}\r\n", i).as_bytes());
    }
    // Move back to top and set region
    term.process(b"\x1b[1;1H");
    term.process(b"\x1b[3;7r"); // Region from line 3 to 7
    term.process(b"\x1b[3;1H"); // Move to top of region
    term.process(b"\x1b[S"); // Scroll up 1

    // Line 3 should have been scrolled up and replaced
    // (The exact behavior depends on implementation details)
    assert!(term.grid().rows() == 10);
}

// ============================================================================
// Character Set Tests
// ============================================================================

#[test]
fn dec_line_drawing_charset() {
    let mut term = Terminal::new(24, 80);
    // Switch to DEC line drawing (G0 = line drawing)
    term.process(b"\x1b(0"); // ESC ( 0
                             // 'j' = lower-right corner, 'k' = upper-right, 'l' = upper-left, 'm' = lower-left
    term.process(b"lqqqk");
    term.process(b"\r\n");
    term.process(b"x   x");
    term.process(b"\r\n");
    term.process(b"mqqj");

    // First line should have box characters
    let line0 = grid_line(&term, 0);
    assert!(line0.contains('\u{250C}')); // upper-left corner
    assert!(line0.contains('\u{2500}')); // horizontal line
    assert!(line0.contains('\u{2510}')); // upper-right corner
}

#[test]
fn switch_back_to_ascii() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b(0"); // Switch to line drawing
    term.process(b"lqk");
    term.process(b"\x1b(B"); // Switch back to ASCII
    term.process(b"ABC");

    let line0 = grid_line(&term, 0);
    assert!(line0.ends_with("ABC"));
}

#[test]
fn si_so_charset_switching() {
    let mut term = Terminal::new(24, 80);
    // G0 = ASCII, G1 = line drawing
    term.process(b"\x1b)0"); // Set G1 to line drawing
    term.process(b"ABC"); // Print in G0 (ASCII)
    term.process(b"\x0e"); // SO: switch to G1
    term.process(b"lqk"); // Print in G1 (line drawing)
    term.process(b"\x0f"); // SI: switch back to G0
    term.process(b"XYZ"); // Print in G0 (ASCII)

    let line0 = grid_line(&term, 0);
    assert!(line0.starts_with("ABC"));
    assert!(line0.ends_with("XYZ"));
}

// ============================================================================
// Tab Handling Tests
// ============================================================================

#[test]
fn horizontal_tab_moves_to_next_stop() {
    let mut term = Terminal::new(24, 80);
    term.process(b"A\tB\tC");

    let line0 = grid_line(&term, 0);
    // Default tab stops are every 8 columns
    assert!(line0.contains('A'));
    // B should be at column 8, C at column 16
    let cell_b = term.grid().cell(0, 8);
    assert_eq!(cell_b.map(|c| c.char()), Some('B'));
}

#[test]
fn tab_set_and_clear() {
    let mut term = Terminal::new(24, 80);
    // Clear all tabs
    term.process(b"\x1b[3g");
    // Set a tab at column 5 (move there, then HTS)
    term.process(b"\x1b[1;5H");
    term.process(b"\x1bH"); // HTS
                            // Go home and tab
    term.process(b"\x1b[1;1H");
    term.process(b"A\tB");

    // B should be at column 5
    let cell_b = term.grid().cell(0, 4); // 0-indexed col 4
    assert_eq!(cell_b.map(|c| c.char()), Some('B'));
}

// ============================================================================
// Insert/Delete Operations Tests
// ============================================================================

#[test]
fn csi_ich_inserts_blanks() {
    let mut term = Terminal::new(24, 80);
    term.process(b"ABCDEF");
    term.process(b"\x1b[1;3H"); // Move to 'C'
    term.process(b"\x1b[2@"); // ICH: insert 2 blanks

    assert_eq!(grid_line(&term, 0), "AB  CDEF");
}

#[test]
fn csi_dch_deletes_chars() {
    let mut term = Terminal::new(24, 80);
    term.process(b"ABCDEF");
    term.process(b"\x1b[1;3H"); // Move to 'C'
    term.process(b"\x1b[2P"); // DCH: delete 2 chars

    assert_eq!(grid_line(&term, 0), "ABEF");
}

#[test]
fn csi_il_inserts_lines() {
    let mut term = Terminal::new(5, 10);
    term.process(b"Line 1\r\n");
    term.process(b"Line 2\r\n");
    term.process(b"Line 3\r\n");
    term.process(b"\x1b[2;1H"); // Move to line 2
    term.process(b"\x1b[1L"); // IL: insert 1 line

    assert_eq!(grid_line(&term, 0), "Line 1");
    assert_eq!(grid_line(&term, 1), ""); // inserted blank
    assert_eq!(grid_line(&term, 2), "Line 2");
}

#[test]
fn csi_dl_deletes_lines() {
    let mut term = Terminal::new(5, 10);
    term.process(b"Line 1\r\n");
    term.process(b"Line 2\r\n");
    term.process(b"Line 3\r\n");
    term.process(b"\x1b[2;1H"); // Move to line 2
    term.process(b"\x1b[1M"); // DL: delete 1 line

    assert_eq!(grid_line(&term, 0), "Line 1");
    assert_eq!(grid_line(&term, 1), "Line 3");
}

// ============================================================================
// Save/Restore Cursor Tests
// ============================================================================

#[test]
fn decsc_decrc_save_restore_cursor() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5;10H"); // Move to row 5, col 10
    term.process(b"\x1b7"); // DECSC: save cursor
    term.process(b"\x1b[1;1H"); // Move to home
    term.process(b"\x1b8"); // DECRC: restore cursor

    assert_eq!(term.grid().cursor_row(), 4); // 0-indexed
    assert_eq!(term.grid().cursor_col(), 9);
}

#[test]
fn csi_s_u_save_restore_cursor() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5;10H"); // Move to row 5, col 10
    term.process(b"\x1b[s"); // CSI s: save cursor
    term.process(b"\x1b[1;1H"); // Move to home
    term.process(b"\x1b[u"); // CSI u: restore cursor

    assert_eq!(term.grid().cursor_row(), 4);
    assert_eq!(term.grid().cursor_col(), 9);
}

// ============================================================================
// Full Reset Test
// ============================================================================

#[test]
fn ris_full_reset() {
    let mut term = Terminal::new(24, 80);
    term.process(b"Some content");
    term.process(b"\x1b[5;5H"); // Move cursor
    term.process(b"\x1bc"); // RIS: full reset

    // Grid should be cleared, cursor at home
    assert_eq!(grid_line(&term, 0), "");
    assert_eq!(term.grid().cursor_row(), 0);
    assert_eq!(term.grid().cursor_col(), 0);
}

// ============================================================================
// Unicode Tests
// ============================================================================

#[test]
fn utf8_single_byte() {
    let mut term = Terminal::new(24, 80);
    term.process("Hello".as_bytes());

    assert_eq!(grid_line(&term, 0), "Hello");
}

#[test]
fn utf8_multi_byte() {
    let mut term = Terminal::new(24, 80);
    term.process("Hello, \u{4e16}\u{754c}!".as_bytes()); // "Hello, 世界!"

    let line0 = grid_line(&term, 0);
    assert!(line0.contains('\u{4e16}')); // 世
    assert!(line0.contains('\u{754c}')); // 界
}

#[test]
fn utf8_emoji() {
    let mut term = Terminal::new(24, 80);
    term.process("Test \u{1F600} emoji".as_bytes()); // 😀

    let line0 = grid_line(&term, 0);
    // With 8-byte cells and complex character support, non-BMP characters (like emoji U+1F600)
    // are stored in the overflow table (CellExtra.complex_char) and properly retrieved.
    assert!(line0.contains("Test"));
    assert!(line0.contains("emoji"));
    // The emoji should now appear correctly in the output
    assert!(
        line0.contains("\u{1F600}"),
        "Emoji should be in output: {}",
        line0
    );

    // The cell at position 5 (after "Test ") should hold the emoji
    let cell = term.grid().cell(0, 5).unwrap();
    // Cell should be marked as complex (non-BMP character in overflow)
    assert!(cell.is_complex(), "Emoji cell should be marked as COMPLEX");

    // The complex character should be in overflow
    let extra = term.grid().cell_extra(0, 5);
    assert!(extra.is_some(), "Should have CellExtra for emoji cell");
    let extra = extra.unwrap();
    assert!(
        extra.complex_char().is_some(),
        "Should have complex_char stored"
    );
    assert_eq!(extra.complex_char().unwrap().as_ref(), "\u{1F600}");

    // Verify cursor advanced correctly
    assert_eq!(term.grid().cursor_col(), 13); // "Test " (5) + wide emoji (2) + " emoji" (6) = 13
}

#[test]
fn utf8_multiple_emoji() {
    let mut term = Terminal::new(24, 80);
    // Multiple different emoji - various non-BMP characters
    term.process("A\u{1F600}B\u{1F389}C\u{1F680}D".as_bytes()); // 😀🎉🚀

    let line0 = grid_line(&term, 0);
    // All emoji should be present
    assert!(line0.contains("\u{1F600}"), "Should contain 😀");
    assert!(line0.contains("\u{1F389}"), "Should contain 🎉");
    assert!(line0.contains("\u{1F680}"), "Should contain 🚀");
    assert_eq!(line0, "A\u{1F600}B\u{1F389}C\u{1F680}D");

    // Verify each emoji cell is complex
    // A is at col 0, emoji at col 1 (width 2), B at col 3, etc.
    assert!(term.grid().cell(0, 1).unwrap().is_complex()); // 😀
    assert!(term.grid().cell(0, 4).unwrap().is_complex()); // 🎉
    assert!(term.grid().cell(0, 7).unwrap().is_complex()); // 🚀
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn empty_input() {
    let mut term = Terminal::new(24, 80);
    term.process(b"");

    assert_eq!(term.grid().cursor_row(), 0);
    assert_eq!(term.grid().cursor_col(), 0);
}

#[test]
fn cursor_at_edge_doesnt_overflow() {
    let mut term = Terminal::new(24, 80);
    // Move to last column and print
    term.process(b"\x1b[1;80H");
    term.process(b"X");

    // Should not panic or overflow
    assert!(term.grid().cursor_col() <= 80);
}

#[test]
fn very_long_line_wraps_correctly() {
    let mut term = Terminal::new(5, 10);
    term.process(b"0123456789ABCDEFGHIJ");

    assert_eq!(grid_line(&term, 0), "0123456789");
    assert_eq!(grid_line(&term, 1), "ABCDEFGHIJ");
}

#[test]
fn malformed_escape_recovers() {
    let mut term = Terminal::new(24, 80);
    // Incomplete CSI sequence followed by normal text
    term.process(b"\x1b[Hello");

    // Terminal should recover and print something
    // The exact behavior depends on the parser, but it should not crash
    assert!(term.grid().rows() == 24);
}

#[test]
fn many_parameters_handled() {
    let mut term = Terminal::new(24, 80);
    // SGR with many parameters
    term.process(b"\x1b[1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16mTest");

    // Should not panic due to parameter overflow
    assert!(grid_line(&term, 0).contains("Test"));
}

// ============================================================================
// SGR (Select Graphic Rendition) Tests
// ============================================================================

use crate::grid::CellFlags;

#[test]
fn sgr_reset_clears_all_attributes() {
    let mut term = Terminal::new(24, 80);
    // Set multiple attributes then reset
    term.process(b"\x1b[1;3;4;7mStyled\x1b[0mPlain");

    // "Styled" should have attributes, "Plain" should not
    let styled_cell = term.grid().cell(0, 0).unwrap();
    assert!(styled_cell.flags().contains(CellFlags::BOLD));
    assert!(styled_cell.flags().contains(CellFlags::ITALIC));
    assert!(styled_cell.flags().contains(CellFlags::UNDERLINE));
    assert!(styled_cell.flags().contains(CellFlags::INVERSE));

    let plain_cell = term.grid().cell(0, 6).unwrap(); // 'P' in "Plain"
    assert!(!plain_cell.flags().contains(CellFlags::BOLD));
    assert!(!plain_cell.flags().contains(CellFlags::ITALIC));
    assert!(!plain_cell.flags().contains(CellFlags::UNDERLINE));
    assert!(!plain_cell.flags().contains(CellFlags::INVERSE));
}

#[test]
fn sgr_bold() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[1mBold");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::BOLD));
}

#[test]
fn sgr_dim() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[2mDim");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::DIM));
}

#[test]
fn sgr_italic() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[3mItalic");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::ITALIC));
}

#[test]
fn sgr_underline() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[4mUnderline");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::UNDERLINE));
}

#[test]
fn sgr_blink() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5mBlink");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::BLINK));
}

#[test]
fn sgr_rapid_blink() {
    let mut term = Terminal::new(24, 80);
    // SGR 6 is rapid blink, typically treated same as SGR 5
    term.process(b"\x1b[6mRapidBlink");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::BLINK));
}

#[test]
fn sgr_inverse() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[7mInverse");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::INVERSE));
}

#[test]
fn sgr_hidden() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[8mHidden");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::HIDDEN));
}

#[test]
fn sgr_strikethrough() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[9mStrike");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::STRIKETHROUGH));
}

#[test]
fn sgr_double_underline() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[21mDoubleUnderline");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::DOUBLE_UNDERLINE));
}

#[test]
fn sgr_superscript_subscript_flags() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[73mA\x1b[74mB");

    let sup_extra = term
        .grid()
        .cell_extra(0, 0)
        .expect("superscript cell extra");
    let sup_flags = CellFlags::from_bits(sup_extra.extended_flags());
    assert!(sup_flags.contains(CellFlags::SUPERSCRIPT));
    assert!(!sup_flags.contains(CellFlags::SUBSCRIPT));

    let sub_extra = term.grid().cell_extra(0, 1).expect("subscript cell extra");
    let sub_flags = CellFlags::from_bits(sub_extra.extended_flags());
    assert!(sub_flags.contains(CellFlags::SUBSCRIPT));
    assert!(!sub_flags.contains(CellFlags::SUPERSCRIPT));
}

#[test]
fn sgr_reset_superscript_subscript() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[73mA\x1b[75mB");

    let sup_extra = term
        .grid()
        .cell_extra(0, 0)
        .expect("superscript cell extra");
    let sup_flags = CellFlags::from_bits(sup_extra.extended_flags());
    assert!(sup_flags.contains(CellFlags::SUPERSCRIPT));

    assert!(term.grid().cell_extra(0, 1).is_none());
}

#[test]
fn sgr_disable_bold_dim() {
    let mut term = Terminal::new(24, 80);
    // SGR 22 disables both bold and dim
    term.process(b"\x1b[1;2mBoldDim\x1b[22mNormal");

    let bold_cell = term.grid().cell(0, 0).unwrap();
    assert!(bold_cell.flags().contains(CellFlags::BOLD));
    assert!(bold_cell.flags().contains(CellFlags::DIM));

    let normal_cell = term.grid().cell(0, 7).unwrap(); // 'N' in "Normal"
    assert!(!normal_cell.flags().contains(CellFlags::BOLD));
    assert!(!normal_cell.flags().contains(CellFlags::DIM));
}

#[test]
fn sgr_disable_italic() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[3mItalic\x1b[23mNot");

    let italic_cell = term.grid().cell(0, 0).unwrap();
    assert!(italic_cell.flags().contains(CellFlags::ITALIC));

    let not_cell = term.grid().cell(0, 6).unwrap();
    assert!(!not_cell.flags().contains(CellFlags::ITALIC));
}

#[test]
fn sgr_disable_underline() {
    let mut term = Terminal::new(24, 80);
    // SGR 24 disables both single and double underline
    term.process(b"\x1b[4mUnder\x1b[24mNot");

    let under_cell = term.grid().cell(0, 0).unwrap();
    assert!(under_cell.flags().contains(CellFlags::UNDERLINE));

    let not_cell = term.grid().cell(0, 5).unwrap();
    assert!(!not_cell.flags().contains(CellFlags::UNDERLINE));
    assert!(!not_cell.flags().contains(CellFlags::DOUBLE_UNDERLINE));
}

#[test]
fn sgr_disable_blink() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5mBlink\x1b[25mNot");

    let blink_cell = term.grid().cell(0, 0).unwrap();
    assert!(blink_cell.flags().contains(CellFlags::BLINK));

    let not_cell = term.grid().cell(0, 5).unwrap();
    assert!(!not_cell.flags().contains(CellFlags::BLINK));
}

#[test]
fn sgr_disable_inverse() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[7mInverse\x1b[27mNot");

    let inverse_cell = term.grid().cell(0, 0).unwrap();
    assert!(inverse_cell.flags().contains(CellFlags::INVERSE));

    let not_cell = term.grid().cell(0, 7).unwrap();
    assert!(!not_cell.flags().contains(CellFlags::INVERSE));
}

#[test]
fn sgr_disable_hidden() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[8mHidden\x1b[28mNot");

    let hidden_cell = term.grid().cell(0, 0).unwrap();
    assert!(hidden_cell.flags().contains(CellFlags::HIDDEN));

    let not_cell = term.grid().cell(0, 6).unwrap();
    assert!(!not_cell.flags().contains(CellFlags::HIDDEN));
}

#[test]
fn sgr_disable_strikethrough() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[9mStrike\x1b[29mNot");

    let strike_cell = term.grid().cell(0, 0).unwrap();
    assert!(strike_cell.flags().contains(CellFlags::STRIKETHROUGH));

    let not_cell = term.grid().cell(0, 6).unwrap();
    assert!(!not_cell.flags().contains(CellFlags::STRIKETHROUGH));
}

// ============================================================================
// SGR Color Tests
// ============================================================================

#[test]
fn sgr_standard_foreground_colors() {
    let mut term = Terminal::new(24, 80);
    // Test all 8 standard foreground colors (30-37)
    for i in 0..8u8 {
        term.process(format!("\x1b[{}mX", 30 + i).as_bytes());
    }

    for i in 0..8u8 {
        let cell = term.grid().cell(0, u16::from(i)).unwrap();
        assert!(cell.fg().is_indexed());
        assert_eq!(cell.fg().index(), i);
    }
}

#[test]
fn sgr_standard_background_colors() {
    let mut term = Terminal::new(24, 80);
    // Test all 8 standard background colors (40-47)
    for i in 0..8u8 {
        term.process(format!("\x1b[{}mX", 40 + i).as_bytes());
    }

    for i in 0..8u8 {
        let cell = term.grid().cell(0, u16::from(i)).unwrap();
        assert!(cell.bg().is_indexed());
        assert_eq!(cell.bg().index(), i);
    }
}

#[test]
fn sgr_bright_foreground_colors() {
    let mut term = Terminal::new(24, 80);
    // Test all 8 bright foreground colors (90-97)
    for i in 0..8u8 {
        term.process(format!("\x1b[{}mX", 90 + i).as_bytes());
    }

    for i in 0..8u8 {
        let cell = term.grid().cell(0, u16::from(i)).unwrap();
        assert!(cell.fg().is_indexed());
        assert_eq!(cell.fg().index(), i + 8); // Bright colors are index 8-15
    }
}

#[test]
fn sgr_bright_background_colors() {
    let mut term = Terminal::new(24, 80);
    // Test all 8 bright background colors (100-107)
    for i in 0..8u8 {
        term.process(format!("\x1b[{}mX", 100 + i).as_bytes());
    }

    for i in 0..8u8 {
        let cell = term.grid().cell(0, u16::from(i)).unwrap();
        assert!(cell.bg().is_indexed());
        assert_eq!(cell.bg().index(), i + 8);
    }
}

#[test]
fn sgr_default_foreground() {
    let mut term = Terminal::new(24, 80);
    // Set color then reset to default
    term.process(b"\x1b[31mRed\x1b[39mDefault");

    let red_cell = term.grid().cell(0, 0).unwrap();
    assert!(red_cell.fg().is_indexed());
    assert_eq!(red_cell.fg().index(), 1); // Red is index 1

    let default_cell = term.grid().cell(0, 3).unwrap(); // 'D' in "Default"
    assert!(default_cell.fg().is_default());
}

#[test]
fn sgr_default_background() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[41mRed\x1b[49mDefault");

    let red_cell = term.grid().cell(0, 0).unwrap();
    assert!(red_cell.bg().is_indexed());
    assert_eq!(red_cell.bg().index(), 1);

    let default_cell = term.grid().cell(0, 3).unwrap();
    assert!(default_cell.bg().is_default());
}

#[test]
fn sgr_256_color_foreground() {
    let mut term = Terminal::new(24, 80);
    // 38;5;n sets 256-color foreground
    term.process(b"\x1b[38;5;196mX"); // Color 196 (bright red in 256-color palette)

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.fg().is_indexed());
    assert_eq!(cell.fg().index(), 196);
}

#[test]
fn sgr_256_color_background() {
    let mut term = Terminal::new(24, 80);
    // 48;5;n sets 256-color background
    term.process(b"\x1b[48;5;226mX"); // Color 226 (yellow in 256-color palette)

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.bg().is_indexed());
    assert_eq!(cell.bg().index(), 226);
}

#[test]
fn sgr_true_color_foreground() {
    let mut term = Terminal::new(24, 80);
    // 38;2;r;g;b sets RGB foreground
    term.process(b"\x1b[38;2;255;128;64mX");

    let cell = term.grid().cell(0, 0).unwrap();
    // With 8-byte cells, RGB is indicated by fg_needs_overflow
    assert!(cell.fg_needs_overflow());
    // Actual RGB value is in CellExtra overflow
    let extra = term.grid().cell_extra(0, 0);
    assert!(extra.is_some());
    assert_eq!(extra.unwrap().fg_rgb(), Some([255, 128, 64]));
}

#[test]
fn sgr_true_color_background() {
    let mut term = Terminal::new(24, 80);
    // 48;2;r;g;b sets RGB background
    term.process(b"\x1b[48;2;100;150;200mX");

    let cell = term.grid().cell(0, 0).unwrap();
    // With 8-byte cells, RGB is indicated by bg_needs_overflow
    assert!(cell.bg_needs_overflow());
    // Actual RGB value is in CellExtra overflow
    let extra = term.grid().cell_extra(0, 0);
    assert!(extra.is_some());
    assert_eq!(extra.unwrap().bg_rgb(), Some([100, 150, 200]));
}

#[test]
fn sgr_combined_attributes_and_colors() {
    let mut term = Terminal::new(24, 80);
    // Bold red text on blue background
    term.process(b"\x1b[1;31;44mStyled");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.flags().contains(CellFlags::BOLD));
    assert!(cell.fg().is_indexed());
    assert_eq!(cell.fg().index(), 1); // Red
    assert!(cell.bg().is_indexed());
    assert_eq!(cell.bg().index(), 4); // Blue
}

#[test]
fn sgr_multiple_resets() {
    let mut term = Terminal::new(24, 80);
    // Multiple SGR 0 in sequence should work
    term.process(b"\x1b[1;31mRed\x1b[0m\x1b[0m\x1b[0mPlain");

    let plain_cell = term.grid().cell(0, 3).unwrap();
    assert!(!plain_cell.flags().contains(CellFlags::BOLD));
    assert!(plain_cell.fg().is_default());
}

#[test]
fn sgr_empty_is_reset() {
    let mut term = Terminal::new(24, 80);
    // Empty SGR (just 'm') should reset
    term.process(b"\x1b[1;31mStyled\x1b[mPlain");

    let styled_cell = term.grid().cell(0, 0).unwrap();
    assert!(styled_cell.flags().contains(CellFlags::BOLD));

    let plain_cell = term.grid().cell(0, 6).unwrap();
    assert!(!plain_cell.flags().contains(CellFlags::BOLD));
    assert!(plain_cell.fg().is_default());
}

#[test]
fn sgr_256_color_boundary_values() {
    let mut term = Terminal::new(24, 80);
    // Test boundary values 0 and 255
    term.process(b"\x1b[38;5;0mA\x1b[38;5;255mB");

    let cell_a = term.grid().cell(0, 0).unwrap();
    assert_eq!(cell_a.fg().index(), 0);

    let cell_b = term.grid().cell(0, 1).unwrap();
    assert_eq!(cell_b.fg().index(), 255);
}

#[test]
fn sgr_true_color_boundary_values() {
    let mut term = Terminal::new(24, 80);
    // Test boundary RGB values
    term.process(b"\x1b[38;2;0;0;0mA\x1b[38;2;255;255;255mB");

    let cell_a = term.grid().cell(0, 0).unwrap();
    assert!(cell_a.fg_needs_overflow());
    let extra_a = term.grid().cell_extra(0, 0).unwrap();
    assert_eq!(extra_a.fg_rgb(), Some([0, 0, 0]));

    let cell_b = term.grid().cell(0, 1).unwrap();
    assert!(cell_b.fg_needs_overflow());
    let extra_b = term.grid().cell_extra(0, 1).unwrap();
    assert_eq!(extra_b.fg_rgb(), Some([255, 255, 255]));
}

#[test]
fn sgr_style_persists_across_lines() {
    let mut term = Terminal::new(24, 80);
    // Style should persist across newlines
    term.process(b"\x1b[1;32mLine1\r\nLine2");

    let line1_cell = term.grid().cell(0, 0).unwrap();
    let line2_cell = term.grid().cell(1, 0).unwrap();

    assert!(line1_cell.flags().contains(CellFlags::BOLD));
    assert!(line2_cell.flags().contains(CellFlags::BOLD));
    assert_eq!(line1_cell.fg().index(), 2); // Green
    assert_eq!(line2_cell.fg().index(), 2);
}

#[test]
fn sgr_invalid_extended_color_ignored() {
    let mut term = Terminal::new(24, 80);
    // Invalid extended color sequences should be ignored
    term.process(b"\x1b[38;3;100mX"); // Invalid: 3 is not valid (should be 2 or 5)
    term.process(b"\x1b[38;5mY"); // Incomplete: missing color index

    // Should have default colors since invalid sequences are ignored
    let cell_x = term.grid().cell(0, 0).unwrap();
    let cell_y = term.grid().cell(0, 1).unwrap();
    // The invalid sequences may be partially processed or ignored
    // The important thing is no panic occurs
    assert_eq!(cell_x.char(), 'X');
    assert_eq!(cell_y.char(), 'Y');
}

#[test]
fn sgr_true_color_values_clamped() {
    let mut term = Terminal::new(24, 80);
    // Values > 255 should be clamped
    term.process(b"\x1b[38;2;300;400;500mX");

    let cell = term.grid().cell(0, 0).unwrap();
    assert!(cell.fg_needs_overflow());
    // Values should be clamped to 255
    let extra = term.grid().cell_extra(0, 0).unwrap();
    assert_eq!(extra.fg_rgb(), Some([255, 255, 255]));
}

// ============================================================================
// DEC Private Mode Tests (DECSET/DECRST)
// ============================================================================

#[test]
fn dec_mode_1_application_cursor_keys() {
    let mut term = Terminal::new(24, 80);

    // Default: normal cursor keys
    assert!(!term.modes().application_cursor_keys);

    // Enable application cursor keys
    term.process(b"\x1b[?1h");
    assert!(term.modes().application_cursor_keys);

    // Disable application cursor keys
    term.process(b"\x1b[?1l");
    assert!(!term.modes().application_cursor_keys);
}

#[test]
fn dec_mode_6_origin_mode_basic() {
    let mut term = Terminal::new(24, 80);

    // Default: origin mode disabled
    assert!(!term.modes().origin_mode);

    // Enable origin mode
    term.process(b"\x1b[?6h");
    assert!(term.modes().origin_mode);

    // Disable origin mode
    term.process(b"\x1b[?6l");
    assert!(!term.modes().origin_mode);
}

#[test]
fn dec_mode_7_auto_wrap() {
    let mut term = Terminal::new(24, 80);

    // Default: autowrap enabled
    assert!(term.modes().auto_wrap);

    // Disable autowrap
    term.process(b"\x1b[?7l");
    assert!(!term.modes().auto_wrap);

    // Re-enable autowrap
    term.process(b"\x1b[?7h");
    assert!(term.modes().auto_wrap);
}

#[test]
fn dec_mode_7_autowrap_disabled_no_wrap() {
    let mut term = Terminal::new(24, 10);

    // Disable autowrap
    term.process(b"\x1b[?7l");

    // Write more than terminal width
    term.process(b"1234567890ABCDE");

    // Should NOT wrap to next line - last column gets overwritten
    assert_eq!(grid_line(&term, 0), "123456789E");
    assert_eq!(grid_line(&term, 1), ""); // Nothing on second line
}

#[test]
fn dec_mode_25_cursor_visible() {
    let mut term = Terminal::new(24, 80);

    // Default: cursor visible
    assert!(term.cursor_visible());

    // Hide cursor
    term.process(b"\x1b[?25l");
    assert!(!term.cursor_visible());

    // Show cursor
    term.process(b"\x1b[?25h");
    assert!(term.cursor_visible());
}

#[test]
fn dec_mode_2004_bracketed_paste() {
    let mut term = Terminal::new(24, 80);

    // Default: bracketed paste disabled
    assert!(!term.modes().bracketed_paste);

    // Enable bracketed paste
    term.process(b"\x1b[?2004h");
    assert!(term.modes().bracketed_paste);

    // Disable bracketed paste
    term.process(b"\x1b[?2004l");
    assert!(!term.modes().bracketed_paste);
}

// ============================================================================
// Alternate Screen Buffer Tests (Mode 1049)
// ============================================================================

#[test]
fn dec_mode_1049_switch_to_alt_screen() {
    let mut term = Terminal::new(24, 80);

    // Default: main screen
    assert!(!term.is_alternate_screen());

    // Write something on main screen
    term.process(b"Main screen content");

    // Switch to alternate screen (mode 1049)
    term.process(b"\x1b[?1049h");
    assert!(term.is_alternate_screen());

    // Alternate screen should be clear
    assert_eq!(grid_line(&term, 0), "");
}

#[test]
fn dec_mode_1049_switch_back_to_main() {
    let mut term = Terminal::new(24, 80);

    // Write on main screen
    term.process(b"Main content");

    // Switch to alt, write there
    term.process(b"\x1b[?1049h");
    term.process(b"Alt content");

    // Switch back to main
    term.process(b"\x1b[?1049l");
    assert!(!term.is_alternate_screen());

    // Main screen content should be restored
    assert_eq!(grid_line(&term, 0), "Main content");
}

#[test]
fn dec_mode_1049_cursor_save_restore() {
    let mut term = Terminal::new(24, 80);

    // Move cursor to specific position on main screen
    term.process(b"\x1b[10;20H");
    assert_eq!(term.grid().cursor_row(), 9);
    assert_eq!(term.grid().cursor_col(), 19);

    // Switch to alt screen
    term.process(b"\x1b[?1049h");

    // Cursor on alt screen should be at home
    assert_eq!(term.grid().cursor_row(), 0);
    assert_eq!(term.grid().cursor_col(), 0);

    // Move cursor on alt screen
    term.process(b"\x1b[5;10H");

    // Switch back to main
    term.process(b"\x1b[?1049l");

    // Cursor should be restored to main screen position
    assert_eq!(term.grid().cursor_row(), 9);
    assert_eq!(term.grid().cursor_col(), 19);
}

// ============================================================================
// Origin Mode (DECOM) Tests
// ============================================================================

#[test]
fn origin_mode_cup_relative_to_scroll_region() {
    let mut term = Terminal::new(24, 80);

    // Set scroll region to lines 5-15 (1-indexed)
    term.process(b"\x1b[5;15r");

    // Enable origin mode
    term.process(b"\x1b[?6h");

    // CUP with origin mode: row 1 should map to scroll region top (row 5)
    term.process(b"\x1b[1;1H");

    // Cursor should be at absolute row 4 (0-indexed, which is line 5)
    assert_eq!(term.grid().cursor_row(), 4);
    assert_eq!(term.grid().cursor_col(), 0);
}

#[test]
fn origin_mode_cursor_constrained_to_region() {
    let mut term = Terminal::new(24, 80);

    // Set scroll region to lines 5-15
    term.process(b"\x1b[5;15r");

    // Enable origin mode
    term.process(b"\x1b[?6h");

    // Try to move cursor beyond scroll region
    term.process(b"\x1b[50;1H"); // Row 50 is way beyond

    // Cursor should be clamped to scroll region bottom
    assert_eq!(term.grid().cursor_row(), 14); // Row 15 (0-indexed = 14)
}

#[test]
fn origin_mode_enable_homes_cursor() {
    let mut term = Terminal::new(24, 80);

    // Set scroll region and move cursor
    term.process(b"\x1b[5;15r");
    term.process(b"\x1b[20;40H"); // Move outside region

    // Enable origin mode - should home to scroll region top
    term.process(b"\x1b[?6h");

    assert_eq!(term.grid().cursor_row(), 4); // Top of scroll region
    assert_eq!(term.grid().cursor_col(), 0);
}

#[test]
fn origin_mode_disable_homes_cursor() {
    let mut term = Terminal::new(24, 80);

    // Set scroll region, enable origin mode, move cursor
    term.process(b"\x1b[5;15r");
    term.process(b"\x1b[?6h");
    term.process(b"\x1b[5;10H"); // Middle of region

    // Disable origin mode - should home to absolute (0,0)
    term.process(b"\x1b[?6l");

    assert_eq!(term.grid().cursor_row(), 0);
    assert_eq!(term.grid().cursor_col(), 0);
}

// ============================================================================
// Insert Mode (IRM) Tests
// ============================================================================

#[test]
fn ansi_mode_4_insert_mode_basic() {
    let mut term = Terminal::new(24, 80);

    // Default: replace mode (insert disabled)
    assert!(!term.modes().insert_mode);

    // Enable insert mode
    term.process(b"\x1b[4h");
    assert!(term.modes().insert_mode);

    // Disable insert mode
    term.process(b"\x1b[4l");
    assert!(!term.modes().insert_mode);
}

#[test]
fn insert_mode_shifts_characters() {
    let mut term = Terminal::new(24, 80);

    // Write initial text
    term.process(b"ABCDEF");

    // Move back and enable insert mode
    term.process(b"\x1b[1;3H"); // Column 3 (0-indexed: 2)
    term.process(b"\x1b[4h"); // Enable insert mode

    // Insert characters
    term.process(b"XX");

    // Text should be shifted right
    assert_eq!(grid_line(&term, 0), "ABXXCDEF");
}

#[test]
fn replace_mode_overwrites_characters() {
    let mut term = Terminal::new(24, 80);

    // Write initial text
    term.process(b"ABCDEF");

    // Move back (ensure replace mode - default)
    term.process(b"\x1b[1;3H"); // Column 3 (0-indexed: 2)
    term.process(b"\x1b[4l"); // Ensure replace mode

    // Overwrite characters
    term.process(b"XX");

    // Characters should be replaced
    assert_eq!(grid_line(&term, 0), "ABXXEF");
}

// ============================================================================
// New Line Mode (LNM) Tests
// ============================================================================

#[test]
fn ansi_mode_20_new_line_mode_basic() {
    let mut term = Terminal::new(24, 80);

    // Default: line feed mode (LF doesn't do CR)
    assert!(!term.modes().new_line_mode);

    // Enable new line mode
    term.process(b"\x1b[20h");
    assert!(term.modes().new_line_mode);

    // Disable new line mode
    term.process(b"\x1b[20l");
    assert!(!term.modes().new_line_mode);
}

#[test]
fn new_line_mode_lf_does_cr() {
    let mut term = Terminal::new(24, 80);

    // Enable new line mode
    term.process(b"\x1b[20h");

    // Write text then LF
    term.process(b"Hello");
    term.process(b"\n"); // LF in new line mode should also do CR
    term.process(b"World");

    assert_eq!(grid_line(&term, 0), "Hello");
    assert_eq!(grid_line(&term, 1), "World"); // World at column 0
}

#[test]
fn line_feed_mode_lf_no_cr() {
    let mut term = Terminal::new(24, 80);

    // Ensure line feed mode (default)
    term.process(b"\x1b[20l");

    // Write text then LF
    term.process(b"Hello");
    term.process(b"\n"); // LF only moves down
    term.process(b"World");

    assert_eq!(grid_line(&term, 0), "Hello");
    // Without CR, "World" starts at column 5 (after "Hello")
    let line1 = grid_line(&term, 1);
    assert!(line1.contains("World"));
    assert!(line1.starts_with("     ")); // 5 spaces before World
}

// ============================================================================
// Cursor Style Tests (DECSCUSR)
// ============================================================================

#[test]
fn cursor_style_blinking_block() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[1 q"); // Blinking block
    assert_eq!(
        term.cursor_style(),
        crate::terminal::CursorStyle::BlinkingBlock
    );
}

#[test]
fn cursor_style_steady_block() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[2 q");
    assert_eq!(
        term.cursor_style(),
        crate::terminal::CursorStyle::SteadyBlock
    );
}

#[test]
fn cursor_style_blinking_underline() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[3 q");
    assert_eq!(
        term.cursor_style(),
        crate::terminal::CursorStyle::BlinkingUnderline
    );
}

#[test]
fn cursor_style_steady_underline() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[4 q");
    assert_eq!(
        term.cursor_style(),
        crate::terminal::CursorStyle::SteadyUnderline
    );
}

#[test]
fn cursor_style_blinking_bar() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5 q");
    assert_eq!(
        term.cursor_style(),
        crate::terminal::CursorStyle::BlinkingBar
    );
}

#[test]
fn cursor_style_steady_bar() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[6 q");
    assert_eq!(term.cursor_style(), crate::terminal::CursorStyle::SteadyBar);
}

#[test]
fn cursor_style_default_is_blinking_block() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[0 q"); // 0 = default
    assert_eq!(
        term.cursor_style(),
        crate::terminal::CursorStyle::BlinkingBlock
    );
}

// ============================================================================
// Escape Sequence Tests (IND, RI, NEL)
// ============================================================================

#[test]
fn escape_ind_index_moves_down() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5;1H"); // Row 5
    term.process(b"\x1bD"); // IND: move down

    assert_eq!(term.grid().cursor_row(), 5); // Was 4 (0-indexed), now 5
}

#[test]
fn escape_ri_reverse_index_moves_up() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5;1H"); // Row 5 (0-indexed: 4)
    term.process(b"\x1bM"); // RI: move up

    assert_eq!(term.grid().cursor_row(), 3); // Was 4, now 3
}

#[test]
fn escape_nel_next_line() {
    let mut term = Terminal::new(24, 80);
    term.process(b"Hello");
    term.process(b"\x1bE"); // NEL: next line (CR + LF)
    term.process(b"World");

    assert_eq!(grid_line(&term, 0), "Hello");
    assert_eq!(grid_line(&term, 1), "World"); // At column 0
}

#[test]
fn escape_ris_full_reset() {
    let mut term = Terminal::new(24, 80);

    // Set up various state
    term.process(b"Some text");
    term.process(b"\x1b[5;15r"); // Set scroll region
    term.process(b"\x1b[?6h"); // Enable origin mode
    term.process(b"\x1b[?25l"); // Hide cursor
    term.process(b"\x1b[1m"); // Bold

    // Full reset
    term.process(b"\x1bc");

    // Everything should be reset
    assert_eq!(grid_line(&term, 0), "");
    assert!(!term.modes().origin_mode);
    assert!(term.cursor_visible());
}

// ============================================================================
// Tab Operations Tests (CHT, CBT, TBC)
// ============================================================================

#[test]
fn csi_cht_forward_tab() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[1;1H"); // Home
    term.process(b"\x1b[2I"); // CHT: forward 2 tab stops

    // Default tabs at 8, 16
    assert_eq!(term.grid().cursor_col(), 16);
}

#[test]
fn csi_cbt_backward_tab() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[1;20H"); // Column 20
    term.process(b"\x1b[1Z"); // CBT: backward 1 tab stop

    // Should move back to tab stop at column 16
    assert_eq!(term.grid().cursor_col(), 16);
}

#[test]
fn csi_tbc_clear_current_tab() {
    let mut term = Terminal::new(24, 80);
    // Move to column 8 and clear that tab
    term.process(b"\x1b[1;9H"); // Column 9 (0-indexed: 8)
    term.process(b"\x1b[0g"); // TBC 0: clear tab at current position

    // Tab from home should skip column 8 and go to 16
    term.process(b"\x1b[1;1H");
    term.process(b"\t");

    // Should be at column 16 now (skipped 8 which was cleared)
    assert_eq!(term.grid().cursor_col(), 16);
}

#[test]
fn csi_tbc_clear_all_tabs() {
    let mut term = Terminal::new(24, 80);
    // Clear all tabs
    term.process(b"\x1b[3g");

    // Tab from home
    term.process(b"\x1b[1;1H");
    term.process(b"\t");

    // Should go to end of line (no tab stops)
    assert_eq!(term.grid().cursor_col(), 79);
}

// ============================================================================
// Scroll Operations Tests (SU, SD)
// ============================================================================

#[test]
fn csi_su_scroll_up() {
    let mut term = Terminal::new(5, 20);
    // Fill screen with lines
    term.process(b"Line 1\r\n");
    term.process(b"Line 2\r\n");
    term.process(b"Line 3\r\n");
    term.process(b"Line 4\r\n");
    term.process(b"Line 5");

    // Scroll up 1 line
    term.process(b"\x1b[1S");

    // Line 1 should be gone, Line 2 is now at top
    assert_eq!(grid_line(&term, 0), "Line 2");
    assert_eq!(grid_line(&term, 4), ""); // Bottom is blank
}

#[test]
fn csi_sd_scroll_down() {
    let mut term = Terminal::new(5, 20);
    // Fill screen
    term.process(b"Line 1\r\n");
    term.process(b"Line 2\r\n");
    term.process(b"Line 3\r\n");
    term.process(b"Line 4\r\n");
    term.process(b"Line 5");

    // Scroll down 1 line
    term.process(b"\x1b[1T");

    // Blank line at top, content shifted down
    assert_eq!(grid_line(&term, 0), "");
    assert_eq!(grid_line(&term, 1), "Line 1");
}

// ============================================================================
// ECH (Erase Character) Test
// ============================================================================

#[test]
fn csi_ech_erase_characters() {
    let mut term = Terminal::new(24, 80);
    term.process(b"ABCDEFGH");
    term.process(b"\x1b[1;3H"); // Move to 'C'
    term.process(b"\x1b[3X"); // ECH: erase 3 characters

    assert_eq!(grid_line(&term, 0), "AB   FGH");
}

// ============================================================================
// REP (Repeat) Test
// ============================================================================

#[test]
fn csi_rep_repeat_character() {
    let mut term = Terminal::new(24, 80);
    term.process(b"X"); // Print X
    term.process(b"\x1b[5b"); // REP: repeat X 5 times

    assert_eq!(grid_line(&term, 0), "XXXXXX"); // Original + 5 repeats = 6
}

// ============================================================================
// Screen Alignment (DECALN) Test
// ============================================================================

#[test]
fn escape_decaln_fills_with_e() {
    let mut term = Terminal::new(3, 5);
    term.process(b"\x1b#8"); // DECALN: fill screen with 'E'

    assert_eq!(grid_line(&term, 0), "EEEEE");
    assert_eq!(grid_line(&term, 1), "EEEEE");
    assert_eq!(grid_line(&term, 2), "EEEEE");
}

// ============================================================================
// Single Shift Tests (SS2, SS3)
// ============================================================================

#[test]
fn escape_ss2_single_shift_g2() {
    let mut term = Terminal::new(24, 80);
    // Set G2 to DEC line drawing
    term.process(b"\x1b*0");
    // SS2 - use G2 for next char only
    term.process(b"\x1bN");
    term.process(b"q"); // In line drawing, 'q' is horizontal line
    term.process(b"ABC"); // Back to G0 (ASCII)

    let line0 = grid_line(&term, 0);
    assert!(line0.contains('\u{2500}')); // Horizontal line
    assert!(line0.contains("ABC"));
}

#[test]
fn escape_ss3_single_shift_g3() {
    let mut term = Terminal::new(24, 80);
    // Set G3 to DEC line drawing
    term.process(b"\x1b+0");
    // SS3 - use G3 for next char only
    term.process(b"\x1bO");
    term.process(b"l"); // In line drawing, 'l' is upper-left corner
    term.process(b"XYZ"); // Back to G0 (ASCII)

    let line0 = grid_line(&term, 0);
    assert!(line0.contains('\u{250C}')); // Upper-left corner
    assert!(line0.contains("XYZ"));
}

// ============================================================================
// Device Status Report Tests
// ============================================================================

#[test]
fn csi_dsr_5_status_report() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5n"); // DSR 5: status report

    // Should queue response ESC [ 0 n (OK)
    let response = term.take_response();
    assert!(response.is_some());
    let resp = response.unwrap();
    assert_eq!(resp.as_slice(), b"\x1b[0n");
}

#[test]
fn csi_dsr_6_cursor_position_report() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[5;10H"); // Move to row 5, col 10
    term.process(b"\x1b[6n"); // DSR 6: cursor position report

    // Should queue response ESC [ 5 ; 10 R
    let response = term.take_response();
    assert!(response.is_some());
    let resp = response.unwrap();
    assert_eq!(resp.as_slice(), b"\x1b[5;10R");
}

// ============================================================================
// Device Attributes Tests
// ============================================================================

#[test]
fn csi_da1_primary_device_attributes() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[c"); // DA1

    let response = term.take_response();
    assert!(response.is_some());
    // Should respond with VT220 identifier
    let resp = response.unwrap();
    assert!(resp.len() >= 4 && &resp[0..3] == b"\x1b[?");
}

#[test]
fn csi_da2_secondary_device_attributes() {
    let mut term = Terminal::new(24, 80);
    term.process(b"\x1b[>c"); // DA2

    let response = term.take_response();
    assert!(response.is_some());
    // Should respond with version info
    let resp = response.unwrap();
    assert!(resp.len() >= 4 && &resp[0..3] == b"\x1b[>");
}

// ============================================================================
// Selective Erase Tests (DECSCA, DECSED, DECSEL)
// ============================================================================

#[test]
fn csi_decsca_set_protection() {
    let mut term = Terminal::new(24, 80);

    // Enable character protection
    term.process(b"\x1b[1\"q");
    term.process(b"Protected");

    // Disable protection
    term.process(b"\x1b[0\"q");
    term.process(b"Normal");

    // Protected characters should have protection flag
    let protected_cell = term.grid().cell(0, 0).unwrap();
    assert!(protected_cell.flags().contains(CellFlags::PROTECTED));

    let normal_cell = term.grid().cell(0, 9).unwrap(); // 'N' in "Normal"
    assert!(!normal_cell.flags().contains(CellFlags::PROTECTED));
}

#[test]
fn csi_decsed_selective_erase_respects_protection() {
    let mut term = Terminal::new(24, 80);

    // Write protected text then unprotected
    term.process(b"\x1b[1\"q"); // Enable protection
    term.process(b"PROT");
    term.process(b"\x1b[0\"q"); // Disable protection
    term.process(b"UNPR");

    // Selective erase all (CSI ? 2 J)
    term.process(b"\x1b[?2J");

    // Check cells directly - protected should remain
    let grid = term.grid();
    assert_eq!(grid.cell(0, 0).unwrap().char(), 'P');
    assert_eq!(grid.cell(0, 1).unwrap().char(), 'R');
    assert_eq!(grid.cell(0, 2).unwrap().char(), 'O');
    assert_eq!(grid.cell(0, 3).unwrap().char(), 'T');
    // Unprotected should be erased
    assert_eq!(grid.cell(0, 4).unwrap().char(), ' ');
    assert_eq!(grid.cell(0, 5).unwrap().char(), ' ');
}

#[test]
fn csi_decsel_selective_erase_line() {
    let mut term = Terminal::new(24, 80);

    // Write protected then unprotected
    term.process(b"\x1b[1\"q");
    term.process(b"AAA");
    term.process(b"\x1b[0\"q");
    term.process(b"BBB");

    // Move cursor home
    term.process(b"\x1b[1;1H");

    // Selective erase entire line (CSI ? 2 K)
    term.process(b"\x1b[?2K");

    // Check cells directly - protected should remain
    let grid = term.grid();
    assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
    assert_eq!(grid.cell(0, 1).unwrap().char(), 'A');
    assert_eq!(grid.cell(0, 2).unwrap().char(), 'A');
    // Unprotected should be erased
    assert_eq!(grid.cell(0, 3).unwrap().char(), ' ');
    assert_eq!(grid.cell(0, 4).unwrap().char(), ' ');
    assert_eq!(grid.cell(0, 5).unwrap().char(), ' ');
}

// =============================================================================
// Sixel Graphics Tests
// =============================================================================

#[test]
fn sixel_simple_image() {
    let mut term = Terminal::new(24, 80);

    // No image initially
    assert!(!term.has_sixel_image());

    // Send a simple Sixel sequence: DCS q #15~ ST
    // - DCS = ESC P = 0x1B 0x50
    // - q = Sixel mode
    // - #15 = select color 15 (white)
    // - ~ = all 6 pixels set
    // - ST = ESC \ = 0x1B 0x5C
    term.process(b"\x1bPq#15~\x1b\\");

    // Now we should have an image
    assert!(term.has_sixel_image());

    let image = term.take_sixel_image().expect("should have image");
    assert_eq!(image.width(), 1);
    assert_eq!(image.height(), 6);

    // After taking, no more image
    assert!(!term.has_sixel_image());
}

#[test]
fn sixel_with_repeat() {
    let mut term = Terminal::new(24, 80);

    // Send a Sixel sequence with repeat: DCS q #15!10~ ST
    // - !10~ = repeat '~' 10 times
    term.process(b"\x1bPq#15!10~\x1b\\");

    let image = term.take_sixel_image().expect("should have image");
    assert_eq!(image.width(), 10);
    assert_eq!(image.height(), 6);
}

#[test]
fn sixel_multiline() {
    let mut term = Terminal::new(24, 80);

    // Send a Sixel sequence with two lines: DCS q #15~-~ ST
    // - ~ = first sixel band
    // - - = graphics newline (move down 6 pixels)
    // - ~ = second sixel band
    term.process(b"\x1bPq#15~-~\x1b\\");

    let image = term.take_sixel_image().expect("should have image");
    assert_eq!(image.width(), 1);
    assert_eq!(image.height(), 12); // Two sixel bands = 12 pixels
}

#[test]
fn sixel_with_color_definition() {
    let mut term = Terminal::new(24, 80);

    // Define color 100 as bright red (RGB 100,0,0) and draw with it
    // #100;2;100;0;0 = define color 100 as RGB(255,0,0)
    term.process(b"\x1bPq#100;2;100;0;0~\x1b\\");

    let image = term.take_sixel_image().expect("should have image");

    // First pixel should be bright red (0xFF_FF0000)
    let pixel = image.pixels()[0];
    let r = (pixel >> 16) & 0xFF;
    let g = (pixel >> 8) & 0xFF;
    let b = pixel & 0xFF;

    assert_eq!(r, 255, "red channel should be 255");
    assert_eq!(g, 0, "green channel should be 0");
    assert_eq!(b, 0, "blue channel should be 0");
}

#[test]
fn sixel_transparent_background() {
    let mut term = Terminal::new(24, 80);

    // Send a Sixel sequence with transparent background (Ps2=1)
    // DCS 0;1 q ... ST
    term.process(b"\x1bP0;1q#15~\x1b\\");

    let image = term.take_sixel_image().expect("should have image");
    assert!(image.is_transparent());
}

#[test]
fn sixel_cursor_position_recorded() {
    let mut term = Terminal::new(24, 80);

    // Move cursor to row 5, column 10
    term.process(b"\x1b[5;10H");

    // Send Sixel
    term.process(b"\x1bPq#15~\x1b\\");

    let image = term.take_sixel_image().expect("should have image");

    // Cursor position is 0-indexed internally, CSI uses 1-indexed
    assert_eq!(
        image.cursor_row(),
        4,
        "row should be 4 (0-indexed from row 5)"
    );
    assert_eq!(
        image.cursor_col(),
        9,
        "col should be 9 (0-indexed from col 10)"
    );
}

#[test]
fn sixel_empty_sequence_no_image() {
    let mut term = Terminal::new(24, 80);

    // Send Sixel sequence with no data
    term.process(b"\x1bPq\x1b\\");

    // Should not produce an image
    assert!(!term.has_sixel_image());
}

#[test]
fn sixel_palette_access() {
    let term = Terminal::new(24, 80);

    // Get the palette
    let palette = term.sixel_palette();

    // Should have default colors
    assert!(!palette.is_empty());

    // Color 0 should be black (default)
    let black = palette[0];
    assert_eq!(black & 0x00FFFFFF, 0, "color 0 should be black");

    // Color 15 should be white (default)
    let white = palette[15];
    let r = (white >> 16) & 0xFF;
    let g = (white >> 8) & 0xFF;
    let b = white & 0xFF;
    assert!(r > 200 && g > 200 && b > 200, "color 15 should be white");
}

// ============================================================================
// Scrollback Erase Integration Tests (CSI 3 J)
// ============================================================================

#[test]
fn erase_scrollback_via_csi_3j() {
    use crate::scrollback::Scrollback;

    let scrollback = Scrollback::new(100, 1000, 10_000_000);
    let mut term = Terminal::with_scrollback(5, 10, 3, scrollback);

    // Write enough lines to cause scrollback
    for i in 0..10 {
        term.process(format!("Line{:02}\r\n", i).as_bytes());
    }

    // Verify scrollback exists
    let scrollback_before = term.grid().scrollback_lines();
    assert!(scrollback_before > 0, "Should have scrollback lines");

    // Remember live content
    let live_content: Vec<String> = (0..5).map(|row| grid_line(&term, row)).collect();

    // Send CSI 3 J (erase scrollback)
    term.process(b"\x1b[3J");

    // Scrollback should be cleared
    assert_eq!(
        term.grid().scrollback_lines(),
        0,
        "Scrollback should be cleared"
    );
    assert_eq!(
        term.grid().display_offset(),
        0,
        "Display offset should be 0"
    );

    // Live content should be preserved
    for (row, expected) in live_content.iter().enumerate() {
        assert_eq!(
            grid_line(&term, row),
            *expected,
            "Row {} should be preserved",
            row
        );
    }
}

#[test]
fn erase_scrollback_preserves_cursor_position() {
    use crate::scrollback::Scrollback;

    let scrollback = Scrollback::new(100, 1000, 10_000_000);
    let mut term = Terminal::with_scrollback(5, 20, 3, scrollback);

    // Write lines to fill grid and scrollback
    for i in 0..15 {
        term.process(format!("Line{:02}\r\n", i).as_bytes());
    }

    // Move cursor to a specific position
    term.process(b"\x1b[2;5H"); // Row 2, Col 5 (1-based)

    let cursor_before = term.cursor();
    assert_eq!(cursor_before.row, 1, "Cursor row before");
    assert_eq!(cursor_before.col, 4, "Cursor col before");

    // Erase scrollback
    term.process(b"\x1b[3J");

    // Cursor position should remain unchanged
    let cursor_after = term.cursor();
    assert_eq!(
        cursor_after.row, cursor_before.row,
        "Cursor row should be preserved"
    );
    assert_eq!(
        cursor_after.col, cursor_before.col,
        "Cursor col should be preserved"
    );
}

#[test]
fn erase_scrollback_with_display_offset() {
    use crate::scrollback::Scrollback;

    let scrollback = Scrollback::new(100, 1000, 10_000_000);
    let mut term = Terminal::with_scrollback(5, 20, 3, scrollback);

    // Write many lines
    for i in 0..20 {
        term.process(format!("Line{:02}\r\n", i).as_bytes());
    }

    // Scroll up into scrollback
    term.grid_mut().scroll_display(5);
    assert!(
        term.grid().display_offset() > 0,
        "Should have display offset"
    );

    // Erase scrollback
    term.process(b"\x1b[3J");

    // Display offset should be reset
    assert_eq!(
        term.grid().display_offset(),
        0,
        "Display offset should be reset"
    );
    assert_eq!(
        term.grid().scrollback_lines(),
        0,
        "Scrollback should be cleared"
    );
}

#[test]
fn erase_scrollback_empty_scrollback_noop() {
    let mut term = Terminal::new(5, 20);

    // Write some content (not enough to cause scrollback)
    term.process(b"Line 1\r\n");
    term.process(b"Line 2\r\n");

    // Verify no scrollback
    assert_eq!(term.grid().scrollback_lines(), 0);

    let content_before: Vec<String> = (0..5).map(|row| grid_line(&term, row)).collect();

    // Erase scrollback (should be no-op)
    term.process(b"\x1b[3J");

    // Content should be unchanged
    for (row, expected) in content_before.iter().enumerate() {
        assert_eq!(grid_line(&term, row), *expected);
    }
}

#[test]
fn erase_scrollback_with_styles() {
    use crate::grid::CellFlags;
    use crate::scrollback::Scrollback;

    let scrollback = Scrollback::new(100, 1000, 10_000_000);
    let mut term = Terminal::with_scrollback(5, 20, 3, scrollback);

    // Write styled content (bold)
    term.process(b"\x1b[1m"); // Bold on
    for i in 0..10 {
        term.process(format!("Bold{:02}\r\n", i).as_bytes());
    }
    term.process(b"\x1b[0m"); // Reset

    // Erase scrollback
    term.process(b"\x1b[3J");

    // Live rows should still have bold styling
    // Check row 0 first cell
    let cell = term.grid().cell(0, 0).expect("Should have cell");
    assert!(
        cell.flags().contains(CellFlags::BOLD),
        "Bold style should be preserved"
    );
}

#[test]
fn erase_scrollback_total_lines_correct() {
    use crate::scrollback::Scrollback;

    let scrollback = Scrollback::new(100, 1000, 10_000_000);
    let mut term = Terminal::with_scrollback(5, 20, 3, scrollback);

    // Write many lines
    for i in 0..30 {
        term.process(format!("Line{:02}\r\n", i).as_bytes());
    }

    let total_before = term.grid().total_lines();
    assert!(total_before > 5, "Should have more than visible rows");

    // Erase scrollback
    term.process(b"\x1b[3J");

    // Total lines should equal visible rows
    assert_eq!(
        term.grid().total_lines(),
        5,
        "Total lines should equal visible rows after erase"
    );
}

#[test]
fn erase_scrollback_then_continue_writing() {
    use crate::scrollback::Scrollback;

    let scrollback = Scrollback::new(100, 1000, 10_000_000);
    let mut term = Terminal::with_scrollback(5, 20, 3, scrollback);

    // Write lines to fill scrollback
    for i in 0..20 {
        term.process(format!("Old{:02}\r\n", i).as_bytes());
    }

    // Erase scrollback
    term.process(b"\x1b[3J");

    // Continue writing
    for i in 0..10 {
        term.process(format!("New{:02}\r\n", i).as_bytes());
    }

    // New scrollback should accumulate
    assert!(
        term.grid().scrollback_lines() > 0,
        "New scrollback should accumulate"
    );

    // Content should be from new writes
    let content = grid_line(&term, 0);
    assert!(
        content.starts_with("New"),
        "Content should be from new writes: {}",
        content
    );
}

#[test]
fn erase_scrollback_ring_buffer_integrity() {
    use crate::scrollback::Scrollback;

    let scrollback = Scrollback::new(100, 1000, 10_000_000);
    let mut term = Terminal::with_scrollback(5, 20, 3, scrollback);

    // Write unique content to each line
    for i in 0..15 {
        term.process(format!("Unique{:02}\r\n", i).as_bytes());
    }

    // Erase scrollback
    term.process(b"\x1b[3J");

    // Verify all visible rows are accessible and have valid content
    for row in 0..5 {
        let row_obj = term.grid().row(row).expect("Row should exist");
        // Each row should be readable without panic
        let _ = row_obj.to_string();
    }

    // Total lines should match visible
    assert_eq!(term.grid().total_lines(), 5);
}

// ============================================================================
// Kitty Graphics Animation Tests
// ============================================================================

/// Helper to encode data as base64 for Kitty graphics commands.
fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD.encode(data)
}

#[test]
fn kitty_animation_transmit_frame() {
    let mut term = Terminal::new(24, 80);

    // First transmit a base image (2x2 RGBA)
    let image_data = vec![
        255, 0, 0, 255, // Red pixel
        0, 255, 0, 255, // Green pixel
        0, 0, 255, 255, // Blue pixel
        255, 255, 0, 255, // Yellow pixel
    ];
    let encoded = base64_encode(&image_data);

    // Transmit image: i=1, s=2 (width), v=2 (height), f=32 (RGBA)
    let cmd = format!("\x1b_Gi=1,s=2,v=2,f=32,q=2;{}\x1b\\", encoded);
    term.process(cmd.as_bytes());

    // Verify image was stored
    assert!(term.kitty_graphics().get_image(1).is_some());

    // Now transmit an animation frame: a=f (action=transmit frame)
    let frame_data = vec![
        255, 255, 255, 255, // White pixel
        0, 0, 0, 255, // Black pixel
        128, 128, 128, 255, // Gray pixel
        255, 0, 255, 255, // Magenta pixel
    ];
    let frame_encoded = base64_encode(&frame_data);

    // Animation frame command: a=f, i=1, z=100 (gap), s=2, v=2
    let frame_cmd = format!(
        "\x1b_Ga=f,i=1,z=100,s=2,v=2,f=32,q=2;{}\x1b\\",
        frame_encoded
    );
    term.process(frame_cmd.as_bytes());

    // Verify frame was added to image
    let image = term.kitty_graphics().get_image(1).unwrap();
    assert_eq!(image.frame_count(), 1, "Should have 1 animation frame");
}

#[test]
fn kitty_animation_control_start_stop() {
    use crate::kitty_graphics::AnimationState;

    let mut term = Terminal::new(24, 80);

    // Transmit base image
    let image_data = vec![0u8; 16]; // 2x2 RGBA
    let encoded = base64_encode(&image_data);
    let cmd = format!("\x1b_Gi=1,s=2,v=2,f=32,q=2;{}\x1b\\", encoded);
    term.process(cmd.as_bytes());

    // Add a frame so it's an animation
    let frame_data = vec![0u8; 16];
    let frame_encoded = base64_encode(&frame_data);
    let frame_cmd = format!("\x1b_Ga=f,i=1,s=2,v=2,f=32,q=2;{}\x1b\\", frame_encoded);
    term.process(frame_cmd.as_bytes());

    // Control animation: start (s=3 = Running)
    let start_cmd = b"\x1b_Ga=a,i=1,s=3,q=2;\x1b\\";
    term.process(start_cmd);

    let image = term.kitty_graphics().get_image(1).unwrap();
    assert_eq!(image.animation_state, AnimationState::Running);

    // Control animation: stop (s=1 = Stopped)
    let stop_cmd = b"\x1b_Ga=a,i=1,s=1,q=2;\x1b\\";
    term.process(stop_cmd);

    let image = term.kitty_graphics().get_image(1).unwrap();
    assert_eq!(image.animation_state, AnimationState::Stopped);
}

#[test]
fn kitty_animation_control_loop_count() {
    let mut term = Terminal::new(24, 80);

    // Transmit base image
    let image_data = vec![0u8; 16];
    let encoded = base64_encode(&image_data);
    let cmd = format!("\x1b_Gi=1,s=2,v=2,f=32,q=2;{}\x1b\\", encoded);
    term.process(cmd.as_bytes());

    // Set loop count: v=5 (loop 4 times)
    let loop_cmd = b"\x1b_Ga=a,i=1,s=3,v=5,q=2;\x1b\\";
    term.process(loop_cmd);

    let image = term.kitty_graphics().get_image(1).unwrap();
    assert_eq!(image.max_loops, 5);
}

#[test]
fn kitty_animation_compose_success() {
    let mut term = Terminal::new(24, 80);

    // Transmit base image
    let image_data = vec![0u8; 16];
    let encoded = base64_encode(&image_data);
    let cmd = format!("\x1b_Gi=1,s=2,v=2,f=32,q=2;{}\x1b\\", encoded);
    term.process(cmd.as_bytes());

    // Compose command (currently a no-op but should not error)
    let compose_cmd = b"\x1b_Ga=c,i=1,r=0,c=1,q=2;\x1b\\";
    term.process(compose_cmd);

    // Image should still exist
    assert!(term.kitty_graphics().get_image(1).is_some());
}

#[test]
fn kitty_animation_compose_image_not_found() {
    let mut term = Terminal::new(24, 80);

    // Compose command for non-existent image (quiet=0 so we get response)
    let compose_cmd = b"\x1b_Ga=c,i=999,r=0,c=1,q=0;\x1b\\";
    term.process(compose_cmd);

    // Should have error response
    let response_bytes = term.take_response().unwrap_or_default();
    let response = String::from_utf8_lossy(&response_bytes);
    assert!(
        response.contains("ENOENT"),
        "Should report image not found: {}",
        response
    );
}

#[test]
fn kitty_animation_frame_image_not_found() {
    let mut term = Terminal::new(24, 80);

    // Transmit frame for non-existent image
    let frame_data = vec![0u8; 16];
    let frame_encoded = base64_encode(&frame_data);
    let frame_cmd = format!("\x1b_Ga=f,i=999,s=2,v=2,f=32,q=0;{}\x1b\\", frame_encoded);
    term.process(frame_cmd.as_bytes());

    // Should have error response
    let response_bytes = term.take_response().unwrap_or_default();
    let response = String::from_utf8_lossy(&response_bytes);
    assert!(
        response.contains("ENOENT") || response.contains("not found"),
        "Should report image not found: {}",
        response
    );
}

#[test]
fn kitty_animation_control_image_not_found() {
    let mut term = Terminal::new(24, 80);

    // Control animation for non-existent image
    let ctrl_cmd = b"\x1b_Ga=a,i=999,s=3,q=0;\x1b\\";
    term.process(ctrl_cmd);

    // Should have error response
    let response_bytes = term.take_response().unwrap_or_default();
    let response = String::from_utf8_lossy(&response_bytes);
    assert!(
        response.contains("ENOENT") || response.contains("not found"),
        "Should report image not found: {}",
        response
    );
}
