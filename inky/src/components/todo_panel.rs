//! Todo/Task panel widget for displaying task status.
//!
//! Provides widgets for displaying a list of todos/tasks with their status,
//! commonly used for showing AI task progress.
//!
//! # Example
//!
//! ```
//! use inky::components::{TodoPanel, TodoItem, TodoStatus};
//!
//! let panel = TodoPanel::new()
//!     .expanded(true)
//!     .items(vec![
//!         TodoItem::new("Fix bug").status(TodoStatus::Completed),
//!         TodoItem::new("Write tests").status(TodoStatus::InProgress),
//!         TodoItem::new("Update docs").status(TodoStatus::Pending),
//!     ]);
//! ```
//!
//! # Visual Output
//!
//! Expanded:
//! ```text
//! ┌─ Tasks ──────────────────────┐
//! │ ● Fix authentication bug     │
//! │ ◐ Writing unit tests...      │
//! │ ○ Update documentation       │
//! │ Next: Update documentation   │
//! └──────────────────────────────┘
//! ```
//!
//! Collapsed (badge in status bar):
//! ```text
//! 5 todos (2 tasks)
//! ```

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{BorderStyle, Color, FlexDirection};

/// Status of a todo item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TodoStatus {
    /// Not yet started.
    #[default]
    Pending,
    /// Currently being worked on.
    InProgress,
    /// Completed successfully.
    Completed,
}

impl TodoStatus {
    /// Get the Unicode indicator for this status.
    pub fn indicator(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "○",
            TodoStatus::InProgress => "◐",
            TodoStatus::Completed => "●",
        }
    }

    /// Get the ASCII indicator for this status.
    pub fn ascii_indicator(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "[ ]",
            TodoStatus::InProgress => "[~]",
            TodoStatus::Completed => "[x]",
        }
    }

    /// Get the color for this status.
    pub fn color(&self) -> Color {
        match self {
            TodoStatus::Pending => Color::BrightBlack,
            TodoStatus::InProgress => Color::Yellow,
            TodoStatus::Completed => Color::Green,
        }
    }
}

/// A single todo item.
#[derive(Debug, Clone)]
pub struct TodoItem {
    /// The todo content/description.
    content: String,
    /// Current status.
    status: TodoStatus,
    /// Alternative text when in progress (e.g., "Fixing bug..." vs "Fix bug").
    active_form: Option<String>,
}

impl TodoItem {
    /// Create a new todo item.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::TodoItem;
    ///
    /// let item = TodoItem::new("Write tests");
    /// ```
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            status: TodoStatus::Pending,
            active_form: None,
        }
    }

    /// Set the status.
    pub fn status(mut self, status: TodoStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the active form (displayed when in progress).
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{TodoItem, TodoStatus};
    ///
    /// let item = TodoItem::new("Write tests")
    ///     .active_form("Writing tests...")
    ///     .status(TodoStatus::InProgress);
    /// ```
    pub fn active_form(mut self, form: impl Into<String>) -> Self {
        self.active_form = Some(form.into());
        self
    }

    /// Get the display text based on current status.
    pub fn display_text(&self) -> &str {
        if self.status == TodoStatus::InProgress {
            self.active_form.as_ref().unwrap_or(&self.content)
        } else {
            &self.content
        }
    }

    /// Get the content.
    pub fn get_content(&self) -> &str {
        &self.content
    }

    /// Get the status.
    pub fn get_status(&self) -> TodoStatus {
        self.status
    }
}

/// A panel displaying a list of todos/tasks.
///
/// # Example
///
/// ```
/// use inky::components::{TodoPanel, TodoItem, TodoStatus};
///
/// let panel = TodoPanel::new()
///     .expanded(true)
///     .items(vec![
///         TodoItem::new("Fix bug").status(TodoStatus::Completed),
///         TodoItem::new("Write tests").status(TodoStatus::InProgress),
///     ]);
/// ```
#[derive(Debug, Clone)]
pub struct TodoPanel {
    /// Title of the panel.
    title: String,
    /// List of todo items.
    items: Vec<TodoItem>,
    /// Whether the panel is expanded.
    expanded: bool,
    /// Number of active background tasks/agents.
    active_tasks: usize,
    /// Border style.
    border: BorderStyle,
    /// Whether to show the "Next:" hint.
    show_next_hint: bool,
    /// Maximum visible items (when expanded).
    max_visible: usize,
}

impl TodoPanel {
    /// Create a new todo panel.
    pub fn new() -> Self {
        Self {
            title: "Tasks".to_string(),
            items: Vec::new(),
            expanded: true,
            active_tasks: 0,
            border: BorderStyle::Rounded,
            show_next_hint: true,
            max_visible: 10,
        }
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the todo items.
    pub fn items(mut self, items: Vec<TodoItem>) -> Self {
        self.items = items;
        self
    }

    /// Add a single item.
    pub fn item(mut self, item: TodoItem) -> Self {
        self.items.push(item);
        self
    }

    /// Set whether the panel is expanded.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Set the number of active background tasks.
    pub fn active_tasks(mut self, count: usize) -> Self {
        self.active_tasks = count;
        self
    }

    /// Set the border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set whether to show the "Next:" hint.
    pub fn show_next_hint(mut self, show: bool) -> Self {
        self.show_next_hint = show;
        self
    }

    /// Set maximum visible items.
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = max;
        self
    }

    /// Toggle expanded state.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Check if panel is expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Get count of pending items.
    pub fn pending_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == TodoStatus::Pending)
            .count()
    }

    /// Get count of in-progress items.
    pub fn in_progress_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == TodoStatus::InProgress)
            .count()
    }

    /// Get count of completed items.
    pub fn completed_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == TodoStatus::Completed)
            .count()
    }

    /// Get total count.
    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    /// Get the next pending item.
    pub fn next_pending(&self) -> Option<&TodoItem> {
        self.items.iter().find(|i| i.status == TodoStatus::Pending)
    }

    /// Get the currently in-progress item (first one found).
    pub fn current_item(&self) -> Option<&TodoItem> {
        self.items
            .iter()
            .find(|i| i.status == TodoStatus::InProgress)
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        if !self.expanded {
            return self.to_collapsed_node();
        }

        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .border(self.border);

        // Title
        let title_text = format!("─ {} ", self.title);
        container = container.child(TextNode::new(title_text).bold());

        // Items
        for (i, item) in self.items.iter().take(self.max_visible).enumerate() {
            let mut item_row = BoxNode::new().flex_direction(FlexDirection::Row);

            // Status indicator
            item_row = item_row.child(
                TextNode::new(format!("{} ", item.status.indicator())).color(item.status.color()),
            );

            // Content
            let mut content = TextNode::new(item.display_text());
            match item.status {
                TodoStatus::Completed => content = content.dim(),
                TodoStatus::InProgress => content = content.color(Color::Yellow),
                TodoStatus::Pending => {}
            }
            item_row = item_row.child(content);

            container = container.child(item_row);

            // Add separator after in-progress items if there are more items
            if item.status == TodoStatus::InProgress && i + 1 < self.items.len() {
                // Just visual spacing, not an actual separator
            }
        }

        // "More items" indicator
        if self.items.len() > self.max_visible {
            let more_text = format!("... {} more", self.items.len() - self.max_visible);
            container = container.child(TextNode::new(more_text).dim());
        }

        // Next hint
        if self.show_next_hint {
            if let Some(next) = self.next_pending() {
                container = container.child(TextNode::new(" "));
                container = container.child(
                    TextNode::new(format!("Next: {}", next.get_content()))
                        .color(Color::BrightBlack),
                );
            }
        }

        // Active tasks indicator
        if self.active_tasks > 0 {
            let task_text = format!(
                "{} background {}",
                self.active_tasks,
                if self.active_tasks == 1 {
                    "task"
                } else {
                    "tasks"
                }
            );
            container = container.child(TextNode::new(task_text).dim());
        }

        container.into()
    }

    /// Convert to a collapsed (badge) node.
    fn to_collapsed_node(&self) -> Node {
        let mut parts = Vec::new();

        let total = self.total_count();
        if total > 0 {
            parts.push(format!(
                "{} {}",
                total,
                if total == 1 { "todo" } else { "todos" }
            ));
        }

        if self.active_tasks > 0 {
            parts.push(format!(
                "{} {}",
                self.active_tasks,
                if self.active_tasks == 1 {
                    "task"
                } else {
                    "tasks"
                }
            ));
        }

        let text = if parts.is_empty() {
            "No tasks".to_string()
        } else {
            parts.join(" | ")
        };

        TextNode::new(text).color(Color::BrightBlack).into()
    }
}

impl Default for TodoPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl From<TodoPanel> for Node {
    fn from(panel: TodoPanel) -> Self {
        panel.to_node()
    }
}

/// A badge for displaying todo/task counts in the status bar.
///
/// # Example
///
/// ```
/// use inky::components::TodoBadge;
///
/// let badge = TodoBadge::new()
///     .todo_count(5)
///     .task_count(2);
/// ```
#[derive(Debug, Clone)]
pub struct TodoBadge {
    /// Number of todos.
    todo_count: usize,
    /// Number of active tasks.
    task_count: usize,
    /// In-progress count.
    in_progress: usize,
    /// Completed count.
    completed: usize,
    /// Color for the badge.
    color: Color,
    /// Whether to show detailed counts.
    detailed: bool,
}

impl TodoBadge {
    /// Create a new todo badge.
    pub fn new() -> Self {
        Self {
            todo_count: 0,
            task_count: 0,
            in_progress: 0,
            completed: 0,
            color: Color::BrightBlack,
            detailed: false,
        }
    }

    /// Set the todo count.
    pub fn todo_count(mut self, count: usize) -> Self {
        self.todo_count = count;
        self
    }

    /// Set the task count.
    pub fn task_count(mut self, count: usize) -> Self {
        self.task_count = count;
        self
    }

    /// Set the in-progress count.
    pub fn in_progress(mut self, count: usize) -> Self {
        self.in_progress = count;
        self
    }

    /// Set the completed count.
    pub fn completed(mut self, count: usize) -> Self {
        self.completed = count;
        self
    }

    /// Set the badge color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Show detailed counts (pending/in-progress/completed).
    pub fn detailed(mut self, detailed: bool) -> Self {
        self.detailed = detailed;
        self
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let mut row = BoxNode::new().flex_direction(FlexDirection::Row);
        let mut parts = Vec::new();

        let text = if self.detailed {
            // Detailed format: "5 todos (1 in progress, 2 done)"
            if self.todo_count > 0 {
                parts.push(format!("{} todos", self.todo_count));
            }

            let mut details = Vec::new();
            if self.in_progress > 0 {
                details.push(format!("{} in progress", self.in_progress));
            }
            if self.completed > 0 {
                details.push(format!("{} done", self.completed));
            }

            if !details.is_empty() {
                parts.push(format!("({})", details.join(", ")));
            }

            if self.task_count > 0 {
                parts.push(format!(
                    "{} {}",
                    self.task_count,
                    if self.task_count == 1 {
                        "task"
                    } else {
                        "tasks"
                    }
                ));
            }

            if parts.is_empty() {
                "No tasks".to_string()
            } else {
                parts.join(" | ")
            }
        } else {
            // Simple format: "5 todos (2 tasks)"
            if self.todo_count > 0 {
                parts.push(format!(
                    "{} {}",
                    self.todo_count,
                    if self.todo_count == 1 {
                        "todo"
                    } else {
                        "todos"
                    }
                ));
            }

            if self.task_count > 0 {
                parts.push(format!(
                    "({} {})",
                    self.task_count,
                    if self.task_count == 1 {
                        "task"
                    } else {
                        "tasks"
                    }
                ));
            }

            if parts.is_empty() {
                "No tasks".to_string()
            } else {
                parts.join(" ")
            }
        };

        row = row.child(TextNode::new(text).color(self.color));
        row.into()
    }
}

impl Default for TodoBadge {
    fn default() -> Self {
        Self::new()
    }
}

impl From<TodoBadge> for Node {
    fn from(badge: TodoBadge) -> Self {
        badge.to_node()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ========== TodoStatus Tests ==========

    #[test]
    fn test_todo_status_indicators() {
        assert_eq!(TodoStatus::Pending.indicator(), "○");
        assert_eq!(TodoStatus::InProgress.indicator(), "◐");
        assert_eq!(TodoStatus::Completed.indicator(), "●");
    }

    #[test]
    fn test_todo_status_ascii_indicators() {
        assert_eq!(TodoStatus::Pending.ascii_indicator(), "[ ]");
        assert_eq!(TodoStatus::InProgress.ascii_indicator(), "[~]");
        assert_eq!(TodoStatus::Completed.ascii_indicator(), "[x]");
    }

    #[test]
    fn test_todo_status_colors() {
        assert_eq!(TodoStatus::Pending.color(), Color::BrightBlack);
        assert_eq!(TodoStatus::InProgress.color(), Color::Yellow);
        assert_eq!(TodoStatus::Completed.color(), Color::Green);
    }

    // ========== TodoItem Tests ==========

    #[test]
    fn test_todo_item_new() {
        let item = TodoItem::new("Test task");
        assert_eq!(item.get_content(), "Test task");
        assert_eq!(item.get_status(), TodoStatus::Pending);
    }

    #[test]
    fn test_todo_item_with_status() {
        let item = TodoItem::new("Test").status(TodoStatus::Completed);
        assert_eq!(item.get_status(), TodoStatus::Completed);
    }

    #[test]
    fn test_todo_item_active_form() {
        let item = TodoItem::new("Fix bug")
            .active_form("Fixing bug...")
            .status(TodoStatus::InProgress);

        // Should show active form when in progress
        assert_eq!(item.display_text(), "Fixing bug...");
    }

    #[test]
    fn test_todo_item_display_text_pending() {
        let item = TodoItem::new("Fix bug")
            .active_form("Fixing bug...")
            .status(TodoStatus::Pending);

        // Should show content when pending
        assert_eq!(item.display_text(), "Fix bug");
    }

    // ========== TodoPanel Tests ==========

    #[test]
    fn test_todo_panel_new() {
        let panel = TodoPanel::new();
        assert!(panel.is_expanded());
        assert_eq!(panel.total_count(), 0);
    }

    #[test]
    fn test_todo_panel_with_items() {
        let panel = TodoPanel::new().items(vec![
            TodoItem::new("Task 1").status(TodoStatus::Completed),
            TodoItem::new("Task 2").status(TodoStatus::InProgress),
            TodoItem::new("Task 3").status(TodoStatus::Pending),
        ]);

        assert_eq!(panel.total_count(), 3);
        assert_eq!(panel.completed_count(), 1);
        assert_eq!(panel.in_progress_count(), 1);
        assert_eq!(panel.pending_count(), 1);
    }

    #[test]
    fn test_todo_panel_next_pending() {
        let panel = TodoPanel::new().items(vec![
            TodoItem::new("Task 1").status(TodoStatus::Completed),
            TodoItem::new("Task 2").status(TodoStatus::Pending),
            TodoItem::new("Task 3").status(TodoStatus::Pending),
        ]);

        let next = panel.next_pending().unwrap();
        assert_eq!(next.get_content(), "Task 2");
    }

    #[test]
    fn test_todo_panel_current_item() {
        let panel = TodoPanel::new().items(vec![
            TodoItem::new("Task 1").status(TodoStatus::Completed),
            TodoItem::new("Task 2").status(TodoStatus::InProgress),
            TodoItem::new("Task 3").status(TodoStatus::Pending),
        ]);

        let current = panel.current_item().unwrap();
        assert_eq!(current.get_content(), "Task 2");
    }

    #[test]
    fn test_todo_panel_toggle() {
        let mut panel = TodoPanel::new();
        assert!(panel.is_expanded());

        panel.toggle();
        assert!(!panel.is_expanded());

        panel.toggle();
        assert!(panel.is_expanded());
    }

    #[test]
    fn test_todo_panel_to_node_expanded() {
        let panel = TodoPanel::new()
            .title("My Tasks")
            .items(vec![TodoItem::new("Test")]);
        let _node: Node = panel.into();
    }

    #[test]
    fn test_todo_panel_to_node_collapsed() {
        let panel = TodoPanel::new()
            .expanded(false)
            .items(vec![TodoItem::new("Test")]);
        let _node: Node = panel.into();
    }

    // ========== TodoBadge Tests ==========

    #[test]
    fn test_todo_badge_new() {
        let badge = TodoBadge::new();
        assert_eq!(badge.todo_count, 0);
        assert_eq!(badge.task_count, 0);
    }

    #[test]
    fn test_todo_badge_with_counts() {
        let badge = TodoBadge::new().todo_count(5).task_count(2);

        assert_eq!(badge.todo_count, 5);
        assert_eq!(badge.task_count, 2);
    }

    #[test]
    fn test_todo_badge_detailed() {
        let badge = TodoBadge::new()
            .todo_count(5)
            .in_progress(1)
            .completed(2)
            .detailed(true);

        assert_eq!(badge.in_progress, 1);
        assert_eq!(badge.completed, 2);
    }

    #[test]
    fn test_todo_badge_to_node() {
        let badge = TodoBadge::new().todo_count(3).task_count(1);
        let _node: Node = badge.into();
    }

    #[test]
    fn test_todo_badge_to_node_detailed() {
        let badge = TodoBadge::new()
            .todo_count(5)
            .in_progress(2)
            .completed(3)
            .detailed(true);
        let _node: Node = badge.into();
    }
}
