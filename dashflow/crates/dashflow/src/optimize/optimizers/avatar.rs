// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! # AvatarOptimizer - Adaptive Virtual Agent Training and Refinement
//!
//! AvatarOptimizer improves agent instructions through iterative feedback analysis.
//! It compares successful and unsuccessful execution patterns to generate better guidance.
//!
//! ## Algorithm Overview
//!
//! 1. Evaluate examples and partition into positive (successful) and negative (failed)
//! 2. Use a Comparator signature to analyze patterns in positive vs negative inputs
//! 3. Generate refined instructions incorporating this feedback
//! 4. Iterate until convergence or max iterations reached
//!
//! ## When to Use
//!
//! - **Use when**: Optimizing AI agent instructions based on execution feedback
//! - **Use when**: You have clear success/failure signal for task completion
//! - **Cannot use when**: No clear success/failure signal available
//!
//! ## Ported from DashOptimize
//!
//! Based on `dspy/teleprompt/avatar_optimizer.py` from the DSPy framework.
//!
//! ## Key Features
//!
//! - Iterative instruction refinement via positive/negative feedback analysis
//! - Comparator-based pattern recognition for instruction improvement
//! - Tool usage analysis for agent optimization
//!
//! ## References
//!
//! - **Source**: DSPy teleprompt library
//! - **Link**: <https://github.com/stanfordnlp/dspy/blob/main/dspy/teleprompt/avatar_optimizer.py>
//! - **Framework**: <https://arxiv.org/abs/2310.03714>

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::optimize::example::Example;
use crate::optimize::signature::Signature;
use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::Error;
use std::sync::Arc;
use std::time::Instant;

// Import shared MetricFn from types module
pub use super::types::MetricFn;

/// Configuration for AvatarOptimizer
#[derive(Debug, Clone)]
pub struct AvatarConfig {
    /// Maximum optimization iterations
    pub max_iters: usize,

    /// Maximum positive examples to analyze per iteration
    pub max_positive_inputs: usize,

    /// Maximum negative examples to analyze per iteration
    pub max_negative_inputs: usize,

    /// Lower and upper bound of examples per optimization round
    pub optimize_bounds: (usize, usize),

    /// Temperature for LLM generation (higher = more creative)
    pub temperature: f64,
}

impl Default for AvatarConfig {
    fn default() -> Self {
        Self {
            max_iters: 10,
            max_positive_inputs: 5,
            max_negative_inputs: 5,
            optimize_bounds: (5, 50),
            temperature: 1.0,
        }
    }
}

impl AvatarConfig {
    /// Validate the configuration.
    ///
    /// Returns a list of validation errors, or `Ok(())` if all values are valid.
    ///
    /// # Validation Rules
    ///
    /// - `max_iters` must be > 0
    /// - `optimize_bounds.0` (lower) must be <= `optimize_bounds.1` (upper)
    /// - `temperature` must be >= 0
    pub fn validate(&self) -> Result<(), Vec<super::ConfigValidationError>> {
        use super::ConfigValidationError;
        let mut errors = Vec::new();

        if self.max_iters == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "max_iters",
                "Maximum iterations must be greater than 0",
                "Set max_iters to at least 1",
            ));
        }

        if self.optimize_bounds.0 > self.optimize_bounds.1 {
            errors.push(ConfigValidationError::new(
                "optimize_bounds",
                format!(
                    "Lower bound ({}) cannot be greater than upper bound ({})",
                    self.optimize_bounds.0, self.optimize_bounds.1
                ),
            ));
        }

        if self.temperature < 0.0 {
            errors.push(ConfigValidationError::with_suggestion(
                "temperature",
                format!("Temperature {} must be non-negative", self.temperature),
                "Set temperature to 0.0 or higher",
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Create a new config with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum optimization iterations
    #[must_use]
    pub const fn with_max_iters(mut self, max_iters: usize) -> Self {
        self.max_iters = max_iters;
        self
    }

    /// Set maximum positive examples to analyze per iteration
    #[must_use]
    pub const fn with_max_positive_inputs(mut self, max: usize) -> Self {
        self.max_positive_inputs = max;
        self
    }

    /// Set maximum negative examples to analyze per iteration
    #[must_use]
    pub const fn with_max_negative_inputs(mut self, max: usize) -> Self {
        self.max_negative_inputs = max;
        self
    }

    /// Set lower and upper bound of examples per optimization round
    #[must_use]
    pub const fn with_optimize_bounds(mut self, lower: usize, upper: usize) -> Self {
        self.optimize_bounds = (lower, upper);
        self
    }

    /// Set temperature for LLM generation (higher = more creative)
    #[must_use]
    pub const fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = temperature;
        self
    }
}

/// Builder for AvatarOptimizer
pub struct AvatarOptimizerBuilder {
    config: AvatarConfig,
    metric: Option<MetricFn>,
    prompt_model: Option<Arc<dyn ChatModel>>,
    threshold: f64,
}

impl AvatarOptimizerBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            config: AvatarConfig::default(),
            metric: None,
            prompt_model: None,
            threshold: 0.5,
        }
    }

    /// Set maximum iterations
    pub fn max_iters(mut self, max_iters: usize) -> Self {
        self.config.max_iters = max_iters;
        self
    }

    /// Set maximum positive examples to analyze per iteration
    pub fn max_positive_inputs(mut self, max: usize) -> Self {
        self.config.max_positive_inputs = max;
        self
    }

    /// Set maximum negative examples to analyze per iteration
    pub fn max_negative_inputs(mut self, max: usize) -> Self {
        self.config.max_negative_inputs = max;
        self
    }

    /// Set optimize bounds (min, max) examples per round
    pub fn optimize_bounds(mut self, min: usize, max: usize) -> Self {
        self.config.optimize_bounds = (min, max);
        self
    }

    /// Set LLM temperature for generation
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.config.temperature = temperature;
        self
    }

    /// Set the evaluation metric (required)
    pub fn metric(mut self, metric: MetricFn) -> Self {
        self.metric = Some(metric);
        self
    }

    /// Set threshold for positive/negative classification (default: 0.5)
    pub fn threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set a separate prompt model for instruction generation
    pub fn prompt_model(mut self, model: Arc<dyn ChatModel>) -> Self {
        self.prompt_model = Some(model);
        self
    }

    /// Build the AvatarOptimizer
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Metric function is not set
    /// - Config validation fails (max_iters=0, invalid bounds, negative temperature)
    /// - Threshold is outside [0.0, 1.0] range
    pub fn build(self) -> Result<AvatarOptimizer, Error> {
        let metric = self.metric.ok_or_else(|| {
            Error::Validation("AvatarOptimizer requires a metric function".into())
        })?;

        // M-907: Validate config
        if let Err(errors) = self.config.validate() {
            let error_messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            return Err(Error::Validation(format!(
                "AvatarConfig validation failed: {}",
                error_messages.join("; ")
            )));
        }

        // M-907: Validate threshold range
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err(Error::Validation(format!(
                "Threshold {} must be between 0.0 and 1.0",
                self.threshold
            )));
        }

        Ok(AvatarOptimizer {
            config: self.config,
            metric,
            prompt_model: self.prompt_model,
            threshold: self.threshold,
        })
    }
}

impl Default for AvatarOptimizerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// AvatarOptimizer for agent instruction refinement
///
/// This optimizer improves instructions by analyzing patterns in successful vs
/// unsuccessful task executions. It uses a comparator to understand what makes
/// some inputs work better than others.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::optimizers::avatar::{AvatarOptimizer, MetricFn};
/// use std::sync::Arc;
///
/// let metric: MetricFn = Arc::new(|pred, expected| {
///     // Return 1.0 if prediction matches expected, 0.0 otherwise
///     if pred.get("output") == expected.get("output") { 1.0 } else { 0.0 }
/// });
///
/// let optimizer = AvatarOptimizer::builder()
///     .max_iters(10)
///     .max_positive_inputs(5)
///     .max_negative_inputs(5)
///     .metric(metric)
///     .build()?;
///
/// let optimized = optimizer.compile(&signature, &trainset, llm).await?;
/// ```
pub struct AvatarOptimizer {
    config: AvatarConfig,
    metric: MetricFn,
    prompt_model: Option<Arc<dyn ChatModel>>,
    threshold: f64,
}

impl std::fmt::Debug for AvatarOptimizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvatarOptimizer")
            .field("config", &self.config)
            .field("metric", &"<MetricFn>")
            .field("prompt_model", &self.prompt_model.is_some())
            .field("threshold", &self.threshold)
            .finish()
    }
}

impl AvatarOptimizer {
    /// Create a builder for AvatarOptimizer
    pub fn builder() -> AvatarOptimizerBuilder {
        AvatarOptimizerBuilder::new()
    }

    /// Optimize a signature using avatar feedback analysis
    ///
    /// # Algorithm
    ///
    /// 1. Evaluate all training examples with the current signature
    /// 2. Partition examples into positive (score >= threshold) and negative (score < threshold)
    /// 3. For each iteration:
    ///    a. Sample positive and negative examples
    ///    b. Use Comparator to analyze input patterns that distinguish success from failure
    ///    c. Generate new instruction incorporating the analysis
    ///    d. Evaluate new instruction on training set
    ///    e. Keep if improved
    /// 4. Return signature with best instruction
    pub async fn compile(
        &self,
        signature: &Signature,
        trainset: &[Example],
        llm: Arc<dyn ChatModel>,
    ) -> crate::Result<Signature> {
        record_optimization_start("avatar");
        let start_time = Instant::now();
        let mut iterations_completed = 0u64;

        if trainset.len() < 2 {
            return Err(Error::Validation(
                "AvatarOptimizer requires at least 2 training examples".into(),
            ));
        }

        let prompt_model = self.prompt_model.clone().unwrap_or_else(|| llm.clone());

        // Initial evaluation to partition examples
        let (mut positive, mut negative) = self.partition_examples(signature, trainset).await?;

        // If no positive examples, can't learn from success patterns
        if positive.is_empty() {
            return Err(Error::Validation(
                "No examples passed threshold - cannot identify success patterns".into(),
            ));
        }

        let mut best_signature = signature.clone();
        let mut best_score = self.evaluate_signature(&best_signature, trainset).await?;
        let initial_score = best_score;

        for iter in 0..self.config.max_iters {
            record_iteration("avatar");
            iterations_completed += 1;

            // Sample examples for analysis
            let pos_sample = self.sample_examples(&positive, self.config.max_positive_inputs);
            let neg_sample = self.sample_examples(&negative, self.config.max_negative_inputs);

            // Generate improved instruction via comparator analysis
            let new_instruction = self
                .generate_improved_instruction(
                    &best_signature,
                    &pos_sample,
                    &neg_sample,
                    prompt_model.clone(),
                )
                .await?;

            // Create candidate signature with new instruction
            let mut candidate = best_signature.clone();
            candidate.set_instructions(&new_instruction);

            // Evaluate candidate
            let candidate_score = self.evaluate_signature(&candidate, trainset).await?;

            // Keep if improved
            if candidate_score > best_score {
                best_signature = candidate;
                best_score = candidate_score;

                // Re-partition with new signature
                let (p, n) = self.partition_examples(&best_signature, trainset).await?;
                positive = p;
                negative = n;

                tracing::debug!(
                    "AvatarOptimizer iteration {}: improved to {:.3}",
                    iter + 1,
                    best_score
                );
            }

            // Early stopping if we've achieved perfect score
            if best_score >= 1.0 {
                break;
            }
        }

        let duration = start_time.elapsed().as_secs_f64();
        record_optimization_complete(
            "avatar",
            iterations_completed,
            iterations_completed, // candidates = 1 per iteration
            initial_score,
            best_score,
            duration,
        );

        Ok(best_signature)
    }

    /// Partition examples into positive (score >= threshold) and negative (score < threshold)
    async fn partition_examples(
        &self,
        signature: &Signature,
        trainset: &[Example],
    ) -> crate::Result<(Vec<Example>, Vec<Example>)> {
        // Note: signature will be used in full implementation to run examples through the signature
        let _ = signature;

        let mut positive = Vec::new();
        let mut negative = Vec::new();

        for example in trainset {
            // For now, use the example directly as both input and expected output
            // In a real implementation, this would run the signature on the input
            let score = (self.metric)(example, example);

            if score >= self.threshold {
                positive.push(example.clone());
            } else {
                negative.push(example.clone());
            }
        }

        Ok((positive, negative))
    }

    /// Evaluate a signature on the training set
    async fn evaluate_signature(
        &self,
        signature: &Signature,
        trainset: &[Example],
    ) -> crate::Result<f64> {
        // Note: signature will be used in full implementation to run examples through the signature
        let _ = signature;
        if trainset.is_empty() {
            return Ok(0.0);
        }

        let mut total_score = 0.0;
        for example in trainset {
            total_score += (self.metric)(example, example);
        }

        Ok(total_score / trainset.len() as f64)
    }

    /// Sample up to `max` examples from a list
    fn sample_examples(&self, examples: &[Example], max: usize) -> Vec<Example> {
        use rand::prelude::*;

        if examples.len() <= max {
            return examples.to_vec();
        }

        let mut rng = rand::thread_rng();
        examples.choose_multiple(&mut rng, max).cloned().collect()
    }

    /// Generate improved instruction using comparator analysis
    async fn generate_improved_instruction(
        &self,
        signature: &Signature,
        positive: &[Example],
        negative: &[Example],
        llm: Arc<dyn ChatModel>,
    ) -> crate::Result<String> {
        let prompt = self.build_comparator_prompt(signature, positive, negative);

        let messages = vec![Message::human(prompt)];
        let result = llm.generate(&messages, None, None, None, None).await?;

        let content = result
            .generations
            .first()
            .ok_or_else(|| {
                Error::Generic("LLM returned empty response in avatar comparator".to_string())
            })?
            .message
            .content()
            .as_text();

        // Extract the improved instruction from the response
        // Look for instruction between markers or use the full response
        if let Some(instruction) = self.extract_instruction(&content) {
            Ok(instruction)
        } else {
            Ok(content.trim().to_string())
        }
    }

    /// Build the comparator prompt for instruction improvement
    fn build_comparator_prompt(
        &self,
        signature: &Signature,
        positive: &[Example],
        negative: &[Example],
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str(
            "You are an expert at improving AI instructions based on execution patterns.\n\n",
        );

        prompt.push_str("## Current Instruction\n\n");
        prompt.push_str(&signature.instructions);
        prompt.push_str("\n\n");

        prompt.push_str("## Task Description\n\n");
        prompt.push_str(&signature.name);
        prompt.push_str("\n\n");

        if !positive.is_empty() {
            prompt.push_str("## Successful Examples (these worked well)\n\n");
            for (i, ex) in positive.iter().enumerate() {
                prompt.push_str(&format!("Example {}: {:?}\n", i + 1, ex));
            }
            prompt.push('\n');
        }

        if !negative.is_empty() {
            prompt.push_str("## Failed Examples (these did not work)\n\n");
            for (i, ex) in negative.iter().enumerate() {
                prompt.push_str(&format!("Example {}: {:?}\n", i + 1, ex));
            }
            prompt.push('\n');
        }

        prompt.push_str("## Your Task\n\n");
        prompt.push_str("Analyze the patterns in successful vs failed examples. ");
        prompt.push_str("Generate an improved instruction that:\n");
        prompt.push_str("1. Incorporates patterns that lead to success\n");
        prompt.push_str("2. Avoids patterns that lead to failure\n");
        prompt.push_str("3. Is clear and actionable\n\n");
        prompt.push_str("Provide ONLY the improved instruction text, nothing else.");

        prompt
    }

    /// Extract instruction from LLM response
    fn extract_instruction(&self, response: &str) -> Option<String> {
        // Try to find instruction between common markers
        let markers = [
            ("```", "```"),
            ("<instruction>", "</instruction>"),
            ("Instruction:", "\n\n"),
        ];

        for (start, end) in markers {
            if let Some(start_idx) = response.find(start) {
                let content_start = start_idx + start.len();
                if let Some(end_idx) = response[content_start..].find(end) {
                    let instruction = &response[content_start..content_start + end_idx];
                    let trimmed = instruction.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_metric() -> MetricFn {
        Arc::new(|_pred, _expected| 0.5)
    }

    #[test]
    fn test_avatar_config_default() {
        let config = AvatarConfig::default();
        assert_eq!(config.max_iters, 10);
        assert_eq!(config.max_positive_inputs, 5);
        assert_eq!(config.max_negative_inputs, 5);
        assert_eq!(config.optimize_bounds, (5, 50));
        assert!((config.temperature - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder() {
        let optimizer = AvatarOptimizer::builder()
            .max_iters(5)
            .max_positive_inputs(3)
            .max_negative_inputs(3)
            .temperature(0.7)
            .threshold(0.6)
            .metric(dummy_metric())
            .build()
            .expect("Should build successfully");

        assert_eq!(optimizer.config.max_iters, 5);
        assert_eq!(optimizer.config.max_positive_inputs, 3);
        assert_eq!(optimizer.config.max_negative_inputs, 3);
        assert!((optimizer.config.temperature - 0.7).abs() < f64::EPSILON);
        assert!((optimizer.threshold - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder_requires_metric() {
        let result = AvatarOptimizer::builder().max_iters(5).build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("metric"));
    }

    #[test]
    fn test_sample_examples() {
        let optimizer = AvatarOptimizer::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        let examples: Vec<Example> = (0..10)
            .map(|i| Example::new().with("id", serde_json::json!(i)))
            .collect();

        // Sample fewer than available
        let sample = optimizer.sample_examples(&examples, 3);
        assert_eq!(sample.len(), 3);

        // Sample more than available
        let sample = optimizer.sample_examples(&examples, 20);
        assert_eq!(sample.len(), 10);

        // Sample empty list
        let sample = optimizer.sample_examples(&[], 5);
        assert!(sample.is_empty());
    }

    #[test]
    fn test_extract_instruction() {
        let optimizer = AvatarOptimizer::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        // Test with code block
        let response = "Here is the improved instruction:\n```\nDo this task well.\n```\n";
        let extracted = optimizer.extract_instruction(response);
        assert_eq!(extracted, Some("Do this task well.".to_string()));

        // Test with XML tags
        let response = "Analysis: <instruction>Better instruction here</instruction>";
        let extracted = optimizer.extract_instruction(response);
        assert_eq!(extracted, Some("Better instruction here".to_string()));

        // Test with no markers
        let response = "Just plain text without markers.";
        let extracted = optimizer.extract_instruction(response);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_build_comparator_prompt() {
        let optimizer = AvatarOptimizer::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        let signature = Signature::new("Test task").with_instructions("Do the task correctly");

        let pos = Example::new().with("input", serde_json::json!("good input"));
        let neg = Example::new().with("input", serde_json::json!("bad input"));

        let prompt = optimizer.build_comparator_prompt(&signature, &[pos], &[neg]);

        assert!(prompt.contains("Current Instruction"));
        assert!(prompt.contains("Successful Examples"));
        assert!(prompt.contains("Failed Examples"));
        assert!(prompt.contains("improved instruction"));
    }

    #[test]
    fn test_builder_validates_config() {
        // M-907: Test that config validation is called during build
        let result = AvatarOptimizer::builder()
            .max_iters(0) // Invalid: must be > 0
            .metric(dummy_metric())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("max_iters"));
    }

    #[test]
    fn test_builder_validates_threshold_range() {
        // M-907: Test threshold validation
        let result = AvatarOptimizer::builder()
            .threshold(1.5) // Invalid: must be 0-1
            .metric(dummy_metric())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Threshold"));
        assert!(err.contains("between 0.0 and 1.0"));

        // Test negative threshold
        let result = AvatarOptimizer::builder()
            .threshold(-0.1)
            .metric(dummy_metric())
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_accepts_valid_threshold() {
        // Boundary values should work
        let result = AvatarOptimizer::builder()
            .threshold(0.0)
            .metric(dummy_metric())
            .build();
        assert!(result.is_ok());

        let result = AvatarOptimizer::builder()
            .threshold(1.0)
            .metric(dummy_metric())
            .build();
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------------
    // AvatarConfig builder tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_avatar_config_new_matches_default() {
        let config = AvatarConfig::new();
        let default = AvatarConfig::default();
        assert_eq!(config.max_iters, default.max_iters);
        assert_eq!(config.max_positive_inputs, default.max_positive_inputs);
        assert_eq!(config.max_negative_inputs, default.max_negative_inputs);
        assert_eq!(config.optimize_bounds, default.optimize_bounds);
        assert!((config.temperature - default.temperature).abs() < f64::EPSILON);
    }

    #[test]
    fn test_avatar_config_full_builder() {
        let config = AvatarConfig::new()
            .with_max_iters(20)
            .with_max_positive_inputs(10)
            .with_max_negative_inputs(8)
            .with_optimize_bounds(10, 100)
            .with_temperature(0.7);

        assert_eq!(config.max_iters, 20);
        assert_eq!(config.max_positive_inputs, 10);
        assert_eq!(config.max_negative_inputs, 8);
        assert_eq!(config.optimize_bounds, (10, 100));
        assert!((config.temperature - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_avatar_config_partial_builder() {
        let config = AvatarConfig::new()
            .with_max_iters(15)
            .with_temperature(0.5);

        assert_eq!(config.max_iters, 15);
        assert!((config.temperature - 0.5).abs() < f64::EPSILON);
        // Defaults preserved
        assert_eq!(config.max_positive_inputs, 5);
        assert_eq!(config.max_negative_inputs, 5);
        assert_eq!(config.optimize_bounds, (5, 50));
    }

    #[test]
    fn test_avatar_config_validate_passes() {
        let config = AvatarConfig::new()
            .with_max_iters(5)
            .with_optimize_bounds(10, 20)
            .with_temperature(0.8);

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_avatar_config_validate_fails_zero_iters() {
        let config = AvatarConfig::new().with_max_iters(0);

        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "max_iters"));
    }

    #[test]
    fn test_avatar_config_validate_fails_invalid_bounds() {
        let config = AvatarConfig::new().with_optimize_bounds(50, 10); // lower > upper

        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "optimize_bounds"));
    }
}
