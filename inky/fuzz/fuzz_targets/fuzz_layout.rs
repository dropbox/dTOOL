//! Fuzz target for Layout computation
//!
//! Tests that arbitrary node trees with various styles compute layout correctly.
//! Run with: cargo +nightly fuzz run fuzz_layout -- -max_total_time=300

#![no_main]

use inky::layout::LayoutEngine;
use inky::prelude::*;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Need at least some bytes for meaningful fuzzing
    if data.len() < 8 {
        return;
    }

    // Parse available size from first 4 bytes
    let width = u16::from_le_bytes([data[0], data[1]]).saturating_add(1);
    let height = u16::from_le_bytes([data[2], data[3]]).saturating_add(1);

    // Limit to reasonable sizes
    if width > 500 || height > 200 {
        return;
    }

    // Create node tree based on fuzz data
    let num_children = (data[4] % 20) as usize; // Max 20 children
    let flex_dir = match data[5] % 2 {
        0 => FlexDirection::Row,
        _ => FlexDirection::Column,
    };

    let mut root = BoxNode::new()
        .flex_direction(flex_dir)
        .width(Dimension::Length(width as f32))
        .height(Dimension::Length(height as f32));

    // Add children based on remaining data
    let child_data = &data[6..];
    for i in 0..num_children {
        if i * 4 >= child_data.len() {
            break;
        }

        let child_type = child_data.get(i * 4).unwrap_or(&0) % 3;
        let flex_grow = child_data.get(i * 4 + 1).unwrap_or(&0) % 10;
        let child_width = child_data.get(i * 4 + 2).unwrap_or(&20);
        let child_height = child_data.get(i * 4 + 3).unwrap_or(&10);

        let child: Node = match child_type {
            0 => {
                // Text node
                let text = format!("Child {}", i);
                TextNode::new(text).into()
            }
            1 => {
                // Box node with fixed size
                BoxNode::new()
                    .width(Dimension::Length(*child_width as f32))
                    .height(Dimension::Length(*child_height as f32))
                    .into()
            }
            _ => {
                // Box node with flex grow
                BoxNode::new().flex_grow(flex_grow as f32).into()
            }
        };

        root = root.child(child);
    }

    // Convert to Node
    let node: Node = root.into();

    // Create layout engine and compute - should never panic
    let mut engine = LayoutEngine::new();
    if engine.build(&node).is_ok() {
        let _ = engine.compute(width, height);

        // Access layout results - should never panic
        for i in 0..(num_children + 1).min(25) {
            let _result = engine.get(NodeId(i as u64));
        }

        // Test get_all
        let _ = engine.get_all();
    }
});
