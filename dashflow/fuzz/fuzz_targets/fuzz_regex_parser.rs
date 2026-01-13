// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for regex-based output parsers in dashflow
//!
//! Tests:
//! 1. RegexParser - parses text using regex capture groups
//! 2. RegexDictParser - template-based regex extraction
//! 3. Direct regex compilation with bounded timeouts
//!
//! SECURITY CRITICAL: These parsers accept user-supplied regex patterns,
//! which can cause ReDoS (catastrophic backtracking) if not bounded.

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

use dashflow::core::output_parsers::{OutputParser, RegexParser, RegexDictParser};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(input) = std::str::from_utf8(data) {
        // Split input into pattern and text to parse
        // First half is potential regex pattern, second half is text
        let mid = input.len() / 2;
        let (pattern_part, text_part) = input.split_at(mid);

        // Test 1: RegexParser with fuzzed pattern
        // Try creating a parser with the fuzzed pattern (uses bounded regex internally)
        if let Ok(parser) = RegexParser::new(
            pattern_part,
            vec!["key1", "key2"],
            None::<&str>,
        ) {
            // Parse the text with the fuzzed regex
            let _ = parser.parse(text_part);
        }

        // Test 2: RegexParser with fuzzed text (using safe patterns)
        // Test with various safe patterns to ensure text parsing is robust
        let safe_patterns = [
            r"(\w+)\s+(\w+)",
            r"(\d+)",
            r"^(.*)$",
            r"(.*)",
        ];

        for pattern in &safe_patterns {
            if let Ok(parser) = RegexParser::new(
                *pattern,
                vec!["key1"],
                None::<&str>,
            ) {
                let _ = parser.parse(input);
            }
        }

        // Test 3: RegexDictParser with fuzzed key/format map
        // Create a map from fuzzed input
        let mut key_to_format = HashMap::new();
        // Use the fuzzed input as both key and format value
        if !pattern_part.is_empty() {
            key_to_format.insert(
                "fuzzed_key".to_string(),
                pattern_part.to_string(),
            );
        }

        // Try creating with fuzzed regex pattern template
        if let Ok(parser) = RegexDictParser::try_new(
            key_to_format.clone(),
            Some(pattern_part.to_string()), // Custom template
            None,
        ) {
            let _ = parser.parse(text_part);
        }

        // Test 4: RegexDictParser with safe template, fuzzed input
        let mut safe_key_to_format = HashMap::new();
        safe_key_to_format.insert("name".to_string(), "Name".to_string());
        safe_key_to_format.insert("value".to_string(), "Value".to_string());

        if let Ok(parser) = RegexDictParser::try_new(
            safe_key_to_format,
            None, // Default template
            None,
        ) {
            let _ = parser.parse(input);
        }

        // Test 5: Direct regex compilation to test bounded regex logic
        // This tests the underlying compile_bounded_regex used by RegexParser
        let _ = regex::Regex::new(pattern_part);
    }
});
