//! Core types for the plugin system.

use std::fmt;
use std::time::Duration;

/// Unique identifier for a plugin instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginId(pub u32);

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "plugin-{}", self.0)
    }
}

/// Current state of a plugin instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Plugin is loading (manifest parsed, wasm validating).
    Loading,
    /// Plugin is ready to process events.
    Ready,
    /// Plugin is currently processing an event.
    Processing,
    /// Plugin is paused (temporarily disabled).
    Paused,
    /// Plugin encountered an error and is disabled.
    Error,
    /// Plugin has been unloaded.
    Unloaded,
}

impl fmt::Display for PluginState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loading => write!(f, "loading"),
            Self::Ready => write!(f, "ready"),
            Self::Processing => write!(f, "processing"),
            Self::Paused => write!(f, "paused"),
            Self::Error => write!(f, "error"),
            Self::Unloaded => write!(f, "unloaded"),
        }
    }
}

/// Events that plugins can receive from the host.
#[derive(Debug, Clone)]
pub enum PluginEvent {
    /// Terminal output data.
    Output {
        /// Raw bytes from terminal.
        data: Vec<u8>,
        /// Whether this is part of a command's output.
        in_command: bool,
    },
    /// Key press event.
    Key(KeyEvent),
    /// Command started executing.
    CommandStart {
        /// Command line text.
        command: String,
        /// Working directory.
        cwd: Option<String>,
    },
    /// Command finished executing.
    CommandComplete {
        /// Command line text.
        command: String,
        /// Exit code (if available).
        exit_code: Option<i32>,
        /// Duration of command execution.
        duration: Duration,
    },
    /// Periodic tick for time-based plugins.
    Tick {
        /// Milliseconds since plugin start.
        now_ms: u64,
    },
}

/// Key event from terminal input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// The key code.
    pub key: KeyCode,
    /// Modifier keys held.
    pub modifiers: KeyModifiers,
}

/// Key codes for key events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// Character key.
    Char(char),
    /// Function key (F1-F12).
    F(u8),
    /// Backspace key.
    Backspace,
    /// Enter/Return key.
    Enter,
    /// Tab key.
    Tab,
    /// Escape key.
    Escape,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Insert key.
    Insert,
    /// Delete key.
    Delete,
}

bitflags::bitflags! {
    /// Modifier keys for key events.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct KeyModifiers: u8 {
        /// Shift key.
        const SHIFT = 0b0000_0001;
        /// Control key.
        const CTRL = 0b0000_0010;
        /// Alt/Option key.
        const ALT = 0b0000_0100;
        /// Meta/Command key.
        const META = 0b0000_1000;
    }
}

/// Actions a plugin can return in response to events.
#[derive(Debug, Clone)]
pub enum PluginAction {
    /// Continue normal processing (no change).
    Continue,
    /// Consume the event (don't pass to terminal/other plugins).
    Consume,
    /// Transform the data (for output/key events).
    Transform(Vec<u8>),
    /// Emit input to the terminal (as if typed).
    EmitInput(Vec<u8>),
    /// Emit a command to execute.
    EmitCommand {
        /// Command to run.
        command: String,
        /// Arguments.
        args: Vec<String>,
    },
    /// Add annotation to terminal content.
    Annotate {
        /// Start column.
        start_col: u16,
        /// End column.
        end_col: u16,
        /// Start row.
        start_row: u32,
        /// End row.
        end_row: u32,
        /// Annotation style/data.
        style: String,
    },
}

/// Information about the terminal, provided to plugins.
#[derive(Debug, Clone)]
pub struct TerminalInfo {
    /// Terminal width in columns.
    pub cols: u16,
    /// Terminal height in rows.
    pub rows: u16,
    /// Cursor column position.
    pub cursor_col: u16,
    /// Cursor row position.
    pub cursor_row: u16,
    /// Current working directory (if known).
    pub cwd: Option<String>,
    /// Whether terminal is in alternate screen.
    pub alternate_screen: bool,
    /// Whether bracketed paste mode is enabled.
    pub bracketed_paste: bool,
}

/// Metrics tracked for each plugin.
#[derive(Debug, Clone, Default)]
pub struct PluginMetrics {
    /// Total events processed.
    pub events_processed: u64,
    /// Events that resulted in Transform action.
    pub events_transformed: u64,
    /// Events that resulted in Consume action.
    pub events_consumed: u64,
    /// Total processing time (microseconds).
    pub total_processing_us: u64,
    /// Number of timeouts.
    pub timeout_count: u32,
    /// Number of traps/errors.
    pub trap_count: u32,
    /// Bytes transformed (input).
    pub bytes_in: u64,
    /// Bytes transformed (output).
    pub bytes_out: u64,
}

/// Errors from the plugin system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// Plugin not found.
    NotFound(PluginId),
    /// Plugin is in wrong state for operation.
    InvalidState {
        /// Plugin ID.
        plugin: PluginId,
        /// Expected state.
        expected: PluginState,
        /// Actual state.
        actual: PluginState,
    },
    /// Permission denied.
    PermissionDenied {
        /// Plugin ID.
        plugin: PluginId,
        /// Requested permission.
        permission: String,
    },
    /// Plugin execution timed out.
    Timeout {
        /// Plugin ID.
        plugin: PluginId,
        /// How long it ran.
        duration: Duration,
    },
    /// Plugin trapped (wasm error).
    Trap {
        /// Plugin ID.
        plugin: PluginId,
        /// Error message.
        message: String,
    },
    /// Plugin exceeded resource limits.
    ResourceExhausted {
        /// Plugin ID.
        plugin: PluginId,
        /// Which resource.
        resource: String,
    },
    /// Storage error.
    Storage {
        /// Plugin ID.
        plugin: PluginId,
        /// Error message.
        message: String,
    },
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "plugin not found: {id}"),
            Self::InvalidState {
                plugin,
                expected,
                actual,
            } => {
                write!(f, "plugin {plugin} in wrong state: expected {expected}, got {actual}")
            }
            Self::PermissionDenied { plugin, permission } => {
                write!(f, "plugin {plugin} denied permission: {permission}")
            }
            Self::Timeout { plugin, duration } => {
                write!(f, "plugin {plugin} timed out after {duration:?}")
            }
            Self::Trap { plugin, message } => {
                write!(f, "plugin {plugin} trapped: {message}")
            }
            Self::ResourceExhausted { plugin, resource } => {
                write!(f, "plugin {plugin} exhausted resource: {resource}")
            }
            Self::Storage { plugin, message } => {
                write!(f, "plugin {plugin} storage error: {message}")
            }
        }
    }
}

impl std::error::Error for PluginError {}

/// Result type for plugin operations.
pub type PluginResult<T> = Result<T, PluginError>;

/// Trait for plugins (used for non-WASM plugins in testing).
pub trait Plugin: Send + Sync {
    /// Get the plugin's unique identifier.
    fn id(&self) -> PluginId;

    /// Get the plugin's name.
    fn name(&self) -> &str;

    /// Get the plugin's current state.
    fn state(&self) -> PluginState;

    /// Process an event and return an action.
    fn process(&mut self, event: &PluginEvent) -> PluginResult<PluginAction>;

    /// Get current metrics.
    fn metrics(&self) -> &PluginMetrics;

    /// Pause the plugin.
    fn pause(&mut self) -> PluginResult<()>;

    /// Resume the plugin.
    fn resume(&mut self) -> PluginResult<()>;

    /// Unload the plugin.
    fn unload(&mut self) -> PluginResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_id_display() {
        let id = PluginId(42);
        assert_eq!(format!("{id}"), "plugin-42");
    }

    #[test]
    fn test_plugin_state_display() {
        assert_eq!(format!("{}", PluginState::Ready), "ready");
        assert_eq!(format!("{}", PluginState::Error), "error");
    }

    #[test]
    fn test_key_modifiers() {
        let mods = KeyModifiers::CTRL | KeyModifiers::SHIFT;
        assert!(mods.contains(KeyModifiers::CTRL));
        assert!(mods.contains(KeyModifiers::SHIFT));
        assert!(!mods.contains(KeyModifiers::ALT));
    }

    #[test]
    fn test_plugin_error_display() {
        let err = PluginError::NotFound(PluginId(1));
        assert_eq!(format!("{err}"), "plugin not found: plugin-1");

        let err = PluginError::PermissionDenied {
            plugin: PluginId(2),
            permission: "terminal.write".to_string(),
        };
        assert_eq!(
            format!("{err}"),
            "plugin plugin-2 denied permission: terminal.write"
        );
    }
}
