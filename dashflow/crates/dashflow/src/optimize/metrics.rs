// Allow clippy warnings for this module
// - float_cmp: Metric comparisons with exact thresholds
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::clone_on_ref_ptr,
    clippy::float_cmp
)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Metrics for DashOptimize
//!
//! Metric functions evaluate the quality of LLM outputs for optimization.
//! Metrics return a score from 0.0 (worst) to 1.0 (best).
//!
//! ## Available Metrics
//!
//! - `exact_match`: Returns 1.0 if strings match exactly (case-insensitive, normalized)
//! - `f1_score`: Token-level F1 score (measures overlap)
//! - `precision_score`: Token-level precision
//! - `max_f1`: Maximum F1 score against multiple reference answers
//! - `SemanticF1`: LLM-as-judge metric using recall and precision via LLM calls (see `semantic_f1` module)

use crate::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// Metric function type: takes expected state and predicted state, returns quality score (0.0 to 1.0)
///
/// # Arguments
/// * `expected` - The training example with expected outputs
/// * `predicted` - The state produced by the node
///
/// # Returns
/// Score from 0.0 (completely wrong) to 1.0 (perfect match)
pub type MetricFn<S> = Arc<dyn Fn(&S, &S) -> Result<f64> + Send + Sync>;

// ============================================================================
// Text Normalization
// ============================================================================

/// Normalize text for string and token comparisons.
///
/// Performs the following transformations:
/// 1. Lowercase conversion
/// 2. Punctuation removal (hyphens are converted to spaces to preserve word boundaries)
/// 3. English article removal ("a", "an", "the")
/// 4. Whitespace collapse
///
/// # Hyphen Handling
///
/// Hyphens are converted to spaces rather than removed, so hyphenated compound words
/// become separate tokens: "state-of-the-art" → "state of art" (after article removal).
/// This preserves the semantic meaning of compound words in token-based metrics.
///
/// # Example
///
/// ```
/// use dashflow::optimize::metrics::normalize_text;
///
/// assert_eq!(normalize_text("The,  Eiffel  Tower!"), "eiffel tower");
/// assert_eq!(normalize_text("state-of-the-art"), "state of art");
/// ```
pub fn normalize_text(s: &str) -> String {
    // Lowercase
    let lower = s.to_lowercase();

    // Convert hyphens to spaces (preserves word boundaries in compounds)
    // Then remove other punctuation
    let no_punc: String = lower
        .chars()
        .map(|c| if c == '-' { ' ' } else { c })
        .filter(|c| !c.is_ascii_punctuation())
        .collect();

    // Remove articles (a, an, the) - must be whole words
    let no_articles = remove_articles(&no_punc);

    // Collapse whitespace
    white_space_fix(&no_articles)
}

/// Remove English articles from text.
///
/// # Language Support
///
/// This function only removes English articles: "a", "an", "the".
/// For multilingual text, consider using language-specific normalization
/// or skipping article removal entirely (use `normalize_text_keep_articles`
/// if added in the future).
///
/// # Note
///
/// Articles must be whole words; partial matches are not removed.
/// E.g., "another" is not affected.
fn remove_articles(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let filtered: Vec<&str> = words
        .into_iter()
        .filter(|&word| !matches!(word, "a" | "an" | "the"))
        .collect();
    filtered.join(" ")
}

fn white_space_fix(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ============================================================================
// Exact Match Metrics
// ============================================================================

/// Exact match metric: returns 1.0 if two strings match exactly after normalization, 0.0 otherwise
///
/// This is a helper for creating metrics that compare string fields.
///
/// # Example
///
/// ```
/// use dashflow::optimize::metrics::exact_match;
///
/// assert_eq!(exact_match("Paris", "paris"), 1.0);
/// assert_eq!(exact_match("Paris", "London"), 0.0);
/// ```
pub fn exact_match(expected: &str, actual: &str) -> f64 {
    if normalize_text(expected) == normalize_text(actual) {
        1.0
    } else {
        0.0
    }
}

/// Check if prediction exactly matches any of the reference answers.
///
/// Returns `true` if any reference exactly matches the prediction after normalization.
///
/// # Example
///
/// ```
/// use dashflow::optimize::metrics::exact_match_any;
///
/// assert!(exact_match_any("The Eiffel Tower", &["Eiffel Tower", "Louvre"]));
/// assert!(exact_match_any("paris", &["Paris"]));
/// assert!(!exact_match_any("paris", &["Paris, France"]));
/// ```
pub fn exact_match_any(prediction: &str, answers_list: &[&str]) -> bool {
    answers_list
        .iter()
        .any(|&ans| exact_match(prediction, ans) == 1.0)
}

// ============================================================================
// F1 Score Metrics
// ============================================================================

/// Compute token-level F1 between prediction and reference (after normalization).
///
/// Strings are normalized and split by whitespace. F1 is computed from token
/// precision and recall. Returns 0.0 if there is no token overlap.
///
/// # Example
///
/// ```
/// use dashflow::optimize::metrics::f1_score;
///
/// let score = f1_score("the Eiffel Tower", "Eiffel Tower");
/// assert!((score - 1.0).abs() < 0.01);
/// ```
pub fn f1_score(prediction: &str, ground_truth: &str) -> f64 {
    let pred_normalized = normalize_text(prediction);
    let gt_normalized = normalize_text(ground_truth);

    let prediction_tokens: Vec<&str> = pred_normalized.split_whitespace().collect();
    let ground_truth_tokens: Vec<&str> = gt_normalized.split_whitespace().collect();

    // Edge case: both empty
    if prediction_tokens.is_empty() && ground_truth_tokens.is_empty() {
        return 0.0;
    }

    // Count token overlaps
    let common = count_overlap(&prediction_tokens, &ground_truth_tokens);
    let num_same = common.values().sum::<usize>();

    if num_same == 0 {
        return 0.0;
    }

    let precision = num_same as f64 / prediction_tokens.len() as f64;
    let recall = num_same as f64 / ground_truth_tokens.len() as f64;

    (2.0 * precision * recall) / (precision + recall)
}

/// Compute the maximum token-level F1 score against reference answers.
///
/// Returns the maximum F1 over all provided references.
///
/// # Example
///
/// ```
/// use dashflow::optimize::metrics::max_f1;
///
/// let score = max_f1("Eiffel Tower is in Paris", &["Paris"]);
/// assert!((score - 0.33).abs() < 0.01);
/// ```
pub fn max_f1(prediction: &str, answers_list: &[&str]) -> f64 {
    answers_list
        .iter()
        .map(|&ans| f1_score(prediction, ans))
        .fold(0.0, f64::max)
}

/// Compute token-level precision of prediction against reference (after normalization).
///
/// Precision is (# overlapping tokens) / (# tokens in prediction).
/// Returns 0.0 if there is no token overlap.
///
/// # Example
///
/// ```
/// use dashflow::optimize::metrics::precision_score;
///
/// let score = precision_score("eiffel tower in paris", "eiffel tower");
/// assert!((score - 0.5).abs() < 0.01); // 2 out of 4 tokens match
/// ```
pub fn precision_score(prediction: &str, ground_truth: &str) -> f64 {
    let pred_normalized = normalize_text(prediction);
    let gt_normalized = normalize_text(ground_truth);

    let prediction_tokens: Vec<&str> = pred_normalized.split_whitespace().collect();
    let ground_truth_tokens: Vec<&str> = gt_normalized.split_whitespace().collect();

    // Edge case: both empty
    if prediction_tokens.is_empty() && ground_truth_tokens.is_empty() {
        return 0.0;
    }

    if prediction_tokens.is_empty() {
        return 0.0;
    }

    let common = count_overlap(&prediction_tokens, &ground_truth_tokens);
    let num_same = common.values().sum::<usize>();

    if num_same == 0 {
        return 0.0;
    }

    num_same as f64 / prediction_tokens.len() as f64
}

/// Count overlapping tokens between two token lists.
///
/// Returns a HashMap where keys are tokens and values are the minimum count
/// of that token in both lists.
fn count_overlap<'a>(tokens1: &[&'a str], tokens2: &[&'a str]) -> HashMap<&'a str, usize> {
    let mut counts1: HashMap<&str, usize> = HashMap::new();
    let mut counts2: HashMap<&str, usize> = HashMap::new();

    for &token in tokens1 {
        *counts1.entry(token).or_insert(0) += 1;
    }

    for &token in tokens2 {
        *counts2.entry(token).or_insert(0) += 1;
    }

    let mut overlap = HashMap::new();
    for (&token, &count1) in &counts1 {
        if let Some(&count2) = counts2.get(token) {
            overlap.insert(token, count1.min(count2));
        }
    }

    overlap
}

// ============================================================================
// LLM-as-Judge Metric (Semantic F1)
// ============================================================================

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;

/// SemanticF1 evaluator using an LLM as a judge.
///
/// This metric uses an LLM to evaluate semantic similarity between
/// expected and actual outputs by computing recall and precision scores.
///
/// ## How it works
///
/// 1. **Recall**: Asks the LLM to assess what fraction of the expected
///    information is captured in the prediction
/// 2. **Precision**: Asks the LLM to assess what fraction of the predicted
///    information is correct/relevant
/// 3. **F1**: Computes the harmonic mean of recall and precision
///
/// ## Example
///
/// ```rust,ignore
/// use dashflow::optimize::metrics::SemanticF1;
/// use std::sync::Arc;
///
/// let llm: Arc<dyn ChatModel> = /* your LLM */;
/// let evaluator = SemanticF1::new(llm);
///
/// let question = "What is the capital of France?";
/// let expected = "Paris is the capital of France.";
/// let prediction = "The capital is Paris.";
///
/// let score = evaluator.evaluate(question, expected, prediction).await?;
/// println!("Semantic F1: {:.2}", score.f1);
/// ```
pub struct SemanticF1 {
    llm: std::sync::Arc<dyn ChatModel>,
}

/// Result of SemanticF1 evaluation
#[derive(Debug, Clone, Copy)]
pub struct SemanticF1Result {
    /// Recall score (0.0-1.0): fraction of expected info captured
    pub recall: f64,
    /// Precision score (0.0-1.0): fraction of prediction that is correct
    pub precision: f64,
    /// F1 score (0.0-1.0): harmonic mean of recall and precision
    pub f1: f64,
}

impl SemanticF1 {
    /// Create a new SemanticF1 evaluator with the given LLM.
    pub fn new(llm: std::sync::Arc<dyn ChatModel>) -> Self {
        Self { llm }
    }

    /// Evaluate semantic similarity between expected and predicted text.
    ///
    /// # Arguments
    ///
    /// * `question` - The original question or context (optional, can be empty)
    /// * `expected` - The ground truth/expected answer
    /// * `prediction` - The model's prediction to evaluate
    ///
    /// # Returns
    ///
    /// A `SemanticF1Result` containing recall, precision, and F1 scores.
    pub async fn evaluate(
        &self,
        question: &str,
        expected: &str,
        prediction: &str,
    ) -> crate::Result<SemanticF1Result> {
        let recall = self.compute_recall(question, expected, prediction).await?;
        let precision = self
            .compute_precision(question, expected, prediction)
            .await?;

        let f1 = if recall + precision > 0.0 {
            2.0 * recall * precision / (recall + precision)
        } else {
            0.0
        };

        Ok(SemanticF1Result {
            recall,
            precision,
            f1,
        })
    }

    /// Compute recall: what fraction of expected information is in prediction?
    async fn compute_recall(
        &self,
        question: &str,
        expected: &str,
        prediction: &str,
    ) -> crate::Result<f64> {
        let prompt = self.build_recall_prompt(question, expected, prediction);
        self.extract_score(&prompt).await
    }

    /// Compute precision: what fraction of prediction is correct/relevant?
    async fn compute_precision(
        &self,
        question: &str,
        expected: &str,
        prediction: &str,
    ) -> crate::Result<f64> {
        let prompt = self.build_precision_prompt(question, expected, prediction);
        self.extract_score(&prompt).await
    }

    fn build_recall_prompt(&self, question: &str, expected: &str, prediction: &str) -> String {
        let context = if question.is_empty() {
            String::new()
        } else {
            format!("Question: {}\n\n", question)
        };

        format!(
            r#"You are evaluating the RECALL of a system response compared to a reference answer.

{context}Reference Answer (ground truth):
{expected}

System Response (prediction):
{prediction}

RECALL measures what fraction of the information in the reference answer is captured by the system response.

Instructions:
1. Identify the key facts/claims in the reference answer
2. Check how many of these are present (even if rephrased) in the system response
3. Score from 0.0 to 1.0 where:
   - 1.0 = All key information from reference is captured
   - 0.5 = About half the key information is captured
   - 0.0 = None of the key information is captured

Output ONLY a single decimal number between 0.0 and 1.0.
Score:"#
        )
    }

    fn build_precision_prompt(&self, question: &str, expected: &str, prediction: &str) -> String {
        let context = if question.is_empty() {
            String::new()
        } else {
            format!("Question: {}\n\n", question)
        };

        format!(
            r#"You are evaluating the PRECISION of a system response compared to a reference answer.

{context}Reference Answer (ground truth):
{expected}

System Response (prediction):
{prediction}

PRECISION measures what fraction of the information in the system response is correct/relevant.

Instructions:
1. Identify the claims/statements in the system response
2. Check how many of these are correct according to the reference answer
3. Score from 0.0 to 1.0 where:
   - 1.0 = All information in the response is correct/relevant
   - 0.5 = About half the information is correct
   - 0.0 = None of the information is correct (all hallucinated or wrong)

Output ONLY a single decimal number between 0.0 and 1.0.
Score:"#
        )
    }

    async fn extract_score(&self, prompt: &str) -> crate::Result<f64> {
        let messages = vec![Message::human(prompt.to_string())];
        let result = self
            .llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| crate::Error::Generic(format!("SemanticF1 LLM call failed: {}", e)))?;

        let response = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        // Parse the score from the response
        parse_score_from_response(&response)
    }
}

/// Parse a score (0.0-1.0) from an LLM response.
///
/// Handles various formats:
/// - Just a number: "0.85"
/// - With text: "The score is 0.85"
/// - Percentage: "85%" -> 0.85
/// - Fractions: "3/4" -> 0.75
///
/// # Limitations
///
/// - Text numbers (e.g., "three quarters") are not supported
/// - Fractions must be simple (numerator/denominator), not mixed numbers
fn parse_score_from_response(response: &str) -> crate::Result<f64> {
    let response = response.trim();

    // Try to find a decimal number
    for word in response.split_whitespace() {
        // Handle percentage first (before trimming)
        if word.contains('%') {
            // Extract just the number part before %
            let num_part = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != '-');
            if let Ok(pct) = num_part.parse::<f64>() {
                return Ok((pct / 100.0).clamp(0.0, 1.0));
            }
        }

        // Handle fractions (e.g., "3/4")
        if word.contains('/') {
            let clean_word = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '/');
            if let Some((num_str, den_str)) = clean_word.split_once('/') {
                if let (Ok(num), Ok(den)) = (num_str.parse::<f64>(), den_str.parse::<f64>()) {
                    if den != 0.0 {
                        return Ok((num / den).clamp(0.0, 1.0));
                    }
                }
            }
        }

        // Trim non-numeric characters but keep decimal point and minus sign
        let word = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != '-');

        // Try to parse as decimal
        if let Ok(score) = word.parse::<f64>() {
            return Ok(score.clamp(0.0, 1.0));
        }
    }

    // Default to 0.0 if we can't parse
    Ok(0.0)
}

/// Configuration for SemanticF1 metric with JsonState.
///
/// This struct provides configuration for using SemanticF1 with the optimization framework.
///
/// # Example
///
/// ```rust,ignore
/// let config = SemanticF1Config::new(llm)
///     .with_question_field("question")
///     .with_expected_field("answer")
///     .with_actual_field("output");
///
/// let score = config.evaluate(&expected_state, &actual_state).await?;
/// ```
#[derive(Clone)]
pub struct SemanticF1Config {
    evaluator: SemanticF1,
    question_field: Option<String>,
    expected_field: String,
    actual_field: String,
}

impl SemanticF1Config {
    /// Create a new SemanticF1 config with the given LLM.
    pub fn new(llm: std::sync::Arc<dyn ChatModel>) -> Self {
        Self {
            evaluator: SemanticF1::new(llm),
            question_field: None,
            expected_field: "answer".to_string(),
            actual_field: "output".to_string(),
        }
    }

    /// Set the field name for the question/context (optional).
    #[must_use]
    pub fn with_question_field(mut self, field: impl Into<String>) -> Self {
        self.question_field = Some(field.into());
        self
    }

    /// Set the field name for the expected output.
    #[must_use]
    pub fn with_expected_field(mut self, field: impl Into<String>) -> Self {
        self.expected_field = field.into();
        self
    }

    /// Set the field name for the actual output.
    #[must_use]
    pub fn with_actual_field(mut self, field: impl Into<String>) -> Self {
        self.actual_field = field.into();
        self
    }

    /// Evaluate two JsonStates and return the SemanticF1 score.
    pub async fn evaluate(&self, expected: &JsonState, actual: &JsonState) -> crate::Result<f64> {
        let question = self
            .question_field
            .as_ref()
            .and_then(|f| expected.get_str(f))
            .unwrap_or("");
        let expected_text = expected.get_str(&self.expected_field).unwrap_or("");
        let actual_text = actual.get_str(&self.actual_field).unwrap_or("");

        let result = self
            .evaluator
            .evaluate(question, expected_text, actual_text)
            .await?;
        Ok(result.f1)
    }

    /// Evaluate two JsonStates and return the full SemanticF1Result.
    pub async fn evaluate_full(
        &self,
        expected: &JsonState,
        actual: &JsonState,
    ) -> crate::Result<SemanticF1Result> {
        let question = self
            .question_field
            .as_ref()
            .and_then(|f| expected.get_str(f))
            .unwrap_or("");
        let expected_text = expected.get_str(&self.expected_field).unwrap_or("");
        let actual_text = actual.get_str(&self.actual_field).unwrap_or("");

        self.evaluator
            .evaluate(question, expected_text, actual_text)
            .await
    }
}

impl Clone for SemanticF1 {
    fn clone(&self) -> Self {
        Self {
            llm: self.llm.clone(),
        }
    }
}

// ============================================================================
// JsonState Metrics for CLI
// ============================================================================

use crate::state::JsonState;

/// Evaluate exact match for JsonState with configurable field names
///
/// Compares the specified `expected_field` and `actual_field` across two states.
/// Returns 1.0 if they match after normalization, 0.0 otherwise.
///
/// # Missing vs Empty Fields
///
/// Missing fields are treated the same as empty strings (`""`). This means:
/// - `{"answer": ""}` (empty field) behaves the same as `{}` (missing field)
/// - Both result in comparing against an empty string
/// - Two missing fields will match (both become empty strings)
///
/// If you need to distinguish missing from empty, check field existence
/// explicitly before calling this function.
///
/// # Example
///
/// ```rust
/// use dashflow::state::JsonState;
/// use dashflow::optimize::metrics::json_exact_match;
///
/// let expected = JsonState::from(serde_json::json!({"answer": "Paris"}));
/// let actual = JsonState::from(serde_json::json!({"output": "paris"}));
///
/// let score = json_exact_match(&expected, &actual, "answer", "output");
/// assert_eq!(score, 1.0);
/// ```
pub fn json_exact_match(
    expected: &JsonState,
    actual: &JsonState,
    expected_field: &str,
    actual_field: &str,
) -> f64 {
    let expected_val = expected.get_str(expected_field).unwrap_or("");
    let actual_val = actual.get_str(actual_field).unwrap_or("");
    exact_match(expected_val, actual_val)
}

/// Evaluate F1 score for JsonState with configurable field names
///
/// Computes token-level F1 between the specified fields.
///
/// # Missing vs Empty Fields
///
/// Missing fields are treated as empty strings. See [`json_exact_match`] for details.
///
/// # Example
///
/// ```rust
/// use dashflow::state::JsonState;
/// use dashflow::optimize::metrics::json_f1_score;
///
/// let expected = JsonState::from(serde_json::json!({"answer": "The Eiffel Tower"}));
/// let actual = JsonState::from(serde_json::json!({"output": "Eiffel Tower in Paris"}));
///
/// let score = json_f1_score(&expected, &actual, "answer", "output");
/// assert!(score > 0.5);
/// ```
pub fn json_f1_score(
    expected: &JsonState,
    actual: &JsonState,
    expected_field: &str,
    actual_field: &str,
) -> f64 {
    let expected_val = expected.get_str(expected_field).unwrap_or("");
    let actual_val = actual.get_str(actual_field).unwrap_or("");
    f1_score(actual_val, expected_val)
}

/// Evaluate precision score for JsonState with configurable field names
///
/// Computes token-level precision between the specified fields.
///
/// # Missing vs Empty Fields
///
/// Missing fields are treated as empty strings. See [`json_exact_match`] for details.
pub fn json_precision_score(
    expected: &JsonState,
    actual: &JsonState,
    expected_field: &str,
    actual_field: &str,
) -> f64 {
    let expected_val = expected.get_str(expected_field).unwrap_or("");
    let actual_val = actual.get_str(actual_field).unwrap_or("");
    precision_score(actual_val, expected_val)
}

/// Evaluate recall score for JsonState with configurable field names
///
/// Computes token-level recall between the specified fields.
/// Recall = (# overlapping tokens) / (# tokens in expected)
///
/// # Missing vs Empty Fields
///
/// Missing fields are treated as empty strings. See [`json_exact_match`] for details.
pub fn json_recall_score(
    expected: &JsonState,
    actual: &JsonState,
    expected_field: &str,
    actual_field: &str,
) -> f64 {
    let expected_val = expected.get_str(expected_field).unwrap_or("");
    let actual_val = actual.get_str(actual_field).unwrap_or("");
    recall_score(actual_val, expected_val)
}

/// Compute token-level recall of prediction against reference (after normalization).
///
/// Recall is (# overlapping tokens) / (# tokens in ground_truth).
/// Returns 0.0 if there is no token overlap.
///
/// # Example
///
/// ```
/// use dashflow::optimize::metrics::recall_score;
///
/// let score = recall_score("eiffel tower in paris", "eiffel tower");
/// assert!((score - 1.0).abs() < 0.01); // all reference tokens present
/// ```
pub fn recall_score(prediction: &str, ground_truth: &str) -> f64 {
    let pred_normalized = normalize_text(prediction);
    let gt_normalized = normalize_text(ground_truth);

    let prediction_tokens: Vec<&str> = pred_normalized.split_whitespace().collect();
    let ground_truth_tokens: Vec<&str> = gt_normalized.split_whitespace().collect();

    // Edge case: both empty
    if prediction_tokens.is_empty() && ground_truth_tokens.is_empty() {
        return 0.0;
    }

    if ground_truth_tokens.is_empty() {
        return 0.0;
    }

    let common = count_overlap(&prediction_tokens, &ground_truth_tokens);
    let num_same = common.values().sum::<usize>();

    if num_same == 0 {
        return 0.0;
    }

    num_same as f64 / ground_truth_tokens.len() as f64
}

/// Evaluation configuration for JsonState metrics
///
/// Specifies which fields to compare when evaluating JsonState.
#[derive(Debug, Clone)]
pub struct JsonMetricConfig {
    /// Field name containing the expected output in ground truth examples
    pub expected_field: String,
    /// Field name containing the actual output from model predictions
    pub actual_field: String,
}

impl Default for JsonMetricConfig {
    fn default() -> Self {
        Self {
            expected_field: "answer".to_string(),
            actual_field: "output".to_string(),
        }
    }
}

impl JsonMetricConfig {
    /// Create a new config with custom field names
    pub fn new(expected_field: impl Into<String>, actual_field: impl Into<String>) -> Self {
        Self {
            expected_field: expected_field.into(),
            actual_field: actual_field.into(),
        }
    }

    /// Create metric function for exact match
    pub fn exact_match_fn(&self) -> impl Fn(&JsonState, &JsonState) -> f64 + '_ {
        move |expected, actual| {
            json_exact_match(expected, actual, &self.expected_field, &self.actual_field)
        }
    }

    /// Create metric function for F1 score
    pub fn f1_score_fn(&self) -> impl Fn(&JsonState, &JsonState) -> f64 + '_ {
        move |expected, actual| {
            json_f1_score(expected, actual, &self.expected_field, &self.actual_field)
        }
    }

    /// Create metric function for precision
    pub fn precision_fn(&self) -> impl Fn(&JsonState, &JsonState) -> f64 + '_ {
        move |expected, actual| {
            json_precision_score(expected, actual, &self.expected_field, &self.actual_field)
        }
    }

    /// Create metric function for recall
    pub fn recall_fn(&self) -> impl Fn(&JsonState, &JsonState) -> f64 + '_ {
        move |expected, actual| {
            json_recall_score(expected, actual, &self.expected_field, &self.actual_field)
        }
    }
}

/// Compute all standard metrics for JsonState evaluation
///
/// Returns a map of metric names to scores.
///
/// # Example
///
/// ```rust
/// use dashflow::state::JsonState;
/// use dashflow::optimize::metrics::{compute_all_json_metrics, JsonMetricConfig};
///
/// let expected = JsonState::from(serde_json::json!({"answer": "Paris"}));
/// let actual = JsonState::from(serde_json::json!({"output": "Paris, France"}));
///
/// let config = JsonMetricConfig::default();
/// let metrics = compute_all_json_metrics(&expected, &actual, &config);
///
/// assert!(metrics.get("exact_match").is_some());
/// assert!(metrics.get("f1").is_some());
/// assert!(metrics.get("precision").is_some());
/// assert!(metrics.get("recall").is_some());
/// ```
pub fn compute_all_json_metrics(
    expected: &JsonState,
    actual: &JsonState,
    config: &JsonMetricConfig,
) -> HashMap<String, f64> {
    let mut metrics = HashMap::new();

    metrics.insert(
        "exact_match".to_string(),
        json_exact_match(
            expected,
            actual,
            &config.expected_field,
            &config.actual_field,
        ),
    );
    metrics.insert(
        "f1".to_string(),
        json_f1_score(
            expected,
            actual,
            &config.expected_field,
            &config.actual_field,
        ),
    );
    metrics.insert(
        "precision".to_string(),
        json_precision_score(
            expected,
            actual,
            &config.expected_field,
            &config.actual_field,
        ),
    );
    metrics.insert(
        "recall".to_string(),
        json_recall_score(
            expected,
            actual,
            &config.expected_field,
            &config.actual_field,
        ),
    );

    metrics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_text() {
        assert_eq!(normalize_text("The,  Eiffel  Tower!"), "eiffel tower");
        assert_eq!(normalize_text("An apple"), "apple");
        assert_eq!(normalize_text("A test"), "test");
        assert_eq!(normalize_text("hello   world"), "hello world");
    }

    #[test]
    fn test_exact_match() {
        assert_eq!(exact_match("Paris", "paris"), 1.0);
        assert_eq!(exact_match("The Eiffel Tower", "Eiffel Tower"), 1.0);
        assert_eq!(exact_match("Paris", "France"), 0.0);
    }

    #[test]
    fn test_exact_match_any() {
        assert!(exact_match_any(
            "The Eiffel Tower",
            &["Eiffel Tower", "Louvre"]
        ));
        assert!(exact_match_any("paris", &["Paris"]));
        assert!(!exact_match_any("paris", &["Paris, France"]));
    }

    #[test]
    fn test_f1_score() {
        let score = f1_score("the Eiffel Tower", "Eiffel Tower");
        assert!((score - 1.0).abs() < 0.01);

        let score = f1_score("Paris France", "Paris");
        assert!((score - 0.67).abs() < 0.01);
    }

    #[test]
    fn test_max_f1() {
        let score = max_f1("Eiffel Tower is in Paris", &["Paris"]);
        assert!((score - 0.33).abs() < 0.01);
    }

    #[test]
    fn test_precision_score() {
        let score = precision_score("eiffel tower in paris", "eiffel tower");
        assert!((score - 0.5).abs() < 0.01); // 2 out of 4 tokens match
    }

    #[test]
    fn test_count_overlap() {
        let tokens1 = vec!["hello", "world", "hello"];
        let tokens2 = vec!["hello", "there"];
        let overlap = count_overlap(&tokens1, &tokens2);
        assert_eq!(overlap.get("hello"), Some(&1));
        assert_eq!(overlap.get("world"), None);
    }

    #[test]
    fn test_f1_empty_strings() {
        assert_eq!(f1_score("", ""), 0.0); // Both empty = no tokens
        assert_eq!(f1_score("test", ""), 0.0); // One empty = no overlap
        assert_eq!(f1_score("", "test"), 0.0);
    }

    #[test]
    fn test_f1_case_sensitivity() {
        // F1 should be case insensitive (normalized)
        let score = f1_score("Paris", "paris");
        assert!((score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_f1_punctuation() {
        // Punctuation should be handled
        let score = f1_score("Hello, world!", "Hello world");
        assert!(score > 0.9); // Should be high overlap
    }

    #[test]
    fn test_f1_perfect_match() {
        let score = f1_score("yes", "yes");
        assert!((score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_f1_no_overlap() {
        // These have no overlap
        let score = f1_score("yes", "no");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_precision_empty() {
        assert_eq!(precision_score("", ""), 0.0); // No tokens
        assert_eq!(precision_score("test", ""), 0.0);
        assert_eq!(precision_score("", "test"), 0.0);
    }

    #[test]
    fn test_precision_perfect_subset() {
        let score = precision_score("hello world", "hello");
        assert!((score - 0.5).abs() < 0.01); // 1 out of 2 tokens match
    }

    #[test]
    fn test_exact_match_normalization() {
        assert_eq!(exact_match("  Yes  ", "yes"), 1.0);
        assert_eq!(exact_match("YES", "yes"), 1.0);
        assert_eq!(exact_match("no", "yes"), 0.0);
    }

    #[test]
    fn test_exact_match_any_normalization() {
        assert!(exact_match_any("Paris", &["paris", "france"]));
        assert!(exact_match_any("PARIS", &["paris"]));
        assert!(!exact_match_any("London", &["paris", "france"]));
    }

    #[test]
    fn test_max_f1_picks_best() {
        // Should pick the highest F1 score among answers
        let score = max_f1("hello world", &["hello", "world", "foo"]);
        // "world" matches one of two tokens, "hello" also matches one of two
        // F1 for "world" vs "hello world" = 2*P*R/(P+R) where P=1.0, R=0.5 = 0.666...
        assert!(score > 0.6 && score < 0.7);
    }

    #[test]
    fn test_max_f1_empty_list() {
        let score = max_f1("hello", &[]);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_count_overlap_empty() {
        let tokens1: Vec<&str> = vec![];
        let tokens2: Vec<&str> = vec![];
        let overlap = count_overlap(&tokens1, &tokens2);
        assert_eq!(overlap.len(), 0);
    }

    #[test]
    fn test_count_overlap_no_match() {
        let tokens1 = vec!["hello", "world"];
        let tokens2 = vec!["foo", "bar"];
        let overlap = count_overlap(&tokens1, &tokens2);
        assert_eq!(overlap.len(), 0);
    }

    // ========================================================================
    // JsonState metrics tests
    // ========================================================================

    #[test]
    fn test_json_exact_match() {
        let expected = JsonState::from(serde_json::json!({"answer": "Paris"}));
        let actual = JsonState::from(serde_json::json!({"output": "paris"}));
        assert_eq!(
            json_exact_match(&expected, &actual, "answer", "output"),
            1.0
        );
    }

    #[test]
    fn test_json_exact_match_miss() {
        let expected = JsonState::from(serde_json::json!({"answer": "Paris"}));
        let actual = JsonState::from(serde_json::json!({"output": "London"}));
        assert_eq!(
            json_exact_match(&expected, &actual, "answer", "output"),
            0.0
        );
    }

    #[test]
    fn test_json_f1_score() {
        let expected = JsonState::from(serde_json::json!({"answer": "Eiffel Tower"}));
        let actual = JsonState::from(serde_json::json!({"output": "The Eiffel Tower"}));
        let score = json_f1_score(&expected, &actual, "answer", "output");
        assert!((score - 1.0).abs() < 0.01); // normalization removes "The"
    }

    #[test]
    fn test_json_precision_score() {
        let expected = JsonState::from(serde_json::json!({"answer": "Paris"}));
        let actual = JsonState::from(serde_json::json!({"output": "Paris France"}));
        let score = json_precision_score(&expected, &actual, "answer", "output");
        assert!((score - 0.5).abs() < 0.01); // 1 out of 2 tokens match
    }

    #[test]
    fn test_json_recall_score() {
        let expected = JsonState::from(serde_json::json!({"answer": "hello world"}));
        let actual = JsonState::from(serde_json::json!({"output": "hello"}));
        let score = json_recall_score(&expected, &actual, "answer", "output");
        assert!((score - 0.5).abs() < 0.01); // 1 out of 2 expected tokens
    }

    #[test]
    fn test_recall_score_basic() {
        let score = recall_score("eiffel tower in paris", "eiffel tower");
        assert!((score - 1.0).abs() < 0.01); // all reference tokens present
    }

    #[test]
    fn test_json_metric_config_default() {
        let config = JsonMetricConfig::default();
        assert_eq!(config.expected_field, "answer");
        assert_eq!(config.actual_field, "output");
    }

    #[test]
    fn test_json_metric_config_custom() {
        let config = JsonMetricConfig::new("target", "prediction");
        assert_eq!(config.expected_field, "target");
        assert_eq!(config.actual_field, "prediction");
    }

    #[test]
    fn test_compute_all_json_metrics() {
        let expected = JsonState::from(serde_json::json!({"answer": "Paris"}));
        let actual = JsonState::from(serde_json::json!({"output": "paris"}));
        let config = JsonMetricConfig::default();

        let metrics = compute_all_json_metrics(&expected, &actual, &config);

        assert_eq!(metrics.get("exact_match"), Some(&1.0));
        assert_eq!(metrics.get("f1"), Some(&1.0));
        assert_eq!(metrics.get("precision"), Some(&1.0));
        assert_eq!(metrics.get("recall"), Some(&1.0));
    }

    #[test]
    fn test_json_metrics_missing_field() {
        let expected = JsonState::from(serde_json::json!({"wrong_field": "Paris"}));
        let actual = JsonState::from(serde_json::json!({"output": "Paris"}));

        // When expected field is missing, should return 0.0 (comparing empty string)
        let score = json_exact_match(&expected, &actual, "answer", "output");
        assert_eq!(score, 0.0);
    }

    // ========================================================================
    // SemanticF1 LLM-as-Judge tests
    // ========================================================================

    use crate::core::callbacks::CallbackManager;
    use crate::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use crate::core::messages::BaseMessage;
    use async_trait::async_trait;

    /// Mock LLM for testing SemanticF1
    struct MockSemanticLLM {
        recall_score: f64,
        precision_score: f64,
    }

    impl MockSemanticLLM {
        fn new(recall: f64, precision: f64) -> Self {
            Self {
                recall_score: recall,
                precision_score: precision,
            }
        }
    }

    #[async_trait]
    impl ChatModel for MockSemanticLLM {
        async fn _generate(
            &self,
            messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _callbacks: Option<&CallbackManager>,
        ) -> crate::core::Result<ChatResult> {
            // Determine if this is a recall or precision prompt
            let prompt = messages
                .first()
                .map(|m| m.content().as_text())
                .unwrap_or_default();
            let score = if prompt.contains("RECALL") {
                self.recall_score
            } else {
                self.precision_score
            };

            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: Message::ai(format!("{:.2}", score)),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_semantic"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_parse_score_decimal() {
        assert!((parse_score_from_response("0.85").unwrap() - 0.85).abs() < 0.01);
        assert!((parse_score_from_response("1.0").unwrap() - 1.0).abs() < 0.01);
        assert!((parse_score_from_response("0.0").unwrap() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_score_percentage() {
        assert!((parse_score_from_response("85%").unwrap() - 0.85).abs() < 0.01);
        assert!((parse_score_from_response("100%").unwrap() - 1.0).abs() < 0.01);
        assert!((parse_score_from_response("0%").unwrap() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_score_with_text() {
        assert!((parse_score_from_response("The score is 0.75").unwrap() - 0.75).abs() < 0.01);
        assert!((parse_score_from_response("Score: 0.9").unwrap() - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_parse_score_clamps() {
        assert!((parse_score_from_response("1.5").unwrap() - 1.0).abs() < 0.01);
        assert!((parse_score_from_response("-0.5").unwrap() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_score_empty() {
        assert!((parse_score_from_response("").unwrap() - 0.0).abs() < 0.01);
        assert!((parse_score_from_response("no number here").unwrap() - 0.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_semantic_f1_perfect_match() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let evaluator = SemanticF1::new(llm);

        let result = evaluator.evaluate("What is 2+2?", "4", "4").await.unwrap();

        assert!((result.recall - 1.0).abs() < 0.01);
        assert!((result.precision - 1.0).abs() < 0.01);
        assert!((result.f1 - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_semantic_f1_partial_match() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(0.8, 0.6));
        let evaluator = SemanticF1::new(llm);

        let result = evaluator
            .evaluate(
                "What is the capital of France?",
                "Paris is the capital of France",
                "Paris",
            )
            .await
            .unwrap();

        assert!((result.recall - 0.8).abs() < 0.01);
        assert!((result.precision - 0.6).abs() < 0.01);
        // F1 = 2 * 0.8 * 0.6 / (0.8 + 0.6) = 0.96 / 1.4 = 0.686
        assert!((result.f1 - 0.686).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_semantic_f1_no_match() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(0.0, 0.0));
        let evaluator = SemanticF1::new(llm);

        let result = evaluator
            .evaluate("What is the capital of France?", "Paris", "Berlin")
            .await
            .unwrap();

        assert!((result.recall - 0.0).abs() < 0.01);
        assert!((result.precision - 0.0).abs() < 0.01);
        assert!((result.f1 - 0.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_semantic_f1_empty_question() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(0.9, 0.9));
        let evaluator = SemanticF1::new(llm);

        // Empty question should still work
        let result = evaluator
            .evaluate("", "Paris", "Paris, France")
            .await
            .unwrap();

        assert!((result.recall - 0.9).abs() < 0.01);
        assert!((result.precision - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_semantic_f1_result_fields() {
        let result = SemanticF1Result {
            recall: 0.8,
            precision: 0.6,
            f1: 0.686,
        };

        assert_eq!(result.recall, 0.8);
        assert_eq!(result.precision, 0.6);
        assert!((result.f1 - 0.686).abs() < 0.001);
    }

    #[test]
    fn test_semantic_f1_clone() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let evaluator = SemanticF1::new(llm);
        let _cloned = evaluator.clone();
    }

    #[test]
    fn test_build_recall_prompt_with_question() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let evaluator = SemanticF1::new(llm);

        let prompt = evaluator.build_recall_prompt("What is 2+2?", "4", "four");

        assert!(prompt.contains("RECALL"));
        assert!(prompt.contains("Question: What is 2+2?"));
        assert!(prompt.contains("Reference Answer"));
        assert!(prompt.contains("4"));
        assert!(prompt.contains("System Response"));
        assert!(prompt.contains("four"));
    }

    #[test]
    fn test_build_recall_prompt_without_question() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let evaluator = SemanticF1::new(llm);

        let prompt = evaluator.build_recall_prompt("", "expected", "actual");

        assert!(prompt.contains("RECALL"));
        assert!(!prompt.contains("Question:"));
    }

    #[test]
    fn test_build_precision_prompt_with_question() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let evaluator = SemanticF1::new(llm);

        let prompt = evaluator.build_precision_prompt("What is 2+2?", "4", "four");

        assert!(prompt.contains("PRECISION"));
        assert!(prompt.contains("Question: What is 2+2?"));
        assert!(prompt.contains("Reference Answer"));
        assert!(prompt.contains("System Response"));
    }

    #[test]
    fn test_build_precision_prompt_without_question() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let evaluator = SemanticF1::new(llm);

        let prompt = evaluator.build_precision_prompt("", "expected", "actual");

        assert!(prompt.contains("PRECISION"));
        assert!(!prompt.contains("Question:"));
    }

    // ========================================================================
    // SemanticF1Config tests
    // ========================================================================

    #[test]
    fn test_semantic_f1_config_defaults() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let config = SemanticF1Config::new(llm);

        assert!(config.question_field.is_none());
        assert_eq!(config.expected_field, "answer");
        assert_eq!(config.actual_field, "output");
    }

    #[test]
    fn test_semantic_f1_config_builder() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let config = SemanticF1Config::new(llm)
            .with_question_field("question")
            .with_expected_field("target")
            .with_actual_field("prediction");

        assert_eq!(config.question_field, Some("question".to_string()));
        assert_eq!(config.expected_field, "target");
        assert_eq!(config.actual_field, "prediction");
    }

    #[tokio::test]
    async fn test_semantic_f1_config_evaluate() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(0.9, 0.85));
        let config = SemanticF1Config::new(llm)
            .with_expected_field("answer")
            .with_actual_field("output");

        let expected = JsonState::from(serde_json::json!({"answer": "Paris"}));
        let actual = JsonState::from(serde_json::json!({"output": "Paris, France"}));

        let score = config.evaluate(&expected, &actual).await.unwrap();

        // F1 of 0.9 and 0.85 = 2 * 0.9 * 0.85 / (0.9 + 0.85) = 1.53 / 1.75 = 0.874
        assert!(score > 0.8 && score < 0.9);
    }

    #[tokio::test]
    async fn test_semantic_f1_config_evaluate_full() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(0.8, 0.6));
        let config = SemanticF1Config::new(llm);

        let expected = JsonState::from(serde_json::json!({"answer": "Paris"}));
        let actual = JsonState::from(serde_json::json!({"output": "Paris"}));

        let result = config.evaluate_full(&expected, &actual).await.unwrap();

        assert!((result.recall - 0.8).abs() < 0.01);
        assert!((result.precision - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_semantic_f1_config_clone() {
        let llm = std::sync::Arc::new(MockSemanticLLM::new(1.0, 1.0));
        let config = SemanticF1Config::new(llm).with_question_field("q");
        let cloned = config.clone();

        assert_eq!(cloned.question_field, Some("q".to_string()));
    }

    // ========================================================================
    // M-835: Hyphen handling tests
    // ========================================================================

    #[test]
    fn test_normalize_text_hyphens() {
        // Hyphens should be converted to spaces, preserving word boundaries
        // Note: "of" is a preposition (not an article), so it remains
        // Only "a", "an", "the" are removed as articles
        assert_eq!(normalize_text("state-of-the-art"), "state of art");
        assert_eq!(normalize_text("self-aware"), "self aware");
        assert_eq!(
            normalize_text("multi-level-marketing"),
            "multi level marketing"
        );
        // Regular text without hyphens still works
        assert_eq!(normalize_text("hello world"), "hello world");
        // Verify articles in hyphenated words are removed
        assert_eq!(normalize_text("the-quick-brown-fox"), "quick brown fox");
    }

    #[test]
    fn test_hyphenated_words_tokenize_correctly() {
        // "state-of-the-art" becomes 3 tokens (only "the" is removed as article)
        let normalized = normalize_text("state-of-the-art");
        let tokens: Vec<&str> = normalized.split_whitespace().collect();
        assert_eq!(tokens, vec!["state", "of", "art"]);
    }

    #[test]
    fn test_f1_with_hyphenated_words() {
        // "state-of-the-art" vs "state of art" should have perfect overlap
        // after normalization (both become "state of art")
        let score = f1_score("state-of-the-art", "state of art");
        assert!((score - 1.0).abs() < 0.01);
    }

    // ========================================================================
    // M-837: Fraction parsing tests
    // ========================================================================

    #[test]
    fn test_parse_score_fraction() {
        // Simple fractions
        assert!((parse_score_from_response("3/4").unwrap() - 0.75).abs() < 0.01);
        assert!((parse_score_from_response("1/2").unwrap() - 0.5).abs() < 0.01);
        assert!((parse_score_from_response("1/4").unwrap() - 0.25).abs() < 0.01);
        assert!((parse_score_from_response("4/5").unwrap() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_parse_score_fraction_with_text() {
        // Fraction embedded in text
        assert!((parse_score_from_response("The score is 3/4").unwrap() - 0.75).abs() < 0.01);
        assert!((parse_score_from_response("Score: 1/2 points").unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_score_fraction_clamps() {
        // Fractions > 1 should be clamped
        assert!((parse_score_from_response("5/4").unwrap() - 1.0).abs() < 0.01);
        assert!((parse_score_from_response("3/2").unwrap() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_score_fraction_zero_denominator() {
        // Zero denominator should be handled (falls through to decimal parsing or default)
        let result = parse_score_from_response("3/0");
        assert!(result.is_ok()); // Should not panic
    }

    // ========================================================================
    // M-838: Missing vs empty field documentation tests
    // ========================================================================

    #[test]
    fn test_json_metrics_missing_vs_empty_field() {
        // Missing field and empty field should behave the same way
        let missing_field = JsonState::from(serde_json::json!({}));
        let empty_field = JsonState::from(serde_json::json!({"answer": ""}));
        let actual = JsonState::from(serde_json::json!({"output": ""}));

        // Both should return 1.0 (empty matches empty)
        let score_missing = json_exact_match(&missing_field, &actual, "answer", "output");
        let score_empty = json_exact_match(&empty_field, &actual, "answer", "output");
        assert_eq!(score_missing, score_empty);
        assert_eq!(score_missing, 1.0); // Empty strings match after normalization
    }

    #[test]
    fn test_json_metrics_both_fields_missing() {
        // Two missing fields should match (both become empty strings)
        let state1 = JsonState::from(serde_json::json!({"other": "data"}));
        let state2 = JsonState::from(serde_json::json!({"different": "data"}));

        let score = json_exact_match(&state1, &state2, "answer", "output");
        assert_eq!(score, 1.0); // Both are empty, so they match
    }
}
