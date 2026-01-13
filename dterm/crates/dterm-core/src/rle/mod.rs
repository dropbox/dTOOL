//! Run-Length Encoding (RLE) for cell attributes.
//!
//! This module provides RLE compression for terminal cell attributes based on
//! Windows Terminal's `til/rle.h` pattern. Attributes are compressed into runs
//! of consecutive cells with identical style.
//!
//! ## Design
//!
//! Terminal output often has runs of cells with identical attributes (e.g., a
//! prompt in one color, then text in another). RLE compression exploits this
//! by storing `(style, count)` pairs instead of per-cell styles.
//!
//! ## Architecture
//!
//! ```text
//! Row Storage (before RLE):
//! [Cell0][Cell1][Cell2][Cell3][Cell4][Cell5][Cell6][Cell7]
//!  Bold   Bold   Bold   Bold  Normal Normal Normal Normal
//!
//! Row Storage (with RLE attributes):
//! Characters: [C0][C1][C2][C3][C4][C5][C6][C7]
//! Attributes: [(Bold, 4), (Normal, 4)]
//! ```
//!
//! ## References
//!
//! - Windows Terminal: `src/inc/til/rle.h`
//! - Ghostty: Style ID indirection in `page.zig`

use std::ops::Index;

/// A run of cells with identical attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run<T> {
    /// The attribute value for this run.
    pub value: T,
    /// Number of consecutive cells with this attribute.
    pub length: u32,
}

impl<T: Default> Default for Run<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
            length: 0,
        }
    }
}

/// Run-Length Encoded sequence of attributes.
///
/// Stores a sequence of attributes as runs of consecutive identical values.
/// Provides O(log n) random access and efficient range operations.
///
/// # Type Parameters
///
/// - `T`: The attribute type (must be `Copy + PartialEq + Default`)
#[derive(Debug, Clone)]
pub struct Rle<T> {
    /// The runs in order.
    runs: Vec<Run<T>>,
    /// Total length (sum of all run lengths).
    total_length: u32,
}

impl<T: Copy + PartialEq + Default> Default for Rle<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + PartialEq + Default> Rle<T> {
    /// Create an empty RLE sequence.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runs: Vec::new(),
            total_length: 0,
        }
    }

    /// Create an RLE sequence with a single value repeated `length` times.
    #[must_use]
    pub fn with_value(value: T, length: u32) -> Self {
        if length == 0 {
            return Self::new();
        }
        Self {
            runs: vec![Run { value, length }],
            total_length: length,
        }
    }

    /// Create an RLE sequence from an iterator of values.
    pub fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut rle = Self::new();
        for value in iter {
            rle.push(value);
        }
        rle
    }

    /// Get the total length of the sequence.
    #[must_use]
    #[inline]
    pub fn len(&self) -> u32 {
        self.total_length
    }

    /// Check if the sequence is empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.total_length == 0
    }

    /// Get the number of runs.
    #[must_use]
    #[inline]
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// Get a reference to the runs.
    #[must_use]
    #[inline]
    pub fn runs(&self) -> &[Run<T>] {
        &self.runs
    }

    /// Clear the sequence.
    pub fn clear(&mut self) {
        self.runs.clear();
        self.total_length = 0;
    }

    /// Push a single value onto the end.
    pub fn push(&mut self, value: T) {
        if let Some(last) = self.runs.last_mut() {
            if last.value == value {
                last.length += 1;
                self.total_length += 1;
                return;
            }
        }
        self.runs.push(Run { value, length: 1 });
        self.total_length += 1;
    }

    /// Extend with multiple copies of the same value.
    pub fn extend_with(&mut self, value: T, count: u32) {
        if count == 0 {
            return;
        }
        if let Some(last) = self.runs.last_mut() {
            if last.value == value {
                last.length += count;
                self.total_length += count;
                return;
            }
        }
        self.runs.push(Run {
            value,
            length: count,
        });
        self.total_length += count;
    }

    /// Get the value at a specific index.
    ///
    /// Returns `None` if index is out of bounds.
    #[must_use]
    pub fn get(&self, index: u32) -> Option<T> {
        if index >= self.total_length {
            return None;
        }
        let (run_idx, _) = self.find_run(index)?;
        Some(self.runs[run_idx].value)
    }

    /// Set the value at a specific index.
    ///
    /// Returns `false` if index is out of bounds.
    pub fn set(&mut self, index: u32, value: T) -> bool {
        if index >= self.total_length {
            return false;
        }

        let Some((run_idx, offset_in_run)) = self.find_run(index) else {
            return false;
        };

        let run = &self.runs[run_idx];
        if run.value == value {
            // Value already matches
            return true;
        }

        // Need to split the run
        self.split_and_set(run_idx, offset_in_run, value);
        true
    }

    /// Set a range of values to the same attribute.
    ///
    /// This is the most efficient way to update multiple cells.
    pub fn set_range(&mut self, start: u32, end: u32, value: T) {
        if start >= end || start >= self.total_length {
            return;
        }
        let end = end.min(self.total_length);

        // Fast path: entire sequence
        if start == 0 && end == self.total_length {
            self.runs.clear();
            self.runs.push(Run {
                value,
                length: self.total_length,
            });
            return;
        }

        // Find start and end runs
        let Some((start_run_idx, start_offset)) = self.find_run(start) else {
            return;
        };

        // We need to find the end run relative to the current state
        // The end-1 index gives us the last cell to modify
        let Some((end_run_idx, end_offset)) = self.find_run(end - 1) else {
            return;
        };

        // Simple case: same run
        if start_run_idx == end_run_idx {
            let run = &self.runs[start_run_idx];
            if run.value == value {
                return; // Already the correct value
            }
            // Split the run into up to 3 parts
            self.split_range_single_run(start_run_idx, start_offset, end_offset + 1, value);
            return;
        }

        // Complex case: spans multiple runs
        self.split_range_multi_run(
            start_run_idx,
            start_offset,
            end_run_idx,
            end_offset + 1,
            value,
        );
    }

    /// Resize the sequence, extending with default value or truncating.
    pub fn resize(&mut self, new_length: u32) {
        if new_length == self.total_length {
            return;
        }

        if new_length == 0 {
            self.clear();
            return;
        }

        if new_length > self.total_length {
            // Extend with default
            self.extend_with(T::default(), new_length - self.total_length);
        } else {
            // Truncate
            self.truncate(new_length);
        }
    }

    /// Resize, extending with a specific value.
    pub fn resize_with(&mut self, new_length: u32, value: T) {
        if new_length == self.total_length {
            return;
        }

        if new_length == 0 {
            self.clear();
            return;
        }

        if new_length > self.total_length {
            self.extend_with(value, new_length - self.total_length);
        } else {
            self.truncate(new_length);
        }
    }

    /// Truncate to a specific length.
    fn truncate(&mut self, new_length: u32) {
        if new_length >= self.total_length {
            return;
        }

        if new_length == 0 {
            self.clear();
            return;
        }

        // Find the run containing the new end
        let mut accumulated = 0u32;
        for (i, run) in self.runs.iter_mut().enumerate() {
            if accumulated + run.length >= new_length {
                // This run contains the new end
                let keep_in_run = new_length - accumulated;
                run.length = keep_in_run;
                self.runs.truncate(i + 1);
                self.total_length = new_length;
                return;
            }
            accumulated += run.length;
        }
    }

    /// Find the run containing an index.
    ///
    /// Returns `(run_index, offset_within_run)`.
    fn find_run(&self, index: u32) -> Option<(usize, u32)> {
        let mut accumulated = 0u32;
        for (i, run) in self.runs.iter().enumerate() {
            if accumulated + run.length > index {
                return Some((i, index - accumulated));
            }
            accumulated += run.length;
        }
        None
    }

    /// Split a single run to set a value at a specific offset.
    fn split_and_set(&mut self, run_idx: usize, offset: u32, value: T) {
        let run = &self.runs[run_idx];
        let run_len = run.length;
        let old_value = run.value;

        if run_len == 1 {
            // Simple case: run of length 1
            self.runs[run_idx].value = value;
            self.compact_around(run_idx);
            return;
        }

        if offset == 0 {
            // At start of run
            self.runs[run_idx].length -= 1;
            self.runs.insert(run_idx, Run { value, length: 1 });
            self.compact_around(run_idx);
        } else if offset == run_len - 1 {
            // At end of run
            self.runs[run_idx].length -= 1;
            self.runs.insert(run_idx + 1, Run { value, length: 1 });
            self.compact_around(run_idx + 1);
        } else {
            // In middle - split into 3
            let after_len = run_len - offset - 1;
            self.runs[run_idx].length = offset;
            self.runs.insert(run_idx + 1, Run { value, length: 1 });
            self.runs.insert(
                run_idx + 2,
                Run {
                    value: old_value,
                    length: after_len,
                },
            );
        }
    }

    /// Split a single run to set a range to a new value.
    fn split_range_single_run(
        &mut self,
        run_idx: usize,
        start_offset: u32,
        end_offset: u32,
        value: T,
    ) {
        let run = &self.runs[run_idx];
        let run_len = run.length;
        let old_value = run.value;
        let range_len = end_offset - start_offset;

        if start_offset == 0 && end_offset >= run_len {
            // Replace entire run
            self.runs[run_idx].value = value;
            self.compact_around(run_idx);
            return;
        }

        let mut new_runs = Vec::with_capacity(3);

        // Before part
        if start_offset > 0 {
            new_runs.push(Run {
                value: old_value,
                length: start_offset,
            });
        }

        // Replaced part
        new_runs.push(Run {
            value,
            length: range_len,
        });

        // After part
        if end_offset < run_len {
            new_runs.push(Run {
                value: old_value,
                length: run_len - end_offset,
            });
        }

        // Replace the run with new runs
        self.runs.splice(run_idx..=run_idx, new_runs);
        self.compact();
    }

    /// Split multiple runs to set a range to a new value.
    fn split_range_multi_run(
        &mut self,
        start_run_idx: usize,
        start_offset: u32,
        end_run_idx: usize,
        end_offset: u32,
        value: T,
    ) {
        // Calculate total length of the range
        let mut range_len = 0u32;
        for i in start_run_idx..=end_run_idx {
            let run = &self.runs[i];
            if i == start_run_idx {
                range_len += run.length - start_offset;
            } else if i == end_run_idx {
                range_len += end_offset;
            } else {
                range_len += run.length;
            }
        }

        let mut new_runs = Vec::new();

        // Before part from start run
        let start_run = &self.runs[start_run_idx];
        if start_offset > 0 {
            new_runs.push(Run {
                value: start_run.value,
                length: start_offset,
            });
        }

        // The new range
        new_runs.push(Run {
            value,
            length: range_len,
        });

        // After part from end run
        let end_run = &self.runs[end_run_idx];
        if end_offset < end_run.length {
            new_runs.push(Run {
                value: end_run.value,
                length: end_run.length - end_offset,
            });
        }

        // Replace the runs
        self.runs.splice(start_run_idx..=end_run_idx, new_runs);
        self.compact();
    }

    /// Compact adjacent runs with the same value.
    fn compact(&mut self) {
        if self.runs.len() <= 1 {
            return;
        }

        let mut write = 0;
        for read in 1..self.runs.len() {
            if self.runs[write].value == self.runs[read].value {
                self.runs[write].length += self.runs[read].length;
            } else {
                write += 1;
                if write != read {
                    self.runs[write] = self.runs[read];
                }
            }
        }
        self.runs.truncate(write + 1);
    }

    /// Compact around a specific index.
    fn compact_around(&mut self, idx: usize) {
        // Merge with previous
        if idx > 0 && self.runs[idx - 1].value == self.runs[idx].value {
            self.runs[idx - 1].length += self.runs[idx].length;
            self.runs.remove(idx);
            // Check if we need to merge with next (now at idx-1)
            if idx < self.runs.len() && self.runs[idx - 1].value == self.runs[idx].value {
                self.runs[idx - 1].length += self.runs[idx].length;
                self.runs.remove(idx);
            }
            return;
        }

        // Merge with next
        if idx + 1 < self.runs.len() && self.runs[idx].value == self.runs[idx + 1].value {
            self.runs[idx].length += self.runs[idx + 1].length;
            self.runs.remove(idx + 1);
        }
    }

    /// Iterate over all values (expanded).
    pub fn iter(&self) -> RleIter<'_, T> {
        RleIter {
            runs: &self.runs,
            run_idx: 0,
            offset_in_run: 0,
        }
    }

    /// Iterate over runs.
    pub fn iter_runs(&self) -> impl Iterator<Item = &Run<T>> {
        self.runs.iter()
    }
}

impl<T: Copy + PartialEq + Default> Index<u32> for Rle<T> {
    type Output = T;

    fn index(&self, index: u32) -> &Self::Output {
        let (run_idx, _) = self.find_run(index).expect("index out of bounds");
        &self.runs[run_idx].value
    }
}

/// Iterator over expanded RLE values.
pub struct RleIter<'a, T> {
    runs: &'a [Run<T>],
    run_idx: usize,
    offset_in_run: u32,
}

impl<'a, T: Copy> Iterator for RleIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.run_idx >= self.runs.len() {
            return None;
        }

        let run = &self.runs[self.run_idx];
        let value = run.value;

        self.offset_in_run += 1;
        if self.offset_in_run >= run.length {
            self.run_idx += 1;
            self.offset_in_run = 0;
        }

        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let mut remaining = 0usize;
        for run in &self.runs[self.run_idx..] {
            remaining += run.length as usize;
        }
        remaining = remaining.saturating_sub(self.offset_in_run as usize);
        (remaining, Some(remaining))
    }
}

impl<'a, T: Copy> ExactSizeIterator for RleIter<'a, T> {}

/// Style ID for compressed cell attributes.
///
/// This is a compact identifier that references a style in a style registry.
/// Default style (no attributes) always has ID 0.
pub type StyleId = u16;

/// The default style ID (no special attributes).
pub const DEFAULT_STYLE_ID: StyleId = 0;

/// Compressed cell attributes using style IDs.
///
/// This provides a more memory-efficient representation when many cells
/// share the same style. The actual style data is stored in a separate
/// registry, and cells reference styles by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressedStyle {
    /// Foreground color.
    pub fg: u32,
    /// Background color.
    pub bg: u32,
    /// Flags (from CellFlags).
    pub flags: u16,
}

/// Default fg color (0xFF_FFFFFF - default type marker + white).
const DEFAULT_FG: u32 = 0xFF_FF_FF_FF;
/// Default bg color (0xFF_000000 - default type marker + black).
const DEFAULT_BG: u32 = 0xFF_00_00_00;

impl Default for CompressedStyle {
    fn default() -> Self {
        Self {
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            flags: 0,
        }
    }
}

impl CompressedStyle {
    /// Create a new compressed style.
    #[must_use]
    pub const fn new(fg: u32, bg: u32, flags: u16) -> Self {
        Self { fg, bg, flags }
    }

    /// Check if this is the default style.
    #[must_use]
    pub const fn is_default(&self) -> bool {
        self.fg == DEFAULT_FG && self.bg == DEFAULT_BG && self.flags == 0
    }
}

/// A style registry that deduplicates styles and assigns IDs.
///
/// Similar to Ghostty's style set, this allows cells to reference styles
/// by compact IDs rather than storing full style data.
#[derive(Debug, Clone)]
pub struct StyleRegistry {
    /// Styles indexed by ID. ID 0 is always the default style.
    styles: Vec<CompressedStyle>,
    /// Generation counter for invalidation.
    generation: u64,
}

impl Default for StyleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl StyleRegistry {
    /// Create a new style registry with default style at ID 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            styles: vec![CompressedStyle::default()],
            generation: 0,
        }
    }

    /// Get or create a style ID for the given style.
    ///
    /// If the style already exists, returns its ID.
    /// Otherwise, allocates a new ID.
    pub fn get_or_insert(&mut self, style: CompressedStyle) -> StyleId {
        // Default style is always ID 0
        if style.is_default() {
            return DEFAULT_STYLE_ID;
        }

        // Search for existing style
        // StyleId is u16, so id will fit since we cap at u16::MAX styles
        for (id, existing) in self.styles.iter().enumerate() {
            if *existing == style {
                #[allow(clippy::cast_possible_truncation)]
                return id as StyleId;
            }
        }

        // Allocate new ID
        // Style count is bounded by terminal usage (typically < 100 distinct styles)
        // Saturate at u16::MAX for safety
        #[allow(clippy::cast_possible_truncation)]
        let id = self.styles.len().min(StyleId::MAX as usize) as StyleId;
        self.styles.push(style);
        self.generation += 1;
        id
    }

    /// Get the style for an ID.
    ///
    /// Returns `None` if the ID is invalid.
    #[must_use]
    pub fn get(&self, id: StyleId) -> Option<&CompressedStyle> {
        self.styles.get(id as usize)
    }

    /// Get the default style.
    #[must_use]
    pub fn default_style(&self) -> &CompressedStyle {
        &self.styles[0]
    }

    /// Get the number of unique styles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Check if only the default style exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.styles.len() == 1
    }

    /// Get the current generation (for cache invalidation).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Clear all styles except the default.
    pub fn clear(&mut self) {
        self.styles.truncate(1);
        self.generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle_new_empty() {
        let rle: Rle<u8> = Rle::new();
        assert!(rle.is_empty());
        assert_eq!(rle.len(), 0);
        assert_eq!(rle.run_count(), 0);
    }

    #[test]
    fn rle_with_value() {
        let rle = Rle::with_value(42u8, 10);
        assert_eq!(rle.len(), 10);
        assert_eq!(rle.run_count(), 1);
        assert_eq!(rle.get(0), Some(42));
        assert_eq!(rle.get(9), Some(42));
        assert_eq!(rle.get(10), None);
    }

    #[test]
    fn rle_push_same_value() {
        let mut rle: Rle<u8> = Rle::new();
        rle.push(1);
        rle.push(1);
        rle.push(1);
        assert_eq!(rle.len(), 3);
        assert_eq!(rle.run_count(), 1);
    }

    #[test]
    fn rle_push_different_values() {
        let mut rle: Rle<u8> = Rle::new();
        rle.push(1);
        rle.push(2);
        rle.push(3);
        assert_eq!(rle.len(), 3);
        assert_eq!(rle.run_count(), 3);
    }

    #[test]
    fn rle_from_iter() {
        let rle = Rle::from_iter([1u8, 1, 1, 2, 2, 3, 3, 3, 3]);
        assert_eq!(rle.len(), 9);
        assert_eq!(rle.run_count(), 3);
        assert_eq!(rle.runs()[0].length, 3);
        assert_eq!(rle.runs()[1].length, 2);
        assert_eq!(rle.runs()[2].length, 4);
    }

    #[test]
    fn rle_get() {
        let rle = Rle::from_iter([1u8, 1, 2, 2, 2, 3]);
        assert_eq!(rle.get(0), Some(1));
        assert_eq!(rle.get(1), Some(1));
        assert_eq!(rle.get(2), Some(2));
        assert_eq!(rle.get(4), Some(2));
        assert_eq!(rle.get(5), Some(3));
        assert_eq!(rle.get(6), None);
    }

    #[test]
    fn rle_set_same_value() {
        let mut rle = Rle::from_iter([1u8, 1, 1]);
        assert!(rle.set(1, 1));
        assert_eq!(rle.run_count(), 1);
    }

    #[test]
    fn rle_set_middle() {
        let mut rle = Rle::from_iter([1u8, 1, 1, 1, 1]);
        assert!(rle.set(2, 9));
        assert_eq!(rle.get(2), Some(9));
        assert_eq!(rle.run_count(), 3);
        assert_eq!(rle.len(), 5);
    }

    #[test]
    fn rle_set_start() {
        let mut rle = Rle::from_iter([1u8, 1, 1]);
        assert!(rle.set(0, 9));
        assert_eq!(rle.get(0), Some(9));
        assert_eq!(rle.run_count(), 2);
    }

    #[test]
    fn rle_set_end() {
        let mut rle = Rle::from_iter([1u8, 1, 1]);
        assert!(rle.set(2, 9));
        assert_eq!(rle.get(2), Some(9));
        assert_eq!(rle.run_count(), 2);
    }

    #[test]
    fn rle_set_range_entire() {
        let mut rle = Rle::from_iter([1u8, 2, 3, 4, 5]);
        rle.set_range(0, 5, 9);
        assert_eq!(rle.run_count(), 1);
        assert_eq!(rle.get(0), Some(9));
        assert_eq!(rle.get(4), Some(9));
    }

    #[test]
    fn rle_set_range_partial() {
        let mut rle = Rle::from_iter([1u8, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
        rle.set_range(2, 5, 9);
        assert_eq!(rle.get(0), Some(1));
        assert_eq!(rle.get(2), Some(9));
        assert_eq!(rle.get(4), Some(9));
        assert_eq!(rle.get(5), Some(1));
    }

    #[test]
    fn rle_set_range_across_runs() {
        let mut rle = Rle::from_iter([1u8, 1, 2, 2, 3, 3]);
        rle.set_range(1, 5, 9);
        assert_eq!(rle.get(0), Some(1));
        assert_eq!(rle.get(1), Some(9));
        assert_eq!(rle.get(4), Some(9));
        assert_eq!(rle.get(5), Some(3));
    }

    #[test]
    fn rle_resize_grow() {
        let mut rle = Rle::from_iter([1u8, 1, 1]);
        rle.resize(5);
        assert_eq!(rle.len(), 5);
        assert_eq!(rle.get(3), Some(0)); // Default value
    }

    #[test]
    fn rle_resize_shrink() {
        let mut rle = Rle::from_iter([1u8, 1, 1, 1, 1]);
        rle.resize(3);
        assert_eq!(rle.len(), 3);
        assert_eq!(rle.get(3), None);
    }

    #[test]
    fn rle_iter() {
        let rle = Rle::from_iter([1u8, 1, 2, 3, 3]);
        let values: Vec<_> = rle.iter().collect();
        assert_eq!(values, vec![1, 1, 2, 3, 3]);
    }

    #[test]
    fn rle_compact_on_set() {
        let mut rle = Rle::from_iter([1u8, 2, 1]);
        // Set middle to match adjacent
        rle.set(1, 1);
        assert_eq!(rle.run_count(), 1);
        assert_eq!(rle.len(), 3);
    }

    #[test]
    fn style_registry_default() {
        let registry = StyleRegistry::new();
        assert_eq!(registry.len(), 1);
        assert!(registry.default_style().is_default());
    }

    #[test]
    fn style_registry_get_or_insert() {
        let mut registry = StyleRegistry::new();

        // Default style should return ID 0
        let default_id = registry.get_or_insert(CompressedStyle::default());
        assert_eq!(default_id, DEFAULT_STYLE_ID);

        // New style should get new ID
        let style1 = CompressedStyle::new(0xFF0000FF, 0xFF_000000, 1);
        let id1 = registry.get_or_insert(style1);
        assert_eq!(id1, 1);

        // Same style should return same ID
        let id1_again = registry.get_or_insert(style1);
        assert_eq!(id1_again, id1);

        // Different style should get different ID
        let style2 = CompressedStyle::new(0xFF00FF00, 0xFF_000000, 0);
        let id2 = registry.get_or_insert(style2);
        assert_eq!(id2, 2);
    }

    #[test]
    fn style_registry_get() {
        let mut registry = StyleRegistry::new();
        let style = CompressedStyle::new(0xFF0000FF, 0xFF_000000, 1);
        let id = registry.get_or_insert(style);

        assert_eq!(registry.get(id), Some(&style));
        assert!(registry.get(999).is_none());
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// RLE length is always the sum of run lengths.
    #[kani::proof]
    fn rle_length_consistent() {
        let len1: u8 = kani::any();
        let len2: u8 = kani::any();
        kani::assume(len1 > 0 && len1 < 50);
        kani::assume(len2 > 0 && len2 < 50);

        let mut rle: Rle<u8> = Rle::new();
        rle.extend_with(1, len1 as u32);
        rle.extend_with(2, len2 as u32);

        kani::assert(
            rle.len() == (len1 as u32) + (len2 as u32),
            "length should be sum of extensions",
        );
    }

    /// Get always returns a value for valid indices.
    #[kani::proof]
    fn rle_get_valid_index() {
        let len: u8 = kani::any();
        let idx: u8 = kani::any();
        kani::assume(len > 0 && len <= 100);
        kani::assume(idx < len);

        let rle = Rle::with_value(42u8, len as u32);
        kani::assert(
            rle.get(idx as u32).is_some(),
            "valid index should return Some",
        );
    }

    /// Get returns None for out-of-bounds indices.
    #[kani::proof]
    fn rle_get_invalid_index() {
        let len: u8 = kani::any();
        kani::assume(len > 0 && len < 100);

        let rle = Rle::with_value(42u8, len as u32);
        kani::assert(
            rle.get(len as u32).is_none(),
            "out-of-bounds should return None",
        );
    }

    /// Set preserves total length.
    #[kani::proof]
    fn rle_set_preserves_length() {
        let len: u8 = kani::any();
        let idx: u8 = kani::any();
        kani::assume(len > 0 && len <= 50);
        kani::assume(idx < len);

        let mut rle = Rle::with_value(1u8, len as u32);
        let original_len = rle.len();
        rle.set(idx as u32, 99);

        kani::assert(rle.len() == original_len, "set should preserve length");
    }

    /// Resize to larger adds correct amount.
    #[kani::proof]
    fn rle_resize_grow_correct() {
        let initial: u8 = kani::any();
        let final_len: u8 = kani::any();
        kani::assume(initial > 0 && initial < 50);
        kani::assume(final_len > initial && final_len < 100);

        let mut rle = Rle::with_value(42u8, initial as u32);
        rle.resize(final_len as u32);

        kani::assert(
            rle.len() == final_len as u32,
            "resize should set correct length",
        );
    }

    /// Resize to smaller truncates correctly.
    #[kani::proof]
    fn rle_resize_shrink_correct() {
        let initial: u8 = kani::any();
        let final_len: u8 = kani::any();
        kani::assume(initial > 1 && initial <= 100);
        kani::assume(final_len > 0 && final_len < initial);

        let mut rle = Rle::with_value(42u8, initial as u32);
        rle.resize(final_len as u32);

        kani::assert(
            rle.len() == final_len as u32,
            "resize should truncate correctly",
        );
    }

    /// Style registry default always has ID 0.
    #[kani::proof]
    fn style_registry_default_is_zero() {
        let mut registry = StyleRegistry::new();
        let id = registry.get_or_insert(CompressedStyle::default());
        kani::assert(id == DEFAULT_STYLE_ID, "default style should have ID 0");
    }

    /// Style registry deduplicates identical styles.
    #[kani::proof]
    fn style_registry_deduplicates() {
        let fg: u32 = kani::any();
        let bg: u32 = kani::any();
        let flags: u16 = kani::any();

        let mut registry = StyleRegistry::new();
        let style = CompressedStyle::new(fg, bg, flags);
        let id1 = registry.get_or_insert(style);
        let id2 = registry.get_or_insert(style);

        kani::assert(id1 == id2, "identical styles should get same ID");
    }
}
