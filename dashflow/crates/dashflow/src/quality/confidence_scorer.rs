// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for confidence scorer
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! Confidence scoring for LLM responses.
//!
//! # Problem
//!
//! LLMs sometimes generate responses without having enough information,
//! leading to hallucinations or incorrect answers. The problem is that
//! they don't know WHEN to search for more information.
//!
//! # Solution (INNOVATION 2)
//!
//! Ask the LLM to rate its own confidence (0.0-1.0) in its response, and
//! use that confidence score for routing:
//! - High confidence (≥0.7): Proceed with response
//! - Low confidence (<0.7): Force search before responding
//!
//! This creates a self-aware agent that knows when it needs more information.

use regex::Regex;
use std::sync::OnceLock;

/// Result of confidence extraction from LLM response.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceScore {
    /// Confidence level (0.0 = no confidence, 1.0 = complete confidence)
    pub confidence: f32,

    /// Whether LLM thinks it should have searched first
    pub should_have_searched: bool,

    /// Optional explanation from LLM about its confidence
    pub explanation: Option<String>,
}

impl ConfidenceScore {
    /// Creates a confidence score with default values.
    #[must_use]
    pub fn new(confidence: f32) -> Self {
        Self {
            confidence: confidence.clamp(0.0, 1.0),
            should_have_searched: false,
            explanation: None,
        }
    }

    /// Creates a confidence score with search suggestion.
    #[must_use]
    pub fn with_search_suggestion(confidence: f32, should_search: bool) -> Self {
        Self {
            confidence: confidence.clamp(0.0, 1.0),
            should_have_searched: should_search,
            explanation: None,
        }
    }

    /// Adds an explanation to the confidence score.
    #[must_use]
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    /// Returns true if this represents low confidence (should search).
    #[must_use]
    pub fn is_low_confidence(&self, threshold: f32) -> bool {
        self.confidence < threshold
    }

    /// Returns true if LLM suggests searching.
    #[must_use]
    pub fn suggests_search(&self) -> bool {
        self.should_have_searched
    }

    /// Returns true if we should force a search based on this score.
    #[must_use]
    pub fn should_force_search(&self, threshold: f32) -> bool {
        self.is_low_confidence(threshold) || self.should_have_searched
    }
}

/// Extracts confidence scores from LLM responses.
///
/// The LLM is prompted to include confidence metadata in its response:
///
/// ```text
/// CONFIDENCE: 0.85 | SHOULD_SEARCH: false | REASON: I have internal knowledge about this
/// ```
///
/// Or simpler format:
/// ```text
/// CONFIDENCE: 0.85
/// ```
///
/// ## Mid-Response Metadata
///
/// Metadata can appear anywhere in the response, but it's typically placed at the end.
/// When metadata appears mid-response, `strip_metadata()` removes only the metadata lines,
/// preserving surrounding content. For cleaner output, prompt the LLM to place metadata
/// at the very end of its response.
///
/// ## Case Sensitivity
///
/// All metadata keywords (CONFIDENCE, SHOULD_SEARCH, REASON) are case-insensitive.
/// `confidence:`, `Confidence:`, and `CONFIDENCE:` are all accepted.
///
/// ## Multi-line REASON
///
/// The REASON field can span multiple lines. It continues until the next metadata
/// keyword (CONFIDENCE, SHOULD_SEARCH) or end of text. Example:
/// ```text
/// REASON: This is a detailed explanation
/// that spans multiple lines
/// and includes more context.
/// ```
#[derive(Debug, Clone)]
pub struct ConfidenceScorer {
    /// Default confidence when not explicitly provided (conservative)
    default_confidence: f32,

    /// Threshold for low confidence (forces search)
    low_confidence_threshold: f32,
}

impl Default for ConfidenceScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfidenceScorer {
    /// Creates a new confidence scorer with default settings.
    ///
    /// Default confidence: 0.5 (neutral)
    /// Low confidence threshold: 0.7 (below this triggers search)
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_confidence: 0.5,
            low_confidence_threshold: 0.7,
        }
    }

    /// Sets the default confidence for responses without explicit scores.
    #[must_use]
    pub fn with_default_confidence(mut self, confidence: f32) -> Self {
        self.default_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Sets the threshold for low confidence.
    #[must_use]
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.low_confidence_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Gets the system prompt fragment to include for confidence scoring.
    ///
    /// Add this to your agent's system prompt to get confidence scores.
    #[must_use]
    pub fn system_prompt_fragment() -> &'static str {
        r"
After your response, rate your confidence in the accuracy and completeness of your answer.

Include this metadata at the end of your response:

CONFIDENCE: <0.0-1.0>
SHOULD_SEARCH: <true|false>
REASON: <brief explanation>

Guidelines:
- 0.0-0.4: Very uncertain, likely need to search
- 0.5-0.6: Somewhat uncertain, might benefit from search
- 0.7-0.8: Confident, but not certain
- 0.9-1.0: Very confident, have complete information

Set SHOULD_SEARCH to true if, in retrospect, you should have searched before answering.
"
    }

    /// Extracts confidence score from response text.
    ///
    /// Looks for patterns like:
    /// - `CONFIDENCE: 0.85`
    /// - `CONFIDENCE: 0.85 | SHOULD_SEARCH: true`
    /// - `CONFIDENCE: 0.85 | SHOULD_SEARCH: false | REASON: I have internal knowledge`
    ///
    /// Returns default confidence if no pattern found.
    #[must_use]
    pub fn extract(&self, response: &str) -> ConfidenceScore {
        // Try to extract confidence
        let confidence = self
            .extract_confidence_value(response)
            .unwrap_or(self.default_confidence);

        // Try to extract should_search flag
        let should_search = self.extract_should_search(response);

        // Try to extract reason
        let explanation = self.extract_reason(response);

        ConfidenceScore {
            confidence,
            should_have_searched: should_search,
            explanation,
        }
    }

    /// Checks if response indicates low confidence.
    #[must_use]
    pub fn is_low_confidence(&self, response: &str) -> bool {
        let score = self.extract(response);
        score.should_force_search(self.low_confidence_threshold)
    }

    /// Extracts confidence value from response.
    ///
    /// Case-insensitive matching: accepts CONFIDENCE, confidence, Confidence, etc.
    fn extract_confidence_value(&self, response: &str) -> Option<f32> {
        static CONFIDENCE_REGEX: OnceLock<Regex> = OnceLock::new();
        let re = CONFIDENCE_REGEX.get_or_init(|| {
            // Case-insensitive (?i), match optional minus sign for negative values
            Regex::new(r"(?i)CONFIDENCE:\s*(-?[0-9]*\.?[0-9]+)")
                .expect("static confidence regex pattern")
        });

        re.captures(response)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<f32>().ok())
            .map(|v| v.clamp(0.0, 1.0))
    }

    /// Extracts `should_search` flag from response.
    ///
    /// Case-insensitive matching: accepts SHOULD_SEARCH, should_search, etc.
    fn extract_should_search(&self, response: &str) -> bool {
        static SEARCH_REGEX: OnceLock<Regex> = OnceLock::new();
        let re = SEARCH_REGEX.get_or_init(|| {
            // Case-insensitive (?i) for both the keyword and the value
            Regex::new(r"(?i)SHOULD_SEARCH:\s*(true|false)").expect("static search regex pattern")
        });

        re.captures(response)
            .and_then(|caps| caps.get(1))
            .is_some_and(|m| m.as_str().eq_ignore_ascii_case("true"))
    }

    /// Extracts reason from response.
    ///
    /// Case-insensitive matching. Supports multi-line REASON content:
    /// the REASON field continues until the next metadata keyword
    /// (CONFIDENCE or SHOULD_SEARCH) or end of text.
    fn extract_reason(&self, response: &str) -> Option<String> {
        // Find the start of REASON: (case-insensitive)
        let response_lower = response.to_lowercase();
        let reason_start = response_lower.find("reason:")?;

        // Extract from after "REASON:" to end
        let after_reason = &response[reason_start + 7..]; // 7 = len("reason:")
        let content_start = after_reason.find(|c: char| !c.is_whitespace()).unwrap_or(0);
        let content = &after_reason[content_start..];

        // Find the end: next metadata keyword (case-insensitive) or end of string
        let content_lower = content.to_lowercase();
        let end_pos = [
            content_lower.find("confidence:"),
            content_lower.find("should_search:"),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(content.len());

        let reason = content[..end_pos].trim();
        if reason.is_empty() {
            None
        } else {
            Some(reason.to_string())
        }
    }

    /// Removes confidence metadata from response text.
    ///
    /// Use this to clean the response before showing to user.
    /// Handles multiline REASON content and case-insensitive keywords.
    ///
    /// Multiline REASON convention: REASON content ends at a blank line
    /// or when another metadata keyword is encountered.
    pub fn strip_metadata(response: &str) -> String {
        let mut result = String::new();
        let mut in_reason_block = false;

        for line in response.lines() {
            let trimmed_lower = line.trim().to_lowercase();

            // Check if this line starts a metadata block
            if trimmed_lower.starts_with("confidence:")
                || trimmed_lower.starts_with("should_search:")
            {
                in_reason_block = false;
                continue; // Skip this line
            }

            if trimmed_lower.starts_with("reason:") {
                in_reason_block = true;
                continue; // Skip this line
            }

            // If we're in a REASON block, check if this is a continuation
            if in_reason_block {
                // Blank line ends the REASON block
                if trimmed_lower.is_empty() {
                    in_reason_block = false;
                    // Don't add the blank line - it was part of ending REASON
                    continue;
                }
                // Non-blank line: still in REASON block, skip it
                continue;
            }

            // Normal line - add to result
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
        }

        result.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_basic_confidence() {
        let scorer = ConfidenceScorer::new();
        let response = "This is my answer.\n\nCONFIDENCE: 0.85";

        let score = scorer.extract(response);
        assert_eq!(score.confidence, 0.85);
        assert!(!score.should_have_searched);
        assert!(score.explanation.is_none());
    }

    #[test]
    fn test_extract_full_metadata() {
        let scorer = ConfidenceScorer::new();
        let response = r#"This is my answer.

CONFIDENCE: 0.65
SHOULD_SEARCH: true
REASON: I'm not completely certain about this topic
"#;

        let score = scorer.extract(response);
        assert_eq!(score.confidence, 0.65);
        assert!(score.should_have_searched);
        assert_eq!(
            score.explanation.as_deref(),
            Some("I'm not completely certain about this topic")
        );
    }

    #[test]
    fn test_extract_inline_metadata() {
        let scorer = ConfidenceScorer::new();
        let response = "Answer here. CONFIDENCE: 0.92 | SHOULD_SEARCH: false | REASON: I have complete knowledge";

        let score = scorer.extract(response);
        assert_eq!(score.confidence, 0.92);
        assert!(!score.should_have_searched);
        assert_eq!(
            score.explanation.as_deref(),
            Some("I have complete knowledge")
        );
    }

    #[test]
    fn test_default_confidence_when_missing() {
        let scorer = ConfidenceScorer::new();
        let response = "Just a regular response without metadata.";

        let score = scorer.extract(response);
        assert_eq!(score.confidence, 0.5); // Default
        assert!(!score.should_have_searched);
    }

    #[test]
    fn test_custom_default_confidence() {
        let scorer = ConfidenceScorer::new().with_default_confidence(0.3);
        let response = "No metadata here.";

        let score = scorer.extract(response);
        assert_eq!(score.confidence, 0.3);
    }

    #[test]
    fn test_is_low_confidence() {
        let scorer = ConfidenceScorer::new().with_threshold(0.7);

        // Below threshold
        assert!(scorer.is_low_confidence("CONFIDENCE: 0.65"));

        // Above threshold
        assert!(!scorer.is_low_confidence("CONFIDENCE: 0.8"));

        // Should search flag overrides
        assert!(scorer.is_low_confidence("CONFIDENCE: 0.9 | SHOULD_SEARCH: true"));
    }

    #[test]
    fn test_should_force_search() {
        let score1 = ConfidenceScore::new(0.6);
        assert!(score1.should_force_search(0.7));

        let score2 = ConfidenceScore::new(0.8);
        assert!(!score2.should_force_search(0.7));

        let score3 = ConfidenceScore::with_search_suggestion(0.9, true);
        assert!(score3.should_force_search(0.7)); // should_search overrides
    }

    #[test]
    fn test_strip_metadata() {
        let response = r#"This is the actual answer.

CONFIDENCE: 0.85
SHOULD_SEARCH: false
REASON: I know this

Some more text after."#;

        let cleaned = ConfidenceScorer::strip_metadata(response);
        assert!(cleaned.contains("This is the actual answer"));
        assert!(!cleaned.contains("CONFIDENCE"));
        assert!(!cleaned.contains("SHOULD_SEARCH"));
    }

    #[test]
    fn test_clamps_confidence_values() {
        let scorer = ConfidenceScorer::new();

        let score = scorer.extract("CONFIDENCE: 1.5"); // Over 1.0
        assert_eq!(score.confidence, 1.0);

        let score = scorer.extract("CONFIDENCE: -0.2"); // Under 0.0
        assert_eq!(score.confidence, 0.0);
    }

    #[test]
    fn test_confidence_score_methods() {
        let score = ConfidenceScore::new(0.6).with_explanation("Not sure about this");

        assert!(score.is_low_confidence(0.7));
        assert!(!score.is_low_confidence(0.5));
        assert!(!score.suggests_search());

        let score2 = ConfidenceScore::with_search_suggestion(0.9, true);
        assert!(score2.suggests_search());
        assert!(score2.should_force_search(0.7)); // Even though confidence is high
    }

    #[test]
    fn test_partial_metadata() {
        let scorer = ConfidenceScorer::new();

        // Only confidence
        let score = scorer.extract("CONFIDENCE: 0.75");
        assert_eq!(score.confidence, 0.75);
        assert!(!score.should_have_searched);

        // Confidence + should_search
        let score = scorer.extract("CONFIDENCE: 0.8 | SHOULD_SEARCH: true");
        assert_eq!(score.confidence, 0.8);
        assert!(score.should_have_searched);
        assert!(score.explanation.is_none());
    }

    #[test]
    fn test_various_formats() {
        let scorer = ConfidenceScorer::new();

        // Spaces
        assert_eq!(scorer.extract("CONFIDENCE: 0.5").confidence, 0.5);
        assert_eq!(scorer.extract("CONFIDENCE:0.5").confidence, 0.5);
        assert_eq!(scorer.extract("CONFIDENCE:  0.5").confidence, 0.5);

        // Decimals
        assert_eq!(scorer.extract("CONFIDENCE: 0.85").confidence, 0.85);
        assert_eq!(scorer.extract("CONFIDENCE: .85").confidence, 0.85);
        assert_eq!(scorer.extract("CONFIDENCE: 1").confidence, 1.0);
    }

    #[test]
    fn test_case_insensitive_keywords() {
        let scorer = ConfidenceScorer::new();

        // Lowercase
        assert_eq!(scorer.extract("confidence: 0.75").confidence, 0.75);

        // Mixed case
        assert_eq!(scorer.extract("Confidence: 0.80").confidence, 0.80);

        // SHOULD_SEARCH case insensitivity
        assert!(
            scorer
                .extract("confidence: 0.9 | should_search: True")
                .should_have_searched
        );
        assert!(
            scorer
                .extract("CONFIDENCE: 0.9 | Should_Search: TRUE")
                .should_have_searched
        );

        // REASON case insensitivity
        let score = scorer.extract("confidence: 0.6\nreason: lower case reason");
        assert_eq!(score.explanation.as_deref(), Some("lower case reason"));

        let score = scorer.extract("CONFIDENCE: 0.6\nReason: Mixed case reason");
        assert_eq!(score.explanation.as_deref(), Some("Mixed case reason"));
    }

    #[test]
    fn test_multiline_reason_extraction() {
        let scorer = ConfidenceScorer::new();

        // Single line reason
        let score = scorer.extract("CONFIDENCE: 0.7\nREASON: Single line");
        assert_eq!(score.explanation.as_deref(), Some("Single line"));

        // Multi-line reason
        let response = "CONFIDENCE: 0.65\nREASON: This is a detailed explanation\nthat spans multiple lines\nand includes more context.";
        let score = scorer.extract(response);
        assert!(score.explanation.is_some());
        let reason = score.explanation.unwrap();
        assert!(reason.contains("detailed explanation"));
        assert!(reason.contains("multiple lines"));
        assert!(reason.contains("more context"));

        // Multi-line reason followed by another keyword
        let response = "REASON: First line\nSecond line\nCONFIDENCE: 0.5";
        let score = scorer.extract(response);
        assert!(score.explanation.is_some());
        let reason = score.explanation.unwrap();
        assert!(reason.contains("First line"));
        assert!(reason.contains("Second line"));
        assert!(!reason.contains("CONFIDENCE"));
    }

    #[test]
    fn test_strip_metadata_multiline_reason() {
        // Multi-line REASON should be fully stripped
        let response = r#"This is the actual answer.

CONFIDENCE: 0.85
SHOULD_SEARCH: false
REASON: This is a detailed explanation
that spans multiple lines
and includes more context.

Some more text after."#;

        let cleaned = ConfidenceScorer::strip_metadata(response);
        assert!(cleaned.contains("This is the actual answer"));
        assert!(!cleaned.contains("CONFIDENCE"));
        assert!(!cleaned.contains("SHOULD_SEARCH"));
        assert!(!cleaned.contains("REASON"));
        assert!(!cleaned.contains("detailed explanation"));
        assert!(!cleaned.contains("multiple lines"));
        assert!(cleaned.contains("Some more text after"));
    }

    #[test]
    fn test_strip_metadata_case_insensitive() {
        let response =
            "Answer here.\n\nconfidence: 0.85\nshould_search: false\nreason: lowercase test";
        let cleaned = ConfidenceScorer::strip_metadata(response);
        assert!(cleaned.contains("Answer here"));
        assert!(!cleaned.to_lowercase().contains("confidence"));
        assert!(!cleaned.to_lowercase().contains("reason"));
    }
}
