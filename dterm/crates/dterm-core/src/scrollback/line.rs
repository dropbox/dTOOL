//! Line representation for scrollback storage.
//!
//! Lines can be stored in different formats depending on tier:
//! - Hot: Full Line with content + RLE-compressed attributes
//! - Warm/Cold: Serialized bytes (compressed)
//!
//! ## RLE Attribute Compression
//!
//! Terminal lines often have runs of cells with identical attributes (e.g.,
//! a prompt in one color, then text in another). RLE compression stores
//! `(style, count)` pairs instead of per-cell styles.
//!
//! Typical compression: 80-column line with 3 color regions → 3 runs vs 80 cells.

use crate::rle::Rle;
use smallvec::SmallVec;

/// Maximum inline storage for line content (avoids heap allocation for short lines).
const INLINE_SIZE: usize = 128;

// ============================================================================
// Cell Attributes for RLE Compression
// ============================================================================

/// Compressed cell attributes for RLE storage.
///
/// This is a compact representation of cell styling that can be efficiently
/// RLE-encoded. It captures the essential visual attributes:
/// - Foreground color (packed)
/// - Background color (packed)
/// - Cell flags (bold, italic, underline, etc.)
///
/// ## Memory Layout
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────┐
/// │ fg: u32 (4 bytes) - Packed foreground color                 │
/// │   Format: 0xTT_RRGGBB where TT = type (default/indexed/rgb) │
/// ├─────────────────────────────────────────────────────────────┤
/// │ bg: u32 (4 bytes) - Packed background color                 │
/// │   Format: 0xTT_RRGGBB where TT = type (default/indexed/rgb) │
/// ├─────────────────────────────────────────────────────────────┤
/// │ flags: u16 (2 bytes) - Visual attribute flags               │
/// │   Bits 0-7: bold, dim, italic, underline, blink, inverse... │
/// └─────────────────────────────────────────────────────────────┘
/// Total: 10 bytes per unique style (vs 8 bytes per cell uncompressed)
/// ```
///
/// ## Compression Benefit
///
/// An 80-column line with plain text: 80 cells × 8 bytes = 640 bytes
/// With RLE (1 style run): ~15 bytes (10 bytes style + 5 bytes overhead)
///
/// An 80-column prompt line with 3 color regions:
/// - Uncompressed: 640 bytes
/// - RLE: ~45 bytes (3 runs × 10 bytes + overhead)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellAttrs {
    /// Packed foreground color.
    /// Format: 0xTT_RRGGBB where TT indicates type:
    /// - 0x00: Indexed color (RRGGBB = 0x00_00_XX where XX is index)
    /// - 0x01: True color RGB
    /// - 0xFF: Default color
    pub fg: u32,
    /// Packed background color (same format as fg).
    pub bg: u32,
    /// Cell flags (bold, italic, underline, etc.).
    /// Excludes WIDE/WIDE_CONTINUATION/COMPLEX flags.
    pub flags: u16,
}

/// Default fg color (0xFF_FFFFFF - default type marker + white placeholder).
const DEFAULT_FG: u32 = 0xFF_FF_FF_FF;
/// Default bg color (0xFF_000000 - default type marker + black placeholder).
const DEFAULT_BG: u32 = 0xFF_00_00_00;

impl CellAttrs {
    /// Default cell attributes (default colors, no flags).
    pub const DEFAULT: Self = Self {
        fg: DEFAULT_FG,
        bg: DEFAULT_BG,
        flags: 0,
    };

    /// Create new cell attributes.
    #[must_use]
    pub const fn new(fg: u32, bg: u32, flags: u16) -> Self {
        Self { fg, bg, flags }
    }

    /// Check if these are default attributes.
    #[must_use]
    #[inline]
    pub const fn is_default(&self) -> bool {
        self.fg == DEFAULT_FG && self.bg == DEFAULT_BG && self.flags == 0
    }

    /// Mask for visual flags we care about in scrollback.
    /// Excludes WIDE, WIDE_CONTINUATION, COMPLEX, PROTECTED which are
    /// cell-specific rather than style-specific.
    const VISUAL_FLAGS_MASK: u16 = 0x01FF; // bits 0-8 (bold through double_underline)

    /// Create from raw cell values, filtering to visual-only flags.
    #[must_use]
    #[inline]
    pub const fn from_raw(fg: u32, bg: u32, flags: u16) -> Self {
        Self {
            fg,
            bg,
            flags: flags & Self::VISUAL_FLAGS_MASK,
        }
    }

    /// Serialize to bytes (10 bytes).
    #[must_use]
    pub fn serialize(&self) -> [u8; 10] {
        let mut buf = [0u8; 10];
        buf[0..4].copy_from_slice(&self.fg.to_le_bytes());
        buf[4..8].copy_from_slice(&self.bg.to_le_bytes());
        buf[8..10].copy_from_slice(&self.flags.to_le_bytes());
        buf
    }

    /// Deserialize from bytes.
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < 10 {
            return None;
        }
        let fg = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let bg = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let flags = u16::from_le_bytes([data[8], data[9]]);
        Some(Self { fg, bg, flags })
    }
}

// ============================================================================
// Line Content Storage
// ============================================================================

/// Line content storage.
///
/// Uses small-string optimization: lines up to 128 bytes are stored inline,
/// longer lines use heap allocation.
#[derive(Debug, Clone)]
pub enum LineContent {
    /// Inline storage for short lines.
    Inline(SmallVec<[u8; INLINE_SIZE]>),
    /// Heap storage for long lines.
    Heap(Vec<u8>),
}

impl Default for LineContent {
    fn default() -> Self {
        Self::Inline(SmallVec::new())
    }
}

impl LineContent {
    /// Create from bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() <= INLINE_SIZE {
            let mut sv = SmallVec::new();
            sv.extend_from_slice(bytes);
            Self::Inline(sv)
        } else {
            Self::Heap(bytes.to_vec())
        }
    }

    /// Get as byte slice.
    #[must_use]
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Inline(sv) => sv.as_slice(),
            Self::Heap(v) => v.as_slice(),
        }
    }

    /// Get the length in bytes.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::Inline(sv) => sv.len(),
            Self::Heap(v) => v.len(),
        }
    }

    /// Check if empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Convert to owned bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Self::Inline(sv) => sv.to_vec(),
            Self::Heap(v) => v,
        }
    }
}

/// A scrollback line.
///
/// Contains the text content, RLE-compressed attributes, and metadata.
///
/// ## Attribute Compression
///
/// When lines scroll off the visible grid into scrollback, we preserve their
/// styling via RLE compression. This stores runs of identical attributes
/// instead of per-cell data.
///
/// Example: A line with "Hello " (green) + "World" (default):
/// - Text: "Hello World" (11 bytes)
/// - Attrs: [(green, 6), (default, 5)] (~24 bytes for 2 runs)
/// - vs uncompressed: 11 cells × 8 bytes = 88 bytes
#[derive(Debug, Clone, Default)]
pub struct Line {
    /// Line content (UTF-8 text).
    content: LineContent,
    /// RLE-compressed cell attributes (colors and flags per character).
    /// None if all cells have default attributes (optimization for plain text).
    attrs: Option<Rle<CellAttrs>>,
    /// Line flags.
    flags: LineFlags,
}

bitflags::bitflags! {
    /// Line flags for metadata.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct LineFlags: u8 {
        /// Line is wrapped (continuation of previous line).
        const WRAPPED = 1 << 0;
        /// Line contains search match.
        const HAS_MATCH = 1 << 1;
        /// Line has been modified.
        const DIRTY = 1 << 2;
    }
}

impl Line {
    /// Create a new empty line.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a line from a string (no attributes).
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        Self {
            content: LineContent::from_bytes(s.as_bytes()),
            attrs: None,
            flags: LineFlags::empty(),
        }
    }

    /// Create a line from bytes (no attributes).
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            content: LineContent::from_bytes(bytes),
            attrs: None,
            flags: LineFlags::empty(),
        }
    }

    /// Create a line with text and RLE-compressed attributes.
    ///
    /// This is the primary constructor when converting from grid Row to scrollback Line.
    /// The attrs RLE should have the same length as the character count in text.
    #[must_use]
    pub fn with_attrs(text: &str, attrs: Rle<CellAttrs>) -> Self {
        // Optimization: if empty or all attrs are default, don't store them
        let is_all_default = attrs.run_count() == 0
            || (attrs.run_count() == 1
                && attrs.runs().first().is_some_and(|r| r.value.is_default()));

        let attrs = if is_all_default { None } else { Some(attrs) };

        Self {
            content: LineContent::from_bytes(text.as_bytes()),
            attrs,
            flags: LineFlags::empty(),
        }
    }

    /// Get the RLE-compressed attributes, if any.
    #[must_use]
    #[inline]
    pub fn attrs(&self) -> Option<&Rle<CellAttrs>> {
        self.attrs.as_ref()
    }

    /// Get the attribute for a specific character index.
    ///
    /// Returns default attributes if the line has no stored attributes
    /// or if the index is out of bounds.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // char_idx bounded by line length (< u16 cols)
    pub fn get_attr(&self, char_idx: usize) -> CellAttrs {
        match &self.attrs {
            Some(rle) => rle.get(char_idx as u32).unwrap_or(CellAttrs::DEFAULT),
            None => CellAttrs::DEFAULT,
        }
    }

    /// Check if this line has styled content (non-default attributes).
    #[must_use]
    #[inline]
    pub fn has_attrs(&self) -> bool {
        self.attrs.is_some()
    }

    /// Get the content as bytes.
    #[must_use]
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.content.as_bytes()
    }

    /// Get the length in bytes.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Get flags.
    #[must_use]
    #[inline]
    pub fn flags(&self) -> LineFlags {
        self.flags
    }

    /// Set flags.
    #[inline]
    pub fn set_flags(&mut self, flags: LineFlags) {
        self.flags = flags;
    }

    /// Check if wrapped.
    #[must_use]
    #[inline]
    pub fn is_wrapped(&self) -> bool {
        self.flags.contains(LineFlags::WRAPPED)
    }

    /// Set wrapped flag.
    #[inline]
    pub fn set_wrapped(&mut self, wrapped: bool) {
        if wrapped {
            self.flags |= LineFlags::WRAPPED;
        } else {
            self.flags -= LineFlags::WRAPPED;
        }
    }

    /// Convert to string (may be lossy if content isn't valid UTF-8).
    #[must_use]
    pub fn to_string(&self) -> String {
        String::from_utf8_lossy(self.as_bytes()).into_owned()
    }

    /// Get content as a string slice (returns None if not valid UTF-8).
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(self.as_bytes()).ok()
    }

    /// Serialize line to bytes for compression.
    ///
    /// Format v2 (with attrs):
    /// ```text
    /// [version:1][flags:1][content_len:4][content:content_len]
    /// [has_attrs:1][if has_attrs: run_count:4 + runs...]
    /// ```
    ///
    /// Version 0 = legacy format (no attrs)
    /// Version 1 = with RLE attrs
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let content = self.content.as_bytes();
        let content_len = content.len();

        // Estimate capacity
        let attrs_size = self.attrs.as_ref().map_or(1, |rle| {
            1 + 4 + rle.run_count() * 14 // has_attrs + run_count + runs
        });
        let mut result = Vec::with_capacity(6 + content_len + attrs_size);

        // Version byte
        result.push(1); // Version 1 = with attrs support

        // Flags
        result.push(self.flags.bits());

        // Content length and content
        #[allow(clippy::cast_possible_truncation)]
        let content_len_u32 = content_len.min(u32::MAX as usize) as u32;
        result.extend_from_slice(&content_len_u32.to_le_bytes());
        result.extend_from_slice(content);

        // Attributes
        if let Some(rle) = &self.attrs {
            result.push(1); // has_attrs = true
            #[allow(clippy::cast_possible_truncation)]
            let run_count = rle.run_count().min(u32::MAX as usize) as u32;
            result.extend_from_slice(&run_count.to_le_bytes());
            for run in rle.runs() {
                // Each run: [value:10][length:4]
                result.extend_from_slice(&run.value.serialize());
                result.extend_from_slice(&run.length.to_le_bytes());
            }
        } else {
            result.push(0); // has_attrs = false
        }

        result
    }

    /// Deserialize line from bytes.
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        // Check version
        let version = data[0];
        if version == 0 {
            // Legacy format (version 0 or old format without version byte)
            return Self::deserialize_legacy(data);
        }

        if data.len() < 7 {
            return None;
        }

        // Version 1 format
        let flags = LineFlags::from_bits_truncate(data[1]);
        let content_len = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;

        let content_end = 6 + content_len;
        if data.len() < content_end + 1 {
            return None;
        }

        let content = LineContent::from_bytes(&data[6..content_end]);

        // Attributes
        let has_attrs = data[content_end] != 0;
        let attrs = if has_attrs {
            let attrs_start = content_end + 1;
            if data.len() < attrs_start + 4 {
                return None;
            }
            let run_count = u32::from_le_bytes([
                data[attrs_start],
                data[attrs_start + 1],
                data[attrs_start + 2],
                data[attrs_start + 3],
            ]) as usize;

            let mut rle = Rle::new();
            let mut offset = attrs_start + 4;
            for _ in 0..run_count {
                if offset + 14 > data.len() {
                    break;
                }
                if let Some(value) = CellAttrs::deserialize(&data[offset..]) {
                    let length = u32::from_le_bytes([
                        data[offset + 10],
                        data[offset + 11],
                        data[offset + 12],
                        data[offset + 13],
                    ]);
                    rle.extend_with(value, length);
                }
                offset += 14;
            }
            Some(rle)
        } else {
            None
        };

        Some(Self {
            content,
            attrs,
            flags,
        })
    }

    /// Deserialize legacy format (without version byte or attrs).
    fn deserialize_legacy(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }

        let flags = LineFlags::from_bits_truncate(data[0]);
        let len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;

        if data.len() < 5 + len {
            return None;
        }

        let content = LineContent::from_bytes(&data[5..5 + len]);
        Some(Self {
            content,
            attrs: None,
            flags,
        })
    }

    /// Calculate memory used by this line.
    #[must_use]
    pub fn memory_used(&self) -> usize {
        let base = std::mem::size_of::<Self>();
        let content_mem = match &self.content {
            LineContent::Inline(_) => 0, // Already counted in size_of
            LineContent::Heap(v) => v.capacity(),
        };
        let attrs_mem = self.attrs.as_ref().map_or(0, |rle| {
            // RLE stores runs in a Vec
            rle.run_count() * std::mem::size_of::<crate::rle::Run<CellAttrs>>()
        });
        base + content_mem + attrs_mem
    }

    /// Calculate the number of attribute runs (for compression stats).
    #[must_use]
    pub fn attr_run_count(&self) -> usize {
        self.attrs.as_ref().map_or(0, |rle| rle.run_count())
    }
}

impl std::fmt::Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Serialize multiple lines for block compression.
#[must_use]
pub fn serialize_lines(lines: &[Line]) -> Vec<u8> {
    // Format: [count:4][line0][line1]...
    let mut result = Vec::new();
    // Block size is bounded by warm tier settings (typically 256-4096 lines)
    // Saturate at u32::MAX for safety
    #[allow(clippy::cast_possible_truncation)]
    let count = lines.len().min(u32::MAX as usize) as u32;
    result.extend_from_slice(&count.to_le_bytes());
    for line in lines {
        let serialized = line.serialize();
        result.extend_from_slice(&serialized);
    }
    result
}

/// Deserialize multiple lines from block.
///
/// Handles both legacy (v0) and new (v1) line formats by computing
/// line size dynamically from the serialized data.
#[must_use]
pub fn deserialize_lines(data: &[u8]) -> Vec<Line> {
    if data.len() < 4 {
        return Vec::new();
    }

    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut lines = Vec::with_capacity(count);
    let mut offset = 4;

    while offset < data.len() && lines.len() < count {
        // Peek at version to determine format
        let version = data[offset];

        let line_size = if version == 0 {
            // Legacy format: [flags:1][len:4][content:len]
            if offset + 5 > data.len() {
                break;
            }
            let content_len = u32::from_le_bytes([
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
            ]) as usize;
            5 + content_len
        } else {
            // v1 format: [version:1][flags:1][len:4][content:len][has_attrs:1][attrs...]
            if offset + 7 > data.len() {
                break;
            }
            let content_len = u32::from_le_bytes([
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
            ]) as usize;
            let attrs_offset = offset + 6 + content_len;
            if attrs_offset >= data.len() {
                break;
            }
            let has_attrs = data[attrs_offset] != 0;
            if has_attrs {
                if attrs_offset + 5 > data.len() {
                    break;
                }
                let run_count = u32::from_le_bytes([
                    data[attrs_offset + 1],
                    data[attrs_offset + 2],
                    data[attrs_offset + 3],
                    data[attrs_offset + 4],
                ]) as usize;
                // Each run is 14 bytes (10 for CellAttrs + 4 for length)
                7 + content_len + 4 + run_count * 14
            } else {
                7 + content_len
            }
        };

        let line_end = offset + line_size;
        if line_end > data.len() {
            break;
        }

        if let Some(line) = Line::deserialize(&data[offset..line_end]) {
            lines.push(line);
        }
        offset = line_end;
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_from_str() {
        let line = Line::from_str("Hello, World!");
        assert_eq!(line.to_string(), "Hello, World!");
        assert_eq!(line.len(), 13);
        assert!(!line.is_empty());
    }

    #[test]
    fn line_empty() {
        let line = Line::new();
        assert!(line.is_empty());
        assert_eq!(line.len(), 0);
    }

    #[test]
    fn line_wrapped_flag() {
        let mut line = Line::from_str("test");
        assert!(!line.is_wrapped());
        line.set_wrapped(true);
        assert!(line.is_wrapped());
        line.set_wrapped(false);
        assert!(!line.is_wrapped());
    }

    #[test]
    fn line_serialize_roundtrip() {
        let mut line = Line::from_str("Hello, World!");
        line.set_wrapped(true);

        let serialized = line.serialize();
        let deserialized = Line::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.to_string(), "Hello, World!");
        assert!(deserialized.is_wrapped());
    }

    #[test]
    fn serialize_lines_roundtrip() {
        let lines: Vec<Line> = (0..10)
            .map(|i| Line::from_str(&format!("Line {i}")))
            .collect();

        let serialized = serialize_lines(&lines);
        let deserialized = deserialize_lines(&serialized);

        assert_eq!(deserialized.len(), 10);
        for (i, line) in deserialized.iter().enumerate() {
            assert_eq!(line.to_string(), format!("Line {i}"));
        }
    }

    #[test]
    fn line_content_inline() {
        let short = LineContent::from_bytes(b"short");
        assert!(matches!(short, LineContent::Inline(_)));
        assert_eq!(short.len(), 5);
    }

    #[test]
    fn line_content_heap() {
        let long_data = vec![b'x'; 200];
        let long = LineContent::from_bytes(&long_data);
        assert!(matches!(long, LineContent::Heap(_)));
        assert_eq!(long.len(), 200);
    }

    #[test]
    fn line_memory_used() {
        let line = Line::from_str("test");
        assert!(line.memory_used() > 0);
    }

    // ==========================================================================
    // RLE Attribute Tests
    // ==========================================================================

    #[test]
    fn cell_attrs_default() {
        let attrs = CellAttrs::DEFAULT;
        assert!(attrs.is_default());
        assert_eq!(attrs.fg, DEFAULT_FG);
        assert_eq!(attrs.bg, DEFAULT_BG);
        assert_eq!(attrs.flags, 0);
    }

    #[test]
    fn cell_attrs_serialize_roundtrip() {
        let attrs = CellAttrs::new(0x01_FF0000, 0x01_00FF00, 0x0007);
        let serialized = attrs.serialize();
        let deserialized = CellAttrs::deserialize(&serialized).unwrap();
        assert_eq!(attrs, deserialized);
    }

    #[test]
    fn line_with_attrs() {
        let mut rle: Rle<CellAttrs> = Rle::new();
        // Simulate: 5 chars with red fg, 5 chars with default
        let red_attrs = CellAttrs::new(0x01_FF0000, DEFAULT_BG, 0);
        for _ in 0..5 {
            rle.push(red_attrs);
        }
        for _ in 0..5 {
            rle.push(CellAttrs::DEFAULT);
        }

        let line = Line::with_attrs("HelloWorld", rle);
        assert!(line.has_attrs());
        assert_eq!(line.attr_run_count(), 2);

        // Check attrs at specific positions
        assert_eq!(line.get_attr(0).fg, 0x01_FF0000);
        assert_eq!(line.get_attr(4).fg, 0x01_FF0000);
        assert_eq!(line.get_attr(5).fg, DEFAULT_FG);
    }

    #[test]
    fn line_with_attrs_all_default() {
        let mut rle: Rle<CellAttrs> = Rle::new();
        for _ in 0..10 {
            rle.push(CellAttrs::DEFAULT);
        }

        // When all attrs are default, the optimization should drop them
        let line = Line::with_attrs("HelloWorld", rle);
        assert!(!line.has_attrs());
        assert_eq!(line.attr_run_count(), 0);
    }

    #[test]
    fn line_serialize_roundtrip_with_attrs() {
        let mut rle: Rle<CellAttrs> = Rle::new();
        let red = CellAttrs::new(0x01_FF0000, DEFAULT_BG, 0);
        let green = CellAttrs::new(0x01_00FF00, DEFAULT_BG, 0);
        for _ in 0..3 {
            rle.push(red);
        }
        for _ in 0..7 {
            rle.push(green);
        }

        let mut line = Line::with_attrs("HelloWorld", rle);
        line.set_wrapped(true);

        let serialized = line.serialize();
        let deserialized = Line::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.to_string(), "HelloWorld");
        assert!(deserialized.is_wrapped());
        assert!(deserialized.has_attrs());
        assert_eq!(deserialized.attr_run_count(), 2);

        // Verify attrs
        assert_eq!(deserialized.get_attr(0).fg, 0x01_FF0000);
        assert_eq!(deserialized.get_attr(5).fg, 0x01_00FF00);
    }

    #[test]
    fn serialize_lines_roundtrip_with_attrs() {
        let mut lines = Vec::new();

        // Line 0: plain text (no attrs)
        lines.push(Line::from_str("Plain text"));

        // Line 1: with red attrs
        let mut rle: Rle<CellAttrs> = Rle::new();
        let red = CellAttrs::new(0x01_FF0000, DEFAULT_BG, 0);
        for _ in 0..10 {
            rle.push(red);
        }
        lines.push(Line::with_attrs("Red styled", rle));

        // Line 2: with mixed attrs
        let mut rle2: Rle<CellAttrs> = Rle::new();
        for _ in 0..5 {
            rle2.push(CellAttrs::DEFAULT);
        }
        for _ in 0..5 {
            rle2.push(CellAttrs::new(0x01_0000FF, DEFAULT_BG, 0x01)); // blue, bold
        }
        lines.push(Line::with_attrs("Mixed text", rle2));

        let serialized = serialize_lines(&lines);
        let deserialized = deserialize_lines(&serialized);

        assert_eq!(deserialized.len(), 3);
        assert_eq!(deserialized[0].to_string(), "Plain text");
        assert!(!deserialized[0].has_attrs());

        assert_eq!(deserialized[1].to_string(), "Red styled");
        assert!(deserialized[1].has_attrs());
        assert_eq!(deserialized[1].get_attr(0).fg, 0x01_FF0000);

        assert_eq!(deserialized[2].to_string(), "Mixed text");
        assert!(deserialized[2].has_attrs());
        assert!(deserialized[2].get_attr(0).is_default());
        assert_eq!(deserialized[2].get_attr(5).fg, 0x01_0000FF);
        assert_eq!(deserialized[2].get_attr(5).flags, 0x01); // bold
    }
}
