//! Terminal state machine using alacritty_terminal
//!
//! Wraps alacritty's battle-tested terminal emulation with a FFI-friendly interface.

use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, Processor};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::agent_parser::{AgentEvent, AgentParser};
use crate::cell::{Cell, CellAttributes, Color};

/// Terminal size in cells
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSize {
    pub cols: usize,
    pub rows: usize,
    /// Number of lines in scrollback history (default: 10000)
    #[serde(default = "default_scrollback")]
    pub scrollback: usize,
}

fn default_scrollback() -> usize {
    10000
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            scrollback: 10000,
        }
    }
}

impl TerminalSize {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            scrollback: 10000,
        }
    }

    pub fn with_scrollback(cols: usize, rows: usize, scrollback: usize) -> Self {
        Self {
            cols,
            rows,
            scrollback,
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.rows + self.scrollback
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// Events emitted by the terminal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalEvent {
    /// Terminal content changed, needs redraw
    Redraw,
    /// Bell character received
    Bell,
    /// Window title changed
    TitleChanged(String),
    /// Terminal exited
    Exit(i32),
}

/// Event collector for alacritty terminal events
#[derive(Default)]
struct DashTermEventListener {
    events: Arc<Mutex<Vec<TerminalEvent>>>,
}

impl DashTermEventListener {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn take_events(&self) -> Vec<TerminalEvent> {
        std::mem::take(&mut *self.events.lock())
    }
}

impl EventListener for DashTermEventListener {
    fn send_event(&self, event: Event) {
        let mut events = self.events.lock();
        match event {
            Event::Bell => events.push(TerminalEvent::Bell),
            Event::Title(title) => events.push(TerminalEvent::TitleChanged(title)),
            Event::ChildExit(code) => events.push(TerminalEvent::Exit(code)),
            Event::Wakeup => events.push(TerminalEvent::Redraw),
            _ => {}
        }
    }
}

/// Terminal configuration
fn default_config() -> Config {
    Config::default()
}

/// Terminal state wrapping alacritty_terminal
pub struct Terminal {
    /// Alacritty terminal instance
    term: Term<DashTermEventListener>,
    /// VTE processor
    processor: Processor,
    /// Event listener
    listener: DashTermEventListener,
    /// Terminal size
    size: TerminalSize,
    /// Window title
    title: String,
    /// Cached events
    pending_events: Vec<TerminalEvent>,
    /// Agent output parser for detecting AI agent events
    agent_parser: Option<AgentParser>,
    /// Pending agent events
    pending_agent_events: Vec<AgentEvent>,
}

impl Terminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self::with_scrollback(cols, rows, 10000)
    }

    pub fn with_scrollback(cols: usize, rows: usize, scrollback: usize) -> Self {
        let listener = DashTermEventListener::new();
        let size = TerminalSize::with_scrollback(cols, rows, scrollback);

        let term = Term::new(default_config(), &size, listener.clone());

        Self {
            term,
            processor: Processor::new(),
            listener,
            size,
            title: String::from("DashTerm"),
            pending_events: Vec::new(),
            agent_parser: None,
            pending_agent_events: Vec::new(),
        }
    }

    /// Process input bytes from the PTY
    pub fn process(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);

        // Collect events from listener
        self.pending_events
            .extend(self.listener.take_events());
        self.pending_events.push(TerminalEvent::Redraw);

        // Parse for agent events if enabled
        if let Some(parser) = &mut self.agent_parser {
            if let Ok(text) = std::str::from_utf8(bytes) {
                let events = parser.process(text);
                self.pending_agent_events.extend(events);
            }
        }
    }

    /// Take pending events
    pub fn take_events(&mut self) -> Vec<TerminalEvent> {
        let mut events = self.listener.take_events();
        events.append(&mut self.pending_events);
        events
    }

    /// Get cells for rendering (respects current scroll position)
    /// Returns a 2D vector of cells matching our Cell type
    pub fn get_cells(&self) -> Vec<Vec<Cell>> {
        let grid = self.term.grid();
        let num_cols = grid.columns();
        let num_lines = grid.screen_lines();
        let offset = grid.display_offset() as i32;

        let mut result = Vec::with_capacity(num_lines);

        for line_idx in 0..num_lines {
            let mut row = Vec::with_capacity(num_cols);
            // Account for display offset: negative lines = history
            let line = alacritty_terminal::index::Line(line_idx as i32 - offset);

            for col_idx in 0..num_cols {
                let col = alacritty_terminal::index::Column(col_idx);
                let point = alacritty_terminal::index::Point::new(line, col);
                let cell = &grid[point];

                row.push(convert_cell(cell));
            }
            result.push(row);
        }

        result
    }

    /// Get cursor position (row, col)
    pub fn cursor(&self) -> (usize, usize) {
        let cursor = self.term.grid().cursor.point;
        (cursor.line.0 as usize, cursor.column.0)
    }

    /// Get cursor visibility
    pub fn cursor_visible(&self) -> bool {
        self.term.mode().contains(alacritty_terminal::term::TermMode::SHOW_CURSOR)
    }

    /// Get terminal size
    pub fn size(&self) -> TerminalSize {
        self.size
    }

    /// Get window title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get current attributes (for compatibility)
    pub fn current_attrs(&self) -> CellAttributes {
        CellAttributes::default()
    }

    /// Resize terminal
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.size = TerminalSize::with_scrollback(cols, rows, self.size.scrollback);
        self.term.resize(self.size);
    }

    /// Get the current scroll display offset (0 = bottom, positive = scrolled up)
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// Get the total number of history lines available
    pub fn history_size(&self) -> usize {
        self.term.grid().history_size()
    }

    /// Scroll the display up by the given number of lines
    pub fn scroll_up(&mut self, lines: usize) {
        self.term.grid_mut().scroll_display(alacritty_terminal::grid::Scroll::Delta(lines as i32));
    }

    /// Scroll the display down by the given number of lines
    pub fn scroll_down(&mut self, lines: usize) {
        self.term.grid_mut().scroll_display(alacritty_terminal::grid::Scroll::Delta(-(lines as i32)));
    }

    /// Scroll to the top of history
    pub fn scroll_to_top(&mut self) {
        self.term.grid_mut().scroll_display(alacritty_terminal::grid::Scroll::Top);
    }

    /// Scroll to the bottom (most recent output)
    pub fn scroll_to_bottom(&mut self) {
        self.term.grid_mut().scroll_display(alacritty_terminal::grid::Scroll::Bottom);
    }

    /// Get damaged lines for efficient partial updates
    pub fn damage(&mut self) -> Vec<(usize, usize, usize)> {
        use alacritty_terminal::term::TermDamage;

        match self.term.damage() {
            TermDamage::Full => {
                // Return all lines as damaged
                let lines = self.term.grid().screen_lines();
                let cols = self.term.grid().columns();
                (0..lines).map(|line| (line, 0, cols)).collect()
            }
            TermDamage::Partial(iter) => {
                iter.map(|d| (d.line, d.left, d.right)).collect()
            }
        }
    }

    /// Reset damage tracking
    pub fn reset_damage(&mut self) {
        self.term.reset_damage();
    }

    // =========================================================================
    // Agent Parsing
    // =========================================================================

    /// Enable agent output parsing
    pub fn enable_agent_parsing(&mut self) {
        if self.agent_parser.is_none() {
            self.agent_parser = Some(AgentParser::new());
        }
    }

    /// Disable agent output parsing
    pub fn disable_agent_parsing(&mut self) {
        self.agent_parser = None;
        self.pending_agent_events.clear();
    }

    /// Check if agent parsing is enabled
    pub fn is_agent_parsing_enabled(&self) -> bool {
        self.agent_parser.is_some()
    }

    /// Take pending agent events
    pub fn take_agent_events(&mut self) -> Vec<AgentEvent> {
        std::mem::take(&mut self.pending_agent_events)
    }

    /// Get the currently active agent node (if any)
    pub fn active_agent_node(&self) -> Option<&str> {
        self.agent_parser.as_ref().and_then(|p| p.active_node())
    }

    /// Get the currently active agent tool (if any)
    pub fn active_agent_tool(&self) -> Option<&str> {
        self.agent_parser.as_ref().and_then(|p| p.active_tool())
    }

    /// Clear agent parser state
    pub fn clear_agent_state(&mut self) {
        if let Some(parser) = &mut self.agent_parser {
            parser.clear();
        }
        self.pending_agent_events.clear();
    }
}

impl Clone for DashTermEventListener {
    fn clone(&self) -> Self {
        Self {
            events: Arc::clone(&self.events),
        }
    }
}

/// Convert alacritty cell to our Cell type
fn convert_cell(cell: &alacritty_terminal::term::cell::Cell) -> Cell {
    let mut content = String::new();
    content.push(cell.c);

    // Add any zerowidth characters
    if let Some(zw) = cell.zerowidth() {
        for c in zw {
            content.push(*c);
        }
    }

    let width = if cell.flags.contains(CellFlags::WIDE_CHAR) {
        2
    } else if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
        0
    } else {
        1
    };

    let attrs = CellAttributes {
        foreground: convert_color(cell.fg),
        background: convert_color(cell.bg),
        bold: cell.flags.contains(CellFlags::BOLD),
        italic: cell.flags.contains(CellFlags::ITALIC),
        underline: cell.flags.intersects(CellFlags::ALL_UNDERLINES),
        strikethrough: cell.flags.contains(CellFlags::STRIKEOUT),
        inverse: cell.flags.contains(CellFlags::INVERSE),
        hidden: cell.flags.contains(CellFlags::HIDDEN),
        dim: cell.flags.contains(CellFlags::DIM),
        blink: false,
    };

    Cell {
        content,
        width,
        attrs,
    }
}

/// Convert alacritty color to our Color type
fn convert_color(color: AnsiColor) -> Color {
    match color {
        AnsiColor::Named(named) => Color::Named(named as u8),
        AnsiColor::Spec(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(idx) => Color::Indexed(idx),
    }
}

// Keep backward compatibility - export Grid from this module too
pub use crate::grid::Grid;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_new() {
        let term = Terminal::new(80, 24);
        let size = term.size();
        assert_eq!(size.cols, 80);
        assert_eq!(size.rows, 24);
        assert_eq!(size.scrollback, 10000); // Default scrollback
    }

    #[test]
    fn test_terminal_with_scrollback() {
        let term = Terminal::with_scrollback(120, 40, 5000);
        let size = term.size();
        assert_eq!(size.cols, 120);
        assert_eq!(size.rows, 40);
        assert_eq!(size.scrollback, 5000);
    }

    #[test]
    fn test_terminal_resize() {
        let mut term = Terminal::new(80, 24);
        term.resize(132, 50);
        let size = term.size();
        assert_eq!(size.cols, 132);
        assert_eq!(size.rows, 50);
    }

    #[test]
    fn test_terminal_initial_cursor() {
        let term = Terminal::new(80, 24);
        let (row, col) = term.cursor();
        assert_eq!(row, 0);
        assert_eq!(col, 0);
        assert!(term.cursor_visible());
    }

    #[test]
    fn test_terminal_process_simple_text() {
        let mut term = Terminal::new(80, 24);
        term.process(b"Hello, World!");

        let cells = term.get_cells();
        assert!(!cells.is_empty());

        // Check first cell contains 'H'
        let first_cell = &cells[0][0];
        assert_eq!(first_cell.content, "H");
    }

    #[test]
    fn test_terminal_process_newline() {
        let mut term = Terminal::new(80, 24);
        term.process(b"Line1\r\nLine2");

        let cells = term.get_cells();

        // First row should contain "Line1"
        assert_eq!(cells[0][0].content, "L");
        assert_eq!(cells[0][4].content, "1");

        // Second row should contain "Line2"
        assert_eq!(cells[1][0].content, "L");
        assert_eq!(cells[1][4].content, "2");
    }

    #[test]
    fn test_terminal_cursor_movement() {
        let mut term = Terminal::new(80, 24);

        // Write some text to move cursor
        term.process(b"ABCD");

        let (row, col) = term.cursor();
        assert_eq!(row, 0);
        assert_eq!(col, 4); // Cursor after 4 characters
    }

    #[test]
    fn test_terminal_scroll_up() {
        let mut term = Terminal::new(80, 24);

        // Generate some history by writing many lines
        for i in 0..50 {
            term.process(format!("Line {}\r\n", i).as_bytes());
        }

        // Initial position should be at bottom
        assert_eq!(term.display_offset(), 0);

        // Scroll up
        term.scroll_up(10);
        assert_eq!(term.display_offset(), 10);
    }

    #[test]
    fn test_terminal_scroll_down() {
        let mut term = Terminal::new(80, 24);

        // Generate history
        for i in 0..50 {
            term.process(format!("Line {}\r\n", i).as_bytes());
        }

        // Scroll up first
        term.scroll_up(20);
        assert_eq!(term.display_offset(), 20);

        // Scroll down
        term.scroll_down(10);
        assert_eq!(term.display_offset(), 10);
    }

    #[test]
    fn test_terminal_scroll_to_top() {
        let mut term = Terminal::new(80, 24);

        // Generate history
        for i in 0..50 {
            term.process(format!("Line {}\r\n", i).as_bytes());
        }

        term.scroll_to_top();
        let history_size = term.history_size();
        assert!(term.display_offset() > 0);
        assert_eq!(term.display_offset(), history_size);
    }

    #[test]
    fn test_terminal_scroll_to_bottom() {
        let mut term = Terminal::new(80, 24);

        // Generate history
        for i in 0..50 {
            term.process(format!("Line {}\r\n", i).as_bytes());
        }

        // Scroll up
        term.scroll_up(20);
        assert!(term.display_offset() > 0);

        // Scroll to bottom
        term.scroll_to_bottom();
        assert_eq!(term.display_offset(), 0);
    }

    #[test]
    fn test_terminal_history_size() {
        let mut term = Terminal::new(80, 24);

        // Initially no history
        assert_eq!(term.history_size(), 0);

        // Generate more than 24 lines to create history
        for i in 0..30 {
            term.process(format!("Line {}\r\n", i).as_bytes());
        }

        // Should have history now (30 lines - 24 visible = 6 history)
        // Note: Actual amount depends on terminal behavior
        assert!(term.history_size() > 0);
    }

    #[test]
    fn test_terminal_damage_tracking() {
        let mut term = Terminal::new(80, 24);

        // Process some input
        term.process(b"Test");

        // Get damage
        let damage = term.damage();
        assert!(!damage.is_empty());

        // Reset damage
        term.reset_damage();

        // After reset, damage should show full (depends on implementation)
    }

    #[test]
    fn test_terminal_events() {
        let mut term = Terminal::new(80, 24);
        term.process(b"test");

        // Take events
        let events = term.take_events();

        // Should have at least a Redraw event
        assert!(events.iter().any(|e| matches!(e, TerminalEvent::Redraw)));
    }

    #[test]
    fn test_terminal_title() {
        let term = Terminal::new(80, 24);
        assert_eq!(term.title(), "DashTerm"); // Default title
    }

    #[test]
    fn test_terminal_ansi_color_codes() {
        let mut term = Terminal::new(80, 24);

        // Write text with ANSI color code (red foreground)
        term.process(b"\x1b[31mRed Text\x1b[0m");

        let cells = term.get_cells();
        let first_cell = &cells[0][0];

        // Check that the cell has a red foreground color
        // Named color 1 = red in ANSI
        assert!(matches!(first_cell.attrs.foreground, Color::Named(1)));
    }

    #[test]
    fn test_terminal_bold_text() {
        let mut term = Terminal::new(80, 24);

        // Write bold text
        term.process(b"\x1b[1mBold\x1b[0m");

        let cells = term.get_cells();
        let first_cell = &cells[0][0];

        assert!(first_cell.attrs.bold);
    }

    #[test]
    fn test_terminal_dimensions_trait() {
        let size = TerminalSize::with_scrollback(80, 24, 1000);

        assert_eq!(size.columns(), 80);
        assert_eq!(size.screen_lines(), 24);
        assert_eq!(size.total_lines(), 24 + 1000);
    }

    #[test]
    fn test_terminal_get_cells_dimensions() {
        let term = Terminal::new(80, 24);
        let cells = term.get_cells();

        // Should have exactly 24 rows
        assert_eq!(cells.len(), 24);

        // Each row should have exactly 80 columns
        for row in &cells {
            assert_eq!(row.len(), 80);
        }
    }
}
