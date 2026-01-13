//! High-level drawing API for painting to buffers.

use super::buffer::Buffer;
use super::cell::{Cell, CellFlags, PackedColor};
use crate::style::{BorderStyle, Color, Style, TextStyle, TextWrap};

/// Border character set for a specific border style.
/// Contains all 6 characters needed to draw a rectangular border.
struct BorderChars {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
}

impl BorderChars {
    const SINGLE: Self = Self {
        top_left: '┌',
        top_right: '┐',
        bottom_left: '└',
        bottom_right: '┘',
        horizontal: '─',
        vertical: '│',
    };

    const DOUBLE: Self = Self {
        top_left: '╔',
        top_right: '╗',
        bottom_left: '╚',
        bottom_right: '╝',
        horizontal: '═',
        vertical: '║',
    };

    const ROUNDED: Self = Self {
        top_left: '╭',
        top_right: '╮',
        bottom_left: '╰',
        bottom_right: '╯',
        horizontal: '─',
        vertical: '│',
    };

    const BOLD: Self = Self {
        top_left: '┏',
        top_right: '┓',
        bottom_left: '┗',
        bottom_right: '┛',
        horizontal: '━',
        vertical: '┃',
    };
}

/// High-level painter for rendering to a buffer.
pub struct Painter<'a> {
    buffer: &'a mut Buffer,
    /// Cursor screen position (x, y), set during text painting when cursor_position is provided.
    cursor_screen_pos: Option<(u16, u16)>,
}

impl<'a> Painter<'a> {
    /// Create a new painter for a buffer.
    pub fn new(buffer: &'a mut Buffer) -> Self {
        Self {
            buffer,
            cursor_screen_pos: None,
        }
    }

    /// Get the cursor screen position if one was set during painting.
    pub fn cursor_screen_pos(&self) -> Option<(u16, u16)> {
        self.cursor_screen_pos
    }

    /// Set the cursor screen position.
    pub fn set_cursor_screen_pos(&mut self, pos: Option<(u16, u16)>) {
        self.cursor_screen_pos = pos;
    }

    /// Get the underlying buffer.
    pub fn buffer(&self) -> &Buffer {
        self.buffer
    }

    /// Get mutable buffer.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }

    /// Paint a box (background and border).
    pub fn paint_box(&mut self, style: &Style, x: u16, y: u16, width: u16, height: u16) {
        // Paint background if specified
        if let Some(bg) = &style.background_color {
            let cell = Cell::blank().with_bg(*bg);
            self.buffer.fill(x, y, width, height, cell);
        }

        // Paint border if specified - use consolidated method with char sets
        let chars = match &style.border {
            BorderStyle::None => return,
            BorderStyle::Single => &BorderChars::SINGLE,
            BorderStyle::Double => &BorderChars::DOUBLE,
            BorderStyle::Rounded => &BorderChars::ROUNDED,
            BorderStyle::Bold => &BorderChars::BOLD,
        };
        self.paint_border(chars, x, y, width, height, style.background_color);
    }

    /// Paint text content.
    ///
    /// The `line_style` parameter, when provided, is used to fill the background
    /// of the entire line (not just the text content). This is useful for
    /// highlighting entire lines or applying line-level background colors.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_text(
        &mut self,
        content: &str,
        style: &TextStyle,
        line_style: Option<&TextStyle>,
        x: u16,
        y: u16,
        max_width: u16,
        max_height: u16,
    ) {
        if max_width == 0 {
            return;
        }

        let merged_style = line_style.map(|line_style| line_style.merge(style));
        let effective_style = merged_style.as_ref().unwrap_or(style);
        let fg = PackedColor::from(effective_style.color.unwrap_or(Color::White));
        let bg = PackedColor::from(effective_style.background_color.unwrap_or(Color::Default));
        let flags = Self::text_style_flags(effective_style);
        let line_cell = line_style.map(Self::cell_from_text_style);

        let max_height = if max_height == 0 {
            u16::MAX
        } else {
            max_height
        };

        // Use streaming approach to avoid Vec<String> allocation
        let mut row = 0u16;
        for_each_wrapped_line(content, effective_style.wrap, max_width, |line| {
            if row >= max_height {
                return false;
            }

            if let Some(cell) = line_cell {
                self.buffer.fill(x, y + row, max_width, 1, cell);
            }

            let mut col = 0u16;
            for c in line.chars() {
                if col >= max_width {
                    break;
                }

                let width = char_width(c) as u16;
                if col + width > max_width {
                    break;
                }

                let is_wide = width == 2;
                let mut cell = Cell::new(c);
                cell.set_fg(fg);
                cell.set_bg(bg);
                cell.flags = flags;
                if is_wide {
                    cell.flags |= CellFlags::WIDE_CHAR;
                }

                self.buffer.set(x + col, y + row, cell);

                // Handle wide characters
                if is_wide && col + 1 < max_width {
                    let mut spacer = Cell::blank();
                    spacer.set_fg(fg);
                    spacer.set_bg(bg);
                    spacer.flags = flags | CellFlags::WIDE_SPACER;
                    self.buffer.set(x + col + 1, y + row, spacer);
                }

                col += width;
            }

            row += 1;
            true
        });
    }

    /// Paint text content with cursor tracking.
    ///
    /// Similar to `paint_text`, but also tracks the screen position of the cursor.
    /// If `cursor_char_pos` is provided, the method calculates the screen coordinates
    /// of that character position and stores it in the painter's cursor_screen_pos.
    ///
    /// This is used by the render pipeline when a TextNode has a cursor_position set.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_text_with_cursor(
        &mut self,
        content: &str,
        style: &TextStyle,
        line_style: Option<&TextStyle>,
        x: u16,
        y: u16,
        max_width: u16,
        max_height: u16,
        cursor_char_pos: Option<usize>,
    ) {
        if max_width == 0 {
            return;
        }

        let merged_style = line_style.map(|line_style| line_style.merge(style));
        let effective_style = merged_style.as_ref().unwrap_or(style);
        let fg = PackedColor::from(effective_style.color.unwrap_or(Color::White));
        let bg = PackedColor::from(effective_style.background_color.unwrap_or(Color::Default));
        let flags = Self::text_style_flags(effective_style);
        let line_cell = line_style.map(Self::cell_from_text_style);

        let max_height = if max_height == 0 {
            u16::MAX
        } else {
            max_height
        };

        // Track character position for cursor calculation
        let mut char_index = 0usize;
        let cursor_pos = cursor_char_pos.unwrap_or(usize::MAX);
        let mut row = 0u16;

        // Use streaming approach with character tracking
        for_each_wrapped_line_with_info(
            content,
            effective_style.wrap,
            max_width,
            |line, is_end_of_orig_line| {
                if row >= max_height {
                    return false;
                }

                if let Some(cell) = line_cell {
                    self.buffer.fill(x, y + row, max_width, 1, cell);
                }

                let mut col = 0u16;
                for c in line.chars() {
                    // Check if cursor is at this character position
                    if char_index == cursor_pos {
                        self.cursor_screen_pos = Some((x + col, y + row));
                    }
                    char_index += 1;

                    if col >= max_width {
                        break;
                    }

                    let width = char_width(c) as u16;
                    if col + width > max_width {
                        break;
                    }

                    let is_wide = width == 2;
                    let mut cell = Cell::new(c);
                    cell.set_fg(fg);
                    cell.set_bg(bg);
                    cell.flags = flags;
                    if is_wide {
                        cell.flags |= CellFlags::WIDE_CHAR;
                    }

                    self.buffer.set(x + col, y + row, cell);

                    // Handle wide characters
                    if is_wide && col + 1 < max_width {
                        let mut spacer = Cell::blank();
                        spacer.set_fg(fg);
                        spacer.set_bg(bg);
                        spacer.flags = flags | CellFlags::WIDE_SPACER;
                        self.buffer.set(x + col + 1, y + row, spacer);
                    }

                    col += width;
                }

                // Check if cursor is at end of this line (after all characters)
                if char_index == cursor_pos {
                    self.cursor_screen_pos = Some((x + col, y + row));
                }

                // Account for newline character if this is the end of an original line
                if is_end_of_orig_line {
                    char_index += 1; // newline
                }

                row += 1;
                true
            },
        );

        // Handle cursor at very end of empty content
        if cursor_pos == 0 && content.is_empty() {
            self.cursor_screen_pos = Some((x, y));
        }
    }

    /// Paint styled spans (for ANSI text passthrough).
    ///
    /// Renders multiple spans with individual styling. This is used when
    /// rendering ANSI-escaped text or syntax-highlighted content.
    ///
    /// The `line_style` parameter, when provided, is used to fill the background
    /// of the entire line (not just the text content). This is useful for
    /// highlighting entire lines or applying line-level background colors.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_spans(
        &mut self,
        spans: &[crate::style::StyledSpan],
        default_style: &TextStyle,
        line_style: Option<&TextStyle>,
        x: u16,
        y: u16,
        max_width: u16,
        max_height: u16,
    ) {
        if max_width == 0 {
            return;
        }

        let merged_style = line_style.map(|line_style| line_style.merge(default_style));
        let effective_default = merged_style.as_ref().unwrap_or(default_style);
        let line_cell = line_style.map(Self::cell_from_text_style);

        let max_height = if max_height == 0 {
            u16::MAX
        } else {
            max_height
        };

        let mut col = 0u16;
        let mut row = 0u16;

        if let Some(cell) = line_cell {
            self.buffer.fill(x, y + row, max_width, 1, cell);
        }

        if spans.is_empty() {
            return;
        }

        for span in spans {
            // Determine style for this span (span style overrides defaults)
            let fg = PackedColor::from(
                span.color
                    .or(effective_default.color)
                    .unwrap_or(Color::White),
            );
            let bg = PackedColor::from(
                span.background_color
                    .or(effective_default.background_color)
                    .unwrap_or(Color::Default),
            );

            let mut flags = CellFlags::empty();
            if span.bold || effective_default.bold {
                flags |= CellFlags::BOLD;
            }
            if span.italic || effective_default.italic {
                flags |= CellFlags::ITALIC;
            }
            if span.underline || effective_default.underline {
                flags |= CellFlags::UNDERLINE;
            }
            if span.dim || effective_default.dim {
                flags |= CellFlags::DIM;
            }
            if span.strikethrough || effective_default.strikethrough {
                flags |= CellFlags::STRIKETHROUGH;
            }

            // Render each character in the span
            for c in span.text.chars() {
                // Handle newlines
                if c == '\n' {
                    row += 1;
                    col = 0;
                    if row >= max_height {
                        return;
                    }
                    if let Some(cell) = line_cell {
                        self.buffer.fill(x, y + row, max_width, 1, cell);
                    }
                    continue;
                }

                // Handle carriage return
                if c == '\r' {
                    col = 0;
                    continue;
                }

                // Skip control characters
                if c.is_control() {
                    continue;
                }

                // Word wrapping: move to next line if at end
                let char_w = char_width(c) as u16;
                if col + char_w > max_width {
                    // Only wrap if configured to do so
                    if effective_default.wrap == TextWrap::Wrap {
                        row += 1;
                        col = 0;
                        if row >= max_height {
                            return;
                        }
                        if let Some(cell) = line_cell {
                            self.buffer.fill(x, y + row, max_width, 1, cell);
                        }
                    } else if effective_default.wrap == TextWrap::NoWrap {
                        // Continue without wrapping (off-screen)
                        continue;
                    } else {
                        // Truncate modes - stop rendering
                        return;
                    }
                }

                let is_wide = char_w == 2;
                let mut cell = Cell::new(c);
                cell.set_fg(fg);
                cell.set_bg(bg);
                cell.flags = flags;
                if is_wide {
                    cell.flags |= CellFlags::WIDE_CHAR;
                }

                self.buffer.set(x + col, y + row, cell);

                // Handle wide characters
                if is_wide && col + 1 < max_width {
                    let mut spacer = Cell::blank();
                    spacer.set_fg(fg);
                    spacer.set_bg(bg);
                    spacer.flags = flags | CellFlags::WIDE_SPACER;
                    self.buffer.set(x + col + 1, y + row, spacer);
                }

                col += char_w;
            }
        }
    }

    /// Paint styled spans with cursor tracking.
    ///
    /// Similar to `paint_spans`, but also tracks the screen position of the cursor.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_spans_with_cursor(
        &mut self,
        spans: &[crate::style::StyledSpan],
        default_style: &TextStyle,
        line_style: Option<&TextStyle>,
        x: u16,
        y: u16,
        max_width: u16,
        max_height: u16,
        cursor_char_pos: Option<usize>,
    ) {
        if max_width == 0 {
            return;
        }

        let merged_style = line_style.map(|line_style| line_style.merge(default_style));
        let effective_default = merged_style.as_ref().unwrap_or(default_style);
        let line_cell = line_style.map(Self::cell_from_text_style);

        let max_height = if max_height == 0 {
            u16::MAX
        } else {
            max_height
        };

        let mut col = 0u16;
        let mut row = 0u16;
        let cursor_pos = cursor_char_pos.unwrap_or(usize::MAX);
        let mut char_index = 0usize;

        if let Some(cell) = line_cell {
            self.buffer.fill(x, y + row, max_width, 1, cell);
        }

        if spans.is_empty() {
            if cursor_pos == 0 {
                self.cursor_screen_pos = Some((x, y));
            }
            return;
        }

        for span in spans {
            // Determine style for this span (span style overrides defaults)
            let fg = PackedColor::from(
                span.color
                    .or(effective_default.color)
                    .unwrap_or(Color::White),
            );
            let bg = PackedColor::from(
                span.background_color
                    .or(effective_default.background_color)
                    .unwrap_or(Color::Default),
            );

            let mut flags = CellFlags::empty();
            if span.bold || effective_default.bold {
                flags |= CellFlags::BOLD;
            }
            if span.italic || effective_default.italic {
                flags |= CellFlags::ITALIC;
            }
            if span.underline || effective_default.underline {
                flags |= CellFlags::UNDERLINE;
            }
            if span.dim || effective_default.dim {
                flags |= CellFlags::DIM;
            }
            if span.strikethrough || effective_default.strikethrough {
                flags |= CellFlags::STRIKETHROUGH;
            }

            // Render each character in the span
            for c in span.text.chars() {
                // Handle newlines
                if c == '\n' {
                    if char_index == cursor_pos {
                        self.cursor_screen_pos = Some((x + col, y + row));
                    }
                    char_index += 1;
                    row += 1;
                    col = 0;
                    if row >= max_height {
                        return;
                    }
                    if let Some(cell) = line_cell {
                        self.buffer.fill(x, y + row, max_width, 1, cell);
                    }
                    continue;
                }

                // Handle carriage return
                if c == '\r' {
                    if char_index == cursor_pos {
                        self.cursor_screen_pos = Some((x + col, y + row));
                    }
                    char_index += 1;
                    col = 0;
                    continue;
                }

                // Skip control characters
                if c.is_control() {
                    if char_index == cursor_pos {
                        self.cursor_screen_pos = Some((x + col, y + row));
                    }
                    char_index += 1;
                    continue;
                }

                // Word wrapping: move to next line if at end
                let char_w = char_width(c) as u16;
                let mut wrapped_line = false;
                if col + char_w > max_width {
                    // Only wrap if configured to do so
                    if effective_default.wrap == TextWrap::Wrap {
                        if char_index == cursor_pos {
                            self.cursor_screen_pos = Some((x + col, y + row));
                            wrapped_line = true;
                        }
                        row += 1;
                        col = 0;
                        if row >= max_height {
                            return;
                        }
                        if let Some(cell) = line_cell {
                            self.buffer.fill(x, y + row, max_width, 1, cell);
                        }
                    } else if effective_default.wrap == TextWrap::NoWrap {
                        // Continue without wrapping (off-screen)
                        continue;
                    } else {
                        // Truncate modes - stop rendering
                        return;
                    }
                }

                if char_index == cursor_pos && !wrapped_line {
                    self.cursor_screen_pos = Some((x + col, y + row));
                }
                char_index += 1;

                let is_wide = char_w == 2;
                let mut cell = Cell::new(c);
                cell.set_fg(fg);
                cell.set_bg(bg);
                cell.flags = flags;
                if is_wide {
                    cell.flags |= CellFlags::WIDE_CHAR;
                }

                self.buffer.set(x + col, y + row, cell);

                // Handle wide characters
                if is_wide && col + 1 < max_width {
                    let mut spacer = Cell::blank();
                    spacer.set_fg(fg);
                    spacer.set_bg(bg);
                    spacer.flags = flags | CellFlags::WIDE_SPACER;
                    self.buffer.set(x + col + 1, y + row, spacer);
                }

                col += char_w;
            }
        }

        if char_index == cursor_pos {
            self.cursor_screen_pos = Some((x + col, y + row));
        }
    }

    /// Consolidated border painting using character set.
    /// Reduces code duplication from 4 nearly identical methods to 1.
    fn paint_border(
        &mut self,
        chars: &BorderChars,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        bg: Option<Color>,
    ) {
        if w < 2 || h < 2 {
            return;
        }

        let bg = bg.map(PackedColor::from).unwrap_or(PackedColor::BLACK);

        // Corners
        self.set_border_char(x, y, chars.top_left, bg);
        self.set_border_char(x + w - 1, y, chars.top_right, bg);
        self.set_border_char(x, y + h - 1, chars.bottom_left, bg);
        self.set_border_char(x + w - 1, y + h - 1, chars.bottom_right, bg);

        // Top and bottom edges
        for dx in 1..w - 1 {
            self.set_border_char(x + dx, y, chars.horizontal, bg);
            self.set_border_char(x + dx, y + h - 1, chars.horizontal, bg);
        }

        // Left and right edges
        for dy in 1..h - 1 {
            self.set_border_char(x, y + dy, chars.vertical, bg);
            self.set_border_char(x + w - 1, y + dy, chars.vertical, bg);
        }
    }

    fn set_border_char(&mut self, x: u16, y: u16, c: char, bg: PackedColor) {
        let mut cell = Cell::new(c);
        cell.set_bg(bg);
        self.buffer.set(x, y, cell);
    }

    /// Create CellFlags from a TextStyle.
    fn text_style_flags(style: &TextStyle) -> CellFlags {
        let mut flags = CellFlags::empty();
        if style.bold {
            flags |= CellFlags::BOLD;
        }
        if style.italic {
            flags |= CellFlags::ITALIC;
        }
        if style.underline {
            flags |= CellFlags::UNDERLINE;
        }
        if style.dim {
            flags |= CellFlags::DIM;
        }
        if style.strikethrough {
            flags |= CellFlags::STRIKETHROUGH;
        }
        flags
    }

    /// Create a blank cell with styling from a TextStyle.
    /// Used for filling line backgrounds.
    fn cell_from_text_style(style: &TextStyle) -> Cell {
        let fg = PackedColor::from(style.color.unwrap_or(Color::White));
        let bg = PackedColor::from(style.background_color.unwrap_or(Color::Default));
        let flags = Self::text_style_flags(style);

        let mut cell = Cell::blank();
        cell.set_fg(fg);
        cell.set_bg(bg);
        cell.flags = flags;
        cell
    }
}

/// Get character display width with ASCII fast path.
/// ASCII characters (0x00-0x7F) are always width 1, avoiding unicode lookup.
#[inline]
fn char_width(c: char) -> usize {
    if c.is_ascii() {
        1 // ASCII is always width 1
    } else {
        unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
    }
}

fn line_width(line: &str) -> usize {
    line.chars().map(char_width).sum()
}

// ============================================================================
// STREAMING TEXT WRAPPING (zero-allocation)
// ============================================================================

/// Process wrapped content by calling a closure for each line.
/// This avoids allocating a Vec<String> - each line is processed inline.
fn for_each_wrapped_line<F>(content: &str, mode: TextWrap, max_width: u16, mut f: F)
where
    F: FnMut(&str) -> bool, // Returns false to stop iteration
{
    let max_width = max_width as usize;
    let mut had_content = false;

    match mode {
        TextWrap::Wrap => {
            for line in content.split('\n') {
                if !for_each_wrapped_segment(line, max_width, |segment| {
                    had_content = true;
                    f(segment)
                }) {
                    return;
                }
            }
            // Handle empty content
            if !had_content {
                f("");
            }
        }
        TextWrap::NoWrap => {
            for line in content.split('\n') {
                if !f(line) {
                    return;
                }
            }
        }
        TextWrap::Truncate | TextWrap::TruncateStart | TextWrap::TruncateMiddle => {
            // Truncation modes use a reusable buffer
            let mut truncated = String::with_capacity(max_width + 4);
            for line in content.split('\n') {
                truncated.clear();
                truncate_line_into(line, max_width, mode, &mut truncated);
                if !f(&truncated) {
                    return;
                }
            }
        }
    }
}

/// Process wrapped content with additional info for cursor tracking.
/// Uses peekable iterator to avoid collecting lines into Vec.
fn for_each_wrapped_line_with_info<F>(content: &str, mode: TextWrap, max_width: u16, mut f: F)
where
    F: FnMut(&str, bool) -> bool, // (line, is_end_of_original_line) -> continue
{
    let max_width = max_width as usize;
    let mut lines = content.split('\n').peekable();

    match mode {
        TextWrap::Wrap => {
            let mut had_content = false;
            while let Some(line) = lines.next() {
                let is_last_orig = lines.peek().is_none();
                let mut seg_count = 0;

                if !for_each_wrapped_segment_with_last(line, max_width, |seg, is_last_seg| {
                    had_content = true;
                    seg_count += 1;
                    let is_end = is_last_seg && !is_last_orig;
                    f(seg, is_end)
                }) {
                    return;
                }

                // Handle empty lines
                if seg_count == 0 {
                    had_content = true;
                    if !f("", !is_last_orig) {
                        return;
                    }
                }
            }
            if !had_content {
                f("", false);
            }
        }
        TextWrap::NoWrap => {
            while let Some(line) = lines.next() {
                let is_last = lines.peek().is_none();
                if !f(line, !is_last) {
                    return;
                }
            }
        }
        TextWrap::Truncate | TextWrap::TruncateStart | TextWrap::TruncateMiddle => {
            let mut truncated = String::with_capacity(max_width + 4);
            while let Some(line) = lines.next() {
                let is_last = lines.peek().is_none();
                truncated.clear();
                truncate_line_into(line, max_width, mode, &mut truncated);
                if !f(&truncated, !is_last) {
                    return;
                }
            }
        }
    }
}

/// Process each wrapped segment of a single line.
fn for_each_wrapped_segment<F>(line: &str, max_width: usize, mut f: F) -> bool
where
    F: FnMut(&str) -> bool,
{
    for_each_wrapped_segment_with_last(line, max_width, |seg, _| f(seg))
}

/// Process each wrapped segment with a flag indicating if it's the last segment.
fn for_each_wrapped_segment_with_last<F>(line: &str, max_width: usize, mut f: F) -> bool
where
    F: FnMut(&str, bool) -> bool,
{
    if max_width == 0 {
        return true;
    }
    if line.is_empty() {
        return f("", true);
    }

    // Fast path: if line fits, no wrapping needed
    let total_width = line_width(line);
    if total_width <= max_width {
        return f(line, true);
    }

    // Wrap character by character
    let mut start_byte = 0;
    let mut current_width = 0;
    let mut last_break_byte: Option<usize> = None;
    let mut last_break_width = 0;

    for (byte_idx, c) in line.char_indices() {
        let w = char_width(c);

        if c.is_whitespace() {
            last_break_byte = Some(byte_idx + c.len_utf8());
            last_break_width = current_width + w;
        }

        if current_width + w > max_width {
            let (end_byte, new_start, new_width) =
                if let Some(brk) = last_break_byte.filter(|&b| b > start_byte) {
                    (brk, brk, current_width - last_break_width + w)
                } else {
                    (byte_idx, byte_idx, w)
                };

            let segment = line[start_byte..end_byte].trim_end();
            if !f(segment, false) {
                return false;
            }

            start_byte = new_start;
            current_width = new_width;
            last_break_byte = None;
        } else {
            current_width += w;
        }
    }

    // Emit remaining segment (last one)
    if start_byte < line.len() && !f(&line[start_byte..], true) {
        return false;
    }

    true
}

/// Truncate a line into a pre-allocated buffer.
fn truncate_line_into(line: &str, max_width: usize, mode: TextWrap, out: &mut String) {
    if max_width == 0 {
        return;
    }
    let lw = line_width(line);
    if lw <= max_width {
        out.push_str(line);
        return;
    }

    let ellipsis = "...";
    let ellipsis_width = 3;
    if ellipsis_width >= max_width {
        take_prefix_into(line, max_width, out);
        return;
    }

    let keep = max_width - ellipsis_width;
    match mode {
        TextWrap::Truncate => {
            take_prefix_into(line, keep, out);
            out.push_str(ellipsis);
        }
        TextWrap::TruncateStart => {
            out.push_str(ellipsis);
            take_suffix_into(line, keep, out);
        }
        TextWrap::TruncateMiddle => {
            let right = keep / 2;
            let left = keep - right;
            take_prefix_into(line, left, out);
            out.push_str(ellipsis);
            take_suffix_into(line, right, out);
        }
        _ => out.push_str(line),
    }
}

/// Take prefix by width into buffer.
fn take_prefix_into(line: &str, max_width: usize, out: &mut String) {
    let mut width = 0;
    for c in line.chars() {
        let w = char_width(c);
        if width + w > max_width {
            break;
        }
        width += w;
        out.push(c);
    }
}

/// Take suffix by width into buffer.
fn take_suffix_into(line: &str, max_width: usize, out: &mut String) {
    if max_width == 0 {
        return;
    }
    let mut width = 0;
    let mut start = line.len();
    for (idx, c) in line.char_indices().rev() {
        let w = char_width(c);
        if width + w > max_width {
            break;
        }
        width += w;
        start = idx;
    }
    out.push_str(&line[start..]);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::render::cell::{CellFlags, PackedColor};
    use crate::render::Buffer;
    use crate::style::Color;
    use crate::style::{StyledSpan, TextStyle};

    fn style_with_wrap(wrap: TextWrap) -> TextStyle {
        TextStyle {
            wrap,
            ..TextStyle::default()
        }
    }

    #[test]
    fn paint_text_wraps_content() {
        let mut buffer = Buffer::new(6, 2);
        let mut painter = Painter::new(&mut buffer);
        let style = style_with_wrap(TextWrap::Wrap);

        painter.paint_text("Hello world", &style, None, 0, 0, 6, 2);

        assert_eq!(buffer.get(0, 0).unwrap().char(), 'H');
        assert_eq!(buffer.get(4, 0).unwrap().char(), 'o');
        assert_eq!(buffer.get(0, 1).unwrap().char(), 'w');
        assert_eq!(buffer.get(4, 1).unwrap().char(), 'd');
    }

    #[test]
    fn paint_text_truncates_end() {
        let mut buffer = Buffer::new(5, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = style_with_wrap(TextWrap::Truncate);

        painter.paint_text("Hello world", &style, None, 0, 0, 5, 1);

        assert_eq!(buffer.get(0, 0).unwrap().char(), 'H');
        assert_eq!(buffer.get(1, 0).unwrap().char(), 'e');
        assert_eq!(buffer.get(2, 0).unwrap().char(), '.');
        assert_eq!(buffer.get(3, 0).unwrap().char(), '.');
        assert_eq!(buffer.get(4, 0).unwrap().char(), '.');
    }

    #[test]
    fn paint_text_truncates_start() {
        let mut buffer = Buffer::new(5, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = style_with_wrap(TextWrap::TruncateStart);

        painter.paint_text("Hello world", &style, None, 0, 0, 5, 1);

        assert_eq!(buffer.get(0, 0).unwrap().char(), '.');
        assert_eq!(buffer.get(1, 0).unwrap().char(), '.');
        assert_eq!(buffer.get(2, 0).unwrap().char(), '.');
        assert_eq!(buffer.get(3, 0).unwrap().char(), 'l');
        assert_eq!(buffer.get(4, 0).unwrap().char(), 'd');
    }

    #[test]
    fn paint_text_truncates_middle() {
        let mut buffer = Buffer::new(5, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = style_with_wrap(TextWrap::TruncateMiddle);

        painter.paint_text("Hello world", &style, None, 0, 0, 5, 1);

        assert_eq!(buffer.get(0, 0).unwrap().char(), 'H');
        assert_eq!(buffer.get(1, 0).unwrap().char(), '.');
        assert_eq!(buffer.get(2, 0).unwrap().char(), '.');
        assert_eq!(buffer.get(3, 0).unwrap().char(), '.');
        assert_eq!(buffer.get(4, 0).unwrap().char(), 'd');
    }

    #[test]
    fn paint_text_sets_wide_char_spacer_background() {
        let mut buffer = Buffer::new(3, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = TextStyle {
            background_color: Some(Color::Blue),
            ..TextStyle::default()
        };

        painter.paint_text("好", &style, None, 0, 0, 3, 1);

        let first = buffer.get(0, 0).unwrap();
        let spacer = buffer.get(1, 0).unwrap();

        assert!(first.flags.contains(CellFlags::WIDE_CHAR));
        assert!(spacer.flags.contains(CellFlags::WIDE_SPACER));
        // Both cells should have the same background color
        assert_eq!(first.bg(), spacer.bg());
        // Background should be blue (accounting for RGB565 quantization)
        let bg = first.bg();
        assert_eq!(bg.r, 0);
        assert_eq!(bg.g, 0);
        assert!(bg.b > 100); // Blue component should be significant
    }

    #[test]
    fn paint_text_applies_line_style_background() {
        let mut buffer = Buffer::new(4, 1);
        let mut painter = Painter::new(&mut buffer);
        let line_style = TextStyle::new().bg(Color::Red);

        painter.paint_text("Hi", &TextStyle::default(), Some(&line_style), 0, 0, 4, 1);

        // RGB565 packing causes quantization, so we compare round-tripped values
        let line_bg = PackedColor::from_rgb565(PackedColor::from(Color::Red).to_rgb565());
        assert_eq!(buffer.get(0, 0).unwrap().bg(), line_bg);
        assert_eq!(buffer.get(2, 0).unwrap().bg(), line_bg);
    }

    #[test]
    fn paint_spans_applies_line_style_background() {
        let mut buffer = Buffer::new(3, 1);
        let mut painter = Painter::new(&mut buffer);
        let spans = vec![StyledSpan::new("Hi")];
        let line_style = TextStyle::new().bg(Color::Blue);

        painter.paint_spans(&spans, &TextStyle::default(), Some(&line_style), 0, 0, 3, 1);

        // RGB565 packing causes quantization, so we compare round-tripped values
        let line_bg = PackedColor::from_rgb565(PackedColor::from(Color::Blue).to_rgb565());
        assert_eq!(buffer.get(2, 0).unwrap().bg(), line_bg);
    }

    // === Cursor tracking tests ===

    #[test]
    fn paint_text_with_cursor_at_start() {
        let mut buffer = Buffer::new(10, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = TextStyle::default();

        painter.paint_text_with_cursor("Hello", &style, None, 2, 3, 10, 1, Some(0));

        assert_eq!(painter.cursor_screen_pos(), Some((2, 3)));
    }

    #[test]
    fn paint_text_with_cursor_in_middle() {
        let mut buffer = Buffer::new(10, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = TextStyle::default();

        painter.paint_text_with_cursor("Hello", &style, None, 2, 3, 10, 1, Some(3));

        // Cursor at index 3 means after "Hel", so x = 2 + 3 = 5
        assert_eq!(painter.cursor_screen_pos(), Some((5, 3)));
    }

    #[test]
    fn paint_text_with_cursor_at_end() {
        let mut buffer = Buffer::new(10, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = TextStyle::default();

        painter.paint_text_with_cursor("Hello", &style, None, 2, 3, 10, 1, Some(5));

        // Cursor at index 5 means after "Hello", so x = 2 + 5 = 7
        assert_eq!(painter.cursor_screen_pos(), Some((7, 3)));
    }

    #[test]
    fn paint_text_with_cursor_no_cursor_set() {
        let mut buffer = Buffer::new(10, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = TextStyle::default();

        painter.paint_text_with_cursor("Hello", &style, None, 2, 3, 10, 1, None);

        assert_eq!(painter.cursor_screen_pos(), None);
    }

    #[test]
    fn paint_text_with_cursor_empty_content() {
        let mut buffer = Buffer::new(10, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = TextStyle::default();

        painter.paint_text_with_cursor("", &style, None, 5, 7, 10, 1, Some(0));

        assert_eq!(painter.cursor_screen_pos(), Some((5, 7)));
    }

    #[test]
    fn paint_text_with_cursor_wide_characters() {
        let mut buffer = Buffer::new(10, 1);
        let mut painter = Painter::new(&mut buffer);
        let style = TextStyle::default();

        // "好" is a wide character (2 cells)
        painter.paint_text_with_cursor("好X", &style, None, 0, 0, 10, 1, Some(1));

        // After wide char "好", cursor should be at column 2
        assert_eq!(painter.cursor_screen_pos(), Some((2, 0)));
    }

    #[test]
    fn paint_spans_with_cursor_in_second_span() {
        let mut buffer = Buffer::new(10, 1);
        let mut painter = Painter::new(&mut buffer);
        let spans = vec![
            StyledSpan::new("He").color(Color::Red),
            StyledSpan::new("llo"),
        ];
        let style = TextStyle::default();

        painter.paint_spans_with_cursor(&spans, &style, None, 1, 2, 10, 1, Some(3));

        assert_eq!(painter.cursor_screen_pos(), Some((4, 2)));
    }

    #[test]
    fn paint_spans_with_cursor_wrap_line_end() {
        let mut buffer = Buffer::new(4, 2);
        let mut painter = Painter::new(&mut buffer);
        let spans = vec![StyledSpan::new("abcd")];
        let style = TextStyle::default();

        painter.paint_spans_with_cursor(&spans, &style, None, 0, 0, 2, 2, Some(2));

        assert_eq!(painter.cursor_screen_pos(), Some((2, 0)));
    }

    #[test]
    fn cursor_screen_pos_get_and_set() {
        let mut buffer = Buffer::new(10, 1);
        let mut painter = Painter::new(&mut buffer);

        assert_eq!(painter.cursor_screen_pos(), None);

        painter.set_cursor_screen_pos(Some((5, 10)));
        assert_eq!(painter.cursor_screen_pos(), Some((5, 10)));

        painter.set_cursor_screen_pos(None);
        assert_eq!(painter.cursor_screen_pos(), None);
    }
}
