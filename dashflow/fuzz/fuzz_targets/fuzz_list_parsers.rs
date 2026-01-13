// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for list parsers in dashflow
//!
//! Tests multiple list parsing paths:
//! 1. CommaSeparatedListOutputParser - parses comma-separated lists
//! 2. NumberedListOutputParser - parses numbered lists (1. 2. 3.)
//! 3. MarkdownListOutputParser - parses markdown lists (- * +)
//! 4. LineListOutputParser - parses newline-separated lists
//! 5. QuestionListOutputParser - parses questions from text

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow::core::output_parsers::{
    CommaSeparatedListOutputParser, LineListOutputParser, MarkdownListOutputParser,
    NumberedListOutputParser, OutputParser, QuestionListOutputParser,
};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: CommaSeparatedListOutputParser (unit struct)
        let parser = CommaSeparatedListOutputParser;
        let _ = parser.parse(input);

        // Test 2: NumberedListOutputParser (struct with regex pattern)
        let parser = NumberedListOutputParser::new();
        let _ = parser.parse(input);

        // Test 3: MarkdownListOutputParser (struct with regex pattern)
        let parser = MarkdownListOutputParser::new();
        let _ = parser.parse(input);

        // Test 4: LineListOutputParser (unit struct)
        let parser = LineListOutputParser;
        let _ = parser.parse(input);

        // Test 5: QuestionListOutputParser (unit struct)
        let parser = QuestionListOutputParser;
        let _ = parser.parse(input);
    }
});
