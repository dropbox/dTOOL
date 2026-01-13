// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for QueryParser in dashflow
//!
//! Tests the structured query parser which converts filter expressions like:
//! - `eq("age", 18)` -> Comparison
//! - `and(gt("age", 18), lt("age", 65))` -> Operation
//!
//! This parser handles untrusted input and is critical to fuzz test.

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow::core::structured_query::parser::QueryParser;
use dashflow::core::structured_query::{Comparator, Operator};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: Parse with no restrictions
        let parser = QueryParser::new();
        let _ = parser.parse(input);

        // Test 2: Parse with restricted comparators
        let parser = QueryParser::new()
            .with_allowed_comparators(vec![Comparator::Eq, Comparator::Ne, Comparator::Gt]);
        let _ = parser.parse(input);

        // Test 3: Parse with restricted operators
        let parser = QueryParser::new()
            .with_allowed_operators(vec![Operator::And, Operator::Or]);
        let _ = parser.parse(input);

        // Test 4: Parse with restricted attributes
        let parser = QueryParser::new()
            .with_allowed_attributes(vec!["age".to_string(), "name".to_string(), "category".to_string()]);
        let _ = parser.parse(input);

        // Test 5: Parse with all restrictions
        let parser = QueryParser::new()
            .with_allowed_comparators(vec![Comparator::Eq, Comparator::In])
            .with_allowed_operators(vec![Operator::And])
            .with_allowed_attributes(vec!["field".to_string()]);
        let _ = parser.parse(input);
    }
});
