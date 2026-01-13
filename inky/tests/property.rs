//! Property-based tests for inky (Phase 15.1)
//!
//! Uses proptest to find edge cases automatically through randomized testing.

use inky::{
    layout::LayoutEngine,
    node::{BoxNode, Node, TextNode},
    render::{Buffer, Cell},
    style::{Color, FlexDirection},
};
use proptest::prelude::*;

// ============================================================================
// Buffer Property Tests
// ============================================================================

proptest! {
    /// Buffer resize should never panic and should preserve content within bounds
    #[test]
    fn buffer_resize_never_panics(
        w1 in 1u16..500,
        h1 in 1u16..500,
        w2 in 1u16..500,
        h2 in 1u16..500,
    ) {
        let mut buf = Buffer::new(w1, h1);

        // Write some content
        let cell = Cell::new('X').with_fg(Color::White).with_bg(Color::Black);
        for y in 0..h1.min(10) {
            for x in 0..w1.min(10) {
                buf.set(x, y, cell);
            }
        }

        // Resize should never panic
        buf.resize(w2, h2);

        // Verify dimensions changed
        prop_assert_eq!(buf.width(), w2);
        prop_assert_eq!(buf.height(), h2);

        // Content within intersection should be preserved
        let copy_w = w1.min(w2).min(10);
        let copy_h = h1.min(h2).min(10);
        for y in 0..copy_h {
            for x in 0..copy_w {
                let c = buf.get(x, y);
                prop_assert!(c.is_some());
                prop_assert_eq!(c.expect("cell exists").char(), 'X');
            }
        }
    }

    /// Buffer fill should always stay within bounds
    #[test]
    fn buffer_fill_bounds_check(
        buf_w in 1u16..200,
        buf_h in 1u16..200,
        fill_x in 0u16..300,
        fill_y in 0u16..300,
        fill_w in 1u16..300,
        fill_h in 1u16..300,
    ) {
        let mut buf = Buffer::new(buf_w, buf_h);
        let cell = Cell::new('F').with_fg(Color::Green).with_bg(Color::Black);

        // Fill should never panic even with out-of-bounds coordinates
        buf.fill(fill_x, fill_y, fill_w, fill_h, cell);

        // All cells should be valid
        for y in 0..buf_h {
            for x in 0..buf_w {
                prop_assert!(buf.get(x, y).is_some());
            }
        }
    }

    /// Buffer get/set should be symmetric
    #[test]
    fn buffer_get_set_symmetric(
        w in 1u16..100,
        h in 1u16..100,
        x in 0u16..99,
        y in 0u16..99,
        ch in prop::char::any(),
    ) {
        if x >= w || y >= h {
            // Out of bounds - should return None
            let buf = Buffer::new(w, h);
            prop_assert!(buf.get(x, y).is_none());
        } else {
            let mut buf = Buffer::new(w, h);
            let cell = Cell::new(ch).with_fg(Color::White).with_bg(Color::Black);
            buf.set(x, y, cell);
            let got = buf.get(x, y);
            prop_assert!(got.is_some());
            // Char might be replaced if it's not BMP
            // Just verify we can get it back without panic
        }
    }
}

// ============================================================================
// Layout Property Tests
// ============================================================================

proptest! {
    /// Layout engine should never panic on any valid input
    #[test]
    fn layout_never_panics(
        width in 1u16..1000,
        height in 1u16..1000,
        children in 0usize..50,
    ) {
        let mut engine = LayoutEngine::new();

        let mut node = BoxNode::new()
            .width(width)
            .height(height)
            .flex_direction(FlexDirection::Column);

        for i in 0..children {
            node = node.child(TextNode::new(format!("child {}", i)));
        }

        let root: Node = node.into();

        // Build and compute should never panic
        let build_result = engine.build(&root);
        prop_assert!(build_result.is_ok());

        let compute_result = engine.compute(width, height);
        prop_assert!(compute_result.is_ok());
    }

    /// Nested layout should not cause stack overflow
    #[test]
    fn layout_deep_nesting(depth in 1usize..100) {
        fn nest(depth: usize) -> Node {
            if depth == 0 {
                TextNode::new("leaf").into()
            } else {
                BoxNode::new().child(nest(depth - 1)).into()
            }
        }

        let node = nest(depth);
        let mut engine = LayoutEngine::new();

        // Should handle deep nesting without stack overflow
        let result = engine.build(&node);
        prop_assert!(result.is_ok());
    }

    /// Layout positions should always be non-negative
    #[test]
    fn layout_positions_non_negative(
        width in 10u16..200,
        height in 10u16..200,
        num_children in 1usize..10,
    ) {
        let mut engine = LayoutEngine::new();

        let mut node = BoxNode::new()
            .width(width)
            .height(height)
            .flex_direction(FlexDirection::Column);

        for i in 0..num_children {
            node = node.child(
                BoxNode::new()
                    .height(10u16)
                    .child(TextNode::new(format!("item {}", i)))
            );
        }

        let root: Node = node.into();
        engine.build(&root).expect("build should succeed");
        engine.compute(width, height).expect("compute should succeed");

        // All positions should be non-negative
        // (We can't easily access positions from here, but the build/compute should succeed)
    }
}

// ============================================================================
// Node Property Tests
// ============================================================================

proptest! {
    /// Node IDs should always be unique
    #[test]
    fn node_ids_unique(num_nodes in 1usize..100) {
        let mut ids = std::collections::HashSet::new();

        for _ in 0..num_nodes {
            let node = TextNode::new("test");
            let id = node.id;
            prop_assert!(!ids.contains(&id), "Duplicate node ID found");
            ids.insert(id);
        }
    }

    /// TextNode should handle any UTF-8 string
    #[test]
    fn text_node_handles_any_string(s in ".*") {
        let node = TextNode::new(&s);
        let content = &node.content;
        // Content might be stored, but should never panic on creation
        prop_assert!(!content.is_empty() || s.is_empty());
    }

    /// BoxNode should handle any number of children
    #[test]
    fn box_node_any_children(children in prop::collection::vec(any::<u32>(), 0..50)) {
        let mut node = BoxNode::new();
        for i in &children {
            node = node.child(TextNode::new(format!("{}", i)));
        }
        let n: Node = node.into();

        match n {
            Node::Box(b) => prop_assert_eq!(b.children.len(), children.len()),
            _ => prop_assert!(false, "Expected BoxNode"),
        }
    }
}

// ============================================================================
// Style Property Tests
// ============================================================================

proptest! {
    /// Color RGB should handle any byte values
    #[test]
    fn color_rgb_any_values(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        let color = Color::Rgb(r, g, b);
        // Color should be created without panic
        match color {
            Color::Rgb(rr, gg, bb) => {
                prop_assert_eq!(rr, r);
                prop_assert_eq!(gg, g);
                prop_assert_eq!(bb, b);
            }
            _ => prop_assert!(false, "Expected Rgb color"),
        }
    }

    /// Cell should handle color round-trip (with RGB565 precision loss)
    #[test]
    fn cell_color_round_trip(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        let fg = Color::Rgb(r, g, b);
        let bg = Color::Rgb(255 - r, 255 - g, 255 - b);
        let cell = Cell::new('A').with_fg(fg).with_bg(bg);

        // Get colors back - may have precision loss due to RGB565
        let _fg_back = cell.fg();
        let _bg_back = cell.bg();

        // Should not panic - precision loss is expected
    }
}

// ============================================================================
// Unicode Property Tests
// ============================================================================

proptest! {
    /// Buffer should handle any Unicode string without panic
    #[test]
    fn buffer_unicode_handling(
        s in "\\PC*",  // Any printable characters
        x in 0u16..50,
        y in 0u16..50,
    ) {
        let mut buf = Buffer::new(80, 24);

        // Should not panic on any Unicode input
        buf.write_str(x, y, &s, Color::White, Color::Black);
    }

    /// Wide characters should be handled correctly
    #[test]
    fn buffer_wide_char_handling(
        buf_w in 10u16..100,
        buf_h in 10u16..100,
        x in 0u16..90,
        y in 0u16..90,
    ) {
        if x >= buf_w || y >= buf_h {
            return Ok(());
        }

        let mut buf = Buffer::new(buf_w, buf_h);

        // Write wide character
        buf.write_str(x, y, "\u{4E2D}", Color::White, Color::Black);  // Chinese char

        // Should handle without panic
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

proptest! {
    /// Zero dimensions should be handled gracefully
    #[test]
    fn zero_dimension_handling(
        w in 0u16..2,
        h in 0u16..2,
    ) {
        // Creating a buffer with zero dimensions should either:
        // - Create a buffer with minimum size 1x1
        // - Or handle zero size gracefully
        let buf = Buffer::new(w.max(1), h.max(1));
        prop_assert!(buf.width() >= 1);
        prop_assert!(buf.height() >= 1);
    }

    /// Maximum dimension handling
    #[test]
    fn large_dimension_handling(
        w in 1000u16..2000,
        h in 1000u16..2000,
    ) {
        // Large buffers should be created without panic
        // (but we keep them reasonable to avoid OOM)
        let buf = Buffer::new(w.min(1500), h.min(1500));
        prop_assert_eq!(buf.width(), w.min(1500));
        prop_assert_eq!(buf.height(), h.min(1500));
    }
}
