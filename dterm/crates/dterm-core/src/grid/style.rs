//! Style deduplication (Ghostty pattern).
//!
//! Most cells share styles. We store unique styles once and reference by ID.
//! This provides significant memory savings when many cells share the same
//! color/attribute combination.
//!
//! ## Memory Savings
//!
//! Without deduplication: Each cell stores colors + attributes inline (6 bytes).
//! With deduplication: Each cell stores a 2-byte style ID, styles are shared.
//!
//! For a typical terminal with 10K lines × 200 cols = 2M cells:
//! - Without: 2M × 6 = 12 MB for style data
//! - With: 2M × 2 + 100 styles × 12 = 4 MB + 1.2 KB ≈ 4 MB
//! - Savings: ~67% (or 3x better)
//!
//! Real-world terminals typically have 50-200 unique styles, not millions.

use std::fmt;

use super::cell::{CellFlags, PackedColor, PackedColors};
use rustc_hash::FxHashMap;

/// RGB color tuple type (R, G, B).
pub type Rgb = (u8, u8, u8);

/// Optional RGB pair for foreground and background.
pub type RgbPair = (Option<Rgb>, Option<Rgb>);

/// The default style ID (index 0).
///
/// Named GRID_DEFAULT_STYLE_ID to avoid conflict with rle::DEFAULT_STYLE_ID in FFI.
pub const GRID_DEFAULT_STYLE_ID: StyleId = StyleId(0);

/// A style identifier.
///
/// Style IDs are indices into a `StyleTable`. The ID 0 is always the default
/// style (white on black, no attributes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct StyleId(pub u16);

impl StyleId {
    /// The default style ID (index 0).
    ///
    /// This is always valid and represents the default terminal style
    /// (white foreground, black background, no attributes).
    /// Equivalent to `GRID_DEFAULT_STYLE_ID`.
    pub const DEFAULT: StyleId = StyleId(0);

    /// Check if this is the default style.
    #[must_use]
    #[inline]
    pub const fn is_default(self) -> bool {
        self.0 == 0
    }

    /// Get the raw index value.
    #[must_use]
    #[inline]
    pub const fn index(self) -> u16 {
        self.0
    }
}

impl fmt::Display for StyleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "style:{}", self.0)
    }
}

/// RGBA color.
///
/// Note: Default is black (0,0,0,255), not zero values.
/// Use `Color::DEFAULT_FG` for white or `Color::DEFAULT_BG` for black explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Color {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
    /// Alpha component.
    pub a: u8,
}

impl Default for Color {
    /// Default color is black (used for background).
    fn default() -> Self {
        Self::DEFAULT_BG
    }
}

impl Color {
    /// Create a new opaque color.
    #[must_use]
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a color with alpha.
    #[must_use]
    #[inline]
    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Default foreground color (white).
    pub const DEFAULT_FG: Self = Self::new(255, 255, 255);

    /// Default background color (black).
    pub const DEFAULT_BG: Self = Self::new(0, 0, 0);

    /// Create from RGB components.
    #[must_use]
    #[inline]
    pub const fn from_rgb(rgb: (u8, u8, u8)) -> Self {
        Self::new(rgb.0, rgb.1, rgb.2)
    }

    /// Get RGB components as a tuple.
    #[must_use]
    #[inline]
    pub const fn to_rgb(self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }

    /// Check if this is the default foreground color.
    #[must_use]
    #[inline]
    pub const fn is_default_fg(self) -> bool {
        self.r == 255 && self.g == 255 && self.b == 255 && self.a == 255
    }

    /// Check if this is the default background color.
    #[must_use]
    #[inline]
    pub const fn is_default_bg(self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0 && self.a == 255
    }

    /// Create a color from an ANSI 256-color index.
    ///
    /// Color indices map as follows:
    /// - 0-7: Standard colors (black, red, green, yellow, blue, magenta, cyan, white)
    /// - 8-15: Bright colors (bright versions of 0-7)
    /// - 16-231: 6×6×6 color cube
    /// - 232-255: Grayscale (dark to light)
    #[must_use]
    pub const fn from_ansi_256(index: u8) -> Self {
        // Standard ANSI colors (xterm defaults)
        const ANSI_16: [(u8, u8, u8); 16] = [
            (0, 0, 0),       // 0: Black
            (205, 0, 0),     // 1: Red
            (0, 205, 0),     // 2: Green
            (205, 205, 0),   // 3: Yellow
            (0, 0, 238),     // 4: Blue
            (205, 0, 205),   // 5: Magenta
            (0, 205, 205),   // 6: Cyan
            (229, 229, 229), // 7: White
            (127, 127, 127), // 8: Bright Black (Gray)
            (255, 0, 0),     // 9: Bright Red
            (0, 255, 0),     // 10: Bright Green
            (255, 255, 0),   // 11: Bright Yellow
            (92, 92, 255),   // 12: Bright Blue
            (255, 0, 255),   // 13: Bright Magenta
            (0, 255, 255),   // 14: Bright Cyan
            (255, 255, 255), // 15: Bright White
        ];

        if index < 16 {
            // Standard and bright colors (0-15)
            let (r, g, b) = ANSI_16[index as usize];
            Self::new(r, g, b)
        } else if index < 232 {
            // 6×6×6 color cube (indices 16-231)
            // idx ranges from 0-215, representing a 6×6×6 cube
            // Formula: color = 16 + 36*r + 6*g + b where r,g,b ∈ [0,5]
            // Note: bounds already enforced by if-else structure
            let idx = index - 16;
            let r = if idx / 36 == 0 {
                0
            } else {
                55 + (idx / 36) * 40
            };
            let g = if (idx % 36) / 6 == 0 {
                0
            } else {
                55 + ((idx % 36) / 6) * 40
            };
            let b = if idx % 6 == 0 { 0 } else { 55 + (idx % 6) * 40 };
            Self::new(r, g, b)
        } else {
            // Grayscale (indices 232-255)
            // idx ranges from 0-23, producing grays from 8 to 238
            // Note: bounds already enforced by if-else structure (index >= 232 implied)
            let gray = 8 + (index - 232) * 10;
            Self::new(gray, gray, gray)
        }
    }
}

/// Text style combining colors and attributes.
///
/// A Style represents the visual appearance of a cell, including:
/// - Foreground color (text color)
/// - Background color
/// - Text attributes (bold, italic, underline, etc.)
///
/// Styles are immutable value types designed for efficient hashing and comparison.
/// The `StyleTable` interns styles so identical combinations share memory.
///
/// The default style is white text on black background with no attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Style {
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Style attributes.
    pub attrs: StyleAttrs,
}

impl Default for Style {
    /// Default style: white text on black background, no attributes.
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Color type for style storage.
///
/// Used to track whether a color is default, indexed (palette), or RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ColorType {
    /// Default terminal color (white fg, black bg).
    #[default]
    Default = 0,
    /// Indexed color (0-255 palette).
    Indexed = 1,
    /// True color RGB.
    Rgb = 2,
}

/// Extended style with color type information.
///
/// This struct stores the full style information including color types,
/// allowing conversion back to `PackedColors + CellFlags` format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ExtendedStyle {
    /// Core style (colors + attrs).
    pub style: Style,
    /// Foreground color type.
    pub fg_type: ColorType,
    /// Background color type.
    pub bg_type: ColorType,
    /// Foreground index (when fg_type == Indexed).
    pub fg_index: u8,
    /// Background index (when bg_type == Indexed).
    pub bg_index: u8,
}

impl Style {
    /// The default style (white text on black background, no attributes).
    pub const DEFAULT: Self = Self {
        fg: Color::DEFAULT_FG,
        bg: Color::DEFAULT_BG,
        attrs: StyleAttrs::empty(),
    };

    /// Create a new style with the given colors and attributes.
    #[must_use]
    #[inline]
    pub const fn new(fg: Color, bg: Color, attrs: StyleAttrs) -> Self {
        Self { fg, bg, attrs }
    }

    /// Create a style with just foreground color.
    #[must_use]
    #[inline]
    pub const fn with_fg(fg: Color) -> Self {
        Self {
            fg,
            bg: Color::DEFAULT_BG,
            attrs: StyleAttrs::empty(),
        }
    }

    /// Create a style with just background color.
    #[must_use]
    #[inline]
    pub const fn with_bg(bg: Color) -> Self {
        Self {
            fg: Color::DEFAULT_FG,
            bg,
            attrs: StyleAttrs::empty(),
        }
    }

    /// Create a style with just attributes.
    #[must_use]
    #[inline]
    pub const fn with_attrs(attrs: StyleAttrs) -> Self {
        Self {
            fg: Color::DEFAULT_FG,
            bg: Color::DEFAULT_BG,
            attrs,
        }
    }

    /// Check if this is the default style.
    #[must_use]
    #[inline]
    pub const fn is_default(&self) -> bool {
        self.fg.is_default_fg() && self.bg.is_default_bg() && self.attrs.is_empty()
    }

    /// Return a style with the foreground color changed.
    #[must_use]
    #[inline]
    pub const fn set_fg(self, fg: Color) -> Self {
        Self { fg, ..self }
    }

    /// Return a style with the background color changed.
    #[must_use]
    #[inline]
    pub const fn set_bg(self, bg: Color) -> Self {
        Self { bg, ..self }
    }

    /// Return a style with the attributes changed.
    #[must_use]
    #[inline]
    pub const fn set_attrs(self, attrs: StyleAttrs) -> Self {
        Self { attrs, ..self }
    }
}

impl ExtendedStyle {
    /// The default extended style.
    pub const DEFAULT: Self = Self {
        style: Style::DEFAULT,
        fg_type: ColorType::Default,
        bg_type: ColorType::Default,
        fg_index: 0,
        bg_index: 0,
    };

    /// Create from PackedColors and CellFlags.
    ///
    /// This converts the cell-format style to an extended style that can
    /// be used with the StyleTable.
    ///
    /// # Arguments
    ///
    /// * `colors` - Packed foreground/background colors
    /// * `flags` - Cell flags (only style-related flags are used)
    /// * `fg_rgb` - Optional RGB value for foreground (when colors indicates RGB mode)
    /// * `bg_rgb` - Optional RGB value for background (when colors indicates RGB mode)
    #[must_use]
    pub fn from_cell_style(
        colors: PackedColors,
        flags: CellFlags,
        fg_rgb: Option<(u8, u8, u8)>,
        bg_rgb: Option<(u8, u8, u8)>,
    ) -> Self {
        // Determine foreground
        let (fg, fg_type, fg_index) = if colors.fg_is_default() {
            (Color::DEFAULT_FG, ColorType::Default, 0)
        } else if colors.fg_is_indexed() {
            let idx = colors.fg_index();
            // For indexed colors, we could resolve to RGB here or store index
            // For now, store the index for later palette lookup
            (Color::DEFAULT_FG, ColorType::Indexed, idx)
        } else if colors.fg_is_rgb() {
            let rgb = fg_rgb.unwrap_or((255, 255, 255));
            (Color::from_rgb(rgb), ColorType::Rgb, 0)
        } else {
            (Color::DEFAULT_FG, ColorType::Default, 0)
        };

        // Determine background
        let (bg, bg_type, bg_index) = if colors.bg_is_default() {
            (Color::DEFAULT_BG, ColorType::Default, 0)
        } else if colors.bg_is_indexed() {
            let idx = colors.bg_index();
            (Color::DEFAULT_BG, ColorType::Indexed, idx)
        } else if colors.bg_is_rgb() {
            let rgb = bg_rgb.unwrap_or((0, 0, 0));
            (Color::from_rgb(rgb), ColorType::Rgb, 0)
        } else {
            (Color::DEFAULT_BG, ColorType::Default, 0)
        };

        // Convert CellFlags to StyleAttrs
        let attrs = Self::cell_flags_to_attrs(flags);

        Self {
            style: Style { fg, bg, attrs },
            fg_type,
            bg_type,
            fg_index,
            bg_index,
        }
    }

    /// Create from separate foreground and background PackedColor values.
    ///
    /// This is used by Terminal's CurrentStyle which stores fg/bg as separate
    /// PackedColor values rather than the combined PackedColors format.
    ///
    /// # Arguments
    ///
    /// * `fg` - Foreground PackedColor
    /// * `bg` - Background PackedColor
    /// * `flags` - Cell flags (only style-related flags are used)
    #[must_use]
    pub fn from_packed_colors_separate(fg: PackedColor, bg: PackedColor, flags: CellFlags) -> Self {
        // Determine foreground - resolve indexed colors to RGB for proper deduplication
        let (fg_color, fg_type, fg_index) = if fg.is_default() {
            (Color::DEFAULT_FG, ColorType::Default, 0)
        } else if fg.is_indexed() {
            let idx = fg.index();
            // Resolve indexed color to RGB so Style can be properly compared/hashed
            (Color::from_ansi_256(idx), ColorType::Indexed, idx)
        } else if fg.is_rgb() {
            let (r, g, b) = fg.rgb_components();
            (Color::new(r, g, b), ColorType::Rgb, 0)
        } else {
            (Color::DEFAULT_FG, ColorType::Default, 0)
        };

        // Determine background - resolve indexed colors to RGB for proper deduplication
        let (bg_color, bg_type, bg_index) = if bg.is_default() {
            (Color::DEFAULT_BG, ColorType::Default, 0)
        } else if bg.is_indexed() {
            let idx = bg.index();
            // Resolve indexed color to RGB so Style can be properly compared/hashed
            (Color::from_ansi_256(idx), ColorType::Indexed, idx)
        } else if bg.is_rgb() {
            let (r, g, b) = bg.rgb_components();
            (Color::new(r, g, b), ColorType::Rgb, 0)
        } else {
            (Color::DEFAULT_BG, ColorType::Default, 0)
        };

        // Convert CellFlags to StyleAttrs
        let attrs = Self::cell_flags_to_attrs(flags);

        Self {
            style: Style {
                fg: fg_color,
                bg: bg_color,
                attrs,
            },
            fg_type,
            bg_type,
            fg_index,
            bg_index,
        }
    }

    /// Convert CellFlags to StyleAttrs.
    ///
    /// Maps the cell-level flags to style attributes.
    #[must_use]
    fn cell_flags_to_attrs(flags: CellFlags) -> StyleAttrs {
        let mut attrs = StyleAttrs::empty();

        if flags.contains(CellFlags::BOLD) {
            attrs |= StyleAttrs::BOLD;
        }
        if flags.contains(CellFlags::DIM) {
            attrs |= StyleAttrs::DIM;
        }
        if flags.contains(CellFlags::ITALIC) {
            attrs |= StyleAttrs::ITALIC;
        }
        if flags.contains(CellFlags::UNDERLINE) {
            attrs |= StyleAttrs::UNDERLINE;
        }
        if flags.contains(CellFlags::BLINK) {
            attrs |= StyleAttrs::BLINK;
        }
        if flags.contains(CellFlags::INVERSE) {
            attrs |= StyleAttrs::INVERSE;
        }
        if flags.contains(CellFlags::HIDDEN) {
            attrs |= StyleAttrs::HIDDEN;
        }
        if flags.contains(CellFlags::STRIKETHROUGH) {
            attrs |= StyleAttrs::STRIKETHROUGH;
        }
        if flags.contains(CellFlags::DOUBLE_UNDERLINE) {
            attrs |= StyleAttrs::DOUBLE_UNDERLINE;
        }
        if flags.contains(CellFlags::CURLY_UNDERLINE) {
            attrs |= StyleAttrs::CURLY_UNDERLINE;
        }

        attrs
    }

    /// Convert StyleAttrs back to CellFlags.
    ///
    /// Note: Only style-related flags are set. Cell-specific flags like
    /// WIDE, WIDE_CONTINUATION, COMPLEX are not affected.
    #[must_use]
    pub fn attrs_to_cell_flags(attrs: StyleAttrs) -> CellFlags {
        let mut flags = CellFlags::empty();

        if attrs.contains(StyleAttrs::BOLD) {
            flags = flags.union(CellFlags::BOLD);
        }
        if attrs.contains(StyleAttrs::DIM) {
            flags = flags.union(CellFlags::DIM);
        }
        if attrs.contains(StyleAttrs::ITALIC) {
            flags = flags.union(CellFlags::ITALIC);
        }
        if attrs.contains(StyleAttrs::UNDERLINE) {
            flags = flags.union(CellFlags::UNDERLINE);
        }
        if attrs.contains(StyleAttrs::BLINK) {
            flags = flags.union(CellFlags::BLINK);
        }
        if attrs.contains(StyleAttrs::INVERSE) {
            flags = flags.union(CellFlags::INVERSE);
        }
        if attrs.contains(StyleAttrs::HIDDEN) {
            flags = flags.union(CellFlags::HIDDEN);
        }
        if attrs.contains(StyleAttrs::STRIKETHROUGH) {
            flags = flags.union(CellFlags::STRIKETHROUGH);
        }
        if attrs.contains(StyleAttrs::DOUBLE_UNDERLINE) {
            flags = flags.union(CellFlags::DOUBLE_UNDERLINE);
        }
        if attrs.contains(StyleAttrs::CURLY_UNDERLINE) {
            flags = flags.union(CellFlags::CURLY_UNDERLINE);
        }

        flags
    }

    /// Convert back to PackedColors.
    ///
    /// Note: For RGB colors, the actual RGB values are stored separately.
    /// This method only sets the color mode indicators.
    #[must_use]
    pub fn to_packed_colors(&self) -> PackedColors {
        let mut colors = PackedColors::DEFAULT;

        match self.fg_type {
            ColorType::Default => {}
            ColorType::Indexed => {
                colors = colors.set_fg_indexed(self.fg_index);
            }
            ColorType::Rgb => {
                colors = colors.with_rgb_fg();
            }
        }

        match self.bg_type {
            ColorType::Default => {}
            ColorType::Indexed => {
                colors = colors.set_bg_indexed(self.bg_index);
            }
            ColorType::Rgb => {
                colors = colors.with_rgb_bg();
            }
        }

        colors
    }

    /// Get the RGB values for foreground and background (if RGB mode).
    ///
    /// Returns (fg_rgb, bg_rgb) where each is Some if the color is RGB mode.
    #[must_use]
    pub fn rgb_values(&self) -> RgbPair {
        let fg = if self.fg_type == ColorType::Rgb {
            Some(self.style.fg.to_rgb())
        } else {
            None
        };

        let bg = if self.bg_type == ColorType::Rgb {
            Some(self.style.bg.to_rgb())
        } else {
            None
        };

        (fg, bg)
    }
}

bitflags::bitflags! {
    /// Style attribute flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    #[repr(transparent)]
    pub struct StyleAttrs: u16 {
        /// Bold text.
        const BOLD = 1 << 0;
        /// Dim/faint text.
        const DIM = 1 << 1;
        /// Italic text.
        const ITALIC = 1 << 2;
        /// Underlined text.
        const UNDERLINE = 1 << 3;
        /// Blinking text.
        const BLINK = 1 << 4;
        /// Inverse video.
        const INVERSE = 1 << 5;
        /// Hidden/invisible text.
        const HIDDEN = 1 << 6;
        /// Strikethrough text.
        const STRIKETHROUGH = 1 << 7;
        /// Double underline.
        const DOUBLE_UNDERLINE = 1 << 8;
        /// Curly underline.
        const CURLY_UNDERLINE = 1 << 9;
        /// Dotted underline (SGR 4:4).
        const DOTTED_UNDERLINE = 1 << 10;
        /// Dashed underline (SGR 4:5).
        const DASHED_UNDERLINE = 1 << 11;
    }
}

/// Deduplicated style storage (Ghostty pattern).
///
/// Styles are interned: identical styles share the same ID.
/// Uses FxHashMap for fast lookup (2-3x faster than std HashMap for small keys).
///
/// ## Reference Counting
///
/// Each style has a reference count tracking how many cells use it.
/// This enables future garbage collection of unused styles.
///
/// ## Thread Safety
///
/// StyleTable is `!Sync` - it cannot be shared between threads.
/// This is enforced at compile-time via a `PhantomData` marker.
///
/// For multi-threaded use, wrap in `Mutex<StyleTable>` or `RwLock<StyleTable>`.
/// Note that ref_counts use regular u32 (not AtomicU32) for single-threaded
/// performance - locking at the table level is more efficient than per-cell
/// atomic operations.
///
/// ## Memory Layout
///
/// - `styles`: Vec of Style structs (12 bytes each)
/// - `ref_counts`: Vec of u32 (4 bytes each)
/// - `lookup`: FxHashMap for O(1) intern lookups
///
/// For 100 unique styles: ~1.6 KB storage + HashMap overhead
#[derive(Debug)]
pub struct StyleTable {
    /// Stored styles (index = StyleId).
    styles: Vec<Style>,
    /// Reference counts per style.
    ref_counts: Vec<u32>,
    /// Lookup table for interning (style -> id).
    lookup: FxHashMap<Style, StyleId>,
    /// Extended style information (optional, for round-trip conversion).
    /// Only populated when extended info is needed.
    extended: Vec<Option<ExtendedStyleInfo>>,
    /// Marker to make StyleTable !Sync (not shareable across threads).
    /// This catches accidental concurrent access at compile time.
    _not_sync: std::marker::PhantomData<std::cell::Cell<()>>,
}

/// Extended style information for round-trip conversion.
#[derive(Debug, Clone, Copy)]
struct ExtendedStyleInfo {
    fg_type: ColorType,
    bg_type: ColorType,
    fg_index: u8,
    bg_index: u8,
}

impl Default for StyleTable {
    fn default() -> Self {
        Self::new()
    }
}

impl StyleTable {
    /// Create a new style table with the default style at index 0.
    ///
    /// The default style is always at `StyleId(0)` and has a permanent
    /// reference count of 1 (never garbage collected).
    #[must_use]
    pub fn new() -> Self {
        let mut table = Self {
            styles: Vec::with_capacity(64),
            ref_counts: Vec::with_capacity(64),
            lookup: FxHashMap::default(),
            extended: Vec::with_capacity(64),
            _not_sync: std::marker::PhantomData,
        };
        // Style 0 is always the default
        table.styles.push(Style::default());
        table.ref_counts.push(1); // Permanent reference
        table.lookup.insert(Style::default(), StyleId(0));
        table.extended.push(None);
        table
    }

    /// Create a style table with pre-allocated capacity.
    ///
    /// Use this when you know approximately how many unique styles to expect.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut table = Self {
            styles: Vec::with_capacity(capacity),
            ref_counts: Vec::with_capacity(capacity),
            lookup: FxHashMap::default(),
            extended: Vec::with_capacity(capacity),
            _not_sync: std::marker::PhantomData,
        };
        // Style 0 is always the default
        table.styles.push(Style::default());
        table.ref_counts.push(1);
        table.lookup.insert(Style::default(), StyleId(0));
        table.extended.push(None);
        table
    }

    /// Intern a style, returning its ID.
    ///
    /// If the style already exists, increments its reference count and returns
    /// the existing ID. Otherwise, creates a new style entry.
    ///
    /// # Performance
    ///
    /// O(1) average case (hash lookup).
    pub fn intern(&mut self, style: Style) -> StyleId {
        if let Some(&id) = self.lookup.get(&style) {
            self.ref_counts[id.0 as usize] = self.ref_counts[id.0 as usize].saturating_add(1);
            return id;
        }

        self.insert_new_style(style, None)
    }

    /// Intern an extended style with color type information.
    ///
    /// This preserves the color type (default/indexed/rgb) for later
    /// conversion back to `PackedColors` format.
    pub fn intern_extended(&mut self, ext_style: ExtendedStyle) -> StyleId {
        if let Some(&id) = self.lookup.get(&ext_style.style) {
            self.ref_counts[id.0 as usize] = self.ref_counts[id.0 as usize].saturating_add(1);
            // Update extended info if not already set
            if self.extended[id.0 as usize].is_none() {
                self.extended[id.0 as usize] = Some(ExtendedStyleInfo {
                    fg_type: ext_style.fg_type,
                    bg_type: ext_style.bg_type,
                    fg_index: ext_style.fg_index,
                    bg_index: ext_style.bg_index,
                });
            }
            return id;
        }

        let info = ExtendedStyleInfo {
            fg_type: ext_style.fg_type,
            bg_type: ext_style.bg_type,
            fg_index: ext_style.fg_index,
            bg_index: ext_style.bg_index,
        };
        self.insert_new_style(ext_style.style, Some(info))
    }

    /// Insert a new style (not in table yet).
    fn insert_new_style(&mut self, style: Style, ext_info: Option<ExtendedStyleInfo>) -> StyleId {
        // Saturate at u16::MAX for safety (extremely unlikely in practice)
        if self.styles.len() >= u16::MAX as usize {
            return GRID_DEFAULT_STYLE_ID;
        }

        #[allow(clippy::cast_possible_truncation)]
        let id = StyleId(self.styles.len() as u16);
        self.styles.push(style);
        self.ref_counts.push(1);
        self.lookup.insert(style, id);
        self.extended.push(ext_info);
        id
    }

    /// Intern a style without incrementing reference count.
    ///
    /// Use this when you just want to check or get an ID without
    /// claiming ownership.
    #[must_use]
    pub fn get_id(&self, style: &Style) -> Option<StyleId> {
        self.lookup.get(style).copied()
    }

    /// Add a reference to an existing style.
    ///
    /// Call this when a cell starts using a style.
    #[inline]
    pub fn add_ref(&mut self, id: StyleId) {
        let idx = id.0 as usize;
        if idx < self.ref_counts.len() {
            self.ref_counts[idx] = self.ref_counts[idx].saturating_add(1);
        }
    }

    /// Release a reference to a style.
    ///
    /// Call this when a cell stops using a style.
    /// The style is not removed even when ref count reaches 0
    /// (to keep IDs stable). Use `compact()` to reclaim space.
    #[inline]
    pub fn release(&mut self, id: StyleId) {
        let idx = id.0 as usize;
        if idx < self.ref_counts.len() && self.ref_counts[idx] > 0 {
            // Don't decrement style 0 (permanent default)
            if idx > 0 {
                self.ref_counts[idx] -= 1;
            }
        }
    }

    /// Release multiple references at once.
    ///
    /// More efficient than calling `release()` in a loop.
    pub fn release_batch(&mut self, ids: &[StyleId]) {
        for &id in ids {
            self.release(id);
        }
    }

    /// Get a style by ID.
    #[must_use]
    #[inline]
    pub fn get(&self, id: StyleId) -> Option<&Style> {
        self.styles.get(id.0 as usize)
    }

    /// Get a style by ID, panicking if not found.
    ///
    /// Use this when you're certain the ID is valid.
    #[must_use]
    #[inline]
    pub fn get_unchecked(&self, id: StyleId) -> &Style {
        &self.styles[id.0 as usize]
    }

    /// Get extended style information for round-trip conversion.
    #[must_use]
    pub fn get_extended(&self, id: StyleId) -> Option<ExtendedStyle> {
        let idx = id.0 as usize;
        let style = self.styles.get(idx)?;
        let info = self.extended.get(idx)?.as_ref();

        Some(match info {
            Some(info) => ExtendedStyle {
                style: *style,
                fg_type: info.fg_type,
                bg_type: info.bg_type,
                fg_index: info.fg_index,
                bg_index: info.bg_index,
            },
            None => ExtendedStyle {
                style: *style,
                ..ExtendedStyle::DEFAULT
            },
        })
    }

    /// Get the number of unique styles.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Returns true if the table has only the default style.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.styles.len() <= 1
    }

    /// Get the reference count for a style.
    #[must_use]
    pub fn ref_count(&self, id: StyleId) -> u32 {
        self.ref_counts.get(id.0 as usize).copied().unwrap_or(0)
    }

    /// Get the number of styles with non-zero reference counts.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.ref_counts.iter().filter(|&&c| c > 0).count()
    }

    /// Estimate memory usage in bytes.
    #[must_use]
    pub fn memory_used(&self) -> usize {
        let styles_size = self.styles.capacity() * std::mem::size_of::<Style>();
        let ref_counts_size = self.ref_counts.capacity() * std::mem::size_of::<u32>();
        let extended_size =
            self.extended.capacity() * std::mem::size_of::<Option<ExtendedStyleInfo>>();
        // HashMap overhead estimate (key + value + bucket)
        let lookup_size = self.lookup.capacity() * (std::mem::size_of::<Style>() + 8);

        styles_size + ref_counts_size + extended_size + lookup_size
    }

    /// Get statistics about the style table.
    #[must_use]
    pub fn stats(&self) -> StyleTableStats {
        let total = self.styles.len();
        let active = self.active_count();
        let total_refs: u64 = self.ref_counts.iter().map(|&c| u64::from(c)).sum();

        StyleTableStats {
            total_styles: total,
            active_styles: active,
            total_refs,
            memory_bytes: self.memory_used(),
        }
    }

    /// Clear all styles except the default.
    ///
    /// This invalidates all existing StyleIds except GRID_DEFAULT_STYLE_ID.
    /// Use with caution - typically only during terminal reset.
    pub fn clear(&mut self) {
        self.styles.truncate(1);
        self.ref_counts.truncate(1);
        self.extended.truncate(1);
        self.lookup.clear();
        self.lookup.insert(Style::default(), StyleId(0));
    }

    /// Compact the table by removing unused styles.
    ///
    /// This remaps StyleIds, so callers must update all stored IDs.
    /// Returns a mapping from old IDs to new IDs.
    ///
    /// Note: This is an expensive operation. Only call during idle time.
    ///
    /// # Implementation
    ///
    /// Uses single-pass in-place compaction:
    /// 1. Build id_map while compacting arrays in-place
    /// 2. Rebuild lookup from compacted data
    /// 3. Truncate arrays to new length
    pub fn compact(&mut self) -> Vec<StyleId> {
        let len = self.styles.len();
        let mut id_map = vec![GRID_DEFAULT_STYLE_ID; len];

        // Style 0 always stays at position 0
        id_map[0] = StyleId(0);
        let mut write_idx = 1usize;

        // Single pass: scan and compact in-place
        // Note: read_idx used to index multiple arrays, not just id_map
        #[allow(clippy::needless_range_loop)]
        for read_idx in 1..len {
            if self.ref_counts[read_idx] > 0 {
                // Map old index to new index
                #[allow(clippy::cast_possible_truncation)]
                {
                    id_map[read_idx] = StyleId(write_idx as u16);
                }

                // Move data to compacted position (skip if already in place)
                if write_idx != read_idx {
                    self.styles[write_idx] = self.styles[read_idx];
                    self.ref_counts[write_idx] = self.ref_counts[read_idx];
                    self.extended[write_idx] = self.extended[read_idx];
                }
                write_idx += 1;
            }
        }

        // Truncate arrays to compacted size
        self.styles.truncate(write_idx);
        self.ref_counts.truncate(write_idx);
        self.extended.truncate(write_idx);

        // Rebuild lookup from compacted data (single iteration)
        self.lookup.clear();
        self.lookup.reserve(write_idx);
        for (idx, style) in self.styles.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            self.lookup.insert(*style, StyleId(idx as u16));
        }

        id_map
    }
}

/// Statistics about a StyleTable.
#[derive(Debug, Clone, Copy)]
pub struct StyleTableStats {
    /// Total number of unique styles.
    pub total_styles: usize,
    /// Number of styles with non-zero reference counts.
    pub active_styles: usize,
    /// Total reference count across all styles.
    pub total_refs: u64,
    /// Estimated memory usage in bytes.
    pub memory_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_id_default() {
        assert!(GRID_DEFAULT_STYLE_ID.is_default());
        assert!(!StyleId(1).is_default());
    }

    #[test]
    fn color_constructors() {
        let c = Color::new(100, 150, 200);
        assert_eq!(c.r, 100);
        assert_eq!(c.g, 150);
        assert_eq!(c.b, 200);
        assert_eq!(c.a, 255);

        let c2 = Color::with_alpha(100, 150, 200, 128);
        assert_eq!(c2.a, 128);
    }

    #[test]
    fn color_from_rgb_tuple() {
        let c = Color::from_rgb((10, 20, 30));
        assert_eq!(c.to_rgb(), (10, 20, 30));
    }

    #[test]
    fn color_is_default() {
        assert!(Color::DEFAULT_FG.is_default_fg());
        assert!(!Color::DEFAULT_FG.is_default_bg());
        assert!(Color::DEFAULT_BG.is_default_bg());
        assert!(!Color::DEFAULT_BG.is_default_fg());
    }

    #[test]
    fn style_constructors() {
        let s = Style::with_fg(Color::new(255, 0, 0));
        assert_eq!(s.fg, Color::new(255, 0, 0));
        assert_eq!(s.bg, Color::DEFAULT_BG);
        assert!(s.attrs.is_empty());

        let s = Style::with_bg(Color::new(0, 0, 255));
        assert_eq!(s.fg, Color::DEFAULT_FG);
        assert_eq!(s.bg, Color::new(0, 0, 255));

        let s = Style::with_attrs(StyleAttrs::BOLD | StyleAttrs::ITALIC);
        assert!(s.attrs.contains(StyleAttrs::BOLD));
        assert!(s.attrs.contains(StyleAttrs::ITALIC));
    }

    #[test]
    fn style_is_default() {
        assert!(Style::DEFAULT.is_default());
        assert!(!Style::with_fg(Color::new(100, 100, 100)).is_default());
        assert!(!Style::with_attrs(StyleAttrs::BOLD).is_default());
    }

    #[test]
    fn style_setters() {
        let s = Style::DEFAULT
            .set_fg(Color::new(255, 0, 0))
            .set_bg(Color::new(0, 255, 0))
            .set_attrs(StyleAttrs::UNDERLINE);

        assert_eq!(s.fg, Color::new(255, 0, 0));
        assert_eq!(s.bg, Color::new(0, 255, 0));
        assert!(s.attrs.contains(StyleAttrs::UNDERLINE));
    }

    #[test]
    fn intern_same_style() {
        let mut table = StyleTable::new();

        let style = Style {
            fg: Color::new(255, 0, 0),
            bg: Color::DEFAULT_BG,
            attrs: StyleAttrs::BOLD,
        };

        let id1 = table.intern(style);
        let id2 = table.intern(style);

        assert_eq!(id1, id2);
        assert_eq!(table.len(), 2); // default + our style
        assert_eq!(table.ref_count(id1), 2);
    }

    #[test]
    fn intern_different_styles() {
        let mut table = StyleTable::new();

        let style1 = Style {
            fg: Color::new(255, 0, 0),
            ..Default::default()
        };
        let style2 = Style {
            fg: Color::new(0, 255, 0),
            ..Default::default()
        };

        let id1 = table.intern(style1);
        let id2 = table.intern(style2);

        assert_ne!(id1, id2);
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn table_default_style() {
        let table = StyleTable::new();
        assert_eq!(table.len(), 1);
        // A table with only the default style is considered "empty" (no user styles)
        assert!(table.is_empty());

        let default = table.get(GRID_DEFAULT_STYLE_ID).unwrap();
        assert!(default.is_default());
    }

    #[test]
    fn table_release() {
        let mut table = StyleTable::new();

        let style = Style::with_fg(Color::new(255, 0, 0));
        let id = table.intern(style);
        assert_eq!(table.ref_count(id), 1);

        table.add_ref(id);
        assert_eq!(table.ref_count(id), 2);

        table.release(id);
        assert_eq!(table.ref_count(id), 1);

        table.release(id);
        assert_eq!(table.ref_count(id), 0);

        // Style still exists, just zero refs
        assert!(table.get(id).is_some());
    }

    #[test]
    fn table_release_default_not_decremented() {
        let mut table = StyleTable::new();

        // Default style always has ref_count >= 1
        let initial = table.ref_count(GRID_DEFAULT_STYLE_ID);
        table.release(GRID_DEFAULT_STYLE_ID);
        assert_eq!(table.ref_count(GRID_DEFAULT_STYLE_ID), initial);
    }

    #[test]
    fn table_get_id() {
        let mut table = StyleTable::new();

        let style = Style::with_fg(Color::new(255, 0, 0));
        assert!(table.get_id(&style).is_none());

        let id = table.intern(style);
        let ref_before = table.ref_count(id);

        let found_id = table.get_id(&style);
        assert_eq!(found_id, Some(id));

        // get_id shouldn't increment ref count
        assert_eq!(table.ref_count(id), ref_before);
    }

    #[test]
    fn table_stats() {
        let mut table = StyleTable::new();

        let style1 = Style::with_fg(Color::new(255, 0, 0));
        let style2 = Style::with_bg(Color::new(0, 0, 255));

        let id1 = table.intern(style1);
        table.intern(style1); // Second ref to style1
        table.intern(style2);

        let stats = table.stats();
        assert_eq!(stats.total_styles, 3);
        assert_eq!(stats.active_styles, 3);
        assert_eq!(stats.total_refs, 4); // 1 default + 2 style1 + 1 style2

        table.release(id1);
        table.release(id1);
        let stats = table.stats();
        assert_eq!(stats.active_styles, 2); // style1 now has 0 refs
    }

    #[test]
    fn table_compact() {
        let mut table = StyleTable::new();

        let style1 = Style::with_fg(Color::new(255, 0, 0));
        let style2 = Style::with_bg(Color::new(0, 0, 255));
        let style3 = Style::with_attrs(StyleAttrs::BOLD);

        let id1 = table.intern(style1);
        let id2 = table.intern(style2);
        let id3 = table.intern(style3);

        // Release style2
        table.release(id2);

        assert_eq!(table.len(), 4);

        let id_map = table.compact();

        // Should have removed style2
        assert_eq!(table.len(), 3);

        // Default should stay at 0
        assert_eq!(id_map[0], GRID_DEFAULT_STYLE_ID);

        // style1 and style3 should be remapped
        assert_ne!(id_map[id1.0 as usize], GRID_DEFAULT_STYLE_ID);
        assert_ne!(id_map[id3.0 as usize], GRID_DEFAULT_STYLE_ID);
    }

    #[test]
    fn table_clear() {
        let mut table = StyleTable::new();

        let style = Style::with_fg(Color::new(255, 0, 0));
        table.intern(style);
        table.intern(Style::with_attrs(StyleAttrs::BOLD));

        assert_eq!(table.len(), 3);

        table.clear();

        assert_eq!(table.len(), 1);
        assert!(table.get(GRID_DEFAULT_STYLE_ID).is_some());
    }

    #[test]
    fn extended_style_from_cell_style_default() {
        let ext =
            ExtendedStyle::from_cell_style(PackedColors::DEFAULT, CellFlags::empty(), None, None);

        assert_eq!(ext.fg_type, ColorType::Default);
        assert_eq!(ext.bg_type, ColorType::Default);
        assert!(ext.style.attrs.is_empty());
    }

    #[test]
    fn extended_style_from_cell_style_indexed() {
        let colors = PackedColors::with_indexed(196, 21);
        let ext = ExtendedStyle::from_cell_style(colors, CellFlags::BOLD, None, None);

        assert_eq!(ext.fg_type, ColorType::Indexed);
        assert_eq!(ext.fg_index, 196);
        assert_eq!(ext.bg_type, ColorType::Indexed);
        assert_eq!(ext.bg_index, 21);
        assert!(ext.style.attrs.contains(StyleAttrs::BOLD));
    }

    #[test]
    fn extended_style_from_cell_style_rgb() {
        let colors = PackedColors::DEFAULT.with_rgb_fg().with_rgb_bg();
        let ext = ExtendedStyle::from_cell_style(
            colors,
            CellFlags::ITALIC,
            Some((255, 128, 64)),
            Some((32, 64, 128)),
        );

        assert_eq!(ext.fg_type, ColorType::Rgb);
        assert_eq!(ext.style.fg.to_rgb(), (255, 128, 64));
        assert_eq!(ext.bg_type, ColorType::Rgb);
        assert_eq!(ext.style.bg.to_rgb(), (32, 64, 128));
        assert!(ext.style.attrs.contains(StyleAttrs::ITALIC));
    }

    #[test]
    fn extended_style_roundtrip() {
        let colors = PackedColors::with_indexed(100, 200);
        let flags = CellFlags::BOLD
            .union(CellFlags::UNDERLINE)
            .union(CellFlags::STRIKETHROUGH);

        let ext = ExtendedStyle::from_cell_style(colors, flags, None, None);

        let packed = ext.to_packed_colors();
        assert!(packed.fg_is_indexed());
        assert_eq!(packed.fg_index(), 100);
        assert!(packed.bg_is_indexed());
        assert_eq!(packed.bg_index(), 200);

        let cell_flags = ExtendedStyle::attrs_to_cell_flags(ext.style.attrs);
        assert!(cell_flags.contains(CellFlags::BOLD));
        assert!(cell_flags.contains(CellFlags::UNDERLINE));
        assert!(cell_flags.contains(CellFlags::STRIKETHROUGH));
    }

    #[test]
    fn intern_extended_style() {
        let mut table = StyleTable::new();

        let colors = PackedColors::with_indexed(100, 200);
        let flags = CellFlags::BOLD;
        let ext = ExtendedStyle::from_cell_style(colors, flags, None, None);

        let id = table.intern_extended(ext);

        let retrieved = table.get_extended(id).unwrap();
        assert_eq!(retrieved.fg_type, ColorType::Indexed);
        assert_eq!(retrieved.fg_index, 100);
        assert_eq!(retrieved.bg_index, 200);
    }

    #[test]
    fn all_style_attrs_roundtrip() {
        let all_attrs = StyleAttrs::BOLD
            | StyleAttrs::DIM
            | StyleAttrs::ITALIC
            | StyleAttrs::UNDERLINE
            | StyleAttrs::BLINK
            | StyleAttrs::INVERSE
            | StyleAttrs::HIDDEN
            | StyleAttrs::STRIKETHROUGH
            | StyleAttrs::DOUBLE_UNDERLINE
            | StyleAttrs::CURLY_UNDERLINE;

        let all_flags = CellFlags::BOLD
            .union(CellFlags::DIM)
            .union(CellFlags::ITALIC)
            .union(CellFlags::UNDERLINE)
            .union(CellFlags::BLINK)
            .union(CellFlags::INVERSE)
            .union(CellFlags::HIDDEN)
            .union(CellFlags::STRIKETHROUGH)
            .union(CellFlags::DOUBLE_UNDERLINE)
            .union(CellFlags::CURLY_UNDERLINE);

        // CellFlags -> StyleAttrs
        let ext = ExtendedStyle::from_cell_style(PackedColors::DEFAULT, all_flags, None, None);
        assert_eq!(ext.style.attrs, all_attrs);

        // StyleAttrs -> CellFlags
        let recovered_flags = ExtendedStyle::attrs_to_cell_flags(all_attrs);
        // Note: Only style-related flags should match
        let style_mask = CellFlags::BOLD
            .union(CellFlags::DIM)
            .union(CellFlags::ITALIC)
            .union(CellFlags::UNDERLINE)
            .union(CellFlags::BLINK)
            .union(CellFlags::INVERSE)
            .union(CellFlags::HIDDEN)
            .union(CellFlags::STRIKETHROUGH)
            .union(CellFlags::DOUBLE_UNDERLINE)
            .union(CellFlags::CURLY_UNDERLINE);

        assert_eq!(
            recovered_flags.bits() & style_mask.bits(),
            all_flags.bits() & style_mask.bits()
        );
    }
}
