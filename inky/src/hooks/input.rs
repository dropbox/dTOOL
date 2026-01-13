//! Input handling hooks.

#![allow(dead_code)] // Public API for future use

use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use parking_lot::RwLock;
use std::sync::Arc;

/// Input handler callback type.
pub type InputHandler = Arc<dyn Fn(&KeyEvent) + Send + Sync>;

/// Global input handlers.
/// Uses parking_lot::RwLock for faster uncontended reads (no poisoning overhead).
static INPUT_HANDLERS: RwLock<Vec<InputHandler>> = RwLock::new(Vec::new());

/// Register an input handler.
///
/// # Example
///
/// ```ignore
/// use_input(|key| {
///     match key.code {
///         KeyCode::Char('q') if key.modifiers.ctrl => {
///             // Handle Ctrl+Q
///         }
///         KeyCode::Enter => {
///             // Handle Enter
///         }
///         _ => {}
///     }
/// });
/// ```
pub fn use_input<F>(handler: F)
where
    F: Fn(&KeyEvent) + Send + Sync + 'static,
{
    INPUT_HANDLERS.write().push(Arc::new(handler));
}

/// Dispatch a key event to all handlers.
pub fn dispatch_key_event(event: &KeyEvent) {
    let handlers = INPUT_HANDLERS.read();
    for handler in &*handlers {
        handler(event);
    }
}

/// Clear all input handlers.
pub fn clear_input_handlers() {
    INPUT_HANDLERS.write().clear();
}

/// Key binding helper.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// Key code to match.
    pub code: KeyCode,
    /// Required modifier keys.
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    /// Create a binding for a simple key.
    pub fn key(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// Create a binding with Ctrl modifier.
    pub fn ctrl(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::CTRL,
        }
    }

    /// Create a binding with Alt modifier.
    pub fn alt(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::ALT,
        }
    }

    /// Check if this binding matches a key event.
    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.code == event.code && self.modifiers == event.modifiers
    }

    /// Create a binding with Shift modifier.
    pub fn shift(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::SHIFT,
        }
    }

    /// Create a binding with Ctrl+Shift modifiers.
    pub fn ctrl_shift(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers {
                ctrl: true,
                shift: true,
                alt: false,
                super_key: false,
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_key_binding_key() {
        let binding = KeyBinding::key(KeyCode::Enter);
        assert_eq!(binding.code, KeyCode::Enter);
        assert_eq!(binding.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_key_binding_ctrl() {
        let binding = KeyBinding::ctrl(KeyCode::Char('c'));
        assert_eq!(binding.code, KeyCode::Char('c'));
        assert!(binding.modifiers.ctrl);
        assert!(!binding.modifiers.shift);
    }

    #[test]
    fn test_key_binding_alt() {
        let binding = KeyBinding::alt(KeyCode::Tab);
        assert_eq!(binding.code, KeyCode::Tab);
        assert!(binding.modifiers.alt);
        assert!(!binding.modifiers.ctrl);
    }

    #[test]
    fn test_key_binding_shift() {
        let binding = KeyBinding::shift(KeyCode::Tab);
        assert_eq!(binding.code, KeyCode::Tab);
        assert!(binding.modifiers.shift);
        assert!(!binding.modifiers.ctrl);
    }

    #[test]
    fn test_key_binding_ctrl_shift() {
        let binding = KeyBinding::ctrl_shift(KeyCode::Char('z'));
        assert_eq!(binding.code, KeyCode::Char('z'));
        assert!(binding.modifiers.ctrl);
        assert!(binding.modifiers.shift);
        assert!(!binding.modifiers.alt);
    }

    #[test]
    fn test_key_binding_matches() {
        let binding = KeyBinding::ctrl(KeyCode::Char('c'));

        let matching_event = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CTRL,
        };

        let non_matching_event = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::NONE,
        };

        assert!(binding.matches(&matching_event));
        assert!(!binding.matches(&non_matching_event));
    }

    #[test]
    #[serial]
    fn test_use_input_and_dispatch() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Clear any existing handlers
        clear_input_handlers();

        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        use_input(move |_event| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let event = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        };

        dispatch_key_event(&event);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        dispatch_key_event(&event);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);

        // Clean up
        clear_input_handlers();
    }

    #[test]
    #[serial]
    fn test_clear_input_handlers() {
        use std::sync::atomic::{AtomicBool, Ordering};

        clear_input_handlers();

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        use_input(move |_| {
            called_clone.store(true, Ordering::SeqCst);
        });

        clear_input_handlers();

        let event = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        };

        dispatch_key_event(&event);
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_multiple_handlers() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        clear_input_handlers();

        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        let c1 = count1.clone();
        let c2 = count2.clone();

        use_input(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
        });

        use_input(move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
        });

        let event = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        };

        dispatch_key_event(&event);

        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);

        clear_input_handlers();
    }
}
