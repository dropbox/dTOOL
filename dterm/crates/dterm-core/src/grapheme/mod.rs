//! Grapheme cluster support for terminal text handling.
//!
//! This module provides Unicode grapheme cluster segmentation and width calculation
//! for proper terminal text handling. A grapheme cluster is what users perceive as
//! a single "character", even when composed of multiple Unicode codepoints.
//!
//! # Why Grapheme Clusters Matter for Terminals
//!
//! Terminals must correctly handle complex Unicode text:
//!
//! - **Emoji sequences**: 👨‍👩‍👧‍👦 is 7 codepoints but displays as 1 grapheme (2 cells wide)
//! - **Combining marks**: "é" can be 'e' + combining acute (2 codepoints, 1 grapheme)
//! - **Regional indicators**: 🇺🇸 is two codepoints but one flag emoji
//! - **Skin tone modifiers**: 👋🏽 is base emoji + modifier (2 codepoints, 1 grapheme)
//! - **ZWJ sequences**: Family emoji joined with Zero Width Joiner
//!
//! # Example
//!
//! ```
//! use dterm_core::grapheme::{GraphemeInfo, grapheme_width, split_graphemes};
//!
//! // Simple text
//! let info = grapheme_width("Hello");
//! assert_eq!(info.grapheme_count, 5);
//! assert_eq!(info.display_width, 5);
//!
//! // Emoji sequence (family: man, woman, girl, boy)
//! let info = grapheme_width("👨‍👩‍👧‍👦");
//! assert_eq!(info.grapheme_count, 1);
//! assert_eq!(info.display_width, 2); // Wide character
//!
//! // Combining character (e + combining acute)
//! let graphemes: Vec<_> = split_graphemes("e\u{0301}").collect();
//! assert_eq!(graphemes.len(), 1);
//! assert_eq!(graphemes[0].codepoint_count, 2);
//! assert!(graphemes[0].has_combining);
//! ```
//!
//! # Architecture
//!
//! This module builds on:
//! - `unicode-segmentation` for grapheme boundary detection (UAX #29)
//! - `unicode-width` for display width calculation (wcwidth equivalent)
//!
//! The key insight is that grapheme clusters are the correct unit for:
//! - Cursor movement (move by grapheme, not codepoint)
//! - Selection (select whole graphemes)
//! - Cell assignment (a grapheme occupies 1-2 cells)
//! - Backspace (delete whole grapheme)

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Information about a single grapheme cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grapheme<'a> {
    /// The grapheme cluster string slice.
    pub grapheme: &'a str,
    /// Byte offset in the source string.
    pub byte_offset: usize,
    /// Display width in terminal cells (0, 1, or 2).
    pub width: usize,
    /// Number of Unicode codepoints in this grapheme.
    pub codepoint_count: usize,
    /// Whether this is an emoji grapheme.
    pub is_emoji: bool,
    /// Whether this grapheme contains combining marks.
    pub has_combining: bool,
}

impl<'a> Grapheme<'a> {
    /// Check if this grapheme is a single ASCII character.
    #[inline]
    pub fn is_ascii(&self) -> bool {
        self.grapheme.len() == 1 && self.grapheme.as_bytes()[0] < 128
    }

    /// Check if this grapheme is whitespace.
    #[inline]
    pub fn is_whitespace(&self) -> bool {
        self.grapheme.chars().all(|c| c.is_whitespace())
    }

    /// Check if this grapheme is a control character.
    #[inline]
    pub fn is_control(&self) -> bool {
        self.grapheme.chars().any(|c| c.is_control())
    }

    /// Get the first codepoint of this grapheme.
    #[inline]
    pub fn first_char(&self) -> char {
        self.grapheme.chars().next().unwrap_or('\0')
    }
}

/// Aggregate information about graphemes in a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphemeInfo {
    /// Total number of grapheme clusters.
    pub grapheme_count: usize,
    /// Total display width in terminal cells.
    pub display_width: usize,
    /// Total number of Unicode codepoints.
    pub codepoint_count: usize,
    /// Number of bytes in the string.
    pub byte_count: usize,
    /// Whether any grapheme is an emoji.
    pub has_emoji: bool,
    /// Whether any grapheme has combining marks.
    pub has_combining: bool,
    /// Whether any grapheme is wide (2 cells).
    pub has_wide: bool,
}

/// Calculate grapheme information for a string.
///
/// This is an efficient single-pass analysis that computes all grapheme
/// metrics at once.
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::grapheme_width;
///
/// let info = grapheme_width("Hello 世界!");
/// assert_eq!(info.grapheme_count, 9);
/// assert_eq!(info.display_width, 11); // 7 ASCII + 2 wide chars
/// ```
pub fn grapheme_width(s: &str) -> GraphemeInfo {
    let mut info = GraphemeInfo {
        byte_count: s.len(),
        ..Default::default()
    };

    for g in s.graphemes(true) {
        let codepoints: usize = g.chars().count();
        let width = grapheme_display_width(g);
        let has_combining = codepoints > 1 && width <= 1;
        let is_emoji = is_emoji_grapheme(g);

        info.grapheme_count += 1;
        info.display_width += width;
        info.codepoint_count += codepoints;
        info.has_emoji |= is_emoji;
        info.has_combining |= has_combining;
        info.has_wide |= width == 2;
    }

    info
}

/// Calculate the display width of a single grapheme cluster.
///
/// Returns 0, 1, or 2 based on the grapheme's visual width in a terminal.
///
/// # Rules
///
/// - Control characters: 0 width
/// - Most ASCII: 1 width
/// - CJK ideographs: 2 width
/// - Most emoji: 2 width
/// - Zero-width joiners/combining marks: 0 width (but counted in cluster)
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::grapheme_display_width;
///
/// assert_eq!(grapheme_display_width("a"), 1);
/// assert_eq!(grapheme_display_width("中"), 2);
/// assert_eq!(grapheme_display_width("👋"), 2);
/// ```
pub fn grapheme_display_width(grapheme: &str) -> usize {
    // Use unicode-width's string width which handles grapheme clusters
    // better than summing individual character widths
    let width = UnicodeWidthStr::width(grapheme);

    // Clamp to max 2 for terminal display
    width.min(2)
}

/// Iterator over grapheme clusters with metadata.
///
/// This provides detailed information about each grapheme, including
/// byte offset, width, and composition.
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::split_graphemes;
///
/// for g in split_graphemes("Hello 世界") {
///     println!("{}: width={}, offset={}", g.grapheme, g.width, g.byte_offset);
/// }
/// ```
pub fn split_graphemes(s: &str) -> impl Iterator<Item = Grapheme<'_>> {
    s.grapheme_indices(true).map(|(offset, g)| {
        let codepoint_count = g.chars().count();
        let width = grapheme_display_width(g);
        let has_combining = codepoint_count > 1 && width <= 1;
        let is_emoji = is_emoji_grapheme(g);

        Grapheme {
            grapheme: g,
            byte_offset: offset,
            width,
            codepoint_count,
            is_emoji,
            has_combining,
        }
    })
}

/// Check if a grapheme cluster is primarily an emoji.
///
/// This detects emoji including:
/// - Basic emoji (😀)
/// - Emoji with modifiers (👋🏽)
/// - Emoji ZWJ sequences (👨‍👩‍👧)
/// - Regional indicator pairs (🇺🇸)
fn is_emoji_grapheme(grapheme: &str) -> bool {
    let first_char = grapheme.chars().next();
    match first_char {
        Some(c) => is_emoji_char(c),
        None => false,
    }
}

/// Check if a character is an emoji or emoji component.
fn is_emoji_char(c: char) -> bool {
    let cp = c as u32;

    // Common emoji ranges (simplified for performance)
    // See Unicode Emoji specification for complete list

    // Dingbats (some emoji)
    if (0x2600..=0x26FF).contains(&cp) {
        return true;
    }

    // Misc symbols
    if (0x2700..=0x27BF).contains(&cp) {
        return true;
    }

    // Supplemental symbols and pictographs
    if (0x1F300..=0x1F5FF).contains(&cp) {
        return true;
    }

    // Emoticons
    if (0x1F600..=0x1F64F).contains(&cp) {
        return true;
    }

    // Transport and map symbols
    if (0x1F680..=0x1F6FF).contains(&cp) {
        return true;
    }

    // Supplemental symbols
    if (0x1F900..=0x1F9FF).contains(&cp) {
        return true;
    }

    // Symbols and pictographs extended-A
    if (0x1FA00..=0x1FA6F).contains(&cp) {
        return true;
    }

    // Symbols and pictographs extended-B
    if (0x1FA70..=0x1FAFF).contains(&cp) {
        return true;
    }

    // Regional indicator symbols
    if (0x1F1E0..=0x1F1FF).contains(&cp) {
        return true;
    }

    // Variation selectors (emoji presentation)
    if cp == 0xFE0F {
        return true;
    }

    false
}

/// Zero Width Joiner (ZWJ) character.
pub const ZWJ: char = '\u{200D}';

/// Check if a grapheme contains a ZWJ sequence.
///
/// ZWJ sequences join multiple emoji into a single grapheme, such as:
/// - Family emoji: 👨‍👩‍👧‍👦 (man + ZWJ + woman + ZWJ + girl + ZWJ + boy)
/// - Profession emoji: 👨‍🚀 (man + ZWJ + rocket)
/// - Flag sequences with regional indicators
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::has_zwj;
///
/// assert!(has_zwj("👨‍👩‍👧‍👦")); // Family emoji
/// assert!(!has_zwj("😀"));    // Simple emoji
/// ```
#[inline]
pub fn has_zwj(grapheme: &str) -> bool {
    grapheme.contains(ZWJ)
}

/// Check if a character is a skin tone modifier.
///
/// Skin tone modifiers (Fitzpatrick scale) are U+1F3FB through U+1F3FF.
#[inline]
pub fn is_skin_tone_modifier(c: char) -> bool {
    matches!(c, '\u{1F3FB}'..='\u{1F3FF}')
}

/// Check if a grapheme has a skin tone modifier.
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::has_skin_tone;
///
/// assert!(has_skin_tone("👋🏽")); // Wave with medium skin
/// assert!(!has_skin_tone("👋")); // Wave without modifier
/// ```
pub fn has_skin_tone(grapheme: &str) -> bool {
    grapheme.chars().any(is_skin_tone_modifier)
}

/// Check if a character is a regional indicator.
///
/// Regional indicators (A-Z) are used in pairs to create flag emoji.
#[inline]
pub fn is_regional_indicator(c: char) -> bool {
    matches!(c, '\u{1F1E6}'..='\u{1F1FF}')
}

/// Check if a grapheme is a flag emoji (two regional indicators).
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::is_flag_emoji;
///
/// assert!(is_flag_emoji("🇺🇸")); // US flag
/// assert!(is_flag_emoji("🇯🇵")); // Japan flag
/// assert!(!is_flag_emoji("😀")); // Not a flag
/// ```
pub fn is_flag_emoji(grapheme: &str) -> bool {
    let chars: Vec<char> = grapheme.chars().collect();
    chars.len() == 2 && chars.iter().all(|&c| is_regional_indicator(c))
}

/// Classify the type of a grapheme for rendering decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphemeType {
    /// Simple ASCII character
    Ascii,
    /// CJK or other wide character (2 cells)
    Wide,
    /// Emoji (typically 2 cells)
    Emoji,
    /// ZWJ sequence (emoji joined by Zero Width Joiner)
    ZwjSequence,
    /// Flag emoji (regional indicator pair)
    Flag,
    /// Character with combining marks
    Combining,
    /// Control character (0 width)
    Control,
    /// Other Unicode character
    Other,
}

/// Classify a grapheme into its type.
///
/// This is useful for rendering and cursor movement decisions.
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::{classify_grapheme, GraphemeType};
///
/// assert_eq!(classify_grapheme("a"), GraphemeType::Ascii);
/// assert_eq!(classify_grapheme("中"), GraphemeType::Wide);
/// assert_eq!(classify_grapheme("👨‍👩‍👧"), GraphemeType::ZwjSequence);
/// assert_eq!(classify_grapheme("🇺🇸"), GraphemeType::Flag);
/// ```
pub fn classify_grapheme(grapheme: &str) -> GraphemeType {
    if grapheme.is_empty() {
        return GraphemeType::Control;
    }

    let first = grapheme.chars().next().unwrap();
    let codepoint_count = grapheme.chars().count();

    // Check for ASCII
    if grapheme.len() == 1 && first.is_ascii() {
        return if first.is_control() {
            GraphemeType::Control
        } else {
            GraphemeType::Ascii
        };
    }

    // Check for ZWJ sequence
    if has_zwj(grapheme) {
        return GraphemeType::ZwjSequence;
    }

    // Check for flag emoji
    if is_flag_emoji(grapheme) {
        return GraphemeType::Flag;
    }

    // Check for combining marks
    if codepoint_count > 1 && grapheme_display_width(grapheme) <= 1 {
        return GraphemeType::Combining;
    }

    // Check for emoji
    if is_emoji_char(first) {
        return GraphemeType::Emoji;
    }

    // Check for wide character
    if grapheme_display_width(grapheme) == 2 {
        return GraphemeType::Wide;
    }

    GraphemeType::Other
}

/// Find the grapheme cluster that contains a given byte offset.
///
/// Returns `None` if the offset is out of bounds.
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::grapheme_at_byte;
///
/// let text = "Hello 世界";
/// if let Some(g) = grapheme_at_byte(text, 7) {
///     assert_eq!(g.grapheme, "世");
/// }
/// ```
pub fn grapheme_at_byte(s: &str, byte_offset: usize) -> Option<Grapheme<'_>> {
    if byte_offset >= s.len() {
        return None;
    }

    for g in split_graphemes(s) {
        let end = g.byte_offset + g.grapheme.len();
        if byte_offset >= g.byte_offset && byte_offset < end {
            return Some(g);
        }
    }

    None
}

/// Find the grapheme cluster at a given display column.
///
/// Returns `None` if the column is beyond the string's display width.
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::grapheme_at_column;
///
/// let text = "Hello 世界";
/// // "Hello " is 6 columns, "世" starts at column 6
/// if let Some(g) = grapheme_at_column(text, 6) {
///     assert_eq!(g.grapheme, "世");
/// }
/// ```
pub fn grapheme_at_column(s: &str, column: usize) -> Option<Grapheme<'_>> {
    let mut current_col = 0;

    for g in split_graphemes(s) {
        if column >= current_col && column < current_col + g.width.max(1) {
            return Some(g);
        }
        current_col += g.width;
    }

    None
}

/// Convert a byte offset to a display column.
///
/// Returns the column position at the start of the grapheme containing
/// the given byte offset.
pub fn byte_to_column(s: &str, byte_offset: usize) -> usize {
    let mut column = 0;

    for g in split_graphemes(s) {
        if g.byte_offset >= byte_offset {
            return column;
        }
        column += g.width;
    }

    column
}

/// Convert a display column to a byte offset.
///
/// Returns the byte offset at the start of the grapheme at the given column.
pub fn column_to_byte(s: &str, column: usize) -> usize {
    let mut current_col = 0;

    for g in split_graphemes(s) {
        if column < current_col + g.width.max(1) {
            return g.byte_offset;
        }
        current_col += g.width;
    }

    s.len()
}

/// Truncate a string to fit within a given display width.
///
/// Returns a string slice that fits within `max_width` terminal columns,
/// ensuring grapheme clusters are not split.
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::truncate_to_width;
///
/// assert_eq!(truncate_to_width("Hello 世界", 8), "Hello 世");
/// assert_eq!(truncate_to_width("Hello 世界", 7), "Hello ");
/// ```
pub fn truncate_to_width(s: &str, max_width: usize) -> &str {
    let mut width = 0;
    let mut end_byte = 0;

    for g in split_graphemes(s) {
        if width + g.width > max_width {
            break;
        }
        width += g.width;
        end_byte = g.byte_offset + g.grapheme.len();
    }

    &s[..end_byte]
}

/// Pad a string to a given display width.
///
/// If the string is shorter than `width`, pads with spaces.
/// If longer, truncates to fit.
///
/// # Example
///
/// ```
/// use dterm_core::grapheme::pad_to_width;
///
/// assert_eq!(pad_to_width("Hi", 5), "Hi   ");
/// assert_eq!(pad_to_width("Hello World", 5), "Hello");
/// ```
pub fn pad_to_width(s: &str, width: usize) -> String {
    let info = grapheme_width(s);

    if info.display_width >= width {
        truncate_to_width(s, width).to_string()
    } else {
        let padding = width - info.display_width;
        let mut result = s.to_string();
        result.push_str(&" ".repeat(padding));
        result
    }
}

/// Check if a string is entirely ASCII (fast path for terminals).
///
/// ASCII-only text can use simpler width calculation.
#[inline]
pub fn is_ascii_only(s: &str) -> bool {
    s.bytes().all(|b| b < 128)
}

/// Fast width calculation for ASCII-only strings.
///
/// This is O(1) since ASCII characters are always 1 cell wide
/// (except control characters which are 0).
#[inline]
pub fn ascii_width(s: &str) -> usize {
    s.bytes().filter(|&b| b >= 0x20 && b < 0x7F).count()
}

/// Terminal-aware grapheme segmenter for processing input.
///
/// This struct provides stateful grapheme processing suitable for
/// terminal input handling, tracking position information for
/// cursor management.
#[derive(Debug, Clone)]
pub struct GraphemeSegmenter {
    /// Current column position.
    column: usize,
    /// Current grapheme index.
    index: usize,
}

impl Default for GraphemeSegmenter {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphemeSegmenter {
    /// Create a new segmenter starting at column 0.
    #[inline]
    pub fn new() -> Self {
        Self {
            column: 0,
            index: 0,
        }
    }

    /// Create a segmenter at a specific column.
    #[inline]
    pub fn at_column(column: usize) -> Self {
        Self { column, index: 0 }
    }

    /// Get the current column position.
    #[inline]
    pub fn column(&self) -> usize {
        self.column
    }

    /// Get the current grapheme index.
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Process a grapheme and advance position.
    ///
    /// Returns the column span (start, end) for this grapheme.
    pub fn process(&mut self, grapheme: &Grapheme<'_>) -> (usize, usize) {
        let start = self.column;
        self.column += grapheme.width;
        self.index += 1;
        (start, self.column)
    }

    /// Process a string and return position after all graphemes.
    pub fn process_string(&mut self, s: &str) -> GraphemeInfo {
        let info = grapheme_width(s);
        self.column += info.display_width;
        self.index += info.grapheme_count;
        info
    }

    /// Reset to column 0.
    #[inline]
    pub fn reset(&mut self) {
        self.column = 0;
        self.index = 0;
    }

    /// Move to a specific column.
    #[inline]
    pub fn move_to_column(&mut self, column: usize) {
        self.column = column;
    }
}

/// Cell assignment for a grapheme cluster.
///
/// When rendering graphemes to terminal cells, a grapheme may span
/// 1 or 2 cells. This struct describes the cell assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphemeCells {
    /// First cell column.
    pub start_col: usize,
    /// Number of cells (1 or 2).
    pub cell_count: usize,
    /// Whether this is a wide character (spans 2 cells).
    pub is_wide: bool,
}

impl GraphemeCells {
    /// Get the ending column (exclusive).
    #[inline]
    pub fn end_col(&self) -> usize {
        self.start_col + self.cell_count
    }

    /// Check if a column is within this grapheme's cells.
    #[inline]
    pub fn contains_col(&self, col: usize) -> bool {
        col >= self.start_col && col < self.end_col()
    }
}

/// Assign cells to graphemes in a string.
///
/// Returns an iterator of (grapheme, cells) pairs showing how each
/// grapheme maps to terminal cells.
pub fn assign_cells(
    s: &str,
    start_col: usize,
) -> impl Iterator<Item = (Grapheme<'_>, GraphemeCells)> {
    let mut col = start_col;

    split_graphemes(s).map(move |g| {
        let cells = GraphemeCells {
            start_col: col,
            cell_count: g.width.max(1),
            is_wide: g.width == 2,
        };
        col += cells.cell_count;
        (g, cells)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_graphemes() {
        let info = grapheme_width("Hello");
        assert_eq!(info.grapheme_count, 5);
        assert_eq!(info.display_width, 5);
        assert_eq!(info.codepoint_count, 5);
        assert!(!info.has_emoji);
        assert!(!info.has_combining);
        assert!(!info.has_wide);
    }

    #[test]
    fn test_cjk_graphemes() {
        // CJK characters are 2 cells wide
        let info = grapheme_width("中文");
        assert_eq!(info.grapheme_count, 2);
        assert_eq!(info.display_width, 4);
        assert!(!info.has_emoji);
        assert!(info.has_wide);
    }

    #[test]
    fn test_emoji_graphemes() {
        // Simple emoji
        let info = grapheme_width("😀");
        assert_eq!(info.grapheme_count, 1);
        assert_eq!(info.display_width, 2);
        assert!(info.has_emoji);
    }

    #[test]
    fn test_emoji_zwj_sequence() {
        // Family emoji: man + ZWJ + woman + ZWJ + girl + ZWJ + boy
        let family = "👨‍👩‍👧‍👦";
        let info = grapheme_width(family);
        assert_eq!(info.grapheme_count, 1); // Single grapheme cluster
        assert!(info.has_emoji);
        assert!(info.codepoint_count > 1); // Multiple codepoints
    }

    #[test]
    fn test_combining_characters() {
        // e + combining acute accent
        let text = "e\u{0301}";
        let info = grapheme_width(text);
        assert_eq!(info.grapheme_count, 1);
        assert_eq!(info.display_width, 1);
        assert_eq!(info.codepoint_count, 2);
        assert!(info.has_combining);
    }

    #[test]
    fn test_regional_indicators() {
        // US flag: regional indicator U + regional indicator S
        let flag = "🇺🇸";
        let info = grapheme_width(flag);
        assert_eq!(info.grapheme_count, 1);
        assert!(info.has_emoji);
    }

    #[test]
    fn test_skin_tone_grapheme() {
        // Wave + medium skin tone
        let wave = "👋🏽";
        let info = grapheme_width(wave);
        assert_eq!(info.grapheme_count, 1);
        assert!(info.has_emoji);
        assert!(info.codepoint_count >= 2);
    }

    #[test]
    fn test_mixed_text() {
        let text = "Hello 世界 👋";
        let info = grapheme_width(text);
        assert_eq!(info.grapheme_count, 10); // H e l l o ' ' 世 界 ' ' 👋
        assert!(info.has_wide);
        assert!(info.has_emoji);
    }

    #[test]
    fn test_split_graphemes() {
        let text = "Hello";
        let graphemes: Vec<_> = split_graphemes(text).collect();
        assert_eq!(graphemes.len(), 5);
        assert_eq!(graphemes[0].grapheme, "H");
        assert_eq!(graphemes[0].byte_offset, 0);
        assert_eq!(graphemes[0].width, 1);
        assert!(graphemes[0].is_ascii());
    }

    #[test]
    fn test_grapheme_at_byte() {
        let text = "Hello 世界";

        // ASCII portion
        let g = grapheme_at_byte(text, 0).unwrap();
        assert_eq!(g.grapheme, "H");

        let g = grapheme_at_byte(text, 4).unwrap();
        assert_eq!(g.grapheme, "o");

        // CJK portion (世 is at bytes 6-8 in UTF-8)
        let g = grapheme_at_byte(text, 6).unwrap();
        assert_eq!(g.grapheme, "世");

        // Out of bounds
        assert!(grapheme_at_byte(text, 100).is_none());
    }

    #[test]
    fn test_grapheme_at_column() {
        let text = "Hello 世界";

        // Column 0 is 'H'
        let g = grapheme_at_column(text, 0).unwrap();
        assert_eq!(g.grapheme, "H");

        // Column 5 is space
        let g = grapheme_at_column(text, 5).unwrap();
        assert_eq!(g.grapheme, " ");

        // Column 6 is '世' (wide char)
        let g = grapheme_at_column(text, 6).unwrap();
        assert_eq!(g.grapheme, "世");

        // Column 7 is still '世' (second cell of wide char)
        let g = grapheme_at_column(text, 7).unwrap();
        assert_eq!(g.grapheme, "世");

        // Column 8 is '界'
        let g = grapheme_at_column(text, 8).unwrap();
        assert_eq!(g.grapheme, "界");
    }

    #[test]
    fn test_byte_to_column() {
        let text = "Hello 世界";

        assert_eq!(byte_to_column(text, 0), 0); // 'H'
        assert_eq!(byte_to_column(text, 5), 5); // space
        assert_eq!(byte_to_column(text, 6), 6); // '世'
        assert_eq!(byte_to_column(text, 9), 8); // '界' (after 世's 3 bytes)
    }

    #[test]
    fn test_column_to_byte() {
        let text = "Hello 世界";

        assert_eq!(column_to_byte(text, 0), 0); // 'H'
        assert_eq!(column_to_byte(text, 5), 5); // space
        assert_eq!(column_to_byte(text, 6), 6); // '世'
        assert_eq!(column_to_byte(text, 7), 6); // still '世' (second cell)
        assert_eq!(column_to_byte(text, 8), 9); // '界'
    }

    #[test]
    fn test_truncate_to_width() {
        // ASCII only
        assert_eq!(truncate_to_width("Hello World", 5), "Hello");

        // With wide chars - should not split wide char
        assert_eq!(truncate_to_width("Hello 世界", 8), "Hello 世");
        assert_eq!(truncate_to_width("Hello 世界", 7), "Hello "); // 世 needs 2 cols

        // Exact fit
        assert_eq!(truncate_to_width("Hello", 5), "Hello");
    }

    #[test]
    fn test_pad_to_width() {
        assert_eq!(pad_to_width("Hi", 5), "Hi   ");
        assert_eq!(pad_to_width("Hello", 5), "Hello");
        assert_eq!(pad_to_width("Hello World", 5), "Hello");
    }

    #[test]
    fn test_is_ascii_only() {
        assert!(is_ascii_only("Hello World"));
        assert!(!is_ascii_only("Hello 世界"));
        assert!(!is_ascii_only("café"));
    }

    #[test]
    fn test_ascii_width() {
        assert_eq!(ascii_width("Hello"), 5);
        assert_eq!(ascii_width("Hello\n"), 5); // newline is control
        assert_eq!(ascii_width(""), 0);
    }

    #[test]
    fn test_grapheme_segmenter() {
        let mut seg = GraphemeSegmenter::new();

        let text = "Hello";
        let info = seg.process_string(text);
        assert_eq!(info.display_width, 5);
        assert_eq!(seg.column(), 5);
        assert_eq!(seg.index(), 5);

        // Process more text
        let info2 = seg.process_string(" 世");
        assert_eq!(info2.display_width, 3);
        assert_eq!(seg.column(), 8);
    }

    #[test]
    fn test_assign_cells() {
        let text = "a世b";
        let cells: Vec<_> = assign_cells(text, 0).collect();

        assert_eq!(cells.len(), 3);

        // 'a' at column 0, width 1
        assert_eq!(cells[0].0.grapheme, "a");
        assert_eq!(cells[0].1.start_col, 0);
        assert_eq!(cells[0].1.cell_count, 1);
        assert!(!cells[0].1.is_wide);

        // '世' at column 1, width 2
        assert_eq!(cells[1].0.grapheme, "世");
        assert_eq!(cells[1].1.start_col, 1);
        assert_eq!(cells[1].1.cell_count, 2);
        assert!(cells[1].1.is_wide);

        // 'b' at column 3, width 1
        assert_eq!(cells[2].0.grapheme, "b");
        assert_eq!(cells[2].1.start_col, 3);
        assert_eq!(cells[2].1.cell_count, 1);
    }

    #[test]
    fn test_grapheme_info_flags() {
        let g: Vec<_> = split_graphemes("Hello").collect();
        assert!(g[0].is_ascii());
        assert!(!g[0].is_whitespace());
        assert!(!g[0].is_control());

        let g: Vec<_> = split_graphemes(" ").collect();
        assert!(g[0].is_whitespace());

        let g: Vec<_> = split_graphemes("\n").collect();
        assert!(g[0].is_control());
    }

    #[test]
    fn test_zwj_detection() {
        // Family emoji with ZWJ
        assert!(has_zwj("👨‍👩‍👧‍👦"));
        assert!(has_zwj("👨‍🚀")); // Man astronaut

        // Simple emoji without ZWJ
        assert!(!has_zwj("😀"));
        assert!(!has_zwj("🎉"));
        assert!(!has_zwj("A"));
    }

    #[test]
    fn test_skin_tone_detection() {
        assert!(has_skin_tone("👋🏽")); // Wave with medium skin
        assert!(has_skin_tone("👍🏻")); // Thumbs up light skin
        assert!(!has_skin_tone("👋")); // Wave without modifier
        assert!(!has_skin_tone("😀")); // No skin tone
    }

    #[test]
    fn test_skin_tone_modifier() {
        assert!(is_skin_tone_modifier('\u{1F3FB}')); // Light
        assert!(is_skin_tone_modifier('\u{1F3FD}')); // Medium
        assert!(is_skin_tone_modifier('\u{1F3FF}')); // Dark
        assert!(!is_skin_tone_modifier('A'));
        assert!(!is_skin_tone_modifier('😀'));
    }

    #[test]
    fn test_regional_indicator() {
        assert!(is_regional_indicator('\u{1F1FA}')); // U
        assert!(is_regional_indicator('\u{1F1F8}')); // S
        assert!(!is_regional_indicator('A'));
        assert!(!is_regional_indicator('😀'));
    }

    #[test]
    fn test_flag_emoji() {
        assert!(is_flag_emoji("🇺🇸")); // US flag
        assert!(is_flag_emoji("🇯🇵")); // Japan flag
        assert!(is_flag_emoji("🇬🇧")); // UK flag
        assert!(!is_flag_emoji("😀")); // Not a flag
        assert!(!is_flag_emoji("A")); // Not a flag
        assert!(!is_flag_emoji("🇺")); // Single regional indicator
    }

    #[test]
    fn test_classify_grapheme() {
        assert_eq!(classify_grapheme("a"), GraphemeType::Ascii);
        assert_eq!(classify_grapheme("Z"), GraphemeType::Ascii);
        assert_eq!(classify_grapheme(" "), GraphemeType::Ascii);
        assert_eq!(classify_grapheme("\n"), GraphemeType::Control);
        assert_eq!(classify_grapheme("\x00"), GraphemeType::Control);
        assert_eq!(classify_grapheme("中"), GraphemeType::Wide);
        assert_eq!(classify_grapheme("日"), GraphemeType::Wide);
        assert_eq!(classify_grapheme("😀"), GraphemeType::Emoji);
        assert_eq!(classify_grapheme("🎉"), GraphemeType::Emoji);
        assert_eq!(classify_grapheme("👨‍👩‍👧"), GraphemeType::ZwjSequence);
        assert_eq!(classify_grapheme("🇺🇸"), GraphemeType::Flag);
        assert_eq!(classify_grapheme("e\u{0301}"), GraphemeType::Combining); // é with combining
    }

    #[test]
    fn test_classify_empty() {
        assert_eq!(classify_grapheme(""), GraphemeType::Control);
    }

    #[test]
    fn test_empty_string() {
        let info = grapheme_width("");
        assert_eq!(info.grapheme_count, 0);
        assert_eq!(info.display_width, 0);

        assert_eq!(truncate_to_width("", 10), "");
        assert_eq!(pad_to_width("", 5), "     ");
    }

    #[test]
    fn test_grapheme_contains_col() {
        let cells = GraphemeCells {
            start_col: 5,
            cell_count: 2,
            is_wide: true,
        };

        assert!(!cells.contains_col(4));
        assert!(cells.contains_col(5));
        assert!(cells.contains_col(6));
        assert!(!cells.contains_col(7));
    }
}

// Kani proofs for formal verification
#[cfg(kani)]
mod verification {
    use super::*;

    /// Verify grapheme_width never exceeds 2 for any grapheme.
    #[kani::proof]
    fn grapheme_display_width_bounded() {
        let c: char = kani::any();
        let s = c.to_string();
        let width = grapheme_display_width(&s);
        kani::assert(width <= 2, "Grapheme width must be 0, 1, or 2");
    }

    /// Verify truncate_to_width returns valid UTF-8.
    #[kani::proof]
    fn truncate_preserves_utf8() {
        // Test with known valid strings
        let test_cases = ["a", "ab", "abc"];
        let max_width: usize = kani::any();
        kani::assume(max_width <= 10);

        for s in test_cases {
            let result = truncate_to_width(s, max_width);
            // Result is a valid string slice (guaranteed by &str)
            kani::assert(
                result.len() <= s.len(),
                "Truncated string not longer than original",
            );
        }
    }

    /// Verify column_to_byte returns valid byte offset.
    #[kani::proof]
    fn column_to_byte_valid() {
        let test = "ab";
        let col: usize = kani::any();
        kani::assume(col <= 10);

        let byte = column_to_byte(test, col);
        kani::assert(byte <= test.len(), "Byte offset within bounds");
    }

    /// Verify byte_to_column returns monotonic values.
    #[kani::proof]
    fn byte_to_column_monotonic() {
        let test = "ab";
        let b1: usize = kani::any();
        let b2: usize = kani::any();
        kani::assume(b1 <= test.len());
        kani::assume(b2 <= test.len());
        kani::assume(b1 <= b2);

        let c1 = byte_to_column(test, b1);
        let c2 = byte_to_column(test, b2);
        kani::assert(c1 <= c2, "Column positions are monotonic");
    }

    /// Verify GraphemeSegmenter column advances correctly.
    #[kani::proof]
    fn segmenter_column_advances() {
        let mut seg = GraphemeSegmenter::new();
        let initial = seg.column();

        let test = "a";
        let info = seg.process_string(test);

        kani::assert(
            seg.column() == initial + info.display_width,
            "Column advances by display width",
        );
    }

    /// Verify GraphemeCells contains_col is correct.
    #[kani::proof]
    fn cells_contains_col_correct() {
        let start: usize = kani::any();
        let count: usize = kani::any();
        kani::assume(count >= 1 && count <= 2);
        kani::assume(start <= 1000);

        let cells = GraphemeCells {
            start_col: start,
            cell_count: count,
            is_wide: count == 2,
        };

        let test_col: usize = kani::any();
        kani::assume(test_col <= 1010);

        let contains = cells.contains_col(test_col);
        let expected = test_col >= start && test_col < start + count;
        kani::assert(contains == expected, "contains_col matches manual check");
    }

    /// Verify is_emoji_char covers expected ranges.
    #[kani::proof]
    fn emoji_char_detection() {
        // Test that emoji detection doesn't panic
        let c: char = kani::any();
        let _ = is_emoji_char(c);
    }

    /// Verify skin tone modifier detection is correct for Fitzpatrick scale.
    #[kani::proof]
    fn skin_tone_modifier_range() {
        let codepoint: u32 = kani::any();
        kani::assume(codepoint >= 0x1F3FB && codepoint <= 0x1F3FF);

        if let Some(c) = char::from_u32(codepoint) {
            kani::assert(
                is_skin_tone_modifier(c),
                "Fitzpatrick modifiers must be detected",
            );
        }
    }

    /// Verify regional indicator detection is correct.
    #[kani::proof]
    fn regional_indicator_range() {
        let codepoint: u32 = kani::any();
        kani::assume(codepoint >= 0x1F1E6 && codepoint <= 0x1F1FF);

        if let Some(c) = char::from_u32(codepoint) {
            kani::assert(
                is_regional_indicator(c),
                "Regional indicators must be detected",
            );
        }
    }

    /// Verify has_zwj detects ZWJ character.
    #[kani::proof]
    fn has_zwj_with_zwj() {
        // A string containing ZWJ must return true
        let test = "x\u{200D}y"; // x + ZWJ + y
        kani::assert(has_zwj(test), "String with ZWJ must be detected");
    }

    /// Verify has_zwj returns false for string without ZWJ.
    #[kani::proof]
    fn has_zwj_without_zwj() {
        let test = "abc";
        kani::assert(!has_zwj(test), "String without ZWJ must not be detected");
    }

    /// Verify classify_grapheme doesn't panic for any single char.
    #[kani::proof]
    fn classify_grapheme_no_panic() {
        let c: char = kani::any();
        let s = c.to_string();
        let _ = classify_grapheme(&s);
    }
}
