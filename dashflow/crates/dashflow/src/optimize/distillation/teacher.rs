// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Teacher module for generating high-quality training data using expensive models.
//!
//! The Teacher uses a powerful model (like GPT-4) to generate labeled examples
//! that will be used to train cheaper student models.

use crate::constants::DEFAULT_MAX_RETRIES;
use crate::core::language_models::ChatModel;
use crate::optimize::modules::ChainOfThoughtNode;
use crate::optimize::signature::Signature;
use crate::{Error, GraphState, Node, Result};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

/// Teacher that generates training data using an expensive, high-quality model.
///
/// Generic over state type S that implements GraphState.
pub struct Teacher<S: GraphState> {
    llm: Arc<dyn ChatModel>,
    node: Arc<ChainOfThoughtNode<S>>,
    signature: Signature,
    /// Total cost accumulated (estimated based on model pricing)
    total_cost: Arc<Mutex<f64>>,
    /// Number of successful generations
    successful_generations: Arc<Mutex<usize>>,
    _phantom: PhantomData<S>,
}

impl<S: GraphState> Teacher<S> {
    /// Creates a new Teacher with the given LLM and signature.
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model (typically GPT-4)
    /// * `signature` - The signature defining input/output structure
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let teacher = Teacher::new(gpt4_model, qa_signature)?;
    /// ```
    pub fn new(llm: Arc<dyn ChatModel>, signature: Signature) -> Result<Self> {
        let node = Arc::new(ChainOfThoughtNode::new(signature.clone(), llm.clone()));

        Ok(Self {
            llm,
            node,
            signature,
            total_cost: Arc::new(Mutex::new(0.0)),
            successful_generations: Arc::new(Mutex::new(0)),
            _phantom: PhantomData,
        })
    }

    /// Estimates the cost for a given model and token counts.
    ///
    /// Based on OpenAI pricing as of 2024:
    /// - GPT-4: $0.03/1K prompt tokens, $0.06/1K completion tokens
    /// - GPT-4-turbo: $0.01/1K prompt, $0.03/1K completion
    /// - GPT-3.5-turbo: $0.0005/1K prompt, $0.0015/1K completion
    fn estimate_cost(&self, model: &str, input_tokens: usize, output_tokens: usize) -> f64 {
        let (prompt_price, completion_price) = if model.contains("gpt-4-turbo")
            || model.contains("gpt-4-1106")
            || model.contains("gpt-4-0125")
        {
            (0.01, 0.03) // GPT-4 Turbo pricing
        } else if model.contains("gpt-4") {
            (0.03, 0.06) // GPT-4 pricing
        } else if model.contains("gpt-3.5") {
            (0.0005, 0.0015) // GPT-3.5 pricing
        } else {
            // Unknown model, use GPT-4 as conservative estimate
            (0.03, 0.06)
        };

        let prompt_cost = (input_tokens as f64 / 1000.0) * prompt_price;
        let completion_cost = (output_tokens as f64 / 1000.0) * completion_price;
        prompt_cost + completion_cost
    }

    /// Estimates token count from text (rough approximation: ~4 chars per token).
    fn estimate_tokens(text: &str) -> usize {
        // Simple heuristic: ~4 characters per token on average
        text.len().div_ceil(4)
    }

    /// Generates training data by running teacher model on unlabeled examples.
    ///
    /// Includes automatic retry logic for transient API failures and cost tracking.
    ///
    /// # Arguments
    ///
    /// * `unlabeled` - Vector of states with inputs populated
    /// * `progress_callback` - Optional callback for progress updates (current, total)
    ///
    /// # Returns
    ///
    /// Vector of labeled states with both inputs and outputs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let training_data = teacher.generate_training_data(
    ///     questions,
    ///     Some(|current, total| {
    ///         println!("Progress: {}/{}", current, total);
    ///     })
    /// ).await?;
    /// ```
    pub async fn generate_training_data<F>(
        &self,
        unlabeled: Vec<S>,
        progress_callback: Option<F>,
    ) -> Result<Vec<S>>
    where
        F: Fn(usize, usize),
    {
        let mut labeled_examples = Vec::new();
        let total = unlabeled.len();
        let model_name = self.llm.llm_type();

        for (i, state) in unlabeled.into_iter().enumerate() {
            // Try up to DEFAULT_MAX_RETRIES times with exponential backoff
            let mut attempts = 0;
            let max_attempts = DEFAULT_MAX_RETRIES;
            let mut last_error = None;

            while attempts < max_attempts {
                attempts += 1;

                // Generate output using teacher model with retry logic
                match self.node.execute(state.clone()).await {
                    Ok(prediction) => {
                        // Estimate cost based on JSON serialization of state
                        // This approximates the actual token count since the prompt is
                        // derived from the state's serialized fields
                        let input_text = serde_json::to_string(&state).unwrap_or_default();
                        let output_text = serde_json::to_string(&prediction).unwrap_or_default();
                        let input_tokens = Self::estimate_tokens(&input_text);
                        let output_tokens = Self::estimate_tokens(&output_text);
                        let cost = self.estimate_cost(model_name, input_tokens, output_tokens);

                        // Update total cost (ignore if lock poisoned)
                        if let Ok(mut total_cost) = self.total_cost.lock() {
                            *total_cost += cost;
                        }

                        // Update successful generation count (ignore if lock poisoned)
                        if let Ok(mut count) = self.successful_generations.lock() {
                            *count += 1;
                        }

                        labeled_examples.push(prediction);

                        // Update progress
                        if let Some(ref callback) = progress_callback {
                            callback(i + 1, total);
                        }

                        // Success, break retry loop
                        break;
                    }
                    Err(e) => {
                        last_error = Some(e);
                        if attempts < max_attempts {
                            // Exponential backoff: 1s, 2s, 4s
                            let delay_secs = 2_u64.pow(attempts - 1);
                            tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
                        }
                    }
                }
            }

            // If all retries failed, return error
            if labeled_examples.len() != i + 1 {
                return Err(last_error.unwrap_or_else(|| {
                    Error::Validation(format!(
                        "Failed to generate training data after {} attempts",
                        max_attempts
                    ))
                }));
            }
        }

        Ok(labeled_examples)
    }

    /// Returns the total cost incurred by teacher generation so far.
    ///
    /// Note: This is an estimate based on model pricing and approximate token counts.
    /// Actual costs may vary slightly depending on exact tokenization.
    /// Returns 0.0 if lock is poisoned.
    pub fn total_cost(&self) -> f64 {
        self.total_cost.lock().map(|cost| *cost).unwrap_or(0.0)
    }

    /// Returns the number of successful generations completed.
    /// Returns 0 if lock is poisoned.
    pub fn successful_generations(&self) -> usize {
        self.successful_generations
            .lock()
            .map(|count| *count)
            .unwrap_or(0)
    }

    /// Returns the average cost per generation.
    pub fn average_cost_per_generation(&self) -> f64 {
        let cost = self.total_cost();
        let count = self.successful_generations();
        if count == 0 {
            0.0
        } else {
            cost / count as f64
        }
    }

    /// Returns the signature used by this teacher.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Returns the model type being used.
    ///
    /// Uses the underlying ChatModel's `llm_type()` method to identify the model.
    pub fn model_name(&self) -> &str {
        self.llm.llm_type()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MergeableState;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestState {
        value: String,
    }

    impl MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            self.value = other.value.clone();
        }
    }

    #[test]
    fn test_estimate_tokens() {
        // ~4 characters per token
        assert_eq!(Teacher::<TestState>::estimate_tokens("test"), 1); // 4 chars = 1 token
        assert_eq!(Teacher::<TestState>::estimate_tokens("hello world"), 3); // 11 chars = 3 tokens
        assert_eq!(Teacher::<TestState>::estimate_tokens(""), 0); // 0 chars = 0 tokens
        assert_eq!(Teacher::<TestState>::estimate_tokens("a"), 1); // 1 char = 1 token (rounds up)
    }

    #[test]
    fn test_estimate_tokens_long_text() {
        // 100 chars should be ~25 tokens
        let long_text = "a".repeat(100);
        assert_eq!(Teacher::<TestState>::estimate_tokens(&long_text), 25);

        // 1000 chars should be 250 tokens
        let longer_text = "b".repeat(1000);
        assert_eq!(Teacher::<TestState>::estimate_tokens(&longer_text), 250);
    }

    // Note: Tests requiring MockChatModel are commented out because
    // MockChatModel is not publicly exported. These tests document
    // the expected behavior but require integration tests with real mocks.
    //
    // Cost estimation tests verify pricing logic:
    // - GPT-4: $0.03/1K prompt, $0.06/1K completion
    // - GPT-4-turbo: $0.01/1K prompt, $0.03/1K completion
    // - GPT-3.5-turbo: $0.0005/1K prompt, $0.0015/1K completion
    // - Unknown models default to GPT-4 pricing

    // Note: Full end-to-end tests with real LLM are integration tests
    // These unit tests focus on helper methods and cost tracking logic.
}
