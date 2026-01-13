//! Vi mode types mirroring Alacritty's vi_mode module.
//!
//! This module provides vi-style navigation mode for the terminal,
//! allowing keyboard-driven cursor movement and text selection.

use std::collections::HashMap;

use crate::grid::Dimensions;
use crate::index::{Boundary, Column, Direction, Line, Point};

/// Vi mode motion commands.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ViMotion {
    /// Move cursor up.
    Up,
    /// Move cursor down.
    Down,
    /// Move cursor left.
    Left,
    /// Move cursor right.
    Right,
    /// Move to first column or beginning of wrapped line.
    First,
    /// Move to last column or end of wrapped line.
    Last,
    /// Move to first non-empty cell in line.
    FirstOccupied,
    /// Move to top of visible screen.
    High,
    /// Move to center of visible screen.
    Middle,
    /// Move to bottom of visible screen.
    Low,
    /// Move to start of previous semantic word (left).
    SemanticLeft,
    /// Move to start of next semantic word (right).
    SemanticRight,
    /// Move to end of previous semantic word.
    SemanticLeftEnd,
    /// Move to end of current/next semantic word.
    SemanticRightEnd,
    /// Move to start of previous whitespace-separated word.
    WordLeft,
    /// Move to start of next whitespace-separated word.
    WordRight,
    /// Move to end of previous whitespace-separated word.
    WordLeftEnd,
    /// Move to end of current/next whitespace-separated word.
    WordRightEnd,
    /// Move to matching bracket.
    Bracket,
    /// Move above current paragraph (to empty line).
    ParagraphUp,
    /// Move below current paragraph (to empty line).
    ParagraphDown,
    /// Jump to the next search match (vim 'n').
    SearchNext,
    /// Jump to the previous search match (vim 'N').
    SearchPrevious,
    /// Go to a mark (vim ` or ').
    ///
    /// The backtick (`) goes to the exact position, while single quote (')
    /// goes to the first non-blank character of the marked line.
    GotoMark(char),
    /// Go to the first non-blank character of a marked line (vim ').
    GotoMarkLine(char),
    /// Jump to the next URL in the terminal content.
    UrlNext,
    /// Jump to the previous URL in the terminal content.
    UrlPrev,
}

/// Inline character search state for f/F/t/T motions.
///
/// This stores the last inline character search so it can be repeated
/// with `;` (same direction) or `,` (opposite direction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlineSearchState {
    /// The character that was searched for.
    pub char: char,
    /// The type of search that was performed.
    pub kind: InlineSearchKind,
}

/// The type of inline character search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineSearchKind {
    /// Find character to the right (f).
    FindRight,
    /// Find character to the left (F).
    FindLeft,
    /// Find character to the right and stop before it (t).
    TillRight,
    /// Find character to the left and stop after it (T).
    TillLeft,
}

impl InlineSearchKind {
    /// Returns the direction of this search kind.
    #[must_use]
    pub fn direction(self) -> Direction {
        match self {
            InlineSearchKind::FindRight | InlineSearchKind::TillRight => Direction::Right,
            InlineSearchKind::FindLeft | InlineSearchKind::TillLeft => Direction::Left,
        }
    }

    /// Returns this search kind with the direction reversed.
    #[must_use]
    pub fn reversed(self) -> Self {
        match self {
            InlineSearchKind::FindRight => InlineSearchKind::FindLeft,
            InlineSearchKind::FindLeft => InlineSearchKind::FindRight,
            InlineSearchKind::TillRight => InlineSearchKind::TillLeft,
            InlineSearchKind::TillLeft => InlineSearchKind::TillRight,
        }
    }

    /// Returns whether this is a "till" search (t/T) vs "find" search (f/F).
    #[must_use]
    pub fn is_till(self) -> bool {
        matches!(
            self,
            InlineSearchKind::TillRight | InlineSearchKind::TillLeft
        )
    }
}

impl ViMotion {
    /// Returns the direction of this motion.
    pub fn direction(self) -> Direction {
        match self {
            ViMotion::Up
            | ViMotion::Left
            | ViMotion::First
            | ViMotion::High
            | ViMotion::SemanticLeft
            | ViMotion::SemanticLeftEnd
            | ViMotion::WordLeft
            | ViMotion::WordLeftEnd
            | ViMotion::ParagraphUp
            | ViMotion::SearchPrevious => Direction::Left,
            ViMotion::Down
            | ViMotion::Right
            | ViMotion::Last
            | ViMotion::FirstOccupied
            | ViMotion::Middle
            | ViMotion::Low
            | ViMotion::SemanticRight
            | ViMotion::SemanticRightEnd
            | ViMotion::WordRight
            | ViMotion::WordRightEnd
            | ViMotion::Bracket
            | ViMotion::ParagraphDown
            | ViMotion::SearchNext => Direction::Right,
            // Mark motions don't have a fixed direction - depends on current position
            ViMotion::GotoMark(_) | ViMotion::GotoMarkLine(_) => Direction::Right,
            // URL navigation
            ViMotion::UrlNext => Direction::Right,
            ViMotion::UrlPrev => Direction::Left,
        }
    }
}

/// Vi mode cursor state.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct ViModeCursor {
    /// Current position of the vi mode cursor.
    pub point: Point,
}

impl ViModeCursor {
    /// Create a new vi mode cursor at the given point.
    pub fn new(point: Point) -> Self {
        Self { point }
    }

    /// Execute a motion command.
    ///
    /// This method applies the given motion to move the cursor,
    /// returning a new cursor state. The `semantic_fn` parameter
    /// allows callers to provide semantic boundary detection.
    pub fn motion<D: Dimensions>(
        mut self,
        dimensions: &D,
        motion: ViMotion,
        boundary: Boundary,
    ) -> Self {
        let screen_lines = dimensions.screen_lines() as i32;
        let columns = dimensions.columns();
        let last_column = Column(columns.saturating_sub(1));
        let topmost = dimensions.topmost_line();
        let bottommost = dimensions.bottommost_line();

        match motion {
            ViMotion::Up => {
                if self.point.line.0 > topmost.0 {
                    self.point.line = self.point.line - 1;
                }
            }
            ViMotion::Down => {
                if self.point.line < bottommost {
                    self.point.line = self.point.line + 1;
                }
            }
            ViMotion::Left => {
                if self.point.column.0 > 0 {
                    self.point.column = self.point.column - 1;
                } else if boundary == Boundary::None && self.point.line.0 > topmost.0 {
                    // Wrap to previous line
                    self.point.line = self.point.line - 1;
                    self.point.column = last_column;
                }
            }
            ViMotion::Right => {
                if self.point.column < last_column {
                    self.point.column = self.point.column + 1;
                } else if boundary == Boundary::None && self.point.line < bottommost {
                    // Wrap to next line
                    self.point.line = self.point.line + 1;
                    self.point.column = Column(0);
                }
            }
            ViMotion::First => {
                self.point.column = Column(0);
            }
            ViMotion::Last => {
                self.point.column = last_column;
            }
            ViMotion::FirstOccupied => {
                // Fallback to column 0 - real implementation is in Term::vi_motion()
                // which has grid access to find the first non-whitespace character.
                self.point.column = Column(0);
            }
            ViMotion::High => {
                // Move to top of visible screen
                self.point.line = Line(0);
            }
            ViMotion::Middle => {
                // Move to center of visible screen
                self.point.line = Line(screen_lines / 2);
            }
            ViMotion::Low => {
                // Move to bottom of visible screen
                self.point.line = Line(screen_lines - 1);
            }
            // These motions require grid content access and are handled
            // by Term::vi_motion() directly - they should not reach here
            // when called via Term, but we provide basic fallback behavior
            // for direct ViModeCursor usage.
            ViMotion::SemanticLeft
            | ViMotion::SemanticRight
            | ViMotion::SemanticLeftEnd
            | ViMotion::SemanticRightEnd
            | ViMotion::WordLeft
            | ViMotion::WordRight
            | ViMotion::WordLeftEnd
            | ViMotion::WordRightEnd => {
                // Fallback: just move left/right one character
                if motion.direction() == Direction::Left {
                    if self.point.column.0 > 0 {
                        self.point.column = self.point.column - 1;
                    }
                } else if self.point.column < last_column {
                    self.point.column = self.point.column + 1;
                }
            }
            ViMotion::Bracket => {
                // Fallback: no-op (bracket matching requires grid access)
            }
            ViMotion::ParagraphUp => {
                // Fallback: just move up one line
                if self.point.line.0 > topmost.0 {
                    self.point.line = self.point.line - 1;
                }
            }
            ViMotion::ParagraphDown => {
                // Fallback: just move down one line
                if self.point.line < bottommost {
                    self.point.line = self.point.line + 1;
                }
            }
            ViMotion::SearchNext | ViMotion::SearchPrevious => {
                // Search motions require Term access for search state
                // Fallback: no-op
            }
            ViMotion::GotoMark(_) | ViMotion::GotoMarkLine(_) => {
                // Mark motions require Term access for mark storage
                // Fallback: no-op
            }
            ViMotion::UrlNext | ViMotion::UrlPrev => {
                // URL motions require Term access for grid content
                // Fallback: no-op
            }
        }

        // Clamp to grid bounds
        self.point.line = Line(self.point.line.0.max(topmost.0).min(bottommost.0));
        self.point.column = Column(self.point.column.0.min(columns.saturating_sub(1)));

        self
    }

    /// Scroll cursor position (like Ctrl+D, Ctrl+U in vim).
    ///
    /// Positive lines scroll down (cursor moves up in content),
    /// negative lines scroll up (cursor moves down in content).
    pub fn scroll<D: Dimensions>(mut self, dimensions: &D, lines: i32) -> Self {
        let topmost = dimensions.topmost_line();
        let bottommost = dimensions.bottommost_line();

        self.point.line = Line((self.point.line.0 + lines).clamp(topmost.0, bottommost.0));

        self
    }
}

/// Vi mode marks storage.
///
/// Stores named marks (positions) that can be jumped to later. In vim:
/// - `ma` sets mark 'a' at the current position
/// - `` `a `` jumps to mark 'a' (exact position)
/// - `'a` jumps to mark 'a' (first non-blank on line)
///
/// Marks are stored by character name. Vim distinguishes between:
/// - Lowercase marks (a-z): local to buffer
/// - Uppercase marks (A-Z): global across files
/// - Special marks (0-9, ', `, etc.): automatic marks
///
/// For terminal vi mode, we support lowercase marks (a-z) and some special marks.
#[derive(Debug, Clone, Default)]
pub struct ViMarks {
    /// Named marks. Key is the mark character, value is the position.
    marks: HashMap<char, Point>,
}

impl ViMarks {
    /// Create a new empty mark storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            marks: HashMap::new(),
        }
    }

    /// Set a mark at the given position.
    ///
    /// Valid mark characters are a-z for local marks.
    /// Returns true if the mark was set, false if the character is invalid.
    pub fn set(&mut self, mark: char, point: Point) -> bool {
        // Accept lowercase letters and some special marks
        if mark.is_ascii_lowercase() || mark == '\'' || mark == '`' {
            self.marks.insert(mark, point);
            true
        } else {
            false
        }
    }

    /// Get the position of a mark.
    ///
    /// Returns `None` if the mark is not set.
    #[must_use]
    pub fn get(&self, mark: char) -> Option<Point> {
        self.marks.get(&mark).copied()
    }

    /// Remove a mark.
    ///
    /// Returns the previous position if the mark was set.
    pub fn remove(&mut self, mark: char) -> Option<Point> {
        self.marks.remove(&mark)
    }

    /// Clear all marks.
    pub fn clear(&mut self) {
        self.marks.clear();
    }

    /// Check if a mark is set.
    #[must_use]
    pub fn contains(&self, mark: char) -> bool {
        self.marks.contains_key(&mark)
    }

    /// Get all set marks.
    pub fn iter(&self) -> impl Iterator<Item = (&char, &Point)> {
        self.marks.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor_at(line: i32, col: usize) -> ViModeCursor {
        ViModeCursor::new(Point::new(Line(line), Column(col)))
    }

    #[test]
    fn vi_motion_up() {
        let dims = (100, 80);
        let cursor = cursor_at(5, 10);
        let moved = cursor.motion(&dims, ViMotion::Up, Boundary::Grid);
        assert_eq!(moved.point.line, Line(4));
        assert_eq!(moved.point.column, Column(10));
    }

    #[test]
    fn vi_motion_down() {
        let dims = (100, 80);
        let cursor = cursor_at(5, 10);
        let moved = cursor.motion(&dims, ViMotion::Down, Boundary::Grid);
        assert_eq!(moved.point.line, Line(6));
        assert_eq!(moved.point.column, Column(10));
    }

    #[test]
    fn vi_motion_left() {
        let dims = (100, 80);
        let cursor = cursor_at(5, 10);
        let moved = cursor.motion(&dims, ViMotion::Left, Boundary::Grid);
        assert_eq!(moved.point.line, Line(5));
        assert_eq!(moved.point.column, Column(9));
    }

    #[test]
    fn vi_motion_right() {
        let dims = (100, 80);
        let cursor = cursor_at(5, 10);
        let moved = cursor.motion(&dims, ViMotion::Right, Boundary::Grid);
        assert_eq!(moved.point.line, Line(5));
        assert_eq!(moved.point.column, Column(11));
    }

    #[test]
    fn vi_motion_first_last() {
        let dims = (100, 80);
        let cursor = cursor_at(5, 40);

        let first = cursor.motion(&dims, ViMotion::First, Boundary::Grid);
        assert_eq!(first.point.column, Column(0));

        let last = cursor.motion(&dims, ViMotion::Last, Boundary::Grid);
        assert_eq!(last.point.column, Column(79));
    }

    #[test]
    fn vi_motion_high_middle_low() {
        let dims = (24, 80); // 24 visible lines
        let cursor = cursor_at(5, 10);

        let high = cursor.motion(&dims, ViMotion::High, Boundary::Grid);
        assert_eq!(high.point.line, Line(0));

        let middle = cursor.motion(&dims, ViMotion::Middle, Boundary::Grid);
        assert_eq!(middle.point.line, Line(12));

        let low = cursor.motion(&dims, ViMotion::Low, Boundary::Grid);
        assert_eq!(low.point.line, Line(23));
    }

    #[test]
    fn vi_motion_at_boundary() {
        let dims = (24, 80);

        // At top, can't go up more
        let cursor = cursor_at(0, 0);
        let moved = cursor.motion(&dims, ViMotion::Up, Boundary::Grid);
        assert_eq!(moved.point.line, Line(0));

        // At left edge
        let cursor = cursor_at(5, 0);
        let moved = cursor.motion(&dims, ViMotion::Left, Boundary::Grid);
        assert_eq!(moved.point.column, Column(0));
    }

    #[test]
    fn vi_motion_wrap_with_no_boundary() {
        let dims = (24, 80);

        // At left edge with no boundary, wraps to previous line
        let cursor = cursor_at(5, 0);
        let moved = cursor.motion(&dims, ViMotion::Left, Boundary::None);
        assert_eq!(moved.point.line, Line(4));
        assert_eq!(moved.point.column, Column(79));

        // At right edge with no boundary, wraps to next line
        let cursor = cursor_at(5, 79);
        let moved = cursor.motion(&dims, ViMotion::Right, Boundary::None);
        assert_eq!(moved.point.line, Line(6));
        assert_eq!(moved.point.column, Column(0));
    }

    #[test]
    fn vi_cursor_scroll() {
        let dims = (24, 80);
        let cursor = cursor_at(10, 10);

        let scrolled = cursor.scroll(&dims, 5);
        assert_eq!(scrolled.point.line, Line(15));

        let scrolled = cursor.scroll(&dims, -5);
        assert_eq!(scrolled.point.line, Line(5));
    }

    #[test]
    fn vi_motion_direction() {
        assert_eq!(ViMotion::Up.direction(), Direction::Left);
        assert_eq!(ViMotion::Down.direction(), Direction::Right);
        assert_eq!(ViMotion::Left.direction(), Direction::Left);
        assert_eq!(ViMotion::Right.direction(), Direction::Right);
        assert_eq!(ViMotion::SemanticLeft.direction(), Direction::Left);
        assert_eq!(ViMotion::SemanticRight.direction(), Direction::Right);
        assert_eq!(ViMotion::SearchNext.direction(), Direction::Right);
        assert_eq!(ViMotion::SearchPrevious.direction(), Direction::Left);
    }

    #[test]
    fn inline_search_kind_direction() {
        assert_eq!(InlineSearchKind::FindRight.direction(), Direction::Right);
        assert_eq!(InlineSearchKind::FindLeft.direction(), Direction::Left);
        assert_eq!(InlineSearchKind::TillRight.direction(), Direction::Right);
        assert_eq!(InlineSearchKind::TillLeft.direction(), Direction::Left);
    }

    #[test]
    fn inline_search_kind_reversed() {
        assert_eq!(
            InlineSearchKind::FindRight.reversed(),
            InlineSearchKind::FindLeft
        );
        assert_eq!(
            InlineSearchKind::FindLeft.reversed(),
            InlineSearchKind::FindRight
        );
        assert_eq!(
            InlineSearchKind::TillRight.reversed(),
            InlineSearchKind::TillLeft
        );
        assert_eq!(
            InlineSearchKind::TillLeft.reversed(),
            InlineSearchKind::TillRight
        );
    }

    #[test]
    fn inline_search_kind_is_till() {
        assert!(!InlineSearchKind::FindRight.is_till());
        assert!(!InlineSearchKind::FindLeft.is_till());
        assert!(InlineSearchKind::TillRight.is_till());
        assert!(InlineSearchKind::TillLeft.is_till());
    }

    #[test]
    fn inline_search_state() {
        let state = InlineSearchState {
            char: 'x',
            kind: InlineSearchKind::FindRight,
        };
        assert_eq!(state.char, 'x');
        assert_eq!(state.kind.direction(), Direction::Right);
    }

    // ===== ViMarks Tests =====

    #[test]
    fn vi_marks_new() {
        let marks = ViMarks::new();
        assert!(!marks.contains('a'));
    }

    #[test]
    fn vi_marks_set_and_get() {
        let mut marks = ViMarks::new();
        let point = Point::new(Line(5), Column(10));

        assert!(marks.set('a', point));
        assert_eq!(marks.get('a'), Some(point));
    }

    #[test]
    fn vi_marks_invalid_char() {
        let mut marks = ViMarks::new();
        let point = Point::new(Line(5), Column(10));

        // Uppercase letters are not valid
        assert!(!marks.set('A', point));
        assert!(!marks.contains('A'));

        // Numbers are not valid
        assert!(!marks.set('1', point));
        assert!(!marks.contains('1'));
    }

    #[test]
    fn vi_marks_special_chars() {
        let mut marks = ViMarks::new();
        let point = Point::new(Line(5), Column(10));

        // Special marks ` and ' are valid
        assert!(marks.set('`', point));
        assert!(marks.set('\'', point));
        assert_eq!(marks.get('`'), Some(point));
        assert_eq!(marks.get('\''), Some(point));
    }

    #[test]
    fn vi_marks_remove() {
        let mut marks = ViMarks::new();
        let point = Point::new(Line(5), Column(10));

        marks.set('a', point);
        assert!(marks.contains('a'));

        let removed = marks.remove('a');
        assert_eq!(removed, Some(point));
        assert!(!marks.contains('a'));
    }

    #[test]
    fn vi_marks_clear() {
        let mut marks = ViMarks::new();
        marks.set('a', Point::new(Line(1), Column(0)));
        marks.set('b', Point::new(Line(2), Column(0)));
        marks.set('c', Point::new(Line(3), Column(0)));

        marks.clear();
        assert!(!marks.contains('a'));
        assert!(!marks.contains('b'));
        assert!(!marks.contains('c'));
    }

    #[test]
    fn vi_marks_iter() {
        let mut marks = ViMarks::new();
        marks.set('a', Point::new(Line(1), Column(0)));
        marks.set('b', Point::new(Line(2), Column(5)));

        let collected: Vec<_> = marks.iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn vi_motion_goto_mark_direction() {
        // GotoMark motions default to Right direction
        assert_eq!(ViMotion::GotoMark('a').direction(), Direction::Right);
        assert_eq!(ViMotion::GotoMarkLine('a').direction(), Direction::Right);
    }
}
