//! Packed cell representation (8 bytes).
//!
//! ## Design
//!
//! Extreme compression cell - 8 bytes total (vs previous 12 bytes).
//!
//! Memory savings: 33% reduction
//! For 10,000 lines x 200 cols = 2M cells:
//!   Before: 24 MB
//!   After:  16 MB
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │ char_data (2 bytes)                                            │
//! │   - UTF-16 code unit for BMP (U+0000-U+FFFF)                   │
//! │   - Overflow table index when flags.COMPLEX is set             │
//! ├────────────────────────────────────────────────────────────────┤
//! │ colors (4 bytes) - Packed foreground and background            │
//! │   - Bits 0-7:   FG color index (0-255) or FG mode indicator    │
//! │   - Bits 8-15:  BG color index (0-255) or BG mode indicator    │
//! │   - Bits 16-23: Extra color data / overflow indicator          │
//! │   - Bits 24-31: Color mode flags                               │
//! ├────────────────────────────────────────────────────────────────┤
//! │ flags (2 bytes) - Cell attributes                              │
//! │   - Bits 0-14: Standard attributes                             │
//! │   - Bit 15: COMPLEX flag (char_data is overflow index)         │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Overflow Tables (in CellExtra)
//!
//! When a cell needs more than 8 bytes can express:
//! - Complex characters (emoji, combining marks, non-BMP): string table
//! - True color RGB: separate fg/bg overflow tables
//! - Hyperlinks: URL storage
//! - Underline color: separate color storage
//!
//! Expected: <1% of cells need overflow.
//!
//! ## Verification
//!
//! - Kani proof: `cell_pack_unpack_roundtrip`
//! - Compile-time assert: `size_of::<Cell>() == 8`

use super::style::StyleId;

/// Packed color representation for 8-byte cells.
///
/// Encodes both foreground and background in 4 bytes.
///
/// ## Color Modes (bits 24-27 for FG, bits 28-31 for BG)
/// - 0x0: Default color
/// - 0x1: Indexed color (index in low bits)
/// - 0x2: RGB color (lookup in overflow table)
///
/// ## Indexed Colors (when mode = 0x1)
/// - FG index in bits 0-7
/// - BG index in bits 8-15
///
/// ## RGB Colors (when mode = 0x2)
/// Colors are stored in CellExtra overflow tables, keyed by (row, col).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct PackedColors(pub u32);

impl PackedColors {
    // Color mode flags (4 bits each for FG and BG)
    const FG_MODE_SHIFT: u32 = 24;
    const BG_MODE_SHIFT: u32 = 28;
    const MODE_MASK: u32 = 0x0F;

    const MODE_DEFAULT: u32 = 0;
    const MODE_INDEXED: u32 = 1;
    const MODE_RGB: u32 = 2;

    /// Both colors are default.
    pub const DEFAULT: Self = Self(0);

    /// Create with default foreground and background.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self::DEFAULT
    }

    /// Create with indexed foreground and default background.
    #[must_use]
    #[inline]
    pub const fn with_indexed_fg(fg_index: u8) -> Self {
        Self((Self::MODE_INDEXED << Self::FG_MODE_SHIFT) | (fg_index as u32))
    }

    /// Create with indexed background and default foreground.
    #[must_use]
    #[inline]
    pub const fn with_indexed_bg(bg_index: u8) -> Self {
        Self((Self::MODE_INDEXED << Self::BG_MODE_SHIFT) | ((bg_index as u32) << 8))
    }

    /// Create with both indexed colors.
    #[must_use]
    #[inline]
    pub const fn with_indexed(fg_index: u8, bg_index: u8) -> Self {
        Self(
            (Self::MODE_INDEXED << Self::FG_MODE_SHIFT)
                | (Self::MODE_INDEXED << Self::BG_MODE_SHIFT)
                | (fg_index as u32)
                | ((bg_index as u32) << 8),
        )
    }

    /// Mark foreground as RGB (actual color in overflow table).
    #[must_use]
    #[inline]
    pub const fn with_rgb_fg(self) -> Self {
        Self(
            (self.0 & !(Self::MODE_MASK << Self::FG_MODE_SHIFT))
                | (Self::MODE_RGB << Self::FG_MODE_SHIFT),
        )
    }

    /// Mark background as RGB (actual color in overflow table).
    #[must_use]
    #[inline]
    pub const fn with_rgb_bg(self) -> Self {
        Self(
            (self.0 & !(Self::MODE_MASK << Self::BG_MODE_SHIFT))
                | (Self::MODE_RGB << Self::BG_MODE_SHIFT),
        )
    }

    /// Get foreground color mode.
    #[must_use]
    #[inline]
    pub const fn fg_mode(&self) -> u32 {
        (self.0 >> Self::FG_MODE_SHIFT) & Self::MODE_MASK
    }

    /// Get background color mode.
    #[must_use]
    #[inline]
    pub const fn bg_mode(&self) -> u32 {
        (self.0 >> Self::BG_MODE_SHIFT) & Self::MODE_MASK
    }

    /// Check if foreground is default.
    #[must_use]
    #[inline]
    pub const fn fg_is_default(&self) -> bool {
        self.fg_mode() == Self::MODE_DEFAULT
    }

    /// Check if background is default.
    #[must_use]
    #[inline]
    pub const fn bg_is_default(&self) -> bool {
        self.bg_mode() == Self::MODE_DEFAULT
    }

    /// Check if foreground is indexed.
    #[must_use]
    #[inline]
    pub const fn fg_is_indexed(&self) -> bool {
        self.fg_mode() == Self::MODE_INDEXED
    }

    /// Check if background is indexed.
    #[must_use]
    #[inline]
    pub const fn bg_is_indexed(&self) -> bool {
        self.bg_mode() == Self::MODE_INDEXED
    }

    /// Check if foreground is RGB (needs overflow lookup).
    #[must_use]
    #[inline]
    pub const fn fg_is_rgb(&self) -> bool {
        self.fg_mode() == Self::MODE_RGB
    }

    /// Check if background is RGB (needs overflow lookup).
    #[must_use]
    #[inline]
    pub const fn bg_is_rgb(&self) -> bool {
        self.bg_mode() == Self::MODE_RGB
    }

    /// Get foreground indexed color (only valid if `fg_is_indexed()`).
    #[must_use]
    #[inline]
    pub const fn fg_index(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Get background indexed color (only valid if `bg_is_indexed()`).
    #[must_use]
    #[inline]
    pub const fn bg_index(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    /// Set foreground to indexed color.
    #[must_use]
    #[inline]
    pub const fn set_fg_indexed(self, index: u8) -> Self {
        Self(
            (self.0 & !0xFF & !(Self::MODE_MASK << Self::FG_MODE_SHIFT))
                | (index as u32)
                | (Self::MODE_INDEXED << Self::FG_MODE_SHIFT),
        )
    }

    /// Set background to indexed color.
    #[must_use]
    #[inline]
    pub const fn set_bg_indexed(self, index: u8) -> Self {
        Self(
            (self.0 & !0xFF00 & !(Self::MODE_MASK << Self::BG_MODE_SHIFT))
                | ((index as u32) << 8)
                | (Self::MODE_INDEXED << Self::BG_MODE_SHIFT),
        )
    }

    /// Set foreground to default.
    #[must_use]
    #[inline]
    pub const fn set_fg_default(self) -> Self {
        Self(self.0 & !(Self::MODE_MASK << Self::FG_MODE_SHIFT))
    }

    /// Set background to default.
    #[must_use]
    #[inline]
    pub const fn set_bg_default(self) -> Self {
        Self(self.0 & !(Self::MODE_MASK << Self::BG_MODE_SHIFT))
    }

    /// Check if both colors are default.
    #[must_use]
    #[inline]
    pub const fn is_default(&self) -> bool {
        self.fg_is_default() && self.bg_is_default()
    }
}

/// Legacy PackedColor for compatibility during transition.
///
/// Format: `0xTT_RRGGBB` where TT is the type:
/// - `0x00_INDEX__`: Indexed color (0-255)
/// - `0x01_RRGGBB`: True color RGB
/// - `0xFF_______`: Default color
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct PackedColor(pub u32);

impl PackedColor {
    /// Default foreground color.
    pub const DEFAULT_FG: Self = Self(0xFF_FFFFFF);

    /// Default background color.
    pub const DEFAULT_BG: Self = Self(0xFF_000000);

    /// Create an indexed color (0-255).
    #[must_use]
    #[inline]
    pub const fn indexed(index: u8) -> Self {
        Self(index as u32)
    }

    /// Create a true color from RGB values.
    #[must_use]
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(0x01_000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    /// Check if this is the default color.
    #[must_use]
    #[inline]
    pub const fn is_default(&self) -> bool {
        (self.0 >> 24) == 0xFF
    }

    /// Check if this is an indexed color.
    #[must_use]
    #[inline]
    pub const fn is_indexed(&self) -> bool {
        (self.0 >> 24) == 0x00
    }

    /// Check if this is a true color.
    #[must_use]
    #[inline]
    pub const fn is_rgb(&self) -> bool {
        (self.0 >> 24) == 0x01
    }

    /// Get the indexed color value (only valid if `is_indexed()`).
    #[must_use]
    #[inline]
    pub const fn index(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Get RGB components (only valid if `is_rgb()`).
    #[must_use]
    #[inline]
    pub const fn rgb_components(&self) -> (u8, u8, u8) {
        let r = ((self.0 >> 16) & 0xFF) as u8;
        let g = ((self.0 >> 8) & 0xFF) as u8;
        let b = (self.0 & 0xFF) as u8;
        (r, g, b)
    }

    /// Create the default foreground color.
    #[must_use]
    #[inline]
    pub const fn default_fg() -> Self {
        Self::DEFAULT_FG
    }

    /// Create the default background color.
    #[must_use]
    #[inline]
    pub const fn default_bg() -> Self {
        Self::DEFAULT_BG
    }
}

/// Cell flags packed into the Cell's flags field.
///
/// The Cell struct stores flags in 16 bits.
///
/// ## Bit allocation
/// - Bits 0-7: Visual attributes (bold, dim, italic, underline, blink, inverse, hidden, strikethrough)
/// - Bit 8: Double underline
/// - Bit 9: Wide character
/// - Bit 10: Wide continuation / Protected (shared bit - mutually exclusive)
/// - Bit 11: Superscript (SGR 73)
/// - Bit 12: Subscript (SGR 74)
/// - Bit 13: Curly underline
/// - Bit 14: Reserved
/// - Bit 15: COMPLEX (char_data is overflow table index)
///
/// Note: WIDE_CONTINUATION and PROTECTED share the same bit. Wide continuation
/// cells (spacers after wide characters) cannot be protected independently;
/// protection applies to the main wide character cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct CellFlags(u16);

impl CellFlags {
    /// Bold text.
    pub const BOLD: Self = Self(1 << 0);
    /// Dim/faint text.
    pub const DIM: Self = Self(1 << 1);
    /// Italic text.
    pub const ITALIC: Self = Self(1 << 2);
    /// Underlined text.
    pub const UNDERLINE: Self = Self(1 << 3);
    /// Blinking text.
    pub const BLINK: Self = Self(1 << 4);
    /// Inverse video.
    pub const INVERSE: Self = Self(1 << 5);
    /// Hidden/invisible text.
    pub const HIDDEN: Self = Self(1 << 6);
    /// Strikethrough text.
    pub const STRIKETHROUGH: Self = Self(1 << 7);
    /// Double underline.
    pub const DOUBLE_UNDERLINE: Self = Self(1 << 8);
    /// Wide character (occupies 2 cells).
    pub const WIDE: Self = Self(1 << 9);
    /// Wide character continuation (spacer cell).
    /// Shares bit with PROTECTED - mutually exclusive.
    pub const WIDE_CONTINUATION: Self = Self(1 << 10);
    /// Protected from selective erase (DECSCA).
    /// Shares bit with WIDE_CONTINUATION - mutually exclusive.
    /// A non-wide cell uses this for protection status.
    pub const PROTECTED: Self = Self(1 << 10);
    /// Superscript text (SGR 73).
    pub const SUPERSCRIPT: Self = Self(1 << 11);
    /// Subscript text (SGR 74).
    pub const SUBSCRIPT: Self = Self(1 << 12);
    /// Curly underline.
    pub const CURLY_UNDERLINE: Self = Self(1 << 13);

    // Underline style encoding for cells:
    // - UNDERLINE alone = single underline
    // - DOUBLE_UNDERLINE alone = double underline
    // - CURLY_UNDERLINE alone = curly underline
    // - UNDERLINE + CURLY_UNDERLINE = dotted underline (SGR 4:4)
    // - DOUBLE_UNDERLINE + CURLY_UNDERLINE = dashed underline (SGR 4:5)
    // These combinations use bitwise OR of existing flags to encode additional styles.

    /// Dotted underline (SGR 4:4) - encoded as UNDERLINE | CURLY_UNDERLINE.
    pub const DOTTED_UNDERLINE: Self = Self((1 << 3) | (1 << 13)); // UNDERLINE | CURLY_UNDERLINE
    /// Dashed underline (SGR 4:5) - encoded as DOUBLE_UNDERLINE | CURLY_UNDERLINE.
    pub const DASHED_UNDERLINE: Self = Self((1 << 8) | (1 << 13)); // DOUBLE_UNDERLINE | CURLY_UNDERLINE

    /// Cell uses StyleId instead of inline colors.
    /// When set, the colors field stores a StyleId in its low 16 bits.
    pub const USES_STYLE_ID: Self = Self(1 << 14);
    /// Complex character - char_data is an index into the overflow string table.
    pub const COMPLEX: Self = Self(1 << 15);

    /// Empty flags.
    #[must_use]
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Check if flag is set.
    #[must_use]
    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Set a flag.
    #[must_use]
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Clear a flag.
    #[must_use]
    #[inline]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Insert a flag (mutating).
    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Remove a flag (mutating).
    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Check if flags are empty.
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Get raw bits.
    #[must_use]
    #[inline]
    pub const fn bits(&self) -> u16 {
        self.0
    }

    /// Create from raw bits.
    #[must_use]
    #[inline]
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Mask for core visual flags (bits 0-13).
    pub const VISUAL_FLAGS_MASK: u16 = 0x3FFF;

    /// Check if this has the COMPLEX flag set.
    #[must_use]
    #[inline]
    pub const fn is_complex(&self) -> bool {
        (self.0 & Self::COMPLEX.0) != 0
    }

    /// Check if this cell uses StyleId instead of inline colors.
    #[must_use]
    #[inline]
    pub const fn uses_style_id(&self) -> bool {
        (self.0 & Self::USES_STYLE_ID.0) != 0
    }

    /// Get only the core flags (excluding COMPLEX).
    #[must_use]
    #[inline]
    pub const fn core_flags(&self) -> Self {
        Self(self.0 & Self::VISUAL_FLAGS_MASK)
    }

    /// Mask for extended flags (bits 11-13) that were previously in CellExtra.
    /// These are now stored directly in Cell.
    pub const EXTENDED_FLAGS_MASK: u16 = 0x3800; // bits 11-13

    /// Get only the extended flags (bits 11-13).
    #[must_use]
    #[inline]
    pub const fn extended_flags(&self) -> Self {
        Self(self.0 & Self::EXTENDED_FLAGS_MASK)
    }

    /// Check if this has any extended flags set.
    #[must_use]
    #[inline]
    pub const fn has_extended_flags(&self) -> bool {
        (self.0 & Self::EXTENDED_FLAGS_MASK) != 0
    }
}

/// A single terminal cell (8 bytes).
///
/// ## Memory layout
///
/// ```text
/// Offset  Size  Field
/// 0       2     char_data (UTF-16 code unit or overflow index)
/// 2       4     colors (packed fg/bg with mode bits)
/// 6       2     flags (attributes + COMPLEX flag)
/// ```
///
/// ## Complex Characters
///
/// When `flags.contains(CellFlags::COMPLEX)`:
/// - `char_data` is an index into the overflow string table in CellExtras
/// - Used for: emoji, combining marks, non-BMP characters, grapheme clusters
///
/// ## RGB Colors
///
/// When `colors.fg_is_rgb()` or `colors.bg_is_rgb()`:
/// - RGB values are stored in CellExtras overflow tables
/// - Keyed by (row, col) position
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct Cell {
    /// Character data.
    /// - BMP characters (U+0000-U+FFFF): UTF-16 code unit directly
    /// - Complex/non-BMP: overflow table index (when COMPLEX flag set)
    char_data: u16,
    /// Packed foreground and background colors.
    colors: PackedColors,
    /// Cell flags including COMPLEX indicator.
    flags: CellFlags,
}

// Compile-time size check - MUST be exactly 8 bytes
const _: () = assert!(std::mem::size_of::<Cell>() == 8);

impl Default for Cell {
    #[inline]
    fn default() -> Self {
        Self::EMPTY
    }
}

impl std::fmt::Debug for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Copy packed fields to avoid unaligned reference errors
        let char_data = self.char_data;
        let flags = self.flags;
        let colors = self.colors;
        if flags.is_complex() {
            f.debug_struct("Cell")
                .field("overflow_index", &char_data)
                .field("complex", &true)
                .field("flags", &flags)
                .field("colors", &colors)
                .finish()
        } else {
            let ch = char::from_u32(u32::from(char_data)).unwrap_or('\u{FFFD}');
            f.debug_struct("Cell")
                .field("char", &ch)
                .field("flags", &flags)
                .field("colors", &colors)
                .finish()
        }
    }
}

impl Cell {
    /// Maximum codepoint that fits directly in char_data (BMP).
    pub const MAX_DIRECT_CODEPOINT: u32 = 0xFFFF;

    /// Empty cell (space with default colors).
    pub const EMPTY: Self = Self {
        char_data: ' ' as u16,
        colors: PackedColors::DEFAULT,
        flags: CellFlags::empty(),
    };

    /// FAST PATH: Create a cell from an ASCII byte with zero overhead.
    ///
    /// # Preconditions (caller must verify)
    /// - `byte` is printable ASCII (0x20..=0x7E)
    ///
    /// This is the hot path for ASCII output. It creates a Cell with:
    /// - char_data = byte value (fits in u16)
    /// - colors = default (0)
    /// - flags = none (0)
    ///
    /// # Performance
    /// This is a single struct construction with no branches or checks.
    #[must_use]
    #[inline]
    pub const fn from_ascii_fast(byte: u8) -> Self {
        Self {
            char_data: byte as u16,
            colors: PackedColors::DEFAULT,
            flags: CellFlags::empty(),
        }
    }

    /// FAST PATH: Create a styled cell from an ASCII byte.
    ///
    /// # Preconditions (caller must verify)
    /// - `byte` is printable ASCII (0x20..=0x7E)
    /// - `colors` is already packed (no RGB overflow needed)
    ///
    /// This creates a Cell directly without char translation or width checks,
    /// ideal for bulk ASCII writes with a known style.
    #[must_use]
    #[inline]
    pub const fn from_ascii_styled(byte: u8, colors: PackedColors, flags: CellFlags) -> Self {
        Self {
            char_data: byte as u16,
            colors,
            flags,
        }
    }

    /// Create a new cell from a character.
    ///
    /// For BMP characters (U+0000-U+FFFF), stores directly.
    /// For non-BMP characters, caller should use overflow mechanism.
    #[must_use]
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // Intentional: cp is verified <= 0xFFFF
    pub const fn new(c: char) -> Self {
        let cp = c as u32;
        if cp <= Self::MAX_DIRECT_CODEPOINT {
            Self {
                char_data: cp as u16,
                colors: PackedColors::DEFAULT,
                flags: CellFlags::empty(),
            }
        } else {
            // Non-BMP character - store replacement char, caller should use overflow
            Self {
                char_data: '\u{FFFD}' as u16,
                colors: PackedColors::DEFAULT,
                flags: CellFlags::empty(),
            }
        }
    }

    /// Create a new cell with colors and flags.
    ///
    /// Note: For RGB colors, the colors should be set up to indicate RGB mode,
    /// and actual RGB values stored in CellExtras overflow.
    #[must_use]
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // Intentional: cp is verified <= 0xFFFF
    pub const fn with_style(c: char, fg: PackedColor, bg: PackedColor, flags: CellFlags) -> Self {
        let cp = c as u32;
        let char_data = if cp <= Self::MAX_DIRECT_CODEPOINT {
            cp as u16
        } else {
            '\u{FFFD}' as u16
        };

        // Convert legacy PackedColor to PackedColors
        let colors = Self::convert_legacy_colors(fg, bg);

        Self {
            char_data,
            colors,
            flags,
        }
    }

    /// Convert PackedColor pair to PackedColors format.
    ///
    /// Public helper for bulk operations that need to pre-compute colors.
    #[must_use]
    #[inline]
    pub const fn convert_colors(fg: PackedColor, bg: PackedColor) -> PackedColors {
        Self::convert_legacy_colors(fg, bg)
    }

    /// Convert legacy PackedColor pair to new PackedColors format.
    #[inline]
    const fn convert_legacy_colors(fg: PackedColor, bg: PackedColor) -> PackedColors {
        let mut colors = PackedColors::DEFAULT;

        // Handle foreground
        if fg.is_indexed() {
            colors = colors.set_fg_indexed(fg.index());
        } else if fg.is_rgb() {
            // RGB needs overflow - mark as RGB mode
            colors = colors.with_rgb_fg();
        }
        // else: default

        // Handle background
        if bg.is_indexed() {
            colors = colors.set_bg_indexed(bg.index());
        } else if bg.is_rgb() {
            // RGB needs overflow - mark as RGB mode
            colors = colors.with_rgb_bg();
        }
        // else: default

        colors
    }

    /// Create a cell from a codepoint (not a char, for grapheme references).
    #[must_use]
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // Intentional: codepoint is verified <= 0xFFFF
    pub const fn from_codepoint(codepoint: u32) -> Self {
        if codepoint <= Self::MAX_DIRECT_CODEPOINT {
            Self {
                char_data: codepoint as u16,
                colors: PackedColors::DEFAULT,
                flags: CellFlags::empty(),
            }
        } else {
            Self {
                char_data: '\u{FFFD}' as u16,
                colors: PackedColors::DEFAULT,
                flags: CellFlags::empty(),
            }
        }
    }

    /// Create a cell with overflow index for complex character.
    ///
    /// The actual character string is stored in CellExtras.
    #[must_use]
    #[inline]
    pub const fn with_overflow_index(index: u16) -> Self {
        Self {
            char_data: index,
            colors: PackedColors::DEFAULT,
            flags: CellFlags::COMPLEX,
        }
    }

    /// Create a cell with a StyleId reference instead of inline colors.
    ///
    /// This is the Ghostty-style approach for memory-efficient style storage.
    /// The StyleId references a style in the StyleTable, which stores the
    /// actual colors and attributes.
    ///
    /// The `cell_flags` parameter should contain cell-specific flags only
    /// (WIDE, WIDE_CONTINUATION, PROTECTED). Style attributes (BOLD, ITALIC,
    /// etc.) are stored in the StyleTable and will be retrieved at render time.
    ///
    /// # Memory Layout
    ///
    /// When using StyleId:
    /// - `colors.0` low 16 bits: StyleId value
    /// - `colors.0` high 16 bits: reserved (for RGB overflow index)
    /// - `flags`: has USES_STYLE_ID set, plus cell-specific flags
    #[must_use]
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // Intentional: cp is verified <= 0xFFFF
    pub const fn with_style_id(c: char, style_id: StyleId, cell_flags: CellFlags) -> Self {
        let cp = c as u32;
        let char_data = if cp <= Self::MAX_DIRECT_CODEPOINT {
            cp as u16
        } else {
            '\u{FFFD}' as u16
        };

        // Store StyleId in the colors field's low 16 bits
        // Set USES_STYLE_ID flag to indicate this cell uses style interning
        let colors = PackedColors(style_id.0 as u32);
        let flags = CellFlags(cell_flags.0 | CellFlags::USES_STYLE_ID.0);

        Self {
            char_data,
            colors,
            flags,
        }
    }

    /// Create a styled cell from an ASCII byte with StyleId.
    ///
    /// # Preconditions (caller must verify)
    /// - `byte` is printable ASCII (0x20..=0x7E)
    ///
    /// This is the hot path for ASCII output with style interning.
    #[must_use]
    #[inline]
    pub const fn from_ascii_with_style_id(
        byte: u8,
        style_id: StyleId,
        cell_flags: CellFlags,
    ) -> Self {
        let colors = PackedColors(style_id.0 as u32);
        let flags = CellFlags(cell_flags.0 | CellFlags::USES_STYLE_ID.0);

        Self {
            char_data: byte as u16,
            colors,
            flags,
        }
    }

    /// Get the raw char_data value.
    ///
    /// If `is_complex()`, this is an overflow table index.
    /// Otherwise, it's a UTF-16 code unit (BMP codepoint).
    #[must_use]
    #[inline]
    pub const fn char_data(&self) -> u16 {
        self.char_data
    }

    /// Check if this cell uses overflow for its character.
    #[must_use]
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub const fn is_complex(&self) -> bool {
        // Copy from packed struct to avoid unaligned access
        let flags = self.flags;
        flags.is_complex()
    }

    /// Get the Unicode codepoint (only valid for non-complex cells).
    ///
    /// For complex cells, use the overflow table with `char_data()` as key.
    #[must_use]
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub const fn codepoint(&self) -> u32 {
        // Copy from packed struct to avoid unaligned access
        let flags = self.flags;
        if flags.is_complex() {
            // Complex cell - return replacement char, caller should use overflow
            0xFFFD
        } else {
            self.char_data as u32
        }
    }

    /// Get the character (may be replacement char if complex or invalid).
    ///
    /// For complex cells with non-BMP characters or grapheme clusters,
    /// use the overflow table.
    #[must_use]
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub fn char(&self) -> char {
        // Copy from packed struct to avoid unaligned access
        let flags = self.flags;
        let char_data = self.char_data;
        if flags.is_complex() {
            '\u{FFFD}' // Complex - use overflow table
        } else {
            char::from_u32(u32::from(char_data)).unwrap_or('\u{FFFD}')
        }
    }

    /// Get the cell flags.
    #[must_use]
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub const fn flags(&self) -> CellFlags {
        self.flags
    }

    /// Get packed colors.
    #[must_use]
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub const fn colors(&self) -> PackedColors {
        self.colors
    }

    /// Get foreground color as legacy PackedColor.
    ///
    /// Note: For RGB colors, this returns a placeholder. Use CellExtras
    /// overflow for actual RGB values.
    #[must_use]
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub const fn fg(&self) -> PackedColor {
        // Copy from packed struct to avoid unaligned access
        let colors = self.colors;
        if colors.fg_is_default() {
            PackedColor::DEFAULT_FG
        } else if colors.fg_is_indexed() {
            PackedColor::indexed(colors.fg_index())
        } else {
            // RGB - return placeholder, actual color in overflow
            PackedColor::rgb(0, 0, 0)
        }
    }

    /// Get background color as legacy PackedColor.
    ///
    /// Note: For RGB colors, this returns a placeholder. Use CellExtras
    /// overflow for actual RGB values.
    #[must_use]
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub const fn bg(&self) -> PackedColor {
        // Copy from packed struct to avoid unaligned access
        let colors = self.colors;
        if colors.bg_is_default() {
            PackedColor::DEFAULT_BG
        } else if colors.bg_is_indexed() {
            PackedColor::indexed(colors.bg_index())
        } else {
            // RGB - return placeholder, actual color in overflow
            PackedColor::rgb(0, 0, 0)
        }
    }

    /// Check if foreground needs overflow lookup for RGB.
    #[must_use]
    #[inline]
    pub const fn fg_needs_overflow(&self) -> bool {
        // Copy from packed struct to avoid unaligned access
        let colors = self.colors;
        colors.fg_is_rgb()
    }

    /// Check if background needs overflow lookup for RGB.
    #[must_use]
    #[inline]
    pub const fn bg_needs_overflow(&self) -> bool {
        // Copy from packed struct to avoid unaligned access
        let colors = self.colors;
        colors.bg_is_rgb()
    }

    /// Check if this cell uses StyleId instead of inline colors.
    ///
    /// When true, use `style_id()` to get the StyleId and look up
    /// colors/attributes from the StyleTable.
    #[must_use]
    #[inline]
    pub const fn uses_style_id(&self) -> bool {
        // Copy from packed struct to avoid unaligned access
        let flags = self.flags;
        flags.uses_style_id()
    }

    /// Get the StyleId (only valid when `uses_style_id()` is true).
    ///
    /// The StyleId is stored in the low 16 bits of the colors field.
    /// Use this to look up the actual style from the StyleTable.
    #[must_use]
    #[inline]
    pub const fn style_id(&self) -> StyleId {
        // Copy from packed struct to avoid unaligned access
        let colors = self.colors;
        #[allow(clippy::cast_possible_truncation)]
        StyleId((colors.0 & 0xFFFF) as u16)
    }

    /// Get the StyleId if this cell uses style interning, otherwise None.
    #[must_use]
    #[inline]
    pub const fn style_id_opt(&self) -> Option<StyleId> {
        if self.uses_style_id() {
            Some(self.style_id())
        } else {
            None
        }
    }

    /// Set the character (BMP only, use overflow for non-BMP).
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // Intentional: cp is verified <= 0xFFFF
    pub fn set_char(&mut self, c: char) {
        let cp = c as u32;
        if cp <= Self::MAX_DIRECT_CODEPOINT {
            self.char_data = cp as u16;
            // Clear COMPLEX flag since this is a direct character
            // Copy-modify-write for packed struct
            let mut flags = self.flags;
            flags.remove(CellFlags::COMPLEX);
            self.flags = flags;
        }
        // For non-BMP, caller should use set_overflow_index
    }

    /// Set the codepoint directly.
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // Intentional: codepoint is verified <= 0xFFFF
    pub fn set_codepoint(&mut self, codepoint: u32) {
        if codepoint <= Self::MAX_DIRECT_CODEPOINT {
            self.char_data = codepoint as u16;
            // Copy-modify-write for packed struct
            let mut flags = self.flags;
            flags.remove(CellFlags::COMPLEX);
            self.flags = flags;
        }
    }

    /// Set overflow index for complex character.
    #[inline]
    pub fn set_overflow_index(&mut self, index: u16) {
        self.char_data = index;
        // Copy-modify-write for packed struct
        let mut flags = self.flags;
        flags.insert(CellFlags::COMPLEX);
        self.flags = flags;
    }

    /// Set the flags.
    #[inline]
    pub fn set_flags(&mut self, flags: CellFlags) {
        self.flags = flags;
    }

    /// Set the foreground color (indexed).
    #[inline]
    pub fn set_fg(&mut self, fg: PackedColor) {
        // Copy from packed struct for modification
        let colors = self.colors;
        if fg.is_default() {
            self.colors = colors.set_fg_default();
        } else if fg.is_indexed() {
            self.colors = colors.set_fg_indexed(fg.index());
        } else {
            // RGB - mark as needing overflow
            self.colors = colors.with_rgb_fg();
        }
    }

    /// Set the background color (indexed).
    #[inline]
    pub fn set_bg(&mut self, bg: PackedColor) {
        // Copy from packed struct for modification
        let colors = self.colors;
        if bg.is_default() {
            self.colors = colors.set_bg_default();
        } else if bg.is_indexed() {
            self.colors = colors.set_bg_indexed(bg.index());
        } else {
            // RGB - mark as needing overflow
            self.colors = colors.with_rgb_bg();
        }
    }

    /// Set the StyleId for this cell.
    ///
    /// This converts the cell to use style interning. The colors field is
    /// repurposed to store the StyleId, and USES_STYLE_ID flag is set.
    ///
    /// Cell-specific flags (WIDE, WIDE_CONTINUATION, COMPLEX, PROTECTED) are preserved.
    #[inline]
    pub fn set_style_id(&mut self, style_id: StyleId) {
        // Store StyleId in colors field
        self.colors = PackedColors(u32::from(style_id.0));
        // Set USES_STYLE_ID flag while preserving cell-specific flags
        let mut flags = self.flags;
        flags.insert(CellFlags::USES_STYLE_ID);
        self.flags = flags;
    }

    /// Clear the StyleId and switch back to inline colors mode.
    ///
    /// This clears the USES_STYLE_ID flag and sets colors to default.
    /// Use this when transitioning a cell from style interning back to inline.
    #[inline]
    pub fn clear_style_id(&mut self) {
        self.colors = PackedColors::DEFAULT;
        let mut flags = self.flags;
        flags.remove(CellFlags::USES_STYLE_ID);
        self.flags = flags;
    }

    /// Check if this is a wide character.
    #[must_use]
    #[inline]
    pub const fn is_wide(&self) -> bool {
        // Copy from packed struct to avoid unaligned access
        let flags = self.flags;
        flags.contains(CellFlags::WIDE)
    }

    /// Check if this is a wide character continuation.
    #[must_use]
    #[inline]
    pub const fn is_wide_continuation(&self) -> bool {
        // Copy from packed struct to avoid unaligned access
        let flags = self.flags;
        flags.contains(CellFlags::WIDE_CONTINUATION)
    }

    /// Check if this cell is protected from selective erase.
    ///
    /// Note: Wide continuation cells share the PROTECTED bit, so this method
    /// checks that the cell is NOT a wide character first. For wide characters,
    /// protection is checked on the main cell, not the continuation.
    #[must_use]
    #[inline]
    pub const fn is_protected(&self) -> bool {
        // Copy from packed struct to avoid unaligned access
        let flags = self.flags;
        flags.contains(CellFlags::PROTECTED) && !flags.contains(CellFlags::WIDE)
    }

    /// Check if this cell is empty (space with default colors and no special flags).
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        // Copy from packed struct to avoid unaligned access
        let colors = self.colors;
        let flags = self.flags;
        self.char_data == ' ' as u16 && colors.is_default() && flags.0 == 0
    }

    /// Clear the cell to empty.
    #[inline]
    pub fn clear(&mut self) {
        *self = Self::EMPTY;
    }

    /// Clear the cell but preserve background color mode and index.
    #[inline]
    pub fn clear_preserve_bg(&mut self) {
        // Copy from packed struct before clearing
        let colors = self.colors;
        let bg_mode = colors.bg_mode();
        let bg_index = colors.bg_index();
        *self = Self::EMPTY;
        if bg_mode == PackedColors::MODE_INDEXED {
            let c = self.colors;
            self.colors = c.set_bg_indexed(bg_index);
        } else if bg_mode == PackedColors::MODE_RGB {
            let c = self.colors;
            self.colors = c.with_rgb_bg();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_size_is_8_bytes() {
        assert_eq!(std::mem::size_of::<Cell>(), 8);
    }

    #[test]
    fn cell_new_bmp() {
        let cell = Cell::new('A');
        assert_eq!(cell.char_data(), 'A' as u16);
        assert!(!cell.is_complex());
        assert_eq!(cell.char(), 'A');
        assert_eq!(cell.codepoint(), 'A' as u32);
    }

    #[test]
    fn cell_new_non_bmp() {
        // Emoji (non-BMP) should trigger complex handling
        let cell = Cell::new('😀');
        // Non-BMP can't be stored directly in 16 bits
        // Cell::new stores replacement char for non-BMP
        assert_eq!(cell.char(), '\u{FFFD}');
    }

    #[test]
    fn cell_cjk() {
        // CJK characters are in BMP
        let cell = Cell::new('あ');
        assert_eq!(cell.char_data(), 'あ' as u16);
        assert!(!cell.is_complex());
        assert_eq!(cell.char(), 'あ');
    }

    #[test]
    fn cell_pack_unpack_flags() {
        let flags = CellFlags::BOLD.union(CellFlags::ITALIC);
        let cell = Cell::with_style('X', PackedColor::DEFAULT_FG, PackedColor::DEFAULT_BG, flags);
        assert!(cell.flags().contains(CellFlags::BOLD));
        assert!(cell.flags().contains(CellFlags::ITALIC));
        assert!(!cell.flags().contains(CellFlags::UNDERLINE));
    }

    #[test]
    fn packed_colors_default() {
        let colors = PackedColors::DEFAULT;
        assert!(colors.fg_is_default());
        assert!(colors.bg_is_default());
        assert!(colors.is_default());
    }

    #[test]
    fn packed_colors_indexed() {
        let colors = PackedColors::with_indexed(196, 21);
        assert!(colors.fg_is_indexed());
        assert!(colors.bg_is_indexed());
        assert_eq!(colors.fg_index(), 196);
        assert_eq!(colors.bg_index(), 21);
    }

    #[test]
    fn packed_color_indexed() {
        let color = PackedColor::indexed(196);
        assert!(color.is_indexed());
        assert!(!color.is_rgb());
        assert_eq!(color.index(), 196);
    }

    #[test]
    fn packed_color_rgb() {
        let color = PackedColor::rgb(255, 128, 64);
        assert!(color.is_rgb());
        assert!(!color.is_indexed());
        assert_eq!(color.rgb_components(), (255, 128, 64));
    }

    #[test]
    fn cell_is_empty() {
        assert!(Cell::EMPTY.is_empty());
        assert!(Cell::default().is_empty());

        let cell = Cell::new('X');
        assert!(!cell.is_empty());
    }

    #[test]
    fn cell_clear() {
        let mut cell = Cell::with_style(
            'X',
            PackedColor::indexed(196),
            PackedColor::indexed(21),
            CellFlags::BOLD,
        );
        cell.clear();
        assert!(cell.is_empty());
    }

    #[test]
    fn cell_set_methods() {
        let mut cell = Cell::EMPTY;

        cell.set_char('Z');
        assert_eq!(cell.char(), 'Z');

        cell.set_fg(PackedColor::indexed(100));
        assert!(cell.colors().fg_is_indexed());
        assert_eq!(cell.colors().fg_index(), 100);

        cell.set_flags(CellFlags::STRIKETHROUGH);
        assert!(cell.flags().contains(CellFlags::STRIKETHROUGH));
    }

    #[test]
    fn cell_with_overflow_index() {
        let cell = Cell::with_overflow_index(42);
        assert!(cell.is_complex());
        assert_eq!(cell.char_data(), 42);
        assert_eq!(cell.codepoint(), 0xFFFD); // Returns replacement for complex
        assert_eq!(cell.char(), '\u{FFFD}');
    }

    #[test]
    fn cell_rgb_needs_overflow() {
        let cell = Cell::with_style(
            'X',
            PackedColor::rgb(255, 0, 0),
            PackedColor::rgb(0, 0, 255),
            CellFlags::empty(),
        );
        assert!(cell.fg_needs_overflow());
        assert!(cell.bg_needs_overflow());
    }

    // =========================================================================
    // StyleId tests
    // =========================================================================

    #[test]
    fn cell_with_style_id_default() {
        use super::super::style::GRID_DEFAULT_STYLE_ID;
        let cell = Cell::with_style_id('A', GRID_DEFAULT_STYLE_ID, CellFlags::empty());
        assert!(cell.uses_style_id());
        assert_eq!(cell.style_id(), GRID_DEFAULT_STYLE_ID);
        assert_eq!(cell.char(), 'A');
        assert!(!cell.is_complex());
    }

    #[test]
    fn cell_with_style_id_non_default() {
        let style_id = StyleId(42);
        let cell = Cell::with_style_id('X', style_id, CellFlags::empty());
        assert!(cell.uses_style_id());
        assert_eq!(cell.style_id(), style_id);
        assert_eq!(cell.char(), 'X');
    }

    #[test]
    fn cell_with_style_id_preserves_cell_flags() {
        let style_id = StyleId(100);
        let cell = Cell::with_style_id('W', style_id, CellFlags::WIDE);
        assert!(cell.uses_style_id());
        assert!(cell.flags().contains(CellFlags::WIDE));
        assert!(cell.flags().contains(CellFlags::USES_STYLE_ID));
        assert_eq!(cell.style_id(), style_id);
    }

    #[test]
    fn cell_from_ascii_with_style_id() {
        let style_id = StyleId(5);
        let cell = Cell::from_ascii_with_style_id(b'H', style_id, CellFlags::empty());
        assert!(cell.uses_style_id());
        assert_eq!(cell.style_id(), style_id);
        assert_eq!(cell.char(), 'H');
    }

    #[test]
    fn cell_style_id_opt_when_using_style() {
        let style_id = StyleId(77);
        let cell = Cell::with_style_id('Y', style_id, CellFlags::empty());
        assert_eq!(cell.style_id_opt(), Some(style_id));
    }

    #[test]
    fn cell_style_id_opt_when_using_inline_colors() {
        let cell = Cell::new('Z');
        assert!(!cell.uses_style_id());
        assert_eq!(cell.style_id_opt(), None);
    }

    #[test]
    fn cell_set_style_id() {
        let mut cell = Cell::new('A');
        assert!(!cell.uses_style_id());

        let style_id = StyleId(123);
        cell.set_style_id(style_id);

        assert!(cell.uses_style_id());
        assert_eq!(cell.style_id(), style_id);
        // Character should be preserved
        assert_eq!(cell.char(), 'A');
    }

    #[test]
    fn cell_clear_style_id() {
        let style_id = StyleId(50);
        let mut cell = Cell::with_style_id('B', style_id, CellFlags::empty());
        assert!(cell.uses_style_id());

        cell.clear_style_id();

        assert!(!cell.uses_style_id());
        assert!(cell.colors().is_default());
    }

    #[test]
    fn cell_style_id_max_value() {
        // Test with maximum StyleId value
        let style_id = StyleId(u16::MAX);
        let cell = Cell::with_style_id('M', style_id, CellFlags::empty());
        assert!(cell.uses_style_id());
        assert_eq!(cell.style_id(), style_id);
    }

    #[test]
    fn cell_with_style_id_wide_continuation() {
        let style_id = StyleId(10);
        let cell = Cell::with_style_id(' ', style_id, CellFlags::WIDE_CONTINUATION);
        assert!(cell.uses_style_id());
        assert!(cell.flags().contains(CellFlags::WIDE_CONTINUATION));
        assert_eq!(cell.style_id(), style_id);
    }

    #[test]
    fn cell_with_style_id_size_unchanged() {
        // Verify that using StyleId doesn't change cell size
        let style_id = StyleId(42);
        let cell = Cell::with_style_id('X', style_id, CellFlags::empty());
        assert_eq!(std::mem::size_of_val(&cell), 8);
    }

    #[test]
    fn cell_flags_uses_style_id() {
        // Test the CellFlags::USES_STYLE_ID constant
        let flags = CellFlags::USES_STYLE_ID;
        assert!(flags.uses_style_id());
        assert!(!flags.is_complex());

        let combined = CellFlags::USES_STYLE_ID.union(CellFlags::WIDE);
        assert!(combined.uses_style_id());
        assert!(combined.contains(CellFlags::WIDE));
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Codepoint pack/unpack is lossless for BMP codepoints.
    #[kani::proof]
    fn cell_bmp_codepoint_roundtrip() {
        let codepoint: u16 = kani::any();
        // Only test valid non-surrogate BMP codepoints
        kani::assume(codepoint < 0xD800 || codepoint > 0xDFFF);

        if let Some(c) = char::from_u32(codepoint as u32) {
            let cell = Cell::new(c);
            kani::assert(!cell.is_complex(), "BMP char should not be complex");
            kani::assert(
                cell.codepoint() == codepoint as u32,
                "BMP codepoint roundtrip failed",
            );
        }
    }

    /// Flags pack/unpack is lossless.
    #[kani::proof]
    fn cell_flags_roundtrip() {
        let flags_bits: u16 = kani::any();
        // Mask to valid range (exclude COMPLEX for this test)
        let flags_bits = flags_bits & CellFlags::VISUAL_FLAGS_MASK;

        let flags = CellFlags::from_bits(flags_bits);
        let cell = Cell::with_style(' ', PackedColor::DEFAULT_FG, PackedColor::DEFAULT_BG, flags);

        kani::assert(
            (cell.flags().bits() & CellFlags::VISUAL_FLAGS_MASK) == flags_bits,
            "flags roundtrip failed",
        );
    }

    /// Cell size is exactly 8 bytes.
    #[kani::proof]
    fn cell_size_is_8_bytes() {
        kani::assert(std::mem::size_of::<Cell>() == 8, "cell size is not 8 bytes");
    }

    /// Indexed color roundtrip.
    #[kani::proof]
    fn packed_colors_indexed_roundtrip() {
        let fg_index: u8 = kani::any();
        let bg_index: u8 = kani::any();

        let colors = PackedColors::with_indexed(fg_index, bg_index);

        kani::assert(colors.fg_is_indexed(), "fg should be indexed");
        kani::assert(colors.bg_is_indexed(), "bg should be indexed");
        kani::assert(colors.fg_index() == fg_index, "fg index mismatch");
        kani::assert(colors.bg_index() == bg_index, "bg index mismatch");
    }

    /// Complex cell flag handling.
    #[kani::proof]
    fn cell_complex_flag() {
        let index: u16 = kani::any();
        let cell = Cell::with_overflow_index(index);

        kani::assert(cell.is_complex(), "should be complex");
        kani::assert(cell.char_data() == index, "index should match");
        kani::assert(
            cell.codepoint() == 0xFFFD,
            "complex codepoint should be replacement",
        );
    }
}
