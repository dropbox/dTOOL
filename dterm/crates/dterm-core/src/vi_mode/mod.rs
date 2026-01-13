//! Vi mode for terminal navigation.
//!
//! This module implements vim-style keyboard navigation for the terminal grid.
//! When enabled, users can move a cursor around the screen to select text,
//! search, and navigate without using the mouse.
//!
//! ## Usage
//!
//! ```rust
//! use dterm_core::vi_mode::{ViModeCursor, ViMotion};
//! use dterm_core::grid::Grid;
//!
//! let mut grid = Grid::new(24, 80);
//! // Position cursor at row 0, col 0
//! let cursor = ViModeCursor::new(0, 0);
//!
//! // Move right
//! let cursor = cursor.motion(&grid, ViMotion::Right);
//!
//! // Move to end of line
//! let cursor = cursor.motion(&grid, ViMotion::Last);
//! ```

use crate::grid::{Grid, Row, RowFlags};

/// Vi mode cursor position.
///
/// This is separate from the terminal's real cursor and is used only
/// for vi-style navigation and selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViModeCursor {
    /// Cursor row (can be negative for scrollback).
    pub row: i64,
    /// Cursor column.
    pub col: u16,
}

impl ViModeCursor {
    /// Create a new vi mode cursor at the given position.
    #[must_use]
    pub fn new(row: i64, col: u16) -> Self {
        Self { row, col }
    }

    /// Create a vi mode cursor at the terminal's current cursor position.
    #[must_use]
    pub fn from_terminal(grid: &Grid) -> Self {
        Self {
            row: i64::from(grid.cursor_row()),
            col: grid.cursor_col(),
        }
    }

    /// Apply a motion to the cursor, returning the new position.
    #[must_use]
    pub fn motion(self, grid: &Grid, motion: ViMotion) -> Self {
        match motion {
            ViMotion::Up => self.up(grid),
            ViMotion::Down => self.down(grid),
            ViMotion::Left => self.left(grid),
            ViMotion::Right => self.right(grid),
            ViMotion::First => self.first(grid),
            ViMotion::Last => self.last(grid),
            ViMotion::FirstOccupied => self.first_occupied(grid),
            ViMotion::High => self.high(grid),
            ViMotion::Middle => self.middle(grid),
            ViMotion::Low => self.low(grid),
            ViMotion::SemanticLeft => self.semantic(grid, Direction::Left, false),
            ViMotion::SemanticRight => self.semantic(grid, Direction::Right, false),
            ViMotion::SemanticLeftEnd => self.semantic(grid, Direction::Left, true),
            ViMotion::SemanticRightEnd => self.semantic(grid, Direction::Right, true),
            ViMotion::WordLeft => self.word(grid, Direction::Left, false),
            ViMotion::WordRight => self.word(grid, Direction::Right, false),
            ViMotion::WordLeftEnd => self.word(grid, Direction::Left, true),
            ViMotion::WordRightEnd => self.word(grid, Direction::Right, true),
            ViMotion::Bracket => self.bracket(grid),
            ViMotion::ParagraphUp => self.paragraph(grid, Direction::Left),
            ViMotion::ParagraphDown => self.paragraph(grid, Direction::Right),
        }
    }

    /// Scroll the cursor by a number of lines.
    ///
    /// Positive values move up (into scrollback), negative values move down.
    #[must_use]
    pub fn scroll(self, grid: &Grid, lines: i32) -> Self {
        let new_row = self.row + i64::from(lines);
        let clamped = self.clamp_row(grid, new_row);
        Self {
            row: clamped,
            col: self.col.min(grid.cols().saturating_sub(1)),
        }
    }

    /// Get the visible row index if the cursor is within the visible area.
    ///
    /// Returns `Some(row)` if visible, `None` if in scrollback.
    #[must_use]
    pub fn visible_row(&self, grid: &Grid) -> Option<u16> {
        // SAFETY: display_offset is bounded by MAX_SCROLLBACK_LINES (1M) which fits in i64
        #[allow(clippy::cast_possible_wrap)]
        let display_offset = grid.display_offset() as i64;
        let adjusted = self.row + display_offset;
        if adjusted >= 0 && adjusted < i64::from(grid.rows()) {
            // SAFETY: adjusted is checked to be >= 0 and < grid.rows() (u16), fits in u16
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            Some(adjusted as u16)
        } else {
            None
        }
    }

    /// Check if the cursor is in the visible area.
    #[must_use]
    pub fn is_visible(&self, grid: &Grid) -> bool {
        self.visible_row(grid).is_some()
    }

    // --- Motion implementations ---

    fn up(self, grid: &Grid) -> Self {
        let new_row = self.row - 1;
        Self {
            row: self.clamp_row(grid, new_row),
            col: self.col,
        }
    }

    fn down(self, grid: &Grid) -> Self {
        let new_row = self.row + 1;
        Self {
            row: self.clamp_row(grid, new_row),
            col: self.col,
        }
    }

    fn left(self, grid: &Grid) -> Self {
        let (new_row, new_col) = if self.col == 0 {
            // At start of line - try to go to end of previous line
            if self.row > self.topmost_row(grid) {
                (self.row - 1, grid.cols().saturating_sub(1))
            } else {
                (self.row, 0)
            }
        } else {
            (self.row, self.col - 1)
        };
        Self {
            row: new_row,
            col: new_col,
        }
    }

    fn right(self, grid: &Grid) -> Self {
        let last_col = grid.cols().saturating_sub(1);
        let (new_row, new_col) = if self.col >= last_col {
            // At end of line - try to go to start of next line
            let bottom = i64::from(grid.rows().saturating_sub(1));
            if self.row < bottom {
                (self.row + 1, 0)
            } else {
                (self.row, last_col)
            }
        } else {
            (self.row, self.col + 1)
        };
        Self {
            row: new_row,
            col: new_col,
        }
    }

    fn first(self, grid: &Grid) -> Self {
        // Go to first column, handling wrapped lines
        let mut cursor = self;

        // Walk back through wrapped lines to find the logical start
        while cursor.row > cursor.topmost_row(grid) {
            if let Some(row_data) = cursor.get_row(grid, cursor.row - 1) {
                if row_data.flags().contains(RowFlags::WRAPPED) {
                    cursor.row -= 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        cursor.col = 0;
        cursor
    }

    fn last(self, grid: &Grid) -> Self {
        // Go to last column, handling wrapped lines
        let mut cursor = self;
        let bottom = i64::from(grid.rows().saturating_sub(1));

        // Walk forward through wrapped lines to find the logical end
        while cursor.row < bottom {
            if let Some(row_data) = cursor.get_row(grid, cursor.row) {
                if row_data.flags().contains(RowFlags::WRAPPED) {
                    cursor.row += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        cursor.col = grid.cols().saturating_sub(1);
        cursor
    }

    fn first_occupied(self, grid: &Grid) -> Self {
        // Find first non-empty cell in the line
        let row_idx = self.row;
        if let Some(row_data) = self.get_row(grid, row_idx) {
            for col in 0..grid.cols() {
                if let Some(cell) = row_data.get(col) {
                    if !cell.is_empty() {
                        return Self { row: row_idx, col };
                    }
                }
            }
        }
        // Default to column 0 if all empty
        Self {
            row: row_idx,
            col: 0,
        }
    }

    fn high(self, grid: &Grid) -> Self {
        // Top of visible screen
        // SAFETY: display_offset is bounded by MAX_SCROLLBACK_LINES (1M) which fits in i64
        #[allow(clippy::cast_possible_wrap)]
        let row = -(grid.display_offset() as i64);
        Self {
            row,
            col: self.col.min(grid.cols().saturating_sub(1)),
        }
    }

    fn middle(self, grid: &Grid) -> Self {
        // Middle of visible screen
        // SAFETY: display_offset is bounded by MAX_SCROLLBACK_LINES (1M) which fits in i64
        #[allow(clippy::cast_possible_wrap)]
        let top = -(grid.display_offset() as i64);
        let middle = top + i64::from(grid.rows() / 2);
        Self {
            row: middle,
            col: self.col.min(grid.cols().saturating_sub(1)),
        }
    }

    fn low(self, grid: &Grid) -> Self {
        // Bottom of visible screen
        // SAFETY: display_offset is bounded by MAX_SCROLLBACK_LINES (1M) which fits in i64
        #[allow(clippy::cast_possible_wrap)]
        let top = -(grid.display_offset() as i64);
        let bottom = top + i64::from(grid.rows().saturating_sub(1));
        Self {
            row: bottom,
            col: self.col.min(grid.cols().saturating_sub(1)),
        }
    }

    fn semantic(self, grid: &Grid, direction: Direction, end: bool) -> Self {
        // Semantic word movement (respects word boundaries like punctuation)
        self.word_motion(grid, direction, end, true)
    }

    fn word(self, grid: &Grid, direction: Direction, end: bool) -> Self {
        // Whitespace-only word movement
        self.word_motion(grid, direction, end, false)
    }

    fn word_motion(self, grid: &Grid, direction: Direction, end: bool, semantic: bool) -> Self {
        let mut cursor = self;
        let topmost = cursor.topmost_row(grid);
        let bottommost = i64::from(grid.rows().saturating_sub(1));

        let in_bounds = |c: &ViModeCursor| c.row >= topmost && c.row <= bottommost;

        if direction == Direction::Right {
            if end {
                // 'e' or 'E' - move to end of current/next word
                // First move forward at least once
                cursor = cursor.advance(grid, direction);
                // Skip any whitespace
                while in_bounds(&cursor) && cursor.is_space(grid) {
                    cursor = cursor.advance(grid, direction);
                }
                // Move to end of word (stop when next char is space or boundary)
                while in_bounds(&cursor) {
                    let next = cursor.advance(grid, direction);
                    if next.is_space(grid) || next.is_word_boundary(grid, semantic) {
                        break;
                    }
                    cursor = next;
                }
            } else {
                // 'w' or 'W' - move to start of next word
                // 1. Skip current word characters
                while in_bounds(&cursor)
                    && !cursor.is_space(grid)
                    && !cursor.is_word_boundary(grid, semantic)
                {
                    cursor = cursor.advance(grid, direction);
                }
                // 2. If we hit a word boundary (punctuation), skip it
                while in_bounds(&cursor)
                    && cursor.is_word_boundary(grid, semantic)
                    && !cursor.is_space(grid)
                {
                    cursor = cursor.advance(grid, direction);
                }
                // 3. Skip any whitespace
                while in_bounds(&cursor) && cursor.is_space(grid) {
                    cursor = cursor.advance(grid, direction);
                }
            }
        } else {
            // Direction::Left
            if end {
                // 'ge' or 'gE' - move to end of previous word
                cursor = cursor.advance(grid, direction);
                while in_bounds(&cursor) && cursor.is_space(grid) {
                    cursor = cursor.advance(grid, direction);
                }
                // Now we're at end of previous word
            } else {
                // 'b' or 'B' - move to start of previous word
                // Move back at least once
                cursor = cursor.advance(grid, direction);
                // Skip any whitespace
                while in_bounds(&cursor) && cursor.is_space(grid) {
                    cursor = cursor.advance(grid, direction);
                }
                // Skip word boundaries (punctuation)
                while in_bounds(&cursor)
                    && cursor.is_word_boundary(grid, semantic)
                    && !cursor.is_space(grid)
                {
                    cursor = cursor.advance(grid, direction);
                }
                // Move to start of word (stop when previous char is space or boundary)
                while in_bounds(&cursor) {
                    let prev = cursor.advance(grid, direction);
                    if prev.is_space(grid)
                        || prev.is_word_boundary(grid, semantic)
                        || !in_bounds(&prev)
                    {
                        break;
                    }
                    cursor = prev;
                }
            }
        }

        cursor
    }

    fn bracket(self, grid: &Grid) -> Self {
        // Find matching bracket
        let char_at_cursor = self.char_at(grid);

        let (open, close, direction) = match char_at_cursor {
            '(' => ('(', ')', Direction::Right),
            ')' => ('(', ')', Direction::Left),
            '[' => ('[', ']', Direction::Right),
            ']' => ('[', ']', Direction::Left),
            '{' => ('{', '}', Direction::Right),
            '}' => ('{', '}', Direction::Left),
            '<' => ('<', '>', Direction::Right),
            '>' => ('<', '>', Direction::Left),
            _ => return self, // Not on a bracket
        };

        let mut cursor = self;
        let mut depth = 1;
        let topmost = cursor.topmost_row(grid);
        let bottommost = i64::from(grid.rows().saturating_sub(1));

        while depth > 0 {
            cursor = cursor.advance(grid, direction);
            if cursor.row < topmost || cursor.row > bottommost {
                return self; // Hit boundary, no match found
            }

            let ch = cursor.char_at(grid);
            if ch == open {
                if direction == Direction::Right {
                    depth += 1;
                } else {
                    depth -= 1;
                }
            } else if ch == close {
                if direction == Direction::Right {
                    depth -= 1;
                } else {
                    depth += 1;
                }
            }
        }

        cursor
    }

    fn paragraph(self, grid: &Grid, direction: Direction) -> Self {
        // Move to the start of the previous/next paragraph
        let mut cursor = self;
        let topmost = cursor.topmost_row(grid);
        let bottommost = i64::from(grid.rows().saturating_sub(1));

        // Skip current paragraph (non-empty lines)
        while cursor.row >= topmost && cursor.row <= bottommost {
            if cursor.is_line_empty(grid) {
                break;
            }
            cursor = match direction {
                Direction::Left => Self {
                    row: cursor.row - 1,
                    col: 0,
                },
                Direction::Right => Self {
                    row: cursor.row + 1,
                    col: 0,
                },
            };
        }

        // Skip empty lines
        while cursor.row >= topmost && cursor.row <= bottommost {
            if !cursor.is_line_empty(grid) {
                break;
            }
            cursor = match direction {
                Direction::Left => Self {
                    row: cursor.row - 1,
                    col: 0,
                },
                Direction::Right => Self {
                    row: cursor.row + 1,
                    col: 0,
                },
            };
        }

        Self {
            row: cursor.row.clamp(topmost, bottommost),
            col: 0,
        }
    }

    // --- Helper methods ---

    fn topmost_row(&self, grid: &Grid) -> i64 {
        // SAFETY: scrollback_lines is bounded by MAX_SCROLLBACK_LINES (1M) which fits in i64
        #[allow(clippy::cast_possible_wrap)]
        {
            -(grid.scrollback_lines() as i64)
        }
    }

    fn clamp_row(&self, grid: &Grid, row: i64) -> i64 {
        let topmost = self.topmost_row(grid);
        let bottommost = i64::from(grid.rows().saturating_sub(1));
        row.clamp(topmost, bottommost)
    }

    fn advance(self, grid: &Grid, direction: Direction) -> Self {
        match direction {
            Direction::Left => self.left(grid),
            Direction::Right => self.right(grid),
        }
    }

    /// Get the row at the given row index (handles display offset).
    fn get_row<'a>(&self, grid: &'a Grid, row: i64) -> Option<&'a Row> {
        // Convert from vi-mode row (where 0 = top visible line when display_offset = 0)
        // to grid's visible row index
        // SAFETY: display_offset is bounded by MAX_SCROLLBACK_LINES (1M) which fits in i64
        #[allow(clippy::cast_possible_wrap)]
        let display_offset = grid.display_offset() as i64;
        let adjusted = row + display_offset;
        if adjusted >= 0 && adjusted < i64::from(grid.rows()) {
            // SAFETY: adjusted is checked to be >= 0 and < grid.rows() (u16), fits in u16
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            grid.row(adjusted as u16)
        } else {
            None
        }
    }

    fn char_at(&self, grid: &Grid) -> char {
        if let Some(row) = self.get_row(grid, self.row) {
            if let Some(cell) = row.get(self.col) {
                return cell.char();
            }
        }
        ' '
    }

    fn is_space(&self, grid: &Grid) -> bool {
        let ch = self.char_at(grid);
        ch == ' ' || ch == '\t' || ch == '\0'
    }

    fn is_word_boundary(&self, grid: &Grid, semantic: bool) -> bool {
        if semantic {
            // Semantic word boundaries include punctuation
            let ch = self.char_at(grid);
            !ch.is_alphanumeric() && ch != '_'
        } else {
            // WORD boundaries are only whitespace
            self.is_space(grid)
        }
    }

    fn is_line_empty(&self, grid: &Grid) -> bool {
        if let Some(row) = self.get_row(grid, self.row) {
            row.is_empty()
        } else {
            true
        }
    }
}

/// Direction for motion commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Left,
    Right,
}

/// Vi mode motion commands.
///
/// These correspond to vim's normal mode movement commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ViMotion {
    /// Move cursor up one line (k).
    Up,
    /// Move cursor down one line (j).
    Down,
    /// Move cursor left one character (h).
    Left,
    /// Move cursor right one character (l).
    Right,
    /// Move to first column (0).
    First,
    /// Move to last column ($).
    Last,
    /// Move to first non-empty cell (^).
    FirstOccupied,
    /// Move to top of screen (H).
    High,
    /// Move to middle of screen (M).
    Middle,
    /// Move to bottom of screen (L).
    Low,
    /// Move to previous semantic word start (b).
    SemanticLeft,
    /// Move to next semantic word start (w).
    SemanticRight,
    /// Move to previous semantic word end (ge).
    SemanticLeftEnd,
    /// Move to next semantic word end (e).
    SemanticRightEnd,
    /// Move to previous whitespace-separated word start (B).
    WordLeft,
    /// Move to next whitespace-separated word start (W).
    WordRight,
    /// Move to previous whitespace-separated word end (gE).
    WordLeftEnd,
    /// Move to next whitespace-separated word end (E).
    WordRightEnd,
    /// Jump to matching bracket (%).
    Bracket,
    /// Move above current paragraph ({).
    ParagraphUp,
    /// Move below current paragraph (}).
    ParagraphDown,
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)] // Test code uses bounded loop indices
mod tests {
    use super::*;

    fn create_grid_with_content(rows: u16, cols: u16, content: &[&str]) -> Grid {
        let mut grid = Grid::new(rows, cols);
        for (row_idx, line) in content.iter().enumerate() {
            for (col_idx, ch) in line.chars().enumerate() {
                // row_idx and col_idx are bounded by rows/cols (u16) from the loop guard
                if row_idx < usize::from(rows) && col_idx < usize::from(cols) {
                    grid.set_cursor(row_idx as u16, col_idx as u16);
                    grid.write_char(ch);
                }
            }
        }
        grid.set_cursor(0, 0);
        grid
    }

    #[test]
    fn vi_cursor_new() {
        let cursor = ViModeCursor::new(5, 10);
        assert_eq!(cursor.row, 5);
        assert_eq!(cursor.col, 10);
    }

    #[test]
    fn vi_cursor_from_terminal() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::from_terminal(&grid);
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn vi_motion_up_down() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(5, 10);

        let cursor = cursor.motion(&grid, ViMotion::Up);
        assert_eq!(cursor.row, 4);
        assert_eq!(cursor.col, 10);

        let cursor = cursor.motion(&grid, ViMotion::Down);
        assert_eq!(cursor.row, 5);
        assert_eq!(cursor.col, 10);
    }

    #[test]
    fn vi_motion_up_clamped_at_top() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(0, 10);

        let cursor = cursor.motion(&grid, ViMotion::Up);
        // Should stay at row 0 (no scrollback)
        assert_eq!(cursor.row, 0);
    }

    #[test]
    fn vi_motion_down_clamped_at_bottom() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(23, 10);

        let cursor = cursor.motion(&grid, ViMotion::Down);
        assert_eq!(cursor.row, 23);
    }

    #[test]
    fn vi_motion_left_right() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(5, 10);

        let cursor = cursor.motion(&grid, ViMotion::Left);
        assert_eq!(cursor.col, 9);

        let cursor = cursor.motion(&grid, ViMotion::Right);
        assert_eq!(cursor.col, 10);
    }

    #[test]
    fn vi_motion_left_wraps_to_previous_line() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(5, 0);

        let cursor = cursor.motion(&grid, ViMotion::Left);
        assert_eq!(cursor.row, 4);
        assert_eq!(cursor.col, 79);
    }

    #[test]
    fn vi_motion_right_wraps_to_next_line() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(5, 79);

        let cursor = cursor.motion(&grid, ViMotion::Right);
        assert_eq!(cursor.row, 6);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn vi_motion_first_last() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(5, 40);

        let cursor = cursor.motion(&grid, ViMotion::First);
        assert_eq!(cursor.col, 0);

        let cursor = cursor.motion(&grid, ViMotion::Last);
        assert_eq!(cursor.col, 79);
    }

    #[test]
    fn vi_motion_high_middle_low() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(10, 40);

        let cursor = cursor.motion(&grid, ViMotion::High);
        assert_eq!(cursor.row, 0);

        let cursor = cursor.motion(&grid, ViMotion::Middle);
        assert_eq!(cursor.row, 12); // 24/2 = 12

        let cursor = cursor.motion(&grid, ViMotion::Low);
        assert_eq!(cursor.row, 23);
    }

    #[test]
    fn vi_motion_first_occupied() {
        let grid = create_grid_with_content(24, 80, &["   hello world"]);
        let cursor = ViModeCursor::new(0, 10);

        let cursor = cursor.motion(&grid, ViMotion::FirstOccupied);
        assert_eq!(cursor.col, 3); // First 'h' in "hello"
    }

    #[test]
    fn vi_motion_bracket_matching() {
        let grid = create_grid_with_content(24, 80, &["(hello)"]);
        let cursor = ViModeCursor::new(0, 0); // On '('

        let cursor = cursor.motion(&grid, ViMotion::Bracket);
        assert_eq!(cursor.col, 6); // On ')'

        // Jump back
        let cursor = cursor.motion(&grid, ViMotion::Bracket);
        assert_eq!(cursor.col, 0); // Back to '('
    }

    #[test]
    fn vi_cursor_scroll() {
        let mut grid = Grid::with_scrollback(24, 80, 100);
        // Add some scrollback by writing beyond visible area
        for i in 0..50 {
            grid.set_cursor(23, 0);
            for ch in format!("Line {i}").chars() {
                grid.write_char(ch);
            }
            grid.line_feed();
        }

        let cursor = ViModeCursor::new(0, 0);

        // Scroll up into scrollback
        let cursor = cursor.scroll(&grid, 10);
        assert_eq!(cursor.row, 10);

        // Scroll back down
        let cursor = cursor.scroll(&grid, -5);
        assert_eq!(cursor.row, 5);
    }

    #[test]
    fn vi_cursor_visible_row() {
        let grid = Grid::new(24, 80);
        let cursor = ViModeCursor::new(5, 10);

        assert!(cursor.is_visible(&grid));
        assert_eq!(cursor.visible_row(&grid), Some(5));
    }

    #[test]
    fn vi_motion_word_right() {
        let grid = create_grid_with_content(24, 80, &["hello world"]);
        let cursor = ViModeCursor::new(0, 0);

        let cursor = cursor.motion(&grid, ViMotion::SemanticRight);
        // Should be at start of "world"
        assert_eq!(cursor.col, 6);
    }

    #[test]
    fn vi_motion_word_left() {
        let grid = create_grid_with_content(24, 80, &["hello world"]);
        let cursor = ViModeCursor::new(0, 10);

        let cursor = cursor.motion(&grid, ViMotion::SemanticLeft);
        // Should be at start of "world"
        assert_eq!(cursor.col, 6);
    }

    #[test]
    fn vi_motion_paragraph_up_down() {
        let grid = create_grid_with_content(
            24,
            80,
            &[
                "line 1",
                "line 2",
                "",
                "",
                "paragraph 2 line 1",
                "paragraph 2 line 2",
            ],
        );
        let cursor = ViModeCursor::new(5, 0);

        // Move up to previous paragraph
        let cursor = cursor.motion(&grid, ViMotion::ParagraphUp);
        assert!(cursor.row <= 2); // Should be in empty lines or first paragraph

        // Move down to next paragraph
        let cursor = ViModeCursor::new(0, 0);
        let cursor = cursor.motion(&grid, ViMotion::ParagraphDown);
        assert!(cursor.row >= 4); // Should be in second paragraph
    }
}
