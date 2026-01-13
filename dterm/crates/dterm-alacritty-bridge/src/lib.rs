//! dterm-alacritty-bridge
//!
//! Minimal compatibility layer to adapt dterm-core to Alacritty-style APIs.
//! This is an incremental bridge; API coverage will expand as integration work
//! proceeds.

pub mod cell;
pub mod event;
pub mod event_loop;
pub mod grid;
pub mod hints;
pub mod index;
pub mod keyboard;
pub mod mouse;
pub mod render;
pub mod search;
pub mod selection;
pub mod sync;
pub mod term;
pub mod term_mode;
pub mod tty;
pub mod url;
pub mod vi_mode;

#[cfg(test)]
mod tests;

pub use event::{
    ClipboardFormatter, ClipboardType, ColorFormatter, Event, EventListener, Notify, OnResize,
    VoidListener, VoidNotify, WindowSize, WindowSizeFormatter,
};
pub use event_loop::{
    EventLoop, EventLoopSendError, EventLoopSender, Msg, Notifier, State as EventLoopState,
};
pub use grid::{
    get_scrollback_line, get_scrollback_text, grid_cell, grid_cell_mut, grid_row, grid_row_mut,
    is_scrollback_line, line_to_row, row_cell, row_cell_mut, scrollback_line_count, AsIndexable,
    AsIndexableMut, BidirectionalIterator, Grid, GridDamageExt, GridExt, GridIterator,
    GridIteratorExt, IndexableGrid, IndexableGridMut, Indexed, LineDamageBounds, RowDamageBounds,
    Scroll, ScrollbackLine, TermDamage, TermDamageIterator,
};
pub use hints::{Hint, HintAction, HintLabels, HintState, DEFAULT_HINT_ALPHABET};
pub use index::{Boundary, Column, Dimensions, Direction, Line, Point, Side};
pub use keyboard::{encode_key, encode_key_with_event, Key, KeyEventType, Modifiers, NamedKey};
pub use mouse::{
    encode_mouse_motion, encode_mouse_press, encode_mouse_release, encode_mouse_wheel, MouseButton,
};
pub use render::{
    get_renderable_cells, RenderColor, RenderableCell, RenderableContent, RenderableCursor,
};
pub use search::{Match, RegexSearch, TermSearch};
pub use selection::{Selection, SelectionRange, SelectionType};
pub use sync::FairMutex;
pub use term::{Config, Term, DEFAULT_SEMANTIC_ESCAPE_CHARS};
pub use term_mode::TermMode;
pub use tty::{setup_env, ChildEvent, EventedPty, EventedReadWrite, Options as TtyOptions, Shell};
#[cfg(unix)]
pub use tty::{Pty, ToWinsize, PTY_CHILD_EVENT_TOKEN, PTY_READ_WRITE_TOKEN};
pub use url::{find_next_url, find_prev_url, find_urls, url_at_point, UrlMatch};
pub use vi_mode::{InlineSearchKind, InlineSearchState, ViMarks, ViModeCursor, ViMotion};

// Re-export core terminal types for Alacritty-style usage
pub use dterm_core::terminal::{
    ColorPalette, CursorStyle, MouseEncoding, MouseMode, Rgb, TerminalModes,
};

// Re-export cell types for Alacritty-style usage
pub use cell::{Cell, Flags, Hyperlink, LineLength};
