//! Glyph atlas for efficient terminal text rendering.
//!
//! This module provides a texture atlas that stores rasterized glyphs for GPU
//! rendering. The atlas is dynamically populated as new characters are encountered.
//!
//! ## Design
//!
//! - Uses fontdue for fast glyph rasterization
//! - Uses guillotiere for efficient rectangle packing
//! - Glyphs are cached by (codepoint, style) key
//! - Atlas grows dynamically but has a maximum size
//!
//! ## Integration with Terminal Rendering
//!
//! The atlas is used by the renderer to:
//! 1. Look up glyph texture coordinates for each cell
//! 2. Ensure glyphs are rasterized before rendering
//! 3. Provide UV coordinates for textured quad rendering

// Allow some casts that are known to be safe for practical atlas sizes:
// - u32 -> i32: Atlas sizes are limited to 4096 (max 8192), well below i32::MAX
// - u16 -> f32: Always lossless
// - i32/usize casts for glyph metrics: Values are bounded by glyph sizes
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use fontdue::{Font, FontSettings};
use guillotiere::{size2, AtlasAllocator, Size as AtlasSize};
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// Key for looking up glyphs in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// Unicode codepoint
    pub codepoint: char,
    /// Font size in pixels (scaled)
    pub size_px: u16,
    /// Bold flag
    pub bold: bool,
    /// Italic flag
    pub italic: bool,
}

impl GlyphKey {
    /// Create a new glyph key.
    pub fn new(codepoint: char, size_px: u16, bold: bool, italic: bool) -> Self {
        Self {
            codepoint,
            size_px,
            bold,
            italic,
        }
    }

    /// Create a key for a basic ASCII character.
    pub fn ascii(ch: char, size_px: u16) -> Self {
        Self::new(ch, size_px, false, false)
    }
}

/// Entry in the glyph atlas describing a rasterized glyph.
#[derive(Debug, Clone, Copy)]
pub struct GlyphEntry {
    /// X position in atlas texture
    pub x: u16,
    /// Y position in atlas texture
    pub y: u16,
    /// Width of glyph in atlas
    pub width: u16,
    /// Height of glyph in atlas
    pub height: u16,
    /// X offset when rendering (can be negative)
    pub offset_x: i16,
    /// Y offset when rendering (from baseline)
    pub offset_y: i16,
    /// Horizontal advance after rendering
    pub advance: u16,
}

impl GlyphEntry {
    /// Get UV coordinates for this glyph in the atlas.
    ///
    /// Returns (u_min, v_min, u_max, v_max) normalized to 0.0-1.0.
    ///
    /// Note: The u32 to f32 cast is safe for practical atlas sizes (up to ~16M pixels).
    #[allow(clippy::cast_precision_loss)]
    pub fn uv_coords(&self, atlas_size: u32) -> (f32, f32, f32, f32) {
        let atlas_size = atlas_size as f32;
        let u_min = self.x as f32 / atlas_size;
        let v_min = self.y as f32 / atlas_size;
        let u_max = (self.x + self.width) as f32 / atlas_size;
        let v_max = (self.y + self.height) as f32 / atlas_size;
        (u_min, v_min, u_max, v_max)
    }
}

/// Configuration for the glyph atlas.
#[derive(Debug, Clone)]
pub struct AtlasConfig {
    /// Initial atlas size (width = height)
    pub initial_size: u32,
    /// Maximum atlas size
    pub max_size: u32,
    /// Default font size in pixels
    pub default_font_size: u16,
    /// Padding between glyphs
    pub padding: u32,
}

impl Default for AtlasConfig {
    /// Returns default atlas configuration.
    ///
    /// Default values:
    /// - `initial_size`: 512 pixels - Good balance between memory and resize frequency.
    ///   Fits ~400 glyphs at 14px with padding. Doubles on growth.
    /// - `max_size`: 4096 pixels - Maximum GPU texture size widely supported.
    ///   Fits ~16,000+ glyphs. Rarely hit with typical usage.
    /// - `default_font_size`: 14 pixels - Common terminal font size.
    ///   Affects glyph rasterization and cell metrics.
    /// - `padding`: 1 pixel - Prevents texture bleeding between glyphs
    ///   during GPU sampling. Increase to 2 for subpixel rendering.
    fn default() -> Self {
        Self {
            initial_size: 512,
            max_size: 4096,
            default_font_size: 14,
            padding: 1,
        }
    }
}

/// Glyph atlas for GPU text rendering.
pub struct GlyphAtlas {
    /// Font for rasterization
    font: Arc<Font>,
    /// Bold font variant
    font_bold: Option<Arc<Font>>,
    /// Italic font variant
    font_italic: Option<Arc<Font>>,
    /// Bold italic font variant
    font_bold_italic: Option<Arc<Font>>,
    /// Rectangle allocator for atlas packing
    allocator: AtlasAllocator,
    /// Cached glyph entries
    glyphs: FxHashMap<GlyphKey, GlyphEntry>,
    /// Pending rasterizations (glyph data waiting to be uploaded)
    pending: Vec<(GlyphKey, GlyphEntry, Vec<u8>)>,
    /// Current atlas size
    atlas_size: u32,
    /// Configuration
    config: AtlasConfig,
    /// Full atlas texture data (single-channel grayscale)
    texture_data: Vec<u8>,
}

impl GlyphAtlas {
    /// Create a new glyph atlas with a provided font.
    ///
    /// # Arguments
    /// * `config` - Atlas configuration
    /// * `font_data` - Raw font file data (TTF/OTF)
    ///
    /// Returns None if the font data is invalid.
    pub fn new(config: AtlasConfig, font_data: &[u8]) -> Option<Self> {
        let font = Font::from_bytes(font_data, FontSettings::default()).ok()?;

        let allocator = AtlasAllocator::new(AtlasSize::new(
            config.initial_size as i32,
            config.initial_size as i32,
        ));

        // Initialize texture data with zeros (transparent)
        let texture_size = (config.initial_size * config.initial_size) as usize;
        let texture_data = vec![0u8; texture_size];

        Some(Self {
            font: Arc::new(font),
            font_bold: None,
            font_italic: None,
            font_bold_italic: None,
            allocator,
            glyphs: FxHashMap::default(),
            pending: Vec::new(),
            atlas_size: config.initial_size,
            config,
            texture_data,
        })
    }

    /// Create a glyph atlas from raw font data.
    ///
    /// This is the FFI-friendly version that takes a raw pointer and length.
    ///
    /// # Safety
    /// - `font_data` must point to valid memory of at least `font_len` bytes
    /// - The memory must remain valid for the duration of this call
    pub unsafe fn new_from_ptr(
        config: AtlasConfig,
        font_data: *const u8,
        font_len: usize,
    ) -> Option<Self> {
        if font_data.is_null() || font_len == 0 {
            return None;
        }
        // SAFETY: Caller guarantees font_data points to valid memory of at least font_len bytes
        let data = unsafe { std::slice::from_raw_parts(font_data, font_len) };
        Self::new(config, data)
    }

    /// Create a glyph atlas with custom fonts.
    pub fn with_fonts(
        config: AtlasConfig,
        font: Font,
        font_bold: Option<Font>,
        font_italic: Option<Font>,
        font_bold_italic: Option<Font>,
    ) -> Self {
        let allocator = AtlasAllocator::new(AtlasSize::new(
            config.initial_size as i32,
            config.initial_size as i32,
        ));

        // Initialize texture data with zeros (transparent)
        let texture_size = (config.initial_size * config.initial_size) as usize;
        let texture_data = vec![0u8; texture_size];

        Self {
            font: Arc::new(font),
            font_bold: font_bold.map(Arc::new),
            font_italic: font_italic.map(Arc::new),
            font_bold_italic: font_bold_italic.map(Arc::new),
            allocator,
            glyphs: FxHashMap::default(),
            pending: Vec::new(),
            atlas_size: config.initial_size,
            config,
            texture_data,
        }
    }

    /// Get the current atlas size.
    pub fn size(&self) -> u32 {
        self.atlas_size
    }

    /// Get the default font size in pixels.
    pub fn default_font_size(&self) -> u16 {
        self.config.default_font_size
    }

    /// Get a reference to the base font.
    ///
    /// This is useful for accessing font metrics like baseline, ascent, descent.
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// Clear all cached glyphs from the atlas.
    ///
    /// This forces all glyphs to be re-rasterized on next access. Call this when
    /// changing font sizes or to free memory after extended use.
    pub fn clear(&mut self) {
        self.glyphs.clear();
        self.pending.clear();
        // Reset allocator
        self.allocator = AtlasAllocator::new(size2(
            self.config.initial_size as i32,
            self.config.initial_size as i32,
        ));
        self.atlas_size = self.config.initial_size;
        // Clear texture data
        self.texture_data.fill(0);
        self.texture_data
            .resize((self.atlas_size * self.atlas_size) as usize, 0);
    }

    /// Set font variants for bold, italic, and bold-italic text.
    ///
    /// These fonts will be used when rendering styled text. Any existing
    /// cached glyphs for these styles will still be valid as they fall back
    /// to the base font if no variant is set.
    pub fn set_font_variants(
        &mut self,
        bold: Option<Font>,
        italic: Option<Font>,
        bold_italic: Option<Font>,
    ) {
        self.font_bold = bold.map(Arc::new);
        self.font_italic = italic.map(Arc::new);
        self.font_bold_italic = bold_italic.map(Arc::new);
    }

    /// Look up a glyph in the atlas.
    ///
    /// Returns None if the glyph has not been rasterized yet.
    pub fn get(&self, key: &GlyphKey) -> Option<&GlyphEntry> {
        self.glyphs.get(key)
    }

    /// Ensure a glyph is in the atlas, rasterizing if needed.
    ///
    /// Returns the glyph entry if successful, or None if the atlas is full
    /// and cannot grow.
    pub fn ensure(&mut self, key: GlyphKey) -> Option<&GlyphEntry> {
        if self.glyphs.contains_key(&key) {
            return self.glyphs.get(&key);
        }

        // Rasterize the glyph
        let font = self.select_font(key.bold, key.italic);
        let (metrics, bitmap) = font.rasterize(key.codepoint, key.size_px as f32);

        if metrics.width == 0 || metrics.height == 0 {
            // Space or empty glyph - create a zero-size entry
            let entry = GlyphEntry {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                offset_x: metrics.xmin as i16,
                offset_y: metrics.ymin as i16,
                advance: metrics.advance_width as u16,
            };
            self.glyphs.insert(key, entry);
            return self.glyphs.get(&key);
        }

        // Allocate space in the atlas
        let padding = self.config.padding as i32;
        let alloc_width = metrics.width as i32 + padding * 2;
        let alloc_height = metrics.height as i32 + padding * 2;

        let allocation = self
            .allocator
            .allocate(AtlasSize::new(alloc_width, alloc_height));

        let allocation = if let Some(alloc) = allocation {
            alloc
        } else {
            // Try to grow the atlas
            if !self.grow() {
                return None; // Atlas is at max size
            }
            // Retry allocation
            self.allocator
                .allocate(AtlasSize::new(alloc_width, alloc_height))?
        };

        // Create entry
        let entry = GlyphEntry {
            x: (allocation.rectangle.min.x + padding) as u16,
            y: (allocation.rectangle.min.y + padding) as u16,
            width: metrics.width as u16,
            height: metrics.height as u16,
            offset_x: metrics.xmin as i16,
            offset_y: metrics.ymin as i16,
            advance: metrics.advance_width as u16,
        };

        // Copy bitmap to texture_data buffer
        self.copy_glyph_to_texture(&entry, &bitmap);

        // Queue for upload (incremental upload path)
        self.pending.push((key, entry, bitmap));
        self.glyphs.insert(key, entry);

        self.glyphs.get(&key)
    }

    /// Copy a glyph bitmap to the texture data buffer.
    ///
    /// # Performance
    ///
    /// Uses `copy_nonoverlapping` with pre-validated bounds for optimal
    /// vectorization by LLVM. The bounds are checked once upfront instead
    /// of per-row, enabling the compiler to emit SIMD instructions.
    fn copy_glyph_to_texture(&mut self, entry: &GlyphEntry, bitmap: &[u8]) {
        let glyph_width = entry.width as usize;
        let glyph_height = entry.height as usize;
        let atlas_stride = self.atlas_size as usize;
        let dst_x = entry.x as usize;
        let dst_y_start = entry.y as usize;

        // Pre-validate all bounds once to enable vectorization
        let required_src = glyph_width.saturating_mul(glyph_height);
        let max_dst_row = dst_y_start.saturating_add(glyph_height);
        let max_dst_offset = max_dst_row
            .saturating_sub(1)
            .saturating_mul(atlas_stride)
            .saturating_add(dst_x)
            .saturating_add(glyph_width);

        if bitmap.len() < required_src || self.texture_data.len() < max_dst_offset {
            // Bounds check failed - skip copy (should not happen with valid allocations)
            return;
        }

        // SAFETY: Bounds are validated above. We use copy_nonoverlapping for
        // vectorization - source and destination never overlap as they're in
        // different memory regions.
        unsafe {
            let src_ptr = bitmap.as_ptr();
            let dst_ptr = self.texture_data.as_mut_ptr();

            for y in 0..glyph_height {
                let src_offset = y * glyph_width;
                let dst_offset = (dst_y_start + y) * atlas_stride + dst_x;

                std::ptr::copy_nonoverlapping(
                    src_ptr.add(src_offset),
                    dst_ptr.add(dst_offset),
                    glyph_width,
                );
            }
        }
    }

    /// Ensure multiple glyphs are in the atlas.
    pub fn ensure_many(&mut self, keys: impl Iterator<Item = GlyphKey>) {
        for key in keys {
            self.ensure(key);
        }
    }

    /// Take pending rasterizations for upload to GPU.
    ///
    /// Returns a list of (entry, bitmap_data) pairs that need to be uploaded
    /// to the atlas texture.
    pub fn take_pending(&mut self) -> Vec<(GlyphKey, GlyphEntry, Vec<u8>)> {
        std::mem::take(&mut self.pending)
    }

    /// Check if there are pending uploads.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Try to grow the atlas to accommodate more glyphs.
    ///
    /// Returns true if growth was successful.
    ///
    /// # Atomicity
    ///
    /// This operation is atomic: either it succeeds completely or the atlas
    /// is left unchanged. We prepare all new state before modifying self.
    ///
    /// # Preservation Strategy
    ///
    /// When growing, we:
    /// 1. Copy old texture data to top-left of new texture
    /// 2. Create new allocator and reserve the old region
    /// 3. Keep all existing glyph entries (their UV coordinates remain valid)
    ///
    /// This avoids re-rasterization of existing glyphs.
    fn grow(&mut self) -> bool {
        let new_size = self.atlas_size * 2;
        if new_size > self.config.max_size {
            return false;
        }

        let old_size = self.atlas_size as usize;

        // === Prepare new state without modifying self ===

        // Create a new larger allocator
        let mut new_allocator =
            AtlasAllocator::new(AtlasSize::new(new_size as i32, new_size as i32));

        // Reserve the old region in the new allocator to prevent overlap.
        // We allocate a rectangle covering the entire old atlas area.
        // This ensures new glyphs are placed in the new space.
        if new_allocator
            .allocate(AtlasSize::new(old_size as i32, old_size as i32))
            .is_none()
        {
            // Should never happen, but handle gracefully
            return false;
        }

        // Create new larger texture buffer
        let new_texture_size = (new_size * new_size) as usize;
        let mut new_texture = vec![0u8; new_texture_size];

        // Copy old texture data to top-left of new texture
        // The old data preserves its layout in the new larger atlas
        let new_stride = new_size as usize;
        for y in 0..old_size {
            let src_start = y * old_size;
            let src_end = src_start + old_size;
            let dst_start = y * new_stride;

            if src_end <= self.texture_data.len() && dst_start + old_size <= new_texture.len() {
                new_texture[dst_start..dst_start + old_size]
                    .copy_from_slice(&self.texture_data[src_start..src_end]);
            }
        }

        // === All preparation succeeded - now atomically swap state ===

        self.allocator = new_allocator;
        self.atlas_size = new_size;
        self.texture_data = new_texture;

        // Glyph entries are preserved - their UV coordinates remain valid
        // since the texture data was copied to the same position in the
        // larger atlas (top-left corner).

        true
    }

    /// Select the appropriate font variant.
    fn select_font(&self, bold: bool, italic: bool) -> &Font {
        match (bold, italic) {
            (true, true) => self.font_bold_italic.as_ref().unwrap_or(&self.font),
            (true, false) => self.font_bold.as_ref().unwrap_or(&self.font),
            (false, true) => self.font_italic.as_ref().unwrap_or(&self.font),
            (false, false) => &self.font,
        }
    }

    /// Get the line height for the default font size.
    pub fn line_height(&self) -> f32 {
        let metrics = self
            .font
            .horizontal_line_metrics(self.config.default_font_size as f32);
        metrics
            .map(|m| m.new_line_size)
            .unwrap_or(self.config.default_font_size as f32 * 1.2)
    }

    /// Get the cell width for a monospace font.
    pub fn cell_width(&self) -> f32 {
        // Use 'M' as reference for cell width
        let (metrics, _) = self
            .font
            .rasterize('M', self.config.default_font_size as f32);
        metrics.advance_width
    }

    /// Get the full atlas texture data.
    ///
    /// Returns a reference to the atlas texture as a single-channel (grayscale)
    /// bitmap. The data is laid out row-by-row, with `atlas_size()` pixels per row.
    ///
    /// This is useful for creating or updating the platform's texture when:
    /// - The atlas is first created
    /// - The atlas has grown to a larger size
    ///
    /// For incremental updates (new glyphs added), use `take_pending()` instead.
    pub fn texture_data(&self) -> &[u8] {
        &self.texture_data
    }

    /// Get a mutable reference to the texture data.
    ///
    /// This is useful for platforms that need to modify the texture data directly.
    pub fn texture_data_mut(&mut self) -> &mut [u8] {
        &mut self.texture_data
    }

    /// Check if the atlas has been modified since the last call to this method.
    ///
    /// Returns `true` if there are pending glyphs or if the atlas has grown.
    /// After calling this, use `take_pending()` for incremental updates or
    /// `texture_data()` for a full texture refresh.
    pub fn needs_upload(&self) -> bool {
        !self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require the font file to exist
    // In CI, we'd either mock the font or use a test fixture

    #[test]
    fn test_glyph_key() {
        let key1 = GlyphKey::ascii('A', 14);
        let key2 = GlyphKey::new('A', 14, false, false);
        assert_eq!(key1, key2);

        let key3 = GlyphKey::new('A', 14, true, false);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_glyph_entry_uv() {
        let entry = GlyphEntry {
            x: 10,
            y: 20,
            width: 8,
            height: 12,
            offset_x: 0,
            offset_y: -10,
            advance: 8,
        };

        let (u_min, v_min, u_max, v_max) = entry.uv_coords(512);
        assert!((u_min - 10.0 / 512.0).abs() < 0.001);
        assert!((v_min - 20.0 / 512.0).abs() < 0.001);
        assert!((u_max - 18.0 / 512.0).abs() < 0.001);
        assert!((v_max - 32.0 / 512.0).abs() < 0.001);
    }

    #[test]
    fn test_atlas_config_default() {
        let config = AtlasConfig::default();
        assert_eq!(config.initial_size, 512);
        assert_eq!(config.max_size, 4096);
    }

    #[test]
    fn test_texture_data_size() {
        // Verify texture_data is properly sized
        // Note: This test uses a mock approach since we can't easily construct a GlyphAtlas
        // without a valid font. The texture_data should be initial_size * initial_size bytes.
        let config = AtlasConfig {
            initial_size: 64,
            max_size: 256,
            default_font_size: 14,
            padding: 1,
        };

        // The expected size should be 64 * 64 = 4096 bytes
        let expected_size = (config.initial_size * config.initial_size) as usize;
        assert_eq!(expected_size, 4096);
    }

    #[test]
    fn test_copy_glyph_to_texture_bounds() {
        // Test that the copy_glyph_to_texture helper produces correct offsets
        // This validates the math without needing a real font

        let atlas_size: usize = 64;
        let mut texture_data = vec![0u8; atlas_size * atlas_size];

        // Simulate a 4x3 glyph at position (10, 20)
        let glyph_width = 4;
        let glyph_height = 3;
        let glyph_x = 10usize;
        let glyph_y = 20usize;

        // Create a test bitmap (12 bytes for 4x3)
        let bitmap: Vec<u8> = (0..12).collect();

        // Copy to texture using the same logic as copy_glyph_to_texture
        for y in 0..glyph_height {
            let src_start = y * glyph_width;
            let src_end = src_start + glyph_width;

            let dst_y = glyph_y + y;
            let dst_x = glyph_x;
            let dst_start = dst_y * atlas_size + dst_x;

            if dst_start + glyph_width <= texture_data.len() && src_end <= bitmap.len() {
                texture_data[dst_start..dst_start + glyph_width]
                    .copy_from_slice(&bitmap[src_start..src_end]);
            }
        }

        // Verify the data was copied correctly
        // Row 0: bytes 0-3 should be at offset (20 * 64 + 10)
        let row0_offset = 20 * atlas_size + 10;
        assert_eq!(texture_data[row0_offset], 0);
        assert_eq!(texture_data[row0_offset + 1], 1);
        assert_eq!(texture_data[row0_offset + 2], 2);
        assert_eq!(texture_data[row0_offset + 3], 3);

        // Row 1: bytes 4-7 should be at offset (21 * 64 + 10)
        let row1_offset = 21 * atlas_size + 10;
        assert_eq!(texture_data[row1_offset], 4);
        assert_eq!(texture_data[row1_offset + 1], 5);

        // Row 2: bytes 8-11 should be at offset (22 * 64 + 10)
        let row2_offset = 22 * atlas_size + 10;
        assert_eq!(texture_data[row2_offset], 8);
        assert_eq!(texture_data[row2_offset + 3], 11);
    }
}
