// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for datetime output parser in dashflow
//!
//! Tests DatetimeOutputParser with:
//! 1. Arbitrary date/time strings to parse
//! 2. Arbitrary format strings (chrono format syntax)
//!
//! SECURITY: Datetime parsing can be slow with malformed inputs;
//! fuzzing helps find edge cases that could cause hangs.

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow::core::output_parsers::{DatetimeOutputParser, OutputParser};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: Default parser with fuzzed input
        let default_parser = DatetimeOutputParser::new();
        let _ = default_parser.parse(input);

        // Test 2: Various standard formats with fuzzed input
        let standard_formats = [
            "%Y-%m-%d",                    // ISO date
            "%Y-%m-%dT%H:%M:%S",          // ISO datetime
            "%Y-%m-%dT%H:%M:%S%.fZ",      // ISO with microseconds
            "%Y/%m/%d %H:%M:%S",          // Common alternative
            "%d/%m/%Y",                    // European date
            "%m/%d/%Y",                    // US date
            "%B %d, %Y",                   // Month name format
            "%Y%m%d%H%M%S",               // Compact format
            "%s",                          // Unix timestamp
        ];

        for format in &standard_formats {
            let parser = DatetimeOutputParser::with_format(*format);
            let _ = parser.parse(input);
        }

        // Test 3: Fuzzed format string with sample dates
        // Split input: first half as format, second half as date string
        let mid = input.len() / 2;
        let (format_part, date_part) = input.split_at(mid);

        // Only try if format_part looks like it could be a format string
        // (has at least one %)
        if format_part.contains('%') {
            let parser = DatetimeOutputParser::with_format(format_part);
            let _ = parser.parse(date_part);
        }

        // Test 4: Parse well-formed dates with fuzzed format
        let sample_dates = [
            "2024-01-15",
            "2024-01-15T10:30:00Z",
            "January 15, 2024",
            "15/01/2024",
            "1705315800",
        ];

        for date in &sample_dates {
            let parser = DatetimeOutputParser::with_format(input);
            let _ = parser.parse(date);
        }
    }
});
