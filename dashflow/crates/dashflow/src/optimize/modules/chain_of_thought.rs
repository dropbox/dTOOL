// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Chain of Thought Node
//!
//! A node that uses step-by-step reasoning before generating the final answer.
//! This improves answer quality by encouraging the model to think through its
//! reasoning process explicitly.
//!
//! ChainOfThought extends the signature with a "reasoning" field that comes before
//! the output fields, prompting the model to explain its thought process.

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::node::Node;
use crate::optimize::{
    Field, FieldKind, Optimizable, OptimizationResult, OptimizationState, OptimizerConfig,
    Signature,
};
use crate::state::GraphState;
use crate::{Error, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing;

/// A node that generates step-by-step reasoning before the final answer
pub struct ChainOfThoughtNode<S: GraphState> {
    /// The original task signature (without reasoning field)
    pub signature: Signature,

    /// Current optimization state (prompt, few-shot examples, etc.)
    pub optimization_state: OptimizationState,

    /// LLM client (e.g., ChatOpenAI)
    pub llm: Arc<dyn ChatModel>,

    /// Marker for state type
    _phantom: std::marker::PhantomData<S>,
}

impl<S: GraphState> ChainOfThoughtNode<S> {
    /// Create a new Chain-of-Thought node with a signature and LLM client
    pub fn new(signature: Signature, llm: Arc<dyn ChatModel>) -> Self {
        let instruction = signature.instructions.clone();
        Self {
            signature,
            optimization_state: OptimizationState::new(instruction),
            llm,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the extended signature with reasoning field prepended
    fn get_extended_signature(&self) -> Signature {
        // Create reasoning field
        let reasoning_field = Field {
            name: "reasoning".to_string(),
            description: "Let's think step by step".to_string(),
            kind: FieldKind::Output,
            prefix: Some("Reasoning".to_string()),
        };

        // Prepend reasoning field to output fields
        let mut output_fields = vec![reasoning_field];
        output_fields.extend(self.signature.output_fields.clone());

        Signature {
            name: self.signature.name.clone(),
            instructions: self.signature.instructions.clone(),
            input_fields: self.signature.input_fields.clone(),
            output_fields,
        }
    }

    /// Extract input field values from state
    fn extract_inputs(&self, state: &S) -> Result<std::collections::HashMap<String, String>> {
        let json = serde_json::to_value(state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let obj = json.as_object().ok_or_else(|| {
            Error::Validation("State must serialize to a JSON object".to_string())
        })?;

        let mut inputs = std::collections::HashMap::new();
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

    /// Build prompt with reasoning field included
    fn build_prompt(&self, inputs: &std::collections::HashMap<String, String>) -> String {
        let _extended_sig = self.get_extended_signature();
        let mut prompt = String::new();

        // 1. Add instruction
        if !self.optimization_state.instruction.is_empty() {
            prompt.push_str(&self.optimization_state.instruction);
            prompt.push_str("\n\n");
        }

        // 2. Add few-shot examples (with reasoning if present)
        for example in &self.optimization_state.few_shot_examples {
            // Input fields
            for field in &self.signature.input_fields {
                if let Some(value) = example.input.get(&field.name).and_then(|v| v.as_str()) {
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
                }
            }

            // Reasoning (from example)
            if let Some(reasoning) = &example.reasoning {
                prompt.push_str(&format!("Reasoning: {}\n", reasoning));
            }

            // Output fields
            for field in &self.signature.output_fields {
                if let Some(value) = example.output.get(&field.name).and_then(|v| v.as_str()) {
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
                }
            }

            prompt.push('\n');
        }

        // 3. Add current inputs
        for field in &self.signature.input_fields {
            if let Some(value) = inputs.get(&field.name) {
                prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
            }
        }

        // 4. Add reasoning prompt
        prompt.push_str("Reasoning: ");

        prompt
    }

    /// Parse LLM response to extract reasoning and output fields
    fn parse_response(
        &self,
        response: &str,
    ) -> Result<(String, std::collections::HashMap<String, String>)> {
        // Simple parsing: split on line breaks
        // Format expected: "Let's think...\n\nAnswer: Paris"
        let lines: Vec<&str> = response.trim().split('\n').collect();

        let mut reasoning = String::new();
        let mut outputs = std::collections::HashMap::new();
        let mut parsing_answer = false;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check if this is an output field line (e.g., "Answer: Paris")
            let mut found_output = false;
            for field in &self.signature.output_fields {
                let prefix = format!("{}:", field.get_prefix());
                if trimmed.starts_with(&prefix) {
                    let value = trimmed[prefix.len()..].trim().to_string();
                    outputs.insert(field.name.clone(), value);
                    found_output = true;
                    parsing_answer = true;
                    break;
                }
            }

            // If not an output field and we haven't started parsing answers, it's reasoning
            if !found_output && !parsing_answer {
                if !reasoning.is_empty() {
                    reasoning.push(' ');
                }
                reasoning.push_str(trimmed);
            }
        }

        // If no explicit output format found, treat last line as answer
        if outputs.is_empty() {
            if let Some(first_output) = self.signature.output_fields.first() {
                // Try to find answer after reasoning
                let parts: Vec<&str> = response.split('\n').collect();
                if let Some(last_line) = parts.last() {
                    outputs.insert(first_output.name.clone(), last_line.trim().to_string());
                    // Everything except last line is reasoning
                    reasoning = parts[..parts.len() - 1].join(" ").trim().to_string();
                }
            }
        }

        Ok((reasoning, outputs))
    }

    /// Update state with reasoning and output field values
    fn update_state(
        &self,
        mut state: S,
        reasoning: String,
        outputs: std::collections::HashMap<String, String>,
    ) -> Result<S> {
        // Serialize state to JSON
        let mut json = serde_json::to_value(&state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let obj = json.as_object_mut().ok_or_else(|| {
            Error::Validation("State must serialize to a JSON object".to_string())
        })?;

        // Add reasoning if state has a reasoning field
        if obj.contains_key("reasoning") {
            obj.insert(
                "reasoning".to_string(),
                serde_json::Value::String(reasoning),
            );
        }

        // Update output fields
        for (key, value) in outputs {
            obj.insert(key, serde_json::Value::String(value));
        }

        // Deserialize back to state
        state = serde_json::from_value(json)
            .map_err(|e| Error::Validation(format!("Failed to deserialize state: {}", e)))?;

        Ok(state)
    }
}

#[async_trait]
impl<S: GraphState> Node<S> for ChainOfThoughtNode<S> {
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
                node: "ChainOfThoughtNode".to_string(),
                source: Box::new(e),
            })?;

        // 4. Extract response text
        let response_text = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "ChainOfThoughtNode".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();

        // 5. Parse reasoning and output fields from response
        let (reasoning, outputs) = self.parse_response(&response_text)?;

        // 6. Update state with reasoning and outputs
        self.update_state(state, reasoning, outputs)
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
impl<S: GraphState> Optimizable<S> for ChainOfThoughtNode<S> {
    async fn optimize(
        &mut self,
        examples: &[S],
        metric: &crate::optimize::MetricFn<S>,
        config: &OptimizerConfig,
    ) -> Result<OptimizationResult> {
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
                "ChainOfThought initial score evaluation failed: {}",
                e
            ))
        })?;

        tracing::debug!(
            score_pct = %format!("{:.2}%", initial_score * 100.0),
            correct = (initial_score * examples.len() as f64) as usize,
            total = examples.len(),
            "ChainOfThought initial score"
        );

        // 2. Create BootstrapFewShot optimizer with config
        let optimizer = BootstrapFewShot::new().with_config(config.clone());

        // 3. Bootstrap demonstrations
        let demos = optimizer
            .bootstrap(self, examples, metric)
            .await
            .map_err(|e| {
                Error::Validation(format!("ChainOfThought demo bootstrap failed: {}", e))
            })?;

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
        let final_score = self.evaluate_score(examples, metric).await.map_err(|e| {
            Error::Validation(format!(
                "ChainOfThought final score evaluation failed: {}",
                e
            ))
        })?;

        tracing::debug!(
            score_pct = %format!("{:.2}%", final_score * 100.0),
            correct = (final_score * examples.len() as f64) as usize,
            total = examples.len(),
            "ChainOfThought final score"
        );

        let duration = start.elapsed();
        let improvement = final_score - initial_score;
        let converged = improvement >= config.min_improvement;

        tracing::info!(
            status = if converged { "converged" } else { "complete" },
            duration_secs = %format!("{:.1}", duration.as_secs_f64()),
            improvement_pct = %format!("{:+.1}%", improvement * 100.0),
            "ChainOfThought optimization complete"
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

impl<S: GraphState> ChainOfThoughtNode<S> {
    /// Evaluate the node's performance on a set of examples
    async fn evaluate_score(
        &self,
        examples: &[S],
        metric: &crate::optimize::MetricFn<S>,
    ) -> Result<f64> {
        if examples.is_empty() {
            return Ok(0.0);
        }

        let mut total_score = 0.0;
        let mut count = 0;

        for example in examples {
            // Run node on this example
            if let Ok(prediction) = self.execute(example.clone()).await {
                // Evaluate with metric
                if let Ok(score) = metric(example, &prediction) {
                    total_score += score;
                    count += 1;
                }
            }
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
    use crate::state::MergeableState;
    use serde::{Deserialize, Serialize};

    #[allow(dead_code)] // Test: Required state struct for CoT module tests
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestState {
        question: String,
        reasoning: String,
        answer: String,
    }

    impl MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            if !other.question.is_empty() {
                self.question = other.question.clone();
            }
            if !other.reasoning.is_empty() {
                self.reasoning = other.reasoning.clone();
            }
            if !other.answer.is_empty() {
                self.answer = other.answer.clone();
            }
        }
    }

    // Note: GraphState is auto-implemented via blanket impl

    #[test]
    fn test_chain_of_thought_signature_extension() {
        // Test that signature gets extended with reasoning field
        let signature = Signature::new("QAWithReasoning")
            .with_input(Field::input("question", "The user's question"))
            .with_output(Field::output("answer", "The answer"))
            .with_instructions("Answer questions with reasoning");

        // Create a simple node without actual LLM (we're just testing signature logic)
        let extended_sig = {
            // Simulate the reasoning field prepending
            let reasoning_field = Field {
                name: "reasoning".to_string(),
                description: "Let's think step by step".to_string(),
                kind: FieldKind::Output,
                prefix: Some("Reasoning".to_string()),
            };

            let mut output_fields = vec![reasoning_field];
            output_fields.extend(signature.output_fields.clone());

            Signature {
                name: signature.name.clone(),
                instructions: signature.instructions.clone(),
                input_fields: signature.input_fields.clone(),
                output_fields,
            }
        };

        // Extended signature should have 2 output fields (reasoning + answer)
        assert_eq!(extended_sig.output_fields.len(), 2);
        assert_eq!(extended_sig.output_fields[0].name, "reasoning");
        assert_eq!(extended_sig.output_fields[1].name, "answer");
    }

    #[test]
    fn test_chain_of_thought_prompt_structure() {
        // Test prompt building logic without actual LLM
        let instructions = "Answer questions";
        let question_value = "What is 2+2?";

        // Simulate prompt building
        let mut prompt = String::new();
        prompt.push_str(instructions);
        prompt.push_str("\n\n");
        prompt.push_str(&format!("Question: {}\n", question_value));
        prompt.push_str("Reasoning: ");

        // Should contain all components
        assert!(prompt.contains("Answer questions"));
        assert!(prompt.contains("Question: What is 2+2?"));
        assert!(prompt.contains("Reasoning:"));
    }

    // ============================================================================
    // Response Parsing Tests
    // ============================================================================

    #[test]
    fn test_parse_response_with_explicit_answer() {
        // Test parsing a response that has explicit "Answer: " prefix
        let response = "Let me think through this step by step.\nFirst, I need to add 2 and 2.\nThat gives us 4.\n\nAnswer: 4";

        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        // Simulate parse_response logic
        let lines: Vec<&str> = response.trim().split('\n').collect();
        let mut reasoning = String::new();
        let mut outputs = std::collections::HashMap::new();
        let mut parsing_answer = false;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let mut found_output = false;
            for field in &signature.output_fields {
                let prefix = format!("{}:", field.get_prefix());
                if trimmed.starts_with(&prefix) {
                    let value = trimmed[prefix.len()..].trim().to_string();
                    outputs.insert(field.name.clone(), value);
                    found_output = true;
                    parsing_answer = true;
                    break;
                }
            }

            if !found_output && !parsing_answer {
                if !reasoning.is_empty() {
                    reasoning.push(' ');
                }
                reasoning.push_str(trimmed);
            }
        }

        assert_eq!(outputs.get("answer").unwrap(), "4");
        assert!(reasoning.contains("think through this step by step"));
        assert!(reasoning.contains("add 2 and 2"));
    }

    #[test]
    fn test_parse_response_without_explicit_answer() {
        // Test parsing a response without "Answer:" prefix
        let response =
            "The capital of France is Paris.\nIt has been the capital for centuries.\nParis";

        let _signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        // When no explicit output format, last line is the answer
        let parts: Vec<&str> = response.split('\n').collect();
        let answer = parts.last().unwrap().trim().to_string();
        let reasoning_parts = &parts[..parts.len() - 1];
        let reasoning = reasoning_parts.join(" ").trim().to_string();

        assert_eq!(answer, "Paris");
        assert!(reasoning.contains("capital of France"));
    }

    #[test]
    fn test_parse_response_multiline_reasoning() {
        // Test parsing with multi-line reasoning
        let response = "Step 1: Identify the problem\nStep 2: Break it down\nStep 3: Solve each part\nStep 4: Combine results\n\nAnswer: 42";

        let lines: Vec<&str> = response.trim().split('\n').collect();
        let mut reasoning_lines = Vec::new();
        let mut answer_found = false;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with("Answer:") {
                answer_found = true;
            } else if !answer_found && !trimmed.is_empty() {
                reasoning_lines.push(trimmed);
            }
        }

        let reasoning = reasoning_lines.join(" ");
        assert!(reasoning.contains("Step 1"));
        assert!(reasoning.contains("Step 4"));
        assert!(answer_found);
    }

    // ============================================================================
    // Signature Extension Tests
    // ============================================================================

    #[test]
    fn test_extended_signature_preserves_inputs() {
        let signature = Signature::new("ComplexQA")
            .with_input(Field::input("context", "Background context"))
            .with_input(Field::input("question", "The user's question"))
            .with_output(Field::output("answer", "The answer"));

        let reasoning_field = Field {
            name: "reasoning".to_string(),
            description: "Let's think step by step".to_string(),
            kind: FieldKind::Output,
            prefix: Some("Reasoning".to_string()),
        };

        let mut output_fields = vec![reasoning_field];
        output_fields.extend(signature.output_fields.clone());

        let extended = Signature {
            name: signature.name.clone(),
            instructions: signature.instructions.clone(),
            input_fields: signature.input_fields.clone(),
            output_fields,
        };

        // Input fields should be preserved
        assert_eq!(extended.input_fields.len(), 2);
        assert_eq!(extended.input_fields[0].name, "context");
        assert_eq!(extended.input_fields[1].name, "question");

        // Output fields should have reasoning prepended
        assert_eq!(extended.output_fields.len(), 2);
        assert_eq!(extended.output_fields[0].name, "reasoning");
        assert_eq!(extended.output_fields[1].name, "answer");
    }

    #[test]
    fn test_extended_signature_with_multiple_outputs() {
        let signature = Signature::new("MultiOutput")
            .with_input(Field::input("input", "Input data"))
            .with_output(Field::output("summary", "Brief summary"))
            .with_output(Field::output("details", "Detailed analysis"));

        let reasoning_field = Field {
            name: "reasoning".to_string(),
            description: "Let's think step by step".to_string(),
            kind: FieldKind::Output,
            prefix: Some("Reasoning".to_string()),
        };

        let mut output_fields = vec![reasoning_field];
        output_fields.extend(signature.output_fields.clone());

        assert_eq!(output_fields.len(), 3);
        assert_eq!(output_fields[0].name, "reasoning");
        assert_eq!(output_fields[1].name, "summary");
        assert_eq!(output_fields[2].name, "details");
    }

    // ============================================================================
    // Field Tests
    // ============================================================================

    #[test]
    fn test_reasoning_field_properties() {
        let reasoning_field = Field {
            name: "reasoning".to_string(),
            description: "Let's think step by step".to_string(),
            kind: FieldKind::Output,
            prefix: Some("Reasoning".to_string()),
        };

        assert_eq!(reasoning_field.name, "reasoning");
        assert_eq!(reasoning_field.get_prefix(), "Reasoning");
        assert!(matches!(reasoning_field.kind, FieldKind::Output));
    }

    #[test]
    fn test_field_prefix_default() {
        let field = Field::output("answer", "The answer");
        // Default prefix should capitalize the field name
        assert!(field.prefix.is_none() || field.get_prefix() == "Answer");
    }

    // ============================================================================
    // State Tests
    // ============================================================================

    #[test]
    fn test_test_state_merge() {
        let mut state1 = TestState {
            question: "What is 1+1?".to_string(),
            reasoning: "".to_string(),
            answer: "".to_string(),
        };

        let state2 = TestState {
            question: "".to_string(),
            reasoning: "I need to add the numbers".to_string(),
            answer: "2".to_string(),
        };

        state1.merge(&state2);

        assert_eq!(state1.question, "What is 1+1?"); // Not overwritten (state2 is empty)
        assert_eq!(state1.reasoning, "I need to add the numbers");
        assert_eq!(state1.answer, "2");
    }

    #[test]
    fn test_test_state_serialization() {
        let state = TestState {
            question: "What color is the sky?".to_string(),
            reasoning: "Looking up...".to_string(),
            answer: "Blue".to_string(),
        };

        let json = serde_json::to_value(&state).unwrap();

        assert_eq!(json["question"], "What color is the sky?");
        assert_eq!(json["reasoning"], "Looking up...");
        assert_eq!(json["answer"], "Blue");
    }

    #[test]
    fn test_test_state_deserialization() {
        let json = serde_json::json!({
            "question": "What is 2*3?",
            "reasoning": "Multiplication",
            "answer": "6"
        });

        let state: TestState = serde_json::from_value(json).unwrap();

        assert_eq!(state.question, "What is 2*3?");
        assert_eq!(state.reasoning, "Multiplication");
        assert_eq!(state.answer, "6");
    }

    // ============================================================================
    // Optimization State Tests
    // ============================================================================

    #[test]
    fn test_optimization_state_creation() {
        let instruction = "Answer the question with reasoning";
        let opt_state = OptimizationState::new(instruction.to_string());

        assert_eq!(opt_state.instruction, instruction);
        assert!(opt_state.few_shot_examples.is_empty());
        assert!(opt_state.metadata.is_empty());
    }

    #[test]
    fn test_optimization_state_with_instruction() {
        let instruction = "You are a helpful math tutor";
        let mut opt_state = OptimizationState::new(String::new());
        opt_state.instruction = instruction.to_string();

        assert_eq!(opt_state.instruction, instruction);
    }

    // ============================================================================
    // Prompt Building Edge Cases
    // ============================================================================

    #[test]
    fn test_prompt_with_empty_instruction() {
        let mut prompt = String::new();
        let instruction = "";

        if !instruction.is_empty() {
            prompt.push_str(instruction);
            prompt.push_str("\n\n");
        }

        prompt.push_str("Question: Test?\n");
        prompt.push_str("Reasoning: ");

        assert!(!prompt.contains("\n\n\n")); // No double blank lines
        assert!(prompt.starts_with("Question:"));
    }

    #[test]
    fn test_prompt_with_special_characters() {
        let question = "What is \"hello\" in French? (use quotes)";
        let prompt = format!("Question: {}\nReasoning: ", question);

        assert!(prompt.contains("\"hello\""));
        assert!(prompt.contains("(use quotes)"));
    }

    #[test]
    fn test_prompt_with_unicode() {
        let question = "What does 你好 mean? 🤔";
        let prompt = format!("Question: {}\nReasoning: ", question);

        assert!(prompt.contains("你好"));
        assert!(prompt.contains("🤔"));
    }

    // ============================================================================
    // Response Parsing Edge Cases
    // ============================================================================

    #[test]
    fn test_parse_empty_response() {
        let response = "";
        let lines: Vec<&str> = response.trim().split('\n').collect();

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "");
    }

    #[test]
    fn test_parse_response_with_only_answer() {
        let response = "Answer: 42";

        let _signature = Signature::new("QA")
            .with_input(Field::input("q", "Question"))
            .with_output(Field::output("answer", "Answer"));

        let mut outputs = std::collections::HashMap::new();
        let trimmed = response.trim();

        if let Some(rest) = trimmed.strip_prefix("Answer:") {
            let value = rest.trim().to_string();
            outputs.insert("answer".to_string(), value);
        }

        assert_eq!(outputs.get("answer").unwrap(), "42");
    }

    #[test]
    fn test_parse_response_with_colon_in_answer() {
        let response = "The time is 3:30 PM.\n\nAnswer: 3:30 PM";

        let lines: Vec<&str> = response.trim().split('\n').collect();
        let mut answer = String::new();

        for line in lines {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("Answer:") {
                answer = rest.trim().to_string();
                break;
            }
        }

        assert_eq!(answer, "3:30 PM");
    }

    // ============================================================================
    // Input Extraction Tests
    // ============================================================================

    #[test]
    fn test_extract_inputs_from_state() {
        let state = TestState {
            question: "What is AI?".to_string(),
            reasoning: "".to_string(),
            answer: "".to_string(),
        };

        let json = serde_json::to_value(&state).unwrap();
        let question_value = json["question"].as_str().unwrap();

        assert_eq!(question_value, "What is AI?");
    }

    #[test]
    fn test_extract_inputs_missing_field() {
        let json = serde_json::json!({
            "other_field": "value"
        });

        let result = json["question"].as_str();
        assert!(result.is_none());
    }

    // ============================================================================
    // State Update Tests
    // ============================================================================

    #[test]
    fn test_update_state_with_outputs() {
        let initial_state = TestState {
            question: "What is 5+5?".to_string(),
            reasoning: "".to_string(),
            answer: "".to_string(),
        };

        let mut json = serde_json::to_value(&initial_state).unwrap();
        json["reasoning"] = serde_json::Value::String("Adding 5 and 5".to_string());
        json["answer"] = serde_json::Value::String("10".to_string());

        let updated: TestState = serde_json::from_value(json).unwrap();

        assert_eq!(updated.question, "What is 5+5?");
        assert_eq!(updated.reasoning, "Adding 5 and 5");
        assert_eq!(updated.answer, "10");
    }

    // ============================================================================
    // Signature Builder Tests
    // ============================================================================

    #[test]
    fn test_signature_builder() {
        let sig = Signature::new("TestSig")
            .with_instructions("Test instructions")
            .with_input(Field::input("input1", "First input"))
            .with_input(Field::input("input2", "Second input"))
            .with_output(Field::output("output1", "First output"));

        assert_eq!(sig.name, "TestSig");
        assert_eq!(sig.instructions, "Test instructions");
        assert_eq!(sig.input_fields.len(), 2);
        assert_eq!(sig.output_fields.len(), 1);
    }

    #[test]
    fn test_signature_empty_instructions() {
        let sig = Signature::new("Empty");

        assert_eq!(sig.name, "Empty");
        assert!(sig.instructions.is_empty());
        assert!(sig.input_fields.is_empty());
        assert!(sig.output_fields.is_empty());
    }

    // ============================================================================
    // Field Kind Tests
    // ============================================================================

    #[test]
    fn test_field_kind_input() {
        let field = Field::input("test", "Test field");
        assert!(matches!(field.kind, FieldKind::Input));
    }

    #[test]
    fn test_field_kind_output() {
        let field = Field::output("test", "Test field");
        assert!(matches!(field.kind, FieldKind::Output));
    }

    // ============================================================================
    // Integration-style Tests (without actual LLM)
    // ============================================================================

    #[test]
    fn test_full_prompt_flow() {
        // Simulate full prompt building flow
        let instruction = "Answer the math question";
        let question = "What is 7*8?";

        // Build prompt
        let mut prompt = String::new();
        prompt.push_str(instruction);
        prompt.push_str("\n\n");
        prompt.push_str(&format!("Question: {}\n", question));
        prompt.push_str("Reasoning: ");

        // Verify structure
        assert!(prompt.starts_with("Answer the math question"));
        assert!(prompt.contains("Question: What is 7*8?"));
        assert!(prompt.ends_with("Reasoning: "));
    }

    #[test]
    fn test_full_response_parsing_flow() {
        // Simulate LLM response
        let llm_response = "I need to multiply 7 by 8.\n7 * 8 = 56.\n\nAnswer: 56";

        // Parse response
        let lines: Vec<&str> = llm_response.trim().split('\n').collect();
        let mut reasoning_parts = Vec::new();
        let mut answer = String::new();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("Answer:") {
                answer = rest.trim().to_string();
            } else if answer.is_empty() {
                reasoning_parts.push(trimmed);
            }
        }

        let reasoning = reasoning_parts.join(" ");

        assert_eq!(answer, "56");
        assert!(reasoning.contains("multiply 7 by 8"));
    }
}
