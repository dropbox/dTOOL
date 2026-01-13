//! Mouse-based text selection state machine.
//!
//! This module implements the text selection system per the TLA+ spec in `tla/Selection.tla`.
//!
//! Selection lifecycle:
//! - None -> InProgress (start selection on mouse down)
//! - InProgress -> Complete (finish selection on mouse up)
//! - Complete -> InProgress (extend selection with shift-click)
//! - InProgress/Complete -> None (clear selection)
//!
//! Selection types:
//! - Simple: Character-by-character selection
//! - Block: Rectangular selection (column mode)
//! - Semantic: Word/URL selection (double-click)
//! - Lines: Full line selection (triple-click)

use std::cmp::Ordering;

/// Selection state enum matching the TLA+ spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionState {
    /// No selection active.
    #[default]
    None,
    /// Selection in progress (mouse button held down).
    InProgress,
    /// Selection complete (mouse button released).
    Complete,
}

/// Selection type enum matching the TLA+ spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionType {
    /// Character-by-character selection (single click + drag).
    #[default]
    Simple,
    /// Rectangular block selection (Alt + click + drag).
    Block,
    /// Semantic selection - words, URLs, etc. (double-click).
    Semantic,
    /// Full line selection (triple-click).
    Lines,
}

/// Which side of a cell the anchor is on.
///
/// This matters for proper selection behavior at cell boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionSide {
    /// Left side of the cell (before the character).
    #[default]
    Left,
    /// Right side of the cell (after the character).
    Right,
}

/// A selection anchor point.
///
/// An anchor marks one end of a selection. It includes:
/// - Row: Can be negative for scrollback
/// - Column: 0-indexed
/// - Side: Which side of the cell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectionAnchor {
    /// Row (can be negative for scrollback).
    pub row: i32,
    /// Column (0-indexed).
    pub col: u16,
    /// Which side of the cell.
    pub side: SelectionSide,
}

impl SelectionAnchor {
    /// Create a new anchor at the given position.
    #[inline]
    pub const fn new(row: i32, col: u16, side: SelectionSide) -> Self {
        Self { row, col, side }
    }

    /// Create an anchor at the left side of a cell.
    #[inline]
    pub const fn left(row: i32, col: u16) -> Self {
        Self::new(row, col, SelectionSide::Left)
    }

    /// Create an anchor at the right side of a cell.
    #[inline]
    pub const fn right(row: i32, col: u16) -> Self {
        Self::new(row, col, SelectionSide::Right)
    }
}

impl PartialOrd for SelectionAnchor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SelectionAnchor {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.row.cmp(&other.row) {
            Ordering::Equal => match self.col.cmp(&other.col) {
                Ordering::Equal => self.side.cmp(&other.side),
                other => other,
            },
            other => other,
        }
    }
}

impl PartialOrd for SelectionSide {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SelectionSide {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (SelectionSide::Left, SelectionSide::Right) => Ordering::Less,
            (SelectionSide::Right, SelectionSide::Left) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

/// Text selection state.
///
/// This is a state machine implementing the TLA+ spec in `tla/Selection.tla`.
#[derive(Debug, Clone, Default)]
pub struct TextSelection {
    /// Current selection state.
    state: SelectionState,
    /// Selection type (only valid when state != None).
    selection_type: SelectionType,
    /// Start anchor (set on mouse down).
    start: SelectionAnchor,
    /// End anchor (updated on mouse move).
    end: SelectionAnchor,
}

impl TextSelection {
    /// Create a new empty selection.
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: SelectionState::None,
            selection_type: SelectionType::Simple,
            start: SelectionAnchor::new(0, 0, SelectionSide::Left),
            end: SelectionAnchor::new(0, 0, SelectionSide::Left),
        }
    }

    /// Get the current selection state.
    #[inline]
    pub const fn state(&self) -> SelectionState {
        self.state
    }

    /// Get the selection type.
    #[inline]
    pub const fn selection_type(&self) -> SelectionType {
        self.selection_type
    }

    /// Check if there is an active selection.
    #[inline]
    pub const fn has_selection(&self) -> bool {
        !matches!(self.state, SelectionState::None)
    }

    /// Check if selection is complete (mouse button released).
    #[inline]
    pub const fn is_complete(&self) -> bool {
        matches!(self.state, SelectionState::Complete)
    }

    /// Check if selection is in progress (mouse button held).
    #[inline]
    pub const fn is_in_progress(&self) -> bool {
        matches!(self.state, SelectionState::InProgress)
    }

    /// Check if the selection is empty (start equals end).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Get the start anchor.
    #[inline]
    pub const fn start(&self) -> SelectionAnchor {
        self.start
    }

    /// Get the end anchor.
    #[inline]
    pub const fn end(&self) -> SelectionAnchor {
        self.end
    }

    /// Get the normalized start (the anchor that comes first).
    ///
    /// For block selection, returns the top-left corner.
    #[inline]
    pub fn normalized_start(&self) -> SelectionAnchor {
        if self.selection_type == SelectionType::Block {
            // For block selection, return top-left
            SelectionAnchor::new(
                self.start.row.min(self.end.row),
                self.start.col.min(self.end.col),
                SelectionSide::Left,
            )
        } else if self.start <= self.end {
            self.start
        } else {
            self.end
        }
    }

    /// Get the normalized end (the anchor that comes last).
    ///
    /// For block selection, returns the bottom-right corner.
    #[inline]
    pub fn normalized_end(&self) -> SelectionAnchor {
        if self.selection_type == SelectionType::Block {
            // For block selection, return bottom-right
            SelectionAnchor::new(
                self.start.row.max(self.end.row),
                self.start.col.max(self.end.col),
                SelectionSide::Right,
            )
        } else if self.start <= self.end {
            self.end
        } else {
            self.start
        }
    }

    /// Start a new selection.
    ///
    /// This clears any existing selection and begins a new one at the given position.
    pub fn start_selection(
        &mut self,
        row: i32,
        col: u16,
        side: SelectionSide,
        selection_type: SelectionType,
    ) {
        self.state = SelectionState::InProgress;
        self.selection_type = selection_type;
        self.start = SelectionAnchor::new(row, col, side);
        self.end = SelectionAnchor::new(row, col, side);
    }

    /// Update the selection endpoint (during mouse drag).
    ///
    /// Only works when selection is in progress.
    pub fn update_selection(&mut self, row: i32, col: u16, side: SelectionSide) {
        if self.state == SelectionState::InProgress {
            self.end = SelectionAnchor::new(row, col, side);
        }
    }

    /// Complete the selection (mouse button released).
    pub fn complete_selection(&mut self) {
        if self.state == SelectionState::InProgress {
            self.state = SelectionState::Complete;
        }
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.state = SelectionState::None;
        // Keep anchors for debugging but they're invalid now
    }

    /// Extend an existing complete selection.
    ///
    /// This is used for shift-click to extend selection.
    pub fn extend_selection(&mut self, row: i32, col: u16, side: SelectionSide) {
        if self.state == SelectionState::Complete {
            self.end = SelectionAnchor::new(row, col, side);
            self.state = SelectionState::InProgress;
        }
    }

    /// Adjust selection for scroll.
    ///
    /// When the terminal scrolls, selection coordinates need to be updated.
    /// Returns false if selection scrolled entirely off-screen and was cleared.
    pub fn adjust_for_scroll(&mut self, delta: i32, max_rows: i32) -> bool {
        if self.state == SelectionState::None {
            return true;
        }

        let new_start_row = self.start.row - delta;
        let new_end_row = self.end.row - delta;

        // Check if selection is still visible
        let min_row = -(max_rows - 1);
        let max_row = max_rows;

        if new_start_row < min_row
            || new_start_row > max_row
            || new_end_row < min_row
            || new_end_row > max_row
        {
            // Selection scrolled off - clear it
            self.clear();
            return false;
        }

        self.start.row = new_start_row;
        self.end.row = new_end_row;
        true
    }

    /// Check if a cell is within the selection.
    ///
    /// Returns true if the cell at (row, col) is selected.
    pub fn contains(&self, row: i32, col: u16) -> bool {
        if self.state == SelectionState::None {
            return false;
        }

        let ns = self.normalized_start();
        let ne = self.normalized_end();

        match self.selection_type {
            SelectionType::Block => {
                // Rectangular selection
                row >= ns.row && row <= ne.row && col >= ns.col && col <= ne.col
            }
            SelectionType::Simple | SelectionType::Semantic | SelectionType::Lines => {
                // Linear selection
                if row < ns.row || row > ne.row {
                    return false;
                }
                if row == ns.row && col < ns.col {
                    return false;
                }
                if row == ne.row && col > ne.col {
                    return false;
                }
                true
            }
        }
    }

    /// Expand selection to semantic boundaries (for word selection).
    ///
    /// This is called after starting a Semantic selection to expand to word boundaries.
    pub fn expand_semantic(&mut self, start_col: u16, end_col: u16) {
        if self.state == SelectionState::InProgress
            && self.selection_type == SelectionType::Semantic
        {
            self.start.col = start_col;
            self.start.side = SelectionSide::Left;
            self.end.col = end_col;
            self.end.side = SelectionSide::Right;
        }
    }

    /// Expand selection to full lines.
    ///
    /// This is called for Lines selection type to select entire lines.
    pub fn expand_lines(&mut self, max_col: u16) {
        if self.state == SelectionState::InProgress && self.selection_type == SelectionType::Lines {
            self.start.col = 0;
            self.start.side = SelectionSide::Left;
            self.end.col = max_col;
            self.end.side = SelectionSide::Right;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_selection() {
        let sel = TextSelection::new();
        assert_eq!(sel.state(), SelectionState::None);
        assert!(!sel.has_selection());
    }

    #[test]
    fn test_start_and_complete_selection() {
        let mut sel = TextSelection::new();

        sel.start_selection(0, 5, SelectionSide::Left, SelectionType::Simple);
        assert_eq!(sel.state(), SelectionState::InProgress);
        assert!(sel.has_selection());
        assert!(sel.is_in_progress());

        sel.update_selection(0, 10, SelectionSide::Right);
        assert_eq!(sel.end().col, 10);

        sel.complete_selection();
        assert_eq!(sel.state(), SelectionState::Complete);
        assert!(sel.is_complete());
    }

    #[test]
    fn test_clear_selection() {
        let mut sel = TextSelection::new();
        sel.start_selection(0, 0, SelectionSide::Left, SelectionType::Simple);
        sel.complete_selection();
        assert!(sel.has_selection());

        sel.clear();
        assert!(!sel.has_selection());
        assert_eq!(sel.state(), SelectionState::None);
    }

    #[test]
    fn test_contains_simple() {
        let mut sel = TextSelection::new();
        sel.start_selection(0, 5, SelectionSide::Left, SelectionType::Simple);
        sel.update_selection(0, 10, SelectionSide::Right);
        sel.complete_selection();

        assert!(sel.contains(0, 5));
        assert!(sel.contains(0, 7));
        assert!(sel.contains(0, 10));
        assert!(!sel.contains(0, 4));
        assert!(!sel.contains(0, 11));
        assert!(!sel.contains(1, 7));
    }

    #[test]
    fn test_contains_multiline() {
        let mut sel = TextSelection::new();
        sel.start_selection(0, 5, SelectionSide::Left, SelectionType::Simple);
        sel.update_selection(2, 3, SelectionSide::Right);
        sel.complete_selection();

        // Row 0: from col 5 to end
        assert!(!sel.contains(0, 4));
        assert!(sel.contains(0, 5));
        assert!(sel.contains(0, 80)); // Full line selected after start

        // Row 1: full line
        assert!(sel.contains(1, 0));
        assert!(sel.contains(1, 80));

        // Row 2: from start to col 3
        assert!(sel.contains(2, 0));
        assert!(sel.contains(2, 3));
        assert!(!sel.contains(2, 4));
    }

    #[test]
    fn test_contains_block() {
        let mut sel = TextSelection::new();
        sel.start_selection(0, 5, SelectionSide::Left, SelectionType::Block);
        sel.update_selection(2, 10, SelectionSide::Right);
        sel.complete_selection();

        // Rectangular region: rows 0-2, cols 5-10
        assert!(sel.contains(0, 5));
        assert!(sel.contains(1, 7));
        assert!(sel.contains(2, 10));
        assert!(!sel.contains(0, 4));
        assert!(!sel.contains(0, 11));
        assert!(!sel.contains(3, 7));
    }

    #[test]
    fn test_normalized_start_end() {
        let mut sel = TextSelection::new();
        // Select backwards
        sel.start_selection(5, 10, SelectionSide::Right, SelectionType::Simple);
        sel.update_selection(2, 3, SelectionSide::Left);
        sel.complete_selection();

        let ns = sel.normalized_start();
        let ne = sel.normalized_end();

        assert_eq!(ns.row, 2);
        assert_eq!(ns.col, 3);
        assert_eq!(ne.row, 5);
        assert_eq!(ne.col, 10);
    }

    #[test]
    fn test_extend_selection() {
        let mut sel = TextSelection::new();
        sel.start_selection(0, 5, SelectionSide::Left, SelectionType::Simple);
        sel.update_selection(0, 10, SelectionSide::Right);
        sel.complete_selection();

        // Shift-click to extend
        sel.extend_selection(2, 15, SelectionSide::Right);
        assert_eq!(sel.state(), SelectionState::InProgress);
        assert_eq!(sel.end().row, 2);
        assert_eq!(sel.end().col, 15);
    }

    #[test]
    fn test_anchor_ordering() {
        let a1 = SelectionAnchor::new(0, 5, SelectionSide::Left);
        let a2 = SelectionAnchor::new(0, 5, SelectionSide::Right);
        let a3 = SelectionAnchor::new(0, 6, SelectionSide::Left);
        let a4 = SelectionAnchor::new(1, 0, SelectionSide::Left);

        assert!(a1 < a2);
        assert!(a2 < a3);
        assert!(a3 < a4);
    }
}
