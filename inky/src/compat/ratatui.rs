//! Ratatui compatibility layer.
//!
//! This module provides types and traits that allow inky to be used as a
//! drop-in replacement for ratatui backends, enabling gradual migration.
//!
//! # Example
//!
//! ```rust,ignore
//! use inky::compat::ratatui::{InkyBackend, TerminalBackend};
//!
//! // Create an inky backend that implements the TerminalBackend trait
//! let mut backend = InkyBackend::new()?;
//!
//! // Use it with ratatui-compatible code
//! backend.clear()?;
//! backend.draw(cells.into_iter())?;
//! backend.flush()?;
//! ```

use crate::render::{Buffer, Cell, CellFlags, PackedColor};
use crate::style::Color;
use crate::terminal::{CrosstermTerminal, Terminal};
use std::io::{self, Write};
use std::ops::Range;

/// Error type for terminal backend operations.
#[derive(Debug)]
pub enum BackendError {
    /// I/O error from the terminal.
    Io(io::Error),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for BackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
        }
    }
}

impl From<io::Error> for BackendError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// A cell for the terminal backend, compatible with ratatui's Cell type.
///
/// This is a simple wrapper that can be used to pass cells to the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCell {
    /// The character to display.
    pub symbol: char,
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Bold attribute.
    pub bold: bool,
    /// Italic attribute.
    pub italic: bool,
    /// Underline attribute.
    pub underline: bool,
    /// Dim attribute.
    pub dim: bool,
    /// Strikethrough attribute.
    pub strikethrough: bool,
}

impl Default for BackendCell {
    fn default() -> Self {
        Self {
            symbol: ' ',
            fg: Color::White,
            bg: Color::Default,
            bold: false,
            italic: false,
            underline: false,
            dim: false,
            strikethrough: false,
        }
    }
}

impl BackendCell {
    /// Create a new cell with the given character.
    #[must_use]
    pub fn new(symbol: char) -> Self {
        Self {
            symbol,
            ..Default::default()
        }
    }

    /// Convert to inky Cell.
    #[must_use]
    pub fn to_cell(&self) -> Cell {
        let mut cell = Cell::new(self.symbol);
        cell.set_fg(PackedColor::from(self.fg));
        cell.set_bg(PackedColor::from(self.bg));

        let mut flags = CellFlags::empty();
        if self.bold {
            flags |= CellFlags::BOLD;
        }
        if self.italic {
            flags |= CellFlags::ITALIC;
        }
        if self.underline {
            flags |= CellFlags::UNDERLINE;
        }
        if self.dim {
            flags |= CellFlags::DIM;
        }
        if self.strikethrough {
            flags |= CellFlags::STRIKETHROUGH;
        }
        cell.flags = flags;

        cell
    }
}

/// Terminal size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
}

/// Window size with pixel dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    /// Width in columns.
    pub columns: u16,
    /// Height in rows.
    pub rows: u16,
    /// Width in pixels (if available).
    pub pixel_width: u16,
    /// Height in pixels (if available).
    pub pixel_height: u16,
}

/// Cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Column (x).
    pub x: u16,
    /// Row (y).
    pub y: u16,
}

impl Position {
    /// Create a new position.
    #[must_use]
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

impl From<(u16, u16)> for Position {
    fn from((x, y): (u16, u16)) -> Self {
        Self { x, y }
    }
}

/// Clear type for terminal clearing operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearType {
    /// Clear the entire screen.
    All,
    /// Clear from cursor to end of screen.
    AfterCursor,
    /// Clear from start of screen to cursor.
    BeforeCursor,
    /// Clear the current line.
    CurrentLine,
    /// Clear from cursor to end of line.
    UntilNewLine,
}

/// Terminal backend trait for ratatui compatibility.
///
/// This trait mirrors ratatui's Backend trait, allowing inky to be used
/// as a drop-in replacement for gradual migration.
pub trait TerminalBackend {
    /// The error type for backend operations.
    type Error: std::error::Error;

    /// Draw content to the terminal.
    ///
    /// The iterator yields (x, y, cell) tuples.
    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a BackendCell)>;

    /// Hide the cursor.
    fn hide_cursor(&mut self) -> Result<(), Self::Error>;

    /// Show the cursor.
    fn show_cursor(&mut self) -> Result<(), Self::Error>;

    /// Get the current cursor position.
    fn get_cursor_position(&mut self) -> Result<Position, Self::Error>;

    /// Set the cursor position.
    fn set_cursor_position(&mut self, position: Position) -> Result<(), Self::Error>;

    /// Clear the screen.
    fn clear(&mut self) -> Result<(), Self::Error>;

    /// Clear a specific region of the screen.
    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error>;

    /// Get the terminal size.
    fn size(&self) -> Result<Size, Self::Error>;

    /// Get the window size including pixel dimensions.
    fn window_size(&mut self) -> Result<WindowSize, Self::Error>;

    /// Flush output to the terminal.
    fn flush(&mut self) -> Result<(), Self::Error>;

    /// Scroll a region up.
    fn scroll_region_up(&mut self, region: Range<u16>, line_count: u16) -> Result<(), Self::Error>;

    /// Scroll a region down.
    fn scroll_region_down(
        &mut self,
        region: Range<u16>,
        line_count: u16,
    ) -> Result<(), Self::Error>;
}

/// Inky backend that implements the TerminalBackend trait.
///
/// This allows using inky as a drop-in replacement for ratatui backends.
pub struct InkyBackend {
    terminal: CrosstermTerminal,
    buffer: Buffer,
    cursor_position: Position,
}

impl InkyBackend {
    /// Create a new inky backend.
    pub fn new() -> Result<Self, BackendError> {
        let terminal = CrosstermTerminal::new()?;
        let (width, height) = terminal.size()?;
        Ok(Self {
            terminal,
            buffer: Buffer::new(width, height),
            cursor_position: Position::new(0, 0),
        })
    }

    /// Get mutable access to the underlying buffer.
    ///
    /// This allows direct buffer manipulation for advanced use cases.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Get read-only access to the underlying buffer.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Get access to the underlying terminal.
    pub fn terminal(&self) -> &CrosstermTerminal {
        &self.terminal
    }

    /// Get mutable access to the underlying terminal.
    pub fn terminal_mut(&mut self) -> &mut CrosstermTerminal {
        &mut self.terminal
    }

    /// Initialize the terminal for rendering.
    ///
    /// This enters raw mode, alternate screen, and hides the cursor.
    pub fn init(&mut self) -> Result<(), BackendError> {
        self.terminal.enter_raw_mode()?;
        self.terminal.enter_alt_screen()?;
        self.terminal.hide_cursor()?;
        self.terminal.enable_mouse_capture()?;
        Ok(())
    }

    /// Restore the terminal to its original state.
    pub fn restore(&mut self) -> Result<(), BackendError> {
        self.terminal.disable_mouse_capture()?;
        self.terminal.show_cursor()?;
        self.terminal.leave_alt_screen()?;
        self.terminal.leave_raw_mode()?;
        Ok(())
    }

    /// Render the buffer to the terminal with diff-based updates.
    ///
    /// This performs efficient incremental rendering by only updating
    /// cells that have changed since the last render.
    pub fn render(&mut self) -> Result<(), BackendError> {
        use std::io::stdout;

        // Create a blank buffer for comparison (first render)
        // In practice, you'd keep the previous buffer for true diff-based rendering
        let (width, height) = self.terminal.size()?;
        if width != self.buffer.width() || height != self.buffer.height() {
            self.buffer.resize(width, height);
        }

        // For now, render all dirty cells
        self.terminal.begin_sync()?;

        let mut stdout = stdout();
        for y in 0..self.buffer.height() {
            if let Some(row) = self.buffer.row(y) {
                for (x, cell) in row.iter().enumerate() {
                    if cell.is_dirty() {
                        // Move cursor and render cell
                        crossterm::execute!(
                            stdout,
                            crossterm::cursor::MoveTo(x as u16, y),
                            crossterm::style::SetForegroundColor(to_crossterm_color(cell.fg())),
                            crossterm::style::SetBackgroundColor(to_crossterm_color(cell.bg())),
                        )?;

                        // Set attributes
                        if cell.flags.contains(CellFlags::BOLD) {
                            crossterm::execute!(
                                stdout,
                                crossterm::style::SetAttribute(crossterm::style::Attribute::Bold)
                            )?;
                        }
                        if cell.flags.contains(CellFlags::DIM) {
                            crossterm::execute!(
                                stdout,
                                crossterm::style::SetAttribute(crossterm::style::Attribute::Dim)
                            )?;
                        }
                        if cell.flags.contains(CellFlags::ITALIC) {
                            crossterm::execute!(
                                stdout,
                                crossterm::style::SetAttribute(crossterm::style::Attribute::Italic)
                            )?;
                        }
                        if cell.flags.contains(CellFlags::UNDERLINE) {
                            crossterm::execute!(
                                stdout,
                                crossterm::style::SetAttribute(
                                    crossterm::style::Attribute::Underlined
                                )
                            )?;
                        }
                        if cell.flags.contains(CellFlags::STRIKETHROUGH) {
                            crossterm::execute!(
                                stdout,
                                crossterm::style::SetAttribute(
                                    crossterm::style::Attribute::CrossedOut
                                )
                            )?;
                        }

                        // Print character
                        write!(stdout, "{}", cell.char())?;

                        // Reset attributes
                        crossterm::execute!(
                            stdout,
                            crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)
                        )?;
                    }
                }
            }
        }

        self.buffer.clear_dirty();
        self.terminal.end_sync()?;
        Ok(())
    }
}

impl Default for InkyBackend {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            terminal: CrosstermTerminal::default(),
            buffer: Buffer::new(80, 24),
            cursor_position: Position::new(0, 0),
        })
    }
}

impl TerminalBackend for InkyBackend {
    type Error = BackendError;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a BackendCell)>,
    {
        for (x, y, cell) in content {
            self.buffer.write_cell(x, y, cell.to_cell());
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.terminal.hide_cursor()?;
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.terminal.show_cursor()?;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        Ok(self.cursor_position)
    }

    fn set_cursor_position(&mut self, position: Position) -> Result<(), Self::Error> {
        self.terminal.move_cursor(position.x, position.y)?;
        self.cursor_position = position;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.buffer.clear();
        self.terminal.clear()?;
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        use crossterm::terminal::{Clear, ClearType as CtClearType};
        let ct_clear = match clear_type {
            ClearType::All => CtClearType::All,
            ClearType::AfterCursor => CtClearType::FromCursorDown,
            ClearType::BeforeCursor => CtClearType::FromCursorUp,
            ClearType::CurrentLine => CtClearType::CurrentLine,
            ClearType::UntilNewLine => CtClearType::UntilNewLine,
        };
        crossterm::execute!(std::io::stdout(), Clear(ct_clear))?;
        Ok(())
    }

    fn size(&self) -> Result<Size, Self::Error> {
        let (width, height) = self.terminal.size()?;
        Ok(Size { width, height })
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        let (width, height) = self.terminal.size()?;
        Ok(WindowSize {
            columns: width,
            rows: height,
            // Pixel dimensions not available through crossterm
            pixel_width: 0,
            pixel_height: 0,
        })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.render()?;
        self.terminal.flush()?;
        Ok(())
    }

    fn scroll_region_up(&mut self, region: Range<u16>, line_count: u16) -> Result<(), Self::Error> {
        // Scroll the specified region up by copying rows
        let start = region.start;
        let end = region.end.min(self.buffer.height());

        if start >= end || line_count == 0 {
            return Ok(());
        }

        // Copy rows up
        for y in start..(end.saturating_sub(line_count)) {
            let src_y = y + line_count;
            if src_y < end {
                // Copy row
                for x in 0..self.buffer.width() {
                    if let Some(cell) = self.buffer.get(x, src_y).copied() {
                        self.buffer.write_cell(x, y, cell);
                    }
                }
            }
        }

        // Clear the bottom lines
        let clear_start = end.saturating_sub(line_count);
        let blank = Cell::blank();
        for y in clear_start..end {
            for x in 0..self.buffer.width() {
                self.buffer.write_cell(x, y, blank);
            }
        }

        Ok(())
    }

    fn scroll_region_down(
        &mut self,
        region: Range<u16>,
        line_count: u16,
    ) -> Result<(), Self::Error> {
        // Scroll the specified region down by copying rows
        let start = region.start;
        let end = region.end.min(self.buffer.height());

        if start >= end || line_count == 0 {
            return Ok(());
        }

        // Copy rows down (iterate in reverse to avoid overwriting)
        for y in ((start + line_count)..end).rev() {
            let src_y = y.saturating_sub(line_count);
            if src_y >= start {
                // Copy row
                for x in 0..self.buffer.width() {
                    if let Some(cell) = self.buffer.get(x, src_y).copied() {
                        self.buffer.write_cell(x, y, cell);
                    }
                }
            }
        }

        // Clear the top lines
        let clear_end = (start + line_count).min(end);
        let blank = Cell::blank();
        for y in start..clear_end {
            for x in 0..self.buffer.width() {
                self.buffer.write_cell(x, y, blank);
            }
        }

        Ok(())
    }
}

/// Convert PackedColor to crossterm Color.
fn to_crossterm_color(color: PackedColor) -> crossterm::style::Color {
    crossterm::style::Color::Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

// =============================================================================
// Ratatui Backend Implementation (requires `compat-ratatui` feature)
// =============================================================================

#[cfg(feature = "compat-ratatui")]
mod ratatui_backend {
    //! Ratatui-compatible backend implementation.
    //!
    //! This module provides `RatatuiBackend`, which implements ratatui's `Backend`
    //! trait, allowing inky to be used as a drop-in replacement for
    //! `CrosstermBackend` or any other ratatui backend.
    //!
    //! # Example
    //!
    //! ```rust,ignore
    //! use inky::compat::ratatui::RatatuiBackend;
    //! use ratatui::Terminal;
    //!
    //! // Create an inky backend that implements ratatui's Backend trait
    //! let backend = RatatuiBackend::new()?;
    //! let mut terminal = Terminal::new(backend)?;
    //!
    //! // Use it exactly like any other ratatui backend
    //! terminal.draw(|f| {
    //!     // ... render your UI
    //! })?;
    //! ```

    use super::{
        to_crossterm_color, BackendError, Buffer, Cell, CellFlags, Color, CrosstermTerminal,
        PackedColor, Terminal,
    };
    use ratatui::backend::Backend;
    use ratatui::buffer::Cell as RatatuiCell;
    use ratatui::layout::{Position as RatPosition, Size as RatSize};
    use ratatui::style::{Color as RatColor, Modifier};
    use std::io::{self, Write};

    /// Ratatui-compatible backend that uses inky's rendering engine.
    ///
    /// This backend implements ratatui's `Backend` trait, enabling drop-in
    /// replacement of `CrosstermBackend` with inky's renderer.
    ///
    /// # Features
    ///
    /// - Full ratatui API compatibility
    /// - Efficient incremental rendering via inky's diff-based updates
    /// - Direct buffer access for advanced use cases
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use inky::compat::ratatui::RatatuiBackend;
    /// use ratatui::Terminal;
    ///
    /// let backend = RatatuiBackend::new()?;
    /// let mut terminal = Terminal::new(backend)?;
    /// ```
    pub struct RatatuiBackend {
        terminal: CrosstermTerminal,
        buffer: Buffer,
    }

    impl RatatuiBackend {
        /// Create a new ratatui-compatible backend.
        ///
        /// # Errors
        ///
        /// Returns an error if terminal initialization fails.
        pub fn new() -> Result<Self, BackendError> {
            let terminal = CrosstermTerminal::new()?;
            let (width, height) = terminal.size()?;
            Ok(Self {
                terminal,
                buffer: Buffer::new(width, height),
            })
        }

        /// Get mutable access to the underlying inky buffer.
        ///
        /// This allows direct buffer manipulation for advanced use cases.
        pub fn buffer_mut(&mut self) -> &mut Buffer {
            &mut self.buffer
        }

        /// Get read-only access to the underlying inky buffer.
        pub fn buffer(&self) -> &Buffer {
            &self.buffer
        }

        /// Get access to the underlying terminal.
        pub fn terminal(&self) -> &CrosstermTerminal {
            &self.terminal
        }

        /// Get mutable access to the underlying terminal.
        pub fn terminal_mut(&mut self) -> &mut CrosstermTerminal {
            &mut self.terminal
        }

        /// Render dirty cells to the terminal.
        fn render_dirty_cells(&mut self) -> Result<(), BackendError> {
            use std::io::stdout;

            self.terminal.begin_sync()?;

            let mut stdout = stdout();
            for y in 0..self.buffer.height() {
                if let Some(row) = self.buffer.row(y) {
                    for (x, cell) in row.iter().enumerate() {
                        if cell.is_dirty() {
                            // Move cursor and render cell
                            crossterm::execute!(
                                stdout,
                                crossterm::cursor::MoveTo(x as u16, y),
                                crossterm::style::SetForegroundColor(to_crossterm_color(cell.fg())),
                                crossterm::style::SetBackgroundColor(to_crossterm_color(cell.bg())),
                            )?;

                            // Set attributes
                            if cell.flags.contains(CellFlags::BOLD) {
                                crossterm::execute!(
                                    stdout,
                                    crossterm::style::SetAttribute(
                                        crossterm::style::Attribute::Bold
                                    )
                                )?;
                            }
                            if cell.flags.contains(CellFlags::DIM) {
                                crossterm::execute!(
                                    stdout,
                                    crossterm::style::SetAttribute(
                                        crossterm::style::Attribute::Dim
                                    )
                                )?;
                            }
                            if cell.flags.contains(CellFlags::ITALIC) {
                                crossterm::execute!(
                                    stdout,
                                    crossterm::style::SetAttribute(
                                        crossterm::style::Attribute::Italic
                                    )
                                )?;
                            }
                            if cell.flags.contains(CellFlags::UNDERLINE) {
                                crossterm::execute!(
                                    stdout,
                                    crossterm::style::SetAttribute(
                                        crossterm::style::Attribute::Underlined
                                    )
                                )?;
                            }
                            if cell.flags.contains(CellFlags::STRIKETHROUGH) {
                                crossterm::execute!(
                                    stdout,
                                    crossterm::style::SetAttribute(
                                        crossterm::style::Attribute::CrossedOut
                                    )
                                )?;
                            }

                            // Print character
                            write!(stdout, "{}", cell.char())?;

                            // Reset attributes
                            crossterm::execute!(
                                stdout,
                                crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)
                            )?;
                        }
                    }
                }
            }

            self.buffer.clear_dirty();
            self.terminal.end_sync()?;
            Ok(())
        }
    }

    impl Default for RatatuiBackend {
        fn default() -> Self {
            Self::new().unwrap_or_else(|_| Self {
                terminal: CrosstermTerminal::default(),
                buffer: Buffer::new(80, 24),
            })
        }
    }

    /// Convert ratatui Color to inky Color.
    fn rat_color_to_inky(color: RatColor) -> Color {
        match color {
            RatColor::Reset => Color::Default,
            RatColor::Black => Color::Black,
            RatColor::Red => Color::Red,
            RatColor::Green => Color::Green,
            RatColor::Yellow => Color::Yellow,
            RatColor::Blue => Color::Blue,
            RatColor::Magenta => Color::Magenta,
            RatColor::Cyan => Color::Cyan,
            RatColor::Gray => Color::White,
            RatColor::DarkGray => Color::BrightBlack,
            RatColor::LightRed => Color::BrightRed,
            RatColor::LightGreen => Color::BrightGreen,
            RatColor::LightYellow => Color::BrightYellow,
            RatColor::LightBlue => Color::BrightBlue,
            RatColor::LightMagenta => Color::BrightMagenta,
            RatColor::LightCyan => Color::BrightCyan,
            RatColor::White => Color::BrightWhite,
            RatColor::Indexed(n) => Color::Ansi256(n),
            RatColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        }
    }

    /// Convert ratatui Cell to inky Cell.
    fn rat_cell_to_inky(rat_cell: &RatatuiCell) -> Cell {
        let symbol = rat_cell.symbol().chars().next().unwrap_or(' ');
        let mut cell = Cell::new(symbol);

        // Convert colors
        cell.set_fg(PackedColor::from(rat_color_to_inky(rat_cell.fg)));
        cell.set_bg(PackedColor::from(rat_color_to_inky(rat_cell.bg)));

        // Convert modifiers
        let mods = rat_cell.modifier;
        let mut flags = CellFlags::empty();
        if mods.contains(Modifier::BOLD) {
            flags |= CellFlags::BOLD;
        }
        if mods.contains(Modifier::DIM) {
            flags |= CellFlags::DIM;
        }
        if mods.contains(Modifier::ITALIC) {
            flags |= CellFlags::ITALIC;
        }
        if mods.contains(Modifier::UNDERLINED) {
            flags |= CellFlags::UNDERLINE;
        }
        if mods.contains(Modifier::CROSSED_OUT) {
            flags |= CellFlags::STRIKETHROUGH;
        }
        cell.flags = flags;

        cell
    }

    impl Backend for RatatuiBackend {
        fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
        where
            I: Iterator<Item = (u16, u16, &'a RatatuiCell)>,
        {
            for (x, y, rat_cell) in content {
                let inky_cell = rat_cell_to_inky(rat_cell);
                self.buffer.write_cell(x, y, inky_cell);
            }
            Ok(())
        }

        fn hide_cursor(&mut self) -> io::Result<()> {
            self.terminal.hide_cursor()
        }

        fn show_cursor(&mut self) -> io::Result<()> {
            self.terminal.show_cursor()
        }

        fn get_cursor_position(&mut self) -> io::Result<RatPosition> {
            // Query actual cursor position from terminal
            let pos = crossterm::cursor::position()?;
            Ok(RatPosition::new(pos.0, pos.1))
        }

        fn set_cursor_position<P: Into<RatPosition>>(&mut self, position: P) -> io::Result<()> {
            let pos = position.into();
            self.terminal.move_cursor(pos.x, pos.y)
        }

        fn clear(&mut self) -> io::Result<()> {
            self.buffer.clear();
            self.terminal.clear()
        }

        fn size(&self) -> io::Result<RatSize> {
            let (width, height) = self.terminal.size()?;
            Ok(RatSize::new(width, height))
        }

        fn window_size(&mut self) -> io::Result<ratatui::backend::WindowSize> {
            let (width, height) = self.terminal.size()?;
            Ok(ratatui::backend::WindowSize {
                columns_rows: RatSize::new(width, height),
                // Pixel dimensions not available
                pixels: RatSize::new(0, 0),
            })
        }

        fn flush(&mut self) -> io::Result<()> {
            // Resize buffer if terminal size changed
            if let Ok((width, height)) = self.terminal.size() {
                if width != self.buffer.width() || height != self.buffer.height() {
                    self.buffer.resize(width, height);
                }
            }

            // Render dirty cells
            self.render_dirty_cells().map_err(|e| match e {
                BackendError::Io(io_err) => io_err,
            })?;

            // Flush terminal
            self.terminal.flush()
        }

        #[cfg(feature = "scrolling-regions")]
        fn scroll_region_up(
            &mut self,
            region: std::ops::Range<u16>,
            line_count: u16,
        ) -> io::Result<()> {
            let start = region.start;
            let end = region.end.min(self.buffer.height());

            if start >= end || line_count == 0 {
                return Ok(());
            }

            // Copy rows up
            for y in start..(end.saturating_sub(line_count)) {
                let src_y = y + line_count;
                if src_y < end {
                    for x in 0..self.buffer.width() {
                        if let Some(cell) = self.buffer.get(x, src_y).copied() {
                            self.buffer.write_cell(x, y, cell);
                        }
                    }
                }
            }

            // Clear the bottom lines
            let clear_start = end.saturating_sub(line_count);
            let blank = Cell::blank();
            for y in clear_start..end {
                for x in 0..self.buffer.width() {
                    self.buffer.write_cell(x, y, blank);
                }
            }

            Ok(())
        }

        #[cfg(feature = "scrolling-regions")]
        fn scroll_region_down(
            &mut self,
            region: std::ops::Range<u16>,
            line_count: u16,
        ) -> io::Result<()> {
            let start = region.start;
            let end = region.end.min(self.buffer.height());

            if start >= end || line_count == 0 {
                return Ok(());
            }

            // Copy rows down (iterate in reverse to avoid overwriting)
            for y in ((start + line_count)..end).rev() {
                let src_y = y.saturating_sub(line_count);
                if src_y >= start {
                    for x in 0..self.buffer.width() {
                        if let Some(cell) = self.buffer.get(x, src_y).copied() {
                            self.buffer.write_cell(x, y, cell);
                        }
                    }
                }
            }

            // Clear the top lines
            let clear_end = (start + line_count).min(end);
            let blank = Cell::blank();
            for y in start..clear_end {
                for x in 0..self.buffer.width() {
                    self.buffer.write_cell(x, y, blank);
                }
            }

            Ok(())
        }
    }

    impl Write for RatatuiBackend {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.terminal.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.terminal.flush()
        }
    }

    impl RatatuiBackend {
        /// Scroll the buffer up by the specified number of lines.
        ///
        /// This is useful for terminal scrolling operations.
        pub fn scroll_up(&mut self, lines: u16) {
            let height = self.buffer.height();
            // Scroll the entire screen region up
            for y in 0..(height.saturating_sub(lines)) {
                let src_y = y + lines;
                for x in 0..self.buffer.width() {
                    if let Some(cell) = self.buffer.get(x, src_y).copied() {
                        self.buffer.write_cell(x, y, cell);
                    }
                }
            }
            // Clear bottom lines
            let blank = Cell::blank();
            for y in height.saturating_sub(lines)..height {
                for x in 0..self.buffer.width() {
                    self.buffer.write_cell(x, y, blank);
                }
            }
        }

        /// Scroll the buffer down by the specified number of lines.
        ///
        /// This is useful for terminal scrolling operations.
        pub fn scroll_down(&mut self, lines: u16) {
            let height = self.buffer.height();
            // Scroll the entire screen region down (iterate in reverse)
            for y in (lines..height).rev() {
                let src_y = y.saturating_sub(lines);
                for x in 0..self.buffer.width() {
                    if let Some(cell) = self.buffer.get(x, src_y).copied() {
                        self.buffer.write_cell(x, y, cell);
                    }
                }
            }
            // Clear top lines
            let blank = Cell::blank();
            for y in 0..lines.min(height) {
                for x in 0..self.buffer.width() {
                    self.buffer.write_cell(x, y, blank);
                }
            }
        }
    }
}

#[cfg(feature = "compat-ratatui")]
pub use ratatui_backend::RatatuiBackend;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_cell_default() {
        let cell = BackendCell::default();
        assert_eq!(cell.symbol, ' ');
        assert!(!cell.bold);
        assert!(!cell.italic);
    }

    #[test]
    fn test_backend_cell_to_cell() {
        let mut bc = BackendCell::new('X');
        bc.fg = Color::Red;
        bc.bold = true;

        let cell = bc.to_cell();
        assert_eq!(cell.char(), 'X');
        assert!(cell.flags.contains(CellFlags::BOLD));
    }

    #[test]
    fn test_position_from_tuple() {
        let pos: Position = (10, 20).into();
        assert_eq!(pos.x, 10);
        assert_eq!(pos.y, 20);
    }

    #[test]
    fn test_backend_error_display() {
        let err = BackendError::Io(io::Error::new(io::ErrorKind::Other, "test"));
        let s = format!("{}", err);
        assert!(s.contains("I/O error"));
    }

    #[test]
    fn test_size_struct() {
        let size = Size {
            width: 80,
            height: 24,
        };
        assert_eq!(size.width, 80);
        assert_eq!(size.height, 24);
    }

    #[test]
    fn test_window_size_struct() {
        let ws = WindowSize {
            columns: 80,
            rows: 24,
            pixel_width: 0,
            pixel_height: 0,
        };
        assert_eq!(ws.columns, 80);
        assert_eq!(ws.rows, 24);
    }
}
