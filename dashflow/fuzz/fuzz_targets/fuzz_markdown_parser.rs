// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for Markdown parsing in dashflow
//!
//! Tests:
//! 1. pulldown_cmark (the crate MarkdownLoader uses internally)
//! 2. MarkdownListOutputParser - extracts bullet lists from markdown
//! 3. NumberedListOutputParser - extracts numbered lists
//!
//! SECURITY: Markdown parsing can have issues with:
//! - Deeply nested structures (stack overflow)
//! - Malformed link/image references
//! - HTML injection in inline HTML blocks

#![no_main]

use libfuzzer_sys::fuzz_target;

use dashflow::core::output_parsers::{
    MarkdownListOutputParser, NumberedListOutputParser, OutputParser,
};
use pulldown_cmark::{Parser, Options, html};

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings (Markdown is text-based)
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: pulldown_cmark parsing (what MarkdownLoader uses)
        let parser = Parser::new(input);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        // Test 2: pulldown_cmark with all options enabled
        let options = Options::all();
        let parser = Parser::new_ext(input, options);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        // Test 3: MarkdownListOutputParser (bullet lists)
        let bullet_parser = MarkdownListOutputParser::new();
        let _ = bullet_parser.parse(input);

        // Test 4: NumberedListOutputParser (numbered lists)
        let numbered_parser = NumberedListOutputParser::new();
        let _ = numbered_parser.parse(input);

        // Test 5: Test with markdown-specific structures
        let md_prefixes = [
            "# ",              // H1
            "## ",             // H2
            "### ",            // H3
            "- ",              // Bullet
            "* ",              // Alternative bullet
            "1. ",             // Numbered
            "> ",              // Blockquote
            "```\n",           // Code fence start
            "```rust\n",       // Code fence with lang
            "    ",            // Indented code block
            "---\n",           // Horizontal rule
            "***\n",           // Alternative rule
            "[",               // Link start
            "![",              // Image start
        ];

        for prefix in &md_prefixes {
            let prefixed = format!("{}{}", prefix, input);
            let _ = bullet_parser.parse(&prefixed);
            let _ = numbered_parser.parse(&prefixed);

            // Also test pulldown_cmark with each prefix
            let parser = Parser::new(&prefixed);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);
        }

        // Test 6: Test deeply nested structures
        let nested_bullets = "- ".repeat(input.len().min(50)) + input;
        let _ = bullet_parser.parse(&nested_bullets);

        let parser = Parser::new(&nested_bullets);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        // Test 7: Deeply nested blockquotes
        let nested_quotes = "> ".repeat(input.len().min(50)) + input;
        let parser = Parser::new(&nested_quotes);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        // Test 8: Nested headers
        let nested_headers = "# ".repeat(input.len().min(6)) + input;
        let parser = Parser::new(&nested_headers);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
    }
});
