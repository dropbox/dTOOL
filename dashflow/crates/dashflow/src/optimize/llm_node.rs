// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # LLM Node - Optimizable Language Model Node
//!
//! LLMNode is a node that calls an LLM and can be optimized with training data.
//!
//! This is a native DashFlow node that generates outputs using an LLM,
//! with automatic prompt optimization via DashOptimize.

use super::{Optimizable, OptimizationResult, OptimizationState, OptimizerConfig, Signature};
use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::node::Node;
use crate::state::GraphState;
use crate::{Error, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing;

/// An LLM node that can be optimized
pub struct LLMNode<S: GraphState> {
    /// The task signature
    pub signature: Signature,

    /// Current optimization state (prompt, few-shot examples, etc.)
    pub optimization_state: OptimizationState,

    /// LLM client (e.g., ChatOpenAI)
    pub llm: Arc<dyn ChatModel>,

    /// Marker for state type
    _phantom: std::marker::PhantomData<S>,
}

impl<S: GraphState> LLMNode<S> {
    /// Create a new LLM node with a signature and LLM client
    pub fn new(signature: Signature, llm: Arc<dyn ChatModel>) -> Self {
        let instruction = signature.instructions.clone();
        Self {
            signature,
            optimization_state: OptimizationState::new(instruction),
            llm,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Extract input field values from state (using serde)
    fn extract_inputs(&self, state: &S) -> Result<std::collections::HashMap<String, String>> {
        let json = serde_json::to_value(state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let mut inputs = std::collections::HashMap::new();
        for field in &self.signature.input_fields {
            let value = json[&field.name]
                .as_str()
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

    /// Build prompt from instruction, few-shot examples, and inputs
    fn build_prompt(&self, inputs: &std::collections::HashMap<String, String>) -> String {
        let mut prompt = String::new();

        // 1. Add instruction
        if !self.optimization_state.instruction.is_empty() {
            prompt.push_str(&self.optimization_state.instruction);
            prompt.push_str("\n\n");
        }

        // 2. Add few-shot examples
        for (example_idx, example) in self.optimization_state.few_shot_examples.iter().enumerate() {
            // Input fields
            for field in &self.signature.input_fields {
                if let Some(value) = example.input.get(&field.name).and_then(|v| v.as_str()) {
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
                } else {
                    tracing::debug!(
                        example_idx,
                        field = %field.name,
                        field_type = "input",
                        "Few-shot example missing expected field"
                    );
                }
            }

            // Output fields
            for field in &self.signature.output_fields {
                if let Some(value) = example.output.get(&field.name).and_then(|v| v.as_str()) {
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
                } else {
                    tracing::debug!(
                        example_idx,
                        field = %field.name,
                        field_type = "output",
                        "Few-shot example missing expected field"
                    );
                }
            }

            // Add reasoning if present
            if let Some(reasoning) = &example.reasoning {
                prompt.push_str(&format!("Reasoning: {}\n", reasoning));
            }

            prompt.push('\n');
        }

        // 3. Add current inputs
        for field in &self.signature.input_fields {
            if let Some(value) = inputs.get(&field.name) {
                prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
            }
        }

        // 4. Add output field prefix (ready for LLM to complete)
        if let Some(first_output) = self.signature.output_fields.first() {
            prompt.push_str(&format!("{}: ", first_output.get_prefix()));
        }

        prompt
    }

    /// Parse LLM response to extract output field values
    ///
    /// Supports two parsing strategies:
    /// 1. **Multi-field parsing**: If the response contains field prefixes (e.g., "Answer: ..."),
    ///    extracts values for each matching field.
    /// 2. **Single-field fallback**: If no prefixes are found and there's exactly one output field,
    ///    assigns the entire trimmed response to that field.
    fn parse_response(&self, response: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut outputs = std::collections::HashMap::new();
        let trimmed = response.trim();

        // Build a map of prefix -> field_name for all output fields
        let prefix_map: std::collections::HashMap<String, &str> = self
            .signature
            .output_fields
            .iter()
            .map(|f| (format!("{}:", f.get_prefix()), f.name.as_str()))
            .collect();

        // Try multi-field parsing: look for "FieldPrefix: value" patterns
        let mut found_any_prefix = false;

        for field in &self.signature.output_fields {
            let prefix = format!("{}:", field.get_prefix());

            // Find the prefix in the response (case-insensitive start search)
            if let Some(start_idx) = trimmed.find(&prefix) {
                found_any_prefix = true;
                let value_start = start_idx + prefix.len();
                let remaining = &trimmed[value_start..];

                // Find where this field's value ends:
                // Either at the next field prefix, a newline, or end of response
                let mut value_end = remaining.len();

                // Check for next field prefix
                for other_prefix in prefix_map.keys() {
                    if other_prefix != &prefix {
                        if let Some(next_idx) = remaining.find(other_prefix.as_str()) {
                            value_end = value_end.min(next_idx);
                        }
                    }
                }

                // Also check for newline boundaries for cleaner parsing
                if let Some(newline_idx) = remaining.find('\n') {
                    // Only use newline as boundary if there's no continuation
                    let after_newline = &remaining[newline_idx..].trim_start();
                    // If next line starts with a known prefix, use newline as boundary
                    for other_prefix in prefix_map.keys() {
                        if after_newline.starts_with(other_prefix.as_str()) {
                            value_end = value_end.min(newline_idx);
                            break;
                        }
                    }
                }

                let value = remaining[..value_end].trim().to_string();
                outputs.insert(field.name.clone(), value);
            }
        }

        // Fallback: if no prefixes found and single output field, use entire response
        if !found_any_prefix {
            if let Some(first_output) = self.signature.output_fields.first() {
                outputs.insert(first_output.name.clone(), trimmed.to_string());
                if self.signature.output_fields.len() > 1 {
                    tracing::debug!(
                        output_fields = self.signature.output_fields.len(),
                        "parse_response: no field prefixes found, using fallback for first field only"
                    );
                }
            }
        }

        Ok(outputs)
    }

    /// Update state with output field values
    fn update_state(
        &self,
        mut state: S,
        outputs: std::collections::HashMap<String, String>,
    ) -> Result<S> {
        // Serialize state to JSON
        let mut json = serde_json::to_value(&state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        // Update output fields
        for (key, value) in outputs {
            json[key] = serde_json::Value::String(value);
        }

        // Deserialize back to state
        state = serde_json::from_value(json)
            .map_err(|e| Error::Validation(format!("Failed to deserialize state: {}", e)))?;

        Ok(state)
    }
}

#[async_trait]
impl<S: GraphState> Node<S> for LLMNode<S> {
    async fn execute(&self, state: S) -> Result<S> {
        // 1. Extract input fields from state
        let inputs = self.extract_inputs(&state)?;

        // 2. Build prompt (instruction + few-shot examples + inputs)
        let prompt = self.build_prompt(&inputs);

        // 3. Call LLM with string prompt
        let messages = vec![Message::human(prompt)];
        let result = self
            .llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "LLMNode".to_string(),
                source: Box::new(e),
            })?;

        // 4. Extract response text
        let response_text = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "LLMNode".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();

        // 5. Parse output fields from response
        let outputs = self.parse_response(&response_text)?;

        // 6. Update state with outputs
        self.update_state(state, outputs)
    }

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
}

#[async_trait]
impl<S: GraphState> Optimizable<S> for LLMNode<S> {
    async fn optimize(
        &mut self,
        examples: &[S],
        metric: &super::MetricFn<S>,
        config: &OptimizerConfig,
    ) -> Result<OptimizationResult> {
        use super::optimizers::BootstrapFewShot;
        use std::time::Instant;

        let start = Instant::now();

        // Validate we have training data
        if examples.is_empty() {
            return Err(Error::Validation(
                "Cannot optimize with empty training set".to_string(),
            ));
        }

        // 1. Evaluate initial score (before optimization)
        let initial_score = self.evaluate_score(examples, metric).await?;

        tracing::debug!(
            score_pct = %format!("{:.2}%", initial_score * 100.0),
            correct = (initial_score * examples.len() as f64) as usize,
            total = examples.len(),
            "LLMNode initial score"
        );

        // 2. Create BootstrapFewShot optimizer with config
        let optimizer = BootstrapFewShot::new().with_config(config.clone());

        // 3. Bootstrap demonstrations
        let demos = optimizer.bootstrap(self, examples, metric).await?;

        // 4. Update optimization state with demonstrations
        self.optimization_state.few_shot_examples = demos;
        self.optimization_state
            .metadata
            .insert("optimizer".to_string(), "BootstrapFewShot".to_string());
        self.optimization_state
            .metadata
            .insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
        self.optimization_state.metadata.insert(
            "num_demos".to_string(),
            self.optimization_state.few_shot_examples.len().to_string(),
        );

        // 5. Evaluate final score (after optimization)
        let final_score = self.evaluate_score(examples, metric).await?;

        tracing::debug!(
            score_pct = %format!("{:.2}%", final_score * 100.0),
            correct = (final_score * examples.len() as f64) as usize,
            total = examples.len(),
            "LLMNode final score"
        );

        let duration = start.elapsed();
        let improvement = final_score - initial_score;
        let converged = improvement >= config.min_improvement;

        tracing::info!(
            status = if converged { "converged" } else { "complete" },
            duration_secs = %format!("{:.1}", duration.as_secs_f64()),
            improvement_pct = %format!("{:+.1}%", improvement * 100.0),
            "LLMNode optimization complete"
        );

        Ok(OptimizationResult {
            initial_score,
            final_score,
            iterations: 1,
            converged,
            duration_secs: duration.as_secs_f64(),
        })
    }

    fn get_optimization_state(&self) -> OptimizationState {
        self.optimization_state.clone()
    }

    fn set_optimization_state(&mut self, state: OptimizationState) {
        self.optimization_state = state;
    }
}

impl<S: GraphState> LLMNode<S> {
    /// Evaluate the node's performance on a set of examples
    ///
    /// Returns average score across all examples. Logs warnings for any execution
    /// or metric failures to aid debugging (failures are counted as score 0.0).
    async fn evaluate_score(&self, examples: &[S], metric: &super::MetricFn<S>) -> Result<f64> {
        if examples.is_empty() {
            return Ok(0.0);
        }

        let mut total_score = 0.0;
        let mut count = 0;
        let mut execution_failures = 0;
        let mut metric_failures = 0;

        for (idx, example) in examples.iter().enumerate() {
            // Run node on this example
            match self.execute(example.clone()).await {
                Ok(prediction) => {
                    // Evaluate with metric
                    match metric(example, &prediction) {
                        Ok(score) => {
                            total_score += score;
                            count += 1;
                        }
                        Err(e) => {
                            metric_failures += 1;
                            tracing::debug!(
                                example_idx = idx,
                                error = %e,
                                "Metric evaluation failed for example"
                            );
                        }
                    }
                }
                Err(e) => {
                    execution_failures += 1;
                    tracing::debug!(
                        example_idx = idx,
                        error = %e,
                        "Node execution failed for example"
                    );
                }
            }
        }

        // Log summary if there were any failures
        if execution_failures > 0 || metric_failures > 0 {
            tracing::warn!(
                total = examples.len(),
                succeeded = count,
                execution_failures,
                metric_failures,
                "evaluate_score encountered failures"
            );
        }

        if count == 0 {
            Ok(0.0)
        } else {
            Ok(total_score / count as f64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::callbacks::CallbackManager;
    use crate::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use crate::core::messages::BaseMessage;
    use crate::optimize::{FewShotExample, Field, MetricFn, Signature};
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};

    // Test state with input and output fields
    // GraphState is auto-implemented via blanket impl for Clone + Send + Sync + Serialize + Deserialize
    #[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
    struct QAState {
        question: String,
        answer: String,
    }

    // Mock ChatModel for testing
    struct MockChatModel {
        response: String,
    }

    impl MockChatModel {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
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
            "mock_chat"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn create_qa_signature() -> Signature {
        Signature::new("QuestionAnswer")
            .with_instructions("Answer the question accurately.")
            .with_input(Field::input("question", "The question to answer"))
            .with_output(Field::output("answer", "The answer to the question"))
    }

    #[tokio::test]
    async fn test_llm_node_creation() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new("Test response"));
        let node: LLMNode<QAState> = LLMNode::new(signature.clone(), llm);

        assert_eq!(node.signature.name, "QuestionAnswer");
        assert_eq!(
            node.optimization_state.instruction,
            "Answer the question accurately."
        );
        assert!(node.optimization_state.few_shot_examples.is_empty());
    }

    #[tokio::test]
    async fn test_extract_inputs() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new("42"));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let state = QAState {
            question: "What is 6 times 7?".to_string(),
            answer: String::new(),
        };

        let inputs = node.extract_inputs(&state).unwrap();
        assert_eq!(
            inputs.get("question"),
            Some(&"What is 6 times 7?".to_string())
        );
    }

    #[tokio::test]
    async fn test_build_prompt_basic() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new(""));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let mut inputs = std::collections::HashMap::new();
        inputs.insert("question".to_string(), "What is 2+2?".to_string());

        let prompt = node.build_prompt(&inputs);

        // Should contain instruction
        assert!(prompt.contains("Answer the question accurately."));
        // Should contain input field
        assert!(prompt.contains("Question: What is 2+2?"));
        // Should end with output prefix
        assert!(prompt.contains("Answer:"));
    }

    #[tokio::test]
    async fn test_build_prompt_with_few_shot_examples() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new(""));
        let mut node: LLMNode<QAState> = LLMNode::new(signature, llm);

        // Add few-shot example
        node.optimization_state
            .few_shot_examples
            .push(FewShotExample {
                input: serde_json::json!({"question": "What is 1+1?"}),
                output: serde_json::json!({"answer": "2"}),
                reasoning: None,
            });

        let mut inputs = std::collections::HashMap::new();
        inputs.insert("question".to_string(), "What is 3+3?".to_string());

        let prompt = node.build_prompt(&inputs);

        // Should contain few-shot example
        assert!(prompt.contains("Question: What is 1+1?"));
        assert!(prompt.contains("Answer: 2"));
        // Should contain current input
        assert!(prompt.contains("Question: What is 3+3?"));
    }

    #[tokio::test]
    async fn test_build_prompt_with_reasoning() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new(""));
        let mut node: LLMNode<QAState> = LLMNode::new(signature, llm);

        // Add few-shot example with reasoning
        node.optimization_state
            .few_shot_examples
            .push(FewShotExample {
                input: serde_json::json!({"question": "What is 5+5?"}),
                output: serde_json::json!({"answer": "10"}),
                reasoning: Some("Adding 5 and 5 gives 10.".to_string()),
            });

        let mut inputs = std::collections::HashMap::new();
        inputs.insert("question".to_string(), "What is 7+7?".to_string());

        let prompt = node.build_prompt(&inputs);

        // Should contain reasoning
        assert!(prompt.contains("Reasoning: Adding 5 and 5 gives 10."));
    }

    #[tokio::test]
    async fn test_parse_response() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new(""));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let response = "  The answer is 42.  ";
        let outputs = node.parse_response(response).unwrap();

        assert_eq!(
            outputs.get("answer"),
            Some(&"The answer is 42.".to_string())
        );
    }

    // State with multiple output fields for testing multi-field parsing
    #[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
    struct MultiOutputState {
        question: String,
        answer: String,
        confidence: String,
    }

    #[tokio::test]
    async fn test_parse_response_multi_field() {
        // Create signature with multiple output fields
        let signature = Signature::new("QAWithConfidence")
            .with_instructions("Answer with confidence level.")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"))
            .with_output(Field::output("confidence", "Confidence level"));

        let llm = Arc::new(MockChatModel::new(""));
        let node: LLMNode<MultiOutputState> = LLMNode::new(signature, llm);

        // Response with field prefixes
        let response = "Answer: Paris\nConfidence: high";
        let outputs = node.parse_response(response).unwrap();

        assert_eq!(outputs.get("answer"), Some(&"Paris".to_string()));
        assert_eq!(outputs.get("confidence"), Some(&"high".to_string()));
    }

    #[tokio::test]
    async fn test_parse_response_multi_field_same_line() {
        let signature = Signature::new("QAWithConfidence")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"))
            .with_output(Field::output("confidence", "Confidence level"));

        let llm = Arc::new(MockChatModel::new(""));
        let node: LLMNode<MultiOutputState> = LLMNode::new(signature, llm);

        // Response with fields on same line (less common but should work)
        let response = "Answer: Berlin Confidence: medium";
        let outputs = node.parse_response(response).unwrap();

        assert_eq!(outputs.get("answer"), Some(&"Berlin".to_string()));
        assert_eq!(outputs.get("confidence"), Some(&"medium".to_string()));
    }

    #[tokio::test]
    async fn test_parse_response_fallback_no_prefix() {
        // Multi-field signature but response has no prefixes - should use fallback
        let signature = Signature::new("QAWithConfidence")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"))
            .with_output(Field::output("confidence", "Confidence level"));

        let llm = Arc::new(MockChatModel::new(""));
        let node: LLMNode<MultiOutputState> = LLMNode::new(signature, llm);

        // Response without any field prefixes
        let response = "Just some raw text without prefixes";
        let outputs = node.parse_response(response).unwrap();

        // Should use fallback: assign to first field
        assert_eq!(
            outputs.get("answer"),
            Some(&"Just some raw text without prefixes".to_string())
        );
        // Second field should not be set
        assert!(outputs.get("confidence").is_none());
    }

    #[tokio::test]
    async fn test_update_state() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new(""));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let state = QAState {
            question: "What is life?".to_string(),
            answer: String::new(),
        };

        let mut outputs = std::collections::HashMap::new();
        outputs.insert("answer".to_string(), "42".to_string());

        let updated = node.update_state(state, outputs).unwrap();

        assert_eq!(updated.question, "What is life?");
        assert_eq!(updated.answer, "42");
    }

    #[tokio::test]
    async fn test_execute_basic() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new("Paris"));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let state = QAState {
            question: "What is the capital of France?".to_string(),
            answer: String::new(),
        };

        let result = node.execute(state).await.unwrap();

        assert_eq!(result.question, "What is the capital of France?");
        assert_eq!(result.answer, "Paris");
    }

    #[tokio::test]
    async fn test_get_optimization_state() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new(""));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let state = node.get_optimization_state();
        assert_eq!(state.instruction, "Answer the question accurately.");
        assert!(state.few_shot_examples.is_empty());
    }

    #[tokio::test]
    async fn test_set_optimization_state() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new(""));
        let mut node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let mut new_state = OptimizationState::new("New instruction");
        new_state.few_shot_examples.push(FewShotExample {
            input: serde_json::json!({"question": "test"}),
            output: serde_json::json!({"answer": "test"}),
            reasoning: None,
        });

        node.set_optimization_state(new_state.clone());

        let retrieved = node.get_optimization_state();
        assert_eq!(retrieved.instruction, "New instruction");
        assert_eq!(retrieved.few_shot_examples.len(), 1);
    }

    #[tokio::test]
    async fn test_evaluate_score_empty() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new("test"));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let metric: MetricFn<QAState> = Arc::new(|_, _| Ok(1.0));
        let examples: Vec<QAState> = vec![];

        let score = node.evaluate_score(&examples, &metric).await.unwrap();
        assert_eq!(score, 0.0);
    }

    #[tokio::test]
    async fn test_evaluate_score_with_examples() {
        let signature = create_qa_signature();
        let llm = Arc::new(MockChatModel::new("expected"));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let metric: MetricFn<QAState> = Arc::new(|expected, predicted| {
            if expected.answer == predicted.answer {
                Ok(1.0)
            } else {
                Ok(0.0)
            }
        });

        let examples = vec![
            QAState {
                question: "Q1".to_string(),
                answer: "expected".to_string(),
            },
            QAState {
                question: "Q2".to_string(),
                answer: "expected".to_string(),
            },
        ];

        let score = node.evaluate_score(&examples, &metric).await.unwrap();
        assert_eq!(score, 1.0); // Both examples should match
    }

    #[tokio::test]
    async fn test_extract_inputs_missing_field() {
        let signature = Signature::new("Test")
            .with_input(Field::input("missing_field", "A field not in state"));

        let llm = Arc::new(MockChatModel::new(""));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let state = QAState {
            question: "test".to_string(),
            answer: String::new(),
        };

        let result = node.extract_inputs(&state);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_llm_node_with_empty_instruction() {
        let signature = Signature::new("NoInstruction")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        let llm = Arc::new(MockChatModel::new("response"));
        let node: LLMNode<QAState> = LLMNode::new(signature, llm);

        let mut inputs = std::collections::HashMap::new();
        inputs.insert("question".to_string(), "test".to_string());

        let prompt = node.build_prompt(&inputs);

        // Should not have instruction block at start
        assert!(prompt.starts_with("Question: test"));
    }
}
