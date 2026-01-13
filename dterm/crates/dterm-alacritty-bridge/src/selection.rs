//! Selection types mirroring Alacritty's selection module.
//!
//! This module provides selection tracking for text in the terminal grid,
//! supporting simple, block (rectangular), semantic (word), and line selection modes.

use std::ops::Range;

use crate::grid::Dimensions;
use crate::index::{Column, Direction, Line, Point, Side};

/// Type of selection being performed.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum SelectionType {
    /// Simple (character) selection.
    #[default]
    Simple,
    /// Block (rectangular) selection.
    Block,
    /// Semantic selection (word boundaries).
    Semantic,
    /// Line selection (full lines).
    Lines,
}

/// Anchor point for selection with associated side.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct Anchor {
    point: Point,
    side: Side,
}

impl Anchor {
    fn new(point: Point, side: Side) -> Self {
        Self { point, side }
    }
}

/// Range of selected content.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SelectionRange {
    /// Start point of selection (top-left in reading order).
    pub start: Point,
    /// End point of selection (bottom-right in reading order).
    pub end: Point,
    /// Whether this is a block (rectangular) selection.
    pub is_block: bool,
}

impl SelectionRange {
    /// Create a new selection range.
    pub fn new(start: Point, end: Point, is_block: bool) -> Self {
        Self {
            start,
            end,
            is_block,
        }
    }

    /// Check if a point is within the selection.
    pub fn contains(&self, point: Point) -> bool {
        self.contains_line(point.line) && self.contains_column(point.line, point.column)
    }

    /// Check if a cell is within the selection.
    ///
    /// This is similar to `contains` but handles wide character cells appropriately.
    /// For cells that are wide char continuations (the trailing half of a double-width char),
    /// check if the preceding cell is selected.
    ///
    /// The `is_wide_continuation` parameter should be true if this cell is a wide char spacer
    /// (the second cell of a double-width character).
    pub fn contains_cell(&self, point: Point, is_wide_continuation: bool) -> bool {
        // For wide char continuations, check the preceding cell
        if is_wide_continuation && point.column.0 > 0 {
            let wide_char_point = Point::new(point.line, Column(point.column.0 - 1));
            return self.contains(wide_char_point);
        }

        self.contains(point)
    }

    /// Check if a line is within the selection.
    pub fn contains_line(&self, line: Line) -> bool {
        line >= self.start.line && line <= self.end.line
    }

    /// Check if a column is within the selection for a given line.
    pub fn contains_column(&self, line: Line, column: Column) -> bool {
        if self.is_block {
            // Block selection: check column bounds only
            let start_col = self.start.column.min(self.end.column);
            let end_col = self.start.column.max(self.end.column);
            column >= start_col && column <= end_col
        } else {
            // Linear selection
            if line == self.start.line && line == self.end.line {
                column >= self.start.column && column <= self.end.column
            } else if line == self.start.line {
                column >= self.start.column
            } else if line == self.end.line {
                column <= self.end.column
            } else {
                true
            }
        }
    }
}

/// Selection state tracking.
#[derive(Debug, Clone)]
pub struct Selection {
    /// Type of selection.
    pub ty: SelectionType,
    /// Selection region (start and end anchors).
    region: Range<Anchor>,
}

impl Selection {
    /// Create a new selection starting at the given point.
    pub fn new(ty: SelectionType, location: Point, side: Side) -> Self {
        let anchor = Anchor::new(location, side);
        Self {
            ty,
            region: anchor..anchor,
        }
    }

    /// Update the end point of the selection.
    pub fn update(&mut self, point: Point, side: Side) {
        self.region.end = Anchor::new(point, side);
    }

    /// Check if the selection is empty (start == end).
    pub fn is_empty(&self) -> bool {
        let start = &self.region.start;
        let end = &self.region.end;

        match self.ty {
            SelectionType::Simple | SelectionType::Semantic | SelectionType::Lines => {
                start.point == end.point && start.side == end.side
            }
            SelectionType::Block => {
                start.point.line == end.point.line
                    && start.point.column == end.point.column
                    && start.side == end.side
            }
        }
    }

    /// Expand selection to include the entire cell on both ends.
    pub fn include_all(&mut self) {
        self.region.start.side = Direction::Left;
        self.region.end.side = Direction::Right;
    }

    /// Get the start point of the selection.
    pub fn start(&self) -> Point {
        self.region.start.point
    }

    /// Get the end point of the selection.
    pub fn end(&self) -> Point {
        self.region.end.point
    }

    /// Check if the selection intersects with a line range.
    pub fn intersects_range(&self, range: Range<Line>) -> bool {
        let (start, end) = self.ordered_bounds();
        end.line >= range.start && start.line < range.end
    }

    /// Get ordered bounds (start always before end in reading order).
    fn ordered_bounds(&self) -> (Point, Point) {
        let mut start = self.region.start;
        let mut end = self.region.end;

        // Ensure start <= end in reading order
        if start.point > end.point || (start.point == end.point && start.side > end.side) {
            std::mem::swap(&mut start, &mut end);
        }

        // Adjust for side
        let start_point = if start.side == Direction::Right {
            Point::new(start.point.line, start.point.column + 1)
        } else {
            start.point
        };

        let end_point = if end.side == Direction::Left && end.point.column.0 > 0 {
            Point::new(end.point.line, end.point.column - 1)
        } else {
            end.point
        };

        (start_point, end_point)
    }

    /// Convert selection to a range with concrete grid coordinates.
    pub fn to_range<D: Dimensions>(&self, dimensions: &D) -> Option<SelectionRange> {
        let (start, end) = self.ordered_bounds();

        // Selection is empty if start > end after adjustments
        if start > end {
            return None;
        }

        match self.ty {
            SelectionType::Simple | SelectionType::Semantic => {
                Some(SelectionRange::new(start, end, false))
            }
            SelectionType::Block => Some(SelectionRange::new(start, end, true)),
            SelectionType::Lines => {
                let start = Point::new(start.line, Column(0));
                let end = Point::new(end.line, dimensions.last_column());
                Some(SelectionRange::new(start, end, false))
            }
        }
    }

    /// Rotate selection when scrollback changes (lines removed or added).
    ///
    /// Returns `None` if the selection is entirely above the visible region.
    pub fn rotate<D: Dimensions>(
        mut self,
        dimensions: &D,
        range: &Range<Line>,
        delta: i32,
    ) -> Option<Selection> {
        let screen_lines = dimensions.screen_lines() as i32;
        let topmost = dimensions.topmost_line();

        // Rotate start
        let mut start = self.region.start.point;
        if range.start <= start.line && start.line < range.end {
            start.line = start.line + delta;

            // Clamp to valid range
            if start.line < topmost {
                start.line = topmost;
                start.column = Column(0);
            }
        }

        // Rotate end
        let mut end = self.region.end.point;
        if range.start <= end.line && end.line < range.end {
            end.line = end.line + delta;

            // Clamp to valid range
            if end.line < topmost {
                end.line = topmost;
                end.column = Column(0);
            }
        }

        // Check if selection is entirely scrolled out
        let bottommost = Line(screen_lines - 1);
        if start.line > bottommost || end.line < topmost {
            return None;
        }

        self.region.start.point = start;
        self.region.end.point = end;

        Some(self)
    }
}

impl PartialOrd for Side {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Side {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Direction::Left, Direction::Right) => std::cmp::Ordering::Less,
            (Direction::Right, Direction::Left) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(line: i32, col: usize) -> Point {
        Point::new(Line(line), Column(col))
    }

    #[test]
    fn selection_new() {
        let sel = Selection::new(SelectionType::Simple, point(0, 5), Direction::Left);
        assert_eq!(sel.ty, SelectionType::Simple);
        assert!(sel.is_empty());
    }

    #[test]
    fn selection_update() {
        let mut sel = Selection::new(SelectionType::Simple, point(0, 0), Direction::Left);
        sel.update(point(2, 10), Direction::Right);
        assert!(!sel.is_empty());
        assert_eq!(sel.start(), point(0, 0));
        assert_eq!(sel.end(), point(2, 10));
    }

    #[test]
    fn selection_range_contains() {
        let range = SelectionRange::new(point(1, 5), point(3, 10), false);

        // Inside selection
        assert!(range.contains(point(2, 0)));
        assert!(range.contains(point(1, 5)));
        assert!(range.contains(point(3, 10)));

        // Outside selection
        assert!(!range.contains(point(0, 5)));
        assert!(!range.contains(point(4, 0)));
        assert!(!range.contains(point(1, 4)));
        assert!(!range.contains(point(3, 11)));
    }

    #[test]
    fn block_selection_contains() {
        // Block selection from (1, 5) to (3, 10)
        let range = SelectionRange::new(point(1, 5), point(3, 10), true);

        // Inside block
        assert!(range.contains(point(2, 7)));

        // Outside block (wrong column)
        assert!(!range.contains(point(2, 4)));
        assert!(!range.contains(point(2, 11)));
    }

    #[test]
    fn line_selection_to_range() {
        let mut sel = Selection::new(SelectionType::Lines, point(1, 5), Direction::Left);
        sel.update(point(3, 2), Direction::Right);

        // Mock dimensions
        let dims = (100, 80); // 100 lines, 80 columns
        let range = sel.to_range(&dims).unwrap();

        assert_eq!(range.start.line, Line(1));
        assert_eq!(range.start.column, Column(0));
        assert_eq!(range.end.line, Line(3));
        assert_eq!(range.end.column, Column(79)); // last_column()
        assert!(!range.is_block);
    }

    #[test]
    fn selection_intersects_range() {
        let mut sel = Selection::new(SelectionType::Simple, point(2, 0), Direction::Left);
        sel.update(point(5, 10), Direction::Right);

        // Intersecting ranges
        assert!(sel.intersects_range(Line(0)..Line(10)));
        assert!(sel.intersects_range(Line(3)..Line(4)));
        assert!(sel.intersects_range(Line(5)..Line(6)));

        // Non-intersecting ranges
        assert!(!sel.intersects_range(Line(6)..Line(10)));
        assert!(!sel.intersects_range(Line(-5)..Line(0)));
    }

    #[test]
    fn selection_range_contains_cell() {
        let range = SelectionRange::new(point(1, 5), point(3, 10), false);

        // Regular cell inside selection
        assert!(range.contains_cell(point(2, 7), false));

        // Regular cell outside selection
        assert!(!range.contains_cell(point(0, 5), false));

        // Wide char continuation inside selection (prev cell at col 6 is selected)
        assert!(range.contains_cell(point(2, 7), true));

        // Wide char continuation at boundary - col 5 is start, so continuation at 6 checks col 5
        assert!(range.contains_cell(point(1, 6), true));

        // Wide char continuation where prev cell is outside selection
        // col 4 is before selection start (col 5)
        assert!(!range.contains_cell(point(1, 5), true));
    }

    #[test]
    fn selection_range_contains_cell_block() {
        let range = SelectionRange::new(point(1, 5), point(3, 10), true);

        // Wide char continuation in block selection
        // At point(2, 6), it checks point(2, 5) which is in block (cols 5-10)
        assert!(range.contains_cell(point(2, 6), true));

        // Wide char continuation outside block column range
        // At point(2, 5), it checks point(2, 4) which is outside block
        assert!(!range.contains_cell(point(2, 5), true));
    }
}
