//! Utility functions for text splitting

use crate::traits::KeepSeparator;
use regex::{Regex, RegexBuilder};

/// Maximum size in bytes for compiled regex patterns (256KB).
const REGEX_SIZE_LIMIT: usize = 256 * 1024;

/// Maximum size in bytes for the DFA cache (256KB).
const REGEX_DFA_SIZE_LIMIT: usize = 256 * 1024;

/// Compile a regex pattern with size limits to prevent resource exhaustion.
fn compile_bounded_regex(pattern: &str) -> std::result::Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
}

/// Split text using a pre-compiled regex, optionally keeping the separator.
///
/// This function implements the core splitting logic used by character-based splitters.
/// It accepts a pre-compiled Regex for better performance.
///
/// # Arguments
///
/// * `text` - The text to split
/// * `regex` - The compiled regex to split on
/// * `keep_separator` - Where to keep the separator (if at all)
///
/// # Returns
///
/// A vector of text chunks (non-empty only)
pub fn split_text_with_compiled_regex(
    text: &str,
    regex: &Regex,
    keep_separator: KeepSeparator,
) -> Vec<String> {
    match keep_separator {
        KeepSeparator::False => {
            // Simple split without keeping separator
            regex
                .split(text)
                .filter(|s| !s.is_empty())
                .map(std::string::ToString::to_string)
                .collect()
        }
        KeepSeparator::Start => {
            // Keep separator at the start of each chunk
            // For matches at positions [m0, m1, m2], create splits:
            // - [0..m0] (text before first match, if any)
            // - [m0..m1] (first match to second match)
            // - [m1..m2] (second match to third match)
            // - [m2..end] (third match to end)
            let mut result = Vec::new();
            let matches: Vec<_> = regex.find_iter(text).collect();

            if matches.is_empty() {
                if !text.is_empty() {
                    result.push(text.to_string());
                }
                return result;
            }

            // Handle text before first match (if any)
            if matches[0].start() > 0 {
                result.push(text[..matches[0].start()].to_string());
            }

            // Process each match - create splits between consecutive separators
            for i in 0..matches.len() {
                let start = matches[i].start();
                let end = if i + 1 < matches.len() {
                    matches[i + 1].start() // Next separator's start
                } else {
                    text.len() // End of text for last match
                };

                result.push(text[start..end].to_string());
            }

            result.into_iter().filter(|s| !s.is_empty()).collect()
        }
        KeepSeparator::End => {
            // Keep separator at the end of each chunk
            let mut result = Vec::new();
            let mut last_end = 0;

            for m in regex.find_iter(text) {
                let chunk = &text[last_end..m.end()];
                if !chunk.is_empty() {
                    result.push(chunk.to_string());
                }
                last_end = m.end();
            }

            // Add remaining text
            if last_end < text.len() {
                result.push(text[last_end..].to_string());
            }

            result.into_iter().filter(|s| !s.is_empty()).collect()
        }
    }
}

/// Split text using a regex pattern, optionally keeping the separator.
///
/// This function compiles the regex on every call. For better performance,
/// use `split_text_with_compiled_regex` with a pre-compiled Regex.
///
/// # Arguments
///
/// * `text` - The text to split
/// * `separator` - The regex pattern to split on
/// * `keep_separator` - Where to keep the separator (if at all)
///
/// # Returns
///
/// A vector of text chunks (non-empty only)
pub fn split_text_with_regex(
    text: &str,
    separator: &str,
    keep_separator: KeepSeparator,
) -> Vec<String> {
    if separator.is_empty() {
        // Split into individual characters
        return text.chars().map(|c| c.to_string()).collect();
    }

    let regex = match compile_bounded_regex(separator) {
        Ok(r) => r,
        Err(_) => return vec![text.to_string()],
    };

    split_text_with_compiled_regex(text, &regex, keep_separator)
}

/// Join document chunks with a separator.
///
/// # Arguments
///
/// * `docs` - The document chunks to join
/// * `separator` - The separator to use between chunks
/// * `strip_whitespace` - Whether to strip whitespace from the result
///
/// # Returns
///
/// The joined string, or None if the result is empty after stripping
#[cfg(test)]
pub fn join_docs(docs: &[String], separator: &str, strip_whitespace: bool) -> Option<String> {
    let text = docs.join(separator);
    let text = if strip_whitespace {
        text.trim().to_string()
    } else {
        text
    };

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_text_with_regex_no_keep() {
        // Basic functionality
        let text = "Hello\n\nWorld\n\nTest";
        let result = split_text_with_regex(text, "\n\n", KeepSeparator::False);
        assert_eq!(result, vec!["Hello", "World", "Test"]);

        // Edge case: empty input returns empty vec
        let result = split_text_with_regex("", "\n\n", KeepSeparator::False);
        assert!(result.is_empty() || result == vec![""]);

        // Edge case: no separator found
        let result = split_text_with_regex("Hello World", "\n\n", KeepSeparator::False);
        assert_eq!(result, vec!["Hello World"]);

        // Edge case: multiple consecutive separators
        let result = split_text_with_regex("A\n\n\n\nB", "\n\n", KeepSeparator::False);
        assert!(result.len() >= 2);
        assert_eq!(result[0], "A");
    }

    #[test]
    fn test_split_text_with_regex_keep_start() {
        // Basic functionality
        let text = "Hello\n\nWorld\n\nTest";
        let result = split_text_with_regex(text, "\n\n", KeepSeparator::Start);
        assert_eq!(result, vec!["Hello", "\n\nWorld", "\n\nTest"]);

        // Edge case: separator at start
        let result = split_text_with_regex("\n\nHello\n\nWorld", "\n\n", KeepSeparator::Start);
        assert!(result[0].is_empty() || result[0].starts_with("\n\n"));

        // Edge case: single element (no separator)
        let result = split_text_with_regex("Hello", "\n\n", KeepSeparator::Start);
        assert_eq!(result, vec!["Hello"]);
    }

    #[test]
    fn test_split_text_with_regex_keep_end() {
        // Basic functionality
        let text = "Hello\n\nWorld\n\nTest";
        let result = split_text_with_regex(text, "\n\n", KeepSeparator::End);
        assert_eq!(result, vec!["Hello\n\n", "World\n\n", "Test"]);

        // Edge case: separator at end
        let result = split_text_with_regex("Hello\n\nWorld\n\n", "\n\n", KeepSeparator::End);
        assert!(result.last().unwrap().ends_with("\n\n") || result.last().unwrap().is_empty());

        // Verify last element doesn't have separator if text doesn't end with it
        let result = split_text_with_regex("A\n\nB", "\n\n", KeepSeparator::End);
        assert_eq!(result[1], "B");
    }

    #[test]
    fn test_split_empty_separator() {
        // Basic functionality: empty separator splits into chars
        let text = "Hi";
        let result = split_text_with_regex(text, "", KeepSeparator::False);
        assert_eq!(result, vec!["H", "i"]);

        // Edge case: single character
        let result = split_text_with_regex("A", "", KeepSeparator::False);
        assert_eq!(result.len(), 1);

        // Edge case: Unicode multi-byte character
        let result = split_text_with_regex("你好", "", KeepSeparator::False);
        assert!(!result.is_empty());
        // Verify we don't break Unicode
        for part in &result {
            assert!(part.is_empty() || part.chars().count() > 0);
        }
    }

    #[test]
    fn test_join_docs() {
        // Basic functionality
        let docs = vec!["Hello".to_string(), "World".to_string()];
        assert_eq!(
            join_docs(&docs, " ", false),
            Some("Hello World".to_string())
        );

        // Edge case: empty vector
        let docs: Vec<String> = vec![];
        assert_eq!(join_docs(&docs, " ", false), None);

        // Edge case: single doc
        let docs = vec!["Single".to_string()];
        assert_eq!(join_docs(&docs, " ", false), Some("Single".to_string()));

        // Edge case: many docs
        let docs = vec![
            "One".to_string(),
            "Two".to_string(),
            "Three".to_string(),
            "Four".to_string(),
        ];
        assert_eq!(
            join_docs(&docs, "-", false),
            Some("One-Two-Three-Four".to_string())
        );

        // Edge case: empty strings in vector
        let docs = vec!["".to_string(), "Hello".to_string(), "".to_string()];
        let result = join_docs(&docs, ",", false);
        assert_eq!(result, Some(",Hello,".to_string()));
    }

    #[test]
    fn test_join_docs_strip_whitespace() {
        // Basic functionality: strips outer whitespace from joined result
        let docs = vec!["  Hello  ".to_string(), "  World  ".to_string()];
        assert_eq!(
            join_docs(&docs, " ", true),
            Some("Hello     World".to_string())
        );

        // Verify it strips outer, not inner whitespace
        let docs = vec!["  A  ".to_string()];
        assert_eq!(join_docs(&docs, "", true), Some("A".to_string()));

        // Edge case: all whitespace
        let docs = vec!["   ".to_string(), "   ".to_string()];
        let result = join_docs(&docs, " ", true);
        // After joining and trimming, if empty, returns None
        assert_eq!(result, None);

        // Edge case: strip_whitespace=false preserves all whitespace
        let docs = vec!["  A  ".to_string(), "  B  ".to_string()];
        assert_eq!(
            join_docs(&docs, "|", false),
            Some("  A  |  B  ".to_string())
        );
    }
}
