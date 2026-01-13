//! Fuzz target for Buffer operations
//!
//! Tests that arbitrary buffer operations never cause panics or crashes.
//! Run with: cargo +nightly fuzz run fuzz_buffer -- -max_total_time=300

#![no_main]

use inky::prelude::*;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Need at least 4 bytes for dimensions
    if data.len() < 4 {
        return;
    }

    // Parse width and height from first 4 bytes
    let w = u16::from_le_bytes([data[0], data[1]]).saturating_add(1);
    let h = u16::from_le_bytes([data[2], data[3]]).saturating_add(1);

    // Limit dimensions to prevent OOM
    if w > 1000 || h > 1000 {
        return;
    }

    // Create buffer - should never panic
    let mut buf = inky::render::Buffer::new(w, h);

    // Try to interpret remaining data as text
    let text = std::str::from_utf8(&data[4..]).unwrap_or("");

    // Write text at various positions - should never panic
    buf.write_str(0, 0, text, Color::White, Color::Black);

    // Try writing at random positions within bounds
    if data.len() >= 8 {
        let x = u16::from_le_bytes([data[4], data[5]]) % w;
        let y = u16::from_le_bytes([data[6], data[7]]) % h;
        let remaining = std::str::from_utf8(&data[8..]).unwrap_or("");
        buf.write_str(x, y, remaining, Color::Red, Color::Blue);
    }

    // Test resize operations
    if data.len() >= 12 {
        let new_w = u16::from_le_bytes([data[8], data[9]]).saturating_add(1);
        let new_h = u16::from_le_bytes([data[10], data[11]]).saturating_add(1);
        if new_w <= 1000 && new_h <= 1000 {
            buf.resize(new_w, new_h);
        }
    }

    // Test clear
    buf.clear();

    // Test cell access
    for y in 0..h.min(10) {
        for x in 0..w.min(10) {
            let _ = buf.get(x, y);
        }
    }
});
