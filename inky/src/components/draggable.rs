//! Draggable wrapper component for drag-and-drop interactions.
//!
//! This module provides the `Draggable` component, which wraps any node
//! and enables drag-and-drop functionality.
//!
//! # Example
//!
//! ```rust
//! use inky::prelude::*;
//! use inky::components::Draggable;
//!
//! // Create a draggable item
//! let item = Draggable::new(TextNode::new("Drag me!"))
//!     .on_drag_start(|event| {
//!         println!("Started dragging from ({}, {})", event.start_x, event.start_y);
//!         true // Allow drag to start
//!     })
//!     .on_drag(|event| {
//!         println!("Dragging... delta: ({}, {})", event.delta_x, event.delta_y);
//!     })
//!     .on_drag_end(|event| {
//!         println!("Drag ended at ({}, {})", event.current_x, event.current_y);
//!     });
//! ```
//!
//! # Drag Data
//!
//! Draggables can carry data that is transferred to drop zones:
//!
//! ```rust
//! use inky::prelude::*;
//! use inky::components::Draggable;
//! use std::sync::Arc;
//!
//! // Create a draggable with associated data
//! let item = Draggable::new(TextNode::new("Item 1"))
//!     .drag_data(Arc::new("item-1".to_string()));
//! ```

use crate::hooks::drag::{
    register_draggable, DragEndHandler, DragEvent, DragHandler, DragStartHandler,
};
use crate::node::{BoxNode, Node, NodeId};
use crate::style::Color;
use std::any::Any;
use std::sync::Arc;

/// A wrapper that makes any node draggable.
///
/// `Draggable` wraps a child node and enables drag-and-drop functionality.
/// The child node is rendered normally, and mouse drag events trigger
/// the registered handlers.
///
/// # Example
///
/// ```rust
/// use inky::prelude::*;
/// use inky::components::Draggable;
///
/// let item = Draggable::new(TextNode::new("Drag me!"))
///     .on_drag_start(|_| true)
///     .on_drag(|event| {
///         println!("Dragging at ({}, {})", event.current_x, event.current_y);
///     });
/// ```
#[derive(Clone)]
pub struct Draggable {
    /// The wrapped child node.
    child: Node,
    /// Handler called when drag starts - returns true to allow drag.
    on_drag_start: Option<DragStartHandler>,
    /// Handler called during drag.
    on_drag: Option<DragHandler>,
    /// Handler called when drag ends.
    on_drag_end: Option<DragEndHandler>,
    /// Data to associate with this draggable.
    drag_data: Option<Arc<dyn Any + Send + Sync>>,
    /// Whether this draggable is disabled.
    disabled: bool,
    /// Optional node ID for registration.
    id: Option<NodeId>,
    /// Whether currently being dragged.
    is_dragging: bool,
    /// Background color when being dragged.
    drag_bg: Option<Color>,
}

impl Draggable {
    /// Create a new draggable wrapping the given child.
    pub fn new(child: impl Into<Node>) -> Self {
        Self {
            child: child.into(),
            on_drag_start: None,
            on_drag: None,
            on_drag_end: None,
            drag_data: None,
            disabled: false,
            id: None,
            is_dragging: false,
            drag_bg: None,
        }
    }

    /// Set a drag start handler (called when drag begins).
    ///
    /// Return `true` from the handler to allow the drag, `false` to prevent it.
    pub fn on_drag_start<F>(mut self, handler: F) -> Self
    where
        F: Fn(&DragEvent) -> bool + Send + Sync + 'static,
    {
        self.on_drag_start = Some(Arc::new(handler));
        self
    }

    /// Set a drag handler (called while dragging).
    pub fn on_drag<F>(mut self, handler: F) -> Self
    where
        F: Fn(&DragEvent) + Send + Sync + 'static,
    {
        self.on_drag = Some(Arc::new(handler));
        self
    }

    /// Set a drag end handler (called when drag ends).
    pub fn on_drag_end<F>(mut self, handler: F) -> Self
    where
        F: Fn(&DragEvent) + Send + Sync + 'static,
    {
        self.on_drag_end = Some(Arc::new(handler));
        self
    }

    /// Set data to transfer during drag-and-drop.
    ///
    /// This data will be available to drop zones when the item is dropped.
    pub fn drag_data<T: Any + Send + Sync + 'static>(mut self, data: Arc<T>) -> Self {
        self.drag_data = Some(data);
        self
    }

    /// Disable the draggable (won't respond to drag events).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set a custom node ID for this draggable.
    pub fn id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set background color to apply when being dragged.
    pub fn drag_background(mut self, bg: impl Into<Option<Color>>) -> Self {
        self.drag_bg = bg.into();
        self
    }

    /// Check if this draggable is currently being dragged.
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }

    /// Check if this draggable is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Get the drag data if set.
    pub fn data(&self) -> Option<&Arc<dyn Any + Send + Sync>> {
        self.drag_data.as_ref()
    }

    /// Get the node ID if set.
    pub fn node_id(&self) -> Option<NodeId> {
        self.id
    }

    /// Register this draggable with the global drag system.
    ///
    /// This should be called during render to enable drag functionality.
    pub fn register(&self) {
        let node_id = self.id.unwrap_or_default();
        register_draggable(
            node_id,
            self.on_drag_start.clone(),
            self.on_drag.clone(),
            self.on_drag_end.clone(),
            self.drag_data.clone(),
            self.disabled,
        );
    }
}

impl std::fmt::Debug for Draggable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Draggable")
            .field("child", &self.child)
            .field("has_on_drag_start", &self.on_drag_start.is_some())
            .field("has_on_drag", &self.on_drag.is_some())
            .field("has_on_drag_end", &self.on_drag_end.is_some())
            .field("has_drag_data", &self.drag_data.is_some())
            .field("disabled", &self.disabled)
            .field("is_dragging", &self.is_dragging)
            .finish()
    }
}

impl From<Draggable> for Node {
    fn from(draggable: Draggable) -> Self {
        // Register the draggable with the global drag system
        draggable.register();

        // Wrap the child in a box container
        let mut container = BoxNode::new();

        if let Some(id) = draggable.id {
            container = container.id(id);
        }

        // Apply drag background if dragging and not disabled
        if draggable.is_dragging && !draggable.disabled {
            if let Some(bg) = draggable.drag_bg {
                container = container.background(bg);
            }
        }

        container = container.child(draggable.child);
        container.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hooks::drag::{clear_drag_drop, draggable_count};
    use crate::node::TextNode;
    use serial_test::serial;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn setup() {
        clear_drag_drop();
    }

    #[test]
    #[serial]
    fn test_draggable_new() {
        setup();
        let draggable = Draggable::new(TextNode::new("Test"));
        assert!(!draggable.is_dragging());
        assert!(!draggable.is_disabled());
    }

    #[test]
    #[serial]
    fn test_draggable_disabled() {
        setup();
        let draggable = Draggable::new(TextNode::new("Test")).disabled(true);
        assert!(draggable.is_disabled());
    }

    #[test]
    #[serial]
    fn test_draggable_with_data() {
        setup();
        let data = Arc::new("test-data".to_string());
        let draggable = Draggable::new(TextNode::new("Test")).drag_data(data.clone());
        assert!(draggable.data().is_some());
    }

    #[test]
    #[serial]
    fn test_draggable_with_id() {
        setup();
        let id = NodeId::new();
        let draggable = Draggable::new(TextNode::new("Test")).id(id);
        assert_eq!(draggable.node_id(), Some(id));
    }

    #[test]
    #[serial]
    fn test_draggable_registration() {
        setup();
        let id = NodeId::new();
        let draggable = Draggable::new(TextNode::new("Test")).id(id);
        draggable.register();
        assert_eq!(draggable_count(), 1);
    }

    #[test]
    #[serial]
    fn test_draggable_into_node() {
        setup();
        let draggable = Draggable::new(TextNode::new("Test"));
        let _node: Node = draggable.into();
        // Should register when converted
        assert_eq!(draggable_count(), 1);
    }

    #[test]
    #[serial]
    fn test_draggable_debug() {
        setup();
        let draggable = Draggable::new(TextNode::new("Test"))
            .on_drag_start(|_| true)
            .disabled(true);

        let debug_str = format!("{:?}", draggable);
        assert!(debug_str.contains("Draggable"));
        assert!(debug_str.contains("has_on_drag_start"));
    }

    #[test]
    #[serial]
    fn test_draggable_handlers() {
        setup();
        let started = Arc::new(AtomicBool::new(false));
        let started_clone = started.clone();

        let _draggable = Draggable::new(TextNode::new("Test"))
            .on_drag_start(move |_| {
                started_clone.store(true, Ordering::SeqCst);
                true
            })
            .on_drag(|_| {})
            .on_drag_end(|_| {});

        // Handlers are set but not called yet
        assert!(!started.load(Ordering::SeqCst));
    }
}
