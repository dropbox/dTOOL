//! Thinking block widget for displaying Claude's reasoning process.
//!
//! Provides a collapsible widget for displaying "Thinking..." blocks that show
//! the model's internal reasoning process.
//!
//! # Example
//!
//! ```
//! use inky::components::ThinkingBlock;
//!
//! // Collapsed thinking block
//! let block = ThinkingBlock::new()
//!     .content("Let me analyze this problem...")
//!     .expanded(false);
//!
//! // Streaming thinking block (while content arrives)
//! let block = ThinkingBlock::new()
//!     .content("Analyzing the code structure")
//!     .streaming(true)
//!     .expanded(true);
//! ```
//!
//! # Visual Output
//!
//! Collapsed:
//! ```text
//! ▶ Thinking... (245 chars)
//! ```
//!
//! Expanded:
//! ```text
//! ▼ Thinking...
//!   Let me analyze this problem. First, I need to understand
//!   the requirements. The user wants to...
//! ```

use std::fmt::Write;

use crate::components::adaptive::{AdaptiveComponent, TierFeatures};
use crate::components::SpinnerStyle;
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

/// A collapsible block for displaying Claude's thinking/reasoning process.
///
/// The thinking block shows:
/// - Expand/collapse indicator (▶/▼)
/// - "Thinking..." header (optionally in italics)
/// - Character count when collapsed
/// - Dimmed content when expanded
/// - Streaming cursor while content is arriving
///
/// # Example
///
/// ```
/// use inky::components::ThinkingBlock;
///
/// let block = ThinkingBlock::new()
///     .content("Let me think about this...")
///     .expanded(true)
///     .streaming(false);
///
/// let node = block.to_node();
/// ```
#[derive(Debug, Clone)]
pub struct ThinkingBlock {
    /// The thinking content.
    content: String,
    /// Whether the block is expanded (showing content).
    expanded: bool,
    /// Whether content is still streaming.
    streaming: bool,
    /// Current spinner frame for streaming indicator.
    spinner_frame: usize,
    /// Spinner style for streaming indicator.
    spinner_style: SpinnerStyle,
    /// Maximum preview length when collapsed with preview enabled.
    preview_max_len: usize,
    /// Whether to show a preview when collapsed.
    show_preview: bool,
    /// Custom label (default: "Thinking...").
    label: String,
    /// Indent string for content lines.
    indent: String,
}

impl ThinkingBlock {
    /// Create a new thinking block.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::ThinkingBlock;
    ///
    /// let block = ThinkingBlock::new();
    /// ```
    pub fn new() -> Self {
        Self {
            content: String::new(),
            expanded: true,
            streaming: false,
            spinner_frame: 0,
            spinner_style: SpinnerStyle::Dots,
            preview_max_len: 40,
            show_preview: false,
            label: "Thinking...".to_string(),
            indent: "  ".to_string(),
        }
    }

    /// Set the thinking content.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::ThinkingBlock;
    ///
    /// let block = ThinkingBlock::new()
    ///     .content("Let me analyze this problem...");
    /// ```
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Set whether the block is expanded.
    ///
    /// When collapsed, only the header and character count are shown.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Set whether content is still streaming.
    ///
    /// When streaming, a cursor/spinner is shown at the end of the content.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Set the spinner style for the streaming indicator.
    pub fn spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.spinner_style = style;
        self
    }

    /// Set the maximum preview length when collapsed.
    pub fn preview_max_len(mut self, len: usize) -> Self {
        self.preview_max_len = len;
        self
    }

    /// Set whether to show a preview when collapsed.
    ///
    /// If enabled, shows a truncated preview of the content instead of just
    /// the character count.
    pub fn show_preview(mut self, show: bool) -> Self {
        self.show_preview = show;
        self
    }

    /// Set a custom label (default: "Thinking...").
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the indent string for content lines.
    pub fn indent(mut self, indent: impl Into<String>) -> Self {
        self.indent = indent.into();
        self
    }

    /// Advance the spinner animation.
    pub fn tick(&mut self) {
        if self.streaming {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
    }

    /// Toggle the expanded state.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Check if the block is expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Check if content is streaming.
    pub fn is_streaming(&self) -> bool {
        self.streaming
    }

    /// Get the content.
    pub fn get_content(&self) -> &str {
        &self.content
    }

    /// Get the content length (character count).
    pub fn content_len(&self) -> usize {
        self.content.chars().count()
    }

    /// Truncate a string with ellipsis if too long.
    fn truncate(s: &str, max_len: usize) -> String {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() <= max_len {
            s.to_string()
        } else {
            let truncated: String = chars[..max_len.saturating_sub(3)].iter().collect();
            format!("{}...", truncated)
        }
    }

    /// Get the expand/collapse indicator.
    fn indicator(&self) -> &'static str {
        if self.expanded {
            "▼"
        } else {
            "▶"
        }
    }

    /// Get the ASCII expand/collapse indicator.
    fn ascii_indicator(&self) -> &'static str {
        if self.expanded {
            "v"
        } else {
            ">"
        }
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);

        // Header line: indicator + label + (char count or streaming indicator)
        let mut header = BoxNode::new().flex_direction(FlexDirection::Row);

        // Expand/collapse indicator
        header = header.child(TextNode::new(self.indicator()).color(Color::BrightBlack));
        header = header.child(TextNode::new(" "));

        // Label in italic
        header = header.child(TextNode::new(&self.label).italic().dim());

        // Streaming indicator or char count
        if self.streaming && self.expanded {
            header = header.child(TextNode::new(" "));
            let spinner_text = self.spinner_style.frame(self.spinner_frame);
            header = header.child(TextNode::new(spinner_text).color(Color::Blue));
        } else if !self.expanded {
            // Show char count when collapsed
            let count_text = format!(" ({} chars)", self.content_len());
            header = header.child(TextNode::new(count_text).color(Color::BrightBlack));
        }

        container = container.child(header);

        // Content (only when expanded)
        if self.expanded && !self.content.is_empty() {
            // Split content into lines and render each with indentation
            for line in self.content.lines() {
                let mut content_line = BoxNode::new().flex_direction(FlexDirection::Row);
                content_line = content_line.child(TextNode::new(&self.indent));
                content_line = content_line.child(TextNode::new(line).dim());
                container = container.child(content_line);
            }

            // If streaming, add cursor at end
            if self.streaming {
                let mut cursor_line = BoxNode::new().flex_direction(FlexDirection::Row);
                cursor_line = cursor_line.child(TextNode::new(&self.indent));
                cursor_line = cursor_line.child(TextNode::new("▌").color(Color::Blue));
                container = container.child(cursor_line);
            }
        } else if !self.expanded && self.show_preview && !self.content.is_empty() {
            // Show preview when collapsed with show_preview enabled
            let first_line = self.content.lines().next().unwrap_or("");
            let preview = Self::truncate(first_line, self.preview_max_len);
            let mut preview_line = BoxNode::new().flex_direction(FlexDirection::Row);
            preview_line = preview_line.child(TextNode::new(&self.indent));
            preview_line = preview_line.child(TextNode::new(preview).dim());
            container = container.child(preview_line);
        }

        container.into()
    }

    /// Render Tier 0: Plain text without formatting.
    fn render_tier0(&self) -> Node {
        let indicator = if self.expanded { "[-]" } else { "[+]" };
        let mut text = format!("{} {}", indicator, self.label);

        if self.streaming && self.expanded {
            text.push_str(" [streaming]");
        } else if !self.expanded {
            let _ = write!(text, " ({} chars)", self.content_len());
        }

        if self.expanded && !self.content.is_empty() {
            for line in self.content.lines() {
                let _ = write!(text, "\n  {}", line);
            }
            if self.streaming {
                text.push_str("\n  _");
            }
        }

        TextNode::new(text).into()
    }

    /// Render Tier 1: ASCII indicators with basic structure.
    fn render_tier1(&self) -> Node {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);

        // Header
        let mut header = BoxNode::new().flex_direction(FlexDirection::Row);
        header = header.child(TextNode::new(self.ascii_indicator()));
        header = header.child(TextNode::new(" "));
        header = header.child(TextNode::new(&self.label));

        if self.streaming && self.expanded {
            header = header.child(TextNode::new(" *"));
        } else if !self.expanded {
            header = header.child(TextNode::new(format!(" ({} chars)", self.content_len())));
        }

        container = container.child(header);

        // Content
        if self.expanded && !self.content.is_empty() {
            for line in self.content.lines() {
                let mut content_line = BoxNode::new().flex_direction(FlexDirection::Row);
                content_line = content_line.child(TextNode::new("  "));
                content_line = content_line.child(TextNode::new(line));
                container = container.child(content_line);
            }
            if self.streaming {
                let mut cursor_line = BoxNode::new().flex_direction(FlexDirection::Row);
                cursor_line = cursor_line.child(TextNode::new("  _"));
                container = container.child(cursor_line);
            }
        }

        container.into()
    }
}

impl Default for ThinkingBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveComponent for ThinkingBlock {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.to_node(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("ThinkingBlock")
            .tier0("Plain text with brackets for expand/collapse")
            .tier1("ASCII indicators with indentation")
            .tier2("Unicode indicators with italic label and dimmed content")
            .tier3("Full rendering with GPU acceleration")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None
    }
}

impl From<ThinkingBlock> for Node {
    fn from(block: ThinkingBlock) -> Self {
        block.to_node()
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
    fn test_thinking_block_new() {
        let block = ThinkingBlock::new();
        assert!(block.is_expanded());
        assert!(!block.is_streaming());
        assert!(block.get_content().is_empty());
    }

    #[test]
    fn test_thinking_block_with_content() {
        let block = ThinkingBlock::new().content("Let me analyze this...");

        assert_eq!(block.get_content(), "Let me analyze this...");
        assert_eq!(block.content_len(), 22);
    }

    #[test]
    fn test_thinking_block_collapsed() {
        let block = ThinkingBlock::new()
            .content("Some thinking content here")
            .expanded(false);

        let node = block.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("▶"));
        assert!(text.contains("Thinking..."));
        assert!(text.contains("chars"));
        assert!(!text.contains("Some thinking content"));
    }

    #[test]
    fn test_thinking_block_expanded() {
        let block = ThinkingBlock::new()
            .content("Some thinking content here")
            .expanded(true);

        let node = block.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("▼"));
        assert!(text.contains("Thinking..."));
        assert!(text.contains("Some thinking content here"));
        assert!(!text.contains("chars"));
    }

    #[test]
    fn test_thinking_block_streaming() {
        let block = ThinkingBlock::new()
            .content("Analyzing...")
            .streaming(true)
            .expanded(true);

        let node = block.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("▼"));
        // Should have streaming cursor
        assert!(text.contains("▌"));
    }

    #[test]
    fn test_thinking_block_toggle() {
        let mut block = ThinkingBlock::new();
        assert!(block.is_expanded());

        block.toggle();
        assert!(!block.is_expanded());

        block.toggle();
        assert!(block.is_expanded());
    }

    #[test]
    fn test_thinking_block_tick() {
        let mut block = ThinkingBlock::new().streaming(true);
        let frame1 = block.spinner_frame;

        block.tick();
        let frame2 = block.spinner_frame;

        assert_eq!(frame2, frame1 + 1);
    }

    #[test]
    fn test_thinking_block_custom_label() {
        let block = ThinkingBlock::new().label("Reasoning...").expanded(true);

        let node = block.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Reasoning..."));
        assert!(!text.contains("Thinking..."));
    }

    #[test]
    fn test_thinking_block_multiline_content() {
        let block = ThinkingBlock::new()
            .content("Line 1\nLine 2\nLine 3")
            .expanded(true);

        let node = block.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Line 1"));
        assert!(text.contains("Line 2"));
        assert!(text.contains("Line 3"));
    }

    #[test]
    fn test_thinking_block_show_preview() {
        let block = ThinkingBlock::new()
            .content("This is a long piece of thinking content")
            .expanded(false)
            .show_preview(true)
            .preview_max_len(20);

        let node = block.to_node();
        let text = node_to_text(&node);

        // Should show truncated preview
        assert!(text.contains("This is a long pi..."));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(ThinkingBlock::truncate("short", 10), "short");
        assert_eq!(
            ThinkingBlock::truncate("this is a very long string", 10),
            "this is..."
        );
    }

    #[test]
    fn test_adaptive_rendering() {
        let block = ThinkingBlock::new().content("Test content").expanded(true);

        // All tiers should work
        let _tier0 = block.render_for_tier(RenderTier::Tier0Fallback);
        let _tier1 = block.render_for_tier(RenderTier::Tier1Ansi);
        let _tier2 = block.render_for_tier(RenderTier::Tier2Retained);
        let _tier3 = block.render_for_tier(RenderTier::Tier3Gpu);
    }

    #[test]
    fn test_tier0_collapsed_format() {
        let block = ThinkingBlock::new().content("Some content").expanded(false);

        let node = block.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("[+]"));
        assert!(text.contains("Thinking..."));
        assert!(text.contains("chars"));
    }

    #[test]
    fn test_tier0_expanded_format() {
        let block = ThinkingBlock::new().content("Some content").expanded(true);

        let node = block.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("[-]"));
        assert!(text.contains("Thinking..."));
        assert!(text.contains("Some content"));
    }

    #[test]
    fn test_tier1_format() {
        let block = ThinkingBlock::new().content("Test").expanded(true);

        let node = block.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("v"));
        assert!(text.contains("Thinking..."));
        assert!(text.contains("Test"));
    }

    #[test]
    fn test_from_into_node() {
        let block = ThinkingBlock::new().content("Test");
        let _node: Node = block.into();
    }
}
