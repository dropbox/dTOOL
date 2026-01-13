//! Diff algorithm for computing minimal terminal updates.

use crate::render::{Buffer, Cell};
use smallvec::SmallVec;
use std::io::{self, Write};

/// Type alias for cell collections in changes.
/// SmallVec avoids heap allocation for typical short runs (≤16 cells).
/// Inline storage is 176 bytes (16 * 11 bytes/cell), balancing stack usage vs heap avoidance.
pub type CellVec = SmallVec<[Cell; 16]>;

/// A change to apply to the terminal.
#[derive(Debug, Clone)]
pub enum Change {
    /// Move cursor to position.
    MoveCursor {
        /// Column position (0-indexed).
        x: u16,
        /// Row position (0-indexed).
        y: u16,
    },
    /// Write cells at current cursor position (SmallVec for short runs).
    WriteCells {
        /// Cells to write contiguously.
        cells: CellVec,
    },
    /// Clear the entire screen.
    Clear,
}

/// Diff algorithm for computing minimal updates using double-buffering.
///
/// Uses two internal buffers to avoid cloning every frame:
/// - `current_idx` points to the buffer being rendered to
/// - After diffing, buffers are swapped (O(1) operation)
/// - Previous frame data is preserved in the other buffer
pub struct Differ {
    /// Double-buffered storage: [buffer_0, buffer_1]
    buffers: [Buffer; 2],
    /// Index of current buffer (0 or 1)
    current_idx: usize,
    /// Whether we have a valid previous frame
    has_prev: bool,
}

impl Differ {
    /// Create a new differ with default buffer size.
    pub fn new() -> Self {
        Self {
            buffers: [Buffer::new(80, 24), Buffer::new(80, 24)],
            current_idx: 0,
            has_prev: false,
        }
    }

    /// Create a new differ with specified buffer size.
    pub fn with_size(width: u16, height: u16) -> Self {
        Self {
            buffers: [Buffer::new(width, height), Buffer::new(width, height)],
            current_idx: 0,
            has_prev: false,
        }
    }

    /// Get the current buffer for rendering into.
    pub fn current_buffer(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current_idx]
    }

    /// Get the previous buffer for reference (read-only).
    pub fn prev_buffer(&self) -> Option<&Buffer> {
        if self.has_prev {
            Some(&self.buffers[1 - self.current_idx])
        } else {
            None
        }
    }

    /// Resize both buffers to new dimensions.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.buffers[0].resize(width, height);
        self.buffers[1].resize(width, height);
    }

    /// Compute changes from the current buffer state.
    ///
    /// After calling this, the buffers are swapped so the current buffer
    /// becomes the previous buffer for the next frame.
    #[must_use = "diff changes should be rendered"]
    pub fn diff_and_swap(&mut self) -> Vec<Change> {
        let current = &self.buffers[self.current_idx];

        if !self.has_prev {
            // First render: output everything
            // Pre-allocate: 1 Clear + height * (MoveCursor + WriteCells)
            let capacity = 1 + (current.height() as usize * 2);
            let mut changes = Vec::with_capacity(capacity);
            changes.push(Change::Clear);
            for y in 0..current.height() {
                changes.push(Change::MoveCursor { x: 0, y });
                let cells: CellVec = (0..current.width())
                    .filter_map(|x| current.get(x, y).cloned())
                    .collect();
                changes.push(Change::WriteCells { cells });
            }
            // Swap buffers: current becomes previous, no clone needed!
            self.current_idx = 1 - self.current_idx;
            self.has_prev = true;
            return changes;
        }

        // Incremental update: pre-allocate with reasonable estimate
        // Most incremental updates affect few rows, 32 changes is a good start
        let mut changes = Vec::with_capacity(32);

        // Incremental: only output dirty rows
        for y in current.dirty_rows() {
            // Find contiguous dirty regions in this row
            let mut x = 0u16;
            while x < current.width() {
                // Skip clean cells
                while x < current.width() {
                    if let Some(cell) = current.get(x, y) {
                        if cell.is_dirty() {
                            break;
                        }
                    }
                    x += 1;
                }

                if x >= current.width() {
                    break;
                }

                // Collect dirty cells
                let start_x = x;
                let mut cells = CellVec::new();
                while x < current.width() {
                    if let Some(cell) = current.get(x, y) {
                        if cell.is_dirty() {
                            cells.push(*cell);
                            x += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if !cells.is_empty() {
                    changes.push(Change::MoveCursor { x: start_x, y });
                    changes.push(Change::WriteCells { cells });
                }
            }
        }

        // Swap buffers: current becomes previous, no clone needed!
        self.current_idx = 1 - self.current_idx;
        self.has_prev = true;

        changes
    }

    /// Compute changes between previous and current buffer (legacy API).
    ///
    /// This method is provided for backward compatibility. For best performance,
    /// prefer using `current_buffer()` + `diff_and_swap()` to avoid buffer cloning.
    pub fn diff(&mut self, current: &Buffer) -> Vec<Change> {
        if !self.has_prev {
            // First render: output everything
            // Pre-allocate: 1 Clear + height * (MoveCursor + WriteCells)
            let capacity = 1 + (current.height() as usize * 2);
            let mut changes = Vec::with_capacity(capacity);
            changes.push(Change::Clear);
            for y in 0..current.height() {
                changes.push(Change::MoveCursor { x: 0, y });
                let cells: CellVec = (0..current.width())
                    .filter_map(|x| current.get(x, y).cloned())
                    .collect();
                changes.push(Change::WriteCells { cells });
            }
            // Store current as previous for next diff
            self.buffers[self.current_idx] = current.clone();
            self.has_prev = true;
            return changes;
        }

        // Incremental update: pre-allocate with reasonable estimate
        let mut changes = Vec::with_capacity(32);

        // Incremental: only output dirty rows
        for y in current.dirty_rows() {
            // Find contiguous dirty regions in this row
            let mut x = 0u16;
            while x < current.width() {
                // Skip clean cells
                while x < current.width() {
                    if let Some(cell) = current.get(x, y) {
                        if cell.is_dirty() {
                            break;
                        }
                    }
                    x += 1;
                }

                if x >= current.width() {
                    break;
                }

                // Collect dirty cells
                let start_x = x;
                let mut cells = CellVec::new();
                while x < current.width() {
                    if let Some(cell) = current.get(x, y) {
                        if cell.is_dirty() {
                            cells.push(*cell);
                            x += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if !cells.is_empty() {
                    changes.push(Change::MoveCursor { x: start_x, y });
                    changes.push(Change::WriteCells { cells });
                }
            }
        }

        // Legacy: copy buffer into internal storage (still needed for backward compat)
        // Copy current buffer content into our internal buffer, then swap
        let dest_idx = self.current_idx;
        self.buffers[dest_idx].resize(current.width(), current.height());
        copy_buffer(current, &mut self.buffers[dest_idx]);
        self.current_idx = 1 - self.current_idx;
        self.has_prev = true;

        changes
    }

    /// Reset differ state (forces full redraw next time).
    pub fn reset(&mut self) {
        self.has_prev = false;
    }
}

/// Copy contents from src buffer to dst buffer.
fn copy_buffer(src: &Buffer, dst: &mut Buffer) {
    let cells_src = src.cells();
    let cells_dst = dst.cells_mut();
    let len = cells_src.len().min(cells_dst.len());
    cells_dst[..len].copy_from_slice(&cells_src[..len]);
}

impl Default for Differ {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply changes to a terminal writer using batched writes.
///
/// Uses `queue!()` to batch escape sequences and flushes once at the end,
/// reducing syscalls from O(cells * 5) to O(1).
pub fn apply_changes<W: Write>(writer: &mut W, changes: &[Change]) -> io::Result<()> {
    use crossterm::{cursor, queue, style, terminal};

    // Track current style to avoid redundant escape sequences
    let mut current_fg: Option<style::Color> = None;
    let mut current_bg: Option<style::Color> = None;
    let mut current_attrs: Option<style::Attributes> = None;

    for change in changes {
        match change {
            Change::Clear => {
                queue!(
                    writer,
                    terminal::Clear(terminal::ClearType::All),
                    cursor::MoveTo(0, 0)
                )?;
                // Reset style tracking after clear
                current_fg = None;
                current_bg = None;
                current_attrs = None;
            }
            Change::MoveCursor { x, y } => {
                queue!(writer, cursor::MoveTo(*x, *y))?;
            }
            Change::WriteCells { cells } => {
                for cell in cells {
                    // Build attributes
                    let mut attrs = style::Attributes::default();
                    if cell.flags.contains(crate::render::CellFlags::BOLD) {
                        attrs.set(style::Attribute::Bold);
                    }
                    if cell.flags.contains(crate::render::CellFlags::ITALIC) {
                        attrs.set(style::Attribute::Italic);
                    }
                    if cell.flags.contains(crate::render::CellFlags::UNDERLINE) {
                        attrs.set(style::Attribute::Underlined);
                    }
                    if cell.flags.contains(crate::render::CellFlags::DIM) {
                        attrs.set(style::Attribute::Dim);
                    }

                    let cell_fg = cell.fg();
                    let cell_bg = cell.bg();
                    let fg = style::Color::Rgb {
                        r: cell_fg.r,
                        g: cell_fg.g,
                        b: cell_fg.b,
                    };
                    let bg = style::Color::Rgb {
                        r: cell_bg.r,
                        g: cell_bg.g,
                        b: cell_bg.b,
                    };

                    // Only emit escape sequences when style changes
                    if current_fg != Some(fg) {
                        queue!(writer, style::SetForegroundColor(fg))?;
                        current_fg = Some(fg);
                    }
                    if current_bg != Some(bg) {
                        queue!(writer, style::SetBackgroundColor(bg))?;
                        current_bg = Some(bg);
                    }
                    if current_attrs != Some(attrs) {
                        // Reset attributes before setting new ones to clear any stale state
                        queue!(writer, style::SetAttribute(style::Attribute::Reset))?;
                        queue!(writer, style::SetAttributes(attrs))?;
                        current_attrs = Some(attrs);
                        // After attribute reset, we need to re-emit colors
                        queue!(writer, style::SetForegroundColor(fg))?;
                        queue!(writer, style::SetBackgroundColor(bg))?;
                    }

                    // Print the character (cursor auto-advances)
                    queue!(writer, style::Print(cell.char()))?;
                }
            }
        }
    }

    // Reset colors at the end and single flush
    queue!(writer, style::ResetColor)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::style::Color;

    #[test]
    fn test_first_render_clears() {
        let mut differ = Differ::new();
        let buffer = Buffer::new(10, 5);

        let changes = differ.diff(&buffer);
        assert!(matches!(changes.first(), Some(Change::Clear)));
    }

    #[test]
    fn test_no_changes_when_clean() {
        let mut differ = Differ::new();
        let mut buffer = Buffer::new(10, 5);

        // First render
        let _ = differ.diff(&buffer);

        // Clear dirty flags
        buffer.clear_dirty();

        // Second render - should have no changes
        let changes = differ.diff(&buffer);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_double_buffer_first_render_clears() {
        let mut differ = Differ::with_size(10, 5);

        let changes = differ.diff_and_swap();
        assert!(matches!(changes.first(), Some(Change::Clear)));
    }

    #[test]
    fn test_double_buffer_no_changes_when_clean() {
        let mut differ = Differ::with_size(10, 5);

        // First render (clears, writes everything)
        let _ = differ.diff_and_swap();

        // Second render with no changes - buffer starts clean
        let changes = differ.diff_and_swap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_double_buffer_detects_dirty_cells() {
        let mut differ = Differ::with_size(10, 5);

        // First render
        let _ = differ.diff_and_swap();

        // Write to current buffer
        let buffer = differ.current_buffer();
        buffer.write_str(0, 0, "Test", Color::White, Color::Black);

        // Second render should detect the changes
        let changes = differ.diff_and_swap();
        assert!(!changes.is_empty());
    }

    #[test]
    fn test_double_buffer_swap_preserves_content() {
        let mut differ = Differ::with_size(10, 5);

        // First render - write some content
        {
            let buffer = differ.current_buffer();
            buffer.write_str(0, 0, "Hello", Color::White, Color::Black);
        }
        let _ = differ.diff_and_swap();

        // Now the previous buffer should have "Hello"
        let prev = differ.prev_buffer().expect("should have previous buffer");
        assert_eq!(prev.get(0, 0).unwrap().char(), 'H');
        assert_eq!(prev.get(1, 0).unwrap().char(), 'e');
    }

    #[test]
    fn test_double_buffer_resize() {
        let mut differ = Differ::with_size(10, 5);

        // First render
        let _ = differ.diff_and_swap();

        // Resize
        differ.resize(20, 10);
        differ.reset();

        // Check both buffers are resized
        let buffer = differ.current_buffer();
        assert_eq!(buffer.width(), 20);
        assert_eq!(buffer.height(), 10);

        // After a diff, check prev buffer too
        let _ = differ.diff_and_swap();
        let prev = differ.prev_buffer().unwrap();
        assert_eq!(prev.width(), 20);
        assert_eq!(prev.height(), 10);
    }
}
