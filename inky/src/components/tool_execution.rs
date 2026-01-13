//! Tool execution widget for displaying tool call status.
//!
//! Provides a widget for displaying tool calls (Read, Write, Bash, etc.)
//! with status indicators, arguments, duration, and output preview.
//!
//! # Example
//!
//! ```
//! use inky::components::{ToolExecution, ToolStatus};
//! use std::time::Duration;
//!
//! // Running tool
//! let tool = ToolExecution::new("Read")
//!     .status(ToolStatus::Running)
//!     .args(vec!["src/main.rs"]);
//!
//! // Completed tool with output
//! let tool = ToolExecution::new("Bash")
//!     .status(ToolStatus::Success)
//!     .args(vec!["cargo build"])
//!     .duration(Duration::from_secs(12))
//!     .output_preview("Compiling claude-agent v0.1.0...");
//!
//! // Failed tool with error
//! let tool = ToolExecution::new("Write")
//!     .status(ToolStatus::Error)
//!     .args(vec!["/readonly/file.txt"])
//!     .error_message("Permission denied");
//! ```
//!
//! # Visual Output
//!
//! ```text
//! ⣾ Read src/main.rs
//! ✓ Bash cargo build (12s)
//!   └─ Compiling claude-agent v0.1.0...
//! ✗ Write /readonly/file.txt
//!   └─ Error: Permission denied
//! ```

use std::fmt::Write;
use std::time::Duration;

use crate::components::adaptive::{AdaptiveComponent, TierFeatures};
use crate::components::SpinnerStyle;
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

/// Status of a tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolStatus {
    /// Tool is currently running.
    #[default]
    Running,
    /// Tool completed successfully.
    Success,
    /// Tool failed with an error.
    Error,
}

impl ToolStatus {
    /// Get the color for this status.
    pub fn color(&self) -> Color {
        match self {
            ToolStatus::Running => Color::Blue,
            ToolStatus::Success => Color::Green,
            ToolStatus::Error => Color::Red,
        }
    }

    /// Get the Unicode indicator for this status.
    pub fn indicator(&self) -> &'static str {
        match self {
            ToolStatus::Running => "⣾", // Will be replaced by spinner
            ToolStatus::Success => "✓",
            ToolStatus::Error => "✗",
        }
    }

    /// Get the ASCII indicator for this status.
    pub fn ascii_indicator(&self) -> &'static str {
        match self {
            ToolStatus::Running => "*",
            ToolStatus::Success => "+",
            ToolStatus::Error => "!",
        }
    }
}

/// Tool execution display widget.
///
/// Shows a tool call with:
/// - Tool name and status indicator
/// - Arguments preview
/// - Duration (when completed)
/// - Output preview (collapsible)
/// - Error message (on failure)
///
/// # Example
///
/// ```
/// use inky::components::{ToolExecution, ToolStatus};
/// use std::time::Duration;
///
/// let tool = ToolExecution::new("Bash")
///     .status(ToolStatus::Success)
///     .args(vec!["cargo build"])
///     .duration(Duration::from_secs(12))
///     .output_preview("Compiling claude-agent v0.1.0...");
///
/// let node = tool.to_node();
/// ```
#[derive(Debug, Clone)]
pub struct ToolExecution {
    /// Tool name (e.g., "Read", "Write", "Bash").
    name: String,
    /// Current execution status.
    status: ToolStatus,
    /// Arguments passed to the tool.
    args: Vec<String>,
    /// Execution duration (None while running).
    duration: Option<Duration>,
    /// Output preview text.
    output_preview: Option<String>,
    /// Error message (for Error status).
    error_message: Option<String>,
    /// Whether the output is collapsed.
    collapsed: bool,
    /// Spinner frame for running state.
    spinner_frame: usize,
    /// Spinner style.
    spinner_style: SpinnerStyle,
    /// Maximum length for args display.
    args_max_len: usize,
    /// Maximum length for output preview.
    output_max_len: usize,
}

impl ToolExecution {
    /// Create a new tool execution display.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::ToolExecution;
    ///
    /// let tool = ToolExecution::new("Read");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: ToolStatus::default(),
            args: Vec::new(),
            duration: None,
            output_preview: None,
            error_message: None,
            collapsed: false,
            spinner_frame: 0,
            spinner_style: SpinnerStyle::Dots,
            args_max_len: 50,
            output_max_len: 80,
        }
    }

    /// Set the execution status.
    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the tool arguments.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::ToolExecution;
    ///
    /// let tool = ToolExecution::new("Read")
    ///     .args(vec!["src/main.rs", "src/lib.rs"]);
    /// ```
    pub fn args(mut self, args: Vec<impl Into<String>>) -> Self {
        self.args = args.into_iter().map(|a| a.into()).collect();
        self
    }

    /// Add a single argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Set the execution duration.
    ///
    /// Only displayed when status is Success or Error.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Set the output preview text.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::ToolExecution;
    ///
    /// let tool = ToolExecution::new("Bash")
    ///     .output_preview("Compiling claude-agent v0.1.0...");
    /// ```
    pub fn output_preview(mut self, preview: impl Into<String>) -> Self {
        self.output_preview = Some(preview.into());
        self
    }

    /// Set the error message (for Error status).
    pub fn error_message(mut self, msg: impl Into<String>) -> Self {
        self.error_message = Some(msg.into());
        self
    }

    /// Set whether the output is collapsed.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Set the spinner style for running state.
    pub fn spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.spinner_style = style;
        self
    }

    /// Set the maximum length for args display.
    pub fn args_max_len(mut self, len: usize) -> Self {
        self.args_max_len = len;
        self
    }

    /// Set the maximum length for output preview.
    pub fn output_max_len(mut self, len: usize) -> Self {
        self.output_max_len = len;
        self
    }

    /// Advance the spinner animation.
    pub fn tick(&mut self) {
        if self.status == ToolStatus::Running {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
    }

    /// Get the current status.
    pub fn current_status(&self) -> ToolStatus {
        self.status
    }

    /// Get the tool name.
    pub fn tool_name(&self) -> &str {
        &self.name
    }

    /// Format duration as a short string.
    fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m{}s", secs / 60, secs % 60)
        } else {
            format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Truncate a string with ellipsis if too long.
    fn truncate(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len.saturating_sub(3)])
        }
    }

    /// Format args for display.
    fn format_args(&self) -> String {
        let joined = self.args.join(" ");
        Self::truncate(&joined, self.args_max_len)
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let color = self.status.color();

        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);

        // Main line: indicator + tool name + args + duration
        let mut main_line = BoxNode::new().flex_direction(FlexDirection::Row);

        // Status indicator
        let indicator: Node = if self.status == ToolStatus::Running {
            let spinner_text = self.spinner_style.frame(self.spinner_frame);
            TextNode::new(spinner_text).color(color).into()
        } else {
            TextNode::new(self.status.indicator()).color(color).into()
        };
        main_line = main_line.child(indicator);
        main_line = main_line.child(TextNode::new(" "));

        // Tool name
        main_line = main_line.child(TextNode::new(&self.name).color(color));

        // Args
        if !self.args.is_empty() {
            main_line = main_line.child(TextNode::new(" "));
            main_line = main_line.child(TextNode::new(self.format_args()));
        }

        // Duration (only for completed tools)
        if let Some(duration) = self.duration {
            if self.status != ToolStatus::Running {
                main_line = main_line.child(TextNode::new(" ").color(Color::BrightBlack));
                main_line = main_line.child(
                    TextNode::new(format!("({})", Self::format_duration(duration)))
                        .color(Color::BrightBlack),
                );
            }
        }

        container = container.child(main_line);

        // Output preview (if not collapsed)
        if !self.collapsed {
            if let Some(ref preview) = self.output_preview {
                let mut output_line = BoxNode::new().flex_direction(FlexDirection::Row);
                output_line = output_line.child(TextNode::new("  └─ ").color(Color::BrightBlack));
                output_line = output_line
                    .child(TextNode::new(Self::truncate(preview, self.output_max_len)).dim());
                container = container.child(output_line);
            }

            // Error message
            if let Some(ref error) = self.error_message {
                if self.status == ToolStatus::Error {
                    let mut error_line = BoxNode::new().flex_direction(FlexDirection::Row);
                    error_line = error_line.child(TextNode::new("  └─ ").color(Color::BrightBlack));
                    error_line = error_line
                        .child(TextNode::new(format!("Error: {}", error)).color(Color::Red));
                    container = container.child(error_line);
                }
            }
        }

        container.into()
    }

    /// Render Tier 0: Plain text without formatting.
    fn render_tier0(&self) -> Node {
        let status_text = match self.status {
            ToolStatus::Running => "RUNNING",
            ToolStatus::Success => "SUCCESS",
            ToolStatus::Error => "ERROR",
        };

        let mut text = format!("[{}] {} {}", status_text, self.name, self.format_args());

        if let Some(duration) = self.duration {
            if self.status != ToolStatus::Running {
                let _ = write!(text, " ({})", Self::format_duration(duration));
            }
        }

        if !self.collapsed {
            if let Some(ref preview) = self.output_preview {
                let _ = write!(
                    text,
                    "\n  > {}",
                    Self::truncate(preview, self.output_max_len)
                );
            }
            if let Some(ref error) = self.error_message {
                if self.status == ToolStatus::Error {
                    let _ = write!(text, "\n  > Error: {}", error);
                }
            }
        }

        TextNode::new(text).into()
    }

    /// Render Tier 1: ASCII indicators.
    fn render_tier1(&self) -> Node {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);

        // Main line
        let mut main_line = BoxNode::new().flex_direction(FlexDirection::Row);

        // ASCII indicator
        let indicator = if self.status == ToolStatus::Running {
            const FRAMES: &[&str] = &["-", "\\", "|", "/"];
            format!("[{}]", FRAMES[self.spinner_frame % FRAMES.len()])
        } else {
            format!("[{}]", self.status.ascii_indicator())
        };
        main_line = main_line.child(TextNode::new(indicator));
        main_line = main_line.child(TextNode::new(" "));
        main_line = main_line.child(TextNode::new(&self.name));

        if !self.args.is_empty() {
            main_line = main_line.child(TextNode::new(" "));
            main_line = main_line.child(TextNode::new(self.format_args()));
        }

        if let Some(duration) = self.duration {
            if self.status != ToolStatus::Running {
                main_line = main_line.child(TextNode::new(format!(
                    " ({})",
                    Self::format_duration(duration)
                )));
            }
        }

        container = container.child(main_line);

        // Output
        if !self.collapsed {
            if let Some(ref preview) = self.output_preview {
                let mut output_line = BoxNode::new().flex_direction(FlexDirection::Row);
                output_line = output_line.child(TextNode::new("  +- "));
                output_line =
                    output_line.child(TextNode::new(Self::truncate(preview, self.output_max_len)));
                container = container.child(output_line);
            }

            if let Some(ref error) = self.error_message {
                if self.status == ToolStatus::Error {
                    let mut error_line = BoxNode::new().flex_direction(FlexDirection::Row);
                    error_line = error_line.child(TextNode::new("  +- Error: "));
                    error_line = error_line.child(TextNode::new(error));
                    container = container.child(error_line);
                }
            }
        }

        container.into()
    }
}

impl Default for ToolExecution {
    fn default() -> Self {
        Self::new("Tool")
    }
}

impl AdaptiveComponent for ToolExecution {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.to_node(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("ToolExecution")
            .tier0("Plain text with status prefix")
            .tier1("ASCII indicators with tree structure")
            .tier2("Unicode indicators with colors and spinners")
            .tier3("Full rendering with GPU acceleration")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None
    }
}

impl From<ToolExecution> for Node {
    fn from(tool: ToolExecution) -> Self {
        tool.to_node()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn node_to_text(node: &Node) -> String {
        match node {
            Node::Text(t) => t.content.to_string(),
            Node::Box(b) => {
                let mut result = String::new();
                for c in &b.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Root(r) => {
                let mut result = String::new();
                for c in &r.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Static(s) => {
                let mut result = String::new();
                for c in &s.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Custom(c) => {
                let mut result = String::new();
                for child in c.widget().children() {
                    result.push_str(&node_to_text(child));
                }
                result
            }
        }
    }

    #[test]
    fn test_tool_status_colors() {
        assert_eq!(ToolStatus::Running.color(), Color::Blue);
        assert_eq!(ToolStatus::Success.color(), Color::Green);
        assert_eq!(ToolStatus::Error.color(), Color::Red);
    }

    #[test]
    fn test_tool_status_indicators() {
        assert_eq!(ToolStatus::Success.indicator(), "✓");
        assert_eq!(ToolStatus::Error.indicator(), "✗");
    }

    #[test]
    fn test_tool_execution_new() {
        let tool = ToolExecution::new("Read");
        assert_eq!(tool.tool_name(), "Read");
        assert_eq!(tool.current_status(), ToolStatus::Running);
    }

    #[test]
    fn test_tool_execution_with_args() {
        let tool = ToolExecution::new("Read").args(vec!["src/main.rs", "src/lib.rs"]);

        let node = tool.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Read"));
        assert!(text.contains("src/main.rs"));
        assert!(text.contains("src/lib.rs"));
    }

    #[test]
    fn test_tool_execution_success_with_duration() {
        let tool = ToolExecution::new("Bash")
            .status(ToolStatus::Success)
            .args(vec!["cargo build"])
            .duration(Duration::from_secs(12));

        let node = tool.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("✓"));
        assert!(text.contains("Bash"));
        assert!(text.contains("cargo build"));
        assert!(text.contains("12s"));
    }

    #[test]
    fn test_tool_execution_error_with_message() {
        let tool = ToolExecution::new("Write")
            .status(ToolStatus::Error)
            .args(vec!["/readonly/file.txt"])
            .error_message("Permission denied");

        let node = tool.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("✗"));
        assert!(text.contains("Write"));
        assert!(text.contains("Permission denied"));
    }

    #[test]
    fn test_tool_execution_output_preview() {
        let tool = ToolExecution::new("Bash")
            .status(ToolStatus::Success)
            .args(vec!["cargo build"])
            .output_preview("Compiling claude-agent v0.1.0...");

        let node = tool.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Compiling claude-agent"));
        assert!(text.contains("└─"));
    }

    #[test]
    fn test_tool_execution_collapsed() {
        let tool = ToolExecution::new("Bash")
            .status(ToolStatus::Success)
            .output_preview("This should not appear")
            .collapsed(true);

        let node = tool.to_node();
        let text = node_to_text(&node);

        assert!(!text.contains("This should not appear"));
        assert!(!text.contains("└─"));
    }

    #[test]
    fn test_tool_execution_tick() {
        let mut tool = ToolExecution::new("Read").status(ToolStatus::Running);
        let frame1 = tool.spinner_frame;

        tool.tick();
        let frame2 = tool.spinner_frame;

        assert_eq!(frame2, frame1 + 1);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(ToolExecution::format_duration(Duration::from_secs(5)), "5s");
        assert_eq!(
            ToolExecution::format_duration(Duration::from_secs(65)),
            "1m5s"
        );
        assert_eq!(
            ToolExecution::format_duration(Duration::from_secs(3665)),
            "1h1m"
        );
    }

    #[test]
    fn test_truncate() {
        assert_eq!(ToolExecution::truncate("short", 10), "short");
        assert_eq!(
            ToolExecution::truncate("this is a very long string", 10),
            "this is..."
        );
    }

    #[test]
    fn test_adaptive_rendering() {
        let tool = ToolExecution::new("Read")
            .status(ToolStatus::Success)
            .args(vec!["file.txt"]);

        // All tiers should work
        let _tier0 = tool.render_for_tier(RenderTier::Tier0Fallback);
        let _tier1 = tool.render_for_tier(RenderTier::Tier1Ansi);
        let _tier2 = tool.render_for_tier(RenderTier::Tier2Retained);
        let _tier3 = tool.render_for_tier(RenderTier::Tier3Gpu);
    }

    #[test]
    fn test_tier0_format() {
        let tool = ToolExecution::new("Bash")
            .status(ToolStatus::Success)
            .args(vec!["ls"])
            .duration(Duration::from_secs(1));

        let node = tool.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("[SUCCESS]"));
        assert!(text.contains("Bash"));
        assert!(text.contains("ls"));
        assert!(text.contains("(1s)"));
    }

    #[test]
    fn test_tier1_ascii_indicators() {
        let tool = ToolExecution::new("Write").status(ToolStatus::Error);

        let node = tool.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("[!]"));
        assert!(text.contains("Write"));
    }
}
