//! Popup and modal system for dialogs, selections, and confirmations.
//!
//! Provides a suite of modal components for common UI patterns:
//!
//! - [`Modal`] - Generic modal wrapper with backdrop and centering
//! - [`SelectPopup`] - List selection with keyboard navigation and filtering
//! - [`ConfirmDialog`] - Yes/No confirmation dialog
//! - [`ErrorPopup`] - Error message display
//!
//! # Example: Selection Popup
//!
//! ```
//! use inky::components::{SelectPopup, SelectPopupItem};
//!
//! let popup = SelectPopup::new()
//!     .title("Select Model")
//!     .items(vec![
//!         SelectPopupItem::new("opus", "Claude 3 Opus"),
//!         SelectPopupItem::new("sonnet", "Claude 3 Sonnet"),
//!     ])
//!     .selected(0)
//!     .filterable(true);
//! ```
//!
//! # Example: Confirmation Dialog
//!
//! ```
//! use inky::components::ConfirmDialog;
//!
//! let dialog = ConfirmDialog::new()
//!     .title("Delete file?")
//!     .message("This action cannot be undone.")
//!     .confirm_label("Delete")
//!     .cancel_label("Cancel");
//! ```
//!
//! # Example: Error Popup
//!
//! ```
//! use inky::components::ErrorPopup;
//!
//! let popup = ErrorPopup::new()
//!     .title("Error")
//!     .message("Failed to connect to API")
//!     .details("Connection timeout after 30s");
//! ```

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{AlignItems, BorderStyle, Color, FlexDirection, JustifyContent};

// ==================== Modal ====================

/// Generic modal wrapper with backdrop and centering.
///
/// Use this to wrap custom content in a modal overlay.
///
/// # Example
///
/// ```
/// use inky::components::Modal;
/// use inky::node::TextNode;
///
/// let modal = Modal::new()
///     .backdrop(true)
///     .child(TextNode::new("Modal content"));
/// ```
#[derive(Debug, Clone)]
pub struct Modal {
    /// Whether to show a backdrop behind the modal.
    backdrop: bool,
    /// Backdrop character (default: space with dim background).
    backdrop_char: char,
    /// Whether to center the modal.
    centered: bool,
    /// Modal content.
    child: Option<Node>,
    /// Border style for the modal.
    border: BorderStyle,
    /// Padding inside the modal.
    padding: u16,
}

impl Modal {
    /// Create a new modal.
    pub fn new() -> Self {
        Self {
            backdrop: true,
            backdrop_char: ' ',
            centered: true,
            child: None,
            border: BorderStyle::Rounded,
            padding: 1,
        }
    }

    /// Set whether to show a backdrop.
    pub fn backdrop(mut self, show: bool) -> Self {
        self.backdrop = show;
        self
    }

    /// Set the backdrop character.
    pub fn backdrop_char(mut self, c: char) -> Self {
        self.backdrop_char = c;
        self
    }

    /// Set whether to center the modal.
    pub fn centered(mut self, centered: bool) -> Self {
        self.centered = centered;
        self
    }

    /// Set the modal content.
    pub fn child(mut self, child: impl Into<Node>) -> Self {
        self.child = Some(child.into());
        self
    }

    /// Set the border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set the padding inside the modal.
    pub fn padding(mut self, padding: u16) -> Self {
        self.padding = padding;
        self
    }

    /// Check if modal has a backdrop.
    pub fn has_backdrop(&self) -> bool {
        self.backdrop
    }

    /// Check if modal is centered.
    pub fn is_centered(&self) -> bool {
        self.centered
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        // Build modal content box
        let mut modal_box = BoxNode::new()
            .border(self.border)
            .padding(self.padding as f32)
            .flex_direction(FlexDirection::Column);

        if let Some(ref child) = self.child {
            modal_box = modal_box.child(child.clone());
        }

        // If centered, wrap in centering container
        if self.centered {
            let centering_container = BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .flex_grow(1.0)
                .child(modal_box);

            centering_container.into()
        } else {
            modal_box.into()
        }
    }
}

impl Default for Modal {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Modal> for Node {
    fn from(modal: Modal) -> Self {
        modal.to_node()
    }
}

// ==================== SelectPopupItem ====================

/// An item in a selection popup.
#[derive(Debug, Clone)]
pub struct SelectPopupItem {
    /// Unique key for this item.
    pub key: String,
    /// Display label.
    pub label: String,
    /// Optional description.
    pub description: Option<String>,
}

impl SelectPopupItem {
    /// Create a new selection item.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::SelectPopupItem;
    ///
    /// let item = SelectPopupItem::new("opus", "Claude 3 Opus");
    /// ```
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: None,
        }
    }

    /// Add a description to the item.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

// ==================== SelectPopup ====================

/// A selection popup with keyboard navigation and optional filtering.
///
/// # Example
///
/// ```
/// use inky::components::{SelectPopup, SelectPopupItem};
///
/// let popup = SelectPopup::new()
///     .title("Select Model")
///     .items(vec![
///         SelectPopupItem::new("opus", "Claude 3 Opus"),
///         SelectPopupItem::new("sonnet", "Claude 3 Sonnet"),
///     ])
///     .selected(0);
/// ```
#[derive(Debug, Clone)]
pub struct SelectPopup {
    /// Title of the popup.
    title: Option<String>,
    /// Items to select from.
    items: Vec<SelectPopupItem>,
    /// Currently selected index.
    selected: usize,
    /// Whether filtering is enabled.
    filterable: bool,
    /// Current filter text.
    filter: String,
    /// Maximum visible items.
    max_visible: usize,
    /// Border style.
    border: BorderStyle,
    /// Selection indicator.
    indicator: String,
    /// Selection color.
    selection_color: Color,
}

impl SelectPopup {
    /// Create a new selection popup.
    pub fn new() -> Self {
        Self {
            title: None,
            items: Vec::new(),
            selected: 0,
            filterable: false,
            filter: String::new(),
            max_visible: 10,
            border: BorderStyle::Single,
            indicator: "> ".to_string(),
            selection_color: Color::BrightCyan,
        }
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the items.
    pub fn items(mut self, items: Vec<SelectPopupItem>) -> Self {
        self.items = items;
        self
    }

    /// Set the selected index.
    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index.min(self.items.len().saturating_sub(1));
        self
    }

    /// Enable or disable filtering.
    pub fn filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    /// Set the current filter text.
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = filter.into();
        self
    }

    /// Set maximum visible items.
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = max;
        self
    }

    /// Set the border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set the selection indicator.
    pub fn indicator(mut self, indicator: impl Into<String>) -> Self {
        self.indicator = indicator.into();
        self
    }

    /// Set the selection color.
    pub fn selection_color(mut self, color: Color) -> Self {
        self.selection_color = color;
        self
    }

    /// Get filtered items based on current filter.
    pub fn filtered_items(&self) -> Vec<(usize, &SelectPopupItem)> {
        let filter_lower = self.filter.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if self.filter.is_empty() {
                    true
                } else {
                    item.label.to_lowercase().contains(&filter_lower)
                        || item.key.to_lowercase().contains(&filter_lower)
                }
            })
            .collect()
    }

    /// Select next item.
    pub fn next(&mut self) {
        let filtered = self.filtered_items();
        if !filtered.is_empty() {
            self.selected = (self.selected + 1) % filtered.len();
        }
    }

    /// Select previous item.
    pub fn prev(&mut self) {
        let filtered = self.filtered_items();
        if !filtered.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(filtered.len() - 1);
        }
    }

    /// Get the currently selected item.
    pub fn get_selected(&self) -> Option<&SelectPopupItem> {
        let filtered = self.filtered_items();
        filtered.get(self.selected).map(|(_, item)| *item)
    }

    /// Get the key of the selected item.
    pub fn get_selected_key(&self) -> Option<&str> {
        self.get_selected().map(|item| item.key.as_str())
    }

    /// Add a character to the filter.
    pub fn filter_insert(&mut self, c: char) {
        if self.filterable {
            self.filter.push(c);
            self.selected = 0;
        }
    }

    /// Remove a character from the filter.
    pub fn filter_backspace(&mut self) {
        if self.filterable {
            self.filter.pop();
            self.selected = 0;
        }
    }

    /// Clear the filter.
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.selected = 0;
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .border(self.border);

        // Title
        if let Some(ref title) = self.title {
            container = container.child(TextNode::new(title).bold());
            container = container.child(TextNode::new("─".repeat(20)).dim());
        }

        // Filter input (if filterable)
        if self.filterable {
            let filter_display = if self.filter.is_empty() {
                "Filter...".to_string()
            } else {
                self.filter.clone()
            };
            let filter_color = if self.filter.is_empty() {
                Color::BrightBlack
            } else {
                Color::White
            };
            container = container.child(TextNode::new(filter_display).color(filter_color));
            container = container.child(TextNode::new("─".repeat(20)).dim());
        }

        // Items
        let filtered = self.filtered_items();
        for (i, (_, item)) in filtered.iter().take(self.max_visible).enumerate() {
            let is_selected = i == self.selected;

            let mut item_row = BoxNode::new().flex_direction(FlexDirection::Row);

            // Indicator
            let indicator_text = if is_selected {
                self.indicator.clone()
            } else {
                " ".repeat(self.indicator.len())
            };
            item_row = item_row.child(TextNode::new(indicator_text).color(self.selection_color));

            // Label
            let mut label = TextNode::new(&item.label);
            if is_selected {
                label = label.bold().color(self.selection_color);
            }
            item_row = item_row.child(label);

            // Description (if present)
            if let Some(ref desc) = item.description {
                item_row = item_row.child(TextNode::new(" - ").color(Color::BrightBlack));
                item_row = item_row.child(TextNode::new(desc).dim());
            }

            container = container.child(item_row);
        }

        // Show "more items" indicator if needed
        if filtered.len() > self.max_visible {
            let more_text = format!("... {} more", filtered.len() - self.max_visible);
            container = container.child(TextNode::new(more_text).dim());
        }

        // Empty state
        if filtered.is_empty() {
            container = container.child(TextNode::new("No matching items").dim());
        }

        container.into()
    }
}

impl Default for SelectPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl From<SelectPopup> for Node {
    fn from(popup: SelectPopup) -> Self {
        popup.to_node()
    }
}

// ==================== ConfirmDialog ====================

/// A confirmation dialog with yes/no buttons.
///
/// # Example
///
/// ```
/// use inky::components::ConfirmDialog;
///
/// let dialog = ConfirmDialog::new()
///     .title("Delete file?")
///     .message("This action cannot be undone.")
///     .confirm_label("Delete")
///     .cancel_label("Cancel");
/// ```
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    /// Dialog title.
    title: Option<String>,
    /// Dialog message.
    message: String,
    /// Confirm button label.
    confirm_label: String,
    /// Cancel button label.
    cancel_label: String,
    /// Which button is focused (0 = confirm, 1 = cancel).
    focused: usize,
    /// Border style.
    border: BorderStyle,
    /// Confirm button color.
    confirm_color: Color,
    /// Cancel button color.
    cancel_color: Color,
    /// Focused button color.
    focus_color: Color,
    /// Whether confirm is destructive (shown in red).
    destructive: bool,
}

impl ConfirmDialog {
    /// Create a new confirmation dialog.
    pub fn new() -> Self {
        Self {
            title: None,
            message: String::new(),
            confirm_label: "OK".to_string(),
            cancel_label: "Cancel".to_string(),
            focused: 1, // Default to cancel for safety
            border: BorderStyle::Rounded,
            confirm_color: Color::Green,
            cancel_color: Color::White,
            focus_color: Color::BrightCyan,
            destructive: false,
        }
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the message.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set the confirm button label.
    pub fn confirm_label(mut self, label: impl Into<String>) -> Self {
        self.confirm_label = label.into();
        self
    }

    /// Set the cancel button label.
    pub fn cancel_label(mut self, label: impl Into<String>) -> Self {
        self.cancel_label = label.into();
        self
    }

    /// Set which button is focused (0 = confirm, 1 = cancel).
    pub fn focused(mut self, focused: usize) -> Self {
        self.focused = focused.min(1);
        self
    }

    /// Set the border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Mark the confirm action as destructive (shown in red).
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self.confirm_color = Color::Red;
        self.focused = 1; // Default to cancel for destructive actions
        self
    }

    /// Focus the confirm button.
    pub fn focus_confirm(&mut self) {
        self.focused = 0;
    }

    /// Focus the cancel button.
    pub fn focus_cancel(&mut self) {
        self.focused = 1;
    }

    /// Toggle focus between buttons.
    pub fn toggle_focus(&mut self) {
        self.focused = 1 - self.focused;
    }

    /// Check if confirm button is focused.
    pub fn is_confirm_focused(&self) -> bool {
        self.focused == 0
    }

    /// Check if cancel button is focused.
    pub fn is_cancel_focused(&self) -> bool {
        self.focused == 1
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .border(self.border)
            .padding(1);

        // Title
        if let Some(ref title) = self.title {
            container = container.child(TextNode::new(title).bold());
            container = container.child(TextNode::new(" "));
        }

        // Message
        if !self.message.is_empty() {
            container = container.child(TextNode::new(&self.message));
            container = container.child(TextNode::new(" "));
        }

        // Buttons row
        let mut buttons = BoxNode::new().flex_direction(FlexDirection::Row).gap(2.0);

        // Confirm button
        let confirm_text = format!("[{}]", self.confirm_label);
        let mut confirm = TextNode::new(confirm_text);
        if self.focused == 0 {
            confirm = confirm.bold().color(self.focus_color);
        } else {
            confirm = confirm.color(self.confirm_color);
        }
        buttons = buttons.child(confirm);

        // Cancel button
        let cancel_text = format!("[{}]", self.cancel_label);
        let mut cancel = TextNode::new(cancel_text);
        if self.focused == 1 {
            cancel = cancel.bold().color(self.focus_color);
        } else {
            cancel = cancel.color(self.cancel_color);
        }
        buttons = buttons.child(cancel);

        container = container.child(buttons);

        container.into()
    }
}

impl Default for ConfirmDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ConfirmDialog> for Node {
    fn from(dialog: ConfirmDialog) -> Self {
        dialog.to_node()
    }
}

// ==================== ErrorPopup ====================

/// An error popup for displaying error messages.
///
/// # Example
///
/// ```
/// use inky::components::ErrorPopup;
///
/// let popup = ErrorPopup::new()
///     .title("Error")
///     .message("Failed to connect to API")
///     .details("Connection timeout after 30s");
/// ```
#[derive(Debug, Clone)]
pub struct ErrorPopup {
    /// Error title.
    title: String,
    /// Error message.
    message: String,
    /// Additional details.
    details: Option<String>,
    /// Border style.
    border: BorderStyle,
    /// Title color.
    title_color: Color,
    /// Message color.
    message_color: Color,
    /// Details color.
    details_color: Color,
}

impl ErrorPopup {
    /// Create a new error popup.
    pub fn new() -> Self {
        Self {
            title: "Error".to_string(),
            message: String::new(),
            details: None,
            border: BorderStyle::Rounded,
            title_color: Color::Red,
            message_color: Color::White,
            details_color: Color::BrightBlack,
        }
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the error message.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set additional details.
    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Set the border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set the title color.
    pub fn title_color(mut self, color: Color) -> Self {
        self.title_color = color;
        self
    }

    /// Set the message color.
    pub fn message_color(mut self, color: Color) -> Self {
        self.message_color = color;
        self
    }

    /// Set the details color.
    pub fn details_color(mut self, color: Color) -> Self {
        self.details_color = color;
        self
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .border(self.border)
            .padding(1);

        // Title with error icon
        let title_text = format!("✗ {}", self.title);
        container = container.child(TextNode::new(title_text).bold().color(self.title_color));

        // Message
        if !self.message.is_empty() {
            container = container.child(TextNode::new(" "));
            container = container.child(TextNode::new(&self.message).color(self.message_color));
        }

        // Details
        if let Some(ref details) = self.details {
            container = container.child(TextNode::new(" "));
            container = container.child(TextNode::new(details).color(self.details_color));
        }

        // Dismiss hint
        container = container.child(TextNode::new(" "));
        container = container.child(TextNode::new("Press Enter or Escape to dismiss").dim());

        container.into()
    }
}

impl Default for ErrorPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ErrorPopup> for Node {
    fn from(popup: ErrorPopup) -> Self {
        popup.to_node()
    }
}

// ==================== InfoPopup ====================

/// An information popup for displaying informational messages.
///
/// # Example
///
/// ```
/// use inky::components::InfoPopup;
///
/// let popup = InfoPopup::new()
///     .title("Info")
///     .message("Operation completed successfully");
/// ```
#[derive(Debug, Clone)]
pub struct InfoPopup {
    /// Info title.
    title: String,
    /// Info message.
    message: String,
    /// Border style.
    border: BorderStyle,
    /// Title color.
    title_color: Color,
    /// Message color.
    message_color: Color,
}

impl InfoPopup {
    /// Create a new info popup.
    pub fn new() -> Self {
        Self {
            title: "Info".to_string(),
            message: String::new(),
            border: BorderStyle::Rounded,
            title_color: Color::Blue,
            message_color: Color::White,
        }
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the info message.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set the border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set the title color.
    pub fn title_color(mut self, color: Color) -> Self {
        self.title_color = color;
        self
    }

    /// Set the message color.
    pub fn message_color(mut self, color: Color) -> Self {
        self.message_color = color;
        self
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .border(self.border)
            .padding(1);

        // Title with info icon
        let title_text = format!("ℹ {}", self.title);
        container = container.child(TextNode::new(title_text).bold().color(self.title_color));

        // Message
        if !self.message.is_empty() {
            container = container.child(TextNode::new(" "));
            container = container.child(TextNode::new(&self.message).color(self.message_color));
        }

        // Dismiss hint
        container = container.child(TextNode::new(" "));
        container = container.child(TextNode::new("Press Enter or Escape to dismiss").dim());

        container.into()
    }
}

impl Default for InfoPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl From<InfoPopup> for Node {
    fn from(popup: InfoPopup) -> Self {
        popup.to_node()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ========== Modal Tests ==========

    #[test]
    fn test_modal_new() {
        let modal = Modal::new();
        assert!(modal.has_backdrop());
        assert!(modal.is_centered());
    }

    #[test]
    fn test_modal_no_backdrop() {
        let modal = Modal::new().backdrop(false);
        assert!(!modal.has_backdrop());
    }

    #[test]
    fn test_modal_not_centered() {
        let modal = Modal::new().centered(false);
        assert!(!modal.is_centered());
    }

    #[test]
    fn test_modal_with_child() {
        let modal = Modal::new().child(TextNode::new("Content"));
        let _node: Node = modal.into();
    }

    // ========== SelectPopup Tests ==========

    #[test]
    fn test_select_popup_new() {
        let popup = SelectPopup::new();
        assert!(popup.filtered_items().is_empty());
    }

    #[test]
    fn test_select_popup_with_items() {
        let popup = SelectPopup::new().items(vec![
            SelectPopupItem::new("a", "Item A"),
            SelectPopupItem::new("b", "Item B"),
            SelectPopupItem::new("c", "Item C"),
        ]);
        assert_eq!(popup.filtered_items().len(), 3);
    }

    #[test]
    fn test_select_popup_navigation() {
        let mut popup = SelectPopup::new().items(vec![
            SelectPopupItem::new("a", "Item A"),
            SelectPopupItem::new("b", "Item B"),
            SelectPopupItem::new("c", "Item C"),
        ]);

        assert_eq!(popup.get_selected_key(), Some("a"));

        popup.next();
        assert_eq!(popup.get_selected_key(), Some("b"));

        popup.next();
        assert_eq!(popup.get_selected_key(), Some("c"));

        popup.next(); // Wrap around
        assert_eq!(popup.get_selected_key(), Some("a"));

        popup.prev(); // Wrap back
        assert_eq!(popup.get_selected_key(), Some("c"));
    }

    #[test]
    fn test_select_popup_filtering() {
        let mut popup = SelectPopup::new().filterable(true).items(vec![
            SelectPopupItem::new("apple", "Apple"),
            SelectPopupItem::new("banana", "Banana"),
            SelectPopupItem::new("apricot", "Apricot"),
        ]);

        assert_eq!(popup.filtered_items().len(), 3);

        popup.filter_insert('a');
        popup.filter_insert('p');
        let filtered = popup.filtered_items();
        assert_eq!(filtered.len(), 2);

        popup.clear_filter();
        assert_eq!(popup.filtered_items().len(), 3);
    }

    #[test]
    fn test_select_popup_item_description() {
        let item = SelectPopupItem::new("key", "Label").description("Description");
        assert_eq!(item.description, Some("Description".to_string()));
    }

    #[test]
    fn test_select_popup_to_node() {
        let popup = SelectPopup::new()
            .title("Select")
            .items(vec![SelectPopupItem::new("a", "A")]);
        let _node: Node = popup.into();
    }

    // ========== ConfirmDialog Tests ==========

    #[test]
    fn test_confirm_dialog_new() {
        let dialog = ConfirmDialog::new();
        assert!(dialog.is_cancel_focused()); // Default to cancel for safety
    }

    #[test]
    fn test_confirm_dialog_with_labels() {
        let dialog = ConfirmDialog::new()
            .title("Delete?")
            .message("Are you sure?")
            .confirm_label("Yes")
            .cancel_label("No");

        assert_eq!(dialog.title, Some("Delete?".to_string()));
        assert_eq!(dialog.message, "Are you sure?");
        assert_eq!(dialog.confirm_label, "Yes");
        assert_eq!(dialog.cancel_label, "No");
    }

    #[test]
    fn test_confirm_dialog_focus() {
        let mut dialog = ConfirmDialog::new();

        assert!(dialog.is_cancel_focused());

        dialog.focus_confirm();
        assert!(dialog.is_confirm_focused());

        dialog.toggle_focus();
        assert!(dialog.is_cancel_focused());
    }

    #[test]
    fn test_confirm_dialog_destructive() {
        let dialog = ConfirmDialog::new().destructive();
        assert_eq!(dialog.confirm_color, Color::Red);
        assert!(dialog.is_cancel_focused());
    }

    #[test]
    fn test_confirm_dialog_to_node() {
        let dialog = ConfirmDialog::new()
            .title("Confirm")
            .message("Are you sure?");
        let _node: Node = dialog.into();
    }

    // ========== ErrorPopup Tests ==========

    #[test]
    fn test_error_popup_new() {
        let popup = ErrorPopup::new();
        assert_eq!(popup.title, "Error");
    }

    #[test]
    fn test_error_popup_with_details() {
        let popup = ErrorPopup::new()
            .title("Connection Error")
            .message("Failed to connect")
            .details("Timeout after 30s");

        assert_eq!(popup.title, "Connection Error");
        assert_eq!(popup.message, "Failed to connect");
        assert_eq!(popup.details, Some("Timeout after 30s".to_string()));
    }

    #[test]
    fn test_error_popup_to_node() {
        let popup = ErrorPopup::new().message("Error occurred");
        let _node: Node = popup.into();
    }

    // ========== InfoPopup Tests ==========

    #[test]
    fn test_info_popup_new() {
        let popup = InfoPopup::new();
        assert_eq!(popup.title, "Info");
    }

    #[test]
    fn test_info_popup_with_message() {
        let popup = InfoPopup::new()
            .title("Success")
            .message("Operation completed");

        assert_eq!(popup.title, "Success");
        assert_eq!(popup.message, "Operation completed");
    }

    #[test]
    fn test_info_popup_to_node() {
        let popup = InfoPopup::new().message("Info message");
        let _node: Node = popup.into();
    }
}
