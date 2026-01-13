//! Damage tracking for efficient rendering.
//!
//! Only re-render cells that have changed since the last frame.
//!
//! ## Design
//!
//! - Track damaged row ranges using bitsets for O(1) marking and efficient iteration
//! - Support full damage (resize, scroll, clear screen)
//! - Support partial damage (individual cell changes)
//! - O(1) damage queries
//! - Fast iteration using `trailing_zeros()` to skip undamaged regions
//! - Column-level damage bounds for fine-grained GPU rendering
//! - Rectangle merging to reduce draw calls
//!
//! ## Usage
//!
//! ```rust
//! use dterm_core::grid::Damage;
//!
//! let mut damage = Damage::new(24);
//!
//! // Mark individual cell changes
//! damage.mark_cell(5, 10);
//! damage.mark_cell(5, 20);
//!
//! // Iterate over damaged rows efficiently
//! for row in damage.damaged_rows(24) {
//!     // Only rows with damage are yielded
//! }
//!
//! // Get column bounds for a damaged row
//! if let Some((left, right)) = damage.row_damage_bounds(5, 80) {
//!     // Render only columns left..right
//! }
//!
//! // Iterate with full bounds for rendering
//! for bounds in damage.iter_bounds(24, 80) {
//!     // bounds.line, bounds.left, bounds.right
//! }
//! ```

/// Damage state for the terminal grid.
#[derive(Debug, Clone)]
pub enum Damage {
    /// Full damage - entire screen needs redraw.
    Full,
    /// Partial damage - only specific rows need redraw.
    Partial(DamageTracker),
}

impl Default for Damage {
    fn default() -> Self {
        Damage::Full
    }
}

impl Damage {
    /// Create a new damage tracker with partial tracking.
    #[must_use]
    pub fn new(rows: u16) -> Self {
        Damage::Partial(DamageTracker::new(rows))
    }

    /// Mark full damage (entire screen needs redraw).
    #[inline]
    pub fn mark_full(&mut self) {
        *self = Damage::Full;
    }

    /// Mark a single row as damaged.
    #[inline]
    pub fn mark_row(&mut self, row: u16) {
        match self {
            Damage::Full => {}
            Damage::Partial(tracker) => tracker.mark_row(row),
        }
    }

    /// Mark a range of rows as damaged.
    #[inline]
    pub fn mark_rows(&mut self, start: u16, end: u16) {
        match self {
            Damage::Full => {}
            Damage::Partial(tracker) => tracker.mark_rows(start, end),
        }
    }

    /// Mark a cell as damaged.
    #[inline]
    pub fn mark_cell(&mut self, row: u16, col: u16) {
        match self {
            Damage::Full => {}
            Damage::Partial(tracker) => tracker.mark_cell(row, col),
        }
    }

    /// Check if the entire screen is damaged.
    #[must_use]
    #[inline]
    pub fn is_full(&self) -> bool {
        matches!(self, Damage::Full)
    }

    /// Check if a row is damaged.
    #[must_use]
    #[inline]
    pub fn is_row_damaged(&self, row: u16) -> bool {
        match self {
            Damage::Full => true,
            Damage::Partial(tracker) => tracker.is_row_damaged(row),
        }
    }

    /// Get damaged row bounds for a row (returns column range if damaged).
    #[must_use]
    pub fn row_damage_bounds(&self, row: u16, cols: u16) -> Option<(u16, u16)> {
        match self {
            Damage::Full => Some((0, cols)),
            Damage::Partial(tracker) => tracker.row_damage_bounds(row).and_then(|(left, right)| {
                let left = left.min(cols);
                let right = right.min(cols);
                if left < right {
                    Some((left, right))
                } else {
                    None
                }
            }),
        }
    }

    /// Reset damage tracking (call after render).
    pub fn reset(&mut self, rows: u16) {
        *self = Damage::Partial(DamageTracker::new(rows));
    }

    /// Iterate over damaged rows.
    ///
    /// For `Full` damage, yields all rows 0..rows.
    /// For `Partial` damage, uses bitset operations to efficiently skip undamaged rows.
    pub fn damaged_rows(&self, rows: u16) -> DamagedRowIterator<'_> {
        match self {
            Damage::Full => DamagedRowIterator::Full {
                current: 0,
                max: rows,
            },
            Damage::Partial(tracker) => {
                DamagedRowIterator::Partial(BitsetRowIterator::new(&tracker.row_bits, rows))
            }
        }
    }

    /// Iterate over damaged rows with their column bounds.
    ///
    /// This is the primary API for renderers. Each yielded `LineDamageBounds`
    /// contains the row index and the column range [left, right) that needs
    /// to be redrawn.
    pub fn iter_bounds(&self, rows: u16, cols: u16) -> DamageBoundsIterator<'_> {
        DamageBoundsIterator {
            damage: self,
            row_iter: self.damaged_rows(rows),
            cols,
        }
    }

    /// Check if any damage exists.
    #[must_use]
    #[inline]
    pub fn has_damage(&self) -> bool {
        match self {
            Damage::Full => true,
            Damage::Partial(tracker) => tracker.row_bits.iter().any(|&w| w != 0),
        }
    }
}

/// Tracks which rows (and optionally columns) are damaged.
#[derive(Debug, Clone)]
pub struct DamageTracker {
    /// Bitset for damaged rows (1 bit per row).
    row_bits: Vec<u64>,
    /// Per-row column damage bounds: (min_col, max_col) if damaged.
    row_bounds: Vec<RowDamageBounds>,
}

/// Column damage bounds for a single row.
#[derive(Debug, Clone, Copy, Default)]
pub struct RowDamageBounds {
    /// Minimum damaged column (inclusive).
    pub left: u16,
    /// Maximum damaged column (exclusive).
    pub right: u16,
    /// Whether any damage exists in this row.
    pub damaged: bool,
}

impl DamageTracker {
    /// Create a new damage tracker for the given number of rows.
    #[must_use]
    pub fn new(rows: u16) -> Self {
        let num_words = ((rows as usize) + 63) / 64;
        Self {
            row_bits: vec![0; num_words],
            row_bounds: vec![RowDamageBounds::default(); rows as usize],
        }
    }

    /// Mark a single row as fully damaged.
    #[inline]
    pub fn mark_row(&mut self, row: u16) {
        let row = row as usize;
        if row < self.row_bounds.len() {
            let word = row / 64;
            let bit = row % 64;
            self.row_bits[word] |= 1 << bit;
            self.row_bounds[row] = RowDamageBounds {
                left: 0,
                right: u16::MAX,
                damaged: true,
            };
        }
    }

    /// Mark a range of rows as damaged.
    #[inline]
    pub fn mark_rows(&mut self, start: u16, end: u16) {
        for row in start..end {
            self.mark_row(row);
        }
    }

    /// Mark a specific cell as damaged.
    #[inline]
    pub fn mark_cell(&mut self, row: u16, col: u16) {
        let row_idx = row as usize;
        if row_idx < self.row_bounds.len() {
            let word = row_idx / 64;
            let bit = row_idx % 64;
            self.row_bits[word] |= 1 << bit;

            let bounds = &mut self.row_bounds[row_idx];
            if bounds.damaged {
                bounds.left = bounds.left.min(col);
                bounds.right = bounds.right.max(col + 1);
            } else {
                bounds.left = col;
                bounds.right = col + 1;
                bounds.damaged = true;
            }
        }
    }

    /// Check if a row is damaged.
    #[must_use]
    #[inline]
    pub fn is_row_damaged(&self, row: u16) -> bool {
        let row = row as usize;
        if row >= self.row_bounds.len() {
            return false;
        }
        let word = row / 64;
        let bit = row % 64;
        (self.row_bits[word] & (1 << bit)) != 0
    }

    /// Get damage bounds for a row.
    #[must_use]
    #[inline]
    pub fn row_damage_bounds(&self, row: u16) -> Option<(u16, u16)> {
        let row = row as usize;
        if row < self.row_bounds.len() && self.row_bounds[row].damaged {
            Some((self.row_bounds[row].left, self.row_bounds[row].right))
        } else {
            None
        }
    }

    /// Count total damaged rows.
    #[must_use]
    pub fn damaged_row_count(&self) -> usize {
        self.row_bits.iter().map(|w| w.count_ones() as usize).sum()
    }
}

/// Iterator over damaged rows.
///
/// Uses different strategies for `Full` vs `Partial` damage:
/// - `Full`: Simple counter from 0 to max
/// - `Partial`: Bitset-based iteration using `trailing_zeros()` to skip undamaged rows
pub enum DamagedRowIterator<'a> {
    /// Full damage - iterate all rows.
    Full {
        /// Current row.
        current: u16,
        /// Maximum row (exclusive).
        max: u16,
    },
    /// Partial damage - use bitset iteration.
    Partial(BitsetRowIterator<'a>),
}

impl Iterator for DamagedRowIterator<'_> {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            DamagedRowIterator::Full { current, max } => {
                if *current < *max {
                    let row = *current;
                    *current += 1;
                    Some(row)
                } else {
                    None
                }
            }
            DamagedRowIterator::Partial(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            DamagedRowIterator::Full { current, max } => {
                let remaining = (*max - *current) as usize;
                (remaining, Some(remaining))
            }
            DamagedRowIterator::Partial(iter) => iter.size_hint(),
        }
    }
}

/// Fast iterator over set bits in a bitset using `trailing_zeros()`.
///
/// This iterator efficiently skips over undamaged rows by using bit manipulation
/// to find the next set bit without checking each row individually.
pub struct BitsetRowIterator<'a> {
    /// Reference to the bitset words.
    bits: &'a [u64],
    /// Current word index.
    word_idx: usize,
    /// Current word with consumed bits cleared.
    current_word: u64,
    /// Maximum row to yield (exclusive).
    max_row: u16,
}

impl<'a> BitsetRowIterator<'a> {
    /// Create a new bitset iterator.
    #[inline]
    fn new(bits: &'a [u64], max_row: u16) -> Self {
        let current_word = bits.first().copied().unwrap_or(0);
        Self {
            bits,
            word_idx: 0,
            current_word,
            max_row,
        }
    }
}

impl Iterator for BitsetRowIterator<'_> {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Find next set bit in current word
            if self.current_word != 0 {
                let bit_pos = self.current_word.trailing_zeros() as usize;
                // word_idx is bounded by max_row/64 ≤ MAX_ROWS/64 = 65535/64 < 1024
                // bit_pos is bounded by 0..64 (from trailing_zeros of u64)
                // So word_idx * 64 + bit_pos ≤ 1023 * 64 + 63 = 65535 which fits in u16
                #[allow(clippy::cast_possible_truncation)]
                let row = (self.word_idx * 64 + bit_pos) as u16;

                // Clear this bit for next iteration
                self.current_word &= !(1u64 << bit_pos);

                if row < self.max_row {
                    return Some(row);
                }
                return None;
            }

            // Move to next word
            self.word_idx += 1;
            if self.word_idx >= self.bits.len() {
                return None;
            }
            self.current_word = self.bits[self.word_idx];
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Count remaining set bits (upper bound)
        let remaining: usize = self.current_word.count_ones() as usize
            + self.bits[self.word_idx + 1..]
                .iter()
                .map(|w| w.count_ones() as usize)
                .sum::<usize>();
        (0, Some(remaining))
    }
}

/// Iterator over damaged rows with their column bounds.
///
/// This is the recommended iterator for rendering as it provides
/// both the row index and the damaged column range.
pub struct DamageBoundsIterator<'a> {
    damage: &'a Damage,
    row_iter: DamagedRowIterator<'a>,
    cols: u16,
}

impl Iterator for DamageBoundsIterator<'_> {
    type Item = LineDamageBounds;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let row = self.row_iter.next()?;
        let (left, right) = self.damage.row_damage_bounds(row, self.cols)?;
        Some(LineDamageBounds {
            line: row,
            left,
            right,
        })
    }
}

/// Line damage bounds for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineDamageBounds {
    /// Row index.
    pub line: u16,
    /// Left column (inclusive).
    pub left: u16,
    /// Right column (exclusive).
    pub right: u16,
}

impl LineDamageBounds {
    /// Create new line damage bounds.
    #[inline]
    pub const fn new(line: u16, left: u16, right: u16) -> Self {
        Self { line, left, right }
    }

    /// Create damage bounds for a full row.
    #[inline]
    pub const fn full_row(line: u16, cols: u16) -> Self {
        Self {
            line,
            left: 0,
            right: cols,
        }
    }

    /// Check if this bounds is empty (no damage).
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.left >= self.right
    }

    /// Check if two adjacent rows can be merged into a single rectangle.
    ///
    /// Two rows can be merged if they are consecutive and have overlapping
    /// or adjacent column ranges.
    #[inline]
    pub fn can_merge_with(&self, other: &Self) -> bool {
        // Must be adjacent lines
        if self.line.abs_diff(other.line) != 1 {
            return false;
        }
        // Column ranges must overlap or be adjacent
        self.left <= other.right && other.left <= self.right
    }

    /// Merge with another bounds, returning a rectangle covering both.
    ///
    /// The result will have column bounds covering both inputs.
    /// Call `can_merge_with` first to check if merging is beneficial.
    #[inline]
    pub fn merge_with(&self, other: &Self) -> DamageRect {
        DamageRect {
            top: self.line.min(other.line),
            bottom: self.line.max(other.line) + 1,
            left: self.left.min(other.left),
            right: self.right.max(other.right),
        }
    }
}

/// A rectangular damage region spanning multiple rows.
///
/// Used to batch adjacent damaged rows for more efficient GPU rendering.
/// Instead of rendering many thin horizontal strips, merged rectangles
/// can be rendered with fewer draw calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRect {
    /// Top row (inclusive).
    pub top: u16,
    /// Bottom row (exclusive).
    pub bottom: u16,
    /// Left column (inclusive).
    pub left: u16,
    /// Right column (exclusive).
    pub right: u16,
}

impl DamageRect {
    /// Create a new damage rectangle.
    #[inline]
    pub const fn new(top: u16, bottom: u16, left: u16, right: u16) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
        }
    }

    /// Create a rectangle from a single line bounds.
    #[inline]
    pub const fn from_line(bounds: LineDamageBounds) -> Self {
        Self {
            top: bounds.line,
            bottom: bounds.line + 1,
            left: bounds.left,
            right: bounds.right,
        }
    }

    /// Number of rows in this rectangle.
    #[inline]
    pub const fn height(&self) -> u16 {
        self.bottom.saturating_sub(self.top)
    }

    /// Number of columns in this rectangle.
    #[inline]
    pub const fn width(&self) -> u16 {
        self.right.saturating_sub(self.left)
    }

    /// Total cells in this rectangle.
    #[inline]
    pub const fn cell_count(&self) -> u32 {
        self.height() as u32 * self.width() as u32
    }

    /// Check if a line bounds can be merged into this rectangle.
    #[inline]
    pub fn can_extend_with(&self, bounds: &LineDamageBounds) -> bool {
        // Line must be immediately below
        if bounds.line != self.bottom {
            return false;
        }
        // Column ranges must overlap or be adjacent
        bounds.left <= self.right && self.left <= bounds.right
    }

    /// Extend this rectangle to include a line bounds.
    #[inline]
    pub fn extend_with(&mut self, bounds: &LineDamageBounds) {
        self.bottom = bounds.line + 1;
        self.left = self.left.min(bounds.left);
        self.right = self.right.max(bounds.right);
    }
}

/// Iterator that merges adjacent damaged lines into rectangles.
///
/// This reduces the number of draw calls needed for rendering by combining
/// consecutive damaged rows with overlapping column ranges into single rectangles.
pub struct MergedDamageIterator<'a> {
    inner: DamageBoundsIterator<'a>,
    pending: Option<DamageRect>,
}

impl<'a> MergedDamageIterator<'a> {
    /// Create a new merged damage iterator.
    pub fn new(damage: &'a Damage, rows: u16, cols: u16) -> Self {
        Self {
            inner: damage.iter_bounds(rows, cols),
            pending: None,
        }
    }
}

impl Iterator for MergedDamageIterator<'_> {
    type Item = DamageRect;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(bounds) => {
                    match &mut self.pending {
                        Some(rect) if rect.can_extend_with(&bounds) => {
                            // Extend current rectangle
                            rect.extend_with(&bounds);
                        }
                        Some(_) => {
                            // Return current rectangle, start new one
                            let result = self.pending.take();
                            self.pending = Some(DamageRect::from_line(bounds));
                            return result;
                        }
                        None => {
                            // Start new rectangle
                            self.pending = Some(DamageRect::from_line(bounds));
                        }
                    }
                }
                None => {
                    // No more lines, return pending if any
                    return self.pending.take();
                }
            }
        }
    }
}

impl Damage {
    /// Iterate over merged damage rectangles.
    ///
    /// This is useful for GPU rendering where batching adjacent rows
    /// into rectangles reduces draw calls.
    pub fn iter_merged(&self, rows: u16, cols: u16) -> MergedDamageIterator<'_> {
        MergedDamageIterator::new(self, rows, cols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_full() {
        let mut damage = Damage::new(24);
        assert!(!damage.is_full());
        damage.mark_full();
        assert!(damage.is_full());
        assert!(damage.is_row_damaged(0));
        assert!(damage.is_row_damaged(23));
    }

    #[test]
    fn damage_partial_row() {
        let mut damage = Damage::new(24);
        damage.mark_row(5);
        assert!(damage.is_row_damaged(5));
        assert!(!damage.is_row_damaged(4));
        assert!(!damage.is_row_damaged(6));
    }

    #[test]
    fn damage_row_bounds_clamped_to_cols() {
        let mut damage = Damage::new(10);
        damage.mark_row(3);
        assert_eq!(damage.row_damage_bounds(3, 80), Some((0, 80)));
    }

    #[test]
    fn damage_partial_cell() {
        let mut damage = Damage::new(24);
        damage.mark_cell(5, 10);
        damage.mark_cell(5, 20);
        assert!(damage.is_row_damaged(5));
        let bounds = damage.row_damage_bounds(5, 80);
        assert_eq!(bounds, Some((10, 21)));
    }

    #[test]
    fn damage_reset() {
        let mut damage = Damage::new(24);
        damage.mark_full();
        assert!(damage.is_full());
        damage.reset(24);
        assert!(!damage.is_full());
        assert!(!damage.is_row_damaged(0));
    }

    #[test]
    fn damage_iterator() {
        let mut damage = Damage::new(24);
        damage.mark_row(3);
        damage.mark_row(7);
        damage.mark_row(15);

        let damaged: Vec<_> = damage.damaged_rows(24).collect();
        assert_eq!(damaged, vec![3, 7, 15]);
    }

    #[test]
    fn tracker_many_rows() {
        let mut tracker = DamageTracker::new(200);
        tracker.mark_row(150);
        assert!(tracker.is_row_damaged(150));
        assert!(!tracker.is_row_damaged(149));
        assert!(!tracker.is_row_damaged(151));
    }

    #[test]
    fn damage_iterator_full() {
        let damage = Damage::Full;
        let damaged: Vec<_> = damage.damaged_rows(5).collect();
        assert_eq!(damaged, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn damage_iterator_sparse() {
        // Test bitset iteration with gaps
        let mut damage = Damage::new(100);
        damage.mark_row(0);
        damage.mark_row(63); // End of first word
        damage.mark_row(64); // Start of second word
        damage.mark_row(99);

        let damaged: Vec<_> = damage.damaged_rows(100).collect();
        assert_eq!(damaged, vec![0, 63, 64, 99]);
    }

    #[test]
    fn damage_iterator_empty() {
        let damage = Damage::new(24);
        let damaged: Vec<_> = damage.damaged_rows(24).collect();
        assert!(damaged.is_empty());
    }

    #[test]
    fn damage_has_damage() {
        let mut damage = Damage::new(24);
        assert!(!damage.has_damage());

        damage.mark_cell(5, 10);
        assert!(damage.has_damage());

        damage.reset(24);
        assert!(!damage.has_damage());

        damage.mark_full();
        assert!(damage.has_damage());
    }

    #[test]
    fn damage_iter_bounds() {
        let mut damage = Damage::new(24);
        damage.mark_cell(3, 10);
        damage.mark_cell(3, 20);
        damage.mark_row(7);
        damage.mark_cell(15, 5);

        let bounds: Vec<_> = damage.iter_bounds(24, 80).collect();
        assert_eq!(
            bounds,
            vec![
                LineDamageBounds::new(3, 10, 21),
                LineDamageBounds::new(7, 0, 80),
                LineDamageBounds::new(15, 5, 6),
            ]
        );
    }

    #[test]
    fn line_damage_bounds_merge() {
        let a = LineDamageBounds::new(5, 10, 30);
        let b = LineDamageBounds::new(6, 20, 40);

        assert!(a.can_merge_with(&b));
        let rect = a.merge_with(&b);
        assert_eq!(rect, DamageRect::new(5, 7, 10, 40));
    }

    #[test]
    fn line_damage_bounds_no_merge_gap() {
        let a = LineDamageBounds::new(5, 10, 20);
        let b = LineDamageBounds::new(7, 10, 20); // Row 6 missing

        assert!(!a.can_merge_with(&b));
    }

    #[test]
    fn damage_rect_extend() {
        let mut rect = DamageRect::new(5, 6, 10, 30);
        let bounds = LineDamageBounds::new(6, 5, 40);

        assert!(rect.can_extend_with(&bounds));
        rect.extend_with(&bounds);

        assert_eq!(rect, DamageRect::new(5, 7, 5, 40));
    }

    #[test]
    fn damage_iter_merged_consecutive_overlapping() {
        let mut damage = Damage::new(24);
        // Three consecutive rows with overlapping column damage
        // This will merge into a single rectangle
        damage.mark_cell(3, 10);
        damage.mark_cell(3, 20); // Row 3: [10, 21)
        damage.mark_cell(4, 15); // Row 4: [15, 16) - overlaps with row 3
        damage.mark_cell(5, 18); // Row 5: [18, 19) - overlaps with row 4's merged range

        let rects: Vec<_> = damage.iter_merged(24, 80).collect();
        // Should merge into one rectangle
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], DamageRect::new(3, 6, 10, 21));
    }

    #[test]
    fn damage_iter_merged_non_overlapping() {
        let mut damage = Damage::new(24);
        // Three consecutive rows but non-overlapping columns
        // Each row gets its own rectangle
        damage.mark_cell(3, 10); // Row 3: [10, 11)
        damage.mark_cell(4, 50); // Row 4: [50, 51) - doesn't overlap
        damage.mark_cell(5, 70); // Row 5: [70, 71) - doesn't overlap

        let rects: Vec<_> = damage.iter_merged(24, 80).collect();
        // Each row produces its own rectangle (no overlap)
        assert_eq!(rects.len(), 3);
    }

    #[test]
    fn damage_iter_merged_gap() {
        let mut damage = Damage::new(24);
        // Two groups separated by a gap row
        damage.mark_cell(3, 10);
        damage.mark_cell(3, 20); // Row 3: [10, 21)
        damage.mark_cell(4, 15); // Row 4: [15, 16) - overlaps
                                 // Gap at row 5
        damage.mark_cell(6, 10);
        damage.mark_cell(6, 20); // Row 6: [10, 21)
        damage.mark_cell(7, 15); // Row 7: [15, 16) - overlaps

        let rects: Vec<_> = damage.iter_merged(24, 80).collect();
        // Should produce two rectangles (gap at row 5)
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], DamageRect::new(3, 5, 10, 21));
        assert_eq!(rects[1], DamageRect::new(6, 8, 10, 21));
    }

    #[test]
    fn damage_iter_merged_full_rows() {
        let mut damage = Damage::new(24);
        // Full rows always overlap (0 to u16::MAX)
        damage.mark_row(3);
        damage.mark_row(4);
        damage.mark_row(5);

        let rects: Vec<_> = damage.iter_merged(24, 80).collect();
        // Should merge into one rectangle
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], DamageRect::new(3, 6, 0, 80));
    }

    #[test]
    fn damage_rect_dimensions() {
        let rect = DamageRect::new(5, 10, 20, 50);
        assert_eq!(rect.height(), 5);
        assert_eq!(rect.width(), 30);
        assert_eq!(rect.cell_count(), 150);
    }

    #[test]
    fn bitset_iterator_single_word() {
        // Bits are stored LSB first: bit 0 is row 0, bit 1 is row 1, etc.
        // 0b10101010 in binary puts bits at positions 1, 3, 5, 7 when read from bit 0
        let bits = vec![0b10101010u64]; // Bits 1, 3, 5, 7 set
        let iter = BitsetRowIterator::new(&bits, 64);
        let rows: Vec<_> = iter.collect();
        assert_eq!(rows, vec![1, 3, 5, 7]);
    }

    #[test]
    fn bitset_iterator_multi_word() {
        let mut bits = vec![0u64; 3];
        bits[0] = 1; // Row 0
        bits[1] = 1 << 5; // Row 69 (64 + 5)
        bits[2] = 1 << 10; // Row 138 (128 + 10)

        let iter = BitsetRowIterator::new(&bits, 200);
        let rows: Vec<_> = iter.collect();
        assert_eq!(rows, vec![0, 69, 138]);
    }

    #[test]
    fn bitset_iterator_respects_max() {
        let bits = vec![u64::MAX]; // All 64 bits set
        let iter = BitsetRowIterator::new(&bits, 10); // Only want first 10
        let rows: Vec<_> = iter.collect();
        assert_eq!(rows, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// DamageTracker bitset marking never goes out of bounds.
    #[kani::proof]
    #[kani::unwind(5)]
    fn damage_tracker_mark_row_bounds() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 100);

        let mut tracker = DamageTracker::new(rows);

        let row: u16 = kani::any();
        tracker.mark_row(row);

        // If row was in bounds, it should be marked
        // If out of bounds, should not crash
        if row < rows {
            kani::assert(tracker.is_row_damaged(row), "row should be marked");
        }
    }

    /// Cell damage tracking maintains proper bounds (left < right when damaged).
    #[kani::proof]
    #[kani::unwind(5)]
    fn damage_tracker_cell_bounds_valid() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let mut tracker = DamageTracker::new(rows);

        let row: u16 = kani::any();
        let col: u16 = kani::any();
        kani::assume(row < rows);

        tracker.mark_cell(row, col);

        // After marking a cell, bounds should be valid
        if let Some((left, right)) = tracker.row_damage_bounds(row) {
            kani::assert(left < right, "left must be less than right");
            kani::assert(left <= col, "left must be <= marked col");
            kani::assert(right > col, "right must be > marked col");
        } else {
            kani::assert(false, "bounds should exist after marking cell");
        }
    }

    /// Multiple cell marks expand bounds correctly.
    #[kani::proof]
    #[kani::unwind(5)]
    fn damage_tracker_cell_bounds_expand() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let mut tracker = DamageTracker::new(rows);

        let row: u16 = kani::any();
        let col1: u16 = kani::any();
        let col2: u16 = kani::any();
        kani::assume(row < rows);

        tracker.mark_cell(row, col1);
        tracker.mark_cell(row, col2);

        if let Some((left, right)) = tracker.row_damage_bounds(row) {
            let min_col = col1.min(col2);
            let max_col = col1.max(col2);
            kani::assert(left <= min_col, "left must cover minimum column");
            kani::assert(right > max_col, "right must be past maximum column");
        }
    }

    /// LineDamageBounds is_empty is correct.
    #[kani::proof]
    fn line_damage_bounds_is_empty_correct() {
        let line: u16 = kani::any();
        let left: u16 = kani::any();
        let right: u16 = kani::any();

        let bounds = LineDamageBounds::new(line, left, right);

        if left >= right {
            kani::assert(bounds.is_empty(), "should be empty when left >= right");
        } else {
            kani::assert(!bounds.is_empty(), "should not be empty when left < right");
        }
    }

    /// DamageRect dimensions are consistent.
    #[kani::proof]
    fn damage_rect_dimensions_consistent() {
        let top: u16 = kani::any();
        let bottom: u16 = kani::any();
        let left: u16 = kani::any();
        let right: u16 = kani::any();
        kani::assume(top <= bottom);
        kani::assume(left <= right);

        let rect = DamageRect::new(top, bottom, left, right);

        kani::assert(
            rect.height() == bottom.saturating_sub(top),
            "height calculation",
        );
        kani::assert(
            rect.width() == right.saturating_sub(left),
            "width calculation",
        );
        kani::assert(
            rect.cell_count() == (rect.height() as u32) * (rect.width() as u32),
            "cell count calculation",
        );
    }

    /// DamageRect from_line produces single-row rect.
    #[kani::proof]
    fn damage_rect_from_line_single_row() {
        let line: u16 = kani::any();
        let left: u16 = kani::any();
        let right: u16 = kani::any();
        kani::assume(left < right);
        kani::assume(line < u16::MAX); // Avoid overflow in bottom

        let bounds = LineDamageBounds::new(line, left, right);
        let rect = DamageRect::from_line(bounds);

        kani::assert(rect.top == line, "top should be line");
        kani::assert(rect.bottom == line + 1, "bottom should be line + 1");
        kani::assert(rect.left == left, "left preserved");
        kani::assert(rect.right == right, "right preserved");
        kani::assert(rect.height() == 1, "single row height");
    }

    /// LineDamageBounds can_merge_with is symmetric for adjacent lines.
    #[kani::proof]
    fn line_damage_bounds_merge_symmetric() {
        let line1: u16 = kani::any();
        let left1: u16 = kani::any();
        let right1: u16 = kani::any();
        let left2: u16 = kani::any();
        let right2: u16 = kani::any();

        kani::assume(left1 < right1);
        kani::assume(left2 < right2);
        kani::assume(line1 < u16::MAX);

        let a = LineDamageBounds::new(line1, left1, right1);
        let b = LineDamageBounds::new(line1 + 1, left2, right2);

        // If a can merge with b, b should be able to merge with a
        // (since they're adjacent and overlap check is symmetric)
        if a.can_merge_with(&b) {
            kani::assert(b.can_merge_with(&a), "merge should be symmetric");
        }
    }

    /// DamageRect extend_with grows correctly.
    #[kani::proof]
    fn damage_rect_extend_grows() {
        let top: u16 = kani::any();
        let left: u16 = kani::any();
        let right: u16 = kani::any();

        kani::assume(top > 0 && top < 100);
        kani::assume(left < right);
        kani::assume(right < 1000);

        let mut rect = DamageRect::new(top, top + 1, left, right);

        let new_left: u16 = kani::any();
        let new_right: u16 = kani::any();
        kani::assume(new_left < new_right);
        kani::assume(new_right < 1000);

        // Line immediately below the rect
        let bounds = LineDamageBounds::new(top + 1, new_left, new_right);

        if rect.can_extend_with(&bounds) {
            let old_top = rect.top;
            rect.extend_with(&bounds);

            kani::assert(rect.top == old_top, "top unchanged");
            kani::assert(rect.bottom == top + 2, "bottom extended by 1");
            kani::assert(rect.left <= new_left.min(left), "left covers both");
            kani::assert(rect.right >= new_right.max(right), "right covers both");
        }
    }

    /// Damage row bounds are clamped to cols.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_row_bounds_clamped() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);
        kani::assume(cols > 0 && cols <= 100);

        let mut damage = Damage::new(rows);

        let row: u16 = kani::any();
        kani::assume(row < rows);

        damage.mark_row(row);

        if let Some((left, right)) = damage.row_damage_bounds(row, cols) {
            kani::assert(left <= cols, "left clamped to cols");
            kani::assert(right <= cols, "right clamped to cols");
        }
    }

    // =========================================================================
    // Damage State Machine Proofs (FV-13)
    // =========================================================================
    // These proofs verify the state transitions of the Damage enum:
    // - Damage::new() -> Partial
    // - mark_full() -> Full
    // - reset() -> Partial
    // - Operations preserve/transition state correctly

    /// Damage::new creates Partial state.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_new_creates_partial() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let damage = Damage::new(rows);

        kani::assert(!damage.is_full(), "new damage should be Partial, not Full");
        kani::assert(!damage.has_damage(), "new damage should have no damage");
    }

    /// mark_full transitions to Full state.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_mark_full_transitions_to_full() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let mut damage = Damage::new(rows);
        kani::assert(!damage.is_full(), "starts as Partial");

        damage.mark_full();

        kani::assert(damage.is_full(), "should be Full after mark_full");
        kani::assert(damage.has_damage(), "Full always has damage");
    }

    /// reset transitions Full back to Partial.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_reset_transitions_full_to_partial() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let mut damage = Damage::new(rows);
        damage.mark_full();
        kani::assert(damage.is_full(), "is Full before reset");

        damage.reset(rows);

        kani::assert(!damage.is_full(), "should be Partial after reset");
        kani::assert(!damage.has_damage(), "reset clears all damage");
    }

    /// mark_row does not change state from Partial.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_mark_row_preserves_partial() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let mut damage = Damage::new(rows);
        kani::assert(!damage.is_full(), "starts as Partial");

        let row: u16 = kani::any();
        damage.mark_row(row);

        kani::assert(!damage.is_full(), "mark_row should not transition to Full");
    }

    /// mark_rows does not change state from Partial.
    #[kani::proof]
    #[kani::unwind(10)]
    fn damage_mark_rows_preserves_partial() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 20);

        let mut damage = Damage::new(rows);

        let start: u16 = kani::any();
        let end: u16 = kani::any();
        kani::assume(start <= end);
        kani::assume(end <= rows);

        damage.mark_rows(start, end);

        kani::assert(!damage.is_full(), "mark_rows should not transition to Full");
    }

    /// mark_cell does not change state from Partial.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_mark_cell_preserves_partial() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let mut damage = Damage::new(rows);

        let row: u16 = kani::any();
        let col: u16 = kani::any();

        damage.mark_cell(row, col);

        kani::assert(!damage.is_full(), "mark_cell should not transition to Full");
    }

    /// Operations on Full state are idempotent (no-ops).
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_full_operations_idempotent() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let mut damage = Damage::new(rows);
        damage.mark_full();

        // All rows should report damaged when Full
        let row: u16 = kani::any();
        kani::assume(row < rows);

        kani::assert(damage.is_row_damaged(row), "all rows damaged when Full");

        // Operations should not change the Full state
        damage.mark_row(row);
        kani::assert(damage.is_full(), "still Full after mark_row");

        damage.mark_cell(row, 0);
        kani::assert(damage.is_full(), "still Full after mark_cell");

        damage.mark_full();
        kani::assert(damage.is_full(), "still Full after mark_full");
    }

    /// Full damage reports all rows as damaged.
    #[kani::proof]
    fn damage_full_all_rows_damaged() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 100);

        let damage = Damage::Full;

        let row: u16 = kani::any();
        kani::assume(row < rows);

        kani::assert(
            damage.is_row_damaged(row),
            "Full should report all rows damaged",
        );
    }

    /// Full damage row_damage_bounds returns full column range.
    #[kani::proof]
    fn damage_full_row_bounds_full_width() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 100);
        kani::assume(cols > 0 && cols <= 200);

        let damage = Damage::Full;

        let row: u16 = kani::any();
        kani::assume(row < rows);

        let bounds = damage.row_damage_bounds(row, cols);
        kani::assert(
            bounds == Some((0, cols)),
            "Full damage should return (0, cols)",
        );
    }

    /// State machine: Partial -> Full -> Partial cycle.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_state_machine_cycle() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        // Start: Partial (no damage)
        let mut damage = Damage::new(rows);
        kani::assert(!damage.is_full(), "state 0: Partial");
        kani::assert(!damage.has_damage(), "state 0: no damage");

        // Add some damage
        let row: u16 = kani::any();
        kani::assume(row < rows);
        damage.mark_row(row);
        kani::assert(!damage.is_full(), "state 1: still Partial");
        kani::assert(damage.has_damage(), "state 1: has damage");

        // Transition to Full
        damage.mark_full();
        kani::assert(damage.is_full(), "state 2: Full");
        kani::assert(damage.has_damage(), "state 2: has damage");

        // Reset back to Partial
        damage.reset(rows);
        kani::assert(!damage.is_full(), "state 3: Partial again");
        kani::assert(!damage.has_damage(), "state 3: no damage");
    }

    /// Partial damage: unmarked row reports not damaged.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_partial_unmarked_not_damaged() {
        let rows: u16 = kani::any();
        kani::assume(rows >= 2 && rows <= 50);

        let mut damage = Damage::new(rows);

        // Mark one row
        let marked_row: u16 = kani::any();
        kani::assume(marked_row < rows);
        damage.mark_row(marked_row);

        // Check a different row
        let check_row: u16 = kani::any();
        kani::assume(check_row < rows);
        kani::assume(check_row != marked_row);

        kani::assert(
            !damage.is_row_damaged(check_row),
            "unmarked row should not be damaged",
        );
    }

    /// Partial damage: marked row reports damaged.
    #[kani::proof]
    #[kani::unwind(3)]
    fn damage_partial_marked_is_damaged() {
        let rows: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);

        let mut damage = Damage::new(rows);

        let row: u16 = kani::any();
        kani::assume(row < rows);

        damage.mark_row(row);

        kani::assert(damage.is_row_damaged(row), "marked row should be damaged");
    }
}
