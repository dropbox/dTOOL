// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for JSON parsing in dashflow
//!
//! Tests multiple JSON parsing paths:
//! 1. JsonOutputParser - parses JSON from LLM outputs
//! 2. State deserialization - serde_json::from_str
//! 3. Output parser deserialization - from_json

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow::core::output_parsers::{JsonOutputParser, OutputParser};
use dashflow::core::deserialization::Deserializable;
use dashflow::core::output_parsers::StrOutputParser;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: JsonOutputParser
        let json_parser = JsonOutputParser::new();
        let _ = json_parser.parse(input);

        // Test 2: Direct serde_json parsing (state deserialization path)
        let _ = serde_json::from_str::<serde_json::Value>(input);

        // Test 3: Output parser deserialization
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(input) {
            // Try deserializing as various parser types
            let _ = StrOutputParser::from_json(&value);
        }
    }
});
