//! Pin system for stable references across scrollback eviction.
//!
//! ## Overview
//!
//! Pins provide stable references to positions in the terminal buffer that
//! survive scrollback eviction. When content scrolls off the visible area,
//! normal row/column coordinates become invalid. Pins track logical positions
//! and can detect when they've been invalidated.
//!
//! ## Use Cases
//!
//! - **Selection anchors**: Start and end points of text selection survive scroll
//! - **Hyperlink anchors**: Links remain clickable even after scrolling
//! - **Search matches**: Match positions stay valid during navigation
//! - **Markers**: Custom user-defined bookmarks in history
//!
//! ## Design
//!
//! Each pin stores:
//! - `page_id`: Which page the pinned content lives in
//! - `row_offset`: Offset within the page
//! - `col`: Column position
//! - `generation`: Monotonic counter to detect invalidation
//!
//! When a page is recycled (evicted from scrollback), its generation counter
//! increments. Pins with old generation values are detected as invalid when
//! resolved.

use super::page::PageId;

/// Generation counter for detecting stale pins.
///
/// Each time a page is recycled (evicted and reused), its generation increments.
/// Pins store the generation at creation time; if the page's current generation
/// doesn't match, the pin is stale.
pub type Generation = u64;

/// A stable reference to a position in the terminal buffer.
///
/// Pins survive scrollback eviction by tracking page identity and generation.
/// Use `Pin::is_valid` to check if a pin is still valid before resolving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    /// Page containing the pinned content.
    page_id: PageId,
    /// Row offset within the page (0-indexed from page start).
    row_offset: u32,
    /// Column position (0-indexed).
    col: u16,
    /// Generation at pin creation time.
    generation: Generation,
}

impl Pin {
    /// Create a new pin at the given position.
    ///
    /// # Arguments
    ///
    /// * `page_id` - The page ID containing this position
    /// * `row_offset` - Row offset within the page
    /// * `col` - Column position
    /// * `generation` - Current generation of the page
    #[must_use]
    pub const fn new(page_id: PageId, row_offset: u32, col: u16, generation: Generation) -> Self {
        Self {
            page_id,
            row_offset,
            col,
            generation,
        }
    }

    /// Create a pin from absolute coordinates.
    ///
    /// This is a convenience method that doesn't require knowing the page layout.
    /// The pin will need to be validated against the actual page store.
    ///
    /// # Arguments
    ///
    /// * `absolute_row` - Absolute row number (including scrollback)
    /// * `col` - Column position
    /// * `generation` - Current generation of the grid
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // Intentional truncation for storage format
    pub const fn from_absolute(absolute_row: u64, col: u16, generation: Generation) -> Self {
        // Store absolute row in page_id + row_offset for simplicity
        // This works for single-page-per-row designs
        // The high 32 bits go to page_id, low 32 bits to row_offset
        // On 64-bit platforms, page_id truncation to usize is lossless
        // On 32-bit platforms, this intentionally truncates (design constraint)
        Self {
            page_id: (absolute_row >> 32) as usize,
            row_offset: absolute_row as u32,
            col,
            generation,
        }
    }

    /// Get the page ID.
    #[must_use]
    pub const fn page_id(&self) -> PageId {
        self.page_id
    }

    /// Get the row offset within the page.
    #[must_use]
    pub const fn row_offset(&self) -> u32 {
        self.row_offset
    }

    /// Get the column position.
    #[must_use]
    pub const fn col(&self) -> u16 {
        self.col
    }

    /// Get the generation at pin creation.
    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// Get the absolute row number (for from_absolute pins).
    #[must_use]
    pub const fn absolute_row(&self) -> u64 {
        ((self.page_id as u64) << 32) | (self.row_offset as u64)
    }

    /// Create a new pin with updated column.
    #[must_use]
    pub const fn with_col(self, col: u16) -> Self {
        Self { col, ..self }
    }

    /// Create a new pin with updated row offset.
    #[must_use]
    pub const fn with_row_offset(self, row_offset: u32) -> Self {
        Self { row_offset, ..self }
    }
}

/// Tracks generations for pages to detect pin invalidation.
///
/// When pages are recycled (evicted from scrollback and reused), their
/// generation counter increments. This allows pins to detect when they
/// reference stale data.
#[derive(Debug, Clone)]
pub struct GenerationTracker {
    /// Current generation for each page.
    /// Indexed by page_id. Missing pages have generation 0.
    generations: Vec<Generation>,
    /// Global generation counter (increments on any page eviction).
    global_generation: Generation,
    /// Minimum valid generation (pins older than this are definitely invalid).
    min_valid_generation: Generation,
}

impl Default for GenerationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GenerationTracker {
    /// Create a new generation tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            global_generation: 0,
            min_valid_generation: 0,
        }
    }

    /// Get the current global generation.
    #[must_use]
    pub fn current_generation(&self) -> Generation {
        self.global_generation
    }

    /// Get the generation for a specific page.
    #[must_use]
    pub fn page_generation(&self, page_id: PageId) -> Generation {
        self.generations.get(page_id).copied().unwrap_or(0)
    }

    /// Ensure we have generation tracking for the given page count.
    pub fn ensure_capacity(&mut self, page_count: usize) {
        if self.generations.len() < page_count {
            self.generations.resize(page_count, 0);
        }
    }

    /// Mark a page as evicted (increment its generation).
    ///
    /// This invalidates all pins referencing the old content of this page.
    pub fn evict_page(&mut self, page_id: PageId) {
        self.ensure_capacity(page_id + 1);
        self.generations[page_id] += 1;
        self.global_generation += 1;
    }

    /// Mark multiple pages as evicted starting from `first_page`.
    ///
    /// Used when scrollback is truncated to a certain point.
    /// Note: This does NOT update min_valid_generation since pages before
    /// first_page remain valid. Use evict_all() for full invalidation.
    pub fn evict_pages_from(&mut self, first_page: PageId) {
        for page_id in first_page..self.generations.len() {
            self.generations[page_id] += 1;
        }
        self.global_generation += 1;
        // Note: Don't update min_valid_generation here - pages before first_page
        // remain valid. The per-page generation check handles invalidation.
    }

    /// Invalidate all pins by updating the minimum valid generation.
    ///
    /// Use this when the entire buffer is cleared or reset.
    pub fn evict_all(&mut self) {
        for gen in &mut self.generations {
            *gen += 1;
        }
        self.global_generation += 1;
        self.min_valid_generation = self.global_generation;
    }

    /// Check if a pin is potentially valid.
    ///
    /// Returns `false` if the pin is definitely invalid.
    /// Returns `true` if the pin might be valid (needs full validation).
    #[must_use]
    pub fn is_potentially_valid(&self, pin: &Pin) -> bool {
        // Quick check: if pin's generation is older than min_valid, it's definitely invalid
        if pin.generation < self.min_valid_generation {
            return false;
        }

        // Check specific page generation
        let page_gen = self.page_generation(pin.page_id);
        pin.generation >= page_gen
    }

    /// Check if a pin matches the current generation of its page.
    #[must_use]
    pub fn is_valid(&self, pin: &Pin) -> bool {
        if pin.generation < self.min_valid_generation {
            return false;
        }

        let page_gen = self.page_generation(pin.page_id);
        pin.generation == page_gen
    }
}

/// A pinned range (start and end pins).
///
/// Used for selections, hyperlinks, and other multi-cell references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinnedRange {
    /// Start of the range (inclusive).
    pub start: Pin,
    /// End of the range (inclusive).
    pub end: Pin,
}

impl PinnedRange {
    /// Create a new pinned range.
    #[must_use]
    pub const fn new(start: Pin, end: Pin) -> Self {
        Self { start, end }
    }

    /// Check if both pins are potentially valid.
    #[must_use]
    pub fn is_potentially_valid(&self, tracker: &GenerationTracker) -> bool {
        tracker.is_potentially_valid(&self.start) && tracker.is_potentially_valid(&self.end)
    }

    /// Check if both pins are valid.
    #[must_use]
    pub fn is_valid(&self, tracker: &GenerationTracker) -> bool {
        tracker.is_valid(&self.start) && tracker.is_valid(&self.end)
    }

    /// Create a normalized range (start <= end).
    #[must_use]
    pub fn normalized(self) -> Self {
        if self.start.absolute_row() > self.end.absolute_row()
            || (self.start.absolute_row() == self.end.absolute_row()
                && self.start.col > self.end.col)
        {
            Self {
                start: self.end,
                end: self.start,
            }
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_creation() {
        let pin = Pin::new(0, 10, 5, 0);
        assert_eq!(pin.page_id(), 0);
        assert_eq!(pin.row_offset(), 10);
        assert_eq!(pin.col(), 5);
        assert_eq!(pin.generation(), 0);
    }

    #[test]
    fn pin_from_absolute() {
        let pin = Pin::from_absolute(1000, 42, 5);
        assert_eq!(pin.absolute_row(), 1000);
        assert_eq!(pin.col(), 42);
        assert_eq!(pin.generation(), 5);
    }

    #[test]
    fn pin_with_modifications() {
        let pin = Pin::new(0, 10, 5, 0);
        let pin2 = pin.with_col(20);
        assert_eq!(pin2.col(), 20);
        assert_eq!(pin2.row_offset(), 10);

        let pin3 = pin.with_row_offset(50);
        assert_eq!(pin3.row_offset(), 50);
        assert_eq!(pin3.col(), 5);
    }

    #[test]
    fn generation_tracker_new() {
        let tracker = GenerationTracker::new();
        assert_eq!(tracker.current_generation(), 0);
        assert_eq!(tracker.page_generation(0), 0);
        assert_eq!(tracker.page_generation(100), 0);
    }

    #[test]
    fn generation_tracker_evict() {
        let mut tracker = GenerationTracker::new();
        tracker.ensure_capacity(3);

        // Evict page 1
        tracker.evict_page(1);
        assert_eq!(tracker.page_generation(0), 0);
        assert_eq!(tracker.page_generation(1), 1);
        assert_eq!(tracker.page_generation(2), 0);
        assert_eq!(tracker.current_generation(), 1);

        // Evict page 1 again
        tracker.evict_page(1);
        assert_eq!(tracker.page_generation(1), 2);
        assert_eq!(tracker.current_generation(), 2);
    }

    #[test]
    fn pin_validity() {
        let mut tracker = GenerationTracker::new();
        tracker.ensure_capacity(2);

        // Create a pin at current generation
        let pin = Pin::new(0, 10, 5, tracker.page_generation(0));
        assert!(tracker.is_valid(&pin));
        assert!(tracker.is_potentially_valid(&pin));

        // Evict the page
        tracker.evict_page(0);
        assert!(!tracker.is_valid(&pin));

        // Create new pin at new generation
        let pin2 = Pin::new(0, 10, 5, tracker.page_generation(0));
        assert!(tracker.is_valid(&pin2));
    }

    #[test]
    fn evict_pages_from() {
        let mut tracker = GenerationTracker::new();
        tracker.ensure_capacity(5);

        // Create pins on different pages
        let pin0 = Pin::new(0, 0, 0, 0);
        let pin2 = Pin::new(2, 0, 0, 0);
        let pin4 = Pin::new(4, 0, 0, 0);

        // All should be valid initially
        assert!(tracker.is_valid(&pin0));
        assert!(tracker.is_valid(&pin2));
        assert!(tracker.is_valid(&pin4));

        // Evict pages 2 and above
        tracker.evict_pages_from(2);

        // Page 0 and 1 should still be valid, 2+ should be invalid
        assert!(tracker.is_valid(&pin0));
        assert!(!tracker.is_valid(&pin2));
        assert!(!tracker.is_valid(&pin4));
    }

    #[test]
    fn pinned_range() {
        let start = Pin::from_absolute(100, 10, 0);
        let end = Pin::from_absolute(200, 20, 0);
        let range = PinnedRange::new(start, end);

        assert_eq!(range.start.absolute_row(), 100);
        assert_eq!(range.end.absolute_row(), 200);
    }

    #[test]
    fn pinned_range_normalized() {
        // End before start
        let start = Pin::from_absolute(200, 10, 0);
        let end = Pin::from_absolute(100, 20, 0);
        let range = PinnedRange::new(start, end).normalized();

        assert_eq!(range.start.absolute_row(), 100);
        assert_eq!(range.end.absolute_row(), 200);

        // Same row, end col before start col
        let start = Pin::from_absolute(100, 50, 0);
        let end = Pin::from_absolute(100, 10, 0);
        let range = PinnedRange::new(start, end).normalized();

        assert_eq!(range.start.col(), 10);
        assert_eq!(range.end.col(), 50);
    }

    #[test]
    fn pinned_range_validity() {
        let mut tracker = GenerationTracker::new();
        tracker.ensure_capacity(3);

        let start = Pin::new(0, 0, 0, 0);
        let end = Pin::new(2, 0, 0, 0);
        let range = PinnedRange::new(start, end);

        assert!(range.is_valid(&tracker));

        // Evict page 2
        tracker.evict_page(2);
        assert!(!range.is_valid(&tracker));
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn pin_absolute_row_roundtrip() {
        let row: u64 = kani::any();
        let col: u16 = kani::any();
        let gen: Generation = kani::any();

        let pin = Pin::from_absolute(row, col, gen);
        kani::assert(pin.absolute_row() == row, "absolute row should roundtrip");
        kani::assert(pin.col() == col, "col should be preserved");
        kani::assert(pin.generation() == gen, "generation should be preserved");
    }

    #[kani::proof]
    fn generation_tracker_evict_increments() {
        let page_id: usize = kani::any();
        kani::assume(page_id < 16); // Limit for tractability

        let mut tracker = GenerationTracker::new();
        tracker.ensure_capacity(page_id + 1);

        let gen_before = tracker.page_generation(page_id);
        let global_before = tracker.current_generation();

        tracker.evict_page(page_id);

        kani::assert(
            tracker.page_generation(page_id) == gen_before + 1,
            "page generation should increment",
        );
        kani::assert(
            tracker.current_generation() == global_before + 1,
            "global generation should increment",
        );
    }

    #[kani::proof]
    fn pin_invalidated_after_eviction() {
        let page_id: usize = kani::any();
        kani::assume(page_id < 8);

        let mut tracker = GenerationTracker::new();
        tracker.ensure_capacity(page_id + 1);

        // Create pin at current generation
        let gen = tracker.page_generation(page_id);
        let pin = Pin::new(page_id, 0, 0, gen);

        kani::assert(tracker.is_valid(&pin), "pin should be valid initially");

        // Evict the page
        tracker.evict_page(page_id);

        kani::assert(
            !tracker.is_valid(&pin),
            "pin should be invalid after eviction",
        );
    }

    #[kani::proof]
    fn pinned_range_normalization_preserves_content() {
        let row1: u32 = kani::any();
        let row2: u32 = kani::any();
        let col1: u16 = kani::any();
        let col2: u16 = kani::any();

        let start = Pin::from_absolute(row1 as u64, col1, 0);
        let end = Pin::from_absolute(row2 as u64, col2, 0);
        let range = PinnedRange::new(start, end);
        let normalized = range.normalized();

        // Normalized range should have start <= end
        let start_before = normalized.start.absolute_row() < normalized.end.absolute_row()
            || (normalized.start.absolute_row() == normalized.end.absolute_row()
                && normalized.start.col() <= normalized.end.col());

        kani::assert(start_before, "normalized range should have start <= end");
    }

    #[kani::proof]
    fn evict_pages_from_invalidates_range() {
        let first_page: usize = kani::any();
        kani::assume(first_page < 8);

        let mut tracker = GenerationTracker::new();
        tracker.ensure_capacity(first_page + 4);

        // Create a pin on a page that will be evicted
        let target_page = first_page + 1;
        let pin = Pin::new(target_page, 0, 0, tracker.page_generation(target_page));

        kani::assert(
            tracker.is_valid(&pin),
            "pin should be valid before eviction",
        );

        tracker.evict_pages_from(first_page);

        kani::assert(
            !tracker.is_valid(&pin),
            "pin should be invalid after evict_from",
        );
    }
}
