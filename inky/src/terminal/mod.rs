//! Terminal abstraction layer.

#[cfg(feature = "dterm")]
mod dterm;
mod events;
pub mod signals;

#[cfg(feature = "dterm")]
pub use dterm::{DtermBackend, DtermGpuBuffer};
pub use events::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, TerminalEvent,
};

use crate::diff::{apply_changes, Change};
use crate::render::Buffer;
use crossterm::{
    cursor,
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{self, ClearType},
};
use std::io::{self, IsTerminal, Stdout, Write};

/// Rendering tier that determines what features are available.
///
/// inky automatically selects the best tier based on detected terminal capabilities.
/// Components can adapt their rendering based on the current tier.
///
/// # Tier Hierarchy
///
/// ```text
/// Tier 3 (GPU)      - dashterm2/dterm: 120 FPS, <1ms latency, GPU shaders
/// Tier 2 (Retained) - iTerm2/Kitty: 60 FPS, true color, synchronized output
/// Tier 1 (ANSI)     - Terminal.app: 30 FPS, 256 colors, basic Unicode
/// Tier 0 (Fallback) - dumb terminals: text only, no colors, CI logs
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum RenderTier {
    /// Tier 0: Dumb terminal fallback.
    ///
    /// - No colors or styling
    /// - ASCII only (no Unicode)
    /// - Text-based representations of visual components
    /// - Safe for CI logs, pipes, and non-interactive use
    Tier0Fallback,

    /// Tier 1: Basic ANSI terminal.
    ///
    /// - 256 colors (ANSI palette)
    /// - Basic Unicode support
    /// - No synchronized output
    /// - Examples: Terminal.app, older terminals
    Tier1Ansi,

    /// Tier 2: Modern terminal with retained mode.
    ///
    /// - True color (24-bit RGB)
    /// - Full Unicode with wide character support
    /// - Synchronized output (no tearing)
    /// - Mouse support
    /// - Examples: iTerm2, Kitty, Alacritty, Windows Terminal
    #[default]
    Tier2Retained,

    /// Tier 3: GPU-accelerated terminal.
    ///
    /// - Direct GPU buffer access
    /// - 120 FPS rendering
    /// - <1ms latency
    /// - Zero-copy operations
    /// - Examples: dashterm2, dterm
    Tier3Gpu,
}

impl RenderTier {
    /// Returns true if this tier supports true color (24-bit RGB).
    #[must_use]
    pub const fn supports_true_color(&self) -> bool {
        matches!(self, Self::Tier2Retained | Self::Tier3Gpu)
    }

    /// Returns true if this tier supports Unicode characters.
    #[must_use]
    pub const fn supports_unicode(&self) -> bool {
        !matches!(self, Self::Tier0Fallback)
    }

    /// Returns true if this tier supports mouse input.
    #[must_use]
    pub const fn supports_mouse(&self) -> bool {
        matches!(self, Self::Tier2Retained | Self::Tier3Gpu)
    }

    /// Returns true if this tier supports synchronized output.
    #[must_use]
    pub const fn supports_sync_output(&self) -> bool {
        matches!(self, Self::Tier2Retained | Self::Tier3Gpu)
    }

    /// Returns true if this tier has GPU acceleration.
    #[must_use]
    pub const fn is_gpu_accelerated(&self) -> bool {
        matches!(self, Self::Tier3Gpu)
    }

    /// Returns a human-readable name for this tier.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Tier0Fallback => "Fallback",
            Self::Tier1Ansi => "ANSI",
            Self::Tier2Retained => "Retained",
            Self::Tier3Gpu => "GPU",
        }
    }

    /// Returns a description of this tier's capabilities.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Tier0Fallback => "Text-only, no colors, safe for CI/pipes",
            Self::Tier1Ansi => "256 colors, basic Unicode",
            Self::Tier2Retained => "True color, synchronized output, mouse support",
            Self::Tier3Gpu => "GPU-accelerated, 120 FPS, <1ms latency",
        }
    }

    /// Returns approximate target FPS for this tier.
    #[must_use]
    pub const fn target_fps(&self) -> u32 {
        match self {
            Self::Tier0Fallback => 10,
            Self::Tier1Ansi => 30,
            Self::Tier2Retained => 60,
            Self::Tier3Gpu => 120,
        }
    }
}

impl std::fmt::Display for RenderTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tier {} ({})", *self as u8, self.name())
    }
}

/// Terminal capabilities detected at runtime.
#[derive(Debug, Clone)]
pub struct Capabilities {
    /// Terminal supports true color (24-bit RGB).
    pub true_color: bool,
    /// Terminal supports Unicode.
    pub unicode: bool,
    /// Terminal supports mouse input.
    pub mouse: bool,
    /// Terminal supports Kitty keyboard protocol.
    pub kitty_keyboard: bool,
    /// Terminal supports synchronized output.
    pub sync_output: bool,
    /// Terminal supports Sixel graphics.
    pub sixel: bool,
    /// Terminal supports Kitty graphics protocol.
    pub kitty_graphics: bool,
    /// Detected terminal name (e.g., "iTerm2", "kitty").
    pub term_name: Option<String>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            true_color: true, // Assume modern terminal
            unicode: true,
            mouse: true,
            kitty_keyboard: false,
            sync_output: false,
            sixel: false,
            kitty_graphics: false,
            term_name: None,
        }
    }
}

impl Capabilities {
    /// Detect terminal capabilities.
    pub fn detect() -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        let term_program = std::env::var("TERM_PROGRAM").ok();

        // Check for dumb terminal / CI environment
        let is_dumb = term == "dumb"
            || std::env::var("CI").is_ok()
            || std::env::var("NO_COLOR").is_ok()
            || !std::io::stdout().is_terminal();

        if is_dumb {
            return Self::tier0_fallback();
        }

        let true_color = colorterm == "truecolor" || colorterm == "24bit";

        // Detect specific terminals for enhanced capabilities
        let is_kitty = term.contains("kitty");
        let is_iterm = term_program.as_deref() == Some("iTerm.app");
        let is_wezterm = term_program.as_deref() == Some("WezTerm");
        let is_alacritty = term_program.as_deref() == Some("Alacritty");

        Self {
            true_color,
            unicode: true,
            mouse: true,
            kitty_keyboard: is_kitty,
            sync_output: is_kitty || is_iterm || is_wezterm || is_alacritty,
            sixel: is_iterm, // iTerm2 supports Sixel
            kitty_graphics: is_kitty,
            term_name: term_program,
        }
    }

    /// Create capabilities for Tier 0 fallback (dumb terminals).
    #[must_use]
    pub fn tier0_fallback() -> Self {
        Self {
            true_color: false,
            unicode: false,
            mouse: false,
            kitty_keyboard: false,
            sync_output: false,
            sixel: false,
            kitty_graphics: false,
            term_name: Some("dumb".to_string()),
        }
    }

    /// Create capabilities for Tier 1 ANSI terminals.
    #[must_use]
    pub fn tier1_ansi() -> Self {
        Self {
            true_color: false,
            unicode: true,
            mouse: false,
            kitty_keyboard: false,
            sync_output: false,
            sixel: false,
            kitty_graphics: false,
            term_name: None,
        }
    }

    /// Create capabilities for Tier 2 modern terminals.
    #[must_use]
    pub fn tier2_retained() -> Self {
        Self {
            true_color: true,
            unicode: true,
            mouse: true,
            kitty_keyboard: false,
            sync_output: true,
            sixel: false,
            kitty_graphics: false,
            term_name: None,
        }
    }

    /// Create capabilities for Tier 3 GPU terminals.
    #[must_use]
    pub fn tier3_gpu() -> Self {
        Self {
            true_color: true,
            unicode: true,
            mouse: true,
            kitty_keyboard: true,
            sync_output: true,
            sixel: true,
            kitty_graphics: true,
            term_name: Some("dterm".to_string()),
        }
    }

    /// Determine the rendering tier based on detected capabilities.
    ///
    /// # Tier Selection Logic
    ///
    /// - **Tier 3**: GPU-accelerated (dterm detected)
    /// - **Tier 2**: True color + synchronized output + mouse
    /// - **Tier 1**: Unicode support but limited colors
    /// - **Tier 0**: No colors, no Unicode (dumb terminal)
    #[must_use]
    pub fn tier(&self) -> RenderTier {
        // Check for dterm/GPU terminal
        if let Some(ref name) = self.term_name {
            if name.contains("dterm") || name.contains("dashterm") {
                return RenderTier::Tier3Gpu;
            }
        }

        // Check for dumb terminal
        if !self.unicode && !self.true_color {
            return RenderTier::Tier0Fallback;
        }

        // Modern terminal with full features
        if self.true_color && self.sync_output {
            return RenderTier::Tier2Retained;
        }

        // Basic ANSI terminal
        RenderTier::Tier1Ansi
    }

    /// Returns true if the terminal is interactive (not a pipe or CI).
    #[must_use]
    pub fn is_interactive(&self) -> bool {
        self.term_name.as_deref() != Some("dumb") && self.unicode
    }
}

/// Terminal backend trait.
pub trait Terminal {
    /// Get terminal size (width, height).
    fn size(&self) -> io::Result<(u16, u16)>;

    /// Enter raw mode.
    fn enter_raw_mode(&mut self) -> io::Result<()>;

    /// Leave raw mode.
    fn leave_raw_mode(&mut self) -> io::Result<()>;

    /// Enter alternate screen buffer.
    fn enter_alt_screen(&mut self) -> io::Result<()>;

    /// Leave alternate screen buffer.
    fn leave_alt_screen(&mut self) -> io::Result<()>;

    /// Show cursor.
    fn show_cursor(&mut self) -> io::Result<()>;

    /// Hide cursor.
    fn hide_cursor(&mut self) -> io::Result<()>;

    /// Move cursor to position.
    fn move_cursor(&mut self, x: u16, y: u16) -> io::Result<()>;

    /// Clear screen.
    fn clear(&mut self) -> io::Result<()>;

    /// Write bytes.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;

    /// Flush output.
    fn flush(&mut self) -> io::Result<()>;

    /// Poll for events (with timeout in milliseconds).
    fn poll_event(&mut self, timeout_ms: u64) -> io::Result<Option<TerminalEvent>>;

    /// Get capabilities.
    fn capabilities(&self) -> &Capabilities;

    /// Begin synchronized output (prevents tearing).
    /// Supported by many modern terminals (kitty, iTerm2, etc).
    fn begin_sync(&mut self) -> io::Result<()>;

    /// End synchronized output and flush.
    fn end_sync(&mut self) -> io::Result<()>;

    /// Enable mouse capture to receive mouse events.
    ///
    /// When enabled, the terminal will report mouse clicks, movement, and scroll events.
    /// Should be paired with `disable_mouse_capture()` on cleanup.
    fn enable_mouse_capture(&mut self) -> io::Result<()>;

    /// Disable mouse capture.
    ///
    /// Call this during cleanup to restore normal terminal mouse behavior.
    fn disable_mouse_capture(&mut self) -> io::Result<()>;

    /// Enable bracketed paste to receive paste events.
    ///
    /// When enabled, pasted text is delivered as a single paste event instead of
    /// raw key presses.
    fn enable_bracketed_paste(&mut self) -> io::Result<()>;

    /// Disable bracketed paste.
    fn disable_bracketed_paste(&mut self) -> io::Result<()>;
}

/// Rendering backend trait.
pub trait Backend {
    /// Access the underlying terminal for input/events.
    fn terminal(&mut self) -> &mut dyn Terminal;

    /// Render the buffer changes to the output backend.
    fn render(&mut self, buffer: &Buffer, changes: &[Change]) -> io::Result<()>;
}

/// Crossterm-based terminal implementation.
pub struct CrosstermTerminal {
    stdout: Stdout,
    raw_mode: bool,
    alt_screen: bool,
    cursor_visible: bool,
    mouse_captured: bool,
    bracketed_paste: bool,
    capabilities: Capabilities,
}

impl CrosstermTerminal {
    /// Create a new crossterm terminal.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            stdout: io::stdout(),
            raw_mode: false,
            alt_screen: false,
            cursor_visible: true,
            mouse_captured: false,
            bracketed_paste: false,
            capabilities: Capabilities::detect(),
        })
    }
}

impl Default for CrosstermTerminal {
    fn default() -> Self {
        Self {
            stdout: io::stdout(),
            raw_mode: false,
            alt_screen: false,
            cursor_visible: true,
            mouse_captured: false,
            bracketed_paste: false,
            capabilities: Capabilities::detect(),
        }
    }
}

impl Terminal for CrosstermTerminal {
    fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    fn enter_raw_mode(&mut self) -> io::Result<()> {
        if !self.raw_mode {
            terminal::enable_raw_mode()?;
            self.raw_mode = true;
        }
        Ok(())
    }

    fn leave_raw_mode(&mut self) -> io::Result<()> {
        if self.raw_mode {
            terminal::disable_raw_mode()?;
            self.raw_mode = false;
        }
        Ok(())
    }

    fn enter_alt_screen(&mut self) -> io::Result<()> {
        if !self.alt_screen {
            execute!(self.stdout, terminal::EnterAlternateScreen)?;
            self.alt_screen = true;
        }
        Ok(())
    }

    fn leave_alt_screen(&mut self) -> io::Result<()> {
        if self.alt_screen {
            execute!(self.stdout, terminal::LeaveAlternateScreen)?;
            self.alt_screen = false;
        }
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        if !self.cursor_visible {
            execute!(self.stdout, cursor::Show)?;
            self.cursor_visible = true;
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        if self.cursor_visible {
            execute!(self.stdout, cursor::Hide)?;
            self.cursor_visible = false;
        }
        Ok(())
    }

    fn move_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        execute!(self.stdout, cursor::MoveTo(x, y))
    }

    fn clear(&mut self) -> io::Result<()> {
        execute!(
            self.stdout,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }

    fn poll_event(&mut self, timeout_ms: u64) -> io::Result<Option<TerminalEvent>> {
        use std::time::Duration;

        if crossterm::event::poll(Duration::from_millis(timeout_ms))? {
            let event = crossterm::event::read()?;
            Ok(Some(TerminalEvent::from(event)))
        } else {
            Ok(None)
        }
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn begin_sync(&mut self) -> io::Result<()> {
        // DEC synchronized output mode: CSI ? 2026 h
        // This is widely supported by modern terminals
        self.stdout.write_all(b"\x1b[?2026h")
    }

    fn end_sync(&mut self) -> io::Result<()> {
        // DEC synchronized output mode end: CSI ? 2026 l
        self.stdout.write_all(b"\x1b[?2026l")?;
        self.stdout.flush()
    }

    fn enable_mouse_capture(&mut self) -> io::Result<()> {
        if !self.mouse_captured {
            execute!(self.stdout, EnableMouseCapture)?;
            self.mouse_captured = true;
        }
        Ok(())
    }

    fn disable_mouse_capture(&mut self) -> io::Result<()> {
        if self.mouse_captured {
            execute!(self.stdout, DisableMouseCapture)?;
            self.mouse_captured = false;
        }
        Ok(())
    }

    fn enable_bracketed_paste(&mut self) -> io::Result<()> {
        if !self.bracketed_paste {
            execute!(self.stdout, EnableBracketedPaste)?;
            self.bracketed_paste = true;
        }
        Ok(())
    }

    fn disable_bracketed_paste(&mut self) -> io::Result<()> {
        if self.bracketed_paste {
            execute!(self.stdout, DisableBracketedPaste)?;
            self.bracketed_paste = false;
        }
        Ok(())
    }
}

/// Crossterm backend with diff-based rendering.
pub struct CrosstermBackend {
    terminal: CrosstermTerminal,
}

impl CrosstermBackend {
    /// Create a new crossterm backend.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            terminal: CrosstermTerminal::new()?,
        })
    }
}

impl Backend for CrosstermBackend {
    fn terminal(&mut self) -> &mut dyn Terminal {
        &mut self.terminal
    }

    fn render(&mut self, _buffer: &Buffer, changes: &[Change]) -> io::Result<()> {
        self.terminal.begin_sync()?;
        let mut stdout = io::stdout();
        apply_changes(&mut stdout, changes)?;
        self.terminal.end_sync()
    }
}

impl Drop for CrosstermTerminal {
    fn drop(&mut self) {
        // Restore terminal state
        let _ = self.disable_mouse_capture();
        let _ = self.disable_bracketed_paste();
        let _ = self.show_cursor();
        let _ = self.leave_alt_screen();
        let _ = self.leave_raw_mode();
    }
}

/// Emergency terminal restore function.
/// Call this in panic hooks to ensure terminal is usable after a crash.
pub fn emergency_restore() {
    use std::io::Write;

    // Best-effort terminal restoration - ignore errors
    let mut stdout = io::stdout();

    // Disable mouse capture
    let _ = execute!(stdout, DisableMouseCapture);

    // Disable bracketed paste
    let _ = execute!(stdout, DisableBracketedPaste);

    // End any synchronized output
    let _ = stdout.write_all(b"\x1b[?2026l");

    // Show cursor
    let _ = execute!(stdout, cursor::Show);

    // Leave alternate screen
    let _ = execute!(stdout, terminal::LeaveAlternateScreen);

    // Disable raw mode
    let _ = terminal::disable_raw_mode();

    // Reset colors and attributes
    let _ = stdout.write_all(b"\x1b[0m");

    // Flush
    let _ = stdout.flush();
}

/// Install a panic hook that restores terminal state before printing panic info.
/// This should be called once at application startup.
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Restore terminal BEFORE printing panic message
        emergency_restore();
        original_hook(info);
    }));
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_detect() {
        // Just verify detection doesn't panic
        let caps = Capabilities::detect();
        // Tier should be valid
        let _tier = caps.tier();
    }

    #[test]
    fn test_capabilities_default() {
        let caps = Capabilities::default();
        assert!(caps.true_color);
        assert!(caps.unicode);
        assert!(caps.mouse);
        assert!(!caps.kitty_keyboard);
        assert!(!caps.sync_output);
    }

    #[test]
    fn test_emergency_restore_doesnt_panic() {
        // Emergency restore should never panic, even when not in a terminal
        emergency_restore();
    }

    #[test]
    fn test_render_tier_ordering() {
        // Tiers should be ordered from lowest to highest capability
        assert!(RenderTier::Tier0Fallback < RenderTier::Tier1Ansi);
        assert!(RenderTier::Tier1Ansi < RenderTier::Tier2Retained);
        assert!(RenderTier::Tier2Retained < RenderTier::Tier3Gpu);
    }

    #[test]
    fn test_render_tier_default() {
        // Default tier is Tier2Retained (modern assumption)
        assert_eq!(RenderTier::default(), RenderTier::Tier2Retained);
    }

    #[test]
    fn test_render_tier_display() {
        assert_eq!(
            format!("{}", RenderTier::Tier0Fallback),
            "Tier 0 (Fallback)"
        );
        assert_eq!(format!("{}", RenderTier::Tier1Ansi), "Tier 1 (ANSI)");
        assert_eq!(
            format!("{}", RenderTier::Tier2Retained),
            "Tier 2 (Retained)"
        );
        assert_eq!(format!("{}", RenderTier::Tier3Gpu), "Tier 3 (GPU)");
    }

    #[test]
    fn test_render_tier_capabilities() {
        // Tier 0 has no advanced features
        assert!(!RenderTier::Tier0Fallback.supports_true_color());
        assert!(!RenderTier::Tier0Fallback.supports_unicode());
        assert!(!RenderTier::Tier0Fallback.supports_mouse());

        // Tier 1 has Unicode but not true color
        assert!(!RenderTier::Tier1Ansi.supports_true_color());
        assert!(RenderTier::Tier1Ansi.supports_unicode());
        assert!(!RenderTier::Tier1Ansi.supports_mouse());

        // Tier 2 has true color and mouse
        assert!(RenderTier::Tier2Retained.supports_true_color());
        assert!(RenderTier::Tier2Retained.supports_unicode());
        assert!(RenderTier::Tier2Retained.supports_mouse());

        // Tier 3 has everything
        assert!(RenderTier::Tier3Gpu.supports_true_color());
        assert!(RenderTier::Tier3Gpu.is_gpu_accelerated());
    }

    #[test]
    fn test_capabilities_tier_detection() {
        // Tier 0 fallback
        let caps = Capabilities::tier0_fallback();
        assert_eq!(caps.tier(), RenderTier::Tier0Fallback);

        // Tier 1 ANSI
        let caps = Capabilities::tier1_ansi();
        assert_eq!(caps.tier(), RenderTier::Tier1Ansi);

        // Tier 2 retained
        let caps = Capabilities::tier2_retained();
        assert_eq!(caps.tier(), RenderTier::Tier2Retained);

        // Tier 3 GPU
        let caps = Capabilities::tier3_gpu();
        assert_eq!(caps.tier(), RenderTier::Tier3Gpu);
    }

    #[test]
    fn test_render_tier_target_fps() {
        assert_eq!(RenderTier::Tier0Fallback.target_fps(), 10);
        assert_eq!(RenderTier::Tier1Ansi.target_fps(), 30);
        assert_eq!(RenderTier::Tier2Retained.target_fps(), 60);
        assert_eq!(RenderTier::Tier3Gpu.target_fps(), 120);
    }

    #[test]
    fn test_capabilities_is_interactive() {
        let caps = Capabilities::tier0_fallback();
        assert!(!caps.is_interactive());

        let caps = Capabilities::tier2_retained();
        assert!(caps.is_interactive());
    }
}
