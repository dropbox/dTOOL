//! Mouse event handling hooks.

#![allow(dead_code)] // Public API for future use

use crate::terminal::MouseEvent;
use parking_lot::RwLock;
use std::sync::Arc;

/// Mouse handler callback type.
pub type MouseHandler = Arc<dyn Fn(&MouseEvent) + Send + Sync>;

/// Global mouse handlers.
/// Uses parking_lot::RwLock for faster uncontended reads (no poisoning overhead).
static MOUSE_HANDLERS: RwLock<Vec<MouseHandler>> = RwLock::new(Vec::new());

/// Register a mouse event handler.
///
/// The handler will be called for all mouse events including clicks, scrolls,
/// and movement. Multiple handlers can be registered and will all be called.
///
/// # Example
///
/// ```ignore
/// use_mouse(|mouse| {
///     match mouse.kind {
///         MouseEventKind::Down => {
///             println!("Clicked at ({}, {})", mouse.x, mouse.y);
///         }
///         MouseEventKind::ScrollUp => {
///             println!("Scroll up");
///         }
///         MouseEventKind::ScrollDown => {
///             println!("Scroll down");
///         }
///         _ => {}
///     }
/// });
/// ```
pub fn use_mouse<F>(handler: F)
where
    F: Fn(&MouseEvent) + Send + Sync + 'static,
{
    MOUSE_HANDLERS.write().push(Arc::new(handler));
}

/// Dispatch a mouse event to all handlers.
pub fn dispatch_mouse_event(event: &MouseEvent) {
    let handlers = MOUSE_HANDLERS.read();
    for handler in &*handlers {
        handler(event);
    }
}

/// Clear all mouse handlers.
pub fn clear_mouse_handlers() {
    MOUSE_HANDLERS.write().clear();
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::terminal::{KeyModifiers, MouseButton, MouseEventKind};
    use serial_test::serial;

    fn make_mouse_event(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            button: None,
            kind,
            x,
            y,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn make_click_event(button: MouseButton, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            button: Some(button),
            kind: MouseEventKind::Down,
            x,
            y,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    #[serial]
    fn test_use_mouse_and_dispatch() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Clear any existing handlers
        clear_mouse_handlers();

        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        use_mouse(move |_event| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let event = make_mouse_event(MouseEventKind::Moved, 10, 20);

        dispatch_mouse_event(&event);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        dispatch_mouse_event(&event);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);

        // Clean up
        clear_mouse_handlers();
    }

    #[test]
    #[serial]
    fn test_clear_mouse_handlers() {
        use std::sync::atomic::{AtomicBool, Ordering};

        clear_mouse_handlers();

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        use_mouse(move |_| {
            called_clone.store(true, Ordering::SeqCst);
        });

        clear_mouse_handlers();

        let event = make_mouse_event(MouseEventKind::Down, 5, 5);

        dispatch_mouse_event(&event);
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_multiple_mouse_handlers() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        clear_mouse_handlers();

        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        let c1 = count1.clone();
        let c2 = count2.clone();

        use_mouse(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
        });

        use_mouse(move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
        });

        let event = make_click_event(MouseButton::Left, 0, 0);

        dispatch_mouse_event(&event);

        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);

        clear_mouse_handlers();
    }

    #[test]
    #[serial]
    fn test_mouse_event_coordinates() {
        use std::sync::atomic::{AtomicU32, Ordering};

        clear_mouse_handlers();

        let captured_x = Arc::new(AtomicU32::new(0));
        let captured_y = Arc::new(AtomicU32::new(0));

        let x_clone = captured_x.clone();
        let y_clone = captured_y.clone();

        use_mouse(move |event| {
            x_clone.store(event.x as u32, Ordering::SeqCst);
            y_clone.store(event.y as u32, Ordering::SeqCst);
        });

        let event = make_mouse_event(MouseEventKind::Moved, 42, 73);
        dispatch_mouse_event(&event);

        assert_eq!(captured_x.load(Ordering::SeqCst), 42);
        assert_eq!(captured_y.load(Ordering::SeqCst), 73);

        clear_mouse_handlers();
    }

    #[test]
    #[serial]
    fn test_scroll_events() {
        use std::sync::atomic::{AtomicI32, Ordering};

        clear_mouse_handlers();

        let scroll_delta = Arc::new(AtomicI32::new(0));
        let delta_clone = scroll_delta.clone();

        use_mouse(move |event| match event.kind {
            MouseEventKind::ScrollUp => {
                delta_clone.fetch_add(1, Ordering::SeqCst);
            }
            MouseEventKind::ScrollDown => {
                delta_clone.fetch_sub(1, Ordering::SeqCst);
            }
            _ => {}
        });

        // Scroll up twice
        dispatch_mouse_event(&make_mouse_event(MouseEventKind::ScrollUp, 0, 0));
        dispatch_mouse_event(&make_mouse_event(MouseEventKind::ScrollUp, 0, 0));

        // Scroll down once
        dispatch_mouse_event(&make_mouse_event(MouseEventKind::ScrollDown, 0, 0));

        assert_eq!(scroll_delta.load(Ordering::SeqCst), 1);

        clear_mouse_handlers();
    }

    #[test]
    #[serial]
    fn test_mouse_button_types() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        clear_mouse_handlers();

        let left_clicks = Arc::new(AtomicUsize::new(0));
        let right_clicks = Arc::new(AtomicUsize::new(0));
        let middle_clicks = Arc::new(AtomicUsize::new(0));

        let left = left_clicks.clone();
        let right = right_clicks.clone();
        let middle = middle_clicks.clone();

        use_mouse(move |event| {
            if event.kind == MouseEventKind::Down {
                match event.button {
                    Some(MouseButton::Left) => {
                        left.fetch_add(1, Ordering::SeqCst);
                    }
                    Some(MouseButton::Right) => {
                        right.fetch_add(1, Ordering::SeqCst);
                    }
                    Some(MouseButton::Middle) => {
                        middle.fetch_add(1, Ordering::SeqCst);
                    }
                    None => {}
                }
            }
        });

        dispatch_mouse_event(&make_click_event(MouseButton::Left, 0, 0));
        dispatch_mouse_event(&make_click_event(MouseButton::Left, 0, 0));
        dispatch_mouse_event(&make_click_event(MouseButton::Right, 0, 0));
        dispatch_mouse_event(&make_click_event(MouseButton::Middle, 0, 0));

        assert_eq!(left_clicks.load(Ordering::SeqCst), 2);
        assert_eq!(right_clicks.load(Ordering::SeqCst), 1);
        assert_eq!(middle_clicks.load(Ordering::SeqCst), 1);

        clear_mouse_handlers();
    }
}
