//! Grid fuzz target.
//!
//! This fuzzer tests the terminal grid with arbitrary operations.
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run grid -- -max_total_time=60
//! ```
//!
//! ## Properties Tested
//!
//! - Grid never panics on any operation sequence
//! - Cursor is always in bounds
//! - Dimensions are always valid (positive)
//! - Resize operations maintain invariants
//!
//! ## Correspondence to TLA+
//!
//! This fuzzer validates the TypeInvariant and Safety properties
//! from tla/Grid.tla through exhaustive random testing.

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use dterm_core::grid::Grid;
use libfuzzer_sys::fuzz_target;

/// Operations that can be performed on the grid.
#[derive(Debug, Arbitrary)]
enum GridOp {
    /// Create a new grid with given dimensions
    New { rows: u16, cols: u16 },
    /// Resize to new dimensions
    Resize { rows: u16, cols: u16 },
    /// Move cursor to position
    MoveCursor { row: u16, col: u16 },
    /// Move cursor by relative offset
    MoveCursorBy { dr: i8, dc: i8 },
    /// Write a character
    WriteChar { c: u8 },
    /// Write a character with wrap
    WriteCharWrap { c: u8 },
    /// Line feed
    LineFeed,
    /// Carriage return
    CarriageReturn,
    /// Tab
    Tab,
    /// Backspace
    Backspace,
    /// Save cursor
    SaveCursor,
    /// Restore cursor
    RestoreCursor,
    /// Erase to end of line
    EraseToEndOfLine,
    /// Erase to end of screen
    EraseToEndOfScreen,
    /// Erase line
    EraseLine,
    /// Erase screen
    EraseScreen,
    /// Scroll display
    ScrollDisplay { delta: i8 },
}

fuzz_target!(|data: &[u8]| {
    // Early return for empty/tiny inputs to avoid infinite loops
    if data.len() < 4 {
        return;
    }

    let mut unstructured = Unstructured::new(data);

    // Get initial dimensions (clamped to reasonable values)
    let init_rows: u16 = unstructured.int_in_range(1..=100).unwrap_or(24);
    let init_cols: u16 = unstructured.int_in_range(1..=200).unwrap_or(80);

    let mut grid = Grid::new(init_rows, init_cols);

    // Verify initial state
    assert!(grid.rows() > 0, "rows must be positive");
    assert!(grid.cols() > 0, "cols must be positive");
    assert!(grid.cursor_row() < grid.rows(), "cursor row out of bounds");
    assert!(grid.cursor_col() < grid.cols(), "cursor col out of bounds");

    // Process operations
    while let Ok(op) = unstructured.arbitrary::<GridOp>() {
        match op {
            GridOp::New { rows, cols } => {
                let rows = rows.max(1).min(100);
                let cols = cols.max(1).min(200);
                grid = Grid::new(rows, cols);
            }
            GridOp::Resize { rows, cols } => {
                let rows = rows.max(1).min(100);
                let cols = cols.max(1).min(200);
                grid.resize(rows, cols);
            }
            GridOp::MoveCursor { row, col } => {
                grid.move_cursor_to(row, col);
            }
            GridOp::MoveCursorBy { dr, dc } => {
                grid.move_cursor_by(dr as i32, dc as i32);
            }
            GridOp::WriteChar { c } => {
                if c >= 0x20 && c < 0x7F {
                    grid.write_char(c as char);
                }
            }
            GridOp::WriteCharWrap { c } => {
                if c >= 0x20 && c < 0x7F {
                    grid.write_char_wrap(c as char);
                }
            }
            GridOp::LineFeed => {
                grid.line_feed();
            }
            GridOp::CarriageReturn => {
                grid.carriage_return();
            }
            GridOp::Tab => {
                grid.tab();
            }
            GridOp::Backspace => {
                grid.backspace();
            }
            GridOp::SaveCursor => {
                grid.save_cursor();
            }
            GridOp::RestoreCursor => {
                grid.restore_cursor();
            }
            GridOp::EraseToEndOfLine => {
                grid.erase_to_end_of_line();
            }
            GridOp::EraseToEndOfScreen => {
                grid.erase_to_end_of_screen();
            }
            GridOp::EraseLine => {
                grid.erase_line();
            }
            GridOp::EraseScreen => {
                grid.erase_screen();
            }
            GridOp::ScrollDisplay { delta } => {
                grid.scroll_display(delta as i32);
            }
        }

        // Invariants that must hold after every operation
        assert!(
            grid.rows() > 0 && grid.rows() <= 100,
            "Invalid rows: {}",
            grid.rows()
        );
        assert!(
            grid.cols() > 0 && grid.cols() <= 200,
            "Invalid cols: {}",
            grid.cols()
        );
        assert!(
            grid.cursor_row() < grid.rows(),
            "Cursor row {} out of bounds (rows={})",
            grid.cursor_row(),
            grid.rows()
        );
        assert!(
            grid.cursor_col() < grid.cols(),
            "Cursor col {} out of bounds (cols={})",
            grid.cursor_col(),
            grid.cols()
        );
    }
});
