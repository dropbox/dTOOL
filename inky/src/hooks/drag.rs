//! Drag and drop utilities for mouse interactions.
//!
//! This module provides infrastructure for implementing drag-and-drop
//! functionality using the hit testing system.
//!
//! # Overview
//!
//! Drag and drop operations consist of three phases:
//!
//! 1. **Drag start**: When the user presses the mouse button on a draggable component
//! 2. **Dragging**: While the user moves the mouse with the button held
//! 3. **Drop**: When the user releases the mouse button over a drop zone
//!
//! # Example
//!
//! ```rust,ignore
//! use inky::prelude::*;
//!
//! // Create a draggable item
//! let item = Draggable::new(TextNode::new("Drag me!"))
//!     .on_drag_start(|event| {
//!         println!("Started dragging from ({}, {})", event.start_x, event.start_y);
//!     })
//!     .on_drag(|event| {
//!         println!("Dragging at ({}, {})", event.current_x, event.current_y);
//!     });
//!
//! // Create a drop zone
//! let zone = DropZone::new(TextNode::new("Drop here"))
//!     .on_drop(|event| {
//!         println!("Dropped at ({}, {})", event.drop_x, event.drop_y);
//!     });
//! ```

use crate::hit_test::HitTestResult;
use crate::layout::Layout;
use crate::node::NodeId;
use crate::terminal::{MouseButton, MouseEvent, MouseEventKind};
use parking_lot::RwLock;
use std::any::Any;
use std::sync::{Arc, OnceLock};

/// Event data for drag operations.
#[derive(Debug, Clone)]
pub struct DragEvent {
    /// The mouse button used for dragging.
    pub button: MouseButton,
    /// X coordinate where the drag started.
    pub start_x: u16,
    /// Y coordinate where the drag started.
    pub start_y: u16,
    /// Current X coordinate.
    pub current_x: u16,
    /// Current Y coordinate.
    pub current_y: u16,
    /// Delta X from start position.
    pub delta_x: i32,
    /// Delta Y from start position.
    pub delta_y: i32,
    /// Optional data associated with the drag.
    pub data: Option<Arc<dyn Any + Send + Sync>>,
}

/// Event data for drop operations.
#[derive(Debug, Clone)]
pub struct DropEvent {
    /// The mouse button that was released.
    pub button: MouseButton,
    /// X coordinate where the drop occurred.
    pub drop_x: u16,
    /// Y coordinate where the drop occurred.
    pub drop_y: u16,
    /// Local X within the drop zone.
    pub local_x: u16,
    /// Local Y within the drop zone.
    pub local_y: u16,
    /// Data from the dragged item.
    pub data: Option<Arc<dyn Any + Send + Sync>>,
    /// Node ID of the source draggable.
    pub source_id: NodeId,
}

/// Callback type for drag start events.
pub type DragStartHandler = Arc<dyn Fn(&DragEvent) -> bool + Send + Sync>;

/// Callback type for drag events (during drag).
pub type DragHandler = Arc<dyn Fn(&DragEvent) + Send + Sync>;

/// Callback type for drag end events.
pub type DragEndHandler = Arc<dyn Fn(&DragEvent) + Send + Sync>;

/// Callback type for drop events.
pub type DropHandler = Arc<dyn Fn(&DropEvent) + Send + Sync>;

/// Callback type for checking if a drop is accepted.
pub type AcceptDropHandler = Arc<dyn Fn(&DropEvent) -> bool + Send + Sync>;

/// State of the current drag operation.
#[derive(Debug, Clone)]
pub struct DragState {
    /// Whether a drag is currently in progress.
    pub is_dragging: bool,
    /// The source node being dragged.
    pub source_id: Option<NodeId>,
    /// The mouse button being used.
    pub button: Option<MouseButton>,
    /// Starting X coordinate.
    pub start_x: u16,
    /// Starting Y coordinate.
    pub start_y: u16,
    /// Current X coordinate.
    pub current_x: u16,
    /// Current Y coordinate.
    pub current_y: u16,
    /// Layout of the source at drag start.
    pub source_layout: Option<Layout>,
    /// Data associated with the drag.
    pub data: Option<Arc<dyn Any + Send + Sync>>,
}

impl Default for DragState {
    fn default() -> Self {
        Self::new()
    }
}

impl DragState {
    /// Create a new empty drag state.
    pub fn new() -> Self {
        Self {
            is_dragging: false,
            source_id: None,
            button: None,
            start_x: 0,
            start_y: 0,
            current_x: 0,
            current_y: 0,
            source_layout: None,
            data: None,
        }
    }

    /// Start a drag operation.
    pub fn start(
        &mut self,
        source_id: NodeId,
        button: MouseButton,
        x: u16,
        y: u16,
        layout: Layout,
        data: Option<Arc<dyn Any + Send + Sync>>,
    ) {
        self.is_dragging = true;
        self.source_id = Some(source_id);
        self.button = Some(button);
        self.start_x = x;
        self.start_y = y;
        self.current_x = x;
        self.current_y = y;
        self.source_layout = Some(layout);
        self.data = data;
    }

    /// Update the current position during drag.
    pub fn update(&mut self, x: u16, y: u16) {
        self.current_x = x;
        self.current_y = y;
    }

    /// End the drag operation.
    pub fn end(&mut self) {
        self.is_dragging = false;
        self.source_id = None;
        self.button = None;
        self.source_layout = None;
        self.data = None;
    }

    /// Get the delta from start position.
    pub fn delta(&self) -> (i32, i32) {
        (
            i32::from(self.current_x) - i32::from(self.start_x),
            i32::from(self.current_y) - i32::from(self.start_y),
        )
    }

    /// Create a DragEvent from current state.
    pub fn to_drag_event(&self) -> Option<DragEvent> {
        if !self.is_dragging {
            return None;
        }

        let (delta_x, delta_y) = self.delta();

        Some(DragEvent {
            button: self.button?,
            start_x: self.start_x,
            start_y: self.start_y,
            current_x: self.current_x,
            current_y: self.current_y,
            delta_x,
            delta_y,
            data: self.data.clone(),
        })
    }
}

/// Entry in the draggable registry.
#[derive(Clone)]
struct DraggableEntry {
    /// The node ID for hit testing.
    node_id: NodeId,
    /// Callback for drag start - returns true to allow drag.
    on_drag_start: Option<DragStartHandler>,
    /// Callback during drag.
    on_drag: Option<DragHandler>,
    /// Callback when drag ends.
    on_drag_end: Option<DragEndHandler>,
    /// Data to associate with drags from this source.
    drag_data: Option<Arc<dyn Any + Send + Sync>>,
    /// Whether this draggable is disabled.
    disabled: bool,
}

/// Entry in the drop zone registry.
#[derive(Clone)]
struct DropZoneEntry {
    /// The node ID for hit testing.
    node_id: NodeId,
    /// Callback to check if drop is accepted.
    accept_drop: Option<AcceptDropHandler>,
    /// Callback when drop occurs.
    on_drop: Option<DropHandler>,
    /// Callback when draggable enters the zone.
    on_drag_enter: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Callback when draggable leaves the zone.
    on_drag_leave: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Whether this drop zone is disabled.
    disabled: bool,
    /// Layout bounds (updated after layout).
    layout: Option<Layout>,
}

/// Global drag and drop state.
#[derive(Default)]
struct DragDropState {
    /// Current drag state.
    drag: DragState,
    /// Registered draggable sources.
    draggables: Vec<DraggableEntry>,
    /// Registered drop zones.
    drop_zones: Vec<DropZoneEntry>,
    /// Currently hovered drop zone.
    hovered_zone: Option<NodeId>,
}

/// Global drag/drop state.
static DRAG_STATE: OnceLock<RwLock<DragDropState>> = OnceLock::new();

/// Get or initialize the drag state.
fn get_drag_state() -> &'static RwLock<DragDropState> {
    DRAG_STATE.get_or_init(|| RwLock::new(DragDropState::default()))
}

/// Register a draggable source for drag operations.
pub fn register_draggable(
    node_id: NodeId,
    on_drag_start: Option<DragStartHandler>,
    on_drag: Option<DragHandler>,
    on_drag_end: Option<DragEndHandler>,
    drag_data: Option<Arc<dyn Any + Send + Sync>>,
    disabled: bool,
) {
    get_drag_state().write().draggables.push(DraggableEntry {
        node_id,
        on_drag_start,
        on_drag,
        on_drag_end,
        drag_data,
        disabled,
    });
}

/// Register a drop zone for drop operations.
pub fn register_drop_zone(
    node_id: NodeId,
    accept_drop: Option<AcceptDropHandler>,
    on_drop: Option<DropHandler>,
    on_drag_enter: Option<Arc<dyn Fn() + Send + Sync>>,
    on_drag_leave: Option<Arc<dyn Fn() + Send + Sync>>,
    disabled: bool,
) {
    get_drag_state().write().drop_zones.push(DropZoneEntry {
        node_id,
        accept_drop,
        on_drop,
        on_drag_enter,
        on_drag_leave,
        disabled,
        layout: None,
    });
}

/// Clear all registered draggables and drop zones.
pub fn clear_drag_drop() {
    let mut state = get_drag_state().write();
    state.draggables.clear();
    state.drop_zones.clear();
    // Don't clear drag state - it persists across frames
}

/// Update layout for a drop zone.
pub fn update_drop_zone_layout(node_id: NodeId, layout: Layout) {
    let mut state = get_drag_state().write();
    for zone in &mut state.drop_zones {
        if zone.node_id == node_id {
            zone.layout = Some(layout);
            break;
        }
    }
}

/// Check if a drag operation is in progress.
pub fn is_dragging() -> bool {
    get_drag_state().read().drag.is_dragging
}

/// Get the current drag state.
pub fn get_current_drag() -> Option<DragState> {
    let state = get_drag_state().read();
    if state.drag.is_dragging {
        Some(state.drag.clone())
    } else {
        None
    }
}

/// Dispatch a mouse event for drag and drop handling.
///
/// Returns `true` if the event was handled as a drag/drop operation.
pub fn dispatch_drag_event(event: &MouseEvent, hit_result: Option<&HitTestResult>) -> bool {
    let mut state = get_drag_state().write();

    match event.kind {
        MouseEventKind::Down => {
            // Check if we're starting a drag on a draggable
            if let Some(hit) = hit_result {
                // Find draggable at this node - extract only what we need
                let entry_data = state
                    .draggables
                    .iter()
                    .find(|e| e.node_id == hit.node_id || hit.path.contains(&e.node_id))
                    .map(|e| {
                        (
                            e.node_id,
                            e.disabled,
                            e.on_drag_start.clone(),
                            e.drag_data.clone(),
                        )
                    });

                if let Some((node_id, disabled, on_drag_start, drag_data)) = entry_data {
                    if !disabled {
                        if let Some(button) = event.button {
                            // Create drag event for the callback
                            let drag_event = DragEvent {
                                button,
                                start_x: event.x,
                                start_y: event.y,
                                current_x: event.x,
                                current_y: event.y,
                                delta_x: 0,
                                delta_y: 0,
                                data: drag_data.clone(),
                            };

                            // Check if drag is allowed
                            let allow_drag = on_drag_start
                                .as_ref()
                                .map_or(true, |handler| handler(&drag_event));

                            if allow_drag {
                                state.drag.start(
                                    node_id, button, event.x, event.y, hit.layout, drag_data,
                                );
                                return true;
                            }
                        }
                    }
                }
            }
        }

        MouseEventKind::Drag => {
            // Update drag position if we're dragging
            if state.drag.is_dragging {
                state.drag.update(event.x, event.y);

                // Call drag handler on the source
                if let Some(source_id) = state.drag.source_id {
                    let on_drag = state
                        .draggables
                        .iter()
                        .find(|e| e.node_id == source_id)
                        .and_then(|e| e.on_drag.clone());

                    if let (Some(handler), Some(drag_event)) = (on_drag, state.drag.to_drag_event())
                    {
                        handler(&drag_event);
                    }
                }

                // Check for drop zone hover changes
                let new_hovered = hit_result.and_then(|hit| {
                    state
                        .drop_zones
                        .iter()
                        .find(|z| {
                            !z.disabled
                                && (z.node_id == hit.node_id || hit.path.contains(&z.node_id))
                        })
                        .map(|z| z.node_id)
                });

                let old_hovered = state.hovered_zone;

                // Handle zone enter/leave
                if new_hovered != old_hovered {
                    // Leave old zone
                    if let Some(old_id) = old_hovered {
                        if let Some(handler) = state
                            .drop_zones
                            .iter()
                            .find(|z| z.node_id == old_id)
                            .and_then(|z| z.on_drag_leave.clone())
                        {
                            handler();
                        }
                    }

                    // Enter new zone
                    if let Some(new_id) = new_hovered {
                        if let Some(handler) = state
                            .drop_zones
                            .iter()
                            .find(|z| z.node_id == new_id)
                            .and_then(|z| z.on_drag_enter.clone())
                        {
                            handler();
                        }
                    }

                    state.hovered_zone = new_hovered;
                }

                return true;
            }
        }

        MouseEventKind::Up => {
            // End drag and potentially trigger drop
            if state.drag.is_dragging {
                let source_id = state.drag.source_id;
                let button = state.drag.button;
                let drag_data = state.drag.data.clone();

                // Check if we're over a drop zone
                if let (Some(hit), Some(src_id), Some(btn)) = (hit_result, source_id, button) {
                    // Find drop zone at this location
                    let zone_data = state
                        .drop_zones
                        .iter()
                        .find(|z| {
                            !z.disabled
                                && (z.node_id == hit.node_id || hit.path.contains(&z.node_id))
                        })
                        .map(|z| (z.node_id, z.accept_drop.clone(), z.on_drop.clone()));

                    if let Some((zone_id, accept_drop, on_drop)) = zone_data {
                        let drop_event = DropEvent {
                            button: btn,
                            drop_x: event.x,
                            drop_y: event.y,
                            local_x: hit.local_x,
                            local_y: hit.local_y,
                            data: drag_data.clone(),
                            source_id: src_id,
                        };

                        // Check if drop is accepted
                        let accepted = accept_drop
                            .as_ref()
                            .map_or(true, |handler| handler(&drop_event));

                        if accepted {
                            if let Some(handler) = on_drop {
                                handler(&drop_event);
                            }
                        }

                        // Clear hovered zone
                        if state.hovered_zone == Some(zone_id) {
                            if let Some(handler) = state
                                .drop_zones
                                .iter()
                                .find(|z| z.node_id == zone_id)
                                .and_then(|z| z.on_drag_leave.clone())
                            {
                                handler();
                            }
                            state.hovered_zone = None;
                        }
                    }
                }

                // Call drag end handler on source
                if let Some(src_id) = source_id {
                    let on_drag_end = state
                        .draggables
                        .iter()
                        .find(|e| e.node_id == src_id)
                        .and_then(|e| e.on_drag_end.clone());

                    if let (Some(handler), Some(drag_event)) =
                        (on_drag_end, state.drag.to_drag_event())
                    {
                        handler(&drag_event);
                    }
                }

                state.drag.end();
                return true;
            }
        }

        _ => {}
    }

    false
}

/// Get the number of registered draggables.
#[cfg(test)]
pub fn draggable_count() -> usize {
    get_drag_state().read().draggables.len()
}

/// Get the number of registered drop zones.
#[cfg(test)]
pub fn drop_zone_count() -> usize {
    get_drag_state().read().drop_zones.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::terminal::KeyModifiers;
    use serial_test::serial;
    use smallvec::SmallVec;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn setup() {
        clear_drag_drop();
        get_drag_state().write().drag = DragState::new();
        get_drag_state().write().hovered_zone = None;
    }

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

    fn make_hit_result(node_id: NodeId) -> HitTestResult {
        HitTestResult {
            node_id,
            layout: Layout {
                x: 0,
                y: 0,
                width: 10,
                height: 5,
            },
            local_x: 5,
            local_y: 2,
            path: SmallVec::new(),
        }
    }

    #[test]
    #[serial]
    fn test_register_draggable() {
        setup();

        let id = NodeId::new();
        register_draggable(id, None, None, None, None, false);

        assert_eq!(draggable_count(), 1);
    }

    #[test]
    #[serial]
    fn test_register_drop_zone() {
        setup();

        let id = NodeId::new();
        register_drop_zone(id, None, None, None, None, false);

        assert_eq!(drop_zone_count(), 1);
    }

    #[test]
    #[serial]
    fn test_drag_start() {
        setup();

        let drag_started = Arc::new(AtomicBool::new(false));
        let drag_started_clone = drag_started.clone();

        let id = NodeId::new();
        register_draggable(
            id,
            Some(Arc::new(move |_| {
                drag_started_clone.store(true, Ordering::SeqCst);
                true
            })),
            None,
            None,
            None,
            false,
        );

        let hit = make_hit_result(id);

        // Mouse down should start drag
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 2);
        let handled = dispatch_drag_event(&down_event, Some(&hit));

        assert!(handled);
        assert!(drag_started.load(Ordering::SeqCst));
        assert!(is_dragging());
    }

    #[test]
    #[serial]
    fn test_drag_prevented() {
        setup();

        let id = NodeId::new();
        register_draggable(
            id,
            Some(Arc::new(|_| false)), // Prevent drag
            None,
            None,
            None,
            false,
        );

        let hit = make_hit_result(id);

        // Mouse down should not start drag
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 2);
        dispatch_drag_event(&down_event, Some(&hit));

        assert!(!is_dragging());
    }

    #[test]
    #[serial]
    fn test_drag_move() {
        setup();

        let drag_count = Arc::new(AtomicUsize::new(0));
        let drag_count_clone = drag_count.clone();

        let id = NodeId::new();
        register_draggable(
            id,
            None,
            Some(Arc::new(move |_| {
                drag_count_clone.fetch_add(1, Ordering::SeqCst);
            })),
            None,
            None,
            false,
        );

        let hit = make_hit_result(id);

        // Start drag
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 2);
        dispatch_drag_event(&down_event, Some(&hit));

        // Move during drag
        let drag_event = make_mouse_event(MouseEventKind::Drag, Some(MouseButton::Left), 10, 5);
        dispatch_drag_event(&drag_event, Some(&hit));

        assert_eq!(drag_count.load(Ordering::SeqCst), 1);

        // Check position updated
        let state = get_current_drag().unwrap();
        assert_eq!(state.current_x, 10);
        assert_eq!(state.current_y, 5);
    }

    #[test]
    #[serial]
    fn test_drag_end() {
        setup();

        let drag_ended = Arc::new(AtomicBool::new(false));
        let drag_ended_clone = drag_ended.clone();

        let id = NodeId::new();
        register_draggable(
            id,
            None,
            None,
            Some(Arc::new(move |_| {
                drag_ended_clone.store(true, Ordering::SeqCst);
            })),
            None,
            false,
        );

        let hit = make_hit_result(id);

        // Start drag
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 2);
        dispatch_drag_event(&down_event, Some(&hit));

        // End drag
        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 10, 5);
        dispatch_drag_event(&up_event, Some(&hit));

        assert!(drag_ended.load(Ordering::SeqCst));
        assert!(!is_dragging());
    }

    #[test]
    #[serial]
    fn test_drop_on_zone() {
        setup();

        let dropped = Arc::new(AtomicBool::new(false));
        let dropped_clone = dropped.clone();

        // Register draggable
        let drag_id = NodeId::new();
        register_draggable(drag_id, None, None, None, None, false);

        // Register drop zone
        let drop_id = NodeId::new();
        register_drop_zone(
            drop_id,
            None,
            Some(Arc::new(move |_| {
                dropped_clone.store(true, Ordering::SeqCst);
            })),
            None,
            None,
            false,
        );

        let drag_hit = make_hit_result(drag_id);
        let drop_hit = make_hit_result(drop_id);

        // Start drag
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 2);
        dispatch_drag_event(&down_event, Some(&drag_hit));

        // Drop on zone
        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 15, 8);
        dispatch_drag_event(&up_event, Some(&drop_hit));

        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_drop_rejected() {
        setup();

        let dropped = Arc::new(AtomicBool::new(false));
        let dropped_clone = dropped.clone();

        // Register draggable
        let drag_id = NodeId::new();
        register_draggable(drag_id, None, None, None, None, false);

        // Register drop zone that rejects drops
        let drop_id = NodeId::new();
        register_drop_zone(
            drop_id,
            Some(Arc::new(|_| false)), // Reject all drops
            Some(Arc::new(move |_| {
                dropped_clone.store(true, Ordering::SeqCst);
            })),
            None,
            None,
            false,
        );

        let drag_hit = make_hit_result(drag_id);
        let drop_hit = make_hit_result(drop_id);

        // Start drag
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 2);
        dispatch_drag_event(&down_event, Some(&drag_hit));

        // Try to drop on zone
        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 15, 8);
        dispatch_drag_event(&up_event, Some(&drop_hit));

        // Drop should not have occurred
        assert!(!dropped.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_drag_enter_leave() {
        setup();

        let entered = Arc::new(AtomicBool::new(false));
        let left = Arc::new(AtomicBool::new(false));
        let entered_clone = entered.clone();
        let left_clone = left.clone();

        // Register draggable
        let drag_id = NodeId::new();
        register_draggable(drag_id, None, None, None, None, false);

        // Register drop zone with enter/leave handlers
        let drop_id = NodeId::new();
        register_drop_zone(
            drop_id,
            None,
            None,
            Some(Arc::new(move || {
                entered_clone.store(true, Ordering::SeqCst);
            })),
            Some(Arc::new(move || {
                left_clone.store(true, Ordering::SeqCst);
            })),
            false,
        );

        let drag_hit = make_hit_result(drag_id);
        let drop_hit = make_hit_result(drop_id);

        // Start drag
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 2);
        dispatch_drag_event(&down_event, Some(&drag_hit));

        // Drag over drop zone
        let drag_event = make_mouse_event(MouseEventKind::Drag, Some(MouseButton::Left), 15, 8);
        dispatch_drag_event(&drag_event, Some(&drop_hit));

        assert!(entered.load(Ordering::SeqCst));

        // Drag away from drop zone
        let drag_event2 = make_mouse_event(MouseEventKind::Drag, Some(MouseButton::Left), 50, 50);
        dispatch_drag_event(&drag_event2, None);

        assert!(left.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_disabled_draggable() {
        setup();

        let id = NodeId::new();
        register_draggable(id, None, None, None, None, true); // Disabled

        let hit = make_hit_result(id);

        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 2);
        dispatch_drag_event(&down_event, Some(&hit));

        assert!(!is_dragging());
    }

    #[test]
    #[serial]
    fn test_drag_state() {
        let mut state = DragState::new();
        assert!(!state.is_dragging);

        state.start(
            NodeId::new(),
            MouseButton::Left,
            10,
            20,
            Layout::new(0, 0, 100, 100),
            None,
        );

        assert!(state.is_dragging);
        assert_eq!(state.start_x, 10);
        assert_eq!(state.start_y, 20);

        state.update(30, 40);
        let (dx, dy) = state.delta();
        assert_eq!(dx, 20);
        assert_eq!(dy, 20);

        state.end();
        assert!(!state.is_dragging);
    }
}
