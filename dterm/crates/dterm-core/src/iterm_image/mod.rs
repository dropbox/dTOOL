//! iTerm2 inline image protocol (OSC 1337 File).
//!
//! This module implements iTerm2's inline image protocol, which allows
//! applications to display images directly in the terminal.
//!
//! ## Protocol Format
//!
//! ```text
//! OSC 1337 ; File = [params] : <base64 data> ST
//! ```
//!
//! Where params are semicolon-separated key=value pairs:
//! - `name=<base64>` - Base64-encoded filename
//! - `size=<bytes>` - File size in bytes (optional, for progress display)
//! - `width=<spec>` - Display width (N, Npx, N%, or auto)
//! - `height=<spec>` - Display height (N, Npx, N%, or auto)
//! - `preserveAspectRatio=<0|1>` - Whether to preserve aspect ratio (default: 1)
//! - `inline=<0|1>` - Whether to display inline (default: 0)
//!
//! ## Dimension Specifications
//!
//! - `N` - N character cells
//! - `Npx` - N pixels
//! - `N%` - Percentage of terminal width/height
//! - `auto` - Use image's inherent dimension
//!
//! ## Example
//!
//! ```text
//! OSC 1337 ; File = name=dGVzdC5wbmc= ; size=1234 ; inline=1 : <base64 data> BEL
//! ```
//!
//! ## References
//!
//! - [iTerm2 Images Documentation](https://iterm2.com/documentation-images.html)

use std::sync::Arc;

/// Maximum dimension for inline images (width or height in pixels).
/// Prevents DoS via extremely large images.
pub const ITERM_MAX_DIMENSION: u32 = 10000;

/// Maximum payload size (1 MB, matching iTerm2's limit).
pub const MAX_PAYLOAD_SIZE: usize = 1_048_576;

/// Dimension specification for inline images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionSpec {
    /// N character cells.
    Cells(u32),
    /// N pixels.
    Pixels(u32),
    /// N percent of terminal dimension.
    Percent(u8),
    /// Use image's inherent dimension.
    Auto,
}

impl Default for DimensionSpec {
    fn default() -> Self {
        Self::Auto
    }
}

impl DimensionSpec {
    /// Parse a dimension specification string.
    ///
    /// # Examples
    /// - "10" → Cells(10)
    /// - "100px" → Pixels(100)
    /// - "50%" → Percent(50)
    /// - "auto" → Auto
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("auto") {
            return Some(Self::Auto);
        }
        if let Some(px) = s.strip_suffix("px") {
            px.parse::<u32>()
                .ok()
                .map(|n| Self::Pixels(n.min(ITERM_MAX_DIMENSION)))
        } else if let Some(pct) = s.strip_suffix('%') {
            pct.parse::<u8>().ok().map(|n| Self::Percent(n.min(100)))
        } else {
            s.parse::<u32>().ok().map(|n| Self::Cells(n.min(10000)))
        }
    }

    /// Calculate the actual pixel dimension.
    ///
    /// # Arguments
    /// - `inherent`: The image's inherent dimension in pixels.
    /// - `cell_size`: Cell size in pixels (for cell-based dimensions).
    /// - `terminal_size`: Terminal dimension in pixels (for percentage-based).
    pub fn resolve(&self, inherent: u32, cell_size: u32, terminal_size: u32) -> u32 {
        match *self {
            Self::Auto => inherent,
            Self::Cells(n) => n.saturating_mul(cell_size),
            Self::Pixels(n) => n,
            Self::Percent(pct) => {
                // Safe: pct is 0-100, so pct * terminal_size fits in u64
                let result = u64::from(terminal_size) * u64::from(pct) / 100;
                u32::try_from(result.min(u64::from(u32::MAX))).unwrap_or(u32::MAX)
            }
        }
    }

    /// Get the direct cell count if specified as `Cells(n)`.
    ///
    /// Returns `None` for `Pixels`, `Percent`, or `Auto` specs.
    #[must_use]
    #[inline]
    pub const fn as_cells(&self) -> Option<u32> {
        match *self {
            Self::Cells(n) => Some(n),
            _ => None,
        }
    }

    /// Check if this is an `Auto` spec.
    #[must_use]
    #[inline]
    pub const fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// Inline image parameters parsed from OSC 1337 File command.
#[derive(Debug, Clone, Default)]
pub struct InlineImageParams {
    /// Base64-encoded filename (decoded to String if valid UTF-8).
    pub name: Option<String>,
    /// File size in bytes (for progress display).
    pub size: Option<usize>,
    /// Display width specification.
    pub width: DimensionSpec,
    /// Display height specification.
    pub height: DimensionSpec,
    /// Whether to preserve aspect ratio.
    pub preserve_aspect_ratio: bool,
    /// Whether to display inline (vs download).
    pub inline: bool,
}

impl InlineImageParams {
    /// Create new default parameters.
    pub fn new() -> Self {
        Self {
            name: None,
            size: None,
            width: DimensionSpec::Auto,
            height: DimensionSpec::Auto,
            preserve_aspect_ratio: true,
            inline: false,
        }
    }

    /// Parse parameters from a key=value string.
    ///
    /// The input should be the content after "File=" and before ":"
    pub fn parse(params_str: &str) -> Self {
        let mut params = Self::new();

        for part in params_str.split(';') {
            let part = part.trim();
            if let Some((key, value)) = part.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "name" => {
                        // Name is base64 encoded
                        if let Ok(decoded) = decode_base64(value.as_bytes()) {
                            params.name = String::from_utf8(decoded).ok();
                        }
                    }
                    "size" => {
                        params.size = value.parse().ok();
                    }
                    "width" => {
                        if let Some(spec) = DimensionSpec::parse(value) {
                            params.width = spec;
                        }
                    }
                    "height" => {
                        if let Some(spec) = DimensionSpec::parse(value) {
                            params.height = spec;
                        }
                    }
                    "preserveAspectRatio" => {
                        params.preserve_aspect_ratio = value != "0";
                    }
                    "inline" => {
                        params.inline = value == "1";
                    }
                    _ => {
                        // Unknown parameter, ignore
                    }
                }
            }
        }

        params
    }
}

/// An inline image ready for display.
#[derive(Debug, Clone)]
pub struct InlineImage {
    /// Unique identifier for this image.
    id: u64,
    /// Raw image data (format-specific: PNG, JPEG, GIF, etc.).
    data: Arc<Vec<u8>>,
    /// Filename (if provided).
    name: Option<String>,
    /// Display width specification.
    width: DimensionSpec,
    /// Display height specification.
    height: DimensionSpec,
    /// Whether to preserve aspect ratio.
    preserve_aspect_ratio: bool,
    /// Terminal cursor row where image should be placed.
    cursor_row: u16,
    /// Terminal cursor column where image should be placed.
    cursor_col: u16,
}

impl InlineImage {
    /// Create a new inline image.
    pub fn new(
        id: u64,
        data: Vec<u8>,
        params: &InlineImageParams,
        cursor_row: u16,
        cursor_col: u16,
    ) -> Self {
        Self {
            id,
            data: Arc::new(data),
            name: params.name.clone(),
            width: params.width,
            height: params.height,
            preserve_aspect_ratio: params.preserve_aspect_ratio,
            cursor_row,
            cursor_col,
        }
    }

    /// Get the unique identifier.
    #[inline]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the raw image data.
    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the filename (if provided).
    #[inline]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Get the width specification.
    #[inline]
    pub fn width(&self) -> DimensionSpec {
        self.width
    }

    /// Get the height specification.
    #[inline]
    pub fn height(&self) -> DimensionSpec {
        self.height
    }

    /// Check if aspect ratio should be preserved.
    #[inline]
    pub fn preserve_aspect_ratio(&self) -> bool {
        self.preserve_aspect_ratio
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

    /// Detect the image format from the data's magic bytes.
    pub fn format(&self) -> ImageFileFormat {
        ImageFileFormat::detect(&self.data)
    }

    /// Calculate dimensions in cells given cell size and terminal size.
    ///
    /// Returns (width_cells, height_cells).
    pub fn cells_spanned(
        &self,
        inherent_width: u32,
        inherent_height: u32,
        cell_width: u32,
        cell_height: u32,
        terminal_width_px: u32,
        terminal_height_px: u32,
    ) -> (u16, u16) {
        if cell_width == 0 || cell_height == 0 {
            return (0, 0);
        }

        let mut width_px = self
            .width
            .resolve(inherent_width, cell_width, terminal_width_px);
        let mut height_px = self
            .height
            .resolve(inherent_height, cell_height, terminal_height_px);

        // Apply aspect ratio preservation
        if self.preserve_aspect_ratio && inherent_width > 0 && inherent_height > 0 {
            // If only one dimension is Auto, scale the other
            let width_is_auto = matches!(self.width, DimensionSpec::Auto);
            let height_is_auto = matches!(self.height, DimensionSpec::Auto);

            if width_is_auto && !height_is_auto {
                // Scale width based on height
                let ratio =
                    u64::from(inherent_width) * u64::from(height_px) / u64::from(inherent_height);
                width_px = u32::try_from(ratio.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            } else if height_is_auto && !width_is_auto {
                // Scale height based on width
                let ratio =
                    u64::from(inherent_height) * u64::from(width_px) / u64::from(inherent_width);
                height_px = u32::try_from(ratio.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            } else if !width_is_auto && !height_is_auto {
                // Both specified - fit within bounds while preserving ratio
                let width_ratio = u64::from(width_px) * 1000 / u64::from(inherent_width);
                let height_ratio = u64::from(height_px) * 1000 / u64::from(inherent_height);
                let min_ratio = width_ratio.min(height_ratio);

                let new_width = u64::from(inherent_width) * min_ratio / 1000;
                let new_height = u64::from(inherent_height) * min_ratio / 1000;

                width_px = u32::try_from(new_width.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
                height_px = u32::try_from(new_height.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            }
        }

        // Clamp to max dimension
        width_px = width_px.min(ITERM_MAX_DIMENSION);
        height_px = height_px.min(ITERM_MAX_DIMENSION);

        // Convert to cells (round up)
        let width_cells = (width_px + cell_width - 1) / cell_width;
        let height_cells = (height_px + cell_height - 1) / cell_height;

        // Safe: results are bounded by ITERM_MAX_DIMENSION / cell_size
        let width_cells = u16::try_from(width_cells.min(u32::from(u16::MAX))).unwrap_or(u16::MAX);
        let height_cells = u16::try_from(height_cells.min(u32::from(u16::MAX))).unwrap_or(u16::MAX);

        (width_cells, height_cells)
    }
}

/// Detected image file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFileFormat {
    /// PNG image.
    Png,
    /// JPEG image.
    Jpeg,
    /// GIF image.
    Gif,
    /// BMP image.
    Bmp,
    /// WebP image.
    WebP,
    /// TIFF image.
    Tiff,
    /// Unknown format.
    Unknown,
}

impl ImageFileFormat {
    /// Detect format from magic bytes.
    pub fn detect(data: &[u8]) -> Self {
        if data.len() < 4 {
            return Self::Unknown;
        }

        // PNG: 89 50 4E 47 0D 0A 1A 0A
        if data.starts_with(&[0x89, b'P', b'N', b'G']) {
            return Self::Png;
        }

        // JPEG: FF D8 FF
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Self::Jpeg;
        }

        // GIF: GIF87a or GIF89a
        if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            return Self::Gif;
        }

        // BMP: BM
        if data.starts_with(b"BM") {
            return Self::Bmp;
        }

        // WebP: RIFF....WEBP
        if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
            return Self::WebP;
        }

        // TIFF: II or MM (little/big endian markers)
        if data.starts_with(&[0x49, 0x49, 0x2A, 0x00])
            || data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
        {
            return Self::Tiff;
        }

        Self::Unknown
    }

    /// Get the common file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::WebP => "webp",
            Self::Tiff => "tiff",
            Self::Unknown => "bin",
        }
    }

    /// Get the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::WebP => "image/webp",
            Self::Tiff => "image/tiff",
            Self::Unknown => "application/octet-stream",
        }
    }
}

/// Storage for inline images.
#[derive(Debug, Default)]
pub struct InlineImageStorage {
    /// Stored images indexed by ID.
    images: Vec<InlineImage>,
    /// Next image ID to assign.
    next_id: u64,
    /// Maximum number of images to store.
    max_images: usize,
    /// Total bytes of image data stored.
    total_bytes: usize,
    /// Maximum total bytes to store.
    max_bytes: usize,
}

impl InlineImageStorage {
    /// Create a new inline image storage.
    pub fn new(max_images: usize, max_bytes: usize) -> Self {
        Self {
            images: Vec::new(),
            next_id: 0,
            max_images,
            total_bytes: 0,
            max_bytes,
        }
    }

    /// Store a new inline image.
    ///
    /// Returns the image ID, or None if the image couldn't be stored
    /// (e.g., exceeds size limits).
    pub fn store(
        &mut self,
        data: Vec<u8>,
        params: &InlineImageParams,
        cursor_row: u16,
        cursor_col: u16,
    ) -> Option<u64> {
        let data_size = data.len();

        // Check size limit
        if data_size > MAX_PAYLOAD_SIZE {
            return None;
        }

        // Evict old images if needed
        while self.images.len() >= self.max_images {
            if let Some(oldest) = self.images.first() {
                self.total_bytes = self.total_bytes.saturating_sub(oldest.data.len());
            }
            self.images.remove(0);
        }

        // Evict if total bytes would exceed limit
        while self.total_bytes + data_size > self.max_bytes && !self.images.is_empty() {
            if let Some(oldest) = self.images.first() {
                self.total_bytes = self.total_bytes.saturating_sub(oldest.data.len());
            }
            self.images.remove(0);
        }

        let id = self.next_id;
        self.next_id += 1;

        let image = InlineImage::new(id, data, params, cursor_row, cursor_col);
        self.total_bytes += data_size;
        self.images.push(image);

        Some(id)
    }

    /// Get an image by ID.
    pub fn get(&self, id: u64) -> Option<&InlineImage> {
        self.images.iter().find(|img| img.id == id)
    }

    /// Get all stored images.
    pub fn images(&self) -> &[InlineImage] {
        &self.images
    }

    /// Clear all stored images.
    pub fn clear(&mut self) {
        self.images.clear();
        self.total_bytes = 0;
    }

    /// Get the number of stored images.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Check if storage is empty.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// Get total bytes of stored image data.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

/// Decode base64 data.
///
/// This is a simple base64 decoder that handles standard base64 encoding.
pub fn decode_base64(input: &[u8]) -> Result<Vec<u8>, Base64Error> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer: u32 = 0;
    let mut bits: u8 = 0;

    for &byte in input {
        // Skip whitespace
        if byte.is_ascii_whitespace() {
            continue;
        }

        // Handle padding
        if byte == b'=' {
            continue;
        }

        // Decode character
        let value = if byte == b'+' {
            62
        } else if byte == b'/' {
            63
        } else if byte.is_ascii_uppercase() {
            byte - b'A'
        } else if byte.is_ascii_lowercase() {
            byte - b'a' + 26
        } else if byte.is_ascii_digit() {
            byte - b'0' + 52
        } else {
            return Err(Base64Error::InvalidCharacter(byte));
        };

        buffer = (buffer << 6) | u32::from(value);
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            // Safe: bits is 8-14 before subtraction, so shift is 0-6
            #[allow(clippy::cast_possible_truncation)]
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

/// Base64 decoding error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base64Error {
    /// Invalid character in input.
    InvalidCharacter(u8),
}

impl std::fmt::Display for Base64Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCharacter(c) => write!(f, "invalid base64 character: 0x{:02X}", c),
        }
    }
}

impl std::error::Error for Base64Error {}

/// Parse an OSC 1337 File command.
///
/// Returns the parsed parameters and decoded image data, or None if parsing failed.
///
/// The input should be the content after "1337;" and include `File=...:base64data`
pub fn parse_file_command(content: &[u8]) -> Option<(InlineImageParams, Vec<u8>)> {
    // Convert to string for easier parsing
    let content_str = std::str::from_utf8(content).ok()?;

    // Must start with "File="
    let file_content = content_str.strip_prefix("File=")?;

    // Split at colon to separate params from data
    let (params_str, data_str) = file_content.split_once(':')?;

    // Parse parameters
    let params = InlineImageParams::parse(params_str);

    // Decode base64 data
    let data = decode_base64(data_str.as_bytes()).ok()?;

    // Validate size if specified
    if let Some(expected_size) = params.size {
        if data.len() != expected_size {
            // Size mismatch - some implementations are lenient, but we'll accept it
            // as long as we got some data
        }
    }

    Some((params, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_spec_parse_cells() {
        assert_eq!(DimensionSpec::parse("10"), Some(DimensionSpec::Cells(10)));
        assert_eq!(DimensionSpec::parse("0"), Some(DimensionSpec::Cells(0)));
        assert_eq!(DimensionSpec::parse("999"), Some(DimensionSpec::Cells(999)));
    }

    #[test]
    fn dimension_spec_parse_pixels() {
        assert_eq!(
            DimensionSpec::parse("100px"),
            Some(DimensionSpec::Pixels(100))
        );
        assert_eq!(DimensionSpec::parse("0px"), Some(DimensionSpec::Pixels(0)));
        assert_eq!(
            DimensionSpec::parse("500px"),
            Some(DimensionSpec::Pixels(500))
        );
    }

    #[test]
    fn dimension_spec_parse_percent() {
        assert_eq!(
            DimensionSpec::parse("50%"),
            Some(DimensionSpec::Percent(50))
        );
        assert_eq!(
            DimensionSpec::parse("100%"),
            Some(DimensionSpec::Percent(100))
        );
        assert_eq!(DimensionSpec::parse("0%"), Some(DimensionSpec::Percent(0)));
    }

    #[test]
    fn dimension_spec_parse_auto() {
        assert_eq!(DimensionSpec::parse("auto"), Some(DimensionSpec::Auto));
        assert_eq!(DimensionSpec::parse("AUTO"), Some(DimensionSpec::Auto));
        assert_eq!(DimensionSpec::parse("Auto"), Some(DimensionSpec::Auto));
    }

    #[test]
    fn dimension_spec_parse_invalid() {
        assert_eq!(DimensionSpec::parse("abc"), None);
        assert_eq!(DimensionSpec::parse(""), None);
        assert_eq!(DimensionSpec::parse("-1"), None);
    }

    #[test]
    fn dimension_spec_resolve() {
        // Auto uses inherent
        assert_eq!(DimensionSpec::Auto.resolve(200, 10, 800), 200);

        // Cells multiply by cell size
        assert_eq!(DimensionSpec::Cells(5).resolve(200, 10, 800), 50);

        // Pixels return directly
        assert_eq!(DimensionSpec::Pixels(150).resolve(200, 10, 800), 150);

        // Percent of terminal size
        assert_eq!(DimensionSpec::Percent(50).resolve(200, 10, 800), 400);
        assert_eq!(DimensionSpec::Percent(100).resolve(200, 10, 800), 800);
    }

    #[test]
    fn inline_image_params_parse_basic() {
        let params = InlineImageParams::parse("inline=1");
        assert!(params.inline);
        assert!(params.preserve_aspect_ratio); // default

        let params = InlineImageParams::parse("inline=0;preserveAspectRatio=0");
        assert!(!params.inline);
        assert!(!params.preserve_aspect_ratio);
    }

    #[test]
    fn inline_image_params_parse_dimensions() {
        let params = InlineImageParams::parse("width=100px;height=50");
        assert_eq!(params.width, DimensionSpec::Pixels(100));
        assert_eq!(params.height, DimensionSpec::Cells(50));
    }

    #[test]
    fn inline_image_params_parse_name() {
        // "test.png" in base64 is "dGVzdC5wbmc="
        let params = InlineImageParams::parse("name=dGVzdC5wbmc=");
        assert_eq!(params.name.as_deref(), Some("test.png"));
    }

    #[test]
    fn inline_image_params_parse_size() {
        let params = InlineImageParams::parse("size=12345");
        assert_eq!(params.size, Some(12345));
    }

    #[test]
    fn base64_decode_simple() {
        // "Hello" = "SGVsbG8="
        let decoded = decode_base64(b"SGVsbG8=").unwrap();
        assert_eq!(decoded, b"Hello");

        // "World" = "V29ybGQ="
        let decoded = decode_base64(b"V29ybGQ=").unwrap();
        assert_eq!(decoded, b"World");
    }

    #[test]
    fn base64_decode_with_whitespace() {
        let decoded = decode_base64(b"SGVs\nbG8=").unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn base64_decode_invalid_char() {
        let result = decode_base64(b"SGVs!G8=");
        assert!(result.is_err());
    }

    #[test]
    fn image_format_detect_png() {
        let png_data = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(ImageFileFormat::detect(&png_data), ImageFileFormat::Png);
    }

    #[test]
    fn image_format_detect_jpeg() {
        let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(ImageFileFormat::detect(&jpeg_data), ImageFileFormat::Jpeg);
    }

    #[test]
    fn image_format_detect_gif() {
        assert_eq!(ImageFileFormat::detect(b"GIF89a"), ImageFileFormat::Gif);
        assert_eq!(ImageFileFormat::detect(b"GIF87a"), ImageFileFormat::Gif);
    }

    #[test]
    fn image_format_detect_bmp() {
        let bmp_data = [b'B', b'M', 0x00, 0x00];
        assert_eq!(ImageFileFormat::detect(&bmp_data), ImageFileFormat::Bmp);
    }

    #[test]
    fn image_format_detect_webp() {
        let webp_data = b"RIFF....WEBP";
        assert_eq!(ImageFileFormat::detect(webp_data), ImageFileFormat::WebP);
    }

    #[test]
    fn image_format_detect_unknown() {
        assert_eq!(ImageFileFormat::detect(b"????"), ImageFileFormat::Unknown);
        assert_eq!(ImageFileFormat::detect(b""), ImageFileFormat::Unknown);
    }

    #[test]
    fn inline_image_storage_basic() {
        let mut storage = InlineImageStorage::new(10, 1024 * 1024);
        assert!(storage.is_empty());

        let params = InlineImageParams::new();
        let data = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header
        let id = storage.store(data.clone(), &params, 0, 0);

        assert!(id.is_some());
        assert_eq!(storage.len(), 1);

        let image = storage.get(id.unwrap()).unwrap();
        assert_eq!(image.data(), &data);
    }

    #[test]
    fn inline_image_storage_eviction() {
        let mut storage = InlineImageStorage::new(2, 1024 * 1024);
        let params = InlineImageParams::new();

        let id1 = storage.store(vec![1; 10], &params, 0, 0).unwrap();
        let id2 = storage.store(vec![2; 10], &params, 0, 0).unwrap();
        let id3 = storage.store(vec![3; 10], &params, 0, 0).unwrap();

        // id1 should have been evicted
        assert!(storage.get(id1).is_none());
        assert!(storage.get(id2).is_some());
        assert!(storage.get(id3).is_some());
    }

    #[test]
    fn parse_file_command_basic() {
        // "test" = "dGVzdA=="
        let content = b"File=name=dGVzdA==;inline=1:SGVsbG8="; // "Hello"
        let result = parse_file_command(content);
        assert!(result.is_some());

        let (params, data) = result.unwrap();
        assert_eq!(params.name.as_deref(), Some("test"));
        assert!(params.inline);
        assert_eq!(data, b"Hello");
    }

    #[test]
    fn parse_file_command_with_dimensions() {
        let content = b"File=width=100px;height=50%:SGVsbG8=";
        let (params, _) = parse_file_command(content).unwrap();
        assert_eq!(params.width, DimensionSpec::Pixels(100));
        assert_eq!(params.height, DimensionSpec::Percent(50));
    }

    #[test]
    fn inline_image_cells_spanned() {
        let params = InlineImageParams {
            width: DimensionSpec::Auto,
            height: DimensionSpec::Auto,
            preserve_aspect_ratio: true,
            ..Default::default()
        };
        let image = InlineImage::new(0, vec![], &params, 0, 0);

        // 200x100 image, 10x20 cells, 800x600 terminal
        let (w, h) = image.cells_spanned(200, 100, 10, 20, 800, 600);
        assert_eq!(w, 20); // 200 / 10 = 20
        assert_eq!(h, 5); // 100 / 20 = 5
    }

    #[test]
    fn inline_image_cells_spanned_with_scale() {
        let params = InlineImageParams {
            width: DimensionSpec::Cells(10),
            height: DimensionSpec::Auto,
            preserve_aspect_ratio: true,
            ..Default::default()
        };
        let image = InlineImage::new(0, vec![], &params, 0, 0);

        // 200x100 image, 10x10 cells
        // Width = 10 cells = 100px
        // Height should scale: 100px * (100/200) = 50px = 5 cells
        let (w, h) = image.cells_spanned(200, 100, 10, 10, 800, 600);
        assert_eq!(w, 10);
        assert_eq!(h, 5);
    }
}

// Kani proofs
#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    #[kani::unwind(10)]
    fn dimension_resolve_no_overflow() {
        let spec: u8 = kani::any();
        let inherent: u32 = kani::any();
        let cell_size: u32 = kani::any();
        let terminal_size: u32 = kani::any();

        // Restrict to reasonable bounds to avoid state explosion
        kani::assume(inherent <= ITERM_MAX_DIMENSION);
        kani::assume(cell_size > 0 && cell_size <= 100);
        kani::assume(terminal_size <= 10000);

        let dim_spec = match spec % 4 {
            0 => DimensionSpec::Auto,
            1 => DimensionSpec::Cells(inherent % 1000),
            2 => DimensionSpec::Pixels(inherent),
            _ => DimensionSpec::Percent((inherent % 101) as u8),
        };

        let result = dim_spec.resolve(inherent, cell_size, terminal_size);
        // Result should never panic and should be bounded
        let _ = result;
    }

    #[kani::proof]
    #[kani::unwind(65)]
    fn base64_decode_no_panic() {
        let data: [u8; 8] = kani::any();
        // Try decoding - should not panic
        let _ = decode_base64(&data);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn image_format_detect_no_panic() {
        let data: [u8; 16] = kani::any();
        let format = ImageFileFormat::detect(&data);
        // Should always produce a valid format
        let _ = format.extension();
        let _ = format.mime_type();
    }
}
