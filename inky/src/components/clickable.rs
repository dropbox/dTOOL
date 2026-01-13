//! Clickable wrapper component for mouse-interactive elements.
//!
//! This module provides the `Clickable` component, which wraps any node
//! and provides mouse event callbacks.
//!
//! # Focus on Click
//!
//! Clickable components can be configured to automatically focus when clicked,
//! integrating with the focus system for keyboard navigation:
//!
//! ```rust
//! use inky::prelude::*;
//! use inky::components::Clickable;
//! use inky::hooks::{use_focus, FocusHandle};
//!
//! // Create a focusable clickable button
//! let focus = use_focus();
//! let button = Clickable::new(TextNode::new("Click to focus"))
//!     .focus_on_click(focus.clone())
//!     .on_click(|_| println!("Clicked!"));
//!
//! // After clicking, this component will have keyboard focus
//! ```

use crate::hooks::FocusHandle;
use crate::node::{BoxNode, Node, NodeId};
use crate::style::Color;
use crate::terminal::{MouseButton, MouseEvent, MouseEventKind};
use std::sync::Arc;

/// Callback type for click events.
pub type ClickHandler = Arc<dyn Fn(&ClickEvent) + Send + Sync>;

/// Event data passed to click handlers.
#[derive(Debug, Clone)]
pub struct ClickEvent {
    /// The mouse button that was clicked.
    pub button: MouseButton,
    /// X coordinate of the click within the clickable area (0-based).
    pub local_x: u16,
    /// Y coordinate of the click within the clickable area (0-based).
    pub local_y: u16,
    /// Absolute X coordinate on screen.
    pub screen_x: u16,
    /// Absolute Y coordinate on screen.
    pub screen_y: u16,
    /// Whether modifier keys were held.
    pub modifiers: ClickModifiers,
}

/// Modifier keys held during a click.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClickModifiers {
    /// Shift key was held.
    pub shift: bool,
    /// Control key was held.
    pub ctrl: bool,
    /// Alt key was held.
    pub alt: bool,
}

/// A wrapper that makes any node respond to mouse clicks.
///
/// `Clickable` wraps a child node and provides callbacks for various
/// mouse interactions. The child node is rendered normally, but mouse
/// events within its bounds trigger the registered handlers.
///
/// # Example
///
/// ```rust
/// use inky::prelude::*;
/// use inky::components::Clickable;
/// use std::sync::Arc;
///
/// let button = Clickable::new(TextNode::new("Click me!"))
///     .on_click(|event| {
///         println!("Clicked at ({}, {})", event.local_x, event.local_y);
///     });
/// ```
///
/// # Hover State
///
/// Clickable components can track hover state for visual feedback:
///
/// ```rust
/// use inky::prelude::*;
/// use inky::components::Clickable;
///
/// let button = Clickable::new(TextNode::new("Hover me!"))
///     .hover_background(Color::Blue)
///     .on_hover(|| println!("Mouse entered!"))
///     .on_unhover(|| println!("Mouse left!"));
/// ```
#[derive(Clone)]
pub struct Clickable {
    /// The wrapped child node.
    child: Node,
    /// Click handler (mouse down then up within bounds).
    on_click: Option<ClickHandler>,
    /// Mouse down handler.
    on_mouse_down: Option<ClickHandler>,
    /// Mouse up handler.
    on_mouse_up: Option<ClickHandler>,
    /// Hover enter handler.
    on_hover: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Hover leave handler.
    on_unhover: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Whether this clickable is currently hovered.
    is_hovered: bool,
    /// Background color when hovered.
    hover_bg: Option<Color>,
    /// Whether the clickable is disabled.
    disabled: bool,
    /// Optional node ID (for hit testing registration).
    id: Option<NodeId>,
    /// Focus handle for automatic focus-on-click behavior.
    focus_handle: Option<FocusHandle>,
}

impl Clickable {
    /// Create a new clickable wrapping the given child.
    pub fn new(child: impl Into<Node>) -> Self {
        Self {
            child: child.into(),
            on_click: None,
            on_mouse_down: None,
            on_mouse_up: None,
            on_hover: None,
            on_unhover: None,
            is_hovered: false,
            hover_bg: None,
            disabled: false,
            id: None,
            focus_handle: None,
        }
    }

    /// Set a click handler (triggered on mouse up after mouse down within bounds).
    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ClickEvent) + Send + Sync + 'static,
    {
        self.on_click = Some(Arc::new(handler));
        self
    }

    /// Set a mouse down handler.
    pub fn on_mouse_down<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ClickEvent) + Send + Sync + 'static,
    {
        self.on_mouse_down = Some(Arc::new(handler));
        self
    }

    /// Set a mouse up handler.
    pub fn on_mouse_up<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ClickEvent) + Send + Sync + 'static,
    {
        self.on_mouse_up = Some(Arc::new(handler));
        self
    }

    /// Set a hover enter handler.
    pub fn on_hover<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_hover = Some(Arc::new(handler));
        self
    }

    /// Set a hover leave handler.
    pub fn on_unhover<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_unhover = Some(Arc::new(handler));
        self
    }

    /// Set background color to apply when hovered.
    pub fn hover_background(mut self, bg: impl Into<Option<Color>>) -> Self {
        self.hover_bg = bg.into();
        self
    }

    /// Disable the clickable (won't respond to events).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set a custom node ID for this clickable.
    pub fn id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }

    /// Enable automatic focus when this clickable is clicked.
    ///
    /// When enabled, clicking on this component will automatically focus it,
    /// making it the target of keyboard events. This is useful for creating
    /// interactive components like buttons, inputs, or list items that should
    /// receive keyboard focus after being clicked.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::prelude::*;
    /// use inky::components::Clickable;
    /// use inky::hooks::use_focus;
    ///
    /// let focus = use_focus();
    /// let button = Clickable::new(TextNode::new("Click me"))
    ///     .focus_on_click(focus.clone());
    /// ```
    pub fn focus_on_click(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    /// Check if this clickable is currently hovered.
    pub fn is_hovered(&self) -> bool {
        self.is_hovered
    }

    /// Check if this clickable is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Get the click handler if set.
    pub fn click_handler(&self) -> Option<&ClickHandler> {
        self.on_click.as_ref()
    }

    /// Get the mouse down handler if set.
    pub fn mouse_down_handler(&self) -> Option<&ClickHandler> {
        self.on_mouse_down.as_ref()
    }

    /// Get the mouse up handler if set.
    pub fn mouse_up_handler(&self) -> Option<&ClickHandler> {
        self.on_mouse_up.as_ref()
    }

    /// Get the hover handler if set.
    pub fn hover_handler(&self) -> Option<&Arc<dyn Fn() + Send + Sync>> {
        self.on_hover.as_ref()
    }

    /// Get the unhover handler if set.
    pub fn unhover_handler(&self) -> Option<&Arc<dyn Fn() + Send + Sync>> {
        self.on_unhover.as_ref()
    }

    /// Get the focus handle if set.
    pub fn focus_handle(&self) -> Option<&FocusHandle> {
        self.focus_handle.as_ref()
    }

    /// Handle a mouse event at the given local coordinates.
    ///
    /// Returns `true` if the event was handled.
    pub fn handle_mouse(&mut self, event: &MouseEvent, local_x: u16, local_y: u16) -> bool {
        if self.disabled {
            return false;
        }

        let modifiers = ClickModifiers {
            shift: event.modifiers.shift,
            ctrl: event.modifiers.ctrl,
            alt: event.modifiers.alt,
        };

        match event.kind {
            MouseEventKind::Down => {
                if let (Some(handler), Some(button)) = (&self.on_mouse_down, event.button) {
                    let click_event = ClickEvent {
                        button,
                        local_x,
                        local_y,
                        screen_x: event.x,
                        screen_y: event.y,
                        modifiers,
                    };
                    handler(&click_event);
                    return true;
                }
            }
            MouseEventKind::Up => {
                if let Some(button) = event.button {
                    let click_event = ClickEvent {
                        button,
                        local_x,
                        local_y,
                        screen_x: event.x,
                        screen_y: event.y,
                        modifiers,
                    };

                    if let Some(handler) = &self.on_mouse_up {
                        handler(&click_event);
                    }

                    // Click is triggered on mouse up
                    if let Some(handler) = &self.on_click {
                        handler(&click_event);
                        return true;
                    }
                }
            }
            MouseEventKind::Moved => {
                if !self.is_hovered {
                    self.is_hovered = true;
                    if let Some(handler) = &self.on_hover {
                        handler();
                    }
                }
            }
            _ => {}
        }

        false
    }

    /// Called when the mouse leaves this component's bounds.
    pub fn handle_mouse_leave(&mut self) {
        if self.is_hovered {
            self.is_hovered = false;
            if let Some(handler) = &self.on_unhover {
                handler();
            }
        }
    }
}

impl std::fmt::Debug for Clickable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clickable")
            .field("child", &self.child)
            .field("has_on_click", &self.on_click.is_some())
            .field("has_on_mouse_down", &self.on_mouse_down.is_some())
            .field("has_on_mouse_up", &self.on_mouse_up.is_some())
            .field("has_on_hover", &self.on_hover.is_some())
            .field("has_on_unhover", &self.on_unhover.is_some())
            .field("is_hovered", &self.is_hovered)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl From<Clickable> for Node {
    fn from(clickable: Clickable) -> Self {
        // For now, we wrap the child in a box container
        // The actual mouse handling is done through the hit testing system
        let mut container = BoxNode::new();

        if let Some(id) = clickable.id {
            container = container.id(id);
        }

        // Apply hover background if hovered and not disabled
        if clickable.is_hovered && !clickable.disabled {
            if let Some(bg) = clickable.hover_bg {
                container = container.background(bg);
            }
        }

        container = container.child(clickable.child);
        container.into()
    }
}

/// A registry for tracking clickable components and their bounds.
///
/// Use this to manage mouse event dispatch to clickable components.
#[derive(Default)]
pub struct ClickableRegistry {
    /// Registered clickables with their node IDs.
    clickables: Vec<(NodeId, Clickable)>,
    /// Currently pressed node (for tracking click completion).
    pressed_node: Option<NodeId>,
}

impl ClickableRegistry {
    /// Create a new registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a clickable component.
    pub fn register(&mut self, node_id: NodeId, clickable: Clickable) {
        self.clickables.push((node_id, clickable));
    }

    /// Unregister a clickable by node ID.
    pub fn unregister(&mut self, node_id: NodeId) {
        self.clickables.retain(|(id, _)| *id != node_id);
    }

    /// Get a mutable reference to a clickable by node ID.
    pub fn get_mut(&mut self, node_id: NodeId) -> Option<&mut Clickable> {
        self.clickables
            .iter_mut()
            .find(|(id, _)| *id == node_id)
            .map(|(_, c)| c)
    }

    /// Clear all registered clickables.
    pub fn clear(&mut self) {
        self.clickables.clear();
        self.pressed_node = None;
    }

    /// Get the number of registered clickables.
    pub fn len(&self) -> usize {
        self.clickables.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.clickables.is_empty()
    }

    /// Handle mouse press - track which node was pressed.
    pub fn handle_press(&mut self, node_id: NodeId) {
        self.pressed_node = Some(node_id);
    }

    /// Handle mouse release - check if it completes a click.
    ///
    /// Returns `true` if this was a complete click (press and release on same node).
    pub fn handle_release(&mut self, node_id: NodeId) -> bool {
        let was_click = self.pressed_node == Some(node_id);
        self.pressed_node = None;
        was_click
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::node::TextNode;
    use crate::terminal::KeyModifiers;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn make_mouse_event(
        kind: MouseEventKind,
        button: Option<MouseButton>,
        x: u16,
        y: u16,
    ) -> MouseEvent {
        MouseEvent {
            button,
            kind,
            x,
            y,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn test_clickable_new() {
        let clickable = Clickable::new(TextNode::new("Test"));
        assert!(!clickable.is_hovered());
        assert!(!clickable.is_disabled());
    }

    #[test]
    fn test_clickable_on_click() {
        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = clicked.clone();

        let mut clickable = Clickable::new(TextNode::new("Test")).on_click(move |_| {
            clicked_clone.store(true, Ordering::SeqCst);
        });

        // Simulate mouse up (which triggers click)
        let event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 5, 5);
        clickable.handle_mouse(&event, 0, 0);

        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn test_clickable_disabled() {
        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = clicked.clone();

        let mut clickable = Clickable::new(TextNode::new("Test"))
            .on_click(move |_| {
                clicked_clone.store(true, Ordering::SeqCst);
            })
            .disabled(true);

        let event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 5, 5);
        let handled = clickable.handle_mouse(&event, 0, 0);

        assert!(!handled);
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn test_clickable_hover() {
        let hover_count = Arc::new(AtomicUsize::new(0));
        let unhover_count = Arc::new(AtomicUsize::new(0));

        let hover_clone = hover_count.clone();
        let unhover_clone = unhover_count.clone();

        let mut clickable = Clickable::new(TextNode::new("Test"))
            .on_hover(move || {
                hover_clone.fetch_add(1, Ordering::SeqCst);
            })
            .on_unhover(move || {
                unhover_clone.fetch_add(1, Ordering::SeqCst);
            });

        // Move mouse over
        let event = make_mouse_event(MouseEventKind::Moved, None, 5, 5);
        clickable.handle_mouse(&event, 0, 0);

        assert!(clickable.is_hovered());
        assert_eq!(hover_count.load(Ordering::SeqCst), 1);

        // Move mouse again (should not trigger hover again)
        clickable.handle_mouse(&event, 1, 0);
        assert_eq!(hover_count.load(Ordering::SeqCst), 1);

        // Leave
        clickable.handle_mouse_leave();
        assert!(!clickable.is_hovered());
        assert_eq!(unhover_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_click_event_data() {
        let captured_x = Arc::new(AtomicUsize::new(0));
        let captured_y = Arc::new(AtomicUsize::new(0));

        let x_clone = captured_x.clone();
        let y_clone = captured_y.clone();

        let mut clickable = Clickable::new(TextNode::new("Test")).on_click(move |event| {
            x_clone.store(event.local_x as usize, Ordering::SeqCst);
            y_clone.store(event.local_y as usize, Ordering::SeqCst);
        });

        let event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 10, 20);
        clickable.handle_mouse(&event, 5, 8);

        assert_eq!(captured_x.load(Ordering::SeqCst), 5);
        assert_eq!(captured_y.load(Ordering::SeqCst), 8);
    }

    #[test]
    fn test_clickable_registry() {
        let mut registry = ClickableRegistry::new();
        assert!(registry.is_empty());

        let id1 = NodeId::new();
        let id2 = NodeId::new();

        registry.register(id1, Clickable::new(TextNode::new("1")));
        registry.register(id2, Clickable::new(TextNode::new("2")));

        assert_eq!(registry.len(), 2);

        registry.unregister(id1);
        assert_eq!(registry.len(), 1);

        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_click_press_release_tracking() {
        let mut registry = ClickableRegistry::new();
        let id = NodeId::new();

        registry.register(id, Clickable::new(TextNode::new("Test")));

        // Press on the node
        registry.handle_press(id);

        // Release on same node - should be a click
        assert!(registry.handle_release(id));

        // Press on node, release elsewhere - not a click
        registry.handle_press(id);
        let other_id = NodeId::new();
        assert!(!registry.handle_release(other_id));
    }

    #[test]
    fn test_clickable_into_node() {
        let clickable = Clickable::new(TextNode::new("Test"));
        let _node: Node = clickable.into();
        // Should not panic
    }

    #[test]
    fn test_clickable_debug() {
        let clickable = Clickable::new(TextNode::new("Test"))
            .on_click(|_| {})
            .disabled(true);

        let debug_str = format!("{:?}", clickable);
        assert!(debug_str.contains("Clickable"));
        assert!(debug_str.contains("has_on_click"));
    }
}
