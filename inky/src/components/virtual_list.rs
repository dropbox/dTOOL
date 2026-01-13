//! VirtualList component for efficient rendering of long lists.
//!
//! This component virtualizes list rendering, only creating nodes for items
//! that are currently visible (plus a configurable overscan). Essential for
//! performance when displaying long lists such as chat conversation history.
//!
//! # Example
//!
//! ```ignore
//! use inky::prelude::*;
//! use inky::components::VirtualList;
//!
//! // Create a virtual list with 1000 items
//! let messages = vec!["msg 1", "msg 2", /* ... 1000 items */];
//! let list = VirtualList::new(messages.len())
//!     .item_height(3)
//!     .viewport_height(20)
//!     .render_item(|index| {
//!         TextNode::new(format!("Message {}", index)).into()
//!     });
//! ```

use std::sync::Arc;

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{BorderStyle, Color, FlexDirection, Overflow};

/// Callback type for rendering items by index.
pub type ItemRenderer = Arc<dyn Fn(usize) -> Node + Send + Sync>;

/// Configuration for item heights.
#[derive(Clone)]
pub enum ItemHeight {
    /// All items have the same fixed height.
    Uniform(u16),
    /// Items have variable heights. The callback returns height for each index.
    Variable(Arc<dyn Fn(usize) -> u16 + Send + Sync>),
}

impl std::fmt::Debug for ItemHeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemHeight::Uniform(h) => write!(f, "Uniform({})", h),
            ItemHeight::Variable(_) => write!(f, "Variable(<fn>)"),
        }
    }
}

impl Default for ItemHeight {
    fn default() -> Self {
        ItemHeight::Uniform(1)
    }
}

/// Scrollbar visibility mode for VirtualList.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualScrollbarVisibility {
    /// Show scrollbar only when content overflows viewport.
    #[default]
    Auto,
    /// Always show scrollbar.
    Always,
    /// Never show scrollbar.
    Never,
}

/// VirtualList component for efficient rendering of long lists.
///
/// Only renders items that are visible in the viewport, plus an overscan
/// buffer to ensure smooth scrolling. This dramatically reduces memory
/// usage and rendering time for lists with thousands of items.
///
/// # Features
///
/// - Uniform or variable item heights
/// - Configurable overscan for smooth scrolling
/// - Optional scrollbar
/// - Scroll-to-index functionality
///
/// # Performance
///
/// - Memory: O(viewport_items + overscan) instead of O(total_items)
/// - Render: Only visible items are laid out
/// - Scroll: O(1) for uniform heights, O(log n) for variable with cached heights
#[derive(Clone)]
pub struct VirtualList {
    /// Total number of items in the list.
    item_count: usize,
    /// Height configuration for items.
    item_height: ItemHeight,
    /// Visible viewport height in rows.
    viewport_height: u16,
    /// Viewport width (optional, defaults to fill available).
    viewport_width: Option<u16>,
    /// Current scroll offset (in items for uniform, pixels for variable).
    scroll_offset: usize,
    /// Number of items to render above/below viewport (overscan).
    overscan: usize,
    /// Function to render an item by index.
    render_item: Option<ItemRenderer>,
    /// Scrollbar visibility.
    scrollbar: VirtualScrollbarVisibility,
    /// Scrollbar track character.
    track_char: char,
    /// Scrollbar thumb character.
    thumb_char: char,
    /// Scrollbar color.
    scrollbar_color: Color,
    /// Track color.
    track_color: Color,
    /// Border style.
    border: BorderStyle,
}

impl Default for VirtualList {
    fn default() -> Self {
        Self::new(0)
    }
}

impl VirtualList {
    /// Create a new virtual list with the specified item count.
    pub fn new(item_count: usize) -> Self {
        Self {
            item_count,
            item_height: ItemHeight::Uniform(1),
            viewport_height: 10,
            viewport_width: None,
            scroll_offset: 0,
            overscan: 3,
            render_item: None,
            scrollbar: VirtualScrollbarVisibility::Auto,
            track_char: '│',
            thumb_char: '█',
            scrollbar_color: Color::BrightWhite,
            track_color: Color::BrightBlack,
            border: BorderStyle::None,
        }
    }

    /// Set the item count (total number of items).
    pub fn item_count(mut self, count: usize) -> Self {
        self.item_count = count;
        self
    }

    /// Set uniform item height.
    pub fn item_height(mut self, height: u16) -> Self {
        self.item_height = ItemHeight::Uniform(height);
        self
    }

    /// Set variable item heights with a callback.
    pub fn variable_height<F>(mut self, height_fn: F) -> Self
    where
        F: Fn(usize) -> u16 + Send + Sync + 'static,
    {
        self.item_height = ItemHeight::Variable(Arc::new(height_fn));
        self
    }

    /// Set the viewport height in rows.
    pub fn viewport_height(mut self, height: u16) -> Self {
        self.viewport_height = height;
        self
    }

    /// Set the viewport width (optional).
    pub fn viewport_width(mut self, width: u16) -> Self {
        self.viewport_width = Some(width);
        self
    }

    /// Set the scroll offset (item index for uniform, row for variable).
    pub fn scroll_offset(mut self, offset: usize) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Set the overscan count (items rendered above/below viewport).
    pub fn overscan(mut self, count: usize) -> Self {
        self.overscan = count;
        self
    }

    /// Set the item render function.
    pub fn render_item<F>(mut self, f: F) -> Self
    where
        F: Fn(usize) -> Node + Send + Sync + 'static,
    {
        self.render_item = Some(Arc::new(f));
        self
    }

    /// Set scrollbar visibility.
    pub fn scrollbar(mut self, visibility: VirtualScrollbarVisibility) -> Self {
        self.scrollbar = visibility;
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

    /// Set border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Get current scroll offset.
    pub fn get_scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Get total item count.
    pub fn get_item_count(&self) -> usize {
        self.item_count
    }

    /// Calculate total content height.
    pub fn total_height(&self) -> usize {
        match &self.item_height {
            ItemHeight::Uniform(h) => self.item_count * (*h as usize),
            ItemHeight::Variable(height_fn) => {
                (0..self.item_count).map(|i| height_fn(i) as usize).sum()
            }
        }
    }

    /// Scroll down by a number of items (uniform) or rows (variable).
    pub fn scroll_down(&mut self, amount: usize) {
        let max_offset = self.max_scroll_offset();
        self.scroll_offset = (self.scroll_offset + amount).min(max_offset);
    }

    /// Scroll up by a number of items (uniform) or rows (variable).
    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    /// Scroll to a specific index.
    pub fn scroll_to_index(&mut self, index: usize) {
        match &self.item_height {
            ItemHeight::Uniform(_) => {
                let max_offset = self.max_scroll_offset();
                self.scroll_offset = index.min(max_offset);
            }
            ItemHeight::Variable(height_fn) => {
                // For variable heights, calculate row offset
                let row_offset: usize = (0..index).map(|i| height_fn(i) as usize).sum();
                let max_offset = self.max_scroll_offset();
                self.scroll_offset = row_offset.min(max_offset);
            }
        }
    }

    /// Ensure an index is visible (scroll if needed).
    pub fn scroll_into_view(&mut self, index: usize) {
        let visible_range = self.visible_range();
        if index < visible_range.0 {
            self.scroll_to_index(index);
        } else if index >= visible_range.1 {
            // Scroll so item is at bottom of viewport
            match &self.item_height {
                ItemHeight::Uniform(h) => {
                    let items_in_viewport = self.viewport_height / h;
                    self.scroll_offset = index.saturating_sub(items_in_viewport as usize - 1);
                }
                ItemHeight::Variable(height_fn) => {
                    // Calculate offset to show index at bottom
                    let target_bottom: usize = (0..=index).map(|i| height_fn(i) as usize).sum();
                    let max_offset = self.max_scroll_offset();
                    self.scroll_offset = target_bottom
                        .saturating_sub(self.viewport_height as usize)
                        .min(max_offset);
                }
            }
        }
    }

    /// Scroll to top.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    /// Scroll to bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll_offset();
    }

    /// Page down.
    pub fn page_down(&mut self) {
        self.scroll_down(self.items_per_page());
    }

    /// Page up.
    pub fn page_up(&mut self) {
        self.scroll_up(self.items_per_page());
    }

    /// Calculate items per page for uniform heights.
    fn items_per_page(&self) -> usize {
        match &self.item_height {
            ItemHeight::Uniform(h) => (self.viewport_height / h) as usize,
            ItemHeight::Variable(_) => {
                // Approximate based on viewport
                (self.viewport_height as usize).max(1)
            }
        }
    }

    /// Calculate maximum scroll offset.
    fn max_scroll_offset(&self) -> usize {
        match &self.item_height {
            ItemHeight::Uniform(h) => {
                let items_in_viewport = self.viewport_height / h;
                self.item_count.saturating_sub(items_in_viewport as usize)
            }
            ItemHeight::Variable(_) => {
                let total = self.total_height();
                total.saturating_sub(self.viewport_height as usize)
            }
        }
    }

    /// Calculate the range of visible item indices.
    pub fn visible_range(&self) -> (usize, usize) {
        if self.item_count == 0 {
            return (0, 0);
        }

        match &self.item_height {
            ItemHeight::Uniform(h) => {
                let items_in_viewport = (self.viewport_height / h) as usize;
                let start = self.scroll_offset;
                let end = (start + items_in_viewport).min(self.item_count);
                (start, end)
            }
            ItemHeight::Variable(height_fn) => {
                // Find first visible item
                let mut cumulative = 0usize;
                let mut start = 0usize;
                for i in 0..self.item_count {
                    let h = height_fn(i) as usize;
                    if cumulative + h > self.scroll_offset {
                        start = i;
                        break;
                    }
                    cumulative += h;
                }

                // Find last visible item
                let target_end = self.scroll_offset + self.viewport_height as usize;
                let mut end = start;
                for i in start..self.item_count {
                    end = i + 1;
                    cumulative += height_fn(i) as usize;
                    if cumulative >= target_end {
                        break;
                    }
                }

                (start, end.min(self.item_count))
            }
        }
    }

    /// Calculate visible range with overscan.
    fn visible_range_with_overscan(&self) -> (usize, usize) {
        let (start, end) = self.visible_range();
        let overscan_start = start.saturating_sub(self.overscan);
        let overscan_end = (end + self.overscan).min(self.item_count);
        (overscan_start, overscan_end)
    }

    /// Check if scrollbar should be shown.
    fn should_show_scrollbar(&self) -> bool {
        match self.scrollbar {
            VirtualScrollbarVisibility::Always => true,
            VirtualScrollbarVisibility::Never => false,
            VirtualScrollbarVisibility::Auto => self.total_height() > self.viewport_height as usize,
        }
    }

    /// Calculate scrollbar thumb position and size.
    fn scrollbar_info(&self) -> (u16, u16) {
        let total = self.total_height();
        let viewport = self.viewport_height as usize;

        if total <= viewport {
            return (0, self.viewport_height);
        }

        // Thumb size proportional to viewport/total ratio
        let thumb_size = ((viewport as f32 / total as f32) * self.viewport_height as f32)
            .max(1.0)
            .min(self.viewport_height as f32) as u16;

        // Thumb position proportional to scroll progress
        let scrollable_area = total - viewport;
        let thumb_range = self.viewport_height - thumb_size;
        let thumb_pos = if scrollable_area > 0 {
            ((self.scroll_offset as f32 / scrollable_area as f32) * thumb_range as f32) as u16
        } else {
            0
        };

        (thumb_pos, thumb_size)
    }

    /// Build the scrollbar column.
    fn build_scrollbar(&self) -> Node {
        let (thumb_pos, thumb_size) = self.scrollbar_info();
        let mut scrollbar_chars = String::new();

        for row in 0..self.viewport_height {
            if row >= thumb_pos && row < thumb_pos + thumb_size {
                scrollbar_chars.push(self.thumb_char);
            } else {
                scrollbar_chars.push(self.track_char);
            }
            if row < self.viewport_height - 1 {
                scrollbar_chars.push('\n');
            }
        }

        BoxNode::new()
            .width(1)
            .height(self.viewport_height)
            .child(TextNode::new(scrollbar_chars).color(self.scrollbar_color))
            .into()
    }
}

impl From<VirtualList> for Node {
    fn from(list: VirtualList) -> Node {
        let render_fn = match &list.render_item {
            Some(f) => f.clone(),
            None => {
                // Default renderer shows item index
                Arc::new(|idx: usize| TextNode::new(format!("Item {}", idx)).into())
            }
        };

        let (start, end) = list.visible_range_with_overscan();
        let show_scrollbar = list.should_show_scrollbar();

        // Calculate content box width
        let content_width = list.viewport_width.map(|w| {
            if show_scrollbar {
                w.saturating_sub(1)
            } else {
                w
            }
        });

        // Build content column with visible items
        let mut content_box = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .overflow(Overflow::Hidden)
            .height(list.viewport_height);

        if let Some(w) = content_width {
            content_box = content_box.width(w);
        } else {
            content_box = content_box.flex_grow(1.0);
        }

        // Add padding for items scrolled past top (uniform height only)
        if let ItemHeight::Uniform(h) = &list.item_height {
            let visible_start = list.visible_range().0;
            if start < visible_start {
                // Items before visible area
                let skip_height = (visible_start - start) as u16 * h;
                content_box =
                    content_box.child(BoxNode::new().height(skip_height).flex_shrink(0.0));
            }
        }

        // Render visible items
        for idx in start..end {
            content_box = content_box.child(render_fn(idx));
        }

        // Build main container
        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Row)
            .height(list.viewport_height)
            .overflow(Overflow::Hidden)
            .border(list.border);

        if let Some(w) = list.viewport_width {
            container = container.width(w);
        }

        container = container.child(content_box);

        // Add scrollbar if needed
        if show_scrollbar {
            container = container.child(list.build_scrollbar());
        }

        container.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_list_basic() {
        let list = VirtualList::new(100).item_height(1).viewport_height(10);

        assert_eq!(list.get_item_count(), 100);
        assert_eq!(list.get_scroll_offset(), 0);
    }

    #[test]
    fn test_virtual_list_visible_range() {
        let list = VirtualList::new(100)
            .item_height(1)
            .viewport_height(10)
            .scroll_offset(5);

        let (start, end) = list.visible_range();
        assert_eq!(start, 5);
        assert_eq!(end, 15);
    }

    #[test]
    fn test_virtual_list_visible_range_at_end() {
        let list = VirtualList::new(100)
            .item_height(1)
            .viewport_height(10)
            .scroll_offset(95);

        let (start, end) = list.visible_range();
        assert_eq!(start, 95);
        assert_eq!(end, 100); // Capped at item count
    }

    #[test]
    fn test_virtual_list_overscan() {
        let list = VirtualList::new(100)
            .item_height(1)
            .viewport_height(10)
            .scroll_offset(20)
            .overscan(3);

        let (start, end) = list.visible_range_with_overscan();
        assert_eq!(start, 17); // 20 - 3
        assert_eq!(end, 33); // 30 + 3
    }

    #[test]
    fn test_virtual_list_scroll() {
        let mut list = VirtualList::new(100).item_height(1).viewport_height(10);

        list.scroll_down(5);
        assert_eq!(list.get_scroll_offset(), 5);

        list.scroll_up(3);
        assert_eq!(list.get_scroll_offset(), 2);

        list.scroll_to_bottom();
        assert_eq!(list.get_scroll_offset(), 90); // 100 - 10

        list.scroll_to_top();
        assert_eq!(list.get_scroll_offset(), 0);
    }

    #[test]
    fn test_virtual_list_page_navigation() {
        let mut list = VirtualList::new(100).item_height(1).viewport_height(10);

        list.page_down();
        assert_eq!(list.get_scroll_offset(), 10);

        list.page_up();
        assert_eq!(list.get_scroll_offset(), 0);
    }

    #[test]
    fn test_virtual_list_multi_row_items() {
        let list = VirtualList::new(50).item_height(2).viewport_height(10);

        let (start, end) = list.visible_range();
        assert_eq!(start, 0);
        assert_eq!(end, 5); // 10 rows / 2 per item = 5 items
    }

    #[test]
    fn test_virtual_list_scroll_into_view() {
        let mut list = VirtualList::new(100).item_height(1).viewport_height(10);

        // Item 50 is not visible, should scroll
        list.scroll_into_view(50);
        assert!(list.get_scroll_offset() > 0);

        let (start, end) = list.visible_range();
        assert!(start <= 50 && end > 50);
    }

    #[test]
    fn test_virtual_list_total_height() {
        let list = VirtualList::new(100).item_height(2);

        assert_eq!(list.total_height(), 200);
    }

    #[test]
    fn test_virtual_list_variable_height() {
        let list = VirtualList::new(10)
            .variable_height(|idx| if idx < 5 { 1 } else { 2 })
            .viewport_height(10);

        // First 5 items = 5 rows, next 5 items = 10 rows = 15 total
        assert_eq!(list.total_height(), 15);
    }

    #[test]
    fn test_virtual_list_to_node() {
        let list = VirtualList::new(100)
            .item_height(1)
            .viewport_height(10)
            .render_item(|idx| TextNode::new(format!("Item {}", idx)).into());

        let node: Node = list.into();
        // Should produce a Box node
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_virtual_list_scrollbar_auto() {
        // List with content larger than viewport
        let list = VirtualList::new(100)
            .item_height(1)
            .viewport_height(10)
            .scrollbar(VirtualScrollbarVisibility::Auto);

        assert!(list.should_show_scrollbar());

        // List with content smaller than viewport
        let list2 = VirtualList::new(5)
            .item_height(1)
            .viewport_height(10)
            .scrollbar(VirtualScrollbarVisibility::Auto);

        assert!(!list2.should_show_scrollbar());
    }

    #[test]
    fn test_virtual_list_empty() {
        let list = VirtualList::new(0).item_height(1).viewport_height(10);

        let (start, end) = list.visible_range();
        assert_eq!(start, 0);
        assert_eq!(end, 0);
    }

    #[test]
    fn test_virtual_list_scrollbar_info() {
        let list = VirtualList::new(100)
            .item_height(1)
            .viewport_height(10)
            .scroll_offset(0);

        let (pos, size) = list.scrollbar_info();
        assert_eq!(pos, 0); // At top
        assert!(size > 0 && size <= 10);

        // Scroll to middle
        let list2 = VirtualList::new(100)
            .item_height(1)
            .viewport_height(10)
            .scroll_offset(45);

        let (pos2, _) = list2.scrollbar_info();
        assert!(pos2 > 0); // Should be somewhere in the middle
    }
}
