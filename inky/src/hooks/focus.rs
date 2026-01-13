//! Focus management hooks.
//!
//! This module provides focus management for inky applications.
//! Components can register themselves as focusable and respond
//! to focus changes.
//!
//! # Example
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! let handle = use_focus();
//! handle.focus(); // Focus this component
//!
//! if handle.is_focused() {
//!     // Render with focus styling
//! }
//! ```
//!
//! # String-based Focus IDs
//!
//! For easier programmatic focus control, you can use string IDs:
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! // Create a focus handle with a string ID
//! let handle = use_focus_with_id("chat-input");
//!
//! // Later, focus by ID from anywhere:
//! set_focus("chat-input");
//!
//! // Query the currently focused ID:
//! if let Some(id) = focused_id() {
//!     println!("Focused: {}", id);
//! }
//! ```
//!
//! # Focus Traps (Modal Focus)
//!
//! Focus traps confine Tab navigation to a subset of focusable elements.
//! This is essential for modals where focus should not escape.
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! // Push a focus trap when showing a modal
//! let trap_id = push_focus_trap(&["modal-confirm", "modal-cancel"]);
//!
//! // Tab/Shift+Tab now only cycles between modal elements
//!
//! // Pop the trap when closing the modal
//! pop_focus_trap(trap_id);
//! ```
//!
//! # Focus Groups
//!
//! Focus groups allow organizing focusables into logical regions:
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! // Create focus handles in a group
//! let handle = use_focus_in_group("sidebar");
//!
//! // Focus a specific group (first element in group)
//! focus_group("sidebar");
//!
//! // Navigate within the current group only
//! focus_next_in_group();
//! focus_prev_in_group();
//! ```

use crate::node::NodeId;
use indexmap::IndexSet;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

/// Unique identifier for a focus trap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FocusTrapId(u64);

impl FocusTrapId {
    /// Generate a new unique trap ID.
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// A focus trap that confines Tab navigation to specific elements.
#[derive(Debug, Clone)]
pub struct FocusTrap {
    /// Unique ID for this trap.
    pub id: FocusTrapId,
    /// The focus IDs that are part of this trap.
    pub trapped_ids: Vec<String>,
    /// The NodeIds resolved from trapped_ids (cached).
    resolved_nodes: IndexSet<NodeId>,
    /// Focus to restore when trap is popped.
    previous_focus: Option<NodeId>,
}

impl FocusTrap {
    /// Create a new focus trap with the given focus IDs.
    fn new(trapped_ids: &[&str], previous_focus: Option<NodeId>) -> Self {
        Self {
            id: FocusTrapId::new(),
            trapped_ids: trapped_ids.iter().map(|s| (*s).to_string()).collect(),
            resolved_nodes: IndexSet::new(),
            previous_focus,
        }
    }

    /// Get the trapped nodes as a slice for navigation.
    fn nodes(&self) -> &IndexSet<NodeId> {
        &self.resolved_nodes
    }
}

/// Focus context managing focus state.
/// Uses IndexSet for O(1) contains, O(1) index lookup, and preserved insertion order.
#[derive(Debug, Clone, Default)]
pub struct FocusContext {
    /// Currently focused node.
    focused: Option<NodeId>,
    /// Ordered set of focusable nodes with O(1) contains and O(1) get_index_of.
    focusable: IndexSet<NodeId>,
    /// Map from string focus ID to NodeId for programmatic focus control.
    id_to_node: HashMap<String, NodeId>,
    /// Reverse map from NodeId to string focus ID.
    node_to_id: HashMap<NodeId, String>,
    /// Stack of focus traps (last is active).
    focus_traps: Vec<FocusTrap>,
    /// Map from group name to set of NodeIds in that group.
    groups: HashMap<String, IndexSet<NodeId>>,
    /// Reverse map from NodeId to group name.
    node_to_group: HashMap<NodeId, String>,
}

impl FocusContext {
    /// Create a new focus context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a focusable node.
    /// IndexSet::insert returns false if already present (idempotent).
    pub fn register(&mut self, id: NodeId) {
        self.focusable.insert(id);
    }

    /// Register a focusable node with a string ID for programmatic access.
    pub fn register_with_id(&mut self, node_id: NodeId, focus_id: impl Into<String>) {
        let focus_id = focus_id.into();
        self.focusable.insert(node_id);
        self.id_to_node.insert(focus_id.clone(), node_id);
        self.node_to_id.insert(node_id, focus_id);
    }

    /// Unregister a focusable node.
    /// IndexSet::shift_remove maintains order of remaining elements.
    pub fn unregister(&mut self, id: NodeId) {
        self.focusable.shift_remove(&id);
        if self.focused == Some(id) {
            self.focused = None;
        }
        // Clean up string ID mappings if present
        if let Some(focus_id) = self.node_to_id.remove(&id) {
            self.id_to_node.remove(&focus_id);
        }
    }

    /// Focus a specific node.
    /// O(1) contains check via IndexSet.
    pub fn focus(&mut self, id: NodeId) {
        if self.focusable.contains(&id) {
            self.focused = Some(id);
        }
    }

    /// Focus a node by its string ID.
    /// Returns true if the focus was set, false if the ID was not found.
    pub fn focus_by_id(&mut self, focus_id: &str) -> bool {
        if let Some(&node_id) = self.id_to_node.get(focus_id) {
            self.focused = Some(node_id);
            true
        } else {
            false
        }
    }

    /// Get the currently focused node.
    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    /// Get the string ID of the currently focused node, if it has one.
    pub fn focused_id(&self) -> Option<&str> {
        self.focused
            .and_then(|node_id| self.node_to_id.get(&node_id))
            .map(String::as_str)
    }

    /// Check if a node is focused.
    pub fn is_focused(&self, id: NodeId) -> bool {
        self.focused == Some(id)
    }

    /// Get the string ID for a node, if registered.
    pub fn get_id(&self, node_id: NodeId) -> Option<&str> {
        self.node_to_id.get(&node_id).map(String::as_str)
    }

    /// Get the NodeId for a string ID, if registered.
    pub fn get_node_id(&self, focus_id: &str) -> Option<NodeId> {
        self.id_to_node.get(focus_id).copied()
    }

    /// Focus the next node in the list.
    /// Uses IndexSet::get_index_of for O(1) position lookup.
    pub fn focus_next(&mut self) {
        if self.focusable.is_empty() {
            return;
        }

        let next = match self.focused {
            None => 0,
            Some(id) => {
                // O(1) index lookup with IndexSet
                match self.focusable.get_index_of(&id) {
                    Some(p) => (p + 1) % self.focusable.len(),
                    None => 0,
                }
            }
        };

        self.focused = self.focusable.get_index(next).copied();
    }

    /// Focus the previous node in the list.
    /// Uses IndexSet::get_index_of for O(1) position lookup.
    pub fn focus_prev(&mut self) {
        if self.focusable.is_empty() {
            return;
        }

        let prev = match self.focused {
            None => self.focusable.len() - 1,
            Some(id) => {
                // O(1) index lookup with IndexSet
                match self.focusable.get_index_of(&id) {
                    Some(0) => self.focusable.len() - 1,
                    Some(p) => p - 1,
                    None => self.focusable.len() - 1,
                }
            }
        };

        self.focused = self.focusable.get_index(prev).copied();
    }

    /// Clear focus.
    pub fn blur(&mut self) {
        self.focused = None;
    }

    /// Get the number of registered focusables (for debugging).
    #[cfg(test)]
    pub fn focusable_count(&self) -> usize {
        self.focusable.len()
    }

    // ==================== Focus Trap Methods ====================

    /// Push a new focus trap with the given focus IDs.
    ///
    /// Focus navigation (Tab/Shift+Tab) will be confined to these elements
    /// until the trap is popped. The first element in the trap will be focused.
    ///
    /// Returns the trap ID which must be used to pop the trap.
    pub fn push_trap(&mut self, focus_ids: &[&str]) -> FocusTrapId {
        let previous_focus = self.focused;
        let mut trap = FocusTrap::new(focus_ids, previous_focus);

        // Resolve focus IDs to NodeIds
        for focus_id in focus_ids {
            if let Some(&node_id) = self.id_to_node.get(*focus_id) {
                trap.resolved_nodes.insert(node_id);
            }
        }

        // Focus the first element in the trap
        if let Some(&first_node) = trap.resolved_nodes.get_index(0) {
            self.focused = Some(first_node);
        }

        let id = trap.id;
        self.focus_traps.push(trap);
        id
    }

    /// Pop a focus trap by its ID.
    ///
    /// Returns true if the trap was found and removed, false otherwise.
    /// Focus is restored to the element that was focused before the trap.
    pub fn pop_trap(&mut self, trap_id: FocusTrapId) -> bool {
        if let Some(pos) = self.focus_traps.iter().position(|t| t.id == trap_id) {
            let trap = self.focus_traps.remove(pos);
            // Restore previous focus
            self.focused = trap.previous_focus;
            true
        } else {
            false
        }
    }

    /// Check if there is an active focus trap.
    pub fn has_active_trap(&self) -> bool {
        !self.focus_traps.is_empty()
    }

    /// Get the currently active trap, if any.
    pub fn active_trap(&self) -> Option<&FocusTrap> {
        self.focus_traps.last()
    }

    /// Focus the next element, respecting active focus trap.
    pub fn focus_next_trapped(&mut self) {
        if let Some(trap) = self.focus_traps.last() {
            let nodes = trap.nodes();
            if nodes.is_empty() {
                return;
            }

            let next = match self.focused {
                None => 0,
                Some(id) => match nodes.get_index_of(&id) {
                    Some(p) => (p + 1) % nodes.len(),
                    None => 0,
                },
            };

            self.focused = nodes.get_index(next).copied();
        } else {
            // No trap active, use normal navigation
            self.focus_next();
        }
    }

    /// Focus the previous element, respecting active focus trap.
    pub fn focus_prev_trapped(&mut self) {
        if let Some(trap) = self.focus_traps.last() {
            let nodes = trap.nodes();
            if nodes.is_empty() {
                return;
            }

            let prev = match self.focused {
                None => nodes.len() - 1,
                Some(id) => match nodes.get_index_of(&id) {
                    Some(0) => nodes.len() - 1,
                    Some(p) => p - 1,
                    None => nodes.len() - 1,
                },
            };

            self.focused = nodes.get_index(prev).copied();
        } else {
            // No trap active, use normal navigation
            self.focus_prev();
        }
    }

    // ==================== Focus Group Methods ====================

    /// Register a focusable node with a group.
    pub fn register_with_group(
        &mut self,
        node_id: NodeId,
        focus_id: impl Into<String>,
        group: impl Into<String>,
    ) {
        let focus_id = focus_id.into();
        let group = group.into();

        // Standard registration
        self.focusable.insert(node_id);
        self.id_to_node.insert(focus_id.clone(), node_id);
        self.node_to_id.insert(node_id, focus_id);

        // Group registration
        self.groups
            .entry(group.clone())
            .or_default()
            .insert(node_id);
        self.node_to_group.insert(node_id, group);
    }

    /// Unregister a node, cleaning up group membership.
    pub fn unregister_with_group(&mut self, id: NodeId) {
        // Remove from group
        if let Some(group_name) = self.node_to_group.remove(&id) {
            if let Some(group) = self.groups.get_mut(&group_name) {
                group.shift_remove(&id);
                // Remove empty groups
                if group.is_empty() {
                    self.groups.remove(&group_name);
                }
            }
        }

        // Standard unregistration
        self.unregister(id);
    }

    /// Get the group name for a node, if it belongs to one.
    pub fn get_group(&self, node_id: NodeId) -> Option<&str> {
        self.node_to_group.get(&node_id).map(String::as_str)
    }

    /// Get all nodes in a group.
    pub fn get_group_nodes(&self, group: &str) -> Option<&IndexSet<NodeId>> {
        self.groups.get(group)
    }

    /// Focus the first element in a group.
    ///
    /// Returns true if the group exists and has elements.
    pub fn focus_group(&mut self, group: &str) -> bool {
        if let Some(nodes) = self.groups.get(group) {
            if let Some(&first) = nodes.get_index(0) {
                self.focused = Some(first);
                return true;
            }
        }
        false
    }

    /// Focus the next element in the current element's group.
    ///
    /// If no element is focused or the focused element has no group,
    /// this behaves like focus_next().
    pub fn focus_next_in_group(&mut self) {
        let group = self.focused.and_then(|id| self.node_to_group.get(&id));

        if let Some(group_name) = group {
            if let Some(nodes) = self.groups.get(group_name) {
                if nodes.is_empty() {
                    return;
                }

                let next = match self.focused {
                    None => 0,
                    Some(id) => match nodes.get_index_of(&id) {
                        Some(p) => (p + 1) % nodes.len(),
                        None => 0,
                    },
                };

                self.focused = nodes.get_index(next).copied();
                return;
            }
        }

        // Fall back to normal navigation
        self.focus_next();
    }

    /// Focus the previous element in the current element's group.
    ///
    /// If no element is focused or the focused element has no group,
    /// this behaves like focus_prev().
    pub fn focus_prev_in_group(&mut self) {
        let group = self.focused.and_then(|id| self.node_to_group.get(&id));

        if let Some(group_name) = group {
            if let Some(nodes) = self.groups.get(group_name) {
                if nodes.is_empty() {
                    return;
                }

                let prev = match self.focused {
                    None => nodes.len() - 1,
                    Some(id) => match nodes.get_index_of(&id) {
                        Some(0) => nodes.len() - 1,
                        Some(p) => p - 1,
                        None => nodes.len() - 1,
                    },
                };

                self.focused = nodes.get_index(prev).copied();
                return;
            }
        }

        // Fall back to normal navigation
        self.focus_prev();
    }

    /// Get all group names.
    pub fn group_names(&self) -> impl Iterator<Item = &str> {
        self.groups.keys().map(String::as_str)
    }
}

/// Callback type for focus/blur events.
pub type FocusCallback = Arc<dyn Fn() + Send + Sync>;

/// Handle for a focusable component.
#[derive(Clone)]
pub struct FocusHandle {
    id: NodeId,
    ctx: Arc<RwLock<FocusContext>>,
    /// Optional string ID for programmatic focus control.
    focus_id: Option<String>,
    /// Optional group name for group-based navigation.
    group: Option<String>,
    /// Callback invoked when this component gains focus.
    on_focus: Option<FocusCallback>,
    /// Callback invoked when this component loses focus.
    on_blur: Option<FocusCallback>,
}

impl std::fmt::Debug for FocusHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FocusHandle")
            .field("id", &self.id)
            .field("focus_id", &self.focus_id)
            .field("group", &self.group)
            .field("on_focus", &self.on_focus.is_some())
            .field("on_blur", &self.on_blur.is_some())
            .finish()
    }
}

impl FocusHandle {
    /// Create a new focus handle.
    pub fn new(id: NodeId, ctx: Arc<RwLock<FocusContext>>) -> Self {
        ctx.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .register(id);
        Self {
            id,
            ctx,
            focus_id: None,
            group: None,
            on_focus: None,
            on_blur: None,
        }
    }

    /// Create a new focus handle with a string ID for programmatic access.
    pub fn with_id(
        id: NodeId,
        ctx: Arc<RwLock<FocusContext>>,
        focus_id: impl Into<String>,
    ) -> Self {
        let focus_id = focus_id.into();
        ctx.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .register_with_id(id, focus_id.clone());
        Self {
            id,
            ctx,
            focus_id: Some(focus_id),
            group: None,
            on_focus: None,
            on_blur: None,
        }
    }

    /// Create a new focus handle with a string ID and a group.
    ///
    /// Group-based focus allows navigation within a subset of focusables.
    pub fn with_group(
        id: NodeId,
        ctx: Arc<RwLock<FocusContext>>,
        focus_id: impl Into<String>,
        group: impl Into<String>,
    ) -> Self {
        let focus_id = focus_id.into();
        let group = group.into();
        ctx.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .register_with_group(id, focus_id.clone(), group.clone());
        Self {
            id,
            ctx,
            focus_id: Some(focus_id),
            group: Some(group),
            on_focus: None,
            on_blur: None,
        }
    }

    /// Set a callback to be invoked when this component gains focus.
    pub fn on_focus<F: Fn() + Send + Sync + 'static>(mut self, callback: F) -> Self {
        self.on_focus = Some(Arc::new(callback));
        self
    }

    /// Set a callback to be invoked when this component loses focus.
    pub fn on_blur<F: Fn() + Send + Sync + 'static>(mut self, callback: F) -> Self {
        self.on_blur = Some(Arc::new(callback));
        self
    }

    /// Check if this component is focused.
    pub fn is_focused(&self) -> bool {
        self.ctx
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_focused(self.id)
    }

    /// Focus this component.
    pub fn focus(&self) {
        self.ctx
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .focus(self.id);
        // Invoke on_focus callback if set
        if let Some(ref callback) = self.on_focus {
            callback();
        }
    }

    /// Remove focus from this component.
    pub fn blur(&self) {
        let was_focused = {
            let mut ctx = self
                .ctx
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let was = ctx.is_focused(self.id);
            if was {
                ctx.blur();
            }
            was
        };
        // Invoke on_blur callback if was focused
        if was_focused {
            if let Some(ref callback) = self.on_blur {
                callback();
            }
        }
    }

    /// Get the node ID.
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Get the string focus ID, if set.
    pub fn focus_id(&self) -> Option<&str> {
        self.focus_id.as_deref()
    }

    /// Get the group name, if set.
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// Get the underlying context Arc (for debugging).
    #[cfg(test)]
    pub fn context(&self) -> &Arc<RwLock<FocusContext>> {
        &self.ctx
    }
}

impl Drop for FocusHandle {
    fn drop(&mut self) {
        let mut ctx = self
            .ctx
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Use group-aware unregister if this handle has a group
        if self.group.is_some() {
            ctx.unregister_with_group(self.id);
        } else {
            ctx.unregister(self.id);
        }
    }
}

// Global focus context using OnceLock for initialization-free access.
// This eliminates write locks on every get_focus_context() call after first initialization.
static FOCUS_CTX: OnceLock<Arc<RwLock<FocusContext>>> = OnceLock::new();

/// Get a focus handle for a component.
pub fn use_focus() -> FocusHandle {
    let ctx = get_focus_context();
    FocusHandle::new(NodeId::new(), ctx)
}

/// Get a focus handle with a string ID for programmatic focus control.
///
/// This allows focusing the component by ID from anywhere in the application:
///
/// ```ignore
/// // In a component:
/// let handle = use_focus_with_id("chat-input");
///
/// // Later, from anywhere:
/// set_focus("chat-input");
/// ```
pub fn use_focus_with_id(focus_id: impl Into<String>) -> FocusHandle {
    let ctx = get_focus_context();
    FocusHandle::with_id(NodeId::new(), ctx, focus_id)
}

/// Reset the global focus context (useful for tests).
/// Note: With OnceLock, we can only clear the inner context, not replace the static.
#[doc(hidden)]
#[allow(dead_code)]
pub fn reset_focus_context() {
    if let Some(ctx) = FOCUS_CTX.get() {
        let mut inner = ctx.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        *inner = FocusContext::new();
    }
}

/// Get the global focus context.
/// Returns the shared FocusContext used by all focus hooks.
/// Uses OnceLock for lock-free access after initialization.
pub fn get_focus_context() -> Arc<RwLock<FocusContext>> {
    FOCUS_CTX
        .get_or_init(|| Arc::new(RwLock::new(FocusContext::new())))
        .clone()
}

/// Focus the next registered component.
/// Called by the App in response to Tab key press.
pub fn focus_next() {
    let ctx = get_focus_context();
    ctx.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focus_next();
}

/// Focus the previous registered component.
/// Called by the App in response to Shift+Tab key press.
pub fn focus_prev() {
    let ctx = get_focus_context();
    ctx.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focus_prev();
}

/// Focus a component by its string ID.
///
/// Returns `true` if the focus was set, `false` if the ID was not found.
///
/// # Example
///
/// ```ignore
/// use inky::hooks::focus::{set_focus, use_focus_with_id};
///
/// // Register a focusable with an ID
/// let handle = use_focus_with_id("my-input");
///
/// // Later, focus it by ID
/// if set_focus("my-input") {
///     println!("Focused my-input");
/// }
/// ```
pub fn set_focus(focus_id: &str) -> bool {
    let ctx = get_focus_context();
    let result = ctx
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focus_by_id(focus_id);
    result
}

/// Get the string ID of the currently focused component, if it has one.
///
/// Returns `None` if no component is focused, or if the focused component
/// was not registered with a string ID.
///
/// # Example
///
/// ```ignore
/// use inky::hooks::focus::{focused_id, set_focus};
///
/// set_focus("my-input");
/// assert_eq!(focused_id(), Some("my-input".to_string()));
/// ```
pub fn focused_id() -> Option<String> {
    let ctx = get_focus_context();
    let result = ctx
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focused_id()
        .map(String::from);
    result
}

/// Clear focus from all components.
pub fn blur_all() {
    let ctx = get_focus_context();
    ctx.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .blur();
}

// ==================== Focus Trap Public API ====================

/// Push a focus trap that confines Tab navigation to specific elements.
///
/// When a trap is active, `focus_next()` and `focus_prev()` will only
/// cycle between the trapped elements. This is essential for modals
/// where focus should not escape.
///
/// Returns a `FocusTrapId` that must be used to pop the trap.
///
/// # Example
///
/// ```ignore
/// use inky::hooks::focus::{push_focus_trap, pop_focus_trap, use_focus_with_id};
///
/// // Create focusable elements for the modal
/// let _confirm = use_focus_with_id("modal-confirm");
/// let _cancel = use_focus_with_id("modal-cancel");
///
/// // Push trap - Tab now only cycles between these two
/// let trap_id = push_focus_trap(&["modal-confirm", "modal-cancel"]);
///
/// // ... modal is active ...
///
/// // Pop trap when modal closes
/// pop_focus_trap(trap_id);
/// ```
pub fn push_focus_trap(focus_ids: &[&str]) -> FocusTrapId {
    let ctx = get_focus_context();
    let result = ctx
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push_trap(focus_ids);
    result
}

/// Pop a focus trap by its ID.
///
/// Returns `true` if the trap was found and removed, `false` otherwise.
/// Focus is restored to the element that was focused before the trap was pushed.
pub fn pop_focus_trap(trap_id: FocusTrapId) -> bool {
    let ctx = get_focus_context();
    let result = ctx
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .pop_trap(trap_id);
    result
}

/// Check if there is an active focus trap.
pub fn has_focus_trap() -> bool {
    let ctx = get_focus_context();
    let result = ctx
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .has_active_trap();
    result
}

/// Focus the next element, respecting any active focus trap.
///
/// If a trap is active, navigation is confined to trapped elements.
/// Otherwise, behaves like `focus_next()`.
pub fn focus_next_trapped() {
    let ctx = get_focus_context();
    ctx.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focus_next_trapped();
}

/// Focus the previous element, respecting any active focus trap.
///
/// If a trap is active, navigation is confined to trapped elements.
/// Otherwise, behaves like `focus_prev()`.
pub fn focus_prev_trapped() {
    let ctx = get_focus_context();
    ctx.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focus_prev_trapped();
}

// ==================== Focus Group Public API ====================

/// Get a focus handle with a string ID and group.
///
/// Group-based focus allows navigation within a subset of focusables.
///
/// # Example
///
/// ```ignore
/// use inky::hooks::focus::use_focus_in_group;
///
/// // Create focusables in the "sidebar" group
/// let _item1 = use_focus_in_group("sidebar-item-1", "sidebar");
/// let _item2 = use_focus_in_group("sidebar-item-2", "sidebar");
/// let _item3 = use_focus_in_group("sidebar-item-3", "sidebar");
///
/// // Use focus_next_in_group() to cycle within the group
/// ```
pub fn use_focus_in_group(focus_id: impl Into<String>, group: impl Into<String>) -> FocusHandle {
    let ctx = get_focus_context();
    FocusHandle::with_group(NodeId::new(), ctx, focus_id, group)
}

/// Focus the first element in a named group.
///
/// Returns `true` if the group exists and has elements, `false` otherwise.
pub fn focus_group(group: &str) -> bool {
    let ctx = get_focus_context();
    let result = ctx
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focus_group(group);
    result
}

/// Focus the next element in the current element's group.
///
/// If no element is focused or the focused element has no group,
/// this behaves like `focus_next()`.
pub fn focus_next_in_group() {
    let ctx = get_focus_context();
    ctx.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focus_next_in_group();
}

/// Focus the previous element in the current element's group.
///
/// If no element is focused or the focused element has no group,
/// this behaves like `focus_prev()`.
pub fn focus_prev_in_group() {
    let ctx = get_focus_context();
    ctx.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .focus_prev_in_group();
}

/// Get the group name of the currently focused element, if it has one.
pub fn focused_group() -> Option<String> {
    let ctx = get_focus_context();
    let guard = ctx.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .focused()
        .and_then(|id| guard.get_group(id))
        .map(String::from)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn setup() -> Arc<RwLock<FocusContext>> {
        Arc::new(RwLock::new(FocusContext::new()))
    }

    #[test]
    fn test_focus_context_new() {
        let ctx = FocusContext::new();
        assert!(ctx.focused().is_none());
    }

    #[test]
    fn test_focus_context_register() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        ctx.register(id1);
        ctx.register(id2);

        // Verify both are registered (by checking focus navigation works)
        ctx.focus_next();
        assert!(ctx.focused().is_some());
    }

    #[test]
    fn test_focus_context_register_idempotent() {
        let mut ctx = FocusContext::new();
        let id = NodeId::new();

        ctx.register(id);
        ctx.register(id); // Should not add duplicate

        // Focus navigation should only cycle through one item
        ctx.focus_next();
        let first = ctx.focused();
        ctx.focus_next();
        let second = ctx.focused();

        assert_eq!(first, second);
    }

    #[test]
    fn test_focus_context_unregister() {
        let mut ctx = FocusContext::new();
        let id = NodeId::new();

        ctx.register(id);
        ctx.focus(id);
        assert!(ctx.is_focused(id));

        ctx.unregister(id);
        assert!(!ctx.is_focused(id));
        assert!(ctx.focused().is_none());
    }

    #[test]
    fn test_focus_context_focus() {
        let mut ctx = FocusContext::new();
        let id = NodeId::new();

        // Cannot focus unregistered node
        ctx.focus(id);
        assert!(ctx.focused().is_none());

        // Can focus registered node
        ctx.register(id);
        ctx.focus(id);
        assert_eq!(ctx.focused(), Some(id));
    }

    #[test]
    fn test_focus_context_is_focused() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        ctx.register(id1);
        ctx.register(id2);

        assert!(!ctx.is_focused(id1));
        assert!(!ctx.is_focused(id2));

        ctx.focus(id1);
        assert!(ctx.is_focused(id1));
        assert!(!ctx.is_focused(id2));
    }

    #[test]
    fn test_focus_context_focus_next() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        ctx.register(id1);
        ctx.register(id2);
        ctx.register(id3);

        // No focus -> first item
        ctx.focus_next();
        assert_eq!(ctx.focused(), Some(id1));

        // Navigate forward
        ctx.focus_next();
        assert_eq!(ctx.focused(), Some(id2));

        ctx.focus_next();
        assert_eq!(ctx.focused(), Some(id3));

        // Wrap around
        ctx.focus_next();
        assert_eq!(ctx.focused(), Some(id1));
    }

    #[test]
    fn test_focus_context_focus_prev() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        ctx.register(id1);
        ctx.register(id2);
        ctx.register(id3);

        // No focus -> last item
        ctx.focus_prev();
        assert_eq!(ctx.focused(), Some(id3));

        // Navigate backward
        ctx.focus_prev();
        assert_eq!(ctx.focused(), Some(id2));

        ctx.focus_prev();
        assert_eq!(ctx.focused(), Some(id1));

        // Wrap around
        ctx.focus_prev();
        assert_eq!(ctx.focused(), Some(id3));
    }

    #[test]
    fn test_focus_context_focus_nav_empty() {
        let mut ctx = FocusContext::new();

        // Should not panic on empty list
        ctx.focus_next();
        ctx.focus_prev();

        assert!(ctx.focused().is_none());
    }

    #[test]
    fn test_focus_context_blur() {
        let mut ctx = FocusContext::new();
        let id = NodeId::new();

        ctx.register(id);
        ctx.focus(id);
        assert!(ctx.is_focused(id));

        ctx.blur();
        assert!(ctx.focused().is_none());
    }

    #[test]
    fn test_focus_handle_new() {
        let ctx = setup();
        let id = NodeId::new();
        let handle = FocusHandle::new(id, ctx.clone());

        assert_eq!(handle.id(), id);
        assert!(!handle.is_focused());
    }

    #[test]
    fn test_focus_handle_focus() {
        let ctx = setup();
        let id = NodeId::new();
        let handle = FocusHandle::new(id, ctx.clone());

        handle.focus();
        assert!(handle.is_focused());
    }

    #[test]
    fn test_focus_handle_blur() {
        let ctx = setup();
        let id = NodeId::new();
        let handle = FocusHandle::new(id, ctx.clone());

        handle.focus();
        assert!(handle.is_focused());

        handle.blur();
        assert!(!handle.is_focused());
    }

    #[test]
    fn test_focus_handle_drop_unregisters() {
        let ctx = setup();
        let id = NodeId::new();

        {
            let handle = FocusHandle::new(id, ctx.clone());
            handle.focus();
            assert!(ctx.read().unwrap().is_focused(id));
        }
        // Handle dropped

        // Should be unregistered and unfocused
        assert!(!ctx.read().unwrap().is_focused(id));
    }

    #[test]
    fn test_multiple_focus_handles() {
        let ctx = setup();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        let handle1 = FocusHandle::new(id1, ctx.clone());
        let handle2 = FocusHandle::new(id2, ctx.clone());

        assert!(!handle1.is_focused());
        assert!(!handle2.is_focused());

        handle1.focus();
        assert!(handle1.is_focused());
        assert!(!handle2.is_focused());

        handle2.focus();
        assert!(!handle1.is_focused());
        assert!(handle2.is_focused());
    }

    #[test]
    fn test_get_focus_context() {
        // This tests the basic contract that get_focus_context returns a context
        // Note: Due to global state, this test is simplified to avoid race conditions
        let ctx = get_focus_context();
        // Should be able to access the context
        let _guard = ctx.read().unwrap();
    }

    #[test]
    fn test_focus_next_and_prev_with_local_context() {
        // Use local context to avoid global state issues
        let ctx = Arc::new(RwLock::new(FocusContext::new()));
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        let _handle1 = FocusHandle::new(id1, ctx.clone());
        let _handle2 = FocusHandle::new(id2, ctx.clone());
        let _handle3 = FocusHandle::new(id3, ctx.clone());

        // Focus next
        ctx.write().unwrap().focus_next();
        assert!(ctx.read().unwrap().is_focused(id1));

        ctx.write().unwrap().focus_next();
        assert!(ctx.read().unwrap().is_focused(id2));

        ctx.write().unwrap().focus_next();
        assert!(ctx.read().unwrap().is_focused(id3));

        // Wrap around
        ctx.write().unwrap().focus_next();
        assert!(ctx.read().unwrap().is_focused(id1));

        // Focus prev (from first, wraps to last)
        ctx.write().unwrap().focus_prev();
        assert!(ctx.read().unwrap().is_focused(id3));

        ctx.write().unwrap().focus_prev();
        assert!(ctx.read().unwrap().is_focused(id2));

        ctx.write().unwrap().focus_prev();
        assert!(ctx.read().unwrap().is_focused(id1));
    }

    // === String ID Tests ===

    #[test]
    fn test_focus_context_register_with_id() {
        let mut ctx = FocusContext::new();
        let node_id = NodeId::new();

        ctx.register_with_id(node_id, "my-input");

        // Should be able to focus by ID
        assert!(ctx.focus_by_id("my-input"));
        assert_eq!(ctx.focused(), Some(node_id));
        assert_eq!(ctx.focused_id(), Some("my-input"));
    }

    #[test]
    fn test_focus_context_focus_by_id_not_found() {
        let mut ctx = FocusContext::new();

        // Non-existent ID should return false
        assert!(!ctx.focus_by_id("non-existent"));
        assert!(ctx.focused().is_none());
    }

    #[test]
    fn test_focus_context_unregister_cleans_up_id() {
        let mut ctx = FocusContext::new();
        let node_id = NodeId::new();

        ctx.register_with_id(node_id, "my-input");
        assert!(ctx.focus_by_id("my-input"));

        ctx.unregister(node_id);

        // ID should no longer be registered
        assert!(!ctx.focus_by_id("my-input"));
        assert!(ctx.focused().is_none());
        assert!(ctx.get_node_id("my-input").is_none());
    }

    #[test]
    fn test_focus_context_get_id_and_get_node_id() {
        let mut ctx = FocusContext::new();
        let node_id = NodeId::new();

        ctx.register_with_id(node_id, "chat-input");

        assert_eq!(ctx.get_id(node_id), Some("chat-input"));
        assert_eq!(ctx.get_node_id("chat-input"), Some(node_id));

        // Non-existent lookups
        let other_id = NodeId::new();
        assert!(ctx.get_id(other_id).is_none());
        assert!(ctx.get_node_id("non-existent").is_none());
    }

    #[test]
    fn test_focus_handle_with_id() {
        let ctx = setup();
        let node_id = NodeId::new();

        let handle = FocusHandle::with_id(node_id, ctx.clone(), "test-input");

        assert_eq!(handle.focus_id(), Some("test-input"));
        assert_eq!(handle.id(), node_id);

        // Focus by ID should work
        assert!(ctx.write().unwrap().focus_by_id("test-input"));
        assert!(handle.is_focused());
    }

    #[test]
    fn test_focus_handle_with_id_drop_cleans_up() {
        let ctx = setup();
        let node_id = NodeId::new();

        {
            let _handle = FocusHandle::with_id(node_id, ctx.clone(), "temp-input");
            assert!(ctx.read().unwrap().get_node_id("temp-input").is_some());
        }
        // Handle dropped

        // ID mapping should be cleaned up
        assert!(ctx.read().unwrap().get_node_id("temp-input").is_none());
    }

    #[test]
    fn test_focus_handle_callbacks() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let ctx = setup();
        let node_id = NodeId::new();

        let focus_called = Arc::new(AtomicBool::new(false));
        let blur_called = Arc::new(AtomicBool::new(false));

        let focus_flag = focus_called.clone();
        let blur_flag = blur_called.clone();

        let handle = FocusHandle::new(node_id, ctx.clone())
            .on_focus(move || {
                focus_flag.store(true, Ordering::SeqCst);
            })
            .on_blur(move || {
                blur_flag.store(true, Ordering::SeqCst);
            });

        // Focus should trigger on_focus callback
        handle.focus();
        assert!(focus_called.load(Ordering::SeqCst));
        assert!(!blur_called.load(Ordering::SeqCst));

        // Blur should trigger on_blur callback
        handle.blur();
        assert!(blur_called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_focus_handle_blur_only_calls_callback_if_was_focused() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let ctx = setup();
        let node_id = NodeId::new();

        let blur_count = Arc::new(AtomicUsize::new(0));
        let blur_counter = blur_count.clone();

        let handle = FocusHandle::new(node_id, ctx.clone()).on_blur(move || {
            blur_counter.fetch_add(1, Ordering::SeqCst);
        });

        // Blur without being focused should not call callback
        handle.blur();
        assert_eq!(blur_count.load(Ordering::SeqCst), 0);

        // Focus then blur should call callback
        handle.focus();
        handle.blur();
        assert_eq!(blur_count.load(Ordering::SeqCst), 1);

        // Blur again without focus should not call callback
        handle.blur();
        assert_eq!(blur_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_multiple_string_ids() {
        let ctx = setup();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        let _h1 = FocusHandle::with_id(id1, ctx.clone(), "input-1");
        let _h2 = FocusHandle::with_id(id2, ctx.clone(), "input-2");
        let _h3 = FocusHandle::with_id(id3, ctx.clone(), "input-3");

        // Focus each by ID
        assert!(ctx.write().unwrap().focus_by_id("input-1"));
        assert_eq!(ctx.read().unwrap().focused_id(), Some("input-1"));

        assert!(ctx.write().unwrap().focus_by_id("input-2"));
        assert_eq!(ctx.read().unwrap().focused_id(), Some("input-2"));

        assert!(ctx.write().unwrap().focus_by_id("input-3"));
        assert_eq!(ctx.read().unwrap().focused_id(), Some("input-3"));
    }

    #[test]
    fn test_mixed_id_and_no_id_handles() {
        let ctx = setup();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        // One with ID, one without
        let _h1 = FocusHandle::with_id(id1, ctx.clone(), "named");
        let h2 = FocusHandle::new(id2, ctx.clone());

        // Focus named one
        assert!(ctx.write().unwrap().focus_by_id("named"));
        assert_eq!(ctx.read().unwrap().focused_id(), Some("named"));

        // Focus unnamed one - focused_id should be None
        h2.focus();
        assert!(ctx.read().unwrap().focused_id().is_none());
        assert_eq!(ctx.read().unwrap().focused(), Some(id2));
    }

    // ========== Focus Trap Tests ==========

    #[test]
    fn test_focus_trap_push_and_pop() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        // Register with IDs
        ctx.register_with_id(id1, "modal-confirm");
        ctx.register_with_id(id2, "modal-cancel");
        ctx.register_with_id(id3, "background");

        // Focus the background element
        ctx.focus(id3);
        assert_eq!(ctx.focused(), Some(id3));

        // Push trap for modal elements
        let trap_id = ctx.push_trap(&["modal-confirm", "modal-cancel"]);

        // First element in trap should be focused
        assert_eq!(ctx.focused(), Some(id1));
        assert!(ctx.has_active_trap());

        // Pop the trap
        assert!(ctx.pop_trap(trap_id));

        // Focus should be restored
        assert_eq!(ctx.focused(), Some(id3));
        assert!(!ctx.has_active_trap());
    }

    #[test]
    fn test_focus_trap_navigation() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        ctx.register_with_id(id1, "btn-1");
        ctx.register_with_id(id2, "btn-2");
        ctx.register_with_id(id3, "btn-3");

        // Push trap with only btn-1 and btn-2
        let _trap_id = ctx.push_trap(&["btn-1", "btn-2"]);

        // Should start at btn-1
        assert_eq!(ctx.focused(), Some(id1));

        // Next should go to btn-2
        ctx.focus_next_trapped();
        assert_eq!(ctx.focused(), Some(id2));

        // Next should wrap back to btn-1 (NOT btn-3!)
        ctx.focus_next_trapped();
        assert_eq!(ctx.focused(), Some(id1));

        // Prev should wrap to btn-2
        ctx.focus_prev_trapped();
        assert_eq!(ctx.focused(), Some(id2));
    }

    #[test]
    fn test_focus_trap_empty() {
        let mut ctx = FocusContext::new();

        // Push trap with no valid IDs
        let trap_id = ctx.push_trap(&["nonexistent"]);

        // Should still work, just no focus
        assert!(ctx.has_active_trap());
        ctx.focus_next_trapped();
        ctx.focus_prev_trapped();

        assert!(ctx.pop_trap(trap_id));
    }

    #[test]
    fn test_focus_trap_nested() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();
        let id4 = NodeId::new();

        ctx.register_with_id(id1, "outer-1");
        ctx.register_with_id(id2, "outer-2");
        ctx.register_with_id(id3, "inner-1");
        ctx.register_with_id(id4, "inner-2");

        // Focus outer-1
        ctx.focus(id1);

        // Push outer trap
        let outer_trap = ctx.push_trap(&["outer-1", "outer-2"]);
        assert_eq!(ctx.focused(), Some(id1));

        // Push inner trap (nested)
        let inner_trap = ctx.push_trap(&["inner-1", "inner-2"]);
        assert_eq!(ctx.focused(), Some(id3));

        // Navigation confined to inner trap
        ctx.focus_next_trapped();
        assert_eq!(ctx.focused(), Some(id4));

        // Pop inner trap
        ctx.pop_trap(inner_trap);
        assert_eq!(ctx.focused(), Some(id1)); // Restored to outer trap focus

        // Pop outer trap
        ctx.pop_trap(outer_trap);
        assert_eq!(ctx.focused(), Some(id1)); // Restored to original
    }

    #[test]
    fn test_focus_trap_pop_invalid() {
        let mut ctx = FocusContext::new();

        // Try to pop non-existent trap
        let fake_id = FocusTrapId::new();
        assert!(!ctx.pop_trap(fake_id));
    }

    // ========== Focus Group Tests ==========

    #[test]
    fn test_focus_group_register() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        ctx.register_with_group(id1, "item-1", "sidebar");
        ctx.register_with_group(id2, "item-2", "sidebar");

        // Check group membership
        assert_eq!(ctx.get_group(id1), Some("sidebar"));
        assert_eq!(ctx.get_group(id2), Some("sidebar"));

        // Check group contains both nodes
        let nodes = ctx.get_group_nodes("sidebar").unwrap();
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains(&id1));
        assert!(nodes.contains(&id2));
    }

    #[test]
    fn test_focus_group_unregister() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        ctx.register_with_group(id1, "item-1", "sidebar");
        ctx.register_with_group(id2, "item-2", "sidebar");

        // Unregister one
        ctx.unregister_with_group(id1);

        // Group should still exist with one member
        assert!(ctx.get_group(id1).is_none());
        assert_eq!(ctx.get_group(id2), Some("sidebar"));

        let nodes = ctx.get_group_nodes("sidebar").unwrap();
        assert_eq!(nodes.len(), 1);

        // Unregister the other
        ctx.unregister_with_group(id2);

        // Group should be removed
        assert!(ctx.get_group_nodes("sidebar").is_none());
    }

    #[test]
    fn test_focus_group_focus() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        ctx.register_with_group(id1, "item-1", "sidebar");
        ctx.register_with_group(id2, "item-2", "sidebar");

        // Focus the group
        assert!(ctx.focus_group("sidebar"));
        assert_eq!(ctx.focused(), Some(id1));

        // Non-existent group
        assert!(!ctx.focus_group("nonexistent"));
    }

    #[test]
    fn test_focus_group_navigation() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();
        let other = NodeId::new();

        ctx.register_with_group(id1, "item-1", "sidebar");
        ctx.register_with_group(id2, "item-2", "sidebar");
        ctx.register_with_group(id3, "item-3", "sidebar");
        ctx.register_with_id(other, "other"); // Not in the group

        // Focus first in group
        ctx.focus_group("sidebar");
        assert_eq!(ctx.focused(), Some(id1));

        // Next in group
        ctx.focus_next_in_group();
        assert_eq!(ctx.focused(), Some(id2));

        ctx.focus_next_in_group();
        assert_eq!(ctx.focused(), Some(id3));

        // Wrap around within group
        ctx.focus_next_in_group();
        assert_eq!(ctx.focused(), Some(id1));

        // Prev in group
        ctx.focus_prev_in_group();
        assert_eq!(ctx.focused(), Some(id3));
    }

    #[test]
    fn test_focus_group_names() {
        let mut ctx = FocusContext::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        ctx.register_with_group(id1, "item-1", "sidebar");
        ctx.register_with_group(id2, "item-2", "main");
        ctx.register_with_group(id3, "item-3", "footer");

        let names: Vec<_> = ctx.group_names().collect();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"sidebar"));
        assert!(names.contains(&"main"));
        assert!(names.contains(&"footer"));
    }

    #[test]
    fn test_focus_handle_with_group() {
        let ctx = setup();
        let id = NodeId::new();

        let handle = FocusHandle::with_group(id, ctx.clone(), "my-item", "my-group");

        assert_eq!(handle.focus_id(), Some("my-item"));
        assert_eq!(handle.group(), Some("my-group"));

        // Check context
        assert_eq!(ctx.read().unwrap().get_group(id), Some("my-group"));
    }

    #[test]
    fn test_focus_handle_with_group_drop() {
        let ctx = setup();
        let id = NodeId::new();

        {
            let _handle = FocusHandle::with_group(id, ctx.clone(), "item", "group");
            assert!(ctx.read().unwrap().get_group_nodes("group").is_some());
        }
        // Handle dropped

        // Group should be cleaned up
        assert!(ctx.read().unwrap().get_group_nodes("group").is_none());
    }

    #[test]
    fn test_multiple_groups() {
        let mut ctx = FocusContext::new();
        let s1 = NodeId::new();
        let s2 = NodeId::new();
        let m1 = NodeId::new();
        let m2 = NodeId::new();

        ctx.register_with_group(s1, "sidebar-1", "sidebar");
        ctx.register_with_group(s2, "sidebar-2", "sidebar");
        ctx.register_with_group(m1, "main-1", "main");
        ctx.register_with_group(m2, "main-2", "main");

        // Focus sidebar group
        ctx.focus_group("sidebar");
        assert_eq!(ctx.focused(), Some(s1));

        // Navigate within sidebar
        ctx.focus_next_in_group();
        assert_eq!(ctx.focused(), Some(s2));

        // Switch to main group
        ctx.focus_group("main");
        assert_eq!(ctx.focused(), Some(m1));

        // Navigate within main
        ctx.focus_next_in_group();
        assert_eq!(ctx.focused(), Some(m2));
    }
}
