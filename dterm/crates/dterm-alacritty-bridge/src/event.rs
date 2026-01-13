//! Alacritty-style event definitions for the bridge.

use std::borrow::Cow;
use std::sync::Arc;

use dterm_core::terminal::Rgb;

/// Formatter callback for color requests.
///
/// Takes the RGB color and returns the escape sequence response.
pub type ColorFormatter = Arc<dyn Fn(Rgb) -> String + Sync + Send + 'static>;

/// Formatter callback for clipboard paste.
///
/// Takes the clipboard text and returns the formatted string to send to PTY.
pub type ClipboardFormatter = Arc<dyn Fn(&str) -> String + Sync + Send + 'static>;

/// Formatter callback for window size requests.
///
/// Takes the window size and returns the escape sequence response.
pub type WindowSizeFormatter = Arc<dyn Fn(WindowSize) -> String + Sync + Send + 'static>;

/// Terminal events surfaced to the host application.
#[derive(Clone)]
pub enum Event {
    /// Terminal bell (BEL).
    Bell,
    /// Window title change.
    Title(String),
    /// Reset title to default.
    ResetTitle,
    /// Mouse cursor should be refreshed.
    MouseCursorDirty,
    /// Wake up the event loop.
    Wakeup,
    /// Request to store content in clipboard.
    ClipboardStore(ClipboardType, String),
    /// Request to load content from clipboard.
    ///
    /// The callback formats the clipboard text before sending to PTY.
    /// This matches Alacritty's API where the formatter handles bracketed
    /// paste mode and other transformations.
    ClipboardLoad(ClipboardType, ClipboardFormatter),
    /// Write data to PTY (for responses like DA, CPR, etc.).
    PtyWrite(String),
    /// Color palette request - terminal needs to respond with color value.
    ///
    /// The callback formats the RGB color into the appropriate escape sequence.
    ColorRequest(usize, ColorFormatter),
    /// Terminal requests exit.
    Exit,
    /// Cursor blink state changed.
    CursorBlinkingChange,
    /// Window size request.
    ///
    /// The callback formats the window size into the appropriate escape sequence.
    TextAreaSizeRequest(WindowSizeFormatter),
    /// Child process exited.
    ChildExit(i32),
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Bell => write!(f, "Bell"),
            Event::Title(s) => f.debug_tuple("Title").field(s).finish(),
            Event::ResetTitle => write!(f, "ResetTitle"),
            Event::MouseCursorDirty => write!(f, "MouseCursorDirty"),
            Event::Wakeup => write!(f, "Wakeup"),
            Event::ClipboardStore(ty, s) => {
                f.debug_tuple("ClipboardStore").field(ty).field(s).finish()
            }
            Event::ClipboardLoad(ty, _) => f
                .debug_tuple("ClipboardLoad")
                .field(ty)
                .field(&"<formatter>")
                .finish(),
            Event::PtyWrite(s) => f.debug_tuple("PtyWrite").field(s).finish(),
            Event::ColorRequest(idx, _) => f
                .debug_tuple("ColorRequest")
                .field(idx)
                .field(&"<formatter>")
                .finish(),
            Event::Exit => write!(f, "Exit"),
            Event::CursorBlinkingChange => write!(f, "CursorBlinkingChange"),
            Event::TextAreaSizeRequest(_) => f
                .debug_tuple("TextAreaSizeRequest")
                .field(&"<formatter>")
                .finish(),
            Event::ChildExit(code) => f.debug_tuple("ChildExit").field(code).finish(),
        }
    }
}

/// Clipboard selection type.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ClipboardType {
    /// System clipboard (Ctrl+C/V).
    Clipboard,
    /// Primary selection (X11 middle-click paste).
    Selection,
}

/// Event listener for terminal events.
pub trait EventListener: Send + Sync {
    /// Send an event to the host application.
    fn send_event(&self, event: Event);
}

/// Notification interface for PTY communication.
///
/// This trait allows the terminal to send data back to the PTY,
/// such as responses to device attribute queries or cursor position reports.
pub trait Notify: Send {
    /// Write bytes to the PTY.
    fn notify<B: Into<Cow<'static, [u8]>>>(&self, data: B);
}

/// Window resize callback trait.
///
/// Implementors are notified when the terminal window size changes.
pub trait OnResize: Send {
    /// Called when the terminal size changes.
    fn on_resize(&mut self, size: WindowSize);
}

/// Terminal window dimensions.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct WindowSize {
    /// Width in cells.
    pub num_cols: u16,
    /// Height in cells.
    pub num_lines: u16,
    /// Width in pixels.
    pub cell_width: u16,
    /// Height in pixels.
    pub cell_height: u16,
}

impl WindowSize {
    /// Create a new window size.
    pub fn new(num_cols: u16, num_lines: u16, cell_width: u16, cell_height: u16) -> Self {
        Self {
            num_cols,
            num_lines,
            cell_width,
            cell_height,
        }
    }
}

/// No-op event listener.
#[derive(Debug, Default, Clone, Copy)]
pub struct VoidListener;

impl EventListener for VoidListener {
    fn send_event(&self, _event: Event) {}
}

/// No-op notifier.
#[derive(Debug, Default, Clone, Copy)]
pub struct VoidNotify;

impl Notify for VoidNotify {
    fn notify<B: Into<Cow<'static, [u8]>>>(&self, _data: B) {}
}
