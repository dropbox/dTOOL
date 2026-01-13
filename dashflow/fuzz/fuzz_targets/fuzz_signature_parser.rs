// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for signature parsing in DashOptimize
//!
//! Tests make_signature() with arbitrary input strings
//! to ensure no crashes or panics on malformed signatures.

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow::optimize::signature::make_signature;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: Parse as signature string
        let _ = make_signature(input, "test instructions");

        // Test 2: Parse with various instruction strings
        let _ = make_signature(input, input);

        // Test 3: Test with empty instructions
        let _ = make_signature(input, "");
    }
});
