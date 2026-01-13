//! Streaming search with memory-bounded results.
//!
//! ## Design (from TLA+ spec: StreamingSearch.tla)
//!
//! This module implements a streaming search system that:
//! - Searches through content incrementally (row by row)
//! - Bounds memory usage with configurable result limits
//! - Supports multiple filter modes: Literal, Regex, Fuzzy
//! - Provides navigation with optional wraparound
//! - Handles dynamic content changes (additions/invalidations)
//!
//! ## Safety Invariants (from TLA+ specification)
//!
//! | ID | Invariant | Description |
//! |----|-----------|-------------|
//! | INV-SEARCH-1 | `CurrentIndexValid` | Current match index always valid |
//! | INV-SEARCH-2 | `ResultPositionsValid` | All result positions are valid grid coords |
//! | INV-SEARCH-3 | `MemoryBounded` | Result count never exceeds MaxResults |
//! | INV-SEARCH-4 | `NoDuplicateResults` | No duplicate results in result set |
//! | INV-SEARCH-5 | `ScanProgressConsistent` | Scan progress consistent with state |
//! | INV-SEARCH-6 | `TotalMatchesConsistent` | Total matches >= stored results |
//!
//! ## State Machine
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                                                             │
//! │  ┌──────┐  StartSearch   ┌───────────┐  ScanComplete       │
//! │  │ Idle │ ─────────────▶ │ Searching │ ──────────────┐     │
//! │  └──────┘                └───────────┘               │     │
//! │      ▲                        │                      ▼     │
//! │      │     Cancel             │ ScanComplete    ┌─────────┐│
//! │      ├────────────────────────┤                 │HasResult││
//! │      │                        │                 └─────────┘│
//! │      │     Cancel             ▼                      │     │
//! │      ├───────────────── ┌───────────┐               │     │
//! │      │                  │ NoResults │ ◀─────────────┘     │
//! │      │                  └───────────┘  (results empty)    │
//! │      │                        │                            │
//! │      └────────────────────────┘                            │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashSet;

use crate::grid::Grid;
use crate::scrollback::Scrollback;

/// Search state (from TLA+ SearchStates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchState {
    /// No active search.
    Idle,
    /// Currently scanning rows.
    Searching,
    /// Search completed with results.
    HasResults,
    /// Search completed with no results.
    NoResults,
}

/// Filter mode for pattern matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    /// Exact literal string match.
    #[default]
    Literal,
    /// Regular expression match.
    Regex,
    /// Fuzzy/approximate match.
    Fuzzy,
}

/// Navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Navigate forward (toward newer matches).
    #[default]
    Forward,
    /// Navigate backward (toward older matches).
    Backward,
}

/// A match record (from TLA+ Match type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingMatch {
    /// Row index (0-indexed from oldest).
    pub row: usize,
    /// Starting column (0-indexed).
    pub start_col: usize,
    /// Ending column (exclusive).
    pub end_col: usize,
    /// Match length in characters.
    pub match_len: usize,
}

impl StreamingMatch {
    /// Create a new match record.
    #[must_use]
    pub fn new(row: usize, start_col: usize, end_col: usize) -> Self {
        let match_len = end_col.saturating_sub(start_col);
        Self {
            row,
            start_col,
            end_col,
            match_len,
        }
    }

    /// Check if this match overlaps with another at the same position.
    #[must_use]
    pub fn same_position(&self, other: &Self) -> bool {
        self.row == other.row && self.start_col == other.start_col
    }
}

/// Streaming search configuration.
#[derive(Debug, Clone)]
pub struct StreamingSearchConfig {
    /// Maximum number of stored results (memory bound).
    pub max_results: usize,
    /// Maximum pattern length.
    pub max_pattern_len: usize,
    /// Enable wraparound navigation.
    pub wrap_enabled: bool,
    /// Case-sensitive matching.
    pub case_sensitive: bool,
    /// Highlight all matches (vs just current).
    pub highlight_all: bool,
}

impl Default for StreamingSearchConfig {
    fn default() -> Self {
        Self {
            max_results: 10_000,
            max_pattern_len: 1024,
            wrap_enabled: true,
            case_sensitive: false,
            highlight_all: true,
        }
    }
}

/// Content provider trait for streaming search.
///
/// Implement this to allow streaming search over different content sources.
pub trait SearchContent {
    /// Get the total number of rows.
    fn row_count(&self) -> usize;

    /// Get the text content of a specific row.
    fn get_row_text(&self, row: usize) -> Option<String>;
}

impl SearchContent for Scrollback {
    fn row_count(&self) -> usize {
        self.line_count()
    }

    fn get_row_text(&self, row: usize) -> Option<String> {
        self.get_line(row).map(|line| line.to_string())
    }
}

impl SearchContent for Grid {
    fn row_count(&self) -> usize {
        self.scrollback_lines() + usize::from(self.rows())
    }

    fn get_row_text(&self, row: usize) -> Option<String> {
        let scrollback_lines = self.scrollback_lines();
        if row < scrollback_lines {
            return self.get_history_line(row).map(|line| line.to_string());
        }

        let visible_idx = row.saturating_sub(scrollback_lines);
        if visible_idx >= usize::from(self.rows()) {
            return None;
        }

        let row_u16 = u16::try_from(visible_idx).ok()?;
        self.row_text(row_u16)
    }
}

/// Streaming search engine with memory-bounded results.
///
/// Implements the StreamingSearch.tla specification.
#[derive(Debug)]
pub struct StreamingSearch {
    /// Current search state.
    state: SearchState,
    /// Filter mode.
    filter_mode: FilterMode,
    /// Current search pattern.
    pattern: String,
    /// Compiled regex (if filter mode is Regex).
    #[cfg(feature = "regex")]
    compiled_regex: Option<regex::Regex>,
    /// Search results (bounded by max_results).
    results: Vec<StreamingMatch>,
    /// Current highlighted result index (1-based, 0 = none).
    current_index: usize,
    /// Row currently being scanned (-1 = not scanning).
    scan_progress: isize,
    /// Total matches found (may exceed stored results).
    total_matches: usize,
    /// Search direction.
    search_direction: Direction,
    /// Configuration.
    config: StreamingSearchConfig,
    /// Deduplication set (row, start_col).
    seen_positions: HashSet<(usize, usize)>,
}

impl StreamingSearch {
    /// Create a new streaming search engine.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(StreamingSearchConfig::default())
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(config: StreamingSearchConfig) -> Self {
        Self {
            state: SearchState::Idle,
            filter_mode: FilterMode::Literal,
            pattern: String::new(),
            #[cfg(feature = "regex")]
            compiled_regex: None,
            results: Vec::new(),
            current_index: 0,
            scan_progress: -1,
            total_matches: 0,
            search_direction: Direction::Forward,
            config,
            seen_positions: HashSet::new(),
        }
    }

    /// Get the current search state.
    #[must_use]
    pub fn state(&self) -> SearchState {
        self.state
    }

    /// Get the current filter mode.
    #[must_use]
    pub fn filter_mode(&self) -> FilterMode {
        self.filter_mode
    }

    /// Get the current pattern.
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Get the search results.
    #[must_use]
    pub fn results(&self) -> &[StreamingMatch] {
        &self.results
    }

    /// Get the current match index (1-based, 0 = none).
    #[must_use]
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Get the currently highlighted match.
    #[must_use]
    pub fn current_match(&self) -> Option<&StreamingMatch> {
        if self.current_index > 0 && self.current_index <= self.results.len() {
            Some(&self.results[self.current_index - 1])
        } else {
            None
        }
    }

    /// Get the scan progress (row being scanned, -1 if not scanning).
    #[must_use]
    pub fn scan_progress(&self) -> isize {
        self.scan_progress
    }

    /// Get the total number of matches found (may exceed stored).
    #[must_use]
    pub fn total_matches(&self) -> usize {
        self.total_matches
    }

    /// Get the number of stored results.
    #[must_use]
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Check if wrap-around navigation is enabled.
    #[must_use]
    pub fn wrap_enabled(&self) -> bool {
        self.config.wrap_enabled
    }

    /// Check if case-sensitive matching is enabled.
    #[must_use]
    pub fn case_sensitive(&self) -> bool {
        self.config.case_sensitive
    }

    /// Check if all matches should be highlighted.
    #[must_use]
    pub fn highlight_all(&self) -> bool {
        self.config.highlight_all
    }

    // ========================================================================
    // Search Operations (from TLA+ spec)
    // ========================================================================

    /// Start a new search with the given pattern and mode.
    ///
    /// Corresponds to TLA+ `StartSearch` action.
    pub fn start_search(&mut self, pattern: &str, mode: FilterMode) -> Result<(), SearchError> {
        if pattern.is_empty() {
            return Err(SearchError::EmptyPattern);
        }

        if pattern.len() > self.config.max_pattern_len {
            return Err(SearchError::PatternTooLong);
        }

        // Compile regex if needed
        #[cfg(feature = "regex")]
        if mode == FilterMode::Regex {
            match regex::Regex::new(pattern) {
                Ok(re) => self.compiled_regex = Some(re),
                Err(e) => return Err(SearchError::InvalidRegex(e.to_string())),
            }
        }

        self.pattern = pattern.to_string();
        self.filter_mode = mode;
        self.state = SearchState::Searching;
        self.results.clear();
        self.seen_positions.clear();
        self.current_index = 0;
        self.scan_progress = 0;
        self.total_matches = 0;

        Ok(())
    }

    /// Update the pattern incrementally (as user types).
    ///
    /// Corresponds to TLA+ `UpdatePattern` action.
    pub fn update_pattern(&mut self, new_pattern: &str) -> Result<(), SearchError> {
        if !matches!(
            self.state,
            SearchState::Searching | SearchState::HasResults | SearchState::NoResults
        ) {
            return Err(SearchError::InvalidState);
        }

        if new_pattern == self.pattern {
            return Ok(()); // No change
        }

        if new_pattern.len() > self.config.max_pattern_len {
            return Err(SearchError::PatternTooLong);
        }

        self.pattern = new_pattern.to_string();

        if new_pattern.is_empty() {
            // Pattern cleared - reset to idle
            self.state = SearchState::Idle;
            self.results.clear();
            self.seen_positions.clear();
            self.current_index = 0;
            self.scan_progress = -1;
            self.total_matches = 0;
            #[cfg(feature = "regex")]
            {
                self.compiled_regex = None;
            }
        } else {
            // Pattern changed - restart search
            #[cfg(feature = "regex")]
            if self.filter_mode == FilterMode::Regex {
                match regex::Regex::new(new_pattern) {
                    Ok(re) => self.compiled_regex = Some(re),
                    Err(e) => return Err(SearchError::InvalidRegex(e.to_string())),
                }
            }

            self.state = SearchState::Searching;
            self.results.clear();
            self.seen_positions.clear();
            self.current_index = 0;
            self.scan_progress = 0;
            self.total_matches = 0;
        }

        Ok(())
    }

    /// Scan a single row for matches.
    ///
    /// Corresponds to TLA+ `ScanRow` action.
    /// Returns the number of matches found in this row.
    pub fn scan_row(&mut self, row: usize, text: &str, max_rows: usize) -> usize {
        if self.state != SearchState::Searching {
            return 0;
        }

        // Row count is bounded by terminal dimensions (<<2^31)
        #[allow(clippy::cast_possible_wrap)]
        if self.scan_progress != row as isize {
            return 0;
        }

        if row >= max_rows {
            return 0;
        }

        // Find matches in this row
        let matches = self.find_matches_in_row(row, text);
        let match_count = matches.len();

        // Add matches (respecting memory bound)
        for m in matches {
            let pos_key = (m.row, m.start_col);

            // INV-SEARCH-4: No duplicate results
            if self.seen_positions.contains(&pos_key) {
                // Skip duplicates
            } else if self.results.len() >= self.config.max_results {
                // INV-SEARCH-3: Memory bounded - at capacity, count but don't store
                self.total_matches += 1;
            } else {
                self.seen_positions.insert(pos_key);
                self.results.push(m);
                self.total_matches += 1;
            }
        }

        // Advance scan progress
        // Row count is bounded by terminal dimensions (<<2^31)
        #[allow(clippy::cast_possible_wrap)]
        {
            self.scan_progress = (row + 1) as isize;
        }

        // Check if scan is complete
        if row + 1 >= max_rows {
            self.complete_search();
        }

        match_count
    }

    /// Scan all content in one call.
    ///
    /// Convenience method that scans all rows from a content provider.
    pub fn scan_all<C: SearchContent>(&mut self, content: &C) {
        let max_rows = content.row_count();

        while self.state == SearchState::Searching {
            // scan_progress is >=0 during Searching state, so cast is safe
            #[allow(clippy::cast_sign_loss)]
            let row = self.scan_progress as usize;
            if row >= max_rows {
                self.complete_search();
                break;
            }

            if let Some(text) = content.get_row_text(row) {
                self.scan_row(row, &text, max_rows);
            } else {
                // Skip missing rows
                // Row count is bounded by terminal dimensions (<<2^31)
                #[allow(clippy::cast_possible_wrap)]
                {
                    self.scan_progress = (row + 1) as isize;
                }
                if row + 1 >= max_rows {
                    self.complete_search();
                }
            }
        }
    }

    /// Complete the search scan.
    ///
    /// Corresponds to TLA+ `CompleteSearch` action.
    fn complete_search(&mut self) {
        if self.state != SearchState::Searching {
            return;
        }

        if self.results.is_empty() {
            self.state = SearchState::NoResults;
            self.current_index = 0;
        } else {
            self.state = SearchState::HasResults;
            self.current_index = 1;
        }

        self.scan_progress = -1;
    }

    /// Cancel the current search.
    ///
    /// Corresponds to TLA+ `CancelSearch` action.
    pub fn cancel(&mut self) {
        if !matches!(
            self.state,
            SearchState::Searching | SearchState::HasResults | SearchState::NoResults
        ) {
            return;
        }

        self.state = SearchState::Idle;
        self.pattern.clear();
        self.results.clear();
        self.seen_positions.clear();
        self.current_index = 0;
        self.scan_progress = -1;
        self.total_matches = 0;
        #[cfg(feature = "regex")]
        {
            self.compiled_regex = None;
        }
    }

    // ========================================================================
    // Navigation Operations
    // ========================================================================

    /// Navigate to the next match.
    ///
    /// Corresponds to TLA+ `NextMatch` action.
    pub fn next_match(&mut self) {
        if self.state != SearchState::HasResults || self.results.is_empty() {
            return;
        }

        self.current_index = Self::next_index(
            self.current_index,
            self.results.len(),
            Direction::Forward,
            self.config.wrap_enabled,
        );
    }

    /// Navigate to the previous match.
    ///
    /// Corresponds to TLA+ `PrevMatch` action.
    pub fn prev_match(&mut self) {
        if self.state != SearchState::HasResults || self.results.is_empty() {
            return;
        }

        self.current_index = Self::next_index(
            self.current_index,
            self.results.len(),
            Direction::Backward,
            self.config.wrap_enabled,
        );
    }

    /// Jump to a specific match index (1-based).
    ///
    /// Corresponds to TLA+ `JumpToMatch` action.
    pub fn jump_to_match(&mut self, index: usize) {
        if self.state != SearchState::HasResults {
            return;
        }

        if index >= 1 && index <= self.results.len() {
            self.current_index = index;
        }
    }

    /// Calculate next index with wraparound (from TLA+ NextIndex).
    fn next_index(idx: usize, len: usize, dir: Direction, wrap: bool) -> usize {
        if len == 0 {
            return 0;
        }

        match dir {
            Direction::Forward => {
                if idx >= len {
                    if wrap {
                        1
                    } else {
                        idx
                    }
                } else {
                    idx + 1
                }
            }
            Direction::Backward => {
                if idx <= 1 {
                    if wrap {
                        len
                    } else {
                        idx
                    }
                } else {
                    idx - 1
                }
            }
        }
    }

    // ========================================================================
    // Configuration Operations
    // ========================================================================

    /// Set the search direction.
    pub fn set_direction(&mut self, dir: Direction) {
        self.search_direction = dir;
    }

    /// Toggle wrap-around navigation.
    pub fn toggle_wrap(&mut self) {
        self.config.wrap_enabled = !self.config.wrap_enabled;
    }

    /// Toggle case sensitivity.
    ///
    /// Note: Changing case sensitivity requires re-search.
    pub fn toggle_case_sensitive(&mut self) {
        self.config.case_sensitive = !self.config.case_sensitive;

        // Re-search if we have a pattern
        if matches!(self.state, SearchState::HasResults | SearchState::NoResults)
            && !self.pattern.is_empty()
        {
            self.state = SearchState::Searching;
            self.results.clear();
            self.seen_positions.clear();
            self.current_index = 0;
            self.scan_progress = 0;
            self.total_matches = 0;
        }
    }

    /// Toggle highlight all matches.
    pub fn toggle_highlight_all(&mut self) {
        self.config.highlight_all = !self.config.highlight_all;
    }

    /// Set the filter mode.
    ///
    /// Note: Changing mode requires re-search.
    pub fn set_filter_mode(&mut self, mode: FilterMode) -> Result<(), SearchError> {
        if mode == self.filter_mode {
            return Ok(());
        }

        // Compile new regex if switching to regex mode
        #[cfg(feature = "regex")]
        if mode == FilterMode::Regex && !self.pattern.is_empty() {
            match regex::Regex::new(&self.pattern) {
                Ok(re) => self.compiled_regex = Some(re),
                Err(e) => return Err(SearchError::InvalidRegex(e.to_string())),
            }
        }

        self.filter_mode = mode;

        // Re-search if we have a pattern
        if matches!(self.state, SearchState::HasResults | SearchState::NoResults)
            && !self.pattern.is_empty()
        {
            self.state = SearchState::Searching;
            self.results.clear();
            self.seen_positions.clear();
            self.current_index = 0;
            self.scan_progress = 0;
            self.total_matches = 0;
        }

        Ok(())
    }

    // ========================================================================
    // Content Change Handling
    // ========================================================================

    /// Handle new content added to terminal.
    ///
    /// Corresponds to TLA+ `ContentAdded` action.
    pub fn content_added(&mut self, row: usize, text: &str) {
        if self.state != SearchState::HasResults {
            return;
        }

        // Find new matches
        let matches = self.find_matches_in_row(row, text);

        for m in matches {
            let pos_key = (m.row, m.start_col);

            // Skip duplicates
            if self.seen_positions.contains(&pos_key) {
                continue;
            }

            // Skip if at capacity
            if self.results.len() >= self.config.max_results {
                self.total_matches += 1;
                continue;
            }

            self.seen_positions.insert(pos_key);
            self.results.push(m);
            self.total_matches += 1;
        }

        // Set current index if this is the first result
        if self.current_index == 0 && !self.results.is_empty() {
            self.current_index = 1;
        }
    }

    /// Handle content being invalidated (scrolled out, cleared).
    ///
    /// Corresponds to TLA+ `ContentInvalidated` action.
    pub fn content_invalidated(&mut self, from_row: usize, to_row: usize) {
        if !matches!(self.state, SearchState::HasResults | SearchState::NoResults) {
            return;
        }

        // Remove results in the invalidated range
        self.results.retain(|m| m.row < from_row || m.row > to_row);

        // Update dedup set
        self.seen_positions
            .retain(|(row, _)| *row < from_row || *row > to_row);

        if self.results.is_empty() {
            self.state = SearchState::NoResults;
            self.current_index = 0;
        } else if self.current_index > self.results.len() {
            self.current_index = self.results.len();
        }
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Find matches in a single row.
    fn find_matches_in_row(&self, row: usize, text: &str) -> Vec<StreamingMatch> {
        let mut matches = Vec::new();

        if self.pattern.is_empty() {
            return matches;
        }

        let search_text: std::borrow::Cow<'_, str>;
        let search_pattern: std::borrow::Cow<'_, str>;

        if self.config.case_sensitive {
            search_text = std::borrow::Cow::Borrowed(text);
            search_pattern = std::borrow::Cow::Borrowed(&self.pattern);
        } else {
            search_text = std::borrow::Cow::Owned(text.to_lowercase());
            search_pattern = std::borrow::Cow::Owned(self.pattern.to_lowercase());
        }

        match self.filter_mode {
            FilterMode::Literal => {
                // Literal string search
                let mut start = 0;
                while let Some(pos) = search_text[start..].find(search_pattern.as_ref()) {
                    let abs_pos = start + pos;
                    matches.push(StreamingMatch::new(
                        row,
                        abs_pos,
                        abs_pos + self.pattern.len(),
                    ));
                    start = abs_pos + 1;
                }
            }
            FilterMode::Regex => {
                #[cfg(feature = "regex")]
                if let Some(ref re) = self.compiled_regex {
                    for cap in re.find_iter(text) {
                        matches.push(StreamingMatch::new(row, cap.start(), cap.end()));
                    }
                }
                #[cfg(not(feature = "regex"))]
                {
                    // Fall back to literal if regex feature not enabled
                    let mut start = 0;
                    while let Some(pos) = search_text[start..].find(search_pattern.as_ref()) {
                        let abs_pos = start + pos;
                        matches.push(StreamingMatch::new(
                            row,
                            abs_pos,
                            abs_pos + self.pattern.len(),
                        ));
                        start = abs_pos + 1;
                    }
                }
            }
            FilterMode::Fuzzy => {
                // Simple fuzzy matching: check if all pattern chars appear in order
                if Self::fuzzy_match(&search_text, &search_pattern) {
                    // For fuzzy matches, highlight the entire text where match found
                    // This is a simplification - a real implementation would track
                    // individual character positions
                    matches.push(StreamingMatch::new(row, 0, text.len()));
                }
            }
        }

        matches
    }

    /// Simple fuzzy match: check if all pattern characters appear in text in order.
    fn fuzzy_match(text: &str, pattern: &str) -> bool {
        let mut text_chars = text.chars();
        for p in pattern.chars() {
            loop {
                match text_chars.next() {
                    Some(t) if t == p => break,
                    Some(_) => {}
                    None => return false,
                }
            }
        }
        true
    }

    // ========================================================================
    // Invariant Verification (for testing)
    // ========================================================================

    /// Verify INV-SEARCH-1: Current index is valid.
    #[must_use]
    pub fn verify_current_index_valid(&self) -> bool {
        self.current_index == 0 || self.current_index <= self.results.len()
    }

    /// Verify INV-SEARCH-3: Memory bounded.
    #[must_use]
    pub fn verify_memory_bounded(&self) -> bool {
        self.results.len() <= self.config.max_results
    }

    /// Verify INV-SEARCH-4: No duplicate results.
    #[must_use]
    pub fn verify_no_duplicates(&self) -> bool {
        let mut seen = HashSet::new();
        for m in &self.results {
            if !seen.insert((m.row, m.start_col)) {
                return false;
            }
        }
        true
    }

    /// Verify INV-SEARCH-5: Scan progress consistent with state.
    #[must_use]
    pub fn verify_scan_progress_consistent(&self) -> bool {
        match self.state {
            SearchState::Idle => self.scan_progress == -1,
            SearchState::Searching => self.scan_progress >= 0,
            SearchState::HasResults | SearchState::NoResults => self.scan_progress == -1,
        }
    }

    /// Verify INV-SEARCH-6: Total matches >= stored results.
    #[must_use]
    pub fn verify_total_matches_consistent(&self) -> bool {
        self.total_matches >= self.results.len()
    }

    /// Verify all safety invariants.
    #[must_use]
    pub fn verify_all_invariants(&self) -> bool {
        self.verify_current_index_valid()
            && self.verify_memory_bounded()
            && self.verify_no_duplicates()
            && self.verify_scan_progress_consistent()
            && self.verify_total_matches_consistent()
    }
}

impl Default for StreamingSearch {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during streaming search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// Empty pattern provided.
    EmptyPattern,
    /// Pattern exceeds maximum length.
    PatternTooLong,
    /// Invalid regex pattern.
    InvalidRegex(String),
    /// Operation not valid in current state.
    InvalidState,
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::EmptyPattern => write!(f, "empty search pattern"),
            SearchError::PatternTooLong => write!(f, "pattern exceeds maximum length"),
            SearchError::InvalidRegex(msg) => write!(f, "invalid regex: {msg}"),
            SearchError::InvalidState => write!(f, "operation not valid in current state"),
        }
    }
}

impl std::error::Error for SearchError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple content provider for testing.
    struct TestContent {
        lines: Vec<String>,
    }

    impl TestContent {
        fn new(lines: Vec<&str>) -> Self {
            Self {
                lines: lines.into_iter().map(String::from).collect(),
            }
        }
    }

    impl SearchContent for TestContent {
        fn row_count(&self) -> usize {
            self.lines.len()
        }

        fn get_row_text(&self, row: usize) -> Option<String> {
            self.lines.get(row).cloned()
        }
    }

    #[test]
    fn new_search_is_idle() {
        let search = StreamingSearch::new();
        assert_eq!(search.state(), SearchState::Idle);
        assert_eq!(search.current_index(), 0);
        assert!(search.results().is_empty());
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn start_search_transitions_to_searching() {
        let mut search = StreamingSearch::new();
        search.start_search("hello", FilterMode::Literal).unwrap();

        assert_eq!(search.state(), SearchState::Searching);
        assert_eq!(search.pattern(), "hello");
        assert_eq!(search.scan_progress(), 0);
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn empty_pattern_returns_error() {
        let mut search = StreamingSearch::new();
        let result = search.start_search("", FilterMode::Literal);
        assert_eq!(result, Err(SearchError::EmptyPattern));
    }

    #[test]
    fn scan_finds_matches() {
        let mut search = StreamingSearch::new();
        search.start_search("hello", FilterMode::Literal).unwrap();

        let count = search.scan_row(0, "hello world", 10);
        assert_eq!(count, 1);
        assert_eq!(search.results().len(), 1);
        assert_eq!(search.results()[0].row, 0);
        assert_eq!(search.results()[0].start_col, 0);
        assert_eq!(search.results()[0].end_col, 5);
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn scan_all_completes_search() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["hello world", "goodbye world", "hello again"]);

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.state(), SearchState::HasResults);
        assert_eq!(search.results().len(), 2);
        assert_eq!(search.current_index(), 1);
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn scan_scrollback_tiers() {
        let mut scrollback = Scrollback::with_block_size(2, 2, 1024 * 1024, 2);
        for i in 0..6 {
            scrollback.push_str(&format!("line {i}"));
        }

        assert!(scrollback.cold_line_count() > 0);

        let mut search = StreamingSearch::new();
        search.start_search("line 0", FilterMode::Literal).unwrap();
        search.scan_all(&scrollback);

        assert_eq!(search.state(), SearchState::HasResults);
        assert!(search.results().iter().any(|m| m.row == 0));
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn no_matches_transitions_to_no_results() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["foo", "bar", "baz"]);

        search.start_search("xyz", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.state(), SearchState::NoResults);
        assert!(search.results().is_empty());
        assert_eq!(search.current_index(), 0);
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn navigation_works() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["match here", "match there", "match everywhere"]);

        search.start_search("match", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.current_index(), 1);

        search.next_match();
        assert_eq!(search.current_index(), 2);

        search.next_match();
        assert_eq!(search.current_index(), 3);

        // Wrap around
        search.next_match();
        assert_eq!(search.current_index(), 1);

        search.prev_match();
        assert_eq!(search.current_index(), 3);

        assert!(search.verify_all_invariants());
    }

    #[test]
    fn navigation_without_wrap() {
        let mut search = StreamingSearch::with_config(StreamingSearchConfig {
            wrap_enabled: false,
            ..Default::default()
        });
        let content = TestContent::new(vec!["match", "match"]);

        search.start_search("match", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.current_index(), 1);

        search.next_match();
        assert_eq!(search.current_index(), 2);

        // Should not wrap
        search.next_match();
        assert_eq!(search.current_index(), 2);

        search.prev_match();
        assert_eq!(search.current_index(), 1);

        // Should not wrap backwards
        search.prev_match();
        assert_eq!(search.current_index(), 1);
    }

    #[test]
    fn cancel_clears_state() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["hello"]);

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.state(), SearchState::HasResults);

        search.cancel();

        assert_eq!(search.state(), SearchState::Idle);
        assert!(search.pattern().is_empty());
        assert!(search.results().is_empty());
        assert_eq!(search.current_index(), 0);
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn memory_bounded() {
        let mut search = StreamingSearch::with_config(StreamingSearchConfig {
            max_results: 5,
            ..Default::default()
        });

        search.start_search("a", FilterMode::Literal).unwrap();

        // Create content with many matches
        for i in 0..20 {
            search.scan_row(i, "a a a a a", 20);
        }

        // Should be capped at 5 results
        assert!(search.results().len() <= 5);
        assert!(search.total_matches() > 5);
        assert!(search.verify_memory_bounded());
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn no_duplicate_results() {
        let mut search = StreamingSearch::new();

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_row(0, "hello hello", 10);

        // Should find 2 distinct matches
        assert_eq!(search.results().len(), 2);
        assert_eq!(search.results()[0].start_col, 0);
        assert_eq!(search.results()[1].start_col, 6);
        assert!(search.verify_no_duplicates());
    }

    #[test]
    fn case_insensitive_search() {
        let mut search = StreamingSearch::with_config(StreamingSearchConfig {
            case_sensitive: false,
            ..Default::default()
        });

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_row(0, "HELLO World HeLLo", 10);

        assert_eq!(search.results().len(), 2);
    }

    #[test]
    fn case_sensitive_search() {
        let mut search = StreamingSearch::with_config(StreamingSearchConfig {
            case_sensitive: true,
            ..Default::default()
        });

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_row(0, "HELLO World hello", 10);

        assert_eq!(search.results().len(), 1);
        assert_eq!(search.results()[0].start_col, 12);
    }

    #[test]
    fn fuzzy_match() {
        let mut search = StreamingSearch::new();

        search.start_search("hlo", FilterMode::Fuzzy).unwrap();
        search.scan_row(0, "hello world", 10);

        // "hlo" fuzzy matches "hello" (h...l...o in order)
        assert_eq!(search.results().len(), 1);
    }

    #[test]
    fn fuzzy_no_match() {
        let mut search = StreamingSearch::new();

        search.start_search("xyz", FilterMode::Fuzzy).unwrap();
        search.scan_row(0, "hello world", 10);

        assert!(search.results().is_empty());
    }

    #[test]
    fn update_pattern_restarts_search() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["hello", "world"]);

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.results().len(), 1);

        search.update_pattern("world").unwrap();
        assert_eq!(search.state(), SearchState::Searching);
        assert!(search.results().is_empty());

        search.scan_all(&content);
        assert_eq!(search.results().len(), 1);
        assert_eq!(search.results()[0].row, 1);
    }

    #[test]
    fn clear_pattern_returns_to_idle() {
        let mut search = StreamingSearch::new();

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.update_pattern("").unwrap();

        assert_eq!(search.state(), SearchState::Idle);
    }

    #[test]
    fn content_added_finds_new_matches() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["hello"]);

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.results().len(), 1);

        // Add new content with a match
        search.content_added(1, "hello again");

        assert_eq!(search.results().len(), 2);
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn content_invalidated_removes_matches() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["hello 0", "hello 1", "hello 2"]);

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.results().len(), 3);

        // Invalidate rows 0-1
        search.content_invalidated(0, 1);

        assert_eq!(search.results().len(), 1);
        assert_eq!(search.results()[0].row, 2);
        assert!(search.verify_all_invariants());
    }

    #[test]
    fn toggle_case_sensitive_restarts_search() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["HELLO"]);

        search.start_search("hello", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.results().len(), 1); // Case insensitive

        search.toggle_case_sensitive();
        assert_eq!(search.state(), SearchState::Searching);

        search.scan_all(&content);
        assert!(search.results().is_empty()); // Case sensitive, no match
    }

    #[test]
    fn jump_to_match() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["match", "match", "match"]);

        search.start_search("match", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        assert_eq!(search.current_index(), 1);

        search.jump_to_match(3);
        assert_eq!(search.current_index(), 3);

        search.jump_to_match(2);
        assert_eq!(search.current_index(), 2);

        // Invalid index ignored
        search.jump_to_match(10);
        assert_eq!(search.current_index(), 2);

        search.jump_to_match(0);
        assert_eq!(search.current_index(), 2);
    }

    #[test]
    fn current_match_returns_correct_match() {
        let mut search = StreamingSearch::new();
        let content = TestContent::new(vec!["hello world"]);

        search.start_search("world", FilterMode::Literal).unwrap();
        search.scan_all(&content);

        let current = search.current_match().unwrap();
        assert_eq!(current.row, 0);
        assert_eq!(current.start_col, 6);
    }

    #[test]
    fn scan_progress_consistent() {
        let mut search = StreamingSearch::new();

        // Idle state
        assert_eq!(search.scan_progress(), -1);
        assert!(search.verify_scan_progress_consistent());

        // Searching state
        search.start_search("hello", FilterMode::Literal).unwrap();
        assert!(search.scan_progress() >= 0);
        assert!(search.verify_scan_progress_consistent());

        // HasResults state
        search.scan_row(0, "hello", 1);
        assert_eq!(search.state(), SearchState::HasResults);
        assert_eq!(search.scan_progress(), -1);
        assert!(search.verify_scan_progress_consistent());
    }

    #[test]
    fn all_invariants_hold_after_operations() {
        let mut search = StreamingSearch::new();

        // After creation
        assert!(search.verify_all_invariants());

        // After start
        search.start_search("test", FilterMode::Literal).unwrap();
        assert!(search.verify_all_invariants());

        // After scanning
        search.scan_row(0, "test content test", 5);
        assert!(search.verify_all_invariants());

        search.scan_row(1, "more test here", 5);
        assert!(search.verify_all_invariants());

        // Complete search
        search.scan_row(2, "no match", 5);
        search.scan_row(3, "test again", 5);
        search.scan_row(4, "final test", 5);
        assert!(search.verify_all_invariants());

        // After navigation
        search.next_match();
        assert!(search.verify_all_invariants());

        search.prev_match();
        assert!(search.verify_all_invariants());

        // After content changes
        search.content_added(5, "new test row");
        assert!(search.verify_all_invariants());

        search.content_invalidated(0, 1);
        assert!(search.verify_all_invariants());

        // After cancel
        search.cancel();
        assert!(search.verify_all_invariants());
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// INV-SEARCH-1: Current index is always valid.
    #[kani::proof]
    fn current_index_always_valid() {
        let mut search = StreamingSearch::new();

        // Symbolically choose operations
        let start: bool = kani::any();
        let scan_count: usize = kani::any();
        let nav_count: usize = kani::any();

        kani::assume(scan_count <= 5);
        kani::assume(nav_count <= 5);

        if start {
            let _ = search.start_search("test", FilterMode::Literal);

            for i in 0..scan_count {
                search.scan_row(i, "test content", 10);
            }

            for _ in 0..nav_count {
                if kani::any() {
                    search.next_match();
                } else {
                    search.prev_match();
                }
            }
        }

        kani::assert(
            search.verify_current_index_valid(),
            "INV-SEARCH-1 violated: current index invalid",
        );
    }

    /// INV-SEARCH-3: Memory is always bounded.
    #[kani::proof]
    #[kani::unwind(12)]
    fn memory_always_bounded() {
        let mut search = StreamingSearch::with_config(StreamingSearchConfig {
            max_results: 5,
            ..Default::default()
        });

        let _ = search.start_search("a", FilterMode::Literal);

        let scan_count: usize = kani::any();
        kani::assume(scan_count <= 10);

        for i in 0..scan_count {
            search.scan_row(i, "a a a a a", 20);
        }

        kani::assert(
            search.verify_memory_bounded(),
            "INV-SEARCH-3 violated: memory not bounded",
        );
    }

    /// INV-SEARCH-6: Total matches >= stored results.
    #[kani::proof]
    #[kani::unwind(12)]
    fn total_matches_consistent() {
        let mut search = StreamingSearch::with_config(StreamingSearchConfig {
            max_results: 3,
            ..Default::default()
        });

        let _ = search.start_search("x", FilterMode::Literal);

        let scan_count: usize = kani::any();
        kani::assume(scan_count <= 10);

        for i in 0..scan_count {
            search.scan_row(i, "x x x", 20);
        }

        kani::assert(
            search.verify_total_matches_consistent(),
            "INV-SEARCH-6 violated: total matches < stored results",
        );
    }

    /// Cancel always returns to idle with cleared state.
    #[kani::proof]
    fn cancel_clears_state() {
        let mut search = StreamingSearch::new();

        // Do some operations
        let _ = search.start_search("test", FilterMode::Literal);
        search.scan_row(0, "test", 5);

        search.cancel();

        kani::assert(
            search.state() == SearchState::Idle,
            "state not idle after cancel",
        );
        kani::assert(
            search.pattern().is_empty(),
            "pattern not empty after cancel",
        );
        kani::assert(
            search.results().is_empty(),
            "results not empty after cancel",
        );
        kani::assert(
            search.current_index() == 0,
            "current_index not 0 after cancel",
        );
    }

    /// Navigation never produces invalid index.
    #[kani::proof]
    #[kani::unwind(20)]
    fn navigation_index_valid() {
        let len: usize = kani::any();
        let start_idx: usize = kani::any();
        let wrap: bool = kani::any();

        kani::assume(len > 0 && len <= 10);
        kani::assume(start_idx <= len);

        let fwd = StreamingSearch::next_index(start_idx, len, Direction::Forward, wrap);
        let bwd = StreamingSearch::next_index(start_idx, len, Direction::Backward, wrap);

        // Results must be valid indices (1..=len or 0)
        kani::assert(fwd <= len, "forward navigation produced invalid index");
        kani::assert(bwd <= len, "backward navigation produced invalid index");
    }
}
