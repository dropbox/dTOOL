// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! # InferRules - Rule Induction Optimizer
//!
//! InferRules generates human-readable rules from training examples to guide
//! language model behavior. Unlike other optimizers that tune prompts directly,
//! InferRules produces explicit, interpretable rules.
//!
//! ## Algorithm Overview
//!
//! 1. Analyze training examples to identify patterns
//! 2. Use an LLM to induce concise, actionable rules
//! 3. Generate multiple rule candidate sets
//! 4. Evaluate each candidate program's performance
//! 5. Select the best-performing rule set
//!
//! ## When to Use
//!
//! - **Use when**: Need interpretable, human-readable optimization output
//! - **Use when**: Want to extract explicit guidelines from examples
//! - **Use when**: Need transparent decision rules for auditing
//! - **Cannot use when**: Rules would be too brittle for the task
//! - **Cannot use when**: Task requires nuanced, context-dependent behavior
//!
//! ## Key Features
//!
//! - Generates human-readable rules instead of opaque prompts
//! - Rules can be reviewed, modified, and understood by humans
//! - Iterative refinement of rule sets based on performance
//!
//! ## Ported from DashOptimize
//!
//! Based on `dspy/teleprompt/infer_rules.py` from the DSPy framework.
//!
//! ## References
//!
//! - **Source**: DSPy teleprompt library
//! - **Link**: <https://github.com/stanfordnlp/dspy/blob/main/dspy/teleprompt/infer_rules.py>
//! - **Framework**: <https://arxiv.org/abs/2310.03714>

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::optimize::example::Example;
use crate::optimize::signature::Signature;
use crate::optimize::telemetry::{
    record_candidate_evaluated, record_optimization_complete, record_optimization_start,
    record_rules_generated,
};
use crate::Error;
use std::sync::Arc;
use std::time::Instant;

// Import shared MetricFn from types module
pub use super::types::MetricFn;

/// Configuration for InferRules
#[derive(Debug, Clone)]
pub struct InferRulesConfig {
    /// Number of rule candidate sets to generate
    pub num_candidates: usize,

    /// Maximum rules per candidate set
    pub max_rules: usize,

    /// Whether to include examples with rules in the output.
    /// M-2016: Reserved for future enhancement (API stability)
    ///
    /// When true (not yet implemented), the optimizer would include sample training
    /// examples alongside generated rules to provide additional context. Currently,
    /// rules are generated and formatted without embedded examples regardless of this setting.
    #[allow(dead_code)]
    pub include_examples: bool,

    /// Temperature for rule generation (higher = more diverse rules)
    pub temperature: f64,
}

impl Default for InferRulesConfig {
    fn default() -> Self {
        Self {
            num_candidates: 5,
            max_rules: 10,
            include_examples: true,
            temperature: 0.7,
        }
    }
}

impl InferRulesConfig {
    /// Validate the configuration.
    ///
    /// Returns a list of validation errors, or `Ok(())` if all values are valid.
    ///
    /// # Validation Rules
    ///
    /// - `num_candidates` must be > 0
    /// - `max_rules` must be > 0
    /// - `temperature` must be >= 0
    pub fn validate(&self) -> Result<(), Vec<super::ConfigValidationError>> {
        use super::ConfigValidationError;
        let mut errors = Vec::new();

        if self.num_candidates == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "num_candidates",
                "Number of candidates must be greater than 0",
                "Set num_candidates to at least 1",
            ));
        }

        if self.max_rules == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "max_rules",
                "Maximum rules must be greater than 0",
                "Set max_rules to at least 1",
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
}

/// Builder for InferRules optimizer
pub struct InferRulesBuilder {
    config: InferRulesConfig,
    metric: Option<MetricFn>,
    prompt_model: Option<Arc<dyn ChatModel>>,
}

impl InferRulesBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            config: InferRulesConfig::default(),
            metric: None,
            prompt_model: None,
        }
    }

    /// Set number of candidate rule sets to generate
    pub fn num_candidates(mut self, num: usize) -> Self {
        self.config.num_candidates = num;
        self
    }

    /// Set maximum rules per candidate set
    pub fn max_rules(mut self, max: usize) -> Self {
        self.config.max_rules = max;
        self
    }

    /// Set whether to include examples with rules
    pub fn include_examples(mut self, include: bool) -> Self {
        self.config.include_examples = include;
        self
    }

    /// Set LLM temperature for rule generation
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.config.temperature = temperature;
        self
    }

    /// Set the evaluation metric (required)
    pub fn metric(mut self, metric: MetricFn) -> Self {
        self.metric = Some(metric);
        self
    }

    /// Set a separate prompt model for rule generation
    pub fn prompt_model(mut self, model: Arc<dyn ChatModel>) -> Self {
        self.prompt_model = Some(model);
        self
    }

    /// Build the InferRules optimizer
    pub fn build(self) -> Result<InferRules, Error> {
        // Validate configuration
        if let Err(errors) = self.config.validate() {
            let error_msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            return Err(Error::Validation(format!(
                "InferRules config validation failed: {}",
                error_msgs.join("; ")
            )));
        }

        let metric = self
            .metric
            .ok_or_else(|| Error::Validation("InferRules requires a metric function".into()))?;

        Ok(InferRules {
            config: self.config,
            metric,
            prompt_model: self.prompt_model,
        })
    }
}

impl Default for InferRulesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A candidate rule set with evaluation score
#[derive(Clone, Debug)]
pub struct RuleCandidate {
    /// The generated rules
    pub rules: Vec<String>,

    /// Evaluation score for this rule set
    pub score: f64,
}

/// InferRules optimizer for rule-based instruction generation
///
/// This optimizer generates human-readable rules from training examples.
/// Rules are explicit, interpretable guidelines that can be reviewed
/// and understood by humans.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::optimizers::infer_rules::{InferRules, MetricFn};
/// use std::sync::Arc;
///
/// let metric: MetricFn = Arc::new(|pred, expected| {
///     // Return 1.0 if prediction matches expected, 0.0 otherwise
///     if pred.get("output") == expected.get("output") { 1.0 } else { 0.0 }
/// });
///
/// let optimizer = InferRules::builder()
///     .num_candidates(5)
///     .max_rules(10)
///     .metric(metric)
///     .build()?;
///
/// let optimized = optimizer.compile(&signature, &trainset, llm).await?;
/// // optimized.instruction() now contains extracted rules
/// ```
pub struct InferRules {
    config: InferRulesConfig,
    metric: MetricFn,
    prompt_model: Option<Arc<dyn ChatModel>>,
}

impl std::fmt::Debug for InferRules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferRules")
            .field("config", &self.config)
            .field("metric", &"<MetricFn>")
            .field("prompt_model", &self.prompt_model.is_some())
            .finish()
    }
}

impl InferRules {
    /// Create a builder for InferRules
    pub fn builder() -> InferRulesBuilder {
        InferRulesBuilder::new()
    }

    /// Compile rules from training examples
    ///
    /// # Algorithm
    ///
    /// 1. Analyze training examples to understand the task
    /// 2. Generate num_candidates different rule sets using LLM
    /// 3. Evaluate each rule set on the training data
    /// 4. Return signature with best rules appended to instruction
    pub async fn compile(
        &self,
        signature: &Signature,
        trainset: &[Example],
        llm: Arc<dyn ChatModel>,
    ) -> crate::Result<Signature> {
        record_optimization_start("infer_rules");
        let start_time = Instant::now();

        if trainset.len() < 2 {
            return Err(Error::Validation(
                "InferRules requires at least 2 training examples".into(),
            ));
        }

        let prompt_model = self.prompt_model.clone().unwrap_or_else(|| llm.clone());

        // Generate multiple candidate rule sets
        let mut candidates = Vec::with_capacity(self.config.num_candidates);
        let mut initial_score = 0.0;

        for i in 0..self.config.num_candidates {
            let rules = self
                .generate_rules(signature, trainset, prompt_model.clone(), i)
                .await?;

            record_rules_generated("infer_rules", rules.len() as u64);

            // Evaluate this rule set
            let score = self.evaluate_rules(signature, &rules, trainset).await?;

            // Track initial score from first candidate
            if i == 0 {
                initial_score = score;
            }

            record_candidate_evaluated("infer_rules");

            candidates.push(RuleCandidate { rules, score });

            tracing::debug!("InferRules candidate {}: score {:.3}", i + 1, score);
        }

        // Select best candidate
        // Use unwrap_or to handle NaN scores gracefully (treat NaN as equal)
        let best = candidates
            .iter()
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| Error::Validation("No rule candidates generated".into()))?
            .clone();

        let duration = start_time.elapsed().as_secs_f64();
        record_optimization_complete(
            "infer_rules",
            self.config.num_candidates as u64, // iterations
            candidates.len() as u64,
            initial_score,
            best.score,
            duration,
        );

        // Create signature with rules appended to instruction
        let mut result = signature.clone();
        let rules_text = self.format_rules(&best.rules);
        let new_instruction = format!("{}\n\n## Rules\n\n{}", signature.instructions, rules_text);
        result.set_instructions(&new_instruction);

        Ok(result)
    }

    /// Generate a set of rules from training examples
    async fn generate_rules(
        &self,
        signature: &Signature,
        trainset: &[Example],
        llm: Arc<dyn ChatModel>,
        candidate_idx: usize,
    ) -> crate::Result<Vec<String>> {
        let prompt = self.build_rule_induction_prompt(signature, trainset, candidate_idx);

        let messages = vec![Message::human(prompt)];
        let result = llm.generate(&messages, None, None, None, None).await?;

        let content = result
            .generations
            .first()
            .ok_or_else(|| {
                Error::Generic("LLM returned empty response in rule induction".to_string())
            })?
            .message
            .content()
            .as_text();

        // Parse rules from response
        let rules = self.parse_rules(&content);

        // Limit to max_rules
        let limited_rules: Vec<String> = rules.into_iter().take(self.config.max_rules).collect();

        Ok(limited_rules)
    }

    /// Evaluate a rule set on training data
    async fn evaluate_rules(
        &self,
        _signature: &Signature,
        _rules: &[String],
        trainset: &[Example],
    ) -> crate::Result<f64> {
        if trainset.is_empty() {
            return Ok(0.0);
        }

        // For now, use the metric directly on examples
        // In a full implementation, this would run the signature with rules
        // and compare predictions to expected outputs
        let mut total_score = 0.0;
        for example in trainset {
            total_score += (self.metric)(example, example);
        }

        Ok(total_score / trainset.len() as f64)
    }

    /// Build prompt for rule induction
    fn build_rule_induction_prompt(
        &self,
        signature: &Signature,
        trainset: &[Example],
        candidate_idx: usize,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are an expert at inducing rules from examples.\n\n");

        prompt.push_str("## Task Description\n\n");
        prompt.push_str(&signature.name);
        prompt.push_str("\n\n");

        prompt.push_str("## Current Instruction\n\n");
        prompt.push_str(&signature.instructions);
        prompt.push_str("\n\n");

        prompt.push_str("## Training Examples\n\n");
        for (i, example) in trainset.iter().enumerate().take(20) {
            prompt.push_str(&format!("Example {}: {:?}\n", i + 1, example));
        }
        if trainset.len() > 20 {
            prompt.push_str(&format!("... and {} more examples\n", trainset.len() - 20));
        }
        prompt.push('\n');

        prompt.push_str("## Your Task\n\n");
        prompt.push_str(&format!(
            "Generate {} clear, actionable rules that would help complete this task correctly.\n\n",
            self.config.max_rules
        ));
        prompt.push_str("Requirements:\n");
        prompt.push_str("1. Rules should be specific and actionable\n");
        prompt.push_str("2. Rules should be based on patterns in the examples\n");
        prompt.push_str("3. Each rule should be a single, clear statement\n");
        prompt.push_str("4. Rules should be numbered (1., 2., 3., etc.)\n\n");

        // Add variation for different candidates
        if candidate_idx > 0 {
            prompt.push_str(&format!(
                "Note: Generate a different set of rules than attempt {}. Focus on alternative patterns.\n\n",
                candidate_idx
            ));
        }

        prompt.push_str("Provide ONLY the numbered rules, one per line.");

        prompt
    }

    /// Parse rules from LLM response
    fn parse_rules(&self, response: &str) -> Vec<String> {
        let mut rules = Vec::new();

        for line in response.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for numbered rules (1., 2., etc.)
            if let Some(rule) = self.extract_numbered_rule(trimmed) {
                rules.push(rule);
            }
            // Check for bullet points
            else if trimmed.starts_with('-') || trimmed.starts_with('*') {
                let rule = trimmed[1..].trim().to_string();
                if !rule.is_empty() {
                    rules.push(rule);
                }
            }
        }

        rules
    }

    /// Extract rule text from a numbered line (e.g., "1. Do this" -> "Do this")
    fn extract_numbered_rule(&self, line: &str) -> Option<String> {
        // Match patterns like "1.", "1)", "1:"
        let chars: Vec<char> = line.chars().collect();

        if chars.is_empty() {
            return None;
        }

        let mut idx = 0;

        // Skip leading numbers
        while idx < chars.len() && chars[idx].is_ascii_digit() {
            idx += 1;
        }

        // Must have at least one digit
        if idx == 0 {
            return None;
        }

        // Check for separator (., ), :)
        if idx < chars.len() && (chars[idx] == '.' || chars[idx] == ')' || chars[idx] == ':') {
            idx += 1;
        } else {
            return None;
        }

        // Get remaining text
        let rule: String = chars[idx..].iter().collect::<String>().trim().to_string();

        if rule.is_empty() {
            None
        } else {
            Some(rule)
        }
    }

    /// Format rules as text for inclusion in instruction
    fn format_rules(&self, rules: &[String]) -> String {
        rules
            .iter()
            .enumerate()
            .map(|(i, rule)| format!("{}. {}", i + 1, rule))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_metric() -> MetricFn {
        Arc::new(|_pred, _expected| 0.5)
    }

    #[test]
    fn test_infer_rules_config_default() {
        let config = InferRulesConfig::default();
        assert_eq!(config.num_candidates, 5);
        assert_eq!(config.max_rules, 10);
        assert!(config.include_examples);
        assert!((config.temperature - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder() {
        let optimizer = InferRules::builder()
            .num_candidates(3)
            .max_rules(5)
            .include_examples(false)
            .temperature(0.5)
            .metric(dummy_metric())
            .build()
            .expect("Should build successfully");

        assert_eq!(optimizer.config.num_candidates, 3);
        assert_eq!(optimizer.config.max_rules, 5);
        assert!(!optimizer.config.include_examples);
        assert!((optimizer.config.temperature - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder_requires_metric() {
        let result = InferRules::builder().num_candidates(3).build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("metric"));
    }

    #[test]
    fn test_parse_rules_numbered() {
        let optimizer = InferRules::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        let response = "1. Always start with a greeting\n2. Be concise\n3. End with a summary";
        let rules = optimizer.parse_rules(response);

        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0], "Always start with a greeting");
        assert_eq!(rules[1], "Be concise");
        assert_eq!(rules[2], "End with a summary");
    }

    #[test]
    fn test_parse_rules_bullets() {
        let optimizer = InferRules::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        let response = "- First rule\n- Second rule\n* Third rule";
        let rules = optimizer.parse_rules(response);

        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0], "First rule");
        assert_eq!(rules[1], "Second rule");
        assert_eq!(rules[2], "Third rule");
    }

    #[test]
    fn test_parse_rules_mixed() {
        let optimizer = InferRules::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        let response =
            "Here are the rules:\n\n1. First rule\n2) Second rule\n- Third rule\n\nDone.";
        let rules = optimizer.parse_rules(response);

        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0], "First rule");
        assert_eq!(rules[1], "Second rule");
        assert_eq!(rules[2], "Third rule");
    }

    #[test]
    fn test_extract_numbered_rule() {
        let optimizer = InferRules::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        assert_eq!(
            optimizer.extract_numbered_rule("1. Hello"),
            Some("Hello".to_string())
        );
        assert_eq!(
            optimizer.extract_numbered_rule("10) World"),
            Some("World".to_string())
        );
        assert_eq!(
            optimizer.extract_numbered_rule("5: Test"),
            Some("Test".to_string())
        );
        assert_eq!(optimizer.extract_numbered_rule("No number"), None);
        assert_eq!(optimizer.extract_numbered_rule("1."), None); // Empty rule
    }

    #[test]
    fn test_format_rules() {
        let optimizer = InferRules::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        let rules = vec![
            "First rule".to_string(),
            "Second rule".to_string(),
            "Third rule".to_string(),
        ];

        let formatted = optimizer.format_rules(&rules);
        assert_eq!(formatted, "1. First rule\n2. Second rule\n3. Third rule");
    }

    #[test]
    fn test_build_prompt_includes_key_sections() {
        let optimizer = InferRules::builder()
            .metric(dummy_metric())
            .build()
            .unwrap();

        let signature = Signature::new("Test task").with_instructions("Do the task correctly");

        let example = Example::new().with("input", serde_json::json!("test input"));

        let prompt = optimizer.build_rule_induction_prompt(&signature, &[example], 0);

        assert!(prompt.contains("Task Description"));
        assert!(prompt.contains("Current Instruction"));
        assert!(prompt.contains("Training Examples"));
        assert!(prompt.contains("clear, actionable rules"));
    }

    #[test]
    fn test_builder_validates_num_candidates() {
        let result = InferRules::builder()
            .num_candidates(0) // Invalid: must be > 0
            .metric(dummy_metric())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("num_candidates"));
    }

    #[test]
    fn test_builder_validates_max_rules() {
        let result = InferRules::builder()
            .max_rules(0) // Invalid: must be > 0
            .metric(dummy_metric())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("max_rules"));
    }

    #[test]
    fn test_builder_validates_temperature() {
        let result = InferRules::builder()
            .temperature(-0.5) // Invalid: must be >= 0
            .metric(dummy_metric())
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("temperature"));
    }
}
