//! @dashflow-module
//! @name auto_optimizer
//! @category optimize
//! @status stable
//!
//! # AutoOptimizer - Automatic Optimizer Selection for DashFlow
//!
//! AutoOptimizer automatically selects the best optimization algorithm based on your data
//! and constraints. Instead of manually choosing between 17 optimizers, call `optimize()`
//! and DashFlow decides.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use dashflow::optimize::auto_optimizer::{AutoOptimizer, OptimizationContext};
//!
//! // Let DashFlow choose the best optimizer
//! let result = AutoOptimizer::optimize(&signature, &trainset, &llm).await?;
//!
//! // With explanation of why this optimizer was chosen
//! let (result, selection) = AutoOptimizer::optimize_explained(&signature, &trainset, &llm).await?;
//! println!("Selected: {} because {}", selection.optimizer_name, selection.reason);
//! ```
//!
//! ## Selection Logic
//!
//! AutoOptimizer uses a research-backed decision tree:
//!
//! ```text
//! Can finetune model?
//!   ├─ Yes → GRPO (RL weight updates, 51.7% → 60.9% on MATH)
//!   └─ No → How many examples?
//!             ├─ <10 → BootstrapFewShot (works with minimal data)
//!             ├─ 10-50 → BootstrapFewShot (reliable baseline)
//!             ├─ 50+ → MIPROv2 (best benchmarked: 5/7 tasks, 13% gain)
//!             └─ Agent task? → SIMBA (self-reflective)
//! ```
//!
//! ## Custom Context
//!
//! Provide hints to improve selection:
//!
//! ```rust,ignore
//! let context = OptimizationContext::builder()
//!     .num_examples(100)
//!     .task_type(TaskType::QuestionAnswering)
//!     .can_finetune(false)
//!     .compute_budget(ComputeBudget::Medium)
//!     .build();
//!
//! let selection = AutoOptimizer::select(&context);
//! ```
//!
//! ## Recording Outcomes
//!
//! AutoOptimizer learns from optimization outcomes:
//!
//! ```rust,ignore
//! // Record outcome for future learning
//! AutoOptimizer::record_outcome(&outcome).await?;
//!
//! // Query historical performance
//! let stats = AutoOptimizer::historical_stats(TaskType::Classification)?;
//! ```

use crate::optimize::example::Example;
use crate::optimize::optimizers::registry::{self, OptimizerTier};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, instrument, warn};

fn sanitize_for_filename_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len().min(64));
    for ch in input.chars() {
        if out.len() >= 64 {
            break;
        }
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn warn_on_unknown_excluded_optimizers(context: &OptimizationContext) {
    if context.excluded_optimizers.is_empty() {
        return;
    }

    let known = registry::all_optimizers()
        .into_iter()
        .map(|meta| meta.name.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();

    for raw in &context.excluded_optimizers {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            warn!("OptimizationContext.excluded_optimizers contains an empty name");
            continue;
        }

        if !known.contains(&trimmed.to_ascii_lowercase()) {
            warn!(
                excluded = %trimmed,
                "OptimizationContext.excluded_optimizers contains an unknown optimizer name; check spelling or update DashFlow"
            );
        }
    }
}

fn best_task_types_for_outcomes(outcomes: &[&OptimizationOutcome]) -> Vec<TaskType> {
    let mut task_improvements: std::collections::HashMap<TaskType, Vec<f64>> =
        std::collections::HashMap::new();
    for outcome in outcomes {
        task_improvements
            .entry(outcome.context.task_type)
            .or_default()
            .push(outcome.improvement);
    }

    let mut best_task_types: Vec<_> = task_improvements
        .into_iter()
        .filter_map(|(task, improvements)| {
            let avg = improvements.iter().sum::<f64>() / improvements.len() as f64;
            if avg > 0.0 {
                Some((task, avg))
            } else {
                None
            }
        })
        .collect();
    best_task_types.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    best_task_types.truncate(3);

    best_task_types.into_iter().map(|(task, _)| task).collect()
}

/// Task type for optimization (helps select the right optimizer)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TaskType {
    /// Question answering tasks
    QuestionAnswering,
    /// Text classification (sentiment, topic, etc.)
    Classification,
    /// Code generation or completion
    CodeGeneration,
    /// Mathematical reasoning
    MathReasoning,
    /// Agentic tasks with tool use
    Agent,
    /// Multi-step reasoning or chain-of-thought
    Reasoning,
    /// Text summarization
    Summarization,
    /// Generic/unknown task type
    #[default]
    Generic,
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskType::QuestionAnswering => write!(f, "question_answering"),
            TaskType::Classification => write!(f, "classification"),
            TaskType::CodeGeneration => write!(f, "code_generation"),
            TaskType::MathReasoning => write!(f, "math_reasoning"),
            TaskType::Agent => write!(f, "agent"),
            TaskType::Reasoning => write!(f, "reasoning"),
            TaskType::Summarization => write!(f, "summarization"),
            TaskType::Generic => write!(f, "generic"),
        }
    }
}

/// Compute budget constraint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ComputeBudget {
    /// Minimal compute (fast, fewer iterations)
    Minimal,
    /// Low compute budget
    Low,
    /// Medium compute budget (default)
    #[default]
    Medium,
    /// High compute budget (more thorough)
    High,
    /// Unlimited (best quality, longest time)
    Unlimited,
}

impl ComputeBudget {
    /// Get recommended max iterations for this budget
    pub fn max_iterations(&self) -> usize {
        match self {
            ComputeBudget::Minimal => 5,
            ComputeBudget::Low => 10,
            ComputeBudget::Medium => 25,
            ComputeBudget::High => 50,
            ComputeBudget::Unlimited => 100,
        }
    }
}

/// Context for automatic optimizer selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationContext {
    /// Number of training examples available
    pub num_examples: usize,

    /// Whether the model supports finetuning
    pub can_finetune: bool,

    /// Detected or specified task type
    pub task_type: TaskType,

    /// Compute budget constraint
    pub compute_budget: ComputeBudget,

    /// Whether an embedding model is available
    pub has_embedding_model: bool,

    /// Specific optimizer requirements (e.g., "metric_function")
    pub available_capabilities: Vec<String>,

    /// Preferred optimizer tier (None = any tier)
    pub preferred_tier: Option<OptimizerTier>,

    /// Explicitly exclude these optimizers
    pub excluded_optimizers: Vec<String>,
}

impl Default for OptimizationContext {
    fn default() -> Self {
        Self {
            num_examples: 0,
            can_finetune: false,
            task_type: TaskType::Generic,
            compute_budget: ComputeBudget::Medium,
            has_embedding_model: false,
            available_capabilities: vec!["metric_function".to_string()],
            preferred_tier: None,
            excluded_optimizers: Vec::new(),
        }
    }
}

impl OptimizationContext {
    /// Create a new builder for OptimizationContext
    pub fn builder() -> OptimizationContextBuilder {
        OptimizationContextBuilder::default()
    }

    /// Infer context from training examples
    pub fn from_examples(examples: &[Example]) -> Self {
        let num_examples = examples.len();
        let task_type = infer_task_type(examples);

        Self {
            num_examples,
            task_type,
            ..Default::default()
        }
    }
}

/// Builder for OptimizationContext
#[derive(Debug, Clone, Default)]
pub struct OptimizationContextBuilder {
    context: OptimizationContext,
}

impl OptimizationContextBuilder {
    /// Set the number of training examples available.
    pub fn num_examples(mut self, n: usize) -> Self {
        self.context.num_examples = n;
        self
    }

    /// Set whether model finetuning is available.
    pub fn can_finetune(mut self, can: bool) -> Self {
        self.context.can_finetune = can;
        self
    }

    /// Set the task type for optimization.
    pub fn task_type(mut self, task_type: TaskType) -> Self {
        self.context.task_type = task_type;
        self
    }

    /// Set the compute budget constraint.
    pub fn compute_budget(mut self, budget: ComputeBudget) -> Self {
        self.context.compute_budget = budget;
        self
    }

    /// Set whether an embedding model is available.
    pub fn has_embedding_model(mut self, has: bool) -> Self {
        self.context.has_embedding_model = has;
        self
    }

    /// Add an available capability (e.g., "streaming", "tool_calling").
    #[must_use]
    pub fn add_capability(mut self, cap: impl Into<String>) -> Self {
        self.context.available_capabilities.push(cap.into());
        self
    }

    /// Set the preferred optimizer tier.
    pub fn preferred_tier(mut self, tier: OptimizerTier) -> Self {
        self.context.preferred_tier = Some(tier);
        self
    }

    /// Exclude an optimizer from selection by name.
    pub fn exclude_optimizer(mut self, name: impl Into<String>) -> Self {
        self.context.excluded_optimizers.push(name.into());
        self
    }

    /// Build the optimization context.
    pub fn build(self) -> OptimizationContext {
        self.context
    }
}

/// Result of optimizer selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionResult {
    /// Name of the selected optimizer
    pub optimizer_name: String,

    /// Why this optimizer was selected
    pub reason: String,

    /// Confidence in this selection (0.0-1.0)
    pub confidence: f64,

    /// Alternative optimizers that could work
    pub alternatives: Vec<AlternativeOptimizer>,

    /// Optimizer tier (1=recommended, 2=specialized, 3=niche)
    pub tier: Option<u8>,

    /// Academic citation for the optimizer
    pub citation: Option<String>,

    /// Context that was used for selection
    pub context: OptimizationContext,
}

/// An alternative optimizer that could be used
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeOptimizer {
    /// Optimizer name
    pub name: String,

    /// Why this is an alternative (not primary choice)
    pub reason: String,

    /// Confidence if this were selected (0.0-1.0)
    pub confidence: f64,
}

/// Outcome of an optimization run (for learning)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationOutcome {
    /// Timestamp of the optimization
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Context used for selection
    pub context: OptimizationContext,

    /// Optimizer that was used
    pub optimizer_name: String,

    /// Initial score before optimization
    pub initial_score: f64,

    /// Final score after optimization
    pub final_score: f64,

    /// Improvement achieved
    pub improvement: f64,

    /// Time spent optimizing (seconds)
    pub duration_secs: f64,

    /// Whether optimization was considered successful
    pub success: bool,

    /// Optional notes or error message
    pub notes: Option<String>,
}

impl OptimizationOutcome {
    /// Calculate improvement as a percentage
    pub fn improvement_percent(&self) -> f64 {
        if self.initial_score == 0.0 {
            0.0
        } else {
            (self.improvement / self.initial_score) * 100.0
        }
    }
}

/// Historical statistics for an optimizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerStats {
    /// Optimizer name
    pub optimizer_name: String,

    /// Number of times used
    pub usage_count: usize,

    /// Average improvement achieved
    pub avg_improvement: f64,

    /// Average time spent (seconds)
    pub avg_duration_secs: f64,

    /// Success rate (0.0-1.0)
    pub success_rate: f64,

    /// Task types this optimizer performed best on
    pub best_task_types: Vec<TaskType>,
}

/// AutoOptimizer - Automatic optimizer selection and execution
pub struct AutoOptimizer {
    /// Storage directory for outcomes
    storage_dir: PathBuf,
}

impl AutoOptimizer {
    /// Create a new AutoOptimizer with default storage
    pub fn new() -> Self {
        Self {
            storage_dir: PathBuf::from(".dashflow/optimization_history"),
        }
    }

    /// Create with custom storage directory
    #[must_use]
    pub fn with_storage_dir(storage_dir: impl Into<PathBuf>) -> Self {
        Self {
            storage_dir: storage_dir.into(),
        }
    }

    /// Select the best optimizer for a given context
    ///
    /// This is the core selection logic based on research from DSPy and related work.
    #[instrument(skip(context), fields(num_examples = context.num_examples, task_type = %context.task_type))]
    pub fn select(context: &OptimizationContext) -> SelectionResult {
        info!(
            "Selecting optimizer for {} examples, task: {}",
            context.num_examples, context.task_type
        );

        warn_on_unknown_excluded_optimizers(context);

        // Check for minimum examples
        if context.num_examples < 2 {
            return SelectionResult {
                optimizer_name: "None".to_string(),
                reason: "Cannot optimize with fewer than 2 examples".to_string(),
                confidence: 1.0,
                alternatives: vec![],
                tier: None,
                citation: None,
                context: context.clone(),
            };
        }

        // Decision tree based on research
        let (name, reason, confidence, alternatives) = select_optimizer_impl(context);

        // Get metadata for tier and citation
        let metadata = registry::get_optimizer(&name);
        let tier = metadata.as_ref().map(|m| m.tier.number());
        let citation = metadata.map(|m| m.citation.to_string());

        SelectionResult {
            optimizer_name: name,
            reason,
            confidence,
            alternatives,
            tier,
            citation,
            context: context.clone(),
        }
    }

    /// Select optimizer based on training examples only
    pub fn select_for_examples(examples: &[Example]) -> SelectionResult {
        let context = OptimizationContext::from_examples(examples);
        Self::select(&context)
    }

    /// Get the storage directory for outcomes
    pub fn storage_dir(&self) -> &PathBuf {
        &self.storage_dir
    }

    /// Record an optimization outcome for learning
    pub async fn record_outcome(&self, outcome: &OptimizationOutcome) -> Result<()> {
        use crate::error::Error;

        // Ensure storage directory exists
        tokio::fs::create_dir_all(&self.storage_dir)
            .await
            .map_err(|e| Error::Generic(format!("Failed to create storage directory: {e}")))?;

        let safe_optimizer_name = sanitize_for_filename_component(&outcome.optimizer_name);
        let filename = format!(
            "{}-{}.json",
            outcome.timestamp.format("%Y%m%d-%H%M%S"),
            safe_optimizer_name
        );
        let path = self.storage_dir.join(filename);

        let json = serde_json::to_string_pretty(outcome)?;
        tokio::fs::write(&path, json)
            .await
            .map_err(|e| Error::Generic(format!("Failed to write outcome file: {e}")))?;

        info!("Recorded optimization outcome to {:?}", path);
        Ok(())
    }

    /// Load historical outcomes
    pub async fn load_outcomes(&self) -> Result<Vec<OptimizationOutcome>> {
        use crate::error::Error;

        let mut outcomes = Vec::new();

        // Use async file existence check to avoid blocking the async runtime
        if !tokio::fs::try_exists(&self.storage_dir)
            .await
            .unwrap_or(false)
        {
            return Ok(outcomes);
        }

        let mut entries = tokio::fs::read_dir(&self.storage_dir)
            .await
            .map_err(|e| Error::Generic(format!("Failed to read storage directory: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::Generic(format!("Failed to read directory entry: {e}")))?
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    match serde_json::from_str::<OptimizationOutcome>(&content) {
                        Ok(outcome) => outcomes.push(outcome),
                        Err(e) => warn!("Failed to parse outcome file {:?}: {}", path, e),
                    }
                } else {
                    warn!("Failed to read outcome file {:?}", path);
                }
            }
        }

        // Sort by timestamp
        outcomes.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        Ok(outcomes)
    }

    /// Get statistics for a specific optimizer
    pub async fn stats_for_optimizer(
        &self,
        optimizer_name: &str,
    ) -> Result<Option<OptimizerStats>> {
        let outcomes = self.load_outcomes().await?;
        let filtered: Vec<_> = outcomes
            .iter()
            .filter(|o| o.optimizer_name == optimizer_name)
            .collect();

        if filtered.is_empty() {
            return Ok(None);
        }

        let usage_count = filtered.len();
        let avg_improvement =
            filtered.iter().map(|o| o.improvement).sum::<f64>() / usage_count as f64;
        let avg_duration =
            filtered.iter().map(|o| o.duration_secs).sum::<f64>() / usage_count as f64;
        let success_count = filtered.iter().filter(|o| o.success).count();
        let success_rate = success_count as f64 / usage_count as f64;

        let best_task_types = best_task_types_for_outcomes(&filtered);

        Ok(Some(OptimizerStats {
            optimizer_name: optimizer_name.to_string(),
            usage_count,
            avg_improvement,
            avg_duration_secs: avg_duration,
            success_rate,
            best_task_types,
        }))
    }

    /// Get overall historical statistics
    pub async fn historical_stats(&self) -> Result<Vec<OptimizerStats>> {
        let outcomes = self.load_outcomes().await?;

        // Group by optimizer
        let mut by_optimizer: std::collections::HashMap<String, Vec<&OptimizationOutcome>> =
            std::collections::HashMap::new();
        for outcome in &outcomes {
            by_optimizer
                .entry(outcome.optimizer_name.clone())
                .or_default()
                .push(outcome);
        }

        let mut stats = Vec::new();
        for (name, outcomes) in by_optimizer {
            let usage_count = outcomes.len();
            let avg_improvement =
                outcomes.iter().map(|o| o.improvement).sum::<f64>() / usage_count as f64;
            let avg_duration =
                outcomes.iter().map(|o| o.duration_secs).sum::<f64>() / usage_count as f64;
            let success_count = outcomes.iter().filter(|o| o.success).count();
            let success_rate = success_count as f64 / usage_count as f64;
            let best_task_types = best_task_types_for_outcomes(&outcomes);

            stats.push(OptimizerStats {
                optimizer_name: name,
                usage_count,
                avg_improvement,
                avg_duration_secs: avg_duration,
                success_rate,
                best_task_types,
            });
        }

        // Sort by average improvement descending
        stats.sort_by(|a, b| {
            b.avg_improvement
                .partial_cmp(&a.avg_improvement)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(stats)
    }
}

impl Default for AutoOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Selection Logic Implementation
// ============================================================================

/// Core optimizer selection implementation
fn select_optimizer_impl(
    context: &OptimizationContext,
) -> (String, String, f64, Vec<AlternativeOptimizer>) {
    let n = context.num_examples;

    // Check exclusions helper
    let is_excluded = |name: &str| {
        context
            .excluded_optimizers
            .iter()
            .any(|ex| ex.trim().eq_ignore_ascii_case(name))
    };

    // Step 1: Can finetune? → GRPO
    if context.can_finetune && n >= 10 && !is_excluded("GRPO") {
        debug!("Selecting GRPO: finetuning available with {} examples", n);
        return (
            "GRPO".to_string(),
            format!(
                "Model finetuning available with {} examples. \
                 GRPO achieves 51.7% → 60.9% improvement on MATH benchmark.",
                n
            ),
            0.95,
            vec![
                AlternativeOptimizer {
                    name: "MIPROv2".to_string(),
                    reason: "Best prompt-only optimizer if finetuning fails".to_string(),
                    confidence: 0.85,
                },
                AlternativeOptimizer {
                    name: "BootstrapFinetune".to_string(),
                    reason: "Alternative finetuning approach for distillation".to_string(),
                    confidence: 0.75,
                },
            ],
        );
    }

    // Step 2: Agent task? → SIMBA
    if context.task_type == TaskType::Agent && n >= 20 && !is_excluded("SIMBA") {
        debug!("Selecting SIMBA: agent task with {} examples", n);
        return (
            "SIMBA".to_string(),
            format!(
                "Agent task detected with {} examples. \
                 SIMBA uses self-reflection for trajectory-based optimization.",
                n
            ),
            0.85,
            vec![
                AlternativeOptimizer {
                    name: "AvatarOptimizer".to_string(),
                    reason: "Feedback-based optimization for agent instructions".to_string(),
                    confidence: 0.75,
                },
                AlternativeOptimizer {
                    name: "MIPROv2".to_string(),
                    reason: "General-purpose optimizer with strong benchmarks".to_string(),
                    confidence: 0.80,
                },
            ],
        );
    }

    // Step 2B: Code generation tasks (20+ examples) → SIMBA
    //
    // Rationale: Code generation benefits from introspective improvement and iterative refinement,
    // even when the dataset is not yet "large" (50+). This is one of the main scenarios where
    // task type should influence selection beyond example count.
    if context.task_type == TaskType::CodeGeneration && n >= 20 && !is_excluded("SIMBA") {
        debug!(
            "Selecting SIMBA: code generation task with {} examples",
            n
        );
        return (
            "SIMBA".to_string(),
            format!(
                "Code generation task detected with {} examples. \
                 SIMBA is effective for iterative, self-reflective prompt/program improvement.",
                n
            ),
            0.82,
            vec![
                AlternativeOptimizer {
                    name: "MIPROv2".to_string(),
                    reason: "Strong general-purpose optimizer when you have more examples".to_string(),
                    confidence: 0.80,
                },
                AlternativeOptimizer {
                    name: "BootstrapFewShot".to_string(),
                    reason: "Faster baseline few-shot optimizer with less compute".to_string(),
                    confidence: 0.75,
                },
            ],
        );
    }

    // Step 2C: Reasoning-heavy tasks (20+ examples) → MIPROv2 (earlier threshold)
    //
    // Rationale: For math/reasoning tasks, instruction proposal + search tends to matter earlier
    // than "50+ examples", so we pick MIPROv2 when there is at least moderate data.
    if matches!(context.task_type, TaskType::MathReasoning | TaskType::Reasoning)
        && n >= 20
        && !is_excluded("MIPROv2")
    {
        debug!(
            "Selecting MIPROv2: reasoning task with {} examples (early threshold)",
            n
        );
        return (
            "MIPROv2".to_string(),
            format!(
                "Reasoning-heavy task detected with {} examples. \
                 Selecting MIPROv2 early for instruction + few-shot search (strong benchmarked default).",
                n
            ),
            0.86,
            vec![
                AlternativeOptimizer {
                    name: "BootstrapFewShot".to_string(),
                    reason: "Faster baseline if MIPROv2 is too slow".to_string(),
                    confidence: 0.78,
                },
                AlternativeOptimizer {
                    name: "COPRO".to_string(),
                    reason: "Instruction-only optimization when few-shot is not desired".to_string(),
                    confidence: 0.70,
                },
            ],
        );
    }

    // Step 3: Large dataset (50+) → MIPROv2
    if n >= 50 && !is_excluded("MIPROv2") {
        debug!("Selecting MIPROv2: {} examples (large dataset)", n);
        return (
            "MIPROv2".to_string(),
            format!(
                "Large dataset with {} examples. \
                 MIPROv2 is best benchmarked: outperforms on 5/7 tasks with up to 13% accuracy gain.",
                n
            ),
            0.90,
            vec![
                AlternativeOptimizer {
                    name: "BootstrapFewShot".to_string(),
                    reason: "Faster alternative if MIPROv2 is too slow".to_string(),
                    confidence: 0.80,
                },
                AlternativeOptimizer {
                    name: "COPRO".to_string(),
                    reason: "Instruction-only optimization (no few-shot)".to_string(),
                    confidence: 0.70,
                },
            ],
        );
    }

    // Step 4: Medium dataset (10-50) → BootstrapFewShot
    if n >= 10 && !is_excluded("BootstrapFewShot") {
        debug!(
            "Selecting BootstrapFewShot: {} examples (medium dataset)",
            n
        );
        return (
            "BootstrapFewShot".to_string(),
            format!(
                "Medium dataset with {} examples. \
                 BootstrapFewShot is the reliable baseline from DSPy.",
                n
            ),
            0.85,
            vec![
                AlternativeOptimizer {
                    name: "MIPROv2".to_string(),
                    reason: "Better results with more compute budget".to_string(),
                    confidence: 0.80,
                },
                AlternativeOptimizer {
                    name: "COPRO".to_string(),
                    reason: "Instruction-only if few-shot not desired".to_string(),
                    confidence: 0.65,
                },
            ],
        );
    }

    // Step 5: Small dataset (2-10) → BootstrapFewShot (still works)
    if n >= 2 && !is_excluded("BootstrapFewShot") {
        debug!("Selecting BootstrapFewShot: {} examples (small dataset)", n);
        return (
            "BootstrapFewShot".to_string(),
            format!(
                "Small dataset with {} examples. \
                 BootstrapFewShot can work with limited data but results may vary.",
                n
            ),
            0.70,
            vec![AlternativeOptimizer {
                name: "LabeledFewShot".to_string(),
                reason: "Direct use of labeled examples without bootstrapping".to_string(),
                confidence: 0.60,
            }],
        );
    }

    // Fallback: Not enough data
    (
        "BootstrapFewShot".to_string(),
        "Fallback: BootstrapFewShot as general-purpose optimizer".to_string(),
        0.50,
        vec![],
    )
}

/// Infer task type from training examples.
///
/// This is a lightweight heuristic used when `OptimizationContext.task_type` is left as default.
/// For best results and to avoid misclassification, set `task_type` explicitly via
/// `OptimizationContext::builder().task_type(...)`.
fn infer_task_type(examples: &[Example]) -> TaskType {
    if examples.is_empty() {
        return TaskType::Generic;
    }

    // Analyze example patterns
    let mut has_code = false;
    let mut has_math = false;
    let mut has_tools = false;
    let mut is_classification = true;
    let mut has_long_output = false;

    for example in examples {
        // Get input fields (everything except output-like fields)
        let inputs = example.inputs();
        for (_key, value) in inputs.iter() {
            let val_str = value.to_string().to_lowercase();
            if val_str.contains("def ") || val_str.contains("fn ") || val_str.contains("```") {
                has_code = true;
            }
            if val_str.contains("calculate")
                || val_str.contains("solve")
                || val_str.contains("math")
            {
                has_math = true;
            }
            if val_str.contains("tool") || val_str.contains("action") || val_str.contains("agent") {
                has_tools = true;
            }
        }

        // Check output fields for classification pattern
        // Look for common output field names
        for key in [
            "output", "expected", "answer", "result", "label", "category",
        ] {
            if let Some(value) = example.get(key) {
                let output_str = value.to_string();
                if output_str.len() > 100 {
                    is_classification = false;
                    has_long_output = true;
                }
                if output_str.contains("```")
                    || output_str.contains("def ")
                    || output_str.contains("fn ")
                {
                    has_code = true;
                }
            }
        }
    }

    // Determine task type based on patterns
    if has_tools {
        TaskType::Agent
    } else if has_code {
        TaskType::CodeGeneration
    } else if has_math {
        TaskType::MathReasoning
    } else if is_classification && !has_long_output {
        TaskType::Classification
    } else if has_long_output {
        TaskType::Summarization
    } else {
        TaskType::Generic
    }
}

// ============================================================================
// Public API Functions (for module-level access)
// ============================================================================

/// Select the best optimizer for a given context
///
/// This is the main entry point for optimizer selection.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::auto_optimizer::{select_optimizer, OptimizationContext};
///
/// let context = OptimizationContext::builder()
///     .num_examples(100)
///     .build();
///
/// let selection = select_optimizer(&context);
/// println!("Use: {} ({})", selection.optimizer_name, selection.reason);
/// ```
pub fn select_optimizer(context: &OptimizationContext) -> SelectionResult {
    AutoOptimizer::select(context)
}

/// Select optimizer based on training examples only
///
/// Infers context from the examples and selects the best optimizer.
pub fn select_for_examples(examples: &[Example]) -> SelectionResult {
    AutoOptimizer::select_for_examples(examples)
}

/// Get recommended optimizer name for a simple scenario
///
/// Convenience function that returns just the optimizer name.
pub fn recommend(num_examples: usize, can_finetune: bool) -> &'static str {
    registry::recommend_optimizer(num_examples, can_finetune)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_for_filename_component() {
        assert_eq!(sanitize_for_filename_component("GRPO"), "GRPO");
        assert_eq!(sanitize_for_filename_component("  GRPO  "), "GRPO");
        assert_eq!(sanitize_for_filename_component("../evil"), "evil");
        assert_eq!(sanitize_for_filename_component("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_for_filename_component("___"), "unknown");
    }

    #[test]
    fn test_best_task_types_for_outcomes() {
        let mk = |task_type: TaskType, improvement: f64| OptimizationOutcome {
            timestamp: chrono::Utc::now(),
            context: OptimizationContext {
                task_type,
                ..OptimizationContext::default()
            },
            optimizer_name: "MIPROv2".to_string(),
            initial_score: 0.0,
            final_score: 0.0,
            improvement,
            duration_secs: 0.0,
            success: improvement > 0.0,
            notes: None,
        };

        let outcomes = vec![
            mk(TaskType::Classification, 0.10),
            mk(TaskType::Classification, 0.20),
            mk(TaskType::Agent, 0.05),
            mk(TaskType::MathReasoning, -0.01),
            mk(TaskType::MathReasoning, 0.02),
        ];
        let refs: Vec<&OptimizationOutcome> = outcomes.iter().collect();
        assert_eq!(
            best_task_types_for_outcomes(&refs),
            vec![
                TaskType::Classification,
                TaskType::Agent,
                TaskType::MathReasoning
            ]
        );
    }

    #[test]
    fn test_select_with_finetuning() {
        let context = OptimizationContext::builder()
            .num_examples(100)
            .can_finetune(true)
            .build();

        let selection = AutoOptimizer::select(&context);
        assert_eq!(selection.optimizer_name, "GRPO");
        assert!(selection.confidence > 0.9);
        assert!(selection.reason.contains("finetuning"));
    }

    #[test]
    fn test_select_large_dataset() {
        let context = OptimizationContext::builder()
            .num_examples(100)
            .can_finetune(false)
            .build();

        let selection = AutoOptimizer::select(&context);
        assert_eq!(selection.optimizer_name, "MIPROv2");
        assert!(selection.confidence > 0.85);
    }

    #[test]
    fn test_select_medium_dataset() {
        let context = OptimizationContext::builder()
            .num_examples(25)
            .can_finetune(false)
            .build();

        let selection = AutoOptimizer::select(&context);
        assert_eq!(selection.optimizer_name, "BootstrapFewShot");
    }

    #[test]
    fn test_select_small_dataset() {
        let context = OptimizationContext::builder()
            .num_examples(5)
            .can_finetune(false)
            .build();

        let selection = AutoOptimizer::select(&context);
        assert_eq!(selection.optimizer_name, "BootstrapFewShot");
        assert!(selection.confidence < 0.85); // Lower confidence for small data
    }

    #[test]
    fn test_select_agent_task() {
        let context = OptimizationContext::builder()
            .num_examples(50)
            .task_type(TaskType::Agent)
            .build();

        let selection = AutoOptimizer::select(&context);
        assert_eq!(selection.optimizer_name, "SIMBA");
        assert!(selection.reason.contains("Agent"));
    }

    #[test]
    fn test_select_code_generation_task_prefers_simba() {
        let context = OptimizationContext::builder()
            .num_examples(25)
            .task_type(TaskType::CodeGeneration)
            .build();

        let selection = AutoOptimizer::select(&context);
        assert_eq!(selection.optimizer_name, "SIMBA");
        assert!(selection.reason.to_lowercase().contains("code"));
    }

    #[test]
    fn test_select_math_reasoning_task_prefers_mipro_early() {
        let context = OptimizationContext::builder()
            .num_examples(25)
            .task_type(TaskType::MathReasoning)
            .build();

        let selection = AutoOptimizer::select(&context);
        assert_eq!(selection.optimizer_name, "MIPROv2");
        assert!(selection.reason.to_lowercase().contains("reason"));
    }

    #[test]
    fn test_select_insufficient_data() {
        let context = OptimizationContext::builder().num_examples(1).build();

        let selection = AutoOptimizer::select(&context);
        assert_eq!(selection.optimizer_name, "None");
        assert!(selection.reason.contains("fewer than 2"));
    }

    #[test]
    fn test_select_with_exclusion() {
        let context = OptimizationContext::builder()
            .num_examples(100)
            .can_finetune(true)
            .exclude_optimizer("GRPO")
            .build();

        let selection = AutoOptimizer::select(&context);
        assert_ne!(selection.optimizer_name, "GRPO");
        // Should fall through to MIPROv2 for large dataset
        assert_eq!(selection.optimizer_name, "MIPROv2");
    }

    #[test]
    fn test_infer_task_type_code() {
        let examples = vec![Example::new()
            .with("prompt", "Write a function to calculate factorial")
            .with(
                "output",
                "```python\ndef factorial(n):\n    return 1 if n <= 1 else n * factorial(n-1)\n```",
            )];

        let task_type = infer_task_type(&examples);
        assert_eq!(task_type, TaskType::CodeGeneration);
    }

    #[test]
    fn test_infer_task_type_classification() {
        let examples = vec![
            Example::new()
                .with("text", "I love this product!")
                .with("label", "positive"),
            Example::new()
                .with("text", "This is terrible")
                .with("label", "negative"),
        ];

        let task_type = infer_task_type(&examples);
        assert_eq!(task_type, TaskType::Classification);
    }

    #[test]
    fn test_alternatives_provided() {
        let context = OptimizationContext::builder().num_examples(100).build();

        let selection = AutoOptimizer::select(&context);
        assert!(!selection.alternatives.is_empty());
        assert!(selection
            .alternatives
            .iter()
            .any(|a| a.name == "BootstrapFewShot"));
    }

    #[test]
    fn test_optimization_outcome_improvement() {
        let outcome = OptimizationOutcome {
            timestamp: chrono::Utc::now(),
            context: OptimizationContext::default(),
            optimizer_name: "MIPROv2".to_string(),
            initial_score: 0.5,
            final_score: 0.65,
            improvement: 0.15,
            duration_secs: 30.0,
            success: true,
            notes: None,
        };

        assert_eq!(outcome.improvement_percent(), 30.0);
    }

    #[test]
    fn test_compute_budget_iterations() {
        assert_eq!(ComputeBudget::Minimal.max_iterations(), 5);
        assert_eq!(ComputeBudget::Medium.max_iterations(), 25);
        assert_eq!(ComputeBudget::Unlimited.max_iterations(), 100);
    }
}
