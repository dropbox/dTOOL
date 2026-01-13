#![allow(clippy::unwrap_used)]
//! Chaos engineering tests for robustness under failure conditions.
//!
//! These tests exercise the system with random and extreme inputs to find
//! edge cases and ensure graceful handling of unexpected conditions.

use inky::components::Markdown;
use inky::layout::LayoutEngine;
use inky::node::{BoxNode, Node, TextNode};
use inky::render::{render_to_buffer, Buffer};
use inky::style::{Color, Dimension, FlexDirection};

// =============================================================================
// Random Input Testing
// =============================================================================

/// Test random terminal sizes
/// Uses a seeded random for reproducibility
#[test]
fn chaos_random_terminal_sizes() {
    // Simple PRNG for reproducibility without rand dependency
    let mut seed: u64 = 12345;
    let mut next_rand = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed
    };

    for _ in 0..1000 {
        let w = (next_rand() % 999 + 1) as u16; // 1-999
        let h = (next_rand() % 999 + 1) as u16; // 1-999

        let mut buf = Buffer::new(w, h);
        let mut engine = LayoutEngine::new();
        let node: Node = BoxNode::new()
            .width(Dimension::Length(w as f32))
            .height(Dimension::Length(h as f32))
            .into();

        engine.build(&node).unwrap();
        engine.compute(w, h).unwrap();
        render_to_buffer(&node, &engine, &mut buf);
    }
}

/// Test with random string lengths
#[test]
fn chaos_random_string_lengths() {
    let mut seed: u64 = 54321;
    let mut next_rand = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed
    };

    for _ in 0..100 {
        let len = (next_rand() % 1000) as usize;
        let s: String = (0..len).map(|i| ((i % 26) as u8 + b'a') as char).collect();

        let mut buf = Buffer::new(80, 24);
        buf.write_str(0, 0, &s, Color::White, Color::Black);
    }
}

// =============================================================================
// Unicode Torture Tests
// =============================================================================

/// Test with extreme unicode edge cases
#[test]
fn chaos_unicode_torture() {
    let torture_strings = [
        "",                   // Empty
        "\0",                 // Null
        "\n\n\n",             // Newlines
        "\r\n\r\n",           // Windows newlines
        "\t\t\t",             // Tabs
        "🎉🎊🎁",             // Emoji
        "مرحبا",              // Arabic (RTL)
        "こんにちは",         // Japanese
        "한글",               // Korean
        "中文",               // Chinese
        "\u{FEFF}",           // BOM
        "\u{200B}",           // Zero-width space
        "\u{200D}",           // Zero-width joiner
        "\u{FFFE}",           // Non-character
        "a\u{0308}",          // Combining characters
        "👨‍👩‍👧‍👦",                 // Family emoji (ZWJ sequence)
        "🏳️‍🌈",                 // Rainbow flag (ZWJ)
        "é̃́",                  // Multiple combining marks
        "\u{061C}",           // Arabic letter mark
        "\u{2060}",           // Word joiner
        "\u{FFFC}",           // Object replacement
        "─│┌┐└┘├┤┬┴┼",        // Box drawing
        "▀▄█▌▐░▒▓",           // Block elements
        "⠀⠁⠂⠃⠄⠅⠆⠇",           // Braille
        "\u{1F1FA}\u{1F1F8}", // Flag (US)
        "ℕℤℚℝℂ",              // Mathematical
        "①②③④⑤",              // Enclosed numbers
    ];

    for s in torture_strings {
        let mut buf = Buffer::new(80, 24);
        buf.write_str(0, 0, s, Color::White, Color::Black);
        // Should not panic
    }
}

/// Test with malformed UTF-8 surrogate pairs
/// Note: Rust strings are always valid UTF-8, so we test edge cases
#[test]
fn chaos_utf8_edge_cases() {
    // Maximum valid unicode codepoint
    let max_valid = char::MAX;
    let mut buf = Buffer::new(80, 24);
    buf.write_str(0, 0, &max_valid.to_string(), Color::White, Color::Black);

    // Test with replacement character
    buf.write_str(0, 0, "\u{FFFD}", Color::White, Color::Black);

    // Test with private use area
    buf.write_str(0, 0, "\u{E000}", Color::White, Color::Black);
}

// =============================================================================
// Boundary Condition Tests
// =============================================================================

/// Test with extreme buffer sizes
#[test]
fn chaos_extreme_buffer_sizes() {
    // Very small
    let mut buf = Buffer::new(1, 1);
    buf.write_str(0, 0, "Hello, World!", Color::White, Color::Black);

    // Very wide
    let mut buf = Buffer::new(1000, 1);
    buf.write_str(0, 0, "x".repeat(2000).as_str(), Color::White, Color::Black);

    // Very tall
    let mut buf = Buffer::new(1, 1000);
    buf.write_str(0, 0, "y", Color::White, Color::Black);

    // Zero-sized (edge case)
    let buf = Buffer::new(0, 0);
    assert_eq!(buf.width(), 0);
    assert_eq!(buf.height(), 0);
}

/// Test with extreme node depths
#[test]
fn chaos_extreme_node_depth() {
    // Very deep nesting
    let mut node: Node = TextNode::new("Deep").into();
    for _ in 0..100 {
        node = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .child(node)
            .into();
    }

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap();
}

/// Test with extreme node width (many siblings)
#[test]
fn chaos_extreme_node_width() {
    let mut root = BoxNode::new().flex_direction(FlexDirection::Column);
    for i in 0..200 {
        root = root.child(TextNode::new(format!("Row {}", i)));
    }
    let node: Node = root.into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).unwrap();
    engine.compute(80, 24).unwrap();
}

/// Test rapid layout changes
#[test]
fn chaos_rapid_layout_changes() {
    let mut engine = LayoutEngine::new();

    for i in 0..100 {
        let node: Node = BoxNode::new()
            .width(Dimension::Length((i * 5 % 200) as f32))
            .height(Dimension::Length((i * 3 % 100) as f32))
            .child(TextNode::new(format!("Iteration {}", i)))
            .into();

        engine.build(&node).unwrap();
        engine.compute(80, 24).unwrap();

        // Force cache invalidation
        engine.invalidate();
    }
}

// =============================================================================
// Markdown Chaos Tests
// =============================================================================

/// Test markdown with random-like content
#[test]
fn chaos_markdown_random() {
    let chaos_content = [
        "# Title\n\n```\n\n```",
        "* * *\n___\n---",
        "[]()",
        "![]()",
        "```\n```\n```",
        "<!-- --> <!-- \n -->",
        "`code` ``code`` ```code```",
        "\\*\\*not bold\\*\\*",
        "[link](http://example.com \"title\")",
        "> > > > > deeply nested",
        "1. One\n   1. Two\n      1. Three",
        "| | |\n|-|-|\n| | |",
        "~~strikethrough~~",
        "^superscript^",
        "~subscript~",
    ];

    for content in chaos_content {
        let md = Markdown::new(content);
        let _ = md.to_node();
    }
}

/// Test markdown with very long content
#[test]
fn chaos_markdown_long_content() {
    // Very long heading
    let long_heading = format!("# {}", "x".repeat(1000));
    let md = Markdown::new(&long_heading);
    let _ = md.to_node();

    // Many list items
    use std::fmt::Write;
    let mut many_items = String::new();
    for i in 0..100 {
        writeln!(many_items, "- Item {}", i).unwrap();
    }
    let md = Markdown::new(&many_items);
    let _ = md.to_node();

    // Many paragraphs
    let mut many_paras = String::new();
    for i in 0..100 {
        writeln!(many_paras, "Paragraph {}.\n", i).unwrap();
    }
    let md = Markdown::new(&many_paras);
    let _ = md.to_node();
}

// =============================================================================
// Stress Testing
// =============================================================================

/// Stress test with alternating operations
#[test]
fn chaos_alternating_operations() {
    let mut buf = Buffer::new(80, 24);

    for i in 0..500 {
        match i % 5 {
            0 => buf.write_str(
                i as u16 % 80,
                i as u16 % 24,
                "X",
                Color::White,
                Color::Black,
            ),
            1 => buf.clear(),
            2 => buf.resize((i as u16 % 100) + 1, (i as u16 % 50) + 1),
            3 => {
                let _ = buf.get(i as u16 % 80, i as u16 % 24);
            }
            _ => {
                let _ = buf.to_text();
            }
        }
    }
}

/// Stress test with rapid resizing
#[test]
fn chaos_rapid_resize() {
    let mut buf = Buffer::new(80, 24);

    for i in 0..200 {
        let w = (i % 100) as u16 + 1;
        let h = (i % 50) as u16 + 1;
        buf.resize(w, h);
        buf.write_str(0, 0, "Content", Color::White, Color::Black);
    }
}

// =============================================================================
// Color Chaos Tests
// =============================================================================

/// Test all RGB color combinations (sampled)
#[test]
fn chaos_color_space() {
    let mut buf = Buffer::new(10, 10);

    // Sample color space
    for r in (0..=255).step_by(51) {
        for g in (0..=255).step_by(51) {
            for b in (0..=255).step_by(51) {
                let fg = Color::rgb(r, g, b);
                let bg = Color::rgb(255 - r, 255 - g, 255 - b);
                buf.write_str(0, 0, "X", fg, bg);
            }
        }
    }
}

/// Test named colors
#[test]
fn chaos_named_colors() {
    let colors = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
        Color::BrightBlack,
        Color::BrightRed,
        Color::BrightGreen,
        Color::BrightYellow,
        Color::BrightBlue,
        Color::BrightMagenta,
        Color::BrightCyan,
        Color::BrightWhite,
        Color::Default,
    ];

    let mut buf = Buffer::new(10, 10);
    for fg in &colors {
        for bg in &colors {
            buf.write_str(0, 0, "X", *fg, *bg);
        }
    }
}
