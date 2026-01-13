//! Configuration management for dterm-core terminals.
//!
//! This module provides runtime configuration hot-reload support, allowing
//! integrators to change terminal settings without recreating the terminal.
//!
//! # Example
//!
//! ```
//! use dterm_core::config::{TerminalConfig, ConfigChange};
//! use dterm_core::terminal::Terminal;
//!
//! let mut term = Terminal::new(24, 80);
//!
//! // Create a new configuration
//! let mut config = TerminalConfig::default();
//! config.scrollback_limit = 50_000;
//! config.cursor_style = dterm_core::terminal::CursorStyle::SteadyBar;
//! config.cursor_blink = false;
//!
//! // Apply configuration changes
//! let changes = term.apply_config(&config);
//!
//! // Check what changed
//! for change in changes {
//!     println!("Changed: {:?}", change);
//! }
//! ```
//!
//! # Observer Pattern
//!
//! For reactive UI updates, implement `ConfigObserver`:
//!
//! ```ignore
//! use dterm_core::config::{ConfigObserver, ConfigChange};
//!
//! struct MyObserver;
//!
//! impl ConfigObserver for MyObserver {
//!     fn on_config_changed(&mut self, changes: &[ConfigChange]) {
//!         for change in changes {
//!             match change {
//!                 ConfigChange::Colors => self.update_color_scheme(),
//!                 ConfigChange::CursorStyle => self.update_cursor_appearance(),
//!                 _ => {}
//!             }
//!         }
//!     }
//! }
//! ```

use crate::terminal::{ColorPalette, CursorStyle, Rgb};

/// Terminal configuration settings.
///
/// This struct bundles all configurable aspects of a terminal that can be
/// changed at runtime without recreating the terminal instance.
///
/// # Configuration Categories
///
/// - **Display**: Cursor style, cursor blink, reverse video
/// - **Colors**: Foreground, background, cursor color, palette
/// - **Behavior**: Scrollback limit, auto-wrap, focus reporting
/// - **Performance**: Memory budget, sync timeout
///
/// # Thread Safety
///
/// `TerminalConfig` is `Send + Sync` and can be safely shared between threads.
/// The actual application of configuration to a terminal requires mutable
/// access to the terminal.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalConfig {
    // === Display Settings ===
    /// Cursor style (block, underline, bar).
    pub cursor_style: CursorStyle,

    /// Whether the cursor should blink.
    pub cursor_blink: bool,

    /// Cursor color override (None uses default from color scheme).
    pub cursor_color: Option<Rgb>,

    /// Whether cursor is visible (DECTCEM mode 25).
    pub cursor_visible: bool,

    // === Color Settings ===
    /// Default foreground color.
    pub default_foreground: Rgb,

    /// Default background color.
    pub default_background: Rgb,

    /// Custom color palette (if any).
    /// When `None`, uses the default xterm 256-color palette.
    pub custom_palette: Option<ColorPalette>,

    // === Behavior Settings ===
    /// Maximum scrollback history in lines.
    /// When the scrollback exceeds this limit, older lines are discarded
    /// (or moved to cold storage if configured).
    pub scrollback_limit: usize,

    /// Auto-wrap mode (DECAWM mode 7).
    /// When enabled, lines wrap at the right margin.
    pub auto_wrap: bool,

    /// Focus reporting mode (mode 1004).
    /// When enabled, terminal sends focus/blur notifications.
    pub focus_reporting: bool,

    /// Bracketed paste mode (mode 2004).
    /// When enabled, pasted text is wrapped in escape sequences.
    pub bracketed_paste: bool,

    // === Performance Settings ===
    /// Memory budget for scrollback (in bytes).
    /// This controls when lines are moved from hot to warm to cold storage.
    pub memory_budget: usize,

    /// Synchronized output timeout in milliseconds.
    /// How long to wait before forcing sync mode off.
    pub sync_timeout_ms: u64,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            // Display
            cursor_style: CursorStyle::BlinkingBlock,
            cursor_blink: true,
            cursor_color: None,
            cursor_visible: true,
            // Colors
            default_foreground: Rgb::new(255, 255, 255), // White
            default_background: Rgb::new(0, 0, 0),       // Black
            custom_palette: None,
            // Behavior
            scrollback_limit: 10_000,
            auto_wrap: true,
            focus_reporting: false,
            bracketed_paste: false,
            // Performance
            memory_budget: 100 * 1024 * 1024, // 100 MB
            sync_timeout_ms: 1000,            // 1 second
        }
    }
}

impl TerminalConfig {
    /// Create a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration builder for fluent API.
    #[must_use]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    /// Compare with another config and return list of changes.
    ///
    /// This is useful for determining what UI elements need to be updated
    /// after a configuration change.
    #[must_use]
    pub fn diff(&self, other: &Self) -> Vec<ConfigChange> {
        let mut changes = Vec::new();

        if self.cursor_style != other.cursor_style {
            changes.push(ConfigChange::CursorStyle);
        }
        if self.cursor_blink != other.cursor_blink {
            changes.push(ConfigChange::CursorBlink);
        }
        if self.cursor_color != other.cursor_color {
            changes.push(ConfigChange::CursorColor);
        }
        if self.cursor_visible != other.cursor_visible {
            changes.push(ConfigChange::CursorVisible);
        }
        if self.default_foreground != other.default_foreground
            || self.default_background != other.default_background
            || self.custom_palette != other.custom_palette
        {
            changes.push(ConfigChange::Colors);
        }
        if self.scrollback_limit != other.scrollback_limit {
            changes.push(ConfigChange::ScrollbackLimit);
        }
        if self.auto_wrap != other.auto_wrap {
            changes.push(ConfigChange::AutoWrap);
        }
        if self.focus_reporting != other.focus_reporting {
            changes.push(ConfigChange::FocusReporting);
        }
        if self.bracketed_paste != other.bracketed_paste {
            changes.push(ConfigChange::BracketedPaste);
        }
        if self.memory_budget != other.memory_budget {
            changes.push(ConfigChange::MemoryBudget);
        }
        if self.sync_timeout_ms != other.sync_timeout_ms {
            changes.push(ConfigChange::SyncTimeout);
        }

        changes
    }
}

/// Types of configuration changes.
///
/// Used to identify what aspects of the terminal configuration have changed,
/// enabling efficient UI updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigChange {
    /// Cursor style changed (block, underline, bar).
    CursorStyle,
    /// Cursor blink setting changed.
    CursorBlink,
    /// Cursor color changed.
    CursorColor,
    /// Cursor visibility changed.
    CursorVisible,
    /// Color scheme changed (foreground, background, or palette).
    Colors,
    /// Scrollback limit changed.
    ScrollbackLimit,
    /// Auto-wrap mode changed.
    AutoWrap,
    /// Focus reporting mode changed.
    FocusReporting,
    /// Bracketed paste mode changed.
    BracketedPaste,
    /// Memory budget changed.
    MemoryBudget,
    /// Sync timeout changed.
    SyncTimeout,
}

/// Observer trait for configuration change notifications.
///
/// Implement this trait to receive callbacks when terminal configuration
/// is modified. This is useful for updating UI elements reactively.
///
/// # Example
///
/// ```ignore
/// struct AppObserver {
///     needs_redraw: bool,
/// }
///
/// impl ConfigObserver for AppObserver {
///     fn on_config_changed(&mut self, changes: &[ConfigChange]) {
///         for change in changes {
///             match change {
///                 ConfigChange::Colors | ConfigChange::CursorStyle => {
///                     self.needs_redraw = true;
///                 }
///                 _ => {}
///             }
///         }
///     }
/// }
/// ```
pub trait ConfigObserver: Send {
    /// Called when configuration has been applied to the terminal.
    ///
    /// The `changes` slice contains all configuration aspects that were
    /// modified. This allows for efficient, targeted UI updates.
    fn on_config_changed(&mut self, changes: &[ConfigChange]);
}

/// Builder for `TerminalConfig` with fluent API.
///
/// # Example
///
/// ```
/// use dterm_core::config::TerminalConfig;
/// use dterm_core::terminal::CursorStyle;
///
/// let config = TerminalConfig::builder()
///     .cursor_style(CursorStyle::SteadyBar)
///     .cursor_blink(false)
///     .scrollback_limit(50_000)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    config: TerminalConfig,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigBuilder {
    /// Create a new builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: TerminalConfig::default(),
        }
    }

    /// Set the cursor style.
    #[must_use]
    pub fn cursor_style(mut self, style: CursorStyle) -> Self {
        self.config.cursor_style = style;
        self
    }

    /// Set cursor blink mode.
    #[must_use]
    pub fn cursor_blink(mut self, blink: bool) -> Self {
        self.config.cursor_blink = blink;
        self
    }

    /// Set cursor color.
    #[must_use]
    pub fn cursor_color(mut self, color: Option<Rgb>) -> Self {
        self.config.cursor_color = color;
        self
    }

    /// Set cursor visibility.
    #[must_use]
    pub fn cursor_visible(mut self, visible: bool) -> Self {
        self.config.cursor_visible = visible;
        self
    }

    /// Set default foreground color.
    #[must_use]
    pub fn default_foreground(mut self, color: Rgb) -> Self {
        self.config.default_foreground = color;
        self
    }

    /// Set default background color.
    #[must_use]
    pub fn default_background(mut self, color: Rgb) -> Self {
        self.config.default_background = color;
        self
    }

    /// Set custom color palette.
    #[must_use]
    pub fn custom_palette(mut self, palette: ColorPalette) -> Self {
        self.config.custom_palette = Some(palette);
        self
    }

    /// Set scrollback limit in lines.
    #[must_use]
    pub fn scrollback_limit(mut self, limit: usize) -> Self {
        self.config.scrollback_limit = limit;
        self
    }

    /// Set auto-wrap mode.
    #[must_use]
    pub fn auto_wrap(mut self, enabled: bool) -> Self {
        self.config.auto_wrap = enabled;
        self
    }

    /// Set focus reporting mode.
    #[must_use]
    pub fn focus_reporting(mut self, enabled: bool) -> Self {
        self.config.focus_reporting = enabled;
        self
    }

    /// Set bracketed paste mode.
    #[must_use]
    pub fn bracketed_paste(mut self, enabled: bool) -> Self {
        self.config.bracketed_paste = enabled;
        self
    }

    /// Set memory budget for scrollback.
    #[must_use]
    pub fn memory_budget(mut self, budget: usize) -> Self {
        self.config.memory_budget = budget;
        self
    }

    /// Set synchronized output timeout.
    #[must_use]
    pub fn sync_timeout_ms(mut self, timeout: u64) -> Self {
        self.config.sync_timeout_ms = timeout;
        self
    }

    /// Build the configuration.
    #[must_use]
    pub fn build(self) -> TerminalConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TerminalConfig::default();
        assert_eq!(config.cursor_style, CursorStyle::BlinkingBlock);
        assert!(config.cursor_blink);
        assert!(config.cursor_visible);
        assert_eq!(config.scrollback_limit, 10_000);
        assert!(config.auto_wrap);
    }

    #[test]
    fn test_builder() {
        let config = TerminalConfig::builder()
            .cursor_style(CursorStyle::SteadyUnderline)
            .cursor_blink(false)
            .scrollback_limit(50_000)
            .build();

        assert_eq!(config.cursor_style, CursorStyle::SteadyUnderline);
        assert!(!config.cursor_blink);
        assert_eq!(config.scrollback_limit, 50_000);
    }

    #[test]
    fn test_diff_no_changes() {
        let config1 = TerminalConfig::default();
        let config2 = TerminalConfig::default();
        let changes = config1.diff(&config2);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_with_changes() {
        let config1 = TerminalConfig::default();
        let config2 = TerminalConfig::builder()
            .cursor_style(CursorStyle::SteadyBar)
            .cursor_blink(false)
            .build();

        let changes = config1.diff(&config2);
        assert!(changes.contains(&ConfigChange::CursorStyle));
        assert!(changes.contains(&ConfigChange::CursorBlink));
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn test_diff_colors() {
        let config1 = TerminalConfig::default();
        let config2 = TerminalConfig::builder()
            .default_foreground(Rgb::new(200, 200, 200))
            .build();

        let changes = config1.diff(&config2);
        assert!(changes.contains(&ConfigChange::Colors));
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_config_equality() {
        let config1 = TerminalConfig::default();
        let config2 = TerminalConfig::default();
        assert_eq!(config1, config2);

        let config3 = TerminalConfig::builder().cursor_blink(false).build();
        assert_ne!(config1, config3);
    }
}
