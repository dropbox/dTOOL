//! Click event handling hooks.
//!
//! This module provides infrastructure for dispatching mouse events to
//! [`Clickable`](crate::components::Clickable) components through hit testing.
//!
//! # Overview
//!
//! The click system works in two phases:
//!
//! 1. **Registration**: During render, `Clickable` components register themselves
//!    with the global registry via [`register_clickable`].
//!
//! 2. **Dispatch**: When mouse events occur, the App event loop calls
//!    [`dispatch_click_event`] with the hit test result to invoke handlers.
//!
//! # Example
//!
//! ```rust,ignore
//! use inky::prelude::*;
//! use inky::components::Clickable;
//!
//! // In your render function
//! let button = Clickable::new(TextNode::new("Click me!"))
//!     .on_click(|event| {
//!         println!("Clicked at ({}, {})", event.local_x, event.local_y);
//!     });
//! ```

use crate::components::{ClickEvent, ClickHandler, ClickModifiers, Clickable};
use crate::hit_test::HitTestResult;
use crate::hooks::FocusHandle;
use crate::layout::Layout;
use crate::node::NodeId;
use crate::terminal::{MouseButton, MouseEvent, MouseEventKind};
use parking_lot::RwLock;
use smallvec::SmallVec;
use std::sync::{Arc, OnceLock};

/// Entry in the clickable registry.
#[derive(Clone)]
struct ClickableEntry {
    /// The node ID for hit testing.
    node_id: NodeId,
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
    /// Whether the clickable is disabled.
    disabled: bool,
    /// Layout bounds (updated after layout).
    layout: Option<Layout>,
    /// Focus handle for automatic focus-on-click.
    focus_handle: Option<FocusHandle>,
}

impl ClickableEntry {
    fn from_clickable(node_id: NodeId, clickable: &Clickable) -> Self {
        Self {
            node_id,
            on_click: clickable.click_handler().cloned(),
            on_mouse_down: clickable.mouse_down_handler().cloned(),
            on_mouse_up: clickable.mouse_up_handler().cloned(),
            on_hover: clickable.hover_handler().cloned(),
            on_unhover: clickable.unhover_handler().cloned(),
            disabled: clickable.is_disabled(),
            layout: None,
            focus_handle: clickable.focus_handle().cloned(),
        }
    }
}

/// Global clickable registry state.
struct ClickState {
    /// Registered clickables for the current frame.
    entries: Vec<ClickableEntry>,
    /// Currently pressed node (for tracking click completion).
    pressed_node: Option<NodeId>,
    /// Currently hovered nodes.
    hovered_nodes: SmallVec<[NodeId; 4]>,
}

impl Default for ClickState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            pressed_node: None,
            hovered_nodes: SmallVec::new(),
        }
    }
}

/// Global click state.
/// Uses OnceLock + parking_lot::RwLock for thread-safe lazy initialization.
static CLICK_STATE: OnceLock<RwLock<ClickState>> = OnceLock::new();

/// Get or initialize the click state.
fn get_click_state() -> &'static RwLock<ClickState> {
    CLICK_STATE.get_or_init(|| RwLock::new(ClickState::default()))
}

/// Register a clickable component for mouse event dispatch.
///
/// This should be called during render for each `Clickable` component.
/// The registry is cleared at the start of each frame.
///
/// # Arguments
/// * `node_id` - The node ID assigned to this clickable
/// * `clickable` - The clickable component to register
pub fn register_clickable(node_id: NodeId, clickable: &Clickable) {
    get_click_state()
        .write()
        .entries
        .push(ClickableEntry::from_clickable(node_id, clickable));
}

/// Clear all registered clickables.
///
/// Called at the start of each render frame to reset the registry.
pub fn clear_clickables() {
    let mut state = get_click_state().write();
    state.entries.clear();
    // Keep pressed_node and hovered_nodes across frames
}

/// Update the layout bounds for a clickable.
///
/// Called after layout computation to associate bounds with each clickable.
pub fn update_clickable_layout(node_id: NodeId, layout: Layout) {
    let mut state = get_click_state().write();
    for entry in &mut state.entries {
        if entry.node_id == node_id {
            entry.layout = Some(layout);
            break;
        }
    }
}

/// Dispatch a mouse event to clickables based on hit test results.
///
/// This function is called by the App event loop when mouse events occur.
///
/// # Arguments
/// * `event` - The mouse event from the terminal
/// * `hit_result` - Result from hit testing, if any node was hit
///
/// # Returns
/// `true` if the event was handled by a clickable
pub fn dispatch_click_event(event: &MouseEvent, hit_result: Option<&HitTestResult>) -> bool {
    let mut state = get_click_state().write();

    match event.kind {
        MouseEventKind::Down => {
            // Track which node was pressed
            if let Some(hit) = hit_result {
                // Find clickable at this node - extract only what we need to avoid cloning FocusHandle
                let entry_data = state
                    .entries
                    .iter()
                    .find(|e| e.node_id == hit.node_id || hit.path.contains(&e.node_id))
                    .map(|e| (e.node_id, e.disabled, e.on_mouse_down.clone()));

                if let Some((node_id, disabled, on_mouse_down)) = entry_data {
                    if !disabled {
                        state.pressed_node = Some(node_id);

                        // Call mouse down handler
                        if let (Some(handler), Some(button)) = (&on_mouse_down, event.button) {
                            let click_event = make_click_event(event, hit, button);
                            handler(&click_event);
                            return true;
                        }
                    }
                }
            } else {
                state.pressed_node = None;
            }
        }

        MouseEventKind::Up => {
            let pressed = state.pressed_node.take();

            if let Some(hit) = hit_result {
                // Find clickable at this node - extract index to avoid holding borrow
                let entry_idx = state
                    .entries
                    .iter()
                    .position(|e| e.node_id == hit.node_id || hit.path.contains(&e.node_id));

                if let Some(idx) = entry_idx {
                    // Clone only what we need, not the FocusHandle
                    let node_id = state.entries[idx].node_id;
                    let disabled = state.entries[idx].disabled;
                    let on_mouse_up = state.entries[idx].on_mouse_up.clone();
                    let on_click = state.entries[idx].on_click.clone();
                    let has_focus = state.entries[idx].focus_handle.is_some();

                    if !disabled {
                        if let Some(button) = event.button {
                            let click_event = make_click_event(event, hit, button);

                            // Call mouse up handler
                            if let Some(handler) = &on_mouse_up {
                                handler(&click_event);
                            }

                            // Click is triggered if press and release were on the same clickable
                            if pressed == Some(node_id) {
                                // Focus-on-click: focus the component when clicked
                                // Need to access the entry again to call focus on the handle
                                if let Some(focus_handle) = &state.entries[idx].focus_handle {
                                    focus_handle.focus();
                                }

                                if let Some(handler) = &on_click {
                                    handler(&click_event);
                                    return true;
                                }

                                // Even without a click handler, focus-on-click counts as handled
                                if has_focus {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }

        MouseEventKind::Moved => {
            // Track hover state changes
            let mut new_hovered: SmallVec<[NodeId; 4]> = SmallVec::new();

            if let Some(hit) = hit_result {
                // Find all clickables in the hit path
                for entry in &state.entries {
                    if (entry.node_id == hit.node_id || hit.path.contains(&entry.node_id))
                        && !entry.disabled
                    {
                        new_hovered.push(entry.node_id);
                    }
                }
            }

            let mut hover_changed = false;
            // Find nodes that were hovered but are no longer
            let old_hovered = std::mem::take(&mut state.hovered_nodes);
            for old_id in &old_hovered {
                if !new_hovered.contains(old_id) {
                    hover_changed = true;
                    // Node is no longer hovered - call unhover handler
                    if let Some(entry) = state.entries.iter().find(|e| e.node_id == *old_id) {
                        if let Some(handler) = &entry.on_unhover {
                            handler();
                        }
                    }
                }
            }

            // Find nodes that are newly hovered
            for new_id in &new_hovered {
                if !old_hovered.contains(new_id) {
                    hover_changed = true;
                    // Node is newly hovered - call hover handler
                    if let Some(entry) = state.entries.iter().find(|e| e.node_id == *new_id) {
                        if let Some(handler) = &entry.on_hover {
                            handler();
                        }
                    }
                }
            }

            state.hovered_nodes = new_hovered;

            if hover_changed {
                return true;
            }
        }

        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            // Scroll events could be handled here if needed
        }

        _ => {}
    }

    false
}

/// Clear hover state for all clickables.
///
/// Called when mouse leaves the terminal area.
pub fn clear_hover_state() {
    let mut state = get_click_state().write();
    for old_id in std::mem::take(&mut state.hovered_nodes) {
        if let Some(entry) = state.entries.iter().find(|e| e.node_id == old_id) {
            if let Some(handler) = &entry.on_unhover {
                handler();
            }
        }
    }
}

/// Check if any clickables are registered.
pub fn has_clickables() -> bool {
    !get_click_state().read().entries.is_empty()
}

/// Get the number of registered clickables.
#[cfg(test)]
pub fn clickable_count() -> usize {
    get_click_state().read().entries.len()
}

/// Helper to create a ClickEvent from a MouseEvent and HitTestResult.
fn make_click_event(event: &MouseEvent, hit: &HitTestResult, button: MouseButton) -> ClickEvent {
    ClickEvent {
        button,
        local_x: hit.local_x,
        local_y: hit.local_y,
        screen_x: event.x,
        screen_y: event.y,
        modifiers: ClickModifiers {
            shift: event.modifiers.shift,
            ctrl: event.modifiers.ctrl,
            alt: event.modifiers.alt,
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hooks::FocusContext;
    use crate::node::TextNode;
    use crate::terminal::KeyModifiers;
    use serial_test::serial;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn setup() {
        clear_clickables();
        get_click_state().write().pressed_node = None;
        get_click_state().write().hovered_nodes.clear();
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
                height: 1,
            },
            local_x: 5,
            local_y: 0,
            path: SmallVec::new(),
        }
    }

    #[test]
    #[serial]
    fn test_register_clickable() {
        setup();

        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Test"));
        register_clickable(id, &clickable);

        assert_eq!(clickable_count(), 1);
    }

    #[test]
    #[serial]
    fn test_clear_clickables() {
        setup();

        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Test"));
        register_clickable(id, &clickable);
        assert_eq!(clickable_count(), 1);

        clear_clickables();
        assert_eq!(clickable_count(), 0);
    }

    #[test]
    #[serial]
    fn test_dispatch_click() {
        setup();

        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = clicked.clone();

        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Test")).on_click(move |_| {
            clicked_clone.store(true, Ordering::SeqCst);
        });
        register_clickable(id, &clickable);

        let hit = make_hit_result(id);

        // Mouse down
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&down_event, Some(&hit));

        // Mouse up triggers the click
        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&up_event, Some(&hit));

        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_click_requires_same_target() {
        setup();

        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = clicked.clone();

        let id1 = NodeId::new();
        let id2 = NodeId::new();

        let clickable = Clickable::new(TextNode::new("Test")).on_click(move |_| {
            clicked_clone.store(true, Ordering::SeqCst);
        });
        register_clickable(id1, &clickable);

        let hit1 = make_hit_result(id1);
        let hit2 = make_hit_result(id2);

        // Mouse down on id1
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&down_event, Some(&hit1));

        // Mouse up on different target - should NOT trigger click
        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&up_event, Some(&hit2));

        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_hover_handlers() {
        setup();

        let hover_count = Arc::new(AtomicUsize::new(0));
        let unhover_count = Arc::new(AtomicUsize::new(0));

        let hover_clone = hover_count.clone();
        let unhover_clone = unhover_count.clone();

        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Test"))
            .on_hover(move || {
                hover_clone.fetch_add(1, Ordering::SeqCst);
            })
            .on_unhover(move || {
                unhover_clone.fetch_add(1, Ordering::SeqCst);
            });
        register_clickable(id, &clickable);

        let hit = make_hit_result(id);

        // Move into the clickable
        let move_event = make_mouse_event(MouseEventKind::Moved, None, 5, 0);
        let handled = dispatch_click_event(&move_event, Some(&hit));

        assert!(handled);
        assert_eq!(hover_count.load(Ordering::SeqCst), 1);
        assert_eq!(unhover_count.load(Ordering::SeqCst), 0);

        // Move again (should not trigger again)
        let handled = dispatch_click_event(&move_event, Some(&hit));
        assert!(!handled);
        assert_eq!(hover_count.load(Ordering::SeqCst), 1);

        // Move out (no hit)
        let handled = dispatch_click_event(&move_event, None);
        assert!(handled);
        assert_eq!(unhover_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[serial]
    fn test_disabled_clickable() {
        setup();

        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = clicked.clone();

        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Test"))
            .on_click(move |_| {
                clicked_clone.store(true, Ordering::SeqCst);
            })
            .disabled(true);
        register_clickable(id, &clickable);

        let hit = make_hit_result(id);

        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&down_event, Some(&hit));

        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&up_event, Some(&hit));

        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_mouse_down_handler() {
        setup();

        let down_count = Arc::new(AtomicUsize::new(0));
        let down_clone = down_count.clone();

        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Test")).on_mouse_down(move |_| {
            down_clone.fetch_add(1, Ordering::SeqCst);
        });
        register_clickable(id, &clickable);

        let hit = make_hit_result(id);

        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 0);
        let handled = dispatch_click_event(&down_event, Some(&hit));

        assert!(handled);
        assert_eq!(down_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[serial]
    fn test_has_clickables() {
        setup();
        assert!(!has_clickables());

        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Test"));
        register_clickable(id, &clickable);

        assert!(has_clickables());
    }

    #[test]
    #[serial]
    fn test_focus_on_click() {
        setup();

        // Create a fresh focus context for this test
        let ctx = std::sync::Arc::new(std::sync::RwLock::new(FocusContext::new()));

        // Create a focus handle manually for testing
        let node_id = NodeId::new();
        let focus_handle = FocusHandle::new(node_id, ctx.clone());

        // Create a clickable with focus-on-click enabled
        let id = NodeId::new();
        let clickable =
            Clickable::new(TextNode::new("Focus me")).focus_on_click(focus_handle.clone());

        // Verify the focus handle was set
        assert!(
            clickable.focus_handle().is_some(),
            "focus_handle should be set"
        );

        register_clickable(id, &clickable);

        let hit = make_hit_result(id);

        // Verify not focused initially
        assert!(!focus_handle.is_focused());

        // Mouse down
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&down_event, Some(&hit));

        // Still not focused after down
        assert!(!focus_handle.is_focused());

        // Mouse up triggers focus
        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 5, 0);
        let handled = dispatch_click_event(&up_event, Some(&hit));

        // Now should be focused
        assert!(focus_handle.is_focused(), "should be focused after click");
        assert!(handled, "event should be handled");
    }

    #[test]
    #[serial]
    fn test_focus_on_click_with_handler() {
        setup();

        // Create a fresh focus context for this test
        let ctx = std::sync::Arc::new(std::sync::RwLock::new(FocusContext::new()));

        // Create a focus handle manually for testing
        let node_id = NodeId::new();
        let focus_handle = FocusHandle::new(node_id, ctx.clone());

        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = clicked.clone();

        // Create a clickable with both focus-on-click and click handler
        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Focus me"))
            .focus_on_click(focus_handle.clone())
            .on_click(move |_| {
                clicked_clone.store(true, Ordering::SeqCst);
            });
        register_clickable(id, &clickable);

        let hit = make_hit_result(id);

        // Mouse down + up
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&down_event, Some(&hit));
        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&up_event, Some(&hit));

        // Both focus and click handler should have been triggered
        assert!(focus_handle.is_focused());
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    #[serial]
    fn test_focus_on_click_disabled() {
        setup();

        // Create a fresh focus context for this test
        let ctx = std::sync::Arc::new(std::sync::RwLock::new(FocusContext::new()));

        // Create a focus handle manually for testing
        let node_id = NodeId::new();
        let focus_handle = FocusHandle::new(node_id, ctx.clone());

        // Create a disabled clickable with focus-on-click
        let id = NodeId::new();
        let clickable = Clickable::new(TextNode::new("Focus me"))
            .focus_on_click(focus_handle.clone())
            .disabled(true);
        register_clickable(id, &clickable);

        let hit = make_hit_result(id);

        // Mouse down + up
        let down_event = make_mouse_event(MouseEventKind::Down, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&down_event, Some(&hit));
        let up_event = make_mouse_event(MouseEventKind::Up, Some(MouseButton::Left), 5, 0);
        dispatch_click_event(&up_event, Some(&hit));

        // Should NOT be focused because disabled
        assert!(!focus_handle.is_focused());
    }
}
