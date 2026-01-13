//! Scroll component - scrollable viewport.
//!
//! The Scroll component provides a scrollable container that clips content
//! to a viewport and optionally displays scrollbar indicators.
//!
//! # Persistent Scroll State
//!
//! Scroll positions can be persisted across re-renders using string IDs:
//!
//! ```ignore
//! use inky::prelude::*;
//!
//! // Create a scroll container with a persistent ID
//! let scroll = Scroll::new()
//!     .id("chat-scroll")
//!     .height(10)
//!     .auto_scroll_to_bottom(true)
//!     .children(messages);
//!
//! // Later, get or set scroll position by ID
//! if let Some(offset) = get_scroll_offset("chat-scroll") {
//!     println!("Current offset: {}", offset);
//! }
//!
//! set_scroll_offset("chat-scroll", 50);
//! scroll_to_bottom("chat-scroll");
//! ```

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{BorderStyle, Color, FlexDirection, Overflow};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Scrollbar visibility mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarVisibility {
    /// Show scrollbar only when content overflows.
    #[default]
    Auto,
    /// Always show scrollbar.
    Always,
    /// Never show scrollbar.
    Never,
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollDirection {
    /// Vertical scrolling only.
    #[default]
    Vertical,
    /// Horizontal scrolling only.
    Horizontal,
    /// Both directions.
    Both,
}

// =============================================================================
// Global Scroll State Registry
// =============================================================================

/// Stored scroll state for a named scroll container.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Vertical scroll offset.
    pub offset_y: u16,
    /// Horizontal scroll offset.
    pub offset_x: u16,
    /// Content height (for max offset calculation).
    pub content_height: Option<u16>,
    /// Content width (for max offset calculation).
    pub content_width: Option<u16>,
    /// Viewport height.
    pub viewport_height: Option<u16>,
    /// Viewport width.
    pub viewport_width: Option<u16>,
}

/// Global registry for scroll state persistence.
static SCROLL_REGISTRY: OnceLock<RwLock<HashMap<String, ScrollState>>> = OnceLock::new();

/// Get the global scroll registry.
fn get_scroll_registry() -> &'static RwLock<HashMap<String, ScrollState>> {
    SCROLL_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Get the scroll offset for a named scroll container.
///
/// Returns `None` if the scroll ID is not registered.
///
/// # Example
///
/// ```ignore
/// use inky::components::scroll::get_scroll_offset;
///
/// if let Some(offset) = get_scroll_offset("chat-scroll") {
///     println!("Current Y offset: {}", offset);
/// }
/// ```
pub fn get_scroll_offset(scroll_id: &str) -> Option<u16> {
    let registry = get_scroll_registry();
    let guard = registry.read().unwrap_or_else(|p| p.into_inner());
    guard.get(scroll_id).map(|s| s.offset_y)
}

/// Get both scroll offsets for a named scroll container.
///
/// Returns `None` if the scroll ID is not registered.
pub fn get_scroll_offsets(scroll_id: &str) -> Option<(u16, u16)> {
    let registry = get_scroll_registry();
    let guard = registry.read().unwrap_or_else(|p| p.into_inner());
    guard.get(scroll_id).map(|s| (s.offset_x, s.offset_y))
}

/// Get the full scroll state for a named scroll container.
///
/// Returns `None` if the scroll ID is not registered.
pub fn get_scroll_state(scroll_id: &str) -> Option<ScrollState> {
    let registry = get_scroll_registry();
    let guard = registry.read().unwrap_or_else(|p| p.into_inner());
    guard.get(scroll_id).cloned()
}

/// Set the vertical scroll offset for a named scroll container.
///
/// Creates the scroll state if it doesn't exist.
///
/// # Example
///
/// ```ignore
/// use inky::components::scroll::set_scroll_offset;
///
/// set_scroll_offset("chat-scroll", 50);
/// ```
pub fn set_scroll_offset(scroll_id: &str, offset_y: u16) {
    let registry = get_scroll_registry();
    let mut guard = registry.write().unwrap_or_else(|p| p.into_inner());
    guard.entry(scroll_id.to_string()).or_default().offset_y = offset_y;
}

/// Set both scroll offsets for a named scroll container.
pub fn set_scroll_offsets(scroll_id: &str, offset_x: u16, offset_y: u16) {
    let registry = get_scroll_registry();
    let mut guard = registry.write().unwrap_or_else(|p| p.into_inner());
    let state = guard.entry(scroll_id.to_string()).or_default();
    state.offset_x = offset_x;
    state.offset_y = offset_y;
}

/// Scroll a named scroll container to the bottom.
///
/// Uses the stored content_height and viewport_height to calculate the max offset.
/// If the scroll ID is not registered or dimensions are not set, does nothing.
///
/// # Example
///
/// ```ignore
/// use inky::components::scroll::scroll_to_bottom;
///
/// scroll_to_bottom("chat-scroll");
/// ```
pub fn scroll_to_bottom(scroll_id: &str) {
    let registry = get_scroll_registry();
    let mut guard = registry.write().unwrap_or_else(|p| p.into_inner());
    if let Some(state) = guard.get_mut(scroll_id) {
        let content = state.content_height.unwrap_or(0);
        let viewport = state.viewport_height.unwrap_or(content);
        state.offset_y = content.saturating_sub(viewport);
    }
}

/// Scroll a named scroll container to the top.
pub fn scroll_to_top(scroll_id: &str) {
    let registry = get_scroll_registry();
    let mut guard = registry.write().unwrap_or_else(|p| p.into_inner());
    if let Some(state) = guard.get_mut(scroll_id) {
        state.offset_y = 0;
    }
}

/// Unregister a scroll ID from the registry.
pub fn unregister_scroll(scroll_id: &str) {
    let registry = get_scroll_registry();
    let mut guard = registry.write().unwrap_or_else(|p| p.into_inner());
    guard.remove(scroll_id);
}

/// Clear all scroll state (useful for tests).
#[doc(hidden)]
pub fn clear_scroll_registry() {
    let registry = get_scroll_registry();
    let mut guard = registry.write().unwrap_or_else(|p| p.into_inner());
    guard.clear();
}

// =============================================================================
// Scroll Component
// =============================================================================

/// Scrollable viewport component.
///
/// Provides a scrollable container that clips content to a viewport
/// and optionally displays scrollbar indicators.
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
///
/// let scroll = Scroll::new()
///     .height(10)
///     .children(vec![
///         TextNode::new("Line 1"),
///         TextNode::new("Line 2"),
///         // ... more lines
///     ]);
/// ```
///
/// # Persistent Scroll State
///
/// Use `.id()` to persist scroll position across re-renders:
///
/// ```ignore
/// let scroll = Scroll::new()
///     .id("chat-scroll")
///     .height(10)
///     .auto_scroll_to_bottom(true)
///     .children(messages);
/// ```
#[derive(Debug, Clone)]
pub struct Scroll {
    /// Content children.
    children: Vec<Node>,
    /// Vertical scroll offset.
    offset_y: u16,
    /// Horizontal scroll offset.
    offset_x: u16,
    /// Viewport height (visible rows).
    viewport_height: Option<u16>,
    /// Viewport width (visible columns).
    viewport_width: Option<u16>,
    /// Total content height (for scrollbar calculation).
    content_height: Option<u16>,
    /// Total content width (for scrollbar calculation).
    content_width: Option<u16>,
    /// Scrollbar visibility.
    scrollbar: ScrollbarVisibility,
    /// Scroll direction.
    direction: ScrollDirection,
    /// Border style.
    border: BorderStyle,
    /// Scrollbar track character.
    track_char: char,
    /// Scrollbar thumb character.
    thumb_char: char,
    /// Scrollbar color.
    scrollbar_color: Color,
    /// Track color.
    track_color: Color,
    /// Optional string ID for scroll state persistence.
    scroll_id: Option<String>,
    /// Auto-scroll to bottom when content changes.
    auto_scroll_to_bottom: bool,
}

impl Scroll {
    /// Create a new scroll container.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            offset_y: 0,
            offset_x: 0,
            viewport_height: None,
            viewport_width: None,
            content_height: None,
            content_width: None,
            scrollbar: ScrollbarVisibility::Auto,
            direction: ScrollDirection::Vertical,
            border: BorderStyle::None,
            track_char: '│',
            thumb_char: '█',
            scrollbar_color: Color::BrightWhite,
            track_color: Color::BrightBlack,
            scroll_id: None,
            auto_scroll_to_bottom: false,
        }
    }

    /// Set a string ID for scroll state persistence.
    ///
    /// When an ID is set, the scroll position is stored in a global registry
    /// and restored across re-renders. This is useful for maintaining scroll
    /// position in dynamic content like chat interfaces.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let scroll = Scroll::new()
    ///     .id("chat-scroll")
    ///     .height(10)
    ///     .children(messages);
    ///
    /// // Later, query or set position by ID:
    /// set_scroll_offset("chat-scroll", 50);
    /// ```
    pub fn id(mut self, scroll_id: impl Into<String>) -> Self {
        let id = scroll_id.into();
        // Load existing state from registry if present
        if let Some(state) = get_scroll_state(&id) {
            self.offset_y = state.offset_y;
            self.offset_x = state.offset_x;
        }
        self.scroll_id = Some(id);
        self
    }

    /// Enable auto-scrolling to bottom when content changes.
    ///
    /// When enabled, the scroll container will automatically scroll to the
    /// bottom whenever new content is added. This is useful for chat interfaces
    /// or log viewers where the newest content should always be visible.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let scroll = Scroll::new()
    ///     .id("chat-scroll")
    ///     .height(10)
    ///     .auto_scroll_to_bottom(true)
    ///     .children(messages);
    /// ```
    pub fn auto_scroll_to_bottom(mut self, enabled: bool) -> Self {
        self.auto_scroll_to_bottom = enabled;
        self
    }

    /// Get the scroll ID, if set.
    pub fn get_scroll_id(&self) -> Option<&str> {
        self.scroll_id.as_deref()
    }

    /// Check if auto-scroll to bottom is enabled.
    pub fn is_auto_scroll_to_bottom(&self) -> bool {
        self.auto_scroll_to_bottom
    }

    /// Add a child node.
    pub fn child(mut self, node: impl Into<Node>) -> Self {
        self.children.push(node.into());
        self
    }

    /// Add multiple children.
    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<Node>>) -> Self {
        self.children.extend(nodes.into_iter().map(Into::into));
        self
    }

    /// Set vertical scroll offset.
    pub fn offset_y(mut self, offset: u16) -> Self {
        self.offset_y = offset;
        self
    }

    /// Set horizontal scroll offset.
    pub fn offset_x(mut self, offset: u16) -> Self {
        self.offset_x = offset;
        self
    }

    /// Set viewport height.
    pub fn height(mut self, height: u16) -> Self {
        self.viewport_height = Some(height);
        self
    }

    /// Set viewport width.
    pub fn width(mut self, width: u16) -> Self {
        self.viewport_width = Some(width);
        self
    }

    /// Set content height (for scrollbar calculation).
    pub fn content_height(mut self, height: u16) -> Self {
        self.content_height = Some(height);
        self
    }

    /// Set content width (for scrollbar calculation).
    pub fn content_width(mut self, width: u16) -> Self {
        self.content_width = Some(width);
        self
    }

    /// Set scrollbar visibility.
    pub fn scrollbar(mut self, visibility: ScrollbarVisibility) -> Self {
        self.scrollbar = visibility;
        self
    }

    /// Set scroll direction.
    pub fn direction(mut self, direction: ScrollDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Set border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set scrollbar color.
    pub fn scrollbar_color(mut self, color: Color) -> Self {
        self.scrollbar_color = color;
        self
    }

    /// Set track color.
    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = color;
        self
    }

    /// Get current vertical offset.
    pub fn get_offset_y(&self) -> u16 {
        self.offset_y
    }

    /// Get current horizontal offset.
    pub fn get_offset_x(&self) -> u16 {
        self.offset_x
    }

    /// Scroll down by a number of lines.
    pub fn scroll_down(&mut self, lines: u16) {
        let max_offset = self.max_offset_y();
        self.offset_y = (self.offset_y + lines).min(max_offset);
    }

    /// Scroll up by a number of lines.
    pub fn scroll_up(&mut self, lines: u16) {
        self.offset_y = self.offset_y.saturating_sub(lines);
    }

    /// Scroll right by a number of columns.
    pub fn scroll_right(&mut self, cols: u16) {
        let max_offset = self.max_offset_x();
        self.offset_x = (self.offset_x + cols).min(max_offset);
    }

    /// Scroll left by a number of columns.
    pub fn scroll_left(&mut self, cols: u16) {
        self.offset_x = self.offset_x.saturating_sub(cols);
    }

    /// Scroll to a specific vertical position.
    pub fn scroll_to_y(&mut self, offset: u16) {
        let max_offset = self.max_offset_y();
        self.offset_y = offset.min(max_offset);
    }

    /// Scroll to a specific horizontal position.
    pub fn scroll_to_x(&mut self, offset: u16) {
        let max_offset = self.max_offset_x();
        self.offset_x = offset.min(max_offset);
    }

    /// Scroll to top.
    pub fn scroll_to_top(&mut self) {
        self.offset_y = 0;
    }

    /// Scroll to bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.offset_y = self.max_offset_y();
    }

    /// Internal method to scroll to bottom (uses children.len() for content height).
    fn scroll_to_bottom_internal(&mut self) {
        let content = self.content_height.unwrap_or(self.children.len() as u16);
        let viewport = self.viewport_height.unwrap_or(content);
        self.offset_y = content.saturating_sub(viewport);
    }

    /// Scroll to beginning (left).
    pub fn scroll_to_start(&mut self) {
        self.offset_x = 0;
    }

    /// Scroll to end (right).
    pub fn scroll_to_end(&mut self) {
        self.offset_x = self.max_offset_x();
    }

    /// Page down (scroll by viewport height).
    pub fn page_down(&mut self) {
        let page_size = self.viewport_height.unwrap_or(10);
        self.scroll_down(page_size);
    }

    /// Page up (scroll by viewport height).
    pub fn page_up(&mut self) {
        let page_size = self.viewport_height.unwrap_or(10);
        self.scroll_up(page_size);
    }

    /// Ensure a specific line is visible.
    pub fn scroll_into_view(&mut self, line: u16) {
        let viewport = self.viewport_height.unwrap_or(10);

        if line < self.offset_y {
            self.offset_y = line;
        } else if line >= self.offset_y + viewport {
            self.offset_y = line.saturating_sub(viewport - 1);
        }
    }

    /// Get maximum vertical scroll offset.
    fn max_offset_y(&self) -> u16 {
        let content = self.content_height.unwrap_or(self.children.len() as u16);
        let viewport = self.viewport_height.unwrap_or(content);
        content.saturating_sub(viewport)
    }

    /// Get maximum horizontal scroll offset.
    fn max_offset_x(&self) -> u16 {
        let content = self.content_width.unwrap_or(0);
        let viewport = self.viewport_width.unwrap_or(content);
        content.saturating_sub(viewport)
    }

    /// Check if vertical scrolling is needed.
    fn needs_vertical_scroll(&self) -> bool {
        match self.direction {
            ScrollDirection::Horizontal => false,
            _ => {
                let content = self.content_height.unwrap_or(self.children.len() as u16);
                let viewport = self.viewport_height.unwrap_or(content);
                content > viewport
            }
        }
    }

    /// Check if horizontal scrolling is needed.
    /// Reserved for future horizontal scroll rendering.
    #[allow(dead_code)]
    fn needs_horizontal_scroll(&self) -> bool {
        match self.direction {
            ScrollDirection::Vertical => false,
            _ => {
                let content = self.content_width.unwrap_or(0);
                let viewport = self.viewport_width.unwrap_or(content);
                content > viewport
            }
        }
    }

    /// Should show vertical scrollbar.
    fn show_vertical_scrollbar(&self) -> bool {
        match self.scrollbar {
            ScrollbarVisibility::Always => {
                matches!(
                    self.direction,
                    ScrollDirection::Vertical | ScrollDirection::Both
                )
            }
            ScrollbarVisibility::Auto => self.needs_vertical_scroll(),
            ScrollbarVisibility::Never => false,
        }
    }

    /// Should show horizontal scrollbar.
    /// Reserved for future horizontal scroll rendering.
    #[allow(dead_code)]
    fn show_horizontal_scrollbar(&self) -> bool {
        match self.scrollbar {
            ScrollbarVisibility::Always => {
                matches!(
                    self.direction,
                    ScrollDirection::Horizontal | ScrollDirection::Both
                )
            }
            ScrollbarVisibility::Auto => self.needs_horizontal_scroll(),
            ScrollbarVisibility::Never => false,
        }
    }

    /// Calculate scrollbar thumb position and size for vertical scrollbar.
    fn vertical_scrollbar_info(&self) -> (u16, u16) {
        let viewport = self.viewport_height.unwrap_or(10);
        let content = self.content_height.unwrap_or(self.children.len() as u16);

        if content <= viewport {
            return (0, viewport);
        }

        // Thumb size proportional to viewport/content ratio
        let thumb_size = ((viewport as f32 / content as f32) * viewport as f32)
            .max(1.0)
            .min(viewport as f32) as u16;

        // Thumb position proportional to scroll offset
        let max_offset = content.saturating_sub(viewport);
        let max_thumb_pos = viewport.saturating_sub(thumb_size);
        let thumb_pos = if max_offset > 0 {
            ((self.offset_y as f32 / max_offset as f32) * max_thumb_pos as f32) as u16
        } else {
            0
        };

        (thumb_pos, thumb_size)
    }

    /// Build vertical scrollbar text.
    fn build_vertical_scrollbar(&self) -> String {
        let viewport = self.viewport_height.unwrap_or(10);
        let (thumb_pos, thumb_size) = self.vertical_scrollbar_info();

        let mut scrollbar = String::new();
        for i in 0..viewport {
            if i >= thumb_pos && i < thumb_pos + thumb_size {
                scrollbar.push(self.thumb_char);
            } else {
                scrollbar.push(self.track_char);
            }
            if i < viewport - 1 {
                scrollbar.push('\n');
            }
        }
        scrollbar
    }
}

impl Default for Scroll {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Scroll> for Node {
    fn from(mut scroll: Scroll) -> Self {
        // Handle auto-scroll to bottom before extracting values
        if scroll.auto_scroll_to_bottom {
            scroll.scroll_to_bottom_internal();
        }

        // Save state to registry if we have an ID
        if let Some(ref id) = scroll.scroll_id {
            let registry = get_scroll_registry();
            let mut guard = registry.write().unwrap_or_else(|p| p.into_inner());
            let state = guard.entry(id.clone()).or_default();
            state.offset_y = scroll.offset_y;
            state.offset_x = scroll.offset_x;
            state.content_height = scroll.content_height;
            state.content_width = scroll.content_width;
            state.viewport_height = scroll.viewport_height;
            state.viewport_width = scroll.viewport_width;
        }

        // Extract values before consuming children
        let show_scrollbar = scroll.show_vertical_scrollbar();
        let scrollbar_text = scroll.build_vertical_scrollbar();
        let scrollbar_color = scroll.scrollbar_color;
        let border = scroll.border;
        let viewport_height = scroll.viewport_height;
        let viewport_width = scroll.viewport_width;
        let offset_y = scroll.offset_y;
        let children_len = scroll.children.len();

        // Create content container
        let mut content_box = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .overflow(Overflow::Hidden);

        if let Some(h) = viewport_height {
            content_box = content_box.height(h);
        }
        if let Some(w) = viewport_width {
            content_box = content_box.width(w);
        }

        // Add visible children (sliced by offset)
        let start = (offset_y as usize).min(children_len);
        let end = if let Some(h) = viewport_height {
            (start + h as usize).min(children_len)
        } else {
            children_len
        };

        for child in scroll
            .children
            .into_iter()
            .skip(start)
            .take(end.saturating_sub(start))
        {
            content_box = content_box.child(child);
        }

        // If we need a vertical scrollbar, wrap in a row
        if show_scrollbar {
            let scrollbar = TextNode::new(scrollbar_text).color(scrollbar_color);

            let mut container = BoxNode::new()
                .flex_direction(FlexDirection::Row)
                .border(border);

            container = container.child(content_box).child(scrollbar);

            container.into()
        } else {
            // No scrollbar, just return content box
            content_box = content_box.border(border);
            content_box.into()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::node::TextNode;

    #[test]
    fn test_scroll_new() {
        let scroll = Scroll::new();
        assert_eq!(scroll.get_offset_y(), 0);
        assert_eq!(scroll.get_offset_x(), 0);
    }

    #[test]
    fn test_scroll_offset() {
        let scroll = Scroll::new().offset_y(5).offset_x(3);
        assert_eq!(scroll.get_offset_y(), 5);
        assert_eq!(scroll.get_offset_x(), 3);
    }

    #[test]
    fn test_scroll_navigation() {
        let mut scroll = Scroll::new().height(5).content_height(20);

        assert_eq!(scroll.get_offset_y(), 0);

        scroll.scroll_down(3);
        assert_eq!(scroll.get_offset_y(), 3);

        scroll.scroll_up(1);
        assert_eq!(scroll.get_offset_y(), 2);

        scroll.scroll_to_bottom();
        assert_eq!(scroll.get_offset_y(), 15); // 20 - 5

        scroll.scroll_to_top();
        assert_eq!(scroll.get_offset_y(), 0);
    }

    #[test]
    fn test_scroll_page() {
        let mut scroll = Scroll::new().height(10).content_height(50);

        scroll.page_down();
        assert_eq!(scroll.get_offset_y(), 10);

        scroll.page_down();
        assert_eq!(scroll.get_offset_y(), 20);

        scroll.page_up();
        assert_eq!(scroll.get_offset_y(), 10);
    }

    #[test]
    fn test_scroll_into_view() {
        let mut scroll = Scroll::new().height(5).content_height(20);

        // Line within view
        scroll.scroll_into_view(2);
        assert_eq!(scroll.get_offset_y(), 0);

        // Line below view
        scroll.scroll_into_view(10);
        assert_eq!(scroll.get_offset_y(), 6); // 10 - 5 + 1

        // Line above view
        scroll.scroll_into_view(3);
        assert_eq!(scroll.get_offset_y(), 3);
    }

    #[test]
    fn test_scroll_bounds() {
        let mut scroll = Scroll::new().height(5).content_height(10);

        // Can't scroll past max
        scroll.scroll_down(100);
        assert_eq!(scroll.get_offset_y(), 5); // max is 10 - 5

        // Can't scroll negative
        scroll.scroll_up(100);
        assert_eq!(scroll.get_offset_y(), 0);
    }

    #[test]
    fn test_scroll_needs_scrollbar() {
        let scroll_needed = Scroll::new()
            .height(5)
            .content_height(20)
            .scrollbar(ScrollbarVisibility::Auto);
        assert!(scroll_needed.needs_vertical_scroll());

        let scroll_not_needed = Scroll::new()
            .height(20)
            .content_height(5)
            .scrollbar(ScrollbarVisibility::Auto);
        assert!(!scroll_not_needed.needs_vertical_scroll());
    }

    #[test]
    fn test_scroll_to_node() {
        let scroll = Scroll::new().height(5).children(vec![
            TextNode::new("Line 1"),
            TextNode::new("Line 2"),
            TextNode::new("Line 3"),
        ]);

        let node: Node = scroll.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_scroll_with_scrollbar() {
        let scroll = Scroll::new()
            .height(5)
            .content_height(20)
            .scrollbar(ScrollbarVisibility::Always)
            .children(vec![TextNode::new("Line 1"), TextNode::new("Line 2")]);

        let node: Node = scroll.into();
        // With scrollbar, should be wrapped in a row
        assert!(matches!(&node, Node::Box(_)));
        if let Node::Box(b) = node {
            assert_eq!(b.children.len(), 2); // content + scrollbar
        }
    }

    #[test]
    fn test_scrollbar_info() {
        let scroll = Scroll::new().height(10).content_height(100).offset_y(0);

        let (pos, size) = scroll.vertical_scrollbar_info();
        assert_eq!(pos, 0);
        assert!(size >= 1); // At least 1 char

        // At bottom
        let scroll = Scroll::new().height(10).content_height(100).offset_y(90);

        let (pos, _size) = scroll.vertical_scrollbar_info();
        assert!(pos > 0); // Should be at bottom
    }

    // === Scroll State Persistence Tests ===
    //
    // Note: These tests use unique IDs per test to avoid interference
    // when running in parallel. Each test uses a prefix unique to its test name.

    use std::sync::atomic::{AtomicU64, Ordering};

    // Counter for generating unique test IDs to avoid parallel test interference
    static TEST_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_id(prefix: &str) -> String {
        let n = TEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{}-{}", prefix, n)
    }

    #[test]
    fn test_scroll_id() {
        let id = unique_id("test-scroll-id");
        let scroll = Scroll::new().id(&id).height(10).content_height(50);

        assert_eq!(scroll.get_scroll_id(), Some(id.as_str()));
    }

    #[test]
    fn test_scroll_state_persistence() {
        let id = unique_id("persist");

        // Create scroll with ID and convert to node (saves state)
        let scroll = Scroll::new()
            .id(&id)
            .height(10)
            .content_height(50)
            .offset_y(25)
            .children(vec![TextNode::new("Line 1"), TextNode::new("Line 2")]);
        let _node: Node = scroll.into();

        // State should be saved
        assert_eq!(get_scroll_offset(&id), Some(25));

        // Create new scroll with same ID - should restore position
        let scroll2 = Scroll::new().id(&id).height(10).content_height(50);
        assert_eq!(scroll2.get_offset_y(), 25);
    }

    #[test]
    fn test_set_scroll_offset() {
        let id = unique_id("manual-scroll");

        set_scroll_offset(&id, 42);
        assert_eq!(get_scroll_offset(&id), Some(42));

        set_scroll_offset(&id, 100);
        assert_eq!(get_scroll_offset(&id), Some(100));
    }

    #[test]
    fn test_set_scroll_offsets() {
        let id = unique_id("xy-scroll");

        set_scroll_offsets(&id, 10, 20);
        assert_eq!(get_scroll_offsets(&id), Some((10, 20)));
    }

    #[test]
    fn test_get_scroll_state() {
        let id = unique_id("state-test");

        // Create and convert scroll to save state
        let scroll = Scroll::new()
            .id(&id)
            .height(10)
            .content_height(50)
            .offset_y(15);
        let _node: Node = scroll.into();

        let state = get_scroll_state(&id).unwrap();
        assert_eq!(state.offset_y, 15);
        assert_eq!(state.viewport_height, Some(10));
        assert_eq!(state.content_height, Some(50));
    }

    #[test]
    fn test_scroll_to_bottom_by_id() {
        let id = unique_id("bottom-test");

        // Create scroll and save state with dimensions
        let scroll = Scroll::new()
            .id(&id)
            .height(10)
            .content_height(50)
            .offset_y(0);
        let _node: Node = scroll.into();

        // Initially at top
        assert_eq!(get_scroll_offset(&id), Some(0));

        // Scroll to bottom using global function
        scroll_to_bottom(&id);
        assert_eq!(get_scroll_offset(&id), Some(40)); // 50 - 10
    }

    #[test]
    fn test_scroll_to_top_by_id() {
        let id = unique_id("top-test");

        set_scroll_offset(&id, 50);
        assert_eq!(get_scroll_offset(&id), Some(50));

        scroll_to_top(&id);
        assert_eq!(get_scroll_offset(&id), Some(0));
    }

    #[test]
    fn test_unregister_scroll() {
        let id = unique_id("unregister-test");

        set_scroll_offset(&id, 42);
        assert!(get_scroll_offset(&id).is_some());

        unregister_scroll(&id);
        assert!(get_scroll_offset(&id).is_none());
    }

    #[test]
    fn test_auto_scroll_to_bottom() {
        let id = unique_id("auto-bottom");

        // Create scroll with auto_scroll_to_bottom and many children
        let scroll = Scroll::new()
            .id(&id)
            .height(5)
            .auto_scroll_to_bottom(true)
            .children((0..20).map(|i| TextNode::new(format!("Line {}", i))));

        assert!(scroll.is_auto_scroll_to_bottom());

        // Convert to node - should auto-scroll
        let _node: Node = scroll.into();

        // Should be scrolled to bottom (20 - 5 = 15)
        assert_eq!(get_scroll_offset(&id), Some(15));
    }

    #[test]
    fn test_auto_scroll_disabled() {
        let id = unique_id("no-auto");

        let scroll = Scroll::new()
            .id(&id)
            .height(5)
            .auto_scroll_to_bottom(false)
            .children((0..20).map(|i| TextNode::new(format!("Line {}", i))));

        assert!(!scroll.is_auto_scroll_to_bottom());

        let _node: Node = scroll.into();

        // Should remain at top
        assert_eq!(get_scroll_offset(&id), Some(0));
    }

    #[test]
    fn test_scroll_without_id_no_persistence() {
        let nonexistent_id = unique_id("nonexistent");

        // Create scroll without ID
        let scroll = Scroll::new().height(10).content_height(50).offset_y(25);
        let _node: Node = scroll.into();

        // No ID means nothing in registry for this specific ID
        assert!(get_scroll_offset(&nonexistent_id).is_none());
    }

    #[test]
    fn test_multiple_scroll_ids() {
        let id1 = unique_id("multi-1");
        let id2 = unique_id("multi-2");
        let id3 = unique_id("multi-3");

        // Create multiple scrolls with different IDs
        let scroll1 = Scroll::new().id(&id1).height(10).offset_y(10);
        let scroll2 = Scroll::new().id(&id2).height(10).offset_y(20);
        let scroll3 = Scroll::new().id(&id3).height(10).offset_y(30);

        let _: Node = scroll1.into();
        let _: Node = scroll2.into();
        let _: Node = scroll3.into();

        // Each should have its own state
        assert_eq!(get_scroll_offset(&id1), Some(10));
        assert_eq!(get_scroll_offset(&id2), Some(20));
        assert_eq!(get_scroll_offset(&id3), Some(30));
    }
}
