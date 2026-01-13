//! Fuzz target for Style and BoxNode creation
//!
//! Tests that arbitrary style operations never cause panics or crashes.
//! Run with: cargo +nightly fuzz run fuzz_style -- -max_total_time=300

#![no_main]

use inky::prelude::*;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Need at least a few bytes for meaningful fuzzing
    if data.len() < 8 {
        return;
    }

    // Create BoxNode with various properties derived from fuzz input
    let mut node = BoxNode::new();

    // Parse dimensions from input
    if data.len() >= 4 {
        let val = u16::from_le_bytes([data[0], data[1]]) as f32;
        if val.is_finite() && val < 10000.0 {
            node = node.width(Dimension::Length(val));
        }
    }

    if data.len() >= 6 {
        let val = u16::from_le_bytes([data[2], data[3]]) as f32;
        if val.is_finite() && val < 10000.0 {
            node = node.height(Dimension::Length(val));
        }
    }

    if data.len() >= 8 {
        let val = (data[4] % 100) as f32;
        node = node.padding(val);
    }

    if data.len() >= 10 {
        let val = (data[6] % 100) as f32;
        node = node.margin(val);
    }

    // Test flex properties
    if data.len() >= 11 {
        let flex_dir = match data[8] % 4 {
            0 => FlexDirection::Row,
            1 => FlexDirection::Column,
            2 => FlexDirection::RowReverse,
            _ => FlexDirection::ColumnReverse,
        };
        node = node.flex_direction(flex_dir);
    }

    if data.len() >= 12 {
        let justify = match data[9] % 6 {
            0 => JustifyContent::Start,
            1 => JustifyContent::End,
            2 => JustifyContent::Center,
            3 => JustifyContent::SpaceBetween,
            4 => JustifyContent::SpaceAround,
            _ => JustifyContent::SpaceEvenly,
        };
        node = node.justify_content(justify);
    }

    if data.len() >= 13 {
        let align = match data[10] % 5 {
            0 => AlignItems::Start,
            1 => AlignItems::End,
            2 => AlignItems::Center,
            3 => AlignItems::Baseline,
            _ => AlignItems::Stretch,
        };
        node = node.align_items(align);
    }

    // Test color creation
    if data.len() >= 16 {
        let _fg = Color::rgb(data[11], data[12], data[13]);
        let _bg = Color::rgb(data[14], data[15], data[16.min(data.len() - 1)]);
    }

    // Test TextNode with style
    if data.len() >= 17 {
        let mut text = TextNode::new("test");
        if data[11] & 1 != 0 {
            text = text.bold();
        }
        if data[11] & 2 != 0 {
            text = text.italic();
        }
        if data[11] & 4 != 0 {
            text = text.underline();
        }
        if data[11] & 8 != 0 {
            text = text.dim();
        }
        node = node.child(text);
    }

    // Convert to Node - should never panic
    let _: Node = node.into();
});
