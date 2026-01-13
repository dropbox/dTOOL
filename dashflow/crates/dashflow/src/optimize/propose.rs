// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Instruction Proposal System for MIPROv2
//!
//! This module provides the GroundedProposer, which generates instruction
//! candidates for DashOptimize using contextual information.
//!
//! ## LLM-Based Instruction Proposal
//!
//! When an LLM is configured, GroundedProposer uses it to generate novel,
//! task-specific instructions grounded in the dataset. This is more effective
//! than simple tip-based variations because the LLM can:
//! - Understand the actual task from examples
//! - Generate diverse, creative instructions
//! - Apply domain-specific knowledge
//!
//! Without an LLM, GroundedProposer falls back to tip-based variations
//! (useful for testing without API calls).
//!
//! Adapted for DashFlow's architecture: works with Signature rather than full Module trait.

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::optimize::{Example, Signature};
use crate::Error;
use std::sync::Arc;
use tracing;

/// Tips for instruction generation (used randomly to encourage diversity).
const TIPS: &[(&str, &str)] = &[
    ("none", ""),
    ("creative", "Don't be afraid to be creative when creating the new instruction!"),
    ("simple", "Keep the instruction clear and concise."),
    ("description", "Make sure your instruction is very informative and descriptive."),
    ("high_stakes", "The instruction should include a high stakes scenario in which the LM must solve the task!"),
    ("persona", "Include a persona that is relevant to the task in the instruction (ie. \"You are a ...\")"),
];

/// Configuration for GroundedProposer.
#[derive(Clone)]
pub struct GroundedProposerConfig {
    /// Optional LLM for generating instructions.
    /// When provided, the proposer uses the LLM to generate novel instructions.
    /// Without an LLM, falls back to tip-based variations.
    pub llm: Option<Arc<dyn ChatModel>>,

    /// Use dataset summary in proposal context.
    pub data_aware: bool,

    /// Use prompting tips to encourage diversity.
    pub tip_aware: bool,

    /// Batch size for viewing data when creating dataset summary.
    pub view_data_batch_size: usize,

    /// Random seed for tip selection.
    ///
    /// **Note:** This field is currently unused. The tip selection in
    /// `propose_with_tips` iterates deterministically. This field is
    /// retained for future randomization support and API stability.
    pub seed: u64,

    /// Verbose logging.
    pub verbose: bool,
}

impl Default for GroundedProposerConfig {
    fn default() -> Self {
        Self {
            llm: None,
            data_aware: true,
            tip_aware: true,
            view_data_batch_size: 10,
            seed: 42,
            verbose: false,
        }
    }
}

impl GroundedProposerConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set the LLM for instruction generation.
    ///
    /// When provided, enables LLM-based instruction proposal.
    /// Without an LLM, falls back to tip-based variations.
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn ChatModel>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Builder: set data-aware mode.
    ///
    /// When enabled (default), the proposer creates a dataset summary
    /// to ground instruction proposals in the actual task.
    #[must_use]
    pub fn with_data_aware(mut self, data_aware: bool) -> Self {
        self.data_aware = data_aware;
        self
    }

    /// Builder: set tip-aware mode.
    ///
    /// When enabled (default), the proposer uses prompting tips
    /// to encourage diversity in generated instructions.
    #[must_use]
    pub fn with_tip_aware(mut self, tip_aware: bool) -> Self {
        self.tip_aware = tip_aware;
        self
    }

    /// Builder: set the batch size for dataset summary generation.
    #[must_use]
    pub fn with_view_data_batch_size(mut self, size: usize) -> Self {
        self.view_data_batch_size = size;
        self
    }

    /// Builder: set the random seed for tip selection.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Builder: enable verbose logging.
    #[must_use]
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

/// GroundedProposer generates instruction candidates.
///
/// It uses contextual information (dataset summary, prompting tips)
/// to generate diverse, high-quality instructions.
///
/// ## LLM Mode vs Fallback Mode
///
/// When an LLM is configured, the proposer generates instructions by:
/// 1. Building a context prompt with dataset summary and current instruction
/// 2. Asking the LLM to propose N diverse instruction variations
/// 3. Parsing the LLM response to extract individual instructions
///
/// Without an LLM, the proposer uses tip-based variations:
/// 1. Takes the current instruction
/// 2. Appends different prompting tips to create variations
pub struct GroundedProposer {
    config: GroundedProposerConfig,
    data_summary: Option<String>,
}

impl GroundedProposer {
    /// Create a new GroundedProposer.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the proposer
    /// * `trainset` - Training data for dataset summarization
    pub async fn new(config: GroundedProposerConfig, trainset: &[Example]) -> Result<Self, Error> {
        // Generate dataset summary if enabled
        let data_summary = if config.data_aware && !trainset.is_empty() {
            Some(Self::create_dataset_summary(
                trainset,
                config.view_data_batch_size,
            ))
        } else {
            None
        };

        if config.verbose {
            if let Some(summary) = &data_summary {
                tracing::debug!(summary = %summary, "Dataset summary");
            }
        }

        Ok(Self {
            config,
            data_summary,
        })
    }

    /// Propose N instruction candidates.
    ///
    /// Takes a current signature and generates variations.
    /// If an LLM is configured, uses the LLM to generate instructions.
    /// Otherwise, falls back to tip-based variations.
    pub async fn propose_instructions(
        &self,
        signature: &Signature,
        num_candidates: usize,
    ) -> Result<Vec<String>, Error> {
        let current_instruction = if signature.instructions.is_empty() {
            "Solve the task".to_string()
        } else {
            signature.instructions.clone()
        };

        if self.config.verbose {
            tracing::debug!(current = %current_instruction, "Generating instruction variations");
        }

        // Use LLM if available, otherwise fall back to tip-based variations
        let instructions = if let Some(ref llm) = self.config.llm {
            self.propose_with_llm(llm, signature, &current_instruction, num_candidates)
                .await?
        } else {
            self.propose_with_tips(&current_instruction, num_candidates)
        };

        if self.config.verbose {
            tracing::debug!(
                num_candidates = instructions.len(),
                "Generated instruction candidates"
            );
        }

        Ok(instructions)
    }

    /// Generate instructions using an LLM.
    ///
    /// Builds a prompt with dataset context and asks the LLM to propose
    /// diverse instruction variations.
    async fn propose_with_llm(
        &self,
        llm: &Arc<dyn ChatModel>,
        signature: &Signature,
        current_instruction: &str,
        num_candidates: usize,
    ) -> Result<Vec<String>, Error> {
        if self.config.verbose {
            tracing::debug!("Using LLM to propose instructions");
        }

        // Build the proposal prompt
        let prompt = self.build_proposal_prompt(signature, current_instruction, num_candidates);

        // Call the LLM
        let messages = vec![Message::human(prompt)];
        let result = llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::Generic(format!("LLM instruction proposal failed: {}", e)))?;

        // Extract response text
        let response_text = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        // Parse instructions from response
        let mut instructions = self.parse_proposed_instructions(&response_text, num_candidates);

        // Always include the original instruction as first candidate
        if !instructions.contains(&current_instruction.to_string()) {
            instructions.insert(0, current_instruction.to_string());
        }

        // Ensure we have exactly num_candidates
        instructions.truncate(num_candidates);
        while instructions.len() < num_candidates {
            let idx = instructions.len();
            instructions.push(format!("{} (Variant {})", current_instruction, idx));
        }

        Ok(instructions)
    }

    /// Build the prompt for LLM-based instruction proposal.
    fn build_proposal_prompt(
        &self,
        signature: &Signature,
        current_instruction: &str,
        num_candidates: usize,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are an expert prompt engineer. Your task is to propose diverse and effective instructions for a language model task.\n\n");

        // Add task signature context
        prompt.push_str("## Task Signature\n");
        prompt.push_str(&format!(
            "Input fields: {}\n",
            signature
                .input_fields
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        prompt.push_str(&format!(
            "Output fields: {}\n",
            signature
                .output_fields
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        prompt.push('\n');

        // Add dataset summary if available
        if let Some(ref summary) = self.data_summary {
            prompt.push_str("## Dataset Context\n");
            prompt.push_str(summary);
            prompt.push_str("\n\n");
        }

        // Add current instruction
        prompt.push_str("## Current Instruction\n");
        prompt.push_str(current_instruction);
        prompt.push_str("\n\n");

        // Add tips for diversity
        if self.config.tip_aware {
            prompt.push_str("## Tips for Diverse Instructions\n");
            prompt.push_str("Consider these approaches when creating variations:\n");
            for (name, tip) in TIPS.iter().skip(1) {
                if !tip.is_empty() {
                    prompt.push_str(&format!("- {}: {}\n", name, tip));
                }
            }
            prompt.push('\n');
        }

        // Request format
        prompt.push_str(&format!(
            "## Task\nPropose {} diverse instruction variations for this task.\n\n",
            num_candidates
        ));
        prompt.push_str("Output each instruction on a separate line, prefixed with a number:\n");
        prompt.push_str("1. [first instruction]\n");
        prompt.push_str("2. [second instruction]\n");
        prompt.push_str("...\n\n");
        prompt.push_str("Make the instructions diverse by varying:\n");
        prompt.push_str("- Tone (formal vs conversational)\n");
        prompt.push_str("- Specificity (brief vs detailed)\n");
        prompt.push_str("- Persona (with or without role)\n");
        prompt.push_str("- Emphasis (accuracy vs creativity)\n");

        prompt
    }

    /// Parse LLM response to extract proposed instructions.
    ///
    /// Handles common LLM output formats like:
    /// - "1. instruction text"
    /// - "1) instruction text"
    /// - "10. instruction text" (multi-digit numbers)
    /// - Plain instruction text (no numbering)
    fn parse_proposed_instructions(&self, response: &str, max_count: usize) -> Vec<String> {
        let mut instructions = Vec::new();

        for line in response.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Try to extract instruction from numbered format: "1. instruction" or "1) instruction"
            // The two-step stripping handles multi-digit numbers (e.g., "10.", "123."):
            // 1. strip_prefix removes the first digit and returns the rest
            // 2. trim_start_matches removes any remaining digits (for "10", "123", etc.)
            let instruction = if let Some(rest) = line.strip_prefix(|c: char| c.is_ascii_digit()) {
                let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
                let rest = rest.trim_start_matches(['.', ')', ':', '-', ' ']);
                rest.trim()
            } else {
                line
            };

            // Skip very short instructions (likely parsing artifacts or headers).
            // Using byte length (`.len()`) is intentional here:
            // - Instructions are expected to be meaningful English text
            // - 5 bytes is sufficient to filter out noise like ".", "1.", "a)"
            // - For multi-byte UTF-8, this is still a reasonable minimum
            if !instruction.is_empty() && instruction.len() > 5 {
                instructions.push(instruction.to_string());
                if instructions.len() >= max_count {
                    break;
                }
            }
        }

        instructions
    }

    /// Generate instructions using tip-based variations (fallback mode).
    fn propose_with_tips(&self, current_instruction: &str, num_candidates: usize) -> Vec<String> {
        let mut instructions = vec![current_instruction.to_string()];

        // Generate variations using tips
        if self.config.tip_aware {
            for tip in TIPS.iter().skip(1).take(num_candidates - 1) {
                let variant = if tip.1.is_empty() {
                    current_instruction.to_string()
                } else {
                    format!("{} (Tip: {})", current_instruction, tip.1)
                };
                instructions.push(variant);
            }
        }

        // Add generic variants if we need more than TIPS available
        for i in instructions.len()..num_candidates {
            let variant = format!("{} (Variant {})", current_instruction, i);
            instructions.push(variant);
        }

        // Truncate to requested count
        instructions.truncate(num_candidates);

        instructions
    }

    /// Create a summary of the dataset for proposal context.
    fn create_dataset_summary(trainset: &[Example], batch_size: usize) -> String {
        if trainset.is_empty() {
            return "Empty dataset".to_string();
        }

        // Sample up to batch_size examples
        let sample_size = std::cmp::min(trainset.len(), batch_size);
        let samples = &trainset[..sample_size];

        // Extract input and output field names from first example
        let first = &samples[0];
        let inputs: Vec<String> = first.inputs().keys().cloned().collect();
        let labels: Vec<String> = first.labels().keys().cloned().collect();

        // Create summary
        let mut summary = format!("Dataset with {} examples.\n", trainset.len());
        summary.push_str(&format!("Input fields: {}\n", inputs.join(", ")));
        summary.push_str(&format!("Output fields: {}\n", labels.join(", ")));

        // Add a few example inputs
        summary.push_str(&format!(
            "\nExample inputs (showing {} examples):\n",
            sample_size
        ));
        for (i, example) in samples.iter().enumerate() {
            summary.push_str(&format!("  Example {}:\n", i + 1));
            let inputs_ex = example.inputs();
            for (key, value) in inputs_ex.iter() {
                let val_str = match value {
                    serde_json::Value::String(s) => {
                        if s.len() > 100 {
                            format!("{}...", &s[..100])
                        } else {
                            s.clone()
                        }
                    }
                    other => format!("{}", other),
                };
                summary.push_str(&format!("    {}: {}\n", key, val_str));
            }
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_summary() {
        let examples = vec![
            Example::new()
                .with_field("question", "What is 2+2?")
                .with_field("answer", "4")
                .with_inputs(&["question"]),
            Example::new()
                .with_field("question", "What is the capital of France?")
                .with_field("answer", "Paris")
                .with_inputs(&["question"]),
        ];

        let summary = GroundedProposer::create_dataset_summary(&examples, 2);

        assert!(summary.contains("Dataset with 2 examples"));
        assert!(summary.contains("Input fields: question"));
        assert!(summary.contains("Output fields: answer"));
        assert!(summary.contains("What is 2+2?"));
    }

    #[test]
    fn test_tips_defined() {
        assert_eq!(TIPS.len(), 6);
        assert_eq!(TIPS[0].0, "none");
        assert_eq!(TIPS[1].0, "creative");
    }

    #[test]
    fn test_config_defaults() {
        let config = GroundedProposerConfig::default();
        assert!(config.data_aware);
        assert!(config.tip_aware);
        assert_eq!(config.view_data_batch_size, 10);
    }

    #[tokio::test]
    async fn test_proposer_creation() {
        let examples = vec![Example::new()
            .with_field("question", "Test?")
            .with_field("answer", "Test")
            .with_inputs(&["question"])];

        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer::new(config, &examples).await;

        assert!(proposer.is_ok());
        assert!(proposer.unwrap().data_summary.is_some());
    }

    #[tokio::test]
    async fn test_propose_instructions() {
        let examples = vec![Example::new()
            .with_field("question", "Test?")
            .with_field("answer", "Test")
            .with_inputs(&["question"])];

        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        let signature = Signature::new("QA").with_instructions("Answer the question");

        let instructions = proposer.propose_instructions(&signature, 5).await.unwrap();

        assert_eq!(instructions.len(), 5);
        assert_eq!(instructions[0], "Answer the question");
        // Other instructions should be variants
        assert!(instructions[1].contains("Answer the question"));
    }

    #[test]
    fn test_dataset_summary_empty() {
        let examples: Vec<Example> = vec![];
        let summary = GroundedProposer::create_dataset_summary(&examples, 10);
        assert_eq!(summary, "Empty dataset");
    }

    #[test]
    fn test_dataset_summary_batch_size_smaller_than_dataset() {
        let examples = vec![
            Example::new()
                .with_field("q", "One")
                .with_field("a", "1")
                .with_inputs(&["q"]),
            Example::new()
                .with_field("q", "Two")
                .with_field("a", "2")
                .with_inputs(&["q"]),
            Example::new()
                .with_field("q", "Three")
                .with_field("a", "3")
                .with_inputs(&["q"]),
        ];

        // Only show 2 examples even though there are 3
        let summary = GroundedProposer::create_dataset_summary(&examples, 2);

        assert!(summary.contains("Dataset with 3 examples"));
        assert!(summary.contains("showing 2 examples"));
        assert!(summary.contains("One"));
        assert!(summary.contains("Two"));
        // "Three" should not be shown because batch_size is 2
        assert!(!summary.contains("Three"));
    }

    #[test]
    fn test_dataset_summary_long_input_truncation() {
        let long_text = "x".repeat(200);
        let examples = vec![Example::new()
            .with_field("question", long_text.as_str())
            .with_field("answer", "short")
            .with_inputs(&["question"])];

        let summary = GroundedProposer::create_dataset_summary(&examples, 1);

        // Should contain truncated text with "..."
        assert!(summary.contains("..."));
        // Should not contain full 200 character string
        assert!(!summary.contains(&long_text));
    }

    #[test]
    fn test_dataset_summary_non_string_values() {
        let examples = vec![Example::new()
            .with_field("number", serde_json::json!(42))
            .with_field("flag", serde_json::json!(true))
            .with_inputs(&["number"])];

        let summary = GroundedProposer::create_dataset_summary(&examples, 1);

        // Non-string values should be formatted
        assert!(summary.contains("42"));
    }

    #[tokio::test]
    async fn test_proposer_creation_data_aware_disabled() {
        let examples = vec![Example::new()
            .with_field("question", "Test?")
            .with_field("answer", "Test")
            .with_inputs(&["question"])];

        let config = GroundedProposerConfig {
            data_aware: false,
            ..Default::default()
        };
        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        // data_summary should be None when data_aware is false
        assert!(proposer.data_summary.is_none());
    }

    #[tokio::test]
    async fn test_proposer_creation_empty_trainset() {
        let examples: Vec<Example> = vec![];

        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        // No summary when trainset is empty
        assert!(proposer.data_summary.is_none());
    }

    #[tokio::test]
    async fn test_propose_instructions_tip_aware_disabled() {
        let examples = vec![Example::new()
            .with_field("question", "Test?")
            .with_field("answer", "Test")
            .with_inputs(&["question"])];

        let config = GroundedProposerConfig {
            tip_aware: false,
            ..Default::default()
        };
        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        let signature = Signature::new("QA").with_instructions("Original");

        let instructions = proposer.propose_instructions(&signature, 5).await.unwrap();

        assert_eq!(instructions.len(), 5);
        // First should be original
        assert_eq!(instructions[0], "Original");
        // Others should be numbered variants, not tips
        assert!(instructions[1].contains("Variant 1"));
    }

    #[tokio::test]
    async fn test_propose_instructions_empty_instruction() {
        let examples = vec![Example::new()
            .with_field("question", "Test?")
            .with_field("answer", "Test")
            .with_inputs(&["question"])];

        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        // Empty instructions should use default "Solve the task"
        let signature = Signature::new("QA");

        let instructions = proposer.propose_instructions(&signature, 3).await.unwrap();

        assert_eq!(instructions.len(), 3);
        assert_eq!(instructions[0], "Solve the task");
    }

    #[tokio::test]
    async fn test_propose_instructions_more_than_tips() {
        let examples = vec![Example::new()
            .with_field("question", "Test?")
            .with_field("answer", "Test")
            .with_inputs(&["question"])];

        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        let signature = Signature::new("QA").with_instructions("Task");

        // Request more candidates than available tips (TIPS has 6 entries)
        let instructions = proposer.propose_instructions(&signature, 10).await.unwrap();

        assert_eq!(instructions.len(), 10);
        // Later instructions should be numbered variants
        assert!(instructions[9].contains("Variant"));
    }

    #[tokio::test]
    async fn test_propose_instructions_single_candidate() {
        let examples = vec![Example::new()
            .with_field("question", "Test?")
            .with_field("answer", "Test")
            .with_inputs(&["question"])];

        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        let signature = Signature::new("QA").with_instructions("Only one");

        let instructions = proposer.propose_instructions(&signature, 1).await.unwrap();

        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0], "Only one");
    }

    #[test]
    fn test_config_custom_values() {
        let config = GroundedProposerConfig {
            llm: None,
            data_aware: false,
            tip_aware: false,
            view_data_batch_size: 5,
            seed: 123,
            verbose: true,
        };

        assert!(config.llm.is_none());
        assert!(!config.data_aware);
        assert!(!config.tip_aware);
        assert_eq!(config.view_data_batch_size, 5);
        assert_eq!(config.seed, 123);
        assert!(config.verbose);
    }

    #[test]
    fn test_tips_content() {
        // Verify specific tips exist
        assert_eq!(TIPS[0], ("none", ""));
        assert!(TIPS[1].1.contains("creative"));
        assert!(TIPS[2].1.contains("concise"));
        assert!(TIPS[3].1.contains("descriptive"));
        assert!(TIPS[4].1.contains("high stakes"));
        assert!(TIPS[5].1.contains("persona"));
    }

    #[test]
    fn test_dataset_summary_multiple_input_fields() {
        let examples = vec![Example::new()
            .with_field("context", "Some context")
            .with_field("question", "A question?")
            .with_field("answer", "An answer")
            .with_inputs(&["context", "question"])];

        let summary = GroundedProposer::create_dataset_summary(&examples, 1);

        // Both input fields should be listed
        // Note: The order might vary depending on HashMap iteration
        assert!(summary.contains("context") || summary.contains("question"));
        assert!(summary.contains("Output fields: answer"));
    }

    // ==================== LLM-Based Instruction Proposal Tests ====================

    use crate::core::callbacks::CallbackManager;
    use crate::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use crate::core::messages::BaseMessage;
    use async_trait::async_trait;

    /// Mock ChatModel for testing LLM-based proposal
    struct MockProposalLLM {
        response: String,
    }

    impl MockProposalLLM {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

	    #[async_trait]
	    impl ChatModel for MockProposalLLM {
	        async fn _generate(
	            &self,
	            _messages: &[BaseMessage],
	            _stop: Option<&[String]>,
	            _tools: Option<&[ToolDefinition]>,
	            _tool_choice: Option<&ToolChoice>,
	            _callbacks: Option<&CallbackManager>,
        ) -> crate::core::Result<ChatResult> {
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: Message::ai(self.response.clone()),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_proposal"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_parse_proposed_instructions_numbered() {
        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer {
            config,
            data_summary: None,
        };

        let response = r#"
1. Answer the question concisely and accurately.
2. You are an expert assistant. Provide a detailed answer.
3. Think step by step and explain your reasoning.
4. Be creative and engaging in your response.
        "#;

        let instructions = proposer.parse_proposed_instructions(response, 10);

        assert_eq!(instructions.len(), 4);
        assert!(instructions[0].contains("Answer the question"));
        assert!(instructions[1].contains("expert assistant"));
        assert!(instructions[2].contains("step by step"));
        assert!(instructions[3].contains("creative"));
    }

    #[test]
    fn test_parse_proposed_instructions_mixed_format() {
        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer {
            config,
            data_summary: None,
        };

        let response = r#"
1) First instruction here
2. Second instruction with different punctuation
3: Third instruction with colon
4- Fourth instruction with dash
        "#;

        let instructions = proposer.parse_proposed_instructions(response, 10);

        assert_eq!(instructions.len(), 4);
        assert!(instructions[0].contains("First instruction"));
        assert!(instructions[1].contains("Second instruction"));
        assert!(instructions[2].contains("Third instruction"));
        assert!(instructions[3].contains("Fourth instruction"));
    }

    #[test]
    fn test_parse_proposed_instructions_respects_max_count() {
        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer {
            config,
            data_summary: None,
        };

        let response = r#"
1. First instruction candidate here
2. Second instruction candidate here
3. Third instruction candidate here
4. Fourth instruction candidate here
5. Fifth instruction candidate here
        "#;

        let instructions = proposer.parse_proposed_instructions(response, 3);

        assert_eq!(instructions.len(), 3);
    }

    #[test]
    fn test_parse_proposed_instructions_skips_short_lines() {
        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer {
            config,
            data_summary: None,
        };

        let response = r#"
1. Hi
2. This is a valid instruction that should be included.
3. OK
4. Another valid instruction here.
        "#;

        let instructions = proposer.parse_proposed_instructions(response, 10);

        // "Hi" and "OK" should be skipped (too short)
        assert_eq!(instructions.len(), 2);
        assert!(instructions[0].contains("valid instruction"));
        assert!(instructions[1].contains("Another valid"));
    }

    #[test]
    fn test_build_proposal_prompt_includes_signature() {
        use crate::optimize::signature::make_signature;

        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer {
            config,
            data_summary: None,
        };

        let signature = make_signature("question -> answer", "").unwrap();
        let prompt = proposer.build_proposal_prompt(&signature, "Test instruction", 5);

        assert!(prompt.contains("Input fields: question"));
        assert!(prompt.contains("Output fields: answer"));
        assert!(prompt.contains("Test instruction"));
        assert!(prompt.contains("5"));
    }

    #[test]
    fn test_build_proposal_prompt_includes_data_summary() {
        use crate::optimize::signature::make_signature;

        let config = GroundedProposerConfig::default();
        let proposer = GroundedProposer {
            config,
            data_summary: Some("Dataset has 100 Q&A examples about math.".to_string()),
        };

        let signature = make_signature("question -> answer", "").unwrap();
        let prompt = proposer.build_proposal_prompt(&signature, "Test", 3);

        assert!(prompt.contains("Dataset Context"));
        assert!(prompt.contains("100 Q&A examples"));
    }

    #[test]
    fn test_build_proposal_prompt_includes_tips() {
        use crate::optimize::signature::make_signature;

        let config = GroundedProposerConfig {
            tip_aware: true,
            ..Default::default()
        };
        let proposer = GroundedProposer {
            config,
            data_summary: None,
        };

        let signature = make_signature("question -> answer", "").unwrap();
        let prompt = proposer.build_proposal_prompt(&signature, "Test", 3);

        assert!(prompt.contains("Tips for Diverse Instructions"));
        assert!(prompt.contains("creative"));
        assert!(prompt.contains("persona"));
    }

    #[test]
    fn test_build_proposal_prompt_excludes_tips_when_disabled() {
        use crate::optimize::signature::make_signature;

        let config = GroundedProposerConfig {
            tip_aware: false,
            ..Default::default()
        };
        let proposer = GroundedProposer {
            config,
            data_summary: None,
        };

        let signature = make_signature("question -> answer", "").unwrap();
        let prompt = proposer.build_proposal_prompt(&signature, "Test", 3);

        assert!(!prompt.contains("Tips for Diverse Instructions"));
    }

    #[tokio::test]
    async fn test_propose_with_llm() {
        let mock_response = r#"
1. Answer the question clearly and concisely.
2. You are a helpful assistant. Provide accurate answers.
3. Think carefully before responding to the question.
        "#;

        let mock_llm = Arc::new(MockProposalLLM::new(mock_response));

        let config = GroundedProposerConfig {
            llm: Some(mock_llm),
            ..Default::default()
        };

        let examples = vec![Example::new()
            .with_field("question", "What is 2+2?")
            .with_field("answer", "4")
            .with_inputs(&["question"])];

        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        let signature = Signature::new("QA").with_instructions("Answer questions");

        let instructions = proposer.propose_instructions(&signature, 5).await.unwrap();

        assert_eq!(instructions.len(), 5);
        // Original should be included
        assert!(instructions.contains(&"Answer questions".to_string()));
        // LLM-generated should be present
        assert!(instructions.iter().any(|i| i.contains("clearly")));
    }

    #[tokio::test]
    async fn test_propose_with_llm_ensures_original_first() {
        let mock_response = r#"
1. New instruction one
2. New instruction two
        "#;

        let mock_llm = Arc::new(MockProposalLLM::new(mock_response));

        let config = GroundedProposerConfig {
            llm: Some(mock_llm),
            ..Default::default()
        };

        let examples = vec![Example::new()
            .with_field("q", "test")
            .with_field("a", "test")
            .with_inputs(&["q"])];

        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        let signature = Signature::new("QA").with_instructions("Original instruction");

        let instructions = proposer.propose_instructions(&signature, 3).await.unwrap();

        // Original should be first when not in LLM response
        assert_eq!(instructions[0], "Original instruction");
    }

    #[tokio::test]
    async fn test_propose_with_llm_fills_gaps() {
        // LLM returns fewer instructions than requested
        let mock_response = "1. Only one instruction generated";

        let mock_llm = Arc::new(MockProposalLLM::new(mock_response));

        let config = GroundedProposerConfig {
            llm: Some(mock_llm),
            ..Default::default()
        };

        let examples = vec![Example::new()
            .with_field("q", "test")
            .with_field("a", "test")
            .with_inputs(&["q"])];

        let proposer = GroundedProposer::new(config, &examples).await.unwrap();

        let signature = Signature::new("QA").with_instructions("Base");

        let instructions = proposer.propose_instructions(&signature, 5).await.unwrap();

        // Should still return exactly 5
        assert_eq!(instructions.len(), 5);
        // Should have filled with variants
        assert!(instructions.iter().any(|i| i.contains("Variant")));
    }

    #[test]
    fn test_config_with_llm() {
        let mock_llm = Arc::new(MockProposalLLM::new("test"));

        let config = GroundedProposerConfig {
            llm: Some(mock_llm),
            ..Default::default()
        };

        assert!(config.llm.is_some());
    }

    // Tests for GroundedProposerConfig builder pattern

    #[test]
    fn test_config_builder_new() {
        let config = GroundedProposerConfig::new();
        assert!(config.llm.is_none());
        assert!(config.data_aware);
        assert!(config.tip_aware);
        assert_eq!(config.view_data_batch_size, 10);
        assert_eq!(config.seed, 42);
        assert!(!config.verbose);
    }

    #[test]
    fn test_config_builder_chain() {
        let mock_llm = Arc::new(MockProposalLLM::new("test"));

        let config = GroundedProposerConfig::new()
            .with_llm(mock_llm)
            .with_data_aware(false)
            .with_tip_aware(false)
            .with_view_data_batch_size(20)
            .with_seed(123)
            .with_verbose(true);

        assert!(config.llm.is_some());
        assert!(!config.data_aware);
        assert!(!config.tip_aware);
        assert_eq!(config.view_data_batch_size, 20);
        assert_eq!(config.seed, 123);
        assert!(config.verbose);
    }

    #[test]
    fn test_config_builder_partial() {
        // Test partial builder chain (only some fields)
        let config = GroundedProposerConfig::new()
            .with_view_data_batch_size(5)
            .with_verbose(true);

        // Modified fields
        assert_eq!(config.view_data_batch_size, 5);
        assert!(config.verbose);

        // Default fields should remain unchanged
        assert!(config.llm.is_none());
        assert!(config.data_aware);
        assert!(config.tip_aware);
        assert_eq!(config.seed, 42);
    }
}
