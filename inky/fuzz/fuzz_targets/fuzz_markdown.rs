//! Fuzz target for Markdown parsing
//!
//! Tests that arbitrary markdown input never causes panics or crashes.
//! Run with: cargo +nightly fuzz run fuzz_markdown -- -max_total_time=300

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // Limit input size to prevent OOM
    if data.len() > 100_000 {
        return;
    }

    // Parse markdown - should never panic
    let md = inky::components::Markdown::new(data);

    // Convert to node tree - should never panic
    let _node = md.to_node();
});
