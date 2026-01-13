//! Agent runner with checkpointing and streaming support
//!
//! This module provides a high-level API for running the agent with:
//! - Optional checkpointing for session persistence
//! - Session resume capability
//! - Turn-based execution
//! - Streaming telemetry for real-time visibility
//! - Training data collection for prompt optimization
//!
//! ## Checkpointing Backends
//!
//! The runner supports multiple checkpointing backends:
//! - Memory: In-process, non-persistent (default)
//! - File: Persistent to local filesystem
//! - PostgreSQL: Production-ready persistent storage (requires `postgres` feature)
//!
//! ### PostgreSQL Checkpointing
//!
//! Enable with the `postgres` feature and provide a connection string:
//! ```toml
//! codex-dashflow-core = { version = "0.1", features = ["postgres"] }
//! ```
//!
//! Connection string format: `host=localhost user=postgres password=secret dbname=codex`

use std::sync::Arc;

use dashflow::checkpoint::Checkpointer;
use dashflow::{FileCheckpointer, MemoryCheckpointer};

#[cfg(feature = "postgres")]
use dashflow_postgres_checkpointer::PostgresCheckpointer;

use crate::graph::{build_agent_graph, build_agent_graph_manifest};
use crate::optimize::{PromptRegistry, TrainingData};
use crate::project_doc::ProjectDocOptions;
use crate::state::{AgentState, CompletionStatus};
use crate::streaming::{AgentEvent, NullStreamCallback, StreamCallback};
use crate::Result;

/// Checkpoint retention policy (Audit #87)
///
/// Controls automatic cleanup of old checkpoints to prevent unbounded
/// disk usage. Retention can be based on count (per thread) or age.
#[derive(Clone, Debug, Default)]
pub struct CheckpointRetentionPolicy {
    /// Maximum checkpoints to keep per thread (0 = unlimited)
    /// When exceeded, oldest checkpoints are deleted after each save.
    pub max_checkpoints_per_thread: usize,
    /// Maximum age of checkpoints in seconds (0 = unlimited)
    /// Checkpoints older than this are deleted during cleanup.
    pub max_age_seconds: u64,
    /// Run cleanup after every N saves (0 = never auto-cleanup)
    /// Set to 1 for cleanup after every save, higher values for less frequent cleanup.
    pub cleanup_interval: usize,
}

impl CheckpointRetentionPolicy {
    /// Create a policy keeping only the N most recent checkpoints per thread
    pub fn keep_latest(count: usize) -> Self {
        Self {
            max_checkpoints_per_thread: count,
            cleanup_interval: 1, // Cleanup after every save
            ..Default::default()
        }
    }

    /// Create a policy that keeps checkpoints for a duration
    pub fn keep_for_duration(seconds: u64) -> Self {
        Self {
            max_age_seconds: seconds,
            cleanup_interval: 10, // Less frequent cleanup for time-based
            ..Default::default()
        }
    }

    /// Create a policy with both count and age limits
    pub fn with_limits(max_count: usize, max_age_seconds: u64) -> Self {
        Self {
            max_checkpoints_per_thread: max_count,
            max_age_seconds,
            cleanup_interval: 1,
        }
    }
}

/// Configuration for the agent runner
#[derive(Clone)]
pub struct RunnerConfig {
    /// Whether to enable checkpointing
    pub enable_checkpointing: bool,
    /// Path for file-based checkpointing (None = use memory checkpointer)
    pub checkpoint_path: Option<std::path::PathBuf>,
    /// PostgreSQL connection string for database checkpointing (requires `postgres` feature)
    /// Format: `host=localhost user=postgres password=secret dbname=codex`
    pub postgres_connection_string: Option<String>,
    /// Maximum turns before stopping (0 = unlimited)
    pub max_turns: u32,
    /// Stream callback for telemetry events
    stream_callback: Arc<dyn StreamCallback>,
    /// Whether to collect training data from this run
    pub collect_training: bool,
    /// Custom system prompt (loaded from PromptRegistry or user-specified)
    /// If None, defaults will be loaded from PromptRegistry::load_default()
    pub system_prompt: Option<String>,
    /// Whether to auto-load optimized prompts from default location
    pub load_optimized_prompts: bool,
    /// Options for loading project documentation (AGENTS.md discovery)
    /// If None, project docs are not loaded
    pub project_doc_options: Option<ProjectDocOptions>,
    /// Checkpoint retention policy (Audit #87)
    /// Controls automatic cleanup of old checkpoints
    pub checkpoint_retention: Option<CheckpointRetentionPolicy>,
}

// Manual Debug implementation since Arc<dyn StreamCallback> doesn't impl Debug
impl std::fmt::Debug for RunnerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunnerConfig")
            .field("enable_checkpointing", &self.enable_checkpointing)
            .field("checkpoint_path", &self.checkpoint_path)
            .field(
                "postgres_connection_string",
                &self.postgres_connection_string.is_some(),
            )
            .field("max_turns", &self.max_turns)
            .field("has_stream_callback", &true)
            .field("collect_training", &self.collect_training)
            .field("system_prompt", &self.system_prompt.is_some())
            .field("load_optimized_prompts", &self.load_optimized_prompts)
            .field("project_doc_options", &self.project_doc_options.is_some())
            .field("checkpoint_retention", &self.checkpoint_retention)
            .finish()
    }
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            enable_checkpointing: false,
            checkpoint_path: None,
            postgres_connection_string: None,
            max_turns: 0,
            stream_callback: Arc::new(NullStreamCallback),
            collect_training: false,
            system_prompt: None,
            load_optimized_prompts: false,
            project_doc_options: None,
            checkpoint_retention: None,
        }
    }
}

impl RunnerConfig {
    /// Create a config with memory checkpointing enabled
    pub fn with_memory_checkpointing() -> Self {
        Self {
            enable_checkpointing: true,
            ..Default::default()
        }
    }

    /// Create a config with file-based checkpointing
    pub fn with_file_checkpointing(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            enable_checkpointing: true,
            checkpoint_path: Some(path.into()),
            ..Default::default()
        }
    }

    /// Create a config with PostgreSQL checkpointing (requires `postgres` feature)
    ///
    /// # Arguments
    /// * `connection_string` - PostgreSQL connection string
    ///   Format: `host=localhost user=postgres password=secret dbname=codex`
    ///
    /// # Example
    /// ```rust,ignore
    /// let config = RunnerConfig::with_postgres_checkpointing(
    ///     "host=localhost user=postgres password=secret dbname=codex"
    /// );
    /// ```
    pub fn with_postgres_checkpointing(connection_string: impl Into<String>) -> Self {
        Self {
            enable_checkpointing: true,
            postgres_connection_string: Some(connection_string.into()),
            ..Default::default()
        }
    }

    /// Set the maximum turns
    pub fn with_max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Set the stream callback for telemetry events
    pub fn with_stream_callback(mut self, callback: Arc<dyn StreamCallback>) -> Self {
        self.stream_callback = callback;
        self
    }

    /// Get the stream callback
    pub fn stream_callback(&self) -> Arc<dyn StreamCallback> {
        self.stream_callback.clone()
    }

    /// Enable training data collection
    pub fn with_collect_training(mut self, collect: bool) -> Self {
        self.collect_training = collect;
        self
    }

    /// Set a custom system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Enable auto-loading of optimized prompts from default location
    pub fn with_load_optimized_prompts(mut self, load: bool) -> Self {
        self.load_optimized_prompts = load;
        self
    }

    /// Enable project documentation discovery (AGENTS.md)
    pub fn with_project_doc_options(mut self, options: ProjectDocOptions) -> Self {
        self.project_doc_options = Some(options);
        self
    }

    /// Enable project documentation discovery with default options for given working directory
    pub fn with_project_docs(mut self, working_dir: impl Into<std::path::PathBuf>) -> Self {
        self.project_doc_options = Some(ProjectDocOptions::new(working_dir.into()));
        self
    }

    /// Set checkpoint retention policy (Audit #87)
    ///
    /// Controls automatic cleanup of old checkpoints. Only applies to file-based
    /// and PostgreSQL checkpointing (memory checkpoints are already bounded by process lifetime).
    ///
    /// # Example
    /// ```rust,ignore
    /// let config = RunnerConfig::with_file_checkpointing("/tmp/checkpoints")
    ///     .with_checkpoint_retention(CheckpointRetentionPolicy::keep_latest(10));
    /// ```
    pub fn with_checkpoint_retention(mut self, policy: CheckpointRetentionPolicy) -> Self {
        self.checkpoint_retention = Some(policy);
        self
    }

    /// Resolve the system prompt to use
    ///
    /// Priority:
    /// 1. Explicit system_prompt if set
    /// 2. Loaded from PromptRegistry if load_optimized_prompts is true
    /// 3. None (use default from optimize module)
    pub fn resolve_system_prompt(&self) -> Option<String> {
        if let Some(ref prompt) = self.system_prompt {
            return Some(prompt.clone());
        }

        if self.load_optimized_prompts {
            if let Ok(registry) = PromptRegistry::load_default() {
                let prompt = registry.get_system_prompt();
                // Only use if it's different from default (i.e., has optimized examples)
                if !prompt.is_empty() {
                    return Some(prompt);
                }
            }
        }

        None
    }

    /// Load project documentation if configured
    ///
    /// This loads AGENTS.md files from the repository root to the working directory
    /// and returns the combined content. Returns None if project docs are disabled
    /// or no documentation files are found.
    pub async fn load_project_docs(&self) -> Option<String> {
        let options = self.project_doc_options.as_ref()?;
        match crate::project_doc::get_user_instructions(options).await {
            Some(docs) => {
                tracing::debug!(len = docs.len(), "Loaded project documentation");
                Some(docs)
            }
            None => {
                tracing::debug!("No project documentation found");
                None
            }
        }
    }

    /// Resolve the full system prompt including project documentation
    ///
    /// This combines:
    /// 1. Base system prompt (from resolve_system_prompt or default)
    /// 2. Project documentation (AGENTS.md files)
    ///
    /// Call this async method instead of resolve_system_prompt when you want
    /// to include project-level documentation.
    pub async fn resolve_full_system_prompt(&self) -> Option<String> {
        let base_prompt = self.resolve_system_prompt();
        let project_docs = self.load_project_docs().await;

        match (base_prompt, project_docs) {
            (Some(base), Some(docs)) => Some(format!(
                "{}\n\n--- project documentation ---\n\n{}",
                base, docs
            )),
            (Some(base), None) => Some(base),
            (None, Some(docs)) => {
                // If we only have project docs, prepend default system prompt
                Some(format!(
                    "{}\n\n--- project documentation ---\n\n{}",
                    crate::optimize::DEFAULT_SYSTEM_PROMPT,
                    docs
                ))
            }
            (None, None) => None,
        }
    }
}

/// Result of running the agent
#[derive(Clone, Debug)]
pub struct AgentResult {
    /// Final agent state
    pub state: AgentState,
    /// Thread ID used for checkpointing
    pub thread_id: String,
    /// Number of turns executed
    pub turns: u32,
    /// Training example collected from this run (if collect_training enabled)
    pub training_example: Option<crate::optimize::TrainingExample>,
    /// DashFlow execution metrics (node durations, checkpoint counts, etc.)
    /// Available when metrics collection is enabled (default behavior)
    pub execution_metrics: Option<dashflow::ExecutionMetrics>,
}

/// Calculate a quality score for the agent run based on execution metrics
///
/// Score factors:
/// - Base: 0.5 for completing without errors
/// - +0.2 for successful tool executions (proportional)
/// - +0.2 for efficient turn count (fewer is better, when measurable)
/// - +0.1 for successful completion status
///
/// Audit #30: When max_turns is 0 (unlimited), we skip the turn efficiency
/// penalty since there's no baseline to compare against. This avoids
/// penalizing long-running tasks that legitimately need many turns.
///
/// Returns a score between 0.0 and 1.0
fn calculate_run_score(state: &AgentState) -> f64 {
    let mut score = 0.0;

    // Base score for completing
    if !matches!(state.status, CompletionStatus::Error(_)) {
        score += 0.5;
    }

    // Tool execution success rate
    if !state.tool_results.is_empty() {
        let success_count = state.tool_results.iter().filter(|r| r.success).count();
        let success_rate = success_count as f64 / state.tool_results.len() as f64;
        score += 0.2 * success_rate;
    } else {
        // No tools needed, full credit
        score += 0.2;
    }

    // Turn efficiency (prefer fewer turns when we have a defined limit)
    // Audit #30: Only calculate turn efficiency when max_turns is set.
    // For unlimited runs (max_turns=0), give full credit since we can't
    // meaningfully measure efficiency without a target.
    if state.max_turns > 0 {
        let turn_efficiency = 1.0 - (state.turn_count as f64 / state.max_turns as f64).min(1.0);
        score += 0.2 * turn_efficiency;
    } else {
        // Unlimited mode: give full credit for turn efficiency
        score += 0.2;
    }

    // Completion status bonus
    if matches!(state.status, CompletionStatus::Complete) {
        score += 0.1;
    }

    score.clamp(0.0, 1.0)
}

/// Collect training data from a successful agent run
fn collect_training_example(
    initial_user_message: &str,
    state: &AgentState,
) -> Option<crate::optimize::TrainingExample> {
    // Only collect from successful completions
    if !matches!(
        state.status,
        CompletionStatus::Complete | CompletionStatus::TurnLimitReached
    ) {
        return None;
    }

    // Get the final response
    let agent_output = state.last_response.as_ref()?;
    if agent_output.is_empty() {
        return None;
    }

    // Calculate score
    let score = calculate_run_score(state);

    // Collect tool names used
    let tool_calls: Vec<String> = state.tool_results.iter().map(|r| r.tool.clone()).collect();

    Some(
        crate::optimize::TrainingExample::new(initial_user_message, agent_output.clone(), score)
            .with_tool_calls(tool_calls),
    )
}

/// Helper function to process graph execution result
///
/// Handles event emission, training data collection, and result construction.
#[allow(clippy::too_many_arguments)]
async fn run_agent_with_result(
    result: std::result::Result<dashflow::ExecutionResult<AgentState>, dashflow::Error>,
    thread_id: &str,
    stream_callback: Arc<dyn StreamCallback>,
    initial_user_message: Option<String>,
    config: &RunnerConfig,
    execution_metrics: Option<dashflow::ExecutionMetrics>,
) -> Result<AgentResult> {
    let result = result
        .map_err(|e| crate::Error::GraphExecution(format!("Graph execution failed: {}", e)))?;

    // Emit session complete event
    let status = match &result.final_state.status {
        crate::state::CompletionStatus::Complete => "complete",
        crate::state::CompletionStatus::TurnLimitReached => "turn_limit",
        crate::state::CompletionStatus::Interrupted => "interrupted",
        crate::state::CompletionStatus::Error(_) => "error",
        crate::state::CompletionStatus::InProgress => "in_progress",
    };
    stream_callback
        .on_event(AgentEvent::SessionComplete {
            session_id: thread_id.to_string(),
            total_turns: result.final_state.turn_count,
            status: status.to_string(),
        })
        .await;

    // Audit #77: Emit session metrics with aggregated cost/token data
    let cost = if result.final_state.total_cost_usd > 0.0 {
        Some(result.final_state.total_cost_usd)
    } else {
        None
    };
    stream_callback
        .on_event(AgentEvent::SessionMetrics {
            session_id: thread_id.to_string(),
            total_input_tokens: result.final_state.total_input_tokens,
            total_output_tokens: result.final_state.total_output_tokens,
            total_cached_tokens: result.final_state.total_cached_tokens,
            total_cost_usd: cost,
            llm_call_count: result.final_state.turn_count, // Each turn has at least one LLM call
            duration_ms: 0, // Duration tracked externally; nodes_executed could be used as proxy
        })
        .await;

    // Collect training data if enabled
    let training_example = if config.collect_training {
        initial_user_message
            .as_ref()
            .and_then(|msg| collect_training_example(msg, &result.final_state))
    } else {
        None
    };

    // If training data was collected, emit EvalCapture event for streaming telemetry (audit #78)
    // This enables dashstream consumers to build evaluation datasets from production runs
    if let Some(ref example) = training_example {
        // Serialize messages for the event (JSON string format)
        let input_messages_json: Vec<serde_json::Value> = result
            .final_state
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": format!("{:?}", m.role).to_lowercase(),
                    "content": m.content
                })
            })
            .collect();
        let input_messages = serde_json::to_string(&input_messages_json).unwrap_or_default();

        // Serialize tool definitions used (collect unique tool names)
        let tools_used: Vec<serde_json::Value> = example
            .tool_calls
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .map(|name| serde_json::json!({"name": name}))
            .collect();
        let tools = if tools_used.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&tools_used).unwrap_or_default())
        };

        stream_callback
            .on_event(AgentEvent::EvalCapture {
                session_id: thread_id.to_string(),
                capture_id: uuid::Uuid::new_v4().to_string(),
                input_messages,
                output_response: example.agent_output.clone(),
                model: result.final_state.llm_config.model.clone(),
                tools,
                metadata: Some(serde_json::json!({
                    "score": example.score,
                    "turn_count": result.final_state.turn_count,
                    "status": status,
                    "user_input": example.user_input,
                })),
            })
            .await;
    }

    // If training data was collected and we have data to save, save it
    if let Some(ref example) = training_example {
        // Try to save to default training data file
        if let Ok(mut training_data) = TrainingData::load_default() {
            training_data.examples.push(example.clone());
            // Ignore save errors - training data collection should not fail the run
            let _ = training_data.save_default();
        } else {
            // Create new training data file
            let mut training_data = TrainingData::new();
            training_data.examples.push(example.clone());
            let _ = training_data.save_default();
        }
    }

    Ok(AgentResult {
        state: result.final_state.clone(),
        thread_id: thread_id.to_string(),
        turns: result.final_state.turn_count,
        training_example,
        execution_metrics,
    })
}

/// Run the agent with the given state and configuration
///
/// # Arguments
/// * `state` - Initial agent state with user message
/// * `config` - Runner configuration
///
/// # Returns
/// The final agent result after completion or turn limit
pub async fn run_agent(state: AgentState, config: &RunnerConfig) -> Result<AgentResult> {
    let mut state = state;
    if config.max_turns > 0 {
        state.max_turns = config.max_turns;
    }

    // Apply system prompt from config if specified
    // Only apply if state doesn't already have a custom prompt
    // Uses resolve_full_system_prompt to include project documentation (AGENTS.md)
    if state.system_prompt.is_none() {
        if let Some(prompt) = config.resolve_full_system_prompt().await {
            state.system_prompt = Some(prompt);
        }
    }

    // Capture initial user message for training data collection
    // Audit #32: Use .find() (first matching) instead of .next_back() (last) to capture
    // the initial task description for multi-turn runs. The first user message
    // typically contains the full task context, while subsequent messages may
    // be follow-ups or refinements.
    let initial_user_message = state
        .messages
        .iter()
        .find(|m| matches!(m.role, crate::state::MessageRole::User))
        .map(|m| m.content.clone());

    // Attach stream callback to state
    let stream_callback = config.stream_callback();
    let state = state.with_stream_callback(stream_callback.clone());

    // Emit user turn event if we have a user message
    if let Some(ref user_msg) = initial_user_message {
        stream_callback
            .on_event(AgentEvent::UserTurn {
                session_id: state.session_id.clone(),
                content: user_msg.clone(),
            })
            .await;
    }

    let thread_id = state.session_id.clone();
    let graph = build_agent_graph()?;

    // Inject graph manifest for AI self-awareness (introspection)
    // This enables the AI to answer "What are your capabilities?" and similar questions
    let manifest = Arc::new(build_agent_graph_manifest());
    let state = state.with_graph_manifest(manifest);

    if config.enable_checkpointing {
        // PostgreSQL checkpointing (requires `postgres` feature)
        // Audit #85: Warn if postgres requested but feature not compiled
        #[cfg(not(feature = "postgres"))]
        if config.postgres_connection_string.is_some() {
            tracing::warn!(
                "PostgreSQL checkpointing requested but `postgres` feature not compiled. \
                 Falling back to memory/file checkpointing. \
                 Recompile with `--features postgres` to enable PostgreSQL support."
            );
        }

        #[cfg(feature = "postgres")]
        if let Some(ref conn_str) = config.postgres_connection_string {
            let checkpointer = PostgresCheckpointer::new(conn_str).await.map_err(|e| {
                crate::Error::GraphExecution(format!(
                    "Failed to create PostgreSQL checkpointer: {}",
                    e
                ))
            })?;
            let app = graph
                .with_checkpointer(checkpointer)
                .with_thread_id(&thread_id);
            let result = app.invoke(state).await;
            let metrics = Some(app.metrics());
            return run_agent_with_result(
                result,
                &thread_id,
                stream_callback,
                initial_user_message,
                config,
                metrics,
            )
            .await;
        }

        // File-based checkpointing
        if let Some(ref path) = config.checkpoint_path {
            let checkpointer = FileCheckpointer::new(path).map_err(|e| {
                crate::Error::GraphExecution(format!("Failed to create file checkpointer: {}", e))
            })?;
            let app = graph
                .with_checkpointer(checkpointer)
                .with_thread_id(&thread_id);
            let result = app.invoke(state).await;
            let metrics = Some(app.metrics());
            return run_agent_with_result(
                result,
                &thread_id,
                stream_callback,
                initial_user_message,
                config,
                metrics,
            )
            .await;
        } else {
            // Memory-based checkpointing
            let checkpointer = MemoryCheckpointer::new();
            let app = graph
                .with_checkpointer(checkpointer)
                .with_thread_id(&thread_id);
            let result = app.invoke(state).await;
            let metrics = Some(app.metrics());
            return run_agent_with_result(
                result,
                &thread_id,
                stream_callback,
                initial_user_message,
                config,
                metrics,
            )
            .await;
        }
    }

    // No checkpointing
    let result = graph.invoke(state).await;
    let metrics = Some(graph.metrics());
    run_agent_with_result(
        result,
        &thread_id,
        stream_callback,
        initial_user_message,
        config,
        metrics,
    )
    .await
}

/// Run a single turn of the agent (for interactive use)
///
/// This processes user input and returns after the reasoning cycle completes.
pub async fn run_turn(
    mut state: AgentState,
    user_message: &str,
    config: &RunnerConfig,
) -> Result<AgentResult> {
    state.add_user_message(user_message);
    run_agent(state, config).await
}

/// Resume a session from a checkpoint (Audit #84)
///
/// Loads the most recent checkpoint for the given session ID and resumes execution.
/// This requires checkpointing to be enabled in the config.
///
/// # Arguments
/// * `session_id` - The session/thread ID to resume
/// * `config` - Runner configuration (must have checkpointing enabled)
///
/// # Returns
/// The resumed agent state if a checkpoint exists, or an error if:
/// - Checkpointing is not enabled
/// - No checkpoint exists for the session ID
/// - The checkpoint cannot be loaded
///
/// # Example
/// ```rust,ignore
/// let config = RunnerConfig::with_file_checkpointing("/tmp/checkpoints");
/// let result = resume_session("my-session-123", &config).await?;
/// ```
pub async fn resume_session(session_id: &str, config: &RunnerConfig) -> Result<AgentState> {
    if !config.enable_checkpointing {
        return Err(crate::Error::GraphExecution(
            "Cannot resume session: checkpointing is not enabled. \
             Use RunnerConfig::with_file_checkpointing() or similar."
                .to_string(),
        ));
    }

    // Try PostgreSQL first if configured
    #[cfg(feature = "postgres")]
    if let Some(ref conn_str) = config.postgres_connection_string {
        let checkpointer = dashflow_postgres_checkpointer::PostgresCheckpointer::new(conn_str)
            .await
            .map_err(|e| {
                crate::Error::GraphExecution(format!(
                    "Failed to connect to PostgreSQL for resume: {}",
                    e
                ))
            })?;

        return load_checkpoint_state(&checkpointer, session_id).await;
    }

    // Try file-based checkpointing
    if let Some(ref path) = config.checkpoint_path {
        let checkpointer = FileCheckpointer::new(path).map_err(|e| {
            crate::Error::GraphExecution(format!("Failed to open checkpoint file: {}", e))
        })?;

        return load_checkpoint_state(&checkpointer, session_id).await;
    }

    // Memory checkpointing cannot persist across process restarts
    Err(crate::Error::GraphExecution(
        "Cannot resume session from memory checkpointer. \
         Memory checkpoints are not persisted across process restarts. \
         Use file or PostgreSQL checkpointing for session resume."
            .to_string(),
    ))
}

/// Load agent state from a checkpoint
async fn load_checkpoint_state<C>(checkpointer: &C, session_id: &str) -> Result<AgentState>
where
    C: dashflow::checkpoint::Checkpointer<AgentState>,
{
    let checkpoint = checkpointer
        .get_latest(session_id)
        .await
        .map_err(|e| crate::Error::GraphExecution(format!("Failed to load checkpoint: {}", e)))?
        .ok_or_else(|| {
            crate::Error::GraphExecution(format!(
                "No checkpoint found for session '{}'. \
                 The session may not exist or may have been cleaned up.",
                session_id
            ))
        })?;

    tracing::info!(
        session_id = %session_id,
        checkpoint_id = %checkpoint.id,
        node = %checkpoint.node,
        "Resumed session from checkpoint"
    );

    Ok(checkpoint.state)
}

/// Check if a session can be resumed (checkpoint exists)
///
/// # Arguments
/// * `session_id` - The session/thread ID to check
/// * `config` - Runner configuration (must have checkpointing enabled)
///
/// # Returns
/// `true` if a checkpoint exists for the session, `false` otherwise
pub async fn can_resume_session(session_id: &str, config: &RunnerConfig) -> bool {
    if !config.enable_checkpointing {
        return false;
    }

    // Try PostgreSQL first if configured
    #[cfg(feature = "postgres")]
    if let Some(ref conn_str) = config.postgres_connection_string {
        if let Ok(checkpointer) =
            dashflow_postgres_checkpointer::PostgresCheckpointer::<AgentState>::new(conn_str).await
        {
            if let Ok(Some(_)) = checkpointer.get_latest(session_id).await {
                return true;
            }
        }
        return false;
    }

    // Try file-based checkpointing
    if let Some(ref path) = config.checkpoint_path {
        if let Ok(checkpointer) = FileCheckpointer::<AgentState>::new(path) {
            if let Ok(Some(_)) = checkpointer.get_latest(session_id).await {
                return true;
            }
        }
        return false;
    }

    false
}

/// Apply checkpoint retention policy for a thread (Audit #87)
///
/// Deletes old checkpoints according to the retention policy. This is typically
/// called automatically after checkpoint saves, but can also be called manually.
///
/// # Arguments
/// * `thread_id` - The thread/session to clean up
/// * `config` - Runner configuration with retention policy
///
/// # Returns
/// Number of checkpoints deleted, or error if cleanup fails
pub async fn cleanup_checkpoints(thread_id: &str, config: &RunnerConfig) -> Result<usize> {
    let policy = match &config.checkpoint_retention {
        Some(p) => p,
        None => return Ok(0), // No policy = no cleanup
    };

    // Nothing to cleanup if no limits set
    if policy.max_checkpoints_per_thread == 0 && policy.max_age_seconds == 0 {
        return Ok(0);
    }

    // File-based checkpointing
    if let Some(ref path) = config.checkpoint_path {
        let checkpointer = FileCheckpointer::<AgentState>::new(path).map_err(|e| {
            crate::Error::GraphExecution(format!("Failed to open checkpoint file: {}", e))
        })?;

        return cleanup_checkpoints_for_checkpointer(&checkpointer, thread_id, policy).await;
    }

    // PostgreSQL checkpointing
    #[cfg(feature = "postgres")]
    if let Some(ref conn_str) = config.postgres_connection_string {
        let checkpointer =
            dashflow_postgres_checkpointer::PostgresCheckpointer::<AgentState>::new(conn_str)
                .await
                .map_err(|e| {
                    crate::Error::GraphExecution(format!(
                        "Failed to connect to PostgreSQL for cleanup: {}",
                        e
                    ))
                })?;

        return cleanup_checkpoints_for_checkpointer(&checkpointer, thread_id, policy).await;
    }

    // Memory checkpointer - no persistent cleanup needed
    Ok(0)
}

/// Internal helper to clean up checkpoints for any checkpointer type
async fn cleanup_checkpoints_for_checkpointer<C>(
    checkpointer: &C,
    thread_id: &str,
    policy: &CheckpointRetentionPolicy,
) -> Result<usize>
where
    C: Checkpointer<AgentState>,
{
    let mut deleted = 0;

    // Get list of checkpoints for this thread
    let checkpoints = checkpointer
        .list(thread_id)
        .await
        .map_err(|e| crate::Error::GraphExecution(format!("Failed to list checkpoints: {}", e)))?;

    if checkpoints.is_empty() {
        return Ok(0);
    }

    // Sort by timestamp (newest first)
    let mut sorted_checkpoints = checkpoints;
    sorted_checkpoints.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Apply max count limit
    if policy.max_checkpoints_per_thread > 0
        && sorted_checkpoints.len() > policy.max_checkpoints_per_thread
    {
        let to_delete = &sorted_checkpoints[policy.max_checkpoints_per_thread..];
        for cp in to_delete {
            if let Err(e) = checkpointer.delete(&cp.id).await {
                tracing::warn!(
                    checkpoint_id = %cp.id,
                    error = %e,
                    "Failed to delete checkpoint during retention cleanup"
                );
            } else {
                tracing::debug!(
                    checkpoint_id = %cp.id,
                    thread_id = %thread_id,
                    "Deleted checkpoint due to retention policy (count limit)"
                );
                deleted += 1;
            }
        }
        // Remove deleted entries from our list
        sorted_checkpoints.truncate(policy.max_checkpoints_per_thread);
    }

    // Apply max age limit
    if policy.max_age_seconds > 0 {
        let now = std::time::SystemTime::now();
        let cutoff = now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(policy.max_age_seconds);

        for cp in &sorted_checkpoints {
            let cp_timestamp = cp
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if cp_timestamp < cutoff {
                if let Err(e) = checkpointer.delete(&cp.id).await {
                    tracing::warn!(
                        checkpoint_id = %cp.id,
                        error = %e,
                        "Failed to delete checkpoint during retention cleanup"
                    );
                } else {
                    tracing::debug!(
                        checkpoint_id = %cp.id,
                        thread_id = %thread_id,
                        age_seconds = %(now.duration_since(cp.timestamp).unwrap_or_default().as_secs()),
                        "Deleted checkpoint due to retention policy (age limit)"
                    );
                    deleted += 1;
                }
            }
        }
    }

    if deleted > 0 {
        tracing::info!(
            thread_id = %thread_id,
            deleted = deleted,
            "Checkpoint retention policy applied"
        );
    }

    Ok(deleted)
}

// ============================================================================
// Session Listing
// ============================================================================

// Re-export ThreadInfo for CLI consumers
pub use dashflow::checkpoint::ThreadInfo;

/// List all sessions (threads) that have checkpoints stored.
///
/// This function queries the configured checkpoint storage to enumerate
/// all sessions that can potentially be resumed.
///
/// # Arguments
/// * `config` - Runner configuration containing checkpoint settings
///
/// # Returns
/// * `Ok(Vec<ThreadInfo>)` - List of sessions with metadata
/// * `Err` - If checkpointing is not enabled or storage cannot be queried
///
/// # Example
/// ```no_run
/// use codex_dashflow_core::{list_sessions, RunnerConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = RunnerConfig::with_file_checkpointing("/tmp/sessions");
/// let sessions = list_sessions(&config).await?;
/// for session in sessions {
///     println!("Session: {} (updated: {:?})", session.thread_id, session.updated_at);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn list_sessions(config: &RunnerConfig) -> Result<Vec<ThreadInfo>> {
    if !config.enable_checkpointing {
        return Err(crate::Error::GraphExecution(
            "Cannot list sessions: checkpointing is not enabled. \
             Use RunnerConfig::with_file_checkpointing() or similar."
                .to_string(),
        ));
    }

    // Try PostgreSQL first if configured
    #[cfg(feature = "postgres")]
    if let Some(ref conn_str) = config.postgres_connection_string {
        let checkpointer =
            dashflow_postgres_checkpointer::PostgresCheckpointer::<AgentState>::new(conn_str)
                .await
                .map_err(|e| {
                    crate::Error::GraphExecution(format!(
                        "Failed to connect to PostgreSQL for session listing: {}",
                        e
                    ))
                })?;

        return checkpointer.list_threads().await.map_err(|e| {
            crate::Error::GraphExecution(format!("Failed to list sessions from PostgreSQL: {}", e))
        });
    }

    // Try file-based checkpointing
    if let Some(ref path) = config.checkpoint_path {
        let checkpointer = FileCheckpointer::<AgentState>::new(path).map_err(|e| {
            crate::Error::GraphExecution(format!("Failed to open checkpoint storage: {}", e))
        })?;

        return checkpointer.list_threads().await.map_err(|e| {
            crate::Error::GraphExecution(format!(
                "Failed to list sessions from file storage: {}",
                e
            ))
        });
    }

    // Memory checkpointing cannot persist across process restarts
    Err(crate::Error::GraphExecution(
        "Cannot list sessions from memory checkpointer. \
         Memory checkpoints are not persisted across process restarts. \
         Use file or PostgreSQL checkpointing for session listing."
            .to_string(),
    ))
}

/// Get the most recently updated session ID.
///
/// This function returns the session ID of the session with the most recent checkpoint.
/// Useful for implementing "resume latest session" functionality.
///
/// # Arguments
///
/// * `config` - Runner configuration with checkpointing settings
///
/// # Returns
///
/// Returns `Some(session_id)` if there is at least one session, or `None` if no sessions exist.
///
/// # Errors
///
/// Returns an error if:
/// - Checkpointing is not enabled in the configuration
/// - The checkpointer fails to list sessions
/// - Only memory checkpointing is configured
///
/// # Example
///
/// ```ignore
/// use codex_dashflow_core::{get_latest_session, RunnerConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = RunnerConfig::with_file_checkpointing("/tmp/sessions");
/// if let Some(session_id) = get_latest_session(&config).await? {
///     println!("Most recent session: {}", session_id);
/// } else {
///     println!("No sessions found");
/// }
/// # Ok(())
/// # }
/// ```
pub async fn get_latest_session(config: &RunnerConfig) -> Result<Option<String>> {
    let sessions = list_sessions(config).await?;
    // Sessions are sorted by updated_at descending (most recent first)
    // so the first one is the latest
    Ok(sessions.first().map(|s| s.thread_id.to_string()))
}

/// Get the most recent session that is not older than `max_age_secs`.
///
/// This function returns the most recently updated session ID, but only if
/// that session was updated within `max_age_secs` seconds of the current time.
/// This is useful for auto-resume scenarios where you want to skip stale sessions.
///
/// # Arguments
///
/// * `config` - Runner configuration containing checkpoint settings
/// * `max_age_secs` - Maximum age in seconds. Sessions older than this are skipped.
///   If `None`, behaves the same as `get_latest_session`.
///
/// # Returns
///
/// * `Ok(Some(session_id))` - If a session exists within the max age limit
/// * `Ok(None)` - If no sessions exist or all sessions are older than max_age_secs
/// * `Err` - If checkpointing is not enabled or storage cannot be queried
///
/// # Example
///
/// ```ignore
/// use codex_dashflow_core::{get_latest_session_with_max_age, RunnerConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = RunnerConfig::with_file_checkpointing("/tmp/sessions");
/// // Only resume sessions updated in the last 24 hours
/// if let Some(session_id) = get_latest_session_with_max_age(&config, Some(86400)).await? {
///     println!("Recent session found: {}", session_id);
/// } else {
///     println!("No recent sessions (within 24 hours)");
/// }
/// # Ok(())
/// # }
/// ```
pub async fn get_latest_session_with_max_age(
    config: &RunnerConfig,
    max_age_secs: Option<u64>,
) -> Result<Option<String>> {
    // If no max age specified, delegate to regular get_latest_session
    let Some(max_age) = max_age_secs else {
        return get_latest_session(config).await;
    };

    let sessions = list_sessions(config).await?;

    // Sessions are sorted by updated_at descending (most recent first)
    let Some(latest) = sessions.first() else {
        return Ok(None);
    };

    // Check if the latest session is within the max age limit
    let now = std::time::SystemTime::now();
    let age = now
        .duration_since(latest.updated_at)
        .unwrap_or(std::time::Duration::MAX);

    if age.as_secs() <= max_age {
        Ok(Some(latest.thread_id.to_string()))
    } else {
        tracing::info!(
            session_id = %latest.thread_id,
            age_secs = age.as_secs(),
            max_age_secs = max_age,
            "Latest session is too old for auto-resume"
        );
        Ok(None)
    }
}

/// Delete a session (thread) and all its checkpoints.
///
/// This function removes all checkpoints associated with the given session ID.
/// This is a destructive operation and cannot be undone.
///
/// # Arguments
///
/// * `config` - Runner configuration with checkpointing settings
/// * `session_id` - The session/thread ID to delete
///
/// # Errors
///
/// Returns an error if:
/// - Checkpointing is not enabled in the configuration
/// - The checkpointer fails to delete the session
/// - Only memory checkpointing is configured (cannot delete from memory)
///
/// # Example
///
/// ```ignore
/// use codex_dashflow_core::{delete_session, RunnerConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = RunnerConfig::with_file_checkpointing("/tmp/sessions");
/// delete_session(&config, "session-123").await?;
/// println!("Session deleted successfully");
/// # Ok(())
/// # }
/// ```
pub async fn delete_session(config: &RunnerConfig, session_id: &str) -> Result<()> {
    if !config.enable_checkpointing {
        return Err(crate::Error::GraphExecution(
            "Cannot delete session: checkpointing is not enabled. \
             Use RunnerConfig::with_file_checkpointing() or similar."
                .to_string(),
        ));
    }

    // Try PostgreSQL first if configured
    #[cfg(feature = "postgres")]
    if let Some(ref conn_str) = config.postgres_connection_string {
        let checkpointer =
            dashflow_postgres_checkpointer::PostgresCheckpointer::<AgentState>::new(conn_str)
                .await
                .map_err(|e| {
                    crate::Error::GraphExecution(format!(
                        "Failed to connect to PostgreSQL for session deletion: {}",
                        e
                    ))
                })?;

        return checkpointer.delete_thread(session_id).await.map_err(|e| {
            crate::Error::GraphExecution(format!("Failed to delete session from PostgreSQL: {}", e))
        });
    }

    // Try file-based checkpointing
    if let Some(ref path) = config.checkpoint_path {
        let checkpointer = FileCheckpointer::<AgentState>::new(path).map_err(|e| {
            crate::Error::GraphExecution(format!("Failed to open checkpoint storage: {}", e))
        })?;

        return checkpointer.delete_thread(session_id).await.map_err(|e| {
            crate::Error::GraphExecution(format!(
                "Failed to delete session from file storage: {}",
                e
            ))
        });
    }

    // Memory checkpointing cannot persist across process restarts
    Err(crate::Error::GraphExecution(
        "Cannot delete sessions from memory checkpointer. \
         Memory checkpoints are not persisted across process restarts. \
         Use file or PostgreSQL checkpointing for session management."
            .to_string(),
    ))
}

/// Delete all sessions (all checkpoints) from storage
///
/// This is a destructive operation that removes ALL saved sessions.
/// Requires file-based or PostgreSQL checkpointing to be configured.
///
/// # Arguments
/// * `config` - Runner configuration with checkpointing settings
///
/// # Returns
/// * `Ok(count)` - Number of sessions deleted
/// * `Err` - If checkpointing is not configured or deletion fails
///
/// # Example
/// ```no_run
/// # use codex_dashflow_core::{RunnerConfig, delete_all_sessions};
/// # async fn example() -> anyhow::Result<()> {
/// let config = RunnerConfig::with_file_checkpointing("/tmp/sessions");
/// let count = delete_all_sessions(&config).await?;
/// println!("Deleted {} sessions", count);
/// # Ok(())
/// # }
/// ```
pub async fn delete_all_sessions(config: &RunnerConfig) -> Result<usize> {
    if !config.enable_checkpointing {
        return Err(crate::Error::GraphExecution(
            "Cannot delete sessions: checkpointing is not enabled. \
             Use RunnerConfig::with_file_checkpointing() or similar."
                .to_string(),
        ));
    }

    // First, get the list of all sessions
    let sessions = list_sessions(config).await?;
    let count = sessions.len();

    // Delete each session
    for session in sessions {
        delete_session(config, &session.thread_id).await?;
    }

    Ok(count)
}

/// Detailed information about a session including all checkpoints
#[derive(Clone, Debug, serde::Serialize)]
pub struct SessionDetails {
    /// The session/thread identifier
    pub session_id: String,
    /// Total number of checkpoints
    pub checkpoint_count: usize,
    /// Timestamp of the most recent checkpoint
    pub latest_update: Option<std::time::SystemTime>,
    /// Timestamp of the first checkpoint (session creation time)
    pub created_at: Option<std::time::SystemTime>,
    /// All checkpoint metadata (ordered by timestamp, newest first)
    pub checkpoints: Vec<CheckpointMetadata>,
}

/// Re-export CheckpointMetadata from DashFlow for session details
pub use dashflow::CheckpointMetadata;

/// Get detailed information about a specific session
///
/// Returns the session details including all checkpoint metadata.
/// This is useful for displaying session history and understanding
/// the checkpoint structure.
///
/// # Arguments
/// * `config` - Runner configuration with checkpointing settings
/// * `session_id` - The session/thread ID to get details for
///
/// # Returns
/// * `Ok(SessionDetails)` - Detailed session information
/// * `Err` - If checkpointing is not configured or session not found
pub async fn get_session_info(config: &RunnerConfig, session_id: &str) -> Result<SessionDetails> {
    if !config.enable_checkpointing {
        return Err(crate::Error::GraphExecution(
            "Cannot get session info: checkpointing is not enabled. \
             Use RunnerConfig::with_file_checkpointing() or similar."
                .to_string(),
        ));
    }

    // Try PostgreSQL first if configured
    #[cfg(feature = "postgres")]
    if let Some(ref conn_str) = config.postgres_connection_string {
        let checkpointer =
            dashflow_postgres_checkpointer::PostgresCheckpointer::<AgentState>::new(conn_str)
                .await
                .map_err(|e| {
                    crate::Error::GraphExecution(format!(
                        "Failed to connect to PostgreSQL for session info: {}",
                        e
                    ))
                })?;

        let checkpoints = checkpointer.list(session_id).await.map_err(|e| {
            crate::Error::GraphExecution(format!(
                "Failed to get session info from PostgreSQL: {}",
                e
            ))
        })?;

        return Ok(build_session_details(session_id, checkpoints));
    }

    // Try file-based checkpointing
    if let Some(ref path) = config.checkpoint_path {
        let checkpointer = FileCheckpointer::<AgentState>::new(path).map_err(|e| {
            crate::Error::GraphExecution(format!("Failed to open checkpoint storage: {}", e))
        })?;

        let checkpoints = checkpointer.list(session_id).await.map_err(|e| {
            crate::Error::GraphExecution(format!(
                "Failed to get session info from file storage: {}",
                e
            ))
        })?;

        return Ok(build_session_details(session_id, checkpoints));
    }

    // Memory checkpointing cannot persist across process restarts
    Err(crate::Error::GraphExecution(
        "Cannot get session info from memory checkpointer. \
         Memory checkpoints are not persisted across process restarts. \
         Use file or PostgreSQL checkpointing for session management."
            .to_string(),
    ))
}

/// Build SessionDetails from checkpoint metadata list
fn build_session_details(session_id: &str, checkpoints: Vec<CheckpointMetadata>) -> SessionDetails {
    let checkpoint_count = checkpoints.len();
    let latest_update = checkpoints.first().map(|cp| cp.timestamp);
    let created_at = checkpoints.last().map(|cp| cp.timestamp);

    SessionDetails {
        session_id: session_id.to_string(),
        checkpoint_count,
        latest_update,
        created_at,
        checkpoints,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Message;
    use crate::streaming::MetricsCallback;

    #[tokio::test]
    async fn test_run_agent_simple() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default();
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.state.last_response.is_some());
    }

    #[tokio::test]
    async fn test_run_agent_with_memory_checkpointing() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::with_memory_checkpointing();
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_turn() {
        let state = AgentState::new().with_mock_llm();
        let config = RunnerConfig::default();

        let result = run_turn(state, "Hello there", &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.state.last_response.is_some());
    }

    #[tokio::test]
    async fn test_run_agent_with_tool_call() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("List the files"));

        let config = RunnerConfig::default();
        let result = run_agent(state, &config).await;

        match &result {
            Ok(_) => {}
            Err(e) => eprintln!("Error: {}", e),
        }
        assert!(result.is_ok(), "Expected Ok, got error");
        let result = result.unwrap();
        // After tool execution and result analysis, messages should include tool results
        assert!(result.state.messages.len() > 2);
    }

    #[tokio::test]
    async fn test_run_agent_with_streaming() {
        let metrics = Arc::new(MetricsCallback::new());
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default().with_stream_callback(metrics.clone());
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());

        // Give async events time to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Check that events were emitted
        let events = metrics.events();
        assert!(
            !events.is_empty(),
            "Expected streaming events to be emitted"
        );

        // Should have at least UserTurn and SessionComplete
        let event_types: Vec<_> = events.iter().map(|e| e.event_type()).collect();
        assert!(
            event_types.contains(&"user_turn"),
            "Expected user_turn event"
        );
        assert!(
            event_types.contains(&"session_complete"),
            "Expected session_complete event"
        );
    }

    #[tokio::test]
    async fn test_run_agent_with_streaming_and_tools() {
        let metrics = Arc::new(MetricsCallback::new());
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("List the files"));

        let config = RunnerConfig::default().with_stream_callback(metrics.clone());
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());

        // Give async events time to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Check that events were emitted
        let events = metrics.events();
        assert!(
            !events.is_empty(),
            "Expected streaming events to be emitted"
        );

        // Should have tool-related events
        let event_types: Vec<_> = events.iter().map(|e| e.event_type()).collect();
        assert!(
            event_types.contains(&"reasoning_start"),
            "Expected reasoning_start event"
        );
        assert!(
            event_types.contains(&"reasoning_complete"),
            "Expected reasoning_complete event"
        );
        assert!(
            event_types.contains(&"tool_call_requested"),
            "Expected tool_call_requested event"
        );
    }

    #[test]
    fn test_calculate_run_score_complete() {
        use crate::state::ToolResult;

        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.turn_count = 1;
        state.max_turns = 10;
        state.tool_results = vec![ToolResult {
            tool_call_id: "1".to_string(),
            tool: "shell".to_string(),
            output: "success".to_string(),
            success: true,
            duration_ms: 100,
        }];

        let score = calculate_run_score(&state);
        // Base (0.5) + tool success (0.2) + turn efficiency ~(0.18) + completion (0.1) = ~0.98
        assert!(score > 0.9, "Expected high score, got {}", score);
    }

    #[test]
    fn test_calculate_run_score_with_failed_tools() {
        use crate::state::ToolResult;

        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.turn_count = 5;
        state.max_turns = 10;
        state.tool_results = vec![
            ToolResult {
                tool_call_id: "1".to_string(),
                tool: "shell".to_string(),
                output: "success".to_string(),
                success: true,
                duration_ms: 100,
            },
            ToolResult {
                tool_call_id: "2".to_string(),
                tool: "shell".to_string(),
                output: "failed".to_string(),
                success: false,
                duration_ms: 50,
            },
        ];

        let score = calculate_run_score(&state);
        // Base (0.5) + tool success (0.1 - 50%) + turn efficiency (0.1 - 50%) + completion (0.1) = 0.8
        assert!(
            (0.6..=0.9).contains(&score),
            "Expected medium score, got {}",
            score
        );
    }

    #[test]
    fn test_calculate_run_score_error_status() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Error("test error".to_string());

        let score = calculate_run_score(&state);
        // No base score for error, only tool and turn efficiency
        assert!(score < 0.5, "Expected low score for error, got {}", score);
    }

    #[test]
    fn test_collect_training_example_success() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.last_response = Some("Here is the result".to_string());

        let example = collect_training_example("List files", &state);
        assert!(example.is_some());

        let example = example.unwrap();
        assert_eq!(example.user_input, "List files");
        assert_eq!(example.agent_output, "Here is the result");
        assert!(example.score > 0.0);
    }

    #[test]
    fn test_collect_training_example_error_status() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Error("test error".to_string());
        state.last_response = Some("Partial result".to_string());

        let example = collect_training_example("List files", &state);
        assert!(example.is_none(), "Should not collect from error runs");
    }

    #[test]
    fn test_collect_training_example_no_response() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.last_response = None;

        let example = collect_training_example("List files", &state);
        assert!(example.is_none(), "Should not collect without response");
    }

    #[tokio::test]
    async fn test_run_agent_with_training_collection() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default().with_collect_training(true);
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        // Training example should be collected from successful run
        assert!(
            result.training_example.is_some(),
            "Expected training example"
        );

        let example = result.training_example.unwrap();
        assert_eq!(example.user_input, "Hello");
        assert!(!example.agent_output.is_empty());
    }

    #[tokio::test]
    async fn test_run_agent_without_training_collection() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default(); // collect_training = false by default
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(
            result.training_example.is_none(),
            "Should not collect training by default"
        );
    }

    // System prompt tests
    #[test]
    fn test_runner_config_system_prompt_explicit() {
        let config = RunnerConfig::default().with_system_prompt("Custom system prompt");
        assert_eq!(
            config.system_prompt,
            Some("Custom system prompt".to_string())
        );

        let resolved = config.resolve_system_prompt();
        assert_eq!(resolved, Some("Custom system prompt".to_string()));
    }

    #[test]
    fn test_runner_config_system_prompt_none_by_default() {
        let config = RunnerConfig::default();
        assert!(config.system_prompt.is_none());
        assert!(!config.load_optimized_prompts);

        // Should return None when no explicit prompt and load_optimized_prompts is false
        let resolved = config.resolve_system_prompt();
        assert!(resolved.is_none());
    }

    #[test]
    fn test_runner_config_with_load_optimized_prompts() {
        let config = RunnerConfig::default().with_load_optimized_prompts(true);
        assert!(config.load_optimized_prompts);
    }

    #[tokio::test]
    async fn test_run_agent_with_custom_system_prompt() {
        let custom_prompt = "You are a specialized test assistant.";
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default().with_system_prompt(custom_prompt);
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();

        // The system prompt should have been set on the state
        // After reasoning node runs, the first message should be the system prompt
        // Note: The graph manifest (AI self-awareness) may be appended to the prompt
        let first_system = result
            .state
            .messages
            .iter()
            .find(|m| matches!(m.role, crate::state::MessageRole::System));
        assert!(first_system.is_some(), "Expected system message");
        assert!(
            first_system.unwrap().content.starts_with(custom_prompt),
            "System message should start with custom prompt"
        );
    }

    #[tokio::test]
    async fn test_run_agent_state_prompt_takes_precedence() {
        let state_prompt = "State-level prompt takes precedence";
        let config_prompt = "Config-level prompt";

        let mut state = AgentState::new()
            .with_mock_llm()
            .with_system_prompt(state_prompt);
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default().with_system_prompt(config_prompt);
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();

        // State's system_prompt should be preserved (not overwritten by config)
        assert_eq!(result.state.system_prompt, Some(state_prompt.to_string()));

        // The actual system message should use the state's prompt (before any introspection info)
        // Note: The graph manifest (AI self-awareness) may be appended to the prompt
        let first_system = result
            .state
            .messages
            .iter()
            .find(|m| matches!(m.role, crate::state::MessageRole::System));
        assert!(first_system.is_some());
        assert!(
            first_system.unwrap().content.starts_with(state_prompt),
            "System message should start with state prompt"
        );
    }

    // ============================================
    // RunnerConfig struct tests
    // ============================================

    #[test]
    fn test_runner_config_default() {
        let config = RunnerConfig::default();
        assert!(!config.enable_checkpointing);
        assert!(config.checkpoint_path.is_none());
        assert!(config.postgres_connection_string.is_none());
        assert_eq!(config.max_turns, 0);
        assert!(!config.collect_training);
        assert!(config.system_prompt.is_none());
        assert!(!config.load_optimized_prompts);
        assert!(config.project_doc_options.is_none());
    }

    #[test]
    fn test_runner_config_debug() {
        let config = RunnerConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("RunnerConfig"));
        assert!(debug_str.contains("enable_checkpointing: false"));
        assert!(debug_str.contains("max_turns: 0"));
        assert!(debug_str.contains("has_stream_callback: true"));
    }

    #[test]
    fn test_runner_config_debug_with_postgres() {
        let config = RunnerConfig::with_postgres_checkpointing("host=localhost");
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("postgres_connection_string: true"));
    }

    #[test]
    fn test_runner_config_with_memory_checkpointing() {
        let config = RunnerConfig::with_memory_checkpointing();
        assert!(config.enable_checkpointing);
        assert!(config.checkpoint_path.is_none());
        assert!(config.postgres_connection_string.is_none());
    }

    #[test]
    fn test_runner_config_with_file_checkpointing() {
        let config = RunnerConfig::with_file_checkpointing("/tmp/checkpoints");
        assert!(config.enable_checkpointing);
        assert_eq!(
            config.checkpoint_path,
            Some(std::path::PathBuf::from("/tmp/checkpoints"))
        );
        assert!(config.postgres_connection_string.is_none());
    }

    #[test]
    fn test_runner_config_with_file_checkpointing_pathbuf() {
        let path = std::path::PathBuf::from("/var/data/checkpoints");
        let config = RunnerConfig::with_file_checkpointing(path.clone());
        assert_eq!(config.checkpoint_path, Some(path));
    }

    #[test]
    fn test_runner_config_with_postgres_checkpointing() {
        let config = RunnerConfig::with_postgres_checkpointing(
            "host=localhost user=postgres password=secret dbname=codex",
        );
        assert!(config.enable_checkpointing);
        assert!(config.checkpoint_path.is_none());
        assert_eq!(
            config.postgres_connection_string,
            Some("host=localhost user=postgres password=secret dbname=codex".to_string())
        );
    }

    #[test]
    fn test_runner_config_with_max_turns() {
        let config = RunnerConfig::default().with_max_turns(5);
        assert_eq!(config.max_turns, 5);
    }

    #[test]
    fn test_runner_config_with_max_turns_chaining() {
        let config = RunnerConfig::with_memory_checkpointing()
            .with_max_turns(10)
            .with_collect_training(true);
        assert!(config.enable_checkpointing);
        assert_eq!(config.max_turns, 10);
        assert!(config.collect_training);
    }

    #[test]
    fn test_runner_config_stream_callback() {
        let metrics = Arc::new(MetricsCallback::new());
        let config = RunnerConfig::default().with_stream_callback(metrics.clone());
        let _callback = config.stream_callback();
        // Verify we can get the callback back - it's callable
    }

    #[test]
    fn test_runner_config_with_collect_training_true() {
        let config = RunnerConfig::default().with_collect_training(true);
        assert!(config.collect_training);
    }

    #[test]
    fn test_runner_config_with_collect_training_false() {
        let config = RunnerConfig::default()
            .with_collect_training(true)
            .with_collect_training(false);
        assert!(!config.collect_training);
    }

    // ============================================
    // Checkpoint retention policy tests (Audit #87)
    // ============================================

    #[test]
    fn test_checkpoint_retention_policy_default() {
        let policy = CheckpointRetentionPolicy::default();
        assert_eq!(policy.max_checkpoints_per_thread, 0);
        assert_eq!(policy.max_age_seconds, 0);
        assert_eq!(policy.cleanup_interval, 0);
    }

    #[test]
    fn test_checkpoint_retention_policy_keep_latest() {
        let policy = CheckpointRetentionPolicy::keep_latest(5);
        assert_eq!(policy.max_checkpoints_per_thread, 5);
        assert_eq!(policy.max_age_seconds, 0);
        assert_eq!(policy.cleanup_interval, 1);
    }

    #[test]
    fn test_checkpoint_retention_policy_keep_for_duration() {
        let policy = CheckpointRetentionPolicy::keep_for_duration(3600);
        assert_eq!(policy.max_checkpoints_per_thread, 0);
        assert_eq!(policy.max_age_seconds, 3600);
        assert_eq!(policy.cleanup_interval, 10);
    }

    #[test]
    fn test_checkpoint_retention_policy_with_limits() {
        let policy = CheckpointRetentionPolicy::with_limits(10, 7200);
        assert_eq!(policy.max_checkpoints_per_thread, 10);
        assert_eq!(policy.max_age_seconds, 7200);
        assert_eq!(policy.cleanup_interval, 1);
    }

    #[test]
    fn test_runner_config_with_checkpoint_retention() {
        let policy = CheckpointRetentionPolicy::keep_latest(5);
        let config =
            RunnerConfig::with_file_checkpointing("/tmp/cp").with_checkpoint_retention(policy);
        assert!(config.checkpoint_retention.is_some());
        let retention = config.checkpoint_retention.unwrap();
        assert_eq!(retention.max_checkpoints_per_thread, 5);
    }

    #[test]
    fn test_runner_config_debug_shows_retention() {
        let policy = CheckpointRetentionPolicy::keep_latest(3);
        let config =
            RunnerConfig::with_file_checkpointing("/tmp/cp").with_checkpoint_retention(policy);
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("checkpoint_retention"));
        assert!(debug_str.contains("max_checkpoints_per_thread: 3"));
    }

    #[tokio::test]
    async fn test_cleanup_checkpoints_no_policy() {
        // Without a retention policy, cleanup should be a no-op
        let config = RunnerConfig::with_file_checkpointing("/tmp/no_policy");
        let deleted = cleanup_checkpoints("test-thread", &config).await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_cleanup_checkpoints_zero_limits() {
        // With zero limits (unlimited), cleanup should be a no-op
        let config = RunnerConfig::with_file_checkpointing("/tmp/zero_limits")
            .with_checkpoint_retention(CheckpointRetentionPolicy::default());
        let deleted = cleanup_checkpoints("test-thread", &config).await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_runner_config_with_system_prompt_string() {
        let config = RunnerConfig::default().with_system_prompt("Test prompt");
        assert_eq!(config.system_prompt, Some("Test prompt".to_string()));
    }

    #[test]
    fn test_runner_config_with_system_prompt_owned_string() {
        let prompt = String::from("Owned prompt");
        let config = RunnerConfig::default().with_system_prompt(prompt);
        assert_eq!(config.system_prompt, Some("Owned prompt".to_string()));
    }

    #[test]
    fn test_runner_config_with_load_optimized_prompts_true() {
        let config = RunnerConfig::default().with_load_optimized_prompts(true);
        assert!(config.load_optimized_prompts);
    }

    #[test]
    fn test_runner_config_with_load_optimized_prompts_false() {
        let config = RunnerConfig::default()
            .with_load_optimized_prompts(true)
            .with_load_optimized_prompts(false);
        assert!(!config.load_optimized_prompts);
    }

    #[test]
    fn test_runner_config_with_project_doc_options() {
        let options = ProjectDocOptions::new(std::path::PathBuf::from("/test/dir"));
        let config = RunnerConfig::default().with_project_doc_options(options);
        assert!(config.project_doc_options.is_some());
    }

    #[test]
    fn test_runner_config_with_project_docs() {
        let config = RunnerConfig::default().with_project_docs("/test/working/dir");
        assert!(config.project_doc_options.is_some());
        let options = config.project_doc_options.unwrap();
        assert_eq!(options.cwd, std::path::PathBuf::from("/test/working/dir"));
    }

    #[test]
    fn test_runner_config_with_project_docs_pathbuf() {
        let path = std::path::PathBuf::from("/another/test/dir");
        let config = RunnerConfig::default().with_project_docs(path.clone());
        assert!(config.project_doc_options.is_some());
        assert_eq!(config.project_doc_options.unwrap().cwd, path);
    }

    #[test]
    fn test_runner_config_resolve_system_prompt_explicit() {
        let config = RunnerConfig::default().with_system_prompt("Explicit prompt");
        let resolved = config.resolve_system_prompt();
        assert_eq!(resolved, Some("Explicit prompt".to_string()));
    }

    #[test]
    fn test_runner_config_resolve_system_prompt_none() {
        let config = RunnerConfig::default();
        let resolved = config.resolve_system_prompt();
        assert!(resolved.is_none());
    }

    #[test]
    fn test_runner_config_resolve_system_prompt_explicit_takes_precedence() {
        // Even with load_optimized_prompts, explicit takes precedence
        let config = RunnerConfig::default()
            .with_load_optimized_prompts(true)
            .with_system_prompt("Explicit wins");
        let resolved = config.resolve_system_prompt();
        assert_eq!(resolved, Some("Explicit wins".to_string()));
    }

    #[tokio::test]
    async fn test_runner_config_load_project_docs_none_when_disabled() {
        let config = RunnerConfig::default();
        let docs = config.load_project_docs().await;
        assert!(docs.is_none());
    }

    #[tokio::test]
    async fn test_runner_config_resolve_full_system_prompt_none() {
        let config = RunnerConfig::default();
        let prompt = config.resolve_full_system_prompt().await;
        assert!(prompt.is_none());
    }

    #[tokio::test]
    async fn test_runner_config_resolve_full_system_prompt_explicit_only() {
        let config = RunnerConfig::default().with_system_prompt("Base prompt");
        let prompt = config.resolve_full_system_prompt().await;
        assert_eq!(prompt, Some("Base prompt".to_string()));
    }

    // ============================================
    // AgentResult tests
    // ============================================

    #[test]
    fn test_agent_result_debug() {
        let result = AgentResult {
            state: AgentState::new(),
            thread_id: "test-thread".to_string(),
            turns: 3,
            training_example: None,
            execution_metrics: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("AgentResult"));
        assert!(debug_str.contains("test-thread"));
        assert!(debug_str.contains("turns: 3"));
    }

    #[test]
    fn test_agent_result_clone() {
        let result = AgentResult {
            state: AgentState::new(),
            thread_id: "thread-123".to_string(),
            turns: 5,
            training_example: None,
            execution_metrics: None,
        };
        let cloned = result.clone();
        assert_eq!(cloned.thread_id, "thread-123");
        assert_eq!(cloned.turns, 5);
        assert!(cloned.training_example.is_none());
        assert!(cloned.execution_metrics.is_none());
    }

    #[test]
    fn test_agent_result_with_training_example() {
        let example = crate::optimize::TrainingExample::new("input", "output".to_string(), 0.9);
        let result = AgentResult {
            state: AgentState::new(),
            thread_id: "thread".to_string(),
            turns: 1,
            training_example: Some(example),
            execution_metrics: None,
        };
        assert!(result.training_example.is_some());
        let ex = result.training_example.unwrap();
        assert_eq!(ex.user_input, "input");
        assert_eq!(ex.agent_output, "output");
    }

    #[test]
    fn test_agent_result_with_execution_metrics() {
        use std::time::Duration;

        let mut metrics = dashflow::ExecutionMetrics::new();
        metrics.total_duration = Duration::from_millis(500);

        let result = AgentResult {
            state: AgentState::new(),
            thread_id: "thread-metrics".to_string(),
            turns: 2,
            training_example: None,
            execution_metrics: Some(metrics),
        };

        assert!(result.execution_metrics.is_some());
        let m = result.execution_metrics.unwrap();
        assert_eq!(m.total_duration, Duration::from_millis(500));
    }

    // ============================================
    // calculate_run_score additional tests
    // ============================================

    #[test]
    fn test_calculate_run_score_no_tools_needed() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.turn_count = 1;
        state.max_turns = 10;
        // No tool_results - empty vec

        let score = calculate_run_score(&state);
        // Base (0.5) + no tools full credit (0.2) + turn efficiency (0.18) + completion (0.1)
        assert!(
            score > 0.9,
            "Expected high score for no-tools run, got {}",
            score
        );
    }

    #[test]
    fn test_calculate_run_score_turn_limit_reached() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::TurnLimitReached;
        state.turn_count = 10;
        state.max_turns = 10;

        let score = calculate_run_score(&state);
        // Base (0.5) + no tools (0.2) + turn efficiency (0.0) + no completion bonus
        assert!(
            (0.5..0.8).contains(&score),
            "Expected medium score for turn limit, got {}",
            score
        );
    }

    #[test]
    fn test_calculate_run_score_interrupted() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Interrupted;
        state.turn_count = 2;
        state.max_turns = 10;

        let score = calculate_run_score(&state);
        // Base (0.5) + no tools (0.2) + turn efficiency (0.16) + no completion bonus
        assert!(
            (0.7..0.95).contains(&score),
            "Expected medium-high score for interrupted, got {}",
            score
        );
    }

    #[test]
    fn test_calculate_run_score_in_progress() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::InProgress;
        state.turn_count = 3;
        state.max_turns = 10;

        let score = calculate_run_score(&state);
        // InProgress doesn't match Error, so gets base score
        assert!(score >= 0.5, "Expected at least base score, got {}", score);
    }

    #[test]
    fn test_calculate_run_score_zero_max_turns() {
        // Audit #30: When max_turns=0 (unlimited), give full credit for turn efficiency
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.turn_count = 3;
        state.max_turns = 0; // Unlimited - full credit for turn efficiency

        let score = calculate_run_score(&state);
        // Base (0.5) + tools (0.2) + full efficiency (0.2) + completion (0.1) = 1.0
        assert!(
            score > 0.95,
            "Expected high score for unlimited mode, got {}",
            score
        );
    }

    #[test]
    fn test_calculate_run_score_many_turns() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.turn_count = 20; // More than baseline
        state.max_turns = 10;

        let score = calculate_run_score(&state);
        // Turn efficiency should be 0 (clamped) when exceeding baseline
        // Base (0.5) + tools (0.2) + efficiency (0.0) + completion (0.1) = 0.8
        assert!(
            (0.7..0.9).contains(&score),
            "Expected score around 0.8 for many turns, got {}",
            score
        );
    }

    #[test]
    fn test_calculate_run_score_all_tools_failed() {
        use crate::state::ToolResult;

        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.turn_count = 1;
        state.max_turns = 10;
        state.tool_results = vec![
            ToolResult {
                tool_call_id: "1".to_string(),
                tool: "shell".to_string(),
                output: "failed".to_string(),
                success: false,
                duration_ms: 100,
            },
            ToolResult {
                tool_call_id: "2".to_string(),
                tool: "shell".to_string(),
                output: "also failed".to_string(),
                success: false,
                duration_ms: 50,
            },
        ];

        let score = calculate_run_score(&state);
        // Base (0.5) + tools (0.0 - all failed) + efficiency (0.18) + completion (0.1)
        assert!(
            (0.7..0.85).contains(&score),
            "Expected reduced score for failed tools, got {}",
            score
        );
    }

    #[test]
    fn test_calculate_run_score_clamped_to_one() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.turn_count = 0; // Immediate completion - max efficiency
        state.max_turns = 10;

        let score = calculate_run_score(&state);
        assert!(
            score <= 1.0,
            "Score should be clamped to 1.0, got {}",
            score
        );
    }

    #[test]
    fn test_calculate_run_score_clamped_to_zero() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Error("error".to_string());
        state.turn_count = 100;
        state.max_turns = 10;
        // Even with bad conditions, score shouldn't go negative

        let score = calculate_run_score(&state);
        assert!(
            score >= 0.0,
            "Score should be clamped to 0.0, got {}",
            score
        );
    }

    // ============================================
    // collect_training_example additional tests
    // ============================================

    #[test]
    fn test_collect_training_example_turn_limit_status() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::TurnLimitReached;
        state.last_response = Some("Partial result from turn limit".to_string());

        let example = collect_training_example("Test input", &state);
        assert!(
            example.is_some(),
            "Should collect from TurnLimitReached status"
        );
    }

    #[test]
    fn test_collect_training_example_interrupted_status() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Interrupted;
        state.last_response = Some("Interrupted result".to_string());

        let example = collect_training_example("Test input", &state);
        assert!(
            example.is_none(),
            "Should not collect from Interrupted status"
        );
    }

    #[test]
    fn test_collect_training_example_in_progress_status() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::InProgress;
        state.last_response = Some("In progress result".to_string());

        let example = collect_training_example("Test input", &state);
        assert!(
            example.is_none(),
            "Should not collect from InProgress status"
        );
    }

    #[test]
    fn test_collect_training_example_empty_response() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.last_response = Some(String::new());

        let example = collect_training_example("Test input", &state);
        assert!(example.is_none(), "Should not collect with empty response");
    }

    #[test]
    fn test_collect_training_example_with_tool_calls() {
        use crate::state::ToolResult;

        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.last_response = Some("Result with tools".to_string());
        state.tool_results = vec![
            ToolResult {
                tool_call_id: "1".to_string(),
                tool: "shell".to_string(),
                output: "output1".to_string(),
                success: true,
                duration_ms: 100,
            },
            ToolResult {
                tool_call_id: "2".to_string(),
                tool: "file_read".to_string(),
                output: "output2".to_string(),
                success: true,
                duration_ms: 50,
            },
        ];

        let example = collect_training_example("Test with tools", &state);
        assert!(example.is_some());

        let example = example.unwrap();
        assert_eq!(example.tool_calls, vec!["shell", "file_read"]);
    }

    #[test]
    fn test_collect_training_example_preserves_input() {
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        state.last_response = Some("Agent response".to_string());

        let user_input = "Complex multi-line\nuser input with special chars: !@#$%";
        let example = collect_training_example(user_input, &state);
        assert!(example.is_some());
        assert_eq!(example.unwrap().user_input, user_input);
    }

    // ============================================
    // run_turn tests
    // ============================================

    #[tokio::test]
    async fn test_run_turn_adds_user_message() {
        let state = AgentState::new().with_mock_llm();
        let config = RunnerConfig::default();

        let result = run_turn(state, "User's message here", &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        // Should have user message in messages
        let has_user_msg = result.state.messages.iter().any(|m| {
            matches!(m.role, crate::state::MessageRole::User)
                && m.content.contains("User's message here")
        });
        assert!(has_user_msg, "User message should be in state");
    }

    #[tokio::test]
    async fn test_run_turn_with_streaming() {
        let metrics = Arc::new(MetricsCallback::new());
        let state = AgentState::new().with_mock_llm();
        let config = RunnerConfig::default().with_stream_callback(metrics.clone());

        let result = run_turn(state, "Hello from turn", &config).await;

        assert!(result.is_ok());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let events = metrics.events();
        assert!(!events.is_empty());
    }

    // ============================================
    // run_agent with max_turns tests
    // ============================================

    #[tokio::test]
    async fn test_run_agent_respects_max_turns() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default().with_max_turns(3);
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        // max_turns should be set on the state
        assert_eq!(result.state.max_turns, 3);
    }

    #[tokio::test]
    async fn test_run_agent_state_max_turns_overridden() {
        let mut state = AgentState::new().with_mock_llm();
        state.max_turns = 100; // High initial value
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default().with_max_turns(5);
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        // Config's max_turns should override state's
        assert_eq!(result.state.max_turns, 5);
    }

    #[tokio::test]
    async fn test_run_agent_zero_max_turns_means_unlimited() {
        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::default().with_max_turns(0);
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
        // 0 means unlimited, so it shouldn't limit execution
    }

    // ============================================
    // File checkpointing tests
    // ============================================

    #[tokio::test]
    async fn test_run_agent_with_file_checkpointing() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let checkpoint_path = temp_dir.path().join("checkpoints");

        let mut state = AgentState::new().with_mock_llm();
        state.messages.push(Message::user("Hello"));

        let config = RunnerConfig::with_file_checkpointing(&checkpoint_path);
        let result = run_agent(state, &config).await;

        assert!(result.is_ok());
    }

    // ============================================
    // Complex builder chain tests
    // ============================================

    #[test]
    fn test_runner_config_full_builder_chain() {
        let metrics = Arc::new(MetricsCallback::new());
        let config = RunnerConfig::with_memory_checkpointing()
            .with_max_turns(10)
            .with_stream_callback(metrics)
            .with_collect_training(true)
            .with_system_prompt("Custom prompt")
            .with_load_optimized_prompts(true)
            .with_project_docs("/test/dir");

        assert!(config.enable_checkpointing);
        assert!(config.checkpoint_path.is_none());
        assert_eq!(config.max_turns, 10);
        assert!(config.collect_training);
        assert_eq!(config.system_prompt, Some("Custom prompt".to_string()));
        assert!(config.load_optimized_prompts);
        assert!(config.project_doc_options.is_some());
    }

    #[test]
    fn test_runner_config_file_builder_chain() {
        let config = RunnerConfig::with_file_checkpointing("/data/cp")
            .with_max_turns(20)
            .with_collect_training(false);

        assert!(config.enable_checkpointing);
        assert_eq!(
            config.checkpoint_path,
            Some(std::path::PathBuf::from("/data/cp"))
        );
        assert_eq!(config.max_turns, 20);
        assert!(!config.collect_training);
    }

    #[test]
    fn test_runner_config_postgres_builder_chain() {
        let config = RunnerConfig::with_postgres_checkpointing("host=db user=app")
            .with_max_turns(15)
            .with_system_prompt("DB backed agent");

        assert!(config.enable_checkpointing);
        assert!(config.checkpoint_path.is_none());
        assert_eq!(
            config.postgres_connection_string,
            Some("host=db user=app".to_string())
        );
        assert_eq!(config.max_turns, 15);
        assert_eq!(config.system_prompt, Some("DB backed agent".to_string()));
    }

    #[tokio::test]
    async fn test_get_latest_session_no_checkpointing() {
        // Without checkpointing enabled, should return error
        let config = RunnerConfig::default();
        let result = get_latest_session(&config).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("checkpointing is not enabled"));
    }

    #[tokio::test]
    async fn test_get_latest_session_memory_checkpointing() {
        // Memory checkpointing cannot persist sessions, should return error
        let config = RunnerConfig::with_memory_checkpointing();
        let result = get_latest_session(&config).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("memory checkpointer"));
    }

    #[tokio::test]
    async fn test_get_latest_session_empty_file_storage() {
        // With file checkpointing but no sessions, should return None
        let temp_dir = tempfile::tempdir().unwrap();
        let config = RunnerConfig::with_file_checkpointing(temp_dir.path());
        let result = get_latest_session(&config).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
