//! Inky benchmark implementations.
//!
//! These functions implement the standard benchmark scenarios using inky.
//!
//! ## API Variants
//!
//! Each benchmark has two variants:
//! - Regular: Uses `build()` + `compute()` (always uses Taffy)
//! - Fast: Uses `layout()` (SimpleLayout fast path when applicable)

use inky::layout::LayoutEngine;
use inky::node::{BoxNode, Node, TextNode};
use inky::render::{render_to_buffer, Buffer};
use inky::style::FlexDirection;

use super::scenarios::{generate_grid_text, generate_messages};

/// Create an empty buffer (startup cost).
pub fn create_empty_buffer(width: u16, height: u16) -> Buffer {
    Buffer::new(width, height)
}

/// Render a text grid (layout + render) using Taffy.
pub fn render_text_grid(rows: usize, cols: usize) -> Buffer {
    let grid = generate_grid_text(rows, cols);

    // Build node tree
    let row_nodes: Vec<Node> = grid
        .iter()
        .map(|row| {
            let col_nodes: Vec<Node> = row.iter().map(|text| TextNode::new(text).into()).collect();
            BoxNode::new()
                .flex_direction(FlexDirection::Row)
                .children(col_nodes)
                .into()
        })
        .collect();

    let root: Node = BoxNode::new()
        .width(200)
        .height(50)
        .flex_direction(FlexDirection::Column)
        .children(row_nodes)
        .into();

    // Layout and render (Taffy path)
    let mut engine = LayoutEngine::new();
    engine.build(&root).expect("layout build");
    engine.compute(200, 50).expect("layout compute");

    let mut buffer = Buffer::new(200, 50);
    render_to_buffer(&root, &engine, &mut buffer);
    buffer
}

/// Render a text grid using SimpleLayout fast path.
pub fn render_text_grid_fast(rows: usize, cols: usize) -> Buffer {
    let grid = generate_grid_text(rows, cols);

    // Build node tree
    let row_nodes: Vec<Node> = grid
        .iter()
        .map(|row| {
            let col_nodes: Vec<Node> = row.iter().map(|text| TextNode::new(text).into()).collect();
            BoxNode::new()
                .flex_direction(FlexDirection::Row)
                .children(col_nodes)
                .into()
        })
        .collect();

    let root: Node = BoxNode::new()
        .width(200)
        .height(50)
        .flex_direction(FlexDirection::Column)
        .children(row_nodes)
        .into();

    // Layout and render (SimpleLayout fast path)
    let mut engine = LayoutEngine::new();
    engine.layout(&root, 200, 50).expect("layout");

    let mut buffer = Buffer::new(200, 50);
    render_to_buffer(&root, &engine, &mut buffer);
    buffer
}

/// Render a chat UI (realistic app scenario) using Taffy.
pub fn render_chat_ui(message_count: usize) -> Buffer {
    let messages = generate_messages(message_count);

    // Build node tree
    // Header
    let header: Node = BoxNode::new()
        .height(1)
        .child(TextNode::new("Chat with Claude"))
        .into();

    // Message list
    let message_nodes: Vec<Node> = messages
        .iter()
        .map(|(role, content)| {
            BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .child(TextNode::new(format!("{}:", role)))
                .child(TextNode::new(content))
                .into()
        })
        .collect();

    let message_list: Node = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .children(message_nodes)
        .into();

    // Input area
    let input: Node = BoxNode::new()
        .height(3)
        .child(TextNode::new("> Type your message here..."))
        .into();

    // Full layout
    let root: Node = BoxNode::new()
        .width(120)
        .height(40)
        .flex_direction(FlexDirection::Column)
        .child(header)
        .child(message_list)
        .child(input)
        .into();

    // Layout and render (Taffy path)
    let mut engine = LayoutEngine::new();
    engine.build(&root).expect("layout build");
    engine.compute(120, 40).expect("layout compute");

    let mut buffer = Buffer::new(120, 40);
    render_to_buffer(&root, &engine, &mut buffer);
    buffer
}

/// Render a chat UI using SimpleLayout fast path.
pub fn render_chat_ui_fast(message_count: usize) -> Buffer {
    let messages = generate_messages(message_count);

    // Build node tree
    // Header
    let header: Node = BoxNode::new()
        .height(1)
        .child(TextNode::new("Chat with Claude"))
        .into();

    // Message list
    let message_nodes: Vec<Node> = messages
        .iter()
        .map(|(role, content)| {
            BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .child(TextNode::new(format!("{}:", role)))
                .child(TextNode::new(content))
                .into()
        })
        .collect();

    let message_list: Node = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .children(message_nodes)
        .into();

    // Input area
    let input: Node = BoxNode::new()
        .height(3)
        .child(TextNode::new("> Type your message here..."))
        .into();

    // Full layout
    let root: Node = BoxNode::new()
        .width(120)
        .height(40)
        .flex_direction(FlexDirection::Column)
        .child(header)
        .child(message_list)
        .child(input)
        .into();

    // Layout and render (SimpleLayout fast path)
    let mut engine = LayoutEngine::new();
    engine.layout(&root, 120, 40).expect("layout");

    let mut buffer = Buffer::new(120, 40);
    render_to_buffer(&root, &engine, &mut buffer);
    buffer
}

/// Full screen redraw (worst case) using Taffy.
pub fn full_redraw(width: u16, height: u16) -> Buffer {
    // Create a tree that fills the entire screen with text
    let rows: Vec<Node> = (0..height)
        .map(|i| {
            TextNode::new(format!(
                "{:width$}",
                format!("Line {} with content that fills the row", i),
                width = width as usize
            ))
            .into()
        })
        .collect();

    let root: Node = BoxNode::new()
        .width(width)
        .height(height)
        .flex_direction(FlexDirection::Column)
        .children(rows)
        .into();

    // Layout and render (Taffy path)
    let mut engine = LayoutEngine::new();
    engine.build(&root).expect("layout build");
    engine.compute(width, height).expect("layout compute");

    let mut buffer = Buffer::new(width, height);
    render_to_buffer(&root, &engine, &mut buffer);
    buffer
}

/// Full screen redraw using SimpleLayout fast path.
pub fn full_redraw_fast(width: u16, height: u16) -> Buffer {
    // Create a tree that fills the entire screen with text
    let rows: Vec<Node> = (0..height)
        .map(|i| {
            TextNode::new(format!(
                "{:width$}",
                format!("Line {} with content that fills the row", i),
                width = width as usize
            ))
            .into()
        })
        .collect();

    let root: Node = BoxNode::new()
        .width(width)
        .height(height)
        .flex_direction(FlexDirection::Column)
        .children(rows)
        .into();

    // Layout and render (SimpleLayout fast path)
    let mut engine = LayoutEngine::new();
    engine.layout(&root, width, height).expect("layout");

    let mut buffer = Buffer::new(width, height);
    render_to_buffer(&root, &engine, &mut buffer);
    buffer
}
