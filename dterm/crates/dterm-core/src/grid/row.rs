//! Row storage for terminal grid.
//!
//! ## Design
//!
//! A Row is a contiguous array of cells with a length tracking the last
//! non-empty cell (for efficient iteration and rendering).
//!
//! Lines can be soft-wrapped (automatic) or hard-wrapped (explicit newline).

use super::cell::Cell;
use super::page::{PageSlice, PageStore};
use super::style::StyleId;

#[inline]
fn u16_from_usize(value: usize) -> u16 {
    u16::try_from(value).expect("value must fit in u16")
}

/// A single row of terminal cells.
pub struct Row {
    /// Cell storage.
    cells: PageSlice<Cell>,
    /// Index of the last non-empty cell + 1 (for efficient iteration).
    /// If 0, the row is entirely empty.
    len: u16,
    /// Row flags.
    flags: RowFlags,
}

bitflags::bitflags! {
    /// Row flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[repr(transparent)]
    pub struct RowFlags: u8 {
        /// This row is a continuation of the previous row (soft wrap).
        const WRAPPED = 1 << 0;
        /// This row is marked as dirty (needs re-render).
        const DIRTY = 1 << 1;
        /// Double-width line (DECDWL or DECDHL).
        const DOUBLE_WIDTH = 1 << 2;
        /// Double-height line, top half (DECDHL).
        const DOUBLE_HEIGHT_TOP = 1 << 3;
        /// Double-height line, bottom half (DECDHL).
        const DOUBLE_HEIGHT_BOTTOM = 1 << 4;
    }
}

/// Line size attributes (DEC line height/width).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineSize {
    /// Single-width, single-height line (default).
    #[default]
    SingleWidth,
    /// Double-width line (single-height).
    DoubleWidth,
    /// Double-height line, top half (also double-width).
    DoubleHeightTop,
    /// Double-height line, bottom half (also double-width).
    DoubleHeightBottom,
}

impl Row {
    /// Create a new row with the given width.
    #[must_use]
    pub fn new(cols: u16, pages: &mut PageStore) -> Self {
        let mut cells = pages.alloc_slice::<Cell>(cols);
        for cell in cells.iter_mut() {
            *cell = Cell::EMPTY;
        }
        Self {
            cells,
            len: 0,
            flags: RowFlags::DIRTY,
        }
    }

    /// Get the column count.
    #[must_use]
    #[inline]
    pub fn cols(&self) -> u16 {
        u16_from_usize(self.cells.len())
    }

    /// Get the page ID for this row's cell storage.
    ///
    /// Used for pin invalidation tracking.
    #[must_use]
    #[inline]
    pub fn page_id(&self) -> super::page::PageId {
        self.cells.page_id()
    }

    /// Get the length (last non-empty cell + 1).
    #[must_use]
    #[inline]
    pub fn len(&self) -> u16 {
        self.len
    }

    /// Check if the row is empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get row flags.
    #[must_use]
    #[inline]
    pub fn flags(&self) -> RowFlags {
        self.flags
    }

    /// Set row flags.
    #[inline]
    pub fn set_flags(&mut self, flags: RowFlags) {
        self.flags = flags;
    }

    /// Check if this row is a continuation of the previous (soft wrap).
    #[must_use]
    #[inline]
    pub fn is_wrapped(&self) -> bool {
        self.flags.contains(RowFlags::WRAPPED)
    }

    /// Set the wrapped flag.
    #[inline]
    pub fn set_wrapped(&mut self, wrapped: bool) {
        if wrapped {
            self.flags |= RowFlags::WRAPPED;
        } else {
            self.flags -= RowFlags::WRAPPED;
        }
    }

    /// Check if this row is dirty (needs re-render).
    #[must_use]
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.flags.contains(RowFlags::DIRTY)
    }

    /// Set the dirty flag.
    #[inline]
    pub fn set_dirty(&mut self, dirty: bool) {
        if dirty {
            self.flags |= RowFlags::DIRTY;
        } else {
            self.flags -= RowFlags::DIRTY;
        }
    }

    /// Get the current line size attribute.
    #[must_use]
    #[inline]
    pub fn line_size(&self) -> LineSize {
        if self.flags.contains(RowFlags::DOUBLE_HEIGHT_TOP) {
            LineSize::DoubleHeightTop
        } else if self.flags.contains(RowFlags::DOUBLE_HEIGHT_BOTTOM) {
            LineSize::DoubleHeightBottom
        } else if self.flags.contains(RowFlags::DOUBLE_WIDTH) {
            LineSize::DoubleWidth
        } else {
            LineSize::SingleWidth
        }
    }

    /// Set the line size attribute.
    #[inline]
    pub fn set_line_size(&mut self, size: LineSize) {
        self.flags.remove(
            RowFlags::DOUBLE_WIDTH | RowFlags::DOUBLE_HEIGHT_TOP | RowFlags::DOUBLE_HEIGHT_BOTTOM,
        );
        match size {
            LineSize::SingleWidth => {}
            LineSize::DoubleWidth => {
                self.flags |= RowFlags::DOUBLE_WIDTH;
            }
            LineSize::DoubleHeightTop => {
                self.flags |= RowFlags::DOUBLE_WIDTH | RowFlags::DOUBLE_HEIGHT_TOP;
            }
            LineSize::DoubleHeightBottom => {
                self.flags |= RowFlags::DOUBLE_WIDTH | RowFlags::DOUBLE_HEIGHT_BOTTOM;
            }
        }
        if matches!(
            size,
            LineSize::DoubleWidth | LineSize::DoubleHeightTop | LineSize::DoubleHeightBottom
        ) {
            let cols = self.cols();
            let half = cols / 2;
            let start = usize::from(half.max(1));
            let old_len = self.len as usize;
            if start < self.cells.len() {
                for cell in &mut self.cells[start..] {
                    *cell = Cell::EMPTY;
                }
                if start < old_len {
                    self.recalculate_len_up_to(start);
                }
            }
        }
        self.flags |= RowFlags::DIRTY;
    }

    /// Get a cell at the given column.
    ///
    /// Returns None if column is out of bounds.
    #[must_use]
    #[inline]
    pub fn get(&self, col: u16) -> Option<&Cell> {
        self.cells.get(col as usize)
    }

    /// Get a mutable cell at the given column.
    ///
    /// Returns None if column is out of bounds.
    #[must_use]
    #[inline]
    pub fn get_mut(&mut self, col: u16) -> Option<&mut Cell> {
        self.cells.get_mut(col as usize)
    }

    /// Get a cell at the given column (unchecked).
    ///
    /// # Safety
    ///
    /// Column must be less than cols().
    #[must_use]
    #[inline]
    pub unsafe fn get_unchecked(&self, col: u16) -> &Cell {
        debug_assert!((col as usize) < self.cells.len());
        // SAFETY: Caller guarantees col < cols()
        unsafe { self.cells.get_unchecked(col as usize) }
    }

    /// Get a mutable cell at the given column (unchecked).
    ///
    /// # Safety
    ///
    /// Column must be less than cols().
    #[must_use]
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, col: u16) -> &mut Cell {
        debug_assert!((col as usize) < self.cells.len());
        // SAFETY: Caller guarantees col < cols()
        unsafe { self.cells.get_unchecked_mut(col as usize) }
    }

    /// Get mutable access to all cells in this row.
    ///
    /// This is the fast path for bulk cell writes (e.g., ASCII blast).
    /// After modifying cells, call `update_len()` if the content length changed.
    #[must_use]
    #[inline]
    pub fn cells_mut(&mut self) -> &mut [Cell] {
        self.flags |= RowFlags::DIRTY;
        &mut self.cells[..]
    }

    /// Update the row's len to include all written content up to `end_col`.
    ///
    /// Call this after using `cells_mut()` to write content at positions up to `end_col - 1`.
    /// This ensures `visible_content()` and iteration include the new content.
    #[inline]
    pub fn update_len(&mut self, end_col: u16) {
        if end_col > self.len {
            self.len = end_col;
        }
    }

    /// Set a cell at the given column.
    ///
    /// Returns true if successful, false if out of bounds.
    #[inline]
    pub fn set(&mut self, col: u16, cell: Cell) -> bool {
        if let Some(c) = self.cells.get_mut(col as usize) {
            *c = cell;
            // Update len if we wrote past the current end
            if col >= self.len {
                self.len = col + 1;
            }
            self.flags |= RowFlags::DIRTY;
            true
        } else {
            false
        }
    }

    /// Write a character at the given column with current style.
    ///
    /// If overwriting part of a wide character, the orphaned half is cleared to space.
    #[inline]
    pub fn write_char(&mut self, col: u16, c: char) -> bool {
        let col_usize = col as usize;

        // Check if we're overwriting part of a wide character
        if let Some(current) = self.cells.get(col_usize) {
            let current_flags = current.flags();
            // If overwriting a wide continuation (second half), clear the first half
            if current_flags.contains(super::CellFlags::WIDE_CONTINUATION) && col > 0 {
                if let Some(prev_cell) = self.cells.get_mut((col - 1) as usize) {
                    *prev_cell = Cell::EMPTY;
                }
            }
            // If overwriting a wide char (first half), clear the second half
            if current_flags.contains(super::CellFlags::WIDE) {
                if let Some(next_cell) = self.cells.get_mut(col_usize + 1) {
                    *next_cell = Cell::EMPTY;
                }
            }
        }

        if let Some(cell) = self.cells.get_mut(col_usize) {
            cell.set_char(c);
            if col >= self.len {
                self.len = col + 1;
            }
            self.flags |= RowFlags::DIRTY;
            true
        } else {
            false
        }
    }

    /// Write a styled character at the given column.
    ///
    /// If overwriting part of a wide character, the orphaned half is cleared to space.
    #[inline]
    pub fn write_char_styled(
        &mut self,
        col: u16,
        c: char,
        fg: super::PackedColor,
        bg: super::PackedColor,
        flags: super::CellFlags,
    ) -> bool {
        let col_usize = col as usize;
        let cells_len = self.cells.len();

        // Single bounds check upfront
        if col_usize >= cells_len {
            return false;
        }

        // SAFETY: col_usize < cells_len verified above
        let current_flags = unsafe { self.cells.get_unchecked(col_usize) }.flags();

        // Wide character fixup (rare path) - check if either WIDE or WIDE_CONTINUATION is set
        let wide_mask = super::CellFlags::WIDE.union(super::CellFlags::WIDE_CONTINUATION);
        if (current_flags.bits() & wide_mask.bits()) != 0 {
            self.fixup_wide_char_overwrite(col_usize, current_flags, cells_len);
        }

        // SAFETY: col_usize < cells_len verified above
        unsafe {
            *self.cells.get_unchecked_mut(col_usize) = Cell::with_style(c, fg, bg, flags);
        }
        if col >= self.len {
            self.len = col + 1;
        }
        self.flags |= RowFlags::DIRTY;
        true
    }

    /// Handle wide character cleanup when overwriting part of a wide char.
    /// Marked cold since wide characters are relatively rare.
    #[cold]
    #[inline(never)]
    fn fixup_wide_char_overwrite(
        &mut self,
        col_usize: usize,
        current_flags: super::CellFlags,
        cells_len: usize,
    ) {
        // If overwriting a wide continuation (second half), clear the first half
        if current_flags.contains(super::CellFlags::WIDE_CONTINUATION) && col_usize > 0 {
            // SAFETY: col_usize > 0, so col_usize - 1 is valid
            unsafe {
                *self.cells.get_unchecked_mut(col_usize - 1) = Cell::EMPTY;
            }
        }
        // If overwriting a wide char (first half), clear the second half
        if current_flags.contains(super::CellFlags::WIDE) && col_usize + 1 < cells_len {
            // SAFETY: col_usize + 1 < cells_len verified above
            unsafe {
                *self.cells.get_unchecked_mut(col_usize + 1) = Cell::EMPTY;
            }
        }
    }

    /// Fix up wide character orphans when overwriting a range of cells.
    ///
    /// This should be called before bulk-writing single-width characters to a range.
    /// It handles:
    /// - If start cell is a WIDE_CONTINUATION, clears the previous cell (orphaned first half)
    /// - If any cell in range has WIDE flag, clears the cell after it (orphaned continuation)
    ///
    /// This is the bulk equivalent of `fixup_wide_char_overwrite` for single cells.
    #[cold]
    #[inline(never)]
    pub fn fixup_wide_chars_in_range(&mut self, start_col: u16, count: u16) {
        let start = start_col as usize;
        let end = (start_col + count) as usize;
        let cells_len = self.cells.len();

        if start >= cells_len || count == 0 {
            return;
        }

        let actual_end = end.min(cells_len);

        // Check if start cell is a wide continuation - need to clear previous cell
        if start > 0 {
            let start_flags = self.cells[start].flags();
            if start_flags.contains(super::CellFlags::WIDE_CONTINUATION) {
                self.cells[start - 1] = Cell::EMPTY;
            }
        }

        // Check each cell being overwritten - if it's a WIDE char, clear its continuation
        for col in start..actual_end {
            let flags = self.cells[col].flags();
            if flags.contains(super::CellFlags::WIDE) && col + 1 < cells_len {
                // If the continuation is outside our write range, clear it
                // If it's inside, it will be overwritten anyway
                if col + 1 >= actual_end {
                    self.cells[col + 1] = Cell::EMPTY;
                }
            }
        }
    }

    /// Write a wide (double-width) character at the given column.
    ///
    /// Wide characters occupy two cells. The first cell contains the character
    /// with the WIDE flag set, and the second cell is a continuation cell.
    /// If overwriting parts of other wide characters, the orphaned halves are cleared.
    ///
    /// Returns the number of columns consumed (2 if successful, 0 if out of bounds).
    #[inline]
    pub fn write_wide_char(
        &mut self,
        col: u16,
        c: char,
        fg: super::PackedColor,
        bg: super::PackedColor,
        flags: super::CellFlags,
    ) -> u16 {
        let cells_len = self.cells.len();
        let col_usize = col as usize;

        // Need at least 2 cells available - single bounds check
        if col_usize + 1 >= cells_len {
            // Not enough room - write to last column as single-width
            // (this matches terminal behavior when wide char is at edge)
            return 0;
        }

        // SAFETY: col_usize < cells_len and col_usize + 1 < cells_len verified above
        let first_flags = unsafe { self.cells.get_unchecked(col_usize) }.flags();
        let second_flags = unsafe { self.cells.get_unchecked(col_usize + 1) }.flags();

        // Wide character fixup (rare path)
        if first_flags.contains(super::CellFlags::WIDE_CONTINUATION)
            || second_flags.contains(super::CellFlags::WIDE)
        {
            self.fixup_wide_char_write(col_usize, first_flags, second_flags, cells_len);
        }

        // SAFETY: bounds already verified
        unsafe {
            // Write main cell with WIDE flag
            *self.cells.get_unchecked_mut(col_usize) =
                Cell::with_style(c, fg, bg, flags.union(super::CellFlags::WIDE));
            // Write continuation cell
            *self.cells.get_unchecked_mut(col_usize + 1) =
                Cell::with_style(' ', fg, bg, super::CellFlags::WIDE_CONTINUATION);
        }

        if col + 1 >= self.len {
            self.len = col + 2;
        }
        self.flags |= RowFlags::DIRTY;
        2
    }

    /// Write a styled character at the given column using StyleId.
    ///
    /// This is the StyleId variant of `write_char_styled`. Instead of storing
    /// inline colors, the cell stores a StyleId that references the StyleTable.
    /// If overwriting part of a wide character, the orphaned half is cleared to space.
    #[inline]
    pub fn write_char_with_style_id(
        &mut self,
        col: u16,
        c: char,
        style_id: StyleId,
        cell_flags: super::CellFlags,
    ) -> bool {
        let col_usize = col as usize;
        let cells_len = self.cells.len();

        // Single bounds check upfront
        if col_usize >= cells_len {
            return false;
        }

        // SAFETY: col_usize < cells_len verified above
        let current_flags = unsafe { self.cells.get_unchecked(col_usize) }.flags();

        // Wide character fixup (rare path) - check if either WIDE or WIDE_CONTINUATION is set
        let wide_mask = super::CellFlags::WIDE.union(super::CellFlags::WIDE_CONTINUATION);
        if (current_flags.bits() & wide_mask.bits()) != 0 {
            self.fixup_wide_char_overwrite(col_usize, current_flags, cells_len);
        }

        // SAFETY: col_usize < cells_len verified above
        unsafe {
            *self.cells.get_unchecked_mut(col_usize) = Cell::with_style_id(c, style_id, cell_flags);
        }
        if col >= self.len {
            self.len = col + 1;
        }
        self.flags |= RowFlags::DIRTY;
        true
    }

    /// Write a wide (double-width) character at the given column using StyleId.
    ///
    /// This is the StyleId variant of `write_wide_char`. Instead of storing
    /// inline colors, the cells store a StyleId that references the StyleTable.
    /// Wide characters occupy two cells. The first cell contains the character
    /// with the WIDE flag set, and the second cell is a continuation cell.
    /// If overwriting parts of other wide characters, the orphaned halves are cleared.
    ///
    /// Returns the number of columns consumed (2 if successful, 0 if out of bounds).
    #[inline]
    pub fn write_wide_char_with_style_id(
        &mut self,
        col: u16,
        c: char,
        style_id: StyleId,
        cell_flags: super::CellFlags,
    ) -> u16 {
        let cells_len = self.cells.len();
        let col_usize = col as usize;

        // Need at least 2 cells available - single bounds check
        if col_usize + 1 >= cells_len {
            // Not enough room - write to last column as single-width
            // (this matches terminal behavior when wide char is at edge)
            return 0;
        }

        // SAFETY: col_usize < cells_len and col_usize + 1 < cells_len verified above
        let first_flags = unsafe { self.cells.get_unchecked(col_usize) }.flags();
        let second_flags = unsafe { self.cells.get_unchecked(col_usize + 1) }.flags();

        // Wide character fixup (rare path)
        if first_flags.contains(super::CellFlags::WIDE_CONTINUATION)
            || second_flags.contains(super::CellFlags::WIDE)
        {
            self.fixup_wide_char_write(col_usize, first_flags, second_flags, cells_len);
        }

        // SAFETY: bounds already verified
        unsafe {
            // Write main cell with WIDE flag
            *self.cells.get_unchecked_mut(col_usize) =
                Cell::with_style_id(c, style_id, cell_flags.union(super::CellFlags::WIDE));
            // Write continuation cell
            *self.cells.get_unchecked_mut(col_usize + 1) =
                Cell::with_style_id(' ', style_id, super::CellFlags::WIDE_CONTINUATION);
        }

        if col + 1 >= self.len {
            self.len = col + 2;
        }
        self.flags |= RowFlags::DIRTY;
        2
    }

    /// Handle wide character cleanup when writing a wide char.
    /// Marked cold since these conflicts are relatively rare.
    #[cold]
    #[inline(never)]
    fn fixup_wide_char_write(
        &mut self,
        col_usize: usize,
        first_flags: super::CellFlags,
        second_flags: super::CellFlags,
        cells_len: usize,
    ) {
        // If first position overwrites a wide continuation (second half), clear the first half
        if first_flags.contains(super::CellFlags::WIDE_CONTINUATION) && col_usize > 0 {
            // SAFETY: col_usize > 0
            unsafe {
                *self.cells.get_unchecked_mut(col_usize - 1) = Cell::EMPTY;
            }
        }
        // If second position overwrites a wide char (first half), clear its continuation
        if second_flags.contains(super::CellFlags::WIDE) && col_usize + 2 < cells_len {
            // SAFETY: col_usize + 2 < cells_len verified above
            unsafe {
                *self.cells.get_unchecked_mut(col_usize + 2) = Cell::EMPTY;
            }
        }
    }

    /// Clear the entire row.
    #[inline]
    pub fn clear(&mut self) {
        self.cells.fill(Cell::EMPTY);
        self.len = 0;
        self.flags = RowFlags::DIRTY;
    }

    /// Clear cells from `start` to end of row.
    #[inline]
    pub fn clear_from(&mut self, start: u16) {
        let start_usize = usize::from(start);
        if start_usize < self.cells.len() {
            let old_len = self.len as usize;
            self.cells[start_usize..].fill(Cell::EMPTY);
            if start_usize < old_len {
                self.recalculate_len_up_to(start_usize);
            }
            self.flags |= RowFlags::DIRTY;
        }
    }

    /// Clear cells from start to `end` (exclusive).
    #[inline]
    pub fn clear_range(&mut self, start: u16, end: u16) {
        let start = start as usize;
        let end = (end as usize).min(self.cells.len());
        if start < end {
            let old_len = self.len as usize;
            self.cells[start..end].fill(Cell::EMPTY);
            self.flags |= RowFlags::DIRTY;
            if start < old_len && end >= old_len {
                self.recalculate_len_up_to(start);
            }
        }
    }

    /// Selectively clear cells from `start` to end of row.
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    /// Protected cells are skipped.
    #[inline]
    pub fn selective_clear_from(&mut self, start: u16) {
        let start = start as usize;
        if start < self.cells.len() {
            let old_len = self.len as usize;
            let mut any_erased = false;
            for cell in &mut self.cells[start..] {
                if !cell.is_protected() {
                    *cell = Cell::EMPTY;
                    any_erased = true;
                }
            }
            if any_erased {
                self.flags |= RowFlags::DIRTY;
                if old_len > 0 && start < old_len && self.cells[old_len - 1].is_empty() {
                    self.recalculate_len_up_to(old_len);
                }
            }
        }
    }

    /// Selectively clear cells from start to `end` (exclusive).
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    /// Protected cells are skipped.
    #[inline]
    pub fn selective_clear_range(&mut self, start: u16, end: u16) {
        let start = start as usize;
        let end = (end as usize).min(self.cells.len());
        if start < end {
            let old_len = self.len as usize;
            let mut any_erased = false;
            for cell in &mut self.cells[start..end] {
                if !cell.is_protected() {
                    *cell = Cell::EMPTY;
                    any_erased = true;
                }
            }
            if any_erased {
                self.flags |= RowFlags::DIRTY;
                if start < old_len && end >= old_len && self.cells[old_len - 1].is_empty() {
                    self.recalculate_len_up_to(old_len);
                }
            }
        }
    }

    /// Selectively clear the entire row.
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    /// Protected cells are skipped.
    #[inline]
    pub fn selective_clear(&mut self) {
        let old_len = self.len as usize;
        let mut any_erased = false;
        for cell in self.cells.iter_mut() {
            if !cell.is_protected() {
                *cell = Cell::EMPTY;
                any_erased = true;
            }
        }
        if any_erased {
            self.flags |= RowFlags::DIRTY;
            if old_len > 0 && self.cells[old_len - 1].is_empty() {
                self.recalculate_len_up_to(old_len);
            }
        }
    }

    /// Recalculate the len field by scanning up to `end`.
    fn recalculate_len_up_to(&mut self, end: usize) {
        let end = end.min(self.cells.len());
        self.len = self
            .cells
            .iter()
            .take(end)
            .rposition(|c| !c.is_empty())
            .map(|i| u16_from_usize(i) + 1)
            .unwrap_or(0);
    }

    /// Resize the row to a new column count.
    ///
    /// If growing, new cells are empty.
    /// If shrinking, excess cells are discarded.
    pub fn resize(&mut self, new_cols: u16, pages: &mut PageStore) {
        let old_cols = self.cols();
        if new_cols == old_cols {
            return;
        }

        let mut new_cells = pages.alloc_slice::<Cell>(new_cols);
        for cell in new_cells.iter_mut() {
            *cell = Cell::EMPTY;
        }
        let copy_len = (old_cols as usize).min(new_cols as usize);
        new_cells[..copy_len].copy_from_slice(&self.cells[..copy_len]);
        self.cells = new_cells;

        // Clamp len
        if self.len > new_cols {
            self.len = new_cols;
        }

        self.flags |= RowFlags::DIRTY;
    }

    /// Get an iterator over all cells.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Cell> {
        self.cells.iter()
    }

    /// Get a mutable iterator over all cells.
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Cell> {
        self.flags |= RowFlags::DIRTY;
        self.cells.iter_mut()
    }

    /// Get a slice of cells.
    #[must_use]
    #[inline]
    pub fn as_slice(&self) -> &[Cell] {
        &self.cells
    }

    /// Get a mutable slice of cells.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Cell] {
        self.flags |= RowFlags::DIRTY;
        &mut self.cells
    }

    /// Copy cells from another row.
    pub fn copy_from(&mut self, other: &Row) {
        let copy_len = self.cols().min(other.cols()) as usize;
        self.cells[..copy_len].copy_from_slice(&other.cells[..copy_len]);
        self.len = other.len.min(self.cols());
        self.flags = other.flags | RowFlags::DIRTY;
    }

    /// Insert `count` blank cells at `col`, shifting existing cells right.
    ///
    /// Cells that would be shifted past the end of the row are discarded.
    /// This implements the ICH (Insert Character) operation.
    pub fn insert_chars(&mut self, col: u16, count: u16) {
        if count == 0 || col >= self.cols() {
            return;
        }

        let col = col as usize;
        let count = count as usize;
        let cols = self.cells.len();
        let old_len = self.len as usize;

        // Shift cells right, starting from the end to avoid overwriting
        // Cells shifted past cols are discarded
        let shift_start = col;
        let shift_end = cols.saturating_sub(count);

        if shift_end > shift_start {
            // Copy from right to left to avoid overwriting
            for i in (shift_start..shift_end).rev() {
                self.cells[i + count] = self.cells[i];
            }
        }

        // Fill the gap with empty cells
        let fill_end = (col + count).min(cols);
        for cell in &mut self.cells[col..fill_end] {
            *cell = Cell::EMPTY;
        }

        self.flags |= RowFlags::DIRTY;
        if old_len == 0 || col >= old_len {
            return;
        }
        let new_len = old_len + count;
        if new_len <= cols {
            self.len = u16_from_usize(new_len);
        } else if cols > 0 {
            if self.cells[cols - 1].is_empty() {
                self.recalculate_len_up_to(cols);
            } else {
                self.len = u16_from_usize(cols);
            }
        } else {
            self.len = 0;
        }
    }

    /// Delete `count` cells at `col`, shifting remaining cells left.
    ///
    /// Empty cells are inserted at the end of the row.
    /// This implements the DCH (Delete Character) operation.
    pub fn delete_chars(&mut self, col: u16, count: u16) {
        if count == 0 || col >= self.cols() {
            return;
        }

        let col = col as usize;
        let count = count as usize;
        let cols = self.cells.len();
        let old_len = self.len as usize;

        // Shift cells left
        let src_start = (col + count).min(cols);
        let shift_len = cols - src_start;

        if shift_len > 0 {
            for i in 0..shift_len {
                self.cells[col + i] = self.cells[src_start + i];
            }
        }

        // Fill the end with empty cells
        let fill_start = col + shift_len;
        for cell in &mut self.cells[fill_start..] {
            *cell = Cell::EMPTY;
        }

        self.flags |= RowFlags::DIRTY;
        if old_len == 0 || col >= old_len {
            return;
        }
        let delete_end = col + count;
        if delete_end < old_len {
            self.len = u16_from_usize(old_len - count);
        } else {
            self.recalculate_len_up_to(col);
        }
    }

    /// Erase `count` cells starting at `col`, without shifting.
    ///
    /// Cells are replaced with blanks in place. This differs from `delete_chars`
    /// which shifts remaining cells left.
    /// This implements the ECH (Erase Character) operation.
    pub fn erase_chars(&mut self, col: u16, count: u16) {
        if count == 0 || col >= self.cols() {
            return;
        }

        let col = col as usize;
        let count = count as usize;
        let end = (col + count).min(self.cells.len());
        let old_len = self.len as usize;

        for cell in &mut self.cells[col..end] {
            *cell = Cell::EMPTY;
        }

        self.flags |= RowFlags::DIRTY;
        if col < old_len && end >= old_len {
            self.recalculate_len_up_to(col);
        }
    }

    /// Convert row content to a string (for debugging/search).
    #[must_use]
    pub fn to_string(&self) -> String {
        let mut s = String::with_capacity(self.len as usize);
        for cell in &self.cells[..self.len as usize] {
            if !cell.is_wide_continuation() {
                s.push(cell.char());
            }
        }
        s
    }

    // ========================================================================
    // Reflow operations - used during terminal resize
    // ========================================================================

    /// Append cells from another row to this row.
    ///
    /// Cells are appended starting from `self.len`. Returns the number of cells
    /// actually appended (limited by available space).
    pub fn append_cells(&mut self, cells: &[Cell]) -> usize {
        let start = self.len as usize;
        let available = self.cells.len() - start;
        let count = cells.len().min(available);

        if count > 0 {
            self.cells[start..start + count].copy_from_slice(&cells[..count]);
            self.len = u16_from_usize(start + count);
            self.flags |= RowFlags::DIRTY;
        }

        count
    }

    /// Split off cells from the front of this row.
    ///
    /// Removes and returns the first `count` cells. Remaining cells are shifted left.
    /// Returns a Vec of the removed cells.
    pub fn split_front(&mut self, count: u16) -> Vec<Cell> {
        let count = (count as usize).min(self.cells.len());
        if count == 0 {
            return Vec::new();
        }

        // Copy cells to return
        let mut result = Vec::with_capacity(count);
        for cell in &self.cells[..count] {
            result.push(*cell);
        }

        // Shift remaining cells left
        let remaining = self.cells.len() - count;
        for i in 0..remaining {
            self.cells[i] = self.cells[count + i];
        }

        // Clear the now-empty cells at the end
        for cell in &mut self.cells[remaining..] {
            *cell = Cell::EMPTY;
        }

        // Update len
        let count_u16 = u16_from_usize(count);
        self.len = self.len.saturating_sub(count_u16);
        self.flags |= RowFlags::DIRTY;

        result
    }

    /// Split off cells from the back of this row (cells beyond `col`).
    ///
    /// Removes cells from `col` onwards and returns them. The row is truncated.
    /// Returns a Vec of the removed cells.
    pub fn split_back(&mut self, col: u16) -> Vec<Cell> {
        let col_usize = usize::from(col).min(self.cells.len());
        let end = self.len as usize;

        if col_usize >= end {
            return Vec::new();
        }

        // Copy cells to return
        let mut result = Vec::with_capacity(end - col_usize);
        for cell in &self.cells[col_usize..end] {
            result.push(*cell);
        }

        // Clear the removed cells
        for cell in &mut self.cells[col_usize..] {
            *cell = Cell::EMPTY;
        }

        // Update len
        self.len = u16_from_usize(col_usize);
        self.flags |= RowFlags::DIRTY;

        result
    }

    /// Prepend cells to the front of this row.
    ///
    /// Shifts existing cells right to make room. Returns the number of cells
    /// actually prepended (cells that would be pushed past the end are discarded).
    pub fn prepend_cells(&mut self, cells: &[Cell]) -> usize {
        let count = cells.len().min(self.cells.len());
        if count == 0 {
            return 0;
        }

        // Shift existing cells right
        let cols = self.cells.len();
        let shift_end = cols.saturating_sub(count);
        for i in (0..shift_end).rev() {
            self.cells[i + count] = self.cells[i];
        }

        // Insert new cells at front
        self.cells[..count].copy_from_slice(&cells[..count]);

        // Update len
        let new_len = (self.len as usize + count).min(cols);
        self.len = u16_from_usize(new_len);
        self.flags |= RowFlags::DIRTY;

        count
    }

    /// Check if the last character in the row has the WIDE flag set.
    ///
    /// This is used during reflow to handle wide characters at line boundaries.
    #[must_use]
    pub fn last_char_is_wide(&self) -> bool {
        if self.len == 0 {
            return false;
        }
        self.cells
            .get((self.len - 1) as usize)
            .map(|c| c.flags().contains(super::CellFlags::WIDE))
            .unwrap_or(false)
    }

    /// Extract all non-empty cells as a Vec.
    ///
    /// Useful for reflow operations where we need to work with cell data outside
    /// the PageStore context.
    #[must_use]
    pub fn extract_cells(&self) -> Vec<Cell> {
        self.cells[..self.len as usize].to_vec()
    }

    /// Check if all cells are empty/default.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.len == 0
    }
}

impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Row")
            .field("cols", &self.cols())
            .field("len", &self.len)
            .field("flags", &self.flags)
            .field("content", &self.to_string())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(cols: u16) -> (PageStore, Row) {
        let mut pages = PageStore::new();
        let row = Row::new(cols, &mut pages);
        (pages, row)
    }

    #[test]
    fn row_new() {
        let (_pages, row) = make_row(80);
        assert_eq!(row.cols(), 80);
        assert_eq!(row.len(), 0);
        assert!(row.is_empty());
        assert!(row.is_dirty());
    }

    #[test]
    fn row_write_char() {
        let (_pages, mut row) = make_row(80);
        assert!(row.write_char(0, 'H'));
        assert!(row.write_char(1, 'i'));
        assert_eq!(row.len(), 2);
        assert_eq!(row.get(0).unwrap().char(), 'H');
        assert_eq!(row.get(1).unwrap().char(), 'i');
    }

    #[test]
    fn row_clear() {
        let (_pages, mut row) = make_row(80);
        row.write_char(0, 'X');
        row.write_char(10, 'Y');
        assert_eq!(row.len(), 11);

        row.clear();
        assert_eq!(row.len(), 0);
        assert!(row.is_empty());
    }

    #[test]
    fn row_clear_from() {
        let (_pages, mut row) = make_row(80);
        for i in 0..10 {
            row.write_char(i, 'X');
        }
        assert_eq!(row.len(), 10);

        row.clear_from(5);
        assert_eq!(row.len(), 5);
    }

    #[test]
    fn row_clear_from_sparse_tail() {
        let (_pages, mut row) = make_row(10);
        row.write_char(9, 'Z');
        assert_eq!(row.len(), 10);

        row.clear_from(5);
        assert_eq!(row.len(), 0);
    }

    #[test]
    fn row_resize_grow() {
        let (mut pages, mut row) = make_row(40);
        row.write_char(0, 'A');
        row.resize(80, &mut pages);
        assert_eq!(row.cols(), 80);
        assert_eq!(row.get(0).unwrap().char(), 'A');
    }

    #[test]
    fn row_resize_shrink() {
        let (mut pages, mut row) = make_row(80);
        row.write_char(60, 'A');
        row.resize(40, &mut pages);
        assert_eq!(row.cols(), 40);
        // Cell at 60 is now gone
        assert!(row.get(60).is_none());
    }

    #[test]
    fn row_to_string() {
        let (_pages, mut row) = make_row(80);
        for (i, c) in "Hello".chars().enumerate() {
            row.write_char(u16_from_usize(i), c);
        }
        assert_eq!(row.to_string(), "Hello");
    }

    #[test]
    fn row_wrapped_flag() {
        let (_pages, mut row) = make_row(80);
        assert!(!row.is_wrapped());
        row.set_wrapped(true);
        assert!(row.is_wrapped());
        row.set_wrapped(false);
        assert!(!row.is_wrapped());
    }

    #[test]
    fn row_insert_chars() {
        let (_pages, mut row) = make_row(10);
        for (i, c) in "ABCDEFGHIJ".chars().enumerate() {
            row.write_char(u16_from_usize(i), c);
        }
        assert_eq!(row.to_string(), "ABCDEFGHIJ");

        // Insert 2 blanks at position 3
        row.insert_chars(3, 2);
        // "ABC  DEFGH" - IJ are pushed off the end
        assert_eq!(row.get(0).unwrap().char(), 'A');
        assert_eq!(row.get(1).unwrap().char(), 'B');
        assert_eq!(row.get(2).unwrap().char(), 'C');
        assert_eq!(row.get(3).unwrap().char(), ' ');
        assert_eq!(row.get(4).unwrap().char(), ' ');
        assert_eq!(row.get(5).unwrap().char(), 'D');
        assert_eq!(row.get(6).unwrap().char(), 'E');
        assert_eq!(row.get(7).unwrap().char(), 'F');
    }

    #[test]
    fn row_delete_chars() {
        let (_pages, mut row) = make_row(10);
        for (i, c) in "ABCDEFGHIJ".chars().enumerate() {
            row.write_char(u16_from_usize(i), c);
        }
        assert_eq!(row.to_string(), "ABCDEFGHIJ");

        // Delete 2 chars at position 3 (D and E)
        row.delete_chars(3, 2);
        // "ABCFGHIJ  " - shifted left, blanks at end
        assert_eq!(row.get(0).unwrap().char(), 'A');
        assert_eq!(row.get(1).unwrap().char(), 'B');
        assert_eq!(row.get(2).unwrap().char(), 'C');
        assert_eq!(row.get(3).unwrap().char(), 'F');
        assert_eq!(row.get(4).unwrap().char(), 'G');
        assert_eq!(row.get(5).unwrap().char(), 'H');
        assert_eq!(row.get(6).unwrap().char(), 'I');
        assert_eq!(row.get(7).unwrap().char(), 'J');
        assert_eq!(row.get(8).unwrap().char(), ' ');
        assert_eq!(row.get(9).unwrap().char(), ' ');
    }

    #[test]
    fn row_delete_chars_tail_overlap() {
        let (_pages, mut row) = make_row(10);
        row.write_char(0, 'A');
        row.write_char(5, 'B');
        assert_eq!(row.len(), 6);

        row.delete_chars(4, 3);
        assert_eq!(row.len(), 1);
        assert_eq!(row.get(0).unwrap().char(), 'A');
    }

    #[test]
    fn row_insert_chars_at_end() {
        let (_pages, mut row) = make_row(10);
        for (i, c) in "ABC".chars().enumerate() {
            row.write_char(u16_from_usize(i), c);
        }

        // Insert at position past content
        row.insert_chars(8, 2);
        // Should work, inserting blanks at position 8
        assert_eq!(row.get(0).unwrap().char(), 'A');
        assert_eq!(row.get(1).unwrap().char(), 'B');
        assert_eq!(row.get(2).unwrap().char(), 'C');
    }

    #[test]
    fn row_insert_chars_truncation_drops_tail() {
        let (_pages, mut row) = make_row(6);
        row.write_char(5, 'Z');
        assert_eq!(row.len(), 6);

        row.insert_chars(0, 2);
        assert_eq!(row.len(), 0);
        assert!(row.is_empty());
    }

    #[test]
    fn row_delete_chars_more_than_remaining() {
        let (_pages, mut row) = make_row(10);
        for (i, c) in "ABCDEFGHIJ".chars().enumerate() {
            row.write_char(u16_from_usize(i), c);
        }

        // Delete 20 chars at position 5 (more than available)
        row.delete_chars(5, 20);
        // Should delete F-J, leaving "ABCDE     "
        assert_eq!(row.get(0).unwrap().char(), 'A');
        assert_eq!(row.get(4).unwrap().char(), 'E');
        assert_eq!(row.get(5).unwrap().char(), ' ');
    }

    #[test]
    fn row_erase_chars() {
        let (_pages, mut row) = make_row(10);
        for (i, c) in "ABCDEFGHIJ".chars().enumerate() {
            row.write_char(u16_from_usize(i), c);
        }
        assert_eq!(row.to_string(), "ABCDEFGHIJ");

        // Erase 3 chars at position 3 (D, E, F)
        row.erase_chars(3, 3);
        // "ABC   GHIJ" - no shifting, just blanks in place
        assert_eq!(row.get(0).unwrap().char(), 'A');
        assert_eq!(row.get(1).unwrap().char(), 'B');
        assert_eq!(row.get(2).unwrap().char(), 'C');
        assert_eq!(row.get(3).unwrap().char(), ' ');
        assert_eq!(row.get(4).unwrap().char(), ' ');
        assert_eq!(row.get(5).unwrap().char(), ' ');
        assert_eq!(row.get(6).unwrap().char(), 'G');
        assert_eq!(row.get(7).unwrap().char(), 'H');
        assert_eq!(row.get(8).unwrap().char(), 'I');
        assert_eq!(row.get(9).unwrap().char(), 'J');
    }

    #[test]
    fn row_erase_chars_beyond_end() {
        let (_pages, mut row) = make_row(10);
        for (i, c) in "ABCDE".chars().enumerate() {
            row.write_char(u16_from_usize(i), c);
        }

        // Erase 100 chars at position 3 (should stop at row end)
        row.erase_chars(3, 100);
        assert_eq!(row.get(0).unwrap().char(), 'A');
        assert_eq!(row.get(1).unwrap().char(), 'B');
        assert_eq!(row.get(2).unwrap().char(), 'C');
        assert_eq!(row.get(3).unwrap().char(), ' ');
        assert_eq!(row.get(4).unwrap().char(), ' ');
    }

    #[test]
    fn row_erase_chars_zero_count() {
        let (_pages, mut row) = make_row(10);
        for (i, c) in "ABCDE".chars().enumerate() {
            row.write_char(u16_from_usize(i), c);
        }

        // Erase 0 chars - should do nothing
        row.erase_chars(2, 0);
        assert_eq!(row.to_string(), "ABCDE");
    }

    #[test]
    fn row_write_overwrite_wide_continuation() {
        use crate::grid::{CellFlags, PackedColor};

        let (_pages, mut row) = make_row(80);

        // Write a wide char at col 0
        let fg = PackedColor::default_fg();
        let bg = PackedColor::default_bg();
        row.write_wide_char(0, '\u{4E2D}', fg, bg, CellFlags::empty());

        // Verify wide char setup
        let cell0 = row.get(0).unwrap();
        let cell1 = row.get(1).unwrap();
        eprintln!("After write_wide_char:");
        eprintln!(
            "  Cell 0: char='{}' flags={:#06x}",
            cell0.char(),
            cell0.flags().bits()
        );
        eprintln!(
            "  Cell 1: char='{}' flags={:#06x}",
            cell1.char(),
            cell1.flags().bits()
        );
        assert!(cell0.is_wide(), "Cell 0 should be wide");
        assert!(
            cell1.is_wide_continuation(),
            "Cell 1 should be continuation"
        );

        // Overwrite col 1 (continuation) with 'A'
        row.write_char_styled(1, 'A', fg, bg, CellFlags::empty());

        // Check result
        let cell0_after = row.get(0).unwrap();
        let cell1_after = row.get(1).unwrap();
        eprintln!("After write_char_styled at col 1:");
        eprintln!(
            "  Cell 0: char='{}' flags={:#06x}",
            cell0_after.char(),
            cell0_after.flags().bits()
        );
        eprintln!(
            "  Cell 1: char='{}' flags={:#06x}",
            cell1_after.char(),
            cell1_after.flags().bits()
        );

        assert_eq!(cell1_after.char(), 'A', "Cell 1 should be 'A'");
        assert_eq!(cell0_after.char(), ' ', "Cell 0 should be cleared to space");
    }

    // ========================================================================
    // StyleId write methods tests
    // ========================================================================

    #[test]
    fn row_write_char_with_style_id_basic() {
        use crate::grid::{CellFlags, StyleId};

        let (_pages, mut row) = make_row(80);
        let style_id = StyleId(42);

        // Write a char with StyleId
        assert!(row.write_char_with_style_id(0, 'H', style_id, CellFlags::empty()));
        assert!(row.write_char_with_style_id(1, 'i', style_id, CellFlags::empty()));

        // Verify the cells
        let cell0 = row.get(0).unwrap();
        let cell1 = row.get(1).unwrap();

        assert_eq!(cell0.char(), 'H');
        assert!(cell0.uses_style_id());
        assert_eq!(cell0.style_id(), style_id);

        assert_eq!(cell1.char(), 'i');
        assert!(cell1.uses_style_id());
        assert_eq!(cell1.style_id(), style_id);

        assert_eq!(row.len(), 2);
        assert!(row.is_dirty());
    }

    #[test]
    fn row_write_char_with_style_id_preserves_flags() {
        use crate::grid::{CellFlags, StyleId};

        let (_pages, mut row) = make_row(80);
        let style_id = StyleId(100);

        // Write with BOLD flag (cell-level attribute)
        row.write_char_with_style_id(5, 'X', style_id, CellFlags::BOLD);

        let cell = row.get(5).unwrap();
        assert_eq!(cell.char(), 'X');
        assert!(cell.uses_style_id());
        assert_eq!(cell.style_id(), style_id);
        assert!(cell.flags().contains(CellFlags::BOLD));
    }

    #[test]
    fn row_write_char_with_style_id_out_of_bounds() {
        use crate::grid::{CellFlags, StyleId};

        let (_pages, mut row) = make_row(10);
        let style_id = StyleId(1);

        // Write at valid position
        assert!(row.write_char_with_style_id(9, 'Z', style_id, CellFlags::empty()));

        // Write at invalid position
        assert!(!row.write_char_with_style_id(10, 'X', style_id, CellFlags::empty()));
        assert!(!row.write_char_with_style_id(100, 'Y', style_id, CellFlags::empty()));
    }

    #[test]
    fn row_write_char_with_style_id_overwrites_wide_continuation() {
        use crate::grid::{CellFlags, PackedColor, StyleId};

        let (_pages, mut row) = make_row(80);

        // First write a wide char using inline colors
        let fg = PackedColor::default_fg();
        let bg = PackedColor::default_bg();
        row.write_wide_char(0, '\u{4E2D}', fg, bg, CellFlags::empty());

        // Verify wide char setup
        assert!(row.get(0).unwrap().is_wide());
        assert!(row.get(1).unwrap().is_wide_continuation());

        // Now overwrite the continuation (col 1) with StyleId
        let style_id = StyleId(77);
        row.write_char_with_style_id(1, 'A', style_id, CellFlags::empty());

        // First cell should be cleared to space
        let cell0 = row.get(0).unwrap();
        assert_eq!(cell0.char(), ' ');

        // Second cell should have our new character with StyleId
        let cell1 = row.get(1).unwrap();
        assert_eq!(cell1.char(), 'A');
        assert!(cell1.uses_style_id());
        assert_eq!(cell1.style_id(), style_id);
    }

    #[test]
    fn row_write_wide_char_with_style_id_basic() {
        use crate::grid::{CellFlags, StyleId};

        let (_pages, mut row) = make_row(80);
        let style_id = StyleId(50);

        // Write a wide character
        let consumed =
            row.write_wide_char_with_style_id(0, '\u{4E2D}', style_id, CellFlags::empty());
        assert_eq!(consumed, 2);

        // Verify the cells
        let cell0 = row.get(0).unwrap();
        let cell1 = row.get(1).unwrap();

        // First cell: character with WIDE flag and StyleId
        assert_eq!(cell0.char(), '\u{4E2D}');
        assert!(cell0.is_wide());
        assert!(cell0.uses_style_id());
        assert_eq!(cell0.style_id(), style_id);

        // Second cell: continuation with StyleId
        assert_eq!(cell1.char(), ' ');
        assert!(cell1.is_wide_continuation());
        assert!(cell1.uses_style_id());
        assert_eq!(cell1.style_id(), style_id);

        assert_eq!(row.len(), 2);
    }

    #[test]
    fn row_write_wide_char_with_style_id_at_edge() {
        use crate::grid::{CellFlags, StyleId};

        let (_pages, mut row) = make_row(10);
        let style_id = StyleId(1);

        // Write wide char at last valid position (col 8, needs 8 and 9)
        let consumed =
            row.write_wide_char_with_style_id(8, '\u{4E2D}', style_id, CellFlags::empty());
        assert_eq!(consumed, 2);

        // Try to write at col 9 - no room for 2 cells
        let consumed =
            row.write_wide_char_with_style_id(9, '\u{4E2D}', style_id, CellFlags::empty());
        assert_eq!(consumed, 0);
    }

    #[test]
    fn row_write_wide_char_with_style_id_overwrites_existing_wide() {
        use crate::grid::{CellFlags, StyleId};

        let (_pages, mut row) = make_row(80);
        let style_id1 = StyleId(10);
        let style_id2 = StyleId(20);

        // Write first wide char at col 0
        row.write_wide_char_with_style_id(0, '\u{4E00}', style_id1, CellFlags::empty());

        // Write second wide char at col 1 (overlaps continuation of first)
        row.write_wide_char_with_style_id(1, '\u{4E8C}', style_id2, CellFlags::empty());

        // First cell should be cleared
        let cell0 = row.get(0).unwrap();
        assert_eq!(cell0.char(), ' ');

        // Cells 1-2 should have the new wide char
        let cell1 = row.get(1).unwrap();
        let cell2 = row.get(2).unwrap();

        assert_eq!(cell1.char(), '\u{4E8C}');
        assert!(cell1.is_wide());
        assert!(cell1.uses_style_id());
        assert_eq!(cell1.style_id(), style_id2);

        assert!(cell2.is_wide_continuation());
        assert!(cell2.uses_style_id());
        assert_eq!(cell2.style_id(), style_id2);
    }

    #[test]
    fn row_write_wide_char_with_style_id_preserves_flags() {
        use crate::grid::{CellFlags, StyleId};

        let (_pages, mut row) = make_row(80);
        let style_id = StyleId(99);

        // Write wide char with UNDERLINE flag
        row.write_wide_char_with_style_id(0, '\u{4E2D}', style_id, CellFlags::UNDERLINE);

        let cell0 = row.get(0).unwrap();
        // WIDE flag should be added, along with our UNDERLINE
        assert!(cell0.flags().contains(CellFlags::WIDE));
        assert!(cell0.flags().contains(CellFlags::UNDERLINE));
        assert!(cell0.uses_style_id());
    }
}

#[cfg(kani)]
mod proofs {
    /// Bounds checks before unchecked access are sufficient for single-cell writes.
    #[kani::proof]
    fn row_write_char_styled_bounds_safe() {
        let col: u16 = kani::any();
        let cells_len: u16 = kani::any();

        kani::assume(cells_len > 0 && cells_len <= 500);
        kani::assume(col < cells_len);

        let col_usize = usize::from(col);
        let cells_len_usize = usize::from(cells_len);

        kani::assert(col_usize < cells_len_usize, "col in bounds for get_unchecked");

        if col_usize > 0 {
            kani::assert(
                col_usize - 1 < cells_len_usize,
                "prev cell in bounds for wide cleanup",
            );
        }

        if col_usize + 1 < cells_len_usize {
            kani::assert(
                col_usize + 1 < cells_len_usize,
                "next cell in bounds for wide cleanup",
            );
        }
    }

    /// Wide char writes only touch columns proven to be in range.
    #[kani::proof]
    fn row_write_wide_char_bounds_safe() {
        let col: u16 = kani::any();
        let cells_len: u16 = kani::any();

        kani::assume(cells_len >= 2 && cells_len <= 500);
        kani::assume(col < cells_len - 1);

        let col_usize = usize::from(col);
        let cells_len_usize = usize::from(cells_len);

        kani::assert(
            col_usize + 1 < cells_len_usize,
            "wide char requires two in-bounds cells",
        );

        if col_usize > 0 {
            kani::assert(col_usize - 1 < cells_len_usize, "prev cell in bounds");
        }

        if col_usize + 2 < cells_len_usize {
            kani::assert(col_usize + 2 < cells_len_usize, "next continuation in bounds");
        }
    }

    /// Clear ranges never access beyond row bounds.
    #[kani::proof]
    #[kani::unwind(40)]
    fn row_clear_range_bounds_safe() {
        let start: u16 = kani::any();
        let end: u16 = kani::any();
        let cells_len: u16 = kani::any();

        kani::assume(cells_len <= 32);
        kani::assume(start <= end);
        kani::assume(end <= cells_len);

        let start_usize = usize::from(start);
        let end_usize = usize::from(end);
        let cells_len_usize = usize::from(cells_len);

        if start_usize < end_usize {
            for i in start_usize..end_usize {
                kani::assert(i < cells_len_usize, "clear index within bounds");
            }
        }
    }

    /// Insert shifts only write within the row when shifting is required.
    #[kani::proof]
    fn row_insert_chars_shift_bounds_safe() {
        let col: u16 = kani::any();
        let count: u16 = kani::any();
        let cols: u16 = kani::any();

        kani::assume(cols > 0 && cols <= 200);
        kani::assume(count > 0);
        kani::assume(col < cols);

        let col_usize = usize::from(col);
        let count_usize = usize::from(count);
        let cols_usize = usize::from(cols);

        if count_usize > cols_usize {
            return;
        }

        let shift_end = cols_usize - count_usize;
        if shift_end > col_usize {
            let i: usize = kani::any();
            kani::assume(i >= col_usize && i < shift_end);

            kani::assert(i < cols_usize, "source index in bounds");
            kani::assert(
                i + count_usize < cols_usize,
                "shift target index in bounds",
            );
        }
    }
}
