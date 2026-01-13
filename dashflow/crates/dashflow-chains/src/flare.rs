//! # FLARE Chain - Forward-Looking Active Retrieval
//!
//! Implements Active Retrieval Augmented Generation as described in:
//! [Active Retrieval Augmented Generation](https://arxiv.org/abs/2305.06983)
//!
//! FLARE iteratively generates responses and uses token log probabilities to detect
//! uncertain spans. When uncertainty is detected, it generates questions for those
//! spans, retrieves context, and continues generation.
//!
//! **Adapted from**: <https://github.com/jzbjyb/FLARE>
//!
//! ## Requirements
//!
//! FLARE requires a chat model that provides token-level log probabilities (logprobs).
//! Currently, only `OpenAI`'s `ChatGPT` models support this via the `logprobs=true` parameter.
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_chains::FlareChain;
//! use dashflow::core::retrievers::Retriever;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a chat model with logprobs enabled
//!     // let chat_model = ChatOpenAI::with_config(Default::default())
//!     //     .with_model("gpt-3.5-turbo")
//!     //     .with_logprobs(true)
//!     //     .with_max_tokens(32);
//!
//!     // Create a retriever
//!     // let retriever: Arc<dyn BaseRetriever> = ...;
//!
//!     // Create FLARE chain
//!     // let chain = FlareChain::from_llm(chat_model, retriever)
//!     //     .with_min_prob(0.2)
//!     //     .with_max_iter(10);
//!
//!     // Use the chain
//!     // let mut inputs = HashMap::new();
//!     // inputs.insert("user_input".to_string(), "What is quantum computing?".to_string());
//!     // let result = chain.call(&inputs).await.unwrap();
//! }
//! ```

use dashflow::core::error::{Error, Result};
use dashflow::core::messages::AIMessage;
use dashflow::core::prompts::PromptTemplate;
use dashflow::core::retrievers::Retriever;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Static regex for matching word characters (compiled once).
/// This pattern is used to filter out punctuation/whitespace tokens.
static WORD_CHAR_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_word_char_regex() -> &'static Regex {
    WORD_CHAR_REGEX.get_or_init(|| {
        #[allow(clippy::expect_used)]
        Regex::new(r"\w").expect("WORD_CHAR_REGEX pattern is valid")
    })
}

/// Output parser that checks if the output contains a "FINISHED" marker.
///
/// When FLARE completes generation, it returns "FINISHED" to signal completion.
#[derive(Debug, Clone)]
pub struct FinishedOutputParser {
    /// Value that indicates the output is finished (default: "FINISHED")
    pub finished_value: String,
}

impl Default for FinishedOutputParser {
    fn default() -> Self {
        Self {
            finished_value: "FINISHED".to_string(),
        }
    }
}

impl FinishedOutputParser {
    /// Create a new `FinishedOutputParser` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a parser with a custom finished value.
    pub fn with_finished_value(mut self, value: impl Into<String>) -> Self {
        self.finished_value = value.into();
        self
    }

    /// Parse the text and return (`cleaned_text`, `is_finished`).
    ///
    /// The finished value is removed from the text.
    #[must_use]
    pub fn parse(&self, text: &str) -> (String, bool) {
        let cleaned = text.trim();
        let finished = cleaned.contains(&self.finished_value);
        let result = cleaned.replace(&self.finished_value, "").trim().to_string();
        (result, finished)
    }
}

/// Prompt template for FLARE response generation.
///
/// This prompt instructs the LLM to respond using context and return "FINISHED" when done.
pub const FLARE_PROMPT_TEMPLATE: &str = r"Respond to the user message using any relevant context. If context is provided, you should ground your answer in that context. Once you're done responding return FINISHED.

>>> CONTEXT: {context}
>>> USER INPUT: {user_input}
>>> RESPONSE: {response}";

/// Default FLARE prompt for response generation.
pub fn flare_prompt() -> Result<PromptTemplate> {
    PromptTemplate::from_template(FLARE_PROMPT_TEMPLATE)
}

/// Prompt template for generating questions from uncertain spans.
///
/// This prompt instructs the LLM to generate a question that would be answered
/// by the uncertain term/entity/phrase.
pub const QUESTION_GENERATOR_PROMPT_TEMPLATE: &str = r#"Given a user input and an existing partial response as context, ask a question to which the answer is the given term/entity/phrase:

>>> USER INPUT: {user_input}
>>> EXISTING PARTIAL RESPONSE: {current_response}

The question to which the answer is the term/entity/phrase "{uncertain_span}" is:"#;

/// Default question generator prompt.
pub fn question_generator_prompt() -> Result<PromptTemplate> {
    PromptTemplate::from_template(QUESTION_GENERATOR_PROMPT_TEMPLATE)
}

/// Token with its log probability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLogProb {
    /// The token text
    pub token: String,
    /// The log probability of this token
    pub logprob: f64,
}

/// Extract tokens and log probabilities from an `AIMessage` response.
///
/// Expects `response_metadata` to contain logprobs in `OpenAI` format:
/// ```json
/// {
///   "logprobs": {
///     "content": [
///       {"token": "Hello", "logprob": -0.5},
///       {"token": " world", "logprob": -1.2}
///     ]
///   }
/// }
/// ```
pub fn extract_tokens_and_log_probs(response: &AIMessage) -> Result<(Vec<String>, Vec<f64>)> {
    // Get logprobs from response_metadata
    let logprobs = response
        .response_metadata()
        .get("logprobs")
        .ok_or_else(|| Error::Other("No logprobs in response metadata".to_string()))?;

    // Get content array
    let content = logprobs
        .get("content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::Other("No content array in logprobs".to_string()))?;

    let mut tokens = Vec::new();
    let mut log_probs = Vec::new();

    for item in content {
        let token = item
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Other("Missing token in logprob content".to_string()))?;

        let logprob = item
            .get("logprob")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| Error::Other("Missing logprob in logprob content".to_string()))?;

        tokens.push(token.to_string());
        log_probs.push(logprob);
    }

    Ok((tokens, log_probs))
}

/// Find spans of text with low confidence (low probability tokens).
///
/// # Arguments
///
/// * `tokens` - List of tokens
/// * `log_probs` - Log probabilities for each token
/// * `min_prob` - Minimum probability threshold (tokens below this are "low confidence")
/// * `min_token_gap` - Minimum gap between spans (merge spans closer than this)
/// * `num_pad_tokens` - Number of tokens to include after each low-confidence token
///
/// # Returns
///
/// List of low-confidence text spans
#[must_use]
pub fn low_confidence_spans(
    tokens: &[String],
    log_probs: &[f64],
    min_prob: f64,
    min_token_gap: usize,
    num_pad_tokens: usize,
) -> Vec<String> {
    // Find indices of low-confidence tokens (exp(log_prob) < min_prob)
    // Filter out tokens without word characters (punctuation, whitespace)
    let word_char_regex = get_word_char_regex();
    let low_idx: Vec<usize> = log_probs
        .iter()
        .enumerate()
        .filter(|(i, &logprob)| logprob.exp() < min_prob && word_char_regex.is_match(&tokens[*i]))
        .map(|(i, _)| i)
        .collect();

    if low_idx.is_empty() {
        return Vec::new();
    }

    // Create spans with padding
    // Start with first low-confidence token
    let mut spans: Vec<(usize, usize)> = vec![(low_idx[0], low_idx[0] + num_pad_tokens + 1)];

    // Merge nearby spans
    for i in 1..low_idx.len() {
        let idx = low_idx[i];
        let end = idx + num_pad_tokens + 1;
        let prev_idx = low_idx[i - 1];

        // If tokens are close together, extend the previous span
        if idx - prev_idx < min_token_gap {
            // SAFETY: M-347 - spans initialized with one element at line ~231, never emptied
            #[allow(clippy::expect_used)]
            {
                spans.last_mut().expect("spans has at least one element").1 = end;
            }
        } else {
            // Otherwise create a new span
            spans.push((idx, end));
        }
    }

    // Extract text for each span
    spans
        .into_iter()
        .map(|(start, end)| {
            let end = end.min(tokens.len());
            tokens[start..end].join("")
        })
        .collect()
}

/// FLARE Chain - Forward-Looking Active Retrieval augmented generation.
///
/// This chain:
/// 1. Generates a response token by token
/// 2. Identifies low-confidence spans using token log probabilities
/// 3. Generates questions for uncertain spans
/// 4. Retrieves relevant context
/// 5. Continues generation with new context
/// 6. Repeats until finished or max iterations reached
///
/// # Requirements
///
/// - Chat model with logprobs support (`OpenAI` `ChatGPT` models)
/// - Retriever for context retrieval
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_chains::FlareChain;
/// # async fn example() {
/// // let chat_model = ChatOpenAI::with_config(Default::default()).with_logprobs(true);
/// // let retriever = ...;
/// // let chain = FlareChain::from_llm(chat_model, retriever);
/// // let result = chain.call(&inputs).await.unwrap();
/// # }
/// ```
pub struct FlareChain<Q, R>
where
    Q: Send + Sync,
    R: Send + Sync,
{
    /// Chain that generates questions from uncertain spans
    pub question_generator_chain: Arc<Q>,

    /// Chain that generates responses from user input and context
    pub response_chain: Arc<R>,

    /// Parser that determines whether the chain is finished
    pub output_parser: FinishedOutputParser,

    /// Retriever for fetching relevant documents
    pub retriever: Arc<dyn Retriever>,

    /// Minimum probability for a token to be considered low confidence (default: 0.2)
    pub min_prob: f64,

    /// Minimum number of tokens between two low confidence spans (default: 5)
    pub min_token_gap: usize,

    /// Number of tokens to pad around a low confidence span (default: 2)
    pub num_pad_tokens: usize,

    /// Maximum number of iterations (default: 10)
    pub max_iter: usize,

    /// Whether to start with retrieval before first generation (default: true)
    pub start_with_retrieval: bool,
}

impl<Q, R> FlareChain<Q, R>
where
    Q: Send + Sync,
    R: Send + Sync,
{
    /// Create a new `FlareChain`.
    ///
    /// # Arguments
    ///
    /// * `question_generator_chain` - Chain for generating questions from uncertain spans
    /// * `response_chain` - Chain for generating responses
    /// * `retriever` - Retriever for fetching context
    pub fn new(
        question_generator_chain: Q,
        response_chain: R,
        retriever: Arc<dyn Retriever>,
    ) -> Self {
        Self {
            question_generator_chain: Arc::new(question_generator_chain),
            response_chain: Arc::new(response_chain),
            output_parser: FinishedOutputParser::default(),
            retriever,
            min_prob: 0.2,
            min_token_gap: 5,
            num_pad_tokens: 2,
            max_iter: 10,
            start_with_retrieval: true,
        }
    }

    /// Set the minimum probability threshold for low-confidence detection.
    #[must_use]
    pub fn with_min_prob(mut self, min_prob: f64) -> Self {
        self.min_prob = min_prob;
        self
    }

    /// Set the minimum token gap for merging low-confidence spans.
    #[must_use]
    pub fn with_min_token_gap(mut self, min_token_gap: usize) -> Self {
        self.min_token_gap = min_token_gap;
        self
    }

    /// Set the number of padding tokens around low-confidence spans.
    #[must_use]
    pub fn with_num_pad_tokens(mut self, num_pad_tokens: usize) -> Self {
        self.num_pad_tokens = num_pad_tokens;
        self
    }

    /// Set the maximum number of iterations.
    #[must_use]
    pub fn with_max_iter(mut self, max_iter: usize) -> Self {
        self.max_iter = max_iter;
        self
    }

    /// Set whether to start with retrieval.
    #[must_use]
    pub fn with_start_with_retrieval(mut self, start: bool) -> Self {
        self.start_with_retrieval = start;
        self
    }

    /// Set a custom output parser.
    #[must_use]
    pub fn with_output_parser(mut self, parser: FinishedOutputParser) -> Self {
        self.output_parser = parser;
        self
    }

    /// Get input keys for this chain.
    #[must_use]
    pub fn get_input_keys(&self) -> Vec<String> {
        vec!["user_input".to_string()]
    }

    /// Get output keys for this chain.
    #[must_use]
    pub fn get_output_keys(&self) -> Vec<String> {
        vec!["response".to_string()]
    }

    /// Get the chain type identifier.
    #[must_use]
    pub fn chain_type(&self) -> &'static str {
        "flare_chain"
    }
}

// Trait for chains that can generate text responses
// This allows FlareChain to work with different chain implementations
pub trait ResponseGenerator: Send + Sync {
    /// Generate a response with the given inputs
    fn generate(
        &self,
        inputs: &HashMap<String, String>,
    ) -> impl std::future::Future<Output = Result<AIMessage>> + Send;
}

pub trait QuestionGenerator: Send + Sync {
    /// Generate questions for the given inputs (multiple questions in batch)
    fn batch_generate(
        &self,
        inputs: &[HashMap<String, String>],
    ) -> impl std::future::Future<Output = Result<Vec<String>>> + Send;
}

impl<Q, R> FlareChain<Q, R>
where
    Q: QuestionGenerator + Send + Sync,
    R: ResponseGenerator + Send + Sync,
{
    /// Run the FLARE chain with the given inputs.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input variables containing "`user_input`"
    ///
    /// # Returns
    ///
    /// A `HashMap` containing the generated response under "response" key
    pub async fn call(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        let user_input = inputs
            .get("user_input")
            .ok_or_else(|| Error::InvalidInput("Missing 'user_input' key".to_string()))?;

        let mut response = String::new();

        for _i in 0..self.max_iter {
            // Generate next part of response
            let mut gen_inputs = HashMap::new();
            gen_inputs.insert("user_input".to_string(), user_input.clone());
            gen_inputs.insert("context".to_string(), String::new());
            gen_inputs.insert("response".to_string(), response.clone());

            let ai_message = self.response_chain.generate(&gen_inputs).await?;

            // Extract tokens and log probs
            let (tokens, log_probs) = extract_tokens_and_log_probs(&ai_message)?;

            // Find low-confidence spans
            let spans = low_confidence_spans(
                &tokens,
                &log_probs,
                self.min_prob,
                self.min_token_gap,
                self.num_pad_tokens,
            );

            let initial_response = format!("{} {}", response.trim(), tokens.join(""));

            // If no low-confidence spans, check if finished
            if spans.is_empty() {
                response = initial_response;
                let (final_response, finished) = self.output_parser.parse(&response);
                if finished {
                    let mut output = HashMap::new();
                    output.insert("response".to_string(), final_response);
                    return Ok(output);
                }
                continue;
            }

            // Generate questions for low-confidence spans
            let question_inputs: Vec<HashMap<String, String>> = spans
                .iter()
                .map(|span| {
                    let mut inputs = HashMap::new();
                    inputs.insert("user_input".to_string(), user_input.clone());
                    inputs.insert("current_response".to_string(), initial_response.clone());
                    inputs.insert("uncertain_span".to_string(), span.clone());
                    inputs
                })
                .collect();

            let questions = self
                .question_generator_chain
                .batch_generate(&question_inputs)
                .await?;

            // Retrieve context for questions
            let mut docs = Vec::new();
            for question in &questions {
                let retrieved = self
                    .retriever
                    ._get_relevant_documents(question, None)
                    .await?;
                docs.extend(retrieved);
            }

            let context = docs
                .iter()
                .map(|d| d.page_content.clone())
                .collect::<Vec<_>>()
                .join("\n\n");

            // Generate with new context
            let mut gen_inputs = HashMap::new();
            gen_inputs.insert("user_input".to_string(), user_input.clone());
            gen_inputs.insert("context".to_string(), context);
            gen_inputs.insert("response".to_string(), response.clone());

            let marginal_message = self.response_chain.generate(&gen_inputs).await?;
            let marginal_text = marginal_message.content().to_string();
            let (marginal, finished) = self.output_parser.parse(&marginal_text);

            response = format!("{} {}", response.trim(), marginal);

            if finished {
                break;
            }
        }

        let mut output = HashMap::new();
        output.insert("response".to_string(), response);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finished_output_parser_basic() {
        let parser = FinishedOutputParser::default();
        let (text, finished) = parser.parse("This is a response FINISHED");
        assert_eq!(text, "This is a response");
        assert!(finished);
    }

    #[test]
    fn test_finished_output_parser_not_finished() {
        let parser = FinishedOutputParser::default();
        let (text, finished) = parser.parse("This is a partial response");
        assert_eq!(text, "This is a partial response");
        assert!(!finished);
    }

    #[test]
    fn test_finished_output_parser_custom_value() {
        let parser = FinishedOutputParser::new().with_finished_value("DONE");
        let (text, finished) = parser.parse("Response complete DONE");
        assert_eq!(text, "Response complete");
        assert!(finished);
    }

    #[test]
    fn test_low_confidence_spans_empty() {
        let tokens = vec!["Hello".to_string(), "world".to_string()];
        let log_probs = vec![-0.1, -0.2]; // High confidence (exp(-0.1) ≈ 0.9, exp(-0.2) ≈ 0.82)
        let spans = low_confidence_spans(&tokens, &log_probs, 0.2, 5, 2);
        assert!(spans.is_empty());
    }

    #[test]
    fn test_low_confidence_spans_single() {
        let tokens = vec![
            "The".to_string(),
            "answer".to_string(),
            "is".to_string(),
            "uncertain".to_string(),
            "term".to_string(),
        ];
        // exp(-2.0) ≈ 0.135 < 0.2 (low confidence)
        let log_probs = vec![-0.1, -0.1, -0.1, -2.0, -0.1];
        let spans = low_confidence_spans(&tokens, &log_probs, 0.2, 5, 2);
        assert_eq!(spans.len(), 1);
        // Span should include token at index 3 plus 2 padding tokens (indices 3,4,5 but 5 is out of bounds)
        assert!(spans[0].contains("uncertain"));
        assert!(spans[0].contains("term"));
    }

    #[test]
    fn test_low_confidence_spans_merged() {
        let tokens = vec![
            "The".to_string(),
            "first".to_string(),
            "term".to_string(),
            "and".to_string(),
            "second".to_string(),
            "term".to_string(),
        ];
        // Two low-confidence tokens close together (indices 2 and 4)
        let log_probs = vec![-0.1, -0.1, -2.0, -0.1, -2.0, -0.1];
        // min_token_gap=5 means tokens within 5 positions get merged
        let spans = low_confidence_spans(&tokens, &log_probs, 0.2, 5, 2);
        // Should merge into one span since 4-2 = 2 < 5
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn test_flare_prompts() {
        let flare = flare_prompt().unwrap();
        assert!(flare.template.contains("CONTEXT"));
        assert!(flare.template.contains("USER INPUT"));
        assert!(flare.template.contains("FINISHED"));

        let question_gen = question_generator_prompt().unwrap();
        assert!(question_gen.template.contains("uncertain_span"));
        assert!(question_gen.template.contains("EXISTING PARTIAL RESPONSE"));
    }

    #[test]
    fn test_extract_tokens_and_log_probs() {
        // Create a mock AIMessage with logprobs
        let mut response_metadata = HashMap::new();
        let logprobs_json = serde_json::json!({
            "content": [
                {"token": "Hello", "logprob": -0.5},
                {"token": " world", "logprob": -1.2}
            ]
        });
        response_metadata.insert("logprobs".to_string(), logprobs_json);

        let ai_message = AIMessage::new("Hello world").with_response_metadata(response_metadata);

        let (tokens, log_probs) = extract_tokens_and_log_probs(&ai_message).unwrap();
        assert_eq!(tokens, vec!["Hello", " world"]);
        assert_eq!(log_probs, vec![-0.5, -1.2]);
    }

    #[test]
    fn test_extract_tokens_missing_logprobs() {
        let ai_message = AIMessage::new("Hello world");
        let result = extract_tokens_and_log_probs(&ai_message);
        assert!(result.is_err());
    }
}
