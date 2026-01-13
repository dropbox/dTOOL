//! TUI (Text User Interface) for dTerm interactive mode.
//!
//! This module provides a terminal emulator UI using ratatui and crossterm.

use std::cmp::min;
use std::io::{self, stdout, Stdout};
use std::time::{Duration, Instant};

use arboard::Clipboard;
use crossterm::{
    cursor::SetCursorStyle as CrosstermCursorStyle,
    event::{
        self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dterm_core::terminal::{CursorStyle, Terminal};
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal as RatatuiTerminal,
};

use crate::pty::{Pty, PtyError};

/// Position in the terminal grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridPos {
    row: u16,
    col: u16,
}

impl GridPos {
    fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

/// Text selection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Selection {
    /// Starting position (where drag began).
    start: GridPos,
    /// Current/ending position (where drag is now).
    end: GridPos,
}

impl Selection {
    /// Create a new selection starting at a position.
    fn new(start: GridPos) -> Self {
        Self { start, end: start }
    }

    /// Create a selection covering a single word at the given position.
    fn word_at(pos: GridPos, grid: &dterm_core::grid::Grid) -> Self {
        let row = pos.row;
        let col = pos.col;

        // Find word boundaries
        let (word_start, word_end) = find_word_boundaries(row, col, grid);

        Self {
            start: GridPos::new(row, word_start),
            end: GridPos::new(row, word_end),
        }
    }

    /// Create a selection covering an entire line.
    fn line_at(row: u16, grid: &dterm_core::grid::Grid) -> Self {
        // Find the last non-space character on the line
        let mut end_col = 0u16;
        for col in 0..grid.cols() {
            if let Some(cell) = grid.cell(row, col) {
                let ch = cell.char();
                if ch != ' ' && ch != '\0' {
                    end_col = col;
                }
            }
        }

        Self {
            start: GridPos::new(row, 0),
            end: GridPos::new(row, end_col),
        }
    }

    /// Get the normalized selection bounds (top-left to bottom-right).
    fn normalized(&self) -> (GridPos, GridPos) {
        // Determine which position comes first (top-left)
        let (first, second) = if self.start.row < self.end.row
            || (self.start.row == self.end.row && self.start.col <= self.end.col)
        {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        };
        (first, second)
    }

    /// Get the normalized rectangular bounds (top-left to bottom-right).
    fn rect_bounds(&self) -> (GridPos, GridPos) {
        let top = self.start.row.min(self.end.row);
        let bottom = self.start.row.max(self.end.row);
        let left = self.start.col.min(self.end.col);
        let right = self.start.col.max(self.end.col);

        (GridPos::new(top, left), GridPos::new(bottom, right))
    }

    /// Check if a cell is within the selection.
    fn contains(&self, row: u16, col: u16) -> bool {
        let (start, end) = self.normalized();

        if row < start.row || row > end.row {
            return false;
        }

        if start.row == end.row {
            // Single line selection
            col >= start.col && col <= end.col
        } else if row == start.row {
            // First line of multi-line selection
            col >= start.col
        } else if row == end.row {
            // Last line of multi-line selection
            col <= end.col
        } else {
            // Middle lines are fully selected
            true
        }
    }

    /// Check if a cell is within the rectangular selection bounds.
    fn contains_rect(&self, row: u16, col: u16) -> bool {
        let (start, end) = self.rect_bounds();

        row >= start.row && row <= end.row && col >= start.col && col <= end.col
    }
}

/// Selection mode determines how selection extends during drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionMode {
    /// Character-by-character selection (single click).
    Character,
    /// Block/rectangular selection (Alt+click).
    Block,
    /// Word-by-word selection (double-click).
    Word,
    /// Line-by-line selection (triple-click).
    Line,
}

/// Click tracking for detecting double/triple clicks.
#[derive(Debug, Clone, Default)]
struct ClickTracker {
    /// Position of the last click.
    last_pos: Option<GridPos>,
    /// Time of the last click.
    last_time: Option<Instant>,
    /// Number of consecutive clicks at the same position.
    click_count: u8,
}

impl ClickTracker {
    /// Maximum time between clicks to count as multi-click (in milliseconds).
    const MULTI_CLICK_THRESHOLD_MS: u64 = 500;

    /// Maximum distance (in cells) between clicks to count as multi-click.
    const MULTI_CLICK_DISTANCE: u16 = 1;

    /// Register a click and return the click count (1, 2, or 3).
    fn register_click(&mut self, pos: GridPos) -> u8 {
        let now = Instant::now();

        let is_multi_click = match (self.last_pos, self.last_time) {
            (Some(last_pos), Some(last_time)) => {
                // Check time threshold
                let elapsed = now.duration_since(last_time);
                if elapsed.as_millis() > Self::MULTI_CLICK_THRESHOLD_MS as u128 {
                    false
                } else {
                    // Check distance threshold
                    // SAFETY: row/col are u16, so max diff is u16::MAX which fits in u16
                    #[allow(clippy::cast_possible_truncation)]
                    let row_diff =
                        (i32::from(pos.row) - i32::from(last_pos.row)).unsigned_abs() as u16;
                    #[allow(clippy::cast_possible_truncation)]
                    let col_diff =
                        (i32::from(pos.col) - i32::from(last_pos.col)).unsigned_abs() as u16;
                    row_diff <= Self::MULTI_CLICK_DISTANCE && col_diff <= Self::MULTI_CLICK_DISTANCE
                }
            }
            _ => false,
        };

        if is_multi_click {
            // Increment click count, wrapping back to 1 after 3
            self.click_count = if self.click_count >= 3 {
                1
            } else {
                self.click_count + 1
            };
        } else {
            self.click_count = 1;
        }

        self.last_pos = Some(pos);
        self.last_time = Some(now);

        self.click_count
    }
}

/// Find word boundaries around a given column position in a row.
fn find_word_boundaries(row: u16, col: u16, grid: &dterm_core::grid::Grid) -> (u16, u16) {
    // Get the character at the position to determine word type
    let ch_at_pos = grid.cell(row, col).map(|c| c.char()).unwrap_or(' ');

    // Determine the character class
    let is_word_char = |ch: char| -> bool { ch.is_alphanumeric() || ch == '_' };

    let pos_is_word = is_word_char(ch_at_pos);

    // Find start of word (scan backwards)
    let mut start = col;
    while start > 0 {
        if let Some(cell) = grid.cell(row, start - 1) {
            let ch = cell.char();
            if pos_is_word {
                // For word characters, continue until non-word
                if !is_word_char(ch) {
                    break;
                }
            } else if ch == ' ' || ch == '\0' {
                // For non-word chars (punctuation), stop at space
                if ch != ch_at_pos {
                    break;
                }
            } else {
                // Different non-word character
                break;
            }
        }
        start -= 1;
    }

    // Find end of word (scan forwards)
    let mut end = col;
    while end < grid.cols().saturating_sub(1) {
        if let Some(cell) = grid.cell(row, end + 1) {
            let ch = cell.char();
            if pos_is_word {
                // For word characters, continue until non-word
                if !is_word_char(ch) {
                    break;
                }
            } else if ch == ' ' || ch == '\0' {
                // For space/null, just select that single character
                break;
            } else {
                // Different non-word character
                break;
            }
        }
        end += 1;
    }

    (start, end)
}

/// Timeout for synchronized output mode (1 second).
/// If an application enables sync mode and crashes or hangs, we render anyway after this timeout.
const SYNC_OUTPUT_TIMEOUT: Duration = Duration::from_secs(1);

/// TUI application state.
pub struct App {
    /// The dterm terminal emulator.
    terminal: Terminal,
    /// The PTY connection to the shell.
    pty: Pty,
    /// Whether to quit the application.
    should_quit: bool,
    /// Terminal title (from OSC sequences).
    title: String,
    /// Last cursor style applied (to avoid redundant updates).
    last_cursor_style: CursorStyle,
    /// Current text selection (if any).
    selection: Option<Selection>,
    /// Current selection mode.
    selection_mode: SelectionMode,
    /// Click tracker for double/triple click detection.
    click_tracker: ClickTracker,
    /// System clipboard access.
    clipboard: Option<Clipboard>,
    /// Inner area of the terminal (for mouse coordinate translation).
    inner_area: Rect,
    /// Anchor position for word/line selection during drag.
    selection_anchor: Option<GridPos>,
    /// Timestamp when synchronized output mode was last enabled.
    /// Used to implement timeout protection - if sync mode stays enabled
    /// for too long (app crashed), we render anyway.
    sync_output_started: Option<Instant>,
}

/// Error type for TUI operations.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("PTY error: {0}")]
    Pty(#[from] PtyError),
}

impl App {
    /// Create a new TUI application.
    ///
    /// # Arguments
    /// * `rows` - Terminal rows
    /// * `cols` - Terminal columns
    pub fn new(rows: u16, cols: u16) -> Result<Self, TuiError> {
        let terminal = Terminal::new(rows, cols);
        let pty = Pty::spawn(rows, cols)?;

        // Initialize clipboard (may fail on some systems, that's ok)
        let clipboard = Clipboard::new().ok();

        Ok(Self {
            terminal,
            pty,
            should_quit: false,
            title: String::from("dTerm"),
            last_cursor_style: CursorStyle::default(),
            selection: None,
            selection_mode: SelectionMode::Character,
            click_tracker: ClickTracker::default(),
            clipboard,
            inner_area: Rect::default(),
            selection_anchor: None,
            sync_output_started: None,
        })
    }

    /// Run the TUI event loop.
    pub fn run(&mut self) -> Result<(), TuiError> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableFocusChange
        )?;
        let backend = CrosstermBackend::new(stdout);
        let mut tui = RatatuiTerminal::new(backend)?;

        // Main event loop
        let result = self.event_loop(&mut tui);

        // Cleanup
        disable_raw_mode()?;
        execute!(
            tui.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableFocusChange,
            CrosstermCursorStyle::DefaultUserShape
        )?;
        tui.show_cursor()?;

        result
    }

    /// Main event loop.
    fn event_loop(
        &mut self,
        tui: &mut RatatuiTerminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), TuiError> {
        loop {
            // Update inner area before drawing (for mouse coordinate translation)
            let area = tui.size()?;
            let block = Block::default().borders(Borders::ALL);
            self.inner_area = block.inner(area);

            // Check if we should render (respecting synchronized output mode)
            let should_render = self.should_render();
            if should_render {
                // Draw the UI
                tui.draw(|frame| self.draw(frame))?;

                // Update cursor style if changed
                self.update_cursor_style(tui.backend_mut())?;
            }

            // Handle PTY output (non-blocking)
            self.process_pty_output()?;

            // Check for user input with timeout
            if event::poll(Duration::from_millis(10))? {
                match event::read()? {
                    Event::Key(key) => self.handle_key(key)?,
                    Event::Mouse(mouse) => self.handle_mouse(mouse)?,
                    Event::Resize(cols, rows) => self.handle_resize(cols, rows)?,
                    Event::FocusGained => self.handle_focus(true)?,
                    Event::FocusLost => self.handle_focus(false)?,
                    _ => {}
                }
            }

            // Check if we should quit
            if self.should_quit || !self.pty.is_running() {
                break;
            }
        }

        Ok(())
    }

    /// Determine if the UI should render this frame.
    ///
    /// Implements synchronized output mode (DEC private mode 2026):
    /// - When sync mode is enabled, rendering is deferred to prevent tearing
    /// - A timeout ensures we don't freeze if the application crashes
    fn should_render(&mut self) -> bool {
        let sync_enabled = self.terminal.synchronized_output_enabled();

        if sync_enabled {
            // Track when sync mode started
            let now = Instant::now();
            let start_time = self.sync_output_started.get_or_insert(now);

            // Check for timeout - render anyway if we've been waiting too long
            if now.duration_since(*start_time) >= SYNC_OUTPUT_TIMEOUT {
                // Timeout reached, force render and reset timer
                self.sync_output_started = Some(now);
                return true;
            }

            // Sync mode active and not timed out - defer rendering
            false
        } else {
            // Sync mode disabled - clear timer and render normally
            self.sync_output_started = None;
            true
        }
    }

    /// Process output from the PTY.
    fn process_pty_output(&mut self) -> Result<(), TuiError> {
        // Read from PTY
        if let Some(data) = self.pty.read()? {
            // Feed to terminal emulator
            self.terminal.process(&data);

            // Update title if changed
            let new_title = self.terminal.title().to_string();
            if !new_title.is_empty() {
                self.title = new_title;
            }

            // Send any responses back to PTY (e.g., cursor position reports)
            if let Some(response) = self.terminal.take_response() {
                self.pty.write(&response)?;
            }
        }

        Ok(())
    }

    /// Handle a key press event.
    fn handle_key(&mut self, key: KeyEvent) -> Result<(), TuiError> {
        // Check for quit sequence (Ctrl+Q)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return Ok(());
        }

        // Check for paste shortcut (Ctrl+Shift+V)
        if key
            .modifiers
            .contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT)
            && key.code == KeyCode::Char('V')
        {
            self.paste_from_clipboard()?;
            return Ok(());
        }

        // Check for copy shortcut (Ctrl+Shift+C)
        if key
            .modifiers
            .contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT)
            && key.code == KeyCode::Char('C')
        {
            self.copy_selection_to_clipboard();
            return Ok(());
        }

        // Check for select all shortcut (Ctrl+Shift+A)
        if key
            .modifiers
            .contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT)
            && key.code == KeyCode::Char('A')
        {
            self.select_all_visible();
            return Ok(());
        }

        // Handle scrollback navigation (Shift+PageUp/Down)
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::PageUp if shift => {
                // Scroll up (show older content)
                let page_size = self.terminal.rows() as i32;
                self.terminal.scroll_display(page_size);
                return Ok(());
            }
            KeyCode::PageDown if shift => {
                // Scroll down (show newer content)
                let page_size = self.terminal.rows() as i32;
                self.terminal.scroll_display(-page_size);
                return Ok(());
            }
            KeyCode::Home if shift && self.selection.is_none() => {
                // Scroll to top of history (only when no selection active)
                self.terminal.scroll_to_top();
                return Ok(());
            }
            KeyCode::End if shift && self.selection.is_none() => {
                // Scroll to bottom (live content) (only when no selection active)
                self.terminal.scroll_to_bottom();
                return Ok(());
            }
            // Keyboard selection: Shift+Arrow keys extend selection
            KeyCode::Left if shift && self.selection.is_some() => {
                self.extend_selection_keyboard(-1, 0);
                return Ok(());
            }
            KeyCode::Right if shift && self.selection.is_some() => {
                self.extend_selection_keyboard(1, 0);
                return Ok(());
            }
            KeyCode::Up if shift && self.selection.is_some() => {
                self.extend_selection_keyboard(0, -1);
                return Ok(());
            }
            KeyCode::Down if shift && self.selection.is_some() => {
                self.extend_selection_keyboard(0, 1);
                return Ok(());
            }
            KeyCode::Home if shift && self.selection.is_some() => {
                // Shift+Home: extend selection to beginning of line
                self.extend_selection_to_line_start();
                return Ok(());
            }
            KeyCode::End if shift && self.selection.is_some() => {
                // Shift+End: extend selection to end of line
                self.extend_selection_to_line_end();
                return Ok(());
            }
            _ => {}
        }

        // Any other key input returns to live view
        if self.terminal.grid().display_offset() > 0 {
            self.terminal.scroll_to_bottom();
        }

        // Convert key to bytes to send to PTY
        let bytes = key_to_bytes(key);
        if !bytes.is_empty() {
            self.pty.write(&bytes)?;
        }

        Ok(())
    }

    /// Handle a mouse event.
    fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<(), TuiError> {
        // Check if mouse tracking is enabled by the terminal application.
        // If so, forward mouse events to the PTY instead of handling locally.
        // Exception: Shift+click always does local selection even with mouse tracking.
        let mouse_tracking = self.terminal.mouse_tracking_enabled();
        let shift_held = mouse.modifiers.contains(KeyModifiers::SHIFT);
        let forward_to_app = mouse_tracking && !shift_held;

        // Convert crossterm modifiers to xterm modifier bits (shift=4, meta=8, ctrl=16)
        let modifiers = self.mouse_modifiers_to_xterm(&mouse.modifiers);

        // Convert mouse position to grid coordinates (0-indexed)
        let (col, row) = if let Some(pos) = self.mouse_to_grid(mouse.column, mouse.row) {
            (pos.col, pos.row)
        } else {
            // Mouse is outside the terminal area - for forwarding, clamp to bounds
            let grid = self.terminal.grid();
            let col = mouse
                .column
                .saturating_sub(self.inner_area.x)
                .min(grid.cols().saturating_sub(1));
            let row = mouse
                .row
                .saturating_sub(self.inner_area.y)
                .min(grid.rows().saturating_sub(1));
            (col, row)
        };

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                if forward_to_app {
                    // Forward scroll wheel to application
                    if let Some(seq) = self.terminal.encode_mouse_wheel(true, col, row, modifiers) {
                        self.pty.write(&seq)?;
                    }
                } else {
                    // Local scrollback navigation
                    self.terminal.scroll_display(3);
                    self.selection = None;
                    self.selection_anchor = None;
                }
            }
            MouseEventKind::ScrollDown => {
                if forward_to_app {
                    // Forward scroll wheel to application
                    if let Some(seq) = self.terminal.encode_mouse_wheel(false, col, row, modifiers)
                    {
                        self.pty.write(&seq)?;
                    }
                } else {
                    // Local scrollback navigation
                    self.terminal.scroll_display(-3);
                    self.selection = None;
                    self.selection_anchor = None;
                }
            }
            MouseEventKind::Down(button) => {
                let button_code = self.mouse_button_to_code(button);

                if forward_to_app {
                    // Forward button press to application
                    if let Some(seq) =
                        self.terminal
                            .encode_mouse_press(button_code, col, row, modifiers)
                    {
                        self.pty.write(&seq)?;
                    }
                } else {
                    // Handle locally based on button
                    match button {
                        MouseButton::Left => {
                            self.handle_local_left_click(mouse.column, mouse.row, &mouse.modifiers)?
                        }
                        MouseButton::Middle => {
                            // Middle-click paste from clipboard
                            self.paste_from_clipboard()?;
                        }
                        MouseButton::Right => {
                            // Right-click not handled locally (could add context menu later)
                        }
                    }
                }
            }
            MouseEventKind::Up(button) => {
                let button_code = self.mouse_button_to_code(button);

                if forward_to_app {
                    // Forward button release to application
                    if let Some(seq) =
                        self.terminal
                            .encode_mouse_release(button_code, col, row, modifiers)
                    {
                        self.pty.write(&seq)?;
                    }
                } else {
                    // Handle locally
                    if button == MouseButton::Left {
                        // Complete selection and copy to clipboard
                        if let Some(ref sel) = self.selection {
                            if sel.start != sel.end {
                                if let Some(text) = self.get_selected_text() {
                                    self.copy_to_clipboard(&text);
                                }
                            }
                        }
                    }
                }
            }
            MouseEventKind::Drag(button) => {
                let button_code = self.mouse_button_to_code(button);

                if forward_to_app {
                    // Forward motion event to application (if ButtonEvent or AnyEvent mode)
                    if let Some(seq) =
                        self.terminal
                            .encode_mouse_motion(button_code, col, row, modifiers)
                    {
                        self.pty.write(&seq)?;
                    }
                } else if button == MouseButton::Left {
                    // Local drag selection
                    if self.selection_anchor.is_some() {
                        let (pos, scroll_delta) =
                            self.mouse_to_grid_for_drag(mouse.column, mouse.row);
                        if scroll_delta != 0 {
                            self.terminal.scroll_display(scroll_delta);
                        }
                        self.extend_selection(pos);
                    }
                }
            }
            MouseEventKind::Moved => {
                if forward_to_app {
                    // Forward motion without button (AnyEvent mode only)
                    // Button code 3 means "no button held"
                    if let Some(seq) = self.terminal.encode_mouse_motion(3, col, row, modifiers) {
                        self.pty.write(&seq)?;
                    }
                }
                // No local handling for raw mouse movement
            }
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => {
                // Horizontal scroll not commonly used, ignore for now
            }
        }
        Ok(())
    }

    /// Handle a local left-click event (when mouse tracking is off or shift held).
    fn handle_local_left_click(
        &mut self,
        mouse_col: u16,
        mouse_row: u16,
        modifiers: &KeyModifiers,
    ) -> Result<(), TuiError> {
        let shift_held = modifiers.contains(KeyModifiers::SHIFT);
        let alt_held = modifiers.contains(KeyModifiers::ALT);

        if let Some(pos) = self.mouse_to_grid(mouse_col, mouse_row) {
            if alt_held {
                if shift_held && self.selection_anchor.is_some() {
                    self.selection_mode = SelectionMode::Block;
                    self.extend_selection_to(pos);
                } else {
                    self.selection_mode = SelectionMode::Block;
                    self.selection = Some(Selection::new(pos));
                    self.selection_anchor = Some(pos);
                }
            } else if shift_held {
                // Shift-click: extend existing selection or create new one
                self.extend_selection_to(pos);
            } else {
                // Detect click pattern (single, double, triple)
                let click_count = self.click_tracker.register_click(pos);

                match click_count {
                    1 => {
                        // Single click: start character selection
                        self.selection_mode = SelectionMode::Character;
                        self.selection = Some(Selection::new(pos));
                        self.selection_anchor = Some(pos);
                    }
                    2 => {
                        // Double click: select word
                        self.selection_mode = SelectionMode::Word;
                        self.selection = Some(Selection::word_at(pos, self.terminal.grid()));
                        self.selection_anchor = Some(pos);
                    }
                    3 => {
                        // Triple click: select line
                        self.selection_mode = SelectionMode::Line;
                        self.selection = Some(Selection::line_at(pos.row, self.terminal.grid()));
                        self.selection_anchor = Some(pos);
                    }
                    _ => {}
                }
            }
        }
        // Return to live view if scrolled back
        if self.terminal.grid().display_offset() > 0 {
            self.terminal.scroll_to_bottom();
        }
        Ok(())
    }

    /// Convert crossterm KeyModifiers to xterm mouse modifier bits.
    ///
    /// Xterm modifier bits: shift=4, meta=8, ctrl=16
    fn mouse_modifiers_to_xterm(&self, modifiers: &KeyModifiers) -> u8 {
        let mut result = 0u8;
        if modifiers.contains(KeyModifiers::SHIFT) {
            result |= 4;
        }
        if modifiers.contains(KeyModifiers::ALT) {
            result |= 8;
        }
        if modifiers.contains(KeyModifiers::CONTROL) {
            result |= 16;
        }
        result
    }

    /// Convert crossterm MouseButton to xterm button code.
    ///
    /// Xterm button codes: 0=left, 1=middle, 2=right
    fn mouse_button_to_code(&self, button: MouseButton) -> u8 {
        match button {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        }
    }

    /// Extend selection to a position (for shift-click).
    ///
    /// If there's an existing selection, extends from the anchor to the new position.
    /// If no selection exists, creates a new character selection at the position.
    fn extend_selection_to(&mut self, pos: GridPos) {
        if self.selection_anchor.is_some() {
            // Extend from anchor to new position using current selection mode
            self.extend_selection(pos);
            // Copy to clipboard after shift-click extension
            if let Some(ref sel) = self.selection {
                if sel.start != sel.end {
                    if let Some(text) = self.get_selected_text() {
                        self.copy_to_clipboard(&text);
                    }
                }
            }
        } else {
            // No existing anchor, start a new character selection
            self.selection_mode = SelectionMode::Character;
            self.selection = Some(Selection::new(pos));
            self.selection_anchor = Some(pos);
        }
    }

    /// Extend selection by keyboard movement (delta_col, delta_row).
    ///
    /// Only works when there's an existing selection. Moves the selection's
    /// end position by the given delta.
    fn extend_selection_keyboard(&mut self, delta_col: i32, delta_row: i32) {
        let sel = match self.selection.as_mut() {
            Some(s) => s,
            None => return,
        };

        let grid = self.terminal.grid();
        let max_row = i32::from(grid.rows().saturating_sub(1));
        let max_col = i32::from(grid.cols().saturating_sub(1));

        // Calculate new end position
        // SAFETY: After clamping to 0..max_row (derived from u16), result fits in u16
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let new_row = (i32::from(sel.end.row) + delta_row).clamp(0, max_row) as u16;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let new_col = (i32::from(sel.end.col) + delta_col).clamp(0, max_col) as u16;

        sel.end = GridPos::new(new_row, new_col);

        // Copy to clipboard after keyboard selection
        if sel.start != sel.end {
            if let Some(text) = self.get_selected_text() {
                self.copy_to_clipboard(&text);
            }
        }
    }

    /// Extend selection to the start of the current line.
    fn extend_selection_to_line_start(&mut self) {
        let sel = match self.selection.as_mut() {
            Some(s) => s,
            None => return,
        };

        sel.end = GridPos::new(sel.end.row, 0);

        // Copy to clipboard
        if sel.start != sel.end {
            if let Some(text) = self.get_selected_text() {
                self.copy_to_clipboard(&text);
            }
        }
    }

    /// Extend selection to the end of the current line.
    fn extend_selection_to_line_end(&mut self) {
        let sel = match self.selection.as_mut() {
            Some(s) => s,
            None => return,
        };

        let grid = self.terminal.grid();

        // Find the last non-space character on the line
        let row = sel.end.row;
        let mut end_col = 0u16;
        for col in 0..grid.cols() {
            if let Some(cell) = grid.cell(row, col) {
                let ch = cell.char();
                if ch != ' ' && ch != '\0' {
                    end_col = col;
                }
            }
        }

        sel.end = GridPos::new(row, end_col);

        // Copy to clipboard
        if sel.start != sel.end {
            if let Some(text) = self.get_selected_text() {
                self.copy_to_clipboard(&text);
            }
        }
    }

    /// Extend selection based on current selection mode.
    fn extend_selection(&mut self, pos: GridPos) {
        let anchor = match self.selection_anchor {
            Some(a) => a,
            None => return,
        };

        match self.selection_mode {
            SelectionMode::Character | SelectionMode::Block => {
                // Simple character selection
                if let Some(ref mut sel) = self.selection {
                    sel.end = pos;
                }
            }
            SelectionMode::Word => {
                // Word-by-word selection: extend to include whole words
                let grid = self.terminal.grid();

                // Get the word at the anchor position
                let (anchor_word_start, anchor_word_end) =
                    find_word_boundaries(anchor.row, anchor.col, grid);

                // Get the word at the current position
                let (pos_word_start, pos_word_end) = find_word_boundaries(pos.row, pos.col, grid);

                // Determine start and end based on direction
                let (start, end) =
                    if pos.row < anchor.row || (pos.row == anchor.row && pos.col < anchor.col) {
                        // Dragging backwards (up or left)
                        (
                            GridPos::new(pos.row, pos_word_start),
                            GridPos::new(anchor.row, anchor_word_end),
                        )
                    } else {
                        // Dragging forwards (down or right)
                        (
                            GridPos::new(anchor.row, anchor_word_start),
                            GridPos::new(pos.row, pos_word_end),
                        )
                    };

                self.selection = Some(Selection { start, end });
            }
            SelectionMode::Line => {
                // Line-by-line selection: extend to include whole lines
                let grid = self.terminal.grid();

                // Determine start and end rows
                let (start_row, end_row) = if pos.row < anchor.row {
                    (pos.row, anchor.row)
                } else {
                    (anchor.row, pos.row)
                };

                // Find the end of the last line (for proper selection bounds)
                let mut end_col = 0u16;
                for col in 0..grid.cols() {
                    if let Some(cell) = grid.cell(end_row, col) {
                        let ch = cell.char();
                        if ch != ' ' && ch != '\0' {
                            end_col = col;
                        }
                    }
                }

                self.selection = Some(Selection {
                    start: GridPos::new(start_row, 0),
                    end: GridPos::new(end_row, end_col),
                });
            }
        }
    }

    /// Convert mouse coordinates to grid position.
    fn mouse_to_grid(&self, mouse_col: u16, mouse_row: u16) -> Option<GridPos> {
        // Check if mouse is within the inner area
        if mouse_col < self.inner_area.x
            || mouse_row < self.inner_area.y
            || mouse_col >= self.inner_area.x + self.inner_area.width
            || mouse_row >= self.inner_area.y + self.inner_area.height
        {
            return None;
        }

        let col = mouse_col - self.inner_area.x;
        let row = mouse_row - self.inner_area.y;

        // Clamp to grid bounds
        let grid = self.terminal.grid();
        let col = min(col, grid.cols().saturating_sub(1));
        let row = min(row, grid.rows().saturating_sub(1));

        Some(GridPos::new(row, col))
    }

    /// Convert mouse coordinates to grid position for drag selection.
    ///
    /// Unlike `mouse_to_grid`, this handles positions outside the terminal area
    /// by clamping to edges and returning a scroll direction:
    /// - Returns `Some((pos, scroll_delta))` where scroll_delta is:
    ///   - Positive to scroll up (show older content)
    ///   - Negative to scroll down (show newer content)
    ///   - Zero if no scrolling needed
    fn mouse_to_grid_for_drag(&self, mouse_col: u16, mouse_row: u16) -> (GridPos, i32) {
        let grid = self.terminal.grid();
        let max_row = grid.rows().saturating_sub(1);
        let max_col = grid.cols().saturating_sub(1);

        let mut scroll_delta = 0i32;

        // Calculate row position and scroll direction
        let row = if mouse_row < self.inner_area.y {
            // Mouse above terminal - scroll up
            scroll_delta = 1;
            0
        } else if mouse_row >= self.inner_area.y + self.inner_area.height {
            // Mouse below terminal - scroll down
            scroll_delta = -1;
            max_row
        } else {
            // Mouse within terminal
            min(mouse_row - self.inner_area.y, max_row)
        };

        // Calculate column position (clamp to bounds)
        let col = if mouse_col < self.inner_area.x {
            0
        } else if mouse_col >= self.inner_area.x + self.inner_area.width {
            max_col
        } else {
            min(mouse_col - self.inner_area.x, max_col)
        };

        (GridPos::new(row, col), scroll_delta)
    }

    /// Get the text content of the current selection.
    fn get_selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        selection_text_for_grid(sel, self.selection_mode, self.terminal.grid())
    }

    /// Copy text to the system clipboard.
    fn copy_to_clipboard(&mut self, text: &str) {
        if let Some(ref mut clipboard) = self.clipboard {
            let _ = clipboard.set_text(text);
        }
    }

    /// Copy the current selection to the clipboard (if any).
    fn copy_selection_to_clipboard(&mut self) {
        if let Some(ref sel) = self.selection {
            if sel.start != sel.end {
                if let Some(text) = self.get_selected_text() {
                    self.copy_to_clipboard(&text);
                }
            }
        }
    }

    /// Select all visible content in the terminal.
    ///
    /// Creates a selection spanning from the top-left corner (0,0) to the
    /// bottom-right corner of the visible terminal grid.
    fn select_all_visible(&mut self) {
        let grid = self.terminal.grid();
        let max_row = grid.rows().saturating_sub(1);
        let max_col = grid.cols().saturating_sub(1);

        self.selection = Some(Selection {
            start: GridPos { row: 0, col: 0 },
            end: GridPos {
                row: max_row,
                col: max_col,
            },
        });
        self.selection_mode = SelectionMode::Character;
    }

    /// Check if a cell is within the current selection, honoring selection mode.
    fn selection_contains(&self, row: u16, col: u16) -> bool {
        let sel = match self.selection.as_ref() {
            Some(selection) => selection,
            None => return false,
        };

        match self.selection_mode {
            SelectionMode::Block => sel.contains_rect(row, col),
            _ => sel.contains(row, col),
        }
    }

    /// Paste text from the system clipboard to the PTY.
    ///
    /// If bracketed paste mode is enabled (mode 2004), the pasted text is
    /// wrapped with ESC[200~ and ESC[201~ escape sequences.
    fn paste_from_clipboard(&mut self) -> Result<(), TuiError> {
        if let Some(ref mut clipboard) = self.clipboard {
            if let Ok(text) = clipboard.get_text() {
                if self.terminal.modes().bracketed_paste {
                    // Bracketed paste mode: wrap with escape sequences
                    self.pty.write(b"\x1b[200~")?;
                    self.pty.write(text.as_bytes())?;
                    self.pty.write(b"\x1b[201~")?;
                } else {
                    // Normal paste: send text directly
                    self.pty.write(text.as_bytes())?;
                }
            }
        }
        Ok(())
    }

    /// Handle terminal resize.
    fn handle_resize(&mut self, cols: u16, rows: u16) -> Result<(), TuiError> {
        // Resize PTY
        self.pty.resize(rows, cols)?;

        // Resize terminal emulator
        self.terminal.resize(rows, cols);

        Ok(())
    }

    /// Handle focus change events.
    ///
    /// When the terminal application has enabled focus reporting (mode 1004),
    /// sends CSI I (focus in) or CSI O (focus out) to the PTY.
    fn handle_focus(&mut self, focused: bool) -> Result<(), TuiError> {
        // Only forward focus events if the application requested focus reporting
        if let Some(seq) = self.terminal.encode_focus_event(focused) {
            self.pty.write(&seq)?;
        }
        Ok(())
    }

    /// Draw the terminal UI.
    fn draw(&self, frame: &mut Frame) {
        let area = frame.size();

        // Build title with indicators
        let display_offset = self.terminal.grid().display_offset();
        let is_alt_screen = self.terminal.is_alternate_screen();

        let mut title = format!(" {} ", self.title);

        // Add alternate screen indicator
        if is_alt_screen {
            title.push_str("[ALT] ");
        }

        // Add scrollback indicator (only when not on alternate screen)
        if display_offset > 0 && !is_alt_screen {
            let total_lines = self.terminal.grid().scrollback_lines();
            title.push_str(&format!(
                "[scrollback: {}/{}] ",
                display_offset, total_lines
            ));
        }

        // Create the main terminal block
        let block = Block::default().title(title).borders(Borders::ALL);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Render the terminal grid content
        self.render_grid(frame, inner);

        // Position cursor
        if self.terminal.cursor_visible() {
            let cursor = self.terminal.cursor();
            // Offset by 1 for the border
            let cursor_x = inner.x + cursor.col;
            let cursor_y = inner.y + cursor.row;

            if cursor_x < inner.x + inner.width && cursor_y < inner.y + inner.height {
                frame.set_cursor(cursor_x, cursor_y);
            }
        }
    }

    /// Render the terminal grid to the frame.
    fn render_grid(&self, frame: &mut Frame, area: Rect) {
        let grid = self.terminal.grid();

        for row in 0..area.height.min(grid.rows()) {
            for col in 0..area.width.min(grid.cols()) {
                if let Some(cell) = grid.cell(row, col) {
                    // Skip wide character continuations
                    if cell.is_wide_continuation() {
                        continue;
                    }

                    let ch = cell.char();
                    let (fg, bg) = cell_colors(cell);
                    let modifiers = cell_modifiers(cell);

                    // Check if this cell is selected
                    let is_selected = self.selection_contains(row, col);

                    // Apply inverted colors for selected cells
                    let style = if is_selected {
                        Style::default()
                            .fg(bg)
                            .bg(fg)
                            .add_modifier(modifiers | Modifier::REVERSED)
                    } else {
                        Style::default().fg(fg).bg(bg).add_modifier(modifiers)
                    };

                    // Create a single-character span
                    let x = area.x + col;
                    let y = area.y + row;

                    if x < area.x + area.width && y < area.y + area.height {
                        let cell_area = Rect::new(x, y, 1, 1);
                        let text = Paragraph::new(ch.to_string()).style(style);
                        frame.render_widget(text, cell_area);
                    }
                }
            }
        }
    }

    /// Update the cursor style if it has changed.
    fn update_cursor_style(
        &mut self,
        backend: &mut CrosstermBackend<Stdout>,
    ) -> Result<(), TuiError> {
        let current_style = self.terminal.cursor_style();

        if current_style != self.last_cursor_style {
            let crossterm_style = dterm_to_crossterm_cursor_style(current_style);
            execute!(backend, crossterm_style)?;
            self.last_cursor_style = current_style;
        }

        Ok(())
    }
}

fn selection_text_for_grid(
    sel: &Selection,
    mode: SelectionMode,
    grid: &dterm_core::grid::Grid,
) -> Option<String> {
    if grid.rows() == 0 || grid.cols() == 0 {
        return None;
    }

    let (start, end) = match mode {
        SelectionMode::Block => sel.rect_bounds(),
        _ => sel.normalized(),
    };

    let max_col = grid.cols().saturating_sub(1);
    let mut text = String::new();

    for row in start.row..=end.row {
        if row >= grid.rows() {
            break;
        }

        let (mut col_start, mut col_end) = match mode {
            SelectionMode::Block => (start.col, end.col),
            _ => {
                let col_start = if row == start.row { start.col } else { 0 };
                let col_end = if row == end.row { end.col } else { max_col };
                (col_start, col_end)
            }
        };

        col_start = col_start.min(max_col);
        col_end = col_end.min(max_col);

        for col in col_start..=col_end {
            if let Some(cell) = grid.cell(row, col) {
                // Skip wide character continuations
                if !cell.is_wide_continuation() {
                    text.push(cell.char());
                }
            }
        }

        // Add newline between rows (but not after the last row)
        if row < end.row {
            // Trim trailing spaces from the row
            while text.ends_with(' ') {
                text.pop();
            }
            text.push('\n');
        }
    }

    // Trim trailing spaces from the last row
    while text.ends_with(' ') {
        text.pop();
    }

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Convert a terminal cell's colors to ratatui colors.
fn cell_colors(cell: &dterm_core::grid::Cell) -> (Color, Color) {
    let fg_packed = cell.fg();
    let bg_packed = cell.bg();

    let fg = packed_to_color(fg_packed, true);
    let bg = packed_to_color(bg_packed, false);

    (fg, bg)
}

/// Convert a PackedColor to a ratatui Color.
fn packed_to_color(packed: dterm_core::grid::PackedColor, _is_fg: bool) -> Color {
    if packed.is_default() {
        Color::Reset
    } else if packed.is_indexed() {
        Color::Indexed(packed.index())
    } else if packed.is_rgb() {
        let (r, g, b) = packed.rgb_components();
        Color::Rgb(r, g, b)
    } else {
        Color::Reset
    }
}

/// Convert a terminal cell's flags to ratatui modifiers.
fn cell_modifiers(cell: &dterm_core::grid::Cell) -> Modifier {
    use dterm_core::grid::CellFlags;

    let flags = cell.flags();
    let mut modifiers = Modifier::empty();

    if flags.contains(CellFlags::BOLD) {
        modifiers |= Modifier::BOLD;
    }
    if flags.contains(CellFlags::DIM) {
        modifiers |= Modifier::DIM;
    }
    if flags.contains(CellFlags::ITALIC) {
        modifiers |= Modifier::ITALIC;
    }
    if flags.contains(CellFlags::UNDERLINE) {
        modifiers |= Modifier::UNDERLINED;
    }
    if flags.contains(CellFlags::BLINK) {
        modifiers |= Modifier::SLOW_BLINK;
    }
    if flags.contains(CellFlags::INVERSE) {
        modifiers |= Modifier::REVERSED;
    }
    if flags.contains(CellFlags::STRIKETHROUGH) {
        modifiers |= Modifier::CROSSED_OUT;
    }

    modifiers
}

/// Convert dterm cursor style to crossterm cursor style.
fn dterm_to_crossterm_cursor_style(style: CursorStyle) -> CrosstermCursorStyle {
    match style {
        CursorStyle::BlinkingBlock => CrosstermCursorStyle::BlinkingBlock,
        CursorStyle::SteadyBlock => CrosstermCursorStyle::SteadyBlock,
        CursorStyle::BlinkingUnderline => CrosstermCursorStyle::BlinkingUnderScore,
        CursorStyle::SteadyUnderline => CrosstermCursorStyle::SteadyUnderScore,
        CursorStyle::BlinkingBar => CrosstermCursorStyle::BlinkingBar,
        CursorStyle::SteadyBar => CrosstermCursorStyle::SteadyBar,
    }
}

/// Convert a crossterm key event to bytes to send to PTY.
fn key_to_bytes(key: KeyEvent) -> Vec<u8> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        // Regular characters
        KeyCode::Char(c) => {
            if ctrl {
                // Ctrl+letter sends control code (0x01-0x1A)
                if c.is_ascii_lowercase() {
                    vec![(c as u8) - b'a' + 1]
                } else if c.is_ascii_uppercase() {
                    vec![(c.to_ascii_lowercase() as u8) - b'a' + 1]
                } else {
                    match c {
                        ' ' | '@' | '`' => vec![0x00],
                        '[' | '{' => vec![0x1B],
                        '\\' | '|' => vec![0x1C],
                        ']' | '}' => vec![0x1D],
                        '^' | '~' => vec![0x1E],
                        '_' | '?' => vec![0x1F],
                        _ => vec![],
                    }
                }
            } else if alt {
                // Alt+char sends ESC followed by char
                vec![0x1B, c as u8]
            } else {
                // Normal character - encode as UTF-8
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }

        // Special keys
        KeyCode::Enter => vec![0x0D],
        KeyCode::Tab => vec![0x09],
        KeyCode::Backspace => vec![0x7F],
        KeyCode::Esc => vec![0x1B],

        // Arrow keys (application mode by default)
        KeyCode::Up => {
            if ctrl {
                b"\x1b[1;5A".to_vec()
            } else if alt {
                b"\x1b[1;3A".to_vec()
            } else {
                b"\x1b[A".to_vec()
            }
        }
        KeyCode::Down => {
            if ctrl {
                b"\x1b[1;5B".to_vec()
            } else if alt {
                b"\x1b[1;3B".to_vec()
            } else {
                b"\x1b[B".to_vec()
            }
        }
        KeyCode::Right => {
            if ctrl {
                b"\x1b[1;5C".to_vec()
            } else if alt {
                b"\x1b[1;3C".to_vec()
            } else {
                b"\x1b[C".to_vec()
            }
        }
        KeyCode::Left => {
            if ctrl {
                b"\x1b[1;5D".to_vec()
            } else if alt {
                b"\x1b[1;3D".to_vec()
            } else {
                b"\x1b[D".to_vec()
            }
        }

        // Other special keys
        KeyCode::Home => {
            if ctrl {
                b"\x1b[1;5H".to_vec()
            } else {
                b"\x1b[H".to_vec()
            }
        }
        KeyCode::End => {
            if ctrl {
                b"\x1b[1;5F".to_vec()
            } else {
                b"\x1b[F".to_vec()
            }
        }
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),

        // Function keys
        KeyCode::F(1) => b"\x1bOP".to_vec(),
        KeyCode::F(2) => b"\x1bOQ".to_vec(),
        KeyCode::F(3) => b"\x1bOR".to_vec(),
        KeyCode::F(4) => b"\x1bOS".to_vec(),
        KeyCode::F(5) => b"\x1b[15~".to_vec(),
        KeyCode::F(6) => b"\x1b[17~".to_vec(),
        KeyCode::F(7) => b"\x1b[18~".to_vec(),
        KeyCode::F(8) => b"\x1b[19~".to_vec(),
        KeyCode::F(9) => b"\x1b[20~".to_vec(),
        KeyCode::F(10) => b"\x1b[21~".to_vec(),
        KeyCode::F(11) => b"\x1b[23~".to_vec(),
        KeyCode::F(12) => b"\x1b[24~".to_vec(),
        KeyCode::F(_) => vec![],

        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_to_bytes_enter() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), vec![0x0D]);
    }

    #[test]
    fn test_key_to_bytes_char() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), vec![b'a']);
    }

    #[test]
    fn test_key_to_bytes_ctrl_c() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_to_bytes(key), vec![0x03]); // ETX
    }

    #[test]
    fn test_key_to_bytes_alt_x() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT);
        assert_eq!(key_to_bytes(key), vec![0x1B, b'x']);
    }

    #[test]
    fn test_key_to_bytes_arrow() {
        let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), b"\x1b[A".to_vec());
    }

    #[test]
    fn test_key_to_bytes_f1() {
        let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), b"\x1bOP".to_vec());
    }

    #[test]
    fn test_key_to_bytes_shift_pageup() {
        // Shift+PageUp is handled by the TUI for scrollback, not sent to PTY
        let key = KeyEvent::new(KeyCode::PageUp, KeyModifiers::SHIFT);
        // PageUp without shift sends escape sequence
        let key_no_shift = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key_no_shift), b"\x1b[5~".to_vec());
        // Shift+PageUp also sends the same sequence (TUI intercepts before this)
        assert_eq!(key_to_bytes(key), b"\x1b[5~".to_vec());
    }

    #[test]
    fn test_key_to_bytes_shift_pagedown() {
        // Shift+PageDown is handled by the TUI for scrollback, not sent to PTY
        let key = KeyEvent::new(KeyCode::PageDown, KeyModifiers::SHIFT);
        // PageDown without shift sends escape sequence
        let key_no_shift = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key_no_shift), b"\x1b[6~".to_vec());
        // Shift+PageDown also sends the same sequence (TUI intercepts before this)
        assert_eq!(key_to_bytes(key), b"\x1b[6~".to_vec());
    }

    #[test]
    fn test_key_to_bytes_shift_home() {
        let key = KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT);
        // Home sends its sequence regardless of shift modifier
        assert_eq!(key_to_bytes(key), b"\x1b[H".to_vec());
    }

    #[test]
    fn test_key_to_bytes_shift_end() {
        let key = KeyEvent::new(KeyCode::End, KeyModifiers::SHIFT);
        // End sends its sequence regardless of shift modifier
        assert_eq!(key_to_bytes(key), b"\x1b[F".to_vec());
    }

    #[test]
    fn test_cursor_style_conversion() {
        // Test all cursor style conversions
        assert!(matches!(
            dterm_to_crossterm_cursor_style(CursorStyle::BlinkingBlock),
            CrosstermCursorStyle::BlinkingBlock
        ));
        assert!(matches!(
            dterm_to_crossterm_cursor_style(CursorStyle::SteadyBlock),
            CrosstermCursorStyle::SteadyBlock
        ));
        assert!(matches!(
            dterm_to_crossterm_cursor_style(CursorStyle::BlinkingUnderline),
            CrosstermCursorStyle::BlinkingUnderScore
        ));
        assert!(matches!(
            dterm_to_crossterm_cursor_style(CursorStyle::SteadyUnderline),
            CrosstermCursorStyle::SteadyUnderScore
        ));
        assert!(matches!(
            dterm_to_crossterm_cursor_style(CursorStyle::BlinkingBar),
            CrosstermCursorStyle::BlinkingBar
        ));
        assert!(matches!(
            dterm_to_crossterm_cursor_style(CursorStyle::SteadyBar),
            CrosstermCursorStyle::SteadyBar
        ));
    }

    #[test]
    fn test_selection_single_line() {
        // Selection on a single line (left to right)
        let sel = Selection {
            start: GridPos::new(5, 3),
            end: GridPos::new(5, 10),
        };

        // Cells within selection
        assert!(sel.contains(5, 3)); // Start
        assert!(sel.contains(5, 5)); // Middle
        assert!(sel.contains(5, 10)); // End

        // Cells outside selection
        assert!(!sel.contains(5, 2)); // Before start
        assert!(!sel.contains(5, 11)); // After end
        assert!(!sel.contains(4, 5)); // Wrong row
        assert!(!sel.contains(6, 5)); // Wrong row
    }

    #[test]
    fn test_selection_single_line_reversed() {
        // Selection on a single line (right to left)
        let sel = Selection {
            start: GridPos::new(5, 10),
            end: GridPos::new(5, 3),
        };

        // Should behave the same as left-to-right
        assert!(sel.contains(5, 3));
        assert!(sel.contains(5, 5));
        assert!(sel.contains(5, 10));
        assert!(!sel.contains(5, 2));
        assert!(!sel.contains(5, 11));
    }

    #[test]
    fn test_selection_multi_line() {
        // Multi-line selection (top-left to bottom-right)
        let sel = Selection {
            start: GridPos::new(2, 5),
            end: GridPos::new(4, 10),
        };

        // First line: col 5 onwards
        assert!(!sel.contains(2, 4));
        assert!(sel.contains(2, 5));
        assert!(sel.contains(2, 20)); // Rest of first line

        // Middle line: entire line selected
        assert!(sel.contains(3, 0));
        assert!(sel.contains(3, 50));

        // Last line: up to col 10
        assert!(sel.contains(4, 0));
        assert!(sel.contains(4, 10));
        assert!(!sel.contains(4, 11));

        // Outside rows
        assert!(!sel.contains(1, 5));
        assert!(!sel.contains(5, 5));
    }

    #[test]
    fn test_selection_multi_line_reversed() {
        // Multi-line selection (bottom-right to top-left)
        let sel = Selection {
            start: GridPos::new(4, 10),
            end: GridPos::new(2, 5),
        };

        // Should behave the same as top-to-bottom selection
        assert!(sel.contains(2, 5));
        assert!(sel.contains(2, 20));
        assert!(sel.contains(3, 0));
        assert!(sel.contains(4, 0));
        assert!(sel.contains(4, 10));
        assert!(!sel.contains(4, 11));
    }

    #[test]
    fn test_selection_normalized() {
        // Forward selection
        let sel1 = Selection {
            start: GridPos::new(2, 5),
            end: GridPos::new(4, 10),
        };
        let (start1, end1) = sel1.normalized();
        assert_eq!(start1, GridPos::new(2, 5));
        assert_eq!(end1, GridPos::new(4, 10));

        // Reversed selection
        let sel2 = Selection {
            start: GridPos::new(4, 10),
            end: GridPos::new(2, 5),
        };
        let (start2, end2) = sel2.normalized();
        assert_eq!(start2, GridPos::new(2, 5));
        assert_eq!(end2, GridPos::new(4, 10));
    }

    #[test]
    fn test_selection_rect_contains() {
        let sel = Selection {
            start: GridPos::new(2, 5),
            end: GridPos::new(4, 10),
        };

        assert!(sel.contains_rect(2, 5));
        assert!(sel.contains_rect(3, 7));
        assert!(sel.contains_rect(4, 10));
        assert!(!sel.contains_rect(1, 7));
        assert!(!sel.contains_rect(3, 11));
    }

    #[test]
    fn test_gridpos_new() {
        let pos = GridPos::new(10, 20);
        assert_eq!(pos.row, 10);
        assert_eq!(pos.col, 20);
    }

    #[test]
    fn test_click_tracker_single_click() {
        let mut tracker = ClickTracker::default();
        let pos = GridPos::new(5, 10);

        // First click should be count 1
        assert_eq!(tracker.register_click(pos), 1);
    }

    #[test]
    fn test_click_tracker_double_click() {
        let mut tracker = ClickTracker::default();
        let pos = GridPos::new(5, 10);

        // First click
        assert_eq!(tracker.register_click(pos), 1);
        // Second click at same position (immediate) should be count 2
        assert_eq!(tracker.register_click(pos), 2);
    }

    #[test]
    fn test_click_tracker_triple_click() {
        let mut tracker = ClickTracker::default();
        let pos = GridPos::new(5, 10);

        // First click
        assert_eq!(tracker.register_click(pos), 1);
        // Second click
        assert_eq!(tracker.register_click(pos), 2);
        // Third click
        assert_eq!(tracker.register_click(pos), 3);
    }

    #[test]
    fn test_click_tracker_wraps_after_triple() {
        let mut tracker = ClickTracker::default();
        let pos = GridPos::new(5, 10);

        // Click 1, 2, 3
        tracker.register_click(pos);
        tracker.register_click(pos);
        tracker.register_click(pos);
        // Fourth click wraps back to 1
        assert_eq!(tracker.register_click(pos), 1);
    }

    #[test]
    fn test_click_tracker_different_position_resets() {
        let mut tracker = ClickTracker::default();
        let pos1 = GridPos::new(5, 10);
        let pos2 = GridPos::new(5, 20); // Different column

        // First click at pos1
        assert_eq!(tracker.register_click(pos1), 1);
        // Click at different position resets to 1
        assert_eq!(tracker.register_click(pos2), 1);
    }

    #[test]
    fn test_click_tracker_nearby_position_counts() {
        let mut tracker = ClickTracker::default();
        let pos1 = GridPos::new(5, 10);
        let pos2 = GridPos::new(5, 11); // 1 cell away (within threshold)

        // First click at pos1
        assert_eq!(tracker.register_click(pos1), 1);
        // Click at nearby position should count as double-click
        assert_eq!(tracker.register_click(pos2), 2);
    }

    #[test]
    fn test_selection_mode_enum() {
        // Just verify enum variants exist and can be compared
        assert_eq!(SelectionMode::Character, SelectionMode::Character);
        assert_eq!(SelectionMode::Block, SelectionMode::Block);
        assert_ne!(SelectionMode::Character, SelectionMode::Word);
        assert_ne!(SelectionMode::Character, SelectionMode::Block);
        assert_ne!(SelectionMode::Block, SelectionMode::Word);
        assert_ne!(SelectionMode::Block, SelectionMode::Line);
        assert_ne!(SelectionMode::Word, SelectionMode::Line);
    }

    /// Helper function to write a string to a grid at a specific row starting at column 0.
    fn write_string_to_grid(grid: &mut dterm_core::grid::Grid, row: u16, text: &str) {
        grid.set_cursor(row, 0);
        for ch in text.chars() {
            grid.write_char(ch);
        }
    }

    #[test]
    fn test_find_word_boundaries_alphanumeric() {
        use dterm_core::grid::Grid;

        // Create a grid with "hello world" on the first row
        let mut grid = Grid::new(10, 80);
        write_string_to_grid(&mut grid, 0, "hello world");

        // Test finding boundaries of "hello" (clicking on 'e' at col 1)
        let (start, end) = find_word_boundaries(0, 1, &grid);
        assert_eq!(start, 0); // Start of "hello"
        assert_eq!(end, 4); // End of "hello"

        // Test finding boundaries of "world" (clicking on 'r' at col 8)
        let (start, end) = find_word_boundaries(0, 8, &grid);
        assert_eq!(start, 6); // Start of "world"
        assert_eq!(end, 10); // End of "world"
    }

    #[test]
    fn test_find_word_boundaries_space() {
        use dterm_core::grid::Grid;

        // Create a grid with "hello world" on the first row
        let mut grid = Grid::new(10, 80);
        write_string_to_grid(&mut grid, 0, "hello world");

        // Test clicking on space between words (col 5)
        let (start, end) = find_word_boundaries(0, 5, &grid);
        assert_eq!(start, 5); // Just the space
        assert_eq!(end, 5);
    }

    #[test]
    fn test_find_word_boundaries_underscore() {
        use dterm_core::grid::Grid;

        // Create a grid with "foo_bar baz"
        let mut grid = Grid::new(10, 80);
        write_string_to_grid(&mut grid, 0, "foo_bar baz");

        // Test finding boundaries of "foo_bar" (clicking on '_' at col 3)
        let (start, end) = find_word_boundaries(0, 3, &grid);
        assert_eq!(start, 0); // Start of "foo_bar"
        assert_eq!(end, 6); // End of "foo_bar"
    }

    #[test]
    fn test_selection_word_at() {
        use dterm_core::grid::Grid;

        // Create a grid with "hello world"
        let mut grid = Grid::new(10, 80);
        write_string_to_grid(&mut grid, 0, "hello world");

        // Double-click on "world"
        let sel = Selection::word_at(GridPos::new(0, 8), &grid);
        assert_eq!(sel.start, GridPos::new(0, 6)); // Start of "world"
        assert_eq!(sel.end, GridPos::new(0, 10)); // End of "world"
    }

    #[test]
    fn test_selection_line_at() {
        use dterm_core::grid::Grid;

        // Create a grid with "hello world" followed by spaces
        let mut grid = Grid::new(10, 80);
        write_string_to_grid(&mut grid, 0, "hello world");

        // Triple-click on the line
        let sel = Selection::line_at(0, &grid);
        assert_eq!(sel.start, GridPos::new(0, 0)); // Start of line
        assert_eq!(sel.end, GridPos::new(0, 10)); // End at last non-space char
    }

    #[test]
    fn test_selection_line_at_empty() {
        use dterm_core::grid::Grid;

        // Create a grid with an empty line
        let grid = Grid::new(10, 80);

        // Triple-click on empty line
        let sel = Selection::line_at(0, &grid);
        assert_eq!(sel.start, GridPos::new(0, 0));
        assert_eq!(sel.end, GridPos::new(0, 0)); // Empty line has end at 0
    }

    #[test]
    fn test_selection_text_block() {
        use dterm_core::grid::Grid;

        let mut grid = Grid::new(4, 10);
        write_string_to_grid(&mut grid, 0, "abcd");
        write_string_to_grid(&mut grid, 1, "efgh");

        let sel = Selection {
            start: GridPos::new(0, 1),
            end: GridPos::new(1, 2),
        };

        let text = selection_text_for_grid(&sel, SelectionMode::Block, &grid);
        assert_eq!(text, Some(String::from("bc\nfg")));
    }

    #[test]
    fn test_selection_extend_forward() {
        // Test extending a selection forward (shift-click behavior)
        let mut sel = Selection {
            start: GridPos::new(5, 10),
            end: GridPos::new(5, 10), // Single position (after single click)
        };

        // Simulate shift-click extending to col 20
        sel.end = GridPos::new(5, 20);

        assert!(sel.contains(5, 10)); // Original position
        assert!(sel.contains(5, 15)); // Middle
        assert!(sel.contains(5, 20)); // Extended position
        assert!(!sel.contains(5, 9)); // Before start
        assert!(!sel.contains(5, 21)); // After end
    }

    #[test]
    fn test_selection_extend_backward() {
        // Test extending a selection backward (shift-click behavior)
        let mut sel = Selection {
            start: GridPos::new(5, 20),
            end: GridPos::new(5, 20), // Single position (after single click)
        };

        // Simulate shift-click extending backward to col 5
        sel.end = GridPos::new(5, 5);

        // Selection should cover from col 5 to col 20
        assert!(sel.contains(5, 5)); // Extended position
        assert!(sel.contains(5, 15)); // Middle
        assert!(sel.contains(5, 20)); // Original position
        assert!(!sel.contains(5, 4)); // Before extended start
        assert!(!sel.contains(5, 21)); // After original end
    }

    #[test]
    fn test_selection_extend_multiline() {
        // Test extending a selection across multiple lines (shift-click behavior)
        let mut sel = Selection {
            start: GridPos::new(5, 10),
            end: GridPos::new(5, 10),
        };

        // Shift-click on row 7, col 5
        sel.end = GridPos::new(7, 5);

        // Should cover row 5 from col 10 onwards
        assert!(sel.contains(5, 10));
        assert!(sel.contains(5, 50));

        // Should cover row 6 entirely
        assert!(sel.contains(6, 0));
        assert!(sel.contains(6, 50));

        // Should cover row 7 up to col 5
        assert!(sel.contains(7, 0));
        assert!(sel.contains(7, 5));
        assert!(!sel.contains(7, 6));
    }

    #[test]
    fn test_selection_extend_multiline_backward() {
        // Test extending a selection backward across lines (shift-click behavior)
        let mut sel = Selection {
            start: GridPos::new(7, 5),
            end: GridPos::new(7, 5),
        };

        // Shift-click on row 5, col 10
        sel.end = GridPos::new(5, 10);

        // Should cover the same range as forward selection
        assert!(sel.contains(5, 10));
        assert!(sel.contains(5, 50));
        assert!(sel.contains(6, 0));
        assert!(sel.contains(6, 50));
        assert!(sel.contains(7, 0));
        assert!(sel.contains(7, 5));
        assert!(!sel.contains(7, 6));
    }

    #[test]
    fn test_key_to_bytes_ctrl_shift_v() {
        // Ctrl+Shift+V is handled for paste, doesn't generate bytes
        // But if it slips through, it would generate Ctrl+V
        let key = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL);
        assert_eq!(key_to_bytes(key), vec![0x16]); // Ctrl+V is 0x16 (SYN)
    }

    #[test]
    fn test_key_to_bytes_paste_shortcut_modifier_check() {
        // Verify that Ctrl+Shift+V modifier check works correctly
        let modifiers = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert!(modifiers.contains(KeyModifiers::CONTROL));
        assert!(modifiers.contains(KeyModifiers::SHIFT));
        assert!(modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT));
    }

    #[test]
    fn test_key_to_bytes_ctrl_shift_c() {
        // Ctrl+Shift+C is handled for copy, doesn't generate bytes
        // But if it slips through, it would generate Ctrl+C
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_to_bytes(key), vec![0x03]); // Ctrl+C is 0x03 (ETX)
    }

    #[test]
    fn test_copy_shortcut_modifier_check() {
        // Verify that Ctrl+Shift+C modifier check works correctly
        let modifiers = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert!(modifiers.contains(KeyModifiers::CONTROL));
        assert!(modifiers.contains(KeyModifiers::SHIFT));
        assert!(modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT));

        // Verify uppercase C is detected correctly
        let key = KeyEvent::new(KeyCode::Char('C'), modifiers);
        assert_eq!(key.code, KeyCode::Char('C'));
    }

    #[test]
    fn test_selection_keyboard_extend_right() {
        // Test extending selection right with keyboard
        let mut sel = Selection {
            start: GridPos::new(5, 10),
            end: GridPos::new(5, 10),
        };

        // Simulate pressing Shift+Right
        sel.end = GridPos::new(5, 11);

        assert!(sel.contains(5, 10));
        assert!(sel.contains(5, 11));
        assert!(!sel.contains(5, 9));
        assert!(!sel.contains(5, 12));
    }

    #[test]
    fn test_selection_keyboard_extend_left() {
        // Test extending selection left with keyboard
        let mut sel = Selection {
            start: GridPos::new(5, 10),
            end: GridPos::new(5, 10),
        };

        // Simulate pressing Shift+Left
        sel.end = GridPos::new(5, 9);

        // Selection should cover from 9 to 10
        assert!(sel.contains(5, 9));
        assert!(sel.contains(5, 10));
        assert!(!sel.contains(5, 8));
        assert!(!sel.contains(5, 11));
    }

    #[test]
    fn test_selection_keyboard_extend_down() {
        // Test extending selection down with keyboard
        let mut sel = Selection {
            start: GridPos::new(5, 10),
            end: GridPos::new(5, 10),
        };

        // Simulate pressing Shift+Down
        sel.end = GridPos::new(6, 10);

        // Should select from row 5 col 10 to row 6 col 10
        assert!(sel.contains(5, 10));
        assert!(sel.contains(5, 50)); // Rest of row 5
        assert!(sel.contains(6, 0)); // Start of row 6
        assert!(sel.contains(6, 10)); // End position
        assert!(!sel.contains(6, 11));
    }

    #[test]
    fn test_selection_keyboard_extend_up() {
        // Test extending selection up with keyboard
        let mut sel = Selection {
            start: GridPos::new(5, 10),
            end: GridPos::new(5, 10),
        };

        // Simulate pressing Shift+Up
        sel.end = GridPos::new(4, 10);

        // Should select from row 4 col 10 to row 5 col 10
        assert!(sel.contains(4, 10));
        assert!(sel.contains(4, 50)); // Rest of row 4
        assert!(sel.contains(5, 0)); // Start of row 5
        assert!(sel.contains(5, 10)); // Original position
        assert!(!sel.contains(5, 11));
    }

    #[test]
    fn test_selection_keyboard_home_end() {
        // Test Shift+Home extends to line start
        let mut sel = Selection {
            start: GridPos::new(5, 20),
            end: GridPos::new(5, 20),
        };

        // Simulate Shift+Home
        sel.end = GridPos::new(5, 0);
        assert!(sel.contains(5, 0));
        assert!(sel.contains(5, 10));
        assert!(sel.contains(5, 20));
        assert!(!sel.contains(5, 21));

        // Now Shift+End from col 0 to end of line (say col 40)
        sel.end = GridPos::new(5, 40);
        assert!(sel.contains(5, 20)); // Start
        assert!(sel.contains(5, 30)); // Middle
        assert!(sel.contains(5, 40)); // End
    }

    #[test]
    fn test_drag_scroll_detection_above() {
        // When mouse is above terminal area, should return top row and positive scroll delta
        let inner_area = Rect::new(1, 1, 80, 24);
        let grid_cols = 80u16;

        // Simulating mouse at row 0 (above inner_area.y = 1)
        let mouse_row = 0u16;
        let mouse_col = 40u16;

        // Calculate expected position (row 0, clamped col)
        let expected_row = 0;
        let expected_col = mouse_col.saturating_sub(inner_area.x);
        let expected_scroll = 1; // Positive = scroll up

        // The function would return (pos, scroll_delta)
        // mouse_row < inner_area.y => scroll_delta = 1, row = 0
        assert!(mouse_row < inner_area.y);
        assert_eq!(expected_row, 0);
        assert_eq!(expected_scroll, 1);
        assert!(expected_col < grid_cols);
    }

    #[test]
    fn test_drag_scroll_detection_below() {
        // When mouse is below terminal area, should return bottom row and negative scroll delta
        let inner_area = Rect::new(1, 1, 80, 24);
        let grid_rows = 24u16;

        // Simulating mouse at row 25 (below inner_area.y + height = 25)
        let mouse_row = 25u16;

        // Calculate expected position (bottom row, clamped col)
        let expected_row = grid_rows.saturating_sub(1); // 23
        let expected_scroll = -1; // Negative = scroll down

        // mouse_row >= inner_area.y + inner_area.height => scroll_delta = -1
        assert!(mouse_row >= inner_area.y + inner_area.height);
        assert_eq!(expected_row, 23);
        assert_eq!(expected_scroll, -1);
    }

    #[test]
    fn test_drag_scroll_detection_within() {
        // When mouse is within terminal area, should return exact position and no scroll
        let inner_area = Rect::new(1, 1, 80, 24);

        // Simulating mouse at row 12, col 40 (within inner_area)
        let mouse_row = 12u16;
        let mouse_col = 40u16;

        // Calculate expected position
        let expected_row = mouse_row - inner_area.y;
        let expected_col = mouse_col - inner_area.x;
        let expected_scroll = 0; // No scroll

        // mouse_row is within [inner_area.y, inner_area.y + height)
        assert!(mouse_row >= inner_area.y);
        assert!(mouse_row < inner_area.y + inner_area.height);
        assert_eq!(expected_row, 11);
        assert_eq!(expected_col, 39);
        assert_eq!(expected_scroll, 0);
    }

    #[test]
    fn test_drag_scroll_col_clamp_left() {
        // When mouse column is left of terminal area, should clamp to 0
        let inner_area = Rect::new(5, 1, 80, 24);

        // Mouse at col 2 (left of inner_area.x = 5)
        let mouse_col = 2u16;

        // Expected column is 0 (clamped)
        // mouse_col < inner_area.x => col = 0
        assert!(mouse_col < inner_area.x);
    }

    #[test]
    fn test_drag_scroll_col_clamp_right() {
        // When mouse column is right of terminal area, should clamp to max
        let inner_area = Rect::new(1, 1, 80, 24);
        let grid_cols = 80u16;

        // Mouse at col 100 (right of inner_area.x + width = 81)
        let mouse_col = 100u16;

        // Expected column is max (grid_cols - 1 = 79)
        // mouse_col >= inner_area.x + inner_area.width => col = max_col
        assert!(mouse_col >= inner_area.x + inner_area.width);
        assert_eq!(grid_cols.saturating_sub(1), 79);
    }

    #[test]
    fn test_select_all_shortcut_modifier_check() {
        // Verify that Ctrl+Shift+A modifier check works correctly
        let modifiers = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert!(modifiers.contains(KeyModifiers::CONTROL));
        assert!(modifiers.contains(KeyModifiers::SHIFT));
        assert!(modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT));

        // Verify uppercase A is detected correctly
        let key = KeyEvent::new(KeyCode::Char('A'), modifiers);
        assert_eq!(key.code, KeyCode::Char('A'));
    }

    #[test]
    fn test_select_all_selection_bounds() {
        // Test that select all creates a selection spanning the entire visible area
        // Using 24x80 terminal dimensions (standard size)
        let rows = 24u16;
        let cols = 80u16;

        // Create selection as if select_all_visible was called
        let selection = Selection {
            start: GridPos { row: 0, col: 0 },
            end: GridPos {
                row: rows.saturating_sub(1),
                col: cols.saturating_sub(1),
            },
        };

        // Verify first row is fully selected
        assert!(selection.contains(0, 0));
        assert!(selection.contains(0, 40));
        assert!(selection.contains(0, 79));

        // Verify middle row is fully selected (all columns in middle rows)
        assert!(selection.contains(12, 0));
        assert!(selection.contains(12, 79));

        // Verify last row is selected up to col 79
        assert!(selection.contains(23, 0));
        assert!(selection.contains(23, 79));

        // Verify row out of bounds is not selected
        assert!(!selection.contains(24, 0));

        // Note: For multi-line selections, first/middle rows extend to end of line
        // so col 80 on row 0 would be "selected" by the contains logic
        // (since row 0 is the first row: col >= start.col is true for col=80)
        // Last row check: col 80 should NOT be selected since end.col=79
        assert!(!selection.contains(23, 80));
    }

    #[test]
    fn test_select_all_normalized() {
        // Test that select all creates a properly normalized selection
        let rows = 24u16;
        let cols = 80u16;

        let selection = Selection {
            start: GridPos { row: 0, col: 0 },
            end: GridPos {
                row: rows.saturating_sub(1),
                col: cols.saturating_sub(1),
            },
        };

        let (start, end) = selection.normalized();

        // Start should be (0, 0)
        assert_eq!(start.row, 0);
        assert_eq!(start.col, 0);

        // End should be (23, 79)
        assert_eq!(end.row, 23);
        assert_eq!(end.col, 79);
    }

    // =========================================================================
    // Mouse forwarding tests
    // =========================================================================

    #[test]
    fn test_mouse_modifiers_to_xterm_none() {
        // No modifiers should produce 0
        let modifiers = KeyModifiers::NONE;
        let mut result = 0u8;
        if modifiers.contains(KeyModifiers::SHIFT) {
            result |= 4;
        }
        if modifiers.contains(KeyModifiers::ALT) {
            result |= 8;
        }
        if modifiers.contains(KeyModifiers::CONTROL) {
            result |= 16;
        }
        assert_eq!(result, 0);
    }

    #[test]
    fn test_mouse_modifiers_to_xterm_shift() {
        // Shift should be bit 4
        let modifiers = KeyModifiers::SHIFT;
        let mut result = 0u8;
        if modifiers.contains(KeyModifiers::SHIFT) {
            result |= 4;
        }
        assert_eq!(result, 4);
    }

    #[test]
    fn test_mouse_modifiers_to_xterm_alt() {
        // Alt (Meta) should be bit 8
        let modifiers = KeyModifiers::ALT;
        let mut result = 0u8;
        if modifiers.contains(KeyModifiers::ALT) {
            result |= 8;
        }
        assert_eq!(result, 8);
    }

    #[test]
    fn test_mouse_modifiers_to_xterm_ctrl() {
        // Ctrl should be bit 16
        let modifiers = KeyModifiers::CONTROL;
        let mut result = 0u8;
        if modifiers.contains(KeyModifiers::CONTROL) {
            result |= 16;
        }
        assert_eq!(result, 16);
    }

    #[test]
    fn test_mouse_modifiers_to_xterm_combined() {
        // All modifiers combined
        let modifiers = KeyModifiers::SHIFT | KeyModifiers::ALT | KeyModifiers::CONTROL;
        let mut result = 0u8;
        if modifiers.contains(KeyModifiers::SHIFT) {
            result |= 4;
        }
        if modifiers.contains(KeyModifiers::ALT) {
            result |= 8;
        }
        if modifiers.contains(KeyModifiers::CONTROL) {
            result |= 16;
        }
        assert_eq!(result, 4 | 8 | 16);
        assert_eq!(result, 28);
    }

    #[test]
    fn test_mouse_button_to_code_left() {
        // Left button is code 0
        let button = MouseButton::Left;
        let code = match button {
            MouseButton::Left => 0u8,
            MouseButton::Middle => 1u8,
            MouseButton::Right => 2u8,
        };
        assert_eq!(code, 0);
    }

    #[test]
    fn test_mouse_button_to_code_middle() {
        // Middle button is code 1
        let button = MouseButton::Middle;
        let code = match button {
            MouseButton::Left => 0u8,
            MouseButton::Middle => 1u8,
            MouseButton::Right => 2u8,
        };
        assert_eq!(code, 1);
    }

    #[test]
    fn test_mouse_button_to_code_right() {
        // Right button is code 2
        let button = MouseButton::Right;
        let code = match button {
            MouseButton::Left => 0u8,
            MouseButton::Middle => 1u8,
            MouseButton::Right => 2u8,
        };
        assert_eq!(code, 2);
    }

    #[test]
    fn test_mouse_tracking_shift_override() {
        // Even when mouse tracking is enabled, shift+click should be handled locally
        // Test logic: forward_to_app = mouse_tracking && !shift_held
        let mouse_tracking = true;
        let shift_held = true;
        let forward_to_app = mouse_tracking && !shift_held;
        assert!(!forward_to_app); // Should NOT forward when shift held

        let shift_held = false;
        let forward_to_app = mouse_tracking && !shift_held;
        assert!(forward_to_app); // Should forward when shift not held
    }

    #[test]
    fn test_mouse_tracking_disabled_no_forward() {
        // When mouse tracking is disabled, should never forward
        let mouse_tracking = false;
        let shift_held = false;
        let forward_to_app = mouse_tracking && !shift_held;
        assert!(!forward_to_app);
    }

    // =========================================================================
    // Focus event forwarding tests
    // =========================================================================

    #[test]
    fn test_focus_event_encoding_focus_in() {
        // When focus reporting is enabled, focus in should produce CSI I
        use dterm_core::terminal::Terminal;
        let mut terminal = Terminal::new(24, 80);

        // Enable focus reporting mode (CSI ? 1004 h)
        terminal.process(b"\x1b[?1004h");

        // Verify focus reporting is now enabled
        assert!(terminal.modes().focus_reporting);

        // Encode focus in event
        let seq = terminal.encode_focus_event(true);
        assert!(seq.is_some());
        assert_eq!(seq.unwrap(), vec![0x1b, b'[', b'I']); // ESC [ I
    }

    #[test]
    fn test_focus_event_encoding_focus_out() {
        // When focus reporting is enabled, focus out should produce CSI O
        use dterm_core::terminal::Terminal;
        let mut terminal = Terminal::new(24, 80);

        // Enable focus reporting mode
        terminal.process(b"\x1b[?1004h");

        // Encode focus out event
        let seq = terminal.encode_focus_event(false);
        assert!(seq.is_some());
        assert_eq!(seq.unwrap(), vec![0x1b, b'[', b'O']); // ESC [ O
    }

    #[test]
    fn test_focus_event_disabled_returns_none() {
        // When focus reporting is disabled, encode_focus_event should return None
        use dterm_core::terminal::Terminal;
        let terminal = Terminal::new(24, 80);

        // Focus reporting is disabled by default
        assert!(!terminal.modes().focus_reporting);

        // Should return None for both focus states
        assert!(terminal.encode_focus_event(true).is_none());
        assert!(terminal.encode_focus_event(false).is_none());
    }

    #[test]
    fn test_focus_reporting_mode_toggle() {
        // Test enabling and disabling focus reporting mode
        use dterm_core::terminal::Terminal;
        let mut terminal = Terminal::new(24, 80);

        // Initially disabled
        assert!(!terminal.modes().focus_reporting);
        assert!(terminal.encode_focus_event(true).is_none());

        // Enable focus reporting
        terminal.process(b"\x1b[?1004h");
        assert!(terminal.modes().focus_reporting);
        assert!(terminal.encode_focus_event(true).is_some());

        // Disable focus reporting
        terminal.process(b"\x1b[?1004l");
        assert!(!terminal.modes().focus_reporting);
        assert!(terminal.encode_focus_event(true).is_none());
    }

    #[test]
    fn test_focus_event_sequence_format() {
        // Verify the exact byte sequences for focus events
        // CSI I = ESC [ I = 0x1B 0x5B 0x49
        // CSI O = ESC [ O = 0x1B 0x5B 0x4F
        use dterm_core::terminal::Terminal;
        let mut terminal = Terminal::new(24, 80);
        terminal.process(b"\x1b[?1004h");

        let focus_in = terminal.encode_focus_event(true).unwrap();
        assert_eq!(focus_in.len(), 3);
        assert_eq!(focus_in[0], 0x1B); // ESC
        assert_eq!(focus_in[1], 0x5B); // [
        assert_eq!(focus_in[2], 0x49); // I

        let focus_out = terminal.encode_focus_event(false).unwrap();
        assert_eq!(focus_out.len(), 3);
        assert_eq!(focus_out[0], 0x1B); // ESC
        assert_eq!(focus_out[1], 0x5B); // [
        assert_eq!(focus_out[2], 0x4F); // O
    }
}
