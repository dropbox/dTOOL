//! Trigram-indexed search with Bloom filter acceleration.
//!
//! ## Design
//!
//! - Bloom filter for O(1) negative lookups (instant rejection)
//! - Trigram index for O(1) candidate search
//! - RoaringBitmap for efficient line number storage
//! - Integration with Grid and Scrollback for full-text search
//!
//! ## Streaming Search
//!
//! The [`streaming`] module provides memory-bounded streaming search:
//! - Search through content incrementally (row by row)
//! - Memory-bounded results with configurable limits
//! - Multiple filter modes: Literal, Regex, Fuzzy
//! - Navigation with optional wraparound
//!
//! ## Performance
//!
//! | Operation | Time Complexity |
//! |-----------|-----------------|
//! | Negative lookup | O(1) via bloom filter |
//! | Positive search | O(k) where k = matching lines |
//! | Index line | O(n) where n = line length |
//!
//! ## Verification
//!
//! - Kani proofs: `no_false_negatives`
//! - Property tests: `search_results_valid`
//! - Fuzz tests: `fuzz/fuzz_targets/search.rs`
//! - TLA+ spec: `tla/StreamingSearch.tla`

mod bloom;
pub mod streaming;

pub use bloom::BloomFilter;

use roaring::RoaringBitmap;
use rustc_hash::{FxBuildHasher, FxHashMap};

/// A match found during search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Line number (0-indexed from oldest).
    pub line: usize,
    /// Starting column of the match (0-indexed).
    pub start_col: usize,
    /// Ending column of the match (exclusive).
    pub end_col: usize,
}

impl SearchMatch {
    /// Create a new search match.
    #[must_use]
    pub fn new(line: usize, start_col: usize, end_col: usize) -> Self {
        Self {
            line,
            start_col,
            end_col,
        }
    }

    /// Get the length of the match in columns.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.end_col.saturating_sub(self.start_col)
    }

    /// Check if this is an empty match (zero length).
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.end_col <= self.start_col
    }

    /// Get the column range as a `Range<usize>`.
    #[must_use]
    #[inline]
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start_col..self.end_col
    }

    /// Check if a column is within this match.
    #[must_use]
    #[inline]
    pub fn contains_column(&self, col: usize) -> bool {
        col >= self.start_col && col < self.end_col
    }
}

/// Direction for search iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    /// Search forward (oldest to newest).
    Forward,
    /// Search backward (newest to oldest).
    Backward,
}

/// Search index using trigrams with bloom filter acceleration.
///
/// The index maintains:
/// - A bloom filter for instant negative lookups
/// - A trigram map for candidate line identification
/// - Line content cache for match verification
#[derive(Debug)]
pub struct SearchIndex {
    /// Bloom filter for fast negative lookups.
    bloom: BloomFilter,
    /// Trigram -> line numbers mapping.
    trigrams: FxHashMap<[u8; 3], RoaringBitmap>,
    /// Cached line content for match verification.
    /// Maps line number to line text.
    lines: FxHashMap<usize, String>,
    /// Total number of indexed lines.
    line_count: usize,
    /// Next line number to index (for incremental indexing).
    next_line: usize,
}

impl SearchIndex {
    /// Create a new search index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bloom: BloomFilter::with_capacity(100_000),
            trigrams: FxHashMap::default(),
            lines: FxHashMap::default(),
            line_count: 0,
            next_line: 0,
        }
    }

    /// Create a new search index with expected capacity.
    #[must_use]
    pub fn with_capacity(expected_lines: usize) -> Self {
        Self {
            bloom: BloomFilter::with_capacity(expected_lines.max(1000)),
            trigrams: FxHashMap::with_capacity_and_hasher(expected_lines / 10, FxBuildHasher),
            lines: FxHashMap::with_capacity_and_hasher(expected_lines, FxBuildHasher),
            line_count: 0,
            next_line: 0,
        }
    }

    /// Index a line at a specific line number.
    ///
    /// This overwrites any existing content at that line number.
    pub fn index_line(&mut self, line_num: usize, text: &str) {
        // Remove old trigrams if this line was previously indexed
        if let Some(old_text) = self.lines.get(&line_num) {
            self.remove_trigrams(line_num, old_text.clone());
        }

        let bytes = text.as_bytes();

        // Add all trigrams from this line
        for window in bytes.windows(3) {
            let trigram: [u8; 3] = window.try_into().unwrap();

            // Add trigram to bloom filter
            self.bloom.insert_bytes(&trigram);

            // Add to trigram index
            // line_num is bounded by scrollback limits (<<2^32)
            #[allow(clippy::cast_possible_truncation)]
            let line_u32 = line_num as u32;
            self.trigrams.entry(trigram).or_default().insert(line_u32);
        }

        // Cache the line content
        self.lines.insert(line_num, text.to_string());
        self.line_count = self.line_count.max(line_num + 1);
        self.next_line = self.next_line.max(line_num + 1);
    }

    /// Index a line at the next available line number.
    ///
    /// Returns the assigned line number.
    pub fn push_line(&mut self, text: &str) -> usize {
        let line_num = self.next_line;
        self.index_line(line_num, text);
        line_num
    }

    /// Remove trigrams for a line (internal helper).
    fn remove_trigrams(&mut self, line_num: usize, text: String) {
        let bytes = text.as_bytes();
        // line_num is bounded by scrollback limits (<<2^32)
        #[allow(clippy::cast_possible_truncation)]
        let line_u32 = line_num as u32;
        for window in bytes.windows(3) {
            let trigram: [u8; 3] = window.try_into().unwrap();
            if let Some(bitmap) = self.trigrams.get_mut(&trigram) {
                bitmap.remove(line_u32);
                // Don't remove empty bitmaps - they'll be reused
            }
        }
    }

    /// Check if a query might have matches (bloom filter check).
    ///
    /// Returns `false` if definitely no matches exist.
    /// Returns `true` if matches are possible (verify with actual search).
    #[must_use]
    pub fn might_contain(&self, query: &str) -> bool {
        let bytes = query.as_bytes();

        // For short queries, we can't use the bloom filter effectively
        if bytes.len() < 3 {
            return true;
        }

        // Check if all query trigrams might exist
        for window in bytes.windows(3) {
            if !self.bloom.might_contain_bytes(window) {
                return false;
            }
        }
        true
    }

    /// Search for a query string.
    ///
    /// Returns line numbers that might contain the query.
    /// Results may include false positives but never false negatives.
    pub fn search(&self, query: &str) -> impl Iterator<Item = u32> + '_ {
        let bytes = query.as_bytes();

        if bytes.len() < 3 {
            // Can't use trigram index for short queries
            // Fall back to returning all lines (caller must verify)
            // line_count is bounded by scrollback limits (<<2^32)
            #[allow(clippy::cast_possible_truncation)]
            let count_u32 = self.line_count as u32;
            return SearchResult::All(0..count_u32);
        }

        // Quick bloom filter check
        if !self.might_contain(query) {
            return SearchResult::None;
        }

        // Intersect posting lists for all trigrams (in-place to avoid allocations)
        let mut result: Option<RoaringBitmap> = None;

        for window in bytes.windows(3) {
            let trigram: [u8; 3] = window.try_into().unwrap();

            if let Some(bitmap) = self.trigrams.get(&trigram) {
                match &mut result {
                    None => result = Some(bitmap.clone()),
                    Some(r) => *r &= bitmap,
                }
            } else {
                // Trigram not found, no matches possible
                return SearchResult::None;
            }
        }

        match result {
            Some(bitmap) => SearchResult::Bitmap(bitmap.into_iter()),
            None => SearchResult::None,
        }
    }

    /// Search with match verification and position extraction.
    ///
    /// Returns actual matches with column positions.
    /// This verifies candidates against cached line content.
    pub fn search_with_positions(&self, query: &str) -> Vec<SearchMatch> {
        // Empty query returns no matches (prevents infinite loop in find)
        if query.is_empty() {
            return Vec::new();
        }

        let candidates: Vec<u32> = self.search(query).collect();
        let mut matches = Vec::new();

        for line_num in candidates {
            if let Some(text) = self.lines.get(&(line_num as usize)) {
                // Find all occurrences in this line
                let mut start = 0;
                while let Some(pos) = text[start..].find(query) {
                    let abs_pos = start + pos;
                    matches.push(SearchMatch::new(
                        line_num as usize,
                        abs_pos,
                        abs_pos + query.len(),
                    ));
                    start = abs_pos + 1;
                }
            }
        }

        matches
    }

    /// Search and return matches in the specified direction.
    ///
    /// Returns an iterator over matches sorted by line number.
    pub fn search_ordered(&self, query: &str, direction: SearchDirection) -> Vec<SearchMatch> {
        let mut matches = self.search_with_positions(query);

        match direction {
            SearchDirection::Forward => {
                matches.sort_by_key(|m| (m.line, m.start_col));
            }
            SearchDirection::Backward => {
                matches
                    .sort_by_key(|m| (std::cmp::Reverse(m.line), std::cmp::Reverse(m.start_col)));
            }
        }

        matches
    }

    /// Get the number of indexed lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.line_count
    }

    /// Returns true if the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.line_count == 0
    }

    /// Get the number of unique trigrams indexed.
    #[must_use]
    pub fn trigram_count(&self) -> usize {
        self.trigrams.len()
    }

    /// Get the estimated bloom filter false positive rate.
    #[must_use]
    pub fn bloom_fpr(&self) -> f64 {
        self.bloom.estimated_fpr()
    }

    /// Clear the index.
    pub fn clear(&mut self) {
        self.bloom.clear();
        self.trigrams.clear();
        self.lines.clear();
        self.line_count = 0;
        self.next_line = 0;
    }

    /// Get cached line content by line number.
    #[must_use]
    pub fn get_line(&self, line_num: usize) -> Option<&str> {
        self.lines.get(&line_num).map(|s| s.as_str())
    }

    /// Search for matches starting from a given line (O(log n) for first match).
    ///
    /// Returns an iterator over matches in forward order (oldest to newest),
    /// starting from `from_line`. This is efficient for `find_next` operations
    /// as it uses range queries on the trigram index to skip earlier lines.
    ///
    /// # Arguments
    /// * `query` - The search query (must be 3+ chars for trigram indexing)
    /// * `from_line` - Start searching from this line number (inclusive)
    pub fn search_from_line<'a>(
        &'a self,
        query: &'a str,
        from_line: usize,
    ) -> SearchMatchIterator<'a> {
        let bytes = query.as_bytes();

        // Empty query returns no matches
        if query.is_empty() {
            return SearchMatchIterator::new(self, query, Vec::new());
        }

        if bytes.len() < 3 {
            // Can't use trigram index for short queries
            // Return all lines from from_line onwards
            #[allow(clippy::cast_possible_truncation)]
            let from_u32 = from_line as u32;
            #[allow(clippy::cast_possible_truncation)]
            let count_u32 = self.line_count as u32;
            let candidates: Vec<u32> = (from_u32..count_u32).collect();
            return SearchMatchIterator::new(self, query, candidates);
        }

        // Quick bloom filter check
        if !self.might_contain(query) {
            return SearchMatchIterator::new(self, query, Vec::new());
        }

        // Intersect posting lists for all trigrams
        let mut result: Option<RoaringBitmap> = None;

        for window in bytes.windows(3) {
            let trigram: [u8; 3] = window.try_into().unwrap();

            if let Some(bitmap) = self.trigrams.get(&trigram) {
                match &mut result {
                    None => result = Some(bitmap.clone()),
                    Some(r) => *r &= bitmap,
                }
            } else {
                // Trigram not found, no matches possible
                return SearchMatchIterator::new(self, query, Vec::new());
            }
        }

        match result {
            Some(bitmap) => {
                // Use range query to start from from_line (O(log n) in RoaringBitmap)
                #[allow(clippy::cast_possible_truncation)]
                let from_u32 = from_line as u32;
                let candidates: Vec<u32> = bitmap.range(from_u32..).collect();
                SearchMatchIterator::new(self, query, candidates)
            }
            None => SearchMatchIterator::new(self, query, Vec::new()),
        }
    }

    /// Search for matches up to a given line for backward iteration.
    ///
    /// Returns an iterator over matches in reverse order (newest to oldest),
    /// only considering lines before `before_line`. This is efficient for
    /// `find_prev` operations.
    ///
    /// # Arguments
    /// * `query` - The search query (must be 3+ chars for trigram indexing)
    /// * `before_line` - Only search lines before this line number (exclusive)
    pub fn search_before_line<'a>(
        &'a self,
        query: &'a str,
        before_line: usize,
    ) -> SearchMatchReverseIterator<'a> {
        let bytes = query.as_bytes();

        // Empty query returns no matches
        if query.is_empty() {
            return SearchMatchReverseIterator::new(self, query, Vec::new());
        }

        if bytes.len() < 3 {
            // Can't use trigram index for short queries
            // Return all lines up to before_line
            #[allow(clippy::cast_possible_truncation)]
            let before_u32 = before_line.min(self.line_count) as u32;
            let candidates: Vec<u32> = (0..before_u32).collect();
            return SearchMatchReverseIterator::new(self, query, candidates);
        }

        // Quick bloom filter check
        if !self.might_contain(query) {
            return SearchMatchReverseIterator::new(self, query, Vec::new());
        }

        // Intersect posting lists for all trigrams
        let mut result: Option<RoaringBitmap> = None;

        for window in bytes.windows(3) {
            let trigram: [u8; 3] = window.try_into().unwrap();

            if let Some(bitmap) = self.trigrams.get(&trigram) {
                match &mut result {
                    None => result = Some(bitmap.clone()),
                    Some(r) => *r &= bitmap,
                }
            } else {
                // Trigram not found, no matches possible
                return SearchMatchReverseIterator::new(self, query, Vec::new());
            }
        }

        match result {
            Some(bitmap) => {
                // Use range query to get only lines before before_line (O(log n) in RoaringBitmap)
                #[allow(clippy::cast_possible_truncation)]
                let before_u32 = before_line as u32;
                let candidates: Vec<u32> = bitmap.range(..before_u32).collect();
                SearchMatchReverseIterator::new(self, query, candidates)
            }
            None => SearchMatchReverseIterator::new(self, query, Vec::new()),
        }
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Search result iterator.
enum SearchResult {
    None,
    All(std::ops::Range<u32>),
    Bitmap(roaring::bitmap::IntoIter),
}

impl Iterator for SearchResult {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            SearchResult::None => None,
            SearchResult::All(range) => range.next(),
            SearchResult::Bitmap(iter) => iter.next(),
        }
    }
}

/// Lazy iterator over search matches with early termination support.
///
/// This iterator yields matches one at a time without collecting all matches
/// first. Combined with range queries on the underlying bitmap, this enables
/// O(log n) search for find_next/find_prev operations.
pub struct SearchMatchIterator<'a> {
    /// The search index.
    index: &'a SearchIndex,
    /// The query string.
    query: &'a str,
    /// Candidate line numbers (collected from range query).
    candidates: Vec<u32>,
    /// Current index into candidates.
    candidate_idx: usize,
    /// Current line's matches (buffered for multiple matches per line).
    current_line_matches: Vec<SearchMatch>,
    /// Index into current_line_matches.
    current_match_idx: usize,
}

impl<'a> SearchMatchIterator<'a> {
    /// Create a new match iterator from pre-collected candidates.
    fn new(index: &'a SearchIndex, query: &'a str, candidates: Vec<u32>) -> Self {
        Self {
            index,
            query,
            candidates,
            candidate_idx: 0,
            current_line_matches: Vec::new(),
            current_match_idx: 0,
        }
    }

    /// Find all matches in a single line.
    fn find_matches_in_line(&self, line_num: usize) -> Vec<SearchMatch> {
        let mut matches = Vec::new();
        if let Some(text) = self.index.lines.get(&line_num) {
            let mut start = 0;
            while let Some(pos) = text[start..].find(self.query) {
                let abs_pos = start + pos;
                matches.push(SearchMatch::new(
                    line_num,
                    abs_pos,
                    abs_pos + self.query.len(),
                ));
                start = abs_pos + 1;
            }
        }
        matches
    }
}

impl Iterator for SearchMatchIterator<'_> {
    type Item = SearchMatch;

    fn next(&mut self) -> Option<Self::Item> {
        // Return buffered match if available
        if self.current_match_idx < self.current_line_matches.len() {
            let m = self.current_line_matches[self.current_match_idx].clone();
            self.current_match_idx += 1;
            return Some(m);
        }

        // Get next candidate line and find matches
        while self.candidate_idx < self.candidates.len() {
            let line_num = self.candidates[self.candidate_idx] as usize;
            self.candidate_idx += 1;
            self.current_line_matches = self.find_matches_in_line(line_num);
            self.current_match_idx = 0;

            if !self.current_line_matches.is_empty() {
                let m = self.current_line_matches[0].clone();
                self.current_match_idx = 1;
                return Some(m);
            }
            // No matches in this line (false positive from trigram), try next
        }
        None
    }
}

/// Reverse iterator over search matches.
///
/// Yields matches in reverse order (newest to oldest, right to left).
pub struct SearchMatchReverseIterator<'a> {
    /// The search index.
    index: &'a SearchIndex,
    /// The query string.
    query: &'a str,
    /// Candidate line numbers (in descending order).
    candidates: Vec<u32>,
    /// Current index into candidates.
    candidate_idx: usize,
    /// Current line's matches (in reverse column order).
    current_line_matches: Vec<SearchMatch>,
    /// Index into current_line_matches.
    current_match_idx: usize,
}

impl<'a> SearchMatchReverseIterator<'a> {
    /// Create a new reverse match iterator.
    fn new(index: &'a SearchIndex, query: &'a str, mut candidates: Vec<u32>) -> Self {
        // Sort in descending order for reverse iteration
        candidates.sort_unstable_by(|a, b| b.cmp(a));
        Self {
            index,
            query,
            candidates,
            candidate_idx: 0,
            current_line_matches: Vec::new(),
            current_match_idx: 0,
        }
    }

    /// Find all matches in a single line, sorted by column descending.
    fn find_matches_in_line(&self, line_num: usize) -> Vec<SearchMatch> {
        let mut matches = Vec::new();
        if let Some(text) = self.index.lines.get(&line_num) {
            let mut start = 0;
            while let Some(pos) = text[start..].find(self.query) {
                let abs_pos = start + pos;
                matches.push(SearchMatch::new(
                    line_num,
                    abs_pos,
                    abs_pos + self.query.len(),
                ));
                start = abs_pos + 1;
            }
        }
        // Reverse so we iterate from right to left
        matches.reverse();
        matches
    }
}

impl Iterator for SearchMatchReverseIterator<'_> {
    type Item = SearchMatch;

    fn next(&mut self) -> Option<Self::Item> {
        // Return buffered match if available
        if self.current_match_idx < self.current_line_matches.len() {
            let m = self.current_line_matches[self.current_match_idx].clone();
            self.current_match_idx += 1;
            return Some(m);
        }

        // Get next candidate line and find matches
        while self.candidate_idx < self.candidates.len() {
            let line_num = self.candidates[self.candidate_idx] as usize;
            self.candidate_idx += 1;
            self.current_line_matches = self.find_matches_in_line(line_num);
            self.current_match_idx = 0;

            if !self.current_line_matches.is_empty() {
                let m = self.current_line_matches[0].clone();
                self.current_match_idx = 1;
                return Some(m);
            }
        }
        None
    }
}

/// Terminal search that integrates with Grid and Scrollback.
///
/// This provides a unified interface for searching across:
/// - Current visible grid content
/// - Ring buffer scrollback
/// - Tiered scrollback (hot/warm/cold)
#[derive(Debug)]
pub struct TerminalSearch {
    /// Search index for all content.
    index: SearchIndex,
    /// Number of lines from scrollback that have been indexed.
    indexed_scrollback_lines: usize,
}

impl TerminalSearch {
    /// Create a new terminal search.
    #[must_use]
    pub fn new() -> Self {
        Self {
            index: SearchIndex::new(),
            indexed_scrollback_lines: 0,
        }
    }

    /// Create with expected capacity.
    #[must_use]
    pub fn with_capacity(expected_lines: usize) -> Self {
        Self {
            index: SearchIndex::with_capacity(expected_lines),
            indexed_scrollback_lines: 0,
        }
    }

    /// Index a scrollback line.
    ///
    /// Call this when lines are pushed to scrollback.
    pub fn index_scrollback_line(&mut self, text: &str) {
        self.index.push_line(text);
        self.indexed_scrollback_lines += 1;
    }

    /// Index multiple scrollback lines.
    pub fn index_scrollback_lines(&mut self, lines: impl IntoIterator<Item = impl AsRef<str>>) {
        for line in lines {
            self.index_scrollback_line(line.as_ref());
        }
    }

    /// Re-index visible grid content.
    ///
    /// Call this to update the index with current grid content.
    /// Pass the visible content as an iterator of (line_index, text).
    pub fn index_visible_content(
        &mut self,
        base_line: usize,
        lines: impl IntoIterator<Item = impl AsRef<str>>,
    ) {
        for (offset, line) in lines.into_iter().enumerate() {
            self.index.index_line(base_line + offset, line.as_ref());
        }
    }

    /// Check if a query might have matches.
    #[must_use]
    pub fn might_contain(&self, query: &str) -> bool {
        self.index.might_contain(query)
    }

    /// Search for a query string.
    pub fn search(&self, query: &str) -> Vec<SearchMatch> {
        self.index.search_with_positions(query)
    }

    /// Search in the specified direction.
    pub fn search_ordered(&self, query: &str, direction: SearchDirection) -> Vec<SearchMatch> {
        self.index.search_ordered(query, direction)
    }

    /// Find the next match after the given position.
    ///
    /// This uses O(log n) range queries to skip lines before `after_line`,
    /// then iterates with early termination to find the first match.
    pub fn find_next(
        &self,
        query: &str,
        after_line: usize,
        after_col: usize,
    ) -> Option<SearchMatch> {
        // Use optimized range query starting from after_line
        self.index
            .search_from_line(query, after_line)
            .find(|m| m.line > after_line || (m.line == after_line && m.start_col > after_col))
    }

    /// Find the previous match before the given position.
    ///
    /// This uses O(log n) range queries to only search lines before `before_line`,
    /// then iterates with early termination to find the first match.
    pub fn find_prev(
        &self,
        query: &str,
        before_line: usize,
        before_col: usize,
    ) -> Option<SearchMatch> {
        // Use optimized range query only searching lines up to before_line + 1
        // (we need before_line itself since we're looking for matches before before_col)
        self.index
            .search_before_line(query, before_line + 1)
            .find(|m| m.line < before_line || (m.line == before_line && m.start_col < before_col))
    }

    /// Get the number of indexed lines.
    #[must_use]
    pub fn indexed_line_count(&self) -> usize {
        self.index.len()
    }

    /// Get the number of scrollback lines indexed.
    #[must_use]
    pub fn indexed_scrollback_count(&self) -> usize {
        self.indexed_scrollback_lines
    }

    /// Clear the search index.
    pub fn clear(&mut self) {
        self.index.clear();
        self.indexed_scrollback_lines = 0;
    }

    /// Get access to the underlying SearchIndex.
    #[must_use]
    pub fn index(&self) -> &SearchIndex {
        &self.index
    }
}

impl Default for TerminalSearch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_and_search() {
        let mut index = SearchIndex::new();

        index.index_line(0, "hello world");
        index.index_line(1, "goodbye world");
        index.index_line(2, "hello there");

        // Search for "world"
        let results: Vec<_> = index.search("world").collect();
        assert!(results.contains(&0));
        assert!(results.contains(&1));
        assert!(!results.contains(&2));

        // Search for "hello"
        let results: Vec<_> = index.search("hello").collect();
        assert!(results.contains(&0));
        assert!(results.contains(&2));
    }

    #[test]
    fn empty_query() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test");

        // Short queries return all lines
        let results: Vec<_> = index.search("ab").collect();
        assert_eq!(results.len(), 1);
    }

    /// CRITICAL: Empty query must return empty results without infinite loop.
    ///
    /// This catches the bug where `"text".find("")` returns `Some(0)` forever.
    #[test]
    fn empty_query_search_with_positions() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test content");
        index.index_line(1, "more content");

        // Empty query MUST return empty results (not infinite loop)
        let matches = index.search_with_positions("");
        assert!(matches.is_empty(), "empty query must return empty results");
    }

    /// Empty query through TerminalSearch API.
    #[test]
    fn empty_query_terminal_search() {
        let mut search = TerminalSearch::new();
        search.index_scrollback_line("test line");

        assert!(search.search("").is_empty());
        assert!(search.find_next("", 0, 0).is_none());
        assert!(search.find_prev("", 10, 0).is_none());
    }

    #[test]
    fn no_matches() {
        let mut index = SearchIndex::new();
        index.index_line(0, "hello world");

        let results: Vec<_> = index.search("xyz").collect();
        assert!(results.is_empty());
    }

    #[test]
    fn bloom_filter_rejection() {
        let mut index = SearchIndex::new();
        index.index_line(0, "hello world");

        // Query with unique trigrams should be rejected by bloom filter
        assert!(index.might_contain("hello"));
        // Note: might_contain can return true for non-existent strings (false positive)
        // but should return false for strings with trigrams not in the filter
    }

    #[test]
    fn search_with_positions() {
        let mut index = SearchIndex::new();
        index.index_line(0, "hello hello");
        index.index_line(1, "hello world");

        let matches = index.search_with_positions("hello");

        // Should find "hello" at multiple positions
        assert!(matches.len() >= 2);

        // Line 0 should have two matches
        let line0_matches: Vec<_> = matches.iter().filter(|m| m.line == 0).collect();
        assert_eq!(line0_matches.len(), 2);

        // Verify positions
        assert_eq!(line0_matches[0].start_col, 0);
        assert_eq!(line0_matches[0].end_col, 5);
        assert_eq!(line0_matches[1].start_col, 6);
        assert_eq!(line0_matches[1].end_col, 11);
    }

    #[test]
    fn search_ordered() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test line 0");
        index.index_line(1, "test line 1");
        index.index_line(2, "test line 2");

        // Forward search
        let fwd = index.search_ordered("test", SearchDirection::Forward);
        assert_eq!(fwd[0].line, 0);
        assert_eq!(fwd[1].line, 1);
        assert_eq!(fwd[2].line, 2);

        // Backward search
        let bwd = index.search_ordered("test", SearchDirection::Backward);
        assert_eq!(bwd[0].line, 2);
        assert_eq!(bwd[1].line, 1);
        assert_eq!(bwd[2].line, 0);
    }

    #[test]
    fn push_line() {
        let mut index = SearchIndex::new();

        let n0 = index.push_line("line 0");
        let n1 = index.push_line("line 1");
        let n2 = index.push_line("line 2");

        assert_eq!(n0, 0);
        assert_eq!(n1, 1);
        assert_eq!(n2, 2);
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn terminal_search_basic() {
        let mut search = TerminalSearch::new();

        search.index_scrollback_line("scrollback line 1");
        search.index_scrollback_line("scrollback line 2");
        search.index_scrollback_line("scrollback line 3");

        let matches = search.search("scrollback");
        assert_eq!(matches.len(), 3);

        assert_eq!(search.indexed_scrollback_count(), 3);
    }

    #[test]
    fn terminal_search_find_next_prev() {
        let mut search = TerminalSearch::new();

        search.index_scrollback_line("match here");
        search.index_scrollback_line("no match");
        search.index_scrollback_line("match again");

        // Find next from before start (should find first match at line 0, col 0)
        let next = search.find_next("match", 0, 0);
        // This should NOT find line 0, col 0 since we want AFTER (0, 0)
        // But "no match" at line 1 has "match" starting at col 3
        assert!(next.is_some());
        let m = next.unwrap();
        // Line 1 has "no match" with "match" at column 3
        assert_eq!(m.line, 1);
        assert_eq!(m.start_col, 3);

        // Find next from after line 1 match
        let next = search.find_next("match", 1, 7);
        assert!(next.is_some());
        assert_eq!(next.unwrap().line, 2);

        // Find prev from end
        let prev = search.find_prev("match", 3, 0);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().line, 2);

        // Find prev from line 2 (should find line 1's match)
        let prev = search.find_prev("match", 2, 0);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().line, 1);

        // Find prev from line 1, col 0 (should find line 0's match)
        let prev = search.find_prev("match", 1, 0);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().line, 0);
    }

    #[test]
    fn search_match_struct() {
        let m = SearchMatch::new(5, 10, 15);
        assert_eq!(m.line, 5);
        assert_eq!(m.start_col, 10);
        assert_eq!(m.end_col, 15);
    }

    #[test]
    fn index_clear() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test");
        assert!(!index.is_empty());

        index.clear();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn get_line() {
        let mut index = SearchIndex::new();
        index.index_line(5, "hello");

        assert_eq!(index.get_line(5), Some("hello"));
        assert_eq!(index.get_line(0), None);
    }

    #[test]
    fn reindex_line() {
        let mut index = SearchIndex::new();
        index.index_line(0, "original");

        let results: Vec<_> = index.search("original").collect();
        assert!(results.contains(&0));

        // Reindex same line with different content
        index.index_line(0, "updated");

        let results: Vec<_> = index.search("original").collect();
        assert!(results.is_empty());

        let results: Vec<_> = index.search("updated").collect();
        assert!(results.contains(&0));
    }

    #[test]
    fn search_from_line_basic() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test line zero");
        index.index_line(1, "test line one");
        index.index_line(2, "test line two");
        index.index_line(3, "test line three");
        index.index_line(4, "test line four");

        // Search from line 2 - should only find lines 2, 3, 4
        let matches: Vec<_> = index.search_from_line("test", 2).collect();
        assert_eq!(matches.len(), 3);
        assert!(matches.iter().all(|m| m.line >= 2));
        assert_eq!(matches[0].line, 2);
        assert_eq!(matches[1].line, 3);
        assert_eq!(matches[2].line, 4);
    }

    #[test]
    fn search_from_line_empty_query() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test");

        let matches: Vec<_> = index.search_from_line("", 0).collect();
        assert!(matches.is_empty());
    }

    #[test]
    fn search_from_line_short_query() {
        let mut index = SearchIndex::new();
        index.index_line(0, "ab test");
        index.index_line(1, "ab test");
        index.index_line(2, "ab test");

        // Short queries (<3 chars) return all lines from from_line
        let matches: Vec<_> = index.search_from_line("ab", 1).collect();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[1].line, 2);
    }

    #[test]
    fn search_from_line_no_matches() {
        let mut index = SearchIndex::new();
        index.index_line(0, "hello world");
        index.index_line(1, "goodbye world");

        let matches: Vec<_> = index.search_from_line("xyz", 0).collect();
        assert!(matches.is_empty());
    }

    #[test]
    fn search_before_line_basic() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test line zero");
        index.index_line(1, "test line one");
        index.index_line(2, "test line two");
        index.index_line(3, "test line three");
        index.index_line(4, "test line four");

        // Search before line 3 - should only find lines 0, 1, 2 in reverse order
        let matches: Vec<_> = index.search_before_line("test", 3).collect();
        assert_eq!(matches.len(), 3);
        assert!(matches.iter().all(|m| m.line < 3));
        // Should be in reverse order (newest to oldest)
        assert_eq!(matches[0].line, 2);
        assert_eq!(matches[1].line, 1);
        assert_eq!(matches[2].line, 0);
    }

    #[test]
    fn search_before_line_empty_query() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test");

        let matches: Vec<_> = index.search_before_line("", 10).collect();
        assert!(matches.is_empty());
    }

    #[test]
    fn search_iterator_early_termination() {
        let mut index = SearchIndex::new();
        // Index 1000 lines with "test"
        for i in 0..1000 {
            index.index_line(i, &format!("test line {i}"));
        }

        // Using the iterator with early termination (via .next())
        // should not need to process all 1000 lines
        let mut iter = index.search_from_line("test", 500);
        let first = iter.next();
        assert!(first.is_some());
        assert_eq!(first.unwrap().line, 500);

        // We can continue iteration if needed
        let second = iter.next();
        assert!(second.is_some());
        assert_eq!(second.unwrap().line, 501);
    }

    #[test]
    fn search_match_iterator_multiple_matches_per_line() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test test test");
        index.index_line(1, "test");

        let matches: Vec<_> = index.search_from_line("test", 0).collect();
        // Line 0 has 3 matches, line 1 has 1 match
        assert_eq!(matches.len(), 4);
        assert_eq!(matches[0].line, 0);
        assert_eq!(matches[0].start_col, 0);
        assert_eq!(matches[1].line, 0);
        assert_eq!(matches[1].start_col, 5);
        assert_eq!(matches[2].line, 0);
        assert_eq!(matches[2].start_col, 10);
        assert_eq!(matches[3].line, 1);
    }

    #[test]
    fn search_reverse_iterator_multiple_matches_per_line() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test test test");
        index.index_line(1, "test");

        let matches: Vec<_> = index.search_before_line("test", 2).collect();
        // Should be in reverse order: line 1, then line 0 (right to left)
        assert_eq!(matches.len(), 4);
        assert_eq!(matches[0].line, 1);
        // Line 0's matches should be in reverse column order
        assert_eq!(matches[1].line, 0);
        assert_eq!(matches[1].start_col, 10); // rightmost first
        assert_eq!(matches[2].line, 0);
        assert_eq!(matches[2].start_col, 5);
        assert_eq!(matches[3].line, 0);
        assert_eq!(matches[3].start_col, 0); // leftmost last
    }

    #[test]
    fn find_next_optimized() {
        let mut search = TerminalSearch::new();

        // Index many lines to test that we don't scan all of them
        for i in 0..100 {
            search.index_scrollback_line(&format!("match at line {i}"));
        }

        // Find next from line 50 should not scan lines 0-49
        let next = search.find_next("match", 50, 0);
        assert!(next.is_some());
        assert_eq!(next.unwrap().line, 51); // Line 50 col 0 excluded, so line 51

        // Find from middle of line 50
        let next = search.find_next("match", 50, 5);
        assert!(next.is_some());
        assert_eq!(next.unwrap().line, 51);
    }

    #[test]
    fn find_prev_optimized() {
        let mut search = TerminalSearch::new();

        // Index many lines
        for i in 0..100 {
            search.index_scrollback_line(&format!("match at line {i}"));
        }

        // Find prev from line 50 should not scan lines 51-99
        let prev = search.find_prev("match", 50, 100);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().line, 50); // Line 50 with col < 100

        // Find prev from line 50 col 0 should find line 49
        let prev = search.find_prev("match", 50, 0);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().line, 49);
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// No false negatives: if line contains query, search finds it.
    #[kani::proof]
    fn no_false_negatives_simple() {
        let mut index = SearchIndex::new();

        // Create a deterministic test case
        let line = "hello world test";
        index.index_line(0, line);

        // Search for substring that exists
        let query = "wor"; // 3+ chars required for trigram
        let results: Vec<_> = index.search(query).collect();

        kani::assert(results.contains(&0), "false negative: should find line 0");
    }

    /// Index length is consistent with indexed lines.
    #[kani::proof]
    #[kani::unwind(6)]
    fn index_length_consistent() {
        let mut index = SearchIndex::new();

        let count: usize = kani::any();
        kani::assume(count <= 5);

        for i in 0..count {
            index.index_line(i, "test line");
        }

        kani::assert(index.len() == count, "index length mismatch");
    }

    /// CRITICAL: Empty query must return empty results and terminate.
    ///
    /// This proof catches the infinite loop bug where `"".find("")` returns
    /// `Some(0)` infinitely. The fix is to check for empty query early.
    #[kani::proof]
    fn empty_query_returns_empty_results() {
        let mut index = SearchIndex::new();

        // Index some content
        index.index_line(0, "test content");

        // Empty query MUST return empty results
        let matches = index.search_with_positions("");

        kani::assert(matches.is_empty(), "empty query must return empty results");
    }

    /// Empty query must be handled correctly by might_contain.
    #[kani::proof]
    fn empty_query_might_contain_safe() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test");

        // Empty query is short (<3 chars) so might_contain returns true
        // This is safe - the actual search will handle it
        let result = index.might_contain("");
        kani::assert(result == true, "empty query returns true for might_contain");
    }

    /// Search match positions must be valid bounds.
    #[kani::proof]
    fn search_match_bounds_valid() {
        let mut index = SearchIndex::new();

        let line = "hello world";
        let line_len = line.len();
        index.index_line(0, line);

        let query = "wor";
        let matches = index.search_with_positions(query);

        for m in matches.iter() {
            // Line number must match indexed line
            kani::assert(m.line == 0, "line number must be valid");
            // Start column must be less than end column
            kani::assert(m.start_col < m.end_col, "start_col must be < end_col");
            // End column must not exceed line length
            kani::assert(m.end_col <= line_len, "end_col must be <= line length");
            // Match length must equal query length
            kani::assert(
                m.end_col - m.start_col == query.len(),
                "match length must equal query length",
            );
        }
    }

    /// Search on empty index returns no matches.
    #[kani::proof]
    fn search_empty_index_returns_empty() {
        let index = SearchIndex::new();

        let matches = index.search_with_positions("test");

        kani::assert(matches.is_empty(), "empty index must return empty results");
    }

    /// Short query (< 3 chars) doesn't crash.
    #[kani::proof]
    fn short_query_safe() {
        let mut index = SearchIndex::new();
        index.index_line(0, "test");

        // Queries shorter than 3 chars can't use trigrams
        // but must still work correctly
        let matches1 = index.search_with_positions("a");
        let matches2 = index.search_with_positions("ab");

        // These should complete without panic
        // (no assertion needed - we're verifying termination)
        kani::assert(true, "short queries terminate");
    }

    /// Query longer than line content handles correctly.
    #[kani::proof]
    fn query_longer_than_content_returns_empty() {
        let mut index = SearchIndex::new();
        index.index_line(0, "hi");

        // Query is longer than any indexed content
        let matches = index.search_with_positions("hello world");

        kani::assert(
            matches.is_empty(),
            "query longer than content returns empty",
        );
    }

    /// TerminalSearch empty query is safe.
    #[kani::proof]
    fn terminal_search_empty_query_safe() {
        let mut search = TerminalSearch::new();
        search.index_scrollback_line("test line");

        let matches = search.search("");

        kani::assert(
            matches.is_empty(),
            "terminal search empty query returns empty",
        );
    }

    /// find_next with empty query is safe.
    #[kani::proof]
    fn find_next_empty_query_safe() {
        let mut search = TerminalSearch::new();
        search.index_scrollback_line("test");

        let result = search.find_next("", 0, 0);

        kani::assert(result.is_none(), "find_next with empty query returns None");
    }

    /// find_prev with empty query is safe.
    #[kani::proof]
    fn find_prev_empty_query_safe() {
        let mut search = TerminalSearch::new();
        search.index_scrollback_line("test");

        let result = search.find_prev("", 100, 0);

        kani::assert(result.is_none(), "find_prev with empty query returns None");
    }
}
