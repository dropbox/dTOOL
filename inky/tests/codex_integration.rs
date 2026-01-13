#![allow(clippy::unwrap_used)]
//! Integration tests for Phase 8 Codex UX components.
//!
//! These tests verify that ChatView, DiffView, StatusBar, and Markdown
//! components compose correctly and integrate with the core inky layout system.

use inky::components::{
    ChatMessage, ChatView, DiffLine, DiffLineKind, DiffView, Markdown, MessageRole, StatusBar,
    StatusState,
};
use inky::layout::LayoutEngine;
use inky::node::{BoxNode, Node, TextNode};
use inky::render::{render_to_buffer, Buffer};
use inky::style::{Color, FlexDirection};

// ============================================================================
// Helper functions
// ============================================================================

/// Convert a node tree to text by extracting all TextNode content.
fn node_to_text(node: &Node) -> String {
    match node {
        Node::Text(t) => t.content.to_string(),
        Node::Box(b) => b
            .children
            .iter()
            .map(|c| node_to_text(c))
            .collect::<Vec<_>>()
            .join(""),
        Node::Root(r) => r
            .children
            .iter()
            .map(|c| node_to_text(c))
            .collect::<Vec<_>>()
            .join(""),
        Node::Static(s) => s
            .children
            .iter()
            .map(|c| node_to_text(c))
            .collect::<Vec<_>>()
            .join(""),
        Node::Custom(c) => c
            .widget()
            .children()
            .iter()
            .map(|child| node_to_text(child))
            .collect::<Vec<_>>()
            .join(""),
    }
}

/// Check if a text node with specific content has a certain color.
fn find_text_with_color(node: &Node, content: &str, expected_color: Color) -> bool {
    match node {
        Node::Text(t) => t.content.contains(content) && t.text_style.color == Some(expected_color),
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

// ============================================================================
// Component Composition Tests
// ============================================================================

/// Test that ChatView and DiffView can be composed in a vertical layout.
#[test]
fn test_chat_and_diff_composition() {
    let chat = ChatView::new()
        .message(ChatMessage::new(MessageRole::User, "Hello"))
        .message(ChatMessage::new(MessageRole::Assistant, "Hi there"));

    let diff = DiffView::new()
        .file_path("test.rs")
        .line(DiffLine::add(1, "new line"));

    // Compose in a layout
    let layout: Node = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .child(chat.to_node())
        .child(diff.to_node())
        .into();

    // Should produce valid Node tree
    assert!(matches!(layout, Node::Box(_)));

    // Should contain both chat and diff content
    let text = node_to_text(&layout);
    assert!(text.contains("Hello"), "Should contain user message");
    assert!(
        text.contains("Hi there"),
        "Should contain assistant message"
    );
    assert!(text.contains("test.rs"), "Should contain diff file path");
    assert!(text.contains("+new line"), "Should contain diff add line");
}

/// Test that StatusBar composes with other components.
#[test]
fn test_status_bar_composition() {
    let chat = ChatView::new().message(ChatMessage::new(MessageRole::User, "Test"));

    let status = StatusBar::new()
        .state(StatusState::Thinking)
        .message("Processing...");

    let layout: Node = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .child(chat.to_node())
        .child(status.to_node())
        .into();

    let text = node_to_text(&layout);
    assert!(text.contains("Test"), "Should contain chat message");
    assert!(
        text.contains("Processing..."),
        "Should contain status message"
    );
}

/// Test full Codex-style layout with all components.
#[test]
fn test_full_codex_layout() {
    // Header
    let header = TextNode::new("Codex TUI").bold();

    // Chat view
    let chat = ChatView::new()
        .message(ChatMessage::new(MessageRole::User, "Fix the bug"))
        .message(ChatMessage::new(MessageRole::Assistant, "I'll fix it"));

    // Diff view
    let diff = DiffView::new()
        .file_path("main.rs")
        .line(DiffLine::context(1, "fn main() {"))
        .line(DiffLine::delete(2, "    old_code();"))
        .line(DiffLine::add(2, "    new_code();"))
        .line(DiffLine::context(3, "}"));

    // Status bar
    let status = StatusBar::new().state(StatusState::Idle).message("Ready");

    // Compose everything
    let layout: Node = BoxNode::new()
        .width(80)
        .height(24)
        .flex_direction(FlexDirection::Column)
        .child(header)
        .child(chat.to_node())
        .child(diff.to_node())
        .child(status.to_node())
        .into();

    let text = node_to_text(&layout);

    // Verify all sections are present
    assert!(text.contains("Codex TUI"), "Should have header");
    assert!(text.contains("Fix the bug"), "Should have user message");
    assert!(
        text.contains("I'll fix it"),
        "Should have assistant message"
    );
    assert!(text.contains("main.rs"), "Should have diff file path");
    assert!(text.contains("-    old_code();"), "Should have delete line");
    assert!(text.contains("+    new_code();"), "Should have add line");
    assert!(text.contains("Ready"), "Should have status message");
}

// ============================================================================
// Layout Integration Tests
// ============================================================================

/// Test that components render correctly through the layout engine.
#[test]
fn test_layout_engine_integration() {
    let diff = DiffView::new()
        .file_path("test.rs")
        .line(DiffLine::add(1, "added"))
        .line(DiffLine::delete(2, "removed"));

    let root: Node = BoxNode::new()
        .width(80)
        .height(20)
        .flex_direction(FlexDirection::Column)
        .child(diff.to_node())
        .into();

    // Build layout - should not panic
    let mut engine = LayoutEngine::new();
    engine.build(&root).unwrap();
    engine.compute(80, 20).unwrap();

    // Verify layout was computed
    let layout = engine.get(root.id()).unwrap();
    assert_eq!(layout.width, 80);
    assert_eq!(layout.height, 20);

    // Render to buffer - should not panic
    let mut buffer = Buffer::new(80, 20);
    render_to_buffer(&root, &engine, &mut buffer);

    // Verify buffer was created successfully
    assert_eq!(buffer.width(), 80);
    assert_eq!(buffer.height(), 20);
}

/// Test ChatView renders properly through layout engine.
#[test]
fn test_chat_view_render() {
    let chat = ChatView::new().message(ChatMessage::new(MessageRole::User, "Hello world"));

    let root: Node = BoxNode::new()
        .width(80)
        .height(20)
        .flex_direction(FlexDirection::Column)
        .child(chat.to_node())
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&root).unwrap();
    engine.compute(80, 20).unwrap();

    // Verify layout was computed
    let layout = engine.get(root.id()).unwrap();
    assert_eq!(layout.width, 80);
    assert_eq!(layout.height, 20);

    // Render to buffer - should not panic
    let mut buffer = Buffer::new(80, 20);
    render_to_buffer(&root, &engine, &mut buffer);

    // Verify buffer was created successfully
    assert_eq!(buffer.width(), 80);
    assert_eq!(buffer.height(), 20);
}

/// Test StatusBar renders through layout.
#[test]
fn test_status_bar_render() {
    let status = StatusBar::new()
        .state(StatusState::Error)
        .message("Something went wrong");

    let root: Node = BoxNode::new()
        .width(80)
        .height(10)
        .flex_direction(FlexDirection::Column)
        .child(status.to_node())
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&root).unwrap();
    engine.compute(80, 10).unwrap();

    // Verify layout was computed
    let layout = engine.get(root.id()).unwrap();
    assert_eq!(layout.width, 80);
    assert_eq!(layout.height, 10);

    // Render to buffer - should not panic
    let mut buffer = Buffer::new(80, 10);
    render_to_buffer(&root, &engine, &mut buffer);

    // Verify buffer was created successfully
    assert_eq!(buffer.width(), 80);
    assert_eq!(buffer.height(), 10);
}

// ============================================================================
// API Compatibility Tests
// ============================================================================

/// Test ChatMessage builder API.
#[test]
fn test_chat_message_api() {
    let msg = ChatMessage::new(MessageRole::User, "Test message").timestamp("10:30");

    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.content, "Test message");
    assert_eq!(msg.timestamp, Some("10:30".to_string()));
}

/// Test ChatView builder API.
#[test]
fn test_chat_view_api() {
    let view = ChatView::new()
        .show_timestamps(true)
        .max_visible(10)
        .scroll_offset(5)
        .message(ChatMessage::new(MessageRole::User, "First"))
        .message(ChatMessage::new(MessageRole::Assistant, "Second"))
        .messages(vec![
            ChatMessage::new(MessageRole::System, "Third"),
            ChatMessage::new(MessageRole::User, "Fourth"),
        ]);

    let node = view.to_node();
    let text = node_to_text(&node);

    // All messages should be present
    assert!(text.contains("First"));
    assert!(text.contains("Second"));
    assert!(text.contains("Third"));
    assert!(text.contains("Fourth"));
}

/// Test DiffLine constructors.
#[test]
fn test_diff_line_api() {
    let add = DiffLine::add(10, "new code");
    assert_eq!(add.kind, DiffLineKind::Add);
    assert_eq!(add.line_number, Some(10));
    assert_eq!(add.content, "new code");

    let delete = DiffLine::delete(20, "old code");
    assert_eq!(delete.kind, DiffLineKind::Delete);
    assert_eq!(delete.line_number, Some(20));
    assert_eq!(delete.content, "old code");

    let context = DiffLine::context(30, "unchanged");
    assert_eq!(context.kind, DiffLineKind::Context);
    assert_eq!(context.line_number, Some(30));
    assert_eq!(context.content, "unchanged");

    let separator = DiffLine::hunk_separator();
    assert_eq!(separator.kind, DiffLineKind::HunkSeparator);
    assert_eq!(separator.line_number, None);
}

/// Test DiffView builder API.
#[test]
fn test_diff_view_api() {
    let view = DiffView::new()
        .file_path("path/to/file.rs")
        .show_line_numbers(true)
        .show_summary(true)
        .line(DiffLine::add(1, "a"))
        .lines(vec![DiffLine::delete(2, "b"), DiffLine::context(3, "c")]);

    let node = view.to_node();
    let text = node_to_text(&node);

    assert!(text.contains("path/to/file.rs"));
    assert!(text.contains("+a"));
    assert!(text.contains("-b"));
    assert!(text.contains(" c"));
}

/// Test StatusState methods.
#[test]
fn test_status_state_api() {
    // is_active
    assert!(!StatusState::Idle.is_active());
    assert!(StatusState::Thinking.is_active());
    assert!(StatusState::Executing.is_active());
    assert!(!StatusState::Error.is_active());

    // colors
    assert_eq!(StatusState::Idle.color(), Color::Green);
    assert_eq!(StatusState::Thinking.color(), Color::Yellow);
    assert_eq!(StatusState::Executing.color(), Color::Blue);
    assert_eq!(StatusState::Error.color(), Color::Red);

    // labels
    assert_eq!(StatusState::Idle.label(), "Ready");
    assert_eq!(StatusState::Thinking.label(), "Thinking");
    assert_eq!(StatusState::Executing.label(), "Executing");
    assert_eq!(StatusState::Error.label(), "Error");

    // indicators
    assert_eq!(StatusState::Idle.indicator(), "●");
    assert_eq!(StatusState::Error.indicator(), "✗");
}

/// Test StatusBar builder API.
#[test]
fn test_status_bar_api() {
    let status = StatusBar::new()
        .state(StatusState::Thinking)
        .message("Custom message");

    assert_eq!(status.current_state(), StatusState::Thinking);
    assert_eq!(status.current_message(), "Custom message");
}

/// Test StatusBar spinner animation.
#[test]
fn test_status_bar_spinner() {
    let mut status = StatusBar::new()
        .state(StatusState::Thinking)
        .message("Working...");

    assert_eq!(status.spinner_frame(), 0);

    // Tick the spinner multiple times
    for i in 1..=10 {
        status.tick();
        assert_eq!(status.spinner_frame(), i);
    }

    // Should still produce valid node
    let _node = status.to_node();
}

/// Test Markdown component renders correctly.
#[test]
fn test_markdown_api() {
    let md = Markdown::new("**Bold** and *italic* text");
    let node = md.to_node();
    let text = node_to_text(&node);

    assert!(text.contains("Bold"));
    assert!(text.contains("italic"));
}

// ============================================================================
// From<T> for Node Implementation Tests
// ============================================================================

/// Test From<ChatView> for Node.
#[test]
fn test_chat_view_into_node() {
    let chat = ChatView::new().message(ChatMessage::new(MessageRole::User, "Test"));

    let node: Node = chat.into();
    assert!(matches!(node, Node::Box(_)));
}

/// Test From<DiffView> for Node.
#[test]
fn test_diff_view_into_node() {
    let diff = DiffView::new().line(DiffLine::add(1, "line"));

    let node: Node = diff.into();
    assert!(matches!(node, Node::Box(_)));
}

/// Test From<StatusBar> for Node.
#[test]
fn test_status_bar_into_node() {
    let status = StatusBar::new().state(StatusState::Idle);

    let node: Node = status.into();
    assert!(matches!(node, Node::Box(_)));
}

/// Test From<Markdown> for Node.
#[test]
fn test_markdown_into_node() {
    let md = Markdown::new("# Header");

    let node: Node = md.into();
    assert!(matches!(node, Node::Box(_)));
}

// ============================================================================
// Default Implementation Tests
// ============================================================================

/// Test ChatView::default().
#[test]
fn test_chat_view_default() {
    let chat = ChatView::default();
    let node = chat.to_node();

    // Empty chat should produce an empty box
    let text = node_to_text(&node);
    assert!(text.is_empty(), "Default ChatView should be empty");
}

/// Test DiffView::default().
#[test]
fn test_diff_view_default() {
    let diff = DiffView::default();
    let node = diff.to_node();

    // Empty diff should produce a box (possibly with no summary if no changes)
    assert!(matches!(node, Node::Box(_)));
}

/// Test StatusBar::default().
#[test]
fn test_status_bar_default() {
    let status = StatusBar::default();

    assert_eq!(status.current_state(), StatusState::Idle);
    assert_eq!(status.current_message(), "Ready");
}

/// Test StatusState::default().
#[test]
fn test_status_state_default() {
    let state = StatusState::default();
    assert_eq!(state, StatusState::Idle);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test ChatView with only system messages.
#[test]
fn test_chat_view_system_messages_only() {
    let chat = ChatView::new()
        .message(ChatMessage::new(MessageRole::System, "System initialized"))
        .message(ChatMessage::new(MessageRole::System, "Loading..."));

    let node = chat.to_node();
    let text = node_to_text(&node);

    assert!(text.contains("System initialized"));
    assert!(text.contains("Loading..."));
}

/// Test DiffView with only hunk separators.
#[test]
fn test_diff_view_only_separators() {
    let diff = DiffView::new()
        .show_summary(false)
        .line(DiffLine::hunk_separator())
        .line(DiffLine::hunk_separator());

    let node = diff.to_node();
    let text = node_to_text(&node);

    // Should contain separator characters
    assert!(text.contains("⋮"), "Should contain hunk separator");
}

/// Test DiffView with very long line numbers.
#[test]
fn test_diff_view_large_line_numbers() {
    let diff = DiffView::new()
        .show_summary(false)
        .line(DiffLine::add(1, "start"))
        .line(DiffLine::add(99999, "end"));

    let node = diff.to_node();
    let text = node_to_text(&node);

    // Line numbers should be properly aligned
    assert!(text.contains("99999"), "Should contain large line number");
}

/// Test StatusBar state transitions.
#[test]
fn test_status_bar_state_transitions() {
    let states = [
        StatusState::Idle,
        StatusState::Thinking,
        StatusState::Executing,
        StatusState::Error,
    ];

    for state in states {
        let status = StatusBar::new().state(state);
        let node = status.to_node();
        let text = node_to_text(&node);

        // Each state should render its label
        assert!(
            text.contains(state.label()),
            "State {:?} should render label '{}'",
            state,
            state.label()
        );
    }
}

/// Test ChatView scrolling behavior.
#[test]
fn test_chat_view_scrolling() {
    // Create many messages
    let messages: Vec<ChatMessage> = (0..20)
        .map(|i| ChatMessage::new(MessageRole::User, format!("Message {}", i)))
        .collect();

    let chat = ChatView::new()
        .messages(messages)
        .max_visible(5)
        .scroll_offset(10);

    let node = chat.to_node();
    let text = node_to_text(&node);

    // Should only show messages 10-14 (5 visible starting at offset 10)
    assert!(
        !text.contains("Message 9"),
        "Should not show message before scroll offset"
    );
    assert!(
        text.contains("Message 10"),
        "Should show first visible message"
    );
    assert!(
        text.contains("Message 14"),
        "Should show last visible message"
    );
    assert!(
        !text.contains("Message 15"),
        "Should not show message after visible window"
    );
}

/// Test component color consistency.
#[test]
fn test_component_colors() {
    // DiffView colors
    let diff = DiffView::new()
        .show_summary(false)
        .line(DiffLine::add(1, "added"))
        .line(DiffLine::delete(2, "removed"));

    let diff_node = diff.to_node();
    assert!(
        find_text_with_color(&diff_node, "+added", Color::Green),
        "Add lines should be green"
    );
    assert!(
        find_text_with_color(&diff_node, "-removed", Color::Red),
        "Delete lines should be red"
    );

    // StatusBar colors
    let status = StatusBar::new().state(StatusState::Error);
    let status_node = status.to_node();
    assert!(
        find_text_with_color(&status_node, "Error", Color::Red),
        "Error status should be red"
    );
}

// ============================================================================
// Nested Layout Tests
// ============================================================================

/// Test deeply nested component composition.
#[test]
fn test_nested_composition() {
    let inner_chat = ChatView::new().message(ChatMessage::new(MessageRole::User, "Inner"));

    let inner_box: Node = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .child(inner_chat.to_node())
        .into();

    let outer: Node = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .child(inner_box)
        .child(StatusBar::new().to_node())
        .into();

    let text = node_to_text(&outer);
    assert!(text.contains("Inner"), "Nested chat should render");
    assert!(text.contains("Ready"), "Outer status should render");
}

/// Test horizontal layout with components.
#[test]
fn test_horizontal_layout() {
    let diff1 = DiffView::new()
        .file_path("file1.rs")
        .line(DiffLine::add(1, "left"));

    let diff2 = DiffView::new()
        .file_path("file2.rs")
        .line(DiffLine::add(1, "right"));

    let layout: Node = BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .child(diff1.to_node())
        .child(diff2.to_node())
        .into();

    let text = node_to_text(&layout);
    assert!(text.contains("file1.rs"), "Left diff should render");
    assert!(text.contains("file2.rs"), "Right diff should render");
}
