// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! # MIPROv2 - Multi-stage Instruction, Prompt, and Demo Optimization
//!
//! MIPROv2 is a sophisticated optimizer that jointly optimizes:
//! 1. Instructions (task descriptions for the LLM)
//! 2. Few-shot demonstrations (examples of input/output pairs)
//! 3. Their combinations (finding the best pairing)
//!
//! ## Algorithm Overview
//!
//! **Stage 1: Bootstrap Few-Shot Examples**
//! - Generate N candidate sets of demonstrations by sampling from trainset
//! - Each set contains up to max_bootstrapped_demos + max_labeled_demos examples
//!
//! **Stage 2: Propose Instruction Candidates**
//! - Use GroundedProposer to generate N instruction variations
//! - Instructions are grounded in dataset characteristics and prompting tips
//!
//! **Stage 3: Optimize Prompt Parameters**
//! - Try different combinations of instructions and demo sets
//! - Use random search (simplified from baseline's Bayesian optimization)
//! - Evaluate each combination and keep the best
//!
//! ## Adaptation from Baseline
//!
//! **Baseline (DashOptimize):**
//! - Works with Module trait (program.predictors(), program.deepcopy())
//! - Uses Optuna for Bayesian optimization (TPESampler)
//! - Supports minibatch evaluation for efficiency
//! - Multi-predictor optimization (optimize all predictors in a program)
//!
//! **Our Version:**
//! - Works with single Signature (not full Module abstraction)
//! - Random search instead of Optuna (simpler, no external dependency)
//! - Full evaluation only (no minibatch complexity)
//! - Single-node optimization (extend to multi-node via GraphOptimizer)
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use dashflow::optimize::{MIPROv2, Signature};
//!
//! let optimizer = MIPROv2::builder()
//!     .auto_mode(AutoMode::Light)
//!     .metric(my_metric_fn)
//!     .build()?;
//!
//! let optimized_signature = optimizer.compile(
//!     &signature,
//!     &trainset,
//!     Some(&valset)
//! ).await?;
//! ```
//!
//! ## References
//!
//! - **Paper**: "Optimizing Instructions and Demonstrations for Multi-Stage Language Model Programs"
//! - **Authors**: Khattab et al. (Stanford/MIT)
//! - **Link**: <https://arxiv.org/abs/2406.11695>
//! - **Published**: June 2024, EMNLP 2024
//! - **Key Result**: Outperforms baselines on 5/7 benchmarks, up to 13% accuracy improvement

use super::OptimizationResult;
use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::optimize::example::Example;
use crate::optimize::propose::{GroundedProposer, GroundedProposerConfig};
use crate::optimize::signature::Signature;
use crate::optimize::telemetry::{record_optimization_complete, record_optimization_start};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing;

// Import shared MetricFn from types module
pub use super::types::MetricFn;

/// Auto-run presets for MIPROv2
///
/// These presets configure num_candidates and validation set size based on
/// computational budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoMode {
    /// Light: Fast optimization (6 candidates, 100 validation examples)
    Light,
    /// Medium: Balanced optimization (12 candidates, 300 validation examples)
    Medium,
    /// Heavy: Thorough optimization (18 candidates, 1000 validation examples)
    Heavy,
}

impl AutoMode {
    /// Get the number of candidates for this mode
    pub fn num_candidates(&self) -> usize {
        match self {
            AutoMode::Light => 6,
            AutoMode::Medium => 12,
            AutoMode::Heavy => 18,
        }
    }

    /// Get the validation set size for this mode
    pub fn val_size(&self) -> usize {
        match self {
            AutoMode::Light => 100,
            AutoMode::Medium => 300,
            AutoMode::Heavy => 1000,
        }
    }
}

/// Configuration for MIPROv2 optimizer
#[derive(Clone)]
pub struct MIPROv2Config {
    /// Metric function for evaluating quality
    pub metric: MetricFn,

    /// Optional LLM for real evaluation (if None, uses mock evaluation)
    pub llm: Option<Arc<dyn ChatModel>>,

    /// Auto-run mode (if set, overrides num_candidates and val_size)
    pub auto_mode: Option<AutoMode>,

    /// Number of instruction candidates to propose (ignored if auto_mode is set)
    pub num_instruct_candidates: Option<usize>,

    /// Number of few-shot candidate sets to generate (ignored if auto_mode is set)
    pub num_fewshot_candidates: Option<usize>,

    /// Number of optimization trials (instruction×demo combinations to try)
    pub num_trials: Option<usize>,

    /// Maximum bootstrapped demonstrations per set
    pub max_bootstrapped_demos: usize,

    /// Maximum labeled demonstrations per set
    pub max_labeled_demos: usize,

    /// Random seed for reproducibility
    pub seed: u64,

    /// Whether to print verbose logs
    pub verbose: bool,

    /// GroundedProposer config (for instruction generation)
    pub proposer_config: GroundedProposerConfig,
}

impl Default for MIPROv2Config {
    fn default() -> Self {
        Self {
            metric: Arc::new(|_pred, _gold| 0.0), // User must provide metric
            llm: None,
            auto_mode: Some(AutoMode::Light),
            num_instruct_candidates: None,
            num_fewshot_candidates: None,
            num_trials: None,
            max_bootstrapped_demos: 4,
            max_labeled_demos: 4,
            seed: 9,
            verbose: false,
            proposer_config: GroundedProposerConfig::default(),
        }
    }
}

impl MIPROv2Config {
    /// Validate the configuration.
    ///
    /// Returns a list of validation errors, or `Ok(())` if all values are valid.
    ///
    /// # Validation Rules
    ///
    /// - `max_bootstrapped_demos` must be > 0
    /// - `max_labeled_demos` must be > 0
    /// - `num_instruct_candidates` if set must be > 0
    /// - `num_fewshot_candidates` if set must be > 0
    /// - `num_trials` if set must be > 0
    pub fn validate(&self) -> Result<(), Vec<super::ConfigValidationError>> {
        use super::ConfigValidationError;
        let mut errors = Vec::new();

        if self.max_bootstrapped_demos == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "max_bootstrapped_demos",
                "Maximum bootstrapped demos must be greater than 0",
                "Set max_bootstrapped_demos to at least 1",
            ));
        }

        if self.max_labeled_demos == 0 {
            errors.push(ConfigValidationError::with_suggestion(
                "max_labeled_demos",
                "Maximum labeled demos must be greater than 0",
                "Set max_labeled_demos to at least 1",
            ));
        }

        if let Some(num) = self.num_instruct_candidates {
            if num == 0 {
                errors.push(ConfigValidationError::with_suggestion(
                    "num_instruct_candidates",
                    "Number of instruction candidates must be greater than 0",
                    "Set num_instruct_candidates to at least 1 or use None with auto_mode",
                ));
            }
        }

        if let Some(num) = self.num_fewshot_candidates {
            if num == 0 {
                errors.push(ConfigValidationError::with_suggestion(
                    "num_fewshot_candidates",
                    "Number of few-shot candidates must be greater than 0",
                    "Set num_fewshot_candidates to at least 1 or use None with auto_mode",
                ));
            }
        }

        if let Some(num) = self.num_trials {
            if num == 0 {
                errors.push(ConfigValidationError::with_suggestion(
                    "num_trials",
                    "Number of trials must be greater than 0",
                    "Set num_trials to at least 1 or use None with auto_mode",
                ));
            }
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

    /// Set the metric function for evaluating quality
    #[must_use]
    pub fn with_metric(mut self, metric: MetricFn) -> Self {
        self.metric = metric;
        self
    }

    /// Set the LLM for real evaluation
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn ChatModel>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Set auto mode (overrides num_candidates and val_size)
    #[must_use]
    pub fn with_auto_mode(mut self, mode: AutoMode) -> Self {
        self.auto_mode = Some(mode);
        self
    }

    /// Set number of instruction candidates to propose
    #[must_use]
    pub const fn with_num_instruct_candidates(mut self, num: usize) -> Self {
        self.num_instruct_candidates = Some(num);
        self
    }

    /// Set number of few-shot candidate sets to generate
    #[must_use]
    pub const fn with_num_fewshot_candidates(mut self, num: usize) -> Self {
        self.num_fewshot_candidates = Some(num);
        self
    }

    /// Set number of optimization trials
    #[must_use]
    pub const fn with_num_trials(mut self, num: usize) -> Self {
        self.num_trials = Some(num);
        self
    }

    /// Set maximum bootstrapped demonstrations per set
    #[must_use]
    pub const fn with_max_bootstrapped_demos(mut self, max: usize) -> Self {
        self.max_bootstrapped_demos = max;
        self
    }

    /// Set maximum labeled demonstrations per set
    #[must_use]
    pub const fn with_max_labeled_demos(mut self, max: usize) -> Self {
        self.max_labeled_demos = max;
        self
    }

    /// Set random seed for reproducibility
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Enable or disable verbose logging
    #[must_use]
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set the GroundedProposer config
    #[must_use]
    pub fn with_proposer_config(mut self, config: GroundedProposerConfig) -> Self {
        self.proposer_config = config;
        self
    }
}

/// Builder for MIPROv2 optimizer
pub struct MIPROv2Builder {
    config: MIPROv2Config,
}

impl MIPROv2Builder {
    /// Create a new builder with default config
    pub fn new() -> Self {
        Self {
            config: MIPROv2Config::default(),
        }
    }

    /// Set the metric function
    pub fn metric<F>(mut self, metric: F) -> Self
    where
        F: Fn(&Example, &Example) -> f64 + Send + Sync + 'static,
    {
        self.config.metric = Arc::new(metric);
        self
    }

    /// Set auto-run mode (Light/Medium/Heavy)
    ///
    /// Mutually exclusive with manual num_candidates/num_trials
    pub fn auto_mode(mut self, mode: AutoMode) -> Self {
        self.config.auto_mode = Some(mode);
        self
    }

    /// Set number of instruction candidates (manual mode only)
    pub fn num_instruct_candidates(mut self, num: usize) -> Self {
        self.config.num_instruct_candidates = Some(num);
        self
    }

    /// Set number of few-shot candidate sets (manual mode only)
    pub fn num_fewshot_candidates(mut self, num: usize) -> Self {
        self.config.num_fewshot_candidates = Some(num);
        self
    }

    /// Set number of optimization trials (manual mode only)
    pub fn num_trials(mut self, num: usize) -> Self {
        self.config.num_trials = Some(num);
        self
    }

    /// Set maximum bootstrapped demonstrations
    pub fn max_bootstrapped_demos(mut self, max: usize) -> Self {
        self.config.max_bootstrapped_demos = max;
        self
    }

    /// Set maximum labeled demonstrations
    pub fn max_labeled_demos(mut self, max: usize) -> Self {
        self.config.max_labeled_demos = max;
        self
    }

    /// Set random seed
    pub fn seed(mut self, seed: u64) -> Self {
        self.config.seed = seed;
        self
    }

    /// Enable verbose logging
    pub fn verbose(mut self) -> Self {
        self.config.verbose = true;
        self
    }

    /// Set GroundedProposer configuration
    pub fn proposer_config(mut self, config: GroundedProposerConfig) -> Self {
        self.config.proposer_config = config;
        self
    }

    /// Set the LLM for real evaluation
    ///
    /// When an LLM is provided, MIPROv2 will call the LLM to generate predictions
    /// for each example in the validation set, then evaluate using the metric function.
    /// Without an LLM, MIPROv2 uses mock evaluation (useful for testing).
    pub fn llm(mut self, llm: Arc<dyn ChatModel>) -> Self {
        self.config.llm = Some(llm);
        self
    }

    /// Build the MIPROv2 optimizer
    pub fn build(self) -> Result<MIPROv2, String> {
        // Validate configuration
        let config = self.config;

        // Run config validation (M-885: now actually called)
        if let Err(errors) = config.validate() {
            let error_msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            return Err(format!(
                "Configuration validation failed: {}",
                error_msgs.join("; ")
            ));
        }

        // Check auto_mode vs manual params
        if config.auto_mode.is_some() {
            if config.num_instruct_candidates.is_some()
                || config.num_fewshot_candidates.is_some()
                || config.num_trials.is_some()
            {
                return Err(
                    "Cannot set both auto_mode and manual parameters (num_instruct_candidates, num_fewshot_candidates, num_trials)".to_string()
                );
            }
        } else {
            // Manual mode - require all parameters
            if config.num_instruct_candidates.is_none()
                || config.num_fewshot_candidates.is_none()
                || config.num_trials.is_none()
            {
                return Err(
                    "When auto_mode is None, must provide num_instruct_candidates, num_fewshot_candidates, and num_trials".to_string()
                );
            }
        }

        Ok(MIPROv2 { config })
    }
}

impl Default for MIPROv2Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// MIPROv2 Optimizer
///
/// Multi-stage instruction, prompt, and demonstration optimization.
pub struct MIPROv2 {
    config: MIPROv2Config,
}

impl MIPROv2 {
    /// Create a new builder
    pub fn builder() -> MIPROv2Builder {
        MIPROv2Builder::new()
    }

    /// Compile (optimize) a signature using trainset and optional valset
    ///
    /// Returns optimized signature with best instruction and demonstrations
    pub async fn compile(
        &self,
        signature: &Signature,
        trainset: &[Example],
        valset: Option<&[Example]>,
    ) -> Result<(Signature, OptimizationResult), String> {
        record_optimization_start("mipro_v2");

        if trainset.is_empty() {
            return Err("Trainset cannot be empty".to_string());
        }

        // Initialize RNG
        let mut rng = StdRng::seed_from_u64(self.config.seed);

        // Split trainset/valset if valset not provided
        let (trainset, valset) = if let Some(v) = valset {
            (trainset, v)
        } else {
            if trainset.len() < 2 {
                return Err(
                    "Trainset must have at least 2 examples if no valset specified".to_string(),
                );
            }
            let val_size = (trainset.len() as f64 * 0.2).ceil() as usize;
            let split_idx = trainset.len() - val_size;
            (&trainset[..split_idx], &trainset[split_idx..])
        };

        // Determine num_candidates and num_trials based on mode
        // Note: valset may be randomly sampled if larger than mode's target size
        let (num_instruct_candidates, num_fewshot_candidates, num_trials, sampled_valset) =
            self.resolve_hyperparameters(valset, &mut rng);
        let valset = &sampled_valset[..];

        if self.config.verbose {
            tracing::info!(
                trainset_size = trainset.len(),
                valset_size = valset.len(),
                num_instruct_candidates,
                num_fewshot_candidates,
                num_trials,
                "MIPROv2 Optimization starting"
            );
        }

        let start_time = std::time::Instant::now();

        // Stage 1: Bootstrap few-shot examples
        let demo_candidates =
            self.bootstrap_fewshot_examples(trainset, num_fewshot_candidates, &mut rng);

        // Stage 2: Propose instruction candidates
        let instruction_candidates = self
            .propose_instructions(
                signature,
                trainset,
                &demo_candidates,
                num_instruct_candidates,
            )
            .await?;

        // Stage 3: Optimize prompt parameters (find best instruction×demo combination)
        let (optimized_signature, final_score, initial_score, iterations) = self
            .optimize_prompt_parameters(
                signature,
                &instruction_candidates,
                &demo_candidates,
                valset,
                num_trials,
                &mut rng,
            )
            .await?;

        let duration_secs = start_time.elapsed().as_secs_f64();

        // Calculate total candidates evaluated
        let total_candidates = (instruction_candidates.len() * demo_candidates.len()) as u64;

        record_optimization_complete(
            "mipro_v2",
            iterations as u64,
            total_candidates,
            initial_score,
            final_score,
            duration_secs,
        );

        let result = OptimizationResult {
            final_score,
            initial_score,
            iterations,
            converged: true, // Random search always completes all trials
            duration_secs,
        };

        if self.config.verbose {
            tracing::info!(
                initial_score = %format!("{:.4}", initial_score),
                final_score = %format!("{:.4}", final_score),
                improvement = %format!("{:.4}", result.improvement()),
                improvement_percent = %format!("{:.1}%", result.improvement_percent()),
                duration_secs = %format!("{:.2}", duration_secs),
                "MIPROv2 Optimization complete"
            );
        }

        Ok((optimized_signature, result))
    }

    /// Resolve hyperparameters based on auto_mode or manual settings
    ///
    /// When `auto_mode` is set and valset exceeds the mode's target size, a random
    /// subset is sampled (using the provided RNG) for more representative evaluation.
    /// Returns owned `Vec<Example>` to support both subsampled and full valset cases.
    fn resolve_hyperparameters(
        &self,
        valset: &[Example],
        rng: &mut StdRng,
    ) -> (usize, usize, usize, Vec<Example>) {
        if let Some(mode) = self.config.auto_mode {
            let num_candidates = mode.num_candidates();
            let val_size = mode.val_size().min(valset.len());

            // M-886/M-888: Properly sample valset using RNG if too large
            let sampled_valset = if valset.len() > val_size {
                // Random sampling without replacement for representative evaluation
                let mut indices: Vec<usize> = (0..valset.len()).collect();
                indices.shuffle(rng);
                indices
                    .into_iter()
                    .take(val_size)
                    .map(|i| valset[i].clone())
                    .collect()
            } else {
                valset.to_vec()
            };

            // For zero-shot optimization, allocate all budget to instructions
            let zeroshot =
                self.config.max_bootstrapped_demos == 0 && self.config.max_labeled_demos == 0;
            let num_instruct_candidates = if zeroshot {
                num_candidates
            } else {
                num_candidates / 2
            };
            let num_fewshot_candidates = num_candidates;

            // Calculate num_trials: max(2 * log2(N), 1.5 * N)
            let num_trials = (2.0 * (num_candidates as f64).log2())
                .max(1.5 * num_candidates as f64)
                .ceil() as usize;

            (
                num_instruct_candidates,
                num_fewshot_candidates,
                num_trials,
                sampled_valset,
            )
        } else {
            // Manual mode: these values must be set when auto_mode is None
            let num_instruct = self
                .config
                .num_instruct_candidates
                .expect("num_instruct_candidates must be set in manual mode (auto_mode=None)");
            let num_fewshot = self
                .config
                .num_fewshot_candidates
                .expect("num_fewshot_candidates must be set in manual mode (auto_mode=None)");
            let num_trials = self
                .config
                .num_trials
                .expect("num_trials must be set in manual mode (auto_mode=None)");
            (num_instruct, num_fewshot, num_trials, valset.to_vec())
        }
    }

    /// Stage 1: Bootstrap few-shot example candidates
    ///
    /// Generates N candidate sets by sampling from trainset.
    fn bootstrap_fewshot_examples(
        &self,
        trainset: &[Example],
        num_candidates: usize,
        rng: &mut StdRng,
    ) -> Vec<Vec<Example>> {
        if self.config.verbose {
            tracing::info!(num_candidates, "Stage 1: Bootstrap fewshot examples");
        }

        let max_demos = self.config.max_bootstrapped_demos + self.config.max_labeled_demos;
        if max_demos == 0 {
            // Zero-shot mode
            return vec![vec![]; num_candidates];
        }

        let mut demo_candidates = Vec::new();

        for i in 0..num_candidates {
            // Sample random subset of trainset
            let num_samples = max_demos.min(trainset.len());
            let mut samples: Vec<Example> = trainset.to_vec();
            samples.shuffle(rng);
            let demo_set = samples.into_iter().take(num_samples).collect();

            if self.config.verbose {
                tracing::debug!(
                    candidate = i,
                    demos = num_samples,
                    "Demo candidate generated"
                );
            }

            demo_candidates.push(demo_set);
        }

        demo_candidates
    }

    /// Stage 2: Propose instruction candidates
    ///
    /// Uses GroundedProposer to generate instruction variations.
    /// If MIPROv2 has an LLM configured, it will be passed to the proposer
    /// for LLM-based instruction generation.
    async fn propose_instructions(
        &self,
        signature: &Signature,
        trainset: &[Example],
        _demo_candidates: &[Vec<Example>],
        num_candidates: usize,
    ) -> Result<Vec<String>, String> {
        if self.config.verbose {
            tracing::info!(num_candidates, "Stage 2: Propose instruction candidates");
        }

        // Create proposer config with LLM from MIPROv2 config
        let mut proposer_config = self.config.proposer_config.clone();
        proposer_config.llm = self.config.llm.clone();

        let proposer = GroundedProposer::new(proposer_config, trainset)
            .await
            .map_err(|e| format!("Failed to create proposer: {}", e))?;

        let instructions = proposer
            .propose_instructions(signature, num_candidates)
            .await
            .map_err(|e| format!("Failed to propose instructions: {}", e))?;

        if self.config.verbose {
            for (i, instruction) in instructions.iter().enumerate() {
                tracing::debug!(index = i, instruction = %instruction, "Proposed instruction");
            }
        }

        Ok(instructions)
    }

    /// Stage 3: Optimize prompt parameters
    ///
    /// Tries different combinations of instructions and demo sets using random search.
    async fn optimize_prompt_parameters(
        &self,
        signature: &Signature,
        instruction_candidates: &[String],
        demo_candidates: &[Vec<Example>],
        valset: &[Example],
        num_trials: usize,
        rng: &mut StdRng,
    ) -> Result<(Signature, f64, f64, usize), String> {
        if self.config.verbose {
            tracing::info!(num_trials, "Stage 3: Optimize prompt parameters");
        }

        // Evaluate default configuration (with first demo set, or empty demos)
        let default_demos = demo_candidates.first().map(|v| v.as_slice()).unwrap_or(&[]);
        let initial_score = self
            .evaluate_signature(signature, default_demos, valset)
            .await;

        if self.config.verbose {
            tracing::debug!(trial = 0, score = %format!("{:.4}", initial_score), "Initial (default) trial");
        }

        let mut best_signature = signature.clone();
        let mut best_score = initial_score;
        let mut best_instruction_idx = 0;
        let mut best_demo_idx = 0;
        let mut best_demos = default_demos.to_vec();

        // Random search over instruction×demo combinations
        for trial in 0..num_trials {
            // Randomly select instruction and demo set
            let instruction_idx = (rng.next_u64() as usize) % instruction_candidates.len();
            let demo_idx = (rng.next_u64() as usize) % demo_candidates.len();

            // Create candidate signature with selected instruction
            let mut candidate_signature = signature.clone();
            candidate_signature.instructions = instruction_candidates[instruction_idx].clone();

            // Get the selected demo set
            let demos = &demo_candidates[demo_idx];

            // Evaluate candidate with both instruction and demos
            let score = self
                .evaluate_signature(&candidate_signature, demos, valset)
                .await;

            if self.config.verbose {
                tracing::debug!(
                    trial = trial + 1,
                    instruction_idx,
                    demo_idx,
                    score = %format!("{:.4}", score),
                    "Trial result"
                );
            }

            // Update best if improved
            if score > best_score {
                best_score = score;
                best_signature = candidate_signature;
                best_instruction_idx = instruction_idx;
                best_demo_idx = demo_idx;
                best_demos = demos.clone();

                if self.config.verbose {
                    tracing::debug!("New best score!");
                }
            }
        }

        // M-887: Log best demos information for debugging/reproducibility
        // The optimized demos are not included in the returned Signature because:
        // 1. Signature is a static template (inputs/outputs/instructions)
        // 2. Demos are runtime context added during prompt building
        // 3. Users should use best_demo_idx to select demos from their candidate sets
        // Future: Consider adding demos to OptimizationResult or a separate return value
        if self.config.verbose {
            tracing::info!(
                instruction_idx = best_instruction_idx,
                demo_idx = best_demo_idx,
                demo_count = best_demos.len(),
                score = %format!("{:.4}", best_score),
                "Best configuration found (use demo_idx to select optimal demos at inference time)"
            );
        }
        // Drop best_demos - see comment above for why they're not returned
        drop(best_demos);

        Ok((best_signature, best_score, initial_score, num_trials))
    }

    /// Evaluate a signature on validation set
    ///
    /// If an LLM is configured, calls the LLM to generate predictions for each example.
    /// Otherwise, uses mock evaluation (useful for testing without LLM API calls).
    async fn evaluate_signature(
        &self,
        signature: &Signature,
        demos: &[Example],
        valset: &[Example],
    ) -> f64 {
        if valset.is_empty() {
            return 0.0;
        }

        let mut total_score = 0.0;

        for example in valset {
            let score = if let Some(ref llm) = self.config.llm {
                // Real LLM evaluation
                match self
                    .evaluate_single_example(llm, signature, demos, example)
                    .await
                {
                    Ok(pred) => (self.config.metric)(&pred, example),
                    Err(e) => {
                        if self.config.verbose {
                            tracing::warn!(error = %e, "LLM call failed during evaluation");
                        }
                        0.0
                    }
                }
            } else {
                // Mock evaluation: use example as both pred and gold
                // This gives perfect score, useful for testing the optimization mechanics
                (self.config.metric)(example, example)
            };
            total_score += score;
        }

        total_score / valset.len() as f64
    }

    /// Evaluate a single example using the LLM
    async fn evaluate_single_example(
        &self,
        llm: &Arc<dyn ChatModel>,
        signature: &Signature,
        demos: &[Example],
        example: &Example,
    ) -> Result<Example, String> {
        // Build prompt from signature, demos, and input
        let prompt = self.build_prompt(signature, demos, example);

        // Call LLM
        let messages = vec![Message::human(prompt)];
        let result = llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| format!("LLM generation failed: {}", e))?;

        // Extract response text
        let response_text = result
            .generations
            .first()
            .map(|g| g.message.content().as_text())
            .unwrap_or_default();

        // Parse response into prediction Example
        self.parse_response(signature, example, &response_text)
    }

    /// Build a prompt from signature, demos, and input example
    fn build_prompt(&self, signature: &Signature, demos: &[Example], example: &Example) -> String {
        let mut prompt = String::new();

        // 1. Add instruction
        if !signature.instructions.is_empty() {
            prompt.push_str(&signature.instructions);
            prompt.push_str("\n\n");
        }

        // 2. Add few-shot demonstrations
        for demo in demos {
            // Input fields
            for field in &signature.input_fields {
                if let Some(value) = demo.get(&field.name) {
                    let value_str = match value {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value_str));
                }
            }

            // Output fields (from demo labels)
            for field in &signature.output_fields {
                if let Some(value) = demo.get(&field.name) {
                    let value_str = match value {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value_str));
                }
            }
            prompt.push('\n');
        }

        // 3. Add current input
        for field in &signature.input_fields {
            if let Some(value) = example.get(&field.name) {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value_str));
            }
        }

        // 4. Add output field prefix (ready for LLM to complete)
        if let Some(first_output) = signature.output_fields.first() {
            prompt.push_str(&format!("{}: ", first_output.get_prefix()));
        }

        prompt
    }

    /// Parse LLM response into an Example with output fields filled in
    fn parse_response(
        &self,
        signature: &Signature,
        input_example: &Example,
        response: &str,
    ) -> Result<Example, String> {
        // Start with the input example's data
        let mut pred = input_example.clone();

        // Simple parsing: use response as the first output field
        if let Some(first_output) = signature.output_fields.first() {
            let value = response.trim().to_string();
            pred = pred.with(&first_output.name, value);
        }

        Ok(pred)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_match_metric(pred: &Example, gold: &Example) -> f64 {
        // Simple exact match metric
        let pred_answer = pred.get("answer").and_then(|v| v.as_str()).unwrap_or("");
        let gold_answer = gold.get("answer").and_then(|v| v.as_str()).unwrap_or("");
        if pred_answer == gold_answer {
            1.0
        } else {
            0.0
        }
    }

    #[test]
    fn test_auto_mode_values() {
        assert_eq!(AutoMode::Light.num_candidates(), 6);
        assert_eq!(AutoMode::Light.val_size(), 100);

        assert_eq!(AutoMode::Medium.num_candidates(), 12);
        assert_eq!(AutoMode::Medium.val_size(), 300);

        assert_eq!(AutoMode::Heavy.num_candidates(), 18);
        assert_eq!(AutoMode::Heavy.val_size(), 1000);
    }

    #[test]
    fn test_builder_auto_mode() {
        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .build()
            .unwrap();

        assert_eq!(optimizer.config.auto_mode, Some(AutoMode::Light));
    }

    #[test]
    fn test_builder_manual_mode() {
        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .num_instruct_candidates(10)
            .num_fewshot_candidates(10)
            .num_trials(20)
            .metric(exact_match_metric)
            .build();

        // Should fail because auto_mode + manual params are mutually exclusive
        assert!(optimizer.is_err());
    }

    #[test]
    fn test_builder_manual_mode_valid() {
        // Remove auto_mode to use manual mode
        let mut builder = MIPROv2::builder();
        builder.config.auto_mode = None;

        let optimizer = builder
            .num_instruct_candidates(10)
            .num_fewshot_candidates(10)
            .num_trials(20)
            .metric(exact_match_metric)
            .build()
            .unwrap();

        assert_eq!(optimizer.config.auto_mode, None);
        assert_eq!(optimizer.config.num_instruct_candidates, Some(10));
        assert_eq!(optimizer.config.num_fewshot_candidates, Some(10));
        assert_eq!(optimizer.config.num_trials, Some(20));
    }

    #[test]
    fn test_bootstrap_fewshot_examples() {
        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .max_bootstrapped_demos(3)
            .max_labeled_demos(1)
            .build()
            .unwrap();

        let trainset: Vec<Example> = (0..10)
            .map(|i| {
                Example::new()
                    .with("question", format!("Q{}", i))
                    .with("answer", format!("A{}", i))
            })
            .collect();

        let mut rng = StdRng::seed_from_u64(42);
        let demo_candidates = optimizer.bootstrap_fewshot_examples(&trainset, 5, &mut rng);

        assert_eq!(demo_candidates.len(), 5);
        for demos in &demo_candidates {
            assert!(demos.len() <= 4); // max_bootstrapped + max_labeled
        }
    }

    #[tokio::test]
    async fn test_compile_basic() {
        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .seed(42)
            .build()
            .unwrap();

        let signature =
            Signature::new("question -> answer").with_instructions("Answer the question");

        let trainset: Vec<Example> = vec![
            Example::new()
                .with("question", "What is 2+2?".to_string())
                .with("answer", "4".to_string()),
            Example::new()
                .with("question", "What is 3+3?".to_string())
                .with("answer", "6".to_string()),
        ];

        let valset: Vec<Example> = vec![Example::new()
            .with("question", "What is 1+1?".to_string())
            .with("answer", "2".to_string())];

        let result = optimizer
            .compile(&signature, &trainset, Some(&valset))
            .await;
        assert!(result.is_ok());

        let (optimized_sig, opt_result) = result.unwrap();
        assert!(!optimized_sig.instructions.is_empty());
        assert_eq!(opt_result.iterations, 9); // Light mode: max(2*log2(6), 1.5*6) = 9
    }

    // Mock ChatModel for testing LLM-based evaluation
    use crate::core::callbacks::CallbackManager;
    use crate::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use crate::core::messages::BaseMessage;
    use async_trait::async_trait;

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

    #[test]
    fn test_build_prompt_basic() {
        use crate::optimize::signature::make_signature;

        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .build()
            .unwrap();

        let signature = make_signature("question -> answer", "Answer the question").unwrap();

        let example = Example::new()
            .with("question", "What is 2+2?")
            .with_inputs(&["question"]);

        let prompt = optimizer.build_prompt(&signature, &[], &example);

        assert!(prompt.contains("Answer the question"));
        assert!(prompt.contains("Question: What is 2+2?"));
        assert!(prompt.contains("Answer:"));
    }

    #[test]
    fn test_build_prompt_with_demos() {
        use crate::optimize::signature::make_signature;

        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .build()
            .unwrap();

        let signature = make_signature("question -> answer", "Answer the question").unwrap();

        let demos = vec![
            Example::new()
                .with("question", "What is 1+1?")
                .with("answer", "2"),
            Example::new()
                .with("question", "What is 3+3?")
                .with("answer", "6"),
        ];

        let example = Example::new()
            .with("question", "What is 5+5?")
            .with_inputs(&["question"]);

        let prompt = optimizer.build_prompt(&signature, &demos, &example);

        // Check instruction
        assert!(prompt.contains("Answer the question"));

        // Check demos are included
        assert!(prompt.contains("What is 1+1?"));
        assert!(prompt.contains("Answer: 2"));
        assert!(prompt.contains("What is 3+3?"));
        assert!(prompt.contains("Answer: 6"));

        // Check current input
        assert!(prompt.contains("What is 5+5?"));
    }

    #[test]
    fn test_parse_response() {
        use crate::optimize::signature::make_signature;

        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .build()
            .unwrap();

        let signature = make_signature("question -> answer", "").unwrap();

        let input = Example::new()
            .with("question", "What is 2+2?")
            .with_inputs(&["question"]);

        let result = optimizer
            .parse_response(&signature, &input, "  4  ")
            .unwrap();

        assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("4"));
        assert_eq!(
            result.get("question").and_then(|v| v.as_str()),
            Some("What is 2+2?")
        );
    }

    #[tokio::test]
    async fn test_compile_with_mock_llm() {
        // Mock LLM that always returns "4"
        let mock_llm = Arc::new(MockChatModel::new("4"));

        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .llm(mock_llm)
            .seed(42)
            .build()
            .unwrap();

        let signature =
            Signature::new("question -> answer").with_instructions("Answer the question");

        // All answers are "4" so LLM should get 50% correct (2 out of 4)
        let trainset: Vec<Example> = vec![
            Example::new()
                .with("question", "What is 2+2?")
                .with("answer", "4"),
            Example::new()
                .with("question", "What is 3+3?")
                .with("answer", "6"),
        ];

        let valset: Vec<Example> = vec![
            Example::new()
                .with("question", "What is 1+1?")
                .with("answer", "4"), // LLM will get this right
            Example::new()
                .with("question", "What is 5+5?")
                .with("answer", "10"), // LLM will get this wrong
        ];

        let result = optimizer
            .compile(&signature, &trainset, Some(&valset))
            .await;
        assert!(result.is_ok());

        let (_optimized_sig, opt_result) = result.unwrap();
        // With mock LLM returning "4", should get 50% correct
        assert!(opt_result.final_score >= 0.0 && opt_result.final_score <= 1.0);
    }

    #[tokio::test]
    async fn test_evaluate_signature_with_llm() {
        use crate::optimize::signature::make_signature;

        let mock_llm = Arc::new(MockChatModel::new("correct_answer"));

        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .llm(mock_llm)
            .build()
            .unwrap();

        let signature = make_signature("question -> answer", "").unwrap();

        let valset = vec![
            Example::new()
                .with("question", "Q1")
                .with("answer", "correct_answer"),
            Example::new()
                .with("question", "Q2")
                .with("answer", "wrong_answer"),
        ];

        let score = optimizer.evaluate_signature(&signature, &[], &valset).await;

        // One correct, one wrong = 50%
        assert!((score - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_evaluate_signature_without_llm_uses_mock() {
        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            // No LLM set - uses mock evaluation
            .build()
            .unwrap();

        let signature = Signature::new("question -> answer");

        let valset = vec![Example::new().with("question", "Q1").with("answer", "A1")];

        // Mock evaluation uses example as both pred and gold, so always 100%
        let score = optimizer.evaluate_signature(&signature, &[], &valset).await;

        assert!((score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_builder_with_llm() {
        let mock_llm = Arc::new(MockChatModel::new("test"));

        let optimizer = MIPROv2::builder()
            .auto_mode(AutoMode::Light)
            .metric(exact_match_metric)
            .llm(mock_llm)
            .build()
            .unwrap();

        assert!(optimizer.config.llm.is_some());
    }

    // -------------------------------------------------------------------------
    // MIPROv2Config builder tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_miprov2_config_default() {
        let config = MIPROv2Config::new();
        assert!(config.llm.is_none());
        assert!(config.auto_mode.is_some());
        assert!(config.num_instruct_candidates.is_none());
        assert!(config.num_fewshot_candidates.is_none());
        assert!(config.num_trials.is_none());
        assert_eq!(config.max_bootstrapped_demos, 4);
        assert_eq!(config.max_labeled_demos, 4);
        assert_eq!(config.seed, 9);
        assert!(!config.verbose);
    }

    #[test]
    fn test_miprov2_config_full_builder() {
        let mock_llm = Arc::new(MockChatModel::new("test"));

        let config = MIPROv2Config::new()
            .with_llm(mock_llm)
            .with_auto_mode(AutoMode::Medium)
            .with_num_instruct_candidates(5)
            .with_num_fewshot_candidates(3)
            .with_num_trials(10)
            .with_max_bootstrapped_demos(8)
            .with_max_labeled_demos(2)
            .with_seed(42)
            .with_verbose(true);

        assert!(config.llm.is_some());
        assert_eq!(config.auto_mode, Some(AutoMode::Medium));
        assert_eq!(config.num_instruct_candidates, Some(5));
        assert_eq!(config.num_fewshot_candidates, Some(3));
        assert_eq!(config.num_trials, Some(10));
        assert_eq!(config.max_bootstrapped_demos, 8);
        assert_eq!(config.max_labeled_demos, 2);
        assert_eq!(config.seed, 42);
        assert!(config.verbose);
    }

    #[test]
    fn test_miprov2_config_partial_builder() {
        let config = MIPROv2Config::new()
            .with_seed(123)
            .with_max_bootstrapped_demos(6)
            .with_verbose(true);

        assert_eq!(config.seed, 123);
        assert_eq!(config.max_bootstrapped_demos, 6);
        assert!(config.verbose);
        // Defaults preserved
        assert!(config.auto_mode.is_some());
        assert_eq!(config.max_labeled_demos, 4);
    }

    #[test]
    fn test_miprov2_config_with_metric() {
        // Create a custom metric wrapped in Arc
        let custom_metric: MetricFn = Arc::new(|pred, gold| {
            // Simple comparison: 1.0 if answers match, 0.0 otherwise
            let pred_answer = pred.get("answer").and_then(|v| v.as_str()).unwrap_or("");
            let gold_answer = gold.get("answer").and_then(|v| v.as_str()).unwrap_or("");
            if pred_answer == gold_answer {
                1.0
            } else {
                0.0
            }
        });

        let config = MIPROv2Config::new().with_metric(custom_metric);

        // Verify metric works
        let example = Example::new().with("answer", "test");
        let score = (config.metric)(&example, &example);
        assert!((score - 1.0).abs() < f64::EPSILON); // exact match = 1.0
    }
}
