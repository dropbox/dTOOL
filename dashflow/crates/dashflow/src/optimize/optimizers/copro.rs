// Allow clippy warnings for optimizer
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! # COPRO - Collaborative Prompt Optimizer (Original)
//!
//! > **Prefer [`crate::optimize::optimizers::copro_v2::COPROv2`] for new code.**
//! >
//! > COPROv2 offers improved performance with parallel candidate evaluation,
//! > better memory management, and enhanced prompt generation. This module is
//! > maintained for backward compatibility but receives only maintenance updates.
//!
//! COPRO is an LLM-based prompt optimizer that uses an LLM to iteratively
//! generate better instructions and output prefixes. It's particularly effective
//! for complex tasks where manual prompt engineering is difficult.
//!
//! ## Algorithm Overview
//!
//! **Initialization (Depth 0):**
//! 1. Take the current instruction from the signature
//! 2. Use LLM to generate BREADTH-1 instruction variations
//! 3. Add the original instruction as a candidate (total = BREADTH)
//! 4. Evaluate all candidates on the training set
//!
//! **Iteration (Depth 1 to DEPTH):**
//! 1. Sort evaluated candidates by score (best to worst)
//! 2. Create a "history" of attempts with scores
//! 3. Use LLM to generate BREADTH new candidates informed by history
//! 4. Evaluate new candidates
//! 5. Keep track of all evaluated candidates (no duplicates)
//! 6. After iteration, use best candidate for next iteration
//!
//! **Final Selection:**
//! - Return the signature with best instruction and prefix
//!
//! ## Adaptation from Baseline
//!
//! **Baseline (DashOptimize copro_optimizer.py, 358 lines):**
//! - Works with Module.predictors() (multi-predictor optimization)
//! - Uses dashoptimize.Predict() for LLM calls with temperature and n parameter
//! - Tracks statistics (results_best, results_latest) if track_stats=True
//! - Handles multi-predictor programs (reevaluate all with new best)
//! - Deduplicates candidates by signature equality
//!
//! **Our Version:**
//! - Works with single Signature (simpler scope)
//! - Uses ChatModel directly (no dashoptimize.Predict abstraction)
//! - Simplified statistics tracking (optional)
//! - Single-signature optimization (extend to multi-sig via GraphOptimizer)
//! - Same deduplication strategy (instruction + prefix equality)
//!
//! ## Parameters
//!
//! - **breadth**: Number of candidates to generate per iteration (default: 10)
//! - **depth**: Number of optimization iterations (default: 3)
//! - **temperature**: LLM temperature for generation (default: 1.4 - creative)
//! - **prompt_model**: Optional separate LLM for instruction generation
//!   (if None, uses the same LLM as the task)
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use dashflow::optimize::{COPRO, Signature};
//!
//! let optimizer = COPRO::builder()
//!     .breadth(10)
//!     .depth(3)
//!     .temperature(1.4)
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
//! - **Paper**: "DSPy: Compiling Declarative Language Model Calls into Self-Improving Pipelines"
//! - **Authors**: Khattab et al. (Stanford)
//! - **Link**: <https://arxiv.org/abs/2310.03714>
//! - **Published**: October 2023

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::optimize::example::Example;
use crate::optimize::signature::Signature;
use crate::optimize::telemetry::{record_optimization_complete, record_optimization_start};
use crate::Error;
use futures::future::try_join_all;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing;

// Import shared MetricFn from types module
pub use super::types::MetricFn;

/// Candidate instruction with evaluation results
#[derive(Clone, Debug)]
struct Candidate {
    instruction: String,
    prefix: String,
    score: f64,
    depth: usize,
}

/// COPRO optimizer builder
pub struct COPROBuilder {
    breadth: Option<usize>,
    depth: Option<usize>,
    temperature: Option<f64>,
    metric: Option<MetricFn>,
    track_stats: bool,
    prompt_model: Option<Arc<dyn ChatModel>>,
}

impl COPROBuilder {
    /// Create a new COPRO optimizer builder with default settings.
    pub fn new() -> Self {
        Self {
            breadth: None,
            depth: None,
            temperature: None,
            metric: None,
            track_stats: false,
            prompt_model: None,
        }
    }

    /// Set the number of candidates to generate per iteration
    pub fn breadth(mut self, breadth: usize) -> Self {
        self.breadth = Some(breadth);
        self
    }

    /// Set the number of optimization iterations
    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = Some(depth);
        self
    }

    /// Set the LLM temperature for generation (higher = more creative)
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the evaluation metric
    pub fn metric(mut self, metric: MetricFn) -> Self {
        self.metric = Some(metric);
        self
    }

    /// Enable statistics tracking
    pub fn track_stats(mut self, track: bool) -> Self {
        self.track_stats = track;
        self
    }

    /// Set a separate LLM for instruction generation
    pub fn prompt_model(mut self, model: Arc<dyn ChatModel>) -> Self {
        self.prompt_model = Some(model);
        self
    }

    /// Build the COPRO optimizer
    pub fn build(self) -> Result<COPRO, Error> {
        let breadth = self.breadth.unwrap_or(10);
        if breadth <= 1 {
            return Err(Error::Validation(
                "Breadth must be greater than 1".to_string(),
            ));
        }

        Ok(COPRO {
            breadth,
            depth: self.depth.unwrap_or(3),
            temperature: self.temperature.unwrap_or(1.4),
            metric: self
                .metric
                .ok_or_else(|| Error::Validation("Metric is required".to_string()))?,
            track_stats: self.track_stats,
            prompt_model: self.prompt_model,
        })
    }
}

impl Default for COPROBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// COPRO optimizer
#[derive(Clone)]
pub struct COPRO {
    breadth: usize,
    depth: usize,
    temperature: f64,
    metric: MetricFn,
    /// M-893: Field retained for API compatibility and future detailed statistics logging.
    /// Currently not used internally, but available for users who enable `track_stats(true)`
    /// to know the optimizer was configured with statistics tracking intent.
    #[allow(dead_code)]
    track_stats: bool,
    prompt_model: Option<Arc<dyn ChatModel>>,
}

impl std::fmt::Debug for COPRO {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("COPRO")
            .field("breadth", &self.breadth)
            .field("depth", &self.depth)
            .field("temperature", &self.temperature)
            .field("track_stats", &self.track_stats)
            .field("metric", &"<function>")
            .field("prompt_model", &self.prompt_model.is_some())
            .finish()
    }
}

impl COPRO {
    /// Create a new COPRO builder
    pub fn builder() -> COPROBuilder {
        COPROBuilder::new()
    }

    /// Compile (optimize) a signature using the training set
    ///
    /// # Arguments
    ///
    /// * `signature` - The signature to optimize
    /// * `trainset` - Training examples for evaluation
    /// * `task_model` - LLM for the task (used if prompt_model not specified)
    ///
    /// # Evaluation Semantics (M-894)
    ///
    /// Candidate evaluations run in parallel using `try_join_all`. If ANY evaluation
    /// fails (e.g., LLM API error, rate limit, network timeout), the entire batch
    /// aborts and the error propagates. This fail-fast behavior is intentional:
    ///
    /// - **Consistency**: Partial evaluation sets could lead to suboptimal selection
    ///   (picking a candidate that only succeeded because better ones failed)
    /// - **Reproducibility**: With fail-fast, the same inputs produce the same
    ///   outputs (either all succeed or all fail)
    ///
    /// **Workarounds for unreliable LLM providers:**
    /// - Implement retry logic in your ChatModel wrapper
    /// - Use rate limiting / backoff in your ChatModel
    /// - Run with smaller `breadth` to reduce parallel load
    pub async fn compile(
        &self,
        signature: &Signature,
        trainset: &[Example],
        task_model: Arc<dyn ChatModel>,
    ) -> Result<Signature, Error> {
        record_optimization_start("copro");
        let start_time = Instant::now();
        let mut evaluated_candidates: HashMap<(String, String), Candidate> = HashMap::new();
        let mut initial_score = 0.0;

        // Get the LLM for instruction generation
        let prompt_model = self.prompt_model.as_ref().unwrap_or(&task_model);

        // Initialization: Generate BREADTH candidates from basic instruction
        let basic_instruction = if signature.instructions.is_empty() {
            "Solve the task".to_string()
        } else {
            signature.instructions.clone()
        };

        let basic_prefix = signature
            .output_fields
            .last()
            .map(|f| f.get_prefix())
            .unwrap_or_else(|| "Output".to_string());

        tracing::info!(
            breadth = self.breadth,
            "COPRO Initialization: Generating candidates"
        );
        tracing::debug!(instruction = %basic_instruction, "Basic instruction");

        // Generate BREADTH-1 variations using LLM
        let mut initial_instructions = Vec::new();
        let mut initial_prefixes = Vec::new();

        if self.breadth > 1 {
            let variations = self
                .generate_basic_instructions(prompt_model, &basic_instruction, self.breadth - 1)
                .await?;
            initial_instructions.extend(variations.0);
            initial_prefixes.extend(variations.1);
        }

        // Add the original instruction as a candidate
        initial_instructions.push(basic_instruction.clone());
        initial_prefixes.push(basic_prefix.clone());

        // Evaluate all initial candidates in parallel
        let eval_futures: Vec<_> = initial_instructions
            .iter()
            .zip(initial_prefixes.iter())
            .map(|(instruction, prefix)| {
                let sig = signature.clone();
                let instr = instruction.clone();
                let pref = prefix.clone();
                let ts = trainset.to_vec();
                let model = task_model.clone();
                let metric = self.metric.clone();
                async move {
                    let score =
                        Self::evaluate_candidate_static(&sig, &instr, &pref, &ts, &model, &metric)
                            .await?;
                    Ok::<_, Error>((instr, pref, score))
                }
            })
            .collect();

        let eval_results = try_join_all(eval_futures).await?;

        for (instruction, prefix, score) in eval_results {
            // Track initial score (score of the original instruction)
            if instruction == basic_instruction {
                initial_score = score;
            }
            let candidate = Candidate {
                instruction: instruction.clone(),
                prefix: prefix.clone(),
                score,
                depth: 0,
            };
            evaluated_candidates.insert((instruction, prefix), candidate);
        }

        tracing::info!(
            count = evaluated_candidates.len(),
            "Evaluated initial candidates"
        );

        // Iterative optimization
        for d in 0..self.depth {
            tracing::info!(depth = d + 1, max = self.depth, "Starting iteration");

            // Get best candidates so far
            let mut best_candidates: Vec<_> = evaluated_candidates.values().cloned().collect();
            best_candidates.sort_by(|a, b| b.score.total_cmp(&a.score));

            // Create "history" of attempts for LLM
            let attempts = self.format_attempts(&best_candidates);

            tracing::debug!(
                count = self.breadth,
                "Generating new candidates based on history"
            );

            // Generate new candidates informed by history
            let (new_instructions, new_prefixes) = self
                .generate_instructions_from_attempts(prompt_model, &attempts, self.breadth)
                .await?;

            // Filter out duplicates and evaluate new candidates in parallel
            let new_candidates: Vec<_> = new_instructions
                .iter()
                .zip(new_prefixes.iter())
                .filter(|(instruction, prefix)| {
                    let key = ((*instruction).clone(), (*prefix).clone());
                    if evaluated_candidates.contains_key(&key) {
                        tracing::debug!("Skipping duplicate candidate");
                        false
                    } else {
                        true
                    }
                })
                .collect();

            let eval_futures: Vec<_> = new_candidates
                .iter()
                .map(|(instruction, prefix)| {
                    let sig = signature.clone();
                    let instr = (*instruction).clone();
                    let pref = (*prefix).clone();
                    let ts = trainset.to_vec();
                    let model = task_model.clone();
                    let metric = self.metric.clone();
                    let depth = d + 1;
                    async move {
                        let score = Self::evaluate_candidate_static(
                            &sig, &instr, &pref, &ts, &model, &metric,
                        )
                        .await?;
                        Ok::<_, Error>((instr, pref, score, depth))
                    }
                })
                .collect();

            let eval_results = try_join_all(eval_futures).await?;

            for (instruction, prefix, score, depth) in eval_results {
                let candidate = Candidate {
                    instruction: instruction.clone(),
                    prefix: prefix.clone(),
                    score,
                    depth,
                };
                evaluated_candidates.insert((instruction, prefix), candidate);
            }

            tracing::debug!(
                count = evaluated_candidates.len(),
                "Total evaluated candidates"
            );
        }

        // Find best candidate
        let best_candidate = evaluated_candidates
            .values()
            .max_by(|a, b| a.score.total_cmp(&b.score))
            .ok_or_else(|| Error::Validation("No candidates evaluated".to_string()))?;

        tracing::info!(
            score = best_candidate.score,
            depth = best_candidate.depth,
            instruction = %best_candidate.instruction,
            prefix = %best_candidate.prefix,
            "Best candidate found"
        );

        // Create optimized signature
        let mut optimized = signature.clone();
        optimized.instructions = best_candidate.instruction.clone();

        // Update the last output field's prefix
        if let Some(last_field) = optimized.output_fields.last_mut() {
            last_field.prefix = Some(best_candidate.prefix.clone());
        }

        let duration = start_time.elapsed().as_secs_f64();
        let total_candidates = evaluated_candidates.len() as u64;

        record_optimization_complete(
            "copro",
            self.depth as u64, // iterations
            total_candidates,
            initial_score,
            best_candidate.score,
            duration,
        );

        tracing::info!(duration_secs = %format!("{:.2}", duration), "Optimization completed");

        Ok(optimized)
    }

    /// Generate basic instruction variations (Depth 0)
    async fn generate_basic_instructions(
        &self,
        llm: &Arc<dyn ChatModel>,
        basic_instruction: &str,
        n: usize,
    ) -> Result<(Vec<String>, Vec<String>), Error> {
        let prompt = format!(
            r#"You are an instruction optimizer for large language models. I will give you a ``signature`` of fields (inputs and outputs) in English. Your task is to propose an instruction that will lead a good language model to perform the task well. Don't be afraid to be creative.

Basic Instruction: {}

Generate {} alternative instructions that might work better. For each, also provide a short prefix that will help the model start solving the task.

Format your response as:
INSTRUCTION 1: <instruction>
PREFIX 1: <prefix>

INSTRUCTION 2: <instruction>
PREFIX 2: <prefix>

...and so on."#,
            basic_instruction, n
        );

        let messages = vec![Message::human(prompt)];
        let result = llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "COPRO".to_string(),
                source: Box::new(e),
            })?;

        let response = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "COPRO".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();

        // Parse the response
        self.parse_instruction_prefix_pairs(&response, n)
    }

    /// Generate instruction variations informed by past attempts
    async fn generate_instructions_from_attempts(
        &self,
        llm: &Arc<dyn ChatModel>,
        attempts: &str,
        n: usize,
    ) -> Result<(Vec<String>, Vec<String>), Error> {
        let prompt = format!(
            r#"You are an instruction optimizer for large language models. I will give some task instructions I've tried, along with their corresponding validation scores. The instructions are arranged in increasing order based on their scores, where higher scores indicate better quality.

Your task is to propose a new instruction that will lead a good language model to perform the task even better. Don't be afraid to be creative.

Previous Attempts:
{}

Generate {} new instructions that improve upon these attempts. For each, also provide a short prefix that will help the model start solving the task.

Format your response as:
INSTRUCTION 1: <instruction>
PREFIX 1: <prefix>

INSTRUCTION 2: <instruction>
PREFIX 2: <prefix>

...and so on."#,
            attempts, n
        );

        let messages = vec![Message::human(prompt)];
        let result = llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "COPRO".to_string(),
                source: Box::new(e),
            })?;

        let response = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "COPRO".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();

        // Parse the response
        self.parse_instruction_prefix_pairs(&response, n)
    }

    /// Parse instruction/prefix pairs from LLM response
    fn parse_instruction_prefix_pairs(
        &self,
        response: &str,
        expected: usize,
    ) -> Result<(Vec<String>, Vec<String>), Error> {
        let mut instructions = Vec::new();
        let mut prefixes = Vec::new();

        let lines: Vec<&str> = response.lines().collect();
        let mut i = 0;

        while i < lines.len() && instructions.len() < expected {
            let line = lines[i].trim();

            // Look for INSTRUCTION N:
            if line.to_uppercase().starts_with("INSTRUCTION") {
                if let Some(colon_pos) = line.find(':') {
                    let instruction = line[colon_pos + 1..].trim().to_string();
                    if !instruction.is_empty() {
                        // Look for PREFIX on next line
                        if i + 1 < lines.len() {
                            let next_line = lines[i + 1].trim();
                            if next_line.to_uppercase().starts_with("PREFIX") {
                                if let Some(prefix_colon) = next_line.find(':') {
                                    let prefix = next_line[prefix_colon + 1..].trim().to_string();
                                    if !prefix.is_empty() {
                                        instructions.push(instruction);
                                        prefixes.push(prefix);
                                        i += 2;
                                        continue;
                                    }
                                }
                            }
                        }
                        // If no prefix found, use default
                        instructions.push(instruction);
                        prefixes.push("Answer".to_string());
                    }
                }
            }
            i += 1;
        }

        // If we didn't get enough, add generic fallbacks
        while instructions.len() < expected {
            instructions.push("Solve the task carefully and accurately.".to_string());
            prefixes.push("Answer".to_string());
        }

        Ok((instructions, prefixes))
    }

    /// Format attempts as a string for LLM prompt
    fn format_attempts(&self, candidates: &[Candidate]) -> String {
        let mut attempts = String::new();
        let num_to_show = std::cmp::min(candidates.len(), self.breadth);

        for (i, candidate) in candidates.iter().take(num_to_show).enumerate() {
            attempts.push_str(&format!(
                "Instruction #{}: {}\n",
                i + 1,
                candidate.instruction
            ));
            attempts.push_str(&format!("Prefix #{}: {}\n", i + 1, candidate.prefix));
            attempts.push_str(&format!(
                "Resulting Score #{}: {:.4}\n\n",
                i + 1,
                candidate.score
            ));
        }

        attempts
    }

    /// Evaluate a candidate instruction + prefix on the training set (static version for parallel use)
    async fn evaluate_candidate_static(
        signature: &Signature,
        instruction: &str,
        prefix: &str,
        trainset: &[Example],
        llm: &Arc<dyn ChatModel>,
        metric: &MetricFn,
    ) -> Result<f64, Error> {
        // Create a temporary signature with this instruction/prefix
        let mut temp_sig = signature.clone();
        temp_sig.instructions = instruction.to_string();
        if let Some(last_field) = temp_sig.output_fields.last_mut() {
            last_field.prefix = Some(prefix.to_string());
        }

        // Evaluate on trainset in parallel
        let eval_futures: Vec<_> = trainset
            .iter()
            .map(|example| {
                let sig = temp_sig.clone();
                let ex = example.clone();
                let model = llm.clone();
                async move {
                    // Build prompt
                    let prompt = Self::build_prompt_static(&sig, &ex);

                    // Call LLM
                    let messages = vec![Message::human(prompt)];
                    let result = model
                        .generate(&messages, None, None, None, None)
                        .await
                        .map_err(|e| Error::NodeExecution {
                            node: "COPRO".to_string(),
                            source: Box::new(e),
                        })?;

                    let response = result
                        .generations
                        .first()
                        .ok_or_else(|| Error::NodeExecution {
                            node: "COPRO".to_string(),
                            source: Box::new(Error::Generic(
                                "LLM returned empty response".to_string(),
                            )),
                        })?
                        .message
                        .content()
                        .as_text();

                    // Parse output
                    let prediction = Self::parse_output_static(&sig, &response, &ex)?;

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

    /// Build prompt from signature and example inputs (static version for parallel use)
    fn build_prompt_static(signature: &Signature, example: &Example) -> String {
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

    /// Parse LLM output into an Example (static version for parallel use)
    fn parse_output_static(
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
            .with_instructions("Answer the question concisely")
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
    fn test_copro_builder() {
        let result = COPRO::builder()
            .breadth(5)
            .depth(2)
            .temperature(1.0)
            .metric(exact_match_metric())
            .build();

        assert!(result.is_ok());
        let copro = result.unwrap();
        assert_eq!(copro.breadth, 5);
        assert_eq!(copro.depth, 2);
        assert_eq!(copro.temperature, 1.0);
    }

    #[test]
    fn test_copro_builder_validates_breadth() {
        let result = COPRO::builder()
            .breadth(1) // Invalid!
            .metric(exact_match_metric())
            .build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Breadth must be greater than 1"));
    }

    #[test]
    fn test_copro_builder_requires_metric() {
        let result = COPRO::builder().breadth(5).build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Metric is required"));
    }

    #[test]
    fn test_parse_instruction_prefix_pairs() {
        let copro = COPRO::builder()
            .breadth(3)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        let response = r#"
INSTRUCTION 1: Provide a clear and accurate answer to the question.
PREFIX 1: Answer

INSTRUCTION 2: Think carefully and respond with the correct information.
PREFIX 2: Response

INSTRUCTION 3: Use your knowledge to give the best answer.
PREFIX 3: Result
        "#;

        let (instructions, prefixes) = copro.parse_instruction_prefix_pairs(response, 3).unwrap();

        assert_eq!(instructions.len(), 3);
        assert_eq!(prefixes.len(), 3);
        assert_eq!(
            instructions[0],
            "Provide a clear and accurate answer to the question."
        );
        assert_eq!(prefixes[0], "Answer");
        assert_eq!(
            instructions[1],
            "Think carefully and respond with the correct information."
        );
        assert_eq!(prefixes[1], "Response");
    }

    #[test]
    fn test_format_attempts() {
        let copro = COPRO::builder()
            .breadth(3)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        let candidates = vec![
            Candidate {
                instruction: "Answer carefully".to_string(),
                prefix: "Answer".to_string(),
                score: 0.9,
                depth: 0,
            },
            Candidate {
                instruction: "Provide accurate response".to_string(),
                prefix: "Response".to_string(),
                score: 0.85,
                depth: 0,
            },
        ];

        let attempts = copro.format_attempts(&candidates);

        assert!(attempts.contains("Instruction #1: Answer carefully"));
        assert!(attempts.contains("Prefix #1: Answer"));
        assert!(attempts.contains("Resulting Score #1: 0.9000"));
        assert!(attempts.contains("Instruction #2: Provide accurate response"));
    }

    #[tokio::test]
    #[ignore = "requires API key"]
    async fn test_copro_compile_basic() {
        let signature = create_test_signature();
        let trainset = create_test_examples();

        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));

        let copro = COPRO::builder()
            .breadth(3)
            .depth(1)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        let optimized = copro.compile(&signature, &trainset, llm).await;

        assert!(optimized.is_ok());
        let opt_sig = optimized.unwrap();

        // Instructions should be different from original (optimized)
        println!("Original: {}", signature.instructions);
        println!("Optimized: {}", opt_sig.instructions);
    }
}
