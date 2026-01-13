//! Drop zone wrapper component for receiving dropped items.
//!
//! This module provides the `DropZone` component, which wraps any node
//! and enables receiving dropped items from drag-and-drop operations.
//!
//! # Example
//!
//! ```rust
//! use inky::prelude::*;
//! use inky::components::DropZone;
//!
//! // Create a drop zone
//! let zone = DropZone::new(TextNode::new("Drop here"))
//!     .on_drop(|event| {
//!         println!("Received drop at ({}, {})", event.drop_x, event.drop_y);
//!     })
//!     .on_drag_enter(|| {
//!         println!("Draggable entered zone");
//!     })
//!     .on_drag_leave(|| {
//!         println!("Draggable left zone");
//!     });
//! ```
//!
//! # Accepting Drops
//!
//! Drop zones can selectively accept or reject drops:
//!
//! ```rust
//! use inky::prelude::*;
//! use inky::components::DropZone;
//!
//! // Only accept certain items
//! let zone = DropZone::new(TextNode::new("Drop files here"))
//!     .accept_drop(|event| {
//!         // Check the drag data to decide if we accept
//!         event.data.is_some()
//!     })
//!     .on_drop(|event| {
//!         println!("Accepted drop!");
//!     });
//! ```

use crate::hooks::drag::{register_drop_zone, AcceptDropHandler, DropEvent, DropHandler};
use crate::node::{BoxNode, Node, NodeId};
use crate::style::Color;
use std::sync::Arc;

/// A wrapper that makes any node a drop target for drag-and-drop.
///
/// `DropZone` wraps a child node and enables it to receive dropped items.
/// The child node is rendered normally, and drop events trigger
/// the registered handlers.
///
/// # Example
///
/// ```rust
/// use inky::prelude::*;
/// use inky::components::DropZone;
///
/// let zone = DropZone::new(TextNode::new("Drop here"))
///     .on_drop(|event| {
///         println!("Dropped at ({}, {})", event.drop_x, event.drop_y);
///     });
/// ```
#[derive(Clone)]
pub struct DropZone {
    /// The wrapped child node.
    child: Node,
    /// Handler to check if a drop is accepted.
    accept_drop: Option<AcceptDropHandler>,
    /// Handler called when a drop occurs.
    on_drop: Option<DropHandler>,
    /// Handler called when a draggable enters the zone.
    on_drag_enter: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Handler called when a draggable leaves the zone.
    on_drag_leave: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Whether this drop zone is disabled.
    disabled: bool,
    /// Optional node ID for registration.
    id: Option<NodeId>,
    /// Whether a draggable is currently hovering over this zone.
    is_drag_over: bool,
    /// Background color when a draggable is hovering.
    drag_over_bg: Option<Color>,
}

impl DropZone {
    /// Create a new drop zone wrapping the given child.
    pub fn new(child: impl Into<Node>) -> Self {
        Self {
            child: child.into(),
            accept_drop: None,
            on_drop: None,
            on_drag_enter: None,
            on_drag_leave: None,
            disabled: false,
            id: None,
            is_drag_over: false,
            drag_over_bg: None,
        }
    }

    /// Set a handler to check if a drop should be accepted.
    ///
    /// Return `true` to accept the drop, `false` to reject it.
    /// If not set, all drops are accepted.
    pub fn accept_drop<F>(mut self, handler: F) -> Self
    where
        F: Fn(&DropEvent) -> bool + Send + Sync + 'static,
    {
        self.accept_drop = Some(Arc::new(handler));
        self
    }

    /// Set a drop handler (called when an item is dropped).
    pub fn on_drop<F>(mut self, handler: F) -> Self
    where
        F: Fn(&DropEvent) + Send + Sync + 'static,
    {
        self.on_drop = Some(Arc::new(handler));
        self
    }

    /// Set a handler called when a draggable enters this zone.
    pub fn on_drag_enter<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_drag_enter = Some(Arc::new(handler));
        self
    }

    /// Set a handler called when a draggable leaves this zone.
    pub fn on_drag_leave<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_drag_leave = Some(Arc::new(handler));
        self
    }

    /// Disable the drop zone (won't accept drops).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set a custom node ID for this drop zone.
    pub fn id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set background color to apply when a draggable is hovering.
    pub fn drag_over_background(mut self, bg: impl Into<Option<Color>>) -> Self {
        self.drag_over_bg = bg.into();
        self
    }

    /// Check if a draggable is currently hovering over this zone.
    pub fn is_drag_over(&self) -> bool {
        self.is_drag_over
    }

    /// Check if this drop zone is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Get the node ID if set.
    pub fn node_id(&self) -> Option<NodeId> {
        self.id
    }

    /// Register this drop zone with the global drag system.
    ///
    /// This should be called during render to enable drop functionality.
    pub fn register(&self) {
        let node_id = self.id.unwrap_or_default();
        register_drop_zone(
            node_id,
            self.accept_drop.clone(),
            self.on_drop.clone(),
            self.on_drag_enter.clone(),
            self.on_drag_leave.clone(),
            self.disabled,
        );
    }
}

impl std::fmt::Debug for DropZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropZone")
            .field("child", &self.child)
            .field("has_accept_drop", &self.accept_drop.is_some())
            .field("has_on_drop", &self.on_drop.is_some())
            .field("has_on_drag_enter", &self.on_drag_enter.is_some())
            .field("has_on_drag_leave", &self.on_drag_leave.is_some())
            .field("disabled", &self.disabled)
            .field("is_drag_over", &self.is_drag_over)
            .finish()
    }
}

impl From<DropZone> for Node {
    fn from(drop_zone: DropZone) -> Self {
        // Register the drop zone with the global drag system
        drop_zone.register();

        // Wrap the child in a box container
        let mut container = BoxNode::new();

        if let Some(id) = drop_zone.id {
            container = container.id(id);
        }

        // Apply drag-over background if hovering and not disabled
        if drop_zone.is_drag_over && !drop_zone.disabled {
            if let Some(bg) = drop_zone.drag_over_bg {
                container = container.background(bg);
            }
        }

        container = container.child(drop_zone.child);
        container.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hooks::drag::{clear_drag_drop, drop_zone_count};
    use crate::node::TextNode;
    use serial_test::serial;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn setup() {
        clear_drag_drop();
    }

    #[test]
    #[serial]
    fn test_drop_zone_new() {
        setup();
        let zone = DropZone::new(TextNode::new("Test"));
        assert!(!zone.is_drag_over());
        assert!(!zone.is_disabled());
    }

    #[test]
    #[serial]
    fn test_drop_zone_disabled() {
        setup();
        let zone = DropZone::new(TextNode::new("Test")).disabled(true);
        assert!(zone.is_disabled());
    }

    #[test]
    #[serial]
    fn test_drop_zone_with_id() {
        setup();
        let id = NodeId::new();
        let zone = DropZone::new(TextNode::new("Test")).id(id);
        assert_eq!(zone.node_id(), Some(id));
    }

    #[test]
    #[serial]
    fn test_drop_zone_registration() {
        setup();
        let id = NodeId::new();
        let zone = DropZone::new(TextNode::new("Test")).id(id);
        zone.register();
        assert_eq!(drop_zone_count(), 1);
    }

    #[test]
    #[serial]
    fn test_drop_zone_into_node() {
        setup();
        let zone = DropZone::new(TextNode::new("Test"));
        let _node: Node = zone.into();
        // Should register when converted
        assert_eq!(drop_zone_count(), 1);
    }

    #[test]
    #[serial]
    fn test_drop_zone_debug() {
        setup();
        let zone = DropZone::new(TextNode::new("Test"))
            .on_drop(|_| {})
            .disabled(true);

        let debug_str = format!("{:?}", zone);
        assert!(debug_str.contains("DropZone"));
        assert!(debug_str.contains("has_on_drop"));
    }

    #[test]
    #[serial]
    fn test_drop_zone_handlers() {
        setup();
        let dropped = Arc::new(AtomicBool::new(false));
        let dropped_clone = dropped.clone();
        let entered = Arc::new(AtomicBool::new(false));
        let entered_clone = entered.clone();

        let _zone = DropZone::new(TextNode::new("Test"))
            .accept_drop(|_| true)
            .on_drop(move |_| {
                dropped_clone.store(true, Ordering::SeqCst);
            })
            .on_drag_enter(move || {
                entered_clone.store(true, Ordering::SeqCst);
            })
            .on_drag_leave(|| {});

        // Handlers are set but not called yet
        assert!(!dropped.load(Ordering::SeqCst));
        assert!(!entered.load(Ordering::SeqCst));
    }
}
