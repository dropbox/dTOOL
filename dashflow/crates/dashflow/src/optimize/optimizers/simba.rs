// Allow clippy warnings for optimizer
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! SIMBA (Stochastic Introspective Mini-Batch Ascent) Optimizer
//!
//! SIMBA is an advanced optimizer that uses Monte Carlo search with LLM introspection.
//! It analyzes program performance and generates improvement rules by comparing successful
//! and unsuccessful execution trajectories.
//!
//! ## Algorithm Overview
//!
//! SIMBA works by:
//! 1. Sampling mini-batches of training examples
//! 2. Generating multiple program trajectories per example (with different seeds/temperatures)
//! 3. Identifying examples with high output variability (max-min score gap)
//! 4. For those challenging examples, either:
//!    - Adding successful demonstrations (append_a_demo)
//!    - Generating self-reflective improvement rules (append_a_rule)
//! 5. Maintaining a pool of candidate programs, using softmax sampling
//! 6. Repeating for multiple iterations
//!
//! ## Key Features
//!
//! - **Variability-based selection**: Focuses on examples where different program
//!   variants produce different scores (high learning signal)
//! - **Program pool**: Maintains multiple programs, samples from them probabilistically
//! - **Stochastic dropout**: Randomly drops demos to make room for new ones
//! - **Two improvement strategies**: Demos (concrete examples) + Rules (abstract guidance)
//! - **Softmax sampling**: Higher-scoring programs are more likely to be selected
//!
//! ## ExecutionTrace Integration
//!
//! This optimizer now uses unified telemetry via `ExecutionTrace`:
//! - `SimbaOutput.trace` contains an optional `ExecutionTrace` for each execution
//! - Traces capture state snapshots, timing, and execution metadata
//! - Can be converted to training data using `ExecutionTrace::to_examples()`
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use dashflow::optimize::optimizers::SIMBA;
//!
//! let simba = SIMBA::new()
//!     .with_bsize(32)
//!     .with_max_steps(8)
//!     .with_num_candidates(6)
//!     .with_max_demos(4);
//!
//! let optimized = simba.optimize(node, &trainset, &metric).await?;
//! ```
//!
//! ## Adapted from DashOpt
//!
//! Source: ~/dsp_rs/dashopt_teleprompt/src/simba.rs (31KB)
//! Source: ~/dsp_rs/dashopt_teleprompt/src/simba_utils.rs (21KB)
//!
//! This implementation adapts the DashOpt SIMBA algorithm for the DashFlow `Node<S>` architecture.
//!
//! ## References
//!
//! - **Source**: DSPy teleprompt library
//! - **Link**: <https://github.com/stanfordnlp/dspy/blob/main/dspy/teleprompt/simba.py>
//! - **Algorithm**: Stochastic Introspective Mini-Batch Ascent
//! - **Key Feature**: Self-reflective improvement via trajectory analysis

use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::introspection::{ExecutionTrace, ExecutionTraceBuilder, NodeExecution};
use crate::node::Node;
use crate::optimize::telemetry::{
    record_candidate_evaluated, record_iteration, record_optimization_complete,
    record_optimization_start,
};
use crate::optimize::{FewShotExample, MetricFn, Optimizable};
use crate::state::GraphState;
use crate::Result;
use async_trait::async_trait;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing;

/// SIMBA optimizer for DashFlow nodes.
///
/// SIMBA (Stochastic Introspective Mini-Batch Ascent) uses Monte Carlo search
/// with LLM introspection to optimize prompts. It identifies challenging
/// examples (those with high output variability) and applies improvement strategies.
pub struct SIMBA<S: GraphState> {
    /// Mini-batch size for sampling
    bsize: usize,

    /// Number of new candidate programs to produce per iteration
    num_candidates: usize,

    /// Number of optimization steps to run
    max_steps: usize,

    /// Maximum number of demos a predictor can hold before dropping some
    /// (0 = no limit, but stochastic dropout still applies)
    max_demos: usize,

    /// Number of threads for parallel execution (None = use default)
    ///
    /// Note: This field is defined and can be set via `with_num_threads()`, but
    /// parallel execution is not yet implemented. The SIMBA algorithm currently
    /// runs sequentially. Retained for API stability and future parallelization.
    #[allow(dead_code)] // M-870: Defined for future parallel execution support
    num_threads: Option<usize>,

    /// Temperature for picking programs during trajectory sampling
    temperature_for_sampling: f64,

    /// Temperature for picking source program when building candidates
    temperature_for_candidates: f64,

    /// Random seed for reproducibility
    random_seed: Option<u64>,

    /// Optional LLM for generating improvement rules (AppendARule strategy)
    rule_llm: Option<Arc<dyn ChatModel>>,

    /// Phantom data for state type
    _phantom: std::marker::PhantomData<S>,
}

impl<S: GraphState> SIMBA<S> {
    /// Create a new SIMBA optimizer with default settings.
    ///
    /// # Example
    /// ```rust,ignore
    /// let simba = SIMBA::new()
    ///     .with_bsize(32)
    ///     .with_max_steps(8)
    ///     .with_num_candidates(6);
    /// ```
    pub fn new() -> Self {
        Self {
            bsize: 32,
            num_candidates: 6,
            max_steps: 8,
            max_demos: 4,
            num_threads: None,
            temperature_for_sampling: 0.2,
            temperature_for_candidates: 0.2,
            random_seed: None,
            rule_llm: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Set the mini-batch size.
    ///
    /// Larger batches give more stable gradient estimates but are slower.
    /// Default: 32
    #[must_use]
    pub fn with_bsize(mut self, bsize: usize) -> Self {
        self.bsize = bsize;
        self
    }

    /// Set the number of candidate programs per iteration.
    ///
    /// More candidates increase exploration but are slower.
    /// Default: 6
    #[must_use]
    pub fn with_num_candidates(mut self, num_candidates: usize) -> Self {
        self.num_candidates = num_candidates;
        self
    }

    /// Set the number of optimization steps.
    ///
    /// More steps allow more improvement but take longer.
    /// Default: 8
    #[must_use]
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Set the maximum number of demos per predictor.
    ///
    /// Set to 0 to disable demo limits (stochastic dropout still applies).
    /// Default: 4
    #[must_use]
    pub fn with_max_demos(mut self, max_demos: usize) -> Self {
        self.max_demos = max_demos;
        self
    }

    /// Set the number of parallel threads.
    ///
    /// None means use default (number of CPUs).
    #[must_use]
    pub fn with_num_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = Some(num_threads);
        self
    }

    /// Set the temperature for trajectory sampling.
    ///
    /// Lower values make sampling more deterministic (favor high-scoring programs).
    /// Default: 0.2
    #[must_use]
    pub fn with_temperature_for_sampling(mut self, temperature: f64) -> Self {
        self.temperature_for_sampling = temperature;
        self
    }

    /// Set the temperature for candidate program selection.
    ///
    /// Lower values make selection more deterministic (favor high-scoring programs).
    /// Default: 0.2
    #[must_use]
    pub fn with_temperature_for_candidates(mut self, temperature: f64) -> Self {
        self.temperature_for_candidates = temperature;
        self
    }

    /// Set the random seed for reproducibility.
    #[must_use]
    pub fn with_random_seed(mut self, seed: u64) -> Self {
        self.random_seed = Some(seed);
        self
    }

    /// Set an LLM for generating improvement rules (enables AppendARule strategy).
    #[must_use]
    pub fn with_rule_llm(mut self, llm: Arc<dyn ChatModel>) -> Self {
        self.rule_llm = Some(llm);
        self
    }

    /// Optimize a node using SIMBA algorithm.
    ///
    /// # Arguments
    /// * `node` - The node to optimize
    /// * `trainset` - Training examples with expected outputs
    /// * `metric` - Function that scores quality (0.0 to 1.0)
    ///
    /// # Returns
    /// Optimized node (clone with improved prompts/demos)
    ///
    /// # Errors
    /// Returns error if trainset is too small or optimization fails
    pub async fn optimize<N>(&self, node: &N, trainset: &[S], metric: &MetricFn<S>) -> Result<N>
    where
        N: Optimizable<S> + Clone,
    {
        // Validate trainset size
        if trainset.len() < self.bsize {
            return Err(crate::Error::Validation(format!(
                "Trainset too small: {} < {}",
                trainset.len(),
                self.bsize
            )));
        }

        tracing::info!(
            max_steps = self.max_steps,
            trainset_size = trainset.len(),
            batch_size = self.bsize,
            num_candidates = self.num_candidates,
            "SIMBA: Starting optimization"
        );

        // Record telemetry start
        record_optimization_start("simba");
        let optimization_start = std::time::Instant::now();
        let mut total_candidates_evaluated: u64 = 0;
        let mut initial_score: f64 = 0.0;

        // Initialize RNG
        // M-873: Default seed=0 provides reproducibility, but users may expect random behavior.
        // Log a warning when using the default so users understand their runs are deterministic.
        let seed = self.random_seed.unwrap_or_else(|| {
            tracing::warn!(
                "SIMBA: No random_seed specified, defaulting to seed=0 for reproducibility. \
                 Use .with_random_seed(None) with a random value for non-deterministic behavior."
            );
            0
        });
        let mut rng = StdRng::seed_from_u64(seed);

        // Program pool and scores
        let mut programs: Vec<N> = Vec::new();
        let mut program_scores: HashMap<usize, Vec<f64>> = HashMap::new();
        let mut next_program_idx = 0;

        // Helper: Calculate average score for a program
        let calc_average_score = |prog_idx: usize, scores: &HashMap<usize, Vec<f64>>| -> f64 {
            scores
                .get(&prog_idx)
                .map(|s| {
                    if s.is_empty() {
                        0.0
                    } else {
                        s.iter().sum::<f64>() / s.len() as f64
                    }
                })
                .unwrap_or(0.0)
        };

        // Helper: Register a new program in the pool
        let register_new_program = |next_idx: &mut usize,
                                    programs: &mut Vec<N>,
                                    program_scores: &mut HashMap<usize, Vec<f64>>,
                                    prog: N,
                                    score_list: Vec<f64>| {
            *next_idx += 1;
            programs.push(prog);
            program_scores.insert(*next_idx, score_list);
        };

        // Initialize baseline program (index 0)
        let baseline = node.clone();
        programs.push(baseline.clone());
        program_scores.insert(0, vec![]);

        let mut winning_programs = vec![baseline];

        // Shuffle training data
        let mut data_indices: Vec<usize> = (0..trainset.len()).collect();
        data_indices.shuffle(&mut rng);
        let mut instance_idx = 0;

        // Main optimization loop
        for batch_idx in 0..self.max_steps {
            // Record iteration telemetry
            record_iteration("simba");

            tracing::info!(
                batch = batch_idx + 1,
                total_batches = self.max_steps,
                "SIMBA: Starting batch"
            );

            // STEP 1: Get next mini-batch
            if instance_idx + self.bsize > trainset.len() {
                data_indices.shuffle(&mut rng);
                instance_idx = 0;
            }

            let batch_indices = &data_indices[instance_idx..instance_idx + self.bsize];
            let batch: Vec<&S> = batch_indices.iter().map(|&i| &trainset[i]).collect();
            instance_idx += self.bsize;

            // STEP 2: Get top programs for this iteration
            let top_programs = Self::top_k_plus_baseline(
                self.num_candidates,
                &programs,
                &program_scores,
                &calc_average_score,
            );

            tracing::debug!(
                num_candidates = self.num_candidates,
                "Sampling trajectories per example"
            );

            // STEP 3: Generate execution trajectories
            let mut outputs = Vec::new();

            for _model_idx in 0..self.num_candidates {
                for example in &batch {
                    // Choose program via softmax
                    let chosen_prog_idx = Self::softmax_sample(
                        &mut rng,
                        &top_programs,
                        self.temperature_for_sampling,
                        &program_scores,
                        &calc_average_score,
                    )?;
                    let candidate_node = &programs[chosen_prog_idx];

                    // Execute and capture output
                    let output = Self::execute_and_score(candidate_node, example, metric).await;
                    outputs.push(output);
                }
            }

            assert_eq!(outputs.len(), self.bsize * self.num_candidates);

            // Track candidates evaluated
            let candidates_this_batch = (self.bsize * self.num_candidates) as u64;
            total_candidates_evaluated += candidates_this_batch;
            for _ in 0..candidates_this_batch {
                record_candidate_evaluated("simba");
            }

            // STEP 4: Analyze variability and create buckets
            tracing::debug!(batch_size = self.bsize, "Analyzing output variability");

            let all_scores: Vec<f64> = outputs.iter().map(|o| o.score).collect();
            let batch_10p_score = percentile(&all_scores, 10.0);
            let batch_90p_score = percentile(&all_scores, 90.0);

            // Capture initial score from first batch for telemetry
            if batch_idx == 0 {
                initial_score = all_scores.iter().sum::<f64>() / all_scores.len() as f64;
            }

            let buckets = Self::create_buckets(&outputs, self.bsize, self.num_candidates);

            let baseline_score = all_scores.iter().sum::<f64>() / all_scores.len() as f64;
            tracing::debug!(score = %format!("{:.4}", baseline_score), "Baseline mini-batch score");

            // STEP 5: Build new candidate programs
            tracing::debug!(
                num_candidates = self.num_candidates + 1,
                "Building candidate programs"
            );

            let mut system_candidates = Vec::new();

            for (bucket_idx, (bucket, bucket_stats)) in buckets.iter().enumerate() {
                let (max_to_min_gap, max_score, max_to_avg_gap) = bucket_stats;

                tracing::debug!(
                    bucket = bucket_idx + 1,
                    max_score = %format!("{:.4}", max_score),
                    gap = %format!("{:.4}", max_to_min_gap),
                    avg_gap = %format!("{:.4}", max_to_avg_gap),
                    "Bucket analysis"
                );

                // Pick source program via softmax
                let top_progs = Self::top_k_plus_baseline(
                    self.num_candidates,
                    &programs,
                    &program_scores,
                    &calc_average_score,
                );
                let src_prog_idx = Self::softmax_sample(
                    &mut rng,
                    &top_progs,
                    self.temperature_for_candidates,
                    &program_scores,
                    &calc_average_score,
                )?;

                let mut system_candidate = programs[src_prog_idx].clone();

                // Feature: Drop demos stochastically using Poisson distribution.
                // This would add regularization by randomly removing demos
                // based on a Poisson-distributed count, preventing overfitting.
                // Requires: Access to node's OptimizationState for demo manipulation.

                // Pick a random strategy (AppendARule only if an LLM is configured)
                let use_rule_strategy = self.rule_llm.is_some();
                let strategy_idx = if use_rule_strategy {
                    rng.gen_range(0..2) // 0 = AppendADemo, 1 = AppendARule
                } else {
                    0
                };
                let strategy_name = if strategy_idx == 0 {
                    "append_a_demo"
                } else {
                    "append_a_rule"
                };

                tracing::debug!(strategy = strategy_name, "Applying strategy");

                // Apply strategy
                let context = StrategyContext {
                    batch_10p_score,
                    batch_90p_score,
                    max_demos: self.max_demos,
                };

                let applied = if strategy_idx == 0 {
                    let strategy = AppendADemo::default();
                    strategy
                        .apply(bucket, &mut system_candidate, &context)
                        .await?
                } else {
                    let strategy = AppendARule::with_llm(
                        self.rule_llm
                            .as_ref()
                            .expect("use_rule_strategy implies rule_llm is Some")
                            .clone(),
                    );
                    strategy
                        .apply(bucket, &mut system_candidate, &context)
                        .await?
                };

                if applied {
                    system_candidates.push(system_candidate);
                } else {
                    tracing::debug!("Strategy skipped");
                }

                if system_candidates.len() > self.num_candidates {
                    break;
                }
            }

            // STEP 6: Evaluate new candidates on the mini-batch
            tracing::debug!(
                num_candidates = system_candidates.len(),
                "Evaluating candidates on mini-batch"
            );

            let mut eval_outputs = Vec::new();
            for candidate in &system_candidates {
                for example in &batch {
                    let output = Self::execute_and_score(candidate, example, metric).await;
                    eval_outputs.push(output);
                }
            }

            assert_eq!(eval_outputs.len(), system_candidates.len() * self.bsize);

            // STEP 7: Compute average scores for each candidate
            let mut candidate_scores = Vec::new();
            for (idx_cand, _) in system_candidates.iter().enumerate() {
                let start = idx_cand * self.bsize;
                let end = (idx_cand + 1) * self.bsize;
                let sys_scores: Vec<f64> =
                    eval_outputs[start..end].iter().map(|o| o.score).collect();
                let avg_sys_score = if sys_scores.is_empty() {
                    0.0
                } else {
                    sys_scores.iter().sum::<f64>() / sys_scores.len() as f64
                };
                candidate_scores.push(avg_sys_score);
            }

            tracing::debug!(
                scores = ?candidate_scores.iter().map(|s| format!("{:.4}", s)).collect::<Vec<_>>(),
                "Candidate scores"
            );

            if !candidate_scores.is_empty() {
                if let Some(best_score) = candidate_scores
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                {
                    tracing::debug!(score = %format!("{:.4}", best_score), "Best candidate score");
                }
            }

            // STEP 8: Select best candidate as "winning program"
            if !candidate_scores.is_empty() {
                if let Some((best_idx, _)) = candidate_scores
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                {
                    let best_program = system_candidates[best_idx].clone();
                    winning_programs.push(best_program);
                }
            }

            // STEP 9: Register all new candidates in program pool
            for (idx_cand, candidate) in system_candidates.iter().enumerate() {
                let start = idx_cand * self.bsize;
                let end = (idx_cand + 1) * self.bsize;
                let sys_scores: Vec<f64> =
                    eval_outputs[start..end].iter().map(|o| o.score).collect();
                register_new_program(
                    &mut next_program_idx,
                    &mut programs,
                    &mut program_scores,
                    candidate.clone(),
                    sys_scores,
                );
            }

            tracing::debug!(
                pool_size = programs.len(),
                new_programs = system_candidates.len(),
                "Program pool updated"
            );
        }

        // Final evaluation: Select diverse programs from winning_programs
        tracing::info!("SIMBA: Final evaluation on full trainset");

        let m = winning_programs.len() - 1;
        let n = self.num_candidates + 1;

        let program_idxs: Vec<usize> = if m < 1 {
            vec![0; n]
        } else {
            (0..n).map(|i| (i * m) / (n - 1)).collect()
        };

        // Remove duplicates
        let mut unique_idxs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for idx in program_idxs {
            if seen.insert(idx) {
                unique_idxs.push(idx);
            }
        }

        let candidate_programs: Vec<N> = unique_idxs
            .iter()
            .map(|&i| winning_programs[i].clone())
            .collect();

        tracing::debug!(
            num_programs = candidate_programs.len(),
            num_examples = trainset.len(),
            "Evaluating diverse programs"
        );

        // Evaluate all candidates on full trainset
        let mut final_outputs = Vec::new();
        for candidate in &candidate_programs {
            for example in trainset {
                let output = Self::execute_and_score(candidate, example, metric).await;
                final_outputs.push(output);
            }
        }

        // Calculate final scores
        let mut final_scores = Vec::new();
        for (idx_prog, _) in candidate_programs.iter().enumerate() {
            let start = idx_prog * trainset.len();
            let end = (idx_prog + 1) * trainset.len();
            let sys_scores: Vec<f64> = final_outputs[start..end].iter().map(|o| o.score).collect();
            let avg_score = if sys_scores.is_empty() {
                0.0
            } else {
                sys_scores.iter().sum::<f64>() / sys_scores.len() as f64
            };
            final_scores.push(avg_score);
        }

        tracing::debug!(
            scores = ?final_scores.iter().map(|s| format!("{:.4}", s)).collect::<Vec<_>>(),
            "Final scores"
        );

        // Select best program
        let best_idx = final_scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        let best_score = final_scores[best_idx];
        let best_program = candidate_programs[best_idx].clone();

        tracing::info!(
            best_program_idx = best_idx,
            best_score = %format!("{:.4}", best_score),
            "SIMBA: Optimization complete"
        );

        // Record telemetry completion
        let duration = optimization_start.elapsed().as_secs_f64();
        record_optimization_complete(
            "simba",
            self.max_steps as u64,      // iterations
            total_candidates_evaluated, // candidates
            initial_score,
            best_score,
            duration,
        );

        Ok(best_program)
    }

    /// Execute a node and compute its score
    ///
    /// Captures a full `ExecutionTrace` with state snapshots for unified telemetry.
    async fn execute_and_score<N>(node: &N, example: &S, metric: &MetricFn<S>) -> SimbaOutput<S>
    where
        N: Node<S>,
    {
        let start = std::time::Instant::now();

        // Serialize input state for trace
        let state_before = serde_json::to_value(example).ok();

        // Execute the node
        let state = example.clone();
        let (prediction, success, error_message) = match node.execute(state).await {
            Ok(new_state) => (Some(new_state), true, None),
            Err(e) => {
                tracing::warn!("Node execution failed: {:?}", e);
                (None, false, Some(e.to_string()))
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Serialize output state for trace
        let state_after = prediction
            .as_ref()
            .and_then(|p| serde_json::to_value(p).ok());

        // Compute score using the metric
        let score = if let Some(ref pred) = prediction {
            match metric(example, pred) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Metric evaluation failed: {:?}", e);
                    0.0
                }
            }
        } else {
            0.0
        };

        // Build ExecutionTrace for unified telemetry
        let node_exec = NodeExecution {
            node: node.name(),
            duration_ms,
            tokens_used: 0,
            index: 0,
            success,
            error_message,
            state_before,
            state_after: state_after.clone(),
            tools_called: Vec::new(),
            started_at: None,
            metadata: HashMap::new(),
        };

        let mut builder = ExecutionTraceBuilder::new()
            .execution_id(format!("simba-{}", uuid::Uuid::new_v4()))
            .add_node_execution(node_exec)
            .completed(success)
            .total_duration_ms(duration_ms);

        if let Some(final_state) = state_after {
            builder = builder.final_state(final_state);
        }

        let trace = builder.build();

        SimbaOutput {
            prediction,
            trace: Some(trace),
            score,
            example: example.clone(),
            output_metadata: HashMap::new(),
        }
    }

    /// Get top K programs plus baseline
    #[allow(clippy::type_complexity)] // Generic scoring function with HashMap-based score lookup
    fn top_k_plus_baseline<N>(
        k: usize,
        programs: &[N],
        program_scores: &HashMap<usize, Vec<f64>>,
        calc_average_score: &dyn Fn(usize, &HashMap<usize, Vec<f64>>) -> f64,
    ) -> Vec<usize>
    where
        N: Node<S>,
    {
        // Create list of (program_idx, avg_score)
        let mut scored_programs: Vec<(usize, f64)> = programs
            .iter()
            .enumerate()
            .map(|(idx, _)| (idx, calc_average_score(idx, program_scores)))
            .collect();

        // Sort by score descending
        scored_programs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        let mut top_k: Vec<usize> = scored_programs
            .iter()
            .take(k)
            .map(|(idx, _)| *idx)
            .collect();

        // Ensure baseline (0) is included
        if !top_k.contains(&0) && !top_k.is_empty() {
            if let Some(last) = top_k.last_mut() {
                *last = 0;
            }
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        top_k.retain(|&x| seen.insert(x));

        top_k
    }

    /// Softmax sampling from program pool
    ///
    /// # Temperature Behavior
    /// - High temperature (>1.0): More uniform sampling across programs
    /// - Low temperature (<1.0): Stronger preference for higher-scoring programs
    /// - Near-zero temperature (<0.01): Deterministically selects the highest-scoring program
    ///
    /// # Errors
    /// Returns error if `program_idxs` is empty or if temperature is negative.
    #[allow(clippy::type_complexity)] // Generic scoring function for softmax temperature sampling
    fn softmax_sample(
        rng: &mut StdRng,
        program_idxs: &[usize],
        temperature: f64,
        program_scores: &HashMap<usize, Vec<f64>>,
        calc_average_score: &dyn Fn(usize, &HashMap<usize, Vec<f64>>) -> f64,
    ) -> Result<usize> {
        if program_idxs.is_empty() {
            return Err(crate::Error::Validation(
                "No programs available for softmax sampling".to_string(),
            ));
        }

        // M-871: Handle invalid temperature values with clear error messages
        if temperature < 0.0 {
            return Err(crate::Error::Validation(format!(
                "Temperature must be non-negative, got {}. Use temperature >= 0.01 for softmax sampling, \
                 or temperature near 0 for deterministic (argmax) selection.",
                temperature
            )));
        }

        // Calculate unnormalized weights
        let prog_scores: Vec<f64> = program_idxs
            .iter()
            .map(|&idx| calc_average_score(idx, program_scores))
            .collect();

        // M-871: Near-zero temperature causes exp() overflow to infinity.
        // Fall back to deterministic (argmax) selection when temperature is very small.
        const MIN_SOFTMAX_TEMPERATURE: f64 = 0.01;
        if temperature < MIN_SOFTMAX_TEMPERATURE {
            tracing::debug!(
                temperature = temperature,
                threshold = MIN_SOFTMAX_TEMPERATURE,
                "Temperature below threshold, using deterministic (argmax) selection"
            );
            // Return the program with the highest score (deterministic)
            let (best_idx, _) = prog_scores
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or((0, &0.0));
            return Ok(program_idxs[best_idx]);
        }

        let exps: Vec<f64> = prog_scores
            .iter()
            .map(|&s| (s / temperature).exp())
            .collect();

        let sum_exps: f64 = exps.iter().sum();

        // M-871: Handle overflow (infinity) or underflow (zero/NaN) cases
        if !sum_exps.is_finite() || sum_exps <= 0.0 {
            tracing::debug!(
                sum_exps = sum_exps,
                temperature = temperature,
                "Softmax overflow/underflow detected, falling back to uniform sampling"
            );
            // Fallback: uniform sampling
            return program_idxs
                .choose(rng)
                .copied()
                .ok_or_else(|| crate::Error::Validation("Failed to sample program".to_string()));
        }

        // Calculate probabilities
        let probs: Vec<f64> = exps.iter().map(|&e| e / sum_exps).collect();

        // Validate probabilities are all finite (guards against NaN from inf/inf)
        if !probs.iter().all(|&p| p.is_finite()) {
            tracing::debug!(
                "Softmax produced non-finite probabilities, falling back to uniform sampling"
            );
            return program_idxs
                .choose(rng)
                .copied()
                .ok_or_else(|| crate::Error::Validation("Failed to sample program".to_string()));
        }

        // Weighted random choice
        let dist = rand::distributions::WeightedIndex::new(&probs).map_err(|e| {
            crate::Error::Validation(format!("Failed to create weighted distribution: {}", e))
        })?;

        Ok(program_idxs[rng.sample(dist)])
    }

    /// Create buckets from outputs, grouping by example and sorting by variability
    #[allow(clippy::type_complexity)] // Returns bucketed outputs with (min, max, std) statistics
    fn create_buckets(
        outputs: &[SimbaOutput<S>],
        bsize: usize,
        num_candidates: usize,
    ) -> Vec<(Vec<SimbaOutput<S>>, (f64, f64, f64))> {
        let mut buckets = Vec::new();

        // Group outputs by example (each group has num_candidates outputs)
        for ex_idx in 0..bsize {
            // Gather all outputs for this example
            let mut bucket: Vec<SimbaOutput<S>> = (0..num_candidates)
                .map(|model_idx| outputs[ex_idx + model_idx * bsize].clone())
                .collect();

            // Sort by score descending (best first)
            bucket.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // M-872: Use defensive accessors instead of direct indexing.
            // Bucket should never be empty (num_candidates > 0), but guard against it.
            let (max_score, min_score, avg_score) =
                if let (Some(first), Some(last)) = (bucket.first(), bucket.last()) {
                    let avg = bucket.iter().map(|o| o.score).sum::<f64>() / bucket.len() as f64;
                    (first.score, last.score, avg)
                } else {
                    // Empty bucket (num_candidates=0) - skip this example
                    tracing::warn!(
                        example_idx = ex_idx,
                        "Empty bucket in create_buckets (num_candidates=0?), skipping"
                    );
                    continue;
                };

            let max_to_min_gap = max_score - min_score;
            let max_to_avg_gap = max_score - avg_score;

            buckets.push((bucket, (max_to_min_gap, max_score, max_to_avg_gap)));
        }

        // Sort buckets by (max_to_min_gap, max_score, max_to_avg_gap) descending
        buckets.sort_by(|a, b| {
            let (gap_a, max_a, avg_gap_a) = a.1;
            let (gap_b, max_b, avg_gap_b) = b.1;

            gap_b
                .partial_cmp(&gap_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    max_b
                        .partial_cmp(&max_a)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    avg_gap_b
                        .partial_cmp(&avg_gap_a)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        buckets
    }
}

impl<S: GraphState> Default for SIMBA<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// Output from a single program execution in SIMBA.
///
/// Contains prediction, trace, score, and metadata.
#[derive(Debug, Clone)]
pub struct SimbaOutput<S: GraphState> {
    /// The prediction made by the program
    pub prediction: Option<S>,

    /// The execution trace (unified telemetry via ExecutionTrace)
    ///
    /// Contains state snapshots, timing, and execution metadata.
    /// Can be converted to training data using `ExecutionTrace::to_examples()`.
    pub trace: Option<ExecutionTrace>,

    /// The score assigned by the metric
    pub score: f64,

    /// The original example that was processed
    pub example: S,

    /// Additional metadata from the metric
    pub output_metadata: HashMap<String, serde_json::Value>,
}

/// A single step in the execution trace.
///
/// Records which node was executed and its inputs/outputs.
///
/// **Deprecated:** This type is kept for backward compatibility. New code should
/// use `ExecutionTrace` and `NodeExecution` from `introspection` module instead.
#[deprecated(
    since = "1.11.3",
    note = "Use ExecutionTrace and NodeExecution from introspection module instead"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    /// Name of the node (from graph structure)
    pub node_name: String,

    /// Inputs to the node
    pub inputs: HashMap<String, serde_json::Value>,

    /// Outputs from the node
    pub outputs: HashMap<String, serde_json::Value>,
}

/// Context passed to strategy functions.
pub struct StrategyContext {
    /// 10th percentile score in the current batch
    pub batch_10p_score: f64,

    /// 90th percentile score in the current batch
    pub batch_90p_score: f64,

    /// Maximum number of demos to retain in the node state (0 = no limit)
    pub max_demos: usize,
}

/// A strategy for improving programs in SIMBA.
///
/// Strategies take a bucket of outputs (sorted by score) and modify the node.
#[async_trait]
pub trait SimbaStrategy<S: GraphState>: Send + Sync {
    /// Apply this strategy to improve the node.
    ///
    /// # Arguments
    /// * `bucket` - Outputs for a single example, sorted by score (best first)
    /// * `node` - The node to modify
    /// * `context` - Additional context (scores, etc.)
    ///
    /// # Returns
    /// - `Ok(true)` if the strategy was applied AND the node was modified
    /// - `Ok(false)` if the strategy was skipped (node unchanged)
    /// - `Err` if the strategy failed
    ///
    /// # Contract (M-874)
    /// Implementations MUST modify `node` before returning `Ok(true)`. If conditions
    /// are not met (e.g., score too low, missing data), return `Ok(false)` instead.
    /// This ensures callers can trust that `true` means meaningful work was done.
    async fn apply<N: Optimizable<S> + Send>(
        &self,
        bucket: &[SimbaOutput<S>],
        node: &mut N,
        context: &StrategyContext,
    ) -> Result<bool>;

    /// Get the name of this strategy (for logging)
    fn name(&self) -> &str;
}

/// Strategy: Append a demonstration from the best trajectory.
///
/// Takes the best-scoring trajectory from the bucket and creates a few-shot
/// demonstration from the example/prediction pair.
pub struct AppendADemo {
    /// Maximum length of input fields in demos (truncate if longer)
    max_input_len: usize,

    /// Collected demos from successful applications
    demos: std::sync::Mutex<Vec<FewShotExample>>,
}

impl AppendADemo {
    /// Create a new AppendADemo strategy.
    ///
    /// # Arguments
    /// * `max_input_len` - Maximum length for input fields (default: 100,000)
    pub fn new(max_input_len: usize) -> Self {
        Self {
            max_input_len,
            demos: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get the collected demos.
    ///
    /// Returns demos extracted from successful trajectories during optimization.
    pub fn get_demos(&self) -> Vec<FewShotExample> {
        self.demos.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Clear collected demos.
    pub fn clear_demos(&self) {
        self.demos.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Truncate a string value if it exceeds max_input_len.
    fn truncate_value(&self, value: &serde_json::Value) -> serde_json::Value {
        if let serde_json::Value::String(s) = value {
            if s.len() > self.max_input_len {
                serde_json::Value::String(s.chars().take(self.max_input_len).collect())
            } else {
                value.clone()
            }
        } else {
            value.clone()
        }
    }
}

impl Default for AppendADemo {
    fn default() -> Self {
        Self::new(100_000)
    }
}

#[async_trait]
impl<S: GraphState> SimbaStrategy<S> for AppendADemo {
    async fn apply<N: Optimizable<S> + Send>(
        &self,
        bucket: &[SimbaOutput<S>],
        node: &mut N,
        context: &StrategyContext,
    ) -> Result<bool> {
        // M-872: Use defensive accessor instead of direct indexing
        let Some(good) = bucket.first() else {
            tracing::debug!("Skipping append_a_demo: empty bucket");
            return Ok(false);
        };

        // Skip if score is too low (at or below 10th percentile)
        if good.score <= context.batch_10p_score {
            tracing::debug!(
                "Skipping append_a_demo: score {} <= 10th percentile {}",
                good.score,
                context.batch_10p_score
            );
            return Ok(false);
        }

        // Skip if no prediction was made
        let Some(ref prediction) = good.prediction else {
            tracing::debug!("Skipping append_a_demo: no prediction available");
            return Ok(false);
        };

        // Convert example to JSON for input
        let example_json = serde_json::to_value(&good.example)?;

        // Convert prediction to JSON for output
        let prediction_json = serde_json::to_value(prediction)?;

        // Truncate values if needed
        let input = self.truncate_value(&example_json);
        let output = self.truncate_value(&prediction_json);

        // Create FewShotExample from the successful trajectory
        let demo = FewShotExample {
            input,
            output,
            reasoning: if let Some(ref trace) = good.trace {
                // Include trace steps as reasoning if available
                if !trace.nodes_executed.is_empty() {
                    let reasoning_text: Vec<String> = trace
                        .nodes_executed
                        .iter()
                        .map(|node_exec| {
                            let inputs = node_exec
                                .state_before
                                .as_ref()
                                .map(|v| format!("{}", v))
                                .unwrap_or_else(|| "{}".to_string());
                            let outputs = node_exec
                                .state_after
                                .as_ref()
                                .map(|v| format!("{}", v))
                                .unwrap_or_else(|| "{}".to_string());
                            format!(
                                "Step {}: inputs={}, outputs={}",
                                node_exec.node, inputs, outputs
                            )
                        })
                        .collect();
                    Some(reasoning_text.join("\n"))
                } else {
                    None
                }
            } else {
                None
            },
        };

        // Add to collected demos
        self.demos
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(demo.clone());

        // Apply the demo to the node so candidates actually differ.
        let mut state = node.get_optimization_state();
        state.few_shot_examples.push(demo);
        if context.max_demos > 0 {
            while state.few_shot_examples.len() > context.max_demos {
                state.few_shot_examples.remove(0);
            }
        }
        state.metadata.insert(
            "simba_last_strategy".to_string(),
            "append_a_demo".to_string(),
        );
        node.set_optimization_state(state);

        tracing::info!(
            "append_a_demo: Added demo from example with score {:.4}",
            good.score
        );

        Ok(true) // Demo added successfully
    }

    fn name(&self) -> &str {
        "append_a_demo"
    }
}

/// Strategy: Append an improvement rule via LLM introspection.
///
/// Compares the best and worst trajectories, then uses an LLM to
/// generate advice on how to improve. Requires an LLM for rule generation.
pub struct AppendARule {
    /// LLM for generating improvement rules
    llm: Option<Arc<dyn ChatModel>>,

    /// Collected rules from successful applications
    rules: std::sync::Mutex<Vec<String>>,
}

impl Default for AppendARule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendARule {
    /// Create a new AppendARule strategy without LLM (rules will be skipped).
    pub fn new() -> Self {
        Self {
            llm: None,
            rules: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Create a new AppendARule strategy with an LLM for rule generation.
    ///
    /// # Arguments
    /// * `llm` - The language model to use for generating improvement rules
    #[must_use]
    pub fn with_llm(llm: Arc<dyn ChatModel>) -> Self {
        Self {
            llm: Some(llm),
            rules: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get the collected rules.
    ///
    /// Returns improvement rules generated during optimization.
    pub fn get_rules(&self) -> Vec<String> {
        self.rules.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Clear collected rules.
    pub fn clear_rules(&self) {
        self.rules.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Build a prompt for LLM introspection comparing good and bad trajectories.
    fn build_introspection_prompt<S: GraphState>(
        &self,
        good: &SimbaOutput<S>,
        bad: &SimbaOutput<S>,
    ) -> String {
        // Serialize examples and predictions for comparison
        let good_example = serde_json::to_string_pretty(&good.example).unwrap_or_default();
        let bad_example = serde_json::to_string_pretty(&bad.example).unwrap_or_default();

        let good_pred = good
            .prediction
            .as_ref()
            .map(|p| serde_json::to_string_pretty(p).unwrap_or_default())
            .unwrap_or_else(|| "(no prediction)".to_string());
        let bad_pred = bad
            .prediction
            .as_ref()
            .map(|p| serde_json::to_string_pretty(p).unwrap_or_default())
            .unwrap_or_else(|| "(no prediction)".to_string());

        format!(
            r#"You are analyzing the performance of an AI model on a task to generate improvement rules.

## Successful Example (Score: {good_score:.4})

### Input:
{good_example}

### Output:
{good_pred}

## Failed Example (Score: {bad_score:.4})

### Input:
{bad_example}

### Output:
{bad_pred}

## Task

Compare these two examples and provide a concise improvement rule that would help the model perform better on challenging examples.

Focus on:
1. What made the successful example work well
2. What specific aspect caused the failure
3. A concrete, actionable rule the model should follow

Respond with a single improvement rule (1-2 sentences) that the model should apply to future examples. Be specific and actionable."#,
            good_score = good.score,
            bad_score = bad.score,
        )
    }

    /// Parse the LLM response to extract the improvement rule.
    fn parse_rule_response(&self, response: &str) -> Option<String> {
        let rule = response.trim();
        if rule.is_empty() || rule.len() < 10 {
            // Rule too short to be useful
            None
        } else {
            // Clean up the rule - remove common prefixes
            let rule = rule
                .trim_start_matches("Rule:")
                .trim_start_matches("Improvement:")
                .trim_start_matches("- ")
                .trim();
            Some(rule.to_string())
        }
    }
}

#[async_trait]
impl<S: GraphState> SimbaStrategy<S> for AppendARule {
    async fn apply<N: Optimizable<S> + Send>(
        &self,
        bucket: &[SimbaOutput<S>],
        node: &mut N,
        context: &StrategyContext,
    ) -> Result<bool> {
        // Check if LLM is available
        let Some(ref llm) = self.llm else {
            tracing::debug!("Skipping append_a_rule: no LLM configured");
            return Ok(false);
        };

        // M-872: Use defensive accessors instead of direct indexing
        let (Some(good), Some(bad)) = (bucket.first(), bucket.last()) else {
            tracing::debug!("Skipping append_a_rule: empty bucket");
            return Ok(false);
        };

        // Skip if scores don't have good separation
        if good.score <= context.batch_10p_score || bad.score >= context.batch_90p_score {
            tracing::debug!(
                "Skipping append_a_rule: insufficient score separation (good={}, bad={}, 10p={}, 90p={})",
                good.score, bad.score, context.batch_10p_score, context.batch_90p_score
            );
            return Ok(false);
        }

        // Additional check: good should be better than bad
        if good.score <= bad.score {
            tracing::debug!(
                "Skipping append_a_rule: good score {} <= bad score {}",
                good.score,
                bad.score
            );
            return Ok(false);
        }

        // Build prompt for LLM introspection
        let prompt = self.build_introspection_prompt(good, bad);

        // Call LLM
        let messages = vec![Message::human(prompt)];
        let result = llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| {
                crate::Error::Generic(format!("Failed to generate improvement rule: {}", e))
            })?;

        // Extract response text
        let response_text = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        // Parse response to extract rule
        if let Some(rule) = self.parse_rule_response(&response_text) {
            self.rules
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(rule.clone());
            apply_rule_to_node(node, &rule);
            tracing::info!(
                "append_a_rule: Generated rule from good ({:.4}) vs bad ({:.4}): {}",
                good.score,
                bad.score,
                &rule[..rule.len().min(100)]
            );
            Ok(true)
        } else {
            tracing::debug!("append_a_rule: Failed to parse rule from LLM response");
            Ok(false)
        }
    }

    fn name(&self) -> &str {
        "append_a_rule"
    }
}

fn apply_rule_to_node<S: GraphState, N: Optimizable<S>>(node: &mut N, rule: &str) {
    let mut state = node.get_optimization_state();
    state.instruction = append_improvement_rule(&state.instruction, rule);
    state.metadata.insert(
        "simba_last_strategy".to_string(),
        "append_a_rule".to_string(),
    );
    node.set_optimization_state(state);
}

fn append_improvement_rule(instruction: &str, rule: &str) -> String {
    let instruction = instruction.trim_end();
    let rule = rule.trim();
    if instruction.is_empty() {
        format!("Improvement Rule: {rule}")
    } else {
        format!("{instruction}\n\nImprovement Rule: {rule}")
    }
}

/// Calculate percentile of a sorted or unsorted array.
///
/// Uses linear interpolation to match NumPy's default percentile behavior.
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Use linear interpolation to match NumPy's default percentile behavior
    let index = p / 100.0 * (sorted.len() - 1) as f64;
    let lower_index = index.floor() as usize;
    let upper_index = index.ceil() as usize;

    if lower_index == upper_index {
        sorted[lower_index]
    } else {
        let lower_value = sorted[lower_index];
        let upper_value = sorted[upper_index];
        let fraction = index - lower_index as f64;
        lower_value + fraction * (upper_value - lower_value)
    }
}

/// Sample from a Poisson distribution with given lambda.
///
/// Returns a random integer >= 0.
/// Uses Knuth's algorithm for all values of lambda.
#[cfg(test)]
fn sample_poisson(rng: &mut StdRng, lambda: f64) -> usize {
    if lambda <= 0.0 {
        return 0;
    }

    // Use Knuth's algorithm
    // For large lambda (>30), this is slower but avoids external dependencies
    let l = (-lambda).exp();
    let mut k = 0;
    let mut p = 1.0;

    loop {
        k += 1;
        p *= rng.gen::<f64>();
        if p <= l {
            break;
        }
        // Safety check to prevent infinite loop for large lambda
        if k > 1000 {
            // For very large lambda, return approximately lambda (expectation value)
            return lambda.round() as usize;
        }
    }

    k - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimize::{OptimizationResult, OptimizationState, OptimizerConfig};

    #[test]
    fn test_percentile() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

        // Using linear interpolation to match NumPy's default behavior
        assert_eq!(percentile(&values, 0.0), 1.0);
        assert_eq!(percentile(&values, 50.0), 5.5); // Linear interpolation between 5 and 6
        assert_eq!(percentile(&values, 100.0), 10.0);

        assert_eq!(percentile(&[], 50.0), 0.0);
    }

    #[test]
    fn test_sample_poisson() {
        let mut rng = StdRng::seed_from_u64(42);

        // Test small lambda
        let samples: Vec<usize> = (0..100).map(|_| sample_poisson(&mut rng, 3.0)).collect();
        let avg = samples.iter().sum::<usize>() as f64 / samples.len() as f64;
        // Average should be close to lambda=3.0
        assert!(
            (avg - 3.0).abs() < 1.5,
            "Average {:.2} not close to 3.0",
            avg
        );

        // Test lambda=0
        assert_eq!(sample_poisson(&mut rng, 0.0), 0);

        // Test moderate lambda
        let samples: Vec<usize> = (0..100).map(|_| sample_poisson(&mut rng, 10.0)).collect();
        let avg = samples.iter().sum::<usize>() as f64 / samples.len() as f64;
        // Average should be close to lambda=10.0
        assert!(
            (avg - 10.0).abs() < 3.0,
            "Average {:.2} not close to 10.0",
            avg
        );
    }

    #[test]
    fn test_simba_builder() {
        // Create a simple test state type
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        let simba = SIMBA::<TestState>::new()
            .with_bsize(64)
            .with_max_steps(10)
            .with_num_candidates(8)
            .with_max_demos(6)
            .with_temperature_for_sampling(0.3)
            .with_temperature_for_candidates(0.4)
            .with_random_seed(12345);

        assert_eq!(simba.bsize, 64);
        assert_eq!(simba.max_steps, 10);
        assert_eq!(simba.num_candidates, 8);
        assert_eq!(simba.max_demos, 6);
        assert_eq!(simba.temperature_for_sampling, 0.3);
        assert_eq!(simba.temperature_for_candidates, 0.4);
        assert_eq!(simba.random_seed, Some(12345));
    }

    #[test]
    fn test_append_a_demo_default() {
        // Create a simple test state type
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        let strategy = AppendADemo::default();
        assert_eq!(strategy.max_input_len, 100_000);
        // Test the name method via trait bound
        assert_eq!(
            <AppendADemo as SimbaStrategy<TestState>>::name(&strategy),
            "append_a_demo"
        );
    }

    #[test]
    fn test_append_a_rule_name() {
        // Create a simple test state type
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        let strategy = AppendARule::new();
        // Test the name method via trait bound
        assert_eq!(
            <AppendARule as SimbaStrategy<TestState>>::name(&strategy),
            "append_a_rule"
        );
    }

    #[test]
    fn test_softmax_sample() {
        use std::collections::HashMap;

        let mut rng = StdRng::seed_from_u64(42);
        let program_idxs = vec![0, 1, 2];
        let mut program_scores = HashMap::new();
        program_scores.insert(0, vec![0.5, 0.6, 0.7]); // avg = 0.6
        program_scores.insert(1, vec![0.8, 0.9, 1.0]); // avg = 0.9
        program_scores.insert(2, vec![0.2, 0.3, 0.4]); // avg = 0.3

        let calc_average_score = |prog_idx: usize, scores: &HashMap<usize, Vec<f64>>| -> f64 {
            scores
                .get(&prog_idx)
                .map(|s| {
                    if s.is_empty() {
                        0.0
                    } else {
                        s.iter().sum::<f64>() / s.len() as f64
                    }
                })
                .unwrap_or(0.0)
        };

        // Sample many times and verify high-scoring programs are selected more often
        let mut counts = HashMap::new();
        for _ in 0..100 {
            let idx = SIMBA::<TestState>::softmax_sample(
                &mut rng,
                &program_idxs,
                0.2,
                &program_scores,
                &calc_average_score,
            )
            .unwrap();
            *counts.entry(idx).or_insert(0) += 1;
        }

        // Program 1 (avg=0.9) should be selected most frequently
        // Program 2 (avg=0.3) should be selected least frequently
        assert!(counts[&1] > counts[&0]);
        assert!(counts[&0] > counts[&2]);

        // Create a simple test state type
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }
    }

    #[test]
    fn test_top_k_plus_baseline() {
        use std::collections::HashMap;

        let calc_average_score = |prog_idx: usize, scores: &HashMap<usize, Vec<f64>>| -> f64 {
            scores
                .get(&prog_idx)
                .map(|s| {
                    if s.is_empty() {
                        0.0
                    } else {
                        s.iter().sum::<f64>() / s.len() as f64
                    }
                })
                .unwrap_or(0.0)
        };

        // Create a simple test state type
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        #[derive(Clone)]
        struct TestNode {
            #[allow(dead_code)] // Test: Node ID for test identification
            id: usize,
        }

        #[async_trait]
        impl Node<TestState> for TestNode {
            async fn execute(&self, state: TestState) -> crate::Result<TestState> {
                Ok(state)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        // Create mock programs (we just need the count)
        let programs = vec![
            TestNode { id: 0 },
            TestNode { id: 1 },
            TestNode { id: 2 },
            TestNode { id: 3 },
        ];

        let mut program_scores = HashMap::new();
        program_scores.insert(0, vec![0.5]); // Baseline - avg = 0.5
        program_scores.insert(1, vec![0.9]); // Best - avg = 0.9
        program_scores.insert(2, vec![0.7]); // Second best - avg = 0.7
        program_scores.insert(3, vec![0.3]); // Worst - avg = 0.3

        // Get top 2
        let top = SIMBA::<TestState>::top_k_plus_baseline(
            2,
            &programs,
            &program_scores,
            &calc_average_score,
        );

        // Should contain top 2 scores: 1 (0.9) and 2 (0.7)
        // But baseline (0) should replace the second one to ensure baseline is included
        assert!(top.contains(&0)); // Baseline always included
        assert!(top.contains(&1)); // Best program

        // Should not contain all 4
        assert!(top.len() <= 2);
    }

    #[test]
    fn test_create_buckets() {
        // Create a simple test state type
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        // Create outputs for 2 examples with 3 candidates each
        let outputs = vec![
            // Example 0 - candidate 0
            SimbaOutput {
                prediction: None,
                trace: None,
                score: 0.8,
                example: TestState {
                    value: "ex0".to_string(),
                },
                output_metadata: HashMap::new(),
            },
            // Example 1 - candidate 0
            SimbaOutput {
                prediction: None,
                trace: None,
                score: 0.5,
                example: TestState {
                    value: "ex1".to_string(),
                },
                output_metadata: HashMap::new(),
            },
            // Example 0 - candidate 1
            SimbaOutput {
                prediction: None,
                trace: None,
                score: 0.9,
                example: TestState {
                    value: "ex0".to_string(),
                },
                output_metadata: HashMap::new(),
            },
            // Example 1 - candidate 1
            SimbaOutput {
                prediction: None,
                trace: None,
                score: 0.6,
                example: TestState {
                    value: "ex1".to_string(),
                },
                output_metadata: HashMap::new(),
            },
            // Example 0 - candidate 2
            SimbaOutput {
                prediction: None,
                trace: None,
                score: 0.3,
                example: TestState {
                    value: "ex0".to_string(),
                },
                output_metadata: HashMap::new(),
            },
            // Example 1 - candidate 2
            SimbaOutput {
                prediction: None,
                trace: None,
                score: 0.55,
                example: TestState {
                    value: "ex1".to_string(),
                },
                output_metadata: HashMap::new(),
            },
        ];

        let buckets = SIMBA::<TestState>::create_buckets(&outputs, 2, 3);

        assert_eq!(buckets.len(), 2); // 2 examples = 2 buckets

        // First bucket should have higher variability (ex0: 0.9-0.3 = 0.6 gap)
        // Second bucket has lower variability (ex1: 0.6-0.5 = 0.1 gap)
        let (bucket0, (gap0, _, _)) = &buckets[0];
        let (bucket1, (gap1, _, _)) = &buckets[1];

        // Buckets should be sorted by gap descending
        assert!(gap0 > gap1);

        // Each bucket should have 3 outputs (one per candidate)
        assert_eq!(bucket0.len(), 3);
        assert_eq!(bucket1.len(), 3);

        // Outputs within each bucket should be sorted by score descending
        assert!(bucket0[0].score >= bucket0[1].score);
        assert!(bucket0[1].score >= bucket0[2].score);
    }

    #[test]
    fn test_simba_validate_trainset_size() {
        use std::sync::Arc;
        use tokio::runtime::Runtime;

        // Create a simple test state and node
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        #[derive(Clone, Debug)]
        struct TestNode {
            optimization_state: OptimizationState,
        }

        #[async_trait]
        impl Node<TestState> for TestNode {
            async fn execute(&self, state: TestState) -> crate::Result<TestState> {
                Ok(state)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        #[async_trait]
        impl Optimizable<TestState> for TestNode {
            async fn optimize(
                &mut self,
                _examples: &[TestState],
                _metric: &MetricFn<TestState>,
                _config: &OptimizerConfig,
            ) -> crate::Result<OptimizationResult> {
                Ok(OptimizationResult::new(0.0, 0.0, 0, false, 0.0))
            }

            fn get_optimization_state(&self) -> OptimizationState {
                self.optimization_state.clone()
            }

            fn set_optimization_state(&mut self, state: OptimizationState) {
                self.optimization_state = state;
            }
        }

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let simba = SIMBA::<TestState>::new().with_bsize(10);

            let node = TestNode {
                optimization_state: OptimizationState::new(""),
            };
            let trainset = vec![TestState {
                value: "test".to_string(),
            }]; // Only 1 example, but bsize=10

            let metric: MetricFn<TestState> = Arc::new(|_, _| Ok(1.0));

            let result = simba.optimize(&node, &trainset, &metric).await;
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Trainset too small"));
        });
    }

    #[test]
    fn test_simba_default_values() {
        // Create a simple test state type
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        let simba = SIMBA::<TestState>::new();

        // Verify all default values
        assert_eq!(simba.bsize, 32);
        assert_eq!(simba.num_candidates, 6);
        assert_eq!(simba.max_steps, 8);
        assert_eq!(simba.max_demos, 4);
        assert!(simba.num_threads.is_none());
        assert_eq!(simba.temperature_for_sampling, 0.2);
        assert_eq!(simba.temperature_for_candidates, 0.2);
        assert!(simba.random_seed.is_none());
        assert!(simba.rule_llm.is_none());
    }

    #[test]
    fn test_simba_with_num_threads() {
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        let simba = SIMBA::<TestState>::new().with_num_threads(4);
        assert_eq!(simba.num_threads, Some(4));
    }

    #[test]
    fn test_simba_output_struct() {
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        let output: SimbaOutput<TestState> = SimbaOutput {
            prediction: Some(TestState {
                value: "answer".to_string(),
            }),
            trace: None,
            score: 0.95,
            example: TestState {
                value: "example".to_string(),
            },
            output_metadata: HashMap::new(),
        };

        assert_eq!(output.score, 0.95);
        assert!(output.prediction.is_some());
        assert!(output.trace.is_none());
    }

    #[test]
    fn test_simba_output_with_metadata() {
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        let mut metadata = HashMap::new();
        metadata.insert("latency_ms".to_string(), serde_json::json!(150));
        metadata.insert("tokens_used".to_string(), serde_json::json!(500));

        let output: SimbaOutput<TestState> = SimbaOutput {
            prediction: None,
            trace: None,
            score: 0.0,
            example: TestState {
                value: "test".to_string(),
            },
            output_metadata: metadata,
        };

        assert_eq!(
            output.output_metadata.get("latency_ms"),
            Some(&serde_json::json!(150))
        );
        assert_eq!(
            output.output_metadata.get("tokens_used"),
            Some(&serde_json::json!(500))
        );
    }

    #[test]
    fn test_create_buckets_empty() {
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        let outputs: Vec<SimbaOutput<TestState>> = vec![];
        let buckets = SIMBA::<TestState>::create_buckets(&outputs, 0, 0);
        assert!(buckets.is_empty());
    }

    #[test]
    fn test_create_buckets_single_example() {
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        // Single example with 2 candidates
        let outputs = vec![
            SimbaOutput {
                prediction: None,
                trace: None,
                score: 0.8,
                example: TestState {
                    value: "ex0".to_string(),
                },
                output_metadata: HashMap::new(),
            },
            SimbaOutput {
                prediction: None,
                trace: None,
                score: 0.6,
                example: TestState {
                    value: "ex0".to_string(),
                },
                output_metadata: HashMap::new(),
            },
        ];

        let buckets = SIMBA::<TestState>::create_buckets(&outputs, 1, 2);

        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].0.len(), 2);
        // Should be sorted by score descending
        assert!(buckets[0].0[0].score >= buckets[0].0[1].score);
    }

    #[test]
    fn test_poisson_edge_cases() {
        let mut rng = StdRng::seed_from_u64(42);

        // Very small lambda should mostly return 0
        let samples: Vec<usize> = (0..100).map(|_| sample_poisson(&mut rng, 0.001)).collect();
        let zeros = samples.iter().filter(|&&x| x == 0).count();
        assert!(
            zeros > 90,
            "With lambda=0.001, most samples should be 0, got {} zeros",
            zeros
        );
    }

    #[test]
    fn test_softmax_sample_single_program() {
        use std::collections::HashMap;

        let mut rng = StdRng::seed_from_u64(42);
        let program_idxs = vec![0];
        let mut program_scores = HashMap::new();
        program_scores.insert(0, vec![0.5]);

        let calc_average_score = |prog_idx: usize, scores: &HashMap<usize, Vec<f64>>| -> f64 {
            scores
                .get(&prog_idx)
                .map(|s| {
                    if s.is_empty() {
                        0.0
                    } else {
                        s.iter().sum::<f64>() / s.len() as f64
                    }
                })
                .unwrap_or(0.0)
        };

        // With only one program, it should always be selected
        for _ in 0..10 {
            let idx = SIMBA::<TestState>::softmax_sample(
                &mut rng,
                &program_idxs,
                0.2,
                &program_scores,
                &calc_average_score,
            )
            .unwrap();
            assert_eq!(idx, 0);
        }

        // Local test state for this test
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }
    }

    #[test]
    fn test_softmax_sample_near_zero_temperature() {
        // M-871: Test that near-zero temperature falls back to deterministic (argmax) selection
        use std::collections::HashMap;

        let mut rng = StdRng::seed_from_u64(42);
        let program_idxs = vec![0, 1, 2];
        let mut program_scores = HashMap::new();
        program_scores.insert(0, vec![0.3]); // avg = 0.3
        program_scores.insert(1, vec![0.9]); // avg = 0.9 (highest)
        program_scores.insert(2, vec![0.5]); // avg = 0.5

        let calc_average_score = |prog_idx: usize, scores: &HashMap<usize, Vec<f64>>| -> f64 {
            scores
                .get(&prog_idx)
                .map(|s| {
                    if s.is_empty() {
                        0.0
                    } else {
                        s.iter().sum::<f64>() / s.len() as f64
                    }
                })
                .unwrap_or(0.0)
        };

        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        // Near-zero temperature should always select the highest-scoring program (idx 1)
        for _ in 0..10 {
            let idx = SIMBA::<TestState>::softmax_sample(
                &mut rng,
                &program_idxs,
                0.001, // Near-zero temperature
                &program_scores,
                &calc_average_score,
            )
            .unwrap();
            assert_eq!(
                idx, 1,
                "Near-zero temperature should deterministically select best program"
            );
        }

        // Zero temperature should also select the highest-scoring program
        let idx = SIMBA::<TestState>::softmax_sample(
            &mut rng,
            &program_idxs,
            0.0,
            &program_scores,
            &calc_average_score,
        )
        .unwrap();
        assert_eq!(
            idx, 1,
            "Zero temperature should deterministically select best program"
        );
    }

    #[test]
    fn test_softmax_sample_negative_temperature_error() {
        // M-871: Test that negative temperature returns a clear error
        use std::collections::HashMap;

        let mut rng = StdRng::seed_from_u64(42);
        let program_idxs = vec![0, 1];
        let mut program_scores = HashMap::new();
        program_scores.insert(0, vec![0.5]);
        program_scores.insert(1, vec![0.6]);

        let calc_average_score = |prog_idx: usize, scores: &HashMap<usize, Vec<f64>>| -> f64 {
            scores
                .get(&prog_idx)
                .map(|s| {
                    if s.is_empty() {
                        0.0
                    } else {
                        s.iter().sum::<f64>() / s.len() as f64
                    }
                })
                .unwrap_or(0.0)
        };

        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        // Negative temperature should return an error with clear message
        let result = SIMBA::<TestState>::softmax_sample(
            &mut rng,
            &program_idxs,
            -0.5,
            &program_scores,
            &calc_average_score,
        );

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("non-negative"),
            "Error should mention non-negative: {}",
            err_msg
        );
        assert!(
            err_msg.contains("-0.5"),
            "Error should include the invalid value: {}",
            err_msg
        );
    }

    #[test]
    fn test_append_a_demo_custom_input_len() {
        let strategy = AppendADemo::default();
        // Default max_input_len is 100_000
        assert_eq!(strategy.max_input_len, 100_000);

        // Create with custom value
        let custom_strategy = AppendADemo::new(50_000);
        assert_eq!(custom_strategy.max_input_len, 50_000);
    }

    #[test]
    fn test_append_a_demo_get_and_clear_demos() {
        let strategy = AppendADemo::default();

        // Initially empty
        assert!(strategy.get_demos().is_empty());

        // Add a demo manually via the internal mutex (simulating what apply() does)
        {
            let mut demos = strategy.demos.lock().unwrap();
            demos.push(FewShotExample {
                input: serde_json::json!({"question": "What is 2+2?"}),
                output: serde_json::json!({"answer": "4"}),
                reasoning: None,
            });
        }

        // Now should have one demo
        assert_eq!(strategy.get_demos().len(), 1);

        // Clear demos
        strategy.clear_demos();
        assert!(strategy.get_demos().is_empty());
    }

    #[test]
    fn test_append_a_demo_truncate_value() {
        let strategy = AppendADemo::new(10); // Max 10 chars

        // String within limit - no truncation
        let short = serde_json::Value::String("hello".to_string());
        let result = strategy.truncate_value(&short);
        assert_eq!(result, serde_json::Value::String("hello".to_string()));

        // String exceeding limit - truncate
        let long = serde_json::Value::String("this is a very long string".to_string());
        let result = strategy.truncate_value(&long);
        assert_eq!(result, serde_json::Value::String("this is a ".to_string()));

        // Non-string values are not truncated
        let number = serde_json::json!(12345);
        let result = strategy.truncate_value(&number);
        assert_eq!(result, serde_json::json!(12345));
    }

    #[test]
    fn test_append_a_rule_new_and_with_llm() {
        // Test default constructor (no LLM)
        let strategy = AppendARule::new();
        assert!(strategy.llm.is_none());
        assert!(strategy.get_rules().is_empty());

        // Test with_llm constructor (requires a mock LLM)
        // We just test that the struct is constructed correctly
        // LLM integration is tested separately
    }

    #[test]
    fn test_append_a_rule_get_and_clear_rules() {
        let strategy = AppendARule::new();

        // Initially empty
        assert!(strategy.get_rules().is_empty());

        // Add a rule manually
        {
            let mut rules = strategy.rules.lock().unwrap();
            rules.push("Always provide specific examples when explaining concepts.".to_string());
        }

        // Now should have one rule
        assert_eq!(strategy.get_rules().len(), 1);
        assert!(strategy.get_rules()[0].contains("specific examples"));

        // Clear rules
        strategy.clear_rules();
        assert!(strategy.get_rules().is_empty());
    }

    #[test]
    fn test_append_a_rule_parse_response() {
        let strategy = AppendARule::new();

        // Valid rule
        let response = "Always check edge cases before generating output.";
        assert_eq!(
            strategy.parse_rule_response(response),
            Some("Always check edge cases before generating output.".to_string())
        );

        // Rule with prefix
        let response = "Rule: Validate input format first.";
        assert_eq!(
            strategy.parse_rule_response(response),
            Some("Validate input format first.".to_string())
        );

        // Rule with improvement prefix
        let response = "Improvement: Use step-by-step reasoning.";
        assert_eq!(
            strategy.parse_rule_response(response),
            Some("Use step-by-step reasoning.".to_string())
        );

        // Rule with bullet
        let response = "- Break complex problems into smaller parts.";
        assert_eq!(
            strategy.parse_rule_response(response),
            Some("Break complex problems into smaller parts.".to_string())
        );

        // Too short - rejected
        let response = "Hi";
        assert_eq!(strategy.parse_rule_response(response), None);

        // Empty - rejected
        let response = "";
        assert_eq!(strategy.parse_rule_response(response), None);

        // Only whitespace - rejected
        let response = "   ";
        assert_eq!(strategy.parse_rule_response(response), None);
    }

    #[test]
    fn test_append_a_rule_build_introspection_prompt() {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        struct TestState {
            question: String,
            answer: Option<String>,
        }

        let strategy = AppendARule::new();

        let good = SimbaOutput {
            prediction: Some(TestState {
                question: "What is 2+2?".to_string(),
                answer: Some("4".to_string()),
            }),
            trace: None,
            score: 0.95,
            example: TestState {
                question: "What is 2+2?".to_string(),
                answer: None,
            },
            output_metadata: HashMap::new(),
        };

        let bad = SimbaOutput {
            prediction: Some(TestState {
                question: "What is 3+3?".to_string(),
                answer: Some("5".to_string()), // Wrong answer
            }),
            trace: None,
            score: 0.1,
            example: TestState {
                question: "What is 3+3?".to_string(),
                answer: None,
            },
            output_metadata: HashMap::new(),
        };

        let prompt = strategy.build_introspection_prompt(&good, &bad);

        // Check prompt contains key elements
        assert!(prompt.contains("Successful Example (Score: 0.95"));
        assert!(prompt.contains("Failed Example (Score: 0.10"));
        assert!(prompt.contains("What is 2+2?"));
        assert!(prompt.contains("What is 3+3?"));
        assert!(prompt.contains("improvement rule"));
    }

    #[test]
    fn test_append_a_rule_default() {
        // Test the Default trait
        let strategy = AppendARule::default();
        assert!(strategy.llm.is_none());
        assert!(strategy.get_rules().is_empty());

        // Create a simple test state type
        #[derive(Clone, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        // Test the name method via trait bound
        assert_eq!(
            <AppendARule as SimbaStrategy<TestState>>::name(&strategy),
            "append_a_rule"
        );
    }

    #[tokio::test]
    async fn test_append_a_demo_apply_success() {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        struct TestState {
            question: String,
            answer: String,
        }

        #[derive(Clone)]
        struct TestNode {
            optimization_state: OptimizationState,
        }

        #[async_trait]
        impl Node<TestState> for TestNode {
            async fn execute(&self, state: TestState) -> crate::Result<TestState> {
                Ok(state)
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        #[async_trait]
        impl Optimizable<TestState> for TestNode {
            async fn optimize(
                &mut self,
                _examples: &[TestState],
                _metric: &MetricFn<TestState>,
                _config: &OptimizerConfig,
            ) -> crate::Result<OptimizationResult> {
                Ok(OptimizationResult::new(0.0, 0.0, 0, false, 0.0))
            }

            fn get_optimization_state(&self) -> OptimizationState {
                self.optimization_state.clone()
            }

            fn set_optimization_state(&mut self, state: OptimizationState) {
                self.optimization_state = state;
            }
        }

        let strategy = AppendADemo::default();
        let mut node = TestNode {
            optimization_state: OptimizationState::new(""),
        };

        // Create a bucket with a high-scoring output
        let bucket = vec![SimbaOutput {
            prediction: Some(TestState {
                question: "What is 2+2?".to_string(),
                answer: "4".to_string(),
            }),
            trace: None,
            score: 0.9,
            example: TestState {
                question: "What is 2+2?".to_string(),
                answer: "".to_string(),
            },
            output_metadata: HashMap::new(),
        }];

        let context = StrategyContext {
            batch_10p_score: 0.3,
            batch_90p_score: 0.95,
            max_demos: 4,
        };

        // Apply the strategy
        let result = strategy.apply(&bucket, &mut node, &context).await;
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should return true (demo added)
        assert_eq!(node.get_optimization_state().few_shot_examples.len(), 1);

        // Check that a demo was collected
        let demos = strategy.get_demos();
        assert_eq!(demos.len(), 1);
        assert!(demos[0].output.to_string().contains("4"));
    }

    #[tokio::test]
    async fn test_append_a_demo_skip_low_score() {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        #[derive(Clone)]
        struct TestNode {
            optimization_state: OptimizationState,
        }

        #[async_trait]
        impl Node<TestState> for TestNode {
            async fn execute(&self, state: TestState) -> crate::Result<TestState> {
                Ok(state)
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        #[async_trait]
        impl Optimizable<TestState> for TestNode {
            async fn optimize(
                &mut self,
                _examples: &[TestState],
                _metric: &MetricFn<TestState>,
                _config: &OptimizerConfig,
            ) -> crate::Result<OptimizationResult> {
                Ok(OptimizationResult::new(0.0, 0.0, 0, false, 0.0))
            }

            fn get_optimization_state(&self) -> OptimizationState {
                self.optimization_state.clone()
            }

            fn set_optimization_state(&mut self, state: OptimizationState) {
                self.optimization_state = state;
            }
        }

        let strategy = AppendADemo::default();
        let mut node = TestNode {
            optimization_state: OptimizationState::new(""),
        };

        // Create a bucket with a low-scoring output (at or below 10th percentile)
        let bucket = vec![SimbaOutput {
            prediction: Some(TestState {
                value: "answer".to_string(),
            }),
            trace: None,
            score: 0.3, // At the 10th percentile
            example: TestState {
                value: "test".to_string(),
            },
            output_metadata: HashMap::new(),
        }];

        let context = StrategyContext {
            batch_10p_score: 0.3, // Same as score, should skip
            batch_90p_score: 0.9,
            max_demos: 4,
        };

        // Apply the strategy
        let result = strategy.apply(&bucket, &mut node, &context).await;
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false (skipped)
        assert!(node.get_optimization_state().few_shot_examples.is_empty());

        // No demos should be collected
        assert!(strategy.get_demos().is_empty());
    }

    #[tokio::test]
    async fn test_append_a_rule_skip_no_llm() {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        struct TestState {
            value: String,
        }

        #[derive(Clone)]
        struct TestNode {
            optimization_state: OptimizationState,
        }

        #[async_trait]
        impl Node<TestState> for TestNode {
            async fn execute(&self, state: TestState) -> crate::Result<TestState> {
                Ok(state)
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        #[async_trait]
        impl Optimizable<TestState> for TestNode {
            async fn optimize(
                &mut self,
                _examples: &[TestState],
                _metric: &MetricFn<TestState>,
                _config: &OptimizerConfig,
            ) -> crate::Result<OptimizationResult> {
                Ok(OptimizationResult::new(0.0, 0.0, 0, false, 0.0))
            }

            fn get_optimization_state(&self) -> OptimizationState {
                self.optimization_state.clone()
            }

            fn set_optimization_state(&mut self, state: OptimizationState) {
                self.optimization_state = state;
            }
        }

        let strategy = AppendARule::new(); // No LLM
        let mut node = TestNode {
            optimization_state: OptimizationState::new(""),
        };

        let bucket = vec![
            SimbaOutput {
                prediction: Some(TestState {
                    value: "good".to_string(),
                }),
                trace: None,
                score: 0.9,
                example: TestState {
                    value: "test".to_string(),
                },
                output_metadata: HashMap::new(),
            },
            SimbaOutput {
                prediction: Some(TestState {
                    value: "bad".to_string(),
                }),
                trace: None,
                score: 0.1,
                example: TestState {
                    value: "test".to_string(),
                },
                output_metadata: HashMap::new(),
            },
        ];

        let context = StrategyContext {
            batch_10p_score: 0.2,
            batch_90p_score: 0.8,
            max_demos: 4,
        };

        // Apply the strategy - should skip because no LLM
        let result = strategy.apply(&bucket, &mut node, &context).await;
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false (no LLM)

        // No rules should be collected
        assert!(strategy.get_rules().is_empty());
    }
}
