//! Chat view component for displaying conversation history.
//!
//! # Adaptive Rendering
//!
//! `ChatView` implements [`AdaptiveComponent`](crate::components::adaptive::AdaptiveComponent)
//! for graceful degradation:
//!
//! | Tier | Rendering |
//! |------|-----------|
//! | 0 (Fallback) | Plain text with role prefixes, no formatting |
//! | 1 (ANSI) | Structured text with ASCII role markers, no colors |
//! | 2 (Retained) | Full styled rendering with colors and markdown |
//! | 3 (GPU) | Full rendering with GPU acceleration |

use crate::components::adaptive::{AdaptiveComponent, Tier0Fallback, TierFeatures};
use crate::components::Markdown;
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection, TextWrap};
use crate::terminal::RenderTier;

const USER_LABEL: &str = "You";
const ASSISTANT_LABEL: &str = "Assistant";
const SYSTEM_LABEL: &str = "System";

const USER_LABEL_COLOR: Color = Color::BrightCyan;
const ASSISTANT_LABEL_COLOR: Color = Color::BrightGreen;
const SYSTEM_LABEL_COLOR: Color = Color::BrightBlack;
const TIMESTAMP_COLOR: Color = Color::BrightBlack;

/// A chat message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    /// User-authored content.
    User,
    /// Assistant responses.
    Assistant,
    /// System or status messages.
    System,
}

/// A single chat message.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Message role.
    pub role: MessageRole,
    /// Message content.
    pub content: String,
    /// Optional timestamp for display.
    pub timestamp: Option<String>,
}

impl ChatMessage {
    /// Create a new chat message.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{ChatMessage, MessageRole};
    ///
    /// let message = ChatMessage::new(MessageRole::User, "Hello");
    /// ```
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            timestamp: None,
        }
    }

    /// Set the timestamp for this message.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{ChatMessage, MessageRole};
    ///
    /// let message = ChatMessage::new(MessageRole::Assistant, "Hi")
    ///     .timestamp("10:15");
    /// ```
    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }
}

/// Chat view component displaying a list of messages.
///
/// # Example
///
/// ```ignore
/// use inky::components::{ChatMessage, ChatView, MessageRole};
///
/// let view = ChatView::new()
///     .show_timestamps(true)
///     .message(ChatMessage::new(MessageRole::User, "Hello"))
///     .message(ChatMessage::new(MessageRole::Assistant, "**Hi** there"));
///
/// let node = view.to_node();
/// ```
#[derive(Debug, Clone)]
pub struct ChatView {
    messages: Vec<ChatMessage>,
    show_timestamps: bool,
    max_visible: Option<usize>,
    scroll_offset: usize,
}

impl ChatView {
    /// Create an empty chat view.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            show_timestamps: false,
            max_visible: None,
            scroll_offset: 0,
        }
    }

    /// Add a single message.
    pub fn message(mut self, msg: ChatMessage) -> Self {
        self.messages.push(msg);
        self
    }

    /// Add multiple messages.
    pub fn messages(mut self, msgs: impl IntoIterator<Item = ChatMessage>) -> Self {
        self.messages.extend(msgs);
        self
    }

    /// Toggle timestamp display.
    pub fn show_timestamps(mut self, show: bool) -> Self {
        self.show_timestamps = show;
        self
    }

    /// Limit the number of visible messages (enables scrolling).
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = Some(max);
        self
    }

    /// Set the message scroll offset.
    pub fn scroll_offset(mut self, offset: usize) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Convert the chat view to an inky Node tree.
    pub fn to_node(&self) -> Node {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);
        let (start, end) = self.visible_range();

        let mut prev_role = if start > 0 {
            self.messages
                .get(start.saturating_sub(1))
                .map(|msg| msg.role)
        } else {
            None
        };

        for msg in self.messages[start..end].iter() {
            if let Some(prev) = prev_role {
                if prev != msg.role {
                    container = container.child(TextNode::new(""));
                }
            }
            container = container.child(self.render_message(msg));
            prev_role = Some(msg.role);
        }

        container.into()
    }

    fn visible_range(&self) -> (usize, usize) {
        let total = self.messages.len();
        if total == 0 {
            return (0, 0);
        }

        if let Some(max) = self.max_visible {
            let max = max.min(total);
            let max_start = total.saturating_sub(max);
            let start = self.scroll_offset.min(max_start);
            let end = (start + max).min(total);
            (start, end)
        } else {
            (0, total)
        }
    }

    fn render_message(&self, msg: &ChatMessage) -> Node {
        let header = self.render_header(msg);
        let content = match msg.role {
            MessageRole::User => TextNode::new(msg.content.as_str())
                .wrap(TextWrap::Wrap)
                .into(),
            MessageRole::Assistant => Markdown::new(msg.content.as_str()).to_node(),
            MessageRole::System => TextNode::new(msg.content.as_str())
                .wrap(TextWrap::Wrap)
                .color(SYSTEM_LABEL_COLOR)
                .dim()
                .into(),
        };

        BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .child(header)
            .child(content)
            .into()
    }

    fn render_header(&self, msg: &ChatMessage) -> Node {
        let (label, color, dim) = match msg.role {
            MessageRole::User => (USER_LABEL, USER_LABEL_COLOR, false),
            MessageRole::Assistant => (ASSISTANT_LABEL, ASSISTANT_LABEL_COLOR, false),
            MessageRole::System => (SYSTEM_LABEL, SYSTEM_LABEL_COLOR, true),
        };

        let mut label_node = TextNode::new(label).color(color).bold();
        if dim {
            label_node = label_node.dim();
        }

        let mut header = BoxNode::new()
            .flex_direction(FlexDirection::Row)
            .gap(1.0)
            .child(label_node);

        if self.show_timestamps {
            if let Some(timestamp) = msg.timestamp.as_ref() {
                header = header.child(
                    TextNode::new(timestamp.as_str())
                        .color(TIMESTAMP_COLOR)
                        .dim(),
                );
            }
        }

        header.into()
    }
}

impl AdaptiveComponent for ChatView {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.to_node(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("ChatView")
            .tier0("Plain text transcript with role labels")
            .tier1("Structured ASCII with role markers, no colors")
            .tier2("Full styled rendering with colors and markdown")
            .tier3("Full rendering with GPU acceleration")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl ChatView {
    /// Render Tier 0: Plain text summary.
    ///
    /// Shows a simple transcript with role prefixes and no formatting.
    fn render_tier0(&self) -> Node {
        let msg_count = self.messages.len();
        let user_count = self
            .messages
            .iter()
            .filter(|m| m.role == MessageRole::User)
            .count();
        let assistant_count = self
            .messages
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .count();

        if msg_count == 0 {
            return Tier0Fallback::new("ChatView").stat("messages", "0").into();
        }

        Tier0Fallback::new("ChatView")
            .stat("messages", msg_count.to_string())
            .stat("user", user_count.to_string())
            .stat("assistant", assistant_count.to_string())
            .into()
    }

    /// Render Tier 1: ASCII-only structured text.
    ///
    /// Shows messages with ASCII role markers but no colors or styling.
    fn render_tier1(&self) -> Node {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);
        let (start, end) = self.visible_range();

        for (i, msg) in self.messages[start..end].iter().enumerate() {
            if i > 0 {
                container = container.child(TextNode::new(""));
            }
            container = container.child(self.render_message_tier1(msg));
        }

        container.into()
    }

    /// Render a single message for Tier 1.
    fn render_message_tier1(&self, msg: &ChatMessage) -> Node {
        let role_marker = match msg.role {
            MessageRole::User => "[USER]",
            MessageRole::Assistant => "[ASSISTANT]",
            MessageRole::System => "[SYSTEM]",
        };

        let header = if self.show_timestamps {
            if let Some(ts) = msg.timestamp.as_ref() {
                TextNode::new(format!("{} {}", role_marker, ts))
            } else {
                TextNode::new(role_marker)
            }
        } else {
            TextNode::new(role_marker)
        };

        // For Tier 1, strip markdown formatting and show plain text
        let content_text = strip_markdown_simple(&msg.content);
        let content = TextNode::new(content_text).wrap(TextWrap::Wrap);

        BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .child(header)
            .child(content)
            .into()
    }
}

/// Simple markdown stripping for Tier 1 rendering.
///
/// Removes common markdown syntax while preserving readable content.
fn strip_markdown_simple(content: &str) -> String {
    let mut result = String::with_capacity(content.len());

    for line in content.lines() {
        let line = line.trim();

        // Strip heading markers
        let line = line.trim_start_matches('#').trim_start();

        // Strip bold/italic markers
        let line = line.replace("**", "").replace("__", "");
        let line = line.replace(['*', '_'], "");

        // Strip inline code backticks (single)
        let line = line.replace('`', "");

        // Strip link syntax [text](url) -> text
        let mut chars = line.chars().peekable();
        let mut cleaned = String::new();
        while let Some(c) = chars.next() {
            if c == '[' {
                // Collect link text
                let mut link_text = String::new();
                for c2 in chars.by_ref() {
                    if c2 == ']' {
                        break;
                    }
                    link_text.push(c2);
                }
                // Skip (url) part
                if chars.peek() == Some(&'(') {
                    chars.next();
                    for c2 in chars.by_ref() {
                        if c2 == ')' {
                            break;
                        }
                    }
                }
                cleaned.push_str(&link_text);
            } else {
                cleaned.push(c);
            }
        }

        if !result.is_empty() && !line.is_empty() {
            result.push('\n');
        }
        result.push_str(&cleaned);
    }

    result
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ChatView> for Node {
    fn from(view: ChatView) -> Self {
        view.to_node()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::style::TextWrap;

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

    fn find_text_node_containing<'a>(node: &'a Node, needle: &str) -> Option<&'a TextNode> {
        match node {
            Node::Text(t) if t.content.contains(needle) => Some(t),
            Node::Box(b) => b
                .children
                .iter()
                .find_map(|child| find_text_node_containing(child, needle)),
            Node::Root(r) => r
                .children
                .iter()
                .find_map(|child| find_text_node_containing(child, needle)),
            Node::Static(s) => s
                .children
                .iter()
                .find_map(|child| find_text_node_containing(child, needle)),
            Node::Custom(c) => c
                .widget()
                .children()
                .iter()
                .find_map(|child| find_text_node_containing(child, needle)),
            _ => None,
        }
    }

    fn contains_bold_text(node: &Node, needle: &str) -> bool {
        match node {
            Node::Text(t) => t.content.contains(needle) && t.text_style.bold,
            Node::Box(b) => b
                .children
                .iter()
                .any(|child| contains_bold_text(child, needle)),
            Node::Root(r) => r
                .children
                .iter()
                .any(|child| contains_bold_text(child, needle)),
            Node::Static(s) => s
                .children
                .iter()
                .any(|child| contains_bold_text(child, needle)),
            Node::Custom(c) => c
                .widget()
                .children()
                .iter()
                .any(|child| contains_bold_text(child, needle)),
        }
    }

    fn count_empty_text_nodes(node: &Node) -> usize {
        match node {
            Node::Text(t) => usize::from(t.content.is_empty()),
            Node::Box(b) => b.children.iter().map(|c| count_empty_text_nodes(c)).sum(),
            Node::Root(r) => r.children.iter().map(|c| count_empty_text_nodes(c)).sum(),
            Node::Static(s) => s.children.iter().map(|c| count_empty_text_nodes(c)).sum(),
            Node::Custom(c) => c
                .widget()
                .children()
                .iter()
                .map(|child| count_empty_text_nodes(child))
                .sum(),
        }
    }

    #[test]
    fn test_empty_chat() {
        let view = ChatView::new();
        let node = view.to_node();
        let text = node_to_text(&node);
        assert!(text.is_empty());
        assert_eq!(count_empty_text_nodes(&node), 0);
    }

    #[test]
    fn test_user_message() {
        let view = ChatView::new().message(ChatMessage::new(MessageRole::User, "Hello"));
        let node = view.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Hello"));
        assert!(text.contains(USER_LABEL));

        let label = find_text_node_containing(&node, USER_LABEL).expect("missing user label");
        assert!(label.text_style.bold);
        assert_eq!(label.text_style.color, Some(USER_LABEL_COLOR));
    }

    #[test]
    fn test_assistant_message_markdown() {
        let view =
            ChatView::new().message(ChatMessage::new(MessageRole::Assistant, "**Bold** reply"));
        let node = view.to_node();
        let text = node_to_text(&node);

        assert!(text.contains(ASSISTANT_LABEL));
        assert!(contains_bold_text(&node, "Bold"));
    }

    #[test]
    fn test_system_message_dimmed() {
        let view = ChatView::new().message(ChatMessage::new(MessageRole::System, "System update"));
        let node = view.to_node();

        let content =
            find_text_node_containing(&node, "System update").expect("missing system content");
        assert!(content.text_style.dim);
    }

    #[test]
    fn test_timestamps_toggle() {
        let message = ChatMessage::new(MessageRole::User, "Hello").timestamp("10:00");

        let node = ChatView::new()
            .show_timestamps(false)
            .message(message.clone())
            .to_node();
        assert!(!node_to_text(&node).contains("10:00"));

        let node = ChatView::new()
            .show_timestamps(true)
            .message(message)
            .to_node();
        assert!(node_to_text(&node).contains("10:00"));
    }

    #[test]
    fn test_multiple_messages_order() {
        let view = ChatView::new().messages(vec![
            ChatMessage::new(MessageRole::User, "First"),
            ChatMessage::new(MessageRole::Assistant, "Second"),
            ChatMessage::new(MessageRole::User, "Third"),
        ]);

        let text = node_to_text(&view.to_node());
        let first = text.find("First").expect("missing First");
        let second = text.find("Second").expect("missing Second");
        let third = text.find("Third").expect("missing Third");

        assert!(first < second && second < third);
    }

    #[test]
    fn test_grouping_separator() {
        let view = ChatView::new().messages(vec![
            ChatMessage::new(MessageRole::User, "One"),
            ChatMessage::new(MessageRole::User, "Two"),
            ChatMessage::new(MessageRole::Assistant, "Three"),
        ]);

        let node = view.to_node();
        assert_eq!(count_empty_text_nodes(&node), 1);
    }

    #[test]
    fn test_scroll_offset_limits_visible_messages() {
        let view = ChatView::new()
            .max_visible(2)
            .scroll_offset(1)
            .messages(vec![
                ChatMessage::new(MessageRole::User, "First"),
                ChatMessage::new(MessageRole::User, "Second"),
                ChatMessage::new(MessageRole::User, "Third"),
            ]);

        let text = node_to_text(&view.to_node());
        assert!(!text.contains("First"));
        assert!(text.contains("Second"));
        assert!(text.contains("Third"));
    }

    #[test]
    fn test_long_message_wraps() {
        let long = "This is a long message that should wrap across lines.";
        let view = ChatView::new().message(ChatMessage::new(MessageRole::User, long));
        let node = view.to_node();

        let content = find_text_node_containing(&node, long).expect("missing long content");
        assert_eq!(content.text_style.wrap, TextWrap::Wrap);
    }

    // Adaptive rendering tests
    #[test]
    fn test_adaptive_tier0_empty() {
        let view = ChatView::new();
        let node = view.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);
        assert!(text.contains("[ChatView]"));
        assert!(text.contains("messages=0"));
    }

    #[test]
    fn test_adaptive_tier0_with_messages() {
        let view = ChatView::new().messages(vec![
            ChatMessage::new(MessageRole::User, "Hello"),
            ChatMessage::new(MessageRole::Assistant, "Hi there"),
            ChatMessage::new(MessageRole::User, "How are you?"),
        ]);
        let node = view.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("[ChatView]"));
        assert!(text.contains("messages=3"));
        assert!(text.contains("user=2"));
        assert!(text.contains("assistant=1"));
    }

    #[test]
    fn test_adaptive_tier1_role_markers() {
        let view = ChatView::new().messages(vec![
            ChatMessage::new(MessageRole::User, "Hello"),
            ChatMessage::new(MessageRole::Assistant, "Hi"),
            ChatMessage::new(MessageRole::System, "Notice"),
        ]);
        let node = view.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("[USER]"));
        assert!(text.contains("[ASSISTANT]"));
        assert!(text.contains("[SYSTEM]"));
        assert!(text.contains("Hello"));
        assert!(text.contains("Hi"));
        assert!(text.contains("Notice"));
    }

    #[test]
    fn test_adaptive_tier1_strips_markdown() {
        let view =
            ChatView::new().message(ChatMessage::new(MessageRole::Assistant, "**Bold** text"));
        let node = view.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        // Should contain the text without bold markers
        assert!(text.contains("Bold text"));
        // Should not contain asterisks (bold markers stripped)
        assert!(!text.contains("**"));
    }

    #[test]
    fn test_adaptive_tier1_with_timestamp() {
        let view = ChatView::new()
            .show_timestamps(true)
            .message(ChatMessage::new(MessageRole::User, "Hello").timestamp("10:30"));
        let node = view.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("[USER]"));
        assert!(text.contains("10:30"));
    }

    #[test]
    fn test_adaptive_tier2_same_as_default() {
        let view = ChatView::new().message(ChatMessage::new(MessageRole::User, "Hello"));

        let tier2_node = view.render_for_tier(RenderTier::Tier2Retained);
        let default_node = view.to_node();

        // Both should produce equivalent output
        assert_eq!(node_to_text(&tier2_node), node_to_text(&default_node));
    }

    #[test]
    fn test_adaptive_tier3_same_as_tier2() {
        let view = ChatView::new().message(ChatMessage::new(MessageRole::Assistant, "Reply"));

        let tier2_node = view.render_for_tier(RenderTier::Tier2Retained);
        let tier3_node = view.render_for_tier(RenderTier::Tier3Gpu);

        assert_eq!(node_to_text(&tier2_node), node_to_text(&tier3_node));
    }

    #[test]
    fn test_adaptive_tier_features() {
        let view = ChatView::new();
        let features = view.tier_features();

        assert_eq!(features.name, Some("ChatView"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
    }

    #[test]
    fn test_adaptive_minimum_tier() {
        let view = ChatView::new();
        assert_eq!(view.minimum_tier(), None); // Works at all tiers
        assert!(view.supports_tier(RenderTier::Tier0Fallback));
        assert!(view.supports_tier(RenderTier::Tier3Gpu));
    }

    #[test]
    fn test_strip_markdown_simple() {
        // Test bold stripping
        assert_eq!(strip_markdown_simple("**bold**"), "bold");
        assert_eq!(strip_markdown_simple("__bold__"), "bold");

        // Test italic stripping
        assert_eq!(strip_markdown_simple("*italic*"), "italic");
        assert_eq!(strip_markdown_simple("_italic_"), "italic");

        // Test inline code stripping
        assert_eq!(strip_markdown_simple("`code`"), "code");

        // Test heading stripping
        assert_eq!(strip_markdown_simple("# Heading"), "Heading");
        assert_eq!(strip_markdown_simple("## Heading 2"), "Heading 2");

        // Test link stripping
        assert_eq!(strip_markdown_simple("[text](url)"), "text");
        assert_eq!(
            strip_markdown_simple("Check [link](http://example.com) here"),
            "Check link here"
        );
    }
}
