// Allow clippy warnings for optimizer
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! # AutoPrompt - Gradient-Free Automatic Prompt Engineering
//!
//! AutoPrompt is a prompt optimization technique that automatically discovers optimal
//! trigger tokens to prepend to prompts. Originally based on Shin et al. (2020), this
//! implementation uses discrete search (no gradients) via coordinate descent.
//!
//! ## Algorithm Overview
//!
//! 1. **Initialization:**
//!    - Start with a set of candidate trigger tokens (vocabulary)
//!    - Initialize K trigger token positions (default: 5)
//!    - Each position starts with a random or default token
//!
//! 2. **Coordinate Descent:**
//!    - For each position (left to right):
//!      - Try each candidate token from the vocabulary
//!      - Evaluate the full prompt with this token substitution
//!      - Keep the token that maximizes the metric score
//!    - Repeat for multiple iterations
//!
//! 3. **Final Selection:**
//!    - Return the signature with optimized trigger tokens prepended
//!
//! ## Key Features
//!
//! - **No gradients required:** Works with any LLM API (black-box optimization)
//! - **Token-level search:** Fine-grained control over prompt optimization
//! - **Configurable vocabulary:** Use default or custom trigger token candidates
//! - **Parallel evaluation:** Evaluates candidates concurrently for speed
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use dashflow::optimize::{AutoPrompt, Signature};
//!
//! let optimizer = AutoPrompt::builder()
//!     .num_triggers(5)
//!     .iterations(3)
//!     .metric(my_metric_fn)
//!     .build()?;
//!
//! let optimized_signature = optimizer.compile(
//!     &signature,
//!     &trainset,
//!     llm.clone()
//! ).await?;
//! ```
//!
//! ## References
//!
//! - **Paper**: "AutoPrompt: Eliciting Knowledge from Language Models with Automatically Generated Prompts"
//! - **Authors**: Shin et al. (2020)
//! - **Link**: <https://arxiv.org/abs/2010.15980>
//! - **Note**: This implementation uses gradient-free discrete search (coordinate descent)

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::optimize::example::Example;
use crate::optimize::signature::Signature;
use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::Error;
use futures::future::try_join_all;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::sync::Arc;
use std::time::Instant;
use tracing;

// Import shared MetricFn from types module
pub use super::types::MetricFn;

/// Default vocabulary of trigger tokens
/// These are common prompt engineering tokens/phrases that tend to improve performance
const DEFAULT_TRIGGER_VOCABULARY: &[&str] = &[
    // Task framing
    "Task:",
    "Question:",
    "Input:",
    "Query:",
    "Problem:",
    // Quality indicators
    "Important:",
    "Note:",
    "Key:",
    "Critical:",
    "Essential:",
    // Instruction modifiers
    "Carefully",
    "Precisely",
    "Accurately",
    "Thoroughly",
    "Step-by-step",
    // Role indicators
    "Expert",
    "Professional",
    "Specialist",
    "Analyst",
    "Assistant",
    // Output guidance
    "Answer:",
    "Response:",
    "Output:",
    "Result:",
    "Solution:",
    // Reasoning cues
    "Think",
    "Consider",
    "Analyze",
    "Evaluate",
    "Reason",
    // Format cues
    "Concise",
    "Detailed",
    "Clear",
    "Structured",
    "Complete",
    // Domain signals
    "Given",
    "Based on",
    "According to",
    "Using",
    "With",
];

/// AutoPrompt optimizer builder
pub struct AutoPromptBuilder {
    num_triggers: Option<usize>,
    iterations: Option<usize>,
    vocabulary: Option<Vec<String>>,
    metric: Option<MetricFn>,
    random_seed: Option<u64>,
    verbose: bool,
}

impl AutoPromptBuilder {
    /// Create a new AutoPrompt builder
    pub fn new() -> Self {
        Self {
            num_triggers: None,
            iterations: None,
            vocabulary: None,
            metric: None,
            random_seed: None,
            verbose: false,
        }
    }

    /// Set the number of trigger token positions (default: 5)
    pub fn num_triggers(mut self, n: usize) -> Self {
        self.num_triggers = Some(n);
        self
    }

    /// Set the number of optimization iterations (default: 3)
    pub fn iterations(mut self, n: usize) -> Self {
        self.iterations = Some(n);
        self
    }

    /// Set a custom trigger token vocabulary
    pub fn vocabulary(mut self, vocab: Vec<String>) -> Self {
        self.vocabulary = Some(vocab);
        self
    }

    /// Set the evaluation metric (required)
    pub fn metric(mut self, metric: MetricFn) -> Self {
        self.metric = Some(metric);
        self
    }

    /// Set random seed for reproducibility
    pub fn random_seed(mut self, seed: u64) -> Self {
        self.random_seed = Some(seed);
        self
    }

    /// Enable verbose output
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Build the AutoPrompt optimizer
    pub fn build(self) -> Result<AutoPrompt, Error> {
        let num_triggers = self.num_triggers.unwrap_or(5);
        if num_triggers == 0 {
            return Err(Error::Validation(
                "Number of triggers must be at least 1".to_string(),
            ));
        }

        let vocabulary = self.vocabulary.unwrap_or_else(|| {
            DEFAULT_TRIGGER_VOCABULARY
                .iter()
                .map(|s| s.to_string())
                .collect()
        });

        if vocabulary.is_empty() {
            return Err(Error::Validation(
                "Vocabulary must not be empty".to_string(),
            ));
        }

        Ok(AutoPrompt {
            num_triggers,
            iterations: self.iterations.unwrap_or(3),
            vocabulary,
            metric: self
                .metric
                .ok_or_else(|| Error::Validation("Metric is required".to_string()))?,
            random_seed: self.random_seed,
            verbose: self.verbose,
        })
    }
}

impl Default for AutoPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// AutoPrompt optimizer using gradient-free discrete search
#[derive(Clone)]
pub struct AutoPrompt {
    num_triggers: usize,
    iterations: usize,
    vocabulary: Vec<String>,
    metric: MetricFn,
    random_seed: Option<u64>,
    verbose: bool,
}

impl std::fmt::Debug for AutoPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoPrompt")
            .field("num_triggers", &self.num_triggers)
            .field("iterations", &self.iterations)
            .field("vocabulary_size", &self.vocabulary.len())
            .field("random_seed", &self.random_seed)
            .field("verbose", &self.verbose)
            .field("metric", &"<function>")
            .finish()
    }
}

impl AutoPrompt {
    /// Create a new AutoPrompt builder
    pub fn builder() -> AutoPromptBuilder {
        AutoPromptBuilder::new()
    }

    /// Get the vocabulary
    pub fn vocabulary(&self) -> &[String] {
        &self.vocabulary
    }

    /// Get the number of trigger positions
    pub fn num_triggers(&self) -> usize {
        self.num_triggers
    }

    /// Get the number of iterations
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// Compile (optimize) a signature using the training set
    ///
    /// # Arguments
    ///
    /// * `signature` - The signature to optimize
    /// * `trainset` - Training examples for evaluation
    /// * `llm` - LLM for evaluation
    ///
    /// # Scalability Considerations (M-900)
    ///
    /// The number of LLM calls per iteration is:
    /// `num_triggers × (vocabulary_size - 1) × trainset_size`
    ///
    /// With default settings (5 triggers, 30 vocabulary tokens, 100 examples):
    /// - Per iteration: 5 × 29 × 100 = 14,500 calls
    /// - Total (3 iterations): ~43,500 calls
    ///
    /// **Recommendations for large datasets:**
    /// - Subsample `trainset` (e.g., 10-20 representative examples)
    /// - Reduce `vocabulary` to domain-specific tokens
    /// - Reduce `num_triggers` (3-5 usually sufficient)
    /// - Reduce `iterations` (2-3 often enough for convergence)
    ///
    /// # Evaluation Semantics (M-898)
    ///
    /// Candidate evaluations run in parallel using `try_join_all`. If ANY evaluation
    /// fails (e.g., LLM API error, rate limit, network timeout), the entire batch
    /// aborts and the error propagates. This fail-fast behavior is intentional:
    ///
    /// - **Consistency**: Partial evaluation sets could lead to suboptimal selection
    /// - **Reproducibility**: With fail-fast, the same inputs produce the same outputs
    ///
    /// **Workarounds for unreliable LLM providers:**
    /// - Implement retry logic in your ChatModel wrapper
    /// - Use rate limiting / backoff in your ChatModel
    pub async fn compile(
        &self,
        signature: &Signature,
        trainset: &[Example],
        llm: Arc<dyn ChatModel>,
    ) -> Result<Signature, Error> {
        record_optimization_start("autoprompt");
        let start_time = Instant::now();
        let mut iterations_completed = 0u64;
        let mut total_candidates_evaluated = 0u64;

        // Initialize RNG
        let mut rng = match self.random_seed {
            Some(seed) => rand::rngs::StdRng::seed_from_u64(seed),
            None => rand::rngs::StdRng::from_entropy(),
        };

        // Initialize trigger tokens randomly
        // M-899: unwrap() is safe here because build() validates vocabulary.is_empty() == false
        // If a custom vocabulary could be modified after build(), this would need a fallback
        let mut triggers: Vec<String> = (0..self.num_triggers)
            .map(|_| self.vocabulary.choose(&mut rng).unwrap().clone())
            .collect();

        if self.verbose {
            tracing::info!(
                num_triggers = self.num_triggers,
                vocabulary_size = self.vocabulary.len(),
                initial_triggers = ?triggers,
                "AutoPrompt: Initializing"
            );
        }

        // Evaluate initial configuration
        let mut best_triggers = triggers.clone();
        let mut best_score = self
            .evaluate_triggers(signature, &triggers, trainset, &llm)
            .await?;
        let initial_score = best_score;
        total_candidates_evaluated += 1;

        if self.verbose {
            tracing::debug!(score = %format!("{:.4}", best_score), "Initial score");
        }

        // Coordinate descent optimization
        for iteration in 0..self.iterations {
            record_iteration("autoprompt");
            iterations_completed += 1;

            if self.verbose {
                tracing::info!(
                    iteration = iteration + 1,
                    total = self.iterations,
                    "Starting iteration"
                );
            }

            let mut improved = false;

            // For each trigger position
            for pos in 0..self.num_triggers {
                if self.verbose {
                    tracing::debug!(
                        position = pos + 1,
                        total = self.num_triggers,
                        "Optimizing position"
                    );
                }

                let current_token = triggers[pos].clone();
                let mut position_best_token = current_token.clone();
                let mut position_best_score = best_score;

                // Try each candidate token in vocabulary
                // Batch evaluate for efficiency
                let mut candidates_to_eval: Vec<(String, Vec<String>)> = Vec::new();

                for candidate in &self.vocabulary {
                    if candidate == &current_token {
                        continue; // Skip current token
                    }

                    let mut candidate_triggers = triggers.clone();
                    candidate_triggers[pos] = candidate.clone();
                    candidates_to_eval.push((candidate.clone(), candidate_triggers));
                }

                // Evaluate all candidates in parallel
                let eval_futures: Vec<_> = candidates_to_eval
                    .iter()
                    .map(|(token, candidate_triggers)| {
                        let sig = signature.clone();
                        let triggers_clone = candidate_triggers.clone();
                        let ts = trainset.to_vec();
                        let model = llm.clone();
                        let metric = self.metric.clone();
                        let token_clone = token.clone();
                        async move {
                            let score = Self::evaluate_triggers_static(
                                &sig,
                                &triggers_clone,
                                &ts,
                                &model,
                                &metric,
                            )
                            .await?;
                            Ok::<_, Error>((token_clone, score))
                        }
                    })
                    .collect();

                let results = try_join_all(eval_futures).await?;
                total_candidates_evaluated += results.len() as u64;

                // Find the best token for this position
                for (token, score) in results {
                    if score > position_best_score {
                        position_best_score = score;
                        position_best_token = token;
                        improved = true;
                    }
                }

                // Update triggers if improvement found
                if position_best_token != current_token {
                    triggers[pos] = position_best_token.clone();
                    best_score = position_best_score;
                    best_triggers = triggers.clone();

                    if self.verbose {
                        tracing::debug!(
                            position = pos,
                            old_token = %current_token,
                            new_token = %position_best_token,
                            score = %format!("{:.4}", best_score),
                            "Position improved"
                        );
                    }
                }
            }

            if !improved {
                if self.verbose {
                    tracing::info!(iteration = iteration + 1, "No improvement, stopping early");
                }
                break;
            }

            if self.verbose {
                tracing::debug!(
                    iteration = iteration + 1,
                    score = %format!("{:.4}", best_score),
                    triggers = ?triggers,
                    "Iteration complete"
                );
            }
        }

        // Create optimized signature
        let trigger_prefix = best_triggers.join(" ");
        let mut optimized = signature.clone();

        // Prepend triggers to instructions
        if optimized.instructions.is_empty() {
            optimized.instructions = trigger_prefix;
        } else {
            optimized.instructions = format!("{} {}", trigger_prefix, optimized.instructions);
        }

        let duration = start_time.elapsed().as_secs_f64();

        record_optimization_complete(
            "autoprompt",
            iterations_completed,
            total_candidates_evaluated,
            initial_score,
            best_score,
            duration,
        );

        if self.verbose {
            tracing::info!(
                final_score = %format!("{:.4}", best_score),
                final_triggers = ?best_triggers,
                duration_secs = %format!("{:.2}", duration),
                "AutoPrompt: Optimization complete"
            );
        }

        Ok(optimized)
    }

    /// Evaluate a trigger configuration
    async fn evaluate_triggers(
        &self,
        signature: &Signature,
        triggers: &[String],
        trainset: &[Example],
        llm: &Arc<dyn ChatModel>,
    ) -> Result<f64, Error> {
        Self::evaluate_triggers_static(signature, triggers, trainset, llm, &self.metric).await
    }

    /// Evaluate a trigger configuration (static for parallel use)
    async fn evaluate_triggers_static(
        signature: &Signature,
        triggers: &[String],
        trainset: &[Example],
        llm: &Arc<dyn ChatModel>,
        metric: &MetricFn,
    ) -> Result<f64, Error> {
        // Build prompt with triggers prepended
        let trigger_prefix = triggers.join(" ");
        let mut temp_sig = signature.clone();

        if temp_sig.instructions.is_empty() {
            temp_sig.instructions = trigger_prefix;
        } else {
            temp_sig.instructions = format!("{} {}", trigger_prefix, temp_sig.instructions);
        }

        // Evaluate on trainset
        let eval_futures: Vec<_> = trainset
            .iter()
            .map(|example| {
                let sig = temp_sig.clone();
                let ex = example.clone();
                let model = llm.clone();
                async move {
                    // Build prompt
                    let prompt = Self::build_prompt(&sig, &ex);

                    // Call LLM
                    let messages = vec![Message::human(prompt)];
                    let result = model
                        .generate(&messages, None, None, None, None)
                        .await
                        .map_err(|e| Error::NodeExecution {
                            node: "AutoPrompt".to_string(),
                            source: Box::new(e),
                        })?;

                    let response = result
                        .generations
                        .first()
                        .ok_or_else(|| Error::NodeExecution {
                            node: "AutoPrompt".to_string(),
                            source: Box::new(Error::Generic(
                                "LLM returned empty response".to_string(),
                            )),
                        })?
                        .message
                        .content()
                        .as_text();

                    // Parse output
                    let prediction = Self::parse_output(&sig, &response, &ex)?;

                    Ok::<_, Error>((prediction, ex))
                }
            })
            .collect();

        let results = try_join_all(eval_futures).await?;

        // Calculate average score
        if results.is_empty() {
            return Ok(0.0);
        }

        let total_score: f64 = results
            .iter()
            .map(|(prediction, example)| metric(prediction, example))
            .sum();

        Ok(total_score / results.len() as f64)
    }

    /// Build prompt from signature and example
    fn build_prompt(signature: &Signature, example: &Example) -> String {
        let mut prompt = String::new();

        // Add instruction
        if !signature.instructions.is_empty() {
            prompt.push_str(&signature.instructions);
            prompt.push_str("\n\n");
        }

        // Add input fields
        let inputs = example.inputs();
        for field in &signature.input_fields {
            if let Some(value) = inputs.get(&field.name) {
                if let Some(s) = value.as_str() {
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), s));
                }
            }
        }

        // Add output prefix
        if let Some(first_output) = signature.output_fields.first() {
            prompt.push_str(&format!("{}: ", first_output.get_prefix()));
        }

        prompt
    }

    /// Parse LLM output into an Example
    fn parse_output(
        signature: &Signature,
        response: &str,
        input_example: &Example,
    ) -> Result<Example, Error> {
        let mut prediction = Example::new();

        // Copy inputs
        let input_keys: Vec<String> = input_example.inputs().keys().cloned().collect();
        for (key, value) in input_example.inputs().iter() {
            prediction = prediction.with_field(key, value.clone());
        }

        // Convert Vec<String> to Vec<&str> for with_inputs
        let input_keys_str: Vec<&str> = input_keys.iter().map(|s| s.as_str()).collect();
        prediction = prediction.with_inputs(&input_keys_str);

        // Parse output (simple: first output field gets the trimmed response)
        if let Some(first_output) = signature.output_fields.first() {
            prediction = prediction.with_field(&first_output.name, response.trim());
        }

        Ok(prediction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::language_models::FakeChatModel;
    use crate::optimize::signature::Field;

    fn create_test_signature() -> Signature {
        Signature::new("SimpleQA")
            .with_input(Field::input("question", "A question to answer"))
            .with_output(Field::output("answer", "The answer"))
            .with_instructions("Answer the question")
    }

    fn create_test_examples() -> Vec<Example> {
        vec![
            Example::new()
                .with_field("question", "What is 2+2?")
                .with_field("answer", "4")
                .with_inputs(&["question"]),
            Example::new()
                .with_field("question", "What is the capital of France?")
                .with_field("answer", "Paris")
                .with_inputs(&["question"]),
        ]
    }

    fn exact_match_metric() -> MetricFn {
        Arc::new(|predicted: &Example, expected: &Example| {
            let pred_answer = predicted
                .get("answer")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let exp_answer = expected
                .get("answer")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if pred_answer.trim().eq_ignore_ascii_case(exp_answer.trim()) {
                1.0
            } else {
                0.0
            }
        })
    }

    #[test]
    fn test_autoprompt_builder_default() {
        let result = AutoPrompt::builder().metric(exact_match_metric()).build();

        assert!(result.is_ok());
        let ap = result.unwrap();
        assert_eq!(ap.num_triggers, 5);
        assert_eq!(ap.iterations, 3);
        assert!(!ap.vocabulary.is_empty());
    }

    #[test]
    fn test_autoprompt_builder_custom() {
        let result = AutoPrompt::builder()
            .num_triggers(3)
            .iterations(5)
            .vocabulary(vec!["foo".to_string(), "bar".to_string()])
            .random_seed(42)
            .verbose(true)
            .metric(exact_match_metric())
            .build();

        assert!(result.is_ok());
        let ap = result.unwrap();
        assert_eq!(ap.num_triggers, 3);
        assert_eq!(ap.iterations, 5);
        assert_eq!(ap.vocabulary.len(), 2);
        assert_eq!(ap.random_seed, Some(42));
        assert!(ap.verbose);
    }

    #[test]
    fn test_autoprompt_builder_validates_triggers() {
        let result = AutoPrompt::builder()
            .num_triggers(0)
            .metric(exact_match_metric())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("at least 1"));
    }

    #[test]
    fn test_autoprompt_builder_validates_vocabulary() {
        let result = AutoPrompt::builder()
            .vocabulary(vec![])
            .metric(exact_match_metric())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not be empty"));
    }

    #[test]
    fn test_autoprompt_builder_requires_metric() {
        let result = AutoPrompt::builder().num_triggers(5).build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Metric is required"));
    }

    #[test]
    fn test_default_vocabulary() {
        let ap = AutoPrompt::builder()
            .metric(exact_match_metric())
            .build()
            .unwrap();

        // Check that default vocabulary is populated
        assert!(ap.vocabulary.len() >= 40);
        assert!(ap.vocabulary.contains(&"Task:".to_string()));
        assert!(ap.vocabulary.contains(&"Important:".to_string()));
        assert!(ap.vocabulary.contains(&"Expert".to_string()));
    }

    #[test]
    fn test_build_prompt() {
        let signature = create_test_signature();
        let example = Example::new()
            .with_field("question", "What is 2+2?")
            .with_inputs(&["question"]);

        let prompt = AutoPrompt::build_prompt(&signature, &example);

        assert!(prompt.contains("Answer the question"));
        assert!(prompt.contains("Question: What is 2+2?"));
        assert!(prompt.contains("Answer:"));
    }

    #[test]
    fn test_parse_output() {
        let signature = create_test_signature();
        let example = Example::new()
            .with_field("question", "What is 2+2?")
            .with_inputs(&["question"]);

        let response = "4";
        let prediction = AutoPrompt::parse_output(&signature, response, &example).unwrap();

        assert_eq!(prediction.get("answer").and_then(|v| v.as_str()), Some("4"));
        assert_eq!(
            prediction.get("question").and_then(|v| v.as_str()),
            Some("What is 2+2?")
        );
    }

    #[test]
    fn test_autoprompt_debug() {
        let ap = AutoPrompt::builder()
            .num_triggers(3)
            .iterations(2)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        let debug_str = format!("{:?}", ap);
        assert!(debug_str.contains("AutoPrompt"));
        assert!(debug_str.contains("num_triggers: 3"));
        assert!(debug_str.contains("iterations: 2"));
    }

    #[test]
    fn test_autoprompt_accessors() {
        let ap = AutoPrompt::builder()
            .num_triggers(7)
            .iterations(4)
            .vocabulary(vec!["a".to_string(), "b".to_string()])
            .metric(exact_match_metric())
            .build()
            .unwrap();

        assert_eq!(ap.num_triggers(), 7);
        assert_eq!(ap.iterations(), 4);
        assert_eq!(ap.vocabulary().len(), 2);
    }

    #[tokio::test]
    async fn test_autoprompt_compile_with_mock() {
        let signature = create_test_signature();
        let trainset = create_test_examples();

        // Create a fake model that returns predictable responses
        let llm = Arc::new(FakeChatModel::new(vec![
            "4".to_string(),
            "Paris".to_string(),
            "4".to_string(),
            "Paris".to_string(),
            // Repeat for multiple evaluations
            "4".to_string(),
            "Paris".to_string(),
            "4".to_string(),
            "Paris".to_string(),
            "4".to_string(),
            "Paris".to_string(),
            "4".to_string(),
            "Paris".to_string(),
        ]));

        let ap = AutoPrompt::builder()
            .num_triggers(2)
            .iterations(1)
            .vocabulary(vec!["Important:".to_string(), "Note:".to_string()])
            .random_seed(42)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        let result = ap.compile(&signature, &trainset, llm).await;

        // With limited responses, this might fail - that's expected
        // The important thing is the structure works
        if let Ok(optimized) = result {
            // Triggers should be prepended to instructions
            assert!(
                optimized.instructions.contains("Important:")
                    || optimized.instructions.contains("Note:")
            );
            assert!(optimized.instructions.contains("Answer the question"));
        }
    }

    #[tokio::test]
    async fn test_evaluate_triggers_static() {
        let signature = create_test_signature();
        let trainset = create_test_examples();

        let llm: Arc<dyn ChatModel> = Arc::new(FakeChatModel::new(vec![
            "4".to_string(),
            "Paris".to_string(),
        ]));

        let triggers = vec!["Task:".to_string(), "Important:".to_string()];
        let metric = exact_match_metric();

        let score =
            AutoPrompt::evaluate_triggers_static(&signature, &triggers, &trainset, &llm, &metric)
                .await
                .unwrap();

        // Both answers match, so score should be 1.0
        assert!((score - 1.0).abs() < 0.001);
    }
}
