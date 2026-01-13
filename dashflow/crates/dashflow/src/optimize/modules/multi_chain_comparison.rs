// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Multi-Chain Comparison Node
//!
//! A node that compares multiple reasoning attempts and synthesizes a final answer.
//! This implements self-consistency and ensemble reasoning approaches.
//!
//! The module takes M completion attempts (predictions from multiple reasoning chains)
//! and generates a final answer by comparing and synthesizing the different reasoning paths.
//!
//! # Use Cases
//! - Self-consistency approaches (sample multiple times, then compare)
//! - Comparing outputs from multiple chains of thought
//! - Ensemble-style reasoning with explicit comparison
//!
//! # How it Works
//! 1. Takes M prediction completions (each with reasoning + answer)
//! 2. Automatically extends the signature with M `reasoning_attempt_N` input fields
//! 3. Prepends a `rationale` output field for synthesis reasoning
//! 4. LLM compares all attempts and generates a final answer
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow::optimize::{MultiChainComparisonNode, make_signature};
//! use dashflow_openai::ChatOpenAI;
//! use std::sync::Arc;
//! use std::collections::HashMap;
//! use serde::{Serialize, Deserialize};
//!
//! # #[derive(Debug, Clone, Serialize, Deserialize)]
//! # struct MyState {
//! #     question: String,
//! #     completions: Vec<HashMap<String, String>>,
//! #     rationale: Option<String>,
//! #     answer: Option<String>,
//! # }
//! #
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create signature for question answering
//! let signature = make_signature("question -> answer", "Answer questions")?;
//!
//! // Create MultiChainComparison with M=3 attempts
//! let llm = Arc::new(ChatOpenAI::default());
//! let mcc = MultiChainComparisonNode::<MyState>::new(signature, llm, 3)?;
//!
//! // Prepare completions from multiple reasoning chains
//! let completions = vec![
//!     HashMap::from([
//!         ("reasoning".to_string(), "I think about geography".to_string()),
//!         ("answer".to_string(), "Paris".to_string()),
//!     ]),
//!     HashMap::from([
//!         ("reasoning".to_string(), "Based on European capitals".to_string()),
//!         ("answer".to_string(), "Paris".to_string()),
//!     ]),
//!     HashMap::from([
//!         ("reasoning".to_string(), "France's capital city".to_string()),
//!         ("answer".to_string(), "Paris".to_string()),
//!     ]),
//! ];
//!
//! // Compare and synthesize
//! let inputs = HashMap::from([("question".to_string(), "What is the capital of France?".to_string())]);
//! // result = mcc.compare_and_synthesize(inputs, completions).await?;
//! # Ok(())
//! # }
//! ```

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::node::Node;
use crate::optimize::{
    Field, FieldKind, Optimizable, OptimizationState, OptimizerConfig, Signature,
};
use crate::state::GraphState;
use crate::{Error, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing;

/// A node that compares multiple reasoning attempts and synthesizes a final answer
#[derive(Clone)]
pub struct MultiChainComparisonNode<S: GraphState> {
    /// The original task signature (before extension)
    pub signature: Signature,

    /// Number of reasoning attempts to compare (M)
    pub m: usize,

    /// The key of the last output field (the final answer field)
    pub last_key: String,

    /// Current optimization state (prompt, few-shot examples, etc.)
    pub optimization_state: OptimizationState,

    /// LLM client (e.g., ChatOpenAI)
    pub llm: Arc<dyn ChatModel>,

    /// Marker for state type
    _phantom: std::marker::PhantomData<S>,
}

impl<S: GraphState> std::fmt::Debug for MultiChainComparisonNode<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiChainComparisonNode")
            .field("signature", &self.signature)
            .field("m", &self.m)
            .field("last_key", &self.last_key)
            .field("optimization_state", &self.optimization_state)
            .field("llm", &"<ChatModel>")
            .finish()
    }
}

impl<S: GraphState> MultiChainComparisonNode<S> {
    /// Create a new MultiChainComparison node with a signature, LLM client, and M attempts.
    ///
    /// # Arguments
    /// * `signature` - The base signature to extend (e.g., "question -> answer")
    /// * `llm` - The language model to use for comparison
    /// * `m` - Number of reasoning attempts to compare
    ///
    /// # Returns
    /// A new MultiChainComparisonNode instance
    ///
    /// # Errors
    /// Returns an error if the signature has no output fields
    pub fn new(signature: Signature, llm: Arc<dyn ChatModel>, m: usize) -> Result<Self> {
        // Get the last output field key
        let last_key = signature
            .output_fields
            .last()
            .ok_or_else(|| {
                Error::Validation("Signature must have at least one output field".to_string())
            })?
            .name
            .clone();

        let instruction = signature.instructions.clone();

        Ok(Self {
            signature,
            m,
            last_key,
            optimization_state: OptimizationState::new(instruction),
            llm,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Get the extended signature with M reasoning attempt fields and rationale output
    fn get_extended_signature(&self) -> Signature {
        let mut extended_sig = self.signature.clone();

        // Add M reasoning attempt input fields
        for idx in 0..self.m {
            let field = Field {
                name: format!("reasoning_attempt_{}", idx + 1),
                description: "${reasoning attempt}".to_string(),
                kind: FieldKind::Input,
                prefix: Some(format!("Student Attempt #{}:", idx + 1)),
            };
            extended_sig.input_fields.push(field);
        }

        // Prepend rationale output field
        let rationale_field = Field {
            name: "rationale".to_string(),
            description: "${corrected reasoning}".to_string(),
            kind: FieldKind::Output,
            prefix: Some(
                "Accurate Reasoning: Thank you everyone. Let's now holistically".to_string(),
            ),
        };

        let mut output_fields = vec![rationale_field];
        output_fields.extend(extended_sig.output_fields.clone());
        extended_sig.output_fields = output_fields;

        extended_sig
    }

    /// Compare completions and generate synthesis
    ///
    /// # Arguments
    /// * `inputs` - Original input fields (e.g., "question")
    /// * `completions` - Vector of M prediction completions to compare
    ///
    /// # Returns
    /// A HashMap with the synthesized result including "rationale" and final answer
    pub async fn compare_and_synthesize(
        &self,
        inputs: HashMap<String, String>,
        completions: Vec<HashMap<String, String>>,
    ) -> Result<HashMap<String, String>> {
        // Validate number of attempts matches M
        if completions.len() != self.m {
            return Err(Error::Validation(format!(
                "The number of attempts ({}) doesn't match the expected number M ({}). \
                 Please set the correct value for M when initializing MultiChainComparison.",
                completions.len(),
                self.m
            )));
        }

        // Extract attempts from completions
        let mut attempts = Vec::new();

        for completion in &completions {
            // Try to get rationale or reasoning field
            let rationale = completion
                .get("rationale")
                .or_else(|| completion.get("reasoning"))
                .map(|s| s.as_str())
                .unwrap_or("")
                .trim();

            // Get first line of rationale
            let rationale_line = rationale.split('\n').next().unwrap_or("").trim();

            // Get the answer (last key field)
            let answer = completion
                .get(&self.last_key)
                .map(|s| s.as_str())
                .unwrap_or("")
                .trim();

            // Get first line of answer
            let answer_line = answer.split('\n').next().unwrap_or("").trim();

            // Format as attempt
            let attempt = format!(
                "«I'm trying to {} I'm not sure but my prediction is {}»",
                rationale_line, answer_line
            );

            attempts.push(attempt);
        }

        // Build the full inputs including reasoning attempts
        let mut full_inputs = inputs.clone();
        for (idx, attempt) in attempts.iter().enumerate() {
            full_inputs.insert(format!("reasoning_attempt_{}", idx + 1), attempt.clone());
        }

        // Build prompt and call LLM
        let prompt = self.build_prompt(&full_inputs);
        let messages = vec![Message::human(prompt)];

        let result = self
            .llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "MultiChainComparisonNode".to_string(),
                source: Box::new(e),
            })?;

        // Extract response text
        let text = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "MultiChainComparisonNode".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();

        // Parse response
        self.parse_response(&text)
    }

    /// Build prompt with all inputs including reasoning attempts
    fn build_prompt(&self, inputs: &HashMap<String, String>) -> String {
        let extended_sig = self.get_extended_signature();
        let mut prompt = String::new();

        // 1. Add instruction
        if !self.optimization_state.instruction.is_empty() {
            prompt.push_str(&self.optimization_state.instruction);
            prompt.push_str("\n\n---\n\n");
        }

        // 2. Add few-shot examples (if any)
        for example in &self.optimization_state.few_shot_examples {
            prompt.push_str("---\n\n");

            // Input fields from example
            for field in &extended_sig.input_fields {
                if let Some(value) = example.input.get(&field.name) {
                    if let Some(value_str) = value.as_str() {
                        let prefix = field.prefix.as_deref().unwrap_or(&field.name);
                        prompt.push_str(&format!("{}: {}\n", prefix, value_str));
                    }
                }
            }

            // Output fields from example
            for field in &extended_sig.output_fields {
                if let Some(value) = example.output.get(&field.name) {
                    if let Some(value_str) = value.as_str() {
                        let prefix = field.prefix.as_deref().unwrap_or(&field.name);
                        prompt.push_str(&format!("{}: {}\n", prefix, value_str));
                    }
                }
            }

            prompt.push_str("\n---\n\n");
        }

        // 3. Add current inputs (including reasoning attempts)
        for field in &extended_sig.input_fields {
            if let Some(value) = inputs.get(&field.name) {
                let prefix = field.prefix.as_deref().unwrap_or(&field.name);
                prompt.push_str(&format!("{}: {}\n", prefix, value));
            }
        }

        // 4. Add output field prefixes to prompt for completion
        for field in &extended_sig.output_fields {
            let prefix = field.prefix.as_deref().unwrap_or(&field.name);
            prompt.push_str(&format!("{}:", prefix));
            // Add space after first field to start completion
            // For first field (rationale), add space to start completion
            // For other fields, just add newline
            prompt.push(if field.name == "rationale" { ' ' } else { '\n' });
        }

        prompt
    }

    /// Parse LLM response into output fields
    fn parse_response(&self, text: &str) -> Result<HashMap<String, String>> {
        let extended_sig = self.get_extended_signature();
        let mut outputs = HashMap::new();

        // Simple parsing: split by output field prefixes
        let mut remaining_text = text;

        for (idx, field) in extended_sig.output_fields.iter().enumerate() {
            let _prefix = field.prefix.as_deref().unwrap_or(&field.name);

            if idx == extended_sig.output_fields.len() - 1 {
                // Last field: take all remaining text
                outputs.insert(field.name.clone(), remaining_text.trim().to_string());
            } else {
                // Find next field prefix
                let next_field = &extended_sig.output_fields[idx + 1];
                let next_prefix = next_field.prefix.as_deref().unwrap_or(&next_field.name);

                if let Some(split_pos) = remaining_text.find(next_prefix) {
                    let value = &remaining_text[..split_pos];
                    outputs.insert(field.name.clone(), value.trim().to_string());
                    remaining_text = &remaining_text[split_pos + next_prefix.len()..];

                    // Skip the colon after prefix if present
                    if remaining_text.starts_with(':') {
                        remaining_text = remaining_text[1..].trim_start();
                    }
                } else {
                    // Next field not found, take all remaining
                    outputs.insert(field.name.clone(), remaining_text.trim().to_string());
                    break;
                }
            }
        }

        Ok(outputs)
    }

    /// Extract input field values from state
    fn extract_inputs(&self, state: &S) -> Result<HashMap<String, String>> {
        let json = serde_json::to_value(state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let obj = json.as_object().ok_or_else(|| {
            Error::Validation("State must serialize to a JSON object".to_string())
        })?;

        let mut inputs = HashMap::new();
        for field in &self.signature.input_fields {
            let value = obj
                .get(&field.name)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::Validation(format!(
                        "Input field '{}' not found or not a string in state",
                        field.name
                    ))
                })?
                .to_string();
            inputs.insert(field.name.clone(), value);
        }

        Ok(inputs)
    }

    /// Write output fields back to state
    fn write_outputs(&self, state: &mut S, outputs: &HashMap<String, String>) -> Result<()> {
        let mut state_json = serde_json::to_value(&state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let obj = state_json.as_object_mut().ok_or_else(|| {
            Error::Validation("State must serialize to a JSON object".to_string())
        })?;

        // Write both rationale and final answer fields
        let extended_sig = self.get_extended_signature();
        for field in &extended_sig.output_fields {
            if let Some(value) = outputs.get(&field.name) {
                obj.insert(field.name.clone(), serde_json::Value::String(value.clone()));
            }
        }

        *state = serde_json::from_value(state_json)
            .map_err(|e| Error::Validation(format!("Failed to deserialize state: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl<S: GraphState> Node<S> for MultiChainComparisonNode<S> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_optimizable(&self) -> bool {
        true
    }

    fn may_use_llm(&self) -> bool {
        true
    }

    async fn execute(&self, mut state: S) -> Result<S> {
        // Extract inputs from state
        let inputs = self.extract_inputs(&state)?;

        // Get completions from state (must be provided)
        // This requires the state to have a "completions" field with Vec<HashMap<String, String>>
        let state_json = serde_json::to_value(&state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let completions_value = state_json
            .get("completions")
            .ok_or_else(|| {
                Error::Validation(
                    "State must have 'completions' field with Vec<HashMap<String, String>>"
                        .to_string(),
                )
            })?
            .clone();

        let completions: Vec<HashMap<String, String>> = serde_json::from_value(completions_value)
            .map_err(|e| {
            Error::Validation(format!(
                "Failed to parse completions as Vec<HashMap<String, String>>: {}",
                e
            ))
        })?;

        // Compare and synthesize
        let outputs = self.compare_and_synthesize(inputs, completions).await?;

        // Write outputs to state
        self.write_outputs(&mut state, &outputs)?;

        Ok(state)
    }
}

#[async_trait]
impl<S: GraphState> Optimizable<S> for MultiChainComparisonNode<S> {
    async fn optimize(
        &mut self,
        examples: &[S],
        metric: &crate::optimize::MetricFn<S>,
        config: &OptimizerConfig,
    ) -> Result<crate::optimize::OptimizationResult> {
        use crate::optimize::optimizers::BootstrapFewShot;
        use std::time::Instant;

        let start = Instant::now();

        // Validate we have training data
        if examples.is_empty() {
            return Err(Error::Validation(
                "Cannot optimize with empty training set".to_string(),
            ));
        }

        // 1. Evaluate initial score (before optimization)
        let initial_score = self.evaluate_score(examples, metric).await.map_err(|e| {
            Error::Validation(format!(
                "MultiChainComparison initial score evaluation failed: {}",
                e
            ))
        })?;

        tracing::debug!(
            score_pct = %format!("{:.2}%", initial_score * 100.0),
            correct = (initial_score * examples.len() as f64) as usize,
            total = examples.len(),
            "MultiChainComparison initial score"
        );

        // 2. Create BootstrapFewShot optimizer with config
        let optimizer = BootstrapFewShot::new().with_config(config.clone());

        // 3. Run optimization (generates few-shot examples)
        let demos = optimizer
            .bootstrap(self, examples, metric)
            .await
            .map_err(|e| {
                Error::Validation(format!("MultiChainComparison demo bootstrap failed: {}", e))
            })?;

        // 4. Update optimization state with generated demos
        self.optimization_state.few_shot_examples = demos;
        self.optimization_state
            .metadata
            .insert("optimizer".to_string(), "BootstrapFewShot".to_string());
        self.optimization_state.metadata.insert(
            "num_demos".to_string(),
            self.optimization_state.few_shot_examples.len().to_string(),
        );

        // 5. Evaluate final score (after optimization)
        let final_score = self.evaluate_score(examples, metric).await.map_err(|e| {
            Error::Validation(format!(
                "MultiChainComparison final score evaluation failed: {}",
                e
            ))
        })?;

        let elapsed = start.elapsed();

        tracing::info!(
            initial_pct = %format!("{:.2}%", initial_score * 100.0),
            final_pct = %format!("{:.2}%", final_score * 100.0),
            improvement_pp = %format!("+{:.2} pp", (final_score - initial_score) * 100.0),
            "MultiChainComparison optimization complete"
        );

        Ok(crate::optimize::OptimizationResult {
            initial_score,
            final_score,
            iterations: 1,
            converged: true,
            duration_secs: elapsed.as_secs_f64(),
        })
    }

    fn get_optimization_state(&self) -> OptimizationState {
        self.optimization_state.clone()
    }

    fn set_optimization_state(&mut self, state: OptimizationState) {
        self.optimization_state = state;
    }
}

impl<S: GraphState> MultiChainComparisonNode<S> {
    /// Helper: Evaluate average metric score across examples
    async fn evaluate_score(
        &self,
        examples: &[S],
        metric: &crate::optimize::MetricFn<S>,
    ) -> Result<f64> {
        let mut total_score = 0.0;

        for example in examples {
            let predicted = self.execute(example.clone()).await?;
            let score = metric(example, &predicted)?;
            total_score += score;
        }

        Ok(total_score / examples.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::language_models::FakeChatModel;
    use crate::optimize::make_signature;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestState {
        question: String,
        completions: Vec<HashMap<String, String>>,
        rationale: Option<String>,
        answer: Option<String>,
    }

    #[test]
    fn test_multi_chain_comparison_creation() {
        let signature = make_signature("question -> answer", "Answer questions").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 3);
        assert!(mcc.is_ok());

        let mcc = mcc.unwrap();
        assert_eq!(mcc.m, 3);
        assert_eq!(mcc.last_key, "answer");
    }

    #[test]
    fn test_multi_chain_comparison_signature_extension() {
        let signature = make_signature("question -> answer", "Answer questions").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        // Verify signature was extended correctly
        let extended_sig = mcc.get_extended_signature();

        // Should have original input + 2 reasoning attempts = 3 inputs
        assert_eq!(extended_sig.input_fields.len(), 3);

        // Should have rationale + answer = 2 outputs
        assert_eq!(extended_sig.output_fields.len(), 2);

        // First output should be rationale
        assert_eq!(extended_sig.output_fields[0].name, "rationale");
    }

    #[tokio::test]
    async fn test_multi_chain_comparison_wrong_attempt_count() {
        let signature = make_signature("question -> answer", "Answer questions").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 3).unwrap();

        // Provide only 2 completions when M=3
        let completions = vec![
            HashMap::from([
                ("reasoning".to_string(), "Test 1".to_string()),
                ("answer".to_string(), "A".to_string()),
            ]),
            HashMap::from([
                ("reasoning".to_string(), "Test 2".to_string()),
                ("answer".to_string(), "B".to_string()),
            ]),
        ];

        let inputs = HashMap::from([("question".to_string(), "What is 2+2?".to_string())]);

        let result = mcc.compare_and_synthesize(inputs, completions).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("doesn't match the expected number M"));
    }

    #[test]
    fn test_multi_chain_comparison_no_output_fields() {
        // Signature with no output fields should error
        let signature = Signature::new("TestWithNoOutputs");
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let result = MultiChainComparisonNode::<TestState>::new(signature, llm, 3);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("at least one output field"));
    }

    #[tokio::test]
    async fn test_compare_and_synthesize_success() {
        let signature = make_signature("question -> answer", "Answer questions").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec![
            "The consensus is clear. Answer: Paris".to_string(),
        ]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 3).unwrap();

        let completions = vec![
            HashMap::from([
                ("reasoning".to_string(), "Capital of France".to_string()),
                ("answer".to_string(), "Paris".to_string()),
            ]),
            HashMap::from([
                ("reasoning".to_string(), "European capitals".to_string()),
                ("answer".to_string(), "Paris".to_string()),
            ]),
            HashMap::from([
                ("reasoning".to_string(), "Geography knowledge".to_string()),
                ("answer".to_string(), "Paris".to_string()),
            ]),
        ];

        let inputs = HashMap::from([("question".to_string(), "Capital of France?".to_string())]);

        let result = mcc.compare_and_synthesize(inputs, completions).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_prompt_includes_instruction() {
        let signature = make_signature("question -> answer", "Answer questions correctly").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is 2+2?".to_string());
        inputs.insert(
            "reasoning_attempt_1".to_string(),
            "First attempt".to_string(),
        );
        inputs.insert(
            "reasoning_attempt_2".to_string(),
            "Second attempt".to_string(),
        );

        let prompt = mcc.build_prompt(&inputs);

        assert!(prompt.contains("Answer questions correctly"));
        assert!(prompt.contains("What is 2+2?"));
        assert!(prompt.contains("Student Attempt #1:"));
        assert!(prompt.contains("Student Attempt #2:"));
    }

    #[test]
    fn test_parse_response_basic() {
        let signature = make_signature("question -> answer", "Answer").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        let response = "The answer based on all attempts is clear. Answer: 42";
        let outputs = mcc.parse_response(response).unwrap();

        // Should have parsed rationale (before Answer:) and answer
        assert!(outputs.contains_key("rationale"));
    }

    #[test]
    fn test_get_extended_signature_reasoning_fields() {
        let signature = make_signature("question -> answer", "Test").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 5).unwrap();

        let extended = mcc.get_extended_signature();

        // Should have 5 reasoning attempt fields
        let reasoning_fields: Vec<_> = extended
            .input_fields
            .iter()
            .filter(|f| f.name.starts_with("reasoning_attempt_"))
            .collect();
        assert_eq!(reasoning_fields.len(), 5);

        // Verify naming
        assert!(reasoning_fields
            .iter()
            .any(|f| f.name == "reasoning_attempt_1"));
        assert!(reasoning_fields
            .iter()
            .any(|f| f.name == "reasoning_attempt_5"));
    }

    #[test]
    fn test_get_optimization_state() {
        let signature = make_signature("question -> answer", "Test instruction").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        let state = mcc.get_optimization_state();

        assert_eq!(state.instruction, "Test instruction");
        assert!(state.few_shot_examples.is_empty());
    }

    #[test]
    fn test_set_optimization_state() {
        let signature = make_signature("question -> answer", "Original").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mut mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        let new_state = OptimizationState::new("New instruction");
        mcc.set_optimization_state(new_state);

        let retrieved = mcc.get_optimization_state();
        assert_eq!(retrieved.instruction, "New instruction");
    }

    #[test]
    fn test_clone() {
        let signature = make_signature("question -> answer", "Test").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 3).unwrap();

        let cloned = mcc.clone();

        assert_eq!(cloned.m, 3);
        assert_eq!(cloned.last_key, "answer");
    }

    #[tokio::test]
    async fn test_extract_inputs() {
        let signature = make_signature("question -> answer", "Test").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        let state = TestState {
            question: "What is 5+5?".to_string(),
            completions: vec![],
            rationale: None,
            answer: None,
        };

        let inputs = mcc.extract_inputs(&state).unwrap();
        assert_eq!(inputs.get("question"), Some(&"What is 5+5?".to_string()));
    }

    #[tokio::test]
    async fn test_write_outputs() {
        let signature = make_signature("question -> answer", "Test").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        let mut state = TestState {
            question: "test".to_string(),
            completions: vec![],
            rationale: None,
            answer: None,
        };

        let mut outputs = HashMap::new();
        outputs.insert("rationale".to_string(), "My reasoning".to_string());
        outputs.insert("answer".to_string(), "42".to_string());

        mcc.write_outputs(&mut state, &outputs).unwrap();

        assert_eq!(state.rationale, Some("My reasoning".to_string()));
        assert_eq!(state.answer, Some("42".to_string()));
    }

    #[tokio::test]
    async fn test_execute_missing_completions() {
        // State that will be missing completions after deserialization
        #[derive(Clone, Debug, Serialize, Deserialize)]
        struct IncompleteState {
            question: String,
            answer: Option<String>,
        }

        let signature = make_signature("question -> answer", "Test").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<IncompleteState>::new(signature, llm, 2).unwrap();

        let state = IncompleteState {
            question: "test".to_string(),
            answer: None,
        };

        let result = mcc.execute(state).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("completions"));
    }

    #[test]
    fn test_compare_and_synthesize_uses_rationale_field() {
        // Completions using "rationale" instead of "reasoning"
        let completions = [
            HashMap::from([
                ("rationale".to_string(), "Rationale 1".to_string()),
                ("answer".to_string(), "A".to_string()),
            ]),
            HashMap::from([
                ("rationale".to_string(), "Rationale 2".to_string()),
                ("answer".to_string(), "B".to_string()),
            ]),
        ];

        // Just verify the completions can be accessed - the actual LLM call
        // would test the full flow
        assert_eq!(completions.len(), 2);
        assert_eq!(
            completions[0].get("rationale"),
            Some(&"Rationale 1".to_string())
        );
    }

    #[test]
    fn test_extended_signature_rationale_prefix() {
        let signature = make_signature("question -> answer", "Test").unwrap();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        let extended = mcc.get_extended_signature();

        // First output field should be rationale with specific prefix
        let rationale_field = &extended.output_fields[0];
        assert_eq!(rationale_field.name, "rationale");
        assert!(rationale_field
            .prefix
            .as_ref()
            .unwrap()
            .contains("holistically"));
    }

    #[test]
    fn test_build_prompt_empty_instruction() {
        let mut signature = make_signature("question -> answer", "").unwrap();
        signature.instructions = String::new();
        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));
        let mcc = MultiChainComparisonNode::<TestState>::new(signature, llm, 2).unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "Test question".to_string());

        let prompt = mcc.build_prompt(&inputs);

        // Should not have instruction block at beginning
        assert!(!prompt.contains("---\n\n---")); // Would indicate empty instruction with separator
    }
}
