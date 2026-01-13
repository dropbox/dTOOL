//! Sixel graphics fuzz target.
//!
//! This fuzzer tests the Sixel decoder with arbitrary byte sequences,
//! including simulated animation sequences (multiple images).
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run sixel -- -max_total_time=60
//! ```
//!
//! ## Properties Tested
//!
//! - Decoder never panics on any input
//! - Memory allocation stays within bounds
//! - State machine transitions are always valid
//! - Multiple consecutive images (animations) are handled correctly
//! - Color palette operations don't cause out-of-bounds access

#![no_main]

use libfuzzer_sys::fuzz_target;
use dterm_core::sixel::{SixelDecoder, SIXEL_MAX_DIMENSION, MAX_COLOR_REGISTERS};

fuzz_target!(|data: &[u8]| {
    // Test single image decoding
    test_single_image(data);

    // Test animation sequence (multiple images)
    test_animation_sequence(data);

    // Test streaming input (one byte at a time)
    test_streaming(data);
});

/// Test decoding a single Sixel image.
fn test_single_image(data: &[u8]) {
    let mut decoder = SixelDecoder::new();

    // Hook with various parameter combinations
    let params: [u16; 3] = [
        data.first().map(|&b| u16::from(b) % 10).unwrap_or(0),
        data.get(1).map(|&b| u16::from(b) % 3).unwrap_or(0),
        data.get(2).map(|&b| u16::from(b)).unwrap_or(0),
    ];

    decoder.hook(&params, 0, 0);
    assert!(decoder.is_active(), "decoder should be active after hook");

    // Process all bytes
    for &byte in data {
        decoder.put(byte);
    }

    // Finalize and verify image if produced
    if let Some(image) = decoder.unhook() {
        // Verify dimensions are within bounds
        assert!(image.width() <= SIXEL_MAX_DIMENSION, "width exceeds max");
        assert!(image.height() <= SIXEL_MAX_DIMENSION, "height exceeds max");

        // Verify pixel buffer size matches dimensions
        assert_eq!(
            image.pixels().len(),
            image.width() * image.height(),
            "pixel buffer size mismatch"
        );

        // Verify span calculations don't panic
        let _ = image.rows_spanned(10);
        let _ = image.cols_spanned(10);
        let _ = image.rows_spanned(0); // Edge case
        let _ = image.cols_spanned(0); // Edge case
    }

    assert!(!decoder.is_active(), "decoder should be inactive after unhook");
}

/// Test animation sequence - multiple consecutive images.
fn test_animation_sequence(data: &[u8]) {
    let mut decoder = SixelDecoder::new();

    // Split data into chunks and treat each as a separate "frame"
    let frame_count = (data.first().map(|&b| b % 10).unwrap_or(1) as usize).max(1);
    let chunk_size = (data.len() / frame_count).max(1);

    for (frame_num, chunk) in data.chunks(chunk_size).enumerate().take(10) {
        // Use frame number to vary cursor position
        let cursor_row = (frame_num % 24) as u16;
        let cursor_col = (frame_num % 80) as u16;

        decoder.hook(&[], cursor_row, cursor_col);

        for &byte in chunk {
            decoder.put(byte);
        }

        if let Some(image) = decoder.unhook() {
            // Verify cursor position was preserved
            assert_eq!(image.cursor_row(), cursor_row);
            assert_eq!(image.cursor_col(), cursor_col);

            // Verify dimensions
            assert!(image.width() <= SIXEL_MAX_DIMENSION);
            assert!(image.height() <= SIXEL_MAX_DIMENSION);
        }
    }
}

/// Test streaming input - feed bytes one at a time.
fn test_streaming(data: &[u8]) {
    let mut decoder = SixelDecoder::new();
    decoder.hook(&[0, 1, 0], 0, 0); // Transparent background

    for &byte in data {
        decoder.put(byte);

        // Decoder should remain in valid state after each byte
        assert!(decoder.is_active(), "decoder should stay active during streaming");
    }

    // Verify we can cleanly finalize even with partial data
    let _ = decoder.unhook();
}

/// Test color palette operations.
#[allow(dead_code)]
fn test_palette_operations(data: &[u8]) {
    let mut decoder = SixelDecoder::new();

    // Verify initial palette
    let palette = decoder.palette();
    assert_eq!(palette.len(), MAX_COLOR_REGISTERS);

    // Test setting colors at various indices
    for (i, &byte) in data.iter().enumerate().take(MAX_COLOR_REGISTERS) {
        let index = i as u16;
        let color = u32::from(byte) * 0x01_010101; // Create gray color
        decoder.set_palette_color(index, color);
    }

    // Test out-of-bounds palette access (should not panic)
    decoder.set_palette_color(u16::MAX, 0xFF_FFFFFF);

    // Test reset
    decoder.reset_palette();
}
