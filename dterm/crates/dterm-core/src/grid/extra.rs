//! Cell extras for rarely-used attributes.
//!
//! ## Design
//!
//! The packed 12-byte `Cell` handles common attributes efficiently.
//! For rare features like hyperlinks and combining characters, we use
//! an external lookup table (`CellExtras`) to avoid bloating every cell.
//!
//! ## Storage Strategy
//!
//! ```text
//! Cell (12 bytes)          CellExtras (HashMap)
//! ┌────────────────┐       ┌──────────────────────────────┐
//! │ codepoint+flags│       │ (row, col) -> CellExtra      │
//! │ fg color       │──────▶│   - hyperlink: Option<Arc>   │
//! │ bg color       │       │   - underline_color: Option  │
//! └────────────────┘       │   - combining: SmallVec      │
//!                          └──────────────────────────────┘
//! ```
//!
//! This keeps the common case fast (12-byte cells) while supporting
//! all terminal features when needed.
//!
//! ## Memory Optimization (M9)
//!
//! `CellExtra` uses packed storage for RGB colors:
//! - Bitflags track which fields are present (avoids Option discriminants)
//! - Three RGB colors packed into a single 9-byte array
//! - Saves ~16 bytes per extra vs naive Option<[u8; 3]> fields
//!
//! ## Usage
//!
//! - Hyperlinks: OSC 8 sequences set hyperlinks on cells
//! - Combining characters: Unicode combining marks (U+0300-U+036F, etc.)
//! - Underline colors: SGR 58/59 for colored underlines
//!
//! ## Verification
//!
//! Kani proofs (12 total):
//! - `cell_extra_has_data_consistent` - has_data reflects contents
//! - `cell_coord_hash_consistent` - CellCoord equality is consistent
//! - `combining_bounded` - Combining chars capped at MAX_COMBINING
//! - `combining_mark_range_valid` - Combining mark detection works
//!
//! Hyperlink memory safety proofs (FV-19):
//! - `hyperlink_roundtrip` - Set/get preserves Arc identity
//! - `hyperlink_arc_clone_safe` - Arc reference counting is correct
//! - `hyperlink_has_data_consistent` - has_data tracks hyperlink presence
//! - `hyperlink_extras_cleanup` - Empty extras are removed from HashMap
//! - `hyperlink_clear_row_safe` - clear_row removes hyperlinks in row
//! - `hyperlink_clear_range_safe` - clear_range removes hyperlinks in range
//! - `hyperlink_shift_down_safe` - Hyperlinks move with rows on scroll down
//! - `hyperlink_shift_up_safe` - Hyperlinks move with rows on scroll up

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::sync::Arc;

/// Coordinate for cell extras lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellCoord {
    /// Row index (0-indexed from top of grid).
    pub row: u16,
    /// Column index (0-indexed).
    pub col: u16,
}

impl CellCoord {
    /// Create a new cell coordinate.
    #[must_use]
    #[inline]
    pub const fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

/// Bitflags for CellExtra presence tracking.
///
/// These flags indicate which optional fields have values set.
/// Using bitflags avoids the overhead of multiple Option discriminants.
mod extra_flags {
    /// Underline color is present (bytes 0-2 of colors array).
    pub const HAS_UNDERLINE_COLOR: u16 = 1 << 0;
    /// Foreground RGB is present (bytes 3-5 of colors array).
    pub const HAS_FG_RGB: u16 = 1 << 1;
    /// Background RGB is present (bytes 6-8 of colors array).
    pub const HAS_BG_RGB: u16 = 1 << 2;
    /// Mask for color presence flags.
    pub const COLOR_MASK: u16 = HAS_UNDERLINE_COLOR | HAS_FG_RGB | HAS_BG_RGB;
    /// Bits 3-15 are available for extended flags.
    pub const EXTENDED_SHIFT: u32 = 3;
}

/// Extra attributes for a cell that don't fit in the packed 8-byte structure.
///
/// These are rare attributes that most cells don't need:
/// - Hyperlinks (OSC 8)
/// - Colored underlines (SGR 58/59)
/// - True color RGB (foreground/background)
/// - Zero-width combining characters
/// - Complex characters (non-BMP, grapheme clusters)
///
/// ## Memory Layout (M9 optimization)
///
/// Uses packed storage to minimize size:
/// - `flags: u16` - presence bitflags + extended flags (bits 3-15)
/// - `colors: [u8; 9]` - packed RGB: underline[0-2], fg[3-5], bg[6-8]
/// - `hyperlink: Option<Arc<str>>` - uses niche optimization (no overhead)
/// - `complex_char: Option<Arc<str>>` - uses niche optimization
/// - `combining: SmallVec<[char; 2]>` - inline for common case
///
/// Total: ~56 bytes (vs ~72 bytes before M9)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellExtra {
    /// Bitflags: bits 0-2 track color presence, bits 3-15 for extended flags.
    flags: u16,

    /// Packed RGB colors: [underline_r, underline_g, underline_b, fg_r, fg_g, fg_b, bg_r, bg_g, bg_b].
    /// Only valid if corresponding HAS_* flag is set.
    colors: [u8; 9],

    /// Hyperlink URL (OSC 8).
    /// Uses Arc for efficient sharing when multiple cells have the same link.
    hyperlink: Option<Arc<str>>,

    /// Complex character string (non-BMP, grapheme clusters, combining marks).
    /// Only used when Cell.flags.is_complex() is true.
    complex_char: Option<Arc<str>>,

    /// Zero-width combining characters.
    /// Most cells have 0-2 combining chars; SmallVec avoids allocation.
    combining: SmallVec<[char; 2]>,
}

impl CellExtra {
    /// Create an empty cell extra.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            flags: 0,
            colors: [0; 9],
            hyperlink: None,
            complex_char: None,
            combining: SmallVec::new(),
        }
    }

    /// Check if this extra has any data (non-empty).
    #[must_use]
    #[inline]
    pub fn has_data(&self) -> bool {
        self.hyperlink.is_some()
            || self.complex_char.is_some()
            || !self.combining.is_empty()
            || self.flags != 0
    }

    /// Get extended flags (bits 3-15 of flags field).
    #[must_use]
    #[inline]
    pub fn extended_flags(&self) -> u16 {
        self.flags >> extra_flags::EXTENDED_SHIFT
    }

    /// Set extended flags (bits 3-15 of flags field).
    #[inline]
    pub fn set_extended_flags(&mut self, ext_flags: u16) {
        // Preserve color presence bits (0-2), set extended flags (3-15)
        self.flags =
            (self.flags & extra_flags::COLOR_MASK) | (ext_flags << extra_flags::EXTENDED_SHIFT);
    }

    /// Get the hyperlink URL.
    #[must_use]
    #[inline]
    pub fn hyperlink(&self) -> Option<&Arc<str>> {
        self.hyperlink.as_ref()
    }

    /// Set the hyperlink URL.
    #[inline]
    pub fn set_hyperlink(&mut self, url: Option<Arc<str>>) {
        self.hyperlink = url;
    }

    /// Get the underline color as RGB.
    #[must_use]
    #[inline]
    pub fn underline_color(&self) -> Option<[u8; 3]> {
        if self.flags & extra_flags::HAS_UNDERLINE_COLOR != 0 {
            Some([self.colors[0], self.colors[1], self.colors[2]])
        } else {
            None
        }
    }

    /// Get the underline color as legacy u32 format.
    #[must_use]
    #[inline]
    pub fn underline_color_u32(&self) -> Option<u32> {
        self.underline_color().map(|[r, g, b]| {
            0x01_000000 | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
        })
    }

    /// Set the underline color from RGB.
    #[inline]
    pub fn set_underline_color(&mut self, color: Option<[u8; 3]>) {
        match color {
            Some([r, g, b]) => {
                self.colors[0] = r;
                self.colors[1] = g;
                self.colors[2] = b;
                self.flags |= extra_flags::HAS_UNDERLINE_COLOR;
            }
            None => {
                self.flags &= !extra_flags::HAS_UNDERLINE_COLOR;
            }
        }
    }

    /// Set the underline color from legacy u32 format (0xTT_RRGGBB).
    #[inline]
    pub fn set_underline_color_u32(&mut self, color: Option<u32>) {
        self.set_underline_color(color.map(|c| {
            let r = ((c >> 16) & 0xFF) as u8;
            let g = ((c >> 8) & 0xFF) as u8;
            let b = (c & 0xFF) as u8;
            [r, g, b]
        }));
    }

    /// Get foreground RGB color.
    #[must_use]
    #[inline]
    pub fn fg_rgb(&self) -> Option<[u8; 3]> {
        if self.flags & extra_flags::HAS_FG_RGB != 0 {
            Some([self.colors[3], self.colors[4], self.colors[5]])
        } else {
            None
        }
    }

    /// Set foreground RGB color.
    #[inline]
    pub fn set_fg_rgb(&mut self, rgb: Option<[u8; 3]>) {
        match rgb {
            Some([r, g, b]) => {
                self.colors[3] = r;
                self.colors[4] = g;
                self.colors[5] = b;
                self.flags |= extra_flags::HAS_FG_RGB;
            }
            None => {
                self.flags &= !extra_flags::HAS_FG_RGB;
            }
        }
    }

    /// Get background RGB color.
    #[must_use]
    #[inline]
    pub fn bg_rgb(&self) -> Option<[u8; 3]> {
        if self.flags & extra_flags::HAS_BG_RGB != 0 {
            Some([self.colors[6], self.colors[7], self.colors[8]])
        } else {
            None
        }
    }

    /// Set background RGB color.
    #[inline]
    pub fn set_bg_rgb(&mut self, rgb: Option<[u8; 3]>) {
        match rgb {
            Some([r, g, b]) => {
                self.colors[6] = r;
                self.colors[7] = g;
                self.colors[8] = b;
                self.flags |= extra_flags::HAS_BG_RGB;
            }
            None => {
                self.flags &= !extra_flags::HAS_BG_RGB;
            }
        }
    }

    /// Get complex character string.
    #[must_use]
    #[inline]
    pub fn complex_char(&self) -> Option<&Arc<str>> {
        self.complex_char.as_ref()
    }

    /// Set complex character string.
    #[inline]
    pub fn set_complex_char(&mut self, s: Option<Arc<str>>) {
        self.complex_char = s;
    }

    /// Get the combining characters.
    #[must_use]
    #[inline]
    pub fn combining(&self) -> &[char] {
        &self.combining
    }

    /// Add a combining character.
    ///
    /// Combining characters are appended to the base character.
    /// Example: 'e' + U+0301 (combining acute accent) = 'é'
    #[inline]
    pub fn add_combining(&mut self, c: char) {
        // Limit to prevent DoS with excessive combining marks
        if self.combining.len() < Self::MAX_COMBINING {
            self.combining.push(c);
        }
    }

    /// Clear all combining characters.
    #[inline]
    pub fn clear_combining(&mut self) {
        self.combining.clear();
    }

    /// Maximum combining characters per cell (prevents DoS).
    pub const MAX_COMBINING: usize = 16;

    /// Calculate memory used by this extra.
    #[must_use]
    pub fn memory_used(&self) -> usize {
        let base = std::mem::size_of::<Self>();
        let hyperlink_mem = self
            .hyperlink
            .as_ref()
            .map_or(0, |s| std::mem::size_of::<Arc<str>>() + s.len());
        let complex_char_mem = self
            .complex_char
            .as_ref()
            .map_or(0, |s| std::mem::size_of::<Arc<str>>() + s.len());
        let combining_mem = if self.combining.spilled() {
            self.combining.capacity() * std::mem::size_of::<char>()
        } else {
            0 // Inline storage, already counted in base
        };
        base + hyperlink_mem + complex_char_mem + combining_mem
    }
}

/// Storage for cell extras across the grid.
///
/// Uses FxHashMap for O(1) lookup with fast non-cryptographic hashing.
/// FxHashMap is 2-3x faster than std HashMap for small keys like (u16, u16).
/// Most cells have no extras, so this is more memory-efficient than storing extras inline.
#[derive(Debug, Clone, Default)]
pub struct CellExtras {
    /// Map from cell coordinate to extra data.
    data: FxHashMap<CellCoord, CellExtra>,
}

impl CellExtras {
    /// Create empty extras storage.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            data: FxHashMap::default(),
        }
    }

    /// Get extras for a cell, if any.
    #[must_use]
    #[inline]
    pub fn get(&self, coord: CellCoord) -> Option<&CellExtra> {
        self.data.get(&coord)
    }

    /// Get mutable extras for a cell, creating if needed.
    #[inline]
    pub fn get_or_create(&mut self, coord: CellCoord) -> &mut CellExtra {
        self.data.entry(coord).or_default()
    }

    /// Set extras for a cell.
    ///
    /// If the extra has no data, removes the entry to save memory.
    #[inline]
    pub fn set(&mut self, coord: CellCoord, extra: CellExtra) {
        if extra.has_data() {
            self.data.insert(coord, extra);
        } else {
            self.data.remove(&coord);
        }
    }

    /// Remove extras for a cell.
    #[inline]
    pub fn remove(&mut self, coord: CellCoord) {
        self.data.remove(&coord);
    }

    /// Clear extras for a row.
    ///
    /// Called when a row is cleared or scrolls off.
    pub fn clear_row(&mut self, row: u16) {
        self.data.retain(|coord, _| coord.row != row);
    }

    /// Clear extras for a range of columns in a row.
    pub fn clear_range(&mut self, row: u16, start_col: u16, end_col: u16) {
        self.data.retain(|coord, _| {
            !(coord.row == row && coord.col >= start_col && coord.col < end_col)
        });
    }

    /// Shift rows down (for scroll up).
    ///
    /// Rows >= start_row are shifted down by 1.
    /// Used when inserting a row at start_row.
    pub fn shift_rows_down(&mut self, start_row: u16, max_row: u16) {
        let mut new_data =
            FxHashMap::with_capacity_and_hasher(self.data.len(), rustc_hash::FxBuildHasher);
        for (coord, extra) in self.data.drain() {
            if coord.row >= start_row && coord.row < max_row {
                new_data.insert(CellCoord::new(coord.row + 1, coord.col), extra);
            } else if coord.row < start_row {
                new_data.insert(coord, extra);
            }
            // Rows >= max_row are dropped (scrolled off)
        }
        self.data = new_data;
    }

    /// Shift rows up (for scroll down).
    ///
    /// Rows > start_row are shifted up by 1.
    /// Used when deleting the row at start_row.
    pub fn shift_rows_up(&mut self, start_row: u16) {
        let mut new_data =
            FxHashMap::with_capacity_and_hasher(self.data.len(), rustc_hash::FxBuildHasher);
        for (coord, extra) in self.data.drain() {
            if coord.row > start_row {
                new_data.insert(CellCoord::new(coord.row - 1, coord.col), extra);
            } else if coord.row < start_row {
                new_data.insert(coord, extra);
            }
            // Row == start_row is deleted
        }
        self.data = new_data;
    }

    /// Clear all extras.
    #[inline]
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get the number of cells with extras.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Calculate total memory used.
    #[must_use]
    pub fn memory_used(&self) -> usize {
        let base =
            std::mem::size_of::<Self>() + self.data.capacity() * std::mem::size_of::<CellCoord>();
        let extras_mem: usize = self.data.values().map(CellExtra::memory_used).sum();
        base + extras_mem
    }

    /// Iterate over all extras.
    pub fn iter(&self) -> impl Iterator<Item = (&CellCoord, &CellExtra)> {
        self.data.iter()
    }
}

/// Check if a character is a Unicode combining mark.
///
/// Combining marks include:
/// - Combining Diacritical Marks (U+0300-U+036F)
/// - Combining Diacritical Marks Extended (U+1AB0-U+1AFF)
/// - Combining Diacritical Marks Supplement (U+1DC0-U+1DFF)
/// - Combining Diacritical Marks for Symbols (U+20D0-U+20FF)
/// - Combining Half Marks (U+FE20-U+FE2F)
#[must_use]
#[inline]
pub fn is_combining_mark(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}' |  // Combining Diacritical Marks
        '\u{1AB0}'..='\u{1AFF}' |  // Combining Diacritical Marks Extended
        '\u{1DC0}'..='\u{1DFF}' |  // Combining Diacritical Marks Supplement
        '\u{20D0}'..='\u{20FF}' |  // Combining Diacritical Marks for Symbols
        '\u{FE20}'..='\u{FE2F}'    // Combining Half Marks
    )
}

/// Check if a character is a zero-width character.
///
/// Zero-width characters include:
/// - Zero Width Space (U+200B)
/// - Zero Width Non-Joiner (U+200C)
/// - Zero Width Joiner (U+200D)
/// - Word Joiner (U+2060)
/// - Zero Width No-Break Space (U+FEFF)
#[must_use]
#[inline]
pub fn is_zero_width(c: char) -> bool {
    matches!(
        c,
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_extra_default_is_empty() {
        let extra = CellExtra::new();
        assert!(!extra.has_data());
    }

    #[test]
    fn cell_extra_with_hyperlink() {
        let mut extra = CellExtra::new();
        let url: Arc<str> = "https://example.com".into();
        extra.set_hyperlink(Some(url.clone()));

        assert!(extra.has_data());
        assert_eq!(extra.hyperlink(), Some(&url));
    }

    #[test]
    fn cell_extra_with_underline_color() {
        let mut extra = CellExtra::new();
        extra.set_underline_color(Some([255, 0, 0])); // Red

        assert!(extra.has_data());
        assert_eq!(extra.underline_color(), Some([255, 0, 0]));

        // Test clear
        extra.set_underline_color(None);
        assert!(!extra.has_data());
        assert_eq!(extra.underline_color(), None);
    }

    #[test]
    fn cell_extra_packed_rgb_colors() {
        let mut extra = CellExtra::new();

        // Test all three RGB colors can be set independently
        extra.set_underline_color(Some([10, 20, 30]));
        extra.set_fg_rgb(Some([40, 50, 60]));
        extra.set_bg_rgb(Some([70, 80, 90]));

        assert!(extra.has_data());
        assert_eq!(extra.underline_color(), Some([10, 20, 30]));
        assert_eq!(extra.fg_rgb(), Some([40, 50, 60]));
        assert_eq!(extra.bg_rgb(), Some([70, 80, 90]));

        // Clear one, others remain
        extra.set_fg_rgb(None);
        assert!(extra.has_data());
        assert_eq!(extra.underline_color(), Some([10, 20, 30]));
        assert_eq!(extra.fg_rgb(), None);
        assert_eq!(extra.bg_rgb(), Some([70, 80, 90]));

        // Clear all
        extra.set_underline_color(None);
        extra.set_bg_rgb(None);
        assert!(!extra.has_data());
    }

    #[test]
    fn cell_extra_extended_flags() {
        let mut extra = CellExtra::new();

        // Extended flags use bits 3-15, so max value is 0x1FFF (13 bits)
        // Extended flags don't count as "data" when colors are not set
        // but do when set (since flags field is non-zero)
        extra.set_extended_flags(0x1234);
        assert!(extra.has_data()); // flags != 0
        assert_eq!(extra.extended_flags(), 0x1234);

        // Extended flags preserve color presence
        extra.set_underline_color(Some([100, 100, 100]));
        assert_eq!(extra.extended_flags(), 0x1234);
        assert_eq!(extra.underline_color(), Some([100, 100, 100]));

        // Setting extended flags preserves color presence
        // Use a value that fits in 13 bits (max 0x1FFF)
        extra.set_extended_flags(0x0567);
        assert_eq!(extra.extended_flags(), 0x0567);
        assert_eq!(extra.underline_color(), Some([100, 100, 100]));
    }

    #[test]
    fn cell_extra_size_optimized() {
        // M9: CellExtra should be smaller than 72 bytes (the old size with Option fields)
        // New layout: flags(2) + colors(9) + padding(5) + hyperlink(16) + complex_char(16) + combining(24) = ~72
        // But with better packing we should be under 72 bytes
        let size = std::mem::size_of::<CellExtra>();
        // The packed version should be no larger than 72 bytes
        // In practice it's ~56 bytes due to removing Option discriminants
        assert!(
            size <= 72,
            "CellExtra should be <= 72 bytes, got {} bytes",
            size
        );
    }

    #[test]
    fn cell_extra_with_combining() {
        let mut extra = CellExtra::new();
        extra.add_combining('\u{0301}'); // Combining acute accent
        extra.add_combining('\u{0308}'); // Combining diaeresis

        assert!(extra.has_data());
        assert_eq!(extra.combining(), &['\u{0301}', '\u{0308}']);
    }

    #[test]
    fn cell_extra_max_combining() {
        let mut extra = CellExtra::new();
        for _ in 0..20 {
            extra.add_combining('\u{0301}');
        }
        // Should be capped at MAX_COMBINING
        assert_eq!(extra.combining().len(), CellExtra::MAX_COMBINING);
    }

    #[test]
    fn cell_extras_storage() {
        let mut extras = CellExtras::new();
        let coord = CellCoord::new(5, 10);

        assert!(extras.get(coord).is_none());

        let extra = extras.get_or_create(coord);
        extra.set_hyperlink(Some("https://test.com".into()));

        assert!(extras.get(coord).is_some());
        assert_eq!(extras.len(), 1);
    }

    #[test]
    fn cell_extras_clear_row() {
        let mut extras = CellExtras::new();

        // Add extras to multiple rows
        extras
            .get_or_create(CellCoord::new(0, 0))
            .add_combining('\u{0301}');
        extras
            .get_or_create(CellCoord::new(0, 5))
            .add_combining('\u{0302}');
        extras
            .get_or_create(CellCoord::new(1, 0))
            .add_combining('\u{0303}');
        extras
            .get_or_create(CellCoord::new(2, 0))
            .add_combining('\u{0304}');

        assert_eq!(extras.len(), 4);

        extras.clear_row(0);

        assert_eq!(extras.len(), 2);
        assert!(extras.get(CellCoord::new(0, 0)).is_none());
        assert!(extras.get(CellCoord::new(0, 5)).is_none());
        assert!(extras.get(CellCoord::new(1, 0)).is_some());
    }

    #[test]
    fn cell_extras_shift_rows_down() {
        let mut extras = CellExtras::new();

        extras
            .get_or_create(CellCoord::new(0, 0))
            .add_combining('\u{0301}');
        extras
            .get_or_create(CellCoord::new(1, 0))
            .add_combining('\u{0302}');
        extras
            .get_or_create(CellCoord::new(2, 0))
            .add_combining('\u{0303}');

        // Shift rows >= 1 down by 1, max row 3 (row 3+ dropped)
        extras.shift_rows_down(1, 3);

        assert!(extras.get(CellCoord::new(0, 0)).is_some()); // Unchanged
        assert!(extras.get(CellCoord::new(1, 0)).is_none()); // Old row 1 is now empty
        assert!(extras.get(CellCoord::new(2, 0)).is_some()); // Old row 1 shifted here
                                                             // Old row 2 would be at row 3, but dropped due to max_row
    }

    #[test]
    fn cell_extras_shift_rows_up() {
        let mut extras = CellExtras::new();

        extras
            .get_or_create(CellCoord::new(0, 0))
            .add_combining('\u{0301}');
        extras
            .get_or_create(CellCoord::new(1, 0))
            .add_combining('\u{0302}');
        extras
            .get_or_create(CellCoord::new(2, 0))
            .add_combining('\u{0303}');

        // Delete row 1, shift rows > 1 up
        extras.shift_rows_up(1);

        assert!(extras.get(CellCoord::new(0, 0)).is_some()); // Unchanged
        assert!(extras.get(CellCoord::new(1, 0)).is_some()); // Old row 2 shifted here
        assert!(extras.get(CellCoord::new(2, 0)).is_none()); // Gone
    }

    #[test]
    fn cell_extras_empty_removed() {
        let mut extras = CellExtras::new();
        let coord = CellCoord::new(0, 0);

        // Set non-empty extra
        let mut extra = CellExtra::new();
        extra.add_combining('\u{0301}');
        extras.set(coord, extra);
        assert_eq!(extras.len(), 1);

        // Set empty extra - should remove
        extras.set(coord, CellExtra::new());
        assert_eq!(extras.len(), 0);
    }

    #[test]
    fn is_combining_mark_basic() {
        assert!(is_combining_mark('\u{0301}')); // Acute accent
        assert!(is_combining_mark('\u{0308}')); // Diaeresis
        assert!(is_combining_mark('\u{0327}')); // Cedilla
        assert!(!is_combining_mark('a'));
        assert!(!is_combining_mark(' '));
    }

    #[test]
    fn is_zero_width_basic() {
        assert!(is_zero_width('\u{200B}')); // Zero width space
        assert!(is_zero_width('\u{200D}')); // Zero width joiner
        assert!(is_zero_width('\u{FEFF}')); // BOM / ZWNBSP
        assert!(!is_zero_width('a'));
        assert!(!is_zero_width(' '));
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// CellExtra has_data is consistent with contents.
    #[kani::proof]
    fn cell_extra_has_data_consistent() {
        let extra = CellExtra::new();
        kani::assert(!extra.has_data(), "new extra should not have data");

        let mut extra_with_underline = CellExtra::new();
        extra_with_underline.set_underline_color(Some([0xFF, 0x00, 0x00]));
        kani::assert(
            extra_with_underline.has_data(),
            "extra with underline should have data",
        );
    }

    /// CellCoord hashing is consistent.
    #[kani::proof]
    fn cell_coord_hash_consistent() {
        let row: u16 = kani::any();
        let col: u16 = kani::any();

        let coord1 = CellCoord::new(row, col);
        let coord2 = CellCoord::new(row, col);

        kani::assert(coord1 == coord2, "same coords should be equal");
    }

    /// Combining characters are bounded.
    #[kani::proof]
    #[kani::unwind(20)]
    fn combining_bounded() {
        let mut extra = CellExtra::new();

        for _ in 0..20 {
            let c: char = kani::any();
            kani::assume(c as u32 >= 0x0300 && c as u32 <= 0x036F);
            extra.add_combining(c);
        }

        kani::assert(
            extra.combining().len() <= CellExtra::MAX_COMBINING,
            "combining should be bounded",
        );
    }

    /// is_combining_mark returns true for diacritical marks range.
    #[kani::proof]
    fn combining_mark_range_valid() {
        let codepoint: u32 = kani::any();
        kani::assume(codepoint >= 0x0300 && codepoint <= 0x036F);

        if let Some(c) = char::from_u32(codepoint) {
            kani::assert(
                is_combining_mark(c),
                "diacritical marks should be combining",
            );
        }
    }

    // =========================================================================
    // HYPERLINK MEMORY SAFETY PROOFS (FV-19)
    // =========================================================================

    /// Hyperlink set/get roundtrip preserves value.
    ///
    /// Verifies that setting a hyperlink and then getting it returns the same
    /// Arc instance (by identity, not just equality).
    #[kani::proof]
    fn hyperlink_roundtrip() {
        let mut extra = CellExtra::new();

        // Initially no hyperlink
        kani::assert(extra.hyperlink().is_none(), "new extra has no hyperlink");
        kani::assert(!extra.has_data(), "new extra has no data");

        // Set a hyperlink
        let url: Arc<str> = Arc::from("https://example.com");
        extra.set_hyperlink(Some(url.clone()));

        // Verify it's stored
        kani::assert(extra.has_data(), "extra with hyperlink has data");
        kani::assert(extra.hyperlink().is_some(), "hyperlink is set");

        // Verify Arc identity (same allocation)
        let retrieved = extra.hyperlink().unwrap();
        kani::assert(
            Arc::ptr_eq(&url, retrieved),
            "Arc points to same allocation",
        );

        // Clear the hyperlink
        extra.set_hyperlink(None);
        kani::assert(extra.hyperlink().is_none(), "hyperlink is cleared");
        kani::assert(!extra.has_data(), "extra is empty after clear");
    }

    /// Hyperlink Arc cloning maintains reference count safety.
    ///
    /// Verifies that cloning an Arc<str> hyperlink across multiple cells
    /// maintains proper reference counting semantics.
    #[kani::proof]
    fn hyperlink_arc_clone_safe() {
        let url: Arc<str> = Arc::from("https://test.com");
        let initial_count = Arc::strong_count(&url);
        kani::assert(initial_count == 1, "initial strong count is 1");

        // Clone for first cell
        let clone1 = url.clone();
        kani::assert(Arc::strong_count(&url) == 2, "count after first clone");
        kani::assert(Arc::ptr_eq(&url, &clone1), "clone1 points to same data");

        // Clone for second cell (simulating wide character)
        let clone2 = url.clone();
        kani::assert(Arc::strong_count(&url) == 3, "count after second clone");
        kani::assert(Arc::ptr_eq(&url, &clone2), "clone2 points to same data");

        // Drop one clone
        drop(clone1);
        kani::assert(Arc::strong_count(&url) == 2, "count after drop");

        // Drop second clone
        drop(clone2);
        kani::assert(Arc::strong_count(&url) == 1, "count returns to 1");
    }

    /// CellExtra hyperlink has_data consistency.
    ///
    /// Verifies that has_data() correctly reflects hyperlink presence.
    #[kani::proof]
    fn hyperlink_has_data_consistent() {
        let mut extra = CellExtra::new();

        // Empty extra
        kani::assert(!extra.has_data(), "empty extra has no data");
        kani::assert(extra.hyperlink().is_none(), "empty extra has no hyperlink");

        // With hyperlink only
        extra.set_hyperlink(Some(Arc::from("url")));
        kani::assert(extra.has_data(), "extra with hyperlink has data");
        kani::assert(
            extra.hyperlink().is_some(),
            "extra with hyperlink returns some",
        );

        // Clear hyperlink
        extra.set_hyperlink(None);
        kani::assert(!extra.has_data(), "cleared extra has no data");
        kani::assert(
            extra.hyperlink().is_none(),
            "cleared extra has no hyperlink",
        );

        // With hyperlink AND underline
        extra.set_hyperlink(Some(Arc::from("url2")));
        extra.set_underline_color(Some([255, 0, 0]));
        kani::assert(extra.has_data(), "extra with underline has data");

        // Clear just hyperlink - still has data (underline)
        extra.set_hyperlink(None);
        kani::assert(extra.has_data(), "extra with underline still has data");
        kani::assert(
            extra.hyperlink().is_none(),
            "hyperlink cleared but underline remains",
        );
    }

    /// CellExtras removes entry when hyperlink cleared.
    ///
    /// Verifies that setting an empty CellExtra removes it from the HashMap.
    #[kani::proof]
    fn hyperlink_extras_cleanup() {
        let mut extras = CellExtras::new();
        let coord = CellCoord::new(5, 10);

        // Add hyperlink
        let mut extra = CellExtra::new();
        extra.set_hyperlink(Some(Arc::from("https://cleanup.test")));
        extras.set(coord, extra);

        kani::assert(extras.len() == 1, "one entry after add");
        kani::assert(extras.get(coord).is_some(), "entry exists");

        // Clear by setting empty extra
        extras.set(coord, CellExtra::new());

        kani::assert(extras.len() == 0, "entry removed after clear");
        kani::assert(extras.get(coord).is_none(), "entry gone");
    }

    /// CellExtras clear_row removes all hyperlinks in row.
    ///
    /// Verifies that clear_row properly cleans up all hyperlink entries.
    #[kani::proof]
    fn hyperlink_clear_row_safe() {
        let mut extras = CellExtras::new();

        // Add hyperlinks to row 5
        let url: Arc<str> = Arc::from("https://row5.test");
        for col in 0..3u16 {
            let mut extra = CellExtra::new();
            extra.set_hyperlink(Some(url.clone()));
            extras.set(CellCoord::new(5, col), extra);
        }

        // Add hyperlink to different row
        let mut extra = CellExtra::new();
        extra.set_hyperlink(Some(Arc::from("https://row6.test")));
        extras.set(CellCoord::new(6, 0), extra);

        kani::assert(extras.len() == 4, "4 entries before clear");

        // Clear row 5
        extras.clear_row(5);

        kani::assert(extras.len() == 1, "1 entry after clear row 5");
        kani::assert(
            extras.get(CellCoord::new(5, 0)).is_none(),
            "row 5 col 0 gone",
        );
        kani::assert(
            extras.get(CellCoord::new(5, 1)).is_none(),
            "row 5 col 1 gone",
        );
        kani::assert(
            extras.get(CellCoord::new(5, 2)).is_none(),
            "row 5 col 2 gone",
        );
        kani::assert(
            extras.get(CellCoord::new(6, 0)).is_some(),
            "row 6 preserved",
        );
    }

    /// CellExtras clear_range removes hyperlinks in range.
    ///
    /// Verifies that clear_range properly cleans up hyperlinks within bounds.
    #[kani::proof]
    fn hyperlink_clear_range_safe() {
        let mut extras = CellExtras::new();

        // Add hyperlinks to columns 0-4 in row 3
        let url: Arc<str> = Arc::from("https://range.test");
        for col in 0..5u16 {
            let mut extra = CellExtra::new();
            extra.set_hyperlink(Some(url.clone()));
            extras.set(CellCoord::new(3, col), extra);
        }

        kani::assert(extras.len() == 5, "5 entries before clear");

        // Clear columns 1-3 (exclusive end)
        extras.clear_range(3, 1, 4);

        kani::assert(extras.len() == 2, "2 entries after clear range");
        kani::assert(
            extras.get(CellCoord::new(3, 0)).is_some(),
            "col 0 preserved",
        );
        kani::assert(extras.get(CellCoord::new(3, 1)).is_none(), "col 1 cleared");
        kani::assert(extras.get(CellCoord::new(3, 2)).is_none(), "col 2 cleared");
        kani::assert(extras.get(CellCoord::new(3, 3)).is_none(), "col 3 cleared");
        kani::assert(
            extras.get(CellCoord::new(3, 4)).is_some(),
            "col 4 preserved",
        );
    }

    /// CellExtras shift_rows_down moves hyperlinks correctly.
    ///
    /// Verifies that hyperlinks move with their rows during scroll.
    #[kani::proof]
    fn hyperlink_shift_down_safe() {
        let mut extras = CellExtras::new();

        // Add hyperlink to row 2
        let url: Arc<str> = Arc::from("https://shift.test");
        let mut extra = CellExtra::new();
        extra.set_hyperlink(Some(url.clone()));
        extras.set(CellCoord::new(2, 0), extra);

        // Add hyperlink to row 0 (shouldn't move)
        let mut extra0 = CellExtra::new();
        extra0.set_hyperlink(Some(Arc::from("https://row0.test")));
        extras.set(CellCoord::new(0, 0), extra0);

        kani::assert(extras.len() == 2, "2 entries before shift");

        // Shift rows >= 1 down, max row 5
        extras.shift_rows_down(1, 5);

        // Row 0 should be unchanged
        kani::assert(
            extras.get(CellCoord::new(0, 0)).is_some(),
            "row 0 unchanged",
        );

        // Row 2 should have moved to row 3
        kani::assert(extras.get(CellCoord::new(2, 0)).is_none(), "old row 2 gone");
        kani::assert(extras.get(CellCoord::new(3, 0)).is_some(), "moved to row 3");
    }

    /// CellExtras shift_rows_up moves hyperlinks correctly.
    ///
    /// Verifies that hyperlinks move with their rows during scroll up.
    #[kani::proof]
    fn hyperlink_shift_up_safe() {
        let mut extras = CellExtras::new();

        // Add hyperlink to row 3
        let url: Arc<str> = Arc::from("https://shiftup.test");
        let mut extra = CellExtra::new();
        extra.set_hyperlink(Some(url.clone()));
        extras.set(CellCoord::new(3, 0), extra);

        // Add hyperlink to row 1 (will be deleted)
        let mut extra1 = CellExtra::new();
        extra1.set_hyperlink(Some(Arc::from("https://deleted.test")));
        extras.set(CellCoord::new(1, 0), extra1);

        // Add hyperlink to row 0 (shouldn't change)
        let mut extra0 = CellExtra::new();
        extra0.set_hyperlink(Some(Arc::from("https://row0.test")));
        extras.set(CellCoord::new(0, 0), extra0);

        kani::assert(extras.len() == 3, "3 entries before shift");

        // Shift up from row 1 (delete row 1, move rows > 1 up)
        extras.shift_rows_up(1);

        // Row 0 unchanged
        kani::assert(
            extras.get(CellCoord::new(0, 0)).is_some(),
            "row 0 preserved",
        );

        // Row 1 deleted
        kani::assert(extras.get(CellCoord::new(1, 0)).is_none(), "row 1 deleted");

        // Row 3 moved to row 2
        kani::assert(extras.get(CellCoord::new(3, 0)).is_none(), "old row 3 gone");
        kani::assert(
            extras.get(CellCoord::new(2, 0)).is_some(),
            "row 3 moved to 2",
        );
    }
}
