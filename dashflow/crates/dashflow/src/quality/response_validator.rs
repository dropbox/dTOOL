// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Response validation to detect when LLM ignores tool results.
//!
//! # Problem
//!
//! LLMs sometimes say "couldn't find documentation" or "no information available"
//! even when tool results contain relevant information. This happens because:
//! 1. Tool results may not be formatted prominently enough
//! 2. LLM may default to internal knowledge instead of using retrieved data
//! 3. LLM may not recognize tool results as authoritative
//!
//! # Solution (INNOVATION 10)
//!
//! Explicit validation node in the graph that detects these patterns and
//! triggers automatic retry with stronger prompts.

use std::collections::HashSet;

/// Result of response validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationResult {
    /// Response is valid and uses tool results appropriately.
    Valid,

    /// LLM ignored tool results - said "couldn't find" despite having data.
    ToolResultsIgnored {
        /// The specific phrase that triggered this (e.g., "couldn't find")
        phrase: String,
        /// Suggested action to fix this
        action: ValidationAction,
    },

    /// Response lacks citations to tool results.
    MissingCitations {
        /// Suggested action to fix this
        action: ValidationAction,
    },
}

/// Actions to take when validation fails.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationAction {
    /// Retry with stronger system prompt emphasizing tool usage
    RetryWithStrongerPrompt,

    /// Re-inject tool results more prominently
    ReinjectToolResults,

    /// Upgrade to stronger model (e.g., gpt-4o-mini → gpt-4)
    UpgradeModel,

    /// Accept the response (too many retries already)
    Accept,
}

/// Validates responses to detect when LLM ignores tool results.
///
/// This is a critical component of the quality architecture. Instead of hoping
/// the LLM uses tool results, we explicitly check and enforce it.
#[derive(Debug, Clone)]
pub struct ResponseValidator {
    /// Phrases that indicate LLM didn't find information
    ignorance_phrases: HashSet<String>,

    /// Phrases that indicate LLM is using tool results
    citation_phrases: HashSet<String>,

    /// Require citations when tool results are provided?
    require_citations: bool,
}

impl Default for ResponseValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseValidator {
    fn normalize_phrase(phrase: impl Into<String>) -> Option<String> {
        let phrase = phrase.into();
        let phrase = phrase.trim();
        if phrase.is_empty() {
            return None;
        }
        Some(phrase.to_lowercase())
    }

    fn insert_normalized_phrase(set: &mut HashSet<String>, phrase: impl Into<String>) {
        if let Some(phrase) = Self::normalize_phrase(phrase) {
            set.insert(phrase);
        }
    }

    fn tool_results_have_content(results: &str) -> bool {
        let trimmed = results.trim();
        if trimmed.is_empty() {
            return false;
        }

        // Treat explicit "no relevant ..." tool output as "no content", regardless of casing.
        !trimmed.to_lowercase().contains("no relevant")
    }

    fn contains_phrase_with_boundaries(haystack: &str, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }

        let mut search_start = 0;
        while let Some(relative_pos) = haystack[search_start..].find(needle) {
            let match_start = search_start + relative_pos;
            let match_end = match_start + needle.len();

            let before_ok = match_start == 0
                || haystack[..match_start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| !c.is_alphanumeric());
            let after_ok = match_end == haystack.len()
                || haystack[match_end..]
                    .chars()
                    .next()
                    .is_some_and(|c| !c.is_alphanumeric());

            if before_ok && after_ok {
                return true;
            }

            search_start = match_end;
        }

        false
    }

    /// Creates a new response validator with default patterns.
    #[must_use]
    pub fn new() -> Self {
        let mut ignorance_phrases = HashSet::new();

        // Phrases indicating LLM didn't find information
        Self::insert_normalized_phrase(&mut ignorance_phrases, "couldn't find");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "wasn't able to find");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "couldn't locate");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "unable to find");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "no documentation found");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "no information available");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "don't have information");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "don't have access to");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "cannot find");
        Self::insert_normalized_phrase(&mut ignorance_phrases, "can't find");

        let mut citation_phrases = HashSet::new();

        // Phrases indicating LLM is citing/using tool results
        Self::insert_normalized_phrase(&mut citation_phrases, "based on");
        Self::insert_normalized_phrase(&mut citation_phrases, "according to");
        Self::insert_normalized_phrase(&mut citation_phrases, "the search results");
        Self::insert_normalized_phrase(&mut citation_phrases, "the documentation");
        Self::insert_normalized_phrase(&mut citation_phrases, "from the search");
        Self::insert_normalized_phrase(&mut citation_phrases, "retrieved information");
        Self::insert_normalized_phrase(&mut citation_phrases, "found that");

        Self {
            ignorance_phrases,
            citation_phrases,
            require_citations: true,
        }
    }

    /// Creates a validator that doesn't require citations.
    #[must_use]
    pub fn without_citation_requirement() -> Self {
        let mut validator = Self::new();
        validator.require_citations = false;
        validator
    }

    /// Adds a custom ignorance phrase to detect.
    #[must_use]
    pub fn with_ignorance_phrase(mut self, phrase: impl Into<String>) -> Self {
        Self::insert_normalized_phrase(&mut self.ignorance_phrases, phrase);
        self
    }

    /// Adds a custom citation phrase.
    #[must_use]
    pub fn with_citation_phrase(mut self, phrase: impl Into<String>) -> Self {
        Self::insert_normalized_phrase(&mut self.citation_phrases, phrase);
        self
    }

    /// Validates a response to check if it properly uses tool results.
    ///
    /// # Arguments
    ///
    /// * `response` - The LLM's response text
    /// * `tool_was_called` - Whether tools were called to generate this response
    /// * `tool_results` - Optional tool result content for additional validation
    ///
    /// # Returns
    ///
    /// * `ValidationResult::Valid` - Response is good
    /// * `ValidationResult::ToolResultsIgnored` - LLM said "couldn't find" but tools returned data
    /// * `ValidationResult::MissingCitations` - Response doesn't cite tool results
    #[must_use]
    pub fn validate(
        &self,
        response: &str,
        tool_was_called: bool,
        tool_results: Option<&str>,
    ) -> ValidationResult {
        // If no tools were called, no validation needed
        if !tool_was_called {
            return ValidationResult::Valid;
        }

        let response_lower = response.to_lowercase();

        // Check 1: Did LLM say "couldn't find" despite having tool results?
        if let Some(results) = tool_results {
            // Only check if tool actually returned content
            if Self::tool_results_have_content(results) {
                for phrase in &self.ignorance_phrases {
                    if Self::contains_phrase_with_boundaries(&response_lower, phrase) {
                        return ValidationResult::ToolResultsIgnored {
                            phrase: phrase.clone(),
                            action: ValidationAction::ReinjectToolResults,
                        };
                    }
                }
            }
        }

        // Check 2: Does response cite tool results?
        if self.require_citations {
            if let Some(results) = tool_results {
                // Only require citations if tool actually returned content
                if Self::tool_results_have_content(results) {
                    let has_citation = self
                        .citation_phrases
                        .iter()
                        .any(|phrase| Self::contains_phrase_with_boundaries(&response_lower, phrase));

                    if !has_citation {
                        return ValidationResult::MissingCitations {
                            action: ValidationAction::RetryWithStrongerPrompt,
                        };
                    }
                }
            }
        }

        ValidationResult::Valid
    }

    /// Quick check: Does this response ignore tool results?
    ///
    /// This is a simplified version for use in conditionals and quick checks.
    #[must_use]
    pub fn ignores_tool_results(&self, response: &str, tool_results: &str) -> bool {
        matches!(
            self.validate(response, true, Some(tool_results)),
            ValidationResult::ToolResultsIgnored { .. }
        )
    }

    /// Checks if response has proper citations.
    #[must_use]
    pub fn has_citations(&self, response: &str) -> bool {
        let response_lower = response.to_lowercase();
        self.citation_phrases
            .iter()
            .any(|phrase| Self::contains_phrase_with_boundaries(&response_lower, phrase))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_response_with_tool_results() {
        let validator = ResponseValidator::new();
        let response = "Based on the search results, Tokio is an async runtime for Rust.";
        let tool_results = "Tokio is an async runtime...";

        let result = validator.validate(response, true, Some(tool_results));
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_detects_tool_ignorance() {
        let validator = ResponseValidator::new();
        let response = "I couldn't find any documentation about Tokio.";
        let tool_results = "Tokio is an async runtime for Rust that provides...";

        let result = validator.validate(response, true, Some(tool_results));
        match result {
            ValidationResult::ToolResultsIgnored { phrase, .. } => {
                assert_eq!(phrase, "couldn't find");
            }
            _ => panic!("Expected ToolResultsIgnored, got {:?}", result),
        }
    }

    #[test]
    fn test_detects_various_ignorance_phrases() {
        let validator = ResponseValidator::new();
        let tool_results = "Documentation found: Tokio is...";

        let phrases = vec![
            "I couldn't find any information",
            "I wasn't able to find documentation",
            "I couldn't locate any details",
            "No documentation found for this",
            "No information available about this",
        ];

        for phrase in phrases {
            let result = validator.validate(phrase, true, Some(tool_results));
            assert!(
                matches!(result, ValidationResult::ToolResultsIgnored { .. }),
                "Failed to detect: {}",
                phrase
            );
        }
    }

    #[test]
    fn test_accepts_response_without_tool_calls() {
        let validator = ResponseValidator::new();
        let response = "I couldn't find information about that.";

        // No tool was called, so this is valid (LLM correctly says it doesn't know)
        let result = validator.validate(response, false, None);
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_detects_missing_citations() {
        let validator = ResponseValidator::new();
        let response = "Tokio is an async runtime for Rust."; // No citation!
        let tool_results = "Tokio is...";

        let result = validator.validate(response, true, Some(tool_results));
        match result {
            ValidationResult::MissingCitations { .. } => {
                // Expected
            }
            _ => panic!("Expected MissingCitations, got {:?}", result),
        }
    }

    #[test]
    fn test_citation_phrases_detected() {
        let validator = ResponseValidator::new();
        let tool_results = "Tokio docs...";

        let citations = vec![
            "Based on the search results, Tokio is...",
            "According to the documentation, Tokio provides...",
            "The search results show that Tokio...",
            "From the search, we can see that Tokio...",
            "The retrieved information indicates Tokio...",
        ];

        for response in citations {
            let result = validator.validate(response, true, Some(tool_results));
            assert_eq!(
                result,
                ValidationResult::Valid,
                "Failed to accept citation: {}",
                response
            );
        }
    }

    #[test]
    fn test_ignores_tool_results_quick_check() {
        let validator = ResponseValidator::new();

        assert!(validator.ignores_tool_results(
            "I couldn't find documentation",
            "Tokio is an async runtime..."
        ));

        assert!(!validator.ignores_tool_results(
            "Based on the search, Tokio is an async runtime",
            "Tokio is an async runtime..."
        ));
    }

    #[test]
    fn test_has_citations_check() {
        let validator = ResponseValidator::new();

        assert!(validator.has_citations("Based on the documentation, Tokio is..."));
        assert!(validator.has_citations("According to the search results, we can see..."));
        assert!(!validator.has_citations("Tokio is an async runtime."));
    }

    #[test]
    fn test_custom_phrases() {
        let validator = ResponseValidator::new()
            .with_ignorance_phrase("i'm not sure")
            .with_citation_phrase("the codebase shows");

        let result = validator.validate("I'm not sure about that", true, Some("Data found"));
        match result {
            ValidationResult::ToolResultsIgnored { phrase, .. } => {
                assert_eq!(phrase, "i'm not sure");
            }
            _ => panic!("Expected ToolResultsIgnored"),
        }

        let result = validator.validate(
            "The codebase shows that Tokio...",
            true,
            Some("Code: Tokio..."),
        );
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_custom_phrases_are_normalized() {
        let validator = ResponseValidator::new()
            .with_ignorance_phrase("  I'M NOT SURE  ")
            .with_citation_phrase("  THE CODEBASE SHOWS  ");

        let result = validator.validate("I'm not sure about that", true, Some("Data found"));
        assert!(matches!(
            result,
            ValidationResult::ToolResultsIgnored { .. }
        ));

        let result = validator.validate(
            "The codebase shows that Tokio...",
            true,
            Some("Code: Tokio..."),
        );
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_empty_custom_phrases_are_ignored() {
        let validator = ResponseValidator::new().with_ignorance_phrase("   ");
        let result = validator.validate("Hello world", true, Some("Data found"));
        assert!(matches!(result, ValidationResult::MissingCitations { .. }));

        let validator = ResponseValidator::new().with_ignorance_phrase("");
        let result = validator.validate("Hello world", true, Some("Data found"));
        assert!(matches!(result, ValidationResult::MissingCitations { .. }));
    }

    #[test]
    fn test_without_citation_requirement() {
        let validator = ResponseValidator::without_citation_requirement();

        // This would normally fail for missing citation, but we disabled that
        let result = validator.validate(
            "Tokio is an async runtime.", // No citation
            true,
            Some("Tokio docs..."),
        );
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_empty_tool_results_ignored() {
        let validator = ResponseValidator::new();

        // Empty tool results shouldn't trigger validation
        let result = validator.validate("I couldn't find information", true, Some(""));
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_no_relevant_results_accepted() {
        let validator = ResponseValidator::new();

        // Tool explicitly said "No relevant" - LLM is right to say couldn't find
        let result = validator.validate(
            "I couldn't find information",
            true,
            Some("No relevant documentation found"),
        );
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_no_relevant_results_accepted_case_insensitive() {
        let validator = ResponseValidator::new();

        let result = validator.validate(
            "I couldn't find information",
            true,
            Some("NO RELEVANT documentation found"),
        );
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_ignorance_phrase_substring_false_positive_avoided() {
        let validator = ResponseValidator::new();

        let result = validator.validate(
            "I can't finders keep searching.",
            true,
            Some("Tokio is an async runtime..."),
        );

        assert!(matches!(result, ValidationResult::MissingCitations { .. }));
    }

    #[test]
    fn test_citation_phrase_substring_false_positive_avoided() {
        let validator = ResponseValidator::new();

        let result = validator.validate(
            "From the searchable interface, Tokio is an async runtime.",
            true,
            Some("Tokio is an async runtime..."),
        );

        assert!(matches!(result, ValidationResult::MissingCitations { .. }));
    }
}
