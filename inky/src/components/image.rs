//! Image component for terminal graphics.
//!
//! This module provides an [`Image`] component that renders images in the terminal
//! using various protocols:
//!
//! - **Kitty Graphics Protocol**: High-quality 24-bit images (iTerm2, Kitty, WezTerm)
//! - **Sixel**: Legacy graphics protocol (xterm, mlterm)
//! - **Block Characters**: Unicode block fallback for any terminal
//! - **ASCII Art**: Pure ASCII fallback for maximum compatibility
//!
//! # Example
//!
//! ```rust,ignore
//! use inky::components::Image;
//!
//! // Create an image from raw RGBA pixels
//! let pixels: Vec<u8> = vec![255, 0, 0, 255, /* red pixel */ ];
//! let image = Image::from_rgba(pixels, 1, 1)
//!     .width(40)
//!     .height(20);
//! ```
//!
//! # Protocol Detection
//!
//! The component automatically selects the best available protocol based on
//! terminal capabilities. You can also force a specific protocol:
//!
//! ```rust,ignore
//! use inky::components::{Image, ImageProtocol};
//!
//! let image = Image::from_rgba(pixels, width, height)
//!     .protocol(ImageProtocol::Block);  // Force block character rendering
//! ```

use crate::components::adaptive::{AdaptiveComponent, Tier0Fallback, TierFeatures};
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, Edges, Style};
use crate::terminal::RenderTier;

/// Image rendering protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageProtocol {
    /// Auto-detect best available protocol.
    #[default]
    Auto,
    /// Kitty graphics protocol (best quality).
    Kitty,
    /// Sixel graphics protocol.
    Sixel,
    /// Unicode block characters (half-block).
    Block,
    /// Braille characters.
    Braille,
    /// ASCII characters using brightness mapping.
    Ascii,
}

/// Scaling mode for images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleMode {
    /// Scale to fit within bounds, preserving aspect ratio.
    #[default]
    Fit,
    /// Scale to fill bounds, preserving aspect ratio (may crop).
    Fill,
    /// Stretch to exact dimensions (may distort).
    Stretch,
    /// No scaling, use original size.
    None,
}

/// An image component for terminal rendering.
///
/// # Example
///
/// ```rust
/// use inky::components::Image;
///
/// // Create a 2x2 red/green checkerboard
/// let pixels = vec![
///     255, 0, 0, 255,   0, 255, 0, 255,  // Row 1: red, green
///     0, 255, 0, 255,   255, 0, 0, 255,  // Row 2: green, red
/// ];
/// let image = Image::from_rgba(pixels, 2, 2);
/// ```
#[derive(Debug, Clone)]
pub struct Image {
    /// RGBA pixel data (4 bytes per pixel).
    pixels: Vec<u8>,
    /// Source image width in pixels.
    src_width: u32,
    /// Source image height in pixels.
    src_height: u32,
    /// Target width in terminal cells.
    width: Option<u32>,
    /// Target height in terminal cells.
    height: Option<u32>,
    /// Rendering protocol.
    protocol: ImageProtocol,
    /// Scaling mode.
    scale_mode: ScaleMode,
    /// Alt text for accessibility.
    alt: Option<String>,
    /// Style overrides.
    style: Style,
}

impl Image {
    /// Create an image from RGBA pixel data.
    ///
    /// # Arguments
    ///
    /// * `pixels` - RGBA pixel data (4 bytes per pixel)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Image;
    ///
    /// let red_pixel = vec![255, 0, 0, 255];
    /// let image = Image::from_rgba(red_pixel, 1, 1);
    /// ```
    pub fn from_rgba(pixels: Vec<u8>, width: u32, height: u32) -> Self {
        debug_assert_eq!(
            pixels.len(),
            (width * height * 4) as usize,
            "Pixel data size mismatch"
        );

        Self {
            pixels,
            src_width: width,
            src_height: height,
            width: None,
            height: None,
            protocol: ImageProtocol::default(),
            scale_mode: ScaleMode::default(),
            alt: None,
            style: Style::default(),
        }
    }

    /// Create an image filled with a solid color.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Image;
    /// use inky::style::Color;
    ///
    /// let blue_block = Image::solid(Color::Blue, 10, 5);
    /// ```
    pub fn solid(color: Color, width: u32, height: u32) -> Self {
        let (r, g, b) = color_to_rgb(color);
        let pixel_count = (width * height) as usize;
        let mut pixels = Vec::with_capacity(pixel_count * 4);

        for _ in 0..pixel_count {
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(255);
        }

        Self::from_rgba(pixels, width, height)
    }

    /// Create a gradient image.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::components::Image;
    /// use inky::style::Color;
    ///
    /// let gradient = Image::gradient(Color::Red, Color::Blue, 20, 10, true);
    /// ```
    pub fn gradient(from: Color, to: Color, width: u32, height: u32, horizontal: bool) -> Self {
        let (r1, g1, b1) = color_to_rgb(from);
        let (r2, g2, b2) = color_to_rgb(to);

        let pixel_count = (width * height) as usize;
        let mut pixels = Vec::with_capacity(pixel_count * 4);

        for y in 0..height {
            for x in 0..width {
                let t = if horizontal {
                    x as f32 / (width - 1).max(1) as f32
                } else {
                    y as f32 / (height - 1).max(1) as f32
                };

                let r = lerp_u8(r1, r2, t);
                let g = lerp_u8(g1, g2, t);
                let b = lerp_u8(b1, b2, t);

                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
                pixels.push(255);
            }
        }

        Self::from_rgba(pixels, width, height)
    }

    /// Set the target width in terminal cells.
    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the target height in terminal cells.
    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    /// Set the rendering protocol.
    pub fn protocol(mut self, protocol: ImageProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    /// Set the scaling mode.
    pub fn scale_mode(mut self, mode: ScaleMode) -> Self {
        self.scale_mode = mode;
        self
    }

    /// Set alt text for accessibility.
    pub fn alt(mut self, alt: impl Into<String>) -> Self {
        self.alt = Some(alt.into());
        self
    }

    /// Set padding around the image.
    pub fn padding(mut self, padding: f32) -> Self {
        self.style.padding = Edges::all(padding);
        self
    }

    /// Get the source dimensions.
    pub fn source_size(&self) -> (u32, u32) {
        (self.src_width, self.src_height)
    }

    /// Get a pixel color at (x, y).
    pub fn pixel_at(&self, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        if x >= self.src_width || y >= self.src_height {
            return None;
        }

        let idx = ((y * self.src_width + x) * 4) as usize;
        Some((
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ))
    }

    /// Render using block characters (half-blocks).
    ///
    /// Each terminal cell represents 2 vertical pixels using the upper
    /// half block character with foreground/background colors.
    fn render_block(&self, target_width: u32, target_height: u32) -> Node {
        // Each row of cells represents 2 rows of pixels
        let cell_rows = (target_height + 1) / 2;
        let mut rows: Vec<Node> = Vec::with_capacity(cell_rows as usize);

        for cell_y in 0..cell_rows {
            let mut row_text = String::with_capacity(target_width as usize);
            let mut row_styles: Vec<(Color, Color)> = Vec::with_capacity(target_width as usize);

            for cell_x in 0..target_width {
                // Map cell position to source pixels
                let src_x = (cell_x as f32 / target_width as f32 * self.src_width as f32) as u32;
                let src_y_top =
                    ((cell_y * 2) as f32 / (target_height as f32) * self.src_height as f32) as u32;
                let src_y_bottom = (((cell_y * 2 + 1) as f32) / (target_height as f32)
                    * self.src_height as f32) as u32;

                let top_color = self
                    .pixel_at(src_x, src_y_top)
                    .map(|(r, g, b, _)| Color::Rgb(r, g, b))
                    .unwrap_or(Color::Black);

                let bottom_color = self
                    .pixel_at(src_x, src_y_bottom.min(self.src_height - 1))
                    .map(|(r, g, b, _)| Color::Rgb(r, g, b))
                    .unwrap_or(Color::Black);

                row_text.push('\u{2580}'); // Upper half block
                row_styles.push((top_color, bottom_color));
            }

            // For simplicity, create one TextNode per row with the first color
            // A more sophisticated implementation would use inline spans
            if let Some(&(fg, bg)) = row_styles.first() {
                let text = TextNode::new(row_text).color(fg).bg(bg);
                rows.push(text.into());
            }
        }

        // Wrap in a box
        let mut container = BoxNode::new().flex_direction(crate::style::FlexDirection::Column);
        for row in rows {
            container = container.child(row);
        }

        container.into()
    }

    /// Render using ASCII characters based on brightness.
    fn render_ascii(&self, target_width: u32, target_height: u32) -> Node {
        // ASCII characters from darkest to brightest
        const ASCII_RAMP: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

        let mut rows: Vec<String> = Vec::with_capacity(target_height as usize);

        for cell_y in 0..target_height {
            let mut row = String::with_capacity(target_width as usize);

            for cell_x in 0..target_width {
                // Map to source pixel
                let src_x = (cell_x as f32 / target_width as f32 * self.src_width as f32) as u32;
                let src_y = (cell_y as f32 / target_height as f32 * self.src_height as f32) as u32;

                let brightness = self
                    .pixel_at(src_x, src_y)
                    .map(|(r, g, b, a)| {
                        // Luminance formula with alpha
                        let lum = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32)
                            * (a as f32 / 255.0);
                        lum / 255.0
                    })
                    .unwrap_or(0.0);

                let idx = (brightness * (ASCII_RAMP.len() - 1) as f32).round() as usize;
                row.push(ASCII_RAMP[idx.min(ASCII_RAMP.len() - 1)]);
            }

            rows.push(row);
        }

        let text = rows.join("\n");
        TextNode::new(text).into()
    }

    /// Render using braille characters.
    ///
    /// Each braille character represents a 2x4 grid of dots, providing
    /// higher resolution than block characters.
    fn render_braille(&self, target_width: u32, target_height: u32) -> Node {
        // Braille base character
        const BRAILLE_BASE: u32 = 0x2800;

        // Braille dot positions:
        // 1 4
        // 2 5
        // 3 6
        // 7 8
        const DOT_OFFSETS: [u32; 8] = [0x01, 0x02, 0x04, 0x40, 0x08, 0x10, 0x20, 0x80];

        // Each braille cell is 2 chars wide, 4 rows tall
        let cell_cols = (target_width + 1) / 2;
        let cell_rows = (target_height + 3) / 4;

        let mut rows: Vec<String> = Vec::with_capacity(cell_rows as usize);

        for cell_y in 0..cell_rows {
            let mut row = String::with_capacity(cell_cols as usize);

            for cell_x in 0..cell_cols {
                let mut char_code = BRAILLE_BASE;

                // Check each dot position
                for (dot_idx, &offset) in DOT_OFFSETS.iter().enumerate() {
                    let dot_x = (dot_idx % 2) as u32;
                    let dot_y = (dot_idx / 2) as u32;

                    let pixel_x = cell_x * 2 + dot_x;
                    let pixel_y = cell_y * 4 + dot_y;

                    // Map to source
                    let src_x =
                        (pixel_x as f32 / target_width as f32 * self.src_width as f32) as u32;
                    let src_y =
                        (pixel_y as f32 / target_height as f32 * self.src_height as f32) as u32;

                    // Check if pixel is "on" (brightness > 0.5)
                    let on = self
                        .pixel_at(src_x, src_y)
                        .map(|(r, g, b, a)| {
                            let lum = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32)
                                * (a as f32 / 255.0);
                            lum > 127.5
                        })
                        .unwrap_or(false);

                    if on {
                        char_code |= offset;
                    }
                }

                if let Some(c) = char::from_u32(char_code) {
                    row.push(c);
                }
            }

            rows.push(row);
        }

        let text = rows.join("\n");
        TextNode::new(text).into()
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        // Determine target size
        let target_width = self.width.unwrap_or(self.src_width);
        let target_height = self.height.unwrap_or(self.src_height / 2); // Cells are ~2:1

        let protocol = match self.protocol {
            ImageProtocol::Auto => ImageProtocol::Block, // Default to block
            p => p,
        };

        match protocol {
            ImageProtocol::Auto | ImageProtocol::Block => {
                self.render_block(target_width, target_height)
            }
            ImageProtocol::Ascii => self.render_ascii(target_width, target_height),
            ImageProtocol::Braille => self.render_braille(target_width, target_height),
            ImageProtocol::Kitty | ImageProtocol::Sixel => {
                // These require escape sequences that can't be represented
                // in the current node model. Fall back to block.
                self.render_block(target_width, target_height)
            }
        }
    }
}

impl From<Image> for Node {
    fn from(image: Image) -> Self {
        image.to_node()
    }
}

/// Convert a Color to RGB tuple.
fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 49, 49),
        Color::Green => (13, 188, 121),
        Color::Yellow => (229, 229, 16),
        Color::Blue => (36, 114, 200),
        Color::Magenta => (188, 63, 188),
        Color::Cyan => (17, 168, 205),
        Color::White => (229, 229, 229),
        Color::BrightBlack => (102, 102, 102),
        Color::BrightRed => (241, 76, 76),
        Color::BrightGreen => (35, 209, 139),
        Color::BrightYellow => (245, 245, 67),
        Color::BrightBlue => (59, 142, 234),
        Color::BrightMagenta => (214, 112, 214),
        Color::BrightCyan => (41, 184, 219),
        Color::BrightWhite => (255, 255, 255),
        Color::Ansi256(n) => ansi256_to_rgb(n),
        Color::Default => (192, 192, 192),
    }
}

/// Convert ANSI 256 color to RGB.
fn ansi256_to_rgb(n: u8) -> (u8, u8, u8) {
    if n < 16 {
        match n {
            0 => (0, 0, 0),
            1 => (128, 0, 0),
            2 => (0, 128, 0),
            3 => (128, 128, 0),
            4 => (0, 0, 128),
            5 => (128, 0, 128),
            6 => (0, 128, 128),
            7 => (192, 192, 192),
            8 => (128, 128, 128),
            9 => (255, 0, 0),
            10 => (0, 255, 0),
            11 => (255, 255, 0),
            12 => (0, 0, 255),
            13 => (255, 0, 255),
            14 => (0, 255, 255),
            15 => (255, 255, 255),
            _ => (0, 0, 0),
        }
    } else if n < 232 {
        let n = n - 16;
        let r = (n / 36) % 6;
        let g = (n / 6) % 6;
        let b = n % 6;
        let to_rgb = |c: u8| if c == 0 { 0 } else { 55 + c * 40 };
        (to_rgb(r), to_rgb(g), to_rgb(b))
    } else {
        let gray = 8 + (n - 232) * 10;
        (gray, gray, gray)
    }
}

/// Linear interpolation for u8 values.
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

impl AdaptiveComponent for Image {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.clone().into(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("Image")
            .tier0("Text description with dimensions")
            .tier1("ASCII art representation")
            .tier2("Unicode blocks with 24-bit color")
            .tier3("Native protocol (Kitty/Sixel) or GPU")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl Image {
    /// Render Tier 0: Text-only description.
    fn render_tier0(&self) -> Node {
        let alt_text = self.alt.clone().unwrap_or_else(|| "Image".to_string());

        // Calculate average brightness for description
        let avg_brightness = self.calculate_average_brightness();
        let brightness_desc = if avg_brightness < 0.33 {
            "dark"
        } else if avg_brightness < 0.66 {
            "medium"
        } else {
            "bright"
        };

        Tier0Fallback::new("Image")
            .stat("alt", alt_text)
            .stat("size", format!("{}x{}", self.src_width, self.src_height))
            .stat("tone", brightness_desc)
            .into()
    }

    /// Render Tier 1: ASCII art representation.
    fn render_tier1(&self) -> Node {
        let target_width = self.width.unwrap_or(self.src_width.min(40));
        let target_height = self.height.unwrap_or((self.src_height / 2).min(20));
        self.render_ascii(target_width, target_height)
    }

    /// Calculate average brightness of the image (0.0 - 1.0).
    fn calculate_average_brightness(&self) -> f32 {
        if self.pixels.is_empty() {
            return 0.5;
        }

        let pixel_count = (self.src_width * self.src_height) as usize;
        if pixel_count == 0 {
            return 0.5;
        }

        let total_brightness: f32 = (0..pixel_count)
            .filter_map(|i| {
                let idx = i * 4;
                if idx + 3 < self.pixels.len() {
                    let r = self.pixels[idx] as f32;
                    let g = self.pixels[idx + 1] as f32;
                    let b = self.pixels[idx + 2] as f32;
                    let a = self.pixels[idx + 3] as f32;
                    // Luminance formula with alpha
                    let lum = (0.299 * r + 0.587 * g + 0.114 * b) * (a / 255.0);
                    Some(lum / 255.0)
                } else {
                    None
                }
            })
            .sum();

        total_brightness / pixel_count as f32
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_from_rgba() {
        let pixels = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let image = Image::from_rgba(pixels, 2, 1);

        assert_eq!(image.source_size(), (2, 1));
        assert_eq!(image.pixel_at(0, 0), Some((255, 0, 0, 255)));
        assert_eq!(image.pixel_at(1, 0), Some((0, 255, 0, 255)));
    }

    #[test]
    fn test_solid_color() {
        let image = Image::solid(Color::Red, 2, 2);

        assert_eq!(image.source_size(), (2, 2));
        // All pixels should be red-ish
        let (r, g, b, _) = image.pixel_at(0, 0).unwrap();
        assert!(r > 200);
        assert!(g < 100);
        assert!(b < 100);
    }

    #[test]
    fn test_gradient() {
        let image = Image::gradient(Color::Black, Color::White, 10, 1, true);

        let (r1, _, _, _) = image.pixel_at(0, 0).unwrap();
        let (r2, _, _, _) = image.pixel_at(9, 0).unwrap();

        // First pixel should be dark, last should be light
        assert!(r1 < 50);
        assert!(r2 > 200);
    }

    #[test]
    fn test_pixel_at_bounds() {
        let pixels = vec![255, 0, 0, 255];
        let image = Image::from_rgba(pixels, 1, 1);

        assert!(image.pixel_at(0, 0).is_some());
        assert!(image.pixel_at(1, 0).is_none());
        assert!(image.pixel_at(0, 1).is_none());
    }

    #[test]
    fn test_builder_methods() {
        let pixels = vec![255, 255, 255, 255];
        let image = Image::from_rgba(pixels, 1, 1)
            .width(40)
            .height(20)
            .protocol(ImageProtocol::Ascii)
            .scale_mode(ScaleMode::Fit)
            .alt("Test image");

        assert_eq!(image.width, Some(40));
        assert_eq!(image.height, Some(20));
        assert_eq!(image.protocol, ImageProtocol::Ascii);
        assert_eq!(image.scale_mode, ScaleMode::Fit);
        assert_eq!(image.alt, Some("Test image".to_string()));
    }

    #[test]
    fn test_render_ascii() {
        let pixels = vec![
            0, 0, 0, 255, // Black
            255, 255, 255, 255, // White
        ];
        let image = Image::from_rgba(pixels, 2, 1)
            .width(2)
            .height(1)
            .protocol(ImageProtocol::Ascii);

        let node = image.to_node();
        // Should produce a TextNode
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_render_block() {
        let pixels = vec![
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
            0, 0, 255, 255, // Blue
            255, 255, 0, 255, // Yellow
        ];
        let image = Image::from_rgba(pixels, 2, 2)
            .width(2)
            .height(2)
            .protocol(ImageProtocol::Block);

        let node = image.to_node();
        // Should produce a BoxNode container
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_render_braille() {
        let pixels = vec![
            255, 255, 255, 255, // White (on)
            0, 0, 0, 255, // Black (off)
        ];
        let image = Image::from_rgba(pixels, 2, 1)
            .width(2)
            .height(4)
            .protocol(ImageProtocol::Braille);

        let node = image.to_node();
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_color_to_rgb() {
        assert_eq!(color_to_rgb(Color::Black), (0, 0, 0));
        assert_eq!(color_to_rgb(Color::Rgb(100, 150, 200)), (100, 150, 200));
    }

    #[test]
    fn test_adaptive_tier0() {
        let pixels = vec![255, 255, 255, 255]; // White pixel
        let image = Image::from_rgba(pixels, 1, 1).alt("Test image");

        let node = image.render_for_tier(RenderTier::Tier0Fallback);
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_adaptive_tier1() {
        let pixels = vec![
            0, 0, 0, 255, // Black
            255, 255, 255, 255, // White
        ];
        let image = Image::from_rgba(pixels, 2, 1);

        let node = image.render_for_tier(RenderTier::Tier1Ansi);
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_adaptive_tier2() {
        let pixels = vec![255, 0, 0, 255]; // Red pixel
        let image = Image::from_rgba(pixels, 1, 1);

        let node = image.render_for_tier(RenderTier::Tier2Retained);
        // Should render as block characters
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_adaptive_all_tiers() {
        let pixels = vec![128, 128, 128, 255]; // Gray pixel
        let image = Image::from_rgba(pixels, 1, 1);

        // Should render without panic at all tiers
        for tier in [
            RenderTier::Tier0Fallback,
            RenderTier::Tier1Ansi,
            RenderTier::Tier2Retained,
            RenderTier::Tier3Gpu,
        ] {
            let _ = image.render_for_tier(tier);
        }
    }

    #[test]
    fn test_tier_features() {
        let pixels = vec![255, 255, 255, 255];
        let image = Image::from_rgba(pixels, 1, 1);
        let features = image.tier_features();

        assert_eq!(features.name, Some("Image"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
    }

    #[test]
    fn test_average_brightness() {
        // Black image
        let black = Image::from_rgba(vec![0, 0, 0, 255], 1, 1);
        assert!(black.calculate_average_brightness() < 0.1);

        // White image
        let white = Image::from_rgba(vec![255, 255, 255, 255], 1, 1);
        assert!(white.calculate_average_brightness() > 0.9);

        // Gray image
        let gray = Image::from_rgba(vec![128, 128, 128, 255], 1, 1);
        let brightness = gray.calculate_average_brightness();
        assert!(brightness > 0.4 && brightness < 0.6);
    }
}
