//! DRCS (Downloadable Character Sets) - Soft Fonts.
//!
//! Implements DEC soft font support (DECDLD - Downloadable Character Sets)
//! for VT220+ terminals.
//!
//! ## Protocol
//!
//! DECDLD sequence format:
//! ```text
//! DCS Pfn;Pcn;Pe;Pcmw;Pss;Pt;Pcmh;Pcss { Dscs Sxbp1;Sxbp2;... ST
//! ```
//!
//! Parameters:
//! - `Pfn`: Font buffer number (0-3, default 0)
//! - `Pcn`: Starting character code (default 0x20)
//! - `Pe`: Erase mode (0=erase all, 1=erase only loaded, 2=erase all fonts)
//! - `Pcmw`: Character matrix width
//! - `Pss`: Screen size (0=80 col, 1=132 col, 2=any)
//! - `Pt`: Text/full cell (0=text, 1=full cell, 2=text OR full)
//! - `Pcmh`: Character matrix height
//! - `Pcss`: Character set size (0=94 chars, 1=96 chars)
//!
//! ## Verification
//!
//! - Kani proof: `drcs_glyph_count_bounded` - max 96 glyphs per font
//! - Kani proof: `drcs_char_code_valid` - codes in 0x20-0x7F range
//!
//! ## References
//!
//! - DEC VT510 Programmer Reference Manual, Chapter 12
//! - ECMA-35: Character Code Structure and Extension Techniques

use std::collections::HashMap;

/// Maximum number of DRCS font slots.
pub const MAX_FONT_SLOTS: usize = 4;

/// Maximum glyphs per font (96 for DRCS96, 94 for DRCS94).
pub const MAX_GLYPHS_PER_FONT: usize = 96;

/// Default character cell width in pixels.
pub const DEFAULT_CELL_WIDTH: u8 = 10;

/// Default character cell height in pixels.
pub const DEFAULT_CELL_HEIGHT: u8 = 20;

/// Maximum cell width in pixels.
pub const MAX_CELL_WIDTH: u8 = 15;

/// Maximum cell height in pixels.
pub const MAX_CELL_HEIGHT: u8 = 24;

/// Starting character code for DRCS (space).
pub const DRCS_START_CHAR: u8 = 0x20;

/// Ending character code for DRCS (DEL-1).
pub const DRCS_END_CHAR: u8 = 0x7F;

/// A single DRCS glyph (bitmap font character).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrcsGlyph {
    /// Bitmap data: row-major, 1 bit per pixel, LSB first.
    /// Each row is packed into bytes.
    bitmap: Vec<u8>,
    /// Width in pixels.
    width: u8,
    /// Height in pixels.
    height: u8,
}

impl DrcsGlyph {
    /// Create a new DRCS glyph from bitmap data.
    ///
    /// # Arguments
    /// - `bitmap`: Row-major bitmap, packed bytes (LSB first)
    /// - `width`: Glyph width in pixels (1-15)
    /// - `height`: Glyph height in pixels (1-24)
    ///
    /// Returns `None` if dimensions are invalid.
    #[must_use]
    pub fn new(bitmap: Vec<u8>, width: u8, height: u8) -> Option<Self> {
        if width == 0 || width > MAX_CELL_WIDTH {
            return None;
        }
        if height == 0 || height > MAX_CELL_HEIGHT {
            return None;
        }

        // Verify bitmap size
        let bytes_per_row = (usize::from(width) + 7) / 8;
        let expected_size = bytes_per_row * usize::from(height);
        if bitmap.len() != expected_size {
            return None;
        }

        Some(Self {
            bitmap,
            width,
            height,
        })
    }

    /// Get the glyph width in pixels.
    #[must_use]
    pub const fn width(&self) -> u8 {
        self.width
    }

    /// Get the glyph height in pixels.
    #[must_use]
    pub const fn height(&self) -> u8 {
        self.height
    }

    /// Get the raw bitmap data.
    #[must_use]
    pub fn bitmap(&self) -> &[u8] {
        &self.bitmap
    }

    /// Get a pixel value at (x, y).
    ///
    /// Returns `false` if coordinates are out of bounds.
    #[must_use]
    pub fn get_pixel(&self, x: u8, y: u8) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        let bytes_per_row = (usize::from(self.width) + 7) / 8;
        let row_offset = usize::from(y) * bytes_per_row;
        let byte_index = row_offset + usize::from(x) / 8;
        let bit_index = x % 8;

        if byte_index < self.bitmap.len() {
            (self.bitmap[byte_index] >> bit_index) & 1 == 1
        } else {
            false
        }
    }

    /// Create an empty glyph of the specified size.
    #[must_use]
    pub fn empty(width: u8, height: u8) -> Option<Self> {
        if width == 0 || width > MAX_CELL_WIDTH {
            return None;
        }
        if height == 0 || height > MAX_CELL_HEIGHT {
            return None;
        }

        let bytes_per_row = (usize::from(width) + 7) / 8;
        let bitmap = vec![0u8; bytes_per_row * usize::from(height)];
        Some(Self {
            bitmap,
            width,
            height,
        })
    }
}

/// DRCS font slot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrcsFontId {
    /// Font buffer number (0-3).
    pub buffer: u8,
    /// Starting character code (typically 0x20).
    pub start_char: u8,
}

impl DrcsFontId {
    /// Create a new font ID.
    ///
    /// # Arguments
    /// - `buffer`: Font buffer (0-3)
    /// - `start_char`: Starting character (0x20-0x7F)
    #[must_use]
    pub const fn new(buffer: u8, start_char: u8) -> Self {
        Self { buffer, start_char }
    }

    /// Default font ID (buffer 0, starting at space).
    pub const DEFAULT: Self = Self {
        buffer: 0,
        start_char: DRCS_START_CHAR,
    };
}

/// Erase mode for DECDLD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrcsEraseMode {
    /// Erase all characters in the DRCS set being loaded.
    #[default]
    EraseAll,
    /// Erase only the characters being loaded.
    EraseLoaded,
    /// Erase all DRCS characters in all font buffers.
    EraseAllFonts,
}

impl DrcsEraseMode {
    /// Parse from DECDLD Pe parameter.
    #[must_use]
    pub const fn from_param(pe: u16) -> Self {
        match pe {
            0 => Self::EraseAll,
            1 => Self::EraseLoaded,
            2 => Self::EraseAllFonts,
            _ => Self::EraseAll,
        }
    }
}

/// Character set size (94 or 96 characters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrcsCharsetSize {
    /// 94-character set (ISO 2022 compliant).
    #[default]
    Charset94,
    /// 96-character set (includes space and delete).
    Charset96,
}

impl DrcsCharsetSize {
    /// Parse from DECDLD Pcss parameter.
    #[must_use]
    pub const fn from_param(pcss: u16) -> Self {
        match pcss {
            0 => Self::Charset94,
            1 => Self::Charset96,
            _ => Self::Charset94,
        }
    }

    /// Get the maximum number of characters.
    #[must_use]
    pub const fn max_chars(&self) -> u8 {
        match self {
            Self::Charset94 => 94,
            Self::Charset96 => 96,
        }
    }
}

/// A downloadable character set (soft font).
#[derive(Debug, Clone)]
pub struct DrcsFont {
    /// Font identifier.
    id: DrcsFontId,
    /// Glyphs indexed by character code (relative to start_char).
    glyphs: HashMap<u8, DrcsGlyph>,
    /// Character cell width in pixels.
    cell_width: u8,
    /// Character cell height in pixels.
    cell_height: u8,
    /// Character set size.
    charset_size: DrcsCharsetSize,
}

impl DrcsFont {
    /// Create a new empty DRCS font.
    #[must_use]
    pub fn new(
        id: DrcsFontId,
        cell_width: u8,
        cell_height: u8,
        charset_size: DrcsCharsetSize,
    ) -> Self {
        Self {
            id,
            glyphs: HashMap::new(),
            cell_width: cell_width.clamp(1, MAX_CELL_WIDTH),
            cell_height: cell_height.clamp(1, MAX_CELL_HEIGHT),
            charset_size,
        }
    }

    /// Get the font identifier.
    #[must_use]
    pub const fn id(&self) -> DrcsFontId {
        self.id
    }

    /// Get the cell width in pixels.
    #[must_use]
    pub const fn cell_width(&self) -> u8 {
        self.cell_width
    }

    /// Get the cell height in pixels.
    #[must_use]
    pub const fn cell_height(&self) -> u8 {
        self.cell_height
    }

    /// Get the charset size.
    #[must_use]
    pub const fn charset_size(&self) -> DrcsCharsetSize {
        self.charset_size
    }

    /// Set a glyph at a character position.
    ///
    /// # Arguments
    /// - `char_code`: Character code (0-95 relative to start_char)
    /// - `glyph`: The glyph bitmap
    ///
    /// Returns `false` if the char_code is out of range.
    pub fn set_glyph(&mut self, char_code: u8, glyph: DrcsGlyph) -> bool {
        let max_chars = self.charset_size.max_chars();
        if char_code >= max_chars {
            return false;
        }
        self.glyphs.insert(char_code, glyph);
        true
    }

    /// Get a glyph by character code (relative to start_char).
    #[must_use]
    pub fn get_glyph(&self, char_code: u8) -> Option<&DrcsGlyph> {
        self.glyphs.get(&char_code)
    }

    /// Get the number of glyphs defined.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Check if a glyph is defined for a character code.
    #[must_use]
    pub fn has_glyph(&self, char_code: u8) -> bool {
        self.glyphs.contains_key(&char_code)
    }

    /// Clear all glyphs.
    pub fn clear(&mut self) {
        self.glyphs.clear();
    }

    /// Remove specific glyph.
    pub fn remove_glyph(&mut self, char_code: u8) -> Option<DrcsGlyph> {
        self.glyphs.remove(&char_code)
    }
}

/// Storage for all DRCS fonts.
#[derive(Debug, Clone, Default)]
pub struct DrcsStorage {
    /// Fonts indexed by font ID.
    fonts: HashMap<DrcsFontId, DrcsFont>,
}

impl DrcsStorage {
    /// Create empty DRCS storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
        }
    }

    /// Get or create a font slot.
    pub fn get_or_create_font(
        &mut self,
        id: DrcsFontId,
        cell_width: u8,
        cell_height: u8,
        charset_size: DrcsCharsetSize,
    ) -> &mut DrcsFont {
        self.fonts
            .entry(id)
            .or_insert_with(|| DrcsFont::new(id, cell_width, cell_height, charset_size))
    }

    /// Get a font by ID.
    #[must_use]
    pub fn get_font(&self, id: DrcsFontId) -> Option<&DrcsFont> {
        self.fonts.get(&id)
    }

    /// Get a mutable font by ID.
    pub fn get_font_mut(&mut self, id: DrcsFontId) -> Option<&mut DrcsFont> {
        self.fonts.get_mut(&id)
    }

    /// Remove a font.
    pub fn remove_font(&mut self, id: DrcsFontId) -> Option<DrcsFont> {
        self.fonts.remove(&id)
    }

    /// Erase fonts according to erase mode.
    pub fn erase(&mut self, mode: DrcsEraseMode, target_id: Option<DrcsFontId>) {
        match mode {
            DrcsEraseMode::EraseAll => {
                // Erase all characters in the specified font
                if let Some(id) = target_id {
                    if let Some(font) = self.fonts.get_mut(&id) {
                        font.clear();
                    }
                }
            }
            DrcsEraseMode::EraseLoaded => {
                // Only erase chars being loaded (no-op here, handled during load)
            }
            DrcsEraseMode::EraseAllFonts => {
                // Clear all fonts
                self.fonts.clear();
            }
        }
    }

    /// Clear all DRCS fonts.
    pub fn clear(&mut self) {
        self.fonts.clear();
    }

    /// Get the number of fonts loaded.
    #[must_use]
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// Check if any fonts are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// Get a glyph from storage.
    ///
    /// # Arguments
    /// - `id`: Font ID
    /// - `char_code`: Character code (relative to font's start_char)
    #[must_use]
    pub fn get_glyph(&self, id: DrcsFontId, char_code: u8) -> Option<&DrcsGlyph> {
        self.fonts.get(&id).and_then(|f| f.get_glyph(char_code))
    }
}

/// DECDLD sequence parser.
///
/// Parses the DECDLD sequence data format:
/// `{ Dscs Sxbp1;Sxbp2;... }`
///
/// Where each Sxbp is a sixel bitmap pattern encoded as ASCII.
#[derive(Debug, Clone)]
pub struct DecdldParser {
    /// Font buffer number (Pfn).
    pub font_buffer: u8,
    /// Starting character code (Pcn).
    pub start_char: u8,
    /// Erase mode (Pe).
    pub erase_mode: DrcsEraseMode,
    /// Character matrix width (Pcmw).
    pub cell_width: u8,
    /// Character matrix height (Pcmh).
    pub cell_height: u8,
    /// Character set size (Pcss).
    pub charset_size: DrcsCharsetSize,
    /// Current character being defined (relative to start_char).
    current_char: u8,
    /// Current row being parsed.
    current_row: u8,
    /// Accumulated bitmap data for current glyph.
    glyph_data: Vec<u8>,
    /// Parsing state.
    state: DecdldParseState,
}

/// DECDLD parsing state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DecdldParseState {
    /// Waiting for initial data.
    #[default]
    Initial,
    /// Parsing Dscs (character set designator).
    Dscs,
    /// Parsing sixel bitmap data.
    SixelData,
    /// Error occurred.
    Error,
}

impl Default for DecdldParser {
    fn default() -> Self {
        Self::new()
    }
}

impl DecdldParser {
    /// Create a new DECDLD parser with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_buffer: 0,
            start_char: DRCS_START_CHAR,
            erase_mode: DrcsEraseMode::EraseAll,
            cell_width: DEFAULT_CELL_WIDTH,
            cell_height: DEFAULT_CELL_HEIGHT,
            charset_size: DrcsCharsetSize::Charset94,
            current_char: 0,
            current_row: 0,
            glyph_data: Vec::new(),
            state: DecdldParseState::Initial,
        }
    }

    /// Initialize parser with DECDLD parameters.
    ///
    /// # Arguments
    /// - `params`: The numeric parameters from the DCS sequence
    ///
    /// Parameter order: Pfn;Pcn;Pe;Pcmw;Pss;Pt;Pcmh;Pcss
    pub fn init(&mut self, params: &[u16]) {
        // Reset state
        self.current_char = 0;
        self.current_row = 0;
        self.glyph_data.clear();
        self.state = DecdldParseState::Initial;

        // Parse parameters with defaults
        // Pfn: Font buffer (0-3, default 0)
        self.font_buffer = params.first().copied().unwrap_or(0).min(3) as u8;

        // Pcn: Starting character (default 0)
        // The actual start position in the character set
        let pcn = params.get(1).copied().unwrap_or(0);
        self.start_char = if pcn == 0 {
            DRCS_START_CHAR
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let sc = pcn.min(u16::from(DRCS_END_CHAR)) as u8;
            sc.max(DRCS_START_CHAR)
        };

        // Pe: Erase mode (default 0)
        self.erase_mode = DrcsEraseMode::from_param(params.get(2).copied().unwrap_or(0));

        // Pcmw: Character matrix width (default 0 means use terminal default)
        let pcmw = params.get(3).copied().unwrap_or(0);
        self.cell_width = if pcmw == 0 {
            DEFAULT_CELL_WIDTH
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let w = pcmw.min(u16::from(MAX_CELL_WIDTH)) as u8;
            w.max(1)
        };

        // Pss: Screen size selection (ignored - we support all sizes)
        // params.get(4)

        // Pt: Text/full cell (ignored - we always use full cell)
        // params.get(5)

        // Pcmh: Character matrix height (default 0 means use terminal default)
        let pcmh = params.get(6).copied().unwrap_or(0);
        self.cell_height = if pcmh == 0 {
            DEFAULT_CELL_HEIGHT
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let h = pcmh.min(u16::from(MAX_CELL_HEIGHT)) as u8;
            h.max(1)
        };

        // Pcss: Character set size (default 0 = 94-char)
        self.charset_size = DrcsCharsetSize::from_param(params.get(7).copied().unwrap_or(0));
    }

    /// Process a data byte.
    ///
    /// Returns a completed glyph when a character definition is complete.
    pub fn put(&mut self, byte: u8) -> Option<(u8, DrcsGlyph)> {
        match self.state {
            DecdldParseState::Initial => {
                // First byte should be Dscs (charset designator)
                // We ignore it for now, just transition to data
                self.state = DecdldParseState::Dscs;
                None
            }
            DecdldParseState::Dscs => {
                // After Dscs, we enter sixel data mode on any byte
                self.state = DecdldParseState::SixelData;
                self.process_sixel_byte(byte)
            }
            DecdldParseState::SixelData => self.process_sixel_byte(byte),
            DecdldParseState::Error => None,
        }
    }

    /// Process a sixel data byte.
    fn process_sixel_byte(&mut self, byte: u8) -> Option<(u8, DrcsGlyph)> {
        match byte {
            // Semicolon separates characters
            b';' => {
                let result = self.finish_current_glyph();
                self.current_char = self.current_char.saturating_add(1);
                self.current_row = 0;
                self.glyph_data.clear();
                result
            }
            // Slash moves to next row of sixels
            b'/' => {
                self.current_row = self.current_row.saturating_add(6); // Sixels are 6 pixels high
                None
            }
            // Sixel data bytes (0x3F-0x7E represent 6 vertical pixels)
            0x3F..=0x7E => {
                // Convert sixel byte to pixel data
                let sixel_value = byte - 0x3F;
                self.add_sixel_column(sixel_value);
                None
            }
            // Ignore other characters
            _ => None,
        }
    }

    /// Add a sixel column (6 vertical pixels) to the current glyph.
    fn add_sixel_column(&mut self, sixel: u8) {
        // Calculate the byte position and bit offset for this column
        let col = self.glyph_data.len() % usize::from(self.cell_width);
        let bytes_per_row = (usize::from(self.cell_width) + 7) / 8;

        // Sixel represents 6 vertical pixels
        for bit in 0u8..6 {
            if (sixel >> bit) & 1 == 1 {
                let row = usize::from(self.current_row) + usize::from(bit);
                if row >= usize::from(self.cell_height) {
                    continue;
                }

                let byte_idx = row * bytes_per_row + col / 8;
                let bit_idx = col % 8;

                // Ensure we have enough space
                while self.glyph_data.len() <= byte_idx {
                    self.glyph_data.push(0);
                }

                self.glyph_data[byte_idx] |= 1 << bit_idx;
            }
        }
    }

    /// Finish the current glyph and return it if valid.
    fn finish_current_glyph(&mut self) -> Option<(u8, DrcsGlyph)> {
        if self.glyph_data.is_empty() {
            return None;
        }

        // Pad to expected size
        let bytes_per_row = (usize::from(self.cell_width) + 7) / 8;
        let expected_size = bytes_per_row * usize::from(self.cell_height);
        self.glyph_data.resize(expected_size, 0);

        let glyph = DrcsGlyph::new(
            std::mem::take(&mut self.glyph_data),
            self.cell_width,
            self.cell_height,
        )?;

        Some((self.current_char, glyph))
    }

    /// Finalize parsing and return any remaining glyph.
    #[must_use]
    pub fn finish(&mut self) -> Option<(u8, DrcsGlyph)> {
        let result = self.finish_current_glyph();
        self.state = DecdldParseState::Initial;
        result
    }

    /// Get the font ID for storage.
    #[must_use]
    pub fn font_id(&self) -> DrcsFontId {
        DrcsFontId::new(self.font_buffer, self.start_char)
    }

    /// Check if parser is in an error state.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.state == DecdldParseState::Error
    }

    /// Reset the parser state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drcs_glyph_creation() {
        // 8x8 glyph = 1 byte per row * 8 rows = 8 bytes
        let bitmap = vec![0xFF; 8];
        let glyph = DrcsGlyph::new(bitmap, 8, 8);
        assert!(glyph.is_some());
        let glyph = glyph.unwrap();
        assert_eq!(glyph.width(), 8);
        assert_eq!(glyph.height(), 8);
    }

    #[test]
    fn test_drcs_glyph_invalid_size() {
        // Too wide
        let bitmap = vec![0; 100];
        assert!(DrcsGlyph::new(bitmap, 20, 10).is_none());

        // Too tall
        let bitmap = vec![0; 100];
        assert!(DrcsGlyph::new(bitmap, 10, 30).is_none());

        // Zero width
        let bitmap = vec![];
        assert!(DrcsGlyph::new(bitmap, 0, 10).is_none());
    }

    #[test]
    fn test_drcs_glyph_pixel_access() {
        // 8x2 glyph: first row all set, second row all clear
        let bitmap = vec![0xFF, 0x00];
        let glyph = DrcsGlyph::new(bitmap, 8, 2).unwrap();

        // First row should be set
        for x in 0..8 {
            assert!(glyph.get_pixel(x, 0));
        }

        // Second row should be clear
        for x in 0..8 {
            assert!(!glyph.get_pixel(x, 1));
        }

        // Out of bounds should return false
        assert!(!glyph.get_pixel(8, 0));
        assert!(!glyph.get_pixel(0, 2));
    }

    #[test]
    fn test_drcs_font_operations() {
        let id = DrcsFontId::new(0, 0x20);
        let mut font = DrcsFont::new(id, 10, 20, DrcsCharsetSize::Charset94);

        assert_eq!(font.glyph_count(), 0);
        assert!(!font.has_glyph(0));

        // Add a glyph
        let glyph = DrcsGlyph::empty(10, 20).unwrap();
        assert!(font.set_glyph(0, glyph));
        assert_eq!(font.glyph_count(), 1);
        assert!(font.has_glyph(0));

        // Try to add glyph beyond range
        let glyph = DrcsGlyph::empty(10, 20).unwrap();
        assert!(!font.set_glyph(100, glyph)); // Beyond 94 chars
    }

    #[test]
    fn test_drcs_storage() {
        let mut storage = DrcsStorage::new();
        assert!(storage.is_empty());

        let id = DrcsFontId::new(0, 0x20);
        let font = storage.get_or_create_font(id, 10, 20, DrcsCharsetSize::Charset94);
        let glyph = DrcsGlyph::empty(10, 20).unwrap();
        font.set_glyph(0, glyph);

        assert_eq!(storage.font_count(), 1);
        assert!(storage.get_glyph(id, 0).is_some());
        assert!(storage.get_glyph(id, 1).is_none());

        // Erase all fonts
        storage.erase(DrcsEraseMode::EraseAllFonts, None);
        assert!(storage.is_empty());
    }

    #[test]
    fn test_decdld_parser_init() {
        let mut parser = DecdldParser::new();

        // Test with all defaults
        parser.init(&[]);
        assert_eq!(parser.font_buffer, 0);
        assert_eq!(parser.start_char, DRCS_START_CHAR);
        assert_eq!(parser.erase_mode, DrcsEraseMode::EraseAll);
        assert_eq!(parser.cell_width, DEFAULT_CELL_WIDTH);
        assert_eq!(parser.cell_height, DEFAULT_CELL_HEIGHT);

        // Test with specific values
        parser.init(&[1, 0x21, 1, 8, 0, 0, 16, 1]);
        assert_eq!(parser.font_buffer, 1);
        assert_eq!(parser.start_char, 0x21);
        assert_eq!(parser.erase_mode, DrcsEraseMode::EraseLoaded);
        assert_eq!(parser.cell_width, 8);
        assert_eq!(parser.cell_height, 16);
        assert_eq!(parser.charset_size, DrcsCharsetSize::Charset96);
    }

    #[test]
    fn test_decdld_parser_simple_glyph() {
        let mut parser = DecdldParser::new();
        parser.init(&[0, 0, 0, 8, 0, 0, 6, 0]); // 8x6 glyph

        // Dscs byte (ignored)
        parser.put(b' ');

        // Sixel data for a simple pattern
        // Each sixel byte represents 6 vertical pixels
        // 0x7F = all 6 bits set = 0b00111111 + 0x3F
        parser.put(0x7F); // Column 0, all set
        parser.put(0x40); // Column 1, only bottom bit set

        // End glyph
        let result = parser.put(b';');
        assert!(result.is_some());
        let (char_code, glyph) = result.unwrap();
        assert_eq!(char_code, 0);
        assert_eq!(glyph.width(), 8);
        assert_eq!(glyph.height(), 6);
    }

    #[test]
    fn test_drcs_font_id() {
        let id1 = DrcsFontId::new(0, 0x20);
        let id2 = DrcsFontId::new(0, 0x20);
        let id3 = DrcsFontId::new(1, 0x20);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert_eq!(DrcsFontId::DEFAULT, DrcsFontId::new(0, DRCS_START_CHAR));
    }

    #[test]
    fn test_charset_size() {
        assert_eq!(DrcsCharsetSize::Charset94.max_chars(), 94);
        assert_eq!(DrcsCharsetSize::Charset96.max_chars(), 96);

        assert_eq!(DrcsCharsetSize::from_param(0), DrcsCharsetSize::Charset94);
        assert_eq!(DrcsCharsetSize::from_param(1), DrcsCharsetSize::Charset96);
        assert_eq!(DrcsCharsetSize::from_param(99), DrcsCharsetSize::Charset94);
    }
}

// Kani verification proofs
#[cfg(kani)]
mod verification {
    use super::*;

    /// Verify that glyph count is bounded per font.
    #[kani::proof]
    #[kani::unwind(100)]
    fn drcs_glyph_count_bounded() {
        let id = DrcsFontId::new(0, DRCS_START_CHAR);
        let mut font = DrcsFont::new(id, 10, 20, DrcsCharsetSize::Charset96);

        // Add maximum number of glyphs
        for i in 0..MAX_GLYPHS_PER_FONT {
            let glyph = DrcsGlyph::empty(10, 20).unwrap();
            #[allow(clippy::cast_possible_truncation)]
            let _ = font.set_glyph(i as u8, glyph);
        }

        // Glyph count should never exceed MAX_GLYPHS_PER_FONT
        kani::assert(
            font.glyph_count() <= MAX_GLYPHS_PER_FONT,
            "Glyph count exceeds maximum",
        );
    }

    /// Verify character code validation.
    #[kani::proof]
    fn drcs_char_code_valid() {
        let char_code: u8 = kani::any();
        let id = DrcsFontId::new(0, DRCS_START_CHAR);
        let mut font = DrcsFont::new(id, 10, 20, DrcsCharsetSize::Charset94);

        if let Some(glyph) = DrcsGlyph::empty(10, 20) {
            let success = font.set_glyph(char_code, glyph);

            // set_glyph should fail for codes >= 94 (for Charset94)
            if char_code >= 94 {
                kani::assert(!success, "Should reject char codes >= 94");
            }
        }
    }

    /// Verify glyph dimensions are bounded.
    #[kani::proof]
    fn drcs_glyph_dimensions_bounded() {
        let width: u8 = kani::any();
        let height: u8 = kani::any();

        let result = DrcsGlyph::empty(width, height);

        if let Some(glyph) = result {
            kani::assert(glyph.width() >= 1, "Width must be at least 1");
            kani::assert(glyph.width() <= MAX_CELL_WIDTH, "Width must not exceed max");
            kani::assert(glyph.height() >= 1, "Height must be at least 1");
            kani::assert(
                glyph.height() <= MAX_CELL_HEIGHT,
                "Height must not exceed max",
            );
        } else {
            // Should fail for invalid dimensions
            kani::assert(
                width == 0 || width > MAX_CELL_WIDTH || height == 0 || height > MAX_CELL_HEIGHT,
                "Should only fail for invalid dimensions",
            );
        }
    }

    /// Verify pixel access is bounds-checked.
    #[kani::proof]
    fn drcs_pixel_access_safe() {
        let x: u8 = kani::any();
        let y: u8 = kani::any();

        // Create a small 4x4 glyph
        if let Some(glyph) = DrcsGlyph::empty(4, 4) {
            let pixel = glyph.get_pixel(x, y);

            // Out of bounds should always return false
            if x >= 4 || y >= 4 {
                kani::assert(!pixel, "Out of bounds pixel should be false");
            }
        }
    }
}
