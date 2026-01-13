//! AI perception API for reading terminal state.
//!
//! This module provides the [`Perception`] type, which allows AI agents to
//! observe and understand terminal content in multiple modalities:
//!
//! - **Text**: Plain text, tokenized text, and text with semantic markers
//! - **Image**: PNG rendering for vision models (requires `image` feature)
//! - **Diff**: Semantic diffs and activity region detection
//!
//! # Example
//!
//! ```rust,ignore
//! use inky::perception::Perception;
//! use inky::render::Buffer;
//!
//! let buffer = Buffer::new(80, 24);
//! let perception = Perception::new(&buffer);
//!
//! // Get plain text for LLM context
//! let text = perception.as_text();
//!
//! // Get marked text with style annotations
//! let marked = perception.as_marked_text();
//!
//! // Tokenize for efficient LLM processing
//! let tokens = perception.as_tokens();
//! ```
//!
//! # Semantic Diff
//!
//! Track changes between frames for AI attention:
//!
//! ```rust,ignore
//! let prev = Buffer::new(80, 24);
//! let current = Buffer::new(80, 24);
//!
//! let diff = Perception::semantic_diff(&prev, &current);
//! for (row, content) in diff.added_lines {
//!     println!("New content at row {}: {}", row, content);
//! }
//! ```
//!
//! [`Perception`]: crate::perception::Perception

#[cfg(feature = "image")]
use crate::render::PackedColor;
use crate::render::{Buffer, Cell, CellFlags};
#[cfg(feature = "image")]
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
#[cfg(feature = "image")]
use image::{ImageFormat, RgbaImage};
#[cfg(feature = "image")]
use std::io::Cursor;

/// A token extracted from the terminal buffer.
///
/// Tokens represent individual words or content units along with their
/// position and styling information, suitable for LLM processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The text content of this token.
    pub text: String,
    /// Column position (0-indexed).
    pub x: u16,
    /// Row position (0-indexed).
    pub y: u16,
    /// Whether this token has bold styling.
    pub bold: bool,
    /// Whether this token has dim styling.
    pub dim: bool,
    /// Whether this token has italic styling.
    pub italic: bool,
    /// Whether this token has underline styling.
    pub underline: bool,
}

impl Token {
    /// Create a new token.
    pub fn new(text: impl Into<String>, x: u16, y: u16) -> Self {
        Self {
            text: text.into(),
            x,
            y,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
        }
    }

    /// Create a newline token at the end of a row.
    pub fn newline(y: u16) -> Self {
        Self {
            text: "\n".into(),
            x: 0,
            y,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
        }
    }
}

/// A rectangular region in the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// Left column (inclusive).
    pub x: u16,
    /// Top row (inclusive).
    pub y: u16,
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
}

impl Region {
    /// Create a new region.
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if this region contains a point.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Check if this region intersects another.
    pub fn intersects(&self, other: &Region) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// Merge this region with another, returning the bounding box.
    pub fn merge(&self, other: &Region) -> Region {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Region::new(x, y, right - x, bottom - y)
    }
}

/// Semantic diff between two buffer states.
///
/// Captures high-level changes suitable for AI understanding.
#[derive(Debug, Clone, Default)]
pub struct SemanticDiff {
    /// Lines that were added (row index, content).
    pub added_lines: Vec<(u16, String)>,
    /// Lines that were removed (row index, content).
    pub removed_lines: Vec<(u16, String)>,
    /// Lines that were modified (row index, old content, new content).
    pub modified_lines: Vec<(u16, String, String)>,
    /// Regions where changes occurred.
    pub changed_regions: Vec<Region>,
    /// Total number of cells that changed.
    pub cells_changed: usize,
}

impl SemanticDiff {
    /// Check if there are any changes.
    pub fn is_empty(&self) -> bool {
        self.added_lines.is_empty()
            && self.removed_lines.is_empty()
            && self.modified_lines.is_empty()
    }

    /// Get a summary of changes for logging.
    pub fn summary(&self) -> String {
        let mut parts = Vec::with_capacity(3); // max: added, removed, modified
        if !self.added_lines.is_empty() {
            parts.push(format!("+{} lines", self.added_lines.len()));
        }
        if !self.removed_lines.is_empty() {
            parts.push(format!("-{} lines", self.removed_lines.len()));
        }
        if !self.modified_lines.is_empty() {
            parts.push(format!("~{} lines", self.modified_lines.len()));
        }
        if parts.is_empty() {
            "no changes".into()
        } else {
            parts.join(", ")
        }
    }
}

/// AI perception interface for terminal buffers.
///
/// Provides multiple modalities for AI agents to observe terminal state:
/// - Text extraction (plain, tokenized, marked)
/// - Region reading
/// - Semantic diffing
/// - Activity detection
///
/// # Example
///
/// ```rust
/// use inky::perception::Perception;
/// use inky::render::Buffer;
///
/// let buffer = Buffer::new(80, 24);
/// let perception = Perception::new(&buffer);
///
/// // Get content as plain text
/// let text = perception.as_text();
///
/// // Get content with style markers
/// let marked = perception.as_marked_text();
/// ```
pub struct Perception<'a> {
    buffer: &'a Buffer,
}

impl<'a> Perception<'a> {
    /// Create a new perception view of a buffer.
    pub fn new(buffer: &'a Buffer) -> Self {
        Self { buffer }
    }

    /// Get the underlying buffer.
    pub fn buffer(&self) -> &Buffer {
        self.buffer
    }

    // =========================================================================
    // TEXT MODALITY
    // =========================================================================

    /// Get buffer content as plain text.
    ///
    /// Returns the full buffer as a string with newlines separating rows.
    /// This is suitable for direct inclusion in LLM context.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::perception::Perception;
    /// use inky::render::Buffer;
    ///
    /// let buffer = Buffer::new(10, 2);
    /// let perception = Perception::new(&buffer);
    /// let text = perception.as_text();
    /// // Returns "          \n          \n" (10 spaces per row + newlines)
    /// ```
    pub fn as_text(&self) -> String {
        self.buffer.to_text()
    }

    /// Get buffer content with semantic style markers.
    ///
    /// Returns text with XML-like markers indicating style changes:
    /// - `<b>text</b>` for bold
    /// - `<i>text</i>` for italic
    /// - `<u>text</u>` for underline
    /// - `<dim>text</dim>` for dimmed
    ///
    /// This format is easy for LLMs to parse and understand styling.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let marked = perception.as_marked_text();
    /// // Could return: "<b>Error:</b> File not found"
    /// ```
    pub fn as_marked_text(&self) -> String {
        let mut result = String::with_capacity(
            (self.buffer.width() as usize + 1) * self.buffer.height() as usize,
        );

        let mut current_bold = false;
        let mut current_italic = false;
        let mut current_underline = false;
        let mut current_dim = false;

        for y in 0..self.buffer.height() {
            for x in 0..self.buffer.width() {
                if let Some(cell) = self.buffer.get(x, y) {
                    // Skip wide character spacers
                    if cell.flags.contains(CellFlags::WIDE_SPACER) {
                        continue;
                    }

                    let is_bold = cell.flags.contains(CellFlags::BOLD);
                    let is_italic = cell.flags.contains(CellFlags::ITALIC);
                    let is_underline = cell.flags.contains(CellFlags::UNDERLINE);
                    let is_dim = cell.flags.contains(CellFlags::DIM);

                    // Close tags for attributes that changed
                    if current_underline && !is_underline {
                        result.push_str("</u>");
                        current_underline = false;
                    }
                    if current_italic && !is_italic {
                        result.push_str("</i>");
                        current_italic = false;
                    }
                    if current_dim && !is_dim {
                        result.push_str("</dim>");
                        current_dim = false;
                    }
                    if current_bold && !is_bold {
                        result.push_str("</b>");
                        current_bold = false;
                    }

                    // Open tags for new attributes
                    if is_bold && !current_bold {
                        result.push_str("<b>");
                        current_bold = true;
                    }
                    if is_dim && !current_dim {
                        result.push_str("<dim>");
                        current_dim = true;
                    }
                    if is_italic && !current_italic {
                        result.push_str("<i>");
                        current_italic = true;
                    }
                    if is_underline && !current_underline {
                        result.push_str("<u>");
                        current_underline = true;
                    }

                    result.push(cell.char());
                }
            }

            // Close all tags at end of line
            if current_underline {
                result.push_str("</u>");
                current_underline = false;
            }
            if current_italic {
                result.push_str("</i>");
                current_italic = false;
            }
            if current_dim {
                result.push_str("</dim>");
                current_dim = false;
            }
            if current_bold {
                result.push_str("</b>");
                current_bold = false;
            }

            result.push('\n');
        }

        result
    }

    /// Tokenize buffer content for LLM processing.
    ///
    /// Returns a list of tokens, each representing a word or content unit
    /// with position and styling information. This is more efficient than
    /// raw text for LLMs that tokenize differently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::perception::Perception;
    /// use inky::render::Buffer;
    ///
    /// let buffer = Buffer::new(80, 24);
    /// let perception = Perception::new(&buffer);
    /// let tokens = perception.as_tokens();
    /// ```
    pub fn as_tokens(&self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut current_word = String::new();
        let mut word_start_x: u16 = 0;
        let mut current_flags = CellFlags::empty();

        for y in 0..self.buffer.height() {
            for x in 0..self.buffer.width() {
                if let Some(cell) = self.buffer.get(x, y) {
                    // Skip wide character spacers
                    if cell.flags.contains(CellFlags::WIDE_SPACER) {
                        continue;
                    }

                    let c = cell.char();
                    let flags = cell.flags;

                    // If character is whitespace or style changed, end current word
                    let style_relevant =
                        CellFlags::BOLD | CellFlags::DIM | CellFlags::ITALIC | CellFlags::UNDERLINE;
                    let style_changed =
                        (flags & style_relevant) != (current_flags & style_relevant);

                    if c.is_whitespace() || (style_changed && !current_word.is_empty()) {
                        if !current_word.is_empty() {
                            let token = Token {
                                text: std::mem::take(&mut current_word),
                                x: word_start_x,
                                y,
                                bold: current_flags.contains(CellFlags::BOLD),
                                dim: current_flags.contains(CellFlags::DIM),
                                italic: current_flags.contains(CellFlags::ITALIC),
                                underline: current_flags.contains(CellFlags::UNDERLINE),
                            };
                            tokens.push(token);
                        }
                        if style_changed && !c.is_whitespace() {
                            current_word.push(c);
                            word_start_x = x;
                            current_flags = flags;
                        }
                    } else {
                        if current_word.is_empty() {
                            word_start_x = x;
                            current_flags = flags;
                        }
                        current_word.push(c);
                    }
                }
            }

            // End of line: flush current word
            if !current_word.is_empty() {
                let token = Token {
                    text: std::mem::take(&mut current_word),
                    x: word_start_x,
                    y,
                    bold: current_flags.contains(CellFlags::BOLD),
                    dim: current_flags.contains(CellFlags::DIM),
                    italic: current_flags.contains(CellFlags::ITALIC),
                    underline: current_flags.contains(CellFlags::UNDERLINE),
                };
                tokens.push(token);
            }

            // Add newline token
            tokens.push(Token::newline(y));
            current_flags = CellFlags::empty();
        }

        tokens
    }

    // =========================================================================
    // REGION ACCESS
    // =========================================================================

    /// Read a rectangular region as text.
    ///
    /// Returns the content of the specified region with newlines at the end
    /// of each row. Useful for reading specific UI elements.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::perception::{Perception, Region};
    /// use inky::render::Buffer;
    ///
    /// let buffer = Buffer::new(80, 24);
    /// let perception = Perception::new(&buffer);
    /// let region = Region::new(0, 0, 10, 5);
    /// let text = perception.read_region(&region);
    /// ```
    pub fn read_region(&self, region: &Region) -> String {
        let mut result = String::new();

        let end_y = (region.y + region.height).min(self.buffer.height());
        let end_x = (region.x + region.width).min(self.buffer.width());

        for y in region.y..end_y {
            for x in region.x..end_x {
                if let Some(cell) = self.buffer.get(x, y) {
                    if !cell.flags.contains(CellFlags::WIDE_SPACER) {
                        result.push(cell.char());
                    }
                }
            }
            result.push('\n');
        }

        result
    }

    /// Find all occurrences of a string in the buffer.
    ///
    /// Returns a list of (x, y) positions where the string starts.
    /// Search is case-sensitive.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::perception::Perception;
    /// use inky::render::Buffer;
    ///
    /// let buffer = Buffer::new(80, 24);
    /// let perception = Perception::new(&buffer);
    /// let positions = perception.find("error");
    /// ```
    pub fn find(&self, needle: &str) -> Vec<(u16, u16)> {
        let mut positions = Vec::new();
        let text = self.as_text();

        for (line_idx, line) in text.lines().enumerate() {
            for (char_idx, _) in line.match_indices(needle) {
                positions.push((char_idx as u16, line_idx as u16));
            }
        }

        positions
    }

    /// Find all occurrences of a string (case-insensitive).
    pub fn find_ignore_case(&self, needle: &str) -> Vec<(u16, u16)> {
        let mut positions = Vec::new();
        let text = self.as_text().to_lowercase();
        let needle_lower = needle.to_lowercase();

        for (line_idx, line) in text.lines().enumerate() {
            for (char_idx, _) in line.match_indices(&needle_lower) {
                positions.push((char_idx as u16, line_idx as u16));
            }
        }

        positions
    }

    // =========================================================================
    // DIFF & ACTIVITY DETECTION
    // =========================================================================

    /// Compute semantic diff between two buffers.
    ///
    /// Analyzes changes at the line level, identifying added, removed, and
    /// modified lines. This is useful for AI agents to understand what
    /// changed between frames.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::perception::Perception;
    /// use inky::render::Buffer;
    ///
    /// let prev = Buffer::new(80, 24);
    /// let current = Buffer::new(80, 24);
    ///
    /// let diff = Perception::semantic_diff(&prev, &current);
    /// if !diff.is_empty() {
    ///     println!("Changes: {}", diff.summary());
    /// }
    /// ```
    pub fn semantic_diff(prev: &Buffer, current: &Buffer) -> SemanticDiff {
        let mut diff = SemanticDiff::default();

        // Extract lines from both buffers
        let prev_lines: Vec<String> = prev.to_text().lines().map(String::from).collect();
        let current_lines: Vec<String> = current.to_text().lines().map(String::from).collect();

        // Compare line by line
        let max_lines = prev_lines.len().max(current_lines.len());

        for i in 0..max_lines {
            let prev_line = prev_lines.get(i).map(|s| s.as_str()).unwrap_or("");
            let current_line = current_lines.get(i).map(|s| s.as_str()).unwrap_or("");

            if prev_line != current_line {
                // Trim to check for empty vs non-empty
                let prev_empty = prev_line.trim().is_empty();
                let curr_empty = current_line.trim().is_empty();

                if prev_empty && !curr_empty {
                    diff.added_lines.push((i as u16, current_line.to_string()));
                } else if !prev_empty && curr_empty {
                    diff.removed_lines.push((i as u16, prev_line.to_string()));
                } else {
                    diff.modified_lines.push((
                        i as u16,
                        prev_line.to_string(),
                        current_line.to_string(),
                    ));
                }
            }
        }

        // Count cell changes
        let min_width = prev.width().min(current.width());
        let min_height = prev.height().min(current.height());

        for y in 0..min_height {
            for x in 0..min_width {
                if let (Some(prev_cell), Some(curr_cell)) = (prev.get(x, y), current.get(x, y)) {
                    if prev_cell != curr_cell {
                        diff.cells_changed += 1;
                    }
                }
            }
        }

        // Detect changed regions (simplified: bounding box of all changes)
        if diff.cells_changed > 0 {
            let mut min_x = min_width;
            let mut max_x: u16 = 0;
            let mut min_y = min_height;
            let mut max_y: u16 = 0;

            for y in 0..min_height {
                for x in 0..min_width {
                    if let (Some(prev_cell), Some(curr_cell)) = (prev.get(x, y), current.get(x, y))
                    {
                        if prev_cell != curr_cell {
                            min_x = min_x.min(x);
                            max_x = max_x.max(x);
                            min_y = min_y.min(y);
                            max_y = max_y.max(y);
                        }
                    }
                }
            }

            if max_x >= min_x && max_y >= min_y {
                diff.changed_regions.push(Region::new(
                    min_x,
                    min_y,
                    max_x - min_x + 1,
                    max_y - min_y + 1,
                ));
            }
        }

        diff
    }

    /// Find regions with activity (dirty cells) in the buffer.
    ///
    /// Returns a list of regions that contain dirty cells. This is useful
    /// for AI agents to focus attention on recently changed areas.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::perception::Perception;
    /// use inky::render::Buffer;
    ///
    /// let buffer = Buffer::new(80, 24);
    /// let perception = Perception::new(&buffer);
    /// let active = perception.activity_regions();
    /// ```
    pub fn activity_regions(&self) -> Vec<Region> {
        let mut regions = Vec::new();
        let mut min_x: Option<u16> = None;
        let mut max_x: u16 = 0;
        let mut min_y: Option<u16> = None;
        let mut max_y: u16 = 0;

        for y in 0..self.buffer.height() {
            for x in 0..self.buffer.width() {
                if let Some(cell) = self.buffer.get(x, y) {
                    if cell.is_dirty() {
                        min_x = Some(min_x.unwrap_or(x).min(x));
                        max_x = max_x.max(x);
                        min_y = Some(min_y.unwrap_or(y).min(y));
                        max_y = max_y.max(y);
                    }
                }
            }
        }

        if let (Some(x), Some(y)) = (min_x, min_y) {
            regions.push(Region::new(x, y, max_x - x + 1, max_y - y + 1));
        }

        regions
    }

    // =========================================================================
    // CELL-LEVEL ACCESS
    // =========================================================================

    /// Get the style at a specific position.
    ///
    /// Returns the cell flags at the given position, or `None` if out of bounds.
    pub fn style_at(&self, x: u16, y: u16) -> Option<CellFlags> {
        self.buffer.get(x, y).map(|c| c.flags)
    }

    /// Get the character at a specific position.
    pub fn char_at(&self, x: u16, y: u16) -> Option<char> {
        self.buffer.get(x, y).map(|c| c.char())
    }

    /// Get direct access to the raw cell data.
    ///
    /// This provides zero-copy access to the underlying buffer for
    /// high-performance AI applications.
    pub fn cells(&self) -> &[Cell] {
        self.buffer.cells()
    }
}

#[cfg(feature = "image")]
impl<'a> Perception<'a> {
    /// Render the buffer as PNG image bytes.
    ///
    /// Each terminal cell is rendered as a square of `font_size` pixels. Glyphs
    /// are approximated as filled blocks to preserve layout and color context.
    pub fn as_image(&self, font_size: u8) -> Vec<u8> {
        let cell_size = u32::from(font_size.max(1));
        let width = u32::from(self.buffer.width());
        let height = u32::from(self.buffer.height());
        let img_width = width.saturating_mul(cell_size);
        let img_height = height.saturating_mul(cell_size);

        if img_width == 0 || img_height == 0 {
            return Vec::new();
        }

        let pixel_len = (img_width as usize)
            .saturating_mul(img_height as usize)
            .saturating_mul(4);
        let mut pixels = vec![0u8; pixel_len];

        let base_margin = if cell_size > 4 { cell_size / 5 } else { 0 };

        for y in 0..height {
            for x in 0..width {
                let cell = match self.buffer.get(x as u16, y as u16) {
                    Some(cell) => cell,
                    None => continue,
                };
                let mut fg = cell.fg();
                let mut bg = cell.bg();
                if cell.flags.contains(CellFlags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }

                let fg = if cell.flags.contains(CellFlags::DIM) {
                    dim_color(fg)
                } else {
                    fg
                };

                let cell_x = x.saturating_mul(cell_size);
                let cell_y = y.saturating_mul(cell_size);

                fill_rect(
                    &mut pixels,
                    img_width,
                    img_height,
                    cell_x,
                    cell_y,
                    cell_size,
                    cell_size,
                    bg,
                );

                let is_spacer = cell.flags.contains(CellFlags::WIDE_SPACER);
                let is_hidden = cell.flags.contains(CellFlags::HIDDEN);
                let draw_glyph = !is_hidden && !is_spacer && !cell.char().is_whitespace();

                if draw_glyph {
                    let mut margin = base_margin;
                    if cell.flags.contains(CellFlags::BOLD) && margin > 0 {
                        margin -= 1;
                    }
                    let italic_shift =
                        u32::from(cell.flags.contains(CellFlags::ITALIC) && cell_size > 2);
                    let mut glyph_x = cell_x.saturating_add(margin).saturating_add(italic_shift);
                    let mut glyph_y = cell_y.saturating_add(margin);
                    let mut glyph_w = cell_size.saturating_sub(margin.saturating_mul(2));
                    let mut glyph_h = cell_size.saturating_sub(margin.saturating_mul(2));

                    if glyph_w == 0 || glyph_h == 0 {
                        glyph_x = cell_x;
                        glyph_y = cell_y;
                        glyph_w = cell_size;
                        glyph_h = cell_size;
                    }

                    fill_rect(
                        &mut pixels,
                        img_width,
                        img_height,
                        glyph_x,
                        glyph_y,
                        glyph_w,
                        glyph_h,
                        fg,
                    );
                }

                if cell.flags.contains(CellFlags::UNDERLINE) && !is_hidden {
                    let underline_y = cell_y.saturating_add(cell_size.saturating_sub(1));
                    fill_rect(
                        &mut pixels,
                        img_width,
                        img_height,
                        cell_x,
                        underline_y,
                        cell_size,
                        1,
                        fg,
                    );
                }

                if cell.flags.contains(CellFlags::STRIKETHROUGH) && !is_hidden {
                    let strike_y = cell_y.saturating_add(cell_size / 2);
                    fill_rect(
                        &mut pixels,
                        img_width,
                        img_height,
                        cell_x,
                        strike_y,
                        cell_size,
                        1,
                        fg,
                    );
                }
            }
        }

        let image = match RgbaImage::from_raw(img_width, img_height, pixels) {
            Some(image) => image,
            None => {
                #[cfg(debug_assertions)]
                eprintln!("Warning: perception image buffer size mismatch");
                return Vec::new();
            }
        };

        let mut png = Vec::new();
        if image
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .is_err()
        {
            #[cfg(debug_assertions)]
            eprintln!("Warning: perception PNG encoding failed");
            return Vec::new();
        }

        png
    }

    /// Render the buffer as base64-encoded PNG bytes.
    pub fn as_image_base64(&self, font_size: u8) -> String {
        BASE64.encode(self.as_image(font_size))
    }
}

#[cfg(feature = "image")]
#[allow(clippy::too_many_arguments)]
fn fill_rect(
    pixels: &mut [u8],
    img_width: u32,
    img_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: PackedColor,
) {
    let x_end = x.saturating_add(width).min(img_width);
    let y_end = y.saturating_add(height).min(img_height);
    let stride = (img_width as usize).saturating_mul(4);

    for row in y..y_end {
        let row_start = row as usize * stride;
        for col in x..x_end {
            let idx = row_start + col as usize * 4;
            pixels[idx] = color.r;
            pixels[idx + 1] = color.g;
            pixels[idx + 2] = color.b;
            pixels[idx + 3] = 255;
        }
    }
}

#[cfg(feature = "image")]
fn dim_color(color: PackedColor) -> PackedColor {
    const DIM_SCALE: u16 = 160;
    PackedColor::new(
        ((color.r as u16 * DIM_SCALE) / 255) as u8,
        ((color.g as u16 * DIM_SCALE) / 255) as u8,
        ((color.b as u16 * DIM_SCALE) / 255) as u8,
    )
}

// =============================================================================
// SHARED MEMORY PERCEPTION (for AI agents reading from IPC)
// =============================================================================

use crate::render::gpu::{GpuBuffer, GpuCell};
use crate::render::ipc::SharedMemoryBuffer;
use std::io;
use std::path::Path;

/// AI perception interface for shared memory buffers.
///
/// This type allows external processes (like AI agents) to read terminal
/// state from shared memory without any copying. It provides a subset of
/// the [`Perception`] API optimized for GPU cell data.
///
/// # Example
///
/// ```ignore
/// use inky::perception::SharedPerception;
/// use inky::render::ipc::shared_buffer_path;
///
/// // Connect to an inky process by PID
/// let perception = SharedPerception::open_pid(12345)?;
///
/// // Wait for updates and read text
/// loop {
///     if perception.poll_update()? {
///         let text = perception.as_text();
///         println!("Screen content:\n{}", text);
///     }
///     std::thread::sleep(std::time::Duration::from_millis(16));
/// }
/// ```
pub struct SharedPerception {
    buffer: SharedMemoryBuffer,
    last_generation: u64,
}

impl SharedPerception {
    /// Open a shared memory buffer at a specific path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file does not exist or is not a valid inky buffer.
    pub fn open(path: &Path) -> io::Result<Self> {
        let buffer = SharedMemoryBuffer::open(path)?;
        let last_generation = buffer.generation();
        Ok(Self {
            buffer,
            last_generation,
        })
    }

    /// Open a shared memory buffer for a specific process ID.
    ///
    /// This uses the default path for the given PID.
    ///
    /// # Errors
    ///
    /// Returns an error if no buffer exists for the PID.
    pub fn open_pid(pid: u32) -> io::Result<Self> {
        let path = crate::render::ipc::shared_buffer_path(pid);
        Self::open(&path)
    }

    /// Get the width of the buffer in cells.
    pub fn width(&self) -> u16 {
        self.buffer.width()
    }

    /// Get the height of the buffer in cells.
    pub fn height(&self) -> u16 {
        self.buffer.height()
    }

    /// Get the current generation counter.
    ///
    /// This counter increments each time the buffer is updated.
    pub fn generation(&self) -> u64 {
        self.buffer.generation()
    }

    /// Get the last update timestamp in microseconds since Unix epoch.
    pub fn last_update_us(&self) -> u64 {
        self.buffer.last_update_us()
    }

    /// Get the process ID that created the buffer.
    pub fn pid(&self) -> u32 {
        self.buffer.pid()
    }

    /// Check if the buffer has been updated since last read.
    ///
    /// Updates the internal generation counter if true.
    pub fn poll_update(&mut self) -> io::Result<bool> {
        let gen = self.buffer.generation();
        if gen > self.last_generation {
            self.last_generation = gen;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get direct access to the raw GPU cell data.
    ///
    /// This is zero-copy access to the shared memory region.
    pub fn cells(&self) -> &[GpuCell] {
        self.buffer.cells()
    }

    /// Get buffer content as plain text.
    ///
    /// Converts the GPU cells to a text representation with newlines
    /// separating rows.
    pub fn as_text(&self) -> String {
        let width = self.width() as usize;
        let height = self.height() as usize;
        let cells = self.cells();

        let mut result = String::with_capacity((width + 1) * height);

        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                if idx < cells.len() {
                    result.push(cells[idx].char());
                }
            }
            result.push('\n');
        }

        result
    }

    /// Read a rectangular region as text.
    pub fn read_region(&self, region: &Region) -> String {
        let width = self.width();
        let height = self.height();
        let cells = self.cells();

        let mut result = String::new();

        let end_y = (region.y + region.height).min(height);
        let end_x = (region.x + region.width).min(width);

        for y in region.y..end_y {
            for x in region.x..end_x {
                let idx = (y as usize) * (width as usize) + (x as usize);
                if idx < cells.len() {
                    result.push(cells[idx].char());
                }
            }
            result.push('\n');
        }

        result
    }

    /// Get the character at a specific position.
    pub fn char_at(&self, x: u16, y: u16) -> Option<char> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        let idx = (y as usize) * (self.width() as usize) + (x as usize);
        let cells = self.cells();
        if idx < cells.len() {
            Some(cells[idx].char())
        } else {
            None
        }
    }

    /// Find all occurrences of a string in the buffer.
    ///
    /// Returns a list of (x, y) positions where the string starts.
    pub fn find(&self, needle: &str) -> Vec<(u16, u16)> {
        let mut positions = Vec::new();
        let text = self.as_text();

        for (line_idx, line) in text.lines().enumerate() {
            for (char_idx, _) in line.match_indices(needle) {
                positions.push((char_idx as u16, line_idx as u16));
            }
        }

        positions
    }
}

/// List all discoverable shared buffers on the system.
///
/// Returns a list of PIDs and paths for all inky shared memory buffers.
/// This is useful for AI agents to discover available terminal buffers.
pub fn discover_shared_buffers() -> Vec<(u32, std::path::PathBuf)> {
    crate::render::ipc::list_shared_buffers()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::style::Color;
    #[cfg(feature = "image")]
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    #[test]
    fn test_perception_as_text() {
        let mut buffer = Buffer::new(5, 2);
        buffer.write_str(0, 0, "Hello", Color::White, Color::Black);
        buffer.write_str(0, 1, "World", Color::White, Color::Black);

        let perception = Perception::new(&buffer);
        let text = perception.as_text();

        assert_eq!(text, "Hello\nWorld\n");
    }

    #[test]
    fn test_perception_as_tokens() {
        let mut buffer = Buffer::new(12, 1);
        buffer.write_str(0, 0, "Hello World", Color::White, Color::Black);

        let perception = Perception::new(&buffer);
        let tokens = perception.as_tokens();

        // Should have "Hello", "World", and newline tokens
        assert!(tokens.iter().any(|t| t.text == "Hello"));
        assert!(tokens.iter().any(|t| t.text == "World"));
        assert!(tokens.iter().any(|t| t.text == "\n"));
    }

    #[test]
    fn test_perception_read_region() {
        let mut buffer = Buffer::new(10, 5);
        buffer.write_str(0, 0, "AAAAAAAAAA", Color::White, Color::Black);
        buffer.write_str(0, 1, "BBBBBBBBBB", Color::White, Color::Black);
        buffer.write_str(0, 2, "CCCCCCCCCC", Color::White, Color::Black);

        let perception = Perception::new(&buffer);
        let region = Region::new(2, 1, 3, 2);
        let text = perception.read_region(&region);

        assert_eq!(text, "BBB\nCCC\n");
    }

    #[test]
    fn test_perception_find() {
        let mut buffer = Buffer::new(20, 2);
        buffer.write_str(0, 0, "Hello World", Color::White, Color::Black);
        buffer.write_str(0, 1, "World Hello", Color::White, Color::Black);

        let perception = Perception::new(&buffer);
        let positions = perception.find("World");

        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], (6, 0)); // "Hello World"
        assert_eq!(positions[1], (0, 1)); // "World Hello"
    }

    #[test]
    fn test_semantic_diff_no_changes() {
        let buffer1 = Buffer::new(10, 5);
        let buffer2 = Buffer::new(10, 5);

        let diff = Perception::semantic_diff(&buffer1, &buffer2);

        assert!(diff.is_empty());
        assert_eq!(diff.cells_changed, 0);
    }

    #[test]
    fn test_semantic_diff_with_changes() {
        let mut buffer1 = Buffer::new(10, 3);
        buffer1.write_str(0, 0, "Line 1    ", Color::White, Color::Black);
        buffer1.write_str(0, 1, "Line 2    ", Color::White, Color::Black);

        let mut buffer2 = Buffer::new(10, 3);
        buffer2.write_str(0, 0, "Line 1    ", Color::White, Color::Black);
        buffer2.write_str(0, 1, "Changed   ", Color::White, Color::Black);

        let diff = Perception::semantic_diff(&buffer1, &buffer2);

        assert!(!diff.is_empty());
        assert_eq!(diff.modified_lines.len(), 1);
        assert_eq!(diff.modified_lines[0].0, 1);
    }

    #[test]
    fn test_region_contains() {
        let region = Region::new(5, 5, 10, 10);

        assert!(region.contains(5, 5));
        assert!(region.contains(10, 10));
        assert!(region.contains(14, 14));
        assert!(!region.contains(4, 5));
        assert!(!region.contains(15, 5));
    }

    #[test]
    fn test_region_intersects() {
        let r1 = Region::new(0, 0, 10, 10);
        let r2 = Region::new(5, 5, 10, 10);
        let r3 = Region::new(20, 20, 10, 10);

        assert!(r1.intersects(&r2));
        assert!(!r1.intersects(&r3));
    }

    #[test]
    fn test_token_creation() {
        let token = Token::new("hello", 5, 10);
        assert_eq!(token.text, "hello");
        assert_eq!(token.x, 5);
        assert_eq!(token.y, 10);
        assert!(!token.bold);

        let newline = Token::newline(3);
        assert_eq!(newline.text, "\n");
        assert_eq!(newline.y, 3);
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_perception_as_image_png_signature() {
        let buffer = Buffer::new(2, 1);
        let perception = Perception::new(&buffer);
        let png = perception.as_image(2);

        assert!(png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    }

    #[cfg(feature = "image")]
    #[test]
    fn test_perception_as_image_base64_roundtrip() {
        let mut buffer = Buffer::new(2, 1);
        buffer.write_str(0, 0, "Hi", Color::White, Color::Black);
        let perception = Perception::new(&buffer);

        let png = perception.as_image(2);
        let encoded = perception.as_image_base64(2);
        let decoded = BASE64
            .decode(encoded.as_bytes())
            .expect("Failed to decode perception image base64");

        assert_eq!(decoded, png);
    }

    // =========================================================================
    // SharedPerception Tests
    // =========================================================================

    #[test]
    fn test_shared_perception_open_close() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-perception.shm");

        // Create a shared buffer
        let mut shared = SharedMemoryBuffer::create_at(&path, 10, 5).unwrap();
        {
            let cells = shared.map_write();
            cells[0] = GpuCell::new('H');
            cells[1] = GpuCell::new('i');
        }
        shared.unmap();
        shared.submit();

        // Open via SharedPerception
        let perception = SharedPerception::open(&path).unwrap();
        assert_eq!(perception.width(), 10);
        assert_eq!(perception.height(), 5);
        assert_eq!(perception.generation(), 1);

        // Read text
        let text = perception.as_text();
        assert!(text.starts_with("Hi"));
    }

    #[test]
    fn test_shared_perception_poll_update() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-poll.shm");

        let mut shared = SharedMemoryBuffer::create_at(&path, 5, 2).unwrap();
        shared.submit();

        let mut perception = SharedPerception::open(&path).unwrap();

        // Initially no update (we already saw generation 1)
        assert!(!perception.poll_update().unwrap());

        // Write and submit
        {
            let cells = shared.map_write();
            cells[0] = GpuCell::new('X');
        }
        shared.unmap();
        shared.submit();

        // Now should detect update
        assert!(perception.poll_update().unwrap());

        // Second poll without changes should return false
        assert!(!perception.poll_update().unwrap());
    }

    #[test]
    fn test_shared_perception_read_region() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-region.shm");

        let mut shared = SharedMemoryBuffer::create_at(&path, 10, 5).unwrap();
        {
            let cells = shared.map_write();
            // Row 0: "AAAAAAAAAA"
            for cell in &mut cells[0..10] {
                *cell = GpuCell::new('A');
            }
            // Row 1: "BBBBBBBBBB"
            for cell in &mut cells[10..20] {
                *cell = GpuCell::new('B');
            }
            // Row 2: "CCCCCCCCCC"
            for cell in &mut cells[20..30] {
                *cell = GpuCell::new('C');
            }
        }
        shared.unmap();
        shared.submit();

        let perception = SharedPerception::open(&path).unwrap();
        let region = Region::new(2, 1, 3, 2);
        let text = perception.read_region(&region);

        assert_eq!(text, "BBB\nCCC\n");
    }

    #[test]
    fn test_shared_perception_char_at() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-char-at.shm");

        let mut shared = SharedMemoryBuffer::create_at(&path, 5, 3).unwrap();
        {
            let cells = shared.map_write();
            cells[0] = GpuCell::new('X');
            cells[6] = GpuCell::new('Y'); // Position (1, 1) = 1 + 1*5 = 6
        }
        shared.unmap();
        shared.submit();

        let perception = SharedPerception::open(&path).unwrap();

        assert_eq!(perception.char_at(0, 0), Some('X'));
        assert_eq!(perception.char_at(1, 1), Some('Y'));
        assert_eq!(perception.char_at(0, 1), Some(' ')); // Default blank
        assert_eq!(perception.char_at(10, 10), None); // Out of bounds
    }

    #[test]
    fn test_shared_perception_find() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-find.shm");

        let mut shared = SharedMemoryBuffer::create_at(&path, 20, 2).unwrap();
        {
            let cells = shared.map_write();
            // Row 0: "Hello World         "
            let row0 = "Hello World         ";
            for (i, c) in row0.chars().enumerate() {
                cells[i] = GpuCell::new(c);
            }
            // Row 1: "World Hello         "
            let row1 = "World Hello         ";
            for (i, c) in row1.chars().enumerate() {
                cells[20 + i] = GpuCell::new(c);
            }
        }
        shared.unmap();
        shared.submit();

        let perception = SharedPerception::open(&path).unwrap();
        let positions = perception.find("World");

        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], (6, 0)); // "Hello World"
        assert_eq!(positions[1], (0, 1)); // "World Hello"
    }

    #[test]
    fn test_discover_shared_buffers() {
        // This test just verifies the function doesn't panic
        // It may or may not find buffers depending on system state
        let buffers = discover_shared_buffers();
        // Buffers is a Vec, so we can at least check it's valid
        let _ = buffers.len();
    }
}
