//! Agent state for DashFlow StateGraph
//!
//! This module defines the core state struct that flows through the agent graph.
//! The state is designed to be compatible with DashFlow's StateGraph requirements.

use dashflow::core::messages::IntoLlmMessage;
use dashflow::introspection::{ExecutionContext, GraphManifest};
use dashflow::quality::QualityGateConfig;
use dashflow::MergeableState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use codex_dashflow_mcp::McpClient;
use codex_dashflow_sandbox::SandboxMode;

use async_trait::async_trait;

use crate::codex::ApprovalDecision;
use crate::execpolicy::ExecPolicy;
use crate::llm::{LlmConfig, TokenUsage};
use crate::streaming::{NullStreamCallback, StreamCallback};

/// Callback trait for requesting interactive tool approval
///
/// Implementations of this trait are used by the tool execution node to
/// request user approval before executing tools that require it according
/// to the execution policy.
///
/// The TUI provides an implementation that shows an approval overlay.
/// Non-interactive modes can provide auto-approve or auto-reject implementations.
#[async_trait]
pub trait ApprovalCallback: Send + Sync {
    /// Request approval for a tool call
    ///
    /// # Arguments
    /// * `request_id` - Unique ID for tracking this request
    /// * `tool_call_id` - ID of the tool call
    /// * `tool` - Name of the tool
    /// * `args` - Tool arguments as JSON
    /// * `reason` - Optional reason why approval is needed
    ///
    /// # Returns
    /// The approval decision from the user
    async fn request_approval(
        &self,
        request_id: &str,
        tool_call_id: &str,
        tool: &str,
        args: &serde_json::Value,
        reason: Option<&str>,
    ) -> ApprovalDecision;

    /// Check if a tool is already approved for this session
    async fn is_session_approved(&self, tool: &str) -> bool;

    /// Mark a tool as session-approved
    async fn mark_session_approved(&self, tool: &str);
}

/// Null implementation that auto-approves all tool calls
///
/// Used for non-interactive modes like exec mode.
pub struct AutoApproveCallback;

#[async_trait]
impl ApprovalCallback for AutoApproveCallback {
    async fn request_approval(
        &self,
        _request_id: &str,
        _tool_call_id: &str,
        _tool: &str,
        _args: &serde_json::Value,
        _reason: Option<&str>,
    ) -> ApprovalDecision {
        ApprovalDecision::Approve
    }

    async fn is_session_approved(&self, _tool: &str) -> bool {
        true
    }

    async fn mark_session_approved(&self, _tool: &str) {}
}

/// Implementation that auto-rejects all tool calls requiring approval
///
/// Used when running in strict non-interactive mode.
pub struct AutoRejectCallback;

#[async_trait]
impl ApprovalCallback for AutoRejectCallback {
    async fn request_approval(
        &self,
        _request_id: &str,
        _tool_call_id: &str,
        _tool: &str,
        _args: &serde_json::Value,
        _reason: Option<&str>,
    ) -> ApprovalDecision {
        ApprovalDecision::Deny
    }

    async fn is_session_approved(&self, _tool: &str) -> bool {
        false
    }

    async fn mark_session_approved(&self, _tool: &str) {}
}

/// Role of a message in the conversation
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// User message
    User,
    /// Assistant (agent) message
    Assistant,
    /// System message
    System,
    /// Tool result message
    Tool,
}

/// A message in the conversation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message
    pub content: String,
    /// Optional tool call ID (for tool results)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool calls made by the assistant (for assistant messages with tool calls)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<ToolCall>,
}

impl Message {
    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    /// Create a new assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    /// Create a new assistant message with tool calls
    pub fn assistant_with_tool_calls(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.unwrap_or_default(),
            tool_call_id: None,
            tool_calls,
        }
    }

    /// Create a new system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    /// Create a new tool result message
    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: Vec::new(),
        }
    }

    /// Check if this assistant message has tool calls
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

/// Implement DashFlow's IntoLlmMessage trait for codex Message
///
/// This enables using codex's Message type directly with DashFlow's
/// ContextManager token counting APIs like `count_llm_message_tokens()`.
impl IntoLlmMessage for Message {
    fn role(&self) -> &str {
        match self.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
        }
    }

    fn content(&self) -> &str {
        &self.content
    }

    fn tool_calls(&self) -> Option<&[dashflow::core::messages::ToolCall]> {
        // Cannot directly return codex's ToolCall as DashFlow's ToolCall
        // because they have different field names (tool vs name).
        // Return None since the ContextManager can still count tokens
        // from the content field.
        None
    }

    fn tool_call_id(&self) -> Option<&str> {
        self.tool_call_id.as_deref()
    }
}

/// A tool call requested by the LLM
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call
    pub id: String,
    /// Name of the tool to execute
    pub tool: String,
    /// Arguments for the tool (JSON)
    pub args: serde_json::Value,
}

impl ToolCall {
    /// Create a new tool call
    pub fn new(tool: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tool: tool.into(),
            args,
        }
    }
}

/// Result from executing a tool
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    /// ID of the tool call this result corresponds to
    pub tool_call_id: String,
    /// Name of the tool that was executed
    pub tool: String,
    /// Output from the tool (may be truncated)
    pub output: String,
    /// Whether the tool execution succeeded
    pub success: bool,
    /// Duration of execution in milliseconds
    pub duration_ms: u64,
}

/// Completion status for the agent
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionStatus {
    /// Still processing
    #[default]
    InProgress,
    /// Agent completed successfully
    Complete,
    /// Agent hit turn limit
    TurnLimitReached,
    /// User interrupted
    Interrupted,
    /// Error occurred
    Error(String),
}

/// Main agent state for the DashFlow StateGraph
///
/// This state flows through all nodes in the agent graph and contains
/// the full conversation context, pending tool calls, and session metadata.
#[derive(Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Conversation messages
    pub messages: Vec<Message>,

    /// Tool calls pending approval/execution
    pub pending_tool_calls: Vec<ToolCall>,

    /// Results from executed tools
    pub tool_results: Vec<ToolResult>,

    /// Unique session identifier
    pub session_id: String,

    /// Number of agent turns completed
    pub turn_count: u32,

    /// Maximum turns allowed (0 = unlimited)
    pub max_turns: u32,

    /// Completion status
    pub status: CompletionStatus,

    /// Working directory for file operations
    pub working_directory: String,

    /// Last assistant response (for display)
    pub last_response: Option<String>,

    /// Custom system prompt (loaded from PromptRegistry or user-specified)
    /// If None, the default system prompt is used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// LLM configuration
    pub llm_config: LlmConfig,

    /// Whether to use mock LLM (for testing)
    #[serde(default)]
    pub use_mock_llm: bool,

    /// Stream callback for telemetry events (not serialized)
    #[serde(skip)]
    stream_callback: Option<Arc<dyn StreamCallback>>,

    /// MCP client for executing MCP tools (not serialized)
    #[serde(skip)]
    mcp_client: Option<Arc<McpClient>>,

    /// Execution policy for tool approval (not serialized - recreated from config)
    #[serde(skip)]
    exec_policy: Option<Arc<ExecPolicy>>,

    /// Approval callback for interactive tool approval (not serialized)
    #[serde(skip)]
    approval_callback: Option<Arc<dyn ApprovalCallback>>,

    /// Sandbox mode for command execution
    #[serde(default)]
    pub sandbox_mode: SandboxMode,

    /// Additional writable roots for sandbox (Audit #70)
    /// Directories listed here will be writable in WorkspaceWrite mode,
    /// in addition to the working directory.
    #[serde(default)]
    pub sandbox_writable_roots: Vec<PathBuf>,

    /// Audit #60: Tool execution timeout in seconds
    #[serde(default = "default_tool_timeout_secs")]
    pub tool_timeout_secs: u64,

    /// Audit #39: Accumulated token usage across all LLM calls in this session
    /// Tracks total input tokens, output tokens, and cached tokens
    #[serde(default)]
    pub total_input_tokens: u32,
    #[serde(default)]
    pub total_output_tokens: u32,
    #[serde(default)]
    pub total_cached_tokens: u32,

    /// Audit #77: Accumulated cost in USD across all LLM calls in this session
    #[serde(default)]
    pub total_cost_usd: f64,

    /// AI Introspection: Graph manifest describing agent structure
    /// Enables AI to answer "What am I? What nodes do I have? What can I do?"
    #[serde(skip)]
    pub graph_manifest: Option<Arc<GraphManifest>>,

    /// AI Introspection: Current execution context
    /// Enables AI to know "Where am I? What have I done? Am I near limits?"
    #[serde(skip)]
    pub execution_context: Option<ExecutionContext>,

    /// Quality gate configuration for validating LLM outputs
    ///
    /// When set, the reasoning node will use this configuration to validate
    /// LLM responses using the QualityGate retry mechanism. This ensures
    /// response quality meets a specified threshold before accepting.
    #[serde(skip)]
    pub quality_gate_config: Option<QualityGateConfig>,

    /// Enable LLM-as-judge quality scoring
    ///
    /// When set to true along with quality_gate_config, the reasoning node will
    /// use an LLM (via DashFlow's MultiDimensionalJudge) to evaluate response
    /// quality instead of heuristic scoring. This provides more accurate quality
    /// assessment at the cost of additional LLM API calls.
    ///
    /// The judge model is configured via `llm_judge_model`. If not set,
    /// defaults to "gpt-4o-mini" for cost efficiency.
    ///
    /// Requires the `llm-judge` feature to be enabled.
    #[serde(default)]
    pub use_llm_judge: bool,

    /// Model to use for LLM-as-judge quality scoring
    ///
    /// Defaults to "gpt-4o-mini" for cost efficiency. Can be set to "gpt-4o"
    /// for higher quality judgments at increased cost.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_judge_model: Option<String>,
}

/// Default tool timeout in seconds
fn default_tool_timeout_secs() -> u64 {
    60
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            pending_tool_calls: Vec::new(),
            tool_results: Vec::new(),
            session_id: Uuid::new_v4().to_string(),
            turn_count: 0,
            max_turns: 0,
            status: CompletionStatus::InProgress,
            working_directory: String::new(),
            last_response: None,
            system_prompt: None,
            llm_config: LlmConfig::default(),
            use_mock_llm: false,
            stream_callback: None,
            mcp_client: None,
            exec_policy: None,
            approval_callback: None,
            sandbox_mode: SandboxMode::default(),
            sandbox_writable_roots: Vec::new(),
            tool_timeout_secs: default_tool_timeout_secs(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cached_tokens: 0,
            total_cost_usd: 0.0,
            graph_manifest: None,
            execution_context: None,
            quality_gate_config: None,
            use_llm_judge: false,
            llm_judge_model: None,
        }
    }
}

// Manual Debug implementation since Arc<dyn StreamCallback> doesn't impl Debug
impl std::fmt::Debug for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentState")
            .field("messages", &self.messages)
            .field("pending_tool_calls", &self.pending_tool_calls)
            .field("tool_results", &self.tool_results)
            .field("session_id", &self.session_id)
            .field("turn_count", &self.turn_count)
            .field("max_turns", &self.max_turns)
            .field("status", &self.status)
            .field("working_directory", &self.working_directory)
            .field("last_response", &self.last_response)
            .field("system_prompt", &self.system_prompt.is_some())
            .field("llm_config", &self.llm_config)
            .field("use_mock_llm", &self.use_mock_llm)
            .field("stream_callback", &self.stream_callback.is_some())
            .field("mcp_client", &self.mcp_client.is_some())
            .field("exec_policy", &self.exec_policy.is_some())
            .field("approval_callback", &self.approval_callback.is_some())
            .field("sandbox_mode", &self.sandbox_mode)
            .field("tool_timeout_secs", &self.tool_timeout_secs)
            .field("total_input_tokens", &self.total_input_tokens)
            .field("total_output_tokens", &self.total_output_tokens)
            .field("total_cached_tokens", &self.total_cached_tokens)
            .field("total_cost_usd", &self.total_cost_usd)
            .field("graph_manifest", &self.graph_manifest.is_some())
            .field("execution_context", &self.execution_context.is_some())
            .field("quality_gate_config", &self.quality_gate_config.is_some())
            .field("use_llm_judge", &self.use_llm_judge)
            .field("llm_judge_model", &self.llm_judge_model)
            .finish()
    }
}

impl AgentState {
    /// Create a new agent state with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new agent state with a specific session ID
    pub fn with_session_id(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Default::default()
        }
    }

    /// Add a user message to the conversation
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::user(content));
    }

    /// Add an assistant message to the conversation
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        let content = content.into();
        self.last_response = Some(content.clone());
        self.messages.push(Message::assistant(content));
    }

    /// Add a system message to the conversation
    pub fn add_system_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::system(content));
    }

    /// Check if there are pending tool calls
    pub fn has_pending_tool_calls(&self) -> bool {
        !self.pending_tool_calls.is_empty()
    }

    /// Check if the agent should continue processing
    pub fn should_continue(&self) -> bool {
        matches!(self.status, CompletionStatus::InProgress)
            && (self.max_turns == 0 || self.turn_count < self.max_turns)
    }

    /// Mark the agent as complete
    pub fn mark_complete(&mut self) {
        self.status = CompletionStatus::Complete;
    }

    /// Mark the agent as having hit the turn limit
    pub fn mark_turn_limit_reached(&mut self) {
        self.status = CompletionStatus::TurnLimitReached;
    }

    /// Audit #39: Accumulate token usage from an LLM call
    ///
    /// Adds the token counts from a single LLM call to the session totals.
    /// Call this after each LLM call to track total usage for the session.
    pub fn accumulate_token_usage(&mut self, usage: &TokenUsage) {
        self.total_input_tokens = self.total_input_tokens.saturating_add(usage.prompt_tokens);
        self.total_output_tokens = self
            .total_output_tokens
            .saturating_add(usage.completion_tokens);
        self.total_cached_tokens = self.total_cached_tokens.saturating_add(usage.cached_tokens);
    }

    /// Audit #77: Accumulate cost from an LLM call
    ///
    /// Adds the estimated cost from a single LLM call to the session total.
    /// Call this after each LLM call to track total cost for the session.
    pub fn accumulate_cost(&mut self, cost_usd: Option<f64>) {
        if let Some(cost) = cost_usd {
            self.total_cost_usd += cost;
        }
    }

    /// Get total cost in USD for this session
    pub fn total_cost(&self) -> f64 {
        self.total_cost_usd
    }

    /// Get total token usage for this session
    ///
    /// Returns a TokenUsage struct with the accumulated totals.
    pub fn total_token_usage(&self) -> TokenUsage {
        TokenUsage {
            prompt_tokens: self.total_input_tokens,
            completion_tokens: self.total_output_tokens,
            total_tokens: self
                .total_input_tokens
                .saturating_add(self.total_output_tokens),
            cached_tokens: self.total_cached_tokens,
        }
    }

    /// Set the LLM configuration
    pub fn with_llm_config(mut self, config: LlmConfig) -> Self {
        self.llm_config = config;
        self
    }

    /// Set the LLM model
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.llm_config.model = model.into();
        self
    }

    /// Enable mock LLM mode (for testing)
    pub fn with_mock_llm(mut self) -> Self {
        self.use_mock_llm = true;
        self
    }

    /// Set the working directory
    pub fn with_working_directory(mut self, dir: impl Into<String>) -> Self {
        self.working_directory = dir.into();
        self
    }

    /// Set a custom system prompt (overrides default)
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Get the system prompt (returns custom if set, otherwise default)
    pub fn get_system_prompt(&self) -> &str {
        self.system_prompt
            .as_deref()
            .unwrap_or(crate::optimize::DEFAULT_SYSTEM_PROMPT)
    }

    /// Set the stream callback for telemetry events
    pub fn with_stream_callback(mut self, callback: Arc<dyn StreamCallback>) -> Self {
        self.stream_callback = Some(callback);
        self
    }

    /// Get the stream callback
    pub fn stream_callback(&self) -> Arc<dyn StreamCallback> {
        self.stream_callback
            .clone()
            .unwrap_or_else(|| Arc::new(NullStreamCallback))
    }

    /// Check if streaming is enabled
    pub fn has_stream_callback(&self) -> bool {
        self.stream_callback.is_some()
    }

    /// Emit a stream event (fire and forget)
    pub fn emit_event(&self, event: crate::streaming::AgentEvent) {
        let callback = self.stream_callback();
        tokio::spawn(async move {
            callback.on_event(event).await;
        });
    }

    /// Set the MCP client for executing MCP tools
    pub fn with_mcp_client(mut self, client: Arc<McpClient>) -> Self {
        self.mcp_client = Some(client);
        self
    }

    /// Get the MCP client (if configured)
    pub fn mcp_client(&self) -> Option<Arc<McpClient>> {
        self.mcp_client.clone()
    }

    /// Check if MCP client is configured
    pub fn has_mcp_client(&self) -> bool {
        self.mcp_client.is_some()
    }

    /// Set the execution policy for tool approval
    pub fn with_exec_policy(mut self, policy: Arc<ExecPolicy>) -> Self {
        self.exec_policy = Some(policy);
        self
    }

    /// Set sandbox mode for command execution
    pub fn with_sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.sandbox_mode = mode;
        self
    }

    /// Audit #60: Set the tool execution timeout in seconds
    pub fn with_tool_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.tool_timeout_secs = timeout_secs;
        self
    }

    /// Get the execution policy (returns default with dangerous patterns if not configured)
    pub fn exec_policy(&self) -> Arc<ExecPolicy> {
        self.exec_policy
            .clone()
            .unwrap_or_else(|| Arc::new(ExecPolicy::with_dangerous_patterns()))
    }

    /// Check if a custom execution policy is configured
    pub fn has_exec_policy(&self) -> bool {
        self.exec_policy.is_some()
    }

    /// Set the approval callback for interactive tool approval
    pub fn with_approval_callback(mut self, callback: Arc<dyn ApprovalCallback>) -> Self {
        self.approval_callback = Some(callback);
        self
    }

    /// Get the approval callback (returns AutoApproveCallback if not configured)
    pub fn approval_callback(&self) -> Arc<dyn ApprovalCallback> {
        self.approval_callback
            .clone()
            .unwrap_or_else(|| Arc::new(AutoApproveCallback))
    }

    /// Check if an approval callback is configured
    pub fn has_approval_callback(&self) -> bool {
        self.approval_callback.is_some()
    }

    // ========================================================================
    // AI Introspection Methods
    // ========================================================================

    /// Set the graph manifest for AI introspection
    ///
    /// The graph manifest describes the agent's structure, including:
    /// - All nodes and their types
    /// - All edges and routing logic
    /// - Entry point and terminal nodes
    ///
    /// This enables the AI to answer questions like:
    /// - "What am I?"
    /// - "What nodes do I have?"
    /// - "How does my workflow work?"
    #[must_use]
    pub fn with_graph_manifest(mut self, manifest: Arc<GraphManifest>) -> Self {
        self.graph_manifest = Some(manifest);
        self
    }

    /// Set the execution context for AI introspection
    ///
    /// The execution context describes the current runtime state:
    /// - Current node being executed
    /// - Iteration count
    /// - Nodes already executed
    /// - Thread ID and timing
    ///
    /// This enables the AI to answer questions like:
    /// - "Where am I in execution?"
    /// - "How many iterations have I done?"
    /// - "Am I approaching limits?"
    #[must_use]
    pub fn with_execution_context(mut self, context: ExecutionContext) -> Self {
        self.execution_context = Some(context);
        self
    }

    /// Update the execution context in place
    pub fn set_execution_context(&mut self, context: ExecutionContext) {
        self.execution_context = Some(context);
    }

    /// Get the graph manifest as JSON for AI consumption
    ///
    /// Returns None if no manifest is set.
    pub fn introspection_json(&self) -> Option<String> {
        self.graph_manifest.as_ref().and_then(|m| m.to_json().ok())
    }

    /// Get the execution context as JSON for AI consumption
    ///
    /// Returns None if no context is set.
    pub fn execution_context_json(&self) -> Option<String> {
        self.execution_context
            .as_ref()
            .and_then(|c| c.to_json().ok())
    }

    /// Get complete introspection info combining manifest and context
    ///
    /// Returns a JSON object with both graph structure and execution state.
    pub fn full_introspection_json(&self) -> String {
        let manifest = self
            .introspection_json()
            .unwrap_or_else(|| "null".to_string());
        let context = self
            .execution_context_json()
            .unwrap_or_else(|| "null".to_string());
        format!(
            r#"{{"graph_manifest": {}, "execution_context": {}}}"#,
            manifest, context
        )
    }

    /// Check if introspection is available
    pub fn has_introspection(&self) -> bool {
        self.graph_manifest.is_some()
    }

    /// Set the quality gate configuration for LLM output validation
    ///
    /// When enabled, the reasoning node will use a QualityGate to validate
    /// LLM responses against a quality threshold. Responses that don't meet
    /// the threshold will be retried automatically up to the configured limit.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::quality::QualityGateConfig;
    ///
    /// let state = AgentState::new()
    ///     .with_quality_gate(QualityGateConfig {
    ///         threshold: 0.90,
    ///         max_retries: 3,
    ///         ..Default::default()
    ///     });
    /// ```
    #[must_use]
    pub fn with_quality_gate(mut self, config: QualityGateConfig) -> Self {
        self.quality_gate_config = Some(config);
        self
    }

    /// Check if quality gate validation is enabled
    pub fn has_quality_gate(&self) -> bool {
        self.quality_gate_config.is_some()
    }

    /// Enable LLM-as-judge quality scoring
    ///
    /// When enabled alongside quality gate configuration, the reasoning node
    /// will use DashFlow's MultiDimensionalJudge to evaluate response quality
    /// using an LLM instead of heuristic scoring. This provides more accurate
    /// quality assessment across 6 dimensions (accuracy, relevance, completeness,
    /// safety, coherence, conciseness) at the cost of additional LLM API calls.
    ///
    /// Requires the `llm-judge` feature to be enabled.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::quality::QualityGateConfig;
    ///
    /// let state = AgentState::new()
    ///     .with_quality_gate(QualityGateConfig {
    ///         threshold: 0.80,
    ///         max_retries: 2,
    ///         ..Default::default()
    ///     })
    ///     .with_llm_judge("gpt-4o-mini");
    /// ```
    #[must_use]
    pub fn with_llm_judge(mut self, model: impl Into<String>) -> Self {
        self.use_llm_judge = true;
        self.llm_judge_model = Some(model.into());
        self
    }

    /// Enable LLM-as-judge with the default model (gpt-4o-mini)
    #[must_use]
    pub fn with_default_llm_judge(mut self) -> Self {
        self.use_llm_judge = true;
        self.llm_judge_model = None; // Will default to gpt-4o-mini
        self
    }

    /// Check if LLM-as-judge is enabled
    pub fn has_llm_judge(&self) -> bool {
        self.use_llm_judge
    }

    /// Get the LLM judge model (defaults to gpt-4o-mini)
    pub fn llm_judge_model(&self) -> &str {
        self.llm_judge_model.as_deref().unwrap_or("gpt-4o-mini")
    }
}

/// Implement MergeableState for DashFlow parallel edge support
impl MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        // Merge messages (append non-duplicates)
        // Audit #29: Use tool_call_id for deduplication when present,
        // otherwise fall back to role+content deduplication.
        // This preserves tool_call IDs and ordering for tool messages.
        for msg in &other.messages {
            let is_duplicate = self.messages.iter().any(|m| {
                // If both have tool_call_id, compare by that
                if let (Some(id1), Some(id2)) = (&m.tool_call_id, &msg.tool_call_id) {
                    return id1 == id2;
                }
                // Otherwise compare by role and content
                m.content == msg.content && m.role == msg.role
            });
            if !is_duplicate {
                self.messages.push(msg.clone());
            }
        }

        // Merge pending tool calls
        for tc in &other.pending_tool_calls {
            if !self.pending_tool_calls.iter().any(|t| t.id == tc.id) {
                self.pending_tool_calls.push(tc.clone());
            }
        }

        // Merge tool results
        for tr in &other.tool_results {
            if !self
                .tool_results
                .iter()
                .any(|t| t.tool_call_id == tr.tool_call_id)
            {
                self.tool_results.push(tr.clone());
            }
        }

        // Take the higher turn count
        self.turn_count = self.turn_count.max(other.turn_count);

        // Merge status (prefer non-InProgress)
        if self.status == CompletionStatus::InProgress
            && other.status != CompletionStatus::InProgress
        {
            self.status = other.status.clone();
        }

        // Take latest response
        if other.last_response.is_some() {
            self.last_response = other.last_response.clone();
        }

        // Preserve stream callback from other if we don't have one
        if self.stream_callback.is_none() && other.stream_callback.is_some() {
            self.stream_callback = other.stream_callback.clone();
        }

        // Preserve MCP client from other if we don't have one
        if self.mcp_client.is_none() && other.mcp_client.is_some() {
            self.mcp_client = other.mcp_client.clone();
        }

        // Preserve exec policy from other if we don't have one
        if self.exec_policy.is_none() && other.exec_policy.is_some() {
            self.exec_policy = other.exec_policy.clone();
        }

        // Preserve approval callback from other if we don't have one
        if self.approval_callback.is_none() && other.approval_callback.is_some() {
            self.approval_callback = other.approval_callback.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state_default() {
        let state = AgentState::new();
        assert!(state.messages.is_empty());
        assert!(state.pending_tool_calls.is_empty());
        assert_eq!(state.turn_count, 0);
        assert!(state.should_continue());
    }

    #[test]
    fn test_message_creation() {
        let user_msg = Message::user("Hello");
        assert_eq!(user_msg.role, MessageRole::User);
        assert_eq!(user_msg.content, "Hello");

        let assistant_msg = Message::assistant("Hi there");
        assert_eq!(assistant_msg.role, MessageRole::Assistant);
    }

    #[test]
    fn test_tool_call_creation() {
        let args = serde_json::json!({"command": "ls -la"});
        let tc = ToolCall::new("shell", args);
        assert_eq!(tc.tool, "shell");
        assert!(!tc.id.is_empty());
    }

    #[test]
    fn test_turn_limit() {
        let mut state = AgentState::new();
        state.max_turns = 5;
        state.turn_count = 4;
        assert!(state.should_continue());

        state.turn_count = 5;
        assert!(!state.should_continue());
    }

    #[test]
    fn test_exec_policy_default() {
        // AgentState without explicit policy should return default dangerous patterns
        let state = AgentState::new();
        assert!(!state.has_exec_policy());

        // exec_policy() should still return a working policy
        let policy = state.exec_policy();
        // read_file should be allowed by default
        let tc = ToolCall::new("read_file", serde_json::json!({"path": "test.txt"}));
        assert!(policy.evaluate(&tc).is_approved());
    }

    #[test]
    fn test_exec_policy_with_custom() {
        let custom_policy = ExecPolicy::permissive();
        let state = AgentState::new().with_exec_policy(Arc::new(custom_policy));

        assert!(state.has_exec_policy());

        // In permissive mode, shell should be auto-approved
        let policy = state.exec_policy();
        let tc = ToolCall::new("shell", serde_json::json!({"command": "ls"}));
        assert!(policy.evaluate(&tc).is_approved());
    }

    #[test]
    fn test_exec_policy_merge() {
        // Test that exec policy is preserved during state merge
        let policy = Arc::new(ExecPolicy::strict());
        let state1 = AgentState::new();
        let state2 = AgentState::new().with_exec_policy(policy);

        let mut merged = state1;
        merged.merge(&state2);

        // Policy from state2 should be preserved
        assert!(merged.has_exec_policy());
    }

    // System prompt tests
    #[test]
    fn test_system_prompt_default() {
        let state = AgentState::new();
        assert!(state.system_prompt.is_none());

        // get_system_prompt should return default
        let prompt = state.get_system_prompt();
        assert!(prompt.contains("coding assistant"));
    }

    #[test]
    fn test_system_prompt_with_custom() {
        let custom = "You are a specialized Rust assistant.";
        let state = AgentState::new().with_system_prompt(custom);

        assert_eq!(state.system_prompt, Some(custom.to_string()));
        assert_eq!(state.get_system_prompt(), custom);
    }

    #[test]
    fn test_system_prompt_serialization() {
        let custom = "Custom prompt for testing";
        let state = AgentState::new().with_system_prompt(custom);

        // Serialize to JSON
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains(custom));

        // Deserialize back
        let deserialized: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.system_prompt, Some(custom.to_string()));
    }

    #[test]
    fn test_system_prompt_none_not_serialized() {
        let state = AgentState::new();
        let json = serde_json::to_string(&state).unwrap();

        // system_prompt should be omitted when None
        assert!(!json.contains("system_prompt"));
    }

    // Approval callback tests
    #[test]
    fn test_approval_callback_default() {
        // AgentState without explicit callback should use AutoApproveCallback
        let state = AgentState::new();
        assert!(!state.has_approval_callback());

        // approval_callback() should still return a working callback
        let _callback = state.approval_callback();
    }

    #[tokio::test]
    async fn test_approval_callback_auto_approve() {
        // AutoApproveCallback should auto-approve all requests
        let callback = AutoApproveCallback;

        let decision = callback
            .request_approval(
                "req-1",
                "call-1",
                "shell",
                &serde_json::json!({"command": "ls"}),
                None,
            )
            .await;

        assert_eq!(decision, crate::codex::ApprovalDecision::Approve);
        assert!(callback.is_session_approved("any_tool").await);
    }

    #[tokio::test]
    async fn test_approval_callback_auto_reject() {
        // AutoRejectCallback should reject all requests
        let callback = AutoRejectCallback;

        let decision = callback
            .request_approval(
                "req-1",
                "call-1",
                "shell",
                &serde_json::json!({"command": "ls"}),
                None,
            )
            .await;

        assert_eq!(decision, crate::codex::ApprovalDecision::Deny);
        assert!(!callback.is_session_approved("any_tool").await);
    }

    #[test]
    fn test_approval_callback_with_custom() {
        let state = AgentState::new().with_approval_callback(Arc::new(AutoRejectCallback));

        assert!(state.has_approval_callback());
    }

    #[test]
    fn test_approval_callback_merge() {
        // Test that approval callback is preserved during state merge
        let state1 = AgentState::new();
        let state2 = AgentState::new().with_approval_callback(Arc::new(AutoRejectCallback));

        let mut merged = state1;
        merged.merge(&state2);

        // Callback from state2 should be preserved
        assert!(merged.has_approval_callback());
    }

    // === MessageRole tests ===

    #[test]
    fn test_message_role_debug() {
        let role = MessageRole::User;
        let debug_str = format!("{:?}", role);
        assert!(debug_str.contains("User"));
    }

    #[test]
    fn test_message_role_clone() {
        let role = MessageRole::Assistant;
        let cloned = role.clone();
        assert_eq!(cloned, MessageRole::Assistant);
    }

    #[test]
    fn test_message_role_partial_eq() {
        assert_eq!(MessageRole::User, MessageRole::User);
        assert_ne!(MessageRole::User, MessageRole::Assistant);
        assert_ne!(MessageRole::System, MessageRole::Tool);
    }

    #[test]
    fn test_message_role_serialization() {
        assert_eq!(
            serde_json::to_string(&MessageRole::User).unwrap(),
            "\"user\""
        );
        assert_eq!(
            serde_json::to_string(&MessageRole::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(
            serde_json::to_string(&MessageRole::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::to_string(&MessageRole::Tool).unwrap(),
            "\"tool\""
        );
    }

    #[test]
    fn test_message_role_deserialization() {
        assert_eq!(
            serde_json::from_str::<MessageRole>("\"user\"").unwrap(),
            MessageRole::User
        );
        assert_eq!(
            serde_json::from_str::<MessageRole>("\"assistant\"").unwrap(),
            MessageRole::Assistant
        );
    }

    // === Message tests ===

    #[test]
    fn test_message_debug() {
        let msg = Message::user("test");
        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("Message"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_message_clone() {
        let msg = Message::user("hello");
        let cloned = msg.clone();
        assert_eq!(cloned.content, "hello");
        assert_eq!(cloned.role, MessageRole::User);
    }

    #[test]
    fn test_message_system() {
        let msg = Message::system("You are an assistant");
        assert_eq!(msg.role, MessageRole::System);
        assert_eq!(msg.content, "You are an assistant");
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn test_message_tool() {
        let msg = Message::tool("result output", "call-123");
        assert_eq!(msg.role, MessageRole::Tool);
        assert_eq!(msg.content, "result output");
        assert_eq!(msg.tool_call_id, Some("call-123".to_string()));
    }

    #[test]
    fn test_message_has_tool_calls() {
        let msg_no_calls = Message::assistant("just text");
        assert!(!msg_no_calls.has_tool_calls());

        let tc = ToolCall::new("test_tool", serde_json::json!({}));
        let msg_with_calls = Message::assistant_with_tool_calls(Some("text".to_string()), vec![tc]);
        assert!(msg_with_calls.has_tool_calls());
    }

    #[test]
    fn test_message_assistant_with_tool_calls_none_content() {
        let tc = ToolCall::new("tool", serde_json::json!({}));
        let msg = Message::assistant_with_tool_calls(None, vec![tc]);
        assert_eq!(msg.content, ""); // None becomes empty string
        assert_eq!(msg.role, MessageRole::Assistant);
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("test content");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("test content"));
    }

    #[test]
    fn test_message_serialization_skips_empty_tool_calls() {
        let msg = Message::assistant("response");
        let json = serde_json::to_string(&msg).unwrap();
        // tool_calls should be omitted when empty
        assert!(!json.contains("tool_calls"));
    }

    #[test]
    fn test_message_serialization_skips_none_tool_call_id() {
        let msg = Message::user("input");
        let json = serde_json::to_string(&msg).unwrap();
        // tool_call_id should be omitted when None
        assert!(!json.contains("tool_call_id"));
    }

    // === ToolCall tests ===

    #[test]
    fn test_tool_call_debug() {
        let tc = ToolCall::new("shell", serde_json::json!({"cmd": "ls"}));
        let debug_str = format!("{:?}", tc);
        assert!(debug_str.contains("ToolCall"));
        assert!(debug_str.contains("shell"));
    }

    #[test]
    fn test_tool_call_clone() {
        let tc = ToolCall::new("read_file", serde_json::json!({"path": "test.txt"}));
        let cloned = tc.clone();
        assert_eq!(cloned.tool, "read_file");
        assert_eq!(cloned.id, tc.id);
    }

    #[test]
    fn test_tool_call_serialization() {
        let tc = ToolCall::new(
            "write_file",
            serde_json::json!({"path": "out.txt", "content": "data"}),
        );
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains("write_file"));
        assert!(json.contains("out.txt"));
    }

    #[test]
    fn test_tool_call_unique_ids() {
        let tc1 = ToolCall::new("tool", serde_json::json!({}));
        let tc2 = ToolCall::new("tool", serde_json::json!({}));
        assert_ne!(tc1.id, tc2.id);
    }

    // === ToolResult tests ===

    #[test]
    fn test_tool_result_debug() {
        let tr = ToolResult {
            tool_call_id: "call-1".to_string(),
            tool: "shell".to_string(),
            output: "output text".to_string(),
            success: true,
            duration_ms: 100,
        };
        let debug_str = format!("{:?}", tr);
        assert!(debug_str.contains("ToolResult"));
        assert!(debug_str.contains("shell"));
    }

    #[test]
    fn test_tool_result_clone() {
        let tr = ToolResult {
            tool_call_id: "call-2".to_string(),
            tool: "read_file".to_string(),
            output: "file content".to_string(),
            success: true,
            duration_ms: 50,
        };
        let cloned = tr.clone();
        assert_eq!(cloned.tool, "read_file");
        assert_eq!(cloned.duration_ms, 50);
    }

    #[test]
    fn test_tool_result_serialization() {
        let tr = ToolResult {
            tool_call_id: "call-3".to_string(),
            tool: "test".to_string(),
            output: "result".to_string(),
            success: false,
            duration_ms: 200,
        };
        let json = serde_json::to_string(&tr).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"duration_ms\":200"));
    }

    // === CompletionStatus tests ===

    #[test]
    fn test_completion_status_default() {
        let status = CompletionStatus::default();
        assert_eq!(status, CompletionStatus::InProgress);
    }

    #[test]
    fn test_completion_status_debug() {
        let status = CompletionStatus::Complete;
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("Complete"));
    }

    #[test]
    fn test_completion_status_clone() {
        let status = CompletionStatus::Error("test error".to_string());
        let cloned = status.clone();
        assert_eq!(cloned, CompletionStatus::Error("test error".to_string()));
    }

    #[test]
    fn test_completion_status_partial_eq() {
        assert_eq!(CompletionStatus::InProgress, CompletionStatus::InProgress);
        assert_ne!(CompletionStatus::Complete, CompletionStatus::InProgress);
        assert_ne!(
            CompletionStatus::TurnLimitReached,
            CompletionStatus::Interrupted
        );
    }

    #[test]
    fn test_completion_status_error_variant() {
        let error = CompletionStatus::Error("Something went wrong".to_string());
        if let CompletionStatus::Error(msg) = error {
            assert_eq!(msg, "Something went wrong");
        } else {
            panic!("Expected Error variant");
        }
    }

    #[test]
    fn test_completion_status_serialization() {
        assert!(serde_json::to_string(&CompletionStatus::InProgress)
            .unwrap()
            .contains("InProgress"));
        assert!(serde_json::to_string(&CompletionStatus::Complete)
            .unwrap()
            .contains("Complete"));
    }

    // === AgentState additional tests ===

    #[test]
    fn test_agent_state_debug() {
        let state = AgentState::new();
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("AgentState"));
        assert!(debug_str.contains("session_id"));
    }

    #[test]
    fn test_agent_state_clone() {
        let mut state = AgentState::new();
        state.add_user_message("hello");
        let cloned = state.clone();
        assert_eq!(cloned.messages.len(), 1);
        assert_eq!(cloned.session_id, state.session_id);
    }

    #[test]
    fn test_agent_state_with_session_id() {
        let state = AgentState::with_session_id("custom-session-123");
        assert_eq!(state.session_id, "custom-session-123");
    }

    #[test]
    fn test_agent_state_add_user_message() {
        let mut state = AgentState::new();
        state.add_user_message("Hello, agent!");
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, MessageRole::User);
        assert_eq!(state.messages[0].content, "Hello, agent!");
    }

    #[test]
    fn test_agent_state_add_assistant_message() {
        let mut state = AgentState::new();
        state.add_assistant_message("I can help with that.");
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, MessageRole::Assistant);
        assert_eq!(
            state.last_response,
            Some("I can help with that.".to_string())
        );
    }

    #[test]
    fn test_agent_state_add_system_message() {
        let mut state = AgentState::new();
        state.add_system_message("You are a helpful assistant.");
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, MessageRole::System);
    }

    #[test]
    fn test_agent_state_has_pending_tool_calls() {
        let mut state = AgentState::new();
        assert!(!state.has_pending_tool_calls());

        state
            .pending_tool_calls
            .push(ToolCall::new("test", serde_json::json!({})));
        assert!(state.has_pending_tool_calls());
    }

    #[test]
    fn test_agent_state_mark_complete() {
        let mut state = AgentState::new();
        assert!(state.should_continue());

        state.mark_complete();
        assert_eq!(state.status, CompletionStatus::Complete);
        assert!(!state.should_continue());
    }

    #[test]
    fn test_agent_state_mark_turn_limit_reached() {
        let mut state = AgentState::new();
        state.mark_turn_limit_reached();
        assert_eq!(state.status, CompletionStatus::TurnLimitReached);
        assert!(!state.should_continue());
    }

    #[test]
    fn test_agent_state_with_model() {
        let state = AgentState::new().with_model("gpt-4-turbo");
        assert_eq!(state.llm_config.model, "gpt-4-turbo");
    }

    #[test]
    fn test_agent_state_with_mock_llm() {
        let state = AgentState::new().with_mock_llm();
        assert!(state.use_mock_llm);
    }

    #[test]
    fn test_agent_state_with_working_directory() {
        let state = AgentState::new().with_working_directory("/home/user/project");
        assert_eq!(state.working_directory, "/home/user/project");
    }

    #[test]
    fn test_agent_state_unlimited_turns() {
        let mut state = AgentState::new();
        state.max_turns = 0; // 0 means unlimited
        state.turn_count = 1000;
        assert!(state.should_continue());
    }

    #[test]
    fn test_agent_state_serialization() {
        let state = AgentState::new()
            .with_working_directory("/tmp")
            .with_model("test-model");
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("/tmp"));
        assert!(json.contains("test-model"));
    }

    #[test]
    fn test_agent_state_deserialization() {
        let state = AgentState::new()
            .with_working_directory("/data")
            .with_system_prompt("Be helpful");
        let json = serde_json::to_string(&state).unwrap();
        let restored: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.working_directory, "/data");
        assert_eq!(restored.system_prompt, Some("Be helpful".to_string()));
    }

    // === MergeableState tests ===

    #[test]
    fn test_merge_messages() {
        let mut state1 = AgentState::new();
        state1.add_user_message("msg1");

        let mut state2 = AgentState::new();
        state2.add_user_message("msg2");
        state2.add_assistant_message("response");

        state1.merge(&state2);

        // Should have all 3 messages
        assert_eq!(state1.messages.len(), 3);
    }

    #[test]
    fn test_merge_deduplicates_messages() {
        let mut state1 = AgentState::new();
        state1.add_user_message("duplicate");

        let mut state2 = AgentState::new();
        state2.add_user_message("duplicate");

        state1.merge(&state2);

        // Should still have only 1 message
        assert_eq!(state1.messages.len(), 1);
    }

    #[test]
    fn test_merge_turn_count() {
        let mut state1 = AgentState::new();
        state1.turn_count = 3;

        let mut state2 = AgentState::new();
        state2.turn_count = 5;

        state1.merge(&state2);

        // Should take higher count
        assert_eq!(state1.turn_count, 5);
    }

    #[test]
    fn test_merge_status_prefers_non_in_progress() {
        let mut state1 = AgentState::new();
        state1.status = CompletionStatus::InProgress;

        let mut state2 = AgentState::new();
        state2.status = CompletionStatus::Complete;

        state1.merge(&state2);

        // Should take Complete over InProgress
        assert_eq!(state1.status, CompletionStatus::Complete);
    }

    #[test]
    fn test_merge_tool_calls() {
        let mut state1 = AgentState::new();
        state1.pending_tool_calls.push(ToolCall {
            id: "call-1".to_string(),
            tool: "tool1".to_string(),
            args: serde_json::json!({}),
        });

        let mut state2 = AgentState::new();
        state2.pending_tool_calls.push(ToolCall {
            id: "call-2".to_string(),
            tool: "tool2".to_string(),
            args: serde_json::json!({}),
        });

        state1.merge(&state2);

        assert_eq!(state1.pending_tool_calls.len(), 2);
    }

    #[test]
    fn test_merge_tool_results() {
        let mut state1 = AgentState::new();
        state1.tool_results.push(ToolResult {
            tool_call_id: "call-1".to_string(),
            tool: "tool1".to_string(),
            output: "out1".to_string(),
            success: true,
            duration_ms: 100,
        });

        let mut state2 = AgentState::new();
        state2.tool_results.push(ToolResult {
            tool_call_id: "call-2".to_string(),
            tool: "tool2".to_string(),
            output: "out2".to_string(),
            success: true,
            duration_ms: 200,
        });

        state1.merge(&state2);

        assert_eq!(state1.tool_results.len(), 2);
    }

    #[test]
    fn test_merge_last_response() {
        let mut state1 = AgentState::new();
        state1.last_response = Some("old response".to_string());

        let mut state2 = AgentState::new();
        state2.last_response = Some("new response".to_string());

        state1.merge(&state2);

        // Should take other's response
        assert_eq!(state1.last_response, Some("new response".to_string()));
    }

    #[test]
    fn test_merge_deduplicates_tool_messages_by_tool_call_id() {
        // Audit #29: Test that tool messages are deduplicated by tool_call_id,
        // not just by role+content (which loses tool_call_id information).
        let mut state1 = AgentState::new();
        state1.messages.push(Message::tool("output", "call-123"));

        let mut state2 = AgentState::new();
        // Same content but different tool_call_id = NOT a duplicate
        state2.messages.push(Message::tool("output", "call-456"));

        state1.merge(&state2);

        // Should have both messages (different tool_call_ids)
        assert_eq!(state1.messages.len(), 2);
    }

    #[test]
    fn test_merge_deduplicates_same_tool_call_id() {
        // Audit #29: Same tool_call_id = duplicate
        let mut state1 = AgentState::new();
        state1.messages.push(Message::tool("output", "call-123"));

        let mut state2 = AgentState::new();
        state2.messages.push(Message::tool("output", "call-123"));

        state1.merge(&state2);

        // Should have only 1 message (same tool_call_id)
        assert_eq!(state1.messages.len(), 1);
    }

    #[test]
    fn test_merge_mixed_messages_with_and_without_tool_call_id() {
        // Audit #29: Messages without tool_call_id should still use role+content
        let mut state1 = AgentState::new();
        state1.add_user_message("hello");
        state1.messages.push(Message::tool("output1", "call-1"));

        let mut state2 = AgentState::new();
        state2.add_user_message("hello"); // Same content as state1
        state2.messages.push(Message::tool("output2", "call-2")); // Different id

        state1.merge(&state2);

        // user "hello" is deduplicated by role+content
        // tool messages have different ids so both kept
        assert_eq!(state1.messages.len(), 3);
    }

    // --- Quality gate configuration tests ---

    #[test]
    fn test_with_quality_gate() {
        let config = QualityGateConfig {
            threshold: 0.90,
            max_retries: 3,
            ..Default::default()
        };

        let state = AgentState::new().with_quality_gate(config.clone());

        assert!(state.has_quality_gate());
        assert!(state.quality_gate_config.is_some());

        let qg_config = state.quality_gate_config.unwrap();
        assert!((qg_config.threshold - 0.90).abs() < f32::EPSILON);
        assert_eq!(qg_config.max_retries, 3);
    }

    #[test]
    fn test_default_no_quality_gate() {
        let state = AgentState::new();
        assert!(!state.has_quality_gate());
        assert!(state.quality_gate_config.is_none());
    }

    #[test]
    fn test_quality_gate_debug_format() {
        let config = QualityGateConfig::default();
        let state = AgentState::new().with_quality_gate(config);

        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("quality_gate_config: true"));
    }

    #[test]
    fn test_quality_gate_not_serialized() {
        let config = QualityGateConfig::default();
        let state = AgentState::new().with_quality_gate(config);

        // quality_gate_config has #[serde(skip)] so shouldn't be in JSON
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("quality_gate_config"));
    }

    // --- LLM-as-judge configuration tests ---

    #[test]
    fn test_with_llm_judge() {
        let state = AgentState::new().with_llm_judge("gpt-4o");

        assert!(state.use_llm_judge);
        assert!(state.has_llm_judge());
        assert_eq!(state.llm_judge_model(), "gpt-4o");
        assert_eq!(state.llm_judge_model, Some("gpt-4o".to_string()));
    }

    #[test]
    fn test_with_default_llm_judge() {
        let state = AgentState::new().with_default_llm_judge();

        assert!(state.use_llm_judge);
        assert!(state.has_llm_judge());
        assert_eq!(state.llm_judge_model(), "gpt-4o-mini"); // default
        assert!(state.llm_judge_model.is_none()); // explicitly None, defaults in getter
    }

    #[test]
    fn test_llm_judge_disabled_by_default() {
        let state = AgentState::new();

        assert!(!state.use_llm_judge);
        assert!(!state.has_llm_judge());
        // llm_judge_model() returns default even when not enabled
        assert_eq!(state.llm_judge_model(), "gpt-4o-mini");
    }

    #[test]
    fn test_llm_judge_with_quality_gate_combined() {
        let config = QualityGateConfig {
            threshold: 0.85,
            max_retries: 3,
            ..Default::default()
        };

        let state = AgentState::new()
            .with_quality_gate(config)
            .with_llm_judge("gpt-4o");

        assert!(state.has_quality_gate());
        assert!(state.has_llm_judge());
        assert_eq!(state.llm_judge_model(), "gpt-4o");

        let qg_config = state.quality_gate_config.as_ref().unwrap();
        assert!((qg_config.threshold - 0.85).abs() < f32::EPSILON);
        assert_eq!(qg_config.max_retries, 3);
    }

    #[test]
    fn test_llm_judge_in_debug() {
        let state = AgentState::new().with_llm_judge("gpt-4o");

        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("use_llm_judge: true"));
        assert!(debug_str.contains("llm_judge_model: Some(\"gpt-4o\")"));
    }

    #[test]
    fn test_llm_judge_serialization() {
        // use_llm_judge is serialized, llm_judge_model is optional
        let state = AgentState::new().with_llm_judge("gpt-4o");

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"use_llm_judge\":true"));
        assert!(json.contains("\"llm_judge_model\":\"gpt-4o\""));
    }

    #[test]
    fn test_llm_judge_default_serialization() {
        // When llm_judge_model is None, it shouldn't appear in JSON
        let state = AgentState::new().with_default_llm_judge();

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"use_llm_judge\":true"));
        // llm_judge_model: None is skipped due to skip_serializing_if
        assert!(!json.contains("llm_judge_model"));
    }

    #[test]
    fn test_llm_judge_roundtrip_serialization() {
        let original = AgentState::new().with_llm_judge("gpt-4o");

        let json = serde_json::to_string(&original).unwrap();
        let restored: AgentState = serde_json::from_str(&json).unwrap();

        assert!(restored.use_llm_judge);
        assert_eq!(restored.llm_judge_model, Some("gpt-4o".to_string()));
    }
}
