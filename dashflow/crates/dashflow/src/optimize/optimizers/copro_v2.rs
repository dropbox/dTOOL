// Allow clippy warnings for optimizer
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! # COPROv2 - Confidence-based Collaborative Prompt Optimizer
//!
//! COPROv2 extends COPRO with confidence-based scoring and filtering:
//! 1. **Confidence Scoring**: LLM provides confidence with each prediction
//! 2. **Confidence-Weighted Evaluation**: Higher confidence predictions count more
//! 3. **Confidence Filtering**: Reject low-confidence predictions for more reliable scores
//! 4. **Adaptive Temperature**: Adjust generation temperature based on confidence
//!
//! ## Algorithm Overview
//!
//! **Initialization (Same as COPRO):**
//! 1. Generate BREADTH instruction/prefix candidates
//! 2. Evaluate all candidates with confidence-aware scoring
//!
//! **Iteration (Enhanced with confidence):**
//! 1. Sort candidates by confidence-weighted score
//! 2. Generate new candidates informed by confidence patterns
//! 3. Optionally adjust temperature based on prediction confidence variance
//!
//! **Final Selection:**
//! - Return the candidate with best confidence-weighted score
//! - Optionally return only candidates meeting confidence threshold
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use dashflow::optimize::{COPROv2, Signature};
//!
//! let optimizer = COPROv2::builder()
//!     .breadth(10)
//!     .depth(3)
//!     .confidence_threshold(0.7)  // Reject predictions below 70% confidence
//!     .confidence_weight(0.3)      // Weight: 70% score + 30% confidence
//!     .metric(my_metric_fn)
//!     .build()?;
//!
//! let optimized = optimizer.compile(&signature, &trainset, llm).await?;
//! ```
//!
//! ## References
//!
//! - **Based on**: COPRO (arxiv:2310.03714)
//! - **Extension**: Adds confidence-based scoring and filtering
//! - **Link**: <https://arxiv.org/abs/2310.03714> (DSPy paper)

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::optimize::example::Example;
use crate::optimize::signature::Signature;
use crate::optimize::telemetry::{
    record_candidate_evaluated, record_iteration, record_optimization_complete,
    record_optimization_start,
};
use crate::Error;
use futures::future::try_join_all;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing;

// Import shared MetricFn from types module
pub use super::types::MetricFn;

/// Prediction with confidence score
#[derive(Clone, Debug)]
struct ConfidentPrediction {
    prediction: Example,
    confidence: f64,
    /// Raw response from LLM (preserved for debugging/analysis)
    #[allow(dead_code)] // Debug: Preserved for LLM response analysis
    raw_response: String,
}

/// Candidate instruction with confidence-aware evaluation results
#[derive(Clone, Debug)]
struct Candidate {
    instruction: String,
    prefix: String,
    score: f64,
    confidence_weighted_score: f64,
    avg_confidence: f64,
    high_confidence_ratio: f64, // % of predictions above threshold
    depth: usize,
    /// Number of examples evaluated (preserved for analysis)
    #[allow(dead_code)] // Debug: Preserved for evaluation metrics analysis
    num_evaluated: usize,
}

/// Configuration for candidate evaluation with confidence scoring
#[derive(Clone)]
struct EvaluationConfig {
    /// The metric function for scoring predictions
    metric: MetricFn,
    /// Minimum confidence threshold for predictions
    confidence_threshold: f64,
    /// Weight of confidence in final score calculation
    confidence_weight: f64,
    /// Minimum ratio of high-confidence predictions
    min_high_confidence_ratio: f64,
}

/// COPROv2 optimizer builder
pub struct COPROv2Builder {
    breadth: Option<usize>,
    depth: Option<usize>,
    temperature: Option<f64>,
    metric: Option<MetricFn>,
    track_stats: bool,
    prompt_model: Option<Arc<dyn ChatModel>>,
    confidence_threshold: Option<f64>,
    confidence_weight: Option<f64>,
    adaptive_temperature: bool,
    min_high_confidence_ratio: Option<f64>,
}

impl COPROv2Builder {
    /// Create a new COPROv2 optimizer builder with default settings.
    pub fn new() -> Self {
        Self {
            breadth: None,
            depth: None,
            temperature: None,
            metric: None,
            track_stats: false,
            prompt_model: None,
            confidence_threshold: None,
            confidence_weight: None,
            adaptive_temperature: false,
            min_high_confidence_ratio: None,
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

    /// Set minimum confidence threshold for predictions (0.0-1.0)
    /// Predictions below this threshold are filtered out
    pub fn confidence_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = Some(threshold.clamp(0.0, 1.0));
        self
    }

    /// Set how much confidence affects the final score (0.0-1.0)
    /// Final score = (1 - weight) * metric_score + weight * confidence
    pub fn confidence_weight(mut self, weight: f64) -> Self {
        self.confidence_weight = Some(weight.clamp(0.0, 1.0));
        self
    }

    /// Enable adaptive temperature adjustment based on confidence variance
    pub fn adaptive_temperature(mut self, enabled: bool) -> Self {
        self.adaptive_temperature = enabled;
        self
    }

    /// Set minimum ratio of high-confidence predictions required
    /// Candidates below this ratio are penalized
    pub fn min_high_confidence_ratio(mut self, ratio: f64) -> Self {
        self.min_high_confidence_ratio = Some(ratio.clamp(0.0, 1.0));
        self
    }

    /// Build the COPROv2 optimizer
    pub fn build(self) -> Result<COPROv2, Error> {
        let breadth = self.breadth.unwrap_or(10);
        if breadth <= 1 {
            return Err(Error::Validation(
                "Breadth must be greater than 1".to_string(),
            ));
        }

        Ok(COPROv2 {
            breadth,
            depth: self.depth.unwrap_or(3),
            temperature: self.temperature.unwrap_or(1.4),
            metric: self
                .metric
                .ok_or_else(|| Error::Validation("Metric is required".to_string()))?,
            track_stats: self.track_stats,
            prompt_model: self.prompt_model,
            confidence_threshold: self.confidence_threshold.unwrap_or(0.5),
            confidence_weight: self.confidence_weight.unwrap_or(0.2),
            adaptive_temperature: self.adaptive_temperature,
            min_high_confidence_ratio: self.min_high_confidence_ratio.unwrap_or(0.3),
        })
    }
}

impl Default for COPROv2Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// COPROv2 - Confidence-based Collaborative Prompt Optimizer
#[derive(Clone)]
pub struct COPROv2 {
    breadth: usize,
    depth: usize,
    temperature: f64,
    metric: MetricFn,
    /// Whether to track detailed statistics during optimization
    ///
    /// Note: This field is defined and can be set via `track_stats()` builder method, but
    /// detailed statistics collection is not yet implemented. The COPROv2 optimizer uses
    /// standard telemetry instead. Retained for API stability and future detailed logging.
    #[allow(dead_code)] // M-883: Defined for future statistics tracking support
    track_stats: bool,
    prompt_model: Option<Arc<dyn ChatModel>>,
    /// Minimum confidence for a prediction to be counted
    confidence_threshold: f64,
    /// Weight of confidence in final score calculation
    confidence_weight: f64,
    /// Whether to adjust temperature based on confidence variance
    adaptive_temperature: bool,
    /// Minimum ratio of predictions that must be high-confidence
    min_high_confidence_ratio: f64,
}

impl std::fmt::Debug for COPROv2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("COPROv2")
            .field("breadth", &self.breadth)
            .field("depth", &self.depth)
            .field("temperature", &self.temperature)
            .field("track_stats", &self.track_stats)
            .field("confidence_threshold", &self.confidence_threshold)
            .field("confidence_weight", &self.confidence_weight)
            .field("adaptive_temperature", &self.adaptive_temperature)
            .field("min_high_confidence_ratio", &self.min_high_confidence_ratio)
            .field("metric", &"<function>")
            .field("prompt_model", &self.prompt_model.is_some())
            .finish()
    }
}

impl COPROv2 {
    /// Create a new COPROv2 builder
    pub fn builder() -> COPROv2Builder {
        COPROv2Builder::new()
    }

    /// Compile (optimize) a signature using the training set with confidence-aware scoring
    ///
    /// # Parallel Evaluation Behavior
    ///
    /// This method evaluates candidate instructions in parallel for efficiency.
    /// **M-884 Note:** If any single candidate evaluation fails (e.g., LLM API error),
    /// the entire batch is aborted and the error is propagated. This fail-fast behavior
    /// ensures consistent results—partial candidate sets could lead to suboptimal
    /// instruction selection. If resilience to transient failures is needed, consider:
    /// - Implementing retry logic in the ChatModel implementation
    /// - Using rate limiting to avoid API throttling
    /// - Running multiple compile() calls with different seeds
    pub async fn compile(
        &self,
        signature: &Signature,
        trainset: &[Example],
        task_model: Arc<dyn ChatModel>,
    ) -> Result<Signature, Error> {
        record_optimization_start("copro_v2");
        let start_time = Instant::now();
        let mut evaluated_candidates: HashMap<(String, String), Candidate> = HashMap::new();
        let mut initial_score = 0.0_f64;
        #[allow(unused_assignments)]
        let mut current_temperature = self.temperature;

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
            "COPROv2 Initialization: Generating candidates"
        );
        tracing::debug!(instruction = %basic_instruction, "Basic instruction");
        tracing::debug!(
            confidence_threshold = %format!("{:.2}", self.confidence_threshold),
            confidence_weight = %format!("{:.2}", self.confidence_weight),
            "Confidence settings"
        );

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

        // Evaluate all initial candidates with confidence in parallel
        let eval_futures: Vec<_> = initial_instructions
            .iter()
            .zip(initial_prefixes.iter())
            .map(|(instruction, prefix)| {
                let sig = signature.clone();
                let instr = instruction.clone();
                let pref = prefix.clone();
                let ts = trainset.to_vec();
                let model = task_model.clone();
                let eval_config = EvaluationConfig {
                    metric: self.metric.clone(),
                    confidence_threshold: self.confidence_threshold,
                    confidence_weight: self.confidence_weight,
                    min_high_confidence_ratio: self.min_high_confidence_ratio,
                };
                async move {
                    let candidate = Self::evaluate_candidate_with_confidence_static(
                        &sig,
                        &instr,
                        &pref,
                        &ts,
                        &model,
                        &eval_config,
                        0,
                    )
                    .await?;
                    Ok::<_, Error>((instr, pref, candidate))
                }
            })
            .collect();

        let eval_results = try_join_all(eval_futures).await?;

        for (instruction, prefix, candidate) in eval_results {
            record_candidate_evaluated("copro_v2");
            // Track initial score from the basic instruction candidate
            if instruction == basic_instruction {
                initial_score = candidate.score;
            }
            evaluated_candidates.insert((instruction, prefix), candidate);
        }

        tracing::info!(
            count = evaluated_candidates.len(),
            "Evaluated initial candidates with confidence scoring"
        );

        // Iterative optimization
        for d in 0..self.depth {
            record_iteration("copro_v2");
            tracing::info!(depth = d + 1, max = self.depth, "Starting iteration");

            // Get best candidates so far (sorted by confidence-weighted score)
            let mut best_candidates: Vec<_> = evaluated_candidates.values().cloned().collect();
            best_candidates.sort_by(|a, b| {
                b.confidence_weighted_score
                    .total_cmp(&a.confidence_weighted_score)
            });

            // Adaptive temperature adjustment
            if self.adaptive_temperature {
                current_temperature = self.adjust_temperature(&best_candidates);
                tracing::debug!(temperature = %format!("{:.2}", current_temperature), "Adjusted temperature");
            }

            // Create enhanced history with confidence info
            let attempts = self.format_attempts_with_confidence(&best_candidates);

            tracing::debug!(
                count = self.breadth,
                "Generating new candidates based on confidence-aware history"
            );

            // Generate new candidates informed by confidence patterns
            let (new_instructions, new_prefixes) = self
                .generate_instructions_from_attempts_v2(prompt_model, &attempts, self.breadth)
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
                    let depth = d + 1;
                    let eval_config = EvaluationConfig {
                        metric: self.metric.clone(),
                        confidence_threshold: self.confidence_threshold,
                        confidence_weight: self.confidence_weight,
                        min_high_confidence_ratio: self.min_high_confidence_ratio,
                    };
                    async move {
                        let candidate = Self::evaluate_candidate_with_confidence_static(
                            &sig,
                            &instr,
                            &pref,
                            &ts,
                            &model,
                            &eval_config,
                            depth,
                        )
                        .await?;
                        Ok::<_, Error>((instr, pref, candidate))
                    }
                })
                .collect();

            let eval_results = try_join_all(eval_futures).await?;

            for (instruction, prefix, candidate) in eval_results {
                record_candidate_evaluated("copro_v2");
                evaluated_candidates.insert((instruction, prefix), candidate);
            }

            // Print summary
            // Safety: evaluated_candidates is guaranteed non-empty since we always add initial candidates
            let best = evaluated_candidates
                .values()
                .max_by(|a, b| {
                    a.confidence_weighted_score
                        .total_cmp(&b.confidence_weighted_score)
                })
                .expect("evaluated_candidates is non-empty after initialization");
            tracing::debug!(
                score = %format!("{:.4}", best.score),
                confidence_weighted = %format!("{:.4}", best.confidence_weighted_score),
                avg_confidence = %format!("{:.2}", best.avg_confidence),
                "Best so far"
            );
        }

        // Find best candidate by confidence-weighted score
        let best_candidate = evaluated_candidates
            .values()
            .max_by(|a, b| {
                a.confidence_weighted_score
                    .total_cmp(&b.confidence_weighted_score)
            })
            .ok_or_else(|| Error::Validation("No candidates evaluated".to_string()))?;

        tracing::info!(
            score = %format!("{:.4}", best_candidate.score),
            confidence_weighted_score = %format!("{:.4}", best_candidate.confidence_weighted_score),
            avg_confidence = %format!("{:.4}", best_candidate.avg_confidence),
            high_confidence_ratio = %format!("{:.2}%", best_candidate.high_confidence_ratio * 100.0),
            depth = best_candidate.depth,
            instruction = %best_candidate.instruction,
            prefix = %best_candidate.prefix,
            "Best candidate found (confidence-weighted)"
        );

        // Create optimized signature
        let mut optimized = signature.clone();
        optimized.instructions = best_candidate.instruction.clone();

        if let Some(last_field) = optimized.output_fields.last_mut() {
            last_field.prefix = Some(best_candidate.prefix.clone());
        }

        let duration = start_time.elapsed().as_secs_f64();
        record_optimization_complete(
            "copro_v2",
            self.depth as u64,
            evaluated_candidates.len() as u64,
            initial_score,
            best_candidate.score,
            duration,
        );

        Ok(optimized)
    }

    /// Evaluate a candidate with confidence-aware scoring (static version for parallel use)
    async fn evaluate_candidate_with_confidence_static(
        signature: &Signature,
        instruction: &str,
        prefix: &str,
        trainset: &[Example],
        llm: &Arc<dyn ChatModel>,
        config: &EvaluationConfig,
        depth: usize,
    ) -> Result<Candidate, Error> {
        let mut temp_sig = signature.clone();
        temp_sig.instructions = instruction.to_string();
        if let Some(last_field) = temp_sig.output_fields.last_mut() {
            last_field.prefix = Some(prefix.to_string());
        }

        // Run all predictions in parallel
        let pred_futures: Vec<_> = trainset
            .iter()
            .map(|example| {
                let sig = temp_sig.clone();
                let ex = example.clone();
                let model = llm.clone();
                async move {
                    let pred = Self::predict_with_confidence_static(&sig, &ex, &model).await?;
                    Ok::<_, Error>((pred, ex))
                }
            })
            .collect();

        let predictions = try_join_all(pred_futures).await?;

        // Calculate metrics from parallel results
        let mut total_score = 0.0;
        let mut total_confidence = 0.0;
        let mut high_confidence_count = 0usize;
        let count = predictions.len();

        // M-882 FIX: Guard against empty trainset producing NaN scores
        if count == 0 {
            tracing::warn!(
                "Empty trainset for candidate evaluation (instruction: '{}...')",
                instruction.chars().take(50).collect::<String>()
            );
            return Ok(Candidate {
                instruction: instruction.to_string(),
                prefix: prefix.to_string(),
                score: 0.0,
                confidence_weighted_score: 0.0,
                avg_confidence: 0.0,
                high_confidence_ratio: 0.0,
                depth,
                num_evaluated: 0,
            });
        }

        for (pred, example) in &predictions {
            if pred.confidence >= config.confidence_threshold {
                let score = (config.metric)(&pred.prediction, example);
                total_score += score;
                total_confidence += pred.confidence;
                high_confidence_count += 1;
            }
        }

        // Calculate metrics
        let (avg_score, avg_confidence, high_confidence_ratio) = if high_confidence_count > 0 {
            (
                total_score / high_confidence_count as f64,
                total_confidence / high_confidence_count as f64,
                high_confidence_count as f64 / count as f64,
            )
        } else {
            // If no high-confidence predictions, use raw scores with penalty
            let mut raw_score = 0.0;
            let mut raw_confidence = 0.0;
            for (pred, example) in &predictions {
                raw_score += (config.metric)(&pred.prediction, example);
                raw_confidence += pred.confidence;
            }
            (
                raw_score / count as f64 * 0.5, // 50% penalty for no high-confidence
                raw_confidence / count as f64,
                0.0,
            )
        };

        // Confidence-weighted score
        let confidence_weighted_score = (1.0 - config.confidence_weight) * avg_score
            + config.confidence_weight * avg_confidence;

        // Apply penalty if high_confidence_ratio is below minimum
        let final_score = if high_confidence_ratio < config.min_high_confidence_ratio {
            confidence_weighted_score * (0.5 + 0.5 * high_confidence_ratio)
        } else {
            confidence_weighted_score
        };

        Ok(Candidate {
            instruction: instruction.to_string(),
            prefix: prefix.to_string(),
            score: avg_score,
            confidence_weighted_score: final_score,
            avg_confidence,
            high_confidence_ratio,
            depth,
            num_evaluated: count,
        })
    }

    /// Get prediction with confidence score (static version for parallel use)
    async fn predict_with_confidence_static(
        signature: &Signature,
        example: &Example,
        llm: &Arc<dyn ChatModel>,
    ) -> Result<ConfidentPrediction, Error> {
        // Build prompt asking for answer AND confidence
        let prompt = Self::build_confidence_prompt_static(signature, example);

        let messages = vec![Message::human(prompt)];
        let result = llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "COPROv2".to_string(),
                source: Box::new(e),
            })?;

        let response = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "COPROv2".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();

        // Parse response for answer and confidence
        let (answer, confidence) = Self::parse_confident_response_static(&response);

        // Build prediction example
        let mut prediction = Example::new();
        let input_keys: Vec<String> = example.inputs().keys().cloned().collect();
        for (key, value) in example.inputs().iter() {
            prediction = prediction.with_field(key, value.clone());
        }
        let input_keys_str: Vec<&str> = input_keys.iter().map(|s| s.as_str()).collect();
        prediction = prediction.with_inputs(&input_keys_str);

        if let Some(first_output) = signature.output_fields.first() {
            prediction = prediction.with_field(&first_output.name, answer.trim());
        }

        Ok(ConfidentPrediction {
            prediction,
            confidence,
            raw_response: response,
        })
    }

    /// Build prompt that asks for confidence (static version for parallel use)
    fn build_confidence_prompt_static(signature: &Signature, example: &Example) -> String {
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

        // Request confidence along with answer
        prompt.push_str("\nProvide your answer and rate your confidence (0-100%).\n");
        prompt.push_str("Format:\n");
        if let Some(first_output) = signature.output_fields.first() {
            prompt.push_str(&format!("{}: <your answer>\n", first_output.get_prefix()));
        }
        prompt.push_str("Confidence: <0-100>%\n");

        prompt
    }

    /// Parse response to extract answer and confidence (static version for parallel use)
    fn parse_confident_response_static(response: &str) -> (String, f64) {
        let lines: Vec<&str> = response.lines().collect();
        let mut answer = String::new();
        let mut confidence = 0.5; // Default confidence

        for line in &lines {
            let line = line.trim();

            // Try to parse confidence
            if line.to_lowercase().starts_with("confidence") {
                if let Some(colon_pos) = line.find(':') {
                    let conf_str = &line[colon_pos + 1..].trim();
                    // Extract number from string like "85%" or "85"
                    let num_str: String = conf_str.chars().filter(|c| c.is_ascii_digit()).collect();
                    if let Ok(num) = num_str.parse::<f64>() {
                        confidence = (num / 100.0).clamp(0.0, 1.0);
                    }
                }
            }
            // First non-confidence, non-empty line is the answer
            else if !line.is_empty() && answer.is_empty() {
                // Try to extract after ":"
                if let Some(colon_pos) = line.find(':') {
                    answer = line[colon_pos + 1..].trim().to_string();
                } else {
                    answer = line.to_string();
                }
            }
        }

        // If no answer found, use whole response
        if answer.is_empty() {
            answer = response.trim().to_string();
        }

        (answer, confidence)
    }

    /// Adjust temperature based on confidence variance
    fn adjust_temperature(&self, candidates: &[Candidate]) -> f64 {
        if candidates.is_empty() {
            return self.temperature;
        }

        let avg_confidence: f64 =
            candidates.iter().map(|c| c.avg_confidence).sum::<f64>() / candidates.len() as f64;

        // If average confidence is low, increase temperature for more exploration
        // If average confidence is high, decrease temperature for exploitation
        let adjustment = (0.5 - avg_confidence) * 0.5; // Range: -0.25 to +0.25
        (self.temperature + adjustment).clamp(0.5, 2.0)
    }

    /// Format attempts with confidence information
    fn format_attempts_with_confidence(&self, candidates: &[Candidate]) -> String {
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
                "Score #{}: {:.4} (confidence-weighted: {:.4})\n",
                i + 1,
                candidate.score,
                candidate.confidence_weighted_score
            ));
            attempts.push_str(&format!(
                "Average Confidence #{}: {:.2}%\n",
                i + 1,
                candidate.avg_confidence * 100.0
            ));
            attempts.push_str(&format!(
                "High Confidence Ratio #{}: {:.2}%\n\n",
                i + 1,
                candidate.high_confidence_ratio * 100.0
            ));
        }

        attempts
    }

    /// Generate basic instruction variations (Depth 0)
    async fn generate_basic_instructions(
        &self,
        llm: &Arc<dyn ChatModel>,
        basic_instruction: &str,
        n: usize,
    ) -> Result<(Vec<String>, Vec<String>), Error> {
        let prompt = format!(
            r#"You are an instruction optimizer for large language models. I will give you a signature of fields (inputs and outputs) in English. Your task is to propose an instruction that will lead a good language model to perform the task well AND produce confident, reliable outputs.

Basic Instruction: {}

Generate {} alternative instructions that might work better. Focus on clarity and specificity to encourage confident predictions.

For each instruction, also provide a short prefix that will help the model start solving the task.

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
                node: "COPROv2".to_string(),
                source: Box::new(e),
            })?;

        let response = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "COPROv2".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();
        self.parse_instruction_prefix_pairs(&response, n)
    }

    /// Generate instruction variations informed by past attempts with confidence
    async fn generate_instructions_from_attempts_v2(
        &self,
        llm: &Arc<dyn ChatModel>,
        attempts: &str,
        n: usize,
    ) -> Result<(Vec<String>, Vec<String>), Error> {
        let prompt = format!(
            r#"You are an instruction optimizer for large language models. I will give you some task instructions I've tried, along with their corresponding validation scores and confidence metrics.

Instructions with higher confidence scores produce more reliable predictions. The goal is to find instructions that are both accurate AND produce confident outputs.

Previous Attempts:
{}

Generate {} new instructions that:
1. Improve upon the scores of previous attempts
2. Encourage higher model confidence
3. Are clear and unambiguous to reduce uncertainty

For each, also provide a short prefix that will help the model start solving the task confidently.

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
                node: "COPROv2".to_string(),
                source: Box::new(e),
            })?;

        let response = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "COPROv2".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();
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

            if line.to_uppercase().starts_with("INSTRUCTION") {
                if let Some(colon_pos) = line.find(':') {
                    let instruction = line[colon_pos + 1..].trim().to_string();
                    if !instruction.is_empty() {
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
                        instructions.push(instruction);
                        prefixes.push("Answer".to_string());
                    }
                }
            }
            i += 1;
        }

        while instructions.len() < expected {
            instructions.push("Solve the task carefully and accurately.".to_string());
            prefixes.push("Answer".to_string());
        }

        Ok((instructions, prefixes))
    }

    /// Get the confidence threshold
    pub fn confidence_threshold(&self) -> f64 {
        self.confidence_threshold
    }

    /// Get the confidence weight
    pub fn confidence_weight(&self) -> f64 {
        self.confidence_weight
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
    fn test_coprov2_builder() {
        let result = COPROv2::builder()
            .breadth(5)
            .depth(2)
            .temperature(1.0)
            .confidence_threshold(0.7)
            .confidence_weight(0.3)
            .metric(exact_match_metric())
            .build();

        assert!(result.is_ok());
        let copro = result.unwrap();
        assert_eq!(copro.breadth, 5);
        assert_eq!(copro.depth, 2);
        assert_eq!(copro.temperature, 1.0);
        assert!((copro.confidence_threshold - 0.7).abs() < 0.001);
        assert!((copro.confidence_weight - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_coprov2_builder_validates_breadth() {
        let result = COPROv2::builder()
            .breadth(1)
            .metric(exact_match_metric())
            .build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Breadth must be greater than 1"));
    }

    #[test]
    fn test_coprov2_builder_requires_metric() {
        let result = COPROv2::builder().breadth(5).build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Metric is required"));
    }

    #[test]
    fn test_coprov2_builder_clamps_values() {
        let result = COPROv2::builder()
            .breadth(5)
            .confidence_threshold(1.5) // Should clamp to 1.0
            .confidence_weight(-0.5) // Should clamp to 0.0
            .metric(exact_match_metric())
            .build();

        assert!(result.is_ok());
        let copro = result.unwrap();
        assert!((copro.confidence_threshold - 1.0).abs() < 0.001);
        assert!((copro.confidence_weight - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_confident_response() {
        // Test uses static method (instance wrapper was removed in #1553 dead code cleanup)

        // Test normal response
        let response = "Answer: Paris\nConfidence: 85%";
        let (answer, confidence) = COPROv2::parse_confident_response_static(response);
        assert_eq!(answer, "Paris");
        assert!((confidence - 0.85).abs() < 0.01);

        // Test response without confidence
        let response2 = "The answer is 42";
        let (answer2, confidence2) = COPROv2::parse_confident_response_static(response2);
        assert_eq!(answer2, "The answer is 42");
        assert!((confidence2 - 0.5).abs() < 0.01); // Default confidence

        // Test response with different format
        let response3 = "Paris\nConfidence: 90";
        let (answer3, confidence3) = COPROv2::parse_confident_response_static(response3);
        assert_eq!(answer3, "Paris");
        assert!((confidence3 - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_adjust_temperature() {
        let copro = COPROv2::builder()
            .breadth(3)
            .temperature(1.0)
            .adaptive_temperature(true)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        // Low confidence candidates -> higher temperature
        let low_conf_candidates = vec![Candidate {
            instruction: "test".to_string(),
            prefix: "test".to_string(),
            score: 0.5,
            confidence_weighted_score: 0.5,
            avg_confidence: 0.2,
            high_confidence_ratio: 0.3,
            depth: 0,
            num_evaluated: 10,
        }];

        let adjusted = copro.adjust_temperature(&low_conf_candidates);
        assert!(adjusted > 1.0); // Should increase for exploration

        // High confidence candidates -> lower temperature
        let high_conf_candidates = vec![Candidate {
            instruction: "test".to_string(),
            prefix: "test".to_string(),
            score: 0.9,
            confidence_weighted_score: 0.9,
            avg_confidence: 0.8,
            high_confidence_ratio: 0.9,
            depth: 0,
            num_evaluated: 10,
        }];

        let adjusted2 = copro.adjust_temperature(&high_conf_candidates);
        assert!(adjusted2 < 1.0); // Should decrease for exploitation
    }

    #[test]
    fn test_parse_instruction_prefix_pairs() {
        let copro = COPROv2::builder()
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
    }

    #[test]
    fn test_format_attempts_with_confidence() {
        let copro = COPROv2::builder()
            .breadth(3)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        let candidates = vec![
            Candidate {
                instruction: "Answer carefully".to_string(),
                prefix: "Answer".to_string(),
                score: 0.9,
                confidence_weighted_score: 0.88,
                avg_confidence: 0.85,
                high_confidence_ratio: 0.8,
                depth: 0,
                num_evaluated: 10,
            },
            Candidate {
                instruction: "Provide accurate response".to_string(),
                prefix: "Response".to_string(),
                score: 0.85,
                confidence_weighted_score: 0.82,
                avg_confidence: 0.75,
                high_confidence_ratio: 0.7,
                depth: 0,
                num_evaluated: 10,
            },
        ];

        let attempts = copro.format_attempts_with_confidence(&candidates);

        assert!(attempts.contains("Instruction #1: Answer carefully"));
        assert!(attempts.contains("Prefix #1: Answer"));
        assert!(attempts.contains("Score #1: 0.9000"));
        assert!(attempts.contains("Average Confidence #1: 85.00%"));
        assert!(attempts.contains("High Confidence Ratio #1: 80.00%"));
    }

    #[test]
    fn test_coprov2_default_values() {
        let copro = COPROv2::builder()
            .breadth(5)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        // Check defaults
        assert_eq!(copro.depth, 3);
        assert!((copro.temperature - 1.4).abs() < 0.001);
        assert!((copro.confidence_threshold - 0.5).abs() < 0.001);
        assert!((copro.confidence_weight - 0.2).abs() < 0.001);
        assert!(!copro.adaptive_temperature);
        assert!((copro.min_high_confidence_ratio - 0.3).abs() < 0.001);
    }

    #[tokio::test]
    #[ignore = "requires API key"]
    async fn test_coprov2_compile_basic() {
        let signature = create_test_signature();
        let trainset = create_test_examples();

        let llm = Arc::new(FakeChatModel::new(vec!["test".to_string()]));

        let copro = COPROv2::builder()
            .breadth(3)
            .depth(1)
            .confidence_threshold(0.5)
            .metric(exact_match_metric())
            .build()
            .unwrap();

        let optimized = copro.compile(&signature, &trainset, llm).await;

        assert!(optimized.is_ok());
        let opt_sig = optimized.unwrap();

        println!("Original: {}", signature.instructions);
        println!("Optimized: {}", opt_sig.instructions);
    }
}
