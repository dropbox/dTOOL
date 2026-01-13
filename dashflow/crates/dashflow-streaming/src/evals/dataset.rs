// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Golden Dataset Support
//!
//! Enables evaluation against reference query/answer pairs (golden datasets).
//!
//! Golden datasets provide reproducible evaluation with known inputs and expected outputs.
//! This module supports:
//! - Loading eval suites from JSON files
//! - Running applications against test queries
//! - Scoring outputs with exact match and fuzzy match
//! - Computing correctness metrics for quality evaluation

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// A single test case in an eval suite
///
/// # Examples
///
/// ```
/// use dashflow_streaming::evals::EvalCase;
///
/// let case = EvalCase {
///     id: "doc_search_001".to_string(),
///     query: "What is the capital of France?".to_string(),
///     expected_answer: "Paris".to_string(),
///     metadata: None,
/// };
///
/// assert_eq!(case.query, "What is the capital of France?");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    /// Unique identifier for this test case
    pub id: String,

    /// Input query to the application
    pub query: String,

    /// Expected correct answer
    pub expected_answer: String,

    /// Optional metadata (context, difficulty, category, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A collection of eval cases for testing an application
///
/// # Examples
///
/// ```no_run
/// use dashflow_streaming::evals::EvalSuite;
///
/// // Load eval suite from JSON
/// let suite = EvalSuite::load("evals/librarian.json").unwrap();
/// println!("Loaded {} test cases", suite.cases.len());
///
/// // Iterate through cases
/// for case in &suite.cases {
///     println!("Case {}: {}", case.id, case.query);
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSuite {
    /// Suite name (e.g., "`librarian`")
    pub name: String,

    /// Suite description
    pub description: String,

    /// Version of the suite
    pub version: String,

    /// Test cases
    pub cases: Vec<EvalCase>,
}

impl EvalSuite {
    /// Load eval suite from JSON file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to eval suite JSON file
    ///
    /// # Returns
    ///
    /// Loaded eval suite or error
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be read or parsed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dashflow_streaming::evals::EvalSuite;
    ///
    /// let suite = EvalSuite::load("evals/librarian.json")
    ///     .expect("Failed to load eval suite");
    /// ```
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read eval suite: {}", path.display()))?;

        let suite: EvalSuite = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse eval suite JSON: {}", path.display()))?;

        Ok(suite)
    }

    /// Save eval suite to JSON file
    ///
    /// # Arguments
    ///
    /// * `path` - Path where to save the eval suite
    ///
    /// # Returns
    ///
    /// Ok(()) or error
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be written
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dashflow_streaming::evals::{EvalSuite, EvalCase};
    ///
    /// let suite = EvalSuite {
    ///     name: "librarian".to_string(),
    ///     description: "Librarian RAG test cases".to_string(),
    ///     version: "1.0.0".to_string(),
    ///     cases: vec![
    ///         EvalCase {
    ///             id: "test_001".to_string(),
    ///             query: "What is Rust?".to_string(),
    ///             expected_answer: "A systems programming language".to_string(),
    ///             metadata: None,
    ///         }
    ///     ],
    /// };
    ///
    /// suite.save("evals/librarian.json")
    ///     .expect("Failed to save eval suite");
    /// ```
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize eval suite")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write eval suite: {}", path.display()))?;

        Ok(())
    }

    /// Get number of test cases in the suite
    #[must_use]
    pub fn len(&self) -> usize {
        self.cases.len()
    }

    /// Check if suite is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}

/// Scoring methods for comparing actual vs expected answers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoringMethod {
    /// Exact string match (case-sensitive)
    ExactMatch,

    /// Case-insensitive exact match
    CaseInsensitiveMatch,

    /// Fuzzy match using normalized Levenshtein distance
    /// Score = 1.0 - (`edit_distance` / `max_length`)
    FuzzyMatch,

    /// Contains check: expected answer is substring of actual
    Contains,
}

/// Score a single answer against expected output
///
/// # Arguments
///
/// * `actual` - Actual answer produced by the application
/// * `expected` - Expected correct answer
/// * `method` - Scoring method to use
///
/// # Returns
///
/// Score between 0.0 (completely wrong) and 1.0 (perfect match)
///
/// # Examples
///
/// ```
/// use dashflow_streaming::evals::{score_answer, ScoringMethod};
///
/// // Exact match
/// let score = score_answer("Paris", "Paris", ScoringMethod::ExactMatch);
/// assert_eq!(score, 1.0);
///
/// // Case insensitive
/// let score = score_answer("paris", "Paris", ScoringMethod::CaseInsensitiveMatch);
/// assert_eq!(score, 1.0);
///
/// // Fuzzy match
/// let score = score_answer("Parus", "Paris", ScoringMethod::FuzzyMatch);
/// assert!(score > 0.7 && score < 1.0);
///
/// // Contains
/// let score = score_answer("The capital is Paris", "Paris", ScoringMethod::Contains);
/// assert_eq!(score, 1.0);
/// ```
#[must_use]
pub fn score_answer(actual: &str, expected: &str, method: ScoringMethod) -> f64 {
    match method {
        ScoringMethod::ExactMatch => {
            if actual == expected {
                1.0
            } else {
                0.0
            }
        }

        ScoringMethod::CaseInsensitiveMatch => {
            if actual.to_lowercase() == expected.to_lowercase() {
                1.0
            } else {
                0.0
            }
        }

        ScoringMethod::FuzzyMatch => {
            // Normalized Levenshtein distance
            let distance = levenshtein_distance(actual, expected);
            let max_len = actual.len().max(expected.len()) as f64;

            if max_len == 0.0 {
                1.0 // Both strings empty
            } else {
                (max_len - distance as f64) / max_len
            }
        }

        ScoringMethod::Contains => {
            if actual.contains(expected) {
                1.0
            } else {
                0.0
            }
        }
    }
}

/// Compute Levenshtein distance between two strings
///
/// Implementation uses dynamic programming with O(n*m) time and space complexity.
///
/// # Arguments
///
/// * `s1` - First string
/// * `s2` - Second string
///
/// # Returns
///
/// Minimum number of single-character edits (insertions, deletions, substitutions)
/// needed to transform s1 into s2
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    let len1 = s1_chars.len();
    let len2 = s2_chars.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    // Initialize DP table
    let mut dp = vec![vec![0; len2 + 1]; len1 + 1];

    // Base cases: distance from empty string
    #[allow(clippy::needless_range_loop)] // Index equals value: dp[i][0] = i requires index access
    for i in 0..=len1 {
        dp[i][0] = i;
    }
    #[allow(clippy::needless_range_loop)] // Index equals value: dp[0][j] = j requires index access
    for j in 0..=len2 {
        dp[0][j] = j;
    }

    // Fill DP table
    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = usize::from(s1_chars[i - 1] != s2_chars[j - 1]);

            dp[i][j] = (dp[i - 1][j] + 1) // deletion
                .min(dp[i][j - 1] + 1) // insertion
                .min(dp[i - 1][j - 1] + cost); // substitution
        }
    }

    dp[len1][len2]
}

/// Score all cases in an eval suite
///
/// # Arguments
///
/// * `suite` - Eval suite with test cases
/// * `actual_answers` - Actual answers produced by the application (same order as suite.cases)
/// * `method` - Scoring method to use
///
/// # Returns
///
/// Vector of scores (0.0-1.0) for each case, or error if lengths don't match
///
/// # Errors
///
/// Returns error if `actual_answers.len() != suite.cases.len()`
///
/// # Examples
///
/// ```
/// use dashflow_streaming::evals::{EvalSuite, EvalCase, score_suite, ScoringMethod};
///
/// let suite = EvalSuite {
///     name: "test".to_string(),
///     description: "Test suite".to_string(),
///     version: "1.0.0".to_string(),
///     cases: vec![
///         EvalCase {
///             id: "001".to_string(),
///             query: "What is 2+2?".to_string(),
///             expected_answer: "4".to_string(),
///             metadata: None,
///         },
///         EvalCase {
///             id: "002".to_string(),
///             query: "What is the capital of France?".to_string(),
///             expected_answer: "Paris".to_string(),
///             metadata: None,
///         },
///     ],
/// };
///
/// let actual_answers = vec!["4".to_string(), "Paris".to_string()];
/// let scores = score_suite(&suite, &actual_answers, ScoringMethod::ExactMatch).unwrap();
///
/// assert_eq!(scores.len(), 2);
/// assert_eq!(scores[0], 1.0);
/// assert_eq!(scores[1], 1.0);
/// ```
pub fn score_suite(
    suite: &EvalSuite,
    actual_answers: &[String],
    method: ScoringMethod,
) -> Result<Vec<f64>> {
    if actual_answers.len() != suite.cases.len() {
        anyhow::bail!(
            "Mismatch: {} actual answers but {} test cases",
            actual_answers.len(),
            suite.cases.len()
        );
    }

    let scores: Vec<f64> = suite
        .cases
        .iter()
        .zip(actual_answers.iter())
        .map(|(case, actual)| score_answer(actual, &case.expected_answer, method))
        .collect();

    Ok(scores)
}

/// Compute average correctness score from individual case scores
///
/// # Arguments
///
/// * `scores` - Individual case scores (0.0-1.0)
///
/// # Returns
///
/// Average score (0.0-1.0), or 0.0 if scores is empty
///
/// # Examples
///
/// ```
/// use dashflow_streaming::evals::average_correctness;
///
/// let scores = vec![1.0, 0.8, 0.9, 1.0];
/// let avg = average_correctness(&scores);
/// assert_eq!(avg, 0.925);
/// ```
#[must_use]
pub fn average_correctness(scores: &[f64]) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }

    let sum: f64 = scores.iter().sum();
    sum / scores.len() as f64
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_answer_exact_match() {
        assert_eq!(
            score_answer("Paris", "Paris", ScoringMethod::ExactMatch),
            1.0
        );
        assert_eq!(
            score_answer("Paris", "paris", ScoringMethod::ExactMatch),
            0.0
        );
        assert_eq!(
            score_answer("London", "Paris", ScoringMethod::ExactMatch),
            0.0
        );
    }

    #[test]
    fn test_score_answer_case_insensitive() {
        assert_eq!(
            score_answer("Paris", "Paris", ScoringMethod::CaseInsensitiveMatch),
            1.0
        );
        assert_eq!(
            score_answer("Paris", "paris", ScoringMethod::CaseInsensitiveMatch),
            1.0
        );
        assert_eq!(
            score_answer("PARIS", "paris", ScoringMethod::CaseInsensitiveMatch),
            1.0
        );
        assert_eq!(
            score_answer("London", "Paris", ScoringMethod::CaseInsensitiveMatch),
            0.0
        );
    }

    #[test]
    fn test_score_answer_contains() {
        assert_eq!(
            score_answer("The capital is Paris", "Paris", ScoringMethod::Contains),
            1.0
        );
        assert_eq!(
            score_answer("Paris is the capital", "Paris", ScoringMethod::Contains),
            1.0
        );
        assert_eq!(
            score_answer("London", "Paris", ScoringMethod::Contains),
            0.0
        );
    }

    #[test]
    fn test_score_answer_fuzzy_match() {
        // Perfect match
        assert_eq!(
            score_answer("Paris", "Paris", ScoringMethod::FuzzyMatch),
            1.0
        );

        // One character different
        let score = score_answer("Parus", "Paris", ScoringMethod::FuzzyMatch);
        assert!(
            score > 0.7 && score < 1.0,
            "Score should be ~0.8, got {}",
            score
        );

        // Completely different
        let score = score_answer("London", "Paris", ScoringMethod::FuzzyMatch);
        assert!(score < 0.5, "Score should be low, got {}", score);

        // Empty strings
        assert_eq!(score_answer("", "", ScoringMethod::FuzzyMatch), 1.0);
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("Paris", "Paris"), 0);
        assert_eq!(levenshtein_distance("Paris", "Parus"), 1);
        assert_eq!(levenshtein_distance("Paris", "London"), 6);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_score_suite() {
        let suite = EvalSuite {
            name: "test".to_string(),
            description: "Test suite".to_string(),
            version: "1.0.0".to_string(),
            cases: vec![
                EvalCase {
                    id: "001".to_string(),
                    query: "Q1".to_string(),
                    expected_answer: "A1".to_string(),
                    metadata: None,
                },
                EvalCase {
                    id: "002".to_string(),
                    query: "Q2".to_string(),
                    expected_answer: "A2".to_string(),
                    metadata: None,
                },
            ],
        };

        let actual = vec!["A1".to_string(), "A2".to_string()];
        let scores = score_suite(&suite, &actual, ScoringMethod::ExactMatch).unwrap();

        assert_eq!(scores.len(), 2);
        assert_eq!(scores[0], 1.0);
        assert_eq!(scores[1], 1.0);
    }

    #[test]
    fn test_score_suite_length_mismatch() {
        let suite = EvalSuite {
            name: "test".to_string(),
            description: "Test suite".to_string(),
            version: "1.0.0".to_string(),
            cases: vec![EvalCase {
                id: "001".to_string(),
                query: "Q1".to_string(),
                expected_answer: "A1".to_string(),
                metadata: None,
            }],
        };

        let actual = vec!["A1".to_string(), "A2".to_string()];
        let result = score_suite(&suite, &actual, ScoringMethod::ExactMatch);

        assert!(result.is_err());
    }

    #[test]
    fn test_average_correctness() {
        assert_eq!(average_correctness(&[]), 0.0);
        assert_eq!(average_correctness(&[1.0]), 1.0);
        assert_eq!(average_correctness(&[1.0, 0.8, 0.9, 1.0]), 0.925);
        assert_eq!(average_correctness(&[0.0, 0.0]), 0.0);
    }

    #[test]
    fn test_eval_suite_len() {
        let suite = EvalSuite {
            name: "test".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            cases: vec![],
        };

        assert_eq!(suite.len(), 0);
        assert!(suite.is_empty());
    }
}
