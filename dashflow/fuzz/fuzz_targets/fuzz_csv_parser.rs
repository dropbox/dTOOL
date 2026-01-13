// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for CSV/TSV parsing in dashflow
//!
//! Tests the csv crate parsing that CSVLoader and TSVLoader use internally with:
//! 1. Arbitrary CSV content
//! 2. Various delimiter combinations
//! 3. Malformed CSV (unbalanced quotes, newlines in fields, etc.)
//!
//! SECURITY: CSV parsing can be tricky with quote handling and escaping.
//! Fuzzing helps find edge cases that could cause panics or infinite loops.

#![no_main]

use libfuzzer_sys::fuzz_target;

// Test the csv parsing that dashflow's CSVLoader uses internally
use csv::ReaderBuilder;

fuzz_target!(|data: &[u8]| {
    // Test 1: Default CSV parsing (comma delimiter)
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b',')
        .from_reader(data);

    // Try to read headers
    let _ = reader.headers();

    // Try to read all records
    for result in reader.records() {
        let _ = result;
    }

    // Test 2: TSV parsing (tab delimiter)
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b'\t')
        .from_reader(data);

    for result in reader.records() {
        let _ = result;
    }

    // Test 3: Different delimiters
    let delimiters = [b',', b';', b'|', b'\t', b' ', b':'];
    for &delim in &delimiters {
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .delimiter(delim)
            .from_reader(data);

        for result in reader.records() {
            let _ = result;
        }
    }

    // Test 4: No headers mode
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b',')
        .from_reader(data);

    for result in reader.records() {
        let _ = result;
    }

    // Test 5: Flexible mode (variable column count)
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(data);

    for result in reader.records() {
        let _ = result;
    }

    // Test 6: Different quote characters
    let quote_chars = [b'"', b'\'', b'`'];
    for &quote in &quote_chars {
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .quote(quote)
            .from_reader(data);

        for result in reader.records() {
            let _ = result;
        }
    }

    // Test 7: Double quote escape mode
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .double_quote(true)
        .from_reader(data);

    for result in reader.records() {
        let _ = result;
    }

    // Test 8: Escape mode (backslash escapes)
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .escape(Some(b'\\'))
        .from_reader(data);

    for result in reader.records() {
        let _ = result;
    }
});
