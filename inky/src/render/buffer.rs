//! 2D cell buffer for terminal rendering.

use super::cell::{Cell, CellFlags, PackedColor};
use crate::style::Color;

/// Compare two cell slices for equality using SIMD-friendly byte comparison.
///
/// This function compares cells as raw bytes, enabling the compiler to auto-vectorize
/// the comparison. On x86_64 with AVX2, this can compare 32 bytes (4 cells) at a time,
/// providing ~4-8x speedup over cell-by-cell comparison.
///
/// # Safety
///
/// This is safe because Cell is `#[repr(C)]` with all primitive fields,
/// making byte-wise comparison equivalent to field-wise comparison.
#[inline]
pub fn cells_equal(a: &[Cell], b: &[Cell]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    // Compare as byte slices to enable SIMD auto-vectorization
    // SAFETY: Cell is #[repr(C)] with primitive fields, so byte comparison is valid
    let a_bytes =
        unsafe { std::slice::from_raw_parts(a.as_ptr().cast::<u8>(), std::mem::size_of_val(a)) };
    // SAFETY: Cell is #[repr(C)] with primitive fields, so byte comparison is valid.
    let b_bytes =
        unsafe { std::slice::from_raw_parts(b.as_ptr().cast::<u8>(), std::mem::size_of_val(b)) };

    a_bytes == b_bytes
}

/// Compare two rows of cells and return whether they differ.
///
/// Uses SIMD-optimized comparison when cells are properly aligned.
/// This is the primary function for dirty row detection.
#[inline]
pub fn rows_differ(a: &[Cell], b: &[Cell]) -> bool {
    !cells_equal(a, b)
}

/// 2D buffer of terminal cells.
#[derive(Clone)]
pub struct Buffer {
    cells: Vec<Cell>,
    width: u16,
    height: u16,
}

impl Buffer {
    /// Create a new buffer filled with blank cells.
    pub fn new(width: u16, height: u16) -> Self {
        // Use saturating_mul to prevent overflow on 32-bit systems
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            cells: vec![Cell::blank(); size],
            width,
            height,
        }
    }

    /// Get buffer width.
    #[inline]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Get buffer height.
    #[inline]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// Check if coordinates are within buffer bounds.
    #[inline]
    pub fn in_bounds(&self, x: u16, y: u16) -> bool {
        x < self.width && y < self.height
    }

    /// Calculate linear index from coordinates.
    /// Returns None if out of bounds.
    #[inline]
    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if self.in_bounds(x, y) {
            Some((y as usize) * (self.width as usize) + (x as usize))
        } else {
            None
        }
    }

    /// Get cell at position.
    #[inline]
    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|idx| &self.cells[idx])
    }

    /// Get mutable cell at position.
    #[inline]
    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        self.index(x, y).map(|idx| &mut self.cells[idx])
    }

    /// Set cell at position, marking it dirty.
    #[inline]
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(idx) = self.index(x, y) {
            if self.cells[idx] != cell {
                self.cells[idx] = cell;
                self.cells[idx].mark_dirty();
            }
        }
    }

    /// Clear buffer to blank cells.
    ///
    /// **Performance note:** This marks ALL cells as dirty, forcing a full redraw.
    /// For incremental rendering, prefer not clearing the buffer between frames
    /// and instead let paint operations overwrite only the cells they need.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::blank();
            cell.mark_dirty();
        }
    }

    /// Soft reset the buffer without marking cells dirty.
    ///
    /// This clears the buffer content but preserves dirty tracking,
    /// allowing incremental rendering to detect actual changes.
    /// Use this when you need a clean slate but want efficient diffing.
    pub fn soft_clear(&mut self) {
        let blank = Cell::blank();
        for cell in &mut self.cells {
            if *cell != blank {
                *cell = blank;
                cell.mark_dirty();
            }
        }
    }

    /// Fill a rectangular region with a cell.
    ///
    /// Optimized to use direct slice access instead of individual set() calls.
    /// This avoids bounds checking per cell and enables compiler vectorization.
    pub fn fill(&mut self, x: u16, y: u16, w: u16, h: u16, mut cell: Cell) {
        // Early bounds check
        if x >= self.width || y >= self.height || w == 0 || h == 0 {
            return;
        }

        // Clamp dimensions to buffer bounds using saturating_add to prevent overflow
        let end_x = x.saturating_add(w).min(self.width) as usize;
        let end_y = y.saturating_add(h).min(self.height) as usize;
        let x = x as usize;
        let y = y as usize;
        let width = self.width as usize;

        // Pre-mark cell as dirty
        cell.mark_dirty();

        // Fill row by row using slice operations
        for row in y..end_y {
            let row_start = row * width;
            let start = row_start + x;
            let end = row_start + end_x;
            self.cells[start..end].fill(cell);
        }
    }

    /// Write text at position.
    pub fn write_str(&mut self, x: u16, y: u16, text: &str, fg: Color, bg: Color) {
        let fg = PackedColor::from(fg);
        let bg = PackedColor::from(bg);
        let mut col = x;

        for c in text.chars() {
            if col >= self.width {
                break;
            }

            let width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) as u16;
            if col + width > self.width {
                break;
            }

            let is_wide = width == 2;
            let mut cell = Cell::new(c);
            cell.set_fg(fg);
            cell.set_bg(bg);
            if is_wide {
                cell.flags |= CellFlags::WIDE_CHAR;
            }
            self.set(col, y, cell);

            // Handle wide characters
            if is_wide {
                let mut spacer = Cell::blank();
                spacer.set_fg(fg);
                spacer.set_bg(bg);
                spacer.flags |= CellFlags::WIDE_SPACER;
                self.set(col + 1, y, spacer);
            }

            col += width;
        }
    }

    /// Get raw cell slice (for AI/GPU access).
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Get mutable raw cell slice (for AI/GPU access).
    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    /// Iterate over buffer characters row by row.
    ///
    /// Returns an iterator that yields each character followed by newlines
    /// at the end of each row. This allows zero-allocation streaming when
    /// only iterating (not collecting).
    ///
    /// # Example
    ///
    /// ```
    /// use inky::render::Buffer;
    ///
    /// let buf = Buffer::new(2, 2);
    /// let chars: Vec<_> = buf.chars().collect();
    /// assert_eq!(chars, vec![' ', ' ', '\n', ' ', ' ', '\n']);
    /// ```
    pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
        (0..self.height).flat_map(move |y| {
            (0..self.width)
                .map(move |x| {
                    let idx = (y as usize)
                        .saturating_mul(self.width as usize)
                        .saturating_add(x as usize);
                    self.cells.get(idx).map_or(' ', |cell| cell.char())
                })
                .chain(std::iter::once('\n'))
        })
    }

    /// Convert buffer to plain text.
    ///
    /// For streaming consumers that don't need the full string,
    /// use [`chars()`](Self::chars) instead to avoid allocation.
    pub fn to_text(&self) -> String {
        self.chars().collect()
    }

    /// Resize buffer, preserving content where possible.
    pub fn resize(&mut self, new_width: u16, new_height: u16) {
        if new_width == self.width && new_height == self.height {
            return;
        }

        // Use saturating_mul to prevent overflow on 32-bit systems
        let new_size = (new_width as usize).saturating_mul(new_height as usize);
        let mut new_cells = vec![Cell::blank(); new_size];

        // Copy existing content
        let copy_width = self.width.min(new_width);
        let copy_height = self.height.min(new_height);

        let old_width = self.width as usize;
        let new_w = new_width as usize;
        let copy_w = copy_width as usize;
        for y in 0..copy_height as usize {
            let old_start = y * old_width;
            let new_start = y * new_w;
            new_cells[new_start..new_start + copy_w]
                .copy_from_slice(&self.cells[old_start..old_start + copy_w]);
        }

        self.cells = new_cells;
        self.width = new_width;
        self.height = new_height;

        // Mark all as dirty after resize
        for cell in &mut self.cells {
            cell.mark_dirty();
        }
    }

    /// Clear all dirty flags.
    pub fn clear_dirty(&mut self) {
        for cell in &mut self.cells {
            cell.clear_dirty();
        }
    }

    /// Check if any cells are dirty.
    pub fn has_dirty(&self) -> bool {
        self.cells.iter().any(|c| c.is_dirty())
    }

    /// Get dirty rows as an iterator.
    ///
    /// Returns an iterator that yields row indices (y coordinates) containing
    /// at least one dirty cell. This avoids allocating a Vec on every call.
    ///
    /// # Performance
    ///
    /// O(height * width) worst case, but short-circuits per row when a dirty
    /// cell is found. Lazy evaluation means unused rows aren't scanned.
    pub fn dirty_rows(&self) -> impl Iterator<Item = u16> + '_ {
        let width = self.width as usize;
        (0..self.height).filter(move |&y| {
            let row_start = (y as usize) * width;
            let row_end = row_start + width;
            self.cells[row_start..row_end].iter().any(|c| c.is_dirty())
        })
    }

    /// Get a row of cells as a slice.
    ///
    /// Returns `None` if the row index is out of bounds.
    #[inline]
    pub fn row(&self, y: u16) -> Option<&[Cell]> {
        if y < self.height {
            let width = self.width as usize;
            let start = (y as usize) * width;
            Some(&self.cells[start..start + width])
        } else {
            None
        }
    }

    // =========================================================================
    // Direct Buffer Access API (for porter/custom rendering)
    // =========================================================================

    /// Write a single cell at position (direct buffer access API).
    ///
    /// This is an alias for `set()` with a more explicit name for custom rendering.
    /// The cell is marked dirty automatically.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::render::{Buffer, Cell};
    /// use inky::style::Color;
    ///
    /// let mut buf = Buffer::new(80, 24);
    /// let cell = Cell::new('X').with_fg(Color::Red);
    /// buf.write_cell(10, 5, cell);
    /// ```
    #[inline]
    pub fn write_cell(&mut self, x: u16, y: u16, cell: Cell) {
        self.set(x, y, cell);
    }

    /// Fill a rectangular region with a cell (direct buffer access API).
    ///
    /// This is an alias for `fill()` with a more explicit name for custom rendering.
    /// All cells in the region are marked dirty.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::render::{Buffer, Cell};
    /// use inky::style::Color;
    ///
    /// let mut buf = Buffer::new(80, 24);
    /// let cell = Cell::new('█').with_fg(Color::Blue);
    /// buf.fill_region(0, 0, 10, 5, cell);
    /// ```
    #[inline]
    pub fn fill_region(&mut self, x: u16, y: u16, w: u16, h: u16, cell: Cell) {
        self.fill(x, y, w, h, cell);
    }

    /// Copy a rectangular region from another buffer (blit operation).
    ///
    /// Copies cells from the source buffer region to this buffer.
    /// Useful for compositing multiple buffers or rendering cached content.
    ///
    /// # Arguments
    ///
    /// * `src` - Source buffer to copy from
    /// * `src_x`, `src_y` - Top-left corner of source region
    /// * `dst_x`, `dst_y` - Top-left corner of destination in this buffer
    /// * `w`, `h` - Width and height of region to copy
    ///
    /// # Example
    ///
    /// ```
    /// use inky::render::{Buffer, Cell};
    ///
    /// let mut src = Buffer::new(20, 10);
    /// src.write_str(0, 0, "Hello", inky::style::Color::White, inky::style::Color::Black);
    ///
    /// let mut dst = Buffer::new(80, 24);
    /// dst.blit(&src, 0, 0, 5, 5, 10, 1);  // Copy "Hello" to (5, 5)
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn blit(
        &mut self,
        src: &Buffer,
        src_x: u16,
        src_y: u16,
        dst_x: u16,
        dst_y: u16,
        w: u16,
        h: u16,
    ) {
        // Early bounds check
        if w == 0 || h == 0 {
            return;
        }
        if src_x >= src.width || src_y >= src.height {
            return;
        }
        if dst_x >= self.width || dst_y >= self.height {
            return;
        }

        // Clamp dimensions to both buffer bounds
        let effective_w = w
            .min(src.width.saturating_sub(src_x))
            .min(self.width.saturating_sub(dst_x));
        let effective_h = h
            .min(src.height.saturating_sub(src_y))
            .min(self.height.saturating_sub(dst_y));

        if effective_w == 0 || effective_h == 0 {
            return;
        }

        let src_width = src.width as usize;
        let dst_width = self.width as usize;
        let copy_w = effective_w as usize;

        // Copy row by row
        for dy in 0..effective_h {
            let src_row_start = ((src_y + dy) as usize) * src_width + (src_x as usize);
            let dst_row_start = ((dst_y + dy) as usize) * dst_width + (dst_x as usize);

            // Copy cells and mark them dirty
            for dx in 0..copy_w {
                let mut cell = src.cells[src_row_start + dx];
                cell.mark_dirty();
                self.cells[dst_row_start + dx] = cell;
            }
        }
    }

    /// Get mutable access to the raw cell grid for direct manipulation.
    ///
    /// This provides the lowest-level access to the buffer for advanced use cases
    /// like custom renderers or compatibility layers.
    ///
    /// # Safety
    ///
    /// This is safe but the caller is responsible for:
    /// - Properly marking cells as dirty after modification
    /// - Handling wide character spacers correctly
    /// - Respecting buffer bounds
    ///
    /// # Example
    ///
    /// ```
    /// use inky::render::{Buffer, Cell};
    ///
    /// let mut buf = Buffer::new(80, 24);
    /// let raw = buf.raw_mut();
    /// raw[0] = Cell::new('X');
    /// raw[0].mark_dirty();
    /// ```
    #[inline]
    pub fn raw_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    /// Compare two buffers and return rows that differ.
    ///
    /// Uses SIMD-optimized byte comparison for 4-8x faster dirty detection
    /// compared to cell-by-cell comparison.
    ///
    /// # Returns
    ///
    /// An iterator yielding row indices where the buffers differ.
    pub fn diff_rows<'a>(&'a self, other: &'a Buffer) -> impl Iterator<Item = u16> + 'a {
        let min_height = self.height.min(other.height);
        let self_width = self.width as usize;
        let other_width = other.width as usize;

        (0..min_height).filter(move |&y| {
            let self_start = (y as usize) * self_width;
            let other_start = (y as usize) * other_width;

            // If widths differ, rows definitely differ
            if self.width != other.width {
                return true;
            }

            let self_row = &self.cells[self_start..self_start + self_width];
            let other_row = &other.cells[other_start..other_start + other_width];

            rows_differ(self_row, other_row)
        })
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::layout::LayoutEngine;
    use crate::node::BoxNode;
    use crate::render::cell::CellFlags;
    use crate::render::render_to_buffer;
    use crate::style::BorderStyle;

    #[test]
    fn test_buffer_creation() {
        let buf = Buffer::new(80, 24);
        assert_eq!(buf.width(), 80);
        assert_eq!(buf.height(), 24);
        assert_eq!(buf.cells().len(), 80 * 24);
    }

    #[test]
    fn test_buffer_write() {
        let mut buf = Buffer::new(80, 24);
        buf.write_str(0, 0, "Hello", Color::White, Color::Black);

        assert_eq!(buf.get(0, 0).unwrap().char(), 'H');
        assert_eq!(buf.get(1, 0).unwrap().char(), 'e');
        assert_eq!(buf.get(4, 0).unwrap().char(), 'o');
    }

    #[test]
    fn test_buffer_write_wide_char() {
        let mut buf = Buffer::new(4, 1);
        buf.write_str(0, 0, "好A", Color::White, Color::Black);

        let first = buf.get(0, 0).unwrap();
        let spacer = buf.get(1, 0).unwrap();
        let next = buf.get(2, 0).unwrap();

        assert_eq!(first.char(), '好');
        assert!(first.flags.contains(CellFlags::WIDE_CHAR));
        assert!(spacer.flags.contains(CellFlags::WIDE_SPACER));
        assert_eq!(next.char(), 'A');
    }

    #[test]
    fn test_buffer_to_text() {
        let mut buf = Buffer::new(5, 2);
        buf.write_str(0, 0, "Hello", Color::White, Color::Black);
        buf.write_str(0, 1, "World", Color::White, Color::Black);

        let text = buf.to_text();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_render_simple_box_single_border() {
        let mut engine = LayoutEngine::new();
        let node = BoxNode::new()
            .width(10)
            .height(3)
            .border(BorderStyle::Single)
            .into();

        engine.build(&node).unwrap();
        engine.compute(80, 24).unwrap();

        let mut buffer = Buffer::new(80, 24);
        render_to_buffer(&node, &engine, &mut buffer);

        // Check corners for single border
        assert_eq!(buffer.get(0, 0).unwrap().char(), '┌');
        assert_eq!(buffer.get(9, 0).unwrap().char(), '┐');
        assert_eq!(buffer.get(0, 2).unwrap().char(), '└');
        assert_eq!(buffer.get(9, 2).unwrap().char(), '┘');
        // Check horizontal edge
        assert_eq!(buffer.get(1, 0).unwrap().char(), '─');
        // Check vertical edge
        assert_eq!(buffer.get(0, 1).unwrap().char(), '│');
    }

    #[test]
    fn test_render_box_rounded_border() {
        let mut engine = LayoutEngine::new();
        let node = BoxNode::new()
            .width(10)
            .height(3)
            .border(BorderStyle::Rounded)
            .into();

        engine.build(&node).unwrap();
        engine.compute(80, 24).unwrap();

        let mut buffer = Buffer::new(80, 24);
        render_to_buffer(&node, &engine, &mut buffer);

        // Check rounded corners
        assert_eq!(buffer.get(0, 0).unwrap().char(), '╭');
        assert_eq!(buffer.get(9, 0).unwrap().char(), '╮');
        assert_eq!(buffer.get(0, 2).unwrap().char(), '╰');
        assert_eq!(buffer.get(9, 2).unwrap().char(), '╯');
    }

    #[test]
    fn test_render_box_double_border() {
        let mut engine = LayoutEngine::new();
        let node = BoxNode::new()
            .width(10)
            .height(3)
            .border(BorderStyle::Double)
            .into();

        engine.build(&node).unwrap();
        engine.compute(80, 24).unwrap();

        let mut buffer = Buffer::new(80, 24);
        render_to_buffer(&node, &engine, &mut buffer);

        // Check double border corners
        assert_eq!(buffer.get(0, 0).unwrap().char(), '╔');
        assert_eq!(buffer.get(9, 0).unwrap().char(), '╗');
        assert_eq!(buffer.get(0, 2).unwrap().char(), '╚');
        assert_eq!(buffer.get(9, 2).unwrap().char(), '╝');
    }

    #[test]
    fn test_render_box_bold_border() {
        let mut engine = LayoutEngine::new();
        let node = BoxNode::new()
            .width(10)
            .height(3)
            .border(BorderStyle::Bold)
            .into();

        engine.build(&node).unwrap();
        engine.compute(80, 24).unwrap();

        let mut buffer = Buffer::new(80, 24);
        render_to_buffer(&node, &engine, &mut buffer);

        // Check bold border corners
        assert_eq!(buffer.get(0, 0).unwrap().char(), '┏');
        assert_eq!(buffer.get(9, 0).unwrap().char(), '┓');
        assert_eq!(buffer.get(0, 2).unwrap().char(), '┗');
        assert_eq!(buffer.get(9, 2).unwrap().char(), '┛');
    }

    #[test]
    fn test_render_nested_boxes() {
        use crate::node::TextNode;
        use crate::style::FlexDirection;

        let mut engine = LayoutEngine::new();
        let node = BoxNode::new()
            .width(20)
            .height(5)
            .border(BorderStyle::Single)
            .flex_direction(FlexDirection::Column)
            .child(TextNode::new("Hello"))
            .into();

        engine.build(&node).unwrap();
        engine.compute(80, 24).unwrap();

        let mut buffer = Buffer::new(80, 24);
        render_to_buffer(&node, &engine, &mut buffer);

        // Should have border and text
        assert_eq!(buffer.get(0, 0).unwrap().char(), '┌');
        // Text should be inside the border
        assert_eq!(buffer.get(1, 1).unwrap().char(), 'H');
    }

    #[test]
    fn test_buffer_dirty_tracking() {
        let mut buf = Buffer::new(10, 10);

        // Fresh buffer is not dirty (cells start clean)
        assert!(!buf.has_dirty());

        // Writing makes it dirty
        buf.write_str(0, 0, "X", Color::White, Color::Black);
        assert!(buf.has_dirty());

        // Clear dirty flags
        buf.clear_dirty();
        assert!(!buf.has_dirty());

        // Writing again makes it dirty again
        buf.write_str(0, 0, "Y", Color::White, Color::Black);
        assert!(buf.has_dirty());
    }

    #[test]
    fn test_buffer_soft_clear() {
        let mut buf = Buffer::new(10, 10);

        // Write some content
        buf.write_str(0, 0, "Hello", Color::White, Color::Black);
        buf.clear_dirty();
        assert!(!buf.has_dirty());

        // Soft clear - only changed cells should be marked dirty
        buf.soft_clear();

        // All cells that had content should now be dirty (they were blanked)
        assert!(buf.has_dirty());

        // Content should be cleared
        assert_eq!(buf.get(0, 0).unwrap().char(), ' ');

        // Clear dirty and soft_clear again - should not mark anything dirty
        // because buffer is already blank
        buf.clear_dirty();
        buf.soft_clear();
        assert!(!buf.has_dirty());
    }

    #[test]
    fn test_buffer_resize() {
        let mut buf = Buffer::new(10, 10);
        buf.write_str(0, 0, "Hello", Color::White, Color::Black);

        // Resize larger
        buf.resize(20, 20);
        assert_eq!(buf.width(), 20);
        assert_eq!(buf.height(), 20);
        // Content preserved
        assert_eq!(buf.get(0, 0).unwrap().char(), 'H');

        // Resize smaller
        buf.resize(3, 3);
        assert_eq!(buf.width(), 3);
        assert_eq!(buf.height(), 3);
        // Content preserved (up to new size)
        assert_eq!(buf.get(0, 0).unwrap().char(), 'H');
        assert_eq!(buf.get(1, 0).unwrap().char(), 'e');
        assert_eq!(buf.get(2, 0).unwrap().char(), 'l');
    }

    #[test]
    fn test_buffer_fill() {
        let mut buf = Buffer::new(10, 10);
        let mut cell = Cell::new('X');
        cell.set_fg(PackedColor::from(Color::Red));

        // Fill a 3x2 region at position (2, 3)
        buf.fill(2, 3, 3, 2, cell);

        // Check filled region
        for y in 3..5 {
            for x in 2..5 {
                let c = buf.get(x, y).unwrap();
                assert_eq!(c.char(), 'X', "Expected 'X' at ({}, {})", x, y);
                assert!(c.is_dirty());
            }
        }

        // Check outside is untouched
        assert_eq!(buf.get(0, 0).unwrap().char(), ' ');
        assert_eq!(buf.get(1, 3).unwrap().char(), ' ');
        assert_eq!(buf.get(5, 3).unwrap().char(), ' ');
    }

    #[test]
    fn test_buffer_fill_boundary_clamp() {
        let mut buf = Buffer::new(10, 10);
        let cell = Cell::new('X');

        // Fill that extends beyond boundary
        buf.fill(8, 8, 5, 5, cell); // Would extend to (13, 13) but buffer is 10x10

        // Should fill to edge only
        for y in 8..10 {
            for x in 8..10 {
                assert_eq!(buf.get(x, y).unwrap().char(), 'X');
            }
        }
    }

    #[test]
    fn test_buffer_fill_empty() {
        let mut buf = Buffer::new(10, 10);
        let cell = Cell::new('X');

        // Fill with zero width or height should do nothing
        buf.fill(0, 0, 0, 5, cell);
        buf.fill(0, 0, 5, 0, cell);

        // Buffer should still be blank
        assert_eq!(buf.get(0, 0).unwrap().char(), ' ');
    }

    #[test]
    fn test_buffer_chars_iterator() {
        let mut buf = Buffer::new(3, 2);
        buf.set(0, 0, Cell::new('A'));
        buf.set(1, 0, Cell::new('B'));
        buf.set(2, 0, Cell::new('C'));
        buf.set(0, 1, Cell::new('D'));
        buf.set(1, 1, Cell::new('E'));
        buf.set(2, 1, Cell::new('F'));

        // chars() should yield characters row by row with newlines
        let chars: Vec<_> = buf.chars().collect();
        assert_eq!(chars, vec!['A', 'B', 'C', '\n', 'D', 'E', 'F', '\n']);

        // to_text() should use chars() internally
        assert_eq!(buf.to_text(), "ABC\nDEF\n");
    }

    #[test]
    fn test_buffer_chars_streaming() {
        // chars() allows streaming without allocation
        let buf = Buffer::new(80, 24);

        // Count characters without collecting
        let total_chars = buf.chars().count();
        assert_eq!(total_chars, (80 + 1) * 24); // 80 chars + newline per row
    }

    #[test]
    fn test_cells_equal() {
        let cell_a = Cell::new('A');
        let cell_b = Cell::new('B');

        // Same cells
        assert!(super::cells_equal(&[cell_a, cell_a], &[cell_a, cell_a]));

        // Different cells
        assert!(!super::cells_equal(&[cell_a, cell_a], &[cell_a, cell_b]));

        // Different lengths
        assert!(!super::cells_equal(&[cell_a], &[cell_a, cell_a]));

        // Empty slices
        assert!(super::cells_equal(&[], &[]));
    }

    #[test]
    fn test_rows_differ() {
        let cell_a = Cell::new('A');
        let cell_b = Cell::new('B');

        // Same rows don't differ
        assert!(!super::rows_differ(&[cell_a, cell_a], &[cell_a, cell_a]));

        // Different rows differ
        assert!(super::rows_differ(&[cell_a, cell_a], &[cell_a, cell_b]));
    }

    #[test]
    fn test_buffer_row() {
        let mut buf = Buffer::new(10, 5);
        buf.write_str(0, 2, "Hello", Color::White, Color::Black);

        // Valid row
        let row = buf.row(2).expect("row 2 should exist");
        assert_eq!(row.len(), 10);
        assert_eq!(row[0].char(), 'H');
        assert_eq!(row[4].char(), 'o');

        // Out of bounds
        assert!(buf.row(5).is_none());
        assert!(buf.row(100).is_none());
    }

    #[test]
    fn test_buffer_diff_rows() {
        let mut buf1 = Buffer::new(10, 5);
        let buf2 = Buffer::new(10, 5);

        // Identical buffers have no differing rows
        let diff: Vec<_> = buf1.diff_rows(&buf2).collect();
        assert!(diff.is_empty());

        // Modify row 2 in buf1
        buf1.write_str(0, 2, "Hello", Color::White, Color::Black);

        // Now row 2 differs
        let diff: Vec<_> = buf1.diff_rows(&buf2).collect();
        assert_eq!(diff, vec![2]);

        // Modify row 4 as well
        buf1.write_str(0, 4, "World", Color::White, Color::Black);

        let diff: Vec<_> = buf1.diff_rows(&buf2).collect();
        assert_eq!(diff, vec![2, 4]);
    }

    #[test]
    fn test_buffer_diff_rows_large() {
        // Test with larger buffer to exercise SIMD paths
        let mut buf1 = Buffer::new(160, 50);
        let buf2 = Buffer::new(160, 50);

        // Modify a single cell in each of several rows
        buf1.set(80, 10, Cell::new('X'));
        buf1.set(80, 25, Cell::new('Y'));
        buf1.set(80, 49, Cell::new('Z'));

        let diff: Vec<_> = buf1.diff_rows(&buf2).collect();
        assert_eq!(diff, vec![10, 25, 49]);
    }

    // =========================================================================
    // Direct Buffer Access API tests
    // =========================================================================

    #[test]
    fn test_write_cell() {
        let mut buf = Buffer::new(10, 10);
        let cell = Cell::new('X').with_fg(Color::Red);

        buf.write_cell(5, 5, cell);

        let c = buf.get(5, 5).unwrap();
        assert_eq!(c.char(), 'X');
        assert!(c.is_dirty());
    }

    #[test]
    fn test_fill_region() {
        let mut buf = Buffer::new(10, 10);
        let cell = Cell::new('█').with_fg(Color::Blue);

        buf.fill_region(2, 2, 3, 3, cell);

        // Check filled region
        for y in 2..5 {
            for x in 2..5 {
                assert_eq!(buf.get(x, y).unwrap().char(), '█');
            }
        }

        // Check outside is untouched
        assert_eq!(buf.get(0, 0).unwrap().char(), ' ');
        assert_eq!(buf.get(5, 5).unwrap().char(), ' ');
    }

    #[test]
    fn test_blit_basic() {
        let mut src = Buffer::new(10, 10);
        src.write_str(0, 0, "Hello", Color::White, Color::Black);
        src.write_str(0, 1, "World", Color::White, Color::Black);

        let mut dst = Buffer::new(20, 20);

        // Blit "Hello" row to position (5, 5)
        dst.blit(&src, 0, 0, 5, 5, 5, 1);

        // Check blitted content
        assert_eq!(dst.get(5, 5).unwrap().char(), 'H');
        assert_eq!(dst.get(6, 5).unwrap().char(), 'e');
        assert_eq!(dst.get(7, 5).unwrap().char(), 'l');
        assert_eq!(dst.get(8, 5).unwrap().char(), 'l');
        assert_eq!(dst.get(9, 5).unwrap().char(), 'o');

        // Check destination is dirty
        assert!(dst.get(5, 5).unwrap().is_dirty());
    }

    #[test]
    fn test_blit_multi_row() {
        let mut src = Buffer::new(10, 10);
        src.write_str(0, 0, "AAAA", Color::White, Color::Black);
        src.write_str(0, 1, "BBBB", Color::White, Color::Black);
        src.write_str(0, 2, "CCCC", Color::White, Color::Black);

        let mut dst = Buffer::new(20, 20);

        // Blit 3 rows
        dst.blit(&src, 0, 0, 2, 3, 4, 3);

        // Check all three rows copied
        assert_eq!(dst.get(2, 3).unwrap().char(), 'A');
        assert_eq!(dst.get(2, 4).unwrap().char(), 'B');
        assert_eq!(dst.get(2, 5).unwrap().char(), 'C');
    }

    #[test]
    fn test_blit_boundary_clamp() {
        let mut src = Buffer::new(10, 10);
        src.write_str(0, 0, "Hello", Color::White, Color::Black);

        let mut dst = Buffer::new(8, 8);

        // Blit near edge - should clamp to buffer bounds
        dst.blit(&src, 0, 0, 6, 0, 5, 1);

        // Only 2 chars should fit (positions 6 and 7)
        assert_eq!(dst.get(6, 0).unwrap().char(), 'H');
        assert_eq!(dst.get(7, 0).unwrap().char(), 'e');
    }

    #[test]
    fn test_blit_empty() {
        let src = Buffer::new(10, 10);
        let mut dst = Buffer::new(10, 10);

        // Zero width/height should do nothing
        dst.blit(&src, 0, 0, 0, 0, 0, 5);
        dst.blit(&src, 0, 0, 0, 0, 5, 0);

        // Out of bounds source should do nothing
        dst.blit(&src, 100, 100, 0, 0, 5, 5);
    }

    #[test]
    fn test_blit_source_offset() {
        let mut src = Buffer::new(10, 10);
        src.write_str(0, 0, "Hello", Color::White, Color::Black);
        src.write_str(0, 1, "World", Color::White, Color::Black);

        let mut dst = Buffer::new(20, 20);

        // Blit starting from src (2, 1) - should get "rld" from "World"
        dst.blit(&src, 2, 1, 0, 0, 3, 1);

        assert_eq!(dst.get(0, 0).unwrap().char(), 'r');
        assert_eq!(dst.get(1, 0).unwrap().char(), 'l');
        assert_eq!(dst.get(2, 0).unwrap().char(), 'd');
    }

    #[test]
    fn test_raw_mut() {
        let mut buf = Buffer::new(10, 10);

        // Directly modify via raw_mut
        let raw = buf.raw_mut();
        raw[0] = Cell::new('X');
        raw[0].mark_dirty();

        // Verify change
        assert_eq!(buf.get(0, 0).unwrap().char(), 'X');
        assert!(buf.get(0, 0).unwrap().is_dirty());
    }
}
