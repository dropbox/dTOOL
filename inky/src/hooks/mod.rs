//! Hooks for reactive state and input handling.
//!
//! This module provides React-like hooks for managing state and handling input
//! in inky applications. Hooks enable functional, composable state management
//! without complex inheritance hierarchies.
//!
//! # Core Concepts
//!
//! ## Signals
//!
//! [`Signal<T>`] is the foundation of inky's reactivity system. Signals hold
//! values that can be read and written, automatically triggering re-renders
//! when modified.
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! // Create a signal with initial value
//! let count = use_signal(0);
//!
//! // Read current value
//! let current = count.get();
//!
//! // Set a new value (triggers re-render)
//! count.set(42);
//!
//! // Update based on current value
//! count.update(|c| *c += 1);
//!
//! // Read-then-use pattern
//! count.with(|c| println!("Count: {}", c));
//! ```
//!
//! ## Focus Management
//!
//! The focus system enables keyboard navigation between interactive components.
//! Use [`use_focus`] to make a component focusable.
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! let focus = use_focus();
//!
//! // Check if this component has focus
//! if focus.is_focused() {
//!     // Render focused state
//! }
//!
//! // Programmatically control focus
//! focus.focus();  // Take focus
//! focus.blur();   // Release focus
//!
//! // Navigate between focusable components
//! focus_next();   // Move to next component (Tab)
//! focus_prev();   // Move to previous component (Shift+Tab)
//! ```
//!
//! ### String-based Focus IDs
//!
//! For easier programmatic focus control, use [`use_focus_with_id`]:
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! // Create a focusable with a string ID
//! let focus = use_focus_with_id("chat-input");
//!
//! // Later, focus by ID from anywhere
//! set_focus("chat-input");
//!
//! // Query what's currently focused
//! if let Some(id) = focused_id() {
//!     println!("Currently focused: {}", id);
//! }
//!
//! // Add callbacks for focus/blur events
//! let focus = use_focus_with_id("my-input")
//!     .on_focus(|| println!("Focused!"))
//!     .on_blur(|| println!("Blurred!"));
//! ```
//!
//! ## Input Handling
//!
//! Register keyboard handlers with [`use_input`]. Handlers are called in
//! registration order until one returns [`EventResult::Handled`].
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! use_input(|event| {
//!     match event.code {
//!         KeyCode::Enter => {
//!             // Handle Enter key
//!             EventResult::Consumed
//!         }
//!         KeyCode::Char(c) => {
//!             // Handle character input
//!             EventResult::Consumed
//!         }
//!         _ => EventResult::Ignored
//!     }
//! });
//! ```
//!
//! ## Intervals
//!
//! Use [`use_interval`] for periodic updates like animations or polling.
//!
//! ```ignore
//! use inky::prelude::*;
//! use std::time::Duration;
//!
//! // Update every 100ms
//! let handle = use_interval(Duration::from_millis(100), || {
//!     // Called periodically
//!     frame.update(|f| *f += 1);
//! });
//!
//! // Stop the interval when needed
//! handle.cancel();
//! ```
//!
//! ## Mouse Handling
//!
//! Register mouse handlers with [`use_mouse`]. Handlers receive all mouse
//! events including clicks, scrolls, and movement.
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! use_mouse(|event| {
//!     match event.kind {
//!         MouseEventKind::Down => {
//!             println!("Clicked at ({}, {})", event.x, event.y);
//!         }
//!         MouseEventKind::ScrollUp => {
//!             // Handle scroll up
//!         }
//!         MouseEventKind::ScrollDown => {
//!             // Handle scroll down
//!         }
//!         _ => {}
//!     }
//! });
//! ```
//!
//! # Available Hooks
//!
//! | Hook | Purpose |
//! |------|---------|
//! | [`use_signal`] | Create reactive state |
//! | [`use_focus`] | Make component focusable |
//! | [`use_focus_with_id`] | Make component focusable with string ID |
//! | [`use_input`] | Register keyboard handlers |
//! | [`use_mouse`] | Register mouse handlers |
//! | [`use_interval`] | Periodic callbacks |
//! | [`use_app`] | Access app control handle |
//!
//! # Focus Functions
//!
//! | Function | Purpose |
//! |----------|---------|
//! | [`set_focus`] | Focus by string ID |
//! | [`focused_id`] | Get currently focused ID |
//! | [`focus_next`] | Move to next focusable |
//! | [`focus_prev`] | Move to previous focusable |
//! | [`blur_all`] | Clear focus from all |
//!
//! # Event Types
//!
//! - [`Event`] - Unified event type (keyboard, focus, custom)
//! - [`FocusEvent`] - Focus gained/lost events
//! - [`KeyEvent`] - Keyboard events with code and modifiers
//! - [`CustomEvent`] - User-defined events
//! - [`EventResult`] - Handler return type (Consumed/Ignored)
//!
//! # Helper Types
//!
//! - [`KeyBinding`] - Match keyboard shortcuts
//! - [`FocusHandle`] - Control component focus
//! - [`FocusContext`] - Global focus state
//! - [`IntervalHandle`] - Control periodic callbacks
//! - [`MouseHandler`] - Mouse event callback type
//!
//! [`Signal<T>`]: crate::hooks::Signal
//! [`Signal`]: crate::hooks::Signal
//! [`use_signal`]: crate::hooks::use_signal
//! [`use_focus`]: crate::hooks::use_focus
//! [`use_focus_with_id`]: crate::hooks::use_focus_with_id
//! [`set_focus`]: crate::hooks::set_focus
//! [`focused_id`]: crate::hooks::focused_id
//! [`focus_next`]: crate::hooks::focus_next
//! [`focus_prev`]: crate::hooks::focus_prev
//! [`blur_all`]: crate::hooks::blur_all
//! [`use_input`]: crate::hooks::use_input
//! [`use_mouse`]: crate::hooks::use_mouse
//! [`use_interval`]: crate::hooks::use_interval
//! [`use_app`]: crate::hooks::use_app
//! [`Event`]: crate::hooks::Event
//! [`FocusEvent`]: crate::hooks::FocusEvent
//! [`KeyEvent`]: crate::hooks::KeyEvent
//! [`CustomEvent`]: crate::hooks::CustomEvent
//! [`EventResult`]: crate::hooks::EventResult
//! [`EventResult::Handled`]: crate::hooks::EventResult::Handled
//! [`KeyBinding`]: crate::hooks::KeyBinding
//! [`FocusHandle`]: crate::hooks::FocusHandle
//! [`FocusContext`]: crate::hooks::FocusContext
//! [`IntervalHandle`]: crate::hooks::IntervalHandle
//! [`MouseHandler`]: crate::hooks::MouseHandler

mod click;
pub mod drag;
mod events;
mod focus;
mod input;
mod interval;
mod mouse;
mod signal;

pub use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
pub use click::{
    clear_clickables, clear_hover_state, dispatch_click_event, has_clickables, register_clickable,
    update_clickable_layout,
};
pub use drag::{
    clear_drag_drop, dispatch_drag_event, get_current_drag, is_dragging, register_draggable,
    register_drop_zone, update_drop_zone_layout, AcceptDropHandler, DragEndHandler, DragEvent,
    DragHandler, DragStartHandler, DragState, DropEvent, DropHandler,
};
pub use events::{CustomEvent, Event, EventResult, FocusEvent};
pub use focus::{
    blur_all, focus_group, focus_next, focus_next_in_group, focus_next_trapped, focus_prev,
    focus_prev_in_group, focus_prev_trapped, focused_group, focused_id, get_focus_context,
    has_focus_trap, pop_focus_trap, push_focus_trap, set_focus, use_focus, use_focus_in_group,
    use_focus_with_id, FocusCallback, FocusContext, FocusHandle, FocusTrap, FocusTrapId,
};
pub use input::{clear_input_handlers, dispatch_key_event, use_input, InputHandler, KeyBinding};
pub use interval::{use_interval, IntervalHandle};
pub use mouse::{clear_mouse_handlers, dispatch_mouse_event, use_mouse, MouseHandler};
pub use signal::{request_render, take_render_request, use_signal, Signal};

/// Hook for accessing app control.
pub fn use_app() -> crate::app::AppHandle {
    // This would be implemented with thread-local storage
    // For now, return a dummy handle
    crate::app::AppHandle {
        should_quit: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    }
}
