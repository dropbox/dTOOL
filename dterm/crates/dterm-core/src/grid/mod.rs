//! Terminal grid with O(1) scrolling.
//!
//! ## Design
//!
//! - 12-byte packed cells
//! - Ring buffer storage (O(1) scroll)
//! - Damage tracking for efficient rendering
//! - Cursor position tracking with bounds enforcement
//! - Optional tiered scrollback integration
//!
//! ## Scrollback Integration
//!
//! The Grid can optionally be connected to a [`Scrollback`] for long-term
//! history storage. When scrollback is enabled:
//!
//! - Rows that scroll off the top are pushed to the scrollback
//! - Users can scroll back beyond the ring buffer to view historical content
//! - Memory-efficient tiered storage (hot/warm/cold)
//!
//! ## Verification
//!
//! - TLA+ spec: `tla/Grid.tla`
//! - Kani proofs: `cell_access_safe`, `resize_cursor_valid`
//! - Property tests: cursor always in bounds after any operation

mod cell;
mod damage;
mod extra;
mod page;
mod pin;
mod row;
mod style;

pub use cell::{Cell, CellFlags, PackedColor, PackedColors};
pub use damage::{
    BitsetRowIterator, Damage, DamageBoundsIterator, DamageRect, DamageTracker, DamagedRowIterator,
    LineDamageBounds, MergedDamageIterator, RowDamageBounds,
};
pub use extra::{is_combining_mark, is_zero_width, CellCoord, CellExtra, CellExtras};
pub use page::{PageStore, PoolStats, PAGE_SIZE};
pub use pin::{Generation, GenerationTracker, Pin, PinnedRange};
pub use row::{LineSize, Row, RowFlags};
pub use style::{
    Color, ColorType, ExtendedStyle, Rgb, RgbPair, Style, StyleAttrs, StyleId, StyleTable,
    StyleTableStats, GRID_DEFAULT_STYLE_ID,
};

use crate::scrollback::{Line, Scrollback};

// ----------------------------------------------------------------------------
// Row index conversion helpers
// ----------------------------------------------------------------------------

/// Convert usize row index to u16 (saturating).
///
/// Grid row indices are always bounded by visible_rows (a u16), so this
/// conversion is safe. We use saturating to handle edge cases gracefully.
#[inline]
fn row_u16(idx: usize) -> u16 {
    idx.try_into().unwrap_or(u16::MAX)
}

/// Convert i32 result to u16 after clamping to non-negative.
///
/// Used for cursor math where we clamp to [0, max] range.
#[inline]
#[allow(clippy::cast_sign_loss)] // max(0) ensures non-negative
fn clamp_u16(val: i32) -> u16 {
    val.max(0).try_into().unwrap_or(u16::MAX)
}

/// Convert u64 to u16 with saturation to `u16::MAX`.
///
/// Used when computing visible row from absolute row offsets.
/// Large values (> 65535) saturate to `u16::MAX`.
#[inline]
fn u64_to_u16_saturating(val: u64) -> u16 {
    val.try_into().unwrap_or(u16::MAX)
}

/// Cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Row (0-indexed, from top of visible area).
    pub row: u16,
    /// Column (0-indexed).
    pub col: u16,
}

impl Cursor {
    /// Create a new cursor at the given position.
    #[must_use]
    #[inline]
    pub const fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

/// Saved cursor state (for DECSC/DECRC).
#[derive(Debug, Clone, Copy, Default)]
pub struct SavedCursor {
    /// Cursor position.
    pub cursor: Cursor,
    /// Whether a saved cursor exists.
    pub valid: bool,
}

/// Scroll region bounds (top and bottom, inclusive, 0-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollRegion {
    /// Top row of scroll region (inclusive, 0-indexed).
    pub top: u16,
    /// Bottom row of scroll region (inclusive, 0-indexed).
    pub bottom: u16,
}

impl ScrollRegion {
    /// Create a scroll region covering all visible rows.
    #[inline]
    pub fn full(visible_rows: u16) -> Self {
        Self {
            top: 0,
            bottom: visible_rows.saturating_sub(1),
        }
    }

    /// Check if this is the full screen (no restricted region).
    #[inline]
    pub fn is_full(&self, visible_rows: u16) -> bool {
        self.top == 0 && self.bottom == visible_rows.saturating_sub(1)
    }
}

/// Terminal grid.
///
/// Uses a ring buffer for O(1) scrolling. The `display_offset` determines
/// what portion of the history is shown.
///
/// When connected to a [`Scrollback`], rows that scroll off the top of the
/// ring buffer are pushed to the scrollback for long-term storage.
#[derive(Debug)]
pub struct Grid {
    /// Page-backed storage for row cell data.
    pages: PageStore,
    /// Row storage (ring buffer).
    rows: Vec<Row>,
    /// Number of visible rows.
    visible_rows: u16,
    /// Number of columns.
    cols: u16,
    /// Maximum scrollback lines in ring buffer.
    max_scrollback: usize,
    /// Total lines in ring buffer (visible + scrollback).
    total_lines: usize,
    /// Display offset (for O(1) scrolling).
    /// 0 = showing live content, >0 = scrolled back into history.
    display_offset: usize,
    /// Cursor position (within visible area).
    cursor: Cursor,
    /// Saved cursor (DECSC/DECRC).
    saved_cursor: SavedCursor,
    /// Damage tracking.
    damage: Damage,
    /// Ring buffer head index (oldest row).
    ring_head: usize,
    /// Optional tiered scrollback for long-term history.
    scrollback: Option<Scrollback>,
    /// Scroll region (DECSTBM).
    scroll_region: ScrollRegion,
    /// Tab stops (true = tab stop at this column).
    /// Default tab stops are every 8 columns.
    tab_stops: Vec<bool>,
    /// Cell extras (hyperlinks, combining chars, underline colors).
    /// Stored separately from cells to keep the common case fast.
    extras: CellExtras,
    /// Generation tracker for pin invalidation.
    /// Tracks page evictions to detect stale pins.
    generations: GenerationTracker,
    /// Absolute row counter (monotonically increasing).
    /// Used for creating absolute pins that survive scrollback eviction.
    absolute_row_counter: u64,
    /// Style deduplication table (Ghostty pattern).
    /// Interns unique styles and provides IDs for memory-efficient storage.
    /// Typical terminals have 50-200 unique styles, providing ~67% memory savings.
    styles: StyleTable,
}

/// Builder for creating [`Grid`] instances with custom configuration.
///
/// Provides a fluent API for configuring grid options before construction.
///
/// # Example
///
/// ```
/// use dterm_core::grid::GridBuilder;
///
/// let grid = GridBuilder::new()
///     .rows(24)
///     .cols(80)
///     .max_scrollback(10_000)
///     .build();
/// ```
#[derive(Debug)]
pub struct GridBuilder {
    rows: u16,
    cols: u16,
    max_scrollback: usize,
    scrollback: Option<Scrollback>,
}

impl Default for GridBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GridBuilder {
    /// Create a new grid builder with default settings.
    ///
    /// Defaults: 24 rows, 80 cols, 10,000 lines scrollback.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rows: 24,
            cols: 80,
            max_scrollback: 10_000,
            scrollback: None,
        }
    }

    /// Set the number of visible rows.
    #[must_use]
    pub fn rows(mut self, rows: u16) -> Self {
        self.rows = rows;
        self
    }

    /// Set the number of columns.
    #[must_use]
    pub fn cols(mut self, cols: u16) -> Self {
        self.cols = cols;
        self
    }

    /// Set the grid size (rows and cols).
    #[must_use]
    pub fn size(mut self, rows: u16, cols: u16) -> Self {
        self.rows = rows;
        self.cols = cols;
        self
    }

    /// Set the maximum in-memory scrollback lines.
    ///
    /// Default is 10,000 lines.
    #[must_use]
    pub fn max_scrollback(mut self, lines: usize) -> Self {
        self.max_scrollback = lines;
        self
    }

    /// Set the tiered scrollback storage for long-term history.
    ///
    /// When set, lines that scroll off the ring buffer will be
    /// pushed to this storage for memory-efficient long-term access.
    #[must_use]
    pub fn scrollback(mut self, scrollback: Scrollback) -> Self {
        self.scrollback = Some(scrollback);
        self
    }

    /// Build the grid with the configured options.
    #[must_use]
    pub fn build(self) -> Grid {
        match self.scrollback {
            Some(scrollback) => {
                Grid::with_tiered_scrollback(self.rows, self.cols, self.max_scrollback, scrollback)
            }
            None => Grid::with_scrollback(self.rows, self.cols, self.max_scrollback),
        }
    }
}

impl Grid {
    /// Create a new grid builder.
    ///
    /// This is a convenience method equivalent to `GridBuilder::new()`.
    #[must_use]
    pub fn builder() -> GridBuilder {
        GridBuilder::new()
    }

    /// Create default tab stops (every 8 columns).
    fn default_tab_stops(cols: u16) -> Vec<bool> {
        (0..cols).map(|c| c > 0 && c % 8 == 0).collect()
    }

    /// Create a new grid with the given dimensions.
    #[must_use]
    pub fn new(rows: u16, cols: u16) -> Self {
        Self::with_scrollback(rows, cols, 10_000)
    }

    /// Create a new grid with custom ring buffer scrollback limit.
    ///
    /// This sets the size of the in-memory ring buffer. For unlimited
    /// scrollback with tiered storage, use [`Grid::with_tiered_scrollback`].
    #[must_use]
    pub fn with_scrollback(rows: u16, cols: u16, max_scrollback: usize) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let capacity = (rows as usize) + max_scrollback;

        // Pre-heat pages based on initial grid size
        // Each row needs cols * 8 bytes (Cell is 8 bytes)
        // PAGE_SIZE = 64KB = 65536 bytes
        // Preheat enough for initial rows + small buffer for scrolling
        let bytes_per_row = (cols as usize) * std::mem::size_of::<Cell>();
        let initial_bytes = (rows as usize) * bytes_per_row;
        let pages_needed = (initial_bytes / PAGE_SIZE).max(1) + 1; // +1 for headroom
        let mut pages = PageStore::with_capacity(pages_needed);
        let mut row_storage = Vec::with_capacity(capacity);
        for _ in 0..rows {
            row_storage.push(Row::new(cols, &mut pages));
        }

        Self {
            pages,
            rows: row_storage,
            visible_rows: rows,
            cols,
            max_scrollback,
            total_lines: rows as usize,
            display_offset: 0,
            cursor: Cursor::default(),
            saved_cursor: SavedCursor::default(),
            damage: Damage::Full,
            ring_head: 0,
            scrollback: None,
            scroll_region: ScrollRegion::full(rows),
            tab_stops: Self::default_tab_stops(cols),
            extras: CellExtras::new(),
            generations: GenerationTracker::new(),
            absolute_row_counter: u64::from(rows),
            styles: StyleTable::new(),
        }
    }

    /// Create a new grid with tiered scrollback storage.
    ///
    /// The ring buffer holds `ring_buffer_size` lines for fast access.
    /// Older lines are pushed to the tiered scrollback for memory-efficient
    /// long-term storage.
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of visible rows
    /// * `cols` - Number of columns
    /// * `ring_buffer_size` - Size of the fast ring buffer (e.g., 1000)
    /// * `scrollback` - Tiered scrollback for long-term storage
    #[must_use]
    pub fn with_tiered_scrollback(
        rows: u16,
        cols: u16,
        ring_buffer_size: usize,
        scrollback: Scrollback,
    ) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let capacity = (rows as usize) + ring_buffer_size;

        // Pre-heat pages based on initial grid size
        let bytes_per_row = (cols as usize) * std::mem::size_of::<Cell>();
        let initial_bytes = (rows as usize) * bytes_per_row;
        let pages_needed = (initial_bytes / PAGE_SIZE).max(1) + 1;
        let mut pages = PageStore::with_capacity(pages_needed);
        let mut row_storage = Vec::with_capacity(capacity);
        for _ in 0..rows {
            row_storage.push(Row::new(cols, &mut pages));
        }

        Self {
            pages,
            rows: row_storage,
            visible_rows: rows,
            cols,
            max_scrollback: ring_buffer_size,
            total_lines: rows as usize,
            display_offset: 0,
            cursor: Cursor::default(),
            saved_cursor: SavedCursor::default(),
            damage: Damage::Full,
            ring_head: 0,
            scrollback: Some(scrollback),
            scroll_region: ScrollRegion::full(rows),
            tab_stops: Self::default_tab_stops(cols),
            extras: CellExtras::new(),
            generations: GenerationTracker::new(),
            absolute_row_counter: u64::from(rows),
            styles: StyleTable::new(),
        }
    }

    /// Attach a scrollback buffer to this grid.
    ///
    /// Lines that scroll off the ring buffer will be pushed to the scrollback.
    pub fn attach_scrollback(&mut self, scrollback: Scrollback) {
        self.scrollback = Some(scrollback);
    }

    /// Detach and return the scrollback buffer, if any.
    pub fn detach_scrollback(&mut self) -> Option<Scrollback> {
        self.scrollback.take()
    }

    /// Get a reference to the scrollback buffer, if attached.
    #[must_use]
    pub fn scrollback(&self) -> Option<&Scrollback> {
        self.scrollback.as_ref()
    }

    /// Get a mutable reference to the scrollback buffer, if attached.
    pub fn scrollback_mut(&mut self) -> Option<&mut Scrollback> {
        self.scrollback.as_mut()
    }

    /// Estimate total memory used by the grid and attached scrollback.
    #[must_use]
    pub fn memory_used(&self) -> usize {
        let mut total = self.pages.total_memory();
        total += self.extras.memory_used();
        total += self.rows.capacity() * std::mem::size_of::<Row>();
        total += self.tab_stops.capacity() * std::mem::size_of::<bool>();
        if let Some(scrollback) = &self.scrollback {
            total += scrollback.memory_used();
        }
        total
    }

    /// Get the number of visible rows.
    #[must_use]
    #[inline]
    pub fn rows(&self) -> u16 {
        self.visible_rows
    }

    /// Get the number of columns.
    #[must_use]
    #[inline]
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Get total lines in buffer (visible + scrollback).
    #[must_use]
    #[inline]
    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    /// Get the display offset (scroll position).
    #[must_use]
    #[inline]
    pub fn display_offset(&self) -> usize {
        self.display_offset
    }

    /// Get the cursor position.
    #[must_use]
    #[inline]
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Get cursor row.
    #[must_use]
    #[inline]
    pub fn cursor_row(&self) -> u16 {
        self.cursor.row
    }

    /// Get cursor column.
    #[must_use]
    #[inline]
    pub fn cursor_col(&self) -> u16 {
        self.cursor.col
    }

    #[inline]
    fn row_is_double_width(&self, row: u16) -> bool {
        self.row(row)
            .map(|r| {
                matches!(
                    r.line_size(),
                    LineSize::DoubleWidth
                        | LineSize::DoubleHeightTop
                        | LineSize::DoubleHeightBottom
                )
            })
            .unwrap_or(false)
    }

    #[inline]
    fn effective_cols_for_row(&self, row: u16) -> u16 {
        let cols = self.cols.max(1);
        if self.row_is_double_width(row) {
            let half = cols / 2;
            if half == 0 {
                1
            } else {
                half
            }
        } else {
            cols
        }
    }

    #[inline]
    fn max_col_for_row(&self, row: u16) -> u16 {
        self.effective_cols_for_row(row).saturating_sub(1)
    }

    #[inline]
    fn clamp_col_for_row(&self, row: u16, col: u16) -> u16 {
        col.min(self.max_col_for_row(row))
    }

    /// Get maximum scrollback available (ring buffer only).
    #[must_use]
    #[inline]
    pub fn ring_buffer_scrollback(&self) -> usize {
        self.total_lines.saturating_sub(self.visible_rows as usize)
    }

    /// Get total scrollback lines available (ring buffer + tiered scrollback).
    #[must_use]
    #[inline]
    pub fn scrollback_lines(&self) -> usize {
        let ring_buffer = self.total_lines.saturating_sub(self.visible_rows as usize);
        let tiered = self.scrollback.as_ref().map_or(0, |s| s.line_count());
        ring_buffer + tiered
    }

    /// Get the current scroll region.
    #[must_use]
    #[inline]
    pub fn scroll_region(&self) -> ScrollRegion {
        self.scroll_region
    }

    /// Set the scroll region (DECSTBM).
    ///
    /// `top` and `bottom` are 0-indexed row numbers.
    /// If top >= bottom or either is out of bounds, the region is reset to full screen.
    pub fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        if top < bottom && bottom < self.visible_rows {
            self.scroll_region = ScrollRegion { top, bottom };
        } else {
            self.scroll_region = ScrollRegion::full(self.visible_rows);
        }
    }

    /// Reset scroll region to full screen.
    #[inline]
    pub fn reset_scroll_region(&mut self) {
        self.scroll_region = ScrollRegion::full(self.visible_rows);
    }

    /// Get the number of lines in the tiered scrollback (if any).
    #[must_use]
    #[inline]
    pub fn tiered_scrollback_lines(&self) -> usize {
        self.scrollback.as_ref().map_or(0, |s| s.line_count())
    }

    /// Get damage state.
    #[must_use]
    #[inline]
    pub fn damage(&self) -> &Damage {
        &self.damage
    }

    /// Get mutable damage state.
    #[inline]
    pub fn damage_mut(&mut self) -> &mut Damage {
        &mut self.damage
    }

    /// Get cell extras storage.
    #[must_use]
    #[inline]
    pub fn extras(&self) -> &CellExtras {
        &self.extras
    }

    /// Get mutable cell extras storage.
    #[inline]
    pub fn extras_mut(&mut self) -> &mut CellExtras {
        &mut self.extras
    }

    // -------------------------------------------------------------------------
    // Style deduplication API
    // -------------------------------------------------------------------------

    /// Get the style table.
    #[must_use]
    #[inline]
    pub fn styles(&self) -> &StyleTable {
        &self.styles
    }

    /// Get mutable access to the style table.
    #[inline]
    pub fn styles_mut(&mut self) -> &mut StyleTable {
        &mut self.styles
    }

    /// Intern a style and return its ID.
    ///
    /// If the style already exists, returns the existing ID.
    /// Otherwise, creates a new entry.
    ///
    /// # Example
    ///
    /// ```
    /// use dterm_core::grid::{Grid, Style, Color, StyleAttrs};
    ///
    /// let mut grid = Grid::new(24, 80);
    /// let style = Style::new(
    ///     Color::new(255, 0, 0),
    ///     Color::DEFAULT_BG,
    ///     StyleAttrs::BOLD,
    /// );
    /// let id = grid.intern_style(style);
    /// assert!(!id.is_default());
    /// ```
    #[inline]
    pub fn intern_style(&mut self, style: Style) -> StyleId {
        self.styles.intern(style)
    }

    /// Intern an extended style with color type information.
    ///
    /// This preserves the original color type (default/indexed/rgb) for
    /// later conversion back to `PackedColors` format.
    #[inline]
    pub fn intern_extended_style(&mut self, ext_style: ExtendedStyle) -> StyleId {
        self.styles.intern_extended(ext_style)
    }

    /// Get a style by its ID.
    #[must_use]
    #[inline]
    pub fn get_style(&self, id: StyleId) -> Option<&Style> {
        self.styles.get(id)
    }

    /// Get style table statistics.
    ///
    /// Returns information about unique styles, reference counts, and memory usage.
    #[must_use]
    #[inline]
    pub fn style_stats(&self) -> StyleTableStats {
        self.styles.stats()
    }

    /// Clear all styles except the default.
    ///
    /// This invalidates all existing StyleIds. Only call during terminal reset.
    pub fn clear_styles(&mut self) {
        self.styles.clear();
    }

    /// Get extras for a specific cell.
    #[must_use]
    #[inline]
    pub fn cell_extra(&self, row: u16, col: u16) -> Option<&CellExtra> {
        self.extras.get(CellCoord::new(row, col))
    }

    /// Get or create extras for a specific cell.
    #[inline]
    pub fn cell_extra_mut(&mut self, row: u16, col: u16) -> &mut CellExtra {
        self.extras.get_or_create(CellCoord::new(row, col))
    }

    /// Get the display character(s) for a cell, looking up complex chars from overflow.
    ///
    /// For simple BMP characters, returns a string containing just that character.
    /// For complex cells (non-BMP, grapheme clusters), looks up the string from CellExtras.
    /// For wide continuation cells, returns an empty string.
    ///
    /// Returns `None` if the cell doesn't exist.
    #[must_use]
    pub fn cell_display_char(&self, row: u16, col: u16) -> Option<String> {
        let r = self.row(row)?;
        let cell = r.get(col)?;

        if cell.is_wide_continuation() {
            return Some(String::new());
        }

        if cell.is_complex() {
            // Look up in overflow table
            if let Some(extra) = self.extras.get(CellCoord::new(row, col)) {
                if let Some(complex_str) = extra.complex_char() {
                    return Some(complex_str.to_string());
                }
            }
            // Fallback: complex flag set but no overflow data
            return Some(String::from('\u{FFFD}'));
        }

        // Simple BMP character
        Some(cell.char().to_string())
    }

    /// Get the text content of a visible row, resolving complex characters from overflow.
    ///
    /// This is like `Row::to_string()` but properly handles non-BMP characters
    /// stored in the overflow table.
    #[must_use]
    pub fn row_text(&self, row: u16) -> Option<String> {
        let r = self.row(row)?;
        let mut s = String::with_capacity(r.len() as usize);

        for col in 0..r.len() {
            if let Some(cell) = r.get(col) {
                if cell.is_wide_continuation() {
                    continue;
                }

                if cell.is_complex() {
                    // Look up in overflow table
                    if let Some(extra) = self.extras.get(CellCoord::new(row, col)) {
                        if let Some(complex_str) = extra.complex_char() {
                            s.push_str(complex_str);
                            continue;
                        }
                    }
                    // Fallback: complex flag set but no overflow data
                    s.push('\u{FFFD}');
                } else {
                    s.push(cell.char());
                }
            }
        }

        Some(s)
    }

    // === Pin System ===

    /// Get the generation tracker for pin validation.
    #[must_use]
    #[inline]
    pub fn generations(&self) -> &GenerationTracker {
        &self.generations
    }

    /// Get the current generation (for creating pins).
    #[must_use]
    #[inline]
    pub fn current_generation(&self) -> Generation {
        self.generations.current_generation()
    }

    /// Get the absolute row counter (monotonically increasing).
    #[must_use]
    #[inline]
    pub fn absolute_row_counter(&self) -> u64 {
        self.absolute_row_counter
    }

    /// Create a pin at the given visible row and column.
    ///
    /// The pin tracks the absolute position so it survives scrolling.
    /// Use `is_pin_valid` to check if the pin is still valid after scrollback.
    #[must_use]
    pub fn create_pin(&self, visible_row: u16, col: u16) -> Pin {
        // Calculate absolute row from visible row
        let scrollback_lines = self.total_lines.saturating_sub(self.visible_rows as usize);
        let absolute_row =
            self.absolute_row_counter - u64::from(self.visible_rows) + u64::from(visible_row);
        let absolute_row = absolute_row.saturating_sub(scrollback_lines as u64);

        Pin::from_absolute(absolute_row, col, self.current_generation())
    }

    /// Create a pin at the cursor position.
    #[must_use]
    pub fn create_cursor_pin(&self) -> Pin {
        self.create_pin(self.cursor.row, self.cursor.col)
    }

    /// Check if a pin is still valid.
    ///
    /// A pin becomes invalid when the content it references is evicted from scrollback.
    #[must_use]
    pub fn is_pin_valid(&self, pin: &Pin) -> bool {
        self.generations.is_potentially_valid(pin)
    }

    /// Try to resolve a pin to visible coordinates.
    ///
    /// Returns `Some((row, col))` if the pin refers to a currently visible position.
    /// Returns `None` if the pin is invalid or not currently visible.
    #[must_use]
    pub fn resolve_pin(&self, pin: &Pin) -> Option<(u16, u16)> {
        if !self.is_pin_valid(pin) {
            return None;
        }

        let pin_absolute = pin.absolute_row();
        let current_absolute = self.absolute_row_counter;

        // Check if pin is in visible range
        let visible_start = current_absolute.saturating_sub(u64::from(self.visible_rows));
        if pin_absolute < visible_start || pin_absolute >= current_absolute {
            return None;
        }

        let visible_row = u64_to_u16_saturating(pin_absolute - visible_start);
        if visible_row >= self.visible_rows || pin.col() >= self.cols {
            return None;
        }

        Some((visible_row, pin.col()))
    }

    /// Try to resolve a pin to a cell reference.
    ///
    /// Returns `Some(&Cell)` if the pin is valid and visible.
    #[must_use]
    pub fn resolve_pin_to_cell(&self, pin: &Pin) -> Option<&Cell> {
        let (row, col) = self.resolve_pin(pin)?;
        self.cell(row, col)
    }

    /// Check if a pinned range is still valid.
    #[must_use]
    pub fn is_range_valid(&self, range: &PinnedRange) -> bool {
        range.is_potentially_valid(&self.generations)
    }

    /// Convert visible coordinates to absolute row number.
    #[must_use]
    pub fn visible_to_absolute(&self, visible_row: u16) -> u64 {
        let scrollback_lines = self.total_lines.saturating_sub(self.visible_rows as usize);
        self.absolute_row_counter
            .saturating_sub(u64::from(self.visible_rows))
            .saturating_add(u64::from(visible_row))
            .saturating_sub(scrollback_lines as u64)
    }

    /// Convert a visible row index to the internal ring buffer index.
    ///
    /// Performance critical: Called on every cell access. Forces inlining
    /// to ensure this gets inlined across module boundaries.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn row_index(&self, visible_row: u16) -> usize {
        // Calculate the actual row in the ring buffer.
        // The ring buffer stores all rows (scrollback + visible).
        // visible_row 0 is the top of the visible area.
        let scrollback = self.total_lines.saturating_sub(self.visible_rows as usize);
        let absolute_row = scrollback + (visible_row as usize) - self.display_offset;
        (self.ring_head + absolute_row) % self.rows.len()
    }

    /// Copy row content between two ring buffer indices without cloning.
    fn copy_row_indexed(&mut self, dst_idx: usize, src_idx: usize) {
        if dst_idx == src_idx {
            return;
        }
        if dst_idx < src_idx {
            let (left, right) = self.rows.split_at_mut(src_idx);
            let dst = &mut left[dst_idx];
            let src = &right[0];
            dst.copy_from(src);
        } else {
            let (left, right) = self.rows.split_at_mut(dst_idx);
            let src = &left[src_idx];
            let dst = &mut right[0];
            dst.copy_from(src);
        }
    }

    /// Get a row by visible row index.
    #[must_use]
    pub fn row(&self, visible_row: u16) -> Option<&Row> {
        if visible_row >= self.visible_rows {
            return None;
        }
        let idx = self.row_index(visible_row);
        self.rows.get(idx)
    }

    /// Get a mutable row by visible row index.
    pub fn row_mut(&mut self, visible_row: u16) -> Option<&mut Row> {
        if visible_row >= self.visible_rows {
            return None;
        }
        let idx = self.row_index(visible_row);
        self.rows.get_mut(idx)
    }

    /// Get a cell at the given position.
    #[must_use]
    pub fn cell(&self, row: u16, col: u16) -> Option<&Cell> {
        self.row(row).and_then(|r| r.get(col))
    }

    /// Get a mutable cell at the given position.
    pub fn cell_mut(&mut self, row: u16, col: u16) -> Option<&mut Cell> {
        self.row_mut(row).and_then(|r| r.get_mut(col))
    }

    /// Set cursor position (clamped to bounds).
    #[inline]
    pub fn set_cursor(&mut self, row: u16, col: u16) {
        let row = row.min(self.visible_rows.saturating_sub(1));
        self.cursor.row = row;
        self.cursor.col = self.clamp_col_for_row(row, col);
    }

    /// Move cursor to position.
    #[inline]
    pub fn move_cursor_to(&mut self, row: u16, col: u16) {
        self.set_cursor(row, col);
    }

    /// Move cursor by relative offset.
    #[inline]
    pub fn move_cursor_by(&mut self, dr: i32, dc: i32) {
        let new_row = clamp_u16(i32::from(self.cursor.row) + dr);
        let new_col = clamp_u16(i32::from(self.cursor.col) + dc);
        self.set_cursor(new_row, new_col);
    }

    /// Move cursor up by n rows, respecting scroll region margins.
    ///
    /// Per VT510: The cursor stops at the top margin if within the scroll region.
    /// If already above the top margin, stops at the top line (row 0).
    #[inline]
    pub fn cursor_up(&mut self, n: u16) {
        let region = self.scroll_region;
        let min_row = if self.cursor.row >= region.top && self.cursor.row <= region.bottom {
            // Cursor is within scroll region - stop at top margin
            region.top
        } else {
            // Cursor is outside scroll region - stop at line 0
            0
        };
        self.cursor.row = self.cursor.row.saturating_sub(n).max(min_row);
        let row = self.cursor.row;
        self.cursor.col = self.clamp_col_for_row(row, self.cursor.col);
    }

    /// Move cursor down by n rows, respecting scroll region margins.
    ///
    /// Per VT510: The cursor stops at the bottom margin if within the scroll region.
    /// If already below the bottom margin, stops at the bottom line.
    #[inline]
    pub fn cursor_down(&mut self, n: u16) {
        let region = self.scroll_region;
        let max_row = if self.cursor.row >= region.top && self.cursor.row <= region.bottom {
            // Cursor is within scroll region - stop at bottom margin
            region.bottom
        } else {
            // Cursor is outside scroll region - stop at last line
            self.visible_rows.saturating_sub(1)
        };
        self.cursor.row = (self.cursor.row + n).min(max_row);
        let row = self.cursor.row;
        self.cursor.col = self.clamp_col_for_row(row, self.cursor.col);
    }

    /// Move cursor forward (right) by n columns.
    ///
    /// Stops at the right edge of the screen.
    #[inline]
    pub fn cursor_forward(&mut self, n: u16) {
        let max_col = self.max_col_for_row(self.cursor.row);
        self.cursor.col = self.cursor.col.saturating_add(n).min(max_col);
    }

    /// Move cursor backward (left) by n columns.
    ///
    /// Stops at the left edge of the screen (column 0).
    #[inline]
    pub fn cursor_backward(&mut self, n: u16) {
        self.cursor.col = self.cursor.col.saturating_sub(n);
    }

    /// Move cursor to column 0 (carriage return).
    #[inline]
    pub fn carriage_return(&mut self) {
        self.cursor.col = 0;
    }

    /// Move cursor down one row, scrolling if at bottom of scroll region.
    pub fn line_feed(&mut self) {
        let bottom = self.scroll_region.bottom;
        match self.cursor.row.cmp(&bottom) {
            std::cmp::Ordering::Less => {
                // Within scroll region - just move down
                self.cursor.row += 1;
            }
            std::cmp::Ordering::Equal => {
                // At bottom of scroll region - scroll within region
                self.scroll_region_up(1);
            }
            std::cmp::Ordering::Greater => {
                // Below scroll region - move down if possible
                if self.cursor.row < self.visible_rows - 1 {
                    self.cursor.row += 1;
                }
            }
        }
        let row = self.cursor.row;
        self.cursor.col = self.clamp_col_for_row(row, self.cursor.col);
    }

    /// Move cursor up one row, scrolling if at top of scroll region.
    #[inline]
    pub fn reverse_line_feed(&mut self) {
        let top = self.scroll_region.top;
        match self.cursor.row.cmp(&top) {
            std::cmp::Ordering::Greater => {
                // Within scroll region - just move up
                self.cursor.row -= 1;
            }
            std::cmp::Ordering::Equal => {
                // At top of scroll region - scroll region down
                self.scroll_region_down(1);
            }
            std::cmp::Ordering::Less => {
                // Above scroll region - move up if possible
                if self.cursor.row > 0 {
                    self.cursor.row -= 1;
                }
            }
        }
        let row = self.cursor.row;
        self.cursor.col = self.clamp_col_for_row(row, self.cursor.col);
    }

    /// Tab (move to next tab stop).
    #[inline]
    pub fn tab(&mut self) {
        // Find the next tab stop after the current column
        let max_col = self.max_col_for_row(self.cursor.row);
        let start = usize::from(self.cursor.col.saturating_add(1));
        let end = usize::from(max_col);
        if start <= end {
            for col in start..=end {
                if self.tab_stops[col] {
                    self.cursor.col = row_u16(col);
                    return;
                }
            }
        }
        // No tab stop found, move to last column
        self.cursor.col = max_col;
    }

    /// Tab forward by n stops.
    ///
    /// Implements CHT (Cursor Horizontal Tab) - CSI Ps I.
    /// Moves cursor forward through n tab stops.
    #[inline]
    pub fn tab_n(&mut self, n: u16) {
        for _ in 0..n {
            self.tab();
        }
    }

    /// Back tab (move to previous tab stop).
    ///
    /// Implements CBT (Cursor Backward Tabulation) - CSI Ps Z.
    /// Moves cursor to the previous tab stop, or column 0 if no prior tab stop exists.
    #[inline]
    pub fn back_tab(&mut self) {
        // Find the previous tab stop before the current column
        let max_col = usize::from(self.max_col_for_row(self.cursor.row));
        let current = usize::from(self.cursor.col).min(max_col);
        if current == 0 {
            return; // Already at column 0
        }
        for col in (0..current).rev() {
            if self.tab_stops[col] {
                self.cursor.col = row_u16(col);
                return;
            }
        }
        // No tab stop found, move to column 0
        self.cursor.col = 0;
    }

    /// Back tab by n stops.
    ///
    /// Moves cursor backward through n tab stops.
    #[inline]
    pub fn back_tab_n(&mut self, n: u16) {
        for _ in 0..n {
            self.back_tab();
        }
    }

    /// Set a tab stop at the current cursor column (HTS - Horizontal Tab Set).
    #[inline]
    pub fn set_tab_stop(&mut self) {
        let col = self.cursor.col as usize;
        if col < self.tab_stops.len() {
            self.tab_stops[col] = true;
        }
    }

    /// Clear the tab stop at the current cursor column (TBC 0).
    #[inline]
    pub fn clear_tab_stop(&mut self) {
        let col = self.cursor.col as usize;
        if col < self.tab_stops.len() {
            self.tab_stops[col] = false;
        }
    }

    /// Clear all tab stops (TBC 3).
    #[inline]
    pub fn clear_all_tab_stops(&mut self) {
        self.tab_stops.fill(false);
    }

    /// Reset tab stops to default (every 8 columns).
    #[inline]
    pub fn reset_tab_stops(&mut self) {
        self.tab_stops = Self::default_tab_stops(self.cols);
    }

    /// Check if there is a tab stop at the given column.
    ///
    /// Returns `false` if the column is out of bounds.
    #[inline]
    pub fn is_tab_stop(&self, col: u16) -> bool {
        self.tab_stops.get(col as usize).copied().unwrap_or(false)
    }

    /// Get an iterator over all tab stop column positions.
    ///
    /// Returns columns (0-indexed) where tab stops are set.
    #[allow(clippy::cast_possible_truncation)] // cols is u16, so tab_stops index fits in u16
    pub fn tab_stop_positions(&self) -> impl Iterator<Item = u16> + '_ {
        self.tab_stops
            .iter()
            .enumerate()
            .filter_map(|(col, &is_stop)| is_stop.then_some(col as u16))
    }

    /// Backspace (move left by 1).
    #[inline]
    pub fn backspace(&mut self) {
        self.cursor.col = self.cursor.col.saturating_sub(1);
    }

    /// Save cursor position (DECSC).
    #[inline]
    pub fn save_cursor(&mut self) {
        self.saved_cursor = SavedCursor {
            cursor: self.cursor,
            valid: true,
        };
    }

    /// Restore cursor position (DECRC).
    #[inline]
    pub fn restore_cursor(&mut self) {
        if self.saved_cursor.valid {
            self.set_cursor(self.saved_cursor.cursor.row, self.saved_cursor.cursor.col);
        }
    }

    /// Scroll the display by delta lines.
    ///
    /// Positive delta = scroll up (show older content).
    /// Negative delta = scroll down (show newer content).
    pub fn scroll_display(&mut self, delta: i32) {
        let max_offset = self.scrollback_lines();
        // display_offset is bounded by max scrollback (MAX_SCROLLBACK_LINES = 1M)
        // which fits in i32. Use saturating conversion for safety.
        let current: i32 = self.display_offset.try_into().unwrap_or(i32::MAX);
        let clamped = current.saturating_add(delta).max(0);
        // SAFETY: max(0) ensures non-negative, and we clamp to max_offset anyway
        #[allow(clippy::cast_sign_loss)]
        let new_offset = clamped as usize;
        self.display_offset = new_offset.min(max_offset);
        self.damage.mark_full();
    }

    /// Scroll to the top of scrollback.
    pub fn scroll_to_top(&mut self) {
        self.display_offset = self.scrollback_lines();
        self.damage.mark_full();
    }

    /// Scroll to live position (bottom).
    pub fn scroll_to_bottom(&mut self) {
        self.display_offset = 0;
        self.damage.mark_full();
    }

    /// Scroll content up by n lines (new empty lines at bottom).
    ///
    /// When a scrollback is attached and the ring buffer is at capacity,
    /// the oldest row is converted to a [`Line`] and pushed to the scrollback
    /// before being overwritten.
    ///
    /// ## Optimization
    ///
    /// This function is optimized for batch operations:
    /// - Pre-calculates how many rows to add vs reuse
    /// - Batch reserves Vec capacity for growth phase
    /// - Updates counters in bulk to reduce loop overhead
    pub fn scroll_up(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        let capacity = (self.visible_rows as usize) + self.max_scrollback;
        let cols = self.cols;

        // Pre-calculate: how many rows can we add before hitting capacity?
        let rows_until_capacity = capacity.saturating_sub(self.total_lines);
        let rows_to_add = n.min(rows_until_capacity);
        let rows_to_reuse = n.saturating_sub(rows_to_add);

        // Phase 1: Batch grow the ring buffer (if not at capacity)
        if rows_to_add > 0 {
            // Reserve capacity upfront to avoid multiple reallocations
            self.rows.reserve(rows_to_add);

            // Batch create new rows
            for _ in 0..rows_to_add {
                self.rows.push(Row::new(cols, &mut self.pages));
            }
            self.total_lines += rows_to_add;
            self.absolute_row_counter += rows_to_add as u64;
        }

        // Phase 2: Reuse oldest rows (at capacity)
        if rows_to_reuse > 0 {
            let row_count = self.rows.len();

            for _ in 0..rows_to_reuse {
                let oldest = self.ring_head;

                // Push to tiered scrollback before overwriting
                if self.scrollback.is_some() {
                    let line = Self::row_to_line_static(&self.rows[oldest]);
                    self.scrollback.as_mut().unwrap().push_line(line);
                }

                // Mark the page as evicted for pin invalidation
                let page_id = self.rows[oldest].page_id();
                self.generations.evict_page(page_id);

                // Reuse the oldest row
                self.rows[oldest].clear();
                self.rows[oldest].resize(cols, &mut self.pages);
                self.ring_head = (self.ring_head + 1) % row_count;
            }
            self.absolute_row_counter += rows_to_reuse as u64;
        }

        self.damage.mark_full();
    }

    /// Scroll up respecting the active scroll region.
    ///
    /// This is used by Kani proofs and may be used for future optimizations.
    #[inline]
    #[allow(dead_code)]
    fn scroll_up_in_region(&mut self, n: usize) {
        if self.scroll_region.is_full(self.visible_rows) {
            self.scroll_up(n);
        } else {
            self.scroll_region_up(n);
        }
    }

    /// Convert a Row to a Line for scrollback storage.
    fn row_to_line(&self, row: &Row) -> Line {
        Self::row_to_line_static(row)
    }

    /// Convert a Row to a Line (static version to avoid borrow issues).
    ///
    /// Extracts both text content and RLE-compressed cell attributes.
    fn row_to_line_static(row: &Row) -> Line {
        use crate::rle::Rle;
        use crate::scrollback::CellAttrs;

        let len = row.len() as usize;
        if len == 0 {
            let mut line = Line::new();
            if row.is_wrapped() {
                line.set_wrapped(true);
            }
            return line;
        }

        // Extract text and build RLE-compressed attributes simultaneously
        let mut text = String::with_capacity(len);
        let mut attrs_rle: Rle<CellAttrs> = Rle::new();

        for cell in &row.as_slice()[..len] {
            // Skip wide continuation cells (they don't contribute a character)
            if cell.is_wide_continuation() {
                continue;
            }

            // Add character
            text.push(cell.char());

            // Extract cell attributes for RLE
            let fg = cell.fg();
            let bg = cell.bg();
            let flags = cell.flags();

            // Convert to CellAttrs (using raw PackedColor u32 values)
            let attrs = CellAttrs::from_raw(fg.0, bg.0, flags.bits());

            // Push to RLE (will merge with previous run if same)
            attrs_rle.push(attrs);
        }

        // Create line with attrs
        let mut line = Line::with_attrs(&text, attrs_rle);
        if row.is_wrapped() {
            line.set_wrapped(true);
        }
        line
    }

    /// Scroll content down by n lines (new empty lines at top).
    pub fn scroll_down(&mut self, n: usize) {
        // This is reverse scroll - less common
        // For now, just mark full damage and shift content
        for _ in 0..n {
            if self.total_lines > self.visible_rows as usize {
                self.total_lines -= 1;
            }
        }
        self.damage.mark_full();
    }

    /// Scroll within scroll region: move content up (blank line at bottom of region).
    ///
    /// This is used when cursor is at bottom of scroll region and line feed is issued.
    /// Only lines within the scroll region are affected.
    pub fn scroll_region_up(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        let top = usize::from(self.scroll_region.top);
        let bottom = usize::from(self.scroll_region.bottom);

        // If scroll region is full screen, use regular scroll_up (adds to scrollback)
        if self.scroll_region.is_full(self.visible_rows) {
            self.scroll_up(n);
            return;
        }

        // Scroll within the region only (no scrollback)
        let region_size = bottom - top + 1;
        let n = n.min(region_size);

        // Shift rows up within the region
        for dst in top..(bottom + 1 - n) {
            let src = dst + n;
            let src_idx = self.row_index(row_u16(src));
            let dst_idx = self.row_index(row_u16(dst));
            if src_idx != dst_idx {
                self.copy_row_indexed(dst_idx, src_idx);
            }
        }

        // Clear the bottom n rows of the region
        for row in (bottom + 1 - n)..=bottom {
            if let Some(r) = self.row_mut(row_u16(row)) {
                r.clear();
            }
        }

        self.damage.mark_full();
    }

    /// Scroll within scroll region: move content down (blank line at top of region).
    ///
    /// This is used when cursor is at top of scroll region and reverse line feed is issued.
    /// Only lines within the scroll region are affected.
    pub fn scroll_region_down(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        let top = usize::from(self.scroll_region.top);
        let bottom = usize::from(self.scroll_region.bottom);
        let region_size = bottom - top + 1;
        let n = n.min(region_size);

        // Shift rows down within the region (backwards to avoid overwriting)
        for dst in ((top + n)..=bottom).rev() {
            let src = dst - n;
            let src_idx = self.row_index(row_u16(src));
            let dst_idx = self.row_index(row_u16(dst));
            if src_idx != dst_idx {
                self.copy_row_indexed(dst_idx, src_idx);
            }
        }

        // Clear the top n rows of the region
        for row in top..(top + n) {
            if let Some(r) = self.row_mut(row_u16(row)) {
                r.clear();
            }
        }

        self.damage.mark_full();
    }

    /// Write a character at cursor position and advance cursor.
    pub fn write_char(&mut self, c: char) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        if let Some(row) = self.row_mut(cursor_row) {
            row.write_char(cursor_col, c);
        }
        self.damage.mark_cell(cursor_row, cursor_col);

        // Advance cursor
        let max_col = self.max_col_for_row(cursor_row);
        if self.cursor.col < max_col {
            self.cursor.col += 1;
        }
    }

    /// Write a styled character at cursor position and advance cursor.
    pub fn write_char_styled(
        &mut self,
        c: char,
        fg: PackedColor,
        bg: PackedColor,
        flags: CellFlags,
    ) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        if let Some(row) = self.row_mut(cursor_row) {
            row.write_char_styled(cursor_col, c, fg, bg, flags);
        }
        self.damage.mark_cell(cursor_row, cursor_col);

        // Advance cursor
        let max_col = self.max_col_for_row(cursor_row);
        if self.cursor.col < max_col {
            self.cursor.col += 1;
        }
    }

    /// Write a character using a style ID at cursor position and advance cursor.
    ///
    /// This is the style-interning-aware version of `write_char_styled`.
    /// It looks up the style from the StyleTable and writes the cell with
    /// the resolved colors and flags.
    ///
    /// # Arguments
    /// * `c` - Character to write
    /// * `style_id` - ID of the style in the StyleTable
    /// * `extra_flags` - Additional cell flags (e.g., WIDE) to merge with style flags
    pub fn write_char_with_style_id(&mut self, c: char, style_id: StyleId, extra_flags: CellFlags) {
        let (fg, bg, flags) = self.resolve_style_to_colors(style_id, extra_flags);
        self.write_char_styled(c, fg, bg, flags);
    }

    /// Resolve a StyleId to PackedColor values and CellFlags.
    ///
    /// This helper method looks up the style and converts it to the format
    /// needed for cell writes. Returns (fg, bg, flags).
    #[inline]
    fn resolve_style_to_colors(
        &self,
        style_id: StyleId,
        extra_flags: CellFlags,
    ) -> (PackedColor, PackedColor, CellFlags) {
        if let Some(ext_style) = self.styles.get_extended(style_id) {
            // Convert extended style back to PackedColor format
            let fg = match ext_style.fg_type {
                ColorType::Default => PackedColor::DEFAULT_FG,
                ColorType::Indexed => PackedColor::indexed(ext_style.fg_index),
                ColorType::Rgb => {
                    let rgb = ext_style.style.fg.to_rgb();
                    PackedColor::rgb(rgb.0, rgb.1, rgb.2)
                }
            };
            let bg = match ext_style.bg_type {
                ColorType::Default => PackedColor::DEFAULT_BG,
                ColorType::Indexed => PackedColor::indexed(ext_style.bg_index),
                ColorType::Rgb => {
                    let rgb = ext_style.style.bg.to_rgb();
                    PackedColor::rgb(rgb.0, rgb.1, rgb.2)
                }
            };
            let flags =
                ExtendedStyle::attrs_to_cell_flags(ext_style.style.attrs).union(extra_flags);
            (fg, bg, flags)
        } else {
            // Fallback to default style
            (
                PackedColor::DEFAULT_FG,
                PackedColor::DEFAULT_BG,
                extra_flags,
            )
        }
    }

    /// Write a character with autowrap.
    pub fn write_char_wrap(&mut self, c: char) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        if let Some(row) = self.row_mut(cursor_row) {
            row.write_char(cursor_col, c);
        }
        self.damage.mark_cell(cursor_row, cursor_col);

        // Advance cursor with wrap
        let max_col = self.max_col_for_row(cursor_row);
        if self.cursor.col < max_col {
            self.cursor.col += 1;
        } else {
            // Wrap to next line
            self.cursor.col = 0;
            if self.cursor.row < self.visible_rows - 1 {
                self.cursor.row += 1;
                let new_row = self.cursor.row;
                if let Some(row) = self.row_mut(new_row) {
                    row.set_wrapped(true);
                }
            } else {
                self.scroll_up(1);
            }
        }
    }

    /// Write a styled character with autowrap.
    pub fn write_char_wrap_styled(
        &mut self,
        c: char,
        fg: PackedColor,
        bg: PackedColor,
        flags: CellFlags,
    ) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        if let Some(row) = self.row_mut(cursor_row) {
            row.write_char_styled(cursor_col, c, fg, bg, flags);
        }
        self.damage.mark_cell(cursor_row, cursor_col);

        // Advance cursor with wrap
        let max_col = self.max_col_for_row(cursor_row);
        if self.cursor.col < max_col {
            self.cursor.col += 1;
        } else {
            // Wrap to next line
            self.cursor.col = 0;
            if self.cursor.row < self.visible_rows - 1 {
                self.cursor.row += 1;
                let new_row = self.cursor.row;
                if let Some(row) = self.row_mut(new_row) {
                    row.set_wrapped(true);
                }
            } else {
                self.scroll_up(1);
            }
        }
    }

    /// Write a character with autowrap using a style ID.
    ///
    /// This is the style-interning-aware version of `write_char_wrap_styled`.
    /// It looks up the style from the StyleTable and writes the cell with
    /// the resolved colors and flags.
    ///
    /// # Arguments
    /// * `c` - Character to write
    /// * `style_id` - ID of the style in the StyleTable
    /// * `extra_flags` - Additional cell flags (e.g., WIDE) to merge with style flags
    pub fn write_char_wrap_with_style_id(
        &mut self,
        c: char,
        style_id: StyleId,
        extra_flags: CellFlags,
    ) {
        let (fg, bg, flags) = self.resolve_style_to_colors(style_id, extra_flags);
        self.write_char_wrap_styled(c, fg, bg, flags);
    }

    /// FAST PATH: Write a run of ASCII bytes directly to cells.
    ///
    /// # Preconditions (caller must verify)
    /// - All bytes in `ascii` are printable ASCII (0x20..=0x7E)
    /// - Current style is default (no colors, no attributes)
    /// - Insert mode is OFF
    /// - Auto-wrap mode is ON
    ///
    /// This bypasses all per-character overhead:
    /// - No charset translation (ASCII is ASCII)
    /// - No width calculation (ASCII is always width 1)
    /// - No combining character check (ASCII is never combining)
    /// - No wide character handling (ASCII is never wide)
    /// - No Cell::new() overhead - direct memory write
    ///
    /// # Performance
    /// Achieves 400+ MB/s by writing cells as raw u64 values.
    ///
    /// # Returns
    /// Number of bytes written (may be less than `ascii.len()` if wrap/scroll occurs)
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // to_write is bounded by effective_cols (u16)
    pub fn write_ascii_blast(&mut self, ascii: &[u8]) -> usize {
        if ascii.is_empty() {
            return 0;
        }

        let mut written = 0;
        let mut remaining = ascii;

        while !remaining.is_empty() {
            let cursor_row = self.cursor.row;
            let cursor_col = self.cursor.col;
            let effective_cols = self.effective_cols_for_row(cursor_row);
            let max_col = effective_cols.saturating_sub(1);

            // How many chars can we write on this line?
            let available = (effective_cols.saturating_sub(cursor_col)) as usize;
            let to_write = remaining.len().min(available);

            if to_write == 0 {
                // At end of line, need to wrap
                self.cursor.col = 0;
                if self.cursor.row < self.visible_rows - 1 {
                    self.cursor.row += 1;
                    let new_row = self.cursor.row;
                    if let Some(row) = self.row_mut(new_row) {
                        row.set_wrapped(true);
                    }
                } else {
                    self.scroll_up(1);
                }
                continue;
            }

            // Write directly to row cells
            if let Some(row) = self.row_mut(cursor_row) {
                // Fix up any wide characters that would be orphaned by this write
                row.fixup_wide_chars_in_range(cursor_col, to_write as u16);

                // SAFETY: We verified bytes are printable ASCII in preconditions
                // Direct write of 8-byte cells: [char_data:2][colors:4][flags:2]
                // For default style ASCII: char_data = byte, colors = 0, flags = 0
                // This is equivalent to: (byte as u64) in little-endian
                let cells = row.cells_mut();
                let start = cursor_col as usize;

                for (i, &byte) in remaining[..to_write].iter().enumerate() {
                    let cell_idx = start + i;
                    if cell_idx < cells.len() {
                        // Create Cell with ASCII char, default colors, no flags
                        // This is the hot path - minimize overhead
                        cells[cell_idx] = Cell::from_ascii_fast(byte);
                    }
                }

                // Update row len to include written content
                row.update_len(cursor_col + to_write as u16);
            }

            // Mark damage for the row (mark_row is faster than per-cell)
            self.damage.mark_row(cursor_row);

            // Advance cursor - but don't go past last column
            // If we wrote to the last column, wrap immediately for next char
            let new_col = cursor_col + to_write as u16;
            if new_col > max_col {
                // Wrote to last column - wrap to next line
                self.cursor.col = 0;
                if self.cursor.row < self.visible_rows - 1 {
                    self.cursor.row += 1;
                    let new_row = self.cursor.row;
                    if let Some(row) = self.row_mut(new_row) {
                        row.set_wrapped(true);
                    }
                } else {
                    self.scroll_up(1);
                }
            } else {
                self.cursor.col = new_col;
            }

            written += to_write;
            remaining = &remaining[to_write..];
        }

        written
    }

    /// FAST PATH: Write a run of ASCII bytes with style and autowrap.
    ///
    /// This is the high-performance path for ASCII text with non-default styling.
    /// Handles autowrap and scrolling, but skips:
    /// - Character set translation (caller ensures ASCII passthrough)
    /// - Width calculation (ASCII is always width 1)
    /// - Insert mode (caller handles)
    /// - RGB/hyperlink overflow (caller uses simpler colors)
    ///
    /// # Preconditions (caller must verify)
    /// - All bytes in `ascii` are printable ASCII (0x20..=0x7E)
    /// - No RGB colors in fg/bg (or they're handled via overflow by caller)
    /// - Not in insert mode (or caller handles it)
    ///
    /// # Returns
    /// Number of bytes written. Updates `last_byte` with the last character written.
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // to_write is bounded by effective_cols (u16)
    pub fn write_ascii_run_styled(
        &mut self,
        ascii: &[u8],
        fg: PackedColor,
        bg: PackedColor,
        flags: CellFlags,
        last_byte: &mut Option<u8>,
    ) -> usize {
        if ascii.is_empty() {
            return 0;
        }

        // Pre-compute packed colors once for all cells
        let colors = Cell::convert_colors(fg, bg);

        let mut written = 0;
        let mut remaining = ascii;

        while !remaining.is_empty() {
            let cursor_row = self.cursor.row;
            let cursor_col = self.cursor.col;
            let effective_cols = self.effective_cols_for_row(cursor_row);
            let max_col = effective_cols.saturating_sub(1);

            // How many chars can we write on this line?
            let available = (effective_cols.saturating_sub(cursor_col)) as usize;
            let to_write = remaining.len().min(available);

            if to_write == 0 {
                // At end of line, need to wrap
                self.cursor.col = 0;
                if self.cursor.row < self.visible_rows - 1 {
                    self.cursor.row += 1;
                    let new_row = self.cursor.row;
                    if let Some(row) = self.row_mut(new_row) {
                        row.set_wrapped(true);
                    }
                } else {
                    self.scroll_up(1);
                }
                continue;
            }

            // Write styled cells directly to row
            if let Some(row) = self.row_mut(cursor_row) {
                // Fix up any wide characters that would be orphaned by this write
                row.fixup_wide_chars_in_range(cursor_col, to_write as u16);

                let cells = row.cells_mut();
                let start = cursor_col as usize;

                for (i, &byte) in remaining[..to_write].iter().enumerate() {
                    let cell_idx = start + i;
                    if cell_idx < cells.len() {
                        cells[cell_idx] = Cell::from_ascii_styled(byte, colors, flags);
                    }
                }

                // Update row len to include written content
                row.update_len(cursor_col + to_write as u16);
            }

            // Mark damage for the row (mark_row is faster than per-cell)
            self.damage.mark_row(cursor_row);

            // Track last byte for REP
            if let Some(&last) = remaining[..to_write].last() {
                *last_byte = Some(last);
            }

            // Advance cursor - but don't go past last column
            // If we wrote to the last column, wrap immediately for next char
            let new_col = cursor_col + to_write as u16;
            if new_col > max_col {
                // Wrote to last column - wrap to next line
                self.cursor.col = 0;
                if self.cursor.row < self.visible_rows - 1 {
                    self.cursor.row += 1;
                    let new_row = self.cursor.row;
                    if let Some(row) = self.row_mut(new_row) {
                        row.set_wrapped(true);
                    }
                } else {
                    self.scroll_up(1);
                }
            } else {
                self.cursor.col = new_col;
            }

            written += to_write;
            remaining = &remaining[to_write..];
        }

        written
    }

    /// Write a wide (double-width) character with autowrap.
    ///
    /// Wide characters occupy 2 cells. If at the last column, wraps first.
    /// Returns the number of columns consumed (2 for wide, 0 if cannot fit).
    pub fn write_wide_char_wrap_styled(
        &mut self,
        c: char,
        fg: PackedColor,
        bg: PackedColor,
        flags: CellFlags,
    ) -> u16 {
        // If we're at the last column, we need to wrap first
        // (wide char can't start at last column)
        let effective_cols = self.effective_cols_for_row(self.cursor.row);
        if self.cursor.col + 1 >= effective_cols {
            // Wrap to next line
            self.cursor.col = 0;
            if self.cursor.row < self.visible_rows - 1 {
                self.cursor.row += 1;
                let new_row = self.cursor.row;
                if let Some(row) = self.row_mut(new_row) {
                    row.set_wrapped(true);
                }
            } else {
                self.scroll_up(1);
            }
        }

        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);

        // Write the wide character (main cell + continuation)
        let written = if cursor_col + 1 < effective_cols {
            if let Some(row) = self.row_mut(cursor_row) {
                row.write_wide_char(cursor_col, c, fg, bg, flags)
            } else {
                0
            }
        } else {
            0
        };

        if written == 2 {
            self.damage.mark_cell(cursor_row, cursor_col);
            self.damage.mark_cell(cursor_row, cursor_col + 1);

            // Advance cursor by 2 (but handle wrap)
            use std::cmp::Ordering;
            match (self.cursor.col.saturating_add(2)).cmp(&effective_cols) {
                Ordering::Less => {
                    self.cursor.col += 2;
                }
                Ordering::Equal => {
                    // Exactly at end - next char will wrap
                    self.cursor.col = effective_cols.saturating_sub(1);
                }
                Ordering::Greater => {
                    // Need to wrap
                    self.cursor.col = 0;
                    if self.cursor.row < self.visible_rows - 1 {
                        self.cursor.row += 1;
                        let new_row = self.cursor.row;
                        if let Some(row) = self.row_mut(new_row) {
                            row.set_wrapped(true);
                        }
                    } else {
                        self.scroll_up(1);
                    }
                }
            }
        }

        written
    }

    /// Write a wide character without autowrap.
    ///
    /// Wide characters occupy 2 cells. If not enough room, does nothing.
    /// Returns the number of columns consumed (2 for wide, 0 if cannot fit).
    pub fn write_wide_char_styled(
        &mut self,
        c: char,
        fg: PackedColor,
        bg: PackedColor,
        flags: CellFlags,
    ) -> u16 {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);

        // Write the wide character
        let written = if cursor_col + 1 < effective_cols {
            if let Some(row) = self.row_mut(cursor_row) {
                row.write_wide_char(cursor_col, c, fg, bg, flags)
            } else {
                0
            }
        } else {
            0
        };

        if written == 2 {
            self.damage.mark_cell(cursor_row, cursor_col);
            self.damage.mark_cell(cursor_row, cursor_col + 1);

            // Advance cursor by 2, but don't exceed bounds
            self.cursor.col = self
                .cursor
                .col
                .saturating_add(2)
                .min(effective_cols.saturating_sub(1));
        }

        written
    }

    /// Write a wide character with autowrap using a style ID.
    ///
    /// This is the style-interning-aware version of `write_wide_char_wrap_styled`.
    /// It looks up the style from the StyleTable and writes the cell with
    /// the resolved colors and flags.
    ///
    /// # Arguments
    /// * `c` - Character to write (should be a wide character)
    /// * `style_id` - ID of the style in the StyleTable
    /// * `extra_flags` - Additional cell flags to merge with style flags
    ///
    /// # Returns
    /// Number of columns consumed (2 for wide, 0 if cannot fit).
    pub fn write_wide_char_wrap_with_style_id(
        &mut self,
        c: char,
        style_id: StyleId,
        extra_flags: CellFlags,
    ) -> u16 {
        let (fg, bg, flags) = self.resolve_style_to_colors(style_id, extra_flags);
        self.write_wide_char_wrap_styled(c, fg, bg, flags)
    }

    /// Write a wide character without autowrap using a style ID.
    ///
    /// This is the style-interning-aware version of `write_wide_char_styled`.
    /// It looks up the style from the StyleTable and writes the cell with
    /// the resolved colors and flags.
    ///
    /// # Arguments
    /// * `c` - Character to write (should be a wide character)
    /// * `style_id` - ID of the style in the StyleTable
    /// * `extra_flags` - Additional cell flags to merge with style flags
    ///
    /// # Returns
    /// Number of columns consumed (2 for wide, 0 if cannot fit).
    pub fn write_wide_char_with_style_id(
        &mut self,
        c: char,
        style_id: StyleId,
        extra_flags: CellFlags,
    ) -> u16 {
        let (fg, bg, flags) = self.resolve_style_to_colors(style_id, extra_flags);
        self.write_wide_char_styled(c, fg, bg, flags)
    }

    /// Set a cell at the given position.
    pub fn set_cell(&mut self, row: u16, col: u16, cell: Cell) {
        if let Some(r) = self.row_mut(row) {
            r.set(col, cell);
            self.damage.mark_cell(row, col);
        }
    }

    /// Mark a cell as complex and store the character string in overflow.
    ///
    /// This is used for non-BMP characters (emoji, etc.) that cannot fit
    /// in the 16-bit char_data field of the packed Cell.
    ///
    /// The cell at (row, col) should already have been written. This method
    /// sets the COMPLEX flag and stores the string in CellExtras.
    pub fn set_cell_complex_char(&mut self, row: u16, col: u16, s: &str) {
        use std::sync::Arc;

        if let Some(r) = self.row_mut(row) {
            if let Some(cell) = r.get_mut(col) {
                // Set COMPLEX flag and clear char_data (we'll use overflow)
                let mut flags = cell.flags();
                flags.insert(CellFlags::COMPLEX);
                cell.set_flags(flags);
                // Set char_data to 0 (not used when COMPLEX is set; string is in overflow)
                cell.set_overflow_index(0);
            }
        }

        // Store the string in the overflow table
        let extra = self.extras.get_or_create(CellCoord::new(row, col));
        extra.set_complex_char(Some(Arc::from(s)));

        self.damage.mark_cell(row, col);
    }

    /// Erase from cursor to end of line.
    pub fn erase_to_end_of_line(&mut self) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);
        if cursor_col < effective_cols {
            if let Some(row) = self.row_mut(cursor_row) {
                row.clear_range(cursor_col, effective_cols);
            }
            self.extras
                .clear_range(cursor_row, cursor_col, effective_cols);
            self.damage.mark_row(cursor_row);
        }
    }

    /// Erase from start of line to cursor.
    pub fn erase_from_start_of_line(&mut self) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);
        let end = (cursor_col + 1).min(effective_cols);
        if end > 0 {
            if let Some(row) = self.row_mut(cursor_row) {
                row.clear_range(0, end);
            }
            self.extras.clear_range(cursor_row, 0, end);
            self.damage.mark_row(cursor_row);
        }
    }

    /// Erase entire line.
    pub fn erase_line(&mut self) {
        if let Some(row) = self.row_mut(self.cursor.row) {
            row.clear();
            self.extras.clear_row(self.cursor.row);
            self.damage.mark_row(self.cursor.row);
        }
    }

    /// Erase from cursor to end of screen.
    pub fn erase_to_end_of_screen(&mut self) {
        // Erase rest of current line
        self.erase_to_end_of_line();
        // Erase all rows below
        for row in (self.cursor.row + 1)..self.visible_rows {
            if let Some(r) = self.row_mut(row) {
                r.clear();
            }
            self.extras.clear_row(row);
        }
        self.damage.mark_full();
    }

    /// Erase from start of screen to cursor.
    pub fn erase_from_start_of_screen(&mut self) {
        // Erase all rows above
        for row in 0..self.cursor.row {
            if let Some(r) = self.row_mut(row) {
                r.clear();
            }
            self.extras.clear_row(row);
        }
        // Erase current line up to cursor
        self.erase_from_start_of_line();
        self.damage.mark_full();
    }

    /// Erase entire screen.
    pub fn erase_screen(&mut self) {
        for row in 0..self.visible_rows {
            if let Some(r) = self.row_mut(row) {
                r.clear();
            }
        }
        self.extras.clear();
        self.damage.mark_full();
    }

    /// Erase scrollback.
    pub fn erase_scrollback(&mut self) {
        let scrollback = self.total_lines.saturating_sub(self.visible_rows as usize);
        if scrollback == 0 {
            if let Some(scrollback) = self.scrollback.as_mut() {
                scrollback.clear();
            }
            self.display_offset = 0;
            self.damage.mark_full();
            return;
        }

        // Preserve the live (display_offset = 0) visible rows and drop scrollback.
        let live_top = (self.ring_head + scrollback) % self.rows.len();
        let mut new_pages = PageStore::new();
        let mut new_rows = Vec::with_capacity(self.visible_rows as usize);
        for i in 0..self.visible_rows {
            let idx = (live_top + i as usize) % self.rows.len();
            let mut row = Row::new(self.cols, &mut new_pages);
            row.copy_from(&self.rows[idx]);
            new_rows.push(row);
        }

        self.rows = new_rows;
        self.pages = new_pages;
        self.total_lines = self.visible_rows as usize;
        self.ring_head = 0;
        self.display_offset = 0;
        if let Some(scrollback) = self.scrollback.as_mut() {
            scrollback.clear();
        }
        self.generations.evict_all();
        // Note: extras are keyed by visible row, so we don't need to clear them here
        // as scrollback rows don't have extras (they're saved as Line objects)
        self.damage.mark_full();
    }

    // ========================================================================
    // Selective Erase (DECSED/DECSEL) - only erases unprotected cells
    // ========================================================================

    /// Selectively erase from cursor to end of line (DECSEL mode 0).
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    pub fn selective_erase_to_end_of_line(&mut self) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);
        if cursor_col < effective_cols {
            if let Some(row) = self.row_mut(cursor_row) {
                row.selective_clear_range(cursor_col, effective_cols);
            }
            self.damage.mark_row(cursor_row);
        }
    }

    /// Selectively erase from start of line to cursor (DECSEL mode 1).
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    pub fn selective_erase_from_start_of_line(&mut self) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);
        let end = (cursor_col + 1).min(effective_cols);
        if end > 0 {
            if let Some(row) = self.row_mut(cursor_row) {
                row.selective_clear_range(0, end);
            }
            self.damage.mark_row(cursor_row);
        }
    }

    /// Selectively erase entire line (DECSEL mode 2).
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    pub fn selective_erase_line(&mut self) {
        let cursor_row = self.cursor.row;
        if let Some(row) = self.row_mut(cursor_row) {
            row.selective_clear();
        }
        self.damage.mark_row(cursor_row);
    }

    /// Selectively erase from cursor to end of screen (DECSED mode 0).
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    pub fn selective_erase_to_end_of_screen(&mut self) {
        // Erase rest of current line
        self.selective_erase_to_end_of_line();
        // Erase all rows below
        for row in (self.cursor.row + 1)..self.visible_rows {
            if let Some(r) = self.row_mut(row) {
                r.selective_clear();
            }
        }
        self.damage.mark_full();
    }

    /// Selectively erase from start of screen to cursor (DECSED mode 1).
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    pub fn selective_erase_from_start_of_screen(&mut self) {
        // Erase all rows above
        for row in 0..self.cursor.row {
            if let Some(r) = self.row_mut(row) {
                r.selective_clear();
            }
        }
        // Erase current line up to cursor
        self.selective_erase_from_start_of_line();
        self.damage.mark_full();
    }

    /// Selectively erase entire screen (DECSED mode 2).
    ///
    /// Only erases cells that are NOT protected (DECSCA).
    pub fn selective_erase_screen(&mut self) {
        for row in 0..self.visible_rows {
            if let Some(r) = self.row_mut(row) {
                r.selective_clear();
            }
        }
        self.damage.mark_full();
    }

    /// Fill screen with 'E' for alignment test (DECALN - ESC # 8).
    ///
    /// This is used to test screen alignment by filling the entire screen
    /// with the character 'E'. It also resets scroll margins to full screen.
    pub fn screen_alignment_pattern(&mut self) {
        // Reset scroll region to full screen
        self.scroll_region = ScrollRegion::full(self.visible_rows);

        let cols = self.cols;

        // Fill all visible cells with 'E'
        for row in 0..self.visible_rows {
            if let Some(r) = self.row_mut(row) {
                for col in 0..cols {
                    r.write_char(col, 'E');
                }
            }
        }

        // Move cursor to home position
        self.cursor = Cursor::default();

        self.damage.mark_full();
    }

    /// Insert `count` blank characters at cursor position.
    ///
    /// Shifts existing characters right, discarding those that go past the edge.
    /// This implements the ICH (Insert Character) CSI sequence.
    pub fn insert_chars(&mut self, count: u16) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);
        if cursor_col < effective_cols {
            let count = count.min(effective_cols - cursor_col);
            if let Some(row) = self.row_mut(cursor_row) {
                row.insert_chars(cursor_col, count);
            }
            self.damage.mark_row(cursor_row);
        }
    }

    /// Delete `count` characters at cursor position.
    ///
    /// Shifts remaining characters left, filling the end with blanks.
    /// This implements the DCH (Delete Character) CSI sequence.
    pub fn delete_chars(&mut self, count: u16) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);
        if cursor_col < effective_cols {
            let count = count.min(effective_cols - cursor_col);
            if let Some(row) = self.row_mut(cursor_row) {
                row.delete_chars(cursor_col, count);
            }
            self.damage.mark_row(cursor_row);
        }
    }

    /// Erase `count` characters at cursor position without shifting.
    ///
    /// Replaces characters with blanks in place. Does not shift remaining characters.
    /// This implements the ECH (Erase Character) CSI sequence.
    pub fn erase_chars(&mut self, count: u16) {
        let cursor_row = self.cursor.row;
        let cursor_col = self.cursor.col;
        let effective_cols = self.effective_cols_for_row(cursor_row);
        if cursor_col < effective_cols {
            let count = count.min(effective_cols - cursor_col);
            if let Some(row) = self.row_mut(cursor_row) {
                row.erase_chars(cursor_col, count);
            }
            self.damage.mark_row(cursor_row);
        }
    }

    /// Insert `count` blank lines at cursor row.
    ///
    /// Lines below are shifted down within the scroll region, with lines
    /// at the bottom margin discarded. Per VT510, IL has no effect if
    /// cursor is outside the scroll region.
    ///
    /// This implements the IL (Insert Line) CSI sequence.
    pub fn insert_lines(&mut self, count: usize) {
        if count == 0 {
            return;
        }

        let cursor_row = self.cursor.row;
        let region = self.scroll_region;

        // Per VT510: IL has no effect if cursor is outside scroll region
        if cursor_row < region.top || cursor_row > region.bottom {
            return;
        }

        let start_row = usize::from(cursor_row);
        let end_row = usize::from(region.bottom) + 1; // Bottom margin (inclusive) + 1

        // Shift rows down within the scroll region
        // We work backwards to avoid overwriting
        for dst in (start_row + count..end_row).rev() {
            let src = dst - count;
            // Copy row content from src to dst
            let src_idx = self.row_index(row_u16(src));
            let dst_idx = self.row_index(row_u16(dst));
            if src_idx != dst_idx {
                self.copy_row_indexed(dst_idx, src_idx);
            }
        }

        // Clear the inserted rows
        let clear_end = (start_row + count).min(end_row);
        for row in start_row..clear_end {
            if let Some(r) = self.row_mut(row_u16(row)) {
                r.clear();
            }
        }

        self.damage.mark_full();
    }

    /// Delete `count` lines at cursor row.
    ///
    /// Lines below are shifted up within the scroll region, with blank lines
    /// inserted at the bottom margin. Per VT510, DL has no effect if cursor
    /// is outside the scroll region.
    ///
    /// This implements the DL (Delete Line) CSI sequence.
    pub fn delete_lines(&mut self, count: usize) {
        if count == 0 {
            return;
        }

        let cursor_row = self.cursor.row;
        let region = self.scroll_region;

        // Per VT510: DL has no effect if cursor is outside scroll region
        if cursor_row < region.top || cursor_row > region.bottom {
            return;
        }

        let start_row = usize::from(cursor_row);
        let end_row = usize::from(region.bottom) + 1; // Bottom margin (inclusive) + 1

        // Shift rows up within the scroll region
        for dst in start_row..(end_row.saturating_sub(count)).max(start_row) {
            let src = dst + count;
            if src < end_row {
                let src_idx = self.row_index(row_u16(src));
                let dst_idx = self.row_index(row_u16(dst));
                if src_idx != dst_idx {
                    self.copy_row_indexed(dst_idx, src_idx);
                }
            }
        }

        // Clear the bottom rows of the scroll region
        let clear_start = end_row.saturating_sub(count).max(start_row);
        for row in clear_start..end_row {
            if let Some(r) = self.row_mut(row_u16(row)) {
                r.clear();
            }
        }

        self.damage.mark_full();
    }

    /// Resize the grid to new dimensions with line reflow.
    ///
    /// When width changes, soft-wrapped lines are reflowed:
    /// - Narrower: Long lines wrap onto new rows
    /// - Wider: Soft-wrapped lines unwrap (merge back)
    ///
    /// Cursor position is adjusted to follow its logical position in the content.
    pub fn resize(&mut self, new_rows: u16, new_cols: u16) {
        self.resize_with_reflow(new_rows, new_cols, true);
    }

    /// Resize the grid with optional reflow.
    ///
    /// If `reflow` is false, lines are simply truncated/extended without reflowing.
    pub fn resize_with_reflow(&mut self, new_rows: u16, new_cols: u16, reflow: bool) {
        let new_rows = new_rows.max(1);
        let new_cols = new_cols.max(1);
        let old_cols = self.cols;

        // Track cursor position
        let cursor_row = self.cursor.row as usize;
        let cursor_col = self.cursor.col;

        // If column count changes and reflow is enabled, do reflow
        if new_cols != old_cols && reflow {
            self.reflow_columns(new_cols, cursor_row, cursor_col);
        } else if new_cols != old_cols {
            // No reflow - just resize each row
            let mut new_pages = PageStore::new();
            for row in &mut self.rows {
                row.resize(new_cols, &mut new_pages);
            }
            self.pages = new_pages;
        }

        // After reflow, rows.len() may differ from new_rows due to wrapping/unwrapping.
        // Adjust to match the target row count before handling row count changes.
        let target_rows = new_rows as usize;
        while self.rows.len() > target_rows {
            // Trim excess rows from the end
            self.rows.pop();
        }
        // Sync total_lines with actual row count
        self.total_lines = self.rows.len();

        // Handle row count changes (based on how many rows we have after reflow/trim vs target)
        let current_rows = self.rows.len();
        if new_rows as usize > current_rows {
            // Growing - add empty rows to reach target
            let rows_to_add = new_rows as usize - current_rows;
            for _ in 0..rows_to_add {
                self.rows.push(Row::new(new_cols, &mut self.pages));
            }
            self.total_lines += rows_to_add;
        }
        // Note: shrinking is already handled by the trim above - no need for shrink_rows

        // Handle tab stop changes
        if new_cols != old_cols {
            let old_len = self.tab_stops.len();
            self.tab_stops.resize(new_cols as usize, false);
            for col in old_len..self.tab_stops.len() {
                self.tab_stops[col] = col > 0 && col % 8 == 0;
            }
        }

        // Update dimensions
        self.visible_rows = new_rows;
        self.cols = new_cols;

        // Clamp cursor
        self.cursor.row = self.cursor.row.min(new_rows.saturating_sub(1));
        self.cursor.col = self.clamp_col_for_row(self.cursor.row, self.cursor.col);

        // Clamp saved cursor
        if self.saved_cursor.valid {
            self.saved_cursor.cursor.row =
                self.saved_cursor.cursor.row.min(new_rows.saturating_sub(1));
            let saved_row = self.saved_cursor.cursor.row;
            self.saved_cursor.cursor.col =
                self.clamp_col_for_row(saved_row, self.saved_cursor.cursor.col);
        }

        // Adjust display offset
        let max_offset = self.scrollback_lines();
        self.display_offset = self.display_offset.min(max_offset);

        // Reset scroll region
        self.scroll_region = ScrollRegion::full(new_rows);

        // Mark full redraw needed
        self.damage = Damage::Full;
    }

    /// Reflow lines when column count changes.
    fn reflow_columns(&mut self, new_cols: u16, cursor_row: usize, cursor_col: u16) {
        let old_cols = self.cols;

        if new_cols > old_cols {
            // Growing wider - unwrap soft-wrapped lines
            self.reflow_grow_columns(new_cols, cursor_row, cursor_col);
        } else {
            // Shrinking narrower - wrap long lines
            self.reflow_shrink_columns(new_cols, cursor_row, cursor_col);
        }
    }

    /// Reflow when terminal gets wider: unwrap soft-wrapped lines.
    fn reflow_grow_columns(&mut self, new_cols: u16, cursor_row: usize, cursor_col: u16) {
        let mut new_pages = PageStore::new();
        let mut new_rows: Vec<Row> = Vec::new();
        let mut cursor_new_row = cursor_row;
        let mut cursor_new_col = cursor_col;
        let mut _lines_removed = 0usize;

        // First, collect all rows in display order
        let visible_count = usize::from(self.visible_rows);
        let old_rows: Vec<(Vec<Cell>, bool)> = (0..visible_count)
            .filter_map(|i| {
                self.row(row_u16(i))
                    .map(|r| (r.extract_cells(), r.is_wrapped()))
            })
            .collect();

        let mut i = 0;
        while i < old_rows.len() {
            // Start a new logical line
            let (ref first_cells, first_wrapped) = old_rows[i];
            let mut cells: Vec<Cell> = first_cells.clone();
            let first_row_idx = i;

            // Collect all continuation rows (rows that are wrapped from this one)
            while i + 1 < old_rows.len() && old_rows[i + 1].1 {
                i += 1;
                // Track if cursor is in one of the continuation rows
                if i == cursor_row {
                    // Cursor's logical position is its position in the merged line
                    let offset = cells.len();
                    cursor_new_col = cursor_col + row_u16(offset);
                }
                cells.extend(old_rows[i].0.clone());
            }

            // Now split these cells across new rows of width new_cols
            let mut cell_offset = 0;
            let mut first_new_row_for_logical = true;

            while cell_offset < cells.len() {
                let chunk_end = (cell_offset + usize::from(new_cols)).min(cells.len());
                let mut new_row = Row::new(new_cols, &mut new_pages);

                // Handle wide char at boundary
                let mut actual_end = chunk_end;
                if actual_end < cells.len() {
                    // Check if we're splitting a wide character
                    if actual_end > cell_offset {
                        let last_cell = &cells[actual_end - 1];
                        if last_cell.flags().contains(CellFlags::WIDE) {
                            // Wide char would be split - don't include it in this row
                            actual_end -= 1;
                        }
                    }
                }

                // Copy cells to new row
                for (j, cell) in cells[cell_offset..actual_end].iter().enumerate() {
                    new_row.set(row_u16(j), *cell);
                }

                // Set wrapped flag for continuation rows
                if !first_new_row_for_logical {
                    new_row.set_wrapped(true);
                }

                // Track cursor position
                if cursor_row >= first_row_idx && cursor_row <= i {
                    let cursor_logical_offset = if cursor_row == first_row_idx {
                        usize::from(cursor_col)
                    } else {
                        // Calculate offset in the logical line
                        let off: usize = old_rows[first_row_idx..cursor_row]
                            .iter()
                            .map(|(cells, _)| cells.len())
                            .sum();
                        off + usize::from(cursor_col)
                    };

                    if cursor_logical_offset >= cell_offset && cursor_logical_offset < actual_end {
                        cursor_new_row = new_rows.len();
                        cursor_new_col = row_u16(cursor_logical_offset - cell_offset);
                    }
                }

                new_rows.push(new_row);
                cell_offset = actual_end;
                first_new_row_for_logical = false;
            }

            // If no cells (empty line), still add one row
            if cells.is_empty() {
                let mut new_row = Row::new(new_cols, &mut new_pages);
                // Preserve wrapped flag for empty wrapped rows
                if first_wrapped {
                    new_row.set_wrapped(true);
                }
                if cursor_row == first_row_idx {
                    cursor_new_row = new_rows.len();
                    cursor_new_col = cursor_col.min(new_cols.saturating_sub(1));
                }
                new_rows.push(new_row);
            }

            if i >= first_row_idx && i > first_row_idx {
                _lines_removed += i - first_row_idx;
            }

            i += 1;
        }

        // Ensure we have exactly visible_rows rows
        // If we created fewer rows (after unwrapping), pad with empty rows
        let target_rows = usize::from(self.visible_rows);
        while new_rows.len() < target_rows {
            new_rows.push(Row::new(new_cols, &mut new_pages));
        }
        // If we created more rows (shouldn't happen with grow), truncate
        while new_rows.len() > target_rows {
            new_rows.pop();
        }

        self.rows = new_rows;
        self.pages = new_pages;
        self.ring_head = 0; // Reset ring buffer since we rebuilt rows
        self.total_lines = self.rows.len();
        self.cursor.row = row_u16(cursor_new_row).min(row_u16(self.rows.len().saturating_sub(1)));
        self.cursor.col = cursor_new_col.min(new_cols.saturating_sub(1));
    }

    /// Reflow when terminal gets narrower: wrap long lines.
    fn reflow_shrink_columns(&mut self, new_cols: u16, cursor_row: usize, cursor_col: u16) {
        let mut new_pages = PageStore::new();
        let mut new_rows: Vec<Row> = Vec::new();
        let mut cursor_new_row = 0;
        let mut cursor_new_col = cursor_col;
        let mut _lines_added = 0usize;

        // First, collect all rows in display order
        let visible_count = usize::from(self.visible_rows);
        let old_rows: Vec<(Vec<Cell>, bool)> = (0..visible_count)
            .filter_map(|i| {
                self.row(row_u16(i))
                    .map(|r| (r.extract_cells(), r.is_wrapped()))
            })
            .collect();

        for (i, (cells, was_wrapped)) in old_rows.iter().enumerate() {
            if cells.is_empty() {
                // Empty row - just create a new empty row
                let mut new_row = Row::new(new_cols, &mut new_pages);
                if *was_wrapped {
                    new_row.set_wrapped(true);
                }
                if i == cursor_row {
                    cursor_new_row = new_rows.len();
                    cursor_new_col = cursor_col.min(new_cols.saturating_sub(1));
                }
                new_rows.push(new_row);
                continue;
            }

            // Split cells across multiple rows
            let mut cell_offset = 0;
            let mut first_row_for_this_line = true;

            while cell_offset < cells.len() {
                let chunk_end = (cell_offset + usize::from(new_cols)).min(cells.len());

                // Handle wide char at boundary
                let mut actual_end = chunk_end;
                if actual_end < cells.len() && actual_end > cell_offset {
                    let last_cell = &cells[actual_end - 1];
                    if last_cell.flags().contains(CellFlags::WIDE) {
                        actual_end -= 1;
                    }
                }

                let mut new_row = Row::new(new_cols, &mut new_pages);

                // Copy cells
                for (j, cell) in cells[cell_offset..actual_end].iter().enumerate() {
                    new_row.set(row_u16(j), *cell);
                }

                // Set wrapped flag appropriately
                if first_row_for_this_line {
                    if *was_wrapped {
                        new_row.set_wrapped(true);
                    }
                } else {
                    // This is a continuation from wrapping
                    new_row.set_wrapped(true);
                    _lines_added += 1;
                }

                // Track cursor
                if i == cursor_row {
                    let cursor_offset = usize::from(cursor_col);
                    if cursor_offset >= cell_offset && cursor_offset < actual_end {
                        cursor_new_row = new_rows.len();
                        cursor_new_col = row_u16(cursor_offset - cell_offset);
                    } else if cursor_offset >= actual_end
                        && cell_offset + usize::from(new_cols) >= cells.len()
                    {
                        // Cursor was past the wrapped content - put at end of last row
                        cursor_new_row = new_rows.len();
                        cursor_new_col = row_u16(actual_end - cell_offset);
                        cursor_new_col = cursor_new_col.min(new_cols.saturating_sub(1));
                    }
                }

                new_rows.push(new_row);
                cell_offset = actual_end;
                first_row_for_this_line = false;
            }
        }

        // Ensure we have exactly visible_rows rows
        // If we created more rows than visible_rows, truncate from bottom (empty rows)
        let target_rows = usize::from(self.visible_rows);
        while new_rows.len() > target_rows {
            new_rows.pop(); // Remove from bottom (excess empty rows)
        }
        // If we created fewer rows than visible_rows, pad with empty rows
        while new_rows.len() < target_rows {
            new_rows.push(Row::new(new_cols, &mut new_pages));
        }

        self.rows = new_rows;
        self.pages = new_pages;
        self.ring_head = 0; // Reset ring buffer since we rebuilt rows
        self.total_lines = self.rows.len();
        self.cursor.row = row_u16(cursor_new_row).min(row_u16(self.rows.len().saturating_sub(1)));
        self.cursor.col = cursor_new_col.min(new_cols.saturating_sub(1));
    }

    /// Clear damage after rendering.
    pub fn clear_damage(&mut self) {
        self.damage.reset(self.visible_rows);
    }

    /// Check if the grid needs a full redraw.
    #[must_use]
    pub fn needs_full_redraw(&self) -> bool {
        self.damage.is_full()
    }

    /// Get visible row content as a string (for debugging).
    ///
    /// This properly resolves complex characters (non-BMP, grapheme clusters)
    /// from the overflow table.
    #[must_use]
    pub fn visible_content(&self) -> String {
        let mut s = String::new();
        for row in 0..self.visible_rows {
            if let Some(text) = self.row_text(row) {
                s.push_str(&text);
            }
            s.push('\n');
        }
        s
    }

    /// Get a historical line by index (0 = oldest).
    ///
    /// This method provides unified access to all scrollback history:
    /// - First, lines from the tiered scrollback (if any)
    /// - Then, lines from the ring buffer scrollback
    ///
    /// Returns None if the index is out of bounds.
    #[must_use]
    pub fn get_history_line(&self, idx: usize) -> Option<Line> {
        let tiered_count = self.tiered_scrollback_lines();
        let ring_count = self.ring_buffer_scrollback();

        if idx >= tiered_count + ring_count {
            return None;
        }

        if idx < tiered_count {
            // Line is in tiered scrollback
            self.scrollback.as_ref()?.get_line(idx)
        } else {
            // Line is in ring buffer scrollback
            let ring_idx = idx - tiered_count;
            let row_idx = (self.ring_head + ring_idx) % self.rows.len();
            Some(self.row_to_line(&self.rows[row_idx]))
        }
    }

    /// Get a historical line by reverse index (0 = most recent scrollback line).
    ///
    /// This is useful for displaying scrollback from bottom to top.
    #[must_use]
    pub fn get_history_line_rev(&self, rev_idx: usize) -> Option<Line> {
        let total = self.scrollback_lines();
        if rev_idx >= total {
            return None;
        }
        self.get_history_line(total - 1 - rev_idx)
    }

    /// Get total history line count (tiered + ring buffer scrollback).
    #[must_use]
    #[inline]
    pub fn history_line_count(&self) -> usize {
        self.scrollback_lines()
    }

    /// Assert TLA+ specification invariants in debug builds.
    ///
    /// This method validates key invariants from the Terminal.tla specification:
    /// - Cursor position is within bounds
    /// - Wide characters have proper continuations
    /// - Wide characters are not at the last column
    /// - Scroll region is valid
    ///
    /// # Panics
    ///
    /// Panics in debug builds if any invariant is violated.
    /// Does nothing in release builds for performance.
    #[inline]
    pub fn assert_invariants(&self) {
        #[cfg(debug_assertions)]
        {
            // Invariant: CursorInBounds
            // cursor.row < visible_rows && cursor.col < cols
            assert!(
                self.cursor.row < self.visible_rows,
                "TLA+ CursorInBounds violated: cursor row {} >= visible_rows {}",
                self.cursor.row,
                self.visible_rows
            );
            assert!(
                self.cursor.col < self.cols,
                "TLA+ CursorInBounds violated: cursor col {} >= cols {}",
                self.cursor.col,
                self.cols
            );

            // Invariant: WideCharConsistent
            // Every WIDE cell at (row, col) must have WIDE_CONTINUATION at (row, col+1)
            for row_idx in 0..self.visible_rows {
                if let Some(row) = self.row(row_idx) {
                    for col in 0..self.cols.saturating_sub(1) {
                        if let Some(cell) = row.get(col) {
                            if cell.is_wide() {
                                if let Some(next_cell) = row.get(col + 1) {
                                    assert!(
                                        next_cell.is_wide_continuation(),
                                        "TLA+ WideCharConsistent violated: wide char at ({}, {}) missing continuation at ({}, {})",
                                        row_idx, col, row_idx, col + 1
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // Invariant: WideCharNotAtEnd
            // No WIDE cell at the last column
            for row_idx in 0..self.visible_rows {
                if let Some(row) = self.row(row_idx) {
                    let last_col = self.cols.saturating_sub(1);
                    if let Some(cell) = row.get(last_col) {
                        assert!(
                            !cell.is_wide(),
                            "TLA+ WideCharNotAtEnd violated: wide char at ({}, {}) which is last column",
                            row_idx, last_col
                        );
                    }
                }
            }

            // Invariant: ScrollRegionValid
            // scroll_region.top < scroll_region.bottom <= visible_rows
            assert!(
                self.scroll_region.top < self.scroll_region.bottom,
                "TLA+ ScrollRegionValid violated: top {} >= bottom {}",
                self.scroll_region.top,
                self.scroll_region.bottom
            );
            assert!(
                self.scroll_region.bottom <= self.visible_rows,
                "TLA+ ScrollRegionValid violated: bottom {} > visible_rows {}",
                self.scroll_region.bottom,
                self.visible_rows
            );

            // Invariant: DisplayOffsetValid
            // display_offset <= scrollback_lines
            let max_offset = self.scrollback_lines();
            assert!(
                self.display_offset <= max_offset,
                "TLA+ DisplayOffsetValid violated: display_offset {} > scrollback_lines {}",
                self.display_offset,
                max_offset
            );
        }
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new(24, 80)
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)] // Test code uses bounded loop indices
mod tests {
    use super::*;

    #[test]
    fn grid_new() {
        let grid = Grid::new(24, 80);
        assert_eq!(grid.rows(), 24);
        assert_eq!(grid.cols(), 80);
        assert_eq!(grid.cursor_row(), 0);
        assert_eq!(grid.cursor_col(), 0);
    }

    #[test]
    fn grid_assert_invariants_on_new() {
        let grid = Grid::new(24, 80);
        grid.assert_invariants();
    }

    #[test]
    fn grid_assert_invariants_after_operations() {
        let mut grid = Grid::new(24, 80);

        // Write some text
        for c in "Hello, World!".chars() {
            grid.write_char(c);
        }
        grid.assert_invariants();

        // Move cursor
        grid.move_cursor_to(10, 40);
        grid.assert_invariants();

        // Scroll
        grid.scroll_up(5);
        grid.assert_invariants();

        // Resize
        grid.resize(30, 100);
        grid.assert_invariants();
    }

    #[test]
    fn grid_cursor_bounds() {
        let mut grid = Grid::new(24, 80);
        grid.set_cursor(100, 200);
        assert_eq!(grid.cursor_row(), 23);
        assert_eq!(grid.cursor_col(), 79);
    }

    #[test]
    fn grid_cursor_movement() {
        let mut grid = Grid::new(24, 80);

        grid.move_cursor_to(10, 20);
        assert_eq!(grid.cursor(), Cursor::new(10, 20));

        grid.move_cursor_by(5, -10);
        assert_eq!(grid.cursor(), Cursor::new(15, 10));

        grid.move_cursor_by(-100, -100);
        assert_eq!(grid.cursor(), Cursor::new(0, 0));
    }

    #[test]
    fn grid_cursor_up_within_scroll_region() {
        let mut grid = Grid::new(10, 80);
        // Set scroll region: rows 3-7
        grid.set_scroll_region(3, 7);
        // Cursor at row 5 (within region)
        grid.set_cursor(5, 10);
        // Move up 10 - should stop at top margin (row 3)
        grid.cursor_up(10);
        assert_eq!(grid.cursor_row(), 3);
    }

    #[test]
    fn grid_cursor_up_outside_scroll_region() {
        let mut grid = Grid::new(10, 80);
        // Set scroll region: rows 3-7
        grid.set_scroll_region(3, 7);
        // Cursor at row 1 (above region)
        grid.set_cursor(1, 10);
        // Move up 10 - should stop at row 0
        grid.cursor_up(10);
        assert_eq!(grid.cursor_row(), 0);
    }

    #[test]
    fn grid_cursor_down_within_scroll_region() {
        let mut grid = Grid::new(10, 80);
        // Set scroll region: rows 2-6
        grid.set_scroll_region(2, 6);
        // Cursor at row 4 (within region)
        grid.set_cursor(4, 10);
        // Move down 10 - should stop at bottom margin (row 6)
        grid.cursor_down(10);
        assert_eq!(grid.cursor_row(), 6);
    }

    #[test]
    fn grid_cursor_down_outside_scroll_region() {
        let mut grid = Grid::new(10, 80);
        // Set scroll region: rows 2-5
        grid.set_scroll_region(2, 5);
        // Cursor at row 7 (below region)
        grid.set_cursor(7, 10);
        // Move down 10 - should stop at row 9 (last line)
        grid.cursor_down(10);
        assert_eq!(grid.cursor_row(), 9);
    }

    #[test]
    fn grid_cursor_forward_stops_at_edge() {
        let mut grid = Grid::new(10, 80);
        grid.set_cursor(5, 70);
        grid.cursor_forward(20);
        assert_eq!(grid.cursor_col(), 79);
    }

    #[test]
    fn grid_cursor_backward_stops_at_zero() {
        let mut grid = Grid::new(10, 80);
        grid.set_cursor(5, 10);
        grid.cursor_backward(20);
        assert_eq!(grid.cursor_col(), 0);
    }

    #[test]
    fn grid_cursor_movement_exact_amount() {
        let mut grid = Grid::new(10, 80);
        grid.set_cursor(5, 40);

        grid.cursor_up(3);
        assert_eq!(grid.cursor_row(), 2);

        grid.cursor_down(5);
        assert_eq!(grid.cursor_row(), 7);

        grid.cursor_forward(10);
        assert_eq!(grid.cursor_col(), 50);

        grid.cursor_backward(5);
        assert_eq!(grid.cursor_col(), 45);
    }

    #[test]
    fn grid_write_char() {
        let mut grid = Grid::new(24, 80);
        grid.write_char('H');
        grid.write_char('i');

        assert_eq!(grid.cell(0, 0).unwrap().char(), 'H');
        assert_eq!(grid.cell(0, 1).unwrap().char(), 'i');
        assert_eq!(grid.cursor_col(), 2);
    }

    #[test]
    fn grid_write_char_wrap() {
        let mut grid = Grid::new(24, 5);
        for c in "Hello World".chars() {
            grid.write_char_wrap(c);
        }

        // "Hello" on row 0, " Worl" on row 1, "d" on row 2
        assert_eq!(grid.row(0).unwrap().to_string(), "Hello");
        assert!(grid.row(1).unwrap().is_wrapped());
    }

    #[test]
    fn grid_line_feed() {
        let mut grid = Grid::new(24, 80);
        grid.set_cursor(5, 10);
        grid.line_feed();
        assert_eq!(grid.cursor_row(), 6);
        assert_eq!(grid.cursor_col(), 10);
    }

    #[test]
    fn grid_scroll_up() {
        let mut grid = Grid::new(3, 80);
        grid.write_char('A');
        grid.line_feed();
        grid.write_char('B');
        grid.line_feed();
        grid.write_char('C');

        // Now at bottom, scroll
        grid.line_feed();
        grid.write_char('D');

        // Row 0 should now have 'B', row 1 'C', row 2 'D'
        assert_eq!(grid.scrollback_lines(), 1);
    }

    #[test]
    fn grid_resize() {
        let mut grid = Grid::new(24, 80);
        grid.set_cursor(20, 70);
        grid.resize(10, 40);

        assert_eq!(grid.rows(), 10);
        assert_eq!(grid.cols(), 40);
        assert_eq!(grid.cursor_row(), 9);
        assert_eq!(grid.cursor_col(), 39);
    }

    #[test]
    fn grid_save_restore_cursor() {
        let mut grid = Grid::new(24, 80);
        grid.set_cursor(10, 20);
        grid.save_cursor();

        grid.set_cursor(0, 0);
        assert_eq!(grid.cursor(), Cursor::new(0, 0));

        grid.restore_cursor();
        assert_eq!(grid.cursor(), Cursor::new(10, 20));
    }

    #[test]
    fn grid_erase_line() {
        let mut grid = Grid::new(24, 80);
        for c in "Hello".chars() {
            grid.write_char(c);
        }
        grid.erase_line();
        assert!(grid.row(0).unwrap().is_empty());
    }

    #[test]
    fn grid_scroll_display() {
        let mut grid = Grid::with_scrollback(3, 80, 100);

        // Fill some content
        for i in 0..10 {
            grid.write_char((b'A' + i) as char);
            grid.line_feed();
        }

        assert!(grid.scrollback_lines() > 0);

        grid.scroll_display(2);
        assert_eq!(grid.display_offset(), 2);

        grid.scroll_to_bottom();
        assert_eq!(grid.display_offset(), 0);
    }

    #[test]
    fn grid_erase_scrollback_preserves_live_rows() {
        let scrollback = Scrollback::new(100, 1000, 10_000_000);
        let mut grid = Grid::with_tiered_scrollback(3, 4, 2, scrollback);

        for i in 0..8 {
            grid.carriage_return();
            for c in format!("L{i}").chars() {
                grid.write_char(c);
            }
            if i < 7 {
                grid.line_feed();
            }
        }

        assert!(grid.scrollback_lines() > 0);
        assert!(grid.tiered_scrollback_lines() > 0);

        let live_rows: Vec<String> = (0..grid.rows())
            .map(|row| grid.row(row).unwrap().to_string())
            .collect();

        grid.scroll_display(1);
        assert!(grid.display_offset() > 0);

        grid.erase_scrollback();

        assert_eq!(grid.scrollback_lines(), 0);
        assert_eq!(grid.tiered_scrollback_lines(), 0);
        assert_eq!(grid.display_offset(), 0);
        assert_eq!(grid.total_lines(), grid.rows() as usize);

        for (row_idx, expected) in live_rows.iter().enumerate() {
            assert_eq!(grid.row(row_idx as u16).unwrap().to_string(), *expected);
        }
    }

    #[test]
    fn grid_insert_chars() {
        let mut grid = Grid::new(24, 10);
        for c in "ABCDEFGHIJ".chars() {
            grid.write_char(c);
        }
        grid.set_cursor(0, 3); // Position at 'D'
        grid.insert_chars(2);

        // Check that cells shifted: "ABC  DEFGH" (IJ pushed off)
        assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
        assert_eq!(grid.cell(0, 2).unwrap().char(), 'C');
        assert_eq!(grid.cell(0, 3).unwrap().char(), ' ');
        assert_eq!(grid.cell(0, 4).unwrap().char(), ' ');
        assert_eq!(grid.cell(0, 5).unwrap().char(), 'D');
    }

    #[test]
    fn grid_delete_chars() {
        let mut grid = Grid::new(24, 10);
        for c in "ABCDEFGHIJ".chars() {
            grid.write_char(c);
        }
        grid.set_cursor(0, 3); // Position at 'D'
        grid.delete_chars(2);

        // Check that cells shifted: "ABCFGHIJ  " (DE deleted)
        assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
        assert_eq!(grid.cell(0, 2).unwrap().char(), 'C');
        assert_eq!(grid.cell(0, 3).unwrap().char(), 'F');
        assert_eq!(grid.cell(0, 7).unwrap().char(), 'J');
        assert_eq!(grid.cell(0, 8).unwrap().char(), ' ');
    }

    #[test]
    fn grid_insert_lines() {
        let mut grid = Grid::new(5, 10);
        // Write content to each row
        for row in 0..5 {
            grid.set_cursor(row, 0);
            grid.write_char((b'A' + row as u8) as char);
        }
        // Row 0: A, Row 1: B, Row 2: C, Row 3: D, Row 4: E

        grid.set_cursor(1, 0); // At row 1
        grid.insert_lines(2);

        // Row 0: A (unchanged)
        // Row 1: (blank - inserted)
        // Row 2: (blank - inserted)
        // Row 3: B (shifted from row 1)
        // Row 4: C (shifted from row 2)
        // D and E pushed off

        assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
        assert_eq!(grid.cell(1, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(2, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(3, 0).unwrap().char(), 'B');
        assert_eq!(grid.cell(4, 0).unwrap().char(), 'C');
    }

    #[test]
    fn grid_delete_lines() {
        let mut grid = Grid::new(5, 10);
        // Write content to each row
        for row in 0..5 {
            grid.set_cursor(row, 0);
            grid.write_char((b'A' + row as u8) as char);
        }
        // Row 0: A, Row 1: B, Row 2: C, Row 3: D, Row 4: E

        grid.set_cursor(1, 0); // At row 1
        grid.delete_lines(2);

        // Row 0: A (unchanged)
        // Row 1: D (shifted from row 3)
        // Row 2: E (shifted from row 4)
        // Row 3: (blank)
        // Row 4: (blank)

        assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
        assert_eq!(grid.cell(1, 0).unwrap().char(), 'D');
        assert_eq!(grid.cell(2, 0).unwrap().char(), 'E');
        assert_eq!(grid.cell(3, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(4, 0).unwrap().char(), ' ');
    }

    #[test]
    fn grid_with_tiered_scrollback() {
        let scrollback = Scrollback::new(100, 1000, 10_000_000);
        let mut grid = Grid::with_tiered_scrollback(3, 80, 5, scrollback);

        assert!(grid.scrollback().is_some());
        assert_eq!(grid.tiered_scrollback_lines(), 0);

        // Fill content to trigger scrollback
        for i in 0..20 {
            for c in format!("Line {i}").chars() {
                grid.write_char(c);
            }
            grid.line_feed();
        }

        // Some lines should be in tiered scrollback now
        assert!(grid.tiered_scrollback_lines() > 0);

        // Total scrollback should include both ring buffer and tiered
        assert!(grid.scrollback_lines() > grid.ring_buffer_scrollback());
    }

    #[test]
    fn grid_scrollback_content_preserved() {
        let scrollback = Scrollback::new(100, 1000, 10_000_000);
        // Small ring buffer of 2 lines to force early promotion
        let mut grid = Grid::with_tiered_scrollback(3, 80, 2, scrollback);

        // Write 10 lines
        for i in 0..10 {
            for c in format!("Line {i}").chars() {
                grid.write_char(c);
            }
            grid.line_feed();
        }

        // Check that content is preserved in tiered scrollback
        let sb = grid.scrollback().unwrap();
        assert!(sb.line_count() > 0);

        // First line in scrollback should be "Line 0" or close to it
        if let Some(line) = sb.get_line(0) {
            let text = line.to_string();
            assert!(
                text.starts_with("Line "),
                "Expected 'Line X', got '{}'",
                text
            );
        }
    }

    #[test]
    fn grid_attach_detach_scrollback() {
        let mut grid = Grid::new(24, 80);
        assert!(grid.scrollback().is_none());

        let scrollback = Scrollback::new(100, 1000, 10_000_000);
        grid.attach_scrollback(scrollback);
        assert!(grid.scrollback().is_some());

        let detached = grid.detach_scrollback();
        assert!(detached.is_some());
        assert!(grid.scrollback().is_none());
    }

    #[test]
    fn grid_scrollback_wrapped_lines() {
        let scrollback = Scrollback::new(100, 1000, 10_000_000);
        let mut grid = Grid::with_tiered_scrollback(3, 5, 2, scrollback);

        // Write a line that wraps
        for c in "HelloWorld".chars() {
            grid.write_char_wrap(c);
        }
        grid.line_feed();

        // Force more scrolling to push lines to tiered scrollback
        for _ in 0..10 {
            grid.line_feed();
        }

        // Check that wrapped flag is preserved
        let sb = grid.scrollback().unwrap();
        if sb.line_count() > 1 {
            // The second line should be marked as wrapped
            let line = sb.get_line(1).unwrap();
            assert!(
                line.is_wrapped(),
                "Wrapped line should preserve wrapped flag"
            );
        }
    }

    #[test]
    fn grid_default_tab_stops() {
        let grid = Grid::new(24, 80);
        // Default tab stops are at columns 8, 16, 24, 32, 40, 48, 56, 64, 72
        // Column 0 should not be a tab stop
        let expected_tabs = [8, 16, 24, 32, 40, 48, 56, 64, 72];
        for col in &expected_tabs {
            assert!(grid.tab_stops[*col], "Expected tab stop at column {}", col);
        }
        assert!(!grid.tab_stops[0], "Column 0 should not be a tab stop");
        assert!(!grid.tab_stops[1], "Column 1 should not be a tab stop");
    }

    #[test]
    fn grid_set_tab_stop() {
        let mut grid = Grid::new(24, 80);
        // Column 5 is not a default tab stop
        assert!(!grid.tab_stops[5]);

        grid.set_cursor(0, 5);
        grid.set_tab_stop();

        assert!(grid.tab_stops[5], "Tab stop should be set at column 5");
    }

    #[test]
    fn grid_clear_tab_stop() {
        let mut grid = Grid::new(24, 80);
        // Column 8 is a default tab stop
        assert!(grid.tab_stops[8]);

        grid.set_cursor(0, 8);
        grid.clear_tab_stop();

        assert!(!grid.tab_stops[8], "Tab stop should be cleared at column 8");
    }

    #[test]
    fn grid_clear_all_tab_stops() {
        let mut grid = Grid::new(24, 80);
        // Verify some default tab stops exist
        assert!(grid.tab_stops[8]);
        assert!(grid.tab_stops[16]);

        grid.clear_all_tab_stops();

        // All tab stops should be cleared
        for col in 0..80 {
            assert!(
                !grid.tab_stops[col],
                "Tab stop at column {} should be cleared",
                col
            );
        }
    }

    #[test]
    fn grid_reset_tab_stops() {
        let mut grid = Grid::new(24, 80);
        grid.clear_all_tab_stops();
        assert!(!grid.tab_stops[8]);

        grid.reset_tab_stops();

        // Default tab stops should be restored
        assert!(grid.tab_stops[8]);
        assert!(grid.tab_stops[16]);
    }

    #[test]
    fn grid_tab_uses_custom_stops() {
        let mut grid = Grid::new(24, 80);
        // Clear all and set custom tab stops
        grid.clear_all_tab_stops();
        grid.set_cursor(0, 5);
        grid.set_tab_stop();
        grid.set_cursor(0, 12);
        grid.set_tab_stop();

        // Tab from column 0 should go to column 5
        grid.set_cursor(0, 0);
        grid.tab();
        assert_eq!(grid.cursor_col(), 5);

        // Tab from column 5 should go to column 12
        grid.tab();
        assert_eq!(grid.cursor_col(), 12);

        // Tab from column 12 should go to last column (no more stops)
        grid.tab();
        assert_eq!(grid.cursor_col(), 79);
    }

    #[test]
    fn grid_back_tab_with_default_stops() {
        let mut grid = Grid::new(24, 80);
        // Start at column 20
        grid.set_cursor(0, 20);

        // Back tab should go to column 16 (previous default tab stop)
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 16);

        // Back tab should go to column 8
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 8);

        // Back tab should go to column 0 (no stop before 8)
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 0);

        // Back tab at column 0 should stay at 0
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 0);
    }

    #[test]
    fn grid_back_tab_with_custom_stops() {
        let mut grid = Grid::new(24, 80);
        // Clear all and set custom tab stops
        grid.clear_all_tab_stops();
        grid.set_cursor(0, 5);
        grid.set_tab_stop();
        grid.set_cursor(0, 12);
        grid.set_tab_stop();
        grid.set_cursor(0, 25);
        grid.set_tab_stop();

        // Start at column 30
        grid.set_cursor(0, 30);

        // Back tab should go to column 25
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 25);

        // Back tab should go to column 12
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 12);

        // Back tab should go to column 5
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 5);

        // Back tab should go to column 0 (no stop before 5)
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 0);
    }

    #[test]
    fn grid_back_tab_n() {
        let mut grid = Grid::new(24, 80);
        // Start at column 40
        grid.set_cursor(0, 40);

        // Back tab by 3 stops: 40 -> 32 -> 24 -> 16
        grid.back_tab_n(3);
        assert_eq!(grid.cursor_col(), 16);

        // Back tab by 10 stops (more than available): should go to column 0
        grid.back_tab_n(10);
        assert_eq!(grid.cursor_col(), 0);
    }

    #[test]
    fn grid_back_tab_between_stops() {
        let mut grid = Grid::new(24, 80);
        // Start at column 10 (between tab stops 8 and 16)
        grid.set_cursor(0, 10);

        // Back tab should go to column 8
        grid.back_tab();
        assert_eq!(grid.cursor_col(), 8);
    }

    #[test]
    fn grid_tab_n_with_default_stops() {
        let mut grid = Grid::new(24, 80);
        // Start at column 0
        grid.set_cursor(0, 0);

        // Tab forward by 3 stops: 0 -> 8 -> 16 -> 24
        grid.tab_n(3);
        assert_eq!(grid.cursor_col(), 24);

        // Tab forward by 1: 24 -> 32
        grid.tab_n(1);
        assert_eq!(grid.cursor_col(), 32);
    }

    #[test]
    fn grid_tab_n_past_last_stop() {
        let mut grid = Grid::new(24, 80);
        // Start at column 0
        grid.set_cursor(0, 0);

        // Tab forward by 20 stops (more than available): should go to last column (79)
        grid.tab_n(20);
        assert_eq!(grid.cursor_col(), 79);
    }

    #[test]
    fn grid_tab_n_with_custom_stops() {
        let mut grid = Grid::new(24, 80);
        // Clear all and set custom tab stops at columns 5, 15, 30
        grid.clear_all_tab_stops();
        grid.set_cursor(0, 5);
        grid.set_tab_stop();
        grid.set_cursor(0, 15);
        grid.set_tab_stop();
        grid.set_cursor(0, 30);
        grid.set_tab_stop();

        // Start at column 0
        grid.set_cursor(0, 0);

        // Tab forward by 2: 0 -> 5 -> 15
        grid.tab_n(2);
        assert_eq!(grid.cursor_col(), 15);

        // Tab forward by 1: 15 -> 30
        grid.tab_n(1);
        assert_eq!(grid.cursor_col(), 30);
    }

    #[test]
    fn grid_tab_n_from_between_stops() {
        let mut grid = Grid::new(24, 80);
        // Start at column 10 (between default tab stops 8 and 16)
        grid.set_cursor(0, 10);

        // Tab forward by 2: 10 -> 16 -> 24
        grid.tab_n(2);
        assert_eq!(grid.cursor_col(), 24);
    }

    #[test]
    fn grid_screen_alignment_pattern() {
        let mut grid = Grid::new(3, 5);
        // Set some content first
        grid.write_char('X');
        grid.line_feed();
        grid.write_char('Y');

        // Set a scroll region
        grid.set_scroll_region(1, 2);

        grid.screen_alignment_pattern();

        // All cells should be 'E'
        for row in 0..3 {
            for col in 0..5 {
                assert_eq!(
                    grid.cell(row, col).unwrap().char(),
                    'E',
                    "Cell ({}, {}) should be 'E'",
                    row,
                    col
                );
            }
        }

        // Cursor should be at home
        assert_eq!(grid.cursor_row(), 0);
        assert_eq!(grid.cursor_col(), 0);

        // Scroll region should be reset to full screen
        assert!(grid.scroll_region().is_full(3));
    }

    #[test]
    fn grid_resize_preserves_tab_stops() {
        let mut grid = Grid::new(24, 40);
        // Clear defaults and set custom tab stop at column 5
        grid.clear_all_tab_stops();
        grid.set_cursor(0, 5);
        grid.set_tab_stop();

        // Resize to larger width
        grid.resize(24, 80);

        // Custom tab stop should be preserved
        assert!(
            grid.tab_stops[5],
            "Custom tab stop at column 5 should be preserved"
        );
        // New default tab stops should be added for new columns
        assert!(
            grid.tab_stops[48],
            "Default tab stop at column 48 should be added"
        );
    }

    #[test]
    fn grid_is_tab_stop() {
        let mut grid = Grid::new(24, 80);

        // Default tab stops at columns 8, 16, 24, etc.
        assert!(!grid.is_tab_stop(0)); // Column 0 is never a tab stop
        assert!(!grid.is_tab_stop(1));
        assert!(grid.is_tab_stop(8));
        assert!(grid.is_tab_stop(16));
        assert!(!grid.is_tab_stop(10));

        // Clear all and set custom
        grid.clear_all_tab_stops();
        assert!(!grid.is_tab_stop(8));

        grid.set_cursor(0, 5);
        grid.set_tab_stop();
        assert!(grid.is_tab_stop(5));

        // Out of bounds returns false
        assert!(!grid.is_tab_stop(1000));
    }

    #[test]
    fn grid_tab_stop_positions() {
        let mut grid = Grid::new(24, 80);

        // Default tab stops: 8, 16, 24, 32, 40, 48, 56, 64, 72
        let positions: Vec<u16> = grid.tab_stop_positions().collect();
        assert_eq!(positions, vec![8, 16, 24, 32, 40, 48, 56, 64, 72]);

        // Clear all and set custom stops
        grid.clear_all_tab_stops();
        let positions: Vec<u16> = grid.tab_stop_positions().collect();
        assert!(positions.is_empty());

        // Set custom tab stops at 5, 10, 20
        grid.set_cursor(0, 5);
        grid.set_tab_stop();
        grid.set_cursor(0, 10);
        grid.set_tab_stop();
        grid.set_cursor(0, 20);
        grid.set_tab_stop();

        let positions: Vec<u16> = grid.tab_stop_positions().collect();
        assert_eq!(positions, vec![5, 10, 20]);
    }

    // ========================================================================
    // Scroll Region Tests for IL/DL/SU/SD
    // ========================================================================

    #[test]
    fn grid_insert_lines_with_scroll_region() {
        let mut grid = Grid::new(8, 10);
        // Write content to each row
        for row in 0..8 {
            grid.set_cursor(row, 0);
            grid.write_char((b'A' + row as u8) as char);
        }
        // Row 0: A, Row 1: B, Row 2: C, Row 3: D, Row 4: E, Row 5: F, Row 6: G, Row 7: H

        // Set scroll region: rows 2-5
        grid.set_scroll_region(2, 5);

        // Cursor at row 3 (within region)
        grid.set_cursor(3, 0);
        grid.insert_lines(2);

        // Expected result:
        // Row 0: A (unchanged, outside region)
        // Row 1: B (unchanged, outside region)
        // Row 2: C (unchanged, top of region but above cursor)
        // Row 3: (blank - inserted)
        // Row 4: (blank - inserted)
        // Row 5: D (shifted from row 3, E and F pushed off bottom of region)
        // Row 6: G (unchanged, outside region)
        // Row 7: H (unchanged, outside region)

        assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
        assert_eq!(grid.cell(1, 0).unwrap().char(), 'B');
        assert_eq!(grid.cell(2, 0).unwrap().char(), 'C');
        assert_eq!(grid.cell(3, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(4, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(5, 0).unwrap().char(), 'D');
        assert_eq!(grid.cell(6, 0).unwrap().char(), 'G');
        assert_eq!(grid.cell(7, 0).unwrap().char(), 'H');
    }

    #[test]
    fn grid_insert_lines_cursor_outside_scroll_region() {
        let mut grid = Grid::new(8, 10);
        // Write content to each row
        for row in 0..8 {
            grid.set_cursor(row, 0);
            grid.write_char((b'A' + row as u8) as char);
        }

        // Set scroll region: rows 2-5
        grid.set_scroll_region(2, 5);

        // Cursor at row 1 (above region) - IL should have no effect
        grid.set_cursor(1, 0);
        grid.insert_lines(2);

        // All rows unchanged
        for row in 0..8 {
            assert_eq!(
                grid.cell(row, 0).unwrap().char(),
                (b'A' + row as u8) as char,
                "Row {} should be unchanged",
                row
            );
        }
    }

    #[test]
    fn grid_delete_lines_with_scroll_region() {
        let mut grid = Grid::new(8, 10);
        // Write content to each row
        for row in 0..8 {
            grid.set_cursor(row, 0);
            grid.write_char((b'A' + row as u8) as char);
        }
        // Row 0: A, Row 1: B, Row 2: C, Row 3: D, Row 4: E, Row 5: F, Row 6: G, Row 7: H

        // Set scroll region: rows 2-5
        grid.set_scroll_region(2, 5);

        // Cursor at row 3 (within region)
        grid.set_cursor(3, 0);
        grid.delete_lines(2);

        // Expected result:
        // Row 0: A (unchanged, outside region)
        // Row 1: B (unchanged, outside region)
        // Row 2: C (unchanged, top of region but above cursor)
        // Row 3: F (shifted from row 5)
        // Row 4: (blank - inserted at bottom of region)
        // Row 5: (blank - inserted at bottom of region)
        // Row 6: G (unchanged, outside region)
        // Row 7: H (unchanged, outside region)

        assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
        assert_eq!(grid.cell(1, 0).unwrap().char(), 'B');
        assert_eq!(grid.cell(2, 0).unwrap().char(), 'C');
        assert_eq!(grid.cell(3, 0).unwrap().char(), 'F');
        assert_eq!(grid.cell(4, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(5, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(6, 0).unwrap().char(), 'G');
        assert_eq!(grid.cell(7, 0).unwrap().char(), 'H');
    }

    #[test]
    fn grid_delete_lines_cursor_outside_scroll_region() {
        let mut grid = Grid::new(8, 10);
        // Write content to each row
        for row in 0..8 {
            grid.set_cursor(row, 0);
            grid.write_char((b'A' + row as u8) as char);
        }

        // Set scroll region: rows 2-5
        grid.set_scroll_region(2, 5);

        // Cursor at row 7 (below region) - DL should have no effect
        grid.set_cursor(7, 0);
        grid.delete_lines(2);

        // All rows unchanged
        for row in 0..8 {
            assert_eq!(
                grid.cell(row, 0).unwrap().char(),
                (b'A' + row as u8) as char,
                "Row {} should be unchanged",
                row
            );
        }
    }

    #[test]
    fn grid_scroll_region_up_within_region() {
        let mut grid = Grid::new(8, 10);
        // Write content to each row
        for row in 0..8 {
            grid.set_cursor(row, 0);
            grid.write_char((b'A' + row as u8) as char);
        }
        // Row 0: A, Row 1: B, Row 2: C, Row 3: D, Row 4: E, Row 5: F, Row 6: G, Row 7: H

        // Set scroll region: rows 2-5
        grid.set_scroll_region(2, 5);

        // Scroll region up by 2 lines
        grid.scroll_region_up(2);

        // Expected result:
        // Row 0: A (unchanged, outside region)
        // Row 1: B (unchanged, outside region)
        // Row 2: E (shifted from row 4)
        // Row 3: F (shifted from row 5)
        // Row 4: (blank - inserted at bottom of region)
        // Row 5: (blank - inserted at bottom of region)
        // Row 6: G (unchanged, outside region)
        // Row 7: H (unchanged, outside region)

        assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
        assert_eq!(grid.cell(1, 0).unwrap().char(), 'B');
        assert_eq!(grid.cell(2, 0).unwrap().char(), 'E');
        assert_eq!(grid.cell(3, 0).unwrap().char(), 'F');
        assert_eq!(grid.cell(4, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(5, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(6, 0).unwrap().char(), 'G');
        assert_eq!(grid.cell(7, 0).unwrap().char(), 'H');
    }

    #[test]
    fn grid_scroll_region_down_within_region() {
        let mut grid = Grid::new(8, 10);
        // Write content to each row
        for row in 0..8 {
            grid.set_cursor(row, 0);
            grid.write_char((b'A' + row as u8) as char);
        }
        // Row 0: A, Row 1: B, Row 2: C, Row 3: D, Row 4: E, Row 5: F, Row 6: G, Row 7: H

        // Set scroll region: rows 2-5
        grid.set_scroll_region(2, 5);

        // Scroll region down by 2 lines
        grid.scroll_region_down(2);

        // Expected result:
        // Row 0: A (unchanged, outside region)
        // Row 1: B (unchanged, outside region)
        // Row 2: (blank - inserted at top of region)
        // Row 3: (blank - inserted at top of region)
        // Row 4: C (shifted from row 2)
        // Row 5: D (shifted from row 3, E and F pushed off)
        // Row 6: G (unchanged, outside region)
        // Row 7: H (unchanged, outside region)

        assert_eq!(grid.cell(0, 0).unwrap().char(), 'A');
        assert_eq!(grid.cell(1, 0).unwrap().char(), 'B');
        assert_eq!(grid.cell(2, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(3, 0).unwrap().char(), ' ');
        assert_eq!(grid.cell(4, 0).unwrap().char(), 'C');
        assert_eq!(grid.cell(5, 0).unwrap().char(), 'D');
        assert_eq!(grid.cell(6, 0).unwrap().char(), 'G');
        assert_eq!(grid.cell(7, 0).unwrap().char(), 'H');
    }

    // ========================================================================
    // Reflow tests (Gap 10)
    // ========================================================================

    #[test]
    fn reflow_shrink_wraps_long_line() {
        // A line of 10 chars on an 80-col terminal should wrap to 2 rows when
        // the terminal is resized to 5 columns.
        let mut grid = Grid::new(5, 10);
        for c in "ABCDEFGHIJ".chars() {
            grid.write_char(c);
        }
        assert_eq!(grid.row(0).unwrap().to_string(), "ABCDEFGHIJ");

        // Resize narrower to 5 columns
        grid.resize(5, 5);

        // Content should reflow to 2 rows
        assert_eq!(grid.row(0).unwrap().to_string(), "ABCDE");
        assert_eq!(grid.row(1).unwrap().to_string(), "FGHIJ");

        // Second row should be marked as wrapped
        assert!(grid.row(1).unwrap().is_wrapped());
    }

    #[test]
    fn reflow_grow_unwraps_soft_wrapped_lines() {
        // Create a grid with a wrapped line
        let mut grid = Grid::new(5, 5);
        for c in "ABCDE".chars() {
            grid.write_char(c);
        }
        grid.line_feed();
        grid.carriage_return();

        // Manually set the wrapped flag on row 1 and add more content
        if let Some(row) = grid.row_mut(1) {
            row.set_wrapped(true);
            for (i, c) in "FGHIJ".chars().enumerate() {
                row.write_char(i as u16, c);
            }
        }

        assert_eq!(grid.row(0).unwrap().to_string(), "ABCDE");
        assert_eq!(grid.row(1).unwrap().to_string(), "FGHIJ");
        assert!(grid.row(1).unwrap().is_wrapped());

        // Resize wider to 12 columns (enough to fit all content on one line)
        grid.resize(5, 12);

        // Content should merge: the wrapped continuation should unwrap
        assert_eq!(grid.row(0).unwrap().to_string(), "ABCDEFGHIJ");
        // Row 1 should now be empty (the continuation merged up)
        assert!(grid.row(1).unwrap().is_empty() || !grid.row(1).unwrap().is_wrapped());
    }

    #[test]
    fn reflow_shrink_preserves_cursor_position() {
        let mut grid = Grid::new(5, 20);
        // Write "ABCDEFGHIJ" and position cursor at column 7 ('H')
        for c in "ABCDEFGHIJ".chars() {
            grid.write_char(c);
        }
        grid.set_cursor(0, 7); // Position at 'H'

        // Resize to 5 columns - cursor at col 7 should move to row 1, col 2
        grid.resize(5, 5);

        // "ABCDE" on row 0, "FGHIJ" on row 1
        // Original position 7 -> row 1, col 2 (F=0, G=1, H=2)
        assert_eq!(grid.cursor_row(), 1);
        assert_eq!(grid.cursor_col(), 2);
    }

    #[test]
    fn reflow_grow_preserves_cursor_position() {
        // Start with wrapped content
        let mut grid = Grid::new(5, 5);
        for c in "ABCDE".chars() {
            grid.write_char(c);
        }
        grid.line_feed();
        grid.carriage_return();
        if let Some(row) = grid.row_mut(1) {
            row.set_wrapped(true);
            for (i, c) in "FGHIJ".chars().enumerate() {
                row.write_char(i as u16, c);
            }
        }
        // Position cursor at row 1, col 2 ('H')
        grid.set_cursor(1, 2);

        // Resize to 12 columns
        grid.resize(5, 12);

        // After unwrap, "ABCDEFGHIJ" is on row 0
        // Position row 1 col 2 was at logical offset 7 (5 from row 0 + 2)
        // After unwrap to 12 cols, it should be row 0 col 7
        assert_eq!(grid.cursor_row(), 0);
        assert_eq!(grid.cursor_col(), 7);
    }

    #[test]
    fn reflow_round_trip() {
        // Shrink then grow should preserve content
        let mut grid = Grid::new(5, 10);
        for c in "ABCDEFGHIJ".chars() {
            grid.write_char(c);
        }

        // Shrink to 5 cols
        grid.resize(5, 5);
        assert_eq!(grid.row(0).unwrap().to_string(), "ABCDE");
        assert_eq!(grid.row(1).unwrap().to_string(), "FGHIJ");

        // Grow back to 10 cols
        grid.resize(5, 10);
        // Should merge back
        assert_eq!(grid.row(0).unwrap().to_string(), "ABCDEFGHIJ");
        assert!(grid.row(1).unwrap().is_empty());
    }

    #[test]
    fn reflow_without_reflow_flag() {
        // Test resize_with_reflow(reflow=false) just truncates
        let mut grid = Grid::new(5, 10);
        for c in "ABCDEFGHIJ".chars() {
            grid.write_char(c);
        }

        // Resize without reflow - should truncate, not wrap
        grid.resize_with_reflow(5, 5, false);
        assert_eq!(grid.row(0).unwrap().to_string(), "ABCDE");
        // Row 1 should be empty (no wrapping occurred)
        assert!(grid.row(1).unwrap().is_empty());
    }

    #[test]
    fn reflow_handles_empty_rows() {
        let mut grid = Grid::new(5, 10);
        // Row 0 has content, rows 1-4 are empty
        for c in "Hello".chars() {
            grid.write_char(c);
        }

        // Shrink
        grid.resize(5, 3);
        // "Hel" on row 0, "lo" on row 1
        assert_eq!(grid.row(0).unwrap().to_string(), "Hel");
        assert_eq!(grid.row(1).unwrap().to_string(), "lo");
        // Row 2 should still be empty
        assert!(grid.row(2).unwrap().is_empty());
    }

    #[test]
    fn reflow_preserves_hard_line_breaks() {
        let mut grid = Grid::new(5, 10);
        // Write "ABC" then newline, then "DEF"
        for c in "ABC".chars() {
            grid.write_char(c);
        }
        grid.line_feed();
        grid.carriage_return();
        for c in "DEF".chars() {
            grid.write_char(c);
        }

        // Neither row should be marked as wrapped
        assert!(!grid.row(0).unwrap().is_wrapped());
        assert!(!grid.row(1).unwrap().is_wrapped());

        // Resize wider - hard breaks should be preserved (no unwrapping)
        grid.resize(5, 20);
        assert_eq!(grid.row(0).unwrap().to_string(), "ABC");
        assert_eq!(grid.row(1).unwrap().to_string(), "DEF");
    }

    // -------------------------------------------------------------------------
    // Style API tests
    // -------------------------------------------------------------------------

    #[test]
    fn grid_style_table_initialized() {
        let grid = Grid::new(24, 80);
        // Grid should have a style table with the default style
        // Note: StyleTable::is_empty() returns true when only default style exists,
        // so we check that get_style returns the default style
        assert!(grid.get_style(GRID_DEFAULT_STYLE_ID).is_some());
    }

    #[test]
    fn grid_intern_style_returns_id() {
        let mut grid = Grid::new(24, 80);
        let style = Style::new(Color::new(255, 0, 0), Color::DEFAULT_BG, StyleAttrs::BOLD);
        let id = grid.intern_style(style);
        // Should get a non-default ID for non-default style
        assert!(!id.is_default());
        // Should be able to retrieve it
        let retrieved = grid.get_style(id).unwrap();
        assert_eq!(*retrieved, style);
    }

    #[test]
    fn grid_intern_same_style_returns_same_id() {
        let mut grid = Grid::new(24, 80);
        let style = Style::new(Color::new(0, 255, 0), Color::DEFAULT_BG, StyleAttrs::ITALIC);
        let id1 = grid.intern_style(style);
        let id2 = grid.intern_style(style);
        assert_eq!(id1, id2);
    }

    #[test]
    fn grid_intern_default_style() {
        let mut grid = Grid::new(24, 80);
        let id = grid.intern_style(Style::DEFAULT);
        assert!(id.is_default());
    }

    #[test]
    fn grid_style_stats() {
        let mut grid = Grid::new(24, 80);
        let initial_stats = grid.style_stats();
        assert_eq!(initial_stats.total_styles, 1); // Just default

        // Add some styles
        grid.intern_style(Style::new(
            Color::new(255, 0, 0),
            Color::DEFAULT_BG,
            StyleAttrs::BOLD,
        ));
        grid.intern_style(Style::new(
            Color::new(0, 255, 0),
            Color::DEFAULT_BG,
            StyleAttrs::ITALIC,
        ));

        let stats = grid.style_stats();
        assert_eq!(stats.total_styles, 3); // default + 2 new
        assert!(stats.memory_bytes > 0);
    }

    #[test]
    fn grid_clear_styles() {
        let mut grid = Grid::new(24, 80);
        grid.intern_style(Style::new(
            Color::new(255, 0, 0),
            Color::DEFAULT_BG,
            StyleAttrs::empty(),
        ));
        grid.intern_style(Style::new(
            Color::new(0, 255, 0),
            Color::DEFAULT_BG,
            StyleAttrs::empty(),
        ));
        assert_eq!(grid.style_stats().total_styles, 3);

        grid.clear_styles();
        assert_eq!(grid.style_stats().total_styles, 1); // Only default remains
    }

    #[test]
    fn grid_intern_extended_style() {
        let mut grid = Grid::new(24, 80);
        let colors = PackedColors::with_indexed(196, 21);
        let flags = CellFlags::BOLD.union(CellFlags::UNDERLINE);
        let ext = ExtendedStyle::from_cell_style(colors, flags, None, None);
        let id = grid.intern_extended_style(ext);

        assert!(!id.is_default());
        let style = grid.get_style(id).unwrap();
        assert!(style.attrs.contains(StyleAttrs::BOLD));
        assert!(style.attrs.contains(StyleAttrs::UNDERLINE));
    }

    #[test]
    fn grid_write_char_with_style_id_default() {
        let mut grid = Grid::new(24, 80);
        grid.write_char_with_style_id('A', GRID_DEFAULT_STYLE_ID, CellFlags::empty());

        let cell = grid.row(0).unwrap().get(0).unwrap();
        assert_eq!(cell.char(), 'A');
        assert!(cell.colors().is_default());
    }

    #[test]
    fn grid_write_char_with_style_id_indexed_color() {
        let mut grid = Grid::new(24, 80);

        // Intern a style with indexed colors
        let ext = ExtendedStyle::from_packed_colors_separate(
            PackedColor::indexed(196), // Red
            PackedColor::indexed(21),  // Blue
            CellFlags::BOLD,
        );
        let style_id = grid.intern_extended_style(ext);

        grid.write_char_with_style_id('B', style_id, CellFlags::empty());

        let cell = grid.row(0).unwrap().get(0).unwrap();
        assert_eq!(cell.char(), 'B');
        assert!(cell.colors().fg_is_indexed());
        assert_eq!(cell.colors().fg_index(), 196);
        assert!(cell.colors().bg_is_indexed());
        assert_eq!(cell.colors().bg_index(), 21);
        assert!(cell.flags().contains(CellFlags::BOLD));
    }

    #[test]
    fn grid_write_char_with_style_id_rgb_color() {
        let mut grid = Grid::new(24, 80);

        // Intern a style with RGB colors
        let ext = ExtendedStyle::from_packed_colors_separate(
            PackedColor::rgb(255, 128, 64),
            PackedColor::rgb(32, 64, 128),
            CellFlags::ITALIC,
        );
        let style_id = grid.intern_extended_style(ext);

        grid.write_char_with_style_id('C', style_id, CellFlags::empty());

        let cell = grid.row(0).unwrap().get(0).unwrap();
        assert_eq!(cell.char(), 'C');
        assert!(cell.flags().contains(CellFlags::ITALIC));
        // RGB colors are marked as needing overflow
        assert!(cell.fg_needs_overflow());
        assert!(cell.bg_needs_overflow());
    }

    #[test]
    fn grid_write_char_with_style_id_extra_flags() {
        let mut grid = Grid::new(24, 80);

        // Intern a bold style
        let ext = ExtendedStyle::from_packed_colors_separate(
            PackedColor::DEFAULT_FG,
            PackedColor::DEFAULT_BG,
            CellFlags::BOLD,
        );
        let style_id = grid.intern_extended_style(ext);

        // Write with extra PROTECTED flag
        grid.write_char_with_style_id('D', style_id, CellFlags::PROTECTED);

        let cell = grid.row(0).unwrap().get(0).unwrap();
        assert_eq!(cell.char(), 'D');
        assert!(cell.flags().contains(CellFlags::BOLD));
        assert!(cell.flags().contains(CellFlags::PROTECTED));
    }

    #[test]
    fn grid_write_char_wrap_with_style_id() {
        let mut grid = Grid::new(24, 10); // 10 columns

        let ext = ExtendedStyle::from_packed_colors_separate(
            PackedColor::indexed(31),
            PackedColor::DEFAULT_BG,
            CellFlags::empty(),
        );
        let style_id = grid.intern_extended_style(ext);

        // Fill first row and wrap to second
        for _ in 0..12 {
            grid.write_char_wrap_with_style_id('X', style_id, CellFlags::empty());
        }

        // First row should be full (10 chars)
        let row0 = grid.row(0).unwrap();
        assert_eq!(row0.len(), 10);

        // Second row should have 2 chars
        let row1 = grid.row(1).unwrap();
        assert_eq!(row1.len(), 2);
        assert!(row1.is_wrapped());
    }

    #[test]
    fn grid_write_wide_char_with_style_id() {
        let mut grid = Grid::new(24, 80);

        let ext = ExtendedStyle::from_packed_colors_separate(
            PackedColor::indexed(196),
            PackedColor::DEFAULT_BG,
            CellFlags::empty(),
        );
        let style_id = grid.intern_extended_style(ext);

        // Write a wide CJK character
        let written = grid.write_wide_char_with_style_id('あ', style_id, CellFlags::empty());
        assert_eq!(written, 2);

        // Check the main cell
        let cell0 = grid.row(0).unwrap().get(0).unwrap();
        assert_eq!(cell0.char(), 'あ');
        assert!(cell0.is_wide());
        assert!(cell0.colors().fg_is_indexed());

        // Check the continuation cell
        let cell1 = grid.row(0).unwrap().get(1).unwrap();
        assert!(cell1.is_wide_continuation());
    }

    #[test]
    fn grid_write_wide_char_wrap_with_style_id() {
        let mut grid = Grid::new(24, 10);

        let ext = ExtendedStyle::from_packed_colors_separate(
            PackedColor::DEFAULT_FG,
            PackedColor::DEFAULT_BG,
            CellFlags::UNDERLINE,
        );
        let style_id = grid.intern_extended_style(ext);

        // Position cursor at column 9 (last column)
        grid.set_cursor(0, 9);

        // Write wide char - should wrap to next line
        let written = grid.write_wide_char_wrap_with_style_id('中', style_id, CellFlags::empty());
        assert_eq!(written, 2);

        // Character should be on row 1 (wrapped)
        let row1 = grid.row(1).unwrap();
        assert_eq!(row1.get(0).unwrap().char(), '中');
        assert!(row1.is_wrapped());
    }

    #[test]
    fn grid_style_id_deduplication_in_writes() {
        let mut grid = Grid::new(24, 80);

        // Same style interned twice should give same ID
        let ext1 = ExtendedStyle::from_packed_colors_separate(
            PackedColor::indexed(100),
            PackedColor::DEFAULT_BG,
            CellFlags::BOLD,
        );
        let ext2 = ExtendedStyle::from_packed_colors_separate(
            PackedColor::indexed(100),
            PackedColor::DEFAULT_BG,
            CellFlags::BOLD,
        );

        let id1 = grid.intern_extended_style(ext1);
        let id2 = grid.intern_extended_style(ext2);

        assert_eq!(id1, id2, "Same style should produce same ID");

        // Write with both IDs
        grid.write_char_with_style_id('E', id1, CellFlags::empty());
        grid.write_char_with_style_id('F', id2, CellFlags::empty());

        // Both cells should have identical styles
        let cell0 = grid.row(0).unwrap().get(0).unwrap();
        let cell1 = grid.row(0).unwrap().get(1).unwrap();

        assert_eq!(cell0.colors(), cell1.colors());
        assert_eq!(cell0.flags(), cell1.flags());
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Cursor is always in bounds after set_cursor.
    #[kani::proof]
    fn cursor_always_in_bounds() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 100 && rows <= 200);
        kani::assume(cols >= 200 && cols <= 400);

        let mut grid = Grid::new(rows, cols);

        let target_row: u16 = kani::any();
        let target_col: u16 = kani::any();

        grid.set_cursor(target_row, target_col);

        kani::assert(grid.cursor_row() < rows, "cursor row out of bounds");
        kani::assert(grid.cursor_col() < cols, "cursor col out of bounds");
    }

    /// Resize maintains cursor bounds.
    #[kani::proof]
    fn resize_cursor_valid() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 100 && rows <= 200);
        kani::assume(cols >= 200 && cols <= 400);

        let mut grid = Grid::new(rows, cols);

        let cursor_row: u16 = kani::any();
        let cursor_col: u16 = kani::any();
        kani::assume(cursor_row < rows);
        kani::assume(cursor_col < cols);
        grid.set_cursor(cursor_row, cursor_col);

        let new_rows: u16 = kani::any();
        let new_cols: u16 = kani::any();
        kani::assume(new_rows >= 100 && new_rows <= 200);
        kani::assume(new_cols >= 200 && new_cols <= 400);

        grid.resize(new_rows, new_cols);

        kani::assert(
            grid.cursor_row() < new_rows,
            "cursor row out of bounds after resize",
        );
        kani::assert(
            grid.cursor_col() < new_cols,
            "cursor col out of bounds after resize",
        );
    }

    /// Cell access within bounds never panics.
    #[kani::proof]
    fn cell_access_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 100 && rows <= 200);
        kani::assume(cols >= 200 && cols <= 400);

        let grid = Grid::new(rows, cols);

        let row: u16 = kani::any();
        let col: u16 = kani::any();
        kani::assume(row < rows);
        kani::assume(col < cols);

        let cell = grid.cell(row, col);
        kani::assert(cell.is_some(), "cell access failed within bounds");
    }

    /// Row access within bounds never panics.
    #[kani::proof]
    fn grid_row_access_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 100);
        kani::assume(cols > 0 && cols <= 200);

        let grid = Grid::new(rows, cols);

        let row: u16 = kani::any();
        let col: u16 = kani::any();
        kani::assume(row < rows);
        kani::assume(col < cols);

        let row_ref = grid.row(row);
        kani::assert(row_ref.is_some(), "row access failed within bounds");
        let cell = row_ref.unwrap().get(col);
        kani::assert(cell.is_some(), "row cell access failed within bounds");
    }

    /// Move cursor by maintains bounds.
    #[kani::proof]
    fn move_cursor_by_maintains_bounds() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 100 && rows <= 200);
        kani::assume(cols >= 200 && cols <= 400);

        let mut grid = Grid::new(rows, cols);

        let dr: i32 = kani::any();
        let dc: i32 = kani::any();
        kani::assume(dr >= -20 && dr <= 20);
        kani::assume(dc >= -20 && dc <= 20);

        grid.move_cursor_by(dr, dc);

        kani::assert(
            grid.cursor_row() < rows,
            "cursor row out of bounds after move",
        );
        kani::assert(
            grid.cursor_col() < cols,
            "cursor col out of bounds after move",
        );
    }

    /// Ring buffer index is always within bounds.
    ///
    /// This proves that `row_index` always returns a valid index into `self.rows`,
    /// preventing out-of-bounds access in the ring buffer.
    #[kani::proof]
    fn ring_buffer_index_within_bounds() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 100 && rows <= 200);
        kani::assume(cols >= 200 && cols <= 400);

        let mut grid = Grid::with_scrollback(rows, cols, 5);

        // Simulate scrolling by adding lines
        let lines_to_add: u8 = kani::any();
        kani::assume(lines_to_add <= 15); // Test with more lines than capacity

        for _ in 0..lines_to_add {
            grid.scroll_up_in_region(1);
        }

        // Access any visible row - should never panic or access out of bounds
        let visible_row: u16 = kani::any();
        kani::assume(visible_row < rows);

        // The row_index function should always return a valid index
        let idx = grid.row_index(visible_row);
        kani::assert(idx < grid.rows.len(), "ring buffer index out of bounds");

        // Verify the row is accessible
        let row = grid.row(visible_row);
        kani::assert(row.is_some(), "row access failed for valid visible row");
    }

    /// Ring buffer head stays within bounds after scroll operations.
    #[kani::proof]
    fn ring_head_within_bounds_after_scroll() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 100 && rows <= 200);
        kani::assume(cols >= 200 && cols <= 400);

        let mut grid = Grid::with_scrollback(rows, cols, 5);

        // Initial state: ring_head should be 0
        kani::assert(
            grid.ring_head < grid.rows.len(),
            "initial ring_head out of bounds",
        );

        // Scroll up multiple times
        let scroll_count: u8 = kani::any();
        kani::assume(scroll_count <= 20);

        for _ in 0..scroll_count {
            grid.scroll_up_in_region(1);
            // After each scroll, ring_head must still be valid
            kani::assert(
                grid.ring_head < grid.rows.len(),
                "ring_head out of bounds after scroll",
            );
        }
    }

    /// Display offset never exceeds available scrollback.
    #[kani::proof]
    fn display_offset_bounded() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 100 && rows <= 200);
        kani::assume(cols >= 200 && cols <= 400);

        let mut grid = Grid::with_scrollback(rows, cols, 5);

        // Add some scrollback
        let lines_to_add: u8 = kani::any();
        kani::assume(lines_to_add <= 10);

        for _ in 0..lines_to_add {
            grid.scroll_up_in_region(1);
        }

        // Scroll the view by arbitrary amount
        let scroll_delta: i32 = kani::any();
        kani::assume(scroll_delta >= -20 && scroll_delta <= 20);

        grid.scroll_display(scroll_delta);

        // display_offset should be bounded
        let max_scrollback = grid.total_lines.saturating_sub(grid.visible_rows as usize);
        kani::assert(
            grid.display_offset <= max_scrollback,
            "display_offset exceeds available scrollback",
        );
    }

    /// Cursor column respects effective column limit for double-width lines.
    ///
    /// INV-DW-1 from TLA+ spec: cursor_col < effective_cols_for_row(cursor_row)
    #[kani::proof]
    fn double_width_cursor_within_effective_limit() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 4 && rows <= 24);
        kani::assume(cols >= 10 && cols <= 80);

        let mut grid = Grid::new(rows, cols);

        // Set a row to double-width
        let dw_row: u16 = kani::any();
        kani::assume(dw_row < rows);
        if let Some(row) = grid.row_mut(dw_row) {
            row.set_line_size(LineSize::DoubleWidth);
        }

        // Try to position cursor on that row
        let target_col: u16 = kani::any();
        grid.set_cursor(dw_row, target_col);

        // Cursor should be clamped to effective column limit
        let effective_cols = grid.effective_cols_for_row(dw_row);
        kani::assert(
            grid.cursor_col() < effective_cols,
            "cursor exceeds effective column limit on double-width line",
        );
    }

    /// Cursor forward stops at effective limit for double-width lines.
    #[kani::proof]
    fn cursor_forward_respects_double_width() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 4 && rows <= 24);
        kani::assume(cols >= 10 && cols <= 80);

        let mut grid = Grid::new(rows, cols);

        // Set current row to double-width
        let cursor_row = grid.cursor_row();
        if let Some(row) = grid.row_mut(cursor_row) {
            row.set_line_size(LineSize::DoubleWidth);
        }

        // Move cursor forward by arbitrary amount
        let n: u16 = kani::any();
        kani::assume(n > 0 && n <= 100);
        grid.cursor_forward(n);

        // Cursor should still be within effective limit
        let effective_cols = grid.effective_cols_for_row(cursor_row);
        kani::assert(
            grid.cursor_col() < effective_cols,
            "cursor_forward exceeded effective limit",
        );
    }

    /// Cursor movement between rows clamps column to new row's effective limit.
    #[kani::proof]
    fn cursor_row_change_clamps_column() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 4 && rows <= 24);
        kani::assume(cols >= 10 && cols <= 80);

        let mut grid = Grid::new(rows, cols);

        // Set up: row 0 is single-width, row 1 is double-width
        if let Some(row) = grid.row_mut(1) {
            row.set_line_size(LineSize::DoubleWidth);
        }

        // Position cursor at high column on row 0 (single-width)
        let high_col = cols - 1;
        grid.set_cursor(0, high_col);

        // Move down to double-width row
        grid.cursor_down(1);

        // Cursor column should be clamped to double-width effective limit
        let effective_cols = grid.effective_cols_for_row(1);
        kani::assert(
            grid.cursor_col() < effective_cols,
            "cursor column not clamped when moving to double-width row",
        );
    }

    /// Resize preserves scroll region validity (reset to full).
    ///
    /// After resize, scroll region should be valid for new dimensions.
    #[kani::proof]
    fn resize_scroll_region_valid() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 4 && rows <= 24);
        kani::assume(cols >= 10 && cols <= 80);

        let mut grid = Grid::new(rows, cols);

        // Set custom scroll region
        let top: u16 = kani::any();
        let bottom: u16 = kani::any();
        kani::assume(top < rows);
        kani::assume(bottom < rows);
        kani::assume(bottom >= top);
        grid.set_scroll_region(top, bottom);

        // Resize
        let new_rows: u16 = kani::any();
        let new_cols: u16 = kani::any();
        kani::assume(new_rows >= 4 && new_rows <= 24);
        kani::assume(new_cols >= 10 && new_cols <= 80);

        grid.resize(new_rows, new_cols);

        // Scroll region should be valid
        let region = grid.scroll_region();
        kani::assert(region.top < new_rows, "scroll region top out of bounds");
        kani::assert(
            region.bottom < new_rows,
            "scroll region bottom out of bounds",
        );
        kani::assert(region.bottom >= region.top, "scroll region inverted");
    }

    /// Resize preserves saved cursor validity.
    ///
    /// After resize, saved cursor should be within new dimensions.
    #[kani::proof]
    fn resize_saved_cursor_valid() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 4 && rows <= 24);
        kani::assume(cols >= 10 && cols <= 80);

        let mut grid = Grid::new(rows, cols);

        // Save cursor at some position
        let cursor_row: u16 = kani::any();
        let cursor_col: u16 = kani::any();
        kani::assume(cursor_row < rows);
        kani::assume(cursor_col < cols);
        grid.set_cursor(cursor_row, cursor_col);
        grid.save_cursor();

        // Resize to smaller dimensions
        let new_rows: u16 = kani::any();
        let new_cols: u16 = kani::any();
        kani::assume(new_rows >= 1 && new_rows <= 24);
        kani::assume(new_cols >= 1 && new_cols <= 80);

        grid.resize(new_rows, new_cols);

        // Restore cursor - should not panic or leave cursor out of bounds
        grid.restore_cursor();
        kani::assert(
            grid.cursor_row() < new_rows,
            "restored cursor row out of bounds",
        );
        kani::assert(
            grid.cursor_col() < new_cols,
            "restored cursor col out of bounds",
        );
    }

    /// Display offset stays bounded after resize.
    ///
    /// After resize, display offset should not exceed available scrollback.
    #[kani::proof]
    fn resize_display_offset_bounded() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows >= 4 && rows <= 24);
        kani::assume(cols >= 10 && cols <= 80);

        let mut grid = Grid::new(rows, cols);

        // Scroll some content
        let scroll_count: u8 = kani::any();
        kani::assume(scroll_count <= 10);
        for _ in 0..scroll_count {
            grid.scroll_up(1);
        }

        // Set display offset via scroll_display
        let offset: i32 = kani::any();
        kani::assume(offset >= 0 && offset <= 100);
        grid.scroll_display(offset);

        // Resize
        let new_rows: u16 = kani::any();
        let new_cols: u16 = kani::any();
        kani::assume(new_rows >= 1 && new_rows <= 24);
        kani::assume(new_cols >= 1 && new_cols <= 80);

        grid.resize(new_rows, new_cols);

        // Display offset should be bounded by available scrollback
        let max_offset = grid.scrollback_lines();
        kani::assert(
            grid.display_offset() <= max_offset,
            "display offset exceeds available scrollback after resize",
        );
    }
}
