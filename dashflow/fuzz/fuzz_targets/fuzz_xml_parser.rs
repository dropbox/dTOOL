// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for XMLOutputParser
//!
//! Tests the XML parser with arbitrary input to find crashes,
//! panics, or infinite loops.

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow::core::output_parsers::{OutputParser, XMLOutputParser};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(input) = std::str::from_utf8(data) {
        let parser = XMLOutputParser::new();

        // The parser should never panic on any input
        // It may return Ok or Err, both are acceptable
        let _ = parser.parse(input);
    }
});
