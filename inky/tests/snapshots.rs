#![allow(clippy::unwrap_used)]
//! Snapshot tests for visual regression testing.
//!
//! Uses insta for snapshot testing of rendered output.
//! Run `cargo insta review` to review and accept snapshot changes.

use inky::layout::LayoutEngine;
use inky::node::{BoxNode, Node, TextNode};
use inky::render::{render_to_buffer, Buffer};
use inky::style::{BorderStyle, FlexDirection, JustifyContent};

/// Helper to render a node to a text snapshot.
fn render_snapshot(node: &Node, width: u16, height: u16) -> String {
    let mut engine = LayoutEngine::new();
    engine.build(node).unwrap();
    engine.compute(width, height).unwrap();

    let mut buffer = Buffer::new(width, height);
    render_to_buffer(node, &engine, &mut buffer);
    buffer.to_text()
}

// =============================================================================
// Box Component Snapshots
// =============================================================================

#[test]
fn snapshot_box_single_border() {
    let node: Node = BoxNode::new()
        .width(15)
        .height(5)
        .border(BorderStyle::Single)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 20, 10));
}

#[test]
fn snapshot_box_double_border() {
    let node: Node = BoxNode::new()
        .width(15)
        .height(5)
        .border(BorderStyle::Double)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 20, 10));
}

#[test]
fn snapshot_box_rounded_border() {
    let node: Node = BoxNode::new()
        .width(15)
        .height(5)
        .border(BorderStyle::Rounded)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 20, 10));
}

#[test]
fn snapshot_box_bold_border() {
    let node: Node = BoxNode::new()
        .width(15)
        .height(5)
        .border(BorderStyle::Bold)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 20, 10));
}

// =============================================================================
// Text Component Snapshots
// =============================================================================

#[test]
fn snapshot_text_in_box() {
    let node: Node = BoxNode::new()
        .width(20)
        .height(5)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Hello, World!"))
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 25, 10));
}

#[test]
fn snapshot_multiline_text() {
    let node: Node = BoxNode::new()
        .width(20)
        .height(7)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Line 1"))
        .child(TextNode::new("Line 2"))
        .child(TextNode::new("Line 3"))
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 25, 12));
}

// =============================================================================
// Layout Snapshots
// =============================================================================

#[test]
fn snapshot_horizontal_layout() {
    let node: Node = BoxNode::new()
        .width(40)
        .height(5)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Row)
        .child(
            BoxNode::new()
                .width(10)
                .height(3)
                .border(BorderStyle::Single),
        )
        .child(
            BoxNode::new()
                .width(10)
                .height(3)
                .border(BorderStyle::Single),
        )
        .child(
            BoxNode::new()
                .width(10)
                .height(3)
                .border(BorderStyle::Single),
        )
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 10));
}

#[test]
fn snapshot_vertical_layout() {
    let node: Node = BoxNode::new()
        .width(15)
        .height(12)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .child(
            BoxNode::new()
                .width(13)
                .height(3)
                .border(BorderStyle::Single),
        )
        .child(
            BoxNode::new()
                .width(13)
                .height(3)
                .border(BorderStyle::Single),
        )
        .child(
            BoxNode::new()
                .width(13)
                .height(3)
                .border(BorderStyle::Single),
        )
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 20, 15));
}

#[test]
fn snapshot_nested_boxes() {
    let inner = BoxNode::new()
        .width(10)
        .height(3)
        .border(BorderStyle::Rounded);

    let outer: Node = BoxNode::new()
        .width(20)
        .height(7)
        .border(BorderStyle::Double)
        .flex_direction(FlexDirection::Column)
        .padding(1)
        .child(inner)
        .into();

    insta::assert_snapshot!(render_snapshot(&outer, 25, 12));
}

// =============================================================================
// Flex Properties Snapshots
// =============================================================================

#[test]
fn snapshot_flex_grow() {
    let node: Node = BoxNode::new()
        .width(40)
        .height(5)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Row)
        .child(BoxNode::new().flex_grow(1.0).border(BorderStyle::Single))
        .child(BoxNode::new().flex_grow(2.0).border(BorderStyle::Single))
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 10));
}

#[test]
fn snapshot_justify_space_between() {
    let node: Node = BoxNode::new()
        .width(40)
        .height(5)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Row)
        .justify_content(JustifyContent::SpaceBetween)
        .child(
            BoxNode::new()
                .width(8)
                .height(3)
                .border(BorderStyle::Single),
        )
        .child(
            BoxNode::new()
                .width(8)
                .height(3)
                .border(BorderStyle::Single),
        )
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 10));
}

// =============================================================================
// Macro Snapshots
// =============================================================================

#[test]
fn snapshot_vbox_macro() {
    use inky::{text, vbox};

    let node: Node = vbox![
        text!("Header").bold(),
        text!("Content"),
        text!("Footer").italic(),
    ]
    .width(20)
    .height(6)
    .border(BorderStyle::Single)
    .into();

    insta::assert_snapshot!(render_snapshot(&node, 25, 10));
}

#[test]
fn snapshot_hbox_macro() {
    use inky::{hbox, text};

    let node: Node = hbox![text!("Left"), text!("Center"), text!("Right"),]
        .width(30)
        .height(3)
        .border(BorderStyle::Single)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 35, 8));
}

#[test]
fn snapshot_nested_macros() {
    use inky::{hbox, text, vbox};

    let node: Node = vbox![
        text!("Title").bold(),
        hbox![text!("A"), text!("B"), text!("C"),],
        text!("Footer"),
    ]
    .width(25)
    .height(8)
    .border(BorderStyle::Double)
    .into();

    insta::assert_snapshot!(render_snapshot(&node, 30, 12));
}

// =============================================================================
// Complex Layout Snapshots
// =============================================================================

#[test]
fn snapshot_dashboard_layout() {
    let header: Node = BoxNode::new()
        .width(50)
        .height(3)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Dashboard"))
        .into();

    let sidebar: Node = BoxNode::new()
        .width(15)
        .height(10)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Menu"))
        .into();

    let content: Node = BoxNode::new()
        .flex_grow(1.0)
        .height(10)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Content Area"))
        .into();

    let main_row: Node = BoxNode::new()
        .width(50)
        .height(10)
        .flex_direction(FlexDirection::Row)
        .child(sidebar)
        .child(content)
        .into();

    let dashboard: Node = BoxNode::new()
        .width(50)
        .height(15)
        .flex_direction(FlexDirection::Column)
        .child(header)
        .child(main_row)
        .into();

    insta::assert_snapshot!(render_snapshot(&dashboard, 55, 20));
}

#[test]
fn snapshot_form_layout() {
    let label1 = TextNode::new("Name:");
    let label2 = TextNode::new("Email:");
    let label3 = TextNode::new("Password:");

    let field1: Node = BoxNode::new()
        .width(20)
        .height(1)
        .child(TextNode::new("[__________]"))
        .into();

    let field2: Node = BoxNode::new()
        .width(20)
        .height(1)
        .child(TextNode::new("[__________]"))
        .into();

    let field3: Node = BoxNode::new()
        .width(20)
        .height(1)
        .child(TextNode::new("[**********]"))
        .into();

    let row1: Node = BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .child(label1)
        .child(field1)
        .into();

    let row2: Node = BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .child(label2)
        .child(field2)
        .into();

    let row3: Node = BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .child(label3)
        .child(field3)
        .into();

    let form: Node = BoxNode::new()
        .width(40)
        .height(10)
        .border(BorderStyle::Rounded)
        .flex_direction(FlexDirection::Column)
        .padding(1)
        .child(TextNode::new("Login Form"))
        .child(row1)
        .child(row2)
        .child(row3)
        .into();

    insta::assert_snapshot!(render_snapshot(&form, 45, 15));
}

// =============================================================================
// Component Snapshots (Phase 15.6)
// =============================================================================

#[test]
fn snapshot_progress_bar_block() {
    use inky::components::{Progress, ProgressStyle};

    let node: Node = BoxNode::new()
        .width(40)
        .height(3)
        .border(BorderStyle::Single)
        .child(Progress::new().value(75, 100).style(ProgressStyle::Block))
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 5));
}

#[test]
fn snapshot_progress_bar_ascii() {
    use inky::components::{Progress, ProgressStyle};

    let node: Node = BoxNode::new()
        .width(40)
        .height(3)
        .border(BorderStyle::Single)
        .child(Progress::new().value(50, 100).style(ProgressStyle::Ascii))
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 5));
}

#[test]
fn snapshot_sparkline_braille() {
    use inky::components::{Sparkline, SparklineStyle};

    let data = vec![1.0_f32, 4.0, 2.0, 8.0, 3.0, 6.0, 5.0, 7.0];
    let node: Node = BoxNode::new()
        .width(20)
        .height(4)
        .border(BorderStyle::Single)
        .child(Sparkline::new(data).style(SparklineStyle::Braille))
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 25, 6));
}

#[test]
fn snapshot_sparkline_blocks() {
    use inky::components::{Sparkline, SparklineStyle};

    let data = vec![2.0_f32, 5.0, 1.0, 9.0, 4.0, 7.0, 3.0, 8.0];
    let node: Node = BoxNode::new()
        .width(20)
        .height(4)
        .border(BorderStyle::Single)
        .child(Sparkline::new(data).style(SparklineStyle::Blocks))
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 25, 6));
}

#[test]
fn snapshot_chat_view_basic() {
    use inky::components::{ChatMessage, ChatView, MessageRole};

    let chat = ChatView::new()
        .message(ChatMessage::new(MessageRole::User, "Hello!"))
        .message(ChatMessage::new(MessageRole::Assistant, "Hi there!"))
        .message(ChatMessage::new(MessageRole::User, "How are you?"));

    let node: Node = BoxNode::new()
        .width(40)
        .height(12)
        .border(BorderStyle::Single)
        .child(chat)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 15));
}

#[test]
fn snapshot_diff_view() {
    use inky::components::{DiffLine, DiffLineKind, DiffView};

    let diff = DiffView::new()
        .line(DiffLine::new(DiffLineKind::Context, Some(1), "fn main() {"))
        .line(DiffLine::new(
            DiffLineKind::Delete,
            Some(2),
            "    println!(\"old\");",
        ))
        .line(DiffLine::new(
            DiffLineKind::Add,
            Some(2),
            "    println!(\"new\");",
        ))
        .line(DiffLine::new(DiffLineKind::Context, Some(3), "}"));

    let node: Node = BoxNode::new()
        .width(40)
        .height(8)
        .border(BorderStyle::Single)
        .child(diff)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 10));
}

#[test]
fn snapshot_markdown_basic() {
    use inky::components::Markdown;

    let md = Markdown::new("# Header\n\nThis is a **bold** and *italic* paragraph.");

    let node: Node = BoxNode::new()
        .width(40)
        .height(8)
        .border(BorderStyle::Single)
        .child(md.to_node())
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 10));
}

#[test]
fn snapshot_markdown_code_block() {
    use inky::components::Markdown;

    let md = Markdown::new("```rust\nfn hello() {\n    println!(\"Hi!\");\n}\n```");

    let node: Node = BoxNode::new()
        .width(40)
        .height(8)
        .border(BorderStyle::Single)
        .child(md.to_node())
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 10));
}

#[test]
fn snapshot_heatmap() {
    use inky::components::{Heatmap, HeatmapPalette};

    let data = vec![
        vec![0.1_f32, 0.3, 0.5, 0.7],
        vec![0.2, 0.4, 0.6, 0.8],
        vec![0.3, 0.5, 0.7, 0.9],
    ];
    let heatmap = Heatmap::new(data).palette(HeatmapPalette::Heat);

    let node: Node = BoxNode::new()
        .width(20)
        .height(8)
        .border(BorderStyle::Single)
        .child(heatmap)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 25, 10));
}

#[test]
fn snapshot_status_bar() {
    use inky::components::{StatusBar, StatusState};

    let status = StatusBar::new()
        .state(StatusState::Executing)
        .message("Processing files...");

    let node: Node = BoxNode::new()
        .width(40)
        .height(3)
        .border(BorderStyle::Single)
        .child(status)
        .into();

    insta::assert_snapshot!(render_snapshot(&node, 45, 5));
}
