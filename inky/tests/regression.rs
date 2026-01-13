#![allow(clippy::unwrap_used)]
//! Regression tests for specific bugs that have been fixed.
//!
//! Each test documents a specific issue and ensures it doesn't regress.
//! Format: regression_issue_<number>_<short_description>

use inky::components::Markdown;
use inky::layout::LayoutEngine;
use inky::node::{BoxNode, Node, TextNode};
use inky::render::Buffer;
use inky::style::{Color, Dimension, FlexDirection};

// =============================================================================
// Buffer Edge Cases
// =============================================================================

/// Bug: Wide character at buffer boundary caused panic
/// Fix: Bounds checking prevents overflow
#[test]
fn regression_wide_char_at_boundary() {
    let mut buf = Buffer::new(10, 1);
    // Writing a wide char at position 9 (last column)
    // Wide chars take 2 columns, so this should be handled gracefully
    buf.write_str(9, 0, "好", Color::White, Color::Black);
    // Should not panic - char may be truncated but no crash
}

/// Bug: Wide character spanning multiple columns at edge
#[test]
fn regression_wide_char_overflow() {
    let mut buf = Buffer::new(5, 1);
    // "日本語" - each char is 2 columns wide
    // Writing at column 0 with width 5 should handle truncation
    buf.write_str(0, 0, "日本語", Color::White, Color::Black);
    // "日本" fits (4 cols), "語" would overflow - should be truncated
}

/// Bug: Resize to zero dimensions caused panic
#[test]
fn regression_resize_to_zero() {
    let mut buf = Buffer::new(10, 10);
    buf.resize(0, 0);
    // Should handle gracefully - zero-sized buffer is valid
    assert_eq!(buf.width(), 0);
    assert_eq!(buf.height(), 0);
}

/// Bug: Resize to 1x1 after having content
#[test]
fn regression_resize_to_minimum() {
    let mut buf = Buffer::new(80, 24);
    buf.write_str(0, 0, "Hello, World!", Color::White, Color::Black);
    buf.resize(1, 1);
    // Content should be truncated, not crash
    assert_eq!(buf.width(), 1);
    assert_eq!(buf.height(), 1);
}

/// Bug: Writing at coordinates beyond buffer bounds
#[test]
fn regression_write_out_of_bounds() {
    let mut buf = Buffer::new(10, 10);
    // All of these should be no-ops, not panics
    buf.write_str(100, 0, "test", Color::White, Color::Black);
    buf.write_str(0, 100, "test", Color::White, Color::Black);
    buf.write_str(100, 100, "test", Color::White, Color::Black);
    buf.write_str(u16::MAX, u16::MAX, "test", Color::White, Color::Black);
}

/// Bug: Getting cell at invalid position
#[test]
fn regression_get_invalid_cell() {
    let buf = Buffer::new(10, 10);
    // Should return None for out-of-bounds
    assert!(buf.get(10, 10).is_none());
    assert!(buf.get(100, 100).is_none());
    assert!(buf.get(u16::MAX, u16::MAX).is_none());
}

/// Bug: Clear on zero-sized buffer
#[test]
fn regression_clear_empty_buffer() {
    let mut buf = Buffer::new(0, 0);
    buf.clear(); // Should not panic
}

// =============================================================================
// Markdown Edge Cases
// =============================================================================

/// Bug: Empty markdown caused issues
#[test]
fn regression_empty_markdown() {
    let md = Markdown::new("");
    let _ = md.to_node(); // Should not panic
}

/// Bug: Markdown with only whitespace
#[test]
fn regression_whitespace_only_markdown() {
    let md = Markdown::new("   \n\n   \t   ");
    let _ = md.to_node(); // Should not panic
}

/// Bug: Markdown with unclosed fence
#[test]
fn regression_unclosed_code_fence() {
    let md = Markdown::new("```rust\nfn main() {");
    let _ = md.to_node(); // Should handle gracefully
}

/// Bug: Markdown with deeply nested lists
#[test]
fn regression_deeply_nested_markdown() {
    let nested = "- Level 1\n  - Level 2\n    - Level 3\n      - Level 4\n        - Level 5";
    let md = Markdown::new(nested);
    let _ = md.to_node(); // Should handle any depth
}

/// Bug: Markdown with very long lines
#[test]
fn regression_very_long_markdown_line() {
    let long_line = "x".repeat(10000);
    let md = Markdown::new(&long_line);
    let _ = md.to_node(); // Should not cause memory issues
}

// =============================================================================
// Layout Edge Cases
// =============================================================================

/// Bug: Layout with zero available space
#[test]
fn regression_layout_zero_space() {
    let node: Node = BoxNode::new()
        .width(Dimension::Length(100.0))
        .height(Dimension::Length(100.0))
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(0, 0).unwrap(); // Should handle zero viewport
}

/// Bug: Layout with extremely large node count
#[test]
fn regression_layout_many_nodes() {
    let mut root = BoxNode::new().flex_direction(FlexDirection::Column);
    for _ in 0..100 {
        root = root.child(TextNode::new("Row"));
    }
    let node: Node = root.into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap(); // Should complete without issue
}

/// Bug: Layout cache invalidation edge case
#[test]
fn regression_layout_cache_invalidation() {
    let node1: Node = BoxNode::new()
        .width(Dimension::Length(50.0))
        .child(TextNode::new("First"))
        .into();

    let node2: Node = BoxNode::new()
        .width(Dimension::Length(100.0))
        .child(TextNode::new("Second"))
        .into();

    let mut engine = LayoutEngine::new();

    // Build first tree
    engine.build(&node1).unwrap();
    engine.compute(80, 24).unwrap();

    // Build second tree - cache should invalidate
    engine.build(&node2).unwrap();
    engine.compute(80, 24).unwrap();

    // Should use correct layout for second tree
    let layout = engine.get(node2.id());
    assert!(layout.is_some());
}

/// Bug: Deeply nested layout tree
#[test]
fn regression_deeply_nested_layout() {
    // Create a deeply nested structure
    let mut node: Node = TextNode::new("Deep").into();
    for _ in 0..50 {
        node = BoxNode::new().child(node).into();
    }

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap(); // Should handle deep nesting
}

// =============================================================================
// Node Edge Cases
// =============================================================================

/// Bug: TextNode with empty string
#[test]
fn regression_empty_text_node() {
    let node: Node = TextNode::new("").into();
    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap();
}

/// Bug: TextNode with only whitespace
#[test]
fn regression_whitespace_text_node() {
    let node: Node = TextNode::new("     ").into();
    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap();
}

/// Bug: BoxNode with no children
#[test]
fn regression_empty_box_node() {
    let node: Node = BoxNode::new().into();
    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap();
}

/// Bug: Negative or zero dimension values
#[test]
fn regression_zero_dimensions() {
    let node: Node = BoxNode::new()
        .width(Dimension::Length(0.0))
        .height(Dimension::Length(0.0))
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap();
}

// =============================================================================
// Unicode Edge Cases
// =============================================================================

/// Bug: Emoji handling
#[test]
fn regression_emoji_text() {
    let mut buf = Buffer::new(20, 1);
    // Emoji may be 1 or 2 columns depending on terminal
    buf.write_str(0, 0, "👍🎉🚀", Color::White, Color::Black);
    // Should not panic
}

/// Bug: Zero-width joiners
#[test]
fn regression_zwj_sequences() {
    let mut buf = Buffer::new(20, 1);
    // Family emoji with ZWJ
    buf.write_str(0, 0, "👨‍👩‍👧‍👦", Color::White, Color::Black);
    // Should handle gracefully
}

/// Bug: Combining characters
#[test]
fn regression_combining_chars() {
    let mut buf = Buffer::new(20, 1);
    // Character with combining diacritical mark
    buf.write_str(0, 0, "é̃", Color::White, Color::Black);
    // Should not panic
}

/// Bug: Right-to-left text
#[test]
fn regression_rtl_text() {
    let mut buf = Buffer::new(20, 1);
    // Hebrew text
    buf.write_str(0, 0, "שלום", Color::White, Color::Black);
    // Should not panic (display may not be correct but shouldn't crash)
}

// =============================================================================
// Color Edge Cases
// =============================================================================

/// Bug: Color::rgb with boundary values
#[test]
fn regression_color_boundaries() {
    let _ = Color::rgb(0, 0, 0);
    let _ = Color::rgb(255, 255, 255);
    let _ = Color::rgb(128, 128, 128);
    // All should be valid
}

/// Bug: Default color handling
#[test]
fn regression_default_colors() {
    let mut buf = Buffer::new(10, 1);
    buf.write_str(0, 0, "test", Color::Default, Color::Default);
    // Should use terminal default colors
}

// =============================================================================
// Concurrent Access
// =============================================================================

/// Bug: Layout engine reuse
#[test]
fn regression_engine_reuse() {
    let mut engine = LayoutEngine::new();

    for i in 0..10 {
        let node: Node = BoxNode::new()
            .width(Dimension::Length((i * 10) as f32))
            .child(TextNode::new(format!("Iteration {}", i)))
            .into();

        engine.build(&node).unwrap();
        engine.compute(80, 24).unwrap();
    }
    // Should handle repeated builds without issues
}
