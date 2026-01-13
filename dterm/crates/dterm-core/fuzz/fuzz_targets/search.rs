//! Search index fuzz target.
//!
//! This fuzzer tests the trigram search index with arbitrary operations.
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run search -- -max_total_time=60
//! ```
//!
//! ## Properties Tested
//!
//! - Search index never panics on any input
//! - No false negatives (if a line contains the query, it's returned)
//! - Line count is accurate
//!
//! ## Verification Approach
//!
//! For each indexed line that contains the query as a substring,
//! the search must return that line number (no false negatives).

#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use dterm_core::search::SearchIndex;

/// Operations that can be performed on the search index.
#[derive(Debug)]
enum SearchOp<'a> {
    /// Index a line
    IndexLine { line_num: usize, text: &'a str },
    /// Search for a query
    Search { query: &'a str },
}

impl<'a> Arbitrary<'a> for SearchOp<'a> {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let choice: u8 = u.int_in_range(0..=1)?;
        match choice {
            0 => {
                let line_num = u.int_in_range(0..=10000)?;
                let len = u.int_in_range(0..=200)?;
                let bytes = u.bytes(len)?;
                // Convert to valid UTF-8 by replacing invalid sequences
                let text = std::str::from_utf8(bytes).unwrap_or("");
                Ok(SearchOp::IndexLine { line_num, text })
            }
            _ => {
                let len = u.int_in_range(0..=50)?;
                let bytes = u.bytes(len)?;
                let query = std::str::from_utf8(bytes).unwrap_or("");
                Ok(SearchOp::Search { query })
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // Early return for empty/tiny inputs to avoid infinite loops
    if data.len() < 4 {
        return;
    }

    let mut unstructured = Unstructured::new(data);
    let mut index = SearchIndex::new();

    // Track indexed lines for verification
    let mut indexed_lines: Vec<(usize, String)> = Vec::new();

    // Process operations
    while let Ok(op) = unstructured.arbitrary::<SearchOp>() {
        match op {
            SearchOp::IndexLine { line_num, text } => {
                // Only index non-empty text
                if !text.is_empty() {
                    index.index_line(line_num, text);
                    indexed_lines.push((line_num, text.to_string()));
                }
            }
            SearchOp::Search { query } => {
                // Skip short queries (trigram index requires >= 3 chars)
                if query.len() >= 3 {
                    let results: Vec<u32> = index.search(query).collect();

                    // Verify no false negatives:
                    // Every line that contains the query must be in results
                    for (line_num, text) in &indexed_lines {
                        if text.contains(query) {
                            // This line should be in results (no false negatives)
                            // Note: We allow false positives
                            assert!(
                                results.contains(&(*line_num as u32)),
                                "False negative: line {} contains '{}' but wasn't returned. \
                                 Line text: '{}', Results: {:?}",
                                line_num,
                                query,
                                text,
                                results
                            );
                        }
                    }
                }
            }
        }
    }

    // Verify index length consistency
    if !indexed_lines.is_empty() {
        let max_line = indexed_lines.iter().map(|(n, _)| *n).max().unwrap();
        assert!(
            index.len() >= max_line,
            "Index length {} < max indexed line {}",
            index.len(),
            max_line
        );
    }
});
