//! GPU buffer abstraction for Tier 3 rendering.
//!
//! This module provides the `GpuBuffer` trait and related types for
//! zero-copy GPU rendering, designed to integrate with dterm's
//! GPU-accelerated terminal emulator.
//!
//! # Architecture
//!
//! ```text
//! inky Buffer → GpuCell conversion → GpuBuffer → GPU/dterm
//!     ↓               ↓                  ↓           ↓
//! 10-byte Cell   8-byte GpuCell    Zero-copy    Metal/wgpu
//! ```
//!
//! # Feature Flag
//!
//! GPU support is behind the `gpu` feature flag:
//!
//! ```toml
//! [dependencies]
//! inky = { version = "0.1", features = ["gpu"] }
//! ```

use super::cell::{Cell, CellFlags};

/// GPU-compatible cell representation (8 bytes).
///
/// This matches dterm's cell layout for zero-copy buffer sharing.
/// The layout is designed to be directly uploadable to GPU buffers.
///
/// # Memory Layout
///
/// ```text
/// Byte 0-1: char_data (u16) - BMP character or overflow index
/// Byte 2-5: colors (u32)   - Packed fg/bg colors with mode bits
/// Byte 6-7: flags (u16)    - Cell attributes
/// ```
#[repr(C, packed)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct GpuCell {
    /// Character data (BMP char or overflow table index).
    pub char_data: u16,
    /// Packed foreground and background colors.
    pub colors: GpuPackedColors,
    /// Cell flags (bold, italic, etc.).
    pub flags: GpuCellFlags,
}

// Compile-time size assertion
const _: () = assert!(std::mem::size_of::<GpuCell>() == 8);

impl GpuCell {
    /// Create a blank GPU cell.
    pub fn blank() -> Self {
        Self {
            char_data: ' ' as u16,
            colors: GpuPackedColors::default(),
            flags: GpuCellFlags::empty(),
        }
    }

    /// Create a GPU cell from a character.
    pub fn new(c: char) -> Self {
        Self {
            char_data: if c as u32 <= 0xFFFF {
                c as u16
            } else {
                '?' as u16
            },
            colors: GpuPackedColors::default(),
            flags: GpuCellFlags::empty(),
        }
    }

    /// Get the character.
    pub fn char(&self) -> char {
        char::from_u32(self.char_data as u32).unwrap_or(' ')
    }

    /// Set foreground color (RGB).
    pub fn with_fg(mut self, r: u8, g: u8, b: u8) -> Self {
        self.colors = self.colors.with_fg_rgb(r, g, b);
        self
    }

    /// Set background color (RGB).
    pub fn with_bg(mut self, r: u8, g: u8, b: u8) -> Self {
        self.colors = self.colors.with_bg_rgb(r, g, b);
        self
    }
}

impl std::fmt::Debug for GpuCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Copy fields to local variables to avoid unaligned access
        let colors = self.colors;
        let flags = self.flags;
        f.debug_struct("GpuCell")
            .field("char", &self.char())
            .field("colors", &colors)
            .field("flags", &flags)
            .finish()
    }
}

impl From<Cell> for GpuCell {
    fn from(cell: Cell) -> Self {
        let fg = cell.fg();
        let bg = cell.bg();
        Self {
            char_data: cell.char_data,
            colors: GpuPackedColors::from_rgb(fg.r, fg.g, fg.b, bg.r, bg.g, bg.b),
            flags: GpuCellFlags::from(cell.flags),
        }
    }
}

/// Packed foreground and background colors (4 bytes).
///
/// # Bit Layout (dterm-compatible)
///
/// ```text
/// Bits 0-7:   Foreground color index (0-255) or R component
/// Bits 8-15:  Background color index (0-255) or G component (fg)
/// Bits 16-23: B component (fg) / R component (bg)
/// Bits 24-31: Mode bits and G/B components (bg)
/// ```
///
/// For simplicity, we use RGB mode exclusively in inky.
/// The full color is stored in a palette and indexed.
#[repr(transparent)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct GpuPackedColors(pub u32);

impl GpuPackedColors {
    /// Create packed colors from RGB values.
    ///
    /// For GPU rendering, we encode colors as palette indices.
    /// This simplified version uses the RGB values directly
    /// in a compact format.
    pub fn from_rgb(fg_r: u8, fg_g: u8, fg_b: u8, bg_r: u8, bg_g: u8, bg_b: u8) -> Self {
        // Pack as: fg_idx (8) | bg_idx (8) | fg_mode (4) | bg_mode (4) | reserved (8)
        // For now, use a simple encoding that prioritizes common colors
        let fg_idx = Self::rgb_to_index(fg_r, fg_g, fg_b);
        let bg_idx = Self::rgb_to_index(bg_r, bg_g, bg_b);

        // Mode 2 = RGB mode (not indexed)
        let fg_mode: u32 = 2;
        let bg_mode: u32 = 2;

        Self((fg_idx as u32) | ((bg_idx as u32) << 8) | (fg_mode << 24) | (bg_mode << 28))
    }

    /// Create with foreground RGB.
    pub fn with_fg_rgb(self, r: u8, g: u8, b: u8) -> Self {
        let bg_idx = ((self.0 >> 8) & 0xFF) as u8;
        let fg_idx = Self::rgb_to_index(r, g, b);
        Self((self.0 & 0xFFFF_0000) | (fg_idx as u32) | ((bg_idx as u32) << 8))
    }

    /// Create with background RGB.
    pub fn with_bg_rgb(self, r: u8, g: u8, b: u8) -> Self {
        let fg_idx = (self.0 & 0xFF) as u8;
        let bg_idx = Self::rgb_to_index(r, g, b);
        Self((self.0 & 0xFFFF_0000) | (fg_idx as u32) | ((bg_idx as u32) << 8))
    }

    /// Convert RGB to a palette index.
    ///
    /// Uses ANSI 256-color approximation for compatibility.
    fn rgb_to_index(r: u8, g: u8, b: u8) -> u8 {
        // Standard 16 colors
        if r == 0 && g == 0 && b == 0 {
            return 0;
        } // Black
        if r == 255 && g == 255 && b == 255 {
            return 15;
        } // White

        // 6x6x6 color cube (indices 16-231)
        let r_idx = (r as u16 * 5 / 255) as u8;
        let g_idx = (g as u16 * 5 / 255) as u8;
        let b_idx = (b as u16 * 5 / 255) as u8;

        16 + 36 * r_idx + 6 * g_idx + b_idx
    }

    /// Get foreground color index.
    pub fn fg_index(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Get background color index.
    pub fn bg_index(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }
}

impl std::fmt::Debug for GpuPackedColors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GpuPackedColors(fg={}, bg={})",
            self.fg_index(),
            self.bg_index()
        )
    }
}

bitflags::bitflags! {
    /// GPU-compatible cell flags (2 bytes).
    ///
    /// Matches dterm's CellFlags layout for zero-copy rendering.
    #[repr(transparent)]
    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    pub struct GpuCellFlags: u16 {
        /// Bold text.
        const BOLD          = 0b0000_0000_0001;
        /// Dim/faint text.
        const DIM           = 0b0000_0000_0010;
        /// Italic text.
        const ITALIC        = 0b0000_0000_0100;
        /// Underlined text.
        const UNDERLINE     = 0b0000_0000_1000;
        /// Blinking text.
        const BLINK         = 0b0000_0001_0000;
        /// Inverse/reverse video.
        const INVERSE       = 0b0000_0010_0000;
        /// Hidden text.
        const HIDDEN        = 0b0000_0100_0000;
        /// Strikethrough text.
        const STRIKETHROUGH = 0b0000_1000_0000;
        /// Double underline.
        const DOUBLE_UNDERLINE = 0b0001_0000_0000;
        /// Wide character (occupies 2 cells).
        const WIDE          = 0b0010_0000_0000;
        /// Wide character continuation.
        const WIDE_CONT     = 0b0100_0000_0000;
        /// Character is in overflow table.
        const COMPLEX       = 0b1000_0000_0000;
    }
}

impl std::fmt::Debug for GpuCellFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        bitflags::parser::to_writer(self, f)
    }
}

/// Mapping table from CellFlags bit positions to GpuCellFlags bits.
/// Index = CellFlags bit position, Value = corresponding GpuCellFlags bit.
/// This avoids 11 individual branch checks in favor of table lookup + bitwise ops.
const CELL_TO_GPU_FLAG_MAP: [(u16, u16); 11] = [
    (CellFlags::BOLD.bits(), GpuCellFlags::BOLD.bits()),
    (CellFlags::ITALIC.bits(), GpuCellFlags::ITALIC.bits()),
    (CellFlags::UNDERLINE.bits(), GpuCellFlags::UNDERLINE.bits()),
    (
        CellFlags::STRIKETHROUGH.bits(),
        GpuCellFlags::STRIKETHROUGH.bits(),
    ),
    (CellFlags::DIM.bits(), GpuCellFlags::DIM.bits()),
    (CellFlags::INVERSE.bits(), GpuCellFlags::INVERSE.bits()),
    (CellFlags::HIDDEN.bits(), GpuCellFlags::HIDDEN.bits()),
    (CellFlags::BLINK.bits(), GpuCellFlags::BLINK.bits()),
    (CellFlags::WIDE_CHAR.bits(), GpuCellFlags::WIDE.bits()),
    (
        CellFlags::WIDE_SPACER.bits(),
        GpuCellFlags::WIDE_CONT.bits(),
    ),
    (CellFlags::OVERFLOW.bits(), GpuCellFlags::COMPLEX.bits()),
];

impl From<CellFlags> for GpuCellFlags {
    fn from(flags: CellFlags) -> Self {
        let src = flags.bits();
        let mut result: u16 = 0;

        // Unrolled loop using const table - compiler can optimize this well
        for &(cell_bit, gpu_bit) in &CELL_TO_GPU_FLAG_MAP {
            if src & cell_bit != 0 {
                result |= gpu_bit;
            }
        }

        GpuCellFlags::from_bits_truncate(result)
    }
}

/// Trait for GPU buffer backends.
///
/// This trait abstracts the GPU buffer interface, allowing different
/// backends (wgpu, Metal via dterm, etc.) to be used interchangeably.
///
/// # Implementors
///
/// - `DtermGpuBuffer` - Integration with dterm's GPU renderer
/// - `WgpuGpuBuffer` - Direct wgpu integration (future)
///
/// # Example
///
/// ```ignore
/// fn render_to_gpu<B: GpuBuffer>(buffer: &mut B, cells: &[GpuCell]) {
///     let data = buffer.map_write();
///     data.copy_from_slice(cells);
///     buffer.unmap();
///     buffer.submit();
/// }
/// ```
pub trait GpuBuffer {
    /// Get the width of the buffer in cells.
    fn width(&self) -> u16;

    /// Get the height of the buffer in cells.
    fn height(&self) -> u16;

    /// Map the buffer for writing.
    ///
    /// Returns a mutable slice of GPU cells that can be written to.
    /// The caller must call `unmap()` when done.
    fn map_write(&mut self) -> &mut [GpuCell];

    /// Unmap the buffer after writing.
    fn unmap(&mut self);

    /// Submit the buffer for rendering.
    ///
    /// This should be called after `unmap()` to trigger GPU rendering.
    fn submit(&mut self);

    /// Resize the buffer.
    ///
    /// Returns `true` if the resize was successful.
    fn resize(&mut self, width: u16, height: u16) -> bool;

    /// Check if the GPU backend is available.
    fn is_available(&self) -> bool;
}

/// GPU buffer that falls back to CPU rendering.
///
/// This is useful for testing and for systems without GPU support.
/// It stores cells in a CPU-side buffer.
#[derive(Clone)]
pub struct CpuGpuBuffer {
    cells: Vec<GpuCell>,
    width: u16,
    height: u16,
}

impl CpuGpuBuffer {
    /// Create a new CPU-backed GPU buffer.
    pub fn new(width: u16, height: u16) -> Self {
        // Use saturating_mul to prevent overflow on 32-bit systems
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            cells: vec![GpuCell::blank(); size],
            width,
            height,
        }
    }

    /// Get the cells as a slice.
    pub fn cells(&self) -> &[GpuCell] {
        &self.cells
    }

    /// Check if coordinates are within buffer bounds.
    #[inline]
    pub fn in_bounds(&self, x: u16, y: u16) -> bool {
        x < self.width && y < self.height
    }

    /// Calculate linear index from coordinates.
    #[inline]
    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if self.in_bounds(x, y) {
            Some((y as usize) * (self.width as usize) + (x as usize))
        } else {
            None
        }
    }

    /// Get a cell at position.
    pub fn get(&self, x: u16, y: u16) -> Option<&GpuCell> {
        self.index(x, y).map(|idx| &self.cells[idx])
    }
}

impl GpuBuffer for CpuGpuBuffer {
    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }

    fn map_write(&mut self) -> &mut [GpuCell] {
        &mut self.cells
    }

    fn unmap(&mut self) {
        // No-op for CPU buffer
    }

    fn submit(&mut self) {
        // No-op for CPU buffer
    }

    fn resize(&mut self, width: u16, height: u16) -> bool {
        if width == self.width && height == self.height {
            return true;
        }

        // Use saturating_mul to prevent overflow on 32-bit systems
        let new_size = (width as usize).saturating_mul(height as usize);
        let mut new_cells = vec![GpuCell::blank(); new_size];

        // Copy existing content
        let copy_width = self.width.min(width);
        let copy_height = self.height.min(height);

        let old_width = self.width as usize;
        let new_w = width as usize;
        let copy_w = copy_width as usize;
        for y in 0..copy_height as usize {
            let old_start = y * old_width;
            let new_start = y * new_w;
            new_cells[new_start..new_start + copy_w]
                .copy_from_slice(&self.cells[old_start..old_start + copy_w]);
        }

        self.cells = new_cells;
        self.width = width;
        self.height = height;
        true
    }

    fn is_available(&self) -> bool {
        true // CPU buffer is always available
    }
}

impl Default for CpuGpuBuffer {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

/// Convert a standard Buffer to GPU cells.
///
/// This function converts inky's 10-byte cells to 8-byte GPU cells
/// for upload to the GPU.
///
/// # Performance
///
/// Uses pre-allocation and direct iteration for optimal throughput.
/// For a 200x50 buffer (10K cells), conversion takes ~25µs.
#[inline]
pub fn buffer_to_gpu_cells(buffer: &super::Buffer) -> Vec<GpuCell> {
    let cells = buffer.cells();
    let mut gpu_cells = Vec::with_capacity(cells.len());
    for cell in cells {
        gpu_cells.push(GpuCell::from(*cell));
    }
    gpu_cells
}

/// Convert only dirty cells to GPU format.
///
/// Returns a vector of (index, GpuCell) pairs for cells that have been modified.
/// This is useful for incremental GPU updates where only changed cells need
/// to be uploaded.
///
/// # Performance
///
/// For typical UI updates where only a few cells change, this can be
/// significantly faster than full buffer conversion.
#[inline]
pub fn buffer_to_gpu_cells_dirty(buffer: &super::Buffer) -> Vec<(usize, GpuCell)> {
    buffer
        .cells()
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.is_dirty())
        .map(|(i, cell)| (i, GpuCell::from(*cell)))
        .collect()
}

/// Copy a Buffer into a GpuBuffer.
///
/// This is the main entry point for GPU rendering. It converts
/// the standard buffer cells to GPU format and uploads them.
///
/// # Performance
///
/// Uses zip iteration to avoid bounds checking in the inner loop.
/// For a 200x50 buffer, full copy takes ~34µs.
#[inline]
pub fn copy_buffer_to_gpu<B: GpuBuffer>(buffer: &super::Buffer, gpu_buffer: &mut B) {
    // Resize if needed
    if gpu_buffer.width() != buffer.width() || gpu_buffer.height() != buffer.height() {
        gpu_buffer.resize(buffer.width(), buffer.height());
    }

    // Map and copy using zip to avoid bounds checks
    let gpu_cells = gpu_buffer.map_write();
    let src_cells = buffer.cells();

    // Use zip for bounds-check-free iteration
    for (dst, src) in gpu_cells.iter_mut().zip(src_cells.iter()) {
        *dst = GpuCell::from(*src);
    }

    gpu_buffer.unmap();
}

/// Copy only dirty cells from Buffer into GpuBuffer.
///
/// This is an optimized version that only updates cells that have changed,
/// reducing GPU bandwidth for incremental updates.
///
/// # Performance
///
/// For typical UI updates where <5% of cells change, this can be
/// 10-20x faster than full buffer copy.
#[inline]
pub fn copy_buffer_to_gpu_dirty<B: GpuBuffer>(buffer: &super::Buffer, gpu_buffer: &mut B) {
    // Resize if needed
    if gpu_buffer.width() != buffer.width() || gpu_buffer.height() != buffer.height() {
        gpu_buffer.resize(buffer.width(), buffer.height());
    }

    let gpu_cells = gpu_buffer.map_write();
    let src_cells = buffer.cells();

    // Only copy dirty cells
    for (i, src) in src_cells.iter().enumerate() {
        if src.is_dirty() && i < gpu_cells.len() {
            gpu_cells[i] = GpuCell::from(*src);
        }
    }

    gpu_buffer.unmap();
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::render::{Buffer, Cell};
    use crate::style::Color;

    #[test]
    fn test_gpu_cell_size() {
        assert_eq!(std::mem::size_of::<GpuCell>(), 8);
    }

    #[test]
    fn test_gpu_cell_blank() {
        let cell = GpuCell::blank();
        assert_eq!(cell.char(), ' ');
    }

    #[test]
    fn test_gpu_cell_from_cell() {
        let cell = Cell::new('A').with_fg(Color::Red).with_bg(Color::Blue);

        let gpu_cell = GpuCell::from(cell);
        assert_eq!(gpu_cell.char(), 'A');
    }

    #[test]
    fn test_gpu_cell_with_colors() {
        let cell = GpuCell::new('X').with_fg(255, 0, 0).with_bg(0, 0, 255);

        assert_eq!(cell.char(), 'X');
        // Copy colors to avoid unaligned access on packed struct
        let colors = cell.colors;
        // Verify colors are encoded
        assert_ne!(colors.fg_index(), colors.bg_index());
    }

    #[test]
    fn test_gpu_packed_colors() {
        let colors = GpuPackedColors::from_rgb(255, 0, 0, 0, 0, 0);
        assert_ne!(colors.fg_index(), 0); // Red should not be black
        assert_eq!(colors.bg_index(), 0); // Black is index 0
    }

    #[test]
    fn test_cpu_gpu_buffer() {
        let mut buf = CpuGpuBuffer::new(10, 5);
        assert_eq!(buf.width(), 10);
        assert_eq!(buf.height(), 5);
        assert!(buf.is_available());

        // Write to buffer
        {
            let cells = buf.map_write();
            cells[0] = GpuCell::new('H');
            cells[1] = GpuCell::new('i');
        }
        buf.unmap();
        buf.submit();

        assert_eq!(buf.get(0, 0).unwrap().char(), 'H');
        assert_eq!(buf.get(1, 0).unwrap().char(), 'i');
    }

    #[test]
    fn test_cpu_gpu_buffer_resize() {
        let mut buf = CpuGpuBuffer::new(5, 5);
        {
            let cells = buf.map_write();
            cells[0] = GpuCell::new('A');
        }

        buf.resize(10, 10);
        assert_eq!(buf.width(), 10);
        assert_eq!(buf.height(), 10);
        // Content should be preserved
        assert_eq!(buf.get(0, 0).unwrap().char(), 'A');
    }

    #[test]
    fn test_buffer_to_gpu_cells() {
        let mut buffer = Buffer::new(5, 2);
        buffer.write_str(0, 0, "Hello", Color::White, Color::Black);

        let gpu_cells = buffer_to_gpu_cells(&buffer);
        assert_eq!(gpu_cells.len(), 10);
        assert_eq!(gpu_cells[0].char(), 'H');
        assert_eq!(gpu_cells[4].char(), 'o');
    }

    #[test]
    fn test_copy_buffer_to_gpu() {
        let mut buffer = Buffer::new(10, 5);
        buffer.write_str(0, 0, "Test", Color::White, Color::Black);

        let mut gpu_buf = CpuGpuBuffer::new(10, 5);
        copy_buffer_to_gpu(&buffer, &mut gpu_buf);

        assert_eq!(gpu_buf.get(0, 0).unwrap().char(), 'T');
        assert_eq!(gpu_buf.get(3, 0).unwrap().char(), 't');
    }

    #[test]
    fn test_gpu_cell_flags_conversion() {
        let flags = CellFlags::BOLD | CellFlags::ITALIC | CellFlags::UNDERLINE;
        let gpu_flags = GpuCellFlags::from(flags);

        assert!(gpu_flags.contains(GpuCellFlags::BOLD));
        assert!(gpu_flags.contains(GpuCellFlags::ITALIC));
        assert!(gpu_flags.contains(GpuCellFlags::UNDERLINE));
        assert!(!gpu_flags.contains(GpuCellFlags::STRIKETHROUGH));
    }

    #[test]
    fn test_buffer_to_gpu_cells_dirty() {
        let mut buffer = Buffer::new(10, 5);

        // Write some text (marks cells as dirty)
        buffer.write_str(0, 0, "Hi", Color::White, Color::Black);

        // Get dirty cells
        let dirty = buffer_to_gpu_cells_dirty(&buffer);

        // Only the written cells should be in the result
        assert_eq!(dirty.len(), 2);
        assert_eq!(dirty[0].0, 0); // First cell at index 0
        assert_eq!(dirty[0].1.char(), 'H');
        assert_eq!(dirty[1].0, 1); // Second cell at index 1
        assert_eq!(dirty[1].1.char(), 'i');
    }

    #[test]
    fn test_copy_buffer_to_gpu_dirty() {
        let mut buffer = Buffer::new(10, 5);
        let mut gpu_buf = CpuGpuBuffer::new(10, 5);

        // Initial copy
        buffer.write_str(0, 0, "Hello", Color::White, Color::Black);
        copy_buffer_to_gpu(&buffer, &mut gpu_buf);
        buffer.clear_dirty();

        // Modify just one cell
        buffer.write_str(0, 0, "J", Color::Red, Color::Black);

        // Copy only dirty
        copy_buffer_to_gpu_dirty(&buffer, &mut gpu_buf);

        // First cell updated, rest unchanged
        assert_eq!(gpu_buf.get(0, 0).unwrap().char(), 'J');
        assert_eq!(gpu_buf.get(1, 0).unwrap().char(), 'e'); // Unchanged
    }
}
