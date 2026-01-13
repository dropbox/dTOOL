//! Common utilities for integration tests

#![allow(clippy::expect_used)]

use std::env;

/// Load environment variables from .env file
pub fn load_test_env() {
    dotenvy::dotenv().ok();
}

/// Get OpenAI API key from environment
///
/// Panics with clear message if OPENAI_API_KEY is not set
pub fn get_openai_key() -> String {
    env::var("OPENAI_API_KEY").expect(
        "OPENAI_API_KEY environment variable required for integration tests.\n\
         Set it with: export OPENAI_API_KEY=sk-...\n\
         Or create a .env file with: OPENAI_API_KEY=sk-...",
    )
}

/// Check if OpenAI API key is available (non-panicking)
pub fn has_openai_key() -> bool {
    env::var("OPENAI_API_KEY").is_ok()
}

/// Verify answer contains expected keywords (case-insensitive)
///
/// Returns true if answer contains ANY of the expected keywords
pub fn verify_answer_quality(answer: &str, expected_keywords: &[&str]) -> bool {
    let answer_lower = answer.to_lowercase();
    expected_keywords
        .iter()
        .any(|kw| answer_lower.contains(&kw.to_lowercase()))
}

/// Extract numeric values from text for verification
pub fn extract_numbers(text: &str) -> Vec<f64> {
    text.split_whitespace()
        .filter_map(|word| {
            // Try to parse word as number, removing common separators
            let cleaned = word
                .trim_matches(|c: char| !c.is_numeric() && c != '.' && c != '-')
                .replace(',', "");
            cleaned.parse::<f64>().ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_answer_quality() {
        assert!(verify_answer_quality(
            "The capital is Paris",
            &["paris", "france"]
        ));
        assert!(verify_answer_quality("PARIS is the answer", &["paris"]));
        assert!(!verify_answer_quality(
            "The capital is London",
            &["paris", "france"]
        ));
    }

    #[test]
    fn test_extract_numbers() {
        let nums = extract_numbers("The answer is 42 and also 3.25");
        assert_eq!(nums, vec![42.0, 3.25]);

        let nums = extract_numbers("Cost: $1,234.56 and quantity: 100");
        assert_eq!(nums, vec![1234.56, 100.0]);

        let nums = extract_numbers("No numbers here!");
        assert!(nums.is_empty());
    }
}
