//! @cli dashflow train rl
//! @cli-status wired
//!
//! # GRPO (Group Relative Policy Optimization)
//!
//! NOTE: This module uses deprecated `TraceCollector` and `TraceEntry` types
//! from the trace module. These are intentionally allowed as they form the
//! core of the optimizer's trace collection API. Migration to ExecutionTrace
//! is tracked separately.

#![allow(deprecated)]
//!
//! GRPO uses online reinforcement learning to optimize language models based on
//! reward signals from a metric function.
//!
//! ## Algorithm Overview
//!
//! GRPO improves language models through reinforcement learning by:
//!
//! 1. **Trace Collection**: Executes the graph with training examples and collects
//!    execution traces via DashStream (decoupled from runtime).
//!
//! 2. **Reward Computation**: Evaluates each trace with a metric function to compute
//!    reward signals (quality scores).
//!
//! 3. **Rollout Generation**: For each training example, generates multiple rollouts
//!    (samples) to estimate expected rewards under current policy.
//!
//! 4. **Group Relative Normalization**: Normalizes rewards within each group (rollouts
//!    for same example) to reduce variance and improve training stability.
//!
//! 5. **Reinforcement Learning**: Creates reward-weighted training examples and submits
//!    them to the LLM's reinforcement learning API (via `ChatModel::reinforce()`).
//!
//! 6. **Model Update**: Waits for training job completion and returns updated model.
//!
//! ## Reference
//!
//! Based on the Arbor paper: "Group Relative Policy Optimization for Reasoning"
//! - <https://arxiv.org/abs/2402.03300>
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::optimize::optimizers::{GRPO, GRPOConfig};
//! use dashflow::optimize::example::Example;
//! use std::sync::Arc;
//!
//! // Define metric function that returns reward scores
//! let metric = Arc::new(|example: &Example, prediction: &Prediction, _trace| {
//!     // Compare prediction to expected output
//!     let expected = example.get("answer").unwrap();
//!     let actual = prediction.get("answer").unwrap();
//!
//!     if expected == actual {
//!         Ok(1.0) // Perfect match
//!     } else {
//!         Ok(0.0) // Incorrect
//!     }
//! });
//!
//! let config = GRPOConfig::default();
//! let grpo = GRPO::new(metric, config);
//!
//! // Optimize will train the model using RL
//! // let optimized_graph = grpo.optimize(graph, trainset).await?;
//! ```

use crate::core::language_models::{ChatModel, ReinforceConfig, ReinforceExample, ReinforceJob};
use crate::core::messages::{BaseMessage, HumanMessage};
use crate::executor::CompiledGraph;
use crate::optimize::example::Example;
use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::optimize::trace::TraceCollector;
use crate::optimize::trace_types::{Prediction, PredictionOrFailed, TraceEntry};
use crate::state::MergeableState;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Metric function for GRPO optimization
///
/// Takes an example, prediction, and optional trace, returns a reward score.
///
/// # Reward Conventions
///
/// - Higher values = better predictions (should be reinforced)
/// - Lower values = worse predictions (should be discouraged)
/// - Typical range: -1.0 to 1.0
/// - Failed predictions should return negative rewards
///
/// # Example
///
/// ```rust,ignore
/// let metric: GRPOMetricFn = Arc::new(|example, prediction, _trace| {
///     let expected = example.get("answer").unwrap();
///     let actual = prediction.get("answer").unwrap();
///
///     if expected == actual {
///         Ok(1.0) // Correct
///     } else {
///         Ok(0.0) // Incorrect
///     }
/// });
/// ```
pub type GRPOMetricFn =
    Arc<dyn Fn(&Example, &Prediction, Option<&Vec<TraceEntry>>) -> Result<f64> + Send + Sync>;

/// GRPO optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GRPOConfig {
    /// Number of training steps/iterations
    pub num_train_steps: usize,

    /// Number of examples to use per training step
    pub num_examples_per_step: usize,

    /// Number of rollouts (samples) per example
    ///
    /// Multiple rollouts per example are used to estimate expected reward
    /// and compute group-relative advantages for policy gradient.
    pub num_rollouts_per_step: usize,

    /// Kafka broker address for DashStream events
    pub kafka_brokers: String,

    /// Kafka topic name for DashStream events
    pub kafka_topic: String,

    /// Reward score for completely failed predictions (errors, timeouts, etc.)
    pub failure_score: f64,

    /// Reward score for format/parsing failures
    pub format_failure_score: f64,

    /// Reinforcement learning configuration (learning rate, batch size, etc.)
    pub reinforce_config: ReinforceConfig,
}

impl Default for GRPOConfig {
    fn default() -> Self {
        Self {
            num_train_steps: 10,
            num_examples_per_step: 4,
            num_rollouts_per_step: 4,
            kafka_brokers: "localhost:9092".to_string(),
            kafka_topic: "dashstream-events".to_string(),
            failure_score: 0.0,
            format_failure_score: -1.0,
            reinforce_config: ReinforceConfig::default(),
        }
    }
}

impl GRPOConfig {
    /// Creates a new GRPO configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the number of training steps.
    #[must_use]
    pub fn with_num_train_steps(mut self, steps: usize) -> Self {
        self.num_train_steps = steps;
        self
    }

    /// Sets the number of examples per training step.
    #[must_use]
    pub fn with_num_examples_per_step(mut self, num: usize) -> Self {
        self.num_examples_per_step = num;
        self
    }

    /// Sets the number of rollouts per training step.
    #[must_use]
    pub fn with_num_rollouts_per_step(mut self, num: usize) -> Self {
        self.num_rollouts_per_step = num;
        self
    }

    /// Sets the Kafka broker addresses for distributed training.
    #[must_use]
    pub fn with_kafka_brokers(mut self, brokers: impl Into<String>) -> Self {
        self.kafka_brokers = brokers.into();
        self
    }

    /// Sets the Kafka topic for publishing training events.
    #[must_use]
    pub fn with_kafka_topic(mut self, topic: impl Into<String>) -> Self {
        self.kafka_topic = topic.into();
        self
    }

    /// Sets the score assigned to failed examples.
    #[must_use]
    pub fn with_failure_score(mut self, score: f64) -> Self {
        self.failure_score = score;
        self
    }

    /// Sets the score assigned to examples with format failures.
    #[must_use]
    pub fn with_format_failure_score(mut self, score: f64) -> Self {
        self.format_failure_score = score;
        self
    }

    /// Sets the REINFORCE algorithm configuration.
    #[must_use]
    pub fn with_reinforce_config(mut self, config: ReinforceConfig) -> Self {
        self.reinforce_config = config;
        self
    }

    /// Validate the configuration.
    ///
    /// Returns a list of validation errors, or `Ok(())` if all values are valid.
    ///
    /// # Validation Rules
    ///
    /// - `num_train_steps` must be > 0
    /// - `num_examples_per_step` must be > 0
    /// - `num_rollouts_per_step` must be > 0
    /// - `kafka_brokers` must not be empty
    /// - `kafka_topic` must not be empty
    pub fn validate(&self) -> std::result::Result<(), Vec<super::ConfigValidationError>> {
        use super::ConfigValidationError;
        let mut errors = Vec::new();

        if self.num_train_steps == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "num_train_steps",
                "Number of training steps must be greater than 0",
                "Set num_train_steps to at least 1",
            ));
        }

        if self.num_examples_per_step == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "num_examples_per_step",
                "Number of examples per step must be greater than 0",
                "Set num_examples_per_step to at least 1",
            ));
        }

        if self.num_rollouts_per_step == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "num_rollouts_per_step",
                "Number of rollouts per step must be greater than 0",
                "Set num_rollouts_per_step to at least 1",
            ));
        }

        if self.kafka_brokers.is_empty() {
            errors.push(ConfigValidationError::with_suggestion(
                "kafka_brokers",
                "Kafka brokers address cannot be empty",
                "Set kafka_brokers to your Kafka cluster address (e.g., localhost:9092)",
            ));
        }

        if self.kafka_topic.is_empty() {
            errors.push(ConfigValidationError::with_suggestion(
                "kafka_topic",
                "Kafka topic cannot be empty",
                "Set kafka_topic to your DashStream events topic",
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// GRPO error types
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GRPOError {
    /// Failed to collect execution traces from DashStream.
    ///
    /// Check that Kafka is running and the DashStream topic exists.
    #[error("Trace collection error: {0}")]
    TraceCollection(#[from] crate::optimize::trace::TraceError),

    /// Failed to evaluate the metric function on the traces.
    ///
    /// The metric function should return a valid reward score.
    #[error("Metric evaluation error: {0}")]
    MetricEvaluation(String),

    /// The reinforcement learning call to the model failed.
    ///
    /// This can occur if the model doesn't support RL or the training job failed.
    #[error("LLM reinforcement learning error: {0}")]
    Reinforce(String),

    /// The GRPO configuration is invalid.
    ///
    /// Check that all required parameters are within valid ranges.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// The RL training job failed to complete.
    #[error("Training failed: {0}")]
    TrainingFailed(String),
}

/// Result type for GRPO operations.
pub type Result<T> = std::result::Result<T, GRPOError>;

/// GRPO (Group Relative Policy Optimization) optimizer
///
/// GRPO uses reinforcement learning to optimize language models based on
/// reward signals. Unlike prompt-engineering optimizers, GRPO actually
/// fine-tunes the model weights through RL.
///
/// # How It Works
///
/// 1. **Execute graph** with training examples
/// 2. **Collect traces** from DashStream (decoupled, zero overhead)
/// 3. **Compute rewards** using metric function
/// 4. **Generate rollouts** (multiple samples per example)
/// 5. **Normalize rewards** within groups (group-relative)
/// 6. **Submit to RL training** via ChatModel::reinforce() API
/// 7. **Poll for completion** and return updated model
///
/// # Design Decisions
///
/// - **DashStream Integration**: Uses DashStream for trace collection instead of
///   runtime interception. This is decoupled, persistent, and zero-overhead.
///
/// - **ChatModel::reinforce() API**: Generic RL API works across providers
///   (OpenAI fine-tuning, local models with trl, etc.)
///
/// - **Simplified Implementation**: This is N=18 (foundation). N=19 will add
///   complete group-relative normalization and advantage computation.
///
/// # Example
///
/// ```rust,ignore
/// let metric = Arc::new(|example, prediction, _trace| {
///     // Compute reward based on correctness
///     Ok(if correct { 1.0 } else { 0.0 })
/// });
///
/// let config = GRPOConfig::default()
///     .with_num_train_steps(10)
///     .with_num_rollouts_per_step(4);
///
/// let grpo = GRPO::new(metric, config);
///
/// // This will:
/// // 1. Execute graph on training data
/// // 2. Collect traces and compute rewards
/// // 3. Generate rollouts
/// // 4. Train model via RL
/// // let optimized = grpo.optimize(graph, trainset, chat_model).await?;
/// ```
pub struct GRPO {
    metric: GRPOMetricFn,
    config: GRPOConfig,
}

impl GRPO {
    /// Create a new GRPO optimizer
    ///
    /// # Arguments
    ///
    /// - `metric`: Function that computes rewards from predictions
    /// - `config`: GRPO configuration (steps, rollouts, etc.)
    ///
    /// # Panics
    ///
    /// Panics if `failure_score <= format_failure_score` (invalid configuration)
    // SAFETY: Panicking constructor with documented behavior; use try_new() for fallible version
    #[allow(clippy::expect_used)]
    pub fn new(metric: GRPOMetricFn, config: GRPOConfig) -> Self {
        Self::try_new(metric, config)
            .expect("failure_score must be greater than format_failure_score")
    }

    /// Create a new GRPO optimizer, returning an error if configuration is invalid.
    ///
    /// # Arguments
    ///
    /// - `metric`: Function that computes rewards from predictions
    /// - `config`: GRPO configuration (steps, rollouts, etc.)
    ///
    /// # Errors
    ///
    /// Returns `GRPOError::InvalidConfig` if `failure_score <= format_failure_score`.
    pub fn try_new(metric: GRPOMetricFn, config: GRPOConfig) -> Result<Self> {
        if config.failure_score <= config.format_failure_score {
            return Err(GRPOError::InvalidConfig(
                "failure_score must be greater than format_failure_score".to_string(),
            ));
        }
        Ok(Self { metric, config })
    }

    /// Collect traces and compute rewards for a set of examples
    ///
    /// This method:
    /// 1. Creates a TraceCollector to consume DashStream events
    /// 2. Collects traces for each thread_id (execution session)
    /// 3. Evaluates traces with the metric function to compute rewards
    /// 4. Converts traces to ReinforceExample format (prompt, completion, reward)
    ///
    /// # Arguments
    ///
    /// - `thread_ids`: List of execution session IDs to collect traces for
    ///
    /// # Returns
    ///
    /// Vec<ReinforceExample> - Training examples with rewards for RL
    async fn collect_traces_with_rewards(
        &self,
        thread_ids: Vec<String>,
        examples: &[Example],
    ) -> Result<Vec<ReinforceExample>> {
        if thread_ids.len() != examples.len() {
            return Err(GRPOError::InvalidConfig(format!(
                "thread_ids and examples must be aligned 1:1 (thread_ids.len()={}, examples.len()={}); \
                 if you have N rollouts per example, pass expanded examples (one per rollout) in the same order as thread_ids",
                thread_ids.len(),
                examples.len()
            )));
        }

        tracing::debug!(
            "Collecting traces and computing rewards for {} executions...",
            thread_ids.len()
        );

        // Create trace collector
        let mut collector =
            TraceCollector::new(&self.config.kafka_brokers, &self.config.kafka_topic)
                .await
                .map_err(|e| {
                    GRPOError::TraceCollection(crate::optimize::trace::TraceError::Consumer(
                        format!(
                            "Failed to create trace collector for Kafka broker '{}': {}",
                            self.config.kafka_brokers, e
                        ),
                    ))
                })?;

        let mut reinforce_examples = Vec::new();

        // Collect traces for all thread_ids in a single pass (optimized)
        // This is O(messages) instead of O(threads * messages)
        let thread_id_set: std::collections::HashSet<String> = thread_ids.iter().cloned().collect();
        let all_traces = collector
            .collect_batch_parallel(thread_id_set)
            .await
            .map_err(|e| {
                GRPOError::TraceCollection(crate::optimize::trace::TraceError::Consumer(format!(
                    "Failed to collect batch traces: {}",
                    e
                )))
            })?;

        // Process collected traces for each thread_id
        for (i, thread_id) in thread_ids.iter().enumerate() {
            let example = &examples[i];

            // Get pre-collected trace from the batch
            let trace_entries = match all_traces.get(thread_id) {
                Some(entries) => entries,
                None => {
                    tracing::warn!("No trace entries found for thread_id: {}", thread_id);
                    continue;
                }
            };

            if trace_entries.is_empty() {
                tracing::warn!("Empty trace entries for thread_id: {}", thread_id);
                continue;
            }

            // Process each trace entry (node execution)
            for trace_entry in trace_entries {
                match &trace_entry.outputs {
                    PredictionOrFailed::Failed(failed) => {
                        tracing::debug!(
                            "Skipping failed prediction in node {}: {}",
                            trace_entry.predictor_name,
                            failed.error
                        );
                        // Could optionally create ReinforceExample with negative reward
                        // for failed predictions to teach model to avoid failures
                        continue;
                    }
                    PredictionOrFailed::Success(prediction) => {
                        // Compute reward using metric function
                        let reward = (self.metric)(example, prediction, Some(trace_entries))
                            .map_err(|e| GRPOError::MetricEvaluation(e.to_string()))?;

                        // Format prompt from inputs
                        let prompt = format_prompt_from_inputs(
                            &trace_entry.predictor_name,
                            &trace_entry.inputs,
                        )?;

                        // Format completion from prediction
                        let completion = format_completion_from_prediction(prediction)?;

                        reinforce_examples.push(ReinforceExample {
                            prompt,
                            completion,
                            reward,
                        });
                    }
                }
            }
        }

        tracing::debug!(
            "Collected {} training examples with rewards",
            reinforce_examples.len()
        );

        Ok(reinforce_examples)
    }

    /// Execute one training step of GRPO
    ///
    /// A training step:
    /// 1. Selects a batch of examples from the training set
    /// 2. Generates multiple rollouts (samples) per example
    /// 3. Executes graph for each rollout (graph execution happens externally)
    /// 4. Collects traces and computes rewards
    ///
    /// # Note
    ///
    /// This is the foundation implementation. Graph execution is assumed to happen
    /// externally and produce thread_ids. A future iteration will integrate graph execution.
    ///
    /// # Arguments
    ///
    /// - `examples`: Training examples for this step
    /// - `thread_ids`: Thread IDs for graph executions (one per rollout)
    /// - `step`: Current step number (for logging)
    ///
    /// # Returns
    ///
    /// Vec<ReinforceExample> - Training data for this step
    async fn training_step(
        &self,
        examples: Vec<Example>,
        thread_ids: Vec<String>,
        step: usize,
    ) -> Result<Vec<ReinforceExample>> {
        tracing::debug!(
            "GRPO training step {}: {} examples, {} rollouts per example",
            step,
            examples.len(),
            self.config.num_rollouts_per_step
        );

        let expanded_examples = if thread_ids.len() == examples.len() {
            examples
        } else if thread_ids.len() == examples.len() * self.config.num_rollouts_per_step {
            let mut expanded = Vec::with_capacity(thread_ids.len());
            for example in examples {
                for _ in 0..self.config.num_rollouts_per_step {
                    expanded.push(example.clone());
                }
            }
            expanded
        } else {
            return Err(GRPOError::InvalidConfig(format!(
                "thread_ids length must match examples length (already-expanded), or be examples.len()*num_rollouts_per_step; \
                 got thread_ids.len()={}, examples.len()={}, num_rollouts_per_step={}",
                thread_ids.len(),
                examples.len(),
                self.config.num_rollouts_per_step
            )));
        };

        // Collect traces with rewards
        self.collect_traces_with_rewards(thread_ids, &expanded_examples)
            .await
    }

    /// Optimize a model using GRPO
    ///
    /// # Note
    ///
    /// This is the foundation API. It requires pre-executed graph runs with
    /// thread_ids. A future iteration will integrate graph execution directly.
    ///
    /// # Arguments
    ///
    /// - `trainset`: Training examples
    /// - `thread_ids_per_step`: Pre-generated thread IDs for each training step
    /// - `chat_model`: The ChatModel to train via RL
    ///
    /// # Returns
    ///
    /// ReinforceJob - Handle for tracking training progress
    pub async fn optimize_with_pregenerated_traces(
        &self,
        trainset: Vec<Example>,
        thread_ids_per_step: Vec<Vec<String>>,
        chat_model: &dyn ChatModel,
    ) -> Result<ReinforceJob> {
        tracing::info!("Starting GRPO optimization...");

        if trainset.is_empty() {
            return Err(GRPOError::InvalidConfig(
                "Training set is empty".to_string(),
            ));
        }

        if thread_ids_per_step.len() != self.config.num_train_steps {
            return Err(GRPOError::InvalidConfig(format!(
                "Expected {} sets of thread_ids, got {}",
                self.config.num_train_steps,
                thread_ids_per_step.len()
            )));
        }

        // M-877 FIX: Warn when trainset is smaller than num_examples_per_step
        if trainset.len() < self.config.num_examples_per_step {
            tracing::warn!(
                "Training set size ({}) is smaller than num_examples_per_step ({}); \
                 examples will be repeated within each step",
                trainset.len(),
                self.config.num_examples_per_step
            );
        }

        tracing::info!(
            "Collecting training data across {} steps...",
            self.config.num_train_steps
        );

        // Collect all training examples across all steps
        let mut all_training_data = Vec::new();
        let mut empty_step_count = 0usize;
        for (step, thread_ids) in thread_ids_per_step.iter().enumerate() {
            let start_idx = (step * self.config.num_examples_per_step) % trainset.len();
            let step_examples: Vec<Example> = (0..self.config.num_examples_per_step)
                .map(|i| trainset[(start_idx + i) % trainset.len()].clone())
                .collect();

            let step_data = self
                .training_step(step_examples, thread_ids.clone(), step)
                .await?;
            // M-878 FIX: Track empty steps for aggregate warning
            if step_data.is_empty() {
                empty_step_count += 1;
                tracing::warn!("Step {} produced no training data", step);
            }
            all_training_data.extend(step_data);
        }
        // M-878 FIX: Warn if significant portion of steps had no data
        if empty_step_count > 0 {
            tracing::warn!(
                "{}/{} steps produced no training data - check trace collection",
                empty_step_count,
                self.config.num_train_steps
            );
        }

        tracing::info!(
            "Collected {} training examples. Starting RL training...",
            all_training_data.len()
        );

        // Start RL training
        let job = chat_model
            .reinforce(all_training_data, self.config.reinforce_config.clone())
            .await
            .map_err(|e| GRPOError::Reinforce(e.to_string()))?;

        tracing::info!("RL training job submitted: {:?}", job.job_id);

        Ok(job)
    }

    /// Normalize rewards within groups (rollouts for same example)
    ///
    /// Group-relative normalization reduces variance in policy gradient estimation
    /// by normalizing rewards within each group of rollouts for the same example.
    ///
    /// # Algorithm
    ///
    /// For each group of rollouts (samples for same training example):
    /// 1. Compute mean and standard deviation of rewards
    /// 2. Normalize: normalized_reward = (reward - mean) / (std + epsilon)
    /// 3. This centers rewards around 0 and scales by variance
    ///
    /// # Arguments
    ///
    /// - `examples`: Training examples grouped by rollout
    /// - `group_size`: Number of rollouts per example (num_rollouts_per_step)
    ///
    /// # Returns
    ///
    /// Vec<ReinforceExample> with normalized rewards
    fn normalize_rewards_by_group(
        &self,
        mut examples: Vec<ReinforceExample>,
        group_size: usize,
    ) -> Vec<ReinforceExample> {
        if group_size <= 1 {
            return examples; // No normalization needed for single rollout
        }

        // M-876 FIX: Use ceiling division to include all examples (partial trailing groups included)
        // Previously used floor division which dropped trailing examples
        let num_groups = examples.len().div_ceil(group_size);
        let trailing_count = examples.len() % group_size;
        if trailing_count > 0 {
            tracing::debug!(
                "Normalizing rewards: {} examples in {} groups (last group has {} examples instead of {})",
                examples.len(),
                num_groups,
                trailing_count,
                group_size
            );
        } else {
            tracing::debug!(
                "Normalizing rewards: {} examples, {} groups of size {}",
                examples.len(),
                num_groups,
                group_size
            );
        }

        for group_idx in 0..num_groups {
            let start_idx = group_idx * group_size;
            let end_idx = (start_idx + group_size).min(examples.len());

            // Collect rewards for this group
            let group_rewards: Vec<f64> = examples[start_idx..end_idx]
                .iter()
                .map(|ex| ex.reward)
                .collect();

            // Compute mean and std dev
            // M-880 FIX: Use sample variance (N-1 denominator, Bessel's correction) for unbiased
            // variance estimation. For groups of size 1, variance is undefined (return 0 std_dev).
            // For groups of size 2+, sample variance provides better estimation when group is
            // a sample from a larger population of possible rollouts.
            let mean = group_rewards.iter().sum::<f64>() / group_rewards.len() as f64;
            let variance = if group_rewards.len() > 1 {
                group_rewards
                    .iter()
                    .map(|r| (r - mean).powi(2))
                    .sum::<f64>()
                    / (group_rewards.len() - 1) as f64 // Sample variance (N-1)
            } else {
                0.0 // Variance undefined for single sample; use 0 (no normalization effect)
            };
            let std_dev = variance.sqrt();

            // Normalize rewards in this group
            let epsilon = 1e-8; // Prevent division by zero
            for example in examples.iter_mut().take(end_idx).skip(start_idx) {
                let normalized = (example.reward - mean) / (std_dev + epsilon);
                example.reward = normalized;
            }

            tracing::trace!(
                "  Group {}: mean={:.3}, std={:.3}",
                group_idx,
                mean,
                std_dev
            );
        }

        examples
    }

    /// Compute advantages for policy gradient
    ///
    /// Advantages measure how much better an action is compared to the average.
    /// This is used in policy gradient methods to reduce variance.
    ///
    /// # Algorithm
    ///
    /// For simplified GRPO implementation:
    /// 1. Group-relative normalization already centers rewards (mean=0)
    /// 2. Normalized rewards serve as advantages
    /// 3. No baseline subtraction needed (mean already 0)
    ///
    /// # Arguments
    ///
    /// - `examples`: Training examples with normalized rewards
    ///
    /// # Returns
    ///
    /// Vec<ReinforceExample> with advantage-based rewards
    ///
    /// # Note
    ///
    /// The Python DashOpt implementation uses more sophisticated advantage
    /// computation with value function baselines. This is a simplified version
    /// that uses group-relative normalization as the advantage signal.
    fn compute_advantages(&self, examples: Vec<ReinforceExample>) -> Vec<ReinforceExample> {
        // After group normalization, rewards are already centered around 0
        // These normalized rewards serve as advantages
        tracing::debug!("Computing advantages (using group-normalized rewards)");
        examples
    }

    /// Test helper: Apply normalization + advantages + reinforce without trace collection
    ///
    /// This method bypasses TraceCollector and accepts pre-generated ReinforceExamples
    /// with raw rewards. It then applies group normalization, advantage computation,
    /// and submits to ChatModel.reinforce().
    ///
    /// **For testing only** - proves GRPO pipeline works without Kafka infrastructure.
    ///
    /// # Arguments
    ///
    /// - `raw_examples`: Training examples with raw (un-normalized) rewards
    /// - `num_rollouts_per_example`: Group size for normalization
    /// - `chat_model`: ChatModel to receive training data
    pub async fn optimize_with_fake_traces(
        &self,
        raw_examples: Vec<ReinforceExample>,
        num_rollouts_per_example: usize,
        chat_model: &dyn ChatModel,
    ) -> Result<ReinforceJob> {
        tracing::debug!("GRPO test mode: Applying normalization + advantages...");

        // Apply group-relative normalization
        let normalized = self.normalize_rewards_by_group(raw_examples, num_rollouts_per_example);

        // Compute advantages
        let with_advantages = self.compute_advantages(normalized);

        tracing::debug!(
            "Submitting {} training examples to ChatModel.reinforce()...",
            with_advantages.len()
        );

        // Submit to RL training
        let job = chat_model
            .reinforce(with_advantages, self.config.reinforce_config.clone())
            .await
            .map_err(|e| GRPOError::Reinforce(e.to_string()))?;

        tracing::info!("RL job created: {}", job.job_id);
        Ok(job)
    }

    /// Execute graph with multiple rollouts per example
    ///
    /// Generates multiple rollouts (samples) for each training example to
    /// estimate expected reward and compute group-relative advantages.
    ///
    /// # Arguments
    ///
    /// - `graph`: Compiled graph to execute
    /// - `examples`: Training examples for this step
    /// - `step`: Current training step number
    ///
    /// # Returns
    ///
    /// (Vec<String>, Vec<Example>) - Thread IDs and duplicated examples
    async fn execute_rollouts<S>(
        &self,
        graph: &CompiledGraph<S>,
        examples: &[Example],
        step: usize,
    ) -> Result<(Vec<String>, Vec<Example>)>
    where
        S: MergeableState + Clone + Send + Sync + 'static,
        S: TryFrom<Example> + Into<Example>,
        <S as TryFrom<Example>>::Error: std::fmt::Display,
    {
        let total_rollouts = examples.len() * self.config.num_rollouts_per_step;
        tracing::debug!(
            "Executing rollouts for step {}: {} examples × {} rollouts = {} total (parallel)",
            step,
            examples.len(),
            self.config.num_rollouts_per_step,
            total_rollouts
        );

        // Build list of all rollouts to execute
        let mut rollout_data: Vec<(String, Example, S)> = Vec::with_capacity(total_rollouts);

        for (example_idx, example) in examples.iter().enumerate() {
            for rollout_idx in 0..self.config.num_rollouts_per_step {
                // Generate unique thread_id for this rollout
                let thread_id =
                    format!("grpo-step{}-ex{}-rollout{}", step, example_idx, rollout_idx);

                // Convert Example to graph state
                let initial_state = S::try_from(example.clone()).map_err(|e| {
                    GRPOError::TrainingFailed(format!("Failed to convert Example to State: {}", e))
                })?;

                rollout_data.push((thread_id, example.clone(), initial_state));
            }
        }

        // Execute all rollouts in parallel
        let futures: Vec<_> = rollout_data
            .iter()
            .map(|(thread_id, _example, initial_state)| {
                let state = initial_state.clone();
                let tid = thread_id.clone();
                async move {
                    graph.invoke(state).await.map_err(|e| {
                        GRPOError::TrainingFailed(format!(
                            "Graph execution failed for thread {}: {}",
                            tid, e
                        ))
                    })
                }
            })
            .collect();

        // Wait for all rollouts to complete
        try_join_all(futures).await?;

        // Extract thread_ids and examples in order
        let thread_ids: Vec<String> = rollout_data.iter().map(|(tid, _, _)| tid.clone()).collect();
        let expanded_examples: Vec<Example> =
            rollout_data.into_iter().map(|(_, ex, _)| ex).collect();

        tracing::debug!("Completed {} parallel rollouts", thread_ids.len());

        Ok((thread_ids, expanded_examples))
    }

    /// Optimize a model using GRPO with full RL algorithm
    ///
    /// This is the complete GRPO implementation with:
    /// - Graph execution integration
    /// - Multiple rollouts per example
    /// - Group-relative reward normalization
    /// - Advantage computation
    ///
    /// # Algorithm
    ///
    /// For each training step:
    /// 1. Select batch of examples
    /// 2. Execute graph with multiple rollouts per example
    /// 3. Collect traces and compute rewards
    /// 4. Normalize rewards within groups (rollouts for same example)
    /// 5. Compute advantages from normalized rewards
    /// 6. Accumulate training data
    ///
    /// After all steps:
    /// 7. Submit all training data to ChatModel::reinforce()
    /// 8. Return ReinforceJob handle for tracking
    ///
    /// # Arguments
    ///
    /// - `graph`: Compiled graph to optimize
    /// - `trainset`: Training examples
    /// - `chat_model`: ChatModel to train via RL
    ///
    /// # Returns
    ///
    /// ReinforceJob - Handle for tracking RL training progress
    ///
    /// # Note
    ///
    /// Requires that the state type S can be converted from Example via `TryFrom<Example>`.
    /// The graph will be executed with DashStream telemetry enabled to collect traces.
    pub async fn optimize<S>(
        &self,
        graph: &CompiledGraph<S>,
        trainset: Vec<Example>,
        chat_model: &dyn ChatModel,
    ) -> Result<ReinforceJob>
    where
        S: MergeableState + Clone + Send + Sync + 'static,
        S: TryFrom<Example> + Into<Example>,
        <S as TryFrom<Example>>::Error: std::fmt::Display,
    {
        tracing::info!("Starting GRPO optimization with full RL algorithm...");

        // Record telemetry start
        record_optimization_start("grpo");
        let optimization_start = std::time::Instant::now();

        if trainset.is_empty() {
            return Err(GRPOError::InvalidConfig(
                "Training set is empty".to_string(),
            ));
        }

        // M-877 FIX: Warn when trainset is smaller than num_examples_per_step
        // (examples will be repeated via modulo wrap-around, which may reduce diversity)
        if trainset.len() < self.config.num_examples_per_step {
            tracing::warn!(
                "Training set size ({}) is smaller than num_examples_per_step ({}); \
                 examples will be repeated within each step, which may reduce training diversity",
                trainset.len(),
                self.config.num_examples_per_step
            );
        }

        tracing::info!(
            "Collecting training data across {} steps...",
            self.config.num_train_steps
        );

        // Collect all training examples across all steps
        let mut all_training_data = Vec::new();
        let mut empty_step_count = 0usize; // M-878 FIX: Track empty steps

        for step in 0..self.config.num_train_steps {
            // Record iteration telemetry
            record_iteration("grpo");

            // Select examples for this step
            let start_idx = (step * self.config.num_examples_per_step) % trainset.len();
            let step_examples: Vec<Example> = (0..self.config.num_examples_per_step)
                .map(|i| trainset[(start_idx + i) % trainset.len()].clone())
                .collect();

            tracing::info!(
                "Step {}/{}: Processing {} examples with {} rollouts each",
                step + 1,
                self.config.num_train_steps,
                step_examples.len(),
                self.config.num_rollouts_per_step
            );

            // Execute rollouts - this will actually run the graph!
            let (thread_ids, expanded_examples) =
                self.execute_rollouts(graph, &step_examples, step).await?;

            // Collect traces with rewards
            let mut step_data = self
                .collect_traces_with_rewards(thread_ids, &expanded_examples)
                .await?;

            if step_data.is_empty() {
                empty_step_count += 1;
                tracing::warn!("No training data collected for step {}", step);
                continue;
            }

            // Apply group-relative normalization
            step_data =
                self.normalize_rewards_by_group(step_data, self.config.num_rollouts_per_step);

            // Compute advantages
            step_data = self.compute_advantages(step_data);

            all_training_data.extend(step_data);
        }

        // M-878 FIX: Log aggregate summary of empty steps
        if empty_step_count > 0 {
            tracing::warn!(
                "{}/{} steps produced no training data - check trace collection and reward scoring",
                empty_step_count,
                self.config.num_train_steps
            );
        }

        if all_training_data.is_empty() {
            return Err(GRPOError::TrainingFailed(
                "No training data collected across all steps".to_string(),
            ));
        }

        // M-881 FIX: Save length before moving all_training_data to avoid unnecessary clone.
        // The reinforce() method takes ownership, but we need the count for telemetry.
        let training_data_count = all_training_data.len();

        tracing::info!(
            "Collected {} training examples. Starting RL training...",
            training_data_count
        );

        // Start RL training
        let job = chat_model
            .reinforce(all_training_data, self.config.reinforce_config.clone())
            .await
            .map_err(|e| GRPOError::Reinforce(e.to_string()))?;

        tracing::info!("RL training job submitted: {:?}", job.job_id);

        // Record telemetry completion
        // Note: GRPO doesn't track score improvement the same way - it's RL-based
        // We record training data size as the "candidates" metric
        let duration = optimization_start.elapsed().as_secs_f64();
        record_optimization_complete(
            "grpo",
            self.config.num_train_steps as u64, // iterations
            training_data_count as u64,         // candidates (training examples)
            0.0,                                // initial_score (N/A for RL)
            0.0,                                // final_score (determined by RL job)
            duration,
        );

        Ok(job)
    }
}

/// Format prompt from node inputs
///
/// Converts input fields to a prompt message list. This is a simple implementation
/// that creates a single HumanMessage with concatenated fields.
///
/// # Arguments
///
/// - `node_name`: Name of the node/predictor
/// - `inputs`: Input fields as JSON values
///
/// # Returns
///
/// Vec<BaseMessage> suitable for LLM training
///
/// # Note on Field Ordering (M-879)
///
/// This function sorts input fields alphabetically before formatting to ensure
/// reproducible prompt generation. Without sorting, HashMap iteration order would
/// be non-deterministic, causing the same inputs to produce different prompts
/// across runs - which could affect both reproducibility and model performance
/// (LLMs can be sensitive to field ordering in prompts).
fn format_prompt_from_inputs(
    node_name: &str,
    inputs: &HashMap<String, serde_json::Value>,
) -> Result<Vec<BaseMessage>> {
    let mut prompt_parts = Vec::new();

    // Add node name as context
    prompt_parts.push(format!("Node: {}", node_name));

    // FIX (M-879): Sort keys alphabetically for reproducible prompt generation
    // HashMap iteration order is non-deterministic, which would cause the same
    // inputs to produce different prompts across runs.
    let mut sorted_keys: Vec<&String> = inputs.keys().collect();
    sorted_keys.sort();

    // Add input fields in sorted order
    for key in sorted_keys {
        if let Some(value) = inputs.get(key) {
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            prompt_parts.push(format!("{}: {}", key, value_str));
        }
    }

    let prompt_text = prompt_parts.join("\n");
    let message: BaseMessage = HumanMessage::new(prompt_text).into();
    Ok(vec![message])
}

/// Format completion from prediction
///
/// Converts prediction output fields to a completion string.
///
/// # Arguments
///
/// - `prediction`: Prediction with output fields
///
/// # Returns
///
/// String completion for LLM training
fn format_completion_from_prediction(prediction: &Prediction) -> Result<String> {
    let mut completion_parts = Vec::new();

    for (key, value) in &prediction.fields {
        let value_str = match value {
            serde_json::Value::String(s) => s.clone(),
            _ => value.to_string(),
        };
        completion_parts.push(format!("{}: {}", key, value_str));
    }

    Ok(completion_parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_metric(
        _example: &Example,
        prediction: &Prediction,
        _trace: Option<&Vec<TraceEntry>>,
    ) -> Result<f64> {
        // Simple metric: return 1.0 if prediction has "answer" field, 0.0 otherwise
        if prediction.get("answer").is_some() {
            Ok(1.0)
        } else {
            Ok(0.0)
        }
    }

    #[test]
    fn test_grpo_config_default() {
        let config = GRPOConfig::default();
        assert_eq!(config.num_train_steps, 10);
        assert_eq!(config.num_examples_per_step, 4);
        assert_eq!(config.num_rollouts_per_step, 4);
        assert!(config.failure_score > config.format_failure_score);
    }

    #[test]
    fn test_grpo_config_builder() {
        let config = GRPOConfig::new()
            .with_num_train_steps(20)
            .with_num_rollouts_per_step(8)
            .with_kafka_brokers("kafka:9092".to_string());

        assert_eq!(config.num_train_steps, 20);
        assert_eq!(config.num_rollouts_per_step, 8);
        assert_eq!(config.kafka_brokers, "kafka:9092");
    }

    #[test]
    fn test_grpo_new() {
        let metric: GRPOMetricFn = Arc::new(test_metric);
        let config = GRPOConfig::default();
        let grpo = GRPO::new(metric, config);

        assert_eq!(grpo.config.num_train_steps, 10);
    }

    #[test]
    fn test_grpo_try_new_valid() {
        let metric: GRPOMetricFn = Arc::new(test_metric);
        let config = GRPOConfig::default();
        let result = GRPO::try_new(metric, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_grpo_try_new_invalid_scores() {
        let metric: GRPOMetricFn = Arc::new(test_metric);
        let config = GRPOConfig {
            failure_score: -2.0,
            format_failure_score: -1.0,
            ..Default::default()
        };
        let result = GRPO::try_new(metric, config);
        assert!(result.is_err());
        match result {
            Err(GRPOError::InvalidConfig(_)) => {} // Expected
            Err(e) => panic!("Expected InvalidConfig error, got: {e}"),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn test_format_prompt_from_inputs() {
        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), serde_json::json!("What is Rust?"));
        inputs.insert(
            "context".to_string(),
            serde_json::json!("programming language"),
        );

        let prompt_messages =
            format_prompt_from_inputs("llm_node", &inputs).expect("Failed to format");

        assert_eq!(prompt_messages.len(), 1);
        match &prompt_messages[0] {
            BaseMessage::Human { content, .. } => {
                let content_str = match content {
                    crate::core::messages::MessageContent::Text(text) => text,
                    _ => panic!("Expected text content"),
                };
                assert!(content_str.contains("Node: llm_node"));
                assert!(content_str.contains("query:"));
                assert!(content_str.contains("What is Rust?"));
                assert!(content_str.contains("context:"));
            }
            _ => panic!("Expected HumanMessage"),
        }
    }

    #[test]
    fn test_format_completion_from_prediction() {
        let pred = Prediction::new()
            .with_field(
                "answer",
                serde_json::json!("A systems programming language"),
            )
            .with_field("confidence", serde_json::json!(0.95));

        let completion =
            format_completion_from_prediction(&pred).expect("Failed to format completion");

        assert!(completion.contains("answer:"));
        assert!(completion.contains("A systems programming language"));
        assert!(completion.contains("confidence:"));
        assert!(completion.contains("0.95"));
    }

    #[test]
    fn test_format_prompt_handles_non_string_values() {
        let mut inputs = HashMap::new();
        inputs.insert("count".to_string(), serde_json::json!(42));
        inputs.insert("enabled".to_string(), serde_json::json!(true));

        let prompt_messages =
            format_prompt_from_inputs("test_node", &inputs).expect("Failed to format");

        assert_eq!(prompt_messages.len(), 1);
        match &prompt_messages[0] {
            BaseMessage::Human { content, .. } => {
                let content_str = match content {
                    crate::core::messages::MessageContent::Text(text) => text,
                    _ => panic!("Expected text content"),
                };
                assert!(content_str.contains("count: 42"));
                assert!(content_str.contains("enabled: true"));
            }
            _ => panic!("Expected HumanMessage"),
        }
    }

    // Note: Integration tests with actual ChatModel would require mocking
    // the full ChatModel trait (including _generate, llm_type, as_any).
    // For now, we test the core logic (format functions, config, etc.)
    // without full end-to-end integration tests.

    #[test]
    fn test_normalize_rewards_by_group() {
        let metric: GRPOMetricFn = Arc::new(test_metric);
        let config = GRPOConfig::default();
        let grpo = GRPO::new(metric, config);

        // Create test examples with different rewards per group
        let examples = vec![
            // Group 1: rewards [1.0, 0.5, 0.0, -0.5]
            ReinforceExample {
                prompt: vec![],
                completion: "a".to_string(),
                reward: 1.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "b".to_string(),
                reward: 0.5,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "c".to_string(),
                reward: 0.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "d".to_string(),
                reward: -0.5,
            },
            // Group 2: rewards [2.0, 1.0]
            ReinforceExample {
                prompt: vec![],
                completion: "e".to_string(),
                reward: 2.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "f".to_string(),
                reward: 1.0,
            },
        ];

        let normalized = grpo.normalize_rewards_by_group(examples, 4);

        // Group 1 (4 examples): mean=0.25, should be normalized
        // Group 2 (2 examples): mean=1.5, should be normalized
        assert_eq!(normalized.len(), 6);

        // Group 1 rewards should be centered around 0
        let group1_rewards: Vec<f64> = normalized[0..4].iter().map(|ex| ex.reward).collect();
        let group1_mean = group1_rewards.iter().sum::<f64>() / 4.0;
        assert!(
            group1_mean.abs() < 1e-6,
            "Group 1 mean should be ~0, got {}",
            group1_mean
        );

        // Highest reward in group 1 should be positive after normalization
        assert!(
            normalized[0].reward > normalized[3].reward,
            "Higher original reward should remain higher after normalization"
        );
    }

    #[test]
    fn test_normalize_rewards_single_rollout() {
        let metric: GRPOMetricFn = Arc::new(test_metric);
        let config = GRPOConfig::default();
        let grpo = GRPO::new(metric, config);

        let examples = vec![
            ReinforceExample {
                prompt: vec![],
                completion: "a".to_string(),
                reward: 1.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "b".to_string(),
                reward: 2.0,
            },
        ];

        // With group_size=1, no normalization should occur
        let normalized = grpo.normalize_rewards_by_group(examples.clone(), 1);
        assert_eq!(normalized[0].reward, 1.0);
        assert_eq!(normalized[1].reward, 2.0);
    }

    #[test]
    fn test_compute_advantages() {
        let metric: GRPOMetricFn = Arc::new(test_metric);
        let config = GRPOConfig::default();
        let grpo = GRPO::new(metric, config);

        let examples = vec![
            ReinforceExample {
                prompt: vec![],
                completion: "a".to_string(),
                reward: 0.5,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "b".to_string(),
                reward: -0.5,
            },
        ];

        // Compute advantages (in our simplified implementation, this is a no-op)
        let advantages = grpo.compute_advantages(examples.clone());
        assert_eq!(advantages.len(), examples.len());
        assert_eq!(advantages[0].reward, 0.5);
        assert_eq!(advantages[1].reward, -0.5);
    }

    #[test]
    fn test_normalize_rewards_preserves_order() {
        let metric: GRPOMetricFn = Arc::new(test_metric);
        let config = GRPOConfig::default();
        let grpo = GRPO::new(metric, config);

        // Group with clearly ordered rewards
        let examples = vec![
            ReinforceExample {
                prompt: vec![],
                completion: "best".to_string(),
                reward: 10.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "good".to_string(),
                reward: 5.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "bad".to_string(),
                reward: 1.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "worst".to_string(),
                reward: -5.0,
            },
        ];

        let normalized = grpo.normalize_rewards_by_group(examples, 4);

        // Order should be preserved after normalization
        assert!(normalized[0].reward > normalized[1].reward);
        assert!(normalized[1].reward > normalized[2].reward);
        assert!(normalized[2].reward > normalized[3].reward);
    }

    #[test]
    fn test_normalize_rewards_zero_variance() {
        let metric: GRPOMetricFn = Arc::new(test_metric);
        let config = GRPOConfig::default();
        let grpo = GRPO::new(metric, config);

        // All rewards identical (zero variance)
        let examples = vec![
            ReinforceExample {
                prompt: vec![],
                completion: "a".to_string(),
                reward: 5.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "b".to_string(),
                reward: 5.0,
            },
            ReinforceExample {
                prompt: vec![],
                completion: "c".to_string(),
                reward: 5.0,
            },
        ];

        let normalized = grpo.normalize_rewards_by_group(examples, 3);

        // With epsilon, should produce finite values near 0
        for ex in &normalized {
            assert!(
                ex.reward.is_finite(),
                "Normalized reward should be finite, got {}",
                ex.reward
            );
        }
    }
}
