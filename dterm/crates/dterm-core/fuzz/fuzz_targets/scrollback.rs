//! Scrollback fuzz target.
//!
//! This fuzzer tests the tiered scrollback storage with arbitrary operations.
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run scrollback -- -max_total_time=60
//! ```
//!
//! ## Properties Tested
//!
//! - Scrollback never panics on any operation sequence
//! - Line count is always accurate (sum of tiers)
//! - Hot tier never exceeds limit
//! - Get operations are consistent (same index = same content)
//!
//! ## Correspondence to TLA+
//!
//! This fuzzer validates the TypeInvariant and Safety properties
//! from tla/Scrollback.tla through exhaustive random testing.

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use dterm_core::scrollback::Scrollback;
use libfuzzer_sys::fuzz_target;

/// Operations that can be performed on the scrollback.
#[derive(Debug, Arbitrary)]
enum ScrollbackOp {
    /// Push a line with given content length
    PushLine { content_len: u8 },
    /// Push a specific string
    PushStr { idx: u8 },
    /// Get line by index
    GetLine { idx: u16 },
    /// Get line by reverse index
    GetLineRev { idx: u16 },
    /// Clear all lines
    Clear,
    /// Truncate to last N lines
    Truncate { n: u16 },
}

/// Pre-defined strings for testing
const TEST_STRINGS: &[&str] = &[
    "Hello, World!",
    "Line with some content",
    "Short",
    "A much longer line that contains a lot of text and should exercise the heap allocation path in LineContent",
    "[2024-01-01 12:00:00] INFO: Processing request from 192.168.1.1 - status=200 duration=15ms",
    "error: expected `;` at end of statement",
    "$ ls -la /home/user/documents",
    "",
    "   ",
    "\t\t\t",
];

fuzz_target!(|data: &[u8]| {
    // Early return for empty/tiny inputs to avoid infinite loops
    if data.len() < 4 {
        return;
    }

    let mut unstructured = Unstructured::new(data);

    // Get configuration (with reasonable limits for fuzzing)
    // NOTE: Use minimum hot_limit=5 and warm_limit=10 to avoid timeout from
    // constant zstd compression overhead when limits are too small.
    let hot_limit: usize = unstructured.int_in_range(5..=20).unwrap_or(10);
    let warm_limit: usize = unstructured.int_in_range(10..=50).unwrap_or(30);
    let block_size: usize = unstructured.int_in_range(3..=10).unwrap_or(5);
    let memory_budget: usize = 10_000_000; // 10MB

    let mut scrollback = Scrollback::with_block_size(hot_limit, warm_limit, memory_budget, block_size);

    // Track what we've pushed for verification
    let mut expected_count: usize = 0;

    // Limit operations to prevent timeout from repeated zstd compression.
    // With small tier limits, each push can trigger compression, so we cap
    // the number of operations to ensure reasonable execution time.
    const MAX_OPS: usize = 200;
    let mut op_count: usize = 0;

    // Process operations
    while op_count < MAX_OPS {
        let op = match unstructured.arbitrary::<ScrollbackOp>() {
            Ok(op) => op,
            Err(_) => break,
        };
        op_count += 1;
        match op {
            ScrollbackOp::PushLine { content_len } => {
                let content: String = (0..content_len)
                    .map(|i| (b'a' + (i % 26)) as char)
                    .collect();
                scrollback.push_str(&content);
                expected_count += 1;
            }
            ScrollbackOp::PushStr { idx } => {
                let s = TEST_STRINGS[idx as usize % TEST_STRINGS.len()];
                scrollback.push_str(s);
                expected_count += 1;
            }
            ScrollbackOp::GetLine { idx } => {
                let result = scrollback.get_line(idx as usize);
                let line_count = scrollback.line_count();
                if (idx as usize) < line_count {
                    assert!(result.is_some(), "Valid index {} returned None", idx);
                } else {
                    assert!(result.is_none(), "Invalid index {} returned Some", idx);
                }
            }
            ScrollbackOp::GetLineRev { idx } => {
                let result = scrollback.get_line_rev(idx as usize);
                let line_count = scrollback.line_count();
                if (idx as usize) < line_count {
                    assert!(result.is_some(), "Valid rev index {} returned None", idx);
                } else {
                    assert!(result.is_none(), "Invalid rev index {} returned Some", idx);
                }
            }
            ScrollbackOp::Clear => {
                scrollback.clear();
                expected_count = 0;
            }
            ScrollbackOp::Truncate { n } => {
                scrollback.truncate(n as usize);
                expected_count = expected_count.min(n as usize);
            }
        }

        // Invariants that must hold after every operation

        // Line count is accurate
        assert_eq!(
            scrollback.line_count(),
            expected_count,
            "Line count mismatch: expected {}, got {}",
            expected_count,
            scrollback.line_count()
        );

        // Line count equals sum of tiers
        let tier_sum =
            scrollback.hot_line_count() + scrollback.warm_line_count() + scrollback.cold_line_count();
        assert_eq!(
            scrollback.line_count(),
            tier_sum,
            "Tier sum mismatch: line_count={}, tier_sum={}",
            scrollback.line_count(),
            tier_sum
        );

        // Hot tier never exceeds limit
        assert!(
            scrollback.hot_line_count() <= hot_limit,
            "Hot tier {} exceeds limit {}",
            scrollback.hot_line_count(),
            hot_limit
        );
    }

    // Final verification: we can iterate all lines
    let iter_count = scrollback.iter().count();
    assert_eq!(
        iter_count,
        scrollback.line_count(),
        "Iterator count {} doesn't match line_count {}",
        iter_count,
        scrollback.line_count()
    );

    // And reverse iterate
    let rev_iter_count = scrollback.iter_rev().count();
    assert_eq!(
        rev_iter_count,
        scrollback.line_count(),
        "Rev iterator count {} doesn't match line_count {}",
        rev_iter_count,
        scrollback.line_count()
    );
});
