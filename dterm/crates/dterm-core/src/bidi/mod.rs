//! # BiDi (Bidirectional Text) Support
//!
//! Implements the Unicode Bidirectional Algorithm (UBA) for terminals.
//!
//! This module provides support for displaying right-to-left (RTL) text such as
//! Arabic, Hebrew, Syriac, and other RTL scripts alongside left-to-right (LTR) text.
//!
//! ## Architecture
//!
//! - **Storage**: Cells are stored in *logical* order (as received from PTY)
//! - **Rendering**: At render time, cells are reordered to *visual* order
//! - **Per-line**: BiDi resolution is performed per-line for terminal display
//!
//! ## Usage
//!
//! ```rust
//! use dterm_core::bidi::{BidiResolver, ParagraphDirection};
//!
//! let resolver = BidiResolver::new();
//!
//! // Resolve a line of mixed LTR/RTL text
//! let text = "Hello שלום World";
//! let result = resolver.resolve_line(text, ParagraphDirection::Auto);
//!
//! // Get visual ordering for rendering
//! for run in result.visual_runs() {
//!     let direction = run.direction;
//!     let range = run.range.clone();
//!     // Render characters from text[range] in the given direction
//! }
//! ```
//!
//! ## Terminal-Specific Behavior
//!
//! Terminals have specific BiDi requirements:
//!
//! 1. **NSM Reordering**: Non-spacing marks (combining characters) follow
//!    rule L3 of UBA for terminal display
//! 2. **Line-based**: Each line is resolved independently
//! 3. **Explicit Controls**: Terminals support explicit BiDi controls (RLE, LRE, etc.)
//!
//! ## References
//!
//! - [Unicode Standard Annex #9: Unicode Bidirectional Algorithm](https://unicode.org/reports/tr9/)
//! - [Terminal BiDi RFC](https://terminal-wg.pages.freedesktop.org/bidi/)

use unicode_bidi::{bidi_class, BidiClass, BidiInfo, Level, ParagraphInfo};

/// Direction of text flow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Left-to-right (e.g., Latin, Cyrillic)
    #[default]
    LeftToRight,
    /// Right-to-left (e.g., Arabic, Hebrew)
    RightToLeft,
}

impl Direction {
    /// Returns the opposite direction
    #[inline]
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Direction::LeftToRight => Direction::RightToLeft,
            Direction::RightToLeft => Direction::LeftToRight,
        }
    }

    /// Create direction from embedding level (odd = RTL, even = LTR)
    #[inline]
    pub fn from_level(level: Level) -> Self {
        if level.is_rtl() {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        }
    }
}

/// Hint for determining paragraph direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParagraphDirection {
    /// Auto-detect from first strong character, default to LTR
    #[default]
    Auto,
    /// Auto-detect from first strong character, default to RTL
    AutoRtl,
    /// Force left-to-right
    Ltr,
    /// Force right-to-left
    Rtl,
}

impl ParagraphDirection {
    /// Convert to unicode-bidi `Option<Level>` for BidiInfo::new
    fn to_level(self) -> Option<Level> {
        match self {
            ParagraphDirection::Auto | ParagraphDirection::AutoRtl => None,
            ParagraphDirection::Ltr => Some(Level::ltr()),
            ParagraphDirection::Rtl => Some(Level::rtl()),
        }
    }

    /// Get the default direction if auto-detection doesn't find a strong character
    fn default_direction(self) -> Direction {
        match self {
            ParagraphDirection::AutoRtl | ParagraphDirection::Rtl => Direction::RightToLeft,
            ParagraphDirection::Auto | ParagraphDirection::Ltr => Direction::LeftToRight,
        }
    }
}

/// A run of text with uniform direction
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BidiRun {
    /// Direction of this run
    pub direction: Direction,
    /// Embedding level (even = LTR, odd = RTL)
    pub level: u8,
    /// Range of character indices in the original text (logical order)
    pub range: core::ops::Range<usize>,
}

impl BidiRun {
    /// Returns an iterator over indices in visual order
    ///
    /// For LTR runs, indices are in ascending order.
    /// For RTL runs, indices are in descending order.
    pub fn visual_indices(&self) -> impl Iterator<Item = usize> + '_ {
        let range = self.range.clone();
        let is_rtl = self.direction == Direction::RightToLeft;

        VisualIndices {
            range,
            is_rtl,
            pos: 0,
        }
    }
}

struct VisualIndices {
    range: core::ops::Range<usize>,
    is_rtl: bool,
    pos: usize,
}

impl Iterator for VisualIndices {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        let len = self.range.len();
        if self.pos >= len {
            return None;
        }
        let idx = if self.is_rtl {
            self.range.end - 1 - self.pos
        } else {
            self.range.start + self.pos
        };
        self.pos += 1;
        Some(idx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.range.len().saturating_sub(self.pos);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for VisualIndices {}

/// Result of BiDi resolution for a line
#[derive(Debug, Clone)]
pub struct BidiResolution {
    /// Paragraph/base direction
    pub base_direction: Direction,
    /// Runs in visual order (left-to-right on screen)
    runs: Vec<BidiRun>,
    /// Mapping from logical to visual position
    visual_positions: Vec<usize>,
    /// Mapping from visual to logical position
    logical_positions: Vec<usize>,
}

impl BidiResolution {
    /// Returns true if text contains any RTL characters
    pub fn has_rtl(&self) -> bool {
        self.runs
            .iter()
            .any(|r| r.direction == Direction::RightToLeft)
    }

    /// Returns true if the text is purely LTR (no reordering needed)
    pub fn is_pure_ltr(&self) -> bool {
        self.base_direction == Direction::LeftToRight
            && self.runs.len() == 1
            && self.runs[0].direction == Direction::LeftToRight
    }

    /// Get runs in visual order (for rendering)
    pub fn visual_runs(&self) -> &[BidiRun] {
        &self.runs
    }

    /// Get the visual position of a logical character index
    ///
    /// Returns the column where this character should be displayed.
    pub fn logical_to_visual(&self, logical_idx: usize) -> Option<usize> {
        self.visual_positions.get(logical_idx).copied()
    }

    /// Get the logical position of a visual column
    ///
    /// Returns the character index in the original text.
    pub fn visual_to_logical(&self, visual_idx: usize) -> Option<usize> {
        self.logical_positions.get(visual_idx).copied()
    }

    /// Get the total length (number of characters)
    pub fn len(&self) -> usize {
        self.visual_positions.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.visual_positions.is_empty()
    }
}

/// BiDi resolver for terminal text
///
/// This is stateless and can be shared across threads.
#[derive(Debug, Clone, Default)]
pub struct BidiResolver {
    /// Whether to reorder non-spacing marks according to rule L3
    /// (terminal-specific behavior)
    reorder_nsm: bool,
}

impl BidiResolver {
    /// Create a new BiDi resolver with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new BiDi resolver with NSM reordering enabled
    ///
    /// NSM (non-spacing mark) reordering follows UBA rule L3,
    /// which is the expected behavior for terminal applications.
    pub fn with_nsm_reordering() -> Self {
        Self { reorder_nsm: true }
    }

    /// Enable or disable NSM reordering
    pub fn set_reorder_nsm(&mut self, reorder: bool) {
        self.reorder_nsm = reorder;
    }

    /// Resolve BiDi for a line of text
    ///
    /// # Arguments
    ///
    /// * `text` - The line of text to resolve
    /// * `direction` - Hint for paragraph direction
    ///
    /// # Returns
    ///
    /// A `BidiResolution` containing the visual ordering information.
    pub fn resolve_line(&self, text: &str, direction: ParagraphDirection) -> BidiResolution {
        // Fast path: empty text
        if text.is_empty() {
            return BidiResolution {
                base_direction: direction.default_direction(),
                runs: vec![],
                visual_positions: vec![],
                logical_positions: vec![],
            };
        }

        // Fast path: check if all characters are LTR
        // This is the common case for most terminal users
        if self.is_all_ltr(text) && !matches!(direction, ParagraphDirection::Rtl) {
            let len = text.chars().count();
            return BidiResolution {
                base_direction: Direction::LeftToRight,
                runs: vec![BidiRun {
                    direction: Direction::LeftToRight,
                    level: 0,
                    range: 0..len,
                }],
                visual_positions: (0..len).collect(),
                logical_positions: (0..len).collect(),
            };
        }

        // Full BiDi resolution
        let bidi_info = BidiInfo::new(text, direction.to_level());

        // Handle first paragraph (terminals process line by line)
        let para = bidi_info.paragraphs.first().map_or_else(
            || ParagraphInfo {
                range: 0..text.len(),
                level: direction.to_level().unwrap_or_else(Level::ltr),
            },
            |p| p.clone(),
        );

        let base_direction = Direction::from_level(para.level);

        // Get the reordered indices and levels
        let line_range = para.range.clone();
        let (levels, reordered) = bidi_info.visual_runs(&para, line_range.clone());

        // Build visual runs
        let mut runs = Vec::new();
        for run_range in &reordered {
            let run_start = run_range.start;
            let run_end = run_range.end;
            if run_start >= run_end {
                continue;
            }
            // Get level for this run (all chars in run have same level after reordering)
            let level = levels.get(run_start).copied().unwrap_or_else(Level::ltr);
            let direction = Direction::from_level(level);

            // Convert byte indices to char indices
            let char_start = text[..run_start].chars().count();
            let char_end = text[..run_end].chars().count();

            runs.push(BidiRun {
                direction,
                level: level.number(),
                range: char_start..char_end,
            });
        }

        // If no runs were created, create a single LTR run
        if runs.is_empty() {
            let len = text.chars().count();
            runs.push(BidiRun {
                direction: base_direction,
                level: para.level.number(),
                range: 0..len,
            });
        }

        // Build position mappings
        let char_count = text.chars().count();
        let mut visual_positions = vec![0; char_count];
        let mut logical_positions = vec![0; char_count];

        let mut visual_pos = 0;
        for run in &runs {
            for logical_idx in run.visual_indices() {
                if logical_idx < char_count {
                    visual_positions[logical_idx] = visual_pos;
                    logical_positions[visual_pos] = logical_idx;
                    visual_pos += 1;
                }
            }
        }

        // Apply NSM reordering if enabled
        if self.reorder_nsm {
            self.reorder_nsm_marks(text, &mut visual_positions, &mut logical_positions);
        }

        BidiResolution {
            base_direction,
            runs,
            visual_positions,
            logical_positions,
        }
    }

    /// Resolve BiDi for a line represented as cell indices
    ///
    /// This variant works with pre-extracted codepoints, avoiding string allocation.
    ///
    /// # Arguments
    ///
    /// * `codepoints` - Slice of Unicode codepoints
    /// * `direction` - Hint for paragraph direction
    pub fn resolve_codepoints(
        &self,
        codepoints: &[char],
        direction: ParagraphDirection,
    ) -> BidiResolution {
        if codepoints.is_empty() {
            return BidiResolution {
                base_direction: direction.default_direction(),
                runs: vec![],
                visual_positions: vec![],
                logical_positions: vec![],
            };
        }

        // Fast path: check if all characters are LTR
        if self.is_all_ltr_codepoints(codepoints) && !matches!(direction, ParagraphDirection::Rtl) {
            let len = codepoints.len();
            return BidiResolution {
                base_direction: Direction::LeftToRight,
                runs: vec![BidiRun {
                    direction: Direction::LeftToRight,
                    level: 0,
                    range: 0..len,
                }],
                visual_positions: (0..len).collect(),
                logical_positions: (0..len).collect(),
            };
        }

        // Convert to string for unicode-bidi processing
        let text: String = codepoints.iter().collect();
        self.resolve_line(&text, direction)
    }

    /// Check if all characters have LTR or neutral bidi class
    fn is_all_ltr(&self, text: &str) -> bool {
        text.chars().all(|c| {
            let class = bidi_class(c);
            matches!(
                class,
                BidiClass::L
                    | BidiClass::EN
                    | BidiClass::ES
                    | BidiClass::ET
                    | BidiClass::CS
                    | BidiClass::NSM
                    | BidiClass::BN
                    | BidiClass::B
                    | BidiClass::S
                    | BidiClass::WS
                    | BidiClass::ON
            )
        })
    }

    /// Check if all codepoints have LTR or neutral bidi class
    fn is_all_ltr_codepoints(&self, codepoints: &[char]) -> bool {
        codepoints.iter().all(|&c| {
            let class = bidi_class(c);
            matches!(
                class,
                BidiClass::L
                    | BidiClass::EN
                    | BidiClass::ES
                    | BidiClass::ET
                    | BidiClass::CS
                    | BidiClass::NSM
                    | BidiClass::BN
                    | BidiClass::B
                    | BidiClass::S
                    | BidiClass::WS
                    | BidiClass::ON
            )
        })
    }

    /// Reorder non-spacing marks according to rule L3
    ///
    /// In terminal contexts, NSM should stay with their base character
    /// after reordering, adjusting their visual position.
    fn reorder_nsm_marks(
        &self,
        text: &str,
        visual_positions: &mut [usize],
        logical_positions: &mut [usize],
    ) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();

        for i in 1..len {
            let c = chars[i];
            if bidi_class(c) == BidiClass::NSM {
                // NSM should follow its base character in visual order
                let base_visual = visual_positions[i - 1];
                let nsm_visual = visual_positions[i];

                // If base is to the right of NSM visually, swap them
                if base_visual > nsm_visual {
                    // NSM should be immediately after base
                    visual_positions[i] = base_visual + 1;

                    // Update logical mapping
                    if nsm_visual < logical_positions.len() {
                        logical_positions[nsm_visual] = i;
                    }
                    if base_visual + 1 < logical_positions.len() {
                        logical_positions[base_visual + 1] = i;
                    }
                }
            }
        }
    }
}

/// Get the BiDi class of a character
///
/// This is a convenience wrapper around `unicode_bidi::bidi_class`.
pub fn char_bidi_class(c: char) -> CharBidiClass {
    match bidi_class(c) {
        BidiClass::L => CharBidiClass::LeftToRight,
        BidiClass::R => CharBidiClass::RightToLeft,
        BidiClass::AL => CharBidiClass::ArabicLetter,
        BidiClass::EN => CharBidiClass::EuropeanNumber,
        BidiClass::ES => CharBidiClass::EuropeanSeparator,
        BidiClass::ET => CharBidiClass::EuropeanTerminator,
        BidiClass::AN => CharBidiClass::ArabicNumber,
        BidiClass::CS => CharBidiClass::CommonSeparator,
        BidiClass::NSM => CharBidiClass::NonSpacingMark,
        BidiClass::BN => CharBidiClass::BoundaryNeutral,
        BidiClass::B => CharBidiClass::ParagraphSeparator,
        BidiClass::S => CharBidiClass::SegmentSeparator,
        BidiClass::WS => CharBidiClass::Whitespace,
        BidiClass::ON => CharBidiClass::OtherNeutral,
        BidiClass::LRE => CharBidiClass::LeftToRightEmbedding,
        BidiClass::LRO => CharBidiClass::LeftToRightOverride,
        BidiClass::RLE => CharBidiClass::RightToLeftEmbedding,
        BidiClass::RLO => CharBidiClass::RightToLeftOverride,
        BidiClass::PDF => CharBidiClass::PopDirectionalFormat,
        BidiClass::LRI => CharBidiClass::LeftToRightIsolate,
        BidiClass::RLI => CharBidiClass::RightToLeftIsolate,
        BidiClass::FSI => CharBidiClass::FirstStrongIsolate,
        BidiClass::PDI => CharBidiClass::PopDirectionalIsolate,
    }
}

/// Character BiDi class (simplified)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharBidiClass {
    /// Left-to-Right (L)
    LeftToRight,
    /// Right-to-Left (R)
    RightToLeft,
    /// Arabic Letter (AL)
    ArabicLetter,
    /// European Number (EN)
    EuropeanNumber,
    /// European Number Separator (ES)
    EuropeanSeparator,
    /// European Number Terminator (ET)
    EuropeanTerminator,
    /// Arabic Number (AN)
    ArabicNumber,
    /// Common Number Separator (CS)
    CommonSeparator,
    /// Non-Spacing Mark (NSM)
    NonSpacingMark,
    /// Boundary Neutral (BN)
    BoundaryNeutral,
    /// Paragraph Separator (B)
    ParagraphSeparator,
    /// Segment Separator (S)
    SegmentSeparator,
    /// Whitespace (WS)
    Whitespace,
    /// Other Neutral (ON)
    OtherNeutral,
    /// Left-to-Right Embedding (LRE)
    LeftToRightEmbedding,
    /// Left-to-Right Override (LRO)
    LeftToRightOverride,
    /// Right-to-Left Embedding (RLE)
    RightToLeftEmbedding,
    /// Right-to-Left Override (RLO)
    RightToLeftOverride,
    /// Pop Directional Format (PDF)
    PopDirectionalFormat,
    /// Left-to-Right Isolate (LRI)
    LeftToRightIsolate,
    /// Right-to-Left Isolate (RLI)
    RightToLeftIsolate,
    /// First Strong Isolate (FSI)
    FirstStrongIsolate,
    /// Pop Directional Isolate (PDI)
    PopDirectionalIsolate,
}

impl CharBidiClass {
    /// Returns true if this is a strong RTL class (R or AL)
    pub fn is_rtl(self) -> bool {
        matches!(
            self,
            CharBidiClass::RightToLeft | CharBidiClass::ArabicLetter
        )
    }

    /// Returns true if this is a strong LTR class (L)
    pub fn is_ltr(self) -> bool {
        matches!(self, CharBidiClass::LeftToRight)
    }

    /// Returns true if this is a strong class (L, R, or AL)
    pub fn is_strong(self) -> bool {
        self.is_ltr() || self.is_rtl()
    }

    /// Returns true if this is a neutral class
    pub fn is_neutral(self) -> bool {
        matches!(
            self,
            CharBidiClass::BoundaryNeutral
                | CharBidiClass::SegmentSeparator
                | CharBidiClass::Whitespace
                | CharBidiClass::OtherNeutral
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text() {
        let resolver = BidiResolver::new();
        let result = resolver.resolve_line("", ParagraphDirection::Auto);
        assert!(result.is_empty());
        assert_eq!(result.base_direction, Direction::LeftToRight);
    }

    #[test]
    fn test_pure_ltr() {
        let resolver = BidiResolver::new();
        let result = resolver.resolve_line("Hello World", ParagraphDirection::Auto);

        assert!(result.is_pure_ltr());
        assert!(!result.has_rtl());
        assert_eq!(result.len(), 11);
        assert_eq!(result.visual_runs().len(), 1);
        assert_eq!(result.visual_runs()[0].direction, Direction::LeftToRight);

        // Visual order == logical order for pure LTR
        for i in 0..11 {
            assert_eq!(result.logical_to_visual(i), Some(i));
            assert_eq!(result.visual_to_logical(i), Some(i));
        }
    }

    #[test]
    fn test_pure_rtl() {
        let resolver = BidiResolver::new();
        // Hebrew: "שלום" (shalom)
        let result = resolver.resolve_line("שלום", ParagraphDirection::Auto);

        assert!(result.has_rtl());
        assert!(!result.is_pure_ltr());
        assert_eq!(result.base_direction, Direction::RightToLeft);
        assert_eq!(result.len(), 4);

        // All characters should be RTL
        for run in result.visual_runs() {
            assert_eq!(run.direction, Direction::RightToLeft);
        }
    }

    #[test]
    fn test_mixed_ltr_rtl() {
        let resolver = BidiResolver::new();
        // "Hello שלום World"
        let result = resolver.resolve_line("Hello שלום World", ParagraphDirection::Auto);

        assert!(result.has_rtl());
        assert!(!result.is_pure_ltr());
        assert_eq!(result.base_direction, Direction::LeftToRight);

        // Should have multiple runs
        let runs = result.visual_runs();
        assert!(
            runs.len() >= 2,
            "Expected multiple runs, got {}",
            runs.len()
        );

        // Check that we have both directions
        let has_ltr = runs.iter().any(|r| r.direction == Direction::LeftToRight);
        let has_rtl = runs.iter().any(|r| r.direction == Direction::RightToLeft);
        assert!(has_ltr, "Should have LTR runs");
        assert!(has_rtl, "Should have RTL runs");
    }

    #[test]
    fn test_forced_rtl_direction() {
        let resolver = BidiResolver::new();
        let result = resolver.resolve_line("Hello", ParagraphDirection::Rtl);

        // Even though text is LTR, base direction is forced RTL
        assert_eq!(result.base_direction, Direction::RightToLeft);
    }

    #[test]
    fn test_arabic_text() {
        let resolver = BidiResolver::new();
        // Arabic: "مرحبا" (marhaba - hello)
        let result = resolver.resolve_line("مرحبا", ParagraphDirection::Auto);

        assert!(result.has_rtl());
        assert_eq!(result.base_direction, Direction::RightToLeft);
    }

    #[test]
    fn test_bidi_run_visual_indices() {
        let run = BidiRun {
            direction: Direction::LeftToRight,
            level: 0,
            range: 0..5,
        };

        let indices: Vec<_> = run.visual_indices().collect();
        assert_eq!(indices, vec![0, 1, 2, 3, 4]);

        let rtl_run = BidiRun {
            direction: Direction::RightToLeft,
            level: 1,
            range: 0..5,
        };

        let rtl_indices: Vec<_> = rtl_run.visual_indices().collect();
        assert_eq!(rtl_indices, vec![4, 3, 2, 1, 0]);
    }

    #[test]
    fn test_resolve_codepoints() {
        let resolver = BidiResolver::new();
        let codepoints: Vec<char> = "Hello".chars().collect();
        let result = resolver.resolve_codepoints(&codepoints, ParagraphDirection::Auto);

        assert!(result.is_pure_ltr());
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_char_bidi_class() {
        assert!(char_bidi_class('A').is_ltr());
        assert!(char_bidi_class('A').is_strong());

        assert!(char_bidi_class('א').is_rtl()); // Hebrew Aleph
        assert!(char_bidi_class('א').is_strong());

        assert!(char_bidi_class('ا').is_rtl()); // Arabic Alif
        assert!(char_bidi_class('ا').is_strong());

        assert!(char_bidi_class(' ').is_neutral());
        assert!(!char_bidi_class('1').is_strong()); // Numbers are weak
    }

    #[test]
    fn test_direction_opposite() {
        assert_eq!(Direction::LeftToRight.opposite(), Direction::RightToLeft);
        assert_eq!(Direction::RightToLeft.opposite(), Direction::LeftToRight);
    }

    #[test]
    fn test_numbers_in_rtl_context() {
        let resolver = BidiResolver::new();
        // Hebrew text with numbers: "מספר 123"
        let result = resolver.resolve_line("מספר 123", ParagraphDirection::Auto);

        assert!(result.has_rtl());
        assert_eq!(result.base_direction, Direction::RightToLeft);

        // The number should be in LTR order within the RTL context
        let runs = result.visual_runs();
        assert!(runs.len() >= 2);
    }

    #[test]
    fn test_explicit_lre_control() {
        let resolver = BidiResolver::new();
        // LRE (U+202A) followed by text and PDF (U+202C)
        let text = "\u{202A}test\u{202C}";
        let result = resolver.resolve_line(text, ParagraphDirection::Auto);

        // Should process without crashing
        assert!(!result.is_empty());
    }

    #[test]
    fn test_nsm_with_base() {
        let resolver = BidiResolver::with_nsm_reordering();
        // Hebrew letter with combining mark
        let text = "אָ"; // Aleph with Qamats
        let result = resolver.resolve_line(text, ParagraphDirection::Auto);

        assert!(result.has_rtl());
        // NSM should stay with its base
        assert_eq!(result.len(), 2);
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    #[kani::unwind(10)]
    fn direction_opposite_involution() {
        let dir: Direction = if kani::any() {
            Direction::LeftToRight
        } else {
            Direction::RightToLeft
        };

        // opposite(opposite(x)) == x
        assert_eq!(dir.opposite().opposite(), dir);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn level_direction_consistent() {
        let level: u8 = kani::any();
        kani::assume(level <= 125); // MAX_DEPTH

        let dir = if level % 2 == 1 {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        };

        // Verify our logic matches
        let expected = Direction::from_level(Level::new(level).unwrap_or_else(|_| Level::ltr()));
        assert_eq!(dir, expected);
    }

    #[kani::proof]
    #[kani::unwind(52)] // Increased for meaningful bounds
    fn bidi_run_visual_indices_length() {
        let start: usize = kani::any();
        let len: usize = kani::any();

        kani::assume(start < 100);
        kani::assume(len > 0 && len <= 50); // Realistic line length
        kani::assume(start.checked_add(len).is_some());

        let run = BidiRun {
            direction: if kani::any() {
                Direction::LeftToRight
            } else {
                Direction::RightToLeft
            },
            level: if kani::any() { 0 } else { 1 },
            range: start..start + len,
        };

        let indices: Vec<_> = run.visual_indices().collect();
        assert_eq!(indices.len(), len);
    }

    #[kani::proof]
    #[kani::unwind(32)] // Increased for meaningful bounds
    fn bidi_run_visual_indices_contains_all() {
        let start: usize = kani::any();
        let len: usize = kani::any();

        kani::assume(start < 50);
        kani::assume(len > 0 && len <= 30); // Realistic word length
        kani::assume(start.checked_add(len).is_some());

        let run = BidiRun {
            direction: if kani::any() {
                Direction::LeftToRight
            } else {
                Direction::RightToLeft
            },
            level: 0,
            range: start..start + len,
        };

        let mut count = 0;
        for (offset, idx) in run.visual_indices().enumerate() {
            let expected = if run.direction == Direction::RightToLeft {
                start + len - 1 - offset
            } else {
                start + offset
            };
            assert_eq!(idx, expected);
            count += 1;
        }
        assert_eq!(count, len);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn char_bidi_class_strong_classification() {
        // Test that is_strong correctly identifies L, R, and AL
        let class: CharBidiClass = match kani::any::<u8>() % 5 {
            0 => CharBidiClass::LeftToRight,
            1 => CharBidiClass::RightToLeft,
            2 => CharBidiClass::ArabicLetter,
            3 => CharBidiClass::EuropeanNumber,
            _ => CharBidiClass::Whitespace,
        };

        let is_strong = class.is_strong();
        let expected_strong = matches!(
            class,
            CharBidiClass::LeftToRight | CharBidiClass::RightToLeft | CharBidiClass::ArabicLetter
        );

        assert_eq!(is_strong, expected_strong);
    }
}
