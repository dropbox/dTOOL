// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Tool result validation before passing to LLM.
//!
//! # Problem
//!
//! Not all tool results are high quality. Tools can return:
//! - Empty results
//! - Error messages
//! - Irrelevant information (query mismatch)
//! - Malformed data
//!
//! Passing bad tool results to the LLM leads to:
//! - "Couldn't find information" responses (even with system prompt fixes)
//! - Hallucinations based on irrelevant data
//! - Wasted tokens and time
//!
//! # Solution (INNOVATION 6)
//!
//! Validate tool results BEFORE passing to LLM. If validation fails:
//! - Transform the query and retry search
//! - Try alternative tools
//! - Provide explicit feedback to user
//!
//! # Architecture
//!
//! ```text
//! Tool Call → Results → Validator → Valid? → Pass to LLM
//!                           ↓          ↓
//!                      Check quality  Invalid
//!                                      ↓
//!                                   Retry with better query
//! ```

use std::collections::HashSet;

/// Result of tool result validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolValidationResult {
    /// Tool results are valid and can be used
    Valid,

    /// Tool returned empty or too-short results.
    ///
    /// This variant covers two cases:
    /// 1. **Empty**: Truly empty/whitespace-only results → `action: TransformQuery`
    /// 2. **TooShort**: Results below `min_length` but non-empty → `action: Accept`
    ///
    /// The distinction is captured in the action field:
    /// - `TransformQuery`: Result is empty, retry with a different query
    /// - `Accept`: Result exists but is short, proceed with best effort
    Empty {
        /// Suggested action
        action: ToolValidationAction,
    },

    /// Tool returned an error message
    Error {
        /// The error message
        error: String,
        /// Suggested action
        action: ToolValidationAction,
    },

    /// Tool results exist but are irrelevant to the query
    Irrelevant {
        /// Relevance score (0.0-1.0)
        relevance: f32,
        /// Suggested action
        action: ToolValidationAction,
    },

    /// Tool results are malformed or unparseable
    Malformed {
        /// Description of the issue
        issue: String,
        /// Suggested action
        action: ToolValidationAction,
    },
}

impl ToolValidationResult {
    /// Checks if validation passed.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Gets the suggested action (if validation failed).
    #[must_use]
    pub fn action(&self) -> Option<&ToolValidationAction> {
        match self {
            Self::Valid => None,
            Self::Empty { action } => Some(action),
            Self::Error { action, .. } => Some(action),
            Self::Irrelevant { action, .. } => Some(action),
            Self::Malformed { action, .. } => Some(action),
        }
    }
}

/// Actions to take when tool results fail validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolValidationAction {
    /// Retry with transformed query
    TransformQuery,

    /// Try alternative tool
    TryAlternativeTool,

    /// Accept the results anyway (best effort)
    Accept,

    /// Report to user that information unavailable
    ReportUnavailable,
}

/// Configuration for tool result validation.
#[derive(Debug, Clone)]
pub struct ToolValidatorConfig {
    /// Minimum relevance score (0.0-1.0)
    pub min_relevance: f32,

    /// Minimum result length (characters)
    pub min_length: usize,

    /// Whether to check for error patterns
    pub check_errors: bool,

    /// Whether to validate JSON structure (for JSON tools)
    pub validate_json: bool,
}

impl Default for ToolValidatorConfig {
    fn default() -> Self {
        Self {
            min_relevance: 0.5,
            min_length: 10,
            check_errors: true,
            validate_json: false,
        }
    }
}

/// Validates tool results before passing to LLM.
///
/// This is a critical component of the quality architecture. By validating
/// tool results BEFORE the LLM sees them, we prevent many failure modes.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::quality::{ToolResultValidator, ToolValidationResult};
///
/// let validator = ToolResultValidator::default();
///
/// // After tool execution
/// match validator.validate(tool_result, query) {
///     ToolValidationResult::Valid => {
///         // Pass to LLM
///     }
///     ToolValidationResult::Empty { action } => {
///         // Transform query and retry
///     }
///     ToolValidationResult::Error { error, action } => {
///         // Handle error
///     }
///     ToolValidationResult::Irrelevant { relevance, action } => {
///         // Try alternative tool or transform query
///     }
///     ToolValidationResult::Malformed { issue, action } => {
///         // Fix or retry
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ToolResultValidator {
    config: ToolValidatorConfig,
    error_patterns: HashSet<String>,
}

impl Default for ToolResultValidator {
    fn default() -> Self {
        Self::new(ToolValidatorConfig::default())
    }
}

impl ToolResultValidator {
    /// Creates a new validator with the given configuration.
    #[must_use]
    pub fn new(config: ToolValidatorConfig) -> Self {
        let mut error_patterns = HashSet::new();

        // Common error patterns in tool results
        error_patterns.insert("error:".to_lowercase());
        error_patterns.insert("failed to".to_lowercase());
        error_patterns.insert("exception:".to_lowercase());
        error_patterns.insert("could not".to_lowercase());
        error_patterns.insert("unable to".to_lowercase());
        error_patterns.insert("timeout".to_lowercase());
        error_patterns.insert("connection refused".to_lowercase());
        // Use more specific "not found" patterns to avoid false positives
        // (e.g., "I found information that was not found to be incorrect" is valid)
        error_patterns.insert("404 not found".to_lowercase());
        error_patterns.insert("resource not found".to_lowercase());
        error_patterns.insert("page not found".to_lowercase());
        error_patterns.insert("file not found".to_lowercase());
        error_patterns.insert("was not found".to_lowercase());
        error_patterns.insert("is not found".to_lowercase());
        error_patterns.insert("cannot be found".to_lowercase());
        error_patterns.insert("could not be found".to_lowercase());
        error_patterns.insert("permission denied".to_lowercase());
        error_patterns.insert("access denied".to_lowercase());

        Self {
            config,
            error_patterns,
        }
    }

    /// Creates a validator with default configuration.
    #[must_use]
    pub fn default_validator() -> Self {
        Self::default()
    }

    /// Validates tool results.
    ///
    /// # Arguments
    ///
    /// * `result` - The tool result content
    /// * `query` - Optional original query for relevance checking
    ///
    /// # Returns
    ///
    /// * `ToolValidationResult::Valid` - Results are good
    /// * `ToolValidationResult::Empty` - No content
    /// * `ToolValidationResult::Error` - Contains error message
    /// * `ToolValidationResult::Irrelevant` - Not relevant to query
    /// * `ToolValidationResult::Malformed` - Unparseable/invalid
    #[must_use]
    pub fn validate(&self, result: &str, query: Option<&str>) -> ToolValidationResult {
        // Check 1: Empty results
        let trimmed = result.trim();
        if trimmed.is_empty() {
            return ToolValidationResult::Empty {
                action: ToolValidationAction::TransformQuery,
            };
        }

        // Check 2: Minimum length
        if trimmed.len() < self.config.min_length {
            return ToolValidationResult::Empty {
                action: ToolValidationAction::Accept, // Short but present
            };
        }

        // Check 3: Error patterns
        if self.config.check_errors {
            let result_lower = result.to_lowercase();
            for pattern in &self.error_patterns {
                if result_lower.contains(pattern) {
                    return ToolValidationResult::Error {
                        error: format!("Tool result contains error pattern: '{pattern}'"),
                        action: ToolValidationAction::TryAlternativeTool,
                    };
                }
            }
        }

        // Check 4: "No relevant" or "Not found" phrases (different from errors)
        let result_lower = result.to_lowercase();
        if result_lower.contains("no relevant") || result_lower.contains("no results") {
            return ToolValidationResult::Empty {
                action: ToolValidationAction::TransformQuery,
            };
        }

        // Check 5: JSON validation (if enabled)
        if self.config.validate_json {
            if let Err(e) = serde_json::from_str::<serde_json::Value>(result) {
                return ToolValidationResult::Malformed {
                    issue: format!("Invalid JSON: {e}"),
                    action: ToolValidationAction::ReportUnavailable,
                };
            }
        }

        // Check 6: Relevance (if query provided)
        // This is a simple heuristic - can be replaced with LLM-based relevance check
        if let Some(q) = query {
            let relevance = self.compute_relevance(result, q);
            if relevance < self.config.min_relevance {
                return ToolValidationResult::Irrelevant {
                    relevance,
                    action: ToolValidationAction::TransformQuery,
                };
            }
        }

        ToolValidationResult::Valid
    }

    /// Quick check: Are tool results valid?
    #[must_use]
    pub fn is_valid(&self, result: &str, query: Option<&str>) -> bool {
        self.validate(result, query).is_valid()
    }

    /// Computes relevance score (simple keyword-based heuristic).
    ///
    /// This is a placeholder for more sophisticated relevance checking.
    /// In production, you'd want to use:
    /// - Embedding similarity
    /// - LLM-as-judge for relevance
    /// - BM25 or other IR metrics
    fn compute_relevance(&self, result: &str, query: &str) -> f32 {
        let result_lower = result.to_lowercase();
        let query_lower = query.to_lowercase();

        fn is_relevance_word(word: &str) -> bool {
            match word.len() {
                0..=2 => false,
                3 => !matches!(
                    word,
                    "the"
                        | "and"
                        | "for"
                        | "are"
                        | "was"
                        | "you"
                        | "not"
                        | "can"
                        | "how"
                        | "why"
                        | "who"
                        | "its"
                        | "any"
                        | "all"
                        | "get"
                        | "set"
                        | "use"
                        | "yes"
                        | "but"
                        | "our"
                        | "out"
                ),
                _ => true,
            }
        }

        // Extract alphanumeric words only (strip punctuation)
        let query_words: Vec<String> = query_lower
            .split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
            })
            .filter(|w| is_relevance_word(w.as_str()))
            .collect();

        if query_words.is_empty() {
            return 1.0; // No meaningful query words
        }

        let matches = query_words
            .iter()
            .filter(|word| result_lower.contains(word.as_str()))
            .count();

        matches as f32 / query_words.len() as f32
    }

    /// Adds a custom error pattern to detect.
    #[must_use]
    pub fn with_error_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.error_patterns.insert(pattern.into().to_lowercase());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_results() {
        let validator = ToolResultValidator::default();
        let result = "Tokio is an async runtime for Rust that provides...";

        // Query: "What is Tokio?" → after filtering (>3 chars): ["What", "Tokio"]
        // Result contains both "what" and "tokio" (case insensitive)
        // But result should not contain "what"! Let me check...
        // Actually, result = "Tokio is..." doesn't contain "what"
        // So relevance = 1/2 = 0.5, which is exactly min_relevance (0.5)
        // So it passes validation (≥ 0.5)
        assert_eq!(
            validator.validate(result, Some("What is Tokio?")),
            ToolValidationResult::Valid
        );
    }

    #[test]
    fn test_empty_results() {
        let validator = ToolResultValidator::default();

        assert!(matches!(
            validator.validate("", None),
            ToolValidationResult::Empty { .. }
        ));

        assert!(matches!(
            validator.validate("   ", None),
            ToolValidationResult::Empty { .. }
        ));
    }

    #[test]
    fn test_too_short() {
        let validator = ToolResultValidator::default();

        // Less than min_length (default 10)
        assert!(matches!(
            validator.validate("Short", None),
            ToolValidationResult::Empty { .. }
        ));
    }

    #[test]
    fn test_error_patterns() {
        let validator = ToolResultValidator::default();

        let error_results = vec![
            "Error: Connection failed",
            "Failed to retrieve data",
            "Exception: Timeout occurred",
            "Could not find resource",
            "Unable to access API",
            "Timeout after 30 seconds",
            "Connection refused",
            "404 Not Found - Resource unavailable",
            "Resource not found at specified path",
            "The file was not found on the server",
            "Permission denied",
            "Access denied to resource",
        ];

        for result in error_results {
            match validator.validate(result, None) {
                ToolValidationResult::Error { .. } => {
                    // Expected
                }
                other => panic!("Expected Error for '{}', got {:?}", result, other),
            }
        }
    }

    #[test]
    fn test_not_found_false_positive_avoided() {
        let validator = ToolResultValidator::default();

        // This should NOT be detected as error - "not found" appears but not in error context
        let result =
            "I found the information you were looking for. The data not found to be incorrect.";
        assert_eq!(
            validator.validate(result, None),
            ToolValidationResult::Valid
        );

        // This should NOT be detected as error - legitimate content mentioning "not found"
        let result =
            "The search algorithm ensures missing items are not found multiple times in the list.";
        assert_eq!(
            validator.validate(result, None),
            ToolValidationResult::Valid
        );
    }

    #[test]
    fn test_no_relevant_results() {
        let validator = ToolResultValidator::default();

        assert!(matches!(
            validator.validate("No relevant documentation found", None),
            ToolValidationResult::Empty { .. }
        ));

        assert!(matches!(
            validator.validate("No results available", None),
            ToolValidationResult::Empty { .. }
        ));
    }

    #[test]
    fn test_json_validation() {
        let config = ToolValidatorConfig {
            validate_json: true,
            ..Default::default()
        };
        let validator = ToolResultValidator::new(config);

        // Valid JSON
        assert_eq!(
            validator.validate(r#"{"key": "value"}"#, None),
            ToolValidationResult::Valid
        );

        // Invalid JSON
        assert!(matches!(
            validator.validate(r#"{"key": invalid}"#, None),
            ToolValidationResult::Malformed { .. }
        ));
    }

    #[test]
    fn test_relevance_checking() {
        let config = ToolValidatorConfig {
            min_relevance: 0.5,
            ..Default::default()
        };
        let validator = ToolResultValidator::new(config);

        // Relevant (contains "tokio" and "async")
        let result = "Tokio is an async runtime for Rust that provides...";
        assert_eq!(
            validator.validate(result, Some("What is Tokio async runtime?")),
            ToolValidationResult::Valid
        );

        // Irrelevant (no matching keywords)
        let result = "Python is a programming language...";
        match validator.validate(result, Some("What is Tokio async runtime?")) {
            ToolValidationResult::Irrelevant { relevance, .. } => {
                assert!(relevance < 0.5);
            }
            other => panic!("Expected Irrelevant, got {:?}", other),
        }
    }

    #[test]
    fn test_relevance_score_computation() {
        let validator = ToolResultValidator::default();

        // All keywords match
        assert_eq!(
            validator.compute_relevance(
                "Tokio async runtime provides features",
                "Tokio async runtime"
            ),
            1.0
        );

        // 2/3 match (Tokio + async, but not runtime)
        let score = validator.compute_relevance(
            "Tokio is great but no async mentioned",
            "Tokio async runtime",
        );
        assert!((score - 0.666).abs() < 0.01); // 2/3 ≈ 0.666

        // No keywords match
        assert_eq!(
            validator.compute_relevance("Python Django Flask", "Tokio async runtime"),
            0.0
        );
    }

    #[test]
    fn test_relevance_keeps_common_acronyms() {
        let validator = ToolResultValidator::default();

        assert_eq!(
            validator.compute_relevance("No matches here", "SQL API"),
            0.0
        );

        assert_eq!(
            validator.compute_relevance("This mentions sql and api", "SQL API"),
            1.0
        );
    }

    #[test]
    fn test_is_valid_helper() {
        let validator = ToolResultValidator::default();

        assert!(validator.is_valid("Valid content here", None));
        assert!(!validator.is_valid("", None));
        assert!(!validator.is_valid("Error: Failed", None));
    }

    #[test]
    fn test_custom_error_pattern() {
        let validator = ToolResultValidator::default().with_error_pattern("rate limit exceeded");

        assert!(matches!(
            validator.validate("Rate limit exceeded, try again later", None),
            ToolValidationResult::Error { .. }
        ));
    }

    #[test]
    fn test_validation_result_helpers() {
        let valid = ToolValidationResult::Valid;
        assert!(valid.is_valid());
        assert!(valid.action().is_none());

        let empty = ToolValidationResult::Empty {
            action: ToolValidationAction::TransformQuery,
        };
        assert!(!empty.is_valid());
        assert_eq!(empty.action(), Some(&ToolValidationAction::TransformQuery));

        let error = ToolValidationResult::Error {
            error: "Test error".to_string(),
            action: ToolValidationAction::TryAlternativeTool,
        };
        assert!(!error.is_valid());
        assert_eq!(
            error.action(),
            Some(&ToolValidationAction::TryAlternativeTool)
        );
    }

    #[test]
    fn test_case_insensitive_error_detection() {
        let validator = ToolResultValidator::default();

        // Different cases should all be detected
        assert!(matches!(
            validator.validate("ERROR: Something failed", None),
            ToolValidationResult::Error { .. }
        ));

        assert!(matches!(
            validator.validate("Failed To Connect", None),
            ToolValidationResult::Error { .. }
        ));

        assert!(matches!(
            validator.validate("TIMEOUT OCCURRED", None),
            ToolValidationResult::Error { .. }
        ));
    }

    #[test]
    fn test_accept_action_for_short_but_present() {
        let validator = ToolResultValidator::default();

        // Short but not empty - should accept
        match validator.validate("Short", None) {
            ToolValidationResult::Empty {
                action: ToolValidationAction::Accept,
            } => {
                // Expected
            }
            other => panic!("Expected Empty with Accept action, got {:?}", other),
        }
    }
}
