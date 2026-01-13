//! Accessibility API for assistive technology support.
//!
//! This module provides tools for making terminal UIs accessible to users
//! who rely on screen readers and other assistive technologies. It includes:
//!
//! - ARIA-like roles and labels
//! - Live region announcements
//! - Focus management helpers
//! - Semantic structure definitions
//!
//! # Example
//!
//! ```rust
//! use inky::accessibility::{AccessibleNode, Role, LiveRegion};
//!
//! // Create an accessible button
//! let button = AccessibleNode::new(Role::Button)
//!     .label("Submit form")
//!     .hint("Press Enter to submit");
//!
//! // Create a live region for status updates
//! let status = AccessibleNode::new(Role::Status)
//!     .live_region(LiveRegion::Polite);
//! ```
//!
//! # Screen Reader Integration
//!
//! While terminal screen readers vary in their capabilities, this module
//! provides a consistent API for expressing accessibility information that
//! can be:
//!
//! 1. Rendered as visual hints for sighted users
//! 2. Output via terminal escape sequences where supported
//! 3. Used by AI agents to understand UI semantics

use std::collections::HashMap;

/// ARIA-like roles for terminal UI elements.
///
/// These roles describe the semantic purpose of UI elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Role {
    /// Generic element with no specific semantics.
    #[default]
    None,
    /// Interactive button.
    Button,
    /// Checkbox (can be checked/unchecked).
    Checkbox,
    /// Radio button (one of a group).
    Radio,
    /// Text input field.
    TextInput,
    /// Dropdown selection.
    Combobox,
    /// Slider for numeric values.
    Slider,
    /// Progress indicator.
    Progressbar,
    /// Scrollable list.
    List,
    /// Item in a list.
    ListItem,
    /// Data table.
    Table,
    /// Row in a table.
    Row,
    /// Cell in a table.
    Cell,
    /// Column header.
    ColumnHeader,
    /// Row header.
    RowHeader,
    /// Tab in a tablist.
    Tab,
    /// Container for tabs.
    TabList,
    /// Content panel associated with a tab.
    TabPanel,
    /// Modal dialog.
    Dialog,
    /// Alert dialog requiring action.
    AlertDialog,
    /// Tooltip/popup help.
    Tooltip,
    /// Menu container.
    Menu,
    /// Item in a menu.
    MenuItem,
    /// Section heading.
    Heading,
    /// Main content area.
    Main,
    /// Navigation region.
    Navigation,
    /// Search region.
    Search,
    /// Complementary/sidebar content.
    Complementary,
    /// Banner/header region.
    Banner,
    /// Footer region.
    ContentInfo,
    /// Status message region.
    Status,
    /// Log of messages.
    Log,
    /// Timer display.
    Timer,
    /// Alert message.
    Alert,
    /// Toolbar.
    Toolbar,
    /// Group of related elements.
    Group,
    /// Tree structure.
    Tree,
    /// Item in a tree.
    TreeItem,
    /// Image or graphic.
    Image,
    /// Figure with caption.
    Figure,
    /// Generic region with a label.
    Region,
    /// Article or independent content.
    Article,
    /// Form container.
    Form,
    /// Grid layout.
    Grid,
    /// Cell in a grid.
    GridCell,
}

impl Role {
    /// Returns true if this role is interactive.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Role::Button
                | Role::Checkbox
                | Role::Radio
                | Role::TextInput
                | Role::Combobox
                | Role::Slider
                | Role::Tab
                | Role::MenuItem
                | Role::TreeItem
                | Role::GridCell
        )
    }

    /// Returns true if this role is a landmark.
    pub fn is_landmark(&self) -> bool {
        matches!(
            self,
            Role::Main
                | Role::Navigation
                | Role::Search
                | Role::Complementary
                | Role::Banner
                | Role::ContentInfo
                | Role::Region
                | Role::Form
        )
    }

    /// Get the human-readable name for this role.
    pub fn name(&self) -> &'static str {
        match self {
            Role::None => "none",
            Role::Button => "button",
            Role::Checkbox => "checkbox",
            Role::Radio => "radio",
            Role::TextInput => "textbox",
            Role::Combobox => "combobox",
            Role::Slider => "slider",
            Role::Progressbar => "progressbar",
            Role::List => "list",
            Role::ListItem => "listitem",
            Role::Table => "table",
            Role::Row => "row",
            Role::Cell => "cell",
            Role::ColumnHeader => "columnheader",
            Role::RowHeader => "rowheader",
            Role::Tab => "tab",
            Role::TabList => "tablist",
            Role::TabPanel => "tabpanel",
            Role::Dialog => "dialog",
            Role::AlertDialog => "alertdialog",
            Role::Tooltip => "tooltip",
            Role::Menu => "menu",
            Role::MenuItem => "menuitem",
            Role::Heading => "heading",
            Role::Main => "main",
            Role::Navigation => "navigation",
            Role::Search => "search",
            Role::Complementary => "complementary",
            Role::Banner => "banner",
            Role::ContentInfo => "contentinfo",
            Role::Status => "status",
            Role::Log => "log",
            Role::Timer => "timer",
            Role::Alert => "alert",
            Role::Toolbar => "toolbar",
            Role::Group => "group",
            Role::Tree => "tree",
            Role::TreeItem => "treeitem",
            Role::Image => "image",
            Role::Figure => "figure",
            Role::Region => "region",
            Role::Article => "article",
            Role::Form => "form",
            Role::Grid => "grid",
            Role::GridCell => "gridcell",
        }
    }
}

/// Live region politeness levels.
///
/// Live regions announce changes to screen reader users.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiveRegion {
    /// No live updates.
    #[default]
    Off,
    /// Announce when convenient (non-interruptive).
    Polite,
    /// Announce immediately (interruptive).
    Assertive,
}

/// States for interactive elements.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AccessibleState {
    /// Element is selected.
    pub selected: bool,
    /// Element is checked (for checkboxes/radios).
    pub checked: Option<bool>,
    /// Element is expanded (for collapsible elements).
    pub expanded: Option<bool>,
    /// Element is disabled.
    pub disabled: bool,
    /// Element is currently focused.
    pub focused: bool,
    /// Element is hidden from assistive technology.
    pub hidden: bool,
    /// Element is required (for form fields).
    pub required: bool,
    /// Element has invalid input.
    pub invalid: bool,
    /// Element is busy/loading.
    pub busy: bool,
    /// Current value in a range (0.0 to 1.0).
    pub value_now: Option<f32>,
    /// Minimum value in a range.
    pub value_min: Option<f32>,
    /// Maximum value in a range.
    pub value_max: Option<f32>,
    /// Current position in a set (1-indexed).
    pub pos_in_set: Option<usize>,
    /// Size of the set.
    pub set_size: Option<usize>,
    /// Current level (for headings, tree items).
    pub level: Option<u8>,
}

impl AccessibleState {
    /// Create a new default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the checked state.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Set the expanded state.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Set the disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the selected state.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Set the focused state.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set the hidden state.
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Set the required state.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Set the invalid state.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Set the busy state.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Set the current value in a range.
    pub fn value(mut self, now: f32, min: f32, max: f32) -> Self {
        self.value_now = Some(now);
        self.value_min = Some(min);
        self.value_max = Some(max);
        self
    }

    /// Set position in a set.
    pub fn position(mut self, pos: usize, size: usize) -> Self {
        self.pos_in_set = Some(pos);
        self.set_size = Some(size);
        self
    }

    /// Set the heading/tree level.
    pub fn level(mut self, level: u8) -> Self {
        self.level = Some(level);
        self
    }
}

/// Accessible node with semantic information.
///
/// This struct holds all accessibility information for a UI element.
///
/// # Example
///
/// ```rust
/// use inky::accessibility::{AccessibleNode, Role, AccessibleState};
///
/// let node = AccessibleNode::new(Role::Button)
///     .label("Save document")
///     .hint("Press Enter to save")
///     .state(AccessibleState::new().disabled(false));
/// ```
#[derive(Debug, Clone, Default)]
pub struct AccessibleNode {
    /// The semantic role.
    pub role: Role,
    /// Human-readable label.
    pub label: Option<String>,
    /// Additional description.
    pub description: Option<String>,
    /// Keyboard shortcut hint.
    pub hint: Option<String>,
    /// Live region behavior.
    pub live_region: LiveRegion,
    /// Current state.
    pub state: AccessibleState,
    /// ID for relationships.
    pub id: Option<String>,
    /// IDs this element is labelled by.
    pub labelled_by: Vec<String>,
    /// IDs this element is described by.
    pub described_by: Vec<String>,
    /// IDs this element controls.
    pub controls: Vec<String>,
    /// ID of element this owns.
    pub owns: Vec<String>,
    /// ID of the active descendant.
    pub active_descendant: Option<String>,
    /// Custom properties.
    pub properties: HashMap<String, String>,
}

impl AccessibleNode {
    /// Create a new accessible node with a role.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::accessibility::{AccessibleNode, Role};
    ///
    /// let button = AccessibleNode::new(Role::Button);
    /// ```
    pub fn new(role: Role) -> Self {
        Self {
            role,
            ..Default::default()
        }
    }

    /// Set the accessible label.
    ///
    /// This is the primary text read by screen readers.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the description.
    ///
    /// Additional context read after the label.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set a keyboard hint.
    ///
    /// Describes the keyboard shortcut or interaction.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Set the live region behavior.
    pub fn live_region(mut self, live: LiveRegion) -> Self {
        self.live_region = live;
        self
    }

    /// Set the state.
    pub fn state(mut self, state: AccessibleState) -> Self {
        self.state = state;
        self
    }

    /// Set the ID for relationships.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Add a labelled-by relationship.
    pub fn labelled_by(mut self, id: impl Into<String>) -> Self {
        self.labelled_by.push(id.into());
        self
    }

    /// Add a described-by relationship.
    pub fn described_by(mut self, id: impl Into<String>) -> Self {
        self.described_by.push(id.into());
        self
    }

    /// Add a controls relationship.
    pub fn controls(mut self, id: impl Into<String>) -> Self {
        self.controls.push(id.into());
        self
    }

    /// Add an owns relationship.
    pub fn owns(mut self, id: impl Into<String>) -> Self {
        self.owns.push(id.into());
        self
    }

    /// Set the active descendant.
    pub fn active_descendant(mut self, id: impl Into<String>) -> Self {
        self.active_descendant = Some(id.into());
        self
    }

    /// Set a custom property.
    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Generate an announcement string for screen readers.
    ///
    /// This creates a text description of the node suitable for
    /// announcement by assistive technology.
    pub fn announce(&self) -> String {
        let mut parts = Vec::new();

        // Role first (for interactive, landmark, or semantically important elements)
        let announce_role = self.role.is_interactive()
            || self.role.is_landmark()
            || matches!(
                self.role,
                Role::Heading
                    | Role::Alert
                    | Role::Dialog
                    | Role::AlertDialog
                    | Role::List
                    | Role::ListItem
                    | Role::Table
                    | Role::Image
            );

        if announce_role {
            parts.push(self.role.name().to_string());
        }

        // Label
        if let Some(ref label) = self.label {
            parts.push(label.clone());
        }

        // State information
        if let Some(checked) = self.state.checked {
            parts.push(if checked {
                "checked".to_string()
            } else {
                "not checked".to_string()
            });
        }

        if let Some(expanded) = self.state.expanded {
            parts.push(if expanded {
                "expanded".to_string()
            } else {
                "collapsed".to_string()
            });
        }

        if self.state.disabled {
            parts.push("disabled".to_string());
        }

        if self.state.selected {
            parts.push("selected".to_string());
        }

        if self.state.required {
            parts.push("required".to_string());
        }

        if self.state.invalid {
            parts.push("invalid".to_string());
        }

        if self.state.busy {
            parts.push("busy".to_string());
        }

        // Value for ranges
        if let (Some(now), Some(max)) = (self.state.value_now, self.state.value_max) {
            let percent = (now / max * 100.0).round() as i32;
            parts.push(format!("{percent}%"));
        }

        // Position in set
        if let (Some(pos), Some(size)) = (self.state.pos_in_set, self.state.set_size) {
            parts.push(format!("{pos} of {size}"));
        }

        // Level for headings
        if let Some(level) = self.state.level {
            if self.role == Role::Heading {
                parts.push(format!("level {level}"));
            }
        }

        // Description
        if let Some(ref desc) = self.description {
            parts.push(desc.clone());
        }

        // Hint
        if let Some(ref hint) = self.hint {
            parts.push(hint.clone());
        }

        parts.join(", ")
    }
}

/// An announcement to be made by assistive technology.
///
/// Use this to create dynamic announcements for status changes,
/// errors, or other important information.
#[derive(Debug, Clone)]
pub struct Announcement {
    /// The message to announce.
    pub message: String,
    /// Politeness level.
    pub politeness: LiveRegion,
    /// Whether to clear pending announcements.
    pub clear: bool,
}

impl Announcement {
    /// Create a polite announcement (non-interruptive).
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::accessibility::Announcement;
    ///
    /// let ann = Announcement::polite("File saved successfully");
    /// ```
    pub fn polite(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            politeness: LiveRegion::Polite,
            clear: false,
        }
    }

    /// Create an assertive announcement (interruptive).
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::accessibility::Announcement;
    ///
    /// let ann = Announcement::assertive("Error: Connection lost");
    /// ```
    pub fn assertive(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            politeness: LiveRegion::Assertive,
            clear: false,
        }
    }

    /// Clear pending announcements before this one.
    pub fn clear_pending(mut self) -> Self {
        self.clear = true;
        self
    }
}

/// Manager for accessibility announcements.
///
/// This collects announcements during a render cycle for later
/// output to assistive technology.
#[derive(Debug, Clone, Default)]
pub struct AnnouncementQueue {
    /// Pending announcements.
    announcements: Vec<Announcement>,
}

impl AnnouncementQueue {
    /// Create a new empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an announcement to the queue.
    pub fn announce(&mut self, announcement: Announcement) {
        if announcement.clear {
            self.announcements.clear();
        }
        self.announcements.push(announcement);
    }

    /// Add a polite announcement.
    pub fn polite(&mut self, message: impl Into<String>) {
        self.announce(Announcement::polite(message));
    }

    /// Add an assertive announcement.
    pub fn assertive(&mut self, message: impl Into<String>) {
        self.announce(Announcement::assertive(message));
    }

    /// Get and clear pending announcements.
    pub fn drain(&mut self) -> Vec<Announcement> {
        std::mem::take(&mut self.announcements)
    }

    /// Check if there are pending announcements.
    pub fn is_empty(&self) -> bool {
        self.announcements.is_empty()
    }

    /// Get number of pending announcements.
    pub fn len(&self) -> usize {
        self.announcements.len()
    }
}

/// Keyboard navigation helpers.
///
/// These functions help implement accessible keyboard navigation patterns.
pub mod navigation {
    /// Calculate next index with wrap-around.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::accessibility::navigation::wrap_next;
    ///
    /// assert_eq!(wrap_next(2, 5), 3);  // Normal increment
    /// assert_eq!(wrap_next(4, 5), 0);  // Wraps to start
    /// ```
    pub fn wrap_next(current: usize, total: usize) -> usize {
        if total == 0 {
            0
        } else {
            (current + 1) % total
        }
    }

    /// Calculate previous index with wrap-around.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::accessibility::navigation::wrap_prev;
    ///
    /// assert_eq!(wrap_prev(2, 5), 1);  // Normal decrement
    /// assert_eq!(wrap_prev(0, 5), 4);  // Wraps to end
    /// ```
    pub fn wrap_prev(current: usize, total: usize) -> usize {
        if total == 0 {
            0
        } else if current == 0 {
            total - 1
        } else {
            current - 1
        }
    }

    /// Calculate next index without wrap (clamped).
    pub fn clamp_next(current: usize, total: usize) -> usize {
        if total == 0 {
            0
        } else {
            (current + 1).min(total - 1)
        }
    }

    /// Calculate previous index without wrap (clamped).
    pub fn clamp_prev(current: usize, _total: usize) -> usize {
        current.saturating_sub(1)
    }

    /// Navigate in a 2D grid.
    ///
    /// Returns the new index after moving in the given direction.
    pub fn grid_navigate(
        current: usize,
        cols: usize,
        rows: usize,
        direction: GridDirection,
        wrap: bool,
    ) -> usize {
        if cols == 0 || rows == 0 {
            return 0;
        }

        let row = current / cols;
        let col = current % cols;

        let (new_row, new_col) = match direction {
            GridDirection::Up => {
                if wrap {
                    (if row == 0 { rows - 1 } else { row - 1 }, col)
                } else {
                    (row.saturating_sub(1), col)
                }
            }
            GridDirection::Down => {
                if wrap {
                    ((row + 1) % rows, col)
                } else {
                    ((row + 1).min(rows - 1), col)
                }
            }
            GridDirection::Left => {
                if wrap {
                    (row, if col == 0 { cols - 1 } else { col - 1 })
                } else {
                    (row, col.saturating_sub(1))
                }
            }
            GridDirection::Right => {
                if wrap {
                    (row, (col + 1) % cols)
                } else {
                    (row, (col + 1).min(cols - 1))
                }
            }
        };

        new_row * cols + new_col
    }

    /// Direction for grid navigation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GridDirection {
        /// Move up in the grid.
        Up,
        /// Move down in the grid.
        Down,
        /// Move left in the grid.
        Left,
        /// Move right in the grid.
        Right,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_role_properties() {
        assert!(Role::Button.is_interactive());
        assert!(!Role::Button.is_landmark());

        assert!(!Role::Main.is_interactive());
        assert!(Role::Main.is_landmark());

        assert_eq!(Role::Button.name(), "button");
        assert_eq!(Role::Main.name(), "main");
    }

    #[test]
    fn test_accessible_state_builder() {
        let state = AccessibleState::new()
            .checked(true)
            .disabled(false)
            .value(50.0, 0.0, 100.0);

        assert_eq!(state.checked, Some(true));
        assert!(!state.disabled);
        assert_eq!(state.value_now, Some(50.0));
        assert_eq!(state.value_min, Some(0.0));
        assert_eq!(state.value_max, Some(100.0));
    }

    #[test]
    fn test_accessible_node_builder() {
        let node = AccessibleNode::new(Role::Button)
            .label("Submit")
            .hint("Press Enter")
            .state(AccessibleState::new().disabled(false));

        assert_eq!(node.role, Role::Button);
        assert_eq!(node.label, Some("Submit".to_string()));
        assert_eq!(node.hint, Some("Press Enter".to_string()));
    }

    #[test]
    fn test_announce() {
        let node = AccessibleNode::new(Role::Button)
            .label("Save")
            .state(AccessibleState::new().disabled(true));

        let announcement = node.announce();
        assert!(announcement.contains("button"));
        assert!(announcement.contains("Save"));
        assert!(announcement.contains("disabled"));
    }

    #[test]
    fn test_announce_checkbox() {
        let node = AccessibleNode::new(Role::Checkbox)
            .label("Remember me")
            .state(AccessibleState::new().checked(true));

        let announcement = node.announce();
        assert!(announcement.contains("checkbox"));
        assert!(announcement.contains("Remember me"));
        assert!(announcement.contains("checked"));
    }

    #[test]
    fn test_announce_progress() {
        let node = AccessibleNode::new(Role::Progressbar)
            .label("Upload progress")
            .state(AccessibleState::new().value(75.0, 0.0, 100.0));

        let announcement = node.announce();
        assert!(announcement.contains("Upload progress"));
        assert!(announcement.contains("75%"));
    }

    #[test]
    fn test_announcement_queue() {
        let mut queue = AnnouncementQueue::new();

        queue.polite("First message");
        queue.assertive("Important message");

        assert_eq!(queue.len(), 2);

        let announcements = queue.drain();
        assert_eq!(announcements.len(), 2);
        assert_eq!(announcements[0].message, "First message");
        assert!(queue.is_empty());
    }

    #[test]
    fn test_navigation_wrap() {
        use navigation::*;

        assert_eq!(wrap_next(0, 3), 1);
        assert_eq!(wrap_next(2, 3), 0);
        assert_eq!(wrap_prev(0, 3), 2);
        assert_eq!(wrap_prev(1, 3), 0);
    }

    #[test]
    fn test_navigation_clamp() {
        use navigation::*;

        assert_eq!(clamp_next(0, 3), 1);
        assert_eq!(clamp_next(2, 3), 2); // Stays at end
        assert_eq!(clamp_prev(0, 3), 0); // Stays at start
        assert_eq!(clamp_prev(2, 3), 1);
    }

    #[test]
    fn test_grid_navigation() {
        use navigation::*;

        // 3x3 grid: 0 1 2
        //           3 4 5
        //           6 7 8

        // From center (4)
        assert_eq!(grid_navigate(4, 3, 3, GridDirection::Up, false), 1);
        assert_eq!(grid_navigate(4, 3, 3, GridDirection::Down, false), 7);
        assert_eq!(grid_navigate(4, 3, 3, GridDirection::Left, false), 3);
        assert_eq!(grid_navigate(4, 3, 3, GridDirection::Right, false), 5);

        // Wrap from edges
        assert_eq!(grid_navigate(0, 3, 3, GridDirection::Up, true), 6);
        assert_eq!(grid_navigate(0, 3, 3, GridDirection::Left, true), 2);

        // Clamp at edges
        assert_eq!(grid_navigate(0, 3, 3, GridDirection::Up, false), 0);
        assert_eq!(grid_navigate(0, 3, 3, GridDirection::Left, false), 0);
    }

    #[test]
    fn test_heading_announce() {
        let node = AccessibleNode::new(Role::Heading)
            .label("Section Title")
            .state(AccessibleState::new().level(2));

        let announcement = node.announce();
        assert!(announcement.contains("heading"));
        assert!(announcement.contains("Section Title"));
        assert!(announcement.contains("level 2"));
    }

    #[test]
    fn test_list_item_position() {
        let node = AccessibleNode::new(Role::ListItem)
            .label("Item Three")
            .state(AccessibleState::new().position(3, 10));

        let announcement = node.announce();
        assert!(announcement.contains("Item Three"));
        assert!(announcement.contains("3 of 10"));
    }

    #[test]
    fn test_clear_pending() {
        let mut queue = AnnouncementQueue::new();

        queue.polite("Message 1");
        queue.polite("Message 2");
        queue.announce(Announcement::assertive("Clear and say this").clear_pending());

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.drain()[0].message, "Clear and say this");
    }
}
