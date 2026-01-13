//! Search functionality for dterm-alacritty-bridge.
//!
//! This module provides Alacritty-compatible search APIs that wrap
//! dterm-core's trigram-indexed search with bloom filter acceleration.
//!
//! ## Features
//!
//! - Forward and backward search through terminal content
//! - Regex search support (via exact string matching)
//! - Match highlighting coordination
//! - Integration with selection system
//!
//! ## Usage
//!
//! ```ignore
//! use dterm_alacritty_bridge::{Term, VoidListener, search::RegexSearch};
//!
//! let mut term: Term<VoidListener> = /* ... */;
//! let mut search = RegexSearch::new("pattern");
//!
//! // Find next match from cursor
//! if let Some(match_range) = term.search_next(&search) {
//!     term.highlight_match(match_range);
//! }
//! ```

use crate::grid::{get_scrollback_text, scrollback_line_count, Grid};
use crate::index::{Column, Line, Point};
use crate::term::Term;

pub use dterm_core::search::{SearchDirection, SearchIndex, SearchMatch, TerminalSearch};

/// A search query with optional regex support.
///
/// This mirrors Alacritty's `RegexSearch` type. Currently implements
/// exact string matching; regex support can be added via the `regex` crate.
#[derive(Debug, Clone, Default)]
pub struct RegexSearch {
    /// The search pattern.
    pattern: String,
    /// Compiled search state (for future regex support).
    /// Currently unused but reserved for DFA state.
    _compiled: Option<()>,
}

impl RegexSearch {
    /// Create a new search from a pattern string.
    ///
    /// The pattern is treated as a literal string match.
    /// Future versions may support regex syntax.
    #[must_use]
    pub fn new(pattern: &str) -> Option<Self> {
        if pattern.is_empty() {
            return None;
        }
        Some(Self {
            pattern: pattern.to_string(),
            _compiled: None,
        })
    }

    /// Get the search pattern.
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Check if the pattern is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pattern.is_empty()
    }
}

/// A match found in the terminal content.
///
/// This represents a contiguous range of cells that match the search query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    /// Start of the match (inclusive).
    pub start: Point,
    /// End of the match (inclusive).
    pub end: Point,
}

impl Match {
    /// Create a new match.
    #[must_use]
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    /// Check if a point is within this match.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        point >= self.start && point <= self.end
    }

    /// Get the line of the match (start line).
    #[must_use]
    pub fn line(&self) -> Line {
        self.start.line
    }
}

/// Search state for a terminal.
///
/// Maintains the search index and current match state.
#[derive(Debug)]
pub struct TermSearch {
    /// The underlying search index.
    search: TerminalSearch,
    /// Whether the index needs rebuilding.
    dirty: bool,
    /// Current search query.
    query: Option<String>,
    /// All matches for the current query.
    matches: Vec<Match>,
    /// Index of the currently focused match.
    focused_match: Option<usize>,
}

impl TermSearch {
    /// Create a new search state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            search: TerminalSearch::new(),
            dirty: true,
            query: None,
            matches: Vec::new(),
            focused_match: None,
        }
    }

    /// Check if the search state is dirty (needs reindexing).
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the search index as dirty.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Rebuild the search index from grid content.
    pub fn rebuild_index(&mut self, grid: &Grid) {
        self.search.clear();

        // Index scrollback history
        let scrollback_count = scrollback_line_count(grid);
        for i in 0..scrollback_count {
            // Line(-1) = most recent, Line(-scrollback_count) = oldest
            let line = Line(-((scrollback_count - i) as i32));
            if let Some(text) = get_scrollback_text(grid, line) {
                self.search.index_scrollback_line(&text);
            }
        }

        // Index visible rows
        let rows = grid.rows();
        let base_line = scrollback_count;
        for row_idx in 0..rows {
            if let Some(row) = grid.row(row_idx) {
                let text = row.to_string();
                self.search
                    .index_visible_content(base_line + row_idx as usize, std::iter::once(text));
            }
        }

        self.dirty = false;

        // Re-run search if we have an active query
        if let Some(query) = self.query.clone() {
            self.execute_search(&query, scrollback_count);
        }
    }

    /// Set the search query and find all matches.
    pub fn set_query(&mut self, query: Option<&str>, grid: &Grid) {
        if self.dirty {
            self.rebuild_index(grid);
        }

        self.query = query.map(String::from);
        self.matches.clear();
        self.focused_match = None;

        if let Some(q) = query {
            if !q.is_empty() {
                let scrollback_count = scrollback_line_count(grid);
                self.execute_search(q, scrollback_count);
            }
        }
    }

    /// Execute search and populate matches.
    fn execute_search(&mut self, query: &str, scrollback_count: usize) {
        let results = self.search.search(query);

        self.matches = results
            .into_iter()
            .map(|m| {
                // Convert internal line numbers to bridge Line indices
                // Internal: 0..scrollback_count are scrollback, scrollback_count+ are visible
                // Bridge: negative for scrollback, 0+ for visible
                let line = if m.line < scrollback_count {
                    // Scrollback line: convert to negative index
                    // m.line 0 = oldest = Line(-scrollback_count)
                    // m.line scrollback_count-1 = newest = Line(-1)
                    Line(-((scrollback_count - m.line) as i32))
                } else {
                    // Visible line
                    Line((m.line - scrollback_count) as i32)
                };

                Match::new(
                    Point::new(line, Column(m.start_col)),
                    Point::new(line, Column(m.end_col.saturating_sub(1))),
                )
            })
            .collect();
    }

    /// Get the current query.
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// Get all matches for the current query.
    #[must_use]
    pub fn matches(&self) -> &[Match] {
        &self.matches
    }

    /// Get the number of matches.
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Get the currently focused match.
    #[must_use]
    pub fn focused_match(&self) -> Option<&Match> {
        self.focused_match.and_then(|i| self.matches.get(i))
    }

    /// Get the index of the focused match.
    #[must_use]
    pub fn focused_match_index(&self) -> Option<usize> {
        self.focused_match
    }

    /// Focus a specific match by index.
    pub fn focus_match(&mut self, index: usize) {
        if index < self.matches.len() {
            self.focused_match = Some(index);
        }
    }

    /// Find and focus the next match after the given point.
    ///
    /// Returns the focused match if found.
    pub fn focus_next(&mut self, after: Point) -> Option<&Match> {
        if self.matches.is_empty() {
            return None;
        }

        // Find first match after the point
        let next_idx = self
            .matches
            .iter()
            .position(|m| m.start > after)
            .unwrap_or(0); // Wrap to first match

        self.focused_match = Some(next_idx);
        self.matches.get(next_idx)
    }

    /// Find and focus the previous match before the given point.
    ///
    /// Returns the focused match if found.
    pub fn focus_prev(&mut self, before: Point) -> Option<&Match> {
        if self.matches.is_empty() {
            return None;
        }

        // Find last match before the point
        let prev_idx = self
            .matches
            .iter()
            .rposition(|m| m.start < before)
            .unwrap_or(self.matches.len() - 1); // Wrap to last match

        self.focused_match = Some(prev_idx);
        self.matches.get(prev_idx)
    }

    /// Advance to the next match from the current focused match.
    pub fn advance_next(&mut self) -> Option<&Match> {
        if self.matches.is_empty() {
            return None;
        }

        let next_idx = match self.focused_match {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        };

        self.focused_match = Some(next_idx);
        self.matches.get(next_idx)
    }

    /// Advance to the previous match from the current focused match.
    pub fn advance_prev(&mut self) -> Option<&Match> {
        if self.matches.is_empty() {
            return None;
        }

        let prev_idx = match self.focused_match {
            Some(i) => {
                if i == 0 {
                    self.matches.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.matches.len() - 1,
        };

        self.focused_match = Some(prev_idx);
        self.matches.get(prev_idx)
    }

    /// Check if a point is within any match.
    #[must_use]
    pub fn is_match(&self, point: Point) -> bool {
        self.matches.iter().any(|m| m.contains(point))
    }

    /// Check if a point is within the focused match.
    #[must_use]
    pub fn is_focused_match(&self, point: Point) -> bool {
        self.focused_match().is_some_and(|m| m.contains(point))
    }

    /// Clear the search state.
    pub fn clear(&mut self) {
        self.query = None;
        self.matches.clear();
        self.focused_match = None;
    }

    /// Get access to the underlying TerminalSearch.
    #[must_use]
    pub fn terminal_search(&self) -> &TerminalSearch {
        &self.search
    }
}

impl Default for TermSearch {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Term extension methods for search
// ============================================================================

impl<T> Term<T> {
    /// Search for the next occurrence of a pattern.
    ///
    /// Returns the match range if found.
    pub fn search_next(&self, search: &RegexSearch, from: Point) -> Option<Match> {
        if search.is_empty() {
            return None;
        }

        let mut term_search = TermSearch::new();
        term_search.rebuild_index(self.grid());
        term_search.set_query(Some(search.pattern()), self.grid());
        term_search.focus_next(from).copied()
    }

    /// Search for the previous occurrence of a pattern.
    ///
    /// Returns the match range if found.
    pub fn search_prev(&self, search: &RegexSearch, from: Point) -> Option<Match> {
        if search.is_empty() {
            return None;
        }

        let mut term_search = TermSearch::new();
        term_search.rebuild_index(self.grid());
        term_search.set_query(Some(search.pattern()), self.grid());
        term_search.focus_prev(from).copied()
    }

    /// Get all matches for a search pattern.
    pub fn search_all(&self, search: &RegexSearch) -> Vec<Match> {
        if search.is_empty() {
            return Vec::new();
        }

        let mut term_search = TermSearch::new();
        term_search.rebuild_index(self.grid());
        term_search.set_query(Some(search.pattern()), self.grid());
        term_search.matches().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::VoidListener;
    use crate::term::Config;

    fn create_term_with_content() -> Term<VoidListener> {
        let config = Config::default();
        let dims = (24usize, 80usize);
        let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

        term.process(b"Hello, World!\r\n");
        term.process(b"This is a test line.\r\n");
        term.process(b"Another test here.\r\n");
        term.process(b"Final line with test.\r\n");

        term
    }

    #[test]
    fn regex_search_creation() {
        // Empty pattern returns None
        assert!(RegexSearch::new("").is_none());

        // Valid pattern returns Some
        let search = RegexSearch::new("test");
        assert!(search.is_some());
        let search = search.unwrap();
        assert_eq!(search.pattern(), "test");
        assert!(!search.is_empty());
    }

    #[test]
    fn regex_search_default() {
        let search = RegexSearch::default();
        assert!(search.is_empty());
        assert_eq!(search.pattern(), "");
    }

    #[test]
    fn match_struct() {
        let m = Match::new(
            Point::new(Line(5), Column(10)),
            Point::new(Line(5), Column(14)),
        );

        assert_eq!(m.line(), Line(5));

        // Contains checks
        assert!(m.contains(Point::new(Line(5), Column(10))));
        assert!(m.contains(Point::new(Line(5), Column(12))));
        assert!(m.contains(Point::new(Line(5), Column(14))));
        assert!(!m.contains(Point::new(Line(5), Column(9))));
        assert!(!m.contains(Point::new(Line(5), Column(15))));
        assert!(!m.contains(Point::new(Line(4), Column(12))));
    }

    #[test]
    fn term_search_basic() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        assert!(search.is_dirty());
        search.rebuild_index(term.grid());
        assert!(!search.is_dirty());

        search.set_query(Some("test"), term.grid());
        assert!(search.match_count() > 0);
        assert_eq!(search.query(), Some("test"));
    }

    #[test]
    fn term_search_empty_query() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        search.set_query(Some(""), term.grid());

        assert_eq!(search.match_count(), 0);
    }

    #[test]
    fn term_search_no_match() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        search.set_query(Some("xyz123"), term.grid());

        assert_eq!(search.match_count(), 0);
    }

    #[test]
    fn term_search_focus_navigation() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        search.set_query(Some("test"), term.grid());

        // Should have multiple matches
        let count = search.match_count();
        assert!(count >= 2, "Expected at least 2 matches, got {}", count);

        // Focus first match
        let first = search.advance_next();
        assert!(first.is_some());
        assert_eq!(search.focused_match_index(), Some(0));

        // Focus next match
        let second = search.advance_next();
        assert!(second.is_some());
        assert_eq!(search.focused_match_index(), Some(1));

        // Go back
        let back = search.advance_prev();
        assert!(back.is_some());
        assert_eq!(search.focused_match_index(), Some(0));

        // Wrap to last
        let wrap = search.advance_prev();
        assert!(wrap.is_some());
        assert_eq!(search.focused_match_index(), Some(count - 1));
    }

    #[test]
    fn term_search_focus_by_point() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        search.set_query(Some("test"), term.grid());

        // Focus next from beginning
        let from_start = search.focus_next(Point::new(Line(0), Column(0)));
        assert!(from_start.is_some());

        // Focus prev from end
        let from_end = search.focus_prev(Point::new(Line(100), Column(0)));
        assert!(from_end.is_some());
    }

    #[test]
    fn term_search_is_match() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        search.set_query(Some("Hello"), term.grid());

        // First line starts with "Hello"
        if search.match_count() > 0 {
            let first_match = &search.matches()[0];
            assert!(search.is_match(first_match.start));
        }
    }

    #[test]
    fn term_search_clear() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        search.set_query(Some("test"), term.grid());
        assert!(search.match_count() > 0);

        search.clear();
        assert!(search.query().is_none());
        assert_eq!(search.match_count(), 0);
        assert!(search.focused_match().is_none());
    }

    #[test]
    fn term_search_mark_dirty() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        assert!(!search.is_dirty());

        search.mark_dirty();
        assert!(search.is_dirty());
    }

    #[test]
    fn term_search_next() {
        let term = create_term_with_content();
        let search = RegexSearch::new("test").unwrap();

        let result = term.search_next(&search, Point::new(Line(0), Column(0)));
        assert!(result.is_some());
    }

    #[test]
    fn term_search_prev() {
        let term = create_term_with_content();
        let search = RegexSearch::new("test").unwrap();

        let result = term.search_prev(&search, Point::new(Line(100), Column(0)));
        assert!(result.is_some());
    }

    #[test]
    fn term_search_all() {
        let term = create_term_with_content();
        let search = RegexSearch::new("test").unwrap();

        let matches = term.search_all(&search);
        assert!(matches.len() >= 2);
    }

    #[test]
    fn term_search_empty_pattern() {
        let term = create_term_with_content();
        let search = RegexSearch::default();

        let result = term.search_next(&search, Point::new(Line(0), Column(0)));
        assert!(result.is_none());

        let matches = term.search_all(&search);
        assert!(matches.is_empty());
    }

    #[test]
    fn is_focused_match() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        search.set_query(Some("Hello"), term.grid());

        if search.match_count() > 0 {
            // Focus the first match
            let _ = search.advance_next();

            let focused = search.focused_match().unwrap();
            assert!(search.is_focused_match(focused.start));

            // Some other point should not be the focused match
            // (unless it happens to be in the focused match)
            let other = Point::new(Line(100), Column(0));
            assert!(!search.is_focused_match(other));
        }
    }

    #[test]
    fn focus_match_by_index() {
        let term = create_term_with_content();
        let mut search = TermSearch::new();

        search.rebuild_index(term.grid());
        search.set_query(Some("test"), term.grid());

        let count = search.match_count();
        if count >= 2 {
            search.focus_match(1);
            assert_eq!(search.focused_match_index(), Some(1));

            // Out of bounds should not change
            search.focus_match(1000);
            assert_eq!(search.focused_match_index(), Some(1));
        }
    }
}
