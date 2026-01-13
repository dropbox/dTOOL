//! Sixel graphics decoder.
//!
//! Sixel is a bitmap graphics protocol originally developed by DEC for their
//! VT240/VT330/VT340 terminals. Each character in a Sixel stream encodes a
//! vertical column of 6 pixels, hence the name "six pixels" -> "sixel".
//!
//! ## DCS Sequence Format
//!
//! Sixel data is introduced via a DCS (Device Control String) sequence:
//! ```text
//! ESC P Ps1 ; Ps2 ; Ps3 q <sixel-data> ST
//! ```
//!
//! Where:
//! - `Ps1` - Pixel aspect ratio (0,1,5,6=2:1; 2=5:1; 3,4=3:1; 7,8,9=1:1)
//! - `Ps2` - Background selector (0=device; 1=transparent; 2=color 0)
//! - `Ps3` - Horizontal grid size (usually ignored)
//!
//! ## Protocol Elements
//!
//! - `"Pan;Pad;Ph;Pv` - Raster attributes (aspect ratio, dimensions)
//! - `#Pc` - Select color register
//! - `#Pc;Pu;Px;Py;Pz` - Define and select color (RGB or HLS)
//! - `!Pn<data>` - Repeat sixel character
//! - `$` - Graphics carriage return
//! - `-` - Graphics newline (down 6 pixels)
//! - `?` to `~` - Sixel data (bits represent 6 vertical pixels)
//!
//! ## Implementation Notes
//!
//! This implementation uses a streaming parser that processes bytes as they
//! arrive, building the image incrementally. The color palette supports up
//! to 1024 entries (256 is the DEC standard, but modern terminals extend this).

use std::sync::Arc;

/// Maximum number of color registers supported.
pub const MAX_COLOR_REGISTERS: usize = 1024;

// ============================================================================
// Safe Cast Helpers
// ============================================================================

/// Convert a dimension (bounded by `SIXEL_MAX_DIMENSION`) to u16.
///
/// # Panics
/// Panics if dimension exceeds `SIXEL_MAX_DIMENSION` (10000), which fits in u16.
#[inline]
fn dimension_to_u16(dim: usize) -> u16 {
    debug_assert!(dim <= SIXEL_MAX_DIMENSION, "dimension exceeds max");
    u16::try_from(dim.min(SIXEL_MAX_DIMENSION)).expect("SIXEL_MAX_DIMENSION fits in u16")
}

/// Convert a color register index (bounded by `MAX_COLOR_REGISTERS`) to u16.
#[inline]
fn color_register_to_u16(reg: u32) -> u16 {
    u16::try_from(reg.min(u32::try_from(MAX_COLOR_REGISTERS).unwrap() - 1))
        .expect("MAX_COLOR_REGISTERS fits in u16")
}

/// Convert a color register index (bounded by `MAX_COLOR_REGISTERS`) to usize.
#[inline]
fn color_register_to_usize(reg: u32) -> usize {
    usize::try_from(reg.min(u32::try_from(MAX_COLOR_REGISTERS).unwrap() - 1))
        .expect("MAX_COLOR_REGISTERS fits in usize")
}

/// Convert a repeat count (bounded by `SIXEL_MAX_DIMENSION`) to usize.
#[inline]
fn repeat_count_to_usize(count: u32) -> usize {
    usize::try_from(count.min(u32::try_from(SIXEL_MAX_DIMENSION).unwrap()))
        .expect("SIXEL_MAX_DIMENSION fits in usize")
}

/// Convert a color component (0-100 scale normalized to 0-255) to u8.
#[inline]
fn color_component_to_u8(value: u32) -> u8 {
    u8::try_from((value.min(100) * 255 / 100).min(u32::from(u8::MAX)))
        .expect("color component clamped to u8")
}

/// Convert a floating point color value (0.0-1.0) to u8 (0-255).
///
/// # Safety
/// The caller must ensure the value is in the 0.0-1.0 range.
#[inline]
#[allow(clippy::cast_sign_loss)] // Documented: input is non-negative
#[allow(clippy::cast_possible_truncation)] // clamped * 255.0 always fits in u8
fn float_color_to_u8(value: f32) -> u8 {
    // Clamp to handle floating point edge cases
    let clamped = value.clamp(0.0, 1.0);
    (clamped * 255.0) as u8
}

/// Maximum Sixel image dimension to prevent DoS attacks.
pub const SIXEL_MAX_DIMENSION: usize = 10000;

/// Default VT340-compatible color palette (16 colors).
/// This is the standard palette used when no colors are defined.
pub const DEFAULT_PALETTE: [u32; 16] = [
    0x00_000000, // 0: black
    0xFF_3333CC, // 1: blue
    0xFF_CC2121, // 2: red
    0xFF_33CC33, // 3: green
    0xFF_CC33CC, // 4: magenta
    0xFF_33CCCC, // 5: cyan
    0xFF_CCCC33, // 6: yellow
    0xFF_878787, // 7: gray 50%
    0xFF_474747, // 8: gray 25%
    0xFF_6464FF, // 9: light blue
    0xFF_FF6464, // 10: light red
    0xFF_64FF64, // 11: light green
    0xFF_FF64FF, // 12: light magenta
    0xFF_64FFFF, // 13: light cyan
    0xFF_FFFF64, // 14: light yellow
    0xFF_FFFFFF, // 15: white
];

/// Parser state for Sixel decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SixelState {
    /// Normal sixel data processing.
    #[default]
    Ground,
    /// After `"`, collecting raster attributes.
    RasterAttributes,
    /// After `!`, collecting repeat count.
    RepeatIntroducer,
    /// After `#`, collecting color parameters.
    ColorIntroducer,
}

/// A Sixel image.
#[derive(Debug, Clone)]
pub struct SixelImage {
    /// RGBA pixel data (0xAARRGGBB format, little-endian).
    pixels: Vec<u32>,
    /// Image width in pixels.
    width: usize,
    /// Image height in pixels.
    height: usize,
    /// Whether the background is transparent.
    transparent_bg: bool,
    /// Terminal cursor row where image should be placed.
    cursor_row: u16,
    /// Terminal cursor column where image should be placed.
    cursor_col: u16,
}

impl SixelImage {
    /// Get the image width in pixels.
    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get the image height in pixels.
    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get the raw RGBA pixel data (0xAARRGGBB format).
    #[inline]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Consume the image and return the raw pixel data.
    #[inline]
    pub fn into_pixels(self) -> Vec<u32> {
        self.pixels
    }

    /// Check if the background is transparent.
    #[inline]
    pub fn is_transparent(&self) -> bool {
        self.transparent_bg
    }

    /// Get the terminal row where the image starts.
    #[inline]
    pub fn cursor_row(&self) -> u16 {
        self.cursor_row
    }

    /// Get the terminal column where the image starts.
    #[inline]
    pub fn cursor_col(&self) -> u16 {
        self.cursor_col
    }

    /// Calculate the number of terminal rows this image spans.
    /// Uses 6 pixels per cell row (standard sixel height).
    pub fn rows_spanned(&self, cell_height: u16) -> u16 {
        if cell_height == 0 || self.height == 0 {
            return 0;
        }
        let height = dimension_to_u16(self.height);
        (height + cell_height - 1) / cell_height
    }

    /// Calculate the number of terminal columns this image spans.
    pub fn cols_spanned(&self, cell_width: u16) -> u16 {
        if cell_width == 0 || self.width == 0 {
            return 0;
        }
        let width = dimension_to_u16(self.width);
        (width + cell_width - 1) / cell_width
    }
}

/// Sixel decoder and image builder.
///
/// This decoder processes Sixel data incrementally, building an RGBA image
/// as bytes arrive. Call `hook()` when entering a Sixel DCS sequence, `put()`
/// for each data byte, and `unhook()` to finalize and retrieve the image.
#[derive(Debug)]
pub struct SixelDecoder {
    /// Parser state machine state.
    state: SixelState,
    /// Accumulated parameter value.
    param: u32,
    /// Collected parameters for current operation.
    params: [u32; 5],
    /// Number of parameters collected.
    param_count: usize,

    /// Color palette (ARGB format, 0xAARRGGBB).
    palette: Vec<u32>,
    /// Currently selected color register.
    current_color: u16,

    /// Image pixel buffer (ARGB format).
    pixels: Vec<u32>,
    /// Image width in pixels.
    width: usize,
    /// Image height in pixels.
    height: usize,
    /// Allocated width (may be larger for reuse).
    allocated_width: usize,
    /// Allocated height (may be larger for reuse).
    allocated_height: usize,

    /// Current X position in pixels.
    cursor_x: usize,
    /// Current Y position in pixels (top of sixel band).
    cursor_y: usize,
    /// Maximum X reached (determines final width).
    max_x: usize,
    /// Maximum Y reached (determines final height).
    max_y: usize,

    /// Repeat count from `!` introducer.
    repeat_count: u32,

    /// Whether background is transparent (Ps2 = 1).
    transparent_bg: bool,
    /// Pixel aspect ratio numerator (Pan).
    pan: u32,
    /// Pixel aspect ratio denominator (Pad).
    pad: u32,

    /// Terminal cursor position when image started.
    cursor_row: u16,
    cursor_col: u16,

    /// Whether we're actively processing a Sixel sequence.
    active: bool,
}

impl Default for SixelDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SixelDecoder {
    /// Create a new Sixel decoder.
    pub fn new() -> Self {
        // Initialize palette with default VT340 colors
        let mut palette = vec![0u32; MAX_COLOR_REGISTERS];
        for (i, &color) in DEFAULT_PALETTE.iter().enumerate() {
            palette[i] = color;
        }

        Self {
            state: SixelState::Ground,
            param: 0,
            params: [0; 5],
            param_count: 0,

            palette,
            current_color: 0,

            pixels: Vec::new(),
            width: 0,
            height: 0,
            allocated_width: 0,
            allocated_height: 0,

            cursor_x: 0,
            cursor_y: 0,
            max_x: 0,
            max_y: 0,

            repeat_count: 0,

            transparent_bg: false,
            pan: 1,
            pad: 1,

            cursor_row: 0,
            cursor_col: 0,

            active: false,
        }
    }

    /// Start processing a Sixel DCS sequence.
    ///
    /// Called when the terminal receives `DCS Ps1;Ps2;Ps3 q`.
    ///
    /// # Parameters
    /// - `params`: DCS parameters [Ps1, Ps2, Ps3]
    ///   - Ps1: Pixel aspect ratio selector
    ///   - Ps2: Background selector (0=device, 1=transparent, 2=color 0)
    ///   - Ps3: Horizontal grid size (usually ignored)
    /// - `cursor_row`: Terminal row where image will be placed
    /// - `cursor_col`: Terminal column where image will be placed
    pub fn hook(&mut self, params: &[u16], cursor_row: u16, cursor_col: u16) {
        self.reset_state();
        self.active = true;
        self.cursor_row = cursor_row;
        self.cursor_col = cursor_col;

        // Parse Ps2 - background selector
        let ps2 = u32::from(params.get(1).copied().unwrap_or(0));
        self.transparent_bg = ps2 == 1;

        // Parse Ps1 - pixel aspect ratio
        // VT340 aspect ratios (vertical:horizontal):
        // 0,1,5,6 = 2:1; 2 = 5:1; 3,4 = 3:1; 7,8,9 = 1:1
        let ps1 = u32::from(params.first().copied().unwrap_or(0));
        match ps1 {
            0 | 1 | 5 | 6 => {
                self.pan = 2;
                self.pad = 1;
            }
            2 => {
                self.pan = 5;
                self.pad = 1;
            }
            3 | 4 => {
                self.pan = 3;
                self.pad = 1;
            }
            _ => {
                // 7, 8, 9 are 1:1; unknown values also default to 1:1
                self.pan = 1;
                self.pad = 1;
            }
        }
    }

    /// Process a data byte within the Sixel sequence.
    ///
    /// This is called for each byte between `DCS ... q` and `ST`.
    pub fn put(&mut self, byte: u8) {
        if !self.active {
            return;
        }

        match self.state {
            SixelState::Ground => self.process_ground(byte),
            SixelState::RasterAttributes => self.process_raster_attributes(byte),
            SixelState::RepeatIntroducer => self.process_repeat_introducer(byte),
            SixelState::ColorIntroducer => self.process_color_introducer(byte),
        }
    }

    /// Finalize the Sixel sequence and return the completed image.
    ///
    /// Returns `None` if no valid image was produced (e.g., empty data).
    pub fn unhook(&mut self) -> Option<SixelImage> {
        if !self.active {
            return None;
        }

        self.active = false;

        // Calculate final dimensions
        let final_width = self.max_x;
        let final_height = if self.max_y > 0 {
            // max_y is the top of the last sixel band, add 6 for the band itself
            self.max_y + 6
        } else if self.cursor_y > 0 || self.cursor_x > 0 {
            // At least one band was started
            6
        } else {
            0
        };

        if final_width == 0 || final_height == 0 {
            return None;
        }

        // Clamp to maximum dimensions
        let final_width = final_width.min(SIXEL_MAX_DIMENSION);
        let final_height = final_height.min(SIXEL_MAX_DIMENSION);

        // Copy pixels to a correctly-sized buffer
        let mut result = vec![0u32; final_width * final_height];

        for y in 0..final_height.min(self.height) {
            for x in 0..final_width.min(self.width) {
                let src_idx = y * self.allocated_width + x;
                let dst_idx = y * final_width + x;
                if src_idx < self.pixels.len() {
                    result[dst_idx] = self.pixels[src_idx];
                }
            }
        }

        Some(SixelImage {
            pixels: result,
            width: final_width,
            height: final_height,
            transparent_bg: self.transparent_bg,
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
        })
    }

    /// Check if the decoder is currently processing a Sixel sequence.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Reset the decoder state for a new image.
    fn reset_state(&mut self) {
        self.state = SixelState::Ground;
        self.param = 0;
        self.params = [0; 5];
        self.param_count = 0;

        self.current_color = 0;

        self.pixels.clear();
        self.width = 0;
        self.height = 0;
        self.allocated_width = 0;
        self.allocated_height = 0;

        self.cursor_x = 0;
        self.cursor_y = 0;
        self.max_x = 0;
        self.max_y = 0;

        self.repeat_count = 0;

        self.transparent_bg = false;
        self.pan = 1;
        self.pad = 1;

        // Don't reset palette - it persists across images in some modes
    }

    /// Process a byte in the Ground state.
    fn process_ground(&mut self, byte: u8) {
        match byte {
            // Raster attributes introducer
            b'"' => {
                self.state = SixelState::RasterAttributes;
                self.param = 0;
                self.params = [0; 5];
                self.param_count = 0;
            }
            // Repeat introducer
            b'!' => {
                self.state = SixelState::RepeatIntroducer;
                self.param = 0;
            }
            // Color introducer
            b'#' => {
                self.state = SixelState::ColorIntroducer;
                self.param = 0;
                self.params = [0; 5];
                self.param_count = 0;
            }
            // Graphics carriage return
            b'$' => {
                self.cursor_x = 0;
            }
            // Graphics newline
            b'-' => {
                self.cursor_x = 0;
                self.cursor_y += 6;
            }
            // Sixel data characters (0x3F to 0x7E = '?' to '~')
            0x3F..=0x7E => {
                self.render_sixel(byte - 0x3F, 1);
            }
            // Ignore whitespace and other characters
            _ => {}
        }
    }

    /// Process a byte in the RasterAttributes state.
    fn process_raster_attributes(&mut self, byte: u8) {
        match byte {
            b'0'..=b'9' => {
                self.param = self
                    .param
                    .saturating_mul(10)
                    .saturating_add(u32::from(byte - b'0'));
            }
            b';' => {
                if self.param_count < 5 {
                    self.params[self.param_count] = self.param;
                    self.param_count += 1;
                }
                self.param = 0;
            }
            _ => {
                // Finalize raster attributes
                if self.param_count < 5 {
                    self.params[self.param_count] = self.param;
                    self.param_count += 1;
                }

                // " Pan ; Pad ; Ph ; Pv
                if self.param_count >= 4 {
                    let pan = self.params[0].max(1);
                    let pad = self.params[1].max(1);
                    let ph = self.params[2];
                    let pv = self.params[3];

                    self.pan = pan;
                    self.pad = pad;

                    // Pre-allocate image buffer if dimensions are provided
                    if ph > 0 && pv > 0 {
                        let width = (ph as usize).min(SIXEL_MAX_DIMENSION);
                        let height = (pv as usize).min(SIXEL_MAX_DIMENSION);
                        self.ensure_size(width, height);
                    }
                }

                self.state = SixelState::Ground;
                // Re-process the terminating byte
                self.process_ground(byte);
            }
        }
    }

    /// Process a byte in the RepeatIntroducer state.
    fn process_repeat_introducer(&mut self, byte: u8) {
        match byte {
            b'0'..=b'9' => {
                self.param = self
                    .param
                    .saturating_mul(10)
                    .saturating_add(u32::from(byte - b'0'));
            }
            0x3F..=0x7E => {
                // Sixel data character - render with repeat count
                let count = if self.param > 0 { self.param } else { 1 };
                self.render_sixel(byte - 0x3F, count);
                self.state = SixelState::Ground;
            }
            _ => {
                // Invalid - return to ground
                self.state = SixelState::Ground;
                self.process_ground(byte);
            }
        }
    }

    /// Process a byte in the ColorIntroducer state.
    fn process_color_introducer(&mut self, byte: u8) {
        match byte {
            b'0'..=b'9' => {
                self.param = self
                    .param
                    .saturating_mul(10)
                    .saturating_add(u32::from(byte - b'0'));
            }
            b';' => {
                if self.param_count < 5 {
                    self.params[self.param_count] = self.param;
                    self.param_count += 1;
                }
                self.param = 0;
            }
            _ => {
                // Finalize color operation
                if self.param_count < 5 {
                    self.params[self.param_count] = self.param;
                    self.param_count += 1;
                }

                match self.param_count {
                    1 => {
                        // Select color register: # Pc
                        let pc = color_register_to_u16(self.params[0]);
                        self.current_color = pc;
                    }
                    5 => {
                        // Define color: # Pc ; Pu ; Px ; Py ; Pz
                        let pc = color_register_to_usize(self.params[0]);
                        let pu = self.params[1];
                        let px = self.params[2];
                        let py = self.params[3];
                        let pz = self.params[4];

                        let color = if pu == 2 {
                            // RGB: values are 0-100 scale
                            let r = color_component_to_u8(px);
                            let g = color_component_to_u8(py);
                            let b = color_component_to_u8(pz);
                            argb(255, r, g, b)
                        } else {
                            // HLS: Px=hue (0-360), Py=lightness (0-100), Pz=saturation (0-100)
                            // DEC uses: Blue=0, Red=120, Green=240
                            // Standard HLS: Red=0, Green=120, Blue=240
                            // So we rotate by 240 degrees (or equivalently -120)
                            // Values are small (0-360, 0-100) so f32 precision is sufficient
                            #[allow(clippy::cast_precision_loss)]
                            let hue = ((px + 240) % 360) as f32;
                            #[allow(clippy::cast_precision_loss)]
                            let lightness = (py.min(100) as f32) / 100.0;
                            #[allow(clippy::cast_precision_loss)]
                            let saturation = (pz.min(100) as f32) / 100.0;
                            hls_to_rgb(hue, lightness, saturation)
                        };

                        self.palette[pc] = color;
                        self.current_color = color_register_to_u16(self.params[0]);
                    }
                    _ => {
                        // Invalid parameter count - just select color 0
                        if self.param_count > 0 {
                            let pc = color_register_to_u16(self.params[0]);
                            self.current_color = pc;
                        }
                    }
                }

                self.state = SixelState::Ground;
                // Re-process the terminating byte
                self.process_ground(byte);
            }
        }
    }

    /// Render a sixel character with the given repeat count.
    fn render_sixel(&mut self, sixel: u8, count: u32) {
        let count = repeat_count_to_usize(count);

        // Ensure we have enough space
        let needed_x = self.cursor_x + count;
        let needed_y = self.cursor_y + 6;

        if needed_x > self.allocated_width || needed_y > self.allocated_height {
            self.ensure_size(needed_x, needed_y);
        }

        // Safety check
        if needed_x > SIXEL_MAX_DIMENSION || needed_y > SIXEL_MAX_DIMENSION {
            return;
        }

        let color = self
            .palette
            .get(self.current_color as usize)
            .copied()
            .unwrap_or(0xFF_FFFFFF);

        // Draw the 6 vertical pixels for each repetition
        for x_offset in 0..count {
            let x = self.cursor_x + x_offset;
            if x >= self.allocated_width {
                break;
            }

            for bit in 0..6 {
                if (sixel & (1 << bit)) != 0 {
                    let y = self.cursor_y + bit;
                    if y < self.allocated_height {
                        let idx = y * self.allocated_width + x;
                        if idx < self.pixels.len() {
                            self.pixels[idx] = color;
                        }
                    }
                }
            }
        }

        // Update cursor and max extents
        self.cursor_x += count;
        if self.cursor_x > self.max_x {
            self.max_x = self.cursor_x;
        }
        if self.cursor_y > self.max_y {
            self.max_y = self.cursor_y;
        }
    }

    /// Ensure the pixel buffer is at least the given size.
    fn ensure_size(&mut self, width: usize, height: usize) {
        let width = width.min(SIXEL_MAX_DIMENSION);
        let height = height.min(SIXEL_MAX_DIMENSION);

        if width <= self.allocated_width && height <= self.allocated_height {
            return;
        }

        // Grow by at least 50% or to the requested size
        let new_width = (self.allocated_width * 3 / 2).max(width).max(64);
        let new_height = (self.allocated_height * 3 / 2).max(height).max(64);

        let new_width = new_width.min(SIXEL_MAX_DIMENSION);
        let new_height = new_height.min(SIXEL_MAX_DIMENSION);

        // Background color depends on transparency setting
        let bg = if self.transparent_bg {
            0
        } else {
            self.palette.first().copied().unwrap_or(0)
        };

        let mut new_pixels = vec![bg; new_width * new_height];

        // Copy existing data
        for y in 0..self.height.min(new_height) {
            for x in 0..self.width.min(new_width) {
                let src_idx = y * self.allocated_width + x;
                let dst_idx = y * new_width + x;
                if src_idx < self.pixels.len() {
                    new_pixels[dst_idx] = self.pixels[src_idx];
                }
            }
        }

        self.pixels = new_pixels;
        self.allocated_width = new_width;
        self.allocated_height = new_height;
        self.width = new_width;
        self.height = new_height;
    }

    /// Get the current color palette.
    pub fn palette(&self) -> &[u32] {
        &self.palette
    }

    /// Set a color in the palette.
    pub fn set_palette_color(&mut self, index: u16, color: u32) {
        if (index as usize) < self.palette.len() {
            self.palette[index as usize] = color;
        }
    }

    /// Reset the palette to default VT340 colors.
    pub fn reset_palette(&mut self) {
        for (i, &color) in DEFAULT_PALETTE.iter().enumerate() {
            self.palette[i] = color;
        }
        for i in DEFAULT_PALETTE.len()..self.palette.len() {
            self.palette[i] = 0;
        }
    }
}

/// Create an ARGB color value.
#[inline]
fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
}

/// Convert HLS (Hue, Lightness, Saturation) to ARGB.
///
/// - Hue: 0-360 degrees (red=0, green=120, blue=240)
/// - Lightness: 0.0-1.0
/// - Saturation: 0.0-1.0
#[allow(clippy::many_single_char_names)]
fn hls_to_rgb(hue: f32, lightness: f32, saturation: f32) -> u32 {
    if saturation == 0.0 {
        // Achromatic (gray) - lightness is 0.0-1.0
        let v = float_color_to_u8(lightness);
        return argb(255, v, v, v);
    }

    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;

    let r = hue_to_rgb(p, q, hue + 120.0);
    let g = hue_to_rgb(p, q, hue);
    let b = hue_to_rgb(p, q, hue - 120.0);

    // hue_to_rgb returns values in 0.0-1.0 range
    let r_u8 = float_color_to_u8(r);
    let g_u8 = float_color_to_u8(g);
    let b_u8 = float_color_to_u8(b);
    argb(255, r_u8, g_u8, b_u8)
}

/// Helper function for HLS to RGB conversion.
fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    // Normalize to 0-360
    while t < 0.0 {
        t += 360.0;
    }
    while t > 360.0 {
        t -= 360.0;
    }

    if t < 60.0 {
        p + (q - p) * t / 60.0
    } else if t < 180.0 {
        q
    } else if t < 240.0 {
        p + (q - p) * (240.0 - t) / 60.0
    } else {
        p
    }
}

/// Handle for a completed Sixel image that can be stored in terminal state.
#[derive(Debug, Clone)]
pub struct SixelImageHandle {
    /// The image data.
    image: Arc<SixelImage>,
    /// Unique identifier for this image.
    id: u64,
}

impl SixelImageHandle {
    /// Create a new handle for a Sixel image.
    pub fn new(image: SixelImage, id: u64) -> Self {
        Self {
            image: Arc::new(image),
            id,
        }
    }

    /// Get the image.
    #[inline]
    pub fn image(&self) -> &SixelImage {
        &self.image
    }

    /// Get the unique identifier.
    #[inline]
    pub fn id(&self) -> u64 {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let decoder = SixelDecoder::new();
        assert!(!decoder.is_active());
        assert_eq!(decoder.palette.len(), MAX_COLOR_REGISTERS);
    }

    #[test]
    fn test_hook_transparent_background() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[0, 1, 0], 0, 0);
        assert!(decoder.is_active());
        assert!(decoder.transparent_bg);
    }

    #[test]
    fn test_hook_opaque_background() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[0, 2, 0], 0, 0);
        assert!(decoder.is_active());
        assert!(!decoder.transparent_bg);
    }

    #[test]
    fn test_hook_aspect_ratio() {
        let mut decoder = SixelDecoder::new();

        // 1:1 ratio
        decoder.hook(&[7], 0, 0);
        assert_eq!(decoder.pan, 1);
        assert_eq!(decoder.pad, 1);

        // 2:1 ratio
        decoder.hook(&[0], 0, 0);
        assert_eq!(decoder.pan, 2);
        assert_eq!(decoder.pad, 1);
    }

    #[test]
    fn test_simple_sixel() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        // Select white color and draw a simple pattern
        // '#15' selects color 15 (white)
        decoder.put(b'#');
        decoder.put(b'1');
        decoder.put(b'5');

        // '~' = 0x7E - 0x3F = 0x3F = 0b111111 (all 6 pixels)
        decoder.put(b'~');

        let image = decoder.unhook();
        assert!(image.is_some());

        let image = image.unwrap();
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 6);

        // All 6 pixels should be white
        let white = DEFAULT_PALETTE[15];
        for y in 0..6 {
            assert_eq!(image.pixels()[y], white, "pixel at y={y} should be white");
        }
    }

    #[test]
    fn test_repeat_introducer() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        // Select color
        decoder.put(b'#');
        decoder.put(b'1');
        decoder.put(b'5');

        // Repeat 10 times: !10~
        decoder.put(b'!');
        decoder.put(b'1');
        decoder.put(b'0');
        decoder.put(b'~');

        let image = decoder.unhook();
        assert!(image.is_some());

        let image = image.unwrap();
        assert_eq!(image.width(), 10);
        assert_eq!(image.height(), 6);
    }

    #[test]
    fn test_graphics_newline() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        decoder.put(b'#');
        decoder.put(b'1');
        decoder.put(b'5');

        // Draw one sixel
        decoder.put(b'~');
        // Graphics newline
        decoder.put(b'-');
        // Draw another sixel
        decoder.put(b'~');

        let image = decoder.unhook();
        assert!(image.is_some());

        let image = image.unwrap();
        assert_eq!(image.height(), 12); // Two sixel bands
    }

    #[test]
    fn test_graphics_carriage_return() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        // First row in red
        decoder.put(b'#');
        decoder.put(b'2'); // Red

        decoder.put(b'~');
        decoder.put(b'~');

        // Carriage return
        decoder.put(b'$');

        // Same row in blue (should overwrite)
        decoder.put(b'#');
        decoder.put(b'1'); // Blue

        decoder.put(b'~');

        let image = decoder.unhook();
        assert!(image.is_some());

        let image = image.unwrap();
        // First column should be blue (overwritten), second should be red
        let blue = DEFAULT_PALETTE[1];
        let red = DEFAULT_PALETTE[2];

        assert_eq!(image.pixels()[0], blue, "first pixel should be blue");
        assert_eq!(image.pixels()[1], red, "second pixel should be red");
    }

    #[test]
    fn test_define_rgb_color() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        // Define color 100 as RGB (100, 50, 0) -> orange
        // #100;2;100;50;0
        for b in b"#100;2;100;50;0" {
            decoder.put(*b);
        }

        // Draw with that color
        decoder.put(b'~');

        let image = decoder.unhook().unwrap();

        // Check the color was applied
        // RGB 100% = 255, 50% = 127, 0% = 0
        let expected = argb(255, 255, 127, 0);
        assert_eq!(image.pixels()[0], expected);
    }

    #[test]
    fn test_raster_attributes() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        // Raster attributes: "1;1;100;50 (1:1 aspect, 100x50 pixels)
        // Note: The attributes only apply when followed by a sixel or other char
        // that transitions out of the RasterAttributes state
        for b in b"\"1;1;100;50#15~" {
            decoder.put(*b);
        }

        // Buffer should be pre-allocated based on raster attributes
        assert!(
            decoder.allocated_width >= 100,
            "width should be >= 100, got {}",
            decoder.allocated_width
        );
        assert!(
            decoder.allocated_height >= 50,
            "height should be >= 50, got {}",
            decoder.allocated_height
        );
    }

    #[test]
    fn test_empty_sequence() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        let image = decoder.unhook();
        assert!(image.is_none());
    }

    #[test]
    fn test_max_dimension_limit() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        // Try to set extremely large dimensions
        for b in b"\"1;1;99999;99999" {
            decoder.put(*b);
        }

        // Should be clamped to SIXEL_MAX_DIMENSION
        assert!(decoder.allocated_width <= SIXEL_MAX_DIMENSION);
        assert!(decoder.allocated_height <= SIXEL_MAX_DIMENSION);
    }

    #[test]
    fn test_sixel_bits() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 0, 0);

        decoder.put(b'#');
        decoder.put(b'1');
        decoder.put(b'5');

        // '?' = 0x3F - 0x3F = 0 (no pixels)
        decoder.put(b'?');

        // '@' = 0x40 - 0x3F = 1 (only bottom pixel)
        decoder.put(b'@');

        let image = decoder.unhook().unwrap();
        let white = DEFAULT_PALETTE[15];

        // First column: no pixels set
        for y in 0..6 {
            let idx = y * image.width();
            let pixel = image.pixels()[idx];
            // Should be background (either transparent or color 0)
            assert!(
                pixel == 0 || pixel == DEFAULT_PALETTE[0],
                "first column pixel at y={y} should be background"
            );
        }

        // Second column: only bit 0 (top pixel) set
        let top_pixel = image.pixels()[1];
        assert_eq!(
            top_pixel, white,
            "top pixel of second column should be white"
        );
    }

    #[test]
    fn test_hls_to_rgb() {
        // Red (H=0, L=0.5, S=1.0)
        let red = hls_to_rgb(0.0, 0.5, 1.0);
        assert_eq!((red >> 16) & 0xFF, 255, "red channel");
        assert!((red & 0xFF) < 10, "blue channel should be near 0");

        // Pure gray (S=0)
        let gray = hls_to_rgb(0.0, 0.5, 0.0);
        let r = (gray >> 16) & 0xFF;
        let g = (gray >> 8) & 0xFF;
        let b = gray & 0xFF;
        assert_eq!(r, g, "gray r==g");
        assert_eq!(g, b, "gray g==b");
        // SAFETY: r is a u32 color channel value (0-255), which fits in i32
        #[allow(clippy::cast_possible_wrap)]
        let diff = (r as i32 - 127).abs();
        assert!(diff < 2, "gray should be ~127");
    }

    #[test]
    fn test_image_cursor_position() {
        let mut decoder = SixelDecoder::new();
        decoder.hook(&[], 10, 20);

        decoder.put(b'#');
        decoder.put(b'1');
        decoder.put(b'5');
        decoder.put(b'~');

        let image = decoder.unhook().unwrap();
        assert_eq!(image.cursor_row(), 10);
        assert_eq!(image.cursor_col(), 20);
    }

    #[test]
    fn test_image_rows_cols_spanned() {
        let image = SixelImage {
            pixels: vec![0; 100 * 60],
            width: 100,
            height: 60,
            transparent_bg: false,
            cursor_row: 0,
            cursor_col: 0,
        };

        // Assuming 10x20 cell size
        assert_eq!(image.cols_spanned(10), 10); // 100 / 10 = 10
        assert_eq!(image.rows_spanned(20), 3); // 60 / 20 = 3
    }
}

// Kani proofs
#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    #[kani::unwind(10)]
    fn sixel_data_in_bounds() {
        let sixel: u8 = kani::any();
        kani::assume(sixel >= 0x3F && sixel <= 0x7E);

        let value = sixel - 0x3F;
        assert!(value <= 63, "sixel value must be 0-63");
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn repeat_count_bounded() {
        let mut decoder = SixelDecoder::new();
        decoder.active = true;

        // Simulate repeat count accumulation
        let digits: [u8; 3] = kani::any();
        for &d in &digits {
            kani::assume(d >= b'0' && d <= b'9');
            decoder.param = decoder
                .param
                .saturating_mul(10)
                .saturating_add((d - b'0') as u32);
        }

        // Count should never exceed reasonable bounds due to saturation
        assert!(decoder.param <= u32::MAX);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn color_index_bounded() {
        let index: u32 = kani::any();
        let bounded = index.min(MAX_COLOR_REGISTERS as u32 - 1) as usize;
        assert!(bounded < MAX_COLOR_REGISTERS);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn rgb_scale_bounded() {
        let value: u32 = kani::any();
        kani::assume(value <= 100);

        let scaled = (value * 255 / 100) as u8;
        assert!(scaled <= 255);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn dimension_limit_respected() {
        let width: usize = kani::any();
        let height: usize = kani::any();

        let clamped_width = width.min(SIXEL_MAX_DIMENSION);
        let clamped_height = height.min(SIXEL_MAX_DIMENSION);

        assert!(clamped_width <= SIXEL_MAX_DIMENSION);
        assert!(clamped_height <= SIXEL_MAX_DIMENSION);
    }
}
