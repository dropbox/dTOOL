// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for Protobuf codec in dashflow-streaming
//!
//! Tests the decode path with arbitrary bytes to ensure
//! no crashes or panics on malformed input.

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow_streaming::codec::{
    decode_message, decode_message_strict, decode_message_with_validation, SchemaCompatibility,
};

fuzz_target!(|data: &[u8]| {
    // Test 1: Raw decode (no compression header)
    let _ = decode_message(data);

    // Test 2: Decode with size limit (strict mode)
    let _ = decode_message_strict(data, 1024 * 1024); // 1MB limit

    // Test 3: Decode with validation - all compatibility modes
    let _ = decode_message_with_validation(data, SchemaCompatibility::Exact);
    let _ = decode_message_with_validation(data, SchemaCompatibility::ForwardCompatible);
    let _ = decode_message_with_validation(data, SchemaCompatibility::BackwardCompatible);

    // Test 4: Decode with various size limits to test edge cases
    let _ = decode_message_strict(data, 0); // Should fail - too small
    let _ = decode_message_strict(data, 1); // Should fail for most input
    let _ = decode_message_strict(data, 100);
    let _ = decode_message_strict(data, 10000);

    // Test 5: If we have at least one byte, test with various header values
    if !data.is_empty() {
        // Test with explicit uncompressed header (0x00)
        let mut uncompressed = vec![0x00];
        uncompressed.extend_from_slice(data);
        let _ = decode_message_strict(&uncompressed, 1024 * 1024);

        // Test with explicit compressed header (0x01)
        let mut compressed = vec![0x01];
        compressed.extend_from_slice(data);
        let _ = decode_message_strict(&compressed, 1024 * 1024);
    }
});
