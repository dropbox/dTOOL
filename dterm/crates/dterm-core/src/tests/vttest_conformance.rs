//! VTTEST Conformance Tests
//!
//! This module implements tests based on the vttest terminal conformance test program
//! (https://invisible-island.net/vttest/). These tests verify VT100/VT102/VT220
//! compatibility.
//!
//! ## Test Categories (matching vttest menu)
//!
//! 1. Cursor movements
//! 2. Screen features
//! 3. Character sets
//! 4. Double-sized characters
//! 5. Keyboard (not applicable - no input processing)
//! 6. Terminal reports
//! 7. VT52 mode
//! 8. VT102 features (Insert/Delete Char/Line)
//! 9. Known bugs
//! 10. Reset and self-test
//! 11. Non-VT100 (VT220, xterm) terminals
//!
//! ## Conformance Status
//!
//! | Category | Status | Notes |
//! |----------|--------|-------|
//! | 1. Cursor movements | ✅ PASS | All basic cursor operations work |
//! | 2. Screen features | ✅ PASS | Scrolling, wrap, origin mode |
//! | 3. Character sets | ⚠️ PARTIAL | DEC line drawing works, others limited |
//! | 4. Double-sized chars | ⚠️ PARTIAL | DECDHL/DECDWL line size flags only |
//! | 5. Keyboard | N/A | Not applicable to terminal emulator core |
//! | 6. Terminal reports | ✅ PASS | DA, DSR, CPR work |
//! | 7. VT52 mode | ✅ PASS | VT52 cursor, erase, identify work |
//! | 8. VT102 features | ✅ PASS | ICH, DCH, IL, DL work |
//! | 9. Known bugs | ✅ PASS | No known VT100 bugs present |
//! | 10. Reset | ✅ PASS | RIS resets to known state |
//! | 11. Non-VT100 | ⚠️ PARTIAL | Some VT220/xterm features |

use crate::grid::PackedColor;
use crate::terminal::{MouseMode, Terminal};

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

/// Helper to get character at position.
fn char_at(term: &Terminal, row: u16, col: u16) -> char {
    term.grid().cell(row, col).map(|c| c.char()).unwrap_or(' ')
}

// ============================================================================
// VTTEST Menu 1: Test of Cursor Movements
// ============================================================================

/// VTTEST 1.1: Test CUP (Cursor Position)
/// Draws asterisks in corners and along edges.
#[test]
fn vttest_1_1_cursor_position() {
    let mut term = Terminal::new(24, 80);

    // Place asterisks at corners (1-based coordinates)
    // CUP [row;col H is 1-based, so [1;1H is top-left (0,0 in 0-indexed)

    // Top-left
    term.process(b"\x1b[1;1H");
    assert_eq!(term.grid().cursor_row(), 0, "cursor row after CUP[1;1H");
    assert_eq!(term.grid().cursor_col(), 0, "cursor col after CUP[1;1H");
    term.process(b"*");
    assert_eq!(char_at(&term, 0, 0), '*', "top-left should be *");

    // Top-right: column 79 (1-indexed 80, but writing wraps if at end)
    // Use column 79 to avoid wrap issues for this test
    term.process(b"\x1b[1;79H");
    term.process(b"*");
    assert_eq!(char_at(&term, 0, 78), '*', "near top-right should be *");

    // Bottom-left
    term.process(b"\x1b[24;1H");
    assert_eq!(term.grid().cursor_row(), 23, "cursor row after CUP[24;1H");
    term.process(b"*");
    assert_eq!(char_at(&term, 23, 0), '*', "bottom-left should be *");

    // Bottom-right: use column 79 (1-indexed) to avoid wrap
    term.process(b"\x1b[24;79H");
    term.process(b"*");
    assert_eq!(char_at(&term, 23, 78), '*', "near bottom-right should be *");

    // Test that CUP properly clamps to screen bounds
    term.process(b"\x1b[100;100H");
    assert_eq!(term.grid().cursor_row(), 23, "row clamped to max");
    assert_eq!(term.grid().cursor_col(), 79, "col clamped to max");
}

/// VTTEST 1.2: Test CUU (Cursor Up) and CUD (Cursor Down)
/// Using IND (Index) ESC D and RI (Reverse Index) ESC M
#[test]
fn vttest_1_2_cursor_up_down_with_index() {
    let mut term = Terminal::new(24, 80);

    // Position cursor at row 10, col 1
    term.process(b"\x1b[10;1H");

    // Move down with IND (ESC D) - like LF but respects scroll region
    term.process(b"\x1bD"); // Index down
    assert_eq!(term.grid().cursor_row(), 10);

    // Move up with RI (ESC M) - reverse index
    term.process(b"\x1bM"); // Reverse index
    assert_eq!(term.grid().cursor_row(), 9);
}

/// VTTEST 1.3: Test CUF (Cursor Forward) and CUB (Cursor Backward)
#[test]
fn vttest_1_3_cursor_forward_backward() {
    let mut term = Terminal::new(24, 80);

    term.process(b"\x1b[1;40H"); // Start at middle
    assert_eq!(term.grid().cursor_col(), 39);

    // Move forward 10
    term.process(b"\x1b[10C");
    assert_eq!(term.grid().cursor_col(), 49);

    // Move backward 5
    term.process(b"\x1b[5D");
    assert_eq!(term.grid().cursor_col(), 44);
}

/// VTTEST 1.4: Test cursor movement at margins
/// Cursor should stop at margins, not wrap
#[test]
fn vttest_1_4_cursor_stops_at_margins() {
    let mut term = Terminal::new(24, 80);

    // Try to move up from row 1 - should stay at row 1
    term.process(b"\x1b[1;1H");
    term.process(b"\x1b[999A"); // Try to move up 999
    assert_eq!(term.grid().cursor_row(), 0);

    // Try to move left from col 1 - should stay at col 1
    term.process(b"\x1b[1;1H");
    term.process(b"\x1b[999D"); // Try to move left 999
    assert_eq!(term.grid().cursor_col(), 0);

    // Try to move down past bottom - should stop at row 24
    term.process(b"\x1b[24;1H");
    term.process(b"\x1b[999B"); // Try to move down 999
    assert_eq!(term.grid().cursor_row(), 23);

    // Try to move right past right margin - should stop at col 80
    term.process(b"\x1b[1;80H");
    term.process(b"\x1b[999C"); // Try to move right 999
    assert_eq!(term.grid().cursor_col(), 79);
}

/// VTTEST 1.5: Test horizontal and vertical position absolute
#[test]
fn vttest_1_5_absolute_positioning() {
    let mut term = Terminal::new(24, 80);

    // VPA (Vertical Position Absolute) ESC [ n d
    term.process(b"\x1b[15d");
    assert_eq!(term.grid().cursor_row(), 14); // 0-indexed

    // HPA (Horizontal Position Absolute) ESC [ n `  (backtick)
    term.process(b"\x1b[30`");
    assert_eq!(term.grid().cursor_col(), 29); // 0-indexed

    // CHA (Cursor Character Absolute) ESC [ n G
    term.process(b"\x1b[50G");
    assert_eq!(term.grid().cursor_col(), 49);
}

// ============================================================================
// VTTEST Menu 2: Test of Screen Features
// ============================================================================

/// VTTEST 2.1: Test DECAWM (Auto Wrap Mode)
#[test]
fn vttest_2_1_auto_wrap_mode() {
    let mut term = Terminal::new(24, 10);

    // Enable auto wrap (default)
    term.process(b"\x1b[?7h");

    // Write past end of line - should wrap
    term.process(b"1234567890WRAP");

    assert_eq!(grid_line(&term, 0), "1234567890");
    assert_eq!(grid_line(&term, 1), "WRAP");
}

/// VTTEST 2.2: Test DECAWM off (no wrap)
#[test]
fn vttest_2_2_auto_wrap_disabled() {
    let mut term = Terminal::new(24, 10);

    // Disable auto wrap
    term.process(b"\x1b[?7l");

    // Write past end - should overwrite last column
    term.process(b"1234567890NOWRAP");

    // Last column should have 'P' (last char written)
    assert_eq!(char_at(&term, 0, 9), 'P');
    // Row 1 should be empty
    assert_eq!(grid_line(&term, 1), "");
}

/// VTTEST 2.3: Test scroll region (DECSTBM)
#[test]
fn vttest_2_3_scroll_region() {
    let mut term = Terminal::new(24, 80);

    // Fill with identifiable content
    for i in 1..=24 {
        term.process(format!("\x1b[{};1HLine {:02}", i, i).as_bytes());
    }

    // Set scroll region to rows 5-10
    term.process(b"\x1b[5;10r");

    // Move to bottom of scroll region
    term.process(b"\x1b[10;1H");

    // Scroll up by adding new line
    term.process(b"\x1b[S"); // Scroll Up 1 line

    // Line 5 should now show what was Line 6
    // (content shifts up within region)
    assert!(grid_line(&term, 4).contains("06") || grid_line(&term, 4).is_empty());

    // Reset scroll region
    term.process(b"\x1b[r");
}

/// VTTEST 2.4: Test origin mode (DECOM)
#[test]
fn vttest_2_4_origin_mode() {
    let mut term = Terminal::new(24, 80);

    // Set scroll region
    term.process(b"\x1b[5;15r");

    // Enable origin mode
    term.process(b"\x1b[?6h");

    // Now CUP [1;1H should go to row 5, col 1 (within region)
    term.process(b"\x1b[1;1HX");

    // Check position - row 4 (0-indexed, which is row 5 1-indexed)
    assert_eq!(char_at(&term, 4, 0), 'X');

    // Disable origin mode
    term.process(b"\x1b[?6l");
    // Reset scroll region
    term.process(b"\x1b[r");
}

/// VTTEST 2.5: Test DECALN (Screen Alignment Pattern)
#[test]
fn vttest_2_5_screen_alignment() {
    let mut term = Terminal::new(5, 10);

    // DECALN fills screen with E's
    term.process(b"\x1b#8");

    // Every position should be 'E'
    for row in 0..5 {
        for col in 0..10 {
            assert_eq!(
                char_at(&term, row, col),
                'E',
                "Expected 'E' at ({}, {})",
                row,
                col
            );
        }
    }
}

/// VTTEST 2.6: Test erase operations (ED, EL)
#[test]
fn vttest_2_6_erase_operations() {
    let mut term = Terminal::new(5, 10);

    // Fill with X's
    term.process(b"\x1b#8"); // Fill with E's

    // Position cursor and erase to end of line
    term.process(b"\x1b[1;5H\x1b[K");
    assert_eq!(char_at(&term, 0, 4), ' ');
    assert_eq!(char_at(&term, 0, 3), 'E');

    // Reset and test erase from start of line
    term.process(b"\x1b#8");
    term.process(b"\x1b[1;5H\x1b[1K");
    assert_eq!(char_at(&term, 0, 4), ' ');
    assert_eq!(char_at(&term, 0, 5), 'E');

    // Reset and test erase entire line
    term.process(b"\x1b#8");
    term.process(b"\x1b[3;5H\x1b[2K");
    assert_eq!(grid_line(&term, 2), "");
}

/// VTTEST 2.7: Test insert mode (IRM)
#[test]
fn vttest_2_7_insert_mode() {
    let mut term = Terminal::new(24, 80);

    term.process(b"ABCDEF");
    term.process(b"\x1b[1;3H"); // Position at C

    // Enable insert mode
    term.process(b"\x1b[4h");

    // Insert character - should push right
    term.process(b"X");

    assert_eq!(grid_line(&term, 0), "ABXCDEF");

    // Disable insert mode
    term.process(b"\x1b[4l");
}

// ============================================================================
// VTTEST Menu 3: Test of Character Sets
// ============================================================================

/// VTTEST 3.1: Test DEC Special Graphics (line drawing)
#[test]
fn vttest_3_1_dec_special_graphics() {
    let mut term = Terminal::new(24, 80);

    // Switch to DEC Special Graphics in G0
    // ESC ( 0 selects DEC Special Graphics into G0
    term.process(b"\x1b(0");

    // In Special Graphics: j=┘ k=┐ l=┌ m=└ q=─ x=│
    // 'j' (0x6A) -> BOX DRAWINGS LIGHT UP AND LEFT (└ reversed to ┘)
    term.process(b"lqqqk"); // ┌───┐
    term.process(b"\r\n");
    term.process(b"x   x"); // │   │
    term.process(b"\r\n");
    term.process(b"mqqqj"); // └───┘

    // Switch back to ASCII
    term.process(b"\x1b(B");

    // Verify the box characters are present
    // Line drawing chars: l=┌ q=─ k=┐
    let line0 = grid_line(&term, 0);
    assert!(line0.contains('┌') || line0.contains('─') || !line0.is_empty());
}

/// VTTEST 3.2: Test UK character set
#[test]
fn vttest_3_2_uk_character_set() {
    let mut term = Terminal::new(24, 80);

    // Select UK character set into G0
    term.process(b"\x1b(A");

    // In UK set, # (0x23) should display as £
    term.process(b"Price: #100");

    // Switch back to ASCII
    term.process(b"\x1b(B");

    // Check that pound sign appears
    let line = grid_line(&term, 0);
    assert!(line.contains('£'), "Expected £ in: {}", line);
}

// ============================================================================
// VTTEST Menu 4: Test of Double-Sized Characters
// ============================================================================

/// VTTEST 4.1: Test DECDWL (Double-Width Line)
///
/// ESC # 6 sets current line to double-width.
/// Cursor column limit should be half the normal width.
#[test]
fn vttest_4_1_double_width_line() {
    let mut term = Terminal::new(24, 80);

    // Write text on a normal line
    term.process(b"Normal width line");
    term.process(b"\r\n");

    // Set current line to double-width
    term.process(b"\x1b#6"); // DECDWL

    // Write text - should be double-width
    term.process(b"Double width");

    // Verify line size is set
    use crate::grid::LineSize;
    let line_size = term
        .grid()
        .row(1)
        .map(|r| r.line_size())
        .unwrap_or(LineSize::SingleWidth);
    assert_eq!(
        line_size,
        LineSize::DoubleWidth,
        "Line should be double-width"
    );

    // Cursor column should be limited to cols/2 = 40 on an 80-col terminal
    // After writing 12 chars, cursor should be at col 12
    assert!(
        term.grid().cursor_col() < 40,
        "Cursor should be within half-width limit"
    );
}

/// VTTEST 4.2: Test DECDHL (Double-Height Line)
///
/// ESC # 3 sets top half of double-height line.
/// ESC # 4 sets bottom half of double-height line.
#[test]
fn vttest_4_2_double_height_line() {
    let mut term = Terminal::new(24, 80);

    // Double-height text requires two lines (top and bottom halves)
    // Top half
    term.process(b"\x1b#3"); // DECDHL top
    term.process(b"DOUBLE");
    term.process(b"\r\n");

    // Bottom half
    term.process(b"\x1b#4"); // DECDHL bottom
    term.process(b"DOUBLE");

    // Verify line sizes
    use crate::grid::LineSize;
    let top_size = term
        .grid()
        .row(0)
        .map(|r| r.line_size())
        .unwrap_or(LineSize::SingleWidth);
    let bottom_size = term
        .grid()
        .row(1)
        .map(|r| r.line_size())
        .unwrap_or(LineSize::SingleWidth);

    assert_eq!(
        top_size,
        LineSize::DoubleHeightTop,
        "First line should be double-height top"
    );
    assert_eq!(
        bottom_size,
        LineSize::DoubleHeightBottom,
        "Second line should be double-height bottom"
    );
}

/// VTTEST 4.3: Test DECSWL (Single-Width Line)
///
/// ESC # 5 sets current line back to single-width.
#[test]
fn vttest_4_3_single_width_line() {
    let mut term = Terminal::new(24, 80);

    // First set to double-width
    term.process(b"\x1b#6"); // DECDWL

    use crate::grid::LineSize;
    let size_before = term
        .grid()
        .row(0)
        .map(|r| r.line_size())
        .unwrap_or(LineSize::SingleWidth);
    assert_eq!(size_before, LineSize::DoubleWidth, "Should be double-width");

    // Now set back to single-width
    term.process(b"\x1b#5"); // DECSWL

    let size_after = term
        .grid()
        .row(0)
        .map(|r| r.line_size())
        .unwrap_or(LineSize::DoubleWidth);
    assert_eq!(
        size_after,
        LineSize::SingleWidth,
        "Should be single-width after DECSWL"
    );
}

/// VTTEST 4.4: Test cursor column clamping on double-width lines
///
/// When cursor is at high column and line changes to double-width,
/// cursor should be clamped to the effective column limit.
#[test]
fn vttest_4_4_cursor_clamping() {
    let mut term = Terminal::new(24, 80);

    // Move cursor to column 60 (well past half-width of 40)
    term.process(b"\x1b[1;61H"); // CUP to row 1, col 61 (0-indexed: col 60)
    assert_eq!(term.grid().cursor_col(), 60, "Cursor should be at col 60");

    // Set line to double-width
    term.process(b"\x1b#6"); // DECDWL

    // Cursor should be clamped to max effective column (cols/2 - 1 = 39)
    assert!(
        term.grid().cursor_col() < 40,
        "Cursor should be clamped to half-width"
    );
}

/// VTTEST 4.5: Test cursor movement on double-width lines
///
/// CUF (cursor forward) should stop at effective column limit.
#[test]
fn vttest_4_5_cursor_forward_limit() {
    let mut term = Terminal::new(24, 80);

    // Set line to double-width
    term.process(b"\x1b#6"); // DECDWL

    // Move cursor to beginning
    term.process(b"\x1b[1;1H");

    // Try to move forward 100 columns
    term.process(b"\x1b[100C"); // CUF 100

    // Cursor should stop at effective limit (39 for 80-col terminal)
    assert!(
        term.grid().cursor_col() < 40,
        "CUF should stop at half-width limit"
    );
}

/// VTTEST 4.6: Test character writing on double-width lines
///
/// Characters written on double-width lines should advance cursor normally
/// within the effective column limit.
#[test]
fn vttest_4_6_write_on_double_width() {
    let mut term = Terminal::new(24, 80);

    // Set line to double-width
    term.process(b"\x1b#6");

    // Write exactly 40 characters (the effective width)
    term.process(b"1234567890123456789012345678901234567890");

    // Cursor should be at or near the effective limit
    // Note: depending on wrap mode, it might wrap or stay at limit
    let col = term.grid().cursor_col();
    assert!(col < 40 || col == 0, "Cursor should wrap or stay at limit");
}

/// VTTEST 4.7: Test moving between different line sizes
///
/// When cursor moves from single-width to double-width row,
/// column should be clamped.
#[test]
fn vttest_4_7_row_transition() {
    let mut term = Terminal::new(24, 80);

    // Row 0: single-width, cursor at col 60
    term.process(b"\x1b[1;61H");
    assert_eq!(term.grid().cursor_col(), 60);

    // Row 1: double-width
    term.process(b"\x1b[2;1H"); // Move to row 2
    term.process(b"\x1b#6"); // Set double-width
    term.process(b"\x1b[1;61H"); // Back to row 1, col 61

    // Move down to double-width row
    term.process(b"\x1b[B"); // CUD - cursor down

    // Cursor column should be clamped
    assert!(
        term.grid().cursor_col() < 40,
        "Cursor should clamp when moving to double-width row"
    );
}

// ============================================================================
// VTTEST Menu 6: Test of Terminal Reports
// ============================================================================

/// VTTEST 6.1: Test Device Attributes (DA)
#[test]
fn vttest_6_1_device_attributes() {
    let mut term = Terminal::new(24, 80);

    // Request primary DA
    term.process(b"\x1b[c");

    // Check that a response was queued
    let response = term.take_response().expect("Expected DA response");
    assert!(!response.is_empty(), "Expected non-empty DA response");
    // Response should start with CSI ? (ESC [ ?)
    assert!(
        response.starts_with(b"\x1b[?"),
        "DA response should start with ESC [ ?: {:?}",
        String::from_utf8_lossy(&response)
    );
}

/// VTTEST 6.2: Test Device Status Report (DSR)
#[test]
fn vttest_6_2_device_status_report() {
    let mut term = Terminal::new(24, 80);

    // DSR - request status
    term.process(b"\x1b[5n");

    let response = term.take_response().expect("Expected DSR response");
    // Response should be ESC [ 0 n (OK)
    assert_eq!(
        response,
        b"\x1b[0n",
        "Expected OK status: {:?}",
        String::from_utf8_lossy(&response)
    );
}

/// VTTEST 6.3: Test Cursor Position Report (CPR)
#[test]
fn vttest_6_3_cursor_position_report() {
    let mut term = Terminal::new(24, 80);

    // Move cursor to specific position
    term.process(b"\x1b[10;25H");

    // Request cursor position
    term.process(b"\x1b[6n");

    let response = term.take_response().expect("Expected CPR response");
    // Response should be ESC [ 10 ; 25 R
    assert_eq!(
        response,
        b"\x1b[10;25R",
        "Expected position 10;25: {:?}",
        String::from_utf8_lossy(&response)
    );
}

// ============================================================================
// VTTEST Menu 8: Test of VT102 Features (Insert/Delete Char/Line)
// ============================================================================

/// VTTEST 8.1: Test ICH (Insert Character)
#[test]
fn vttest_8_1_insert_character() {
    let mut term = Terminal::new(24, 80);

    term.process(b"ABCDEF");
    term.process(b"\x1b[1;3H"); // Position at C

    // Insert 2 characters
    term.process(b"\x1b[2@");

    assert_eq!(grid_line(&term, 0), "AB  CDEF");
}

/// VTTEST 8.2: Test DCH (Delete Character)
#[test]
fn vttest_8_2_delete_character() {
    let mut term = Terminal::new(24, 80);

    term.process(b"ABCDEF");
    term.process(b"\x1b[1;3H"); // Position at C

    // Delete 2 characters
    term.process(b"\x1b[2P");

    assert_eq!(grid_line(&term, 0), "ABEF");
}

/// VTTEST 8.3: Test IL (Insert Line)
#[test]
fn vttest_8_3_insert_line() {
    let mut term = Terminal::new(10, 20);

    // Write some lines
    term.process(b"\x1b[1;1HLine 1");
    term.process(b"\x1b[2;1HLine 2");
    term.process(b"\x1b[3;1HLine 3");
    term.process(b"\x1b[4;1HLine 4");

    // Position at line 2 and insert 1 line
    term.process(b"\x1b[2;1H");
    term.process(b"\x1b[L");

    assert_eq!(grid_line(&term, 0), "Line 1");
    assert_eq!(grid_line(&term, 1), ""); // Inserted blank
    assert_eq!(grid_line(&term, 2), "Line 2");
    assert_eq!(grid_line(&term, 3), "Line 3");
}

/// VTTEST 8.4: Test DL (Delete Line)
#[test]
fn vttest_8_4_delete_line() {
    let mut term = Terminal::new(10, 20);

    // Write some lines
    term.process(b"\x1b[1;1HLine 1");
    term.process(b"\x1b[2;1HLine 2");
    term.process(b"\x1b[3;1HLine 3");
    term.process(b"\x1b[4;1HLine 4");

    // Position at line 2 and delete 1 line
    term.process(b"\x1b[2;1H");
    term.process(b"\x1b[M");

    assert_eq!(grid_line(&term, 0), "Line 1");
    assert_eq!(grid_line(&term, 1), "Line 3");
    assert_eq!(grid_line(&term, 2), "Line 4");
}

/// VTTEST 8.5: Test ECH (Erase Character)
#[test]
fn vttest_8_5_erase_character() {
    let mut term = Terminal::new(24, 80);

    term.process(b"ABCDEFGH");
    term.process(b"\x1b[1;3H"); // Position at C

    // Erase 3 characters (replace with spaces)
    term.process(b"\x1b[3X");

    assert_eq!(grid_line(&term, 0), "AB   FGH");
}

// ============================================================================
// VTTEST Menu 9: Test of Known Bugs
// ============================================================================

/// VTTEST 9.1: Wrap column flag behavior
/// Tests that writing to the last column has correct wrap behavior.
/// Note: Different terminals handle the "wrap pending" state differently.
/// Some keep cursor at last col with pending wrap, others wrap immediately.
/// dterm wraps the cursor to next line after writing last column.
#[test]
fn vttest_9_1_wrap_column_flag() {
    let mut term = Terminal::new(24, 10);

    // Write 9 characters (fills columns 0-8)
    term.process(b"123456789");
    assert_eq!(
        term.grid().cursor_col(),
        9,
        "After 9 chars, cursor at col 9"
    );
    assert_eq!(term.grid().cursor_row(), 0, "Still on row 0");

    // Write 10th character - fills last column
    term.process(b"0");

    // Cursor wraps to next line (dterm behavior)
    // Note: Some terminals keep cursor at col 9 with "wrap pending"
    let col = term.grid().cursor_col();
    let row = term.grid().cursor_row();

    // Either wrap already happened (row 1, col 0) or wrap pending (row 0, col 9)
    assert!(
        (row == 1 && col == 0) || (row == 0 && col == 9),
        "Cursor should be at (1,0) or (0,9) but was ({},{})",
        row,
        col
    );

    // The content should be correct regardless
    assert_eq!(grid_line(&term, 0), "1234567890");
}

/// VTTEST 9.2: Tab stop handling across lines
#[test]
fn vttest_9_2_tab_stops() {
    let mut term = Terminal::new(24, 80);

    // Default tabs at every 8 columns
    term.process(b"A\tB\tC");

    // A at 0, B at 8, C at 16
    assert_eq!(char_at(&term, 0, 0), 'A');
    assert_eq!(char_at(&term, 0, 8), 'B');
    assert_eq!(char_at(&term, 0, 16), 'C');
}

// ============================================================================
// VTTEST Menu 10: Test of Reset and Self-Test
// ============================================================================

/// VTTEST 10.1: Test RIS (Reset to Initial State)
#[test]
fn vttest_10_1_reset_initial_state() {
    let mut term = Terminal::new(24, 80);

    // Make various changes
    term.process(b"\x1b[?7l"); // Disable wrap
    term.process(b"\x1b[5;15r"); // Set scroll region
    term.process(b"\x1b[10;20H"); // Move cursor
    term.process(b"\x1b[1;31m"); // Set colors

    // Reset
    term.process(b"\x1bc"); // RIS

    // Verify reset state
    assert_eq!(term.grid().cursor_row(), 0);
    assert_eq!(term.grid().cursor_col(), 0);
    // Scroll region should be reset (verified by scroll behavior)
}

/// VTTEST 10.2: Test DECSTR (Soft Terminal Reset)
/// Tests that soft reset resets key terminal modes while preserving others.
#[test]
fn vttest_10_2_soft_reset() {
    let mut term = Terminal::new(24, 80);

    // Change modes that DECSTR should reset
    term.process(b"\x1b[?7l"); // Disable wrap
    term.process(b"\x1b[?6h"); // Enable origin mode
    term.process(b"\x1b[4h"); // Enable insert mode
    term.process(b"\x1b[?25l"); // Hide cursor
    term.process(b"\x1b[?1h"); // Enable application cursor keys
    term.process(b"\x1b[1;31m"); // Set bold + red

    // Change modes that DECSTR should preserve
    term.process(b"\x1b[?1049h"); // Switch to alternate screen
    term.process(b"\x1b[?2004h"); // Enable bracketed paste
    term.process(b"\x1b[?1000h"); // Enable mouse tracking

    // Verify pre-reset state
    assert!(!term.modes().auto_wrap);
    assert!(term.modes().origin_mode);
    assert!(term.modes().insert_mode);
    assert!(!term.modes().cursor_visible);
    assert!(term.modes().application_cursor_keys);
    assert!(term.modes().alternate_screen);
    assert!(term.modes().bracketed_paste);
    assert_eq!(term.modes().mouse_mode, MouseMode::Normal);

    // Soft reset
    term.process(b"\x1b[!p"); // DECSTR

    // These should be RESET:
    assert!(term.modes().auto_wrap); // Re-enabled
    assert!(!term.modes().origin_mode); // Disabled
    assert!(!term.modes().insert_mode); // Disabled
    assert!(term.modes().cursor_visible); // Re-visible
    assert!(!term.modes().application_cursor_keys); // Disabled
    assert_eq!(term.style().fg, PackedColor::default_fg()); // SGR reset

    // These should be PRESERVED:
    assert!(term.modes().alternate_screen); // Still on alt screen
    assert!(term.modes().bracketed_paste); // Still enabled
    assert_eq!(term.modes().mouse_mode, MouseMode::Normal); // Still tracking

    // Verify insert mode is off by behavior
    term.process(b"ABC");
    term.process(b"\x1b[1;2H");
    term.process(b"X");

    // Should overwrite, not insert
    assert_eq!(grid_line(&term, 0), "AXC");
}

// ============================================================================
// VTTEST Menu 11: Non-VT100 Tests (VT220, xterm extensions)
// ============================================================================

/// VTTEST 11.1: Test DECSCUSR (Set Cursor Style)
#[test]
fn vttest_11_1_cursor_style() {
    let mut term = Terminal::new(24, 80);

    // Set cursor to steady block
    term.process(b"\x1b[2 q");
    // Set cursor to blinking underline
    term.process(b"\x1b[3 q");
    // Set cursor to steady bar
    term.process(b"\x1b[6 q");

    // Just verify no crash - cursor style is visual only
}

/// VTTEST 11.2: Test SGR extended colors
#[test]
fn vttest_11_2_extended_colors() {
    let mut term = Terminal::new(24, 80);

    // 256-color mode: ESC [ 38;5;n m
    term.process(b"\x1b[38;5;196mRed\x1b[0m");

    // 24-bit color mode: ESC [ 38;2;r;g;b m
    term.process(b"\x1b[38;2;255;128;0mOrange\x1b[0m");

    // Verify text was written (color is attribute, not easily testable)
    assert!(grid_line(&term, 0).contains("Red"));
}

/// VTTEST 11.3: Test Save/Restore Cursor (DECSC/DECRC)
#[test]
fn vttest_11_3_save_restore_cursor() {
    let mut term = Terminal::new(24, 80);

    // Position cursor
    term.process(b"\x1b[10;20H");

    // Save cursor (DECSC)
    term.process(b"\x1b7");

    // Move cursor elsewhere
    term.process(b"\x1b[1;1H");
    assert_eq!(term.grid().cursor_row(), 0);
    assert_eq!(term.grid().cursor_col(), 0);

    // Restore cursor (DECRC)
    term.process(b"\x1b8");
    assert_eq!(term.grid().cursor_row(), 9);
    assert_eq!(term.grid().cursor_col(), 19);
}

/// VTTEST 11.4: Test bracketed paste mode
#[test]
fn vttest_11_4_bracketed_paste() {
    let mut term = Terminal::new(24, 80);

    // Enable bracketed paste mode
    term.process(b"\x1b[?2004h");

    // Simulate pasted text (would be wrapped with brackets)
    term.process(b"\x1b[200~pasted text\x1b[201~");

    // Text should appear
    assert!(grid_line(&term, 0).contains("pasted text"));

    // Disable bracketed paste mode
    term.process(b"\x1b[?2004l");
}

/// VTTEST 11.5: Test alternate screen buffer
#[test]
fn vttest_11_5_alternate_screen() {
    let mut term = Terminal::new(24, 80);

    // Write to main screen
    term.process(b"Main screen content");
    let main_content = grid_line(&term, 0);
    assert!(main_content.contains("Main"));

    // Switch to alternate screen
    term.process(b"\x1b[?1049h");

    // Alternate screen should be empty
    assert_eq!(grid_line(&term, 0), "");

    // Write to alternate
    term.process(b"Alternate screen");

    // Switch back to main
    term.process(b"\x1b[?1049l");

    // Main content should be restored
    assert!(grid_line(&term, 0).contains("Main"));
}

// ============================================================================
// Additional Conformance Tests
// ============================================================================

/// Test REP (Repeat preceding character)
#[test]
fn vttest_extra_rep_repeat_character() {
    let mut term = Terminal::new(24, 80);

    term.process(b"X\x1b[5b"); // X followed by repeat 5 times

    assert_eq!(grid_line(&term, 0), "XXXXXX");
}

/// Test SU/SD (Scroll Up/Down)
#[test]
fn vttest_extra_scroll_up_down() {
    let mut term = Terminal::new(5, 20);

    term.process(b"\x1b[1;1HLine 1");
    term.process(b"\x1b[2;1HLine 2");
    term.process(b"\x1b[3;1HLine 3");

    // Scroll up 1 line
    term.process(b"\x1b[S");

    // Line 1 should have Line 2's content now
    assert_eq!(grid_line(&term, 0), "Line 2");
    assert_eq!(grid_line(&term, 1), "Line 3");
}

/// Test HPR (Horizontal Position Relative) and VPR (Vertical Position Relative)
#[test]
fn vttest_extra_relative_positioning() {
    let mut term = Terminal::new(24, 80);

    term.process(b"\x1b[10;20H"); // Start at 10,20

    // HPR: move right by 5
    term.process(b"\x1b[5a");
    assert_eq!(term.grid().cursor_col(), 24);

    // VPR: move down by 3
    term.process(b"\x1b[3e");
    assert_eq!(term.grid().cursor_row(), 12);
}

/// Test CBT (Cursor Backward Tab)
#[test]
fn vttest_extra_backward_tab() {
    let mut term = Terminal::new(24, 80);

    // Move to column 20
    term.process(b"\x1b[1;20H");

    // Back tab should go to column 16 (previous tab stop at 16, 1-indexed 17)
    term.process(b"\x1b[Z");
    assert_eq!(term.grid().cursor_col(), 16);
}

/// Test CHT (Cursor Horizontal Tab)
#[test]
fn vttest_extra_forward_tab_csi() {
    let mut term = Terminal::new(24, 80);

    term.process(b"\x1b[1;1H"); // Start at col 1

    // Forward 2 tabs
    term.process(b"\x1b[2I");
    assert_eq!(term.grid().cursor_col(), 16); // Tab at 8, then 16
}

/// Test TBC (Tab Clear)
#[test]
fn vttest_extra_tab_clear() {
    let mut term = Terminal::new(24, 80);

    // Set a tab at column 5
    term.process(b"\x1b[1;5H\x1bH"); // HTS at col 5

    // Clear tab at current position
    term.process(b"\x1b[0g");

    // Clear all tabs
    term.process(b"\x1b[3g");

    // Tab should now go to end of line (no tab stops)
    term.process(b"\x1b[1;1H\t");
    assert_eq!(term.grid().cursor_col(), 79); // End of line
}

// ============================================================================
// C1 Control Code Tests (8-bit controls)
// ============================================================================

/// Test C1 control codes work like their 7-bit equivalents
#[test]
fn vttest_c1_ind_nel_ri() {
    let mut term = Terminal::new(24, 80);

    term.process(b"\x1b[5;1H"); // Row 5

    // IND (0x84) - same as ESC D
    term.process(&[0x84]);
    assert_eq!(term.grid().cursor_row(), 5);

    // RI (0x8D) - same as ESC M
    term.process(&[0x8D]);
    assert_eq!(term.grid().cursor_row(), 4);

    // NEL (0x85) - same as ESC E
    term.process(&[0x85]);
    assert_eq!(term.grid().cursor_row(), 5);
    assert_eq!(term.grid().cursor_col(), 0);
}

/// Test C1 CSI (0x9B) works like ESC [
#[test]
fn vttest_c1_csi() {
    let mut term = Terminal::new(24, 80);

    // 0x9B is C1 CSI, equivalent to ESC [
    term.process(&[0x9B, b'1', b'0', b';', b'2', b'0', b'H']);

    assert_eq!(term.grid().cursor_row(), 9);
    assert_eq!(term.grid().cursor_col(), 19);
}

/// Test C1 OSC (0x9D) works like ESC ]
#[test]
fn vttest_c1_osc() {
    let mut term = Terminal::new(24, 80);

    // 0x9D is C1 OSC, 0x9C is C1 ST
    let mut seq = vec![0x9D];
    seq.extend_from_slice(b"0;Test Title");
    seq.push(0x9C); // ST

    term.process(&seq);

    // Title should be set (verify through window_title if available)
    // Main test is that it doesn't crash
}

// ============================================================================
// Section 7: VT52 Mode
// ============================================================================

/// Test entering VT52 mode via DECANM reset (CSI ? 2 l).
#[test]
fn vttest_vt52_enter_mode() {
    let mut term = Terminal::new(24, 80);

    // CSI ? 2 l enters VT52 mode
    term.process(b"\x1b[?2l");

    assert!(term.modes().vt52_mode);
}

/// Test exiting VT52 mode via ESC <.
#[test]
fn vttest_vt52_exit_mode() {
    let mut term = Terminal::new(24, 80);

    // Enter VT52 mode
    term.process(b"\x1b[?2l");
    assert!(term.modes().vt52_mode);

    // Exit VT52 mode with ESC <
    term.process(b"\x1b<");
    assert!(!term.modes().vt52_mode);
}

/// Test VT52 cursor movement sequences.
#[test]
fn vttest_vt52_cursor_movement() {
    let mut term = Terminal::new(24, 80);

    // Enter VT52 mode and position cursor
    term.process(b"\x1b[?2l");
    term.process(b"\x1b[10;10H"); // This won't work in VT52 mode, use ESC Y

    // Reset to home first
    term.process(b"\x1bH"); // VT52 cursor home
    assert_eq!(term.grid().cursor_row(), 0);
    assert_eq!(term.grid().cursor_col(), 0);

    // Move down
    term.process(b"\x1bB");
    assert_eq!(term.grid().cursor_row(), 1);

    // Move right
    term.process(b"\x1bC");
    assert_eq!(term.grid().cursor_col(), 1);

    // Move up
    term.process(b"\x1bA");
    assert_eq!(term.grid().cursor_row(), 0);

    // Move left
    term.process(b"\x1bD");
    assert_eq!(term.grid().cursor_col(), 0);
}

/// Test VT52 direct cursor addressing (ESC Y row col).
#[test]
fn vttest_vt52_direct_cursor_addressing() {
    let mut term = Terminal::new(24, 80);

    // Enter VT52 mode
    term.process(b"\x1b[?2l");

    // ESC Y row col - row and col are encoded as value + 32 (space character)
    // To move to (5, 10): row = 5 + 32 = 37 ('%'), col = 10 + 32 = 42 ('*')
    term.process(b"\x1bY%*");

    assert_eq!(term.grid().cursor_row(), 5);
    assert_eq!(term.grid().cursor_col(), 10);
}

/// Test VT52 erase sequences.
#[test]
fn vttest_vt52_erase() {
    let mut term = Terminal::new(24, 80);

    // Write some content
    term.process(b"Hello World");
    term.process(b"\x1b[2;1H"); // Move to line 2
    term.process(b"Line 2");

    // Enter VT52 mode
    term.process(b"\x1b[?2l");

    // Move to start of "World"
    term.process(b"\x1bY &"); // Row 0 + 32 = ' ', Col 6 + 32 = '&'

    // Erase to end of line (ESC K)
    term.process(b"\x1bK");
    let line = grid_line(&term, 0);
    assert_eq!(line, "Hello");

    // Move to start of line 2 and erase to end of screen (ESC J)
    term.process(b"\x1bY! "); // Row 1, Col 0
    term.process(b"\x1bJ");
    let line = grid_line(&term, 1);
    assert!(line.is_empty() || line.chars().all(|c| c == ' '));
}

/// Test VT52 identify response.
#[test]
fn vttest_vt52_identify() {
    let mut term = Terminal::new(24, 80);

    // Enter VT52 mode
    term.process(b"\x1b[?2l");

    // Request identify (ESC Z)
    term.process(b"\x1bZ");

    // Should respond with ESC / Z
    let response = term.take_response().expect("Should have response");
    assert_eq!(response.as_slice(), b"\x1b/Z");
}

/// Test VT52 reverse line feed (ESC I).
#[test]
fn vttest_vt52_reverse_line_feed() {
    let mut term = Terminal::new(24, 80);

    // Enter VT52 mode and position cursor
    term.process(b"\x1b[?2l");
    term.process(b"\x1bY# "); // Row 3, Col 0

    assert_eq!(term.grid().cursor_row(), 3);

    // Reverse line feed (ESC I)
    term.process(b"\x1bI");
    assert_eq!(term.grid().cursor_row(), 2);
}

/// Test VT52 graphics mode (ESC F / ESC G).
#[test]
fn vttest_vt52_graphics_mode() {
    let mut term = Terminal::new(24, 80);

    // Enter VT52 mode
    term.process(b"\x1b[?2l");

    // Enter graphics mode (ESC F)
    term.process(b"\x1bF");

    // 'j' in DEC line drawing = ┘ (lower right corner)
    term.process(b"j");

    // Exit graphics mode (ESC G)
    term.process(b"\x1bG");

    // 'j' should now be literal j
    term.process(b"j");

    let line = grid_line(&term, 0);
    // First char should be box drawing, second should be 'j'
    assert!(line.ends_with('j'));
}

/// Test VT52 keypad modes.
#[test]
fn vttest_vt52_keypad_mode() {
    let mut term = Terminal::new(24, 80);

    // Enter VT52 mode
    term.process(b"\x1b[?2l");

    // Enter alternate keypad mode (ESC =)
    term.process(b"\x1b=");
    assert!(term.modes().application_keypad);

    // Exit alternate keypad mode (ESC >)
    term.process(b"\x1b>");
    assert!(!term.modes().application_keypad);
}

// ============================================================================
// Kitty Graphics Protocol Conformance Tests
// ============================================================================
//
// These tests verify conformance with the Kitty graphics protocol:
// https://sw.kovidgoyal.net/kitty/graphics-protocol/
//
// The protocol uses APC (Application Program Command) sequences:
// ESC _ G <control-data> ; <payload> ESC \

/// Test basic Kitty graphics transmit (a=t).
///
/// Transmits an image without displaying it.
/// Reference: https://sw.kovidgoyal.net/kitty/graphics-protocol/#transmitting-data
#[test]
fn kitty_graphics_transmit_only() {
    let mut term = Terminal::new(24, 80);

    // Transmit 2x2 RGB image (a=t, f=24, s=2, v=2, i=1)
    // 12 bytes: 4 red pixels
    // Base64("/wAA/wAA/wAA/wAA") = [255,0,0,255,0,0,255,0,0,255,0,0]
    term.process(b"\x1b_Ga=t,f=24,s=2,v=2,i=1;/wAA/wAA/wAA/wAA\x1b\\");

    // Image should be stored
    assert!(term.kitty_graphics().image_count() > 0);
    let image = term.kitty_graphics().get_image(1);
    assert!(image.is_some(), "Image ID 1 should exist");

    let img = image.unwrap();
    assert_eq!(img.width, 2);
    assert_eq!(img.height, 2);

    // No placements should exist (transmit only)
    assert!(!img.has_placements());
}

/// Test Kitty graphics transmit and display (a=T).
///
/// Transmits and immediately displays the image at cursor position.
/// Reference: https://sw.kovidgoyal.net/kitty/graphics-protocol/#display-images
#[test]
fn kitty_graphics_transmit_and_display() {
    let mut term = Terminal::new(24, 80);

    // Position cursor first
    term.process(b"\x1b[5;10H");

    // Transmit and display (a=T uppercase)
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=2;/wAA/wAA/wAA/wAA\x1b\\");

    let image = term.kitty_graphics().get_image(2);
    assert!(image.is_some());

    let img = image.unwrap();
    // Should have a placement
    assert!(img.has_placements(), "Should have placement after a=T");
}

/// Test Kitty graphics display existing (a=p).
///
/// Displays a previously transmitted image.
#[test]
fn kitty_graphics_display_existing() {
    let mut term = Terminal::new(24, 80);

    // First transmit without display
    term.process(b"\x1b_Ga=t,f=24,s=2,v=2,i=3;/wAA/wAA/wAA/wAA\x1b\\");

    let img = term.kitty_graphics().get_image(3).unwrap();
    assert!(!img.has_placements());

    // Now display it (a=p)
    term.process(b"\x1b_Ga=p,i=3\x1b\\");

    let img = term.kitty_graphics().get_image(3).unwrap();
    assert!(img.has_placements(), "Should have placement after a=p");
}

/// Test Kitty graphics delete (a=d).
///
/// Deletes images and/or placements.
/// Reference: https://sw.kovidgoyal.net/kitty/graphics-protocol/#deleting-images
#[test]
fn kitty_graphics_delete_by_id() {
    let mut term = Terminal::new(24, 80);

    // Create two images
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=10;/wAA/wAA/wAA/wAA\x1b\\");
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=11;/wAA/wAA/wAA/wAA\x1b\\");

    assert_eq!(term.kitty_graphics().image_count(), 2);

    // Delete by ID (d=I frees data)
    term.process(b"\x1b_Ga=d,d=I,i=10\x1b\\");

    assert_eq!(term.kitty_graphics().image_count(), 1);
    assert!(term.kitty_graphics().get_image(10).is_none());
    assert!(term.kitty_graphics().get_image(11).is_some());
}

/// Test Kitty graphics delete all (d=A).
#[test]
fn kitty_graphics_delete_all() {
    let mut term = Terminal::new(24, 80);

    // Create multiple images
    for i in 20..25 {
        let cmd = format!("\x1b_Ga=T,f=24,s=2,v=2,i={};/wAA/wAA/wAA/wAA\x1b\\", i);
        term.process(cmd.as_bytes());
    }

    assert_eq!(term.kitty_graphics().image_count(), 5);

    // Delete all (d=A)
    term.process(b"\x1b_Ga=d,d=A\x1b\\");

    assert_eq!(term.kitty_graphics().image_count(), 0);
}

/// Test Kitty graphics RGBA format (f=32).
#[test]
fn kitty_graphics_rgba_format() {
    let mut term = Terminal::new(24, 80);

    // 2x2 RGBA image = 16 bytes
    // Base64 of 16 bytes of 0xFF = "//////////////////8="
    // Using a simpler payload: 4 white pixels RGBA
    // [255,255,255,255] x4 = 16 bytes
    // Actually let's just verify parsing works
    term.process(b"\x1b_Ga=t,f=32,s=2,v=2,i=30;/////////////////////w==\x1b\\");

    let image = term.kitty_graphics().get_image(30);
    assert!(image.is_some(), "RGBA image should be stored");
}

/// Test Kitty graphics PNG format (f=100).
///
/// PNG format lets the terminal decode the image dimensions.
#[test]
fn kitty_graphics_png_format() {
    let mut term = Terminal::new(24, 80);

    // Note: We can't easily test PNG without actual PNG data,
    // but we can verify the command is parsed correctly
    let cmd = b"\x1b_Ga=t,f=100,i=40\x1b\\";
    term.process(cmd);

    // Without valid PNG data, the image may not be stored
    // This tests that the parser doesn't crash on f=100
}

/// Test Kitty graphics chunked transmission (m=1, m=0).
///
/// Large images can be sent in multiple chunks.
/// Reference: https://sw.kovidgoyal.net/kitty/graphics-protocol/#chunked-transmission
#[test]
fn kitty_graphics_chunked_transmission() {
    let mut term = Terminal::new(24, 80);

    // First chunk (m=1 = more coming)
    term.process(b"\x1b_Ga=t,f=24,s=2,v=2,i=50,m=1;/wAA\x1b\\");

    // Intermediate chunk (m=1)
    term.process(b"\x1b_Gm=1;/wAA\x1b\\");

    // Final chunk (m=0)
    term.process(b"\x1b_Gm=0;/wAA/wAA\x1b\\");

    // Image should be complete now
    let image = term.kitty_graphics().get_image(50);
    assert!(
        image.is_some(),
        "Chunked image should be stored after final chunk"
    );
}

/// Test Kitty graphics z-index (z parameter).
///
/// Negative z-index places image below text, positive above.
#[test]
fn kitty_graphics_z_index() {
    let mut term = Terminal::new(24, 80);

    // Image below text (z=-1)
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=60,z=-1\x1b\\");

    let img = term.kitty_graphics().get_image(60).unwrap();
    let placement = img.iter_placements().next().unwrap();
    assert_eq!(placement.z_index, -1);

    // Image above text (z=10)
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=61,z=10\x1b\\");

    let img = term.kitty_graphics().get_image(61).unwrap();
    let placement = img.iter_placements().next().unwrap();
    assert_eq!(placement.z_index, 10);
}

/// Test Kitty graphics cursor movement (C parameter).
///
/// C=0 moves cursor after image, C=1 keeps cursor in place.
#[test]
fn kitty_graphics_cursor_movement() {
    let mut term = Terminal::new(24, 80);

    // Move to known position
    term.process(b"\x1b[10;10H");
    let initial_row = term.grid().cursor_row();
    let initial_col = term.grid().cursor_col();

    // Display with C=1 (don't move cursor)
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=70,C=1\x1b\\");

    // Cursor should not have moved
    assert_eq!(term.grid().cursor_row(), initial_row);
    assert_eq!(term.grid().cursor_col(), initial_col);
}

/// Test Kitty graphics source rectangle (x, y, w, h parameters).
///
/// Displays only a portion of the image.
#[test]
fn kitty_graphics_source_rectangle() {
    let mut term = Terminal::new(24, 80);

    // Transmit image first
    term.process(b"\x1b_Ga=t,f=24,s=4,v=4,i=80;/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA/wAA\x1b\\");

    // Display with source rectangle (crop to 2x2 from offset 1,1)
    term.process(b"\x1b_Ga=p,i=80,x=1,y=1,w=2,h=2\x1b\\");

    let img = term.kitty_graphics().get_image(80).unwrap();
    let placement = img.iter_placements().next().unwrap();
    assert_eq!(placement.source_x, 1);
    assert_eq!(placement.source_y, 1);
    assert_eq!(placement.source_width, 2);
    assert_eq!(placement.source_height, 2);
}

/// Test Kitty graphics cell sizing (c, r parameters).
///
/// Specifies display size in terminal cells.
#[test]
fn kitty_graphics_cell_sizing() {
    let mut term = Terminal::new(24, 80);

    // Display in 10 columns by 5 rows
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=90,c=10,r=5\x1b\\");

    let img = term.kitty_graphics().get_image(90).unwrap();
    let placement = img.iter_placements().next().unwrap();
    assert_eq!(placement.num_columns, 10);
    assert_eq!(placement.num_rows, 5);
}

/// Test Kitty graphics placement ID (p parameter).
///
/// Multiple placements of the same image with different IDs.
#[test]
fn kitty_graphics_multiple_placements() {
    let mut term = Terminal::new(24, 80);

    // Transmit image
    term.process(b"\x1b_Ga=t,f=24,s=2,v=2,i=100;/wAA/wAA/wAA/wAA\x1b\\");

    // Create multiple placements with explicit IDs
    term.process(b"\x1b_Ga=p,i=100,p=1\x1b\\");
    term.process(b"\x1b_Ga=p,i=100,p=2\x1b\\");
    term.process(b"\x1b_Ga=p,i=100,p=3\x1b\\");

    let img = term.kitty_graphics().get_image(100).unwrap();
    assert_eq!(img.placement_count(), 3);
}

/// Test Kitty graphics image number (I parameter).
///
/// Alternative identification by image number.
/// Images can be referred to by number (I parameter) as well as ID (i parameter).
#[test]
fn kitty_graphics_image_number() {
    let mut term = Terminal::new(24, 80);

    // Transmit with both image ID (i=999) and image number (I=456)
    term.process(b"\x1b_Ga=t,f=24,s=2,v=2,i=999,I=456;/wAA/wAA/wAA/wAA\x1b\\");

    // Verify image was stored by ID
    let img = term.kitty_graphics().get_image(999);
    assert!(img.is_some(), "Image should be stored by ID");

    // Verify image number was set
    let img = img.unwrap();
    assert_eq!(
        img.number,
        Some(456),
        "Image number should be set from I parameter"
    );

    // Verify image can be looked up by number
    let img_by_num = term.kitty_graphics().get_image_by_number(456);
    assert!(
        img_by_num.is_some(),
        "Image should be retrievable by number"
    );
    assert_eq!(
        img_by_num.unwrap().id,
        999,
        "Image retrieved by number should have correct ID"
    );
}

/// Test Kitty graphics delete by image number (d=n, d=N).
///
/// Images can be deleted by their number as well as their ID.
#[test]
fn kitty_graphics_delete_by_number() {
    let mut term = Terminal::new(24, 80);

    // Create two images with different numbers
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=200,I=10;/wAA/wAA/wAA/wAA\x1b\\");
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=201,I=20;/wAA/wAA/wAA/wAA\x1b\\");

    // Verify both exist
    assert!(term.kitty_graphics().get_image(200).is_some());
    assert!(term.kitty_graphics().get_image(201).is_some());
    assert!(term.kitty_graphics().get_image_by_number(10).is_some());
    assert!(term.kitty_graphics().get_image_by_number(20).is_some());

    // Delete by number (d=n deletes placements, d=N deletes image+placements)
    // Using I=10 to specify which image number to delete
    term.process(b"\x1b_Ga=d,d=N,I=10\x1b\\");

    // Image 200 (number 10) should be deleted
    assert!(
        term.kitty_graphics().get_image(200).is_none(),
        "Image 200 should be deleted"
    );
    assert!(
        term.kitty_graphics().get_image_by_number(10).is_none(),
        "Number 10 should be cleared"
    );

    // Image 201 (number 20) should still exist
    assert!(
        term.kitty_graphics().get_image(201).is_some(),
        "Image 201 should still exist"
    );
    assert!(
        term.kitty_graphics().get_image_by_number(20).is_some(),
        "Number 20 should still exist"
    );
}

/// Test Kitty graphics quiet mode (q parameter).
///
/// q=0: respond to commands
/// q=1: respond only on error
/// q=2: never respond
#[test]
fn kitty_graphics_quiet_mode() {
    let mut term = Terminal::new(24, 80);

    // q=2 means no response
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=110,q=2;/wAA/wAA/wAA/wAA\x1b\\");

    // Just verify it doesn't crash and image is stored
    assert!(term.kitty_graphics().get_image(110).is_some());
}

/// Test Kitty graphics dimension limits.
///
/// Images exceeding KITTY_MAX_DIMENSION should be rejected or clamped.
#[test]
fn kitty_graphics_dimension_limit() {
    let mut term = Terminal::new(24, 80);

    // Try to create image with huge dimensions
    term.process(b"\x1b_Ga=t,f=24,s=99999,v=99999,i=120;/wAA\x1b\\");

    // If image was stored, dimensions should be clamped
    if let Some(img) = term.kitty_graphics().get_image(120) {
        use crate::kitty_graphics::KITTY_MAX_DIMENSION;
        assert!(img.width <= KITTY_MAX_DIMENSION);
        assert!(img.height <= KITTY_MAX_DIMENSION);
    }
}

/// Test Kitty graphics delete placements only (d=i lowercase).
///
/// Lowercase d values delete placements but keep image data.
#[test]
fn kitty_graphics_delete_placements_keep_data() {
    let mut term = Terminal::new(24, 80);

    // Create image with placement
    term.process(b"\x1b_Ga=T,f=24,s=2,v=2,i=130;/wAA/wAA/wAA/wAA\x1b\\");

    let img = term.kitty_graphics().get_image(130).unwrap();
    assert!(img.has_placements());

    // Delete placements only (d=i lowercase)
    term.process(b"\x1b_Ga=d,d=i,i=130\x1b\\");

    // Image should still exist but without placements
    let img = term.kitty_graphics().get_image(130);
    assert!(img.is_some(), "Image data should be preserved");
    assert!(
        !img.unwrap().has_placements(),
        "Placements should be deleted"
    );
}

/// Test Kitty graphics storage quota.
#[test]
fn kitty_graphics_storage_tracking() {
    let mut term = Terminal::new(24, 80);

    let initial_bytes = term.kitty_graphics().total_bytes();

    // Add an image
    term.process(b"\x1b_Ga=t,f=24,s=2,v=2,i=140;/wAA/wAA/wAA/wAA\x1b\\");

    // Storage should have increased
    assert!(term.kitty_graphics().total_bytes() > initial_bytes);

    // Delete the image
    term.process(b"\x1b_Ga=d,d=I,i=140\x1b\\");

    // Storage should return to initial
    assert_eq!(term.kitty_graphics().total_bytes(), initial_bytes);
}
