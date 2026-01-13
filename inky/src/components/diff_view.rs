//! Diff view component for displaying code changes.
//!
//! Renders code diffs with syntax highlighting for additions, deletions, and context lines.
//! Supports line numbers, hunk separators, and file path headers.
//!
//! # Example
//!
//! ```
//! use inky::components::{DiffView, DiffLine, DiffLineKind};
//!
//! let view = DiffView::new()
//!     .file_path("src/main.rs")
//!     .line(DiffLine::context(1, "fn main() {"))
//!     .line(DiffLine::delete(2, "    println!(\"old\");"))
//!     .line(DiffLine::add(2, "    println!(\"new\");"))
//!     .line(DiffLine::context(3, "}"));
//! let node = view.to_node();
//! ```
//!
//! # Features
//!
//! - **Added lines** - Green `+` prefix for new lines
//! - **Deleted lines** - Red `-` prefix for removed lines
//! - **Context lines** - Dim styling for unchanged lines
//! - **Line numbers** - Right-aligned gutter with dim styling
//! - **Hunk separators** - Visual `⋮` separator between non-contiguous sections
//! - **File path header** - Optional file path display at the top
//! - **Summary line** - Shows total additions and deletions (+N/-M)

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

use super::adaptive::{AdaptiveComponent, Tier0Fallback, TierFeatures};

/// Type of diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Added line (+).
    Add,
    /// Deleted line (-).
    Delete,
    /// Context line (unchanged).
    Context,
    /// Hunk separator (⋮).
    HunkSeparator,
}

/// A single line in a diff.
///
/// Represents one line of a code diff with its kind (add/delete/context),
/// optional line number, and content.
///
/// # Example
///
/// ```
/// use inky::components::{DiffLine, DiffLineKind};
///
/// // Added line at line 42
/// let added = DiffLine::add(42, "    let x = 5;");
///
/// // Deleted line at line 10
/// let deleted = DiffLine::delete(10, "    let x = 4;");
///
/// // Context line (unchanged)
/// let context = DiffLine::context(9, "fn calculate() {");
///
/// // Hunk separator
/// let sep = DiffLine::hunk_separator();
/// ```
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// The type of diff line.
    pub kind: DiffLineKind,
    /// The line number (None for hunk separators).
    pub line_number: Option<usize>,
    /// The content of the line.
    pub content: String,
}

impl DiffLine {
    /// Create a new diff line with the given kind, line number, and content.
    pub fn new(kind: DiffLineKind, line_number: Option<usize>, content: impl Into<String>) -> Self {
        Self {
            kind,
            line_number,
            content: content.into(),
        }
    }

    /// Create an added line (+).
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DiffLine;
    ///
    /// let line = DiffLine::add(10, "    new code");
    /// ```
    pub fn add(line_number: usize, content: impl Into<String>) -> Self {
        Self::new(DiffLineKind::Add, Some(line_number), content)
    }

    /// Create a deleted line (-).
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DiffLine;
    ///
    /// let line = DiffLine::delete(5, "    old code");
    /// ```
    pub fn delete(line_number: usize, content: impl Into<String>) -> Self {
        Self::new(DiffLineKind::Delete, Some(line_number), content)
    }

    /// Create a context line (unchanged).
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DiffLine;
    ///
    /// let line = DiffLine::context(3, "fn main() {");
    /// ```
    pub fn context(line_number: usize, content: impl Into<String>) -> Self {
        Self::new(DiffLineKind::Context, Some(line_number), content)
    }

    /// Create a hunk separator (⋮).
    ///
    /// Used to indicate non-contiguous sections in a diff.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DiffLine;
    ///
    /// let sep = DiffLine::hunk_separator();
    /// ```
    pub fn hunk_separator() -> Self {
        Self::new(DiffLineKind::HunkSeparator, None, "")
    }
}

/// Diff view component displaying code changes.
///
/// Renders a code diff with proper styling for additions (green), deletions (red),
/// and context (dim). Includes line numbers in a gutter and optional file path header.
///
/// # Example
///
/// ```
/// use inky::components::{DiffView, DiffLine};
///
/// let view = DiffView::new()
///     .file_path("src/main.rs")
///     .line(DiffLine::context(1, "fn main() {"))
///     .line(DiffLine::delete(2, "    println!(\"old\");"))
///     .line(DiffLine::add(2, "    println!(\"new\");"))
///     .line(DiffLine::context(3, "}"));
///
/// // Get the node for rendering
/// let node = view.to_node();
/// ```
///
/// # With Hunk Separators
///
/// ```
/// use inky::components::{DiffView, DiffLine};
///
/// let view = DiffView::new()
///     .line(DiffLine::context(10, "// First section"))
///     .line(DiffLine::add(11, "// Added here"))
///     .line(DiffLine::hunk_separator())
///     .line(DiffLine::context(50, "// Later section"))
///     .line(DiffLine::delete(51, "// Removed here"));
/// ```
#[derive(Debug, Clone)]
pub struct DiffView {
    /// Optional file path to display at the top.
    file_path: Option<String>,
    /// The diff lines to display.
    lines: Vec<DiffLine>,
    /// Whether to show line numbers in the gutter.
    show_line_numbers: bool,
    /// Whether to show the summary line (+N/-M).
    show_summary: bool,
}

impl Default for DiffView {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffView {
    /// Create a new empty diff view.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DiffView;
    ///
    /// let view = DiffView::new();
    /// ```
    pub fn new() -> Self {
        Self {
            file_path: None,
            lines: Vec::new(),
            show_line_numbers: true,
            show_summary: true,
        }
    }

    /// Set the file path to display at the top of the diff.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DiffView;
    ///
    /// let view = DiffView::new()
    ///     .file_path("src/lib.rs");
    /// ```
    pub fn file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Add a single diff line.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{DiffView, DiffLine};
    ///
    /// let view = DiffView::new()
    ///     .line(DiffLine::add(1, "new line"));
    /// ```
    pub fn line(mut self, line: DiffLine) -> Self {
        self.lines.push(line);
        self
    }

    /// Add multiple diff lines at once.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{DiffView, DiffLine};
    ///
    /// let lines = vec![
    ///     DiffLine::context(1, "fn main() {"),
    ///     DiffLine::add(2, "    println!(\"hello\");"),
    ///     DiffLine::context(3, "}"),
    /// ];
    ///
    /// let view = DiffView::new().lines(lines);
    /// ```
    pub fn lines(mut self, lines: impl IntoIterator<Item = DiffLine>) -> Self {
        self.lines.extend(lines);
        self
    }

    /// Set whether to show line numbers in the gutter.
    ///
    /// Default is `true`.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DiffView;
    ///
    /// // Hide line numbers
    /// let view = DiffView::new()
    ///     .show_line_numbers(false);
    /// ```
    pub fn show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// Set whether to show the summary line (+N/-M).
    ///
    /// Default is `true`.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DiffView;
    ///
    /// // Hide summary
    /// let view = DiffView::new()
    ///     .show_summary(false);
    /// ```
    pub fn show_summary(mut self, show: bool) -> Self {
        self.show_summary = show;
        self
    }

    /// Count the number of added and deleted lines.
    fn count_changes(&self) -> (usize, usize) {
        let mut added = 0;
        let mut deleted = 0;
        for line in &self.lines {
            match line.kind {
                DiffLineKind::Add => added += 1,
                DiffLineKind::Delete => deleted += 1,
                DiffLineKind::Context | DiffLineKind::HunkSeparator => {}
            }
        }
        (added, deleted)
    }

    /// Calculate the width needed for line numbers.
    /// Uses integer math instead of String allocation.
    fn line_number_width(&self) -> usize {
        let max_line = self
            .lines
            .iter()
            .filter_map(|l| l.line_number)
            .max()
            .unwrap_or(0);

        if max_line == 0 {
            1
        } else {
            // Count digits using integer math instead of to_string().len()
            // This avoids allocating a String just to count characters
            (max_line as f64).log10().floor() as usize + 1
        }
    }

    /// Convert the diff view to an inky Node tree.
    ///
    /// Returns a [`BoxNode`] containing the rendered diff with appropriate
    /// styling for each line type.
    pub fn to_node(&self) -> Node {
        let mut root = BoxNode::new().flex_direction(FlexDirection::Column);

        // File path header
        if let Some(ref path) = self.file_path {
            let (added, deleted) = self.count_changes();
            let header = self.render_header(path, added, deleted);
            root = root.child(header);
        } else if self.show_summary {
            // Show summary even without file path
            let (added, deleted) = self.count_changes();
            if added > 0 || deleted > 0 {
                let summary = self.render_summary_line(added, deleted);
                root = root.child(summary);
            }
        }

        // Empty diff
        if self.lines.is_empty() {
            return root.into();
        }

        let ln_width = if self.show_line_numbers {
            self.line_number_width()
        } else {
            0
        };

        // Render each line
        for line in &self.lines {
            let line_node = self.render_line(line, ln_width);
            root = root.child(line_node);
        }

        root.into()
    }

    /// Render the file path header with summary.
    fn render_header(&self, path: &str, added: usize, deleted: usize) -> Node {
        let mut header = BoxNode::new().flex_direction(FlexDirection::Row);

        // File path
        header = header.child(TextNode::new(path).bold());

        // Summary (if enabled and there are changes)
        if self.show_summary && (added > 0 || deleted > 0) {
            header = header.child(TextNode::new(" ("));
            header = header.child(TextNode::new(format!("+{}", added)).color(Color::Green));
            header = header.child(TextNode::new(" "));
            header = header.child(TextNode::new(format!("-{}", deleted)).color(Color::Red));
            header = header.child(TextNode::new(")"));
        }

        header.into()
    }

    /// Render just a summary line without file path.
    fn render_summary_line(&self, added: usize, deleted: usize) -> Node {
        let mut line = BoxNode::new().flex_direction(FlexDirection::Row);
        line = line.child(TextNode::new("("));
        line = line.child(TextNode::new(format!("+{}", added)).color(Color::Green));
        line = line.child(TextNode::new(" "));
        line = line.child(TextNode::new(format!("-{}", deleted)).color(Color::Red));
        line = line.child(TextNode::new(")"));
        line.into()
    }

    /// Render a single diff line.
    fn render_line(&self, line: &DiffLine, ln_width: usize) -> Node {
        match line.kind {
            DiffLineKind::HunkSeparator => self.render_hunk_separator(ln_width),
            _ => self.render_content_line(line, ln_width),
        }
    }

    /// Render a hunk separator line.
    fn render_hunk_separator(&self, ln_width: usize) -> Node {
        let mut row = BoxNode::new().flex_direction(FlexDirection::Row);

        if self.show_line_numbers && ln_width > 0 {
            // Empty gutter space + separator character
            let gutter = format!("{:width$} ", "", width = ln_width);
            row = row.child(TextNode::new(gutter).dim());
        }

        row = row.child(TextNode::new("⋮").dim());
        row.into()
    }

    /// Render a content line (add/delete/context).
    fn render_content_line(&self, line: &DiffLine, ln_width: usize) -> Node {
        let mut row = BoxNode::new().flex_direction(FlexDirection::Row);

        // Line number gutter
        if self.show_line_numbers && ln_width > 0 {
            let ln_str = match line.line_number {
                Some(n) => format!("{:>width$} ", n, width = ln_width),
                None => format!("{:width$} ", "", width = ln_width),
            };
            row = row.child(TextNode::new(ln_str).dim());
        }

        // Sign and content
        let (sign, color, is_dim) = match line.kind {
            DiffLineKind::Add => ("+", Some(Color::Green), false),
            DiffLineKind::Delete => ("-", Some(Color::Red), false),
            DiffLineKind::Context => (" ", None, true),
            DiffLineKind::HunkSeparator => (" ", None, true),
        };

        let content = format!("{}{}", sign, line.content);
        let mut text_node = TextNode::new(content);

        if let Some(c) = color {
            text_node = text_node.color(c);
        }
        if is_dim {
            text_node = text_node.dim();
        }

        row = row.child(text_node);
        row.into()
    }
}

impl From<DiffView> for Node {
    fn from(view: DiffView) -> Self {
        view.to_node()
    }
}

impl AdaptiveComponent for DiffView {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.to_node(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("DiffView")
            .tier0("Text summary with +N/-M counts only")
            .tier1("Plain text diff with +/- prefixes, no colors")
            .tier2("Colored diff with green additions, red deletions")
            .tier3("Full color rendering with GPU acceleration")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl DiffView {
    /// Render Tier 0: Text-only summary.
    ///
    /// Shows file path (if set) and a summary of changes (+N/-M).
    /// No actual diff content is displayed.
    fn render_tier0(&self) -> Node {
        let (added, deleted) = self.count_changes();
        let context = self
            .lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Context)
            .count();

        let mut fallback = if let Some(ref path) = self.file_path {
            Tier0Fallback::new(path.clone())
        } else {
            Tier0Fallback::new("Diff")
        };

        fallback = fallback.stat("added", format!("+{}", added));
        fallback = fallback.stat("deleted", format!("-{}", deleted));
        fallback = fallback.stat("context", context.to_string());
        fallback = fallback.stat("total", self.lines.len().to_string());

        fallback.into()
    }

    /// Render Tier 1: Plain ASCII diff without colors.
    ///
    /// Shows the full diff with +/- prefixes but no ANSI colors or styling.
    /// Line numbers are shown if enabled.
    fn render_tier1(&self) -> Node {
        let mut root = BoxNode::new().flex_direction(FlexDirection::Column);

        // File path header (plain text)
        if let Some(ref path) = self.file_path {
            let (added, deleted) = self.count_changes();
            let header = if self.show_summary && (added > 0 || deleted > 0) {
                format!("{} (+{} -{})", path, added, deleted)
            } else {
                path.clone()
            };
            root = root.child(TextNode::new(header));
        } else if self.show_summary {
            let (added, deleted) = self.count_changes();
            if added > 0 || deleted > 0 {
                root = root.child(TextNode::new(format!("(+{} -{})", added, deleted)));
            }
        }

        // Empty diff
        if self.lines.is_empty() {
            return root.into();
        }

        let ln_width = if self.show_line_numbers {
            self.line_number_width()
        } else {
            0
        };

        // Render each line without colors
        for line in &self.lines {
            let line_node = self.render_line_tier1(line, ln_width);
            root = root.child(line_node);
        }

        root.into()
    }

    /// Render a single line for Tier 1 (no colors).
    fn render_line_tier1(&self, line: &DiffLine, ln_width: usize) -> Node {
        let mut row = BoxNode::new().flex_direction(FlexDirection::Row);

        // Line number gutter
        if self.show_line_numbers && ln_width > 0 {
            let ln_str = match line.line_number {
                Some(n) => format!("{:>width$} ", n, width = ln_width),
                None => format!("{:width$} ", "", width = ln_width),
            };
            row = row.child(TextNode::new(ln_str));
        }

        // Sign and content (no colors)
        let content = match line.kind {
            DiffLineKind::Add => format!("+{}", line.content),
            DiffLineKind::Delete => format!("-{}", line.content),
            DiffLineKind::Context => format!(" {}", line.content),
            DiffLineKind::HunkSeparator => "...".to_string(),
        };

        row = row.child(TextNode::new(content));
        row.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Helper to convert a node tree to a simple string representation.
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

    /// Helper to check if a text node with specific content has a color.
    fn find_text_with_color(node: &Node, content: &str, expected_color: Color) -> bool {
        match node {
            Node::Text(t) => {
                t.content.contains(content) && t.text_style.color == Some(expected_color)
            }
            Node::Box(b) => b
                .children
                .iter()
                .any(|c| find_text_with_color(c, content, expected_color)),
            Node::Root(r) => r
                .children
                .iter()
                .any(|c| find_text_with_color(c, content, expected_color)),
            Node::Static(s) => s
                .children
                .iter()
                .any(|c| find_text_with_color(c, content, expected_color)),
            Node::Custom(c) => c
                .widget()
                .children()
                .iter()
                .any(|child| find_text_with_color(child, content, expected_color)),
        }
    }

    /// Helper to check if a text node with specific content is dimmed.
    fn find_text_dimmed(node: &Node, content: &str) -> bool {
        match node {
            Node::Text(t) => t.content.contains(content) && t.text_style.dim,
            Node::Box(b) => b.children.iter().any(|c| find_text_dimmed(c, content)),
            Node::Root(r) => r.children.iter().any(|c| find_text_dimmed(c, content)),
            Node::Static(s) => s.children.iter().any(|c| find_text_dimmed(c, content)),
            Node::Custom(c) => c
                .widget()
                .children()
                .iter()
                .any(|child| find_text_dimmed(child, content)),
        }
    }

    #[test]
    fn test_empty_diff() {
        let view = DiffView::new();
        let node = view.to_node();
        // Should not panic, just return empty box
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_single_add_line() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::add(1, "new line"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should contain the + sign and content
        assert!(text.contains("+new line"));
        // Should have green color on add line
        assert!(find_text_with_color(&node, "+new line", Color::Green));
    }

    #[test]
    fn test_single_delete_line() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::delete(5, "old line"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should contain the - sign and content
        assert!(text.contains("-old line"));
        // Should have red color on delete line
        assert!(find_text_with_color(&node, "-old line", Color::Red));
    }

    #[test]
    fn test_context_lines() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::context(1, "unchanged"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should contain the space sign (context) and content
        assert!(text.contains(" unchanged"));
        // Context lines should be dimmed
        assert!(find_text_dimmed(&node, " unchanged"));
    }

    #[test]
    fn test_mixed_add_delete_context() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::context(1, "fn main() {"))
            .line(DiffLine::delete(2, "    old();"))
            .line(DiffLine::add(2, "    new();"))
            .line(DiffLine::context(3, "}"));
        let node = view.to_node();
        let text = node_to_text(&node);

        assert!(text.contains(" fn main() {"));
        assert!(text.contains("-    old();"));
        assert!(text.contains("+    new();"));
        assert!(text.contains(" }"));

        // Verify colors
        assert!(find_text_with_color(&node, "-    old();", Color::Red));
        assert!(find_text_with_color(&node, "+    new();", Color::Green));
    }

    #[test]
    fn test_line_number_alignment_single_digit() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::add(1, "line one"))
            .line(DiffLine::add(9, "line nine"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Line numbers should be present
        assert!(text.contains("1 "));
        assert!(text.contains("9 "));
    }

    #[test]
    fn test_line_number_alignment_double_digit() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::add(1, "first"))
            .line(DiffLine::add(10, "tenth"))
            .line(DiffLine::add(99, "ninety-nine"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Single digit should be right-aligned with space
        assert!(text.contains(" 1 "));
        assert!(text.contains("10 "));
        assert!(text.contains("99 "));
    }

    #[test]
    fn test_line_number_alignment_triple_digit() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::add(5, "five"))
            .line(DiffLine::add(50, "fifty"))
            .line(DiffLine::add(500, "five hundred"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // All should be right-aligned to 3 digits
        assert!(text.contains("  5 "));
        assert!(text.contains(" 50 "));
        assert!(text.contains("500 "));
    }

    #[test]
    fn test_hunk_separator() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::context(10, "before"))
            .line(DiffLine::hunk_separator())
            .line(DiffLine::context(50, "after"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should contain the hunk separator character
        assert!(text.contains("⋮"));
        // Separator should be dimmed
        assert!(find_text_dimmed(&node, "⋮"));
    }

    #[test]
    fn test_file_path_header() {
        let view = DiffView::new()
            .file_path("src/main.rs")
            .line(DiffLine::add(1, "new"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should contain the file path
        assert!(text.contains("src/main.rs"));
    }

    #[test]
    fn test_summary_with_file_path() {
        let view = DiffView::new()
            .file_path("test.rs")
            .line(DiffLine::add(1, "a"))
            .line(DiffLine::add(2, "b"))
            .line(DiffLine::delete(3, "c"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should show (+2 -1) summary
        assert!(text.contains("+2"));
        assert!(text.contains("-1"));
        // Summary colors
        assert!(find_text_with_color(&node, "+2", Color::Green));
        assert!(find_text_with_color(&node, "-1", Color::Red));
    }

    #[test]
    fn test_summary_without_file_path() {
        let view = DiffView::new()
            .show_summary(true)
            .line(DiffLine::add(1, "added"))
            .line(DiffLine::delete(2, "deleted"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should still show summary
        assert!(text.contains("+1"));
        assert!(text.contains("-1"));
    }

    #[test]
    fn test_hide_line_numbers() {
        let view = DiffView::new()
            .show_line_numbers(false)
            .show_summary(false)
            .line(DiffLine::add(42, "content"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should not contain line number
        assert!(!text.contains("42"));
        // Should still have content
        assert!(text.contains("+content"));
    }

    #[test]
    fn test_hide_summary() {
        let view = DiffView::new()
            .file_path("test.rs")
            .show_summary(false)
            .line(DiffLine::add(1, "a"))
            .line(DiffLine::delete(2, "b"));
        let node = view.to_node();
        let text = node_to_text(&node);

        // Should have file path but no summary
        assert!(text.contains("test.rs"));
        assert!(!text.contains("+1"));
        assert!(!text.contains("-1"));
    }

    #[test]
    fn test_diff_line_constructors() {
        // Test all constructor methods
        let add = DiffLine::add(1, "add");
        assert_eq!(add.kind, DiffLineKind::Add);
        assert_eq!(add.line_number, Some(1));
        assert_eq!(add.content, "add");

        let del = DiffLine::delete(2, "del");
        assert_eq!(del.kind, DiffLineKind::Delete);
        assert_eq!(del.line_number, Some(2));
        assert_eq!(del.content, "del");

        let ctx = DiffLine::context(3, "ctx");
        assert_eq!(ctx.kind, DiffLineKind::Context);
        assert_eq!(ctx.line_number, Some(3));
        assert_eq!(ctx.content, "ctx");

        let sep = DiffLine::hunk_separator();
        assert_eq!(sep.kind, DiffLineKind::HunkSeparator);
        assert_eq!(sep.line_number, None);
        assert_eq!(sep.content, "");
    }

    #[test]
    fn test_from_impl() {
        // Test the From<DiffView> impl for Node
        let view = DiffView::new().line(DiffLine::add(1, "test"));
        let node: Node = view.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_lines_method() {
        // Test adding multiple lines at once
        let lines = vec![
            DiffLine::add(1, "a"),
            DiffLine::add(2, "b"),
            DiffLine::add(3, "c"),
        ];
        let view = DiffView::new().show_summary(false).lines(lines);
        let node = view.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("+a"));
        assert!(text.contains("+b"));
        assert!(text.contains("+c"));
    }

    #[test]
    fn test_default_impl() {
        let view = DiffView::default();
        // Should be same as new()
        assert!(view.file_path.is_none());
        assert!(view.lines.is_empty());
        assert!(view.show_line_numbers);
        assert!(view.show_summary);
    }

    #[test]
    fn test_realistic_diff() {
        // Test a realistic code diff scenario
        let view = DiffView::new()
            .file_path("src/lib.rs")
            .line(DiffLine::context(10, "impl Foo {"))
            .line(DiffLine::context(11, "    fn bar(&self) -> u32 {"))
            .line(DiffLine::delete(12, "        self.value"))
            .line(DiffLine::add(12, "        self.value * 2"))
            .line(DiffLine::context(13, "    }"))
            .line(DiffLine::hunk_separator())
            .line(DiffLine::context(50, "    fn baz(&self) {"))
            .line(DiffLine::add(51, "        println!(\"debug\");"))
            .line(DiffLine::context(52, "    }"))
            .line(DiffLine::context(53, "}"));

        let node = view.to_node();
        let text = node_to_text(&node);

        // Verify file path
        assert!(text.contains("src/lib.rs"));
        // Verify summary (+2 -1)
        assert!(text.contains("+2"));
        assert!(text.contains("-1"));
        // Verify content
        assert!(text.contains("-        self.value"));
        assert!(text.contains("+        self.value * 2"));
        assert!(text.contains("⋮"));
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // AdaptiveComponent Tests
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    #[test]
    fn test_adaptive_tier0_summary_only() {
        let view = DiffView::new()
            .file_path("src/main.rs")
            .line(DiffLine::add(1, "new line"))
            .line(DiffLine::delete(2, "old line"))
            .line(DiffLine::context(3, "unchanged"));

        let node = view.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        // Tier 0 should show summary format
        assert!(text.contains("src/main.rs"));
        assert!(text.contains("added=+1"));
        assert!(text.contains("deleted=-1"));
        assert!(text.contains("context=1"));
        assert!(text.contains("total=3"));

        // Should NOT contain actual diff content
        assert!(!text.contains("new line"));
        assert!(!text.contains("old line"));
    }

    #[test]
    fn test_adaptive_tier0_no_file_path() {
        let view = DiffView::new()
            .line(DiffLine::add(1, "added"))
            .line(DiffLine::add(2, "added2"));

        let node = view.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        // Should use default label
        assert!(text.contains("[Diff]"));
        assert!(text.contains("added=+2"));
        assert!(text.contains("deleted=-0"));
    }

    #[test]
    fn test_adaptive_tier1_plain_text() {
        let view = DiffView::new()
            .file_path("test.rs")
            .line(DiffLine::context(1, "fn main() {"))
            .line(DiffLine::delete(2, "    old();"))
            .line(DiffLine::add(2, "    new();"))
            .line(DiffLine::context(3, "}"));

        let node = view.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        // Tier 1 should show full diff with +/- prefixes
        assert!(text.contains("test.rs"));
        assert!(text.contains("+1 -1")); // Summary
        assert!(text.contains(" fn main() {")); // Context with space prefix
        assert!(text.contains("-    old();")); // Deletion with - prefix
        assert!(text.contains("+    new();")); // Addition with + prefix
        assert!(text.contains(" }")); // Context with space prefix

        // Should NOT have any colors (no Color in text nodes)
        assert!(!find_text_with_color(&node, "+", Color::Green));
        assert!(!find_text_with_color(&node, "-", Color::Red));
    }

    #[test]
    fn test_adaptive_tier1_hunk_separator() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::context(1, "before"))
            .line(DiffLine::hunk_separator())
            .line(DiffLine::context(50, "after"));

        let node = view.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        // Tier 1 uses "..." for hunk separator (ASCII)
        assert!(text.contains("..."));
    }

    #[test]
    fn test_adaptive_tier2_full_colors() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::add(1, "new"))
            .line(DiffLine::delete(2, "old"));

        let node = view.render_for_tier(RenderTier::Tier2Retained);
        let text = node_to_text(&node);

        // Tier 2 should have colors
        assert!(text.contains("+new"));
        assert!(text.contains("-old"));
        assert!(find_text_with_color(&node, "+new", Color::Green));
        assert!(find_text_with_color(&node, "-old", Color::Red));
    }

    #[test]
    fn test_adaptive_tier3_same_as_tier2() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::add(1, "added"));

        let tier2_node = view.render_for_tier(RenderTier::Tier2Retained);
        let tier3_node = view.render_for_tier(RenderTier::Tier3Gpu);

        let tier2_text = node_to_text(&tier2_node);
        let tier3_text = node_to_text(&tier3_node);

        // Tier 3 renders the same as Tier 2
        assert_eq!(tier2_text, tier3_text);
    }

    #[test]
    fn test_adaptive_tier_features() {
        let view = DiffView::new();
        let features = view.tier_features();

        assert_eq!(features.name, Some("DiffView"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
    }

    #[test]
    fn test_adaptive_minimum_tier_none() {
        let view = DiffView::new();

        // DiffView works at all tiers
        assert!(view.minimum_tier().is_none());
        assert!(view.supports_tier(RenderTier::Tier0Fallback));
        assert!(view.supports_tier(RenderTier::Tier1Ansi));
        assert!(view.supports_tier(RenderTier::Tier2Retained));
        assert!(view.supports_tier(RenderTier::Tier3Gpu));
    }

    #[test]
    fn test_adaptive_tier1_line_numbers() {
        let view = DiffView::new()
            .show_summary(false)
            .line(DiffLine::add(1, "one"))
            .line(DiffLine::add(99, "ninety-nine"));

        let node = view.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        // Line numbers should be present and right-aligned
        assert!(text.contains(" 1 "));
        assert!(text.contains("99 "));
    }

    #[test]
    fn test_adaptive_tier1_no_line_numbers() {
        let view = DiffView::new()
            .show_line_numbers(false)
            .show_summary(false)
            .line(DiffLine::add(42, "content"));

        let node = view.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        // Should not contain line number
        assert!(!text.contains("42"));
        // Should still have content
        assert!(text.contains("+content"));
    }
}
