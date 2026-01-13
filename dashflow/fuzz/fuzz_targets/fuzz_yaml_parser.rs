// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for YAML parsing in dashflow
//!
//! Tests:
//! 1. YamlOutputParser - parses YAML from LLM outputs
//! 2. serde_yml (the crate YAMLLoader uses internally)
//!
//! SECURITY: YAML parsing can be vulnerable to:
//! - Billion laughs attacks (entity expansion)
//! - Deep nesting causing stack overflow
//! - Type coercion issues (yaml "yes" -> bool true)

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow::core::output_parsers::{OutputParser, YamlOutputParser};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings for YAML (YAML is text-based)
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: YamlOutputParser (dashflow's parser)
        let yaml_parser = YamlOutputParser::new();
        let _ = yaml_parser.parse(input);

        // Test 2: Direct serde_yml parsing (what YAMLLoader uses)
        let _ = serde_yml::from_str::<serde_yml::Value>(input);

        // Test 3: Try parsing as multi-document YAML
        // (YAML supports multiple documents separated by ---)
        for doc in input.split("---") {
            let trimmed = doc.trim();
            if !trimmed.is_empty() {
                let _ = yaml_parser.parse(trimmed);
                let _ = serde_yml::from_str::<serde_yml::Value>(trimmed);
            }
        }

        // Test 4: Test with various YAML-specific edge cases as prefixes
        let yaml_prefixes = [
            "---\n",           // Document start
            "...\n",           // Document end
            "%YAML 1.2\n---\n", // Version directive
            "!!null ",         // Explicit null type
            "!!str ",          // Explicit string type
            "!!int ",          // Explicit int type
            "!!float ",        // Explicit float type
            "&anchor ",        // Anchor definition
            "*anchor",         // Alias reference
        ];

        for prefix in &yaml_prefixes {
            let prefixed = format!("{}{}", prefix, input);
            let _ = yaml_parser.parse(&prefixed);
            let _ = serde_yml::from_str::<serde_yml::Value>(&prefixed);
        }

        // Test 5: Test deeply nested structures
        let nested_map = "a:\n  ".repeat(input.len().min(50)) + input;
        let _ = serde_yml::from_str::<serde_yml::Value>(&nested_map);

        // Test 6: Test with sequences
        let nested_seq = "- ".repeat(input.len().min(50)) + input;
        let _ = serde_yml::from_str::<serde_yml::Value>(&nested_seq);
    }
});
