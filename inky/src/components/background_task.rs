//! Background task indicator widget.
//!
//! Provides widgets for displaying background task status with shell-style
//! conventions (& prefix).
//!
//! # Example
//!
//! ```
//! use inky::components::{BackgroundTaskList, BackgroundTask, BackgroundTaskStatus};
//!
//! let indicator = BackgroundTaskList::new()
//!     .tasks(vec![
//!         BackgroundTask::new("cargo build").status(BackgroundTaskStatus::Running),
//!         BackgroundTask::new("npm install").status(BackgroundTaskStatus::Completed),
//!     ]);
//! ```
//!
//! # Visual Output
//!
//! Expanded:
//! ```text
//! & 2 background tasks
//!   └─ cargo build (running)
//!   └─ npm install (done)
//! ```
//!
//! Badge (status bar):
//! ```text
//! &2
//! ```

use crate::components::SpinnerStyle;
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};

/// Status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundTaskStatus {
    /// Task is waiting to start.
    #[default]
    Pending,
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
}

impl BackgroundTaskStatus {
    /// Get the display label for this status.
    pub fn label(&self) -> &'static str {
        match self {
            BackgroundTaskStatus::Pending => "pending",
            BackgroundTaskStatus::Running => "running",
            BackgroundTaskStatus::Completed => "done",
            BackgroundTaskStatus::Failed => "failed",
        }
    }

    /// Get the color for this status.
    pub fn color(&self) -> Color {
        match self {
            BackgroundTaskStatus::Pending => Color::BrightBlack,
            BackgroundTaskStatus::Running => Color::Blue,
            BackgroundTaskStatus::Completed => Color::Green,
            BackgroundTaskStatus::Failed => Color::Red,
        }
    }
}

/// A background task.
#[derive(Debug, Clone)]
pub struct BackgroundTask {
    /// Task name/command.
    name: String,
    /// Current status.
    status: BackgroundTaskStatus,
    /// Optional output preview.
    output_preview: Option<String>,
    /// Task ID (for tracking).
    id: Option<String>,
}

impl BackgroundTask {
    /// Create a new background task.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::BackgroundTask;
    ///
    /// let task = BackgroundTask::new("cargo build");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: BackgroundTaskStatus::Pending,
            output_preview: None,
            id: None,
        }
    }

    /// Set the task status.
    pub fn status(mut self, status: BackgroundTaskStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the output preview.
    pub fn output_preview(mut self, preview: impl Into<String>) -> Self {
        self.output_preview = Some(preview.into());
        self
    }

    /// Set the task ID.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Get the task name.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get the task status.
    pub fn get_status(&self) -> BackgroundTaskStatus {
        self.status
    }

    /// Get the task ID.
    pub fn get_id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Check if task is running.
    pub fn is_running(&self) -> bool {
        self.status == BackgroundTaskStatus::Running
    }

    /// Check if task is completed.
    pub fn is_completed(&self) -> bool {
        matches!(
            self.status,
            BackgroundTaskStatus::Completed | BackgroundTaskStatus::Failed
        )
    }
}

/// A list of background tasks.
///
/// # Example
///
/// ```
/// use inky::components::{BackgroundTaskList, BackgroundTask, BackgroundTaskStatus};
///
/// let list = BackgroundTaskList::new()
///     .tasks(vec![
///         BackgroundTask::new("cargo build").status(BackgroundTaskStatus::Running),
///         BackgroundTask::new("npm install").status(BackgroundTaskStatus::Completed),
///     ]);
/// ```
#[derive(Debug, Clone)]
pub struct BackgroundTaskList {
    /// List of tasks.
    tasks: Vec<BackgroundTask>,
    /// Whether to show expanded view.
    expanded: bool,
    /// Maximum tasks to show when expanded.
    max_visible: usize,
    /// Shell-style prefix.
    prefix: String,
    /// Spinner frame for running tasks.
    spinner_frame: usize,
    /// Spinner style.
    spinner_style: SpinnerStyle,
    /// Whether to show output previews.
    show_output: bool,
    /// Maximum output preview length.
    output_max_len: usize,
}

impl BackgroundTaskList {
    /// Create a new background task list.
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            expanded: true,
            max_visible: 5,
            prefix: "&".to_string(),
            spinner_frame: 0,
            spinner_style: SpinnerStyle::Dots,
            show_output: true,
            output_max_len: 50,
        }
    }

    /// Set the tasks.
    pub fn tasks(mut self, tasks: Vec<BackgroundTask>) -> Self {
        self.tasks = tasks;
        self
    }

    /// Add a task.
    pub fn task(mut self, task: BackgroundTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Set whether expanded.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Set maximum visible tasks.
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = max;
        self
    }

    /// Set the prefix (default: "&").
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set spinner style.
    pub fn spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.spinner_style = style;
        self
    }

    /// Set whether to show output previews.
    pub fn show_output(mut self, show: bool) -> Self {
        self.show_output = show;
        self
    }

    /// Set maximum output preview length.
    pub fn output_max_len(mut self, len: usize) -> Self {
        self.output_max_len = len;
        self
    }

    /// Toggle expanded state.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Advance the spinner animation.
    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    /// Get count of running tasks.
    pub fn running_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.is_running()).count()
    }

    /// Get count of completed tasks.
    pub fn completed_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.is_completed()).count()
    }

    /// Get total task count.
    pub fn total_count(&self) -> usize {
        self.tasks.len()
    }

    /// Get tasks.
    pub fn get_tasks(&self) -> &[BackgroundTask] {
        &self.tasks
    }

    /// Truncate a string with ellipsis if too long.
    fn truncate(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len.saturating_sub(3)])
        }
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);

        // Header line
        let mut header = BoxNode::new().flex_direction(FlexDirection::Row);

        // Prefix
        header = header.child(TextNode::new(&self.prefix).color(Color::BrightBlack));
        header = header.child(TextNode::new(" "));

        // Count
        let running = self.running_count();
        if running > 0 {
            header = header.child(
                TextNode::new(format!(
                    "{} background {}",
                    running,
                    if running == 1 { "task" } else { "tasks" }
                ))
                .dim(),
            );

            // Spinner for running state
            header = header.child(TextNode::new(" "));
            header = header.child(
                TextNode::new(self.spinner_style.frame(self.spinner_frame)).color(Color::Blue),
            );
        } else if self.total_count() > 0 {
            header = header.child(TextNode::new("no running tasks").dim());
        } else {
            header = header.child(TextNode::new("no background tasks").dim());
        }

        container = container.child(header);

        // Task list (when expanded)
        if self.expanded && !self.tasks.is_empty() {
            for task in self.tasks.iter().take(self.max_visible) {
                let mut task_row = BoxNode::new().flex_direction(FlexDirection::Row);

                // Tree connector
                task_row = task_row.child(TextNode::new("  └─ ").color(Color::BrightBlack));

                // Task name
                task_row = task_row.child(TextNode::new(task.get_name()).dim());

                // Status
                task_row = task_row.child(TextNode::new(" (").color(Color::BrightBlack));
                task_row =
                    task_row.child(TextNode::new(task.status.label()).color(task.status.color()));
                task_row = task_row.child(TextNode::new(")").color(Color::BrightBlack));

                container = container.child(task_row);

                // Output preview
                if self.show_output {
                    if let Some(ref output) = task.output_preview {
                        let mut output_row = BoxNode::new().flex_direction(FlexDirection::Row);
                        output_row =
                            output_row.child(TextNode::new("     ").color(Color::BrightBlack));
                        output_row = output_row.child(
                            TextNode::new(Self::truncate(output, self.output_max_len)).dim(),
                        );
                        container = container.child(output_row);
                    }
                }
            }

            // "More tasks" indicator
            if self.tasks.len() > self.max_visible {
                container = container.child(
                    TextNode::new(format!(
                        "  ... {} more",
                        self.tasks.len() - self.max_visible
                    ))
                    .dim(),
                );
            }
        }

        container.into()
    }
}

impl Default for BackgroundTaskList {
    fn default() -> Self {
        Self::new()
    }
}

impl From<BackgroundTaskList> for Node {
    fn from(list: BackgroundTaskList) -> Self {
        list.to_node()
    }
}

/// A compact badge for background task count.
///
/// # Example
///
/// ```
/// use inky::components::BackgroundTaskBadge;
///
/// let badge = BackgroundTaskBadge::new().count(3);
/// ```
#[derive(Debug, Clone)]
pub struct BackgroundTaskBadge {
    /// Number of running background tasks.
    count: usize,
    /// Prefix character (default: "&").
    prefix: String,
    /// Badge color.
    color: Color,
    /// Spinner frame.
    spinner_frame: usize,
    /// Spinner style.
    spinner_style: SpinnerStyle,
    /// Whether to show spinner when count > 0.
    show_spinner: bool,
}

impl BackgroundTaskBadge {
    /// Create a new background task badge.
    pub fn new() -> Self {
        Self {
            count: 0,
            prefix: "&".to_string(),
            color: Color::BrightBlack,
            spinner_frame: 0,
            spinner_style: SpinnerStyle::Dots,
            show_spinner: true,
        }
    }

    /// Set the task count.
    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// Set the prefix.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set the badge color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set spinner style.
    pub fn spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.spinner_style = style;
        self
    }

    /// Set whether to show spinner.
    pub fn show_spinner(mut self, show: bool) -> Self {
        self.show_spinner = show;
        self
    }

    /// Advance the spinner animation.
    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    /// Get the current count.
    pub fn get_count(&self) -> usize {
        self.count
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let mut row = BoxNode::new().flex_direction(FlexDirection::Row);

        if self.count == 0 {
            // Empty state - just show prefix dimmed
            row = row.child(TextNode::new(&self.prefix).dim());
        } else {
            // Show prefix + count
            row = row
                .child(TextNode::new(format!("{}{}", self.prefix, self.count)).color(self.color));

            // Optional spinner
            if self.show_spinner {
                row = row.child(TextNode::new(" "));
                row = row.child(
                    TextNode::new(self.spinner_style.frame(self.spinner_frame)).color(Color::Blue),
                );
            }
        }

        row.into()
    }
}

impl Default for BackgroundTaskBadge {
    fn default() -> Self {
        Self::new()
    }
}

impl From<BackgroundTaskBadge> for Node {
    fn from(badge: BackgroundTaskBadge) -> Self {
        badge.to_node()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ========== BackgroundTaskStatus Tests ==========

    #[test]
    fn test_status_labels() {
        assert_eq!(BackgroundTaskStatus::Pending.label(), "pending");
        assert_eq!(BackgroundTaskStatus::Running.label(), "running");
        assert_eq!(BackgroundTaskStatus::Completed.label(), "done");
        assert_eq!(BackgroundTaskStatus::Failed.label(), "failed");
    }

    #[test]
    fn test_status_colors() {
        assert_eq!(BackgroundTaskStatus::Pending.color(), Color::BrightBlack);
        assert_eq!(BackgroundTaskStatus::Running.color(), Color::Blue);
        assert_eq!(BackgroundTaskStatus::Completed.color(), Color::Green);
        assert_eq!(BackgroundTaskStatus::Failed.color(), Color::Red);
    }

    // ========== BackgroundTask Tests ==========

    #[test]
    fn test_task_new() {
        let task = BackgroundTask::new("cargo build");
        assert_eq!(task.get_name(), "cargo build");
        assert_eq!(task.get_status(), BackgroundTaskStatus::Pending);
    }

    #[test]
    fn test_task_with_status() {
        let task = BackgroundTask::new("test").status(BackgroundTaskStatus::Running);
        assert!(task.is_running());
    }

    #[test]
    fn test_task_is_completed() {
        let completed = BackgroundTask::new("test").status(BackgroundTaskStatus::Completed);
        let failed = BackgroundTask::new("test").status(BackgroundTaskStatus::Failed);
        let running = BackgroundTask::new("test").status(BackgroundTaskStatus::Running);

        assert!(completed.is_completed());
        assert!(failed.is_completed());
        assert!(!running.is_completed());
    }

    #[test]
    fn test_task_with_output() {
        let task = BackgroundTask::new("test").output_preview("Some output");
        assert_eq!(task.output_preview, Some("Some output".to_string()));
    }

    #[test]
    fn test_task_with_id() {
        let task = BackgroundTask::new("test").id("task-123");
        assert_eq!(task.get_id(), Some("task-123"));
    }

    // ========== BackgroundTaskList Tests ==========

    #[test]
    fn test_list_new() {
        let list = BackgroundTaskList::new();
        assert_eq!(list.total_count(), 0);
        assert_eq!(list.running_count(), 0);
    }

    #[test]
    fn test_list_with_tasks() {
        let list = BackgroundTaskList::new().tasks(vec![
            BackgroundTask::new("task1").status(BackgroundTaskStatus::Running),
            BackgroundTask::new("task2").status(BackgroundTaskStatus::Running),
            BackgroundTask::new("task3").status(BackgroundTaskStatus::Completed),
        ]);

        assert_eq!(list.total_count(), 3);
        assert_eq!(list.running_count(), 2);
        assert_eq!(list.completed_count(), 1);
    }

    #[test]
    fn test_list_toggle() {
        let mut list = BackgroundTaskList::new();
        assert!(list.expanded);

        list.toggle();
        assert!(!list.expanded);
    }

    #[test]
    fn test_list_tick() {
        let mut list = BackgroundTaskList::new();
        let frame1 = list.spinner_frame;

        list.tick();
        assert_eq!(list.spinner_frame, frame1 + 1);
    }

    #[test]
    fn test_list_to_node() {
        let list = BackgroundTaskList::new().tasks(vec![
            BackgroundTask::new("cargo build").status(BackgroundTaskStatus::Running)
        ]);
        let _node: Node = list.into();
    }

    #[test]
    fn test_list_to_node_empty() {
        let list = BackgroundTaskList::new();
        let _node: Node = list.into();
    }

    #[test]
    fn test_truncate() {
        assert_eq!(BackgroundTaskList::truncate("short", 10), "short");
        assert_eq!(
            BackgroundTaskList::truncate("this is very long", 10),
            "this is..."
        );
    }

    // ========== BackgroundTaskBadge Tests ==========

    #[test]
    fn test_badge_new() {
        let badge = BackgroundTaskBadge::new();
        assert_eq!(badge.get_count(), 0);
    }

    #[test]
    fn test_badge_with_count() {
        let badge = BackgroundTaskBadge::new().count(5);
        assert_eq!(badge.get_count(), 5);
    }

    #[test]
    fn test_badge_tick() {
        let mut badge = BackgroundTaskBadge::new();
        let frame1 = badge.spinner_frame;

        badge.tick();
        assert_eq!(badge.spinner_frame, frame1 + 1);
    }

    #[test]
    fn test_badge_to_node() {
        let badge = BackgroundTaskBadge::new().count(3);
        let _node: Node = badge.into();
    }

    #[test]
    fn test_badge_to_node_empty() {
        let badge = BackgroundTaskBadge::new();
        let _node: Node = badge.into();
    }
}
