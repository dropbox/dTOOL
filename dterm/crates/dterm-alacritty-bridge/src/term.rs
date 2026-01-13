//! Term wrapper backed by dterm-core.

use crate::event::{Event, EventListener};
use crate::grid::{apply_scroll, CellCoord, Dimensions, Grid, Scroll, TermDamage};
use crate::index::{Boundary, Column, Direction, Line, Point, Side};
use crate::search::TermSearch;
use crate::selection::{Selection, SelectionRange, SelectionType};
use crate::term_mode::TermMode;
use crate::vi_mode::{InlineSearchKind, InlineSearchState, ViMarks, ViModeCursor, ViMotion};

use dterm_core::terminal::{ColorPalette, CursorStyle, Terminal, TerminalModes};

/// Configuration options for the terminal bridge.
#[derive(Debug, Clone)]
pub struct Config {
    /// Maximum scrollback history in lines.
    pub scrolling_history: usize,
    /// Characters considered as word separators for semantic selection.
    ///
    /// Used for word selection (double-click) and vi word motions (w, b, e, ge).
    /// Empty string means use default separators.
    pub semantic_escape_chars: String,
}

/// Default word separators used by Alacritty.
pub const DEFAULT_SEMANTIC_ESCAPE_CHARS: &str = ",│`|:\"' ()[]{}<>\t";

impl Default for Config {
    fn default() -> Self {
        Self {
            scrolling_history: 10_000,
            semantic_escape_chars: DEFAULT_SEMANTIC_ESCAPE_CHARS.to_string(),
        }
    }
}

/// Alacritty-style terminal wrapper backed by dterm-core.
pub struct Term<T> {
    terminal: Terminal,
    config: Config,
    event_proxy: T,
    /// Terminal focus state, controlling cursor appearance.
    ///
    /// When true, the terminal has focus and the cursor should be displayed
    /// with full visibility. When false, cursor may be dimmed or hidden
    /// depending on the cursor style.
    pub is_focused: bool,
    /// Current text selection, if any.
    pub selection: Option<Selection>,
    /// Vi mode cursor state.
    pub vi_mode_cursor: ViModeCursor,
    /// Whether vi mode is active.
    vi_mode: bool,
    /// Last inline character search (f/F/t/T) for ; and , repeat.
    inline_search: Option<InlineSearchState>,
    /// Search state for n/N navigation.
    search_state: TermSearch,
    /// Vi mode marks (m, `, ').
    marks: ViMarks,
}

impl<T> Term<T> {
    /// Access the underlying dterm-core terminal.
    #[must_use]
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Mutable access to the underlying dterm-core terminal.
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.terminal
    }

    /// Access the terminal grid.
    #[must_use]
    pub fn grid(&self) -> &Grid {
        self.terminal.grid()
    }

    /// Mutable access to the terminal grid.
    pub fn grid_mut(&mut self) -> &mut Grid {
        self.terminal.grid_mut()
    }

    /// Current configuration.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Update terminal configuration at runtime.
    ///
    /// This allows changing settings like scrollback history size
    /// without recreating the terminal.
    ///
    /// Note: Currently, scrollback history size changes take effect only
    /// for new scrollback content. Existing scrollback is preserved.
    pub fn set_options(&mut self, config: Config) {
        // Store the new config
        // Note: scrolling_history is used when creating new scrollback
        // Future: could truncate existing scrollback if new limit is smaller
        self.config = config;
    }

    /// Event proxy for external listeners.
    #[must_use]
    pub fn event_proxy(&self) -> &T {
        &self.event_proxy
    }

    /// Terminal mode state.
    #[must_use]
    pub fn modes(&self) -> &TerminalModes {
        self.terminal.modes()
    }

    /// Get terminal modes as Alacritty-style bitflags.
    ///
    /// This provides efficient mode checking via bitwise operations.
    /// Includes Kitty keyboard protocol flags.
    #[must_use]
    pub fn mode(&self) -> TermMode {
        TermMode::from_terminal_modes_with_keyboard(
            self.terminal.modes(),
            self.vi_mode,
            self.terminal.kitty_keyboard_flags(),
        )
    }

    /// Current cursor style from terminal modes.
    #[must_use]
    pub fn cursor_style(&self) -> CursorStyle {
        self.terminal.modes().cursor_style
    }

    /// Terminal color palette.
    #[must_use]
    pub fn colors(&self) -> &ColorPalette {
        self.terminal.color_palette()
    }

    /// Mutable access to the color palette for customization.
    pub fn colors_mut(&mut self) -> &mut ColorPalette {
        self.terminal.color_palette_mut()
    }

    /// Process input bytes through the terminal parser.
    pub fn process(&mut self, input: &[u8]) {
        self.terminal.process(input);
    }

    /// Apply an Alacritty-style scroll request.
    pub fn scroll_display(&mut self, scroll: Scroll) {
        apply_scroll(self.terminal.grid_mut(), scroll);
    }

    /// Resize the terminal grid.
    pub fn resize<D: Dimensions>(&mut self, dimensions: &D) {
        let rows = dimensions.screen_lines().max(1) as u16;
        let cols = dimensions.columns().max(1) as u16;
        self.terminal.grid_mut().resize(rows, cols);
    }

    /// Get the instant when synchronized output mode should timeout.
    ///
    /// Returns `None` if sync mode is not enabled.
    /// Returns `Some(instant)` indicating when the sync mode should expire.
    ///
    /// The event loop uses this to calculate poll timeouts and force
    /// the mode off if the application doesn't disable it in time.
    #[must_use]
    pub fn sync_timeout(&self) -> Option<std::time::Instant> {
        self.terminal.sync_timeout()
    }

    /// Force synchronized output mode off.
    ///
    /// Called by the event loop when the sync timeout expires. This prevents
    /// indefinite screen freezes if an application enables sync mode but crashes
    /// or otherwise fails to disable it.
    pub fn stop_sync(&mut self) {
        self.terminal.stop_sync();
    }

    /// Return current damage state for rendering (raw).
    #[must_use]
    pub fn damage(&self) -> &dterm_core::grid::Damage {
        self.terminal.grid().damage()
    }

    /// Return Alacritty-style terminal damage for rendering.
    ///
    /// Returns `TermDamage::Full` if full redraw is needed, or
    /// `TermDamage::Partial` with an iterator over damaged lines.
    #[must_use]
    pub fn term_damage(&self) -> TermDamage<'_> {
        TermDamage::from_grid(self.terminal.grid())
    }

    /// Clear damage after rendering.
    pub fn reset_damage(&mut self) {
        self.terminal.grid_mut().clear_damage();
    }

    // ===== Selection Methods =====

    /// Start a new selection at the given point.
    pub fn start_selection(&mut self, ty: SelectionType, point: Point, side: Side) {
        self.selection = Some(Selection::new(ty, point, side));
    }

    /// Update the end point of the current selection.
    pub fn update_selection(&mut self, point: Point, side: Side) {
        if let Some(sel) = &mut self.selection {
            sel.update(point, side);
        }
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Get the selection range in grid coordinates.
    pub fn selection_range(&self) -> Option<SelectionRange> {
        self.selection.as_ref()?.to_range(self)
    }

    /// Convert the current selection to a string.
    ///
    /// Returns `None` if there is no selection.
    pub fn selection_to_string(&self) -> Option<String> {
        let range = self.selection_range()?;
        Some(self.bounds_to_string_block(range.start, range.end, range.is_block))
    }

    /// Convert a range of points to a string.
    ///
    /// This is the Alacritty-compatible signature that assumes non-block selection.
    /// For block selection support, use [`Self::bounds_to_string_block`].
    pub fn bounds_to_string(&self, start: Point, end: Point) -> String {
        self.bounds_to_string_block(start, end, false)
    }

    /// Convert a range of points to a string, with block selection support.
    ///
    /// When `is_block` is true, the same column range is extracted from each line.
    /// When false, the selection flows from start to end across lines.
    pub fn bounds_to_string_block(&self, start: Point, end: Point, is_block: bool) -> String {
        let grid = self.terminal.grid();
        let mut result = String::new();
        let cols = grid.cols() as usize;

        for line_idx in start.line.0..=end.line.0 {
            // Determine column bounds for this line
            let (start_col, end_col) = if is_block {
                // Block selection: same columns on every line
                let sc = start.column.0.min(end.column.0);
                let ec = start.column.0.max(end.column.0);
                (sc, ec)
            } else if line_idx == start.line.0 && line_idx == end.line.0 {
                // Single line selection
                (start.column.0, end.column.0)
            } else if line_idx == start.line.0 {
                // First line of multi-line selection
                (start.column.0, cols - 1)
            } else if line_idx == end.line.0 {
                // Last line of multi-line selection
                (0, end.column.0)
            } else {
                // Middle line of multi-line selection
                (0, cols - 1)
            };

            // Extract text from this line
            if line_idx >= 0 && line_idx < grid.rows() as i32 {
                let row_idx = line_idx as u16;
                for col in start_col..=end_col.min(cols - 1) {
                    if let Some(cell) = grid.cell(row_idx, col as u16) {
                        let ch = cell.char();
                        if ch != '\0' && ch != ' ' || !result.is_empty() || col > start_col {
                            result.push(ch);
                        }
                    }
                }
            }

            // Add newline between lines (not after the last line)
            if line_idx < end.line.0 && !is_block {
                // Trim trailing spaces before newline
                while result.ends_with(' ') {
                    result.pop();
                }
                result.push('\n');
            }
        }

        // Trim trailing whitespace
        result.trim_end().to_string()
    }

    // ===== Vi Mode Methods =====

    /// Check if vi mode is currently active.
    #[must_use]
    pub fn is_vi_mode(&self) -> bool {
        self.vi_mode
    }

    /// Toggle vi mode on or off.
    pub fn toggle_vi_mode(&mut self) {
        self.vi_mode = !self.vi_mode;

        if self.vi_mode {
            // Initialize vi cursor at the terminal cursor position
            let cursor = self.terminal.grid().cursor();
            self.vi_mode_cursor = ViModeCursor::new(Point::new(
                Line(cursor.row as i32),
                Column(cursor.col as usize),
            ));
        } else {
            // Clear selection when exiting vi mode
            self.selection = None;
        }
    }

    /// Execute a vi motion command.
    pub fn vi_motion(&mut self, motion: ViMotion) {
        if !self.vi_mode {
            return;
        }

        // Handle motions that need grid content access directly
        let new_point = match motion {
            // Semantic word motions (w, b, e, ge)
            ViMotion::SemanticRight => self.semantic_word_right(self.vi_mode_cursor.point),
            ViMotion::SemanticLeft => self.semantic_word_left(self.vi_mode_cursor.point),
            ViMotion::SemanticRightEnd => self.semantic_word_right_end(self.vi_mode_cursor.point),
            ViMotion::SemanticLeftEnd => self.semantic_word_left_end(self.vi_mode_cursor.point),

            // Whitespace-separated word motions (W, B, E, gE)
            ViMotion::WordRight => self.whitespace_word_right(self.vi_mode_cursor.point),
            ViMotion::WordLeft => self.whitespace_word_left(self.vi_mode_cursor.point),
            ViMotion::WordRightEnd => self.whitespace_word_right_end(self.vi_mode_cursor.point),
            ViMotion::WordLeftEnd => self.whitespace_word_left_end(self.vi_mode_cursor.point),

            // Bracket matching (%)
            ViMotion::Bracket => self
                .bracket_match(self.vi_mode_cursor.point)
                .unwrap_or(self.vi_mode_cursor.point),

            // First non-empty cell (^)
            ViMotion::FirstOccupied => self.first_occupied(self.vi_mode_cursor.point.line),

            // Paragraph motions ({ and })
            ViMotion::ParagraphUp => self.paragraph_up(self.vi_mode_cursor.point),
            ViMotion::ParagraphDown => self.paragraph_down(self.vi_mode_cursor.point),

            // Search motions (n/N)
            ViMotion::SearchNext => {
                // vi_search_next already updates the cursor
                self.vi_search_next();
                return;
            }
            ViMotion::SearchPrevious => {
                // vi_search_previous already updates the cursor
                self.vi_search_previous();
                return;
            }

            // Mark motions (`, ')
            ViMotion::GotoMark(mark) => {
                // Jump to exact mark position
                match self.marks.get(mark) {
                    Some(point) => point,
                    None => return, // Mark not set, do nothing
                }
            }
            ViMotion::GotoMarkLine(mark) => {
                // Jump to first non-blank on marked line
                match self.marks.get(mark) {
                    Some(point) => self.first_occupied(point.line),
                    None => return, // Mark not set, do nothing
                }
            }

            // URL navigation
            ViMotion::UrlNext => {
                self.vi_goto_next_url();
                return;
            }
            ViMotion::UrlPrev => {
                self.vi_goto_prev_url();
                return;
            }

            // Other motions handled by ViModeCursor
            _ => {
                self.vi_mode_cursor = self.vi_mode_cursor.motion(self, motion, Boundary::Grid);
                return;
            }
        };

        self.vi_mode_cursor.point = new_point;
    }

    /// Move vi cursor to a specific point.
    pub fn vi_goto_point(&mut self, point: Point) {
        if !self.vi_mode {
            return;
        }

        // Clamp point to valid bounds
        let topmost = self.topmost_line();
        let bottommost = self.bottommost_line();
        let last_col = self.last_column();

        let line = Line(point.line.0.clamp(topmost.0, bottommost.0));
        let column = Column(point.column.0.min(last_col.0));

        self.vi_mode_cursor.point = Point::new(line, column);
    }

    /// Scroll the vi cursor (like Ctrl+D, Ctrl+U).
    pub fn vi_scroll(&mut self, lines: i32) {
        if !self.vi_mode {
            return;
        }

        self.vi_mode_cursor = self.vi_mode_cursor.scroll(self, lines);
    }

    /// Start selection in vi mode at the current cursor position.
    pub fn vi_start_selection(&mut self, ty: SelectionType) {
        if !self.vi_mode {
            return;
        }

        let point = self.vi_mode_cursor.point;
        self.selection = Some(Selection::new(ty, point, Side::Left));
    }

    /// Update selection in vi mode to current cursor position.
    pub fn vi_update_selection(&mut self) {
        if !self.vi_mode {
            return;
        }

        if let Some(sel) = &mut self.selection {
            sel.update(self.vi_mode_cursor.point, Side::Right);
        }
    }

    // ===== Vi Mode Mark Methods =====

    /// Set a mark at the current vi cursor position (vim 'm').
    ///
    /// Valid mark characters are a-z for local marks.
    /// Returns true if the mark was set, false if the character is invalid.
    ///
    /// Before setting the mark, automatically saves the current position
    /// to the special `` ` `` and `'` marks (last jump position).
    pub fn vi_set_mark(&mut self, mark: char) -> bool {
        if !self.vi_mode {
            return false;
        }

        let point = self.vi_mode_cursor.point;
        self.marks.set(mark, point)
    }

    /// Go to a mark (vim `` ` ``).
    ///
    /// Jumps to the exact position of the mark.
    /// Returns the new cursor position, or None if the mark is not set.
    pub fn vi_goto_mark(&mut self, mark: char) -> Option<Point> {
        if !self.vi_mode {
            return None;
        }

        let target = self.marks.get(mark)?;

        // Save current position to special marks before jumping
        let current = self.vi_mode_cursor.point;
        self.marks.set('`', current);
        self.marks.set('\'', current);

        // Clamp to valid bounds
        let topmost = self.topmost_line();
        let bottommost = self.bottommost_line();
        let last_col = self.last_column();

        let line = Line(target.line.0.clamp(topmost.0, bottommost.0));
        let column = Column(target.column.0.min(last_col.0));

        self.vi_mode_cursor.point = Point::new(line, column);
        Some(self.vi_mode_cursor.point)
    }

    /// Go to the first non-blank character on a marked line (vim `'`).
    ///
    /// Jumps to the first non-blank character on the line where the mark is set.
    /// Returns the new cursor position, or None if the mark is not set.
    pub fn vi_goto_mark_line(&mut self, mark: char) -> Option<Point> {
        if !self.vi_mode {
            return None;
        }

        let target = self.marks.get(mark)?;

        // Save current position to special marks before jumping
        let current = self.vi_mode_cursor.point;
        self.marks.set('`', current);
        self.marks.set('\'', current);

        // Clamp line to valid bounds
        let topmost = self.topmost_line();
        let bottommost = self.bottommost_line();
        let line = Line(target.line.0.clamp(topmost.0, bottommost.0));

        // Find first non-blank on the line
        let point = self.first_occupied(line);
        self.vi_mode_cursor.point = point;
        Some(point)
    }

    /// Get the position of a mark.
    ///
    /// Returns `None` if the mark is not set.
    #[must_use]
    pub fn mark(&self, mark: char) -> Option<Point> {
        self.marks.get(mark)
    }

    /// Check if a mark is set.
    #[must_use]
    pub fn has_mark(&self, mark: char) -> bool {
        self.marks.contains(mark)
    }

    /// Get access to the marks storage.
    #[must_use]
    pub fn marks(&self) -> &ViMarks {
        &self.marks
    }

    /// Get mutable access to the marks storage.
    pub fn marks_mut(&mut self) -> &mut ViMarks {
        &mut self.marks
    }

    /// Clear all marks.
    pub fn clear_marks(&mut self) {
        self.marks.clear();
    }

    // ===== Terminal Reset & Clear Methods =====

    /// Full terminal reset (RIS).
    ///
    /// Resets all terminal state to initial values.
    pub fn reset(&mut self) {
        self.terminal.reset();
        self.selection = None;
        self.vi_mode = false;
        self.vi_mode_cursor = ViModeCursor::default();
        self.inline_search = None;
        self.search_state.clear();
        self.marks.clear();
    }

    /// Check if the alternate screen buffer is active.
    #[must_use]
    pub fn is_alt_screen(&self) -> bool {
        self.terminal.is_alternate_screen()
    }

    /// Swap between primary and alternate screen buffers.
    ///
    /// This toggles between the main screen and alternate screen buffer.
    /// Applications like vim, less, or tmux use the alternate screen to
    /// preserve the primary screen content.
    ///
    /// Note: This processes the DECSET/DECRST escape sequence for mode 1049
    /// (alternate screen with saved cursor) internally.
    pub fn swap_alt(&mut self) {
        // Toggle to the opposite screen using the standard escape sequence
        // CSI ? 1049 h = enter alternate screen (save cursor + clear)
        // CSI ? 1049 l = exit alternate screen (restore cursor)
        if self.terminal.is_alternate_screen() {
            // Currently on alternate, switch to primary
            self.terminal.process(b"\x1b[?1049l");
        } else {
            // Currently on primary, switch to alternate
            self.terminal.process(b"\x1b[?1049h");
        }
        // Clear selection when swapping screens
        self.selection = None;
    }

    /// Check if a specific line is wrapped (continues from previous line).
    ///
    /// In Alacritty, wrapped lines have the `RowFlags::WRAPLINE` flag set.
    /// This maps to our row's wrapping state.
    #[must_use]
    pub fn line_is_wrapped(&self, line: Line) -> bool {
        // Convert Alacritty Line to row index
        if line.0 < 0 || line.0 >= self.terminal.grid().rows() as i32 {
            return false;
        }
        let row = line.0 as u16;
        self.terminal
            .grid()
            .row(row)
            .map(|r| r.flags().contains(dterm_core::grid::RowFlags::WRAPPED))
            .unwrap_or(false)
    }

    /// Get the window title.
    #[must_use]
    pub fn title(&self) -> &str {
        self.terminal.title()
    }

    /// Get current working directory (from OSC 7).
    #[must_use]
    pub fn working_directory(&self) -> Option<&str> {
        self.terminal.current_working_directory()
    }

    /// Get the cursor position as an Alacritty-style Point.
    #[must_use]
    pub fn cursor_point(&self) -> Point {
        let cursor = self.terminal.grid().cursor();
        Point::new(Line(cursor.row as i32), Column(cursor.col as usize))
    }

    /// Check if the cursor is visible.
    #[must_use]
    pub fn cursor_visible(&self) -> bool {
        self.terminal.cursor_visible()
    }

    // ===== Wide Character Handling =====

    /// Expand a point past wide character cells.
    ///
    /// Wide characters (like CJK) occupy two cells. When navigating or selecting,
    /// we need to ensure we don't land on a wide char spacer cell. This method
    /// expands the point to include the full wide character.
    #[must_use]
    pub fn expand_wide(&self, point: Point, direction: Direction) -> Point {
        let grid = self.terminal.grid();
        if point.line.0 < 0 || point.line.0 >= grid.rows() as i32 {
            return point;
        }

        let row = point.line.0 as u16;
        let col = point.column.0 as u16;

        if let Some(cell) = grid.cell(row, col) {
            let flags = cell.flags();

            match direction {
                Direction::Right => {
                    // If we're on a wide char, move past the continuation cell
                    if flags.contains(dterm_core::grid::CellFlags::WIDE)
                        && point.column.0 < self.columns() - 1
                    {
                        return Point::new(point.line, point.column + 1);
                    }
                }
                Direction::Left => {
                    // If we're on a wide char continuation, move back to the wide char
                    if flags.contains(dterm_core::grid::CellFlags::WIDE_CONTINUATION)
                        && point.column.0 > 0
                    {
                        return Point::new(point.line, point.column - 1);
                    }
                }
            }
        }

        point
    }

    // ===== Inline Character Search (f/F/t/T vi motions) =====

    /// Search to the left for a character contained in `needles`.
    ///
    /// Used for `F` (find char left) and `T` (till char left) vi motions.
    /// Returns `Ok(Point)` if found, `Err(Point)` with the last searched point if not.
    pub fn inline_search_left(&self, mut point: Point, needles: &str) -> Result<Point, Point> {
        // Clamp to valid bounds
        let topmost = self.topmost_line();
        point.line = Line(point.line.0.max(topmost.0));

        let cols = self.columns();

        while let Some(prev) = self.point_backward(point) {
            // Stop at line breaks (non-wrapped lines)
            if prev.column.0 == cols - 1 && point.column.0 == 0 {
                // Check if the line is wrapped
                if !self.line_is_wrapped(prev.line) {
                    break;
                }
            }

            point = prev;

            // Skip wide char spacers
            let expanded = self.expand_wide(point, Direction::Left);
            if expanded != point {
                point = expanded;
                continue;
            }

            // Check if character matches
            let ch = self.char_at(point);
            if needles.contains(ch) {
                return Ok(point);
            }
        }

        Err(point)
    }

    /// Search to the right for a character contained in `needles`.
    ///
    /// Used for `f` (find char right) and `t` (till char right) vi motions.
    /// Returns `Ok(Point)` if found, `Err(Point)` with the last searched point if not.
    pub fn inline_search_right(&self, mut point: Point, needles: &str) -> Result<Point, Point> {
        // Clamp to valid bounds
        let topmost = self.topmost_line();
        point.line = Line(point.line.0.max(topmost.0));

        let cols = self.columns();

        // Check if we're starting at a line break (non-wrapped)
        if point.column.0 == cols - 1 && !self.line_is_wrapped(point.line) {
            return Err(point);
        }

        while let Some(next) = self.point_forward(point) {
            point = next;

            // Skip wide char spacers
            let expanded = self.expand_wide(point, Direction::Right);
            if expanded != point {
                continue;
            }

            // Check if character matches
            let ch = self.char_at(point);
            if needles.contains(ch) {
                return Ok(point);
            }

            // Stop at line breaks (non-wrapped lines)
            if point.column.0 == cols - 1 && !self.line_is_wrapped(point.line) {
                break;
            }
        }

        Err(point)
    }

    // ===== Inline Search with State (f/F/t/T with ;/, repeat) =====

    /// Perform inline character search and store state for repeat.
    ///
    /// This is used for vi mode f/F/t/T motions. The search is remembered
    /// so it can be repeated with `;` (same direction) or `,` (opposite).
    ///
    /// - `kind`: The type of search (find/till, left/right)
    /// - `ch`: The character to search for
    ///
    /// Returns the new cursor position if found.
    pub fn vi_inline_search(&mut self, kind: InlineSearchKind, ch: char) -> Option<Point> {
        // Store the search state for repeat
        self.inline_search = Some(InlineSearchState { char: ch, kind });

        // Perform the search
        self.perform_inline_search(kind, ch)
    }

    /// Repeat the last inline search in the same direction (vim `;`).
    ///
    /// Returns the new cursor position if found.
    pub fn vi_inline_search_repeat(&mut self) -> Option<Point> {
        let state = self.inline_search?;
        self.perform_inline_search(state.kind, state.char)
    }

    /// Repeat the last inline search in the opposite direction (vim `,`).
    ///
    /// Returns the new cursor position if found.
    pub fn vi_inline_search_repeat_reverse(&mut self) -> Option<Point> {
        let state = self.inline_search?;
        self.perform_inline_search(state.kind.reversed(), state.char)
    }

    /// Get the last inline search state.
    #[must_use]
    pub fn inline_search_state(&self) -> Option<InlineSearchState> {
        self.inline_search
    }

    /// Clear the inline search state.
    pub fn clear_inline_search(&mut self) {
        self.inline_search = None;
    }

    /// Perform an inline search without storing state.
    fn perform_inline_search(&self, kind: InlineSearchKind, ch: char) -> Option<Point> {
        if !self.vi_mode {
            return None;
        }

        let needle = ch.to_string();
        let point = self.vi_mode_cursor.point;

        let result = match kind.direction() {
            Direction::Right => self.inline_search_right(point, &needle),
            Direction::Left => self.inline_search_left(point, &needle),
        };

        match result {
            Ok(mut found) => {
                // For "till" searches, stop one position before the character
                if kind.is_till() {
                    match kind.direction() {
                        Direction::Right => {
                            // t: stop before the character (move back one)
                            if let Some(prev) = self.point_backward(found) {
                                found = prev;
                            }
                        }
                        Direction::Left => {
                            // T: stop after the character (move forward one)
                            if let Some(next) = self.point_forward(found) {
                                found = next;
                            }
                        }
                    }
                }
                Some(found)
            }
            Err(_) => None,
        }
    }

    // ===== Search State (n/N navigation) =====

    /// Get mutable access to the search state.
    pub fn search_state_mut(&mut self) -> &mut TermSearch {
        &mut self.search_state
    }

    /// Get access to the search state.
    #[must_use]
    pub fn search_state(&self) -> &TermSearch {
        &self.search_state
    }

    /// Set the search query and find all matches.
    ///
    /// This updates the internal search state. Use `vi_search_next` and
    /// `vi_search_previous` to navigate between matches.
    pub fn set_search_query(&mut self, query: Option<&str>) {
        // Access grid through terminal to allow disjoint borrows
        let grid = self.terminal.grid();
        self.search_state.set_query(query, grid);
    }

    /// Mark the search index as dirty (needs rebuild).
    ///
    /// Call this when terminal content changes significantly.
    pub fn mark_search_dirty(&mut self) {
        self.search_state.mark_dirty();
    }

    /// Navigate to the next search match (vim `n`).
    ///
    /// Returns the match if found and moves the vi cursor to it.
    pub fn vi_search_next(&mut self) -> Option<Point> {
        if !self.vi_mode {
            return None;
        }

        // Rebuild index if dirty
        if self.search_state.is_dirty() {
            // Access grid through terminal to allow disjoint borrows
            let grid = self.terminal.grid();
            self.search_state.rebuild_index(grid);
        }

        // Focus next match from current position
        let current = self.vi_mode_cursor.point;
        let m = self.search_state.focus_next(current)?.start;
        self.vi_mode_cursor.point = m;
        Some(m)
    }

    /// Navigate to the previous search match (vim `N`).
    ///
    /// Returns the match if found and moves the vi cursor to it.
    pub fn vi_search_previous(&mut self) -> Option<Point> {
        if !self.vi_mode {
            return None;
        }

        // Rebuild index if dirty
        if self.search_state.is_dirty() {
            // Access grid through terminal to allow disjoint borrows
            let grid = self.terminal.grid();
            self.search_state.rebuild_index(grid);
        }

        // Focus previous match from current position
        let current = self.vi_mode_cursor.point;
        let m = self.search_state.focus_prev(current)?.start;
        self.vi_mode_cursor.point = m;
        Some(m)
    }

    // ===== Scroll to Point =====

    /// Scroll the display to make a point visible.
    ///
    /// If the point is in scrollback history, adjusts the display offset
    /// to bring it into view. If already visible, does nothing.
    pub fn scroll_to_point(&mut self, point: Point) {
        let display_offset = self.display_offset() as i32;
        let screen_lines = self.screen_lines() as i32;

        // Calculate the visible range
        // With display_offset=0, visible lines are 0..screen_lines
        // With display_offset=N, visible lines are -N..(screen_lines-N)
        let visible_top = Line(-display_offset);
        let visible_bottom = Line(screen_lines - 1 - display_offset);

        if point.line < visible_top {
            // Point is above visible area, scroll up
            let delta = visible_top.0 - point.line.0;
            self.terminal.grid_mut().scroll_display(delta);
        } else if point.line > visible_bottom {
            // Point is below visible area, scroll down
            let delta = visible_bottom.0 - point.line.0;
            self.terminal.grid_mut().scroll_display(delta);
        }
    }

    /// Get the cursor shape/style.
    #[must_use]
    pub fn cursor_shape(&self) -> CursorStyle {
        self.terminal.cursor_style()
    }

    /// Check if cursor is blinking.
    #[must_use]
    pub fn cursor_blink(&self) -> bool {
        matches!(
            self.terminal.cursor_style(),
            CursorStyle::BlinkingBlock | CursorStyle::BlinkingUnderline | CursorStyle::BlinkingBar
        )
    }

    /// Get the display offset (scrollback position).
    #[must_use]
    pub fn display_offset(&self) -> usize {
        self.terminal.grid().display_offset()
    }

    /// Set display offset for scrollback viewing.
    pub fn set_display_offset(&mut self, offset: usize) {
        // Use scroll_display to adjust the offset relative to current position
        let current = self.terminal.grid().display_offset();
        let delta = offset as i32 - current as i32;
        self.terminal.grid_mut().scroll_display(delta);
    }

    /// Get the number of lines in scrollback history.
    #[must_use]
    pub fn history_size(&self) -> usize {
        self.terminal.grid().scrollback_lines()
    }

    /// Get the default foreground color.
    #[must_use]
    pub fn foreground_color(&self) -> dterm_core::terminal::Rgb {
        self.terminal.default_foreground()
    }

    /// Get the default background color.
    #[must_use]
    pub fn background_color(&self) -> dterm_core::terminal::Rgb {
        self.terminal.default_background()
    }

    /// Get the cursor color (if explicitly set).
    #[must_use]
    pub fn cursor_color(&self) -> Option<dterm_core::terminal::Rgb> {
        self.terminal.cursor_color()
    }

    /// Set a palette color.
    pub fn set_color(&mut self, index: u8, color: dterm_core::terminal::Rgb) {
        self.terminal.set_palette_color(index, color);
    }

    /// Reset a palette color to its default value.
    pub fn reset_color(&mut self, index: u8) {
        // Reset by setting to the default palette value
        let default = dterm_core::terminal::ColorPalette::default_color(index);
        self.terminal.set_palette_color(index, default);
    }

    /// Get the semantic escape characters for word selection.
    ///
    /// These characters define word boundaries for semantic selection.
    /// Returns the configured separators, or default if empty.
    #[must_use]
    pub fn semantic_escape_chars(&self) -> &str {
        if self.config.semantic_escape_chars.is_empty() {
            DEFAULT_SEMANTIC_ESCAPE_CHARS
        } else {
            &self.config.semantic_escape_chars
        }
    }

    /// Clear the scrollback history.
    pub fn clear_history(&mut self) {
        // Scroll to bottom first to reset display offset
        self.terminal.grid_mut().scroll_to_bottom();
        // Clear scrollback through grid's scrollback if available
        if let Some(scrollback) = self.terminal.grid_mut().scrollback_mut() {
            scrollback.clear();
        }
    }

    /// Get a cell at the given point.
    ///
    /// Returns None if the point is out of bounds.
    #[must_use]
    pub fn cell(&self, point: Point) -> Option<&dterm_core::grid::Cell> {
        if point.line.0 < 0 || point.line.0 >= self.terminal.grid().rows() as i32 {
            return None;
        }
        let row = point.line.0 as u16;
        let col = point.column.0 as u16;
        self.terminal.grid().cell(row, col)
    }

    /// Get a mutable cell at the given point.
    ///
    /// Returns None if the point is out of bounds.
    pub fn cell_mut(&mut self, point: Point) -> Option<&mut dterm_core::grid::Cell> {
        if point.line.0 < 0 || point.line.0 >= self.terminal.grid().rows() as i32 {
            return None;
        }
        let row = point.line.0 as u16;
        let col = point.column.0 as u16;
        self.terminal.grid_mut().cell_mut(row, col)
    }

    /// Check if bracketed paste mode is enabled.
    #[must_use]
    pub fn bracketed_paste_mode(&self) -> bool {
        self.terminal.modes().bracketed_paste
    }

    /// Format text for pasting (handles bracketed paste mode).
    #[must_use]
    pub fn format_paste(&self, text: &str) -> Vec<u8> {
        self.terminal.format_paste(text)
    }

    /// Get the mouse mode.
    #[must_use]
    pub fn mouse_mode(&self) -> dterm_core::terminal::MouseMode {
        self.terminal.modes().mouse_mode
    }

    /// Get the mouse encoding mode.
    #[must_use]
    pub fn mouse_encoding(&self) -> dterm_core::terminal::MouseEncoding {
        self.terminal.modes().mouse_encoding
    }

    /// Check if focus tracking is enabled.
    #[must_use]
    pub fn focus_mode(&self) -> bool {
        self.terminal.modes().focus_reporting
    }

    // ===== Kitty Keyboard Protocol =====

    /// Get the current Kitty keyboard enhancement flags.
    ///
    /// These flags control how keyboard input is reported to the application.
    /// See the Kitty keyboard protocol documentation for details.
    #[must_use]
    pub fn kitty_keyboard_flags(&self) -> dterm_core::terminal::KittyKeyboardFlags {
        self.terminal.kitty_keyboard_flags()
    }

    /// Get access to the full Kitty keyboard protocol state.
    ///
    /// This provides access to the keyboard mode stack and current flags.
    #[must_use]
    pub fn kitty_keyboard(&self) -> &dterm_core::terminal::KittyKeyboardState {
        self.terminal.kitty_keyboard()
    }

    /// Get mutable access to the Kitty keyboard protocol state.
    ///
    /// This allows direct manipulation of the keyboard mode stack.
    pub fn kitty_keyboard_mut(&mut self) -> &mut dterm_core::terminal::KittyKeyboardState {
        self.terminal.kitty_keyboard_mut()
    }

    // ===== Semantic Navigation Methods =====

    /// Find the start of a word at the given point.
    ///
    /// A word is a sequence of non-separator characters.
    /// Returns the point at the start of the word.
    #[must_use]
    pub fn word_start(&self, point: Point) -> Point {
        let separators = self.semantic_escape_chars();
        let cols = self.terminal.grid().cols() as usize;

        let mut result = point;

        // Move backward until we hit a separator or start of line
        loop {
            // Check if current character is a separator
            if let Some(cell) = self.cell(result) {
                let ch = cell.char();
                if ch == ' ' || ch == '\0' || separators.contains(ch) {
                    // Move forward one to get back to word start
                    if result.column.0 < cols - 1 {
                        result.column = result.column + 1;
                    } else if result.line < self.bottommost_line() {
                        result.line = result.line + 1;
                        result.column = Column(0);
                    }
                    break;
                }
            }

            // Move backward
            if result.column.0 > 0 {
                result.column = result.column - 1;
            } else if result.line > self.topmost_line() {
                result.line = result.line - 1;
                result.column = Column(cols - 1);
            } else {
                // Reached the start
                break;
            }
        }

        result
    }

    /// Find the end of a word at the given point.
    ///
    /// A word is a sequence of non-separator characters.
    /// Returns the point at the end of the word.
    #[must_use]
    pub fn word_end(&self, point: Point) -> Point {
        let separators = self.semantic_escape_chars();
        let cols = self.terminal.grid().cols() as usize;

        let mut result = point;

        // Move forward until we hit a separator or end of line
        while let Some(cell) = self.cell(result) {
            let ch = cell.char();
            if ch == ' ' || ch == '\0' || separators.contains(ch) {
                // Move back one to get the word end
                if result.column.0 > 0 {
                    result.column = result.column - 1;
                } else if result.line > self.topmost_line() {
                    result.line = result.line - 1;
                    result.column = Column(cols - 1);
                }
                break;
            }

            // Move forward
            if result.column.0 < cols - 1 {
                result.column = result.column + 1;
            } else if result.line < self.bottommost_line() {
                result.line = result.line + 1;
                result.column = Column(0);
            } else {
                // Reached the end
                break;
            }
        }

        result
    }

    /// Find matching bracket for the bracket at the given point.
    ///
    /// Supports: () [] {} <>
    /// Returns None if no bracket at point or no matching bracket found.
    #[must_use]
    pub fn bracket_match(&self, point: Point) -> Option<Point> {
        let cell = self.cell(point)?;
        let ch = cell.char();

        // Determine if opening or closing bracket and its pair
        let (pair, forward) = match ch {
            '(' => (')', true),
            ')' => ('(', false),
            '[' => (']', true),
            ']' => ('[', false),
            '{' => ('}', true),
            '}' => ('{', false),
            '<' => ('>', true),
            '>' => ('<', false),
            _ => return None,
        };

        let cols = self.terminal.grid().cols() as usize;
        let mut depth = 1;
        let mut current = point;

        // Search for matching bracket
        loop {
            // Move in the appropriate direction
            if forward {
                if current.column.0 < cols - 1 {
                    current.column = current.column + 1;
                } else if current.line < self.bottommost_line() {
                    current.line = current.line + 1;
                    current.column = Column(0);
                } else {
                    return None; // Reached end without finding match
                }
            } else if current.column.0 > 0 {
                current.column = current.column - 1;
            } else if current.line > self.topmost_line() {
                current.line = current.line - 1;
                current.column = Column(cols - 1);
            } else {
                return None; // Reached start without finding match
            }

            // Check character at current position
            if let Some(c) = self.cell(current) {
                let curr_ch = c.char();
                if curr_ch == ch {
                    depth += 1;
                } else if curr_ch == pair {
                    depth -= 1;
                    if depth == 0 {
                        return Some(current);
                    }
                }
            }
        }
    }

    // ===== Vi Mode Navigation Helpers =====

    /// Check if a character is a semantic word separator.
    fn is_separator(&self, ch: char) -> bool {
        let separators = self.semantic_escape_chars();
        ch == ' ' || ch == '\0' || separators.contains(ch)
    }

    /// Check if a character is whitespace (for WORD motions).
    fn is_whitespace(ch: char) -> bool {
        ch == ' ' || ch == '\t' || ch == '\0'
    }

    /// Get character at a point, returning space for out-of-bounds.
    fn char_at(&self, point: Point) -> char {
        self.cell(point).map(|c| c.char()).unwrap_or(' ')
    }

    /// Move point forward by one position, wrapping to next line.
    fn point_forward(&self, point: Point) -> Option<Point> {
        let cols = self.columns();
        if point.column.0 < cols - 1 {
            Some(Point::new(point.line, point.column + 1))
        } else if point.line < self.bottommost_line() {
            Some(Point::new(point.line + 1, Column(0)))
        } else {
            None
        }
    }

    /// Move point backward by one position, wrapping to previous line.
    fn point_backward(&self, point: Point) -> Option<Point> {
        let cols = self.columns();
        if point.column.0 > 0 {
            Some(Point::new(point.line, point.column - 1))
        } else if point.line > self.topmost_line() {
            Some(Point::new(point.line - 1, Column(cols - 1)))
        } else {
            None
        }
    }

    /// Move to start of next semantic word (vim 'w').
    #[must_use]
    fn semantic_word_right(&self, point: Point) -> Point {
        let mut current = point;

        // Skip current word (non-separators)
        while !self.is_separator(self.char_at(current)) {
            match self.point_forward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Skip separators/spaces
        while self.is_separator(self.char_at(current)) {
            match self.point_forward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        current
    }

    /// Move to start of previous semantic word (vim 'b').
    #[must_use]
    fn semantic_word_left(&self, point: Point) -> Point {
        let mut current = point;

        // Move back one first if not at start
        if let Some(p) = self.point_backward(current) {
            current = p;
        } else {
            return current;
        }

        // Skip separators/spaces going backward
        while self.is_separator(self.char_at(current)) {
            match self.point_backward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Skip word characters going backward until we hit a separator
        while !self.is_separator(self.char_at(current)) {
            match self.point_backward(current) {
                Some(p) => {
                    if self.is_separator(self.char_at(p)) {
                        // We're at the start of the word
                        return current;
                    }
                    current = p;
                }
                None => return current,
            }
        }

        // Move forward to the first non-separator
        if let Some(p) = self.point_forward(current) {
            current = p;
        }

        current
    }

    /// Move to end of current/next semantic word (vim 'e').
    #[must_use]
    fn semantic_word_right_end(&self, point: Point) -> Point {
        let mut current = point;

        // Move forward one first
        if let Some(p) = self.point_forward(current) {
            current = p;
        } else {
            return current;
        }

        // Skip separators/spaces
        while self.is_separator(self.char_at(current)) {
            match self.point_forward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Skip word characters until we hit a separator
        loop {
            match self.point_forward(current) {
                Some(p) => {
                    if self.is_separator(self.char_at(p)) {
                        // We're at the end of the word
                        return current;
                    }
                    current = p;
                }
                None => return current,
            }
        }
    }

    /// Move to end of previous semantic word (vim 'ge').
    #[must_use]
    fn semantic_word_left_end(&self, point: Point) -> Point {
        let mut current = point;

        // Move back one first
        if let Some(p) = self.point_backward(current) {
            current = p;
        } else {
            return current;
        }

        // Skip current word characters going backward (if we're in a word)
        while !self.is_separator(self.char_at(current)) {
            match self.point_backward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Skip separators/spaces going backward
        while self.is_separator(self.char_at(current)) {
            match self.point_backward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Now we're at the end of the previous word
        current
    }

    /// Move to start of next whitespace-separated word (vim 'W').
    #[must_use]
    fn whitespace_word_right(&self, point: Point) -> Point {
        let mut current = point;

        // Skip current word (non-whitespace)
        while !Self::is_whitespace(self.char_at(current)) {
            match self.point_forward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Skip whitespace
        while Self::is_whitespace(self.char_at(current)) {
            match self.point_forward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        current
    }

    /// Move to start of previous whitespace-separated word (vim 'B').
    #[must_use]
    fn whitespace_word_left(&self, point: Point) -> Point {
        let mut current = point;

        // Move back one first if not at start
        if let Some(p) = self.point_backward(current) {
            current = p;
        } else {
            return current;
        }

        // Skip whitespace going backward
        while Self::is_whitespace(self.char_at(current)) {
            match self.point_backward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Skip non-whitespace going backward until we hit whitespace
        while !Self::is_whitespace(self.char_at(current)) {
            match self.point_backward(current) {
                Some(p) => {
                    if Self::is_whitespace(self.char_at(p)) {
                        return current;
                    }
                    current = p;
                }
                None => return current,
            }
        }

        // Move forward to the first non-whitespace
        if let Some(p) = self.point_forward(current) {
            current = p;
        }

        current
    }

    /// Move to end of current/next whitespace-separated word (vim 'E').
    #[must_use]
    fn whitespace_word_right_end(&self, point: Point) -> Point {
        let mut current = point;

        // Move forward one first
        if let Some(p) = self.point_forward(current) {
            current = p;
        } else {
            return current;
        }

        // Skip whitespace
        while Self::is_whitespace(self.char_at(current)) {
            match self.point_forward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Skip non-whitespace until we hit whitespace
        loop {
            match self.point_forward(current) {
                Some(p) => {
                    if Self::is_whitespace(self.char_at(p)) {
                        return current;
                    }
                    current = p;
                }
                None => return current,
            }
        }
    }

    /// Move to end of previous whitespace-separated word (vim 'gE').
    #[must_use]
    fn whitespace_word_left_end(&self, point: Point) -> Point {
        let mut current = point;

        // Move back one first
        if let Some(p) = self.point_backward(current) {
            current = p;
        } else {
            return current;
        }

        // Skip current word characters going backward (if we're in a word)
        while !Self::is_whitespace(self.char_at(current)) {
            match self.point_backward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Skip whitespace going backward
        while Self::is_whitespace(self.char_at(current)) {
            match self.point_backward(current) {
                Some(p) => current = p,
                None => return current,
            }
        }

        // Now we're at the end of the previous WORD
        current
    }

    /// Find first non-empty cell in a line (vim '^').
    #[must_use]
    fn first_occupied(&self, line: Line) -> Point {
        let cols = self.columns();
        for col in 0..cols {
            let point = Point::new(line, Column(col));
            let ch = self.char_at(point);
            if ch != ' ' && ch != '\t' && ch != '\0' {
                return point;
            }
        }
        // If all empty, return column 0
        Point::new(line, Column(0))
    }

    /// Move up to empty line (vim '{').
    #[must_use]
    fn paragraph_up(&self, point: Point) -> Point {
        let mut current_line = point.line;
        let topmost = self.topmost_line();

        // Move up at least one line
        if current_line > topmost {
            current_line = current_line - 1;
        } else {
            return Point::new(topmost, Column(0));
        }

        // Skip non-empty lines
        while current_line > topmost {
            if self.is_line_empty(current_line) {
                return Point::new(current_line, Column(0));
            }
            current_line = current_line - 1;
        }

        Point::new(topmost, Column(0))
    }

    /// Move down to empty line (vim '}').
    #[must_use]
    fn paragraph_down(&self, point: Point) -> Point {
        let mut current_line = point.line;
        let bottommost = self.bottommost_line();

        // Move down at least one line
        if current_line < bottommost {
            current_line = current_line + 1;
        } else {
            return Point::new(bottommost, Column(0));
        }

        // Skip non-empty lines
        while current_line < bottommost {
            if self.is_line_empty(current_line) {
                return Point::new(current_line, Column(0));
            }
            current_line = current_line + 1;
        }

        Point::new(bottommost, Column(0))
    }

    /// Check if a line is empty (all spaces/nulls).
    fn is_line_empty(&self, line: Line) -> bool {
        let cols = self.columns();
        for col in 0..cols {
            let point = Point::new(line, Column(col));
            let ch = self.char_at(point);
            if ch != ' ' && ch != '\t' && ch != '\0' {
                return false;
            }
        }
        true
    }

    /// Get the text content of a line as a String.
    ///
    /// Returns `None` if the line is out of bounds.
    fn line_text(&self, line: Line) -> Option<String> {
        let grid = self.terminal.grid();
        let rows = grid.rows() as i32;

        // Only handle visible lines for now (line.0 >= 0)
        if line.0 < 0 || line.0 >= rows {
            return None;
        }

        let row_idx = line.0 as u16;
        let cols = grid.cols() as usize;
        let mut text = String::with_capacity(cols);

        for col in 0..cols {
            if let Some(cell) = grid.cell(row_idx, col as u16) {
                let ch = cell.char();
                if ch == '\0' {
                    text.push(' ');
                } else {
                    text.push(ch);
                }
            } else {
                text.push(' ');
            }
        }

        Some(text)
    }

    /// Get all visible OSC 8 hyperlink ranges.
    ///
    /// Returns matches in order from top-left to bottom-right.
    fn visible_hyperlinks(&self) -> Vec<crate::url::UrlMatch> {
        let grid = self.terminal.grid();
        let extras = grid.extras();
        let rows = grid.rows();
        let cols = grid.cols();
        let mut matches = Vec::new();

        for row in 0..rows {
            let line = Line(row as i32);
            let mut col = 0u16;

            while col < cols {
                let coord = CellCoord::new(row, col);
                let hyperlink = extras
                    .get(coord)
                    .and_then(|extra| extra.hyperlink())
                    .map(|url| url.as_ref());

                if let Some(url) = hyperlink {
                    let start_col = col;
                    let mut end_col = col;
                    col += 1;

                    while col < cols {
                        let coord = CellCoord::new(row, col);
                        let next_url = extras
                            .get(coord)
                            .and_then(|extra| extra.hyperlink())
                            .map(|next| next.as_ref());
                        if next_url != Some(url) {
                            break;
                        }
                        end_col = col;
                        col += 1;
                    }

                    matches.push(crate::url::UrlMatch {
                        start: Point::new(line, Column(start_col as usize)),
                        end: Point::new(line, Column(end_col as usize)),
                        url: url.to_string(),
                    });
                    continue;
                }

                col += 1;
            }
        }

        matches
    }

    /// Navigate to the next URL in the terminal.
    ///
    /// Returns the URL that was navigated to, if any.
    pub fn vi_goto_next_url(&mut self) -> Option<crate::url::UrlMatch> {
        if !self.vi_mode {
            return None;
        }

        let current_point = self.vi_mode_cursor.point;
        let url = crate::url::find_next_url(self, current_point, |line| self.line_text(line));

        if let Some(ref url) = url {
            self.vi_mode_cursor.point = url.start;
        }

        url
    }

    /// Navigate to the previous URL in the terminal.
    ///
    /// Returns the URL that was navigated to, if any.
    pub fn vi_goto_prev_url(&mut self) -> Option<crate::url::UrlMatch> {
        if !self.vi_mode {
            return None;
        }

        let current_point = self.vi_mode_cursor.point;
        let url = crate::url::find_prev_url(self, current_point, |line| self.line_text(line));

        if let Some(ref url) = url {
            self.vi_mode_cursor.point = url.start;
        }

        url
    }

    /// Get the URL at the current vi cursor position, if any.
    pub fn url_at_vi_cursor(&self) -> Option<crate::url::UrlMatch> {
        if !self.vi_mode {
            return None;
        }

        crate::url::url_at_point(self, self.vi_mode_cursor.point, |line| self.line_text(line))
    }

    // --- Hint Mode Support ---

    /// Get all visible URLs for hint mode.
    ///
    /// Returns URL and OSC 8 hyperlink matches in order from top-left to bottom-right.
    pub fn visible_urls(&self) -> Vec<crate::url::UrlMatch> {
        let mut matches = crate::url::find_urls(
            self.topmost_line(),
            self.bottommost_line(),
            self.columns(),
            |line| self.line_text(line),
        );

        for hyperlink in self.visible_hyperlinks() {
            let duplicate = matches.iter().any(|existing| {
                existing.start == hyperlink.start
                    && existing.end == hyperlink.end
                    && existing.url == hyperlink.url
            });
            if !duplicate {
                matches.push(hyperlink);
            }
        }

        matches.sort_by(|a, b| {
            a.start
                .line
                .cmp(&b.start.line)
                .then(a.start.column.cmp(&b.start.column))
                .then(a.end.line.cmp(&b.end.line))
                .then(a.end.column.cmp(&b.end.column))
                .then(a.url.cmp(&b.url))
        });

        matches
    }

    /// Create a hint state populated with visible URLs.
    ///
    /// This is a convenience method for entering hint mode.
    pub fn create_hint_state(&self) -> crate::hints::HintState {
        let mut state = crate::hints::HintState::new();
        state.set_urls(self.visible_urls());
        state
    }

    /// Create a hint state with a custom alphabet.
    pub fn create_hint_state_with_alphabet(&self, alphabet: &str) -> crate::hints::HintState {
        let mut state = crate::hints::HintState::with_alphabet(alphabet);
        state.set_urls(self.visible_urls());
        state
    }

    /// Expand selection to word boundaries.
    ///
    /// Used for double-click word selection.
    pub fn expand_selection_to_word(&mut self, point: Point) {
        let start = self.word_start(point);
        let end = self.word_end(point);

        self.selection = Some(Selection::new(SelectionType::Semantic, start, Side::Left));
        if let Some(sel) = &mut self.selection {
            sel.update(end, Side::Right);
        }
    }

    /// Expand selection to line boundaries.
    ///
    /// Used for triple-click line selection.
    pub fn expand_selection_to_line(&mut self, line: Line) {
        let cols = self.terminal.grid().cols() as usize;
        let start = Point::new(line, Column(0));
        let end = Point::new(line, Column(cols.saturating_sub(1)));

        self.selection = Some(Selection::new(SelectionType::Lines, start, Side::Left));
        if let Some(sel) = &mut self.selection {
            sel.update(end, Side::Right);
        }
    }
}

impl<T: EventListener> Term<T> {
    /// Signal the terminal to exit.
    ///
    /// This sends an [`Event::Exit`] to the event listener, which typically
    /// triggers the application to close the terminal window or tab.
    ///
    /// # Example
    ///
    /// This is typically called when the shell or application running in
    /// the terminal exits, or when the user requests to close the terminal.
    #[inline]
    pub fn exit(&mut self) {
        self.event_proxy.send_event(Event::Exit);
    }
}

impl<T: EventListener + Clone + Send + Sync + 'static> Term<T> {
    /// Create a new terminal wrapper.
    pub fn new<D: Dimensions>(config: Config, dimensions: &D, event_proxy: T) -> Self {
        let rows = dimensions.screen_lines().max(1) as u16;
        let cols = dimensions.columns().max(1) as u16;
        let mut terminal = Terminal::new(rows, cols);

        let title_proxy = event_proxy.clone();
        terminal.set_title_callback(move |title| {
            if title.is_empty() {
                title_proxy.send_event(Event::ResetTitle);
            } else {
                title_proxy.send_event(Event::Title(title.to_string()));
            }
        });

        let bell_proxy = event_proxy.clone();
        terminal.set_bell_callback(move || {
            bell_proxy.send_event(Event::Bell);
        });

        Self {
            terminal,
            config,
            event_proxy,
            is_focused: true,
            selection: None,
            vi_mode_cursor: ViModeCursor::default(),
            vi_mode: false,
            inline_search: None,
            search_state: TermSearch::new(),
            marks: ViMarks::new(),
        }
    }
}

impl<T> Dimensions for Term<T> {
    fn total_lines(&self) -> usize {
        self.terminal.grid().total_lines()
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.terminal.grid().rows())
    }

    fn columns(&self) -> usize {
        usize::from(self.terminal.grid().cols())
    }
}
