//! Grid adapter helpers for Alacritty-style integrations.

use std::cmp::min;
use std::ops::{Deref, Index, IndexMut};

use dterm_core::grid::Grid as CoreGrid;

pub use dterm_core::grid::{
    Cell, CellCoord, CellExtra, CellExtras, CellFlags, Cursor, Damage, LineDamageBounds, LineSize,
    PackedColor, PackedColors, Row, RowDamageBounds, RowFlags,
};

// Re-export scrollback Line type for history access
pub use dterm_core::scrollback::Line as ScrollbackLine;

use crate::index::{Column, Line, Point};

// Re-export Dimensions from index module where it's defined
pub use crate::index::Dimensions;

// ============================================================================
// Alacritty-style Damage Types
// ============================================================================

/// Terminal damage state for rendering.
///
/// This mirrors Alacritty's `TermDamage` enum for compatibility with rendering code.
#[derive(Debug, Clone)]
pub enum TermDamage<'a> {
    /// Full terminal redraw required.
    Full,
    /// Partial damage with per-line bounds.
    Partial(TermDamageIterator<'a>),
}

impl<'a> TermDamage<'a> {
    /// Create terminal damage from a grid's damage state.
    pub fn from_grid(grid: &'a Grid) -> Self {
        let damage = grid.damage();
        if damage.is_full() {
            TermDamage::Full
        } else {
            TermDamage::Partial(TermDamageIterator::new(grid))
        }
    }
}

/// Iterator over damaged lines with column bounds.
///
/// This provides line-by-line damage information for efficient partial redraws.
#[derive(Debug, Clone)]
pub struct TermDamageIterator<'a> {
    grid: &'a Grid,
    current_row: u16,
    rows: u16,
    cols: u16,
}

impl<'a> TermDamageIterator<'a> {
    /// Create a new damage iterator for a grid.
    fn new(grid: &'a Grid) -> Self {
        Self {
            grid,
            current_row: 0,
            rows: grid.rows(),
            cols: grid.cols(),
        }
    }
}

impl Iterator for TermDamageIterator<'_> {
    type Item = LineDamageBounds;

    fn next(&mut self) -> Option<Self::Item> {
        let damage = self.grid.damage();

        while self.current_row < self.rows {
            let row = self.current_row;
            self.current_row += 1;

            if damage.is_row_damaged(row) {
                // Get column bounds for this row
                if let Some((left, right)) = damage.row_damage_bounds(row, self.cols) {
                    return Some(LineDamageBounds::new(row, left, right));
                } else {
                    // Row is damaged but no bounds available - assume full row
                    return Some(LineDamageBounds::new(row, 0, self.cols));
                }
            }
        }

        None
    }
}

/// Extension trait for getting Alacritty-style damage from a grid.
pub trait GridDamageExt {
    /// Get terminal damage state for rendering.
    fn term_damage(&self) -> TermDamage<'_>;
}

impl GridDamageExt for Grid {
    fn term_damage(&self) -> TermDamage<'_> {
        TermDamage::from_grid(self)
    }
}

/// Alacritty-style scroll requests.
#[derive(Debug, Copy, Clone)]
pub enum Scroll {
    /// Scroll by a delta in lines.
    Delta(i32),
    /// Scroll up by one page.
    PageUp,
    /// Scroll down by one page.
    PageDown,
    /// Jump to the top of scrollback.
    Top,
    /// Jump to the bottom (live view).
    Bottom,
}

/// Terminal grid type backed by dterm-core.
pub type Grid = CoreGrid;

impl Dimensions for Grid {
    fn total_lines(&self) -> usize {
        CoreGrid::total_lines(self)
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.rows())
    }

    fn columns(&self) -> usize {
        usize::from(self.cols())
    }
}

impl Dimensions for (usize, usize) {
    fn total_lines(&self) -> usize {
        self.0
    }

    fn screen_lines(&self) -> usize {
        self.0
    }

    fn columns(&self) -> usize {
        self.1
    }
}

/// Convenience methods matching Alacritty's grid API.
pub trait GridExt {
    /// Column count.
    fn columns(&self) -> usize;
    /// Visible screen lines.
    fn screen_lines(&self) -> usize;
    /// Total lines in buffer.
    fn total_lines(&self) -> usize;
    /// Display offset in scrollback lines.
    fn display_offset(&self) -> usize;
    /// Apply an Alacritty-style scroll request.
    fn scroll_display(&mut self, scroll: Scroll);
    /// Current damage state.
    fn damage(&self) -> &Damage;
    /// Clear the visible viewport.
    ///
    /// This resets all cells in the visible area to the default state.
    fn clear_viewport(&mut self);
    /// Initialize all cells in the grid.
    ///
    /// Used for ref-tests to ensure deterministic state.
    fn initialize_all(&mut self);
    /// Truncate the grid for serialization.
    ///
    /// Used for ref-tests to prepare grid for saving.
    fn truncate(&mut self);
    /// Get a mutable reference to the cell at the cursor position.
    fn cursor_cell(&mut self) -> Option<&mut Cell>;
}

impl GridExt for Grid {
    fn columns(&self) -> usize {
        usize::from(self.cols())
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.rows())
    }

    fn total_lines(&self) -> usize {
        CoreGrid::total_lines(self)
    }

    fn display_offset(&self) -> usize {
        CoreGrid::display_offset(self)
    }

    fn scroll_display(&mut self, scroll: Scroll) {
        apply_scroll(self, scroll);
    }

    fn damage(&self) -> &Damage {
        CoreGrid::damage(self)
    }

    fn clear_viewport(&mut self) {
        let rows = self.rows();
        for row in 0..rows {
            if let Some(r) = self.row_mut(row) {
                r.clear();
            }
        }
        // Reset cursor to home position
        self.set_cursor(0, 0);
        // Mark full damage
        self.damage_mut().mark_full();
        // Clear extras (hyperlinks, combining chars, etc.)
        self.extras_mut().clear();
        // Clear styles
        self.styles_mut().clear();
    }

    fn initialize_all(&mut self) {
        // Initialize all cells to default state
        let rows = self.rows();
        for row in 0..rows {
            if let Some(r) = self.row_mut(row) {
                r.clear();
            }
        }
        // Reset cursor
        self.set_cursor(0, 0);
        // Clear all damage
        self.clear_damage();
        // Clear extras
        self.extras_mut().clear();
    }

    fn truncate(&mut self) {
        // For ref-tests, truncate scrollback to prepare for serialization
        // This is a no-op if there's no scrollback, otherwise clear it
        if let Some(scrollback) = self.scrollback_mut() {
            scrollback.clear();
        }
        // Scroll to bottom to reset display offset
        self.scroll_to_bottom();
    }

    fn cursor_cell(&mut self) -> Option<&mut Cell> {
        let cursor = self.cursor();
        let row = cursor.row;
        let col = cursor.col;
        self.cell_mut(row, col)
    }
}

/// Apply an Alacritty-style scroll request to a dterm-core grid.
pub fn apply_scroll(grid: &mut Grid, scroll: Scroll) {
    let page = i32::from(grid.rows());
    match scroll {
        Scroll::Delta(delta) => grid.scroll_display(delta),
        Scroll::PageUp => grid.scroll_display(page),
        Scroll::PageDown => grid.scroll_display(-page),
        Scroll::Top => grid.scroll_to_top(),
        Scroll::Bottom => grid.scroll_to_bottom(),
    }
}

// ----------------------------------------------------------------------------
// Cell wrapper with position information
// ----------------------------------------------------------------------------

/// A cell with its position in the grid.
///
/// This mirrors Alacritty's `Indexed<T>` type for compatibility.
#[derive(Debug, PartialEq, Eq)]
pub struct Indexed<T> {
    /// Position of the cell in the grid.
    pub point: Point,
    /// The cell data.
    pub cell: T,
}

impl<T> Deref for Indexed<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.cell
    }
}

// ----------------------------------------------------------------------------
// Bidirectional iterator trait
// ----------------------------------------------------------------------------

/// Trait for iterators that can move both forward and backward.
pub trait BidirectionalIterator: Iterator {
    /// Move to the previous item.
    fn prev(&mut self) -> Option<Self::Item>;
}

// ----------------------------------------------------------------------------
// Grid cell iterator
// ----------------------------------------------------------------------------

/// Iterator over grid cells.
///
/// This mirrors Alacritty's `GridIterator` for compatibility with rendering code.
pub struct GridIterator<'a> {
    /// Reference to the grid.
    grid: &'a Grid,
    /// Current position in the grid.
    point: Point,
    /// End position (inclusive bound).
    end: Point,
}

impl<'a> GridIterator<'a> {
    /// Create a new iterator starting at `point` and ending at `end`.
    fn new(grid: &'a Grid, point: Point, end: Point) -> Self {
        Self { grid, point, end }
    }

    /// Get the current iterator position.
    #[inline]
    pub fn point(&self) -> Point {
        self.point
    }

    /// Get the cell at the current iterator position.
    ///
    /// Returns `None` if the position is invalid.
    #[inline]
    pub fn cell(&self) -> Option<&'a Cell> {
        self.get_cell(self.point)
    }

    /// Get a cell at a specific point.
    #[inline]
    fn get_cell(&self, point: Point) -> Option<&'a Cell> {
        // Convert grid coordinates to dterm-core coordinates
        // Alacritty uses negative lines for scrollback history
        let line = point.line.0;
        let col = point.column.0;

        // Calculate the visible row from the line index
        // line >= 0: visible area (line 0 is top of visible area)
        // line < 0: scrollback history
        let _display_offset = self.grid.display_offset();

        if line < 0 {
            // Scrollback history doesn't preserve per-cell styling,
            // so we can't return Cell references for scrollback lines.
            // Use get_scrollback_line() or get_scrollback_text() for
            // text-only access to scrollback content.
            None
        } else {
            // Visible area
            let row = line as u16;
            if row >= self.grid.rows() {
                return None;
            }
            self.grid.cell(row, col as u16)
        }
    }

    /// Get the number of columns in the grid.
    #[inline]
    fn columns(&self) -> usize {
        usize::from(self.grid.cols())
    }

    /// Get the last column index.
    #[inline]
    fn last_column(&self) -> Column {
        Column(self.columns().saturating_sub(1))
    }

    /// Get the topmost line (farthest into scrollback).
    #[inline]
    fn topmost_line(&self) -> Line {
        let history = self
            .grid
            .total_lines()
            .saturating_sub(usize::from(self.grid.rows()));
        Line(-(history as i32))
    }
}

impl<'a> Iterator for GridIterator<'a> {
    type Item = Indexed<&'a Cell>;

    fn next(&mut self) -> Option<Self::Item> {
        // Stop once we've passed the end
        if self.point >= self.end {
            return None;
        }

        // Advance position
        let last_col = self.last_column();
        if self.point.column >= last_col {
            self.point.column = Column(0);
            self.point.line = self.point.line + 1;
        } else {
            self.point.column = self.point.column + 1;
        }

        // Get the cell at the new position
        self.get_cell(self.point).map(|cell| Indexed {
            point: self.point,
            cell,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.point >= self.end {
            return (0, Some(0));
        }

        let columns = self.columns();
        let size = if self.point.line == self.end.line {
            (self.end.column.0).saturating_sub(self.point.column.0)
        } else {
            let cols_on_first_line = columns.saturating_sub(self.point.column.0 + 1);
            let middle_lines = ((self.end.line - self.point.line) as usize).saturating_sub(1);
            let cols_on_last_line = self.end.column.0 + 1;

            cols_on_first_line + middle_lines * columns + cols_on_last_line
        };

        (size, Some(size))
    }
}

impl BidirectionalIterator for GridIterator<'_> {
    fn prev(&mut self) -> Option<Self::Item> {
        let topmost_line = self.topmost_line();
        let last_column = self.last_column();

        // Stop once we've reached the beginning
        if self.point <= Point::new(topmost_line, Column(0)) {
            return None;
        }

        // Move backward
        if self.point.column == Column(0) {
            self.point.column = last_column;
            self.point.line = self.point.line - 1;
        } else {
            self.point.column = self.point.column - 1;
        }

        // Get the cell at the new position
        self.get_cell(self.point).map(|cell| Indexed {
            point: self.point,
            cell,
        })
    }
}

// ----------------------------------------------------------------------------
// Grid iterator methods
// ----------------------------------------------------------------------------

/// Extension trait adding iterator methods to Grid.
pub trait GridIteratorExt {
    /// Iterate over all cells starting at a specific point.
    ///
    /// The iterator starts *before* the given point, so the first call to
    /// `next()` returns the cell at `point`.
    fn iter_from(&self, point: Point) -> GridIterator<'_>;

    /// Iterate over all visible cells.
    ///
    /// This includes cells that are visible when accounting for the current
    /// display offset (scroll position).
    fn display_iter(&self) -> GridIterator<'_>;
}

impl GridIteratorExt for Grid {
    fn iter_from(&self, point: Point) -> GridIterator<'_> {
        let screen_lines = usize::from(self.rows());
        let columns = usize::from(self.cols());

        let end_line = Line((screen_lines as i32).saturating_sub(1));
        let end_col = Column(columns.saturating_sub(1));
        let end = Point::new(end_line, end_col);

        // Position iterator one step before the starting point
        // so that the first call to next() returns the starting point
        let start = if point.column == Column(0) {
            Point::new(point.line - 1, end_col)
        } else {
            Point::new(point.line, point.column - 1)
        };

        GridIterator::new(self, start, end)
    }

    fn display_iter(&self) -> GridIterator<'_> {
        let screen_lines = usize::from(self.rows());
        let columns = usize::from(self.cols());
        let display_offset = self.display_offset();
        let last_column = Column(columns.saturating_sub(1));

        // Start position: one row before the display area (will be advanced by first next())
        let start_line = Line(-((display_offset as i32) + 1));
        let start = Point::new(start_line, last_column);

        // End position: bottom of visible area (accounting for display offset)
        let visible_end = Line((screen_lines as i32).saturating_sub(1));
        let history_size = self.total_lines().saturating_sub(screen_lines);
        let end_line = min(
            Line(start_line.0 + screen_lines as i32),
            Line((history_size + screen_lines - 1) as i32),
        );
        let end_line = min(end_line, visible_end);
        let end = Point::new(end_line, last_column);

        GridIterator::new(self, start, end)
    }
}

// ----------------------------------------------------------------------------
// Grid indexing helper functions (Alacritty-compatible)
// ----------------------------------------------------------------------------

/// Convert a Line index to a row index (u16) for visible area access.
///
/// Line indexing:
/// - Positive values (0, 1, 2, ...) = visible rows from top
/// - Negative values (-1, -2, ...) = scrollback history (returns None)
///
/// Returns None if:
/// - The line is in scrollback (negative) - use `get_scrollback_line` instead
/// - The line is beyond the visible area
///
/// Note: This function only converts visible area lines. For scrollback access,
/// use `get_scrollback_line` or `get_scrollback_text` which return text-only
/// `ScrollbackLine` objects (scrollback doesn't preserve cell styling).
#[inline]
pub fn line_to_row(grid: &Grid, line: Line) -> Option<u16> {
    let visible_rows = grid.rows();

    if line.0 >= 0 {
        // Visible area
        let row = line.0 as u16;
        if row < visible_rows {
            Some(row)
        } else {
            None
        }
    } else {
        // Scrollback history - use get_scrollback_line() for text access
        None
    }
}

/// Get a row from the grid by Line index (visible area only).
///
/// Returns None if the line is out of bounds or in scrollback history.
/// For scrollback access, use `get_scrollback_line` instead.
#[inline]
pub fn grid_row(grid: &Grid, line: Line) -> Option<&Row> {
    let row_idx = line_to_row(grid, line)?;
    grid.row(row_idx)
}

/// Get a mutable row from the grid by Line index (visible area only).
///
/// Returns None if the line is out of bounds or in scrollback history.
#[inline]
pub fn grid_row_mut(grid: &mut Grid, line: Line) -> Option<&mut Row> {
    let row_idx = line_to_row(grid, line)?;
    grid.row_mut(row_idx)
}

/// Get a cell from the grid by Point index (visible area only).
///
/// Returns None if the point is out of bounds or the line is in scrollback.
/// For scrollback text access, use `get_scrollback_text` instead.
#[inline]
pub fn grid_cell(grid: &Grid, point: Point) -> Option<&Cell> {
    let row = grid_row(grid, point.line)?;
    row.get(point.column.0 as u16)
}

/// Get a mutable cell from the grid by Point index (visible area only).
///
/// Returns None if the point is out of bounds or the line is in scrollback.
#[inline]
pub fn grid_cell_mut(grid: &mut Grid, point: Point) -> Option<&mut Cell> {
    let row = grid_row_mut(grid, point.line)?;
    row.get_mut(point.column.0 as u16)
}

/// Get a cell from a row by Column index.
///
/// Returns None if the column is out of bounds.
#[inline]
pub fn row_cell(row: &Row, column: Column) -> Option<&Cell> {
    row.get(column.0 as u16)
}

/// Get a mutable cell from a row by Column index.
///
/// Returns None if the column is out of bounds.
#[inline]
pub fn row_cell_mut(row: &mut Row, column: Column) -> Option<&mut Cell> {
    row.get_mut(column.0 as u16)
}

// ----------------------------------------------------------------------------
// Scrollback history access (text-only, no cell styling)
// ----------------------------------------------------------------------------

/// Get a scrollback history line by Line index.
///
/// Line indexing for scrollback:
/// - Line(-1) = most recent scrollback line (just above visible area)
/// - Line(-2) = second most recent
/// - etc.
///
/// Returns None if:
/// - The line index is non-negative (use `grid_row` for visible area)
/// - The line index is beyond available scrollback history
///
/// Note: Scrollback lines are text-only and don't preserve cell styling.
/// For styled cell access, use `grid_row` with visible area lines.
#[inline]
pub fn get_scrollback_line(grid: &Grid, line: Line) -> Option<ScrollbackLine> {
    if line.0 >= 0 {
        // Not in scrollback - use grid_row for visible area
        return None;
    }

    // Convert negative line to reverse index
    // Line(-1) -> rev_idx 0 (most recent)
    // Line(-2) -> rev_idx 1
    let rev_idx = ((-line.0) - 1) as usize;
    grid.get_history_line_rev(rev_idx)
}

/// Get the text content of a scrollback line.
///
/// Convenience wrapper around `get_scrollback_line` that returns the text content.
///
/// Returns None if the line is not in scrollback or out of bounds.
#[inline]
pub fn get_scrollback_text(grid: &Grid, line: Line) -> Option<String> {
    get_scrollback_line(grid, line).map(|l| l.to_string())
}

/// Check if a Line index refers to scrollback history.
#[inline]
pub fn is_scrollback_line(line: Line) -> bool {
    line.0 < 0
}

/// Get the total number of scrollback lines available.
#[inline]
pub fn scrollback_line_count(grid: &Grid) -> usize {
    grid.history_line_count()
}

// ----------------------------------------------------------------------------
// Index trait implementations for Alacritty-style grid[Line] access
// ----------------------------------------------------------------------------

/// Newtype wrapper around Grid that implements Alacritty-style indexing.
///
/// This wrapper allows using `grid[Line]` syntax to access rows in the visible
/// area. Scrollback lines are not accessible via indexing; use `get_scrollback_line`
/// instead.
///
/// # Panics
///
/// Panics if the line index is out of bounds or in scrollback history.
/// For fallible access, use `grid_row` or `grid_cell` functions instead.
#[repr(transparent)]
pub struct IndexableGrid<'a>(pub &'a Grid);

#[repr(transparent)]
pub struct IndexableGridMut<'a>(pub &'a mut Grid);

impl<'a> Index<Line> for IndexableGrid<'a> {
    type Output = Row;

    #[inline]
    fn index(&self, line: Line) -> &Self::Output {
        grid_row(self.0, line).expect("Line index out of bounds")
    }
}

impl<'a> Index<Line> for IndexableGridMut<'a> {
    type Output = Row;

    #[inline]
    fn index(&self, line: Line) -> &Self::Output {
        grid_row(self.0, line).expect("Line index out of bounds")
    }
}

impl<'a> IndexMut<Line> for IndexableGridMut<'a> {
    #[inline]
    fn index_mut(&mut self, line: Line) -> &mut Self::Output {
        grid_row_mut(self.0, line).expect("Line index out of bounds")
    }
}

impl<'a> Index<Point> for IndexableGrid<'a> {
    type Output = Cell;

    #[inline]
    fn index(&self, point: Point) -> &Self::Output {
        grid_cell(self.0, point).expect("Point index out of bounds")
    }
}

impl<'a> Index<Point> for IndexableGridMut<'a> {
    type Output = Cell;

    #[inline]
    fn index(&self, point: Point) -> &Self::Output {
        grid_cell(self.0, point).expect("Point index out of bounds")
    }
}

impl<'a> IndexMut<Point> for IndexableGridMut<'a> {
    #[inline]
    fn index_mut(&mut self, point: Point) -> &mut Self::Output {
        grid_cell_mut(self.0, point).expect("Point index out of bounds")
    }
}

/// Extension trait for converting a Grid reference to an indexable form.
pub trait AsIndexable {
    /// Get an indexable view of the grid.
    ///
    /// This allows using `grid.as_indexable()[Line(0)]` or similar syntax.
    fn as_indexable(&self) -> IndexableGrid<'_>;
}

/// Extension trait for converting a mutable Grid reference to an indexable form.
pub trait AsIndexableMut {
    /// Get a mutably indexable view of the grid.
    fn as_indexable_mut(&mut self) -> IndexableGridMut<'_>;
}

impl AsIndexable for Grid {
    #[inline]
    fn as_indexable(&self) -> IndexableGrid<'_> {
        IndexableGrid(self)
    }
}

impl AsIndexableMut for Grid {
    #[inline]
    fn as_indexable_mut(&mut self) -> IndexableGridMut<'_> {
        IndexableGridMut(self)
    }
}
