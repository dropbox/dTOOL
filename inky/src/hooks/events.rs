//! Event types for inky applications.
//!
//! This module defines events that can be propagated through the component tree.
//! Events are used for keyboard input, focus changes, and custom application events.
//!
//! # Example
//!
//! ```ignore
//! use inky::hooks::{Event, EventResult, FocusEvent};
//!
//! fn handle_event(event: &Event) -> EventResult {
//!     match event {
//!         Event::Focus(FocusEvent::FocusIn) => {
//!             // Handle focus gained
//!             EventResult::Handled
//!         }
//!         Event::Focus(FocusEvent::FocusOut) => {
//!             // Handle focus lost
//!             EventResult::Handled
//!         }
//!         _ => EventResult::Continue,
//!     }
//! }
//! ```

use crate::terminal::KeyEvent;
use std::any::Any;
use std::fmt;

/// Event that can be propagated through the component tree.
#[derive(Clone)]
pub enum Event {
    /// Keyboard event.
    Key(KeyEvent),
    /// Focus event.
    Focus(FocusEvent),
    /// Custom event with a name and arbitrary payload.
    Custom(CustomEvent),
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Key(key) => f.debug_tuple("Key").field(key).finish(),
            Event::Focus(focus) => f.debug_tuple("Focus").field(focus).finish(),
            Event::Custom(custom) => f.debug_tuple("Custom").field(&custom.name).finish(),
        }
    }
}

impl Event {
    /// Create a custom event with the given name and payload.
    pub fn custom<T: Any + Send + Sync + Clone + 'static>(
        name: impl Into<String>,
        payload: T,
    ) -> Self {
        Event::Custom(CustomEvent::new(name, payload))
    }

    /// Check if this is a key event.
    pub fn is_key(&self) -> bool {
        matches!(self, Event::Key(_))
    }

    /// Check if this is a focus event.
    pub fn is_focus(&self) -> bool {
        matches!(self, Event::Focus(_))
    }

    /// Check if this is a custom event.
    pub fn is_custom(&self) -> bool {
        matches!(self, Event::Custom(_))
    }

    /// Get the key event if this is one.
    pub fn as_key(&self) -> Option<&KeyEvent> {
        match self {
            Event::Key(key) => Some(key),
            _ => None,
        }
    }

    /// Get the focus event if this is one.
    pub fn as_focus(&self) -> Option<&FocusEvent> {
        match self {
            Event::Focus(focus) => Some(focus),
            _ => None,
        }
    }

    /// Get the custom event if this is one.
    pub fn as_custom(&self) -> Option<&CustomEvent> {
        match self {
            Event::Custom(custom) => Some(custom),
            _ => None,
        }
    }
}

impl From<KeyEvent> for Event {
    fn from(key: KeyEvent) -> Self {
        Event::Key(key)
    }
}

impl From<FocusEvent> for Event {
    fn from(focus: FocusEvent) -> Self {
        Event::Focus(focus)
    }
}

/// Focus event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    /// Component gained focus.
    FocusIn,
    /// Component lost focus.
    FocusOut,
}

/// Custom event with a name and typed payload.
pub struct CustomEvent {
    /// Event name for identification.
    pub name: String,
    /// Payload stored as a type-erased box.
    payload: Box<dyn CloneableAny + Send + Sync>,
}

impl CustomEvent {
    /// Create a new custom event.
    pub fn new<T: Any + Send + Sync + Clone + 'static>(
        name: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            name: name.into(),
            payload: Box::new(payload),
        }
    }

    /// Get the payload if it matches the expected type.
    pub fn payload<T: Any + Clone>(&self) -> Option<T> {
        self.payload.as_any().downcast_ref::<T>().cloned()
    }

    /// Check if this event has the given name.
    pub fn is(&self, name: &str) -> bool {
        self.name == name
    }
}

impl Clone for CustomEvent {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            payload: self.payload.clone_box(),
        }
    }
}

impl fmt::Debug for CustomEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomEvent")
            .field("name", &self.name)
            .finish()
    }
}

/// Event handler result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// Event was handled, stop propagation.
    Handled,
    /// Event was not handled, continue propagation.
    Continue,
}

impl EventResult {
    /// Check if the event was handled.
    pub fn is_handled(&self) -> bool {
        matches!(self, EventResult::Handled)
    }

    /// Check if propagation should continue.
    pub fn should_continue(&self) -> bool {
        matches!(self, EventResult::Continue)
    }
}

/// Trait for cloneable Any types.
trait CloneableAny: Any {
    fn clone_box(&self) -> Box<dyn CloneableAny + Send + Sync>;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Clone + Send + Sync + 'static> CloneableAny for T {
    fn clone_box(&self) -> Box<dyn CloneableAny + Send + Sync> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::terminal::{KeyCode, KeyModifiers};

    #[test]
    fn test_event_key() {
        let key = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        };
        let event = Event::Key(key.clone());

        assert!(event.is_key());
        assert!(!event.is_focus());
        assert!(!event.is_custom());

        let key_ref = event.as_key().unwrap();
        assert_eq!(key_ref.code, KeyCode::Enter);
    }

    #[test]
    fn test_event_focus() {
        let event = Event::Focus(FocusEvent::FocusIn);

        assert!(!event.is_key());
        assert!(event.is_focus());
        assert!(!event.is_custom());

        assert_eq!(event.as_focus(), Some(&FocusEvent::FocusIn));
    }

    #[test]
    fn test_event_custom() {
        let event = Event::custom("user_action", 42i32);

        assert!(!event.is_key());
        assert!(!event.is_focus());
        assert!(event.is_custom());

        let custom = event.as_custom().unwrap();
        assert!(custom.is("user_action"));
        assert_eq!(custom.payload::<i32>(), Some(42));
    }

    #[test]
    fn test_custom_event_wrong_type() {
        let custom = CustomEvent::new("test", "hello");
        assert_eq!(custom.payload::<i32>(), None);
        assert_eq!(custom.payload::<&str>(), Some("hello"));
    }

    #[test]
    fn test_event_from_key() {
        let key = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
        };
        let event: Event = key.into();
        assert!(event.is_key());
    }

    #[test]
    fn test_event_from_focus() {
        let focus = FocusEvent::FocusOut;
        let event: Event = focus.into();
        assert!(event.is_focus());
    }

    #[test]
    fn test_event_result() {
        let handled = EventResult::Handled;
        let cont = EventResult::Continue;

        assert!(handled.is_handled());
        assert!(!handled.should_continue());

        assert!(!cont.is_handled());
        assert!(cont.should_continue());
    }

    #[test]
    fn test_event_debug() {
        let key_event = Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        });
        let debug_str = format!("{:?}", key_event);
        assert!(debug_str.contains("Key"));

        let focus_event = Event::Focus(FocusEvent::FocusIn);
        let debug_str = format!("{:?}", focus_event);
        assert!(debug_str.contains("Focus"));
        assert!(debug_str.contains("FocusIn"));

        let custom_event = Event::custom("test", 42);
        let debug_str = format!("{:?}", custom_event);
        assert!(debug_str.contains("Custom"));
    }

    #[test]
    fn test_custom_event_clone() {
        let original = CustomEvent::new("test", vec![1, 2, 3]);
        let cloned = original.clone();

        assert_eq!(cloned.name, "test");
        assert_eq!(cloned.payload::<Vec<i32>>(), Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_event_clone() {
        let event = Event::custom("counter", 100u64);
        let cloned = event.clone();

        let custom = cloned.as_custom().unwrap();
        assert_eq!(custom.payload::<u64>(), Some(100));
    }

    #[test]
    fn test_focus_event_equality() {
        assert_eq!(FocusEvent::FocusIn, FocusEvent::FocusIn);
        assert_eq!(FocusEvent::FocusOut, FocusEvent::FocusOut);
        assert_ne!(FocusEvent::FocusIn, FocusEvent::FocusOut);
    }
}
