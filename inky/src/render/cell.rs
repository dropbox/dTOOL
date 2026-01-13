//! Terminal cell representation (8 bytes, GPU-compatible).

use crate::style::Color;

/// A single terminal cell, packed into exactly 8 bytes for GPU compatibility.
///
/// Memory layout (8 bytes total):
/// - char_data: 2 bytes (BMP char or overflow index)
/// - fg: 2 bytes (RGB565 foreground)
/// - bg: 2 bytes (RGB565 background)
/// - flags: 2 bytes (bold, italic, underline, etc.)
///
/// Colors use RGB565 format (5 bits red, 6 bits green, 5 bits blue) which provides
/// 65,536 colors - more than sufficient for terminal UIs. The max quantization error
/// is 7 units per channel (out of 255), imperceptible in practice.
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Cell {
    /// Character data (BMP char or overflow table index).
    pub char_data: u16,
    /// Foreground color (RGB565 packed).
    fg_packed: u16,
    /// Background color (RGB565 packed).
    bg_packed: u16,
    /// Cell flags (bold, italic, etc.).
    pub flags: CellFlags,
}

impl Cell {
    /// Create a cell with a character.
    #[inline]
    pub fn new(c: char) -> Self {
        Self {
            char_data: if c as u32 <= 0xFFFF {
                c as u16
            } else {
                // Replace non-BMP with the Unicode replacement character.
                char::REPLACEMENT_CHARACTER as u16
            },
            fg_packed: PackedColor::WHITE.to_rgb565(),
            bg_packed: PackedColor::BLACK.to_rgb565(),
            flags: CellFlags::empty(),
        }
    }

    /// Create a blank cell.
    #[inline]
    pub fn blank() -> Self {
        Self::new(' ')
    }

    /// Get the character.
    #[inline]
    pub fn char(&self) -> char {
        if self.flags.contains(CellFlags::OVERFLOW) {
            // Overflow table not implemented yet; surface replacement char.
            char::REPLACEMENT_CHARACTER
        } else {
            char::from_u32(self.char_data as u32).unwrap_or(' ')
        }
    }

    /// Get foreground color.
    #[inline]
    pub fn fg(&self) -> PackedColor {
        PackedColor::from_rgb565(self.fg_packed)
    }

    /// Get background color.
    #[inline]
    pub fn bg(&self) -> PackedColor {
        PackedColor::from_rgb565(self.bg_packed)
    }

    /// Set foreground color.
    #[inline]
    pub fn set_fg(&mut self, color: PackedColor) {
        self.fg_packed = color.to_rgb565();
    }

    /// Set background color.
    #[inline]
    pub fn set_bg(&mut self, color: PackedColor) {
        self.bg_packed = color.to_rgb565();
    }

    /// Set foreground color (builder pattern).
    pub fn with_fg(mut self, color: Color) -> Self {
        self.fg_packed = PackedColor::from(color).to_rgb565();
        self
    }

    /// Set background color (builder pattern).
    pub fn with_bg(mut self, color: Color) -> Self {
        self.bg_packed = PackedColor::from(color).to_rgb565();
        self
    }

    /// Set bold.
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.flags.set(CellFlags::BOLD, bold);
        self
    }

    /// Set italic.
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.flags.set(CellFlags::ITALIC, italic);
        self
    }

    /// Set underline.
    pub fn with_underline(mut self, underline: bool) -> Self {
        self.flags.set(CellFlags::UNDERLINE, underline);
        self
    }

    /// Set dim.
    pub fn with_dim(mut self, dim: bool) -> Self {
        self.flags.set(CellFlags::DIM, dim);
        self
    }

    /// Check if cell is dirty (needs redraw).
    #[inline]
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.flags.contains(CellFlags::DIRTY)
    }

    /// Mark cell as dirty.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.flags.insert(CellFlags::DIRTY);
    }

    /// Clear dirty flag.
    #[inline]
    pub fn clear_dirty(&mut self) {
        self.flags.remove(CellFlags::DIRTY);
    }
}

impl std::fmt::Debug for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cell")
            .field("char", &self.char())
            .field("fg", &self.fg())
            .field("bg", &self.bg())
            .field("flags", &self.flags)
            .finish()
    }
}

/// Packed RGB color (3 bytes).
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct PackedColor {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
}

impl PackedColor {
    /// Black color (0, 0, 0).
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    /// White color (255, 255, 255).
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };
    /// Red color (255, 0, 0).
    pub const RED: Self = Self { r: 255, g: 0, b: 0 };
    /// Green color (0, 255, 0).
    pub const GREEN: Self = Self { r: 0, g: 255, b: 0 };
    /// Blue color (0, 0, 255).
    pub const BLUE: Self = Self { r: 0, g: 0, b: 255 };

    /// Create a new packed color from RGB components.
    #[inline]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to RGB565 format (5 bits red, 6 bits green, 5 bits blue).
    ///
    /// This packs a 24-bit RGB color into 16 bits with minimal quality loss.
    /// Max quantization error is 7 units per channel (out of 255).
    #[inline]
    pub fn to_rgb565(self) -> u16 {
        let r = (self.r as u16 >> 3) & 0x1F;
        let g = (self.g as u16 >> 2) & 0x3F;
        let b = (self.b as u16 >> 3) & 0x1F;
        (r << 11) | (g << 5) | b
    }

    /// Create from RGB565 format.
    ///
    /// Expands 16-bit RGB565 back to 24-bit RGB by scaling.
    #[inline]
    pub fn from_rgb565(packed: u16) -> Self {
        // Extract and scale back to 8-bit
        // For 5-bit values: multiply by 255/31 ≈ 8.226, we use (v << 3) | (v >> 2)
        // For 6-bit values: multiply by 255/63 ≈ 4.048, we use (v << 2) | (v >> 4)
        let r5 = ((packed >> 11) & 0x1F) as u8;
        let g6 = ((packed >> 5) & 0x3F) as u8;
        let b5 = (packed & 0x1F) as u8;

        Self {
            r: (r5 << 3) | (r5 >> 2),
            g: (g6 << 2) | (g6 >> 4),
            b: (b5 << 3) | (b5 >> 2),
        }
    }

    /// Convert ANSI 256 color to RGB.
    pub fn from_ansi256(n: u8) -> Self {
        match n {
            // Standard colors (0-15) - use bright variants for 8-15
            0 => Self::BLACK,
            1 => Self::new(128, 0, 0),
            2 => Self::new(0, 128, 0),
            3 => Self::new(128, 128, 0),
            4 => Self::new(0, 0, 128),
            5 => Self::new(128, 0, 128),
            6 => Self::new(0, 128, 128),
            7 => Self::new(192, 192, 192),
            8 => Self::new(128, 128, 128),
            9 => Self::RED,
            10 => Self::GREEN,
            11 => Self::new(255, 255, 0),
            12 => Self::BLUE,
            13 => Self::new(255, 0, 255),
            14 => Self::new(0, 255, 255),
            15 => Self::WHITE,
            // 6x6x6 color cube (16-231)
            16..=231 => {
                let n = n - 16;
                let r = (n / 36) % 6;
                let g = (n / 6) % 6;
                let b = n % 6;
                let to_rgb = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                Self::new(to_rgb(r), to_rgb(g), to_rgb(b))
            }
            // Grayscale (232-255)
            232..=255 => {
                let gray = 8 + (n - 232) * 10;
                Self::new(gray, gray, gray)
            }
        }
    }
}

impl std::fmt::Debug for PackedColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl From<Color> for PackedColor {
    fn from(color: Color) -> Self {
        match color {
            Color::Default => Self::WHITE,
            Color::Black => Self::BLACK,
            Color::Red => Self::new(128, 0, 0),
            Color::Green => Self::new(0, 128, 0),
            Color::Yellow => Self::new(128, 128, 0),
            Color::Blue => Self::new(0, 0, 128),
            Color::Magenta => Self::new(128, 0, 128),
            Color::Cyan => Self::new(0, 128, 128),
            Color::White => Self::new(192, 192, 192),
            Color::BrightBlack => Self::new(128, 128, 128),
            Color::BrightRed => Self::RED,
            Color::BrightGreen => Self::GREEN,
            Color::BrightYellow => Self::new(255, 255, 0),
            Color::BrightBlue => Self::BLUE,
            Color::BrightMagenta => Self::new(255, 0, 255),
            Color::BrightCyan => Self::new(0, 255, 255),
            Color::BrightWhite => Self::WHITE,
            Color::Ansi256(n) => Self::from_ansi256(n),
            Color::Rgb(r, g, b) => Self::new(r, g, b),
        }
    }
}

bitflags::bitflags! {
    /// Cell attribute flags.
    ///
    /// Flags control text rendering attributes like bold, italic, underline,
    /// as well as internal state like dirty tracking and wide character markers.
    #[repr(transparent)]
    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    pub struct CellFlags: u16 {
        /// Bold text.
        const BOLD          = 0b0000_0000_0001;
        /// Italic text.
        const ITALIC        = 0b0000_0000_0010;
        /// Underlined text.
        const UNDERLINE     = 0b0000_0000_0100;
        /// Strikethrough text.
        const STRIKETHROUGH = 0b0000_0000_1000;
        /// Dimmed/faint text.
        const DIM           = 0b0000_0001_0000;
        /// Inverse/reverse video (swap fg/bg).
        const INVERSE       = 0b0000_0010_0000;
        /// Hidden/invisible text.
        const HIDDEN        = 0b0000_0100_0000;
        /// Blinking text.
        const BLINK         = 0b0000_1000_0000;
        /// First cell of a wide (CJK) character.
        const WIDE_CHAR     = 0b0001_0000_0000;
        /// Spacer cell following a wide character.
        const WIDE_SPACER   = 0b0010_0000_0000;
        /// Cell content overflowed from adjacent cell.
        const OVERFLOW      = 0b0100_0000_0000;
        /// Cell has been modified since last render.
        const DIRTY         = 0b1000_0000_0000;
    }
}

impl std::fmt::Debug for CellFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        bitflags::parser::to_writer(self, f)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn cell_new_bmp_char_round_trips() {
        let cell = Cell::new('A');
        assert_eq!(cell.char(), 'A');
    }

    #[test]
    fn cell_new_non_bmp_char_uses_replacement() {
        let non_bmp = char::from_u32(0x1F600).expect("valid non-BMP char");
        let cell = Cell::new(non_bmp);

        assert_eq!(cell.char(), char::REPLACEMENT_CHARACTER);
        assert_eq!(cell.char_data, char::REPLACEMENT_CHARACTER as u16);
    }

    #[test]
    fn cell_is_8_bytes() {
        // Critical invariant: Cell must be exactly 8 bytes for GPU compatibility
        assert_eq!(std::mem::size_of::<Cell>(), 8);
    }

    #[test]
    fn rgb565_round_trip() {
        // Test RGB565 color conversion accuracy
        let original = PackedColor::new(255, 128, 64);
        let packed = original.to_rgb565();
        let recovered = PackedColor::from_rgb565(packed);

        // RGB565 has max 7 units error per channel
        assert!((original.r as i32 - recovered.r as i32).abs() <= 8);
        assert!((original.g as i32 - recovered.g as i32).abs() <= 4);
        assert!((original.b as i32 - recovered.b as i32).abs() <= 8);
    }

    #[test]
    fn rgb565_preserves_extremes() {
        // Black and white should be preserved exactly
        let black = PackedColor::BLACK;
        assert_eq!(PackedColor::from_rgb565(black.to_rgb565()), black);

        let white = PackedColor::WHITE;
        assert_eq!(PackedColor::from_rgb565(white.to_rgb565()), white);
    }

    #[test]
    fn cell_color_accessors() {
        let mut cell = Cell::new('X');

        // Default colors
        assert_eq!(cell.fg(), PackedColor::WHITE);
        assert_eq!(cell.bg(), PackedColor::BLACK);

        // Set and get
        cell.set_fg(PackedColor::RED);
        cell.set_bg(PackedColor::BLUE);
        assert_eq!(cell.fg(), PackedColor::RED);
        assert_eq!(cell.bg(), PackedColor::BLUE);
    }
}
